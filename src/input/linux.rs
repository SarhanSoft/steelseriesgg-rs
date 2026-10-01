//! Linux I/O layer: read SteelSeries evdev devices, grab them with `EVIOCGRAB` while bindings
//! need them, run events through the [`Remapper`] and re-emit everything through one uinput
//! virtual keyboard+mouse.
//!
//! Safety model: every grab lives in an open `evdev::Device`. Releasing it happens on every exit
//! path: explicit ungrab on stop, error and escape; `Drop` during unwinding; and, if the process
//! dies, the kernel closing the file descriptor. The uinput device is destroyed the same way, and
//! the kernel releases any key still held on it.

use std::collections::BTreeSet;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId, KeyCode, RelativeAxisCode};
use rustix::event::{PollFd, PollFlags, Timespec};
use tracing::{debug, info, warn};

use super::binding::{BindingSet, MAX_MACRO_STEPS, MacroStep};
use super::engine::{
    DeviceFilter, EngineNotice, EngineOptions, StopReason, VIRTUAL_DEVICE_NAME, device_needs_grab, virtual_key_allowed,
};
use super::keys::{InputKey, KEY_MAX};
use super::record::{RecordedEvent, steps_from_recording};
use super::remapper::{Effect, EscapeChord, KeyEvent, KeyValue, Remapper, RemapperConfig};
use crate::error::{Error, Result};

/// Longest wait before the engine thread checks for control messages.
const CONTROL_POLL: Duration = Duration::from_millis(20);
/// Retry delay for a device that still had keys held when it should have been grabbed.
const GRAB_RETRY: Duration = Duration::from_millis(50);
/// Most key events one recording keeps (each becomes a step, plus delays).
const MAX_RECORDED_EVENTS: usize = MAX_MACRO_STEPS / 2;
const SYN_REPORT: u16 = 0;
/// Hi-res wheel units per detent.
const HIRES_PER_DETENT: i32 = 120;

enum Control {
    Update(BindingSet),
    Stop,
}

/// Handle to the engine thread.
#[derive(Debug)]
pub(super) struct LinuxEngine {
    control: Sender<Control>,
    notices: Receiver<EngineNotice>,
    thread: Option<JoinHandle<Result<()>>>,
    running: Arc<AtomicBool>,
}

impl LinuxEngine {
    pub(super) fn start(bindings: BindingSet, options: EngineOptions) -> Result<Self> {
        bindings.validate()?;
        let (control, control_rx) = mpsc::channel();
        let (notice_tx, notices) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let thread = std::thread::Builder::new()
            .name("ssgg-input".to_string())
            .spawn(move || engine_thread(bindings, options, control_rx, notice_tx, ready_tx, thread_running))?;

        let ready: std::result::Result<Result<()>, mpsc::RecvError> = ready_rx.recv();
        match ready {
            Ok(Ok(())) => Ok(Self {
                control,
                notices,
                thread: Some(thread),
                running,
            }),
            Ok(Err(e)) => {
                if thread.join().is_err() {
                    warn!("input engine thread panicked after a failed start");
                }
                Err(e)
            }
            Err(_) => match thread.join() {
                Ok(Err(e)) => Err(e),
                Ok(Ok(())) => Err(Error::Other("input engine stopped during start-up".to_string())),
                Err(_) => Err(Error::Other("input engine thread panicked during start-up".to_string())),
            },
        }
    }

    pub(super) fn update(&self, bindings: BindingSet) -> Result<()> {
        bindings.validate()?;
        self.control
            .send(Control::Update(bindings))
            .map_err(|_| Error::Other("input engine is not running".to_string()))
    }

    pub(super) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub(super) fn notices(&self) -> &Receiver<EngineNotice> {
        &self.notices
    }

    pub(super) fn stop(mut self) -> Result<()> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        if self.control.send(Control::Stop).is_err() {
            debug!("input engine thread had already exited");
        }
        thread
            .join()
            .map_err(|_| Error::Other("input engine thread panicked".to_string()))?
    }
}

impl Drop for LinuxEngine {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            warn!("input engine stopped with an error: {e}");
        }
    }
}

fn notify(tx: &Sender<EngineNotice>, notice: EngineNotice) {
    if tx.send(notice).is_err() {
        debug!("input engine notice dropped: no receiver");
    }
}

/// Clears the running flag when the engine thread ends, including by panic.
struct RunningFlag(Arc<AtomicBool>);

impl Drop for RunningFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn engine_thread(
    bindings: BindingSet,
    options: EngineOptions,
    control_rx: Receiver<Control>,
    notice_tx: Sender<EngineNotice>,
    ready_tx: SyncSender<Result<()>>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let running = RunningFlag(running);
    let setup = check_input_access().and_then(|()| Engine::new(bindings, &options));
    let mut engine = match setup {
        Ok(engine) => engine,
        Err(e) => {
            drop(running);
            // The error goes to `start()`; if it is no longer waiting, return it from the thread.
            return match ready_tx.send(Err(e)) {
                Ok(()) => Ok(()),
                Err(mpsc::SendError(unsent)) => unsent,
            };
        }
    };
    if ready_tx.send(Ok(())).is_err() {
        return Ok(());
    }

    let result = engine.run(&control_rx, &notice_tx);
    engine.shutdown();
    drop(running);
    let reason = match &result {
        Ok(reason) => reason.clone(),
        Err(e) => StopReason::Error(e.to_string()),
    };
    match &reason {
        StopReason::Requested => info!("input engine stopped"),
        StopReason::EmergencyEscape => warn!("input engine stopped by the emergency escape chord"),
        StopReason::Error(e) => warn!("input engine stopped after an error: {e}"),
    }
    notify(&notice_tx, EngineNotice::Stopped(reason));
    result.map(|_| ())
}

/// Fail early, with a useful message, when no input device can be opened at all.
fn check_input_access() -> Result<()> {
    let visible = std::fs::read_dir("/dev/input")
        .map(|dir| {
            dir.filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("event"))
                .count()
        })
        .unwrap_or(0);
    if visible > 0 && evdev::enumerate().next().is_none() {
        return Err(Error::PermissionDenied(
            "cannot open any /dev/input/event* device; add your user to the `input` group and log in again \
             (see docs/development/input.md)"
                .to_string(),
        ));
    }
    Ok(())
}

fn uinput_error(e: io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Error::PermissionDenied(format!(
            "cannot open /dev/uinput ({e}); install the udev rule from docs/development/input.md and make sure \
             your user is in the `input` group"
        )),
        io::ErrorKind::NotFound => Error::DeviceNotFound(format!(
            "/dev/uinput does not exist ({e}); load the uinput kernel module (`modprobe uinput`)"
        )),
        _ => Error::Io(e),
    }
}

fn is_own_device(device: &Device) -> bool {
    device.name() == Some(VIRTUAL_DEVICE_NAME)
}

fn device_matches(filter: &DeviceFilter, device: &Device) -> bool {
    let id = device.input_id();
    !is_own_device(device) && filter.matches(id.vendor(), id.product(), device.name().unwrap_or(""))
}

fn device_supports(device: &Device, key: InputKey) -> bool {
    key.code() <= KEY_MAX
        && device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::new(key.code())))
}

fn device_wanted(bindings: &BindingSet, device: &Device) -> bool {
    device_needs_grab(bindings, |key| device_supports(device, key))
}

fn device_has_rel(device: &Device, axis: RelativeAxisCode) -> bool {
    device.supported_relative_axes().is_some_and(|axes| axes.contains(axis))
}

fn keys_held(device: &Device) -> io::Result<bool> {
    Ok(device.get_key_state()?.iter().next().is_some())
}

/// Capabilities of the virtual device: every keyboard key and mouse button, plus whatever the
/// matching devices report (minus ranges that would change how the desktop classifies it).
fn virtual_capabilities(filter: &DeviceFilter) -> (AttributeSet<KeyCode>, AttributeSet<RelativeAxisCode>) {
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in (1..=255u16).chain(InputKey::BTN_LEFT.code()..=InputKey::BTN_TASK.code()) {
        keys.insert(KeyCode::new(code));
    }
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    for axis in [
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
    ] {
        axes.insert(axis);
    }
    for (_, device) in evdev::enumerate() {
        if !device_matches(filter, &device) {
            continue;
        }
        if let Some(supported) = device.supported_keys() {
            for key in supported.iter().filter(|k| virtual_key_allowed(k.code())) {
                keys.insert(key);
            }
        }
        if let Some(supported) = device.supported_relative_axes() {
            for axis in supported.iter() {
                axes.insert(axis);
            }
        }
    }
    (keys, axes)
}

fn build_virtual_device(keys: &AttributeSet<KeyCode>, axes: &AttributeSet<RelativeAxisCode>) -> Result<VirtualDevice> {
    let device = VirtualDevice::builder()
        .map_err(uinput_error)?
        .name(VIRTUAL_DEVICE_NAME)
        .input_id(InputId::new(BusType::BUS_VIRTUAL, 0, 0, 1))
        .with_keys(keys)
        .map_err(uinput_error)?
        .with_relative_axes(axes)
        .map_err(uinput_error)?
        .build()
        .map_err(uinput_error)?;
    Ok(device)
}

fn poll_devices<'a>(devices: impl Iterator<Item = &'a Device>, timeout: Duration) -> Result<Vec<(usize, bool)>> {
    let mut fds: Vec<PollFd<'_>> = devices.map(|d| PollFd::new(d, PollFlags::IN)).collect();
    if fds.is_empty() {
        std::thread::sleep(timeout);
        return Ok(Vec::new());
    }
    let timespec = Timespec::try_from(timeout).map_err(|e| Error::Other(format!("invalid poll timeout: {e}")))?;
    match rustix::event::poll(&mut fds, Some(&timespec)) {
        Ok(_) => {}
        Err(rustix::io::Errno::INTR) => return Ok(Vec::new()),
        Err(e) => return Err(Error::Io(io::Error::from(e))),
    }
    Ok(fds
        .iter()
        .enumerate()
        .filter(|(_, fd)| !fd.revents().is_empty())
        .map(|(i, fd)| {
            (
                i,
                fd.revents()
                    .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL),
            )
        })
        .collect())
}

enum Fetched {
    Events(Vec<InputEvent>),
    Nothing,
    Gone,
}

fn fetch(device: &mut Device, path: &Path) -> Result<Fetched> {
    match device.fetch_events() {
        Ok(events) => Ok(Fetched::Events(events.collect())),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(Fetched::Nothing),
        Err(e) if e.raw_os_error() == Some(libc::ENODEV) => Ok(Fetched::Gone),
        Err(e) => Err(Error::DeviceCommunication(format!("reading {}: {e}", path.display()))),
    }
}

struct Grabbed {
    path: PathBuf,
    name: String,
    device: Device,
    hires_wheel: bool,
    hires_hwheel: bool,
}

impl Grabbed {
    fn notice_ids(&self) -> (String, String) {
        (self.path.display().to_string(), self.name.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Escape,
}

struct Engine {
    filter: DeviceFilter,
    rescan_interval: Duration,
    bindings: BindingSet,
    remapper: Remapper,
    virt: VirtualDevice,
    virt_hires_wheel: bool,
    virt_hires_hwheel: bool,
    devices: Vec<Grabbed>,
    next_scan: Instant,
    children: Vec<Child>,
}

impl Engine {
    fn new(bindings: BindingSet, options: &EngineOptions) -> Result<Self> {
        let remapper = Remapper::new(&bindings, options.remapper.clone())?;
        let (keys, axes) = virtual_capabilities(&options.devices);
        let virt = build_virtual_device(&keys, &axes)?;
        info!("created uinput device {VIRTUAL_DEVICE_NAME:?}");
        Ok(Self {
            filter: options.devices.clone(),
            rescan_interval: options.rescan_interval,
            bindings,
            remapper,
            virt,
            virt_hires_wheel: axes.contains(RelativeAxisCode::REL_WHEEL_HI_RES),
            virt_hires_hwheel: axes.contains(RelativeAxisCode::REL_HWHEEL_HI_RES),
            devices: Vec::new(),
            next_scan: Instant::now(),
            children: Vec::new(),
        })
    }

    fn run(&mut self, control_rx: &Receiver<Control>, notice_tx: &Sender<EngineNotice>) -> Result<StopReason> {
        loop {
            loop {
                match control_rx.try_recv() {
                    Ok(Control::Update(bindings)) => {
                        if self.apply_bindings(bindings, notice_tx)? == Flow::Escape {
                            return Ok(StopReason::EmergencyEscape);
                        }
                    }
                    Ok(Control::Stop) | Err(TryRecvError::Disconnected) => return Ok(StopReason::Requested),
                    Err(TryRecvError::Empty) => break,
                }
            }

            let now = Instant::now();
            if now >= self.next_scan {
                self.scan(now, notice_tx)?;
            }
            self.reap_children();

            let mut timeout = CONTROL_POLL.min(self.next_scan.saturating_duration_since(now));
            if let Some(deadline) = self.remapper.next_deadline() {
                timeout = timeout.min(deadline.saturating_duration_since(now));
            }
            let ready = poll_devices(self.devices.iter().map(|g| &g.device), timeout)?;
            // Highest index first, so removing a device keeps the remaining indices valid.
            for (index, hangup) in ready.into_iter().rev() {
                if self.read_device(index, hangup, notice_tx)? == Flow::Escape {
                    return Ok(StopReason::EmergencyEscape);
                }
            }

            let effects = self.remapper.tick(Instant::now());
            if self.apply_effects(effects, notice_tx)? == Flow::Escape {
                return Ok(StopReason::EmergencyEscape);
            }
        }
    }

    fn apply_bindings(&mut self, bindings: BindingSet, notice_tx: &Sender<EngineNotice>) -> Result<Flow> {
        let effects = match self.remapper.set_bindings(&bindings) {
            Ok(effects) => effects,
            Err(e) => {
                warn!("ignoring invalid binding set: {e}");
                return Ok(Flow::Continue);
            }
        };
        self.bindings = bindings;
        let flow = self.apply_effects(effects, notice_tx)?;
        self.scan(Instant::now(), notice_tx)?;
        Ok(flow)
    }

    /// Release devices the bindings no longer need, and grab matching devices that they do.
    fn scan(&mut self, now: Instant, notice_tx: &Sender<EngineNotice>) -> Result<()> {
        self.next_scan = now + self.rescan_interval;

        let keep: Vec<bool> = self
            .devices
            .iter()
            .map(|g| device_wanted(&self.bindings, &g.device))
            .collect();
        if keep.iter().any(|k| !k) {
            let effects = self.remapper.release_all();
            self.apply_effects(effects, notice_tx)?;
            let mut keep = keep.into_iter();
            self.devices.retain_mut(|g| {
                if keep.next().unwrap_or(false) {
                    return true;
                }
                release_device(g, notice_tx);
                false
            });
        }

        if !self.bindings.is_active() {
            return Ok(());
        }
        for (path, mut device) in evdev::enumerate() {
            if self.devices.iter().any(|g| g.path == path)
                || !device_matches(&self.filter, &device)
                || !device_wanted(&self.bindings, &device)
            {
                continue;
            }
            // Grabbing while a key is down would send its release only to us, leaving it stuck
            // down for the rest of the system. Wait until everything on the device is up.
            match keys_held(&device) {
                Ok(false) => {}
                Ok(true) => {
                    debug!("{}: keys held, grab postponed", path.display());
                    self.next_scan = self.next_scan.min(now + GRAB_RETRY);
                    continue;
                }
                Err(e) => {
                    warn!("{}: cannot read key state: {e}", path.display());
                    continue;
                }
            }
            if let Err(e) = device.set_nonblocking(true) {
                warn!("{}: cannot set non-blocking mode: {e}", path.display());
                continue;
            }
            if let Err(e) = device.grab() {
                warn!(
                    "{}: cannot grab ({e}); is another program capturing it?",
                    path.display()
                );
                continue;
            }
            let grabbed = Grabbed {
                name: device.name().unwrap_or("unknown device").to_string(),
                hires_wheel: device_has_rel(&device, RelativeAxisCode::REL_WHEEL_HI_RES),
                hires_hwheel: device_has_rel(&device, RelativeAxisCode::REL_HWHEEL_HI_RES),
                path,
                device,
            };
            info!("grabbed {} ({})", grabbed.name, grabbed.path.display());
            let (path, name) = grabbed.notice_ids();
            notify(notice_tx, EngineNotice::DeviceGrabbed { path, name });
            self.devices.push(grabbed);
        }
        Ok(())
    }

    fn read_device(&mut self, index: usize, hangup: bool, notice_tx: &Sender<EngineNotice>) -> Result<Flow> {
        let Some(grabbed) = self.devices.get_mut(index) else {
            return Ok(Flow::Continue);
        };
        let (hires_wheel, hires_hwheel) = (grabbed.hires_wheel, grabbed.hires_hwheel);
        let events = match fetch(&mut grabbed.device, &grabbed.path)? {
            Fetched::Events(events) => events,
            Fetched::Nothing if !hangup => return Ok(Flow::Continue),
            Fetched::Nothing | Fetched::Gone => {
                self.drop_device(index, notice_tx)?;
                return Ok(Flow::Continue);
            }
        };

        let now = Instant::now();
        let mut frame: Vec<InputEvent> = Vec::new();
        for event in events {
            match event.event_type() {
                EventType::KEY => {
                    let Some(value) = KeyValue::from_evdev(event.value()) else {
                        continue;
                    };
                    let effects = self
                        .remapper
                        .handle(KeyEvent::new(InputKey::from_code(event.code()), value), now);
                    if self.apply_effects(effects, notice_tx)? == Flow::Escape {
                        return Ok(Flow::Escape);
                    }
                }
                EventType::RELATIVE => {
                    frame.push(event);
                    // A device without hi-res scrolling would not scroll through a virtual device
                    // that advertises it, so add the hi-res equivalent.
                    let hires = if event.code() == RelativeAxisCode::REL_WHEEL.0
                        && self.virt_hires_wheel
                        && !hires_wheel
                    {
                        Some(RelativeAxisCode::REL_WHEEL_HI_RES)
                    } else if event.code() == RelativeAxisCode::REL_HWHEEL.0 && self.virt_hires_hwheel && !hires_hwheel
                    {
                        Some(RelativeAxisCode::REL_HWHEEL_HI_RES)
                    } else {
                        None
                    };
                    if let Some(axis) = hires {
                        frame.push(InputEvent::new(
                            EventType::RELATIVE.0,
                            axis.0,
                            event.value().saturating_mul(HIRES_PER_DETENT),
                        ));
                    }
                }
                EventType::SYNCHRONIZATION if event.code() == SYN_REPORT => self.flush(&mut frame)?,
                _ => {}
            }
        }
        self.flush(&mut frame)?;
        Ok(Flow::Continue)
    }

    fn flush(&mut self, frame: &mut Vec<InputEvent>) -> Result<()> {
        if !frame.is_empty() {
            self.virt.emit(frame)?;
            frame.clear();
        }
        Ok(())
    }

    fn drop_device(&mut self, index: usize, notice_tx: &Sender<EngineNotice>) -> Result<()> {
        let effects = self.remapper.release_all();
        self.apply_effects(effects, notice_tx)?;
        if index < self.devices.len() {
            let mut gone = self.devices.remove(index);
            info!("{} ({}) disconnected", gone.name, gone.path.display());
            release_device(&mut gone, notice_tx);
        }
        Ok(())
    }

    fn apply_effects(&mut self, effects: Vec<Effect>, notice_tx: &Sender<EngineNotice>) -> Result<Flow> {
        let mut flow = Flow::Continue;
        for effect in effects {
            match effect {
                Effect::Key(event) => {
                    self.virt.emit(&[InputEvent::new(
                        EventType::KEY.0,
                        event.key.code(),
                        event.value.to_evdev(),
                    )])?;
                }
                Effect::Launch { command, args } => self.launch(command, args, notice_tx),
                Effect::SwitchProfile(name) => {
                    info!("profile switch requested: {name}");
                    notify(notice_tx, EngineNotice::SwitchProfile(name));
                }
                Effect::EmergencyStop => flow = Flow::Escape,
            }
        }
        Ok(flow)
    }

    fn launch(&mut self, command: String, args: Vec<String>, notice_tx: &Sender<EngineNotice>) {
        let mut cmd = Command::new(&command);
        // Own process group: a Ctrl+C aimed at ssgg does not also kill launched programs.
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        match cmd.spawn() {
            Ok(child) => {
                info!("launched {command}");
                self.children.push(child);
            }
            Err(e) => {
                warn!("cannot launch {command}: {e}");
                notify(
                    notice_tx,
                    EngineNotice::LaunchFailed {
                        command,
                        error: e.to_string(),
                    },
                );
            }
        }
    }

    /// Collect exited launched programs so they do not linger as zombies.
    fn reap_children(&mut self) {
        self.children.retain_mut(|child| matches!(child.try_wait(), Ok(None)));
    }

    /// Release every virtual key and every grab. Safe to call more than once.
    fn shutdown(&mut self) {
        for effect in self.remapper.release_all() {
            if let Effect::Key(event) = effect {
                let ev = InputEvent::new(EventType::KEY.0, event.key.code(), event.value.to_evdev());
                if let Err(e) = self.virt.emit(&[ev]) {
                    warn!("cannot release {} on the virtual device: {e}", event.key);
                    break;
                }
            }
        }
        for mut grabbed in self.devices.drain(..) {
            if let Err(e) = grabbed.device.ungrab() {
                warn!(
                    "cannot ungrab {}: {e} (released when the device closes)",
                    grabbed.path.display()
                );
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn release_device(grabbed: &mut Grabbed, notice_tx: &Sender<EngineNotice>) {
    if let Err(e) = grabbed.device.ungrab() {
        debug!("ungrab {}: {e}", grabbed.path.display());
    }
    info!("released {} ({})", grabbed.name, grabbed.path.display());
    let (path, name) = grabbed.notice_ids();
    notify(notice_tx, EngineNotice::DeviceReleased { path, name });
}

/// Grabs held for the duration of a recording; released on drop.
struct RecordingGrabs(Vec<(PathBuf, Device)>);

impl Drop for RecordingGrabs {
    fn drop(&mut self) {
        for (path, device) in &mut self.0 {
            if let Err(e) = device.ungrab() {
                warn!(
                    "cannot ungrab {}: {e} (released when the device closes)",
                    path.display()
                );
            }
        }
    }
}

pub(super) fn record_macro(filter: &DeviceFilter, stop_key: InputKey, timeout: Duration) -> Result<Vec<MacroStep>> {
    let deadline = Instant::now() + timeout;
    let mut grabs = RecordingGrabs(
        evdev::enumerate()
            .filter(|(_, device)| {
                device_matches(filter, device)
                    && device.supported_keys().is_some_and(|keys| keys.iter().next().is_some())
            })
            .collect(),
    );
    if grabs.0.is_empty() {
        return Err(Error::DeviceNotFound(
            "no input device matches the recording filter (is your user in the `input` group?)".to_string(),
        ));
    }

    for (path, device) in &mut grabs.0 {
        while keys_held(device)? {
            if Instant::now() >= deadline {
                return Err(Error::Other(format!(
                    "keys on {} were still held when the recording timed out",
                    path.display()
                )));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        device.set_nonblocking(true)?;
        device.grab().map_err(|e| {
            if e.raw_os_error() == Some(libc::EBUSY) {
                Error::DeviceCommunication(format!(
                    "{} is captured by another program; stop the input engine before recording",
                    path.display()
                ))
            } else {
                Error::Io(e)
            }
        })?;
    }
    info!("recording a macro; press {stop_key} to finish");

    let mut escape = EscapeChord::new(RemapperConfig::default().escape_hold);
    let mut held = BTreeSet::new();
    let mut recorded = Vec::new();
    let mut first: Option<SystemTime> = None;
    'record: loop {
        let now = Instant::now();
        if now >= deadline {
            info!("macro recording timed out");
            break;
        }
        if escape.check(now) {
            return Err(Error::Other(
                "macro recording aborted by the emergency escape chord".to_string(),
            ));
        }
        let mut wait = deadline.saturating_duration_since(now).min(Duration::from_millis(100));
        if let Some(escape_at) = escape.deadline() {
            wait = wait.min(escape_at.saturating_duration_since(now));
        }
        let ready = poll_devices(grabs.0.iter().map(|(_, d)| d), wait)?;
        for (index, hangup) in ready {
            let Some((path, device)) = grabs.0.get_mut(index) else {
                continue;
            };
            let events = match fetch(device, path)? {
                Fetched::Events(events) => events,
                Fetched::Nothing if !hangup => continue,
                Fetched::Nothing | Fetched::Gone => {
                    return Err(Error::DeviceNotFound(format!(
                        "{} disconnected during recording",
                        path.display()
                    )));
                }
            };
            for event in events.into_iter().filter(|e| e.event_type() == EventType::KEY) {
                let Some(value) = KeyValue::from_evdev(event.value()) else {
                    continue;
                };
                let key = InputKey::from_code(event.code());
                match value {
                    KeyValue::Press => {
                        held.insert(key);
                    }
                    KeyValue::Release => {
                        held.remove(&key);
                    }
                    KeyValue::Repeat => {}
                }
                escape.update(&held, Instant::now());
                if key == stop_key && value == KeyValue::Press {
                    break 'record;
                }
                let timestamp = event.timestamp();
                let start = *first.get_or_insert(timestamp);
                let at = timestamp.duration_since(start).unwrap_or_default();
                recorded.push(RecordedEvent::new(at, key, value));
                if recorded.len() >= MAX_RECORDED_EVENTS {
                    warn!("macro recording reached {MAX_RECORDED_EVENTS} events; stopping");
                    break 'record;
                }
            }
        }
    }
    drop(grabs);
    Ok(steps_from_recording(&recorded, Some(stop_key)))
}
