//! The engine: the single owner of every connected SteelSeries device.
//!
//! Every front end goes through [`Engine::execute`] with a [`Command`]: the CLI (directly, or
//! over the control API when the daemon is running), the web control panel, profiles, and the
//! daemon's own loops (hot-plug, lighting animation, battery polling). Because one owner holds
//! the devices, a colour set from the CLI is no longer overwritten by the daemon's next frame,
//! and a device plugged in later gets its settings and lighting back.

pub mod command;
pub mod control;
pub mod state;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

pub use command::{Command, DeviceSnapshot, EngineSnapshot, ProfileSummary, SettingEntry, SettingsView};
pub use state::{DeviceRecord, EngineState, Lighting};

use crate::devices::headsets::Headset;
use crate::devices::key_mapping::KeyId;
use crate::devices::keyboards::Keyboard;
use crate::devices::mice::Mouse;
use crate::devices::settings::{Configurable, DeviceStatus, SettingDescriptor, SettingKind, SettingValue};
use crate::devices::{DeviceInfo, DeviceManager, DeviceType};
use crate::notify::{self, BatteryWatch};
use crate::profiles::{DeviceProfile, Profile, ProfileManager};
use crate::rgb::{Color, Effect, RgbController};
use crate::{Error, Result};

/// How often the daemon rescans for plugged/unplugged devices.
const HOTPLUG_INTERVAL: Duration = Duration::from_secs(2);
/// Lighting frame period (~30 fps). Unchanged frames are not re-sent.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
/// How often battery / ChatMix readings are refreshed.
const STATUS_INTERVAL: Duration = Duration::from_secs(10);
/// How often dirty state is flushed to disk.
const SAVE_INTERVAL: Duration = Duration::from_secs(3);

/// An opened device of any family.
pub enum DeviceHandle {
    Keyboard(Box<dyn Keyboard>),
    Mouse(Box<dyn Mouse>),
    Headset(Box<dyn Headset>),
}

impl DeviceHandle {
    pub fn kind(&self) -> DeviceType {
        match self {
            DeviceHandle::Keyboard(_) => DeviceType::Keyboard,
            DeviceHandle::Mouse(_) => DeviceType::Mouse,
            DeviceHandle::Headset(_) => DeviceType::Headset,
        }
    }

    /// The uniform settings interface, when this device family provides one.
    pub fn configurable(&mut self) -> Option<&mut dyn Configurable> {
        match self {
            DeviceHandle::Mouse(mouse) => Some(mouse.as_mut()),
            DeviceHandle::Keyboard(_) | DeviceHandle::Headset(_) => None,
        }
    }

    fn descriptors(&self) -> Vec<SettingDescriptor> {
        match self {
            DeviceHandle::Mouse(mouse) => mouse.setting_descriptors(),
            DeviceHandle::Keyboard(_) | DeviceHandle::Headset(_) => Vec::new(),
        }
    }

    /// Number of independently coloured zones the lighting engine drives (0 = no lighting).
    fn lighting_zones(&self) -> usize {
        match self {
            DeviceHandle::Keyboard(keyboard) => keyboard.zone_count().max(1),
            DeviceHandle::Mouse(mouse) => mouse.color_zone_names().len(),
            DeviceHandle::Headset(_) => 0,
        }
    }

    fn supports_per_key(&self) -> bool {
        matches!(self, DeviceHandle::Keyboard(k) if k.supports_per_key_rgb())
    }

    async fn write_zones(&mut self, colors: &[Color]) -> Result<()> {
        match self {
            DeviceHandle::Keyboard(keyboard) => keyboard.set_zone_colors(colors).await,
            DeviceHandle::Mouse(mouse) => mouse.set_zone_colors_direct(colors),
            DeviceHandle::Headset(_) => Ok(()),
        }
    }
}

/// A colour pushed on top of the normal lighting for a limited time (GameSense events,
/// notifications).
#[derive(Clone, Debug)]
pub struct Overlay {
    /// Which devices: a specific device key, or every device of a type.
    pub target: OverlayTarget,
    /// Zone index, or `None` for every zone.
    pub zone: Option<usize>,
    pub color: Color,
    pub expires: Instant,
    /// Flash on/off at this frequency instead of a steady colour.
    pub flash_hz: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OverlayTarget {
    Device(String),
    Kind(DeviceType),
}

struct Slot {
    info: DeviceInfo,
    model_key: String,
    handle: DeviceHandle,
    lighting: Option<RgbController>,
    last_frame: Vec<Color>,
    key_colors: Vec<(KeyId, Color)>,
    status: DeviceStatus,
    status_read_at: Option<Instant>,
    battery_watch: BatteryWatch,
    write_failures: u32,
}

impl Slot {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn capabilities(&self) -> Vec<String> {
        let mut caps = Vec::new();
        if self.lighting.is_some() {
            caps.push("lighting".to_string());
        }
        if self.handle.supports_per_key() {
            caps.push("per_key".to_string());
        }
        if !self.handle.descriptors().is_empty() {
            caps.push("settings".to_string());
        }
        caps
    }

    fn configure_lighting(&mut self, lighting: &Lighting) {
        let zones = self.handle.lighting_zones();
        if zones == 0 {
            self.lighting = None;
            return;
        }
        let controller = self.lighting.get_or_insert_with(|| RgbController::new(zones));
        if controller.effect() != &lighting.effect {
            controller.set_effect(lighting.effect.clone());
        }
        controller.set_brightness(f32::from(lighting.brightness.min(100)) / 100.0);
        // Force the next frame out even if the colours happen to match the last one sent.
        self.last_frame.clear();
    }
}

struct Inner {
    manager: DeviceManager,
    slots: BTreeMap<String, Slot>,
    state: EngineState,
    state_dirty: bool,
    profiles: Option<ProfileManager>,
    overlays: Vec<Overlay>,
    /// Device key -> last open error, so a failing device is reported once, not every scan.
    open_failures: HashMap<String, String>,
    /// Time base for flashing overlays.
    epoch: Instant,
}

/// Handles for the daemon's background loops; dropping it does not stop them, call
/// [`BackgroundTasks::abort`].
pub struct BackgroundTasks {
    handles: Vec<JoinHandle<()>>,
}

impl BackgroundTasks {
    pub fn abort(self) {
        for handle in self.handles {
            handle.abort();
        }
    }
}

pub struct Engine {
    inner: Mutex<Inner>,
    daemon: bool,
    started: Instant,
}

impl Engine {
    /// Open the HID layer, load remembered state and connect every present device.
    ///
    /// `daemon` marks the long-lived engine; a one-shot CLI engine passes `false`.
    pub async fn open(daemon: bool) -> Result<Arc<Self>> {
        let mut manager = DeviceManager::new()?;
        if let Ok(config) = crate::config::Config::load_async().await {
            manager.set_device_override(config.device);
        }
        let profiles = match ProfileManager::new().await {
            Ok(profiles) => Some(profiles),
            Err(e) => {
                warn!("Profiles unavailable: {e}");
                None
            }
        };
        let engine = Arc::new(Self {
            inner: Mutex::new(Inner {
                manager,
                slots: BTreeMap::new(),
                state: EngineState::load(),
                state_dirty: false,
                profiles,
                overlays: Vec::new(),
                open_failures: HashMap::new(),
                epoch: Instant::now(),
            }),
            daemon,
            started: Instant::now(),
        });
        engine.refresh_devices().await?;
        Ok(engine)
    }

    pub fn is_daemon(&self) -> bool {
        self.daemon
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// Rescan the bus: open new devices (restoring their settings and lighting) and drop
    /// unplugged ones.
    pub async fn refresh_devices(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.sync_devices().await
    }

    /// Execute one command and return its JSON result.
    pub async fn execute(&self, command: Command) -> Result<Value> {
        let mut inner = self.inner.lock().await;
        let result = inner.execute(command, self.daemon).await;
        if !self.daemon && inner.state_dirty {
            inner.save_state();
        }
        result
    }

    /// Push a temporary lighting overlay (GameSense, notifications).
    pub async fn push_overlay(&self, overlay: Overlay) {
        let mut inner = self.inner.lock().await;
        inner
            .overlays
            .retain(|o| !(o.target == overlay.target && o.zone == overlay.zone));
        inner.overlays.push(overlay);
    }

    /// Drop every overlay (e.g. a game stopped).
    pub async fn clear_overlays(&self) {
        self.inner.lock().await.overlays.clear();
    }

    /// Write the first lighting frame to every device right away. One-shot CLI engines call
    /// this after a lighting change, since they have no animation loop.
    pub async fn render_once(&self) {
        let mut inner = self.inner.lock().await;
        inner.render_frame(Instant::now()).await;
    }

    /// Persist state now (the daemon also does it periodically and on shutdown).
    pub async fn save(&self) {
        let mut inner = self.inner.lock().await;
        inner.save_state();
    }

    /// Start the daemon loops: hot-plug, lighting animation, status polling, state saving.
    pub fn spawn_background(self: &Arc<Self>) -> BackgroundTasks {
        let mut handles = Vec::new();

        let engine = Arc::clone(self);
        handles.push(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(HOTPLUG_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = engine.refresh_devices().await {
                    debug!("Device rescan failed: {e}");
                }
            }
        }));

        let engine = Arc::clone(self);
        handles.push(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(FRAME_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let mut inner = engine.inner.lock().await;
                inner.render_frame(Instant::now()).await;
            }
        }));

        let engine = Arc::clone(self);
        handles.push(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(STATUS_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let mut inner = engine.inner.lock().await;
                inner.poll_status(true);
            }
        }));

        let engine = Arc::clone(self);
        handles.push(tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SAVE_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let mut inner = engine.inner.lock().await;
                if inner.state_dirty {
                    inner.save_state();
                }
            }
        }));

        BackgroundTasks { handles }
    }

    /// Latest status reading for every device that reports one, keyed by device key.
    pub async fn statuses(&self) -> BTreeMap<String, DeviceStatus> {
        let inner = self.inner.lock().await;
        inner
            .slots
            .iter()
            .filter(|(_, slot)| !slot.status.is_empty())
            .map(|(key, slot)| (key.clone(), slot.status.clone()))
            .collect()
    }

    /// Run `f` with exclusive access to one keyboard (used by OLED and diagnostics code).
    pub async fn with_keyboard<R>(
        &self,
        selector: &str,
        f: impl AsyncFnOnce(&mut dyn Keyboard) -> Result<R>,
    ) -> Result<R> {
        let mut inner = self.inner.lock().await;
        let key = inner.resolve(selector)?;
        let slot = inner
            .slots
            .get_mut(&key)
            .ok_or_else(|| Error::DeviceNotFound(selector.to_string()))?;
        match &mut slot.handle {
            DeviceHandle::Keyboard(keyboard) => f(keyboard.as_mut()).await,
            _ => Err(Error::InvalidConfig(format!("{} is not a keyboard", slot.info.name))),
        }
    }
}

/// Stable identity for a physical device: vendor, product and serial (when present).
fn device_key(info: &DeviceInfo) -> String {
    let serial = info
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("0");
    format!("{:04x}:{:04x}:{}", info.vendor_id, info.product_id, serial)
}

/// Identity of a device model, used by profiles so they apply to any unit of that model.
fn model_key(info: &DeviceInfo) -> String {
    format!("{:04x}:{:04x}", info.vendor_id, info.product_id)
}

fn kind_from_selector(selector: &str) -> Option<DeviceType> {
    match selector.to_ascii_lowercase().as_str() {
        "keyboard" | "kb" => Some(DeviceType::Keyboard),
        "mouse" | "mice" => Some(DeviceType::Mouse),
        "headset" | "headphones" => Some(DeviceType::Headset),
        _ => None,
    }
}

impl Inner {
    async fn sync_devices(&mut self) -> Result<()> {
        self.manager.refresh()?;

        let mut present: Vec<(String, DeviceInfo)> = Vec::new();
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for kind in [DeviceType::Keyboard, DeviceType::Mouse, DeviceType::Headset] {
            for info in self.manager.devices_by_type(kind) {
                let base = device_key(info);
                let mut key = base.clone();
                let mut n = 2;
                while taken.contains(&key) {
                    key = format!("{base}#{n}");
                    n += 1;
                }
                taken.insert(key.clone());
                present.push((key, info.clone()));
            }
        }

        let before = self.slots.len();
        self.slots.retain(|key, slot| {
            let keep = taken.contains(key);
            if !keep {
                info!("Disconnected: {} ({key})", slot.info.name);
            }
            keep
        });
        self.open_failures.retain(|key, _| taken.contains(key));
        let removed = before != self.slots.len();

        for (key, info) in present {
            if self.slots.contains_key(&key) {
                continue;
            }
            match self.open(&info) {
                Ok(handle) => {
                    self.open_failures.remove(&key);
                    let mut slot = Slot {
                        model_key: model_key(&info),
                        info,
                        handle,
                        lighting: None,
                        last_frame: Vec::new(),
                        key_colors: Vec::new(),
                        status: DeviceStatus::default(),
                        status_read_at: None,
                        battery_watch: BatteryWatch::default(),
                        write_failures: 0,
                    };
                    self.restore(&key, &mut slot);
                    info!("Connected: {} ({key})", slot.info.name);
                    self.slots.insert(key, slot);
                }
                Err(e) => {
                    let message = describe_open_error(&info, &e);
                    if self.open_failures.get(&key) != Some(&message) {
                        warn!("{message}");
                        self.open_failures.insert(key, message);
                    }
                }
            }
        }
        if removed {
            self.overlays.retain(|o| match &o.target {
                OverlayTarget::Device(key) => taken.contains(key),
                OverlayTarget::Kind(_) => true,
            });
        }
        Ok(())
    }

    fn open(&self, info: &DeviceInfo) -> Result<DeviceHandle> {
        let mut handle = match info.device_type {
            DeviceType::Keyboard => DeviceHandle::Keyboard(self.manager.open_keyboard(info)?),
            DeviceType::Mouse => DeviceHandle::Mouse(self.manager.open_mouse(info)?),
            DeviceType::Headset => DeviceHandle::Headset(self.manager.open_headset(info)?),
            DeviceType::Unknown => {
                return Err(Error::UnsupportedDevice {
                    vendor_id: info.vendor_id,
                    product_id: info.product_id,
                });
            }
        };
        match &mut handle {
            DeviceHandle::Keyboard(d) => d.initialize()?,
            DeviceHandle::Mouse(d) => d.initialize()?,
            DeviceHandle::Headset(d) => d.initialize()?,
        }
        Ok(handle)
    }

    /// Re-apply remembered settings the device does not keep itself, and set up its lighting.
    fn restore(&mut self, key: &str, slot: &mut Slot) {
        let lighting = self.state.effective_lighting(key).clone();
        slot.configure_lighting(&lighting);

        let Some(record) = self.state.devices.get(key) else {
            return;
        };
        let descriptors = slot.handle.descriptors();
        let Some(configurable) = slot.handle.configurable() else {
            return;
        };
        for (id, value) in &record.settings {
            let Some(descriptor) = descriptors.iter().find(|d| &d.id == id) else {
                continue;
            };
            if descriptor.persists_on_device || matches!(descriptor.kind, SettingKind::Action) {
                continue;
            }
            if let Err(e) = configurable.apply_setting(id, value) {
                warn!("Could not restore {id} on {}: {e}", slot.info.name);
            }
        }
    }

    fn resolve(&self, selector: &str) -> Result<String> {
        let selector = selector.trim();
        if self.slots.contains_key(selector) {
            return Ok(selector.to_string());
        }
        let matches: Vec<&String> = if let Some(kind) = kind_from_selector(selector) {
            self.slots
                .iter()
                .filter(|(_, slot)| slot.handle.kind() == kind)
                .map(|(key, _)| key)
                .collect()
        } else {
            let needle = selector.to_ascii_lowercase();
            self.slots
                .iter()
                .filter(|(key, slot)| slot.info.name.to_ascii_lowercase().contains(&needle) || key.starts_with(&needle))
                .map(|(key, _)| key)
                .collect()
        };
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(Error::DeviceNotFound(format!(
                "no connected device matches '{selector}'. Connected: {}",
                self.connected_list()
            ))),
            [first, ..]
                if matches
                    .iter()
                    .all(|k| self.slots[*k].model_key == self.slots[*first].model_key) =>
            {
                Ok((*first).clone())
            }
            _ => Err(Error::InvalidConfig(format!(
                "'{selector}' matches several devices; use a device key: {}",
                matches.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
            ))),
        }
    }

    fn connected_list(&self) -> String {
        if self.slots.is_empty() {
            return "none".to_string();
        }
        self.slots
            .iter()
            .map(|(key, slot)| format!("{} [{key}]", slot.name()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    async fn execute(&mut self, command: Command, daemon: bool) -> Result<Value> {
        match command {
            Command::Ping => Ok(json!("pong")),
            Command::Status => {
                if !daemon {
                    self.poll_status(false);
                }
                Ok(serde_json::to_value(self.snapshot(daemon))?)
            }
            Command::Refresh => {
                self.sync_devices().await?;
                Ok(serde_json::to_value(self.snapshot(daemon))?)
            }
            Command::Settings { device } => {
                let key = self.resolve(&device)?;
                Ok(serde_json::to_value(self.settings_view(&key))?)
            }
            Command::Set { device, setting, value } => {
                let key = self.resolve(&device)?;
                self.apply_setting(&key, &setting, value)?;
                Ok(json!({ "device": key, "setting": setting }))
            }
            Command::SetText { device, setting, value } => {
                let key = self.resolve(&device)?;
                let descriptors = self.slots[&key].handle.descriptors();
                let descriptor = descriptors.iter().find(|d| d.id == setting).ok_or_else(|| {
                    let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
                    Error::Unsupported(format!(
                        "{} has no setting '{setting}'. Available: {}",
                        self.slots[&key].name(),
                        if ids.is_empty() {
                            "none".to_string()
                        } else {
                            ids.join(", ")
                        }
                    ))
                })?;
                let parsed = descriptor.parse_value(&value)?;
                self.apply_setting(&key, &setting, parsed)?;
                Ok(json!({ "device": key, "setting": setting }))
            }
            Command::Lighting {
                device,
                effect,
                brightness,
                follow_global,
            } => {
                self.set_lighting(device.as_deref(), effect, brightness, follow_global)?;
                Ok(serde_json::to_value(self.snapshot(daemon))?)
            }
            Command::KeyColors { device, colors, clear } => {
                let keys = match device {
                    Some(selector) => vec![self.resolve(&selector)?],
                    None => self
                        .slots
                        .iter()
                        .filter(|(_, s)| s.handle.supports_per_key())
                        .map(|(k, _)| k.clone())
                        .collect(),
                };
                if keys.is_empty() {
                    return Err(Error::Unsupported(
                        "no connected keyboard supports per-key lighting".to_string(),
                    ));
                }
                for key in &keys {
                    let slot = self
                        .slots
                        .get_mut(key)
                        .ok_or_else(|| Error::DeviceNotFound(key.clone()))?;
                    let DeviceHandle::Keyboard(keyboard) = &mut slot.handle else {
                        return Err(Error::Unsupported(format!("{} is not a keyboard", slot.info.name)));
                    };
                    if clear {
                        slot.key_colors.clear();
                        slot.last_frame.clear();
                    } else {
                        for (key_id, color) in &colors {
                            slot.key_colors.retain(|(k, _)| k != key_id);
                            slot.key_colors.push((*key_id, *color));
                        }
                        keyboard.set_key_colors(&slot.key_colors).await?;
                    }
                }
                Ok(json!({ "devices": keys }))
            }
            Command::ProfileList => Ok(serde_json::to_value(self.profile_list())?),
            Command::ProfileSave { name, description } => {
                self.save_profile(&name, description)?;
                Ok(json!({ "saved": name }))
            }
            Command::ProfileLoad { name } => {
                self.load_profile(&name)?;
                Ok(serde_json::to_value(self.snapshot(daemon))?)
            }
            Command::ProfileDelete { name } => {
                let profiles = self.profiles_mut()?;
                profiles.delete(&name)?;
                if self.state.active_profile.as_deref() == Some(name.as_str()) {
                    self.state.active_profile = None;
                    self.state_dirty = true;
                }
                Ok(json!({ "deleted": name }))
            }
            Command::ProfileApps { name, apps } => {
                let profiles = self.profiles_mut()?;
                let mut profile = profiles
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| Error::Profile(format!("profile '{name}' not found")))?;
                profile.apps = apps
                    .into_iter()
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect();
                profiles.set(profile)?;
                Ok(json!({ "profile": name }))
            }
        }
    }

    fn apply_setting(&mut self, key: &str, setting: &str, value: SettingValue) -> Result<()> {
        let slot = self
            .slots
            .get_mut(key)
            .ok_or_else(|| Error::DeviceNotFound(key.to_string()))?;
        let descriptors = slot.handle.descriptors();
        let descriptor = crate::devices::settings::validate_against(&descriptors, setting, &value)?;
        let is_action = matches!(descriptor.kind, SettingKind::Action);
        let name = slot.info.name.to_string();
        let configurable = slot
            .handle
            .configurable()
            .ok_or_else(|| Error::Unsupported(format!("{name} has no configurable settings")))?;
        configurable.apply_setting(setting, &value)?;
        if !is_action {
            self.state
                .record_mut(key, &name)
                .settings
                .insert(setting.to_string(), value);
            self.state_dirty = true;
        }
        Ok(())
    }

    fn set_lighting(
        &mut self,
        device: Option<&str>,
        effect: Option<Effect>,
        brightness: Option<u8>,
        follow_global: bool,
    ) -> Result<()> {
        if let Some(b) = brightness
            && b > 100
        {
            return Err(Error::InvalidConfig(format!("brightness {b} is above 100")));
        }
        match device {
            None => {
                if let Some(effect) = effect {
                    self.state.lighting.effect = effect;
                }
                if let Some(b) = brightness {
                    self.state.lighting.brightness = b;
                }
            }
            Some(selector) => {
                let key = self.resolve(selector)?;
                let name = self.slots[&key].info.name.to_string();
                let global = self.state.lighting.clone();
                let record = self.state.record_mut(&key, &name);
                if follow_global {
                    record.lighting = None;
                } else {
                    let lighting = record.lighting.get_or_insert(global);
                    if let Some(effect) = effect {
                        lighting.effect = effect;
                    }
                    if let Some(b) = brightness {
                        lighting.brightness = b;
                    }
                }
            }
        }
        self.state_dirty = true;
        let keys: Vec<String> = self.slots.keys().cloned().collect();
        for key in keys {
            let lighting = self.state.effective_lighting(&key).clone();
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.configure_lighting(&lighting);
            }
        }
        Ok(())
    }

    /// Compute and send one lighting frame to every device whose colours changed.
    async fn render_frame(&mut self, now: Instant) {
        self.overlays.retain(|o| o.expires > now);
        let overlays = self.overlays.clone();
        let epoch = self.epoch;
        for (key, slot) in self.slots.iter_mut() {
            if !slot.key_colors.is_empty() {
                continue;
            }
            let Some(controller) = slot.lighting.as_mut() else {
                continue;
            };
            let mut colors = controller.compute_colors().to_vec();
            let kind = slot.handle.kind();
            for overlay in &overlays {
                let applies = match &overlay.target {
                    OverlayTarget::Device(k) => k == key,
                    OverlayTarget::Kind(t) => *t == kind,
                };
                if !applies {
                    continue;
                }
                let lit = overlay.flash_hz.is_none_or(|hz| {
                    let t = now.duration_since(epoch).as_secs_f32();
                    ((t * hz * 2.0) as u64).is_multiple_of(2)
                });
                let color = if lit { overlay.color } else { Color::BLACK };
                match overlay.zone {
                    Some(zone) if zone < colors.len() => colors[zone] = color,
                    Some(_) => {}
                    None => colors.iter_mut().for_each(|c| *c = color),
                }
            }
            if colors == slot.last_frame {
                continue;
            }
            match slot.handle.write_zones(&colors).await {
                Ok(()) => {
                    slot.last_frame = colors;
                    slot.write_failures = 0;
                }
                Err(e) => {
                    slot.write_failures += 1;
                    if slot.write_failures == 1 || slot.write_failures.is_multiple_of(300) {
                        warn!("Lighting write to {} failed: {e}", slot.info.name);
                    }
                }
            }
        }
    }

    /// Refresh battery / ChatMix readings. `notify` raises desktop notifications for low battery.
    fn poll_status(&mut self, notify: bool) {
        for slot in self.slots.values_mut() {
            let Some(configurable) = slot.handle.configurable() else {
                continue;
            };
            match configurable.read_status() {
                Ok(status) => {
                    if notify
                        && let Some(percent) = status.battery_percent
                        && let Some(urgency) = slot.battery_watch.observe(percent, status.charging.unwrap_or(false))
                    {
                        notify::send(
                            &format!("{} battery low", slot.info.name),
                            &format!("{percent}% remaining"),
                            urgency,
                        );
                    }
                    slot.status = status;
                    slot.status_read_at = Some(Instant::now());
                }
                Err(e) => debug!("Status read from {} failed: {e}", slot.info.name),
            }
        }
    }

    fn snapshot(&self, daemon: bool) -> EngineSnapshot {
        let devices = self
            .slots
            .iter()
            .map(|(key, slot)| DeviceSnapshot {
                key: key.clone(),
                name: slot.info.name.to_string(),
                kind: slot.handle.kind(),
                product_id: slot.info.product_id,
                status: slot.status.clone(),
                lighting: slot
                    .lighting
                    .as_ref()
                    .map(|_| self.state.effective_lighting(key).clone()),
                follows_global_lighting: self.state.devices.get(key).is_none_or(|r| r.lighting.is_none()),
                capabilities: slot.capabilities(),
            })
            .collect();
        EngineSnapshot {
            version: env!("CARGO_PKG_VERSION").to_string(),
            daemon,
            devices,
            lighting: self.state.lighting.clone(),
            active_profile: self.state.active_profile.clone(),
            warnings: self.open_failures.values().cloned().collect(),
        }
    }

    fn settings_view(&self, key: &str) -> SettingsView {
        let slot = &self.slots[key];
        let record = self.state.devices.get(key);
        let settings = slot
            .handle
            .descriptors()
            .into_iter()
            .map(|descriptor| {
                let value = record
                    .and_then(|r| r.settings.get(&descriptor.id).cloned())
                    .or_else(|| descriptor.default.clone());
                SettingEntry { descriptor, value }
            })
            .collect();
        SettingsView {
            device: key.to_string(),
            name: slot.info.name.to_string(),
            settings,
        }
    }

    fn profiles_mut(&mut self) -> Result<&mut ProfileManager> {
        self.profiles
            .as_mut()
            .ok_or_else(|| Error::Profile("profile storage is unavailable".to_string()))
    }

    fn profile_list(&self) -> Vec<ProfileSummary> {
        let Some(profiles) = &self.profiles else {
            return Vec::new();
        };
        let mut list: Vec<ProfileSummary> = profiles
            .all()
            .values()
            .map(|p| ProfileSummary {
                name: p.name.clone(),
                description: p.description.clone(),
                apps: p.apps.clone(),
                active: self.state.active_profile.as_deref() == Some(p.name.as_str()),
            })
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    fn save_profile(&mut self, name: &str, description: Option<String>) -> Result<()> {
        let mut devices = BTreeMap::new();
        for (key, slot) in &self.slots {
            let record = self.state.devices.get(key);
            let descriptors = slot.handle.descriptors();
            let settings: BTreeMap<String, SettingValue> = record
                .map(|r| {
                    r.settings
                        .iter()
                        .filter(|(id, _)| {
                            descriptors
                                .iter()
                                .any(|d| &d.id == *id && !matches!(d.kind, SettingKind::Action))
                        })
                        .map(|(id, v)| (id.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            devices.insert(
                slot.model_key.clone(),
                DeviceProfile {
                    name: slot.info.name.to_string(),
                    settings,
                    lighting: record.and_then(|r| r.lighting.clone()),
                },
            );
        }
        let lighting = self.state.lighting.clone();
        let profiles = self.profiles_mut()?;
        let mut profile = profiles.get(name).cloned().unwrap_or_else(|| Profile::new(name));
        if description.is_some() {
            profile.description = description;
        }
        profile.lighting = Some(lighting);
        // Keep entries for models that are not plugged in right now.
        for (model, device_profile) in devices {
            profile.devices.insert(model, device_profile);
        }
        profiles.set(profile)?;
        Ok(())
    }

    fn load_profile(&mut self, name: &str) -> Result<()> {
        let profile = self
            .profiles
            .as_ref()
            .and_then(|p| p.get(name))
            .cloned()
            .ok_or_else(|| Error::Profile(format!("profile '{name}' not found")))?;

        if let Some(lighting) = &profile.lighting {
            self.state.lighting = lighting.clone();
        } else if let Some(keyboard) = &profile.keyboard {
            self.state.lighting = Lighting {
                effect: keyboard.effect.clone(),
                brightness: keyboard.brightness,
            };
        }

        let keys: Vec<String> = self.slots.keys().cloned().collect();
        let mut errors = Vec::new();
        for key in keys {
            let (model, device_name) = {
                let slot = &self.slots[&key];
                (slot.model_key.clone(), slot.info.name.to_string())
            };
            let Some(device_profile) = profile.devices.get(&model) else {
                continue;
            };
            self.state.record_mut(&key, &device_name).lighting = device_profile.lighting.clone();
            for (id, value) in &device_profile.settings {
                if let Err(e) = self.apply_setting(&key, id, value.clone()) {
                    errors.push(format!("{device_name}: {id}: {e}"));
                }
            }
        }

        self.state.active_profile = Some(profile.name.clone());
        self.state_dirty = true;
        let keys: Vec<String> = self.slots.keys().cloned().collect();
        for key in keys {
            let lighting = self.state.effective_lighting(&key).clone();
            if let Some(slot) = self.slots.get_mut(&key) {
                slot.configure_lighting(&lighting);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Profile(format!(
                "profile '{name}' loaded with errors: {}",
                errors.join("; ")
            )))
        }
    }

    fn save_state(&mut self) {
        match self.state.save() {
            Ok(()) => self.state_dirty = false,
            Err(e) => warn!("Could not save engine state: {e}"),
        }
    }
}

fn describe_open_error(info: &DeviceInfo, error: &Error) -> String {
    let text = error.to_string();
    let permission = text.contains("Permission denied") || text.contains("EACCES") || text.contains("os error 13");
    if permission {
        format!(
            "Cannot open {} ({:04x}:{:04x}): permission denied. Install assets/70-steelseries.rules into \
             /etc/udev/rules.d/, run `sudo udevadm control --reload-rules && sudo udevadm trigger`, then replug it.",
            info.name, info.vendor_id, info.product_id
        )
    } else {
        format!(
            "Cannot open {} ({:04x}:{:04x}): {text}",
            info.name, info.vendor_id, info.product_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(pid: u16, serial: Option<&str>) -> DeviceInfo {
        DeviceInfo {
            name: "Test".into(),
            device_type: DeviceType::Keyboard,
            vendor_id: 0x1038,
            product_id: pid,
            interface_number: 1,
            usage_page: 0xFFC0,
            usage: 1,
            serial_number: serial.map(str::to_string),
            manufacturer: None,
            path: "/dev/hidraw3".into(),
        }
    }

    #[test]
    fn device_keys_ignore_the_hidraw_path() {
        let mut a = info(0x1628, Some("ABC"));
        let key = device_key(&a);
        a.path = "/dev/hidraw7".into();
        assert_eq!(device_key(&a), key);
        assert_eq!(key, "1038:1628:ABC");
        assert_eq!(device_key(&info(0x1628, Some("  "))), "1038:1628:0");
        assert_eq!(model_key(&a), "1038:1628");
    }

    #[test]
    fn type_selectors() {
        assert_eq!(kind_from_selector("Keyboard"), Some(DeviceType::Keyboard));
        assert_eq!(kind_from_selector("mouse"), Some(DeviceType::Mouse));
        assert_eq!(kind_from_selector("headset"), Some(DeviceType::Headset));
        assert_eq!(kind_from_selector("apex"), None);
    }
}
