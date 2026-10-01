//! Pure remapping state machine.
//!
//! [`Remapper`] turns physical key events into virtual key events plus side-effect requests. It
//! never reads a clock: every call takes `now`, so tests drive it with made-up instants and the
//! Linux engine drives it with `Instant::now()`. Timed work (macro steps, the escape chord) is
//! reported through [`Remapper::next_deadline`] and run by [`Remapper::tick`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::binding::{Action, BindingSet, MacroMode, MacroStep, TriggerMode};
use super::keys::InputKey;
use super::text::text_to_steps;
use crate::error::Result;

/// The value of an `EV_KEY` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyValue {
    Release,
    Press,
    /// Kernel auto-repeat while held.
    Repeat,
}

impl KeyValue {
    /// From an evdev event value (0, 1, 2). Other values are not key states.
    pub fn from_evdev(value: i32) -> Option<Self> {
        match value {
            0 => Some(KeyValue::Release),
            1 => Some(KeyValue::Press),
            2 => Some(KeyValue::Repeat),
            _ => None,
        }
    }

    /// The evdev event value.
    pub fn to_evdev(self) -> i32 {
        match self {
            KeyValue::Release => 0,
            KeyValue::Press => 1,
            KeyValue::Repeat => 2,
        }
    }
}

/// A key or mouse-button event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: InputKey,
    pub value: KeyValue,
}

impl KeyEvent {
    pub fn new(key: InputKey, value: KeyValue) -> Self {
        Self { key, value }
    }
}

/// Something the remapper asks its driver to do, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Emit this event on the virtual device.
    Key(KeyEvent),
    /// Start a program.
    Launch { command: String, args: Vec<String> },
    /// Switch to the named profile (the remapper does not know profiles).
    SwitchProfile(String),
    /// The emergency escape chord was held: stop remapping and release every grab. All virtual
    /// keys have already been released by the effects before this one.
    EmergencyStop,
}

/// Timing settings of the [`Remapper`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemapperConfig {
    /// How long a `Tap` (and each typed character) holds its key.
    pub tap_hold: Duration,
    /// Shortest time between the starts of two iterations of a repeating macro. Keeps a macro
    /// without delays from flooding the system.
    pub min_loop_period: Duration,
    /// How long both Ctrl keys and both Shift keys must be held to trigger [`Effect::EmergencyStop`].
    pub escape_hold: Duration,
}

impl Default for RemapperConfig {
    fn default() -> Self {
        Self {
            tap_hold: Duration::from_millis(10),
            min_loop_period: Duration::from_millis(10),
            escape_hold: Duration::from_secs(2),
        }
    }
}

/// The four keys of the emergency escape chord.
pub const ESCAPE_CHORD: [InputKey; 4] = [
    InputKey::KEY_LEFTCTRL,
    InputKey::KEY_RIGHTCTRL,
    InputKey::KEY_LEFTSHIFT,
    InputKey::KEY_RIGHTSHIFT,
];

/// Upper bound of macro operations run by one `tick`, so a pathological set cannot stall the
/// event loop. Remaining work continues on the next call.
const MAX_OPS_PER_TICK: usize = 50_000;

/// Detects the emergency escape chord held for a duration.
#[derive(Debug, Clone)]
pub(crate) struct EscapeChord {
    hold: Duration,
    since: Option<Instant>,
    fired: bool,
}

impl EscapeChord {
    pub(crate) fn new(hold: Duration) -> Self {
        Self {
            hold,
            since: None,
            fired: false,
        }
    }

    /// Feed the current set of physically held keys.
    pub(crate) fn update(&mut self, held: &BTreeSet<InputKey>, now: Instant) {
        if ESCAPE_CHORD.iter().all(|k| held.contains(k)) {
            self.since.get_or_insert(now);
        } else {
            self.since = None;
            self.fired = false;
        }
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        if self.fired {
            return None;
        }
        self.since.map(|since| since + self.hold)
    }

    /// True once per hold, when the chord has been held long enough.
    pub(crate) fn check(&mut self, now: Instant) -> bool {
        match self.deadline() {
            Some(deadline) if now >= deadline => {
                self.fired = true;
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Down(InputKey),
    Up(InputKey),
    Wait(Duration),
}

#[derive(Debug, Clone)]
enum Compiled {
    Passthrough,
    Disabled,
    /// Keys held while the source is held (Key, Combo, Media, MouseButton).
    Hold(Arc<[InputKey]>),
    Play {
        ops: Arc<[Op]>,
        repeat: u32,
        mode: MacroMode,
    },
    Launch {
        command: String,
        args: Vec<String>,
    },
    SwitchProfile(String),
}

#[derive(Debug, Clone)]
struct CompiledBinding {
    action: Compiled,
    trigger: TriggerMode,
}

/// What a source key's press did, so its release undoes exactly that, even after the binding
/// set changed in between.
#[derive(Debug, Clone)]
enum Active {
    Hold(Arc<[InputKey]>),
    Swallow,
    StopOnRelease,
    OnRelease(Compiled),
}

#[derive(Debug)]
struct Playback {
    ops: Arc<[Op]>,
    index: usize,
    /// `None` loops until stopped.
    iterations_left: Option<u32>,
    next_at: Instant,
    iteration_start: Instant,
    held: Vec<InputKey>,
}

/// The remapping state machine. See the module docs.
#[derive(Debug)]
pub struct Remapper {
    config: RemapperConfig,
    bindings: BTreeMap<InputKey, CompiledBinding>,
    active_set: bool,
    physical: BTreeSet<InputKey>,
    active: BTreeMap<InputKey, Active>,
    /// Reference counts of keys held down on the virtual device.
    out_down: BTreeMap<InputKey, u32>,
    playbacks: BTreeMap<InputKey, Playback>,
    escape: EscapeChord,
}

impl Remapper {
    /// Validate and load `bindings`.
    pub fn new(bindings: &BindingSet, config: RemapperConfig) -> Result<Self> {
        let compiled = compile(bindings, &config)?;
        Ok(Self {
            escape: EscapeChord::new(config.escape_hold),
            config,
            bindings: compiled,
            active_set: bindings.is_active(),
            physical: BTreeSet::new(),
            active: BTreeMap::new(),
            out_down: BTreeMap::new(),
            playbacks: BTreeMap::new(),
        })
    }

    /// Whether the loaded set changes anything (see [`BindingSet::is_active`]).
    pub fn is_active(&self) -> bool {
        self.active_set
    }

    /// Replace the binding set. Running macros stop (their keys are released); keys held under
    /// the old set keep their old meaning until released. On error nothing changes.
    pub fn set_bindings(&mut self, bindings: &BindingSet) -> Result<Vec<Effect>> {
        let compiled = compile(bindings, &self.config)?;
        let mut out = Vec::new();
        self.stop_all_playbacks(&mut out);
        for active in self.active.values_mut() {
            if matches!(active, Active::StopOnRelease | Active::OnRelease(_)) {
                *active = Active::Swallow;
            }
        }
        self.bindings = compiled;
        self.active_set = bindings.is_active();
        Ok(out)
    }

    /// Process one physical event.
    pub fn handle(&mut self, event: KeyEvent, now: Instant) -> Vec<Effect> {
        let mut out = Vec::new();
        let key = event.key;
        match event.value {
            KeyValue::Press => {
                if self.physical.insert(key) && !self.active.contains_key(&key) {
                    self.escape.update(&self.physical, now);
                    self.on_press(key, now, &mut out);
                } else {
                    self.on_repeat(key, &mut out);
                }
            }
            KeyValue::Repeat => self.on_repeat(key, &mut out),
            KeyValue::Release => {
                self.physical.remove(&key);
                self.escape.update(&self.physical, now);
                self.on_release(key, now, &mut out);
            }
        }
        self.run_due(now, &mut out);
        out
    }

    /// Run timed work that is due at `now`.
    pub fn tick(&mut self, now: Instant) -> Vec<Effect> {
        let mut out = Vec::new();
        self.run_due(now, &mut out);
        out
    }

    /// When [`Remapper::tick`] next has work to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.playbacks
            .values()
            .map(|p| p.next_at)
            .chain(self.escape.deadline())
            .min()
    }

    /// Stop all macros, release every key held on the virtual device and forget all physical key
    /// state. Repeats and releases of keys still physically held are then ignored; their next
    /// press works normally. Use before ungrabbing or losing a device, and on shutdown.
    pub fn release_all(&mut self) -> Vec<Effect> {
        let mut out = Vec::new();
        self.release_all_into(&mut out);
        out
    }

    fn release_all_into(&mut self, out: &mut Vec<Effect>) {
        self.stop_all_playbacks(out);
        for (_, active) in std::mem::take(&mut self.active) {
            if let Active::Hold(keys) = active {
                for k in keys.iter().rev() {
                    release(&mut self.out_down, *k, out);
                }
            }
        }
        for (key, _) in std::mem::take(&mut self.out_down) {
            out.push(Effect::Key(KeyEvent::new(key, KeyValue::Release)));
        }
        self.physical.clear();
    }

    fn on_press(&mut self, key: InputKey, now: Instant, out: &mut Vec<Effect>) {
        let Some(binding) = self.bindings.get(&key).cloned() else {
            self.start_hold(key, Arc::from([key]), out);
            return;
        };
        let deferrable = !matches!(binding.action, Compiled::Passthrough | Compiled::Disabled);
        if binding.trigger == TriggerMode::Release && deferrable {
            self.active.insert(key, Active::OnRelease(binding.action));
            return;
        }
        match binding.action {
            Compiled::Passthrough => self.start_hold(key, Arc::from([key]), out),
            Compiled::Disabled => {
                self.active.insert(key, Active::Swallow);
            }
            Compiled::Hold(keys) => self.start_hold(key, keys, out),
            Compiled::Play { ops, repeat, mode } => {
                let active = match mode {
                    MacroMode::Once => {
                        self.start_playback(key, ops, Some(repeat), now);
                        Active::Swallow
                    }
                    MacroMode::WhileHeld => {
                        self.start_playback(key, ops, None, now);
                        Active::StopOnRelease
                    }
                    MacroMode::Toggle => {
                        self.toggle_playback(key, ops, now, out);
                        Active::Swallow
                    }
                };
                self.active.insert(key, active);
            }
            Compiled::Launch { command, args } => {
                out.push(Effect::Launch { command, args });
                self.active.insert(key, Active::Swallow);
            }
            Compiled::SwitchProfile(name) => {
                out.push(Effect::SwitchProfile(name));
                self.active.insert(key, Active::Swallow);
            }
        }
    }

    fn on_repeat(&mut self, key: InputKey, out: &mut Vec<Effect>) {
        if let Some(Active::Hold(keys)) = self.active.get(&key)
            && let Some(last) = keys.last()
            && self.out_down.contains_key(last)
        {
            out.push(Effect::Key(KeyEvent::new(*last, KeyValue::Repeat)));
        }
    }

    fn on_release(&mut self, key: InputKey, now: Instant, out: &mut Vec<Effect>) {
        match self.active.remove(&key) {
            Some(Active::Hold(keys)) => {
                for k in keys.iter().rev() {
                    release(&mut self.out_down, *k, out);
                }
            }
            Some(Active::StopOnRelease) => self.stop_playback(key, out),
            Some(Active::OnRelease(action)) => self.fire_on_release(key, action, now, out),
            Some(Active::Swallow) | None => {}
        }
    }

    fn fire_on_release(&mut self, key: InputKey, action: Compiled, now: Instant, out: &mut Vec<Effect>) {
        match action {
            Compiled::Hold(keys) => {
                let ops = tap_ops(&keys, self.config.tap_hold);
                self.start_playback(key, ops, Some(1), now);
            }
            Compiled::Play { ops, repeat, mode } => match mode {
                MacroMode::Toggle => self.toggle_playback(key, ops, now, out),
                MacroMode::Once | MacroMode::WhileHeld => self.start_playback(key, ops, Some(repeat), now),
            },
            Compiled::Launch { command, args } => out.push(Effect::Launch { command, args }),
            Compiled::SwitchProfile(name) => out.push(Effect::SwitchProfile(name)),
            Compiled::Passthrough | Compiled::Disabled => {}
        }
    }

    fn start_hold(&mut self, source: InputKey, keys: Arc<[InputKey]>, out: &mut Vec<Effect>) {
        for k in keys.iter() {
            press(&mut self.out_down, *k, out);
        }
        self.active.insert(source, Active::Hold(keys));
    }

    /// Start a macro for `source` unless one is already playing for it.
    fn start_playback(&mut self, source: InputKey, ops: Arc<[Op]>, iterations: Option<u32>, now: Instant) {
        self.playbacks.entry(source).or_insert(Playback {
            ops,
            index: 0,
            iterations_left: iterations,
            next_at: now,
            iteration_start: now,
            held: Vec::new(),
        });
    }

    fn toggle_playback(&mut self, source: InputKey, ops: Arc<[Op]>, now: Instant, out: &mut Vec<Effect>) {
        if self.playbacks.contains_key(&source) {
            self.stop_playback(source, out);
        } else {
            self.start_playback(source, ops, None, now);
        }
    }

    fn stop_playback(&mut self, source: InputKey, out: &mut Vec<Effect>) {
        if let Some(playback) = self.playbacks.remove(&source) {
            for k in playback.held.iter().rev() {
                release(&mut self.out_down, *k, out);
            }
        }
    }

    fn stop_all_playbacks(&mut self, out: &mut Vec<Effect>) {
        let sources: Vec<InputKey> = self.playbacks.keys().copied().collect();
        for source in sources {
            self.stop_playback(source, out);
        }
    }

    fn run_due(&mut self, now: Instant, out: &mut Vec<Effect>) {
        let mut budget = MAX_OPS_PER_TICK;
        let mut finished = Vec::new();
        for (source, playback) in &mut self.playbacks {
            if run_playback(
                playback,
                now,
                self.config.min_loop_period,
                &mut self.out_down,
                out,
                &mut budget,
            ) {
                finished.push(*source);
            }
        }
        for source in finished {
            self.playbacks.remove(&source);
        }
        if self.escape.check(now) {
            self.release_all_into(out);
            out.push(Effect::EmergencyStop);
        }
    }
}

fn press(out_down: &mut BTreeMap<InputKey, u32>, key: InputKey, out: &mut Vec<Effect>) {
    let count = out_down.entry(key).or_insert(0);
    if *count == 0 {
        out.push(Effect::Key(KeyEvent::new(key, KeyValue::Press)));
    }
    *count += 1;
}

fn release(out_down: &mut BTreeMap<InputKey, u32>, key: InputKey, out: &mut Vec<Effect>) {
    if let Some(count) = out_down.get_mut(&key) {
        *count -= 1;
        if *count == 0 {
            out_down.remove(&key);
            out.push(Effect::Key(KeyEvent::new(key, KeyValue::Release)));
        }
    }
}

/// Advance one playback to `now`. Returns true when it has finished.
fn run_playback(
    playback: &mut Playback,
    now: Instant,
    min_loop_period: Duration,
    out_down: &mut BTreeMap<InputKey, u32>,
    out: &mut Vec<Effect>,
    budget: &mut usize,
) -> bool {
    while playback.next_at <= now && *budget > 0 {
        *budget -= 1;
        let Some(op) = playback.ops.get(playback.index).copied() else {
            for k in playback.held.drain(..).rev() {
                release(out_down, k, out);
            }
            match playback.iterations_left {
                Some(n) if n <= 1 => return true,
                Some(n) => playback.iterations_left = Some(n - 1),
                None => {}
            }
            playback.index = 0;
            let earliest = playback.iteration_start + min_loop_period;
            if playback.next_at < earliest {
                playback.next_at = earliest;
            }
            playback.iteration_start = playback.next_at;
            continue;
        };
        playback.index += 1;
        match op {
            Op::Down(k) => {
                press(out_down, k, out);
                playback.held.push(k);
            }
            Op::Up(k) => {
                if let Some(pos) = playback.held.iter().rposition(|h| *h == k) {
                    playback.held.remove(pos);
                    release(out_down, k, out);
                }
            }
            // Measured from when the wait is reached, so a late tick shifts the rest of the
            // macro instead of squeezing its delays together.
            Op::Wait(d) => playback.next_at = now + d,
        }
    }
    false
}

fn tap_ops(keys: &[InputKey], hold: Duration) -> Arc<[Op]> {
    let mut ops: Vec<Op> = keys.iter().map(|k| Op::Down(*k)).collect();
    if !hold.is_zero() {
        ops.push(Op::Wait(hold));
    }
    ops.extend(keys.iter().rev().map(|k| Op::Up(*k)));
    ops.into()
}

fn macro_ops(steps: &[MacroStep], tap_hold: Duration) -> Arc<[Op]> {
    let mut ops = Vec::with_capacity(steps.len() * 3);
    for step in steps {
        match *step {
            MacroStep::Press(k) => ops.push(Op::Down(k)),
            MacroStep::Release(k) => ops.push(Op::Up(k)),
            MacroStep::Tap(k) => {
                ops.push(Op::Down(k));
                if !tap_hold.is_zero() {
                    ops.push(Op::Wait(tap_hold));
                }
                ops.push(Op::Up(k));
            }
            MacroStep::Delay(0) => {}
            MacroStep::Delay(ms) => ops.push(Op::Wait(Duration::from_millis(u64::from(ms)))),
        }
    }
    ops.into()
}

fn compile(set: &BindingSet, config: &RemapperConfig) -> Result<BTreeMap<InputKey, CompiledBinding>> {
    set.validate()?;
    let mut map = BTreeMap::new();
    for binding in &set.bindings {
        let action = match &binding.action {
            Action::Key(k) => Compiled::Hold(Arc::from([*k])),
            Action::Combo(keys) => Compiled::Hold(Arc::from(keys.as_slice())),
            Action::Media(m) => Compiled::Hold(Arc::from([m.key()])),
            Action::MouseButton(b) => Compiled::Hold(Arc::from([b.key()])),
            Action::Macro { steps, repeat, mode } => Compiled::Play {
                ops: macro_ops(steps, config.tap_hold),
                repeat: *repeat,
                mode: *mode,
            },
            Action::Text(text) => Compiled::Play {
                ops: macro_ops(&text_to_steps(text)?, config.tap_hold),
                repeat: 1,
                mode: MacroMode::Once,
            },
            Action::Launch { command, args } => Compiled::Launch {
                command: command.clone(),
                args: args.clone(),
            },
            Action::SwitchProfile(name) => Compiled::SwitchProfile(name.clone()),
            Action::Disabled => Compiled::Disabled,
            Action::Passthrough => Compiled::Passthrough,
        };
        map.insert(
            binding.source,
            CompiledBinding {
                action,
                trigger: binding.mode,
            },
        );
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::binding::Binding;
    use crate::input::keys::{MediaKey, MouseButton};

    fn key(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn down(name: &str) -> Effect {
        Effect::Key(KeyEvent::new(key(name), KeyValue::Press))
    }

    fn up(name: &str) -> Effect {
        Effect::Key(KeyEvent::new(key(name), KeyValue::Release))
    }

    fn rep(name: &str) -> Effect {
        Effect::Key(KeyEvent::new(key(name), KeyValue::Repeat))
    }

    fn set(bindings: Vec<Binding>) -> BindingSet {
        BindingSet { bindings }
    }

    fn remapper(bindings: Vec<Binding>) -> Remapper {
        Remapper::new(&set(bindings), RemapperConfig::default()).unwrap()
    }

    fn press_at(r: &mut Remapper, name: &str, at: Instant) -> Vec<Effect> {
        r.handle(KeyEvent::new(key(name), KeyValue::Press), at)
    }

    fn release_at(r: &mut Remapper, name: &str, at: Instant) -> Vec<Effect> {
        r.handle(KeyEvent::new(key(name), KeyValue::Release), at)
    }

    fn repeat_at(r: &mut Remapper, name: &str, at: Instant) -> Vec<Effect> {
        r.handle(KeyEvent::new(key(name), KeyValue::Repeat), at)
    }

    /// Drive the remapper through every deadline up to `end`, like the engine loop does.
    /// Returns effects with their time offset from `t0` in milliseconds.
    fn run_until(r: &mut Remapper, t0: Instant, end: Instant) -> Vec<(u64, Effect)> {
        let mut log = Vec::new();
        while let Some(deadline) = r.next_deadline() {
            if deadline > end {
                break;
            }
            let at = (deadline - t0).as_millis() as u64;
            log.extend(r.tick(deadline).into_iter().map(|e| (at, e)));
        }
        log
    }

    fn macro_action(steps: Vec<MacroStep>, repeat: u32, mode: MacroMode) -> Action {
        Action::Macro { steps, repeat, mode }
    }

    #[test]
    fn key_remap_follows_press_repeat_release() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Key(key("leftctrl")))]);
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "capslock", t), vec![down("leftctrl")]);
        assert_eq!(repeat_at(&mut r, "capslock", t), vec![rep("leftctrl")]);
        assert_eq!(release_at(&mut r, "capslock", t), vec![up("leftctrl")]);
        assert_eq!(r.next_deadline(), None);
    }

    #[test]
    fn unbound_keys_pass_through_with_repeat() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Disabled)]);
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "a", t), vec![down("a")]);
        assert_eq!(repeat_at(&mut r, "a", t), vec![rep("a")]);
        assert_eq!(release_at(&mut r, "a", t), vec![up("a")]);
        assert_eq!(press_at(&mut r, "mouse1", t), vec![down("BTN_LEFT")]);
        assert_eq!(release_at(&mut r, "mouse1", t), vec![up("BTN_LEFT")]);
    }

    #[test]
    fn explicit_passthrough_behaves_like_unbound() {
        let mut r = remapper(vec![Binding::new(key("a"), Action::Passthrough)]);
        assert!(!r.is_active());
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "a", t), vec![down("a")]);
        assert_eq!(release_at(&mut r, "a", t), vec![up("a")]);
    }

    #[test]
    fn disabled_key_is_swallowed() {
        let mut r = remapper(vec![Binding::new(key("insert"), Action::Disabled)]);
        assert!(r.is_active());
        let t = Instant::now();
        assert!(press_at(&mut r, "insert", t).is_empty());
        assert!(repeat_at(&mut r, "insert", t).is_empty());
        assert!(release_at(&mut r, "insert", t).is_empty());
    }

    #[test]
    fn combo_presses_in_order_and_releases_in_reverse() {
        let mut r = remapper(vec![Binding::new(
            key("mouse4"),
            Action::Combo(vec![key("ctrl"), key("shift"), key("t")]),
        )]);
        let t = Instant::now();
        assert_eq!(
            press_at(&mut r, "mouse4", t),
            vec![down("leftctrl"), down("leftshift"), down("t")]
        );
        assert_eq!(repeat_at(&mut r, "mouse4", t), vec![rep("t")]);
        assert_eq!(
            release_at(&mut r, "mouse4", t),
            vec![up("t"), up("leftshift"), up("leftctrl")]
        );
    }

    #[test]
    fn media_and_mouse_button_actions_hold_their_key() {
        let mut r = remapper(vec![
            Binding::new(key("f16"), Action::Media(MediaKey::PlayPause)),
            Binding::new(key("f17"), Action::MouseButton(MouseButton::Side)),
        ]);
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "f16", t), vec![down("KEY_PLAYPAUSE")]);
        assert_eq!(release_at(&mut r, "f16", t), vec![up("KEY_PLAYPAUSE")]);
        assert_eq!(press_at(&mut r, "f17", t), vec![down("BTN_SIDE")]);
        assert_eq!(release_at(&mut r, "f17", t), vec![up("BTN_SIDE")]);
    }

    #[test]
    fn user_held_modifier_combines_with_remapped_key() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Key(key("a")))]);
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "leftshift", t), vec![down("leftshift")]);
        assert_eq!(press_at(&mut r, "capslock", t), vec![down("a")]);
        assert_eq!(release_at(&mut r, "capslock", t), vec![up("a")]);
        assert_eq!(release_at(&mut r, "leftshift", t), vec![up("leftshift")]);
    }

    #[test]
    fn macro_steps_follow_the_fake_clock() {
        let steps = vec![
            MacroStep::Tap(key("h")),
            MacroStep::Delay(100),
            MacroStep::Tap(key("i")),
        ];
        let mut r = remapper(vec![Binding::new(key("f13"), macro_action(steps, 1, MacroMode::Once))]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("h")]);
        assert_eq!(r.next_deadline(), Some(t0 + ms(10)));
        assert!(
            release_at(&mut r, "f13", t0 + ms(5)).is_empty(),
            "once-macros ignore release"
        );
        assert!(r.tick(t0 + ms(9)).is_empty());
        let log = run_until(&mut r, t0, t0 + ms(1000));
        assert_eq!(log, vec![(10, up("h")), (110, down("i")), (120, up("i"))]);
        assert_eq!(r.next_deadline(), None);
    }

    #[test]
    fn macro_repeat_count_and_retrigger_ignored_while_playing() {
        let steps = vec![MacroStep::Tap(key("x"))];
        let mut r = remapper(vec![Binding::new(key("f13"), macro_action(steps, 3, MacroMode::Once))]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("x")]);
        release_at(&mut r, "f13", t0 + ms(1));
        assert!(
            press_at(&mut r, "f13", t0 + ms(2)).is_empty(),
            "retrigger while playing is ignored"
        );
        release_at(&mut r, "f13", t0 + ms(3));
        let log = run_until(&mut r, t0, t0 + ms(1000));
        assert_eq!(
            log,
            vec![
                (10, up("x")),
                (10, down("x")),
                (20, up("x")),
                (20, down("x")),
                (30, up("x"))
            ]
        );
    }

    #[test]
    fn repeating_macro_without_delays_is_rate_limited() {
        let steps = vec![MacroStep::Press(key("x")), MacroStep::Release(key("x"))];
        let config = RemapperConfig {
            min_loop_period: ms(25),
            ..RemapperConfig::default()
        };
        let bindings = set(vec![Binding::new(key("f13"), macro_action(steps, 3, MacroMode::Once))]);
        let mut r = Remapper::new(&bindings, config).unwrap();
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("x"), up("x")]);
        let log = run_until(&mut r, t0, t0 + ms(1000));
        assert_eq!(
            log,
            vec![(25, down("x")), (25, up("x")), (50, down("x")), (50, up("x"))]
        );
    }

    #[test]
    fn while_held_macro_loops_and_stops_on_release() {
        let steps = vec![
            MacroStep::Press(key("w")),
            MacroStep::Delay(50),
            MacroStep::Release(key("w")),
            MacroStep::Delay(50),
        ];
        let mut r = remapper(vec![Binding::new(
            key("f13"),
            macro_action(steps, 1, MacroMode::WhileHeld),
        )]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("w")]);
        let log = run_until(&mut r, t0, t0 + ms(130));
        assert_eq!(log, vec![(50, up("w")), (100, down("w"))]);
        assert_eq!(
            release_at(&mut r, "f13", t0 + ms(130)),
            vec![up("w")],
            "held keys released at once"
        );
        assert_eq!(r.next_deadline(), None);
        assert!(run_until(&mut r, t0, t0 + ms(1000)).is_empty());
    }

    #[test]
    fn toggle_macro_starts_and_stops_on_presses() {
        let steps = vec![MacroStep::Tap(key("x")), MacroStep::Delay(90)];
        let mut r = remapper(vec![Binding::new(
            key("f13"),
            macro_action(steps, 1, MacroMode::Toggle),
        )]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("x")]);
        assert!(
            release_at(&mut r, "f13", t0 + ms(1)).is_empty(),
            "toggle keeps playing after release"
        );
        let log = run_until(&mut r, t0, t0 + ms(205));
        assert_eq!(
            log,
            vec![(10, up("x")), (100, down("x")), (110, up("x")), (200, down("x"))]
        );
        assert_eq!(
            press_at(&mut r, "f13", t0 + ms(205)),
            vec![up("x")],
            "second press stops and releases"
        );
        release_at(&mut r, "f13", t0 + ms(206));
        assert!(run_until(&mut r, t0, t0 + ms(2000)).is_empty());
    }

    #[test]
    fn text_action_types_with_shift() {
        let mut r = remapper(vec![Binding::new(key("f14"), Action::Text("Hi".to_string()))]);
        let t0 = Instant::now();
        let mut log: Vec<(u64, Effect)> = press_at(&mut r, "f14", t0).into_iter().map(|e| (0, e)).collect();
        log.extend(run_until(&mut r, t0, t0 + ms(1000)));
        assert_eq!(
            log,
            vec![
                (0, down("leftshift")),
                (0, down("h")),
                (10, up("h")),
                (10, up("leftshift")),
                (10, down("i")),
                (20, up("i")),
            ]
        );
    }

    #[test]
    fn profile_switch_and_launch_are_requested_once() {
        let mut r = remapper(vec![
            Binding::new(key("f17"), Action::SwitchProfile("gaming".to_string())),
            Binding::new(
                key("f15"),
                Action::Launch {
                    command: "firefox".to_string(),
                    args: vec!["--new-window".to_string()],
                },
            ),
        ]);
        let t = Instant::now();
        assert_eq!(
            press_at(&mut r, "f17", t),
            vec![Effect::SwitchProfile("gaming".to_string())]
        );
        assert!(repeat_at(&mut r, "f17", t).is_empty());
        assert!(release_at(&mut r, "f17", t).is_empty());
        assert_eq!(
            press_at(&mut r, "f15", t),
            vec![Effect::Launch {
                command: "firefox".to_string(),
                args: vec!["--new-window".to_string()],
            }]
        );
        assert!(release_at(&mut r, "f15", t).is_empty());
    }

    #[test]
    fn release_trigger_taps_on_release() {
        let mut r = remapper(vec![
            Binding {
                source: key("f13"),
                action: Action::Key(key("b")),
                mode: TriggerMode::Release,
            },
            Binding {
                source: key("f17"),
                action: Action::SwitchProfile("work".to_string()),
                mode: TriggerMode::Release,
            },
        ]);
        let t0 = Instant::now();
        assert!(press_at(&mut r, "f13", t0).is_empty());
        assert!(repeat_at(&mut r, "f13", t0).is_empty());
        assert_eq!(release_at(&mut r, "f13", t0 + ms(300)), vec![down("b")]);
        assert_eq!(run_until(&mut r, t0, t0 + ms(1000)), vec![(310, up("b"))]);
        assert!(press_at(&mut r, "f17", t0).is_empty());
        assert_eq!(
            release_at(&mut r, "f17", t0),
            vec![Effect::SwitchProfile("work".to_string())]
        );
    }

    #[test]
    fn macro_does_not_release_a_key_the_user_holds() {
        let steps = vec![
            MacroStep::Press(key("leftshift")),
            MacroStep::Tap(key("a")),
            MacroStep::Release(key("leftshift")),
        ];
        let mut r = remapper(vec![Binding::new(key("f13"), macro_action(steps, 1, MacroMode::Once))]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "leftshift", t0), vec![down("leftshift")]);
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("a")]);
        assert_eq!(run_until(&mut r, t0, t0 + ms(100)), vec![(10, up("a"))]);
        assert_eq!(release_at(&mut r, "leftshift", t0 + ms(100)), vec![up("leftshift")]);
    }

    #[test]
    fn macro_releases_keys_left_down_at_iteration_end() {
        let steps = vec![MacroStep::Press(key("a")), MacroStep::Delay(30)];
        let mut r = remapper(vec![Binding::new(key("f13"), macro_action(steps, 1, MacroMode::Once))]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("a")]);
        assert_eq!(run_until(&mut r, t0, t0 + ms(100)), vec![(30, up("a"))]);
    }

    #[test]
    fn binding_change_keeps_held_keys_consistent() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Key(key("leftctrl")))]);
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "capslock", t), vec![down("leftctrl")]);
        let effects = r
            .set_bindings(&set(vec![Binding::new(key("capslock"), Action::Key(key("esc")))]))
            .unwrap();
        assert!(effects.is_empty());
        assert_eq!(
            release_at(&mut r, "capslock", t),
            vec![up("leftctrl")],
            "old meaning until release"
        );
        assert_eq!(press_at(&mut r, "capslock", t), vec![down("esc")]);
        assert_eq!(release_at(&mut r, "capslock", t), vec![up("esc")]);
    }

    #[test]
    fn binding_change_stops_running_macros() {
        let steps = vec![MacroStep::Press(key("w")), MacroStep::Delay(1000)];
        let mut r = remapper(vec![Binding::new(
            key("f13"),
            macro_action(steps, 1, MacroMode::Toggle),
        )]);
        let t0 = Instant::now();
        assert_eq!(press_at(&mut r, "f13", t0), vec![down("w")]);
        assert_eq!(r.set_bindings(&BindingSet::new()).unwrap(), vec![up("w")]);
        assert_eq!(r.next_deadline(), None);
        assert!(!r.is_active());
    }

    #[test]
    fn invalid_set_is_rejected_without_changing_state() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Key(key("esc")))]);
        let bad = set(vec![Binding::new(key("a"), Action::Text("\u{e9}".to_string()))]);
        assert!(r.set_bindings(&bad).is_err());
        let t = Instant::now();
        assert_eq!(press_at(&mut r, "capslock", t), vec![down("esc")]);
    }

    #[test]
    fn escape_chord_held_two_seconds_stops() {
        let mut r = remapper(vec![Binding::new(key("capslock"), Action::Key(key("leftctrl")))]);
        let t0 = Instant::now();
        press_at(&mut r, "capslock", t0);
        for name in ["leftctrl", "rightctrl", "leftshift"] {
            press_at(&mut r, name, t0);
        }
        assert_eq!(r.next_deadline(), None);
        press_at(&mut r, "rightshift", t0 + ms(100));
        assert_eq!(r.next_deadline(), Some(t0 + ms(2100)));
        assert!(r.tick(t0 + ms(2099)).is_empty());
        let effects = r.tick(t0 + ms(2100));
        assert_eq!(effects.last(), Some(&Effect::EmergencyStop));
        let released: BTreeSet<InputKey> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Key(KeyEvent {
                    key,
                    value: KeyValue::Release,
                }) => Some(*key),
                _ => None,
            })
            .collect();
        let expected: BTreeSet<InputKey> = ["leftctrl", "rightctrl", "leftshift", "rightshift"]
            .into_iter()
            .map(key)
            .collect();
        assert_eq!(released, expected, "every virtual key is released before stopping");
        assert_eq!(effects.len(), 5);
        assert!(r.tick(t0 + ms(5000)).is_empty(), "fires once per hold");
        assert!(release_at(&mut r, "capslock", t0 + ms(5000)).is_empty());
    }

    #[test]
    fn escape_chord_released_early_does_nothing() {
        let mut r = remapper(vec![]);
        let t0 = Instant::now();
        for name in ["leftctrl", "rightctrl", "leftshift", "rightshift"] {
            press_at(&mut r, name, t0);
        }
        assert_eq!(release_at(&mut r, "rightshift", t0 + ms(1500)), vec![up("rightshift")]);
        assert_eq!(r.next_deadline(), None);
        assert!(!r.tick(t0 + ms(3000)).contains(&Effect::EmergencyStop));
    }

    #[test]
    fn release_all_frees_everything() {
        let steps = vec![MacroStep::Press(key("w")), MacroStep::Delay(1000)];
        let mut r = remapper(vec![
            Binding::new(key("f13"), macro_action(steps, 1, MacroMode::Toggle)),
            Binding::new(key("capslock"), Action::Key(key("leftctrl"))),
        ]);
        let t = Instant::now();
        press_at(&mut r, "f13", t);
        press_at(&mut r, "capslock", t);
        press_at(&mut r, "a", t);
        let mut effects = r.release_all();
        effects.sort_by_key(|e| format!("{e:?}"));
        let mut expected = vec![up("w"), up("leftctrl"), up("a")];
        expected.sort_by_key(|e| format!("{e:?}"));
        assert_eq!(effects, expected);
        assert!(
            release_at(&mut r, "capslock", t).is_empty(),
            "still-held keys are swallowed"
        );
        assert!(release_at(&mut r, "a", t).is_empty());
        assert_eq!(press_at(&mut r, "a", t), vec![down("a")], "fresh presses work again");
    }

    #[test]
    fn release_of_unknown_key_is_ignored() {
        let mut r = remapper(vec![]);
        assert!(release_at(&mut r, "a", Instant::now()).is_empty());
        assert!(repeat_at(&mut r, "a", Instant::now()).is_empty());
    }

    #[test]
    fn key_value_conversions() {
        for v in [KeyValue::Release, KeyValue::Press, KeyValue::Repeat] {
            assert_eq!(KeyValue::from_evdev(v.to_evdev()), Some(v));
        }
        assert_eq!(KeyValue::from_evdev(3), None);
    }
}
