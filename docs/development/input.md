# Key Bindings and Macros (`src/input`)

Linux replacement for SteelSeries GG "Key Bindings" and "Macro Editor": per profile, any key on a
SteelSeries keyboard or button on a SteelSeries mouse can become another key, a combo, a macro, typed
text, a program launch, a media key, a mouse button, a profile switch, or nothing.

**Status:** the model and the remapping state machine are covered by host unit tests. The Linux I/O
layer (`src/input/linux.rs`) compiles and passes clippy for `x86_64-unknown-linux-gnu`, but has not
yet been run on a Linux machine or with real devices.

---

## How it works

```
SteelSeries evdev nodes ──EVIOCGRAB──▶ engine thread ──▶ Remapper ──▶ uinput "ssgg virtual input"
 /dev/input/eventN (0x1038)            poll(2) + read     (pure)        keyboard + mouse
                                                          │
                                                          └─▶ Launch / SwitchProfile requests
```

| Layer | File | Platform |
|-------|------|----------|
| Key names, aliases, media keys, mouse buttons | `keys.rs` | any |
| `Binding`, `Action`, `MacroStep`, `BindingSet`, validation | `binding.rs` | any |
| Text → key steps (US layout) | `text.rs` | any |
| `Remapper` state machine | `remapper.rs` | any |
| Recording → macro steps | `record.rs` | any |
| `InputEngine` API, `DeviceFilter`, notices | `engine.rs` | any (stub off Linux) |
| evdev capture, grab, uinput, threads | `linux.rs` | Linux only |

The remapper never reads a clock. Every call takes `now: Instant`; timed work (macro steps, the
escape chord) is reported by `next_deadline()` and run by `tick(now)`. Tests drive it with
made-up instants; the engine drives it with `Instant::now()`.

On non-Linux targets `InputEngine::start`, `start_with` and `record_macro` return
`Error::Unsupported`. The model and remapper still build and work everywhere.

## Binding model

A profile holds a `BindingSet`. Keys without a binding pass through unchanged. Sample (JSON, as
stored in profiles):

```json
{
  "bindings": [
    { "source": "KEY_CAPSLOCK", "action": { "key": "KEY_LEFTCTRL" } },
    { "source": "BTN_SIDE", "action": { "combo": ["KEY_LEFTCTRL", "KEY_C"] } },
    { "source": "KEY_F13", "action": { "macro": {
        "steps": [ { "tap": "KEY_H" }, { "delay": 50 }, { "press": "KEY_LEFTSHIFT" },
                   { "tap": "KEY_I" }, { "release": "KEY_LEFTSHIFT" } ],
        "repeat": 1, "mode": "once" } } },
    { "source": "KEY_F14", "action": { "text": "Hello!\n" } },
    { "source": "KEY_F15", "action": { "launch": { "command": "firefox", "args": ["--new-window"] } },
      "mode": "release" },
    { "source": "KEY_F16", "action": { "media": "play_pause" } },
    { "source": "BTN_EXTRA", "action": { "mouse_button": "middle" } },
    { "source": "KEY_F17", "action": { "switch_profile": "gaming" } },
    { "source": "KEY_INSERT", "action": "disabled" },
    { "source": "KEY_F18", "action": "passthrough" }
  ]
}
```

The same structure works in TOML (`[[bindings]]`).

### Key names

Keys are evdev `EV_KEY` codes. They serialize as kernel names (`KEY_CAPSLOCK`, `BTN_SIDE`) and
parse case-insensitively, ignoring `_`, `-` and spaces, from:

- kernel names: `KEY_CAPSLOCK`, `BTN_EXTRA`
- names without the prefix: `capslock`, `a`, `f13`, `kp5`, `side`
- aliases: `ctrl`, `rctrl`, `shift`, `alt`, `altgr`, `super`/`win`/`meta`, `esc`, `return`,
  `pgup`, `printscreen`, `apps`, `num0`..`num9`, `mouse1`..`mouse8`, `lmb`/`rmb`/`mmb`, ...
- US punctuation: `-` `=` `[` `]` `\` `;` `'` `` ` `` `,` `.` `/`
- `CODE_<n>` for codes without a name; a bare JSON integer also works

`left`/`right`/`forward`/`back` are the keyboard keys; use `mouse1`/`BTN_LEFT` etc. for buttons.
`mouse4` = `BTN_SIDE`, `mouse5` = `BTN_EXTRA` (the usual back/forward thumb buttons).
`InputKey::from_key_id` maps the per-key RGB `KeyId` to evdev codes.

A combo may also be written as a string: `{ "combo": "ctrl+shift+esc" }`.

### Actions

| Action | Press | Repeat | Release |
|--------|-------|--------|---------|
| `key`, `combo`, `media`, `mouse_button` | press target key(s) in order | repeat last key | release in reverse |
| `macro` | start (see modes) | ignored | `while_held`: stop |
| `text` | type it (US layout) | ignored | — |
| `launch` | start program (no shell) | ignored | — |
| `switch_profile` | request switch | ignored | — |
| `disabled` | swallowed | swallowed | swallowed |
| `passthrough` / unbound | same key | same key | same key |

With `"mode": "release"` on the binding, the action fires on release instead; key-like actions
are then tapped (pressed and released after the tap duration). A `while_held` macro cannot use
release mode.

### Macros

Steps: `press`, `release`, `tap` (held for `RemapperConfig::tap_hold`, default 10 ms),
`delay` (milliseconds).

| Mode | Behaviour |
|------|-----------|
| `once` | plays `repeat` times (1..=1000); a trigger while it is playing is ignored |
| `while_held` | loops while the key is held; stops immediately on release |
| `toggle` | first trigger starts an endless loop; the next trigger stops it |

- Keys a macro still holds at the end of an iteration, or when it is stopped, are released.
- Iterations of a repeating macro start at least `min_loop_period` apart (default 10 ms), so a
  macro without delays cannot flood the system.
- A late tick shifts the rest of the macro; delays are never squeezed together.
- Limits: 4096 steps, 60 s per delay, 4096 text characters, 8 combo keys.

### Text

`text_to_steps` converts a string into steps for a **US layout**: shifted characters are wrapped
in Left Shift, `\n`/`\r\n`/`\r` become Enter, `\t` becomes Tab. Characters a US layout cannot type
directly (for example `é`, emoji) are rejected when the binding set is validated, with the
character and its position in the error.

### Overlapping keys

The virtual device keeps a reference count per key: a key is pressed when its first user presses
it and released when its last user releases it. A macro that presses and releases Shift while you
hold Shift does not release your Shift. Modifiers you hold combine with remapped keys (holding
Shift while pressing a key bound to `a` types `A`).

Each source key remembers what its press did. After `update()`, a key held under the old set
still releases what it pressed; running macros stop and release their keys.

## Engine

```rust
let engine = InputEngine::start(bindings)?;          // all SteelSeries devices
engine.update(new_bindings)?;                        // e.g. on profile switch
while let Ok(notice) = engine.notices().try_recv() { /* SwitchProfile, LaunchFailed, ... */ }
engine.stop()?;
let steps = InputEngine::record_macro(&DeviceFilter::steelseries(), InputKey::KEY_ESC, Duration::from_secs(30))?;
```

- **What is captured.** Only while the set is active (has a binding other than `passthrough`),
  and only devices matching the filter (default vendor `0x1038`) that report at least one bound
  key. Binding only mouse buttons leaves the keyboard alone, and the reverse. An empty set
  captures nothing.
- **Grab timing.** A device is grabbed only when none of its keys is down (checked with
  `EVIOCGKEY`, retried every 50 ms). Grabbing mid-press would send the release only to the engine
  and leave the key stuck for the rest of the system.
- **Re-emission.** Key and relative-axis events of a captured device go out through the virtual
  device: key events through the remapper (one `SYN_REPORT` each), relative axes (motion, wheel,
  hi-res wheel) batched per input frame. `EV_MSC` scan codes are dropped. If the virtual device advertises hi-res scrolling but the source mouse lacks
  it, the engine adds `REL_WHEEL_HI_RES` (`120 ×` detents) so scrolling still works.
- **Virtual device.** Named `ssgg virtual input`, bus `BUS_VIRTUAL`, vendor/product `0`, so it is
  never matched by the SteelSeries filter; the engine also skips any device with that name. It
  advertises keys 1..=255, `BTN_LEFT`..`BTN_TASK` and the keys/axes of devices matching the filter
  at start-up, minus joystick/gamepad/digitizer ranges.
- **Hot-plug.** Matching devices are rescanned every 2 s while the set is active. A disconnected
  device is dropped (all virtual keys are released first).
- **Launch.** `Command::new(command).args(args)`, no shell, stdio to `/dev/null`, its own process
  group; exited children are reaped. Programs inherit the engine's environment
  (`WAYLAND_DISPLAY`, `DISPLAY`), so a systemd user service needs those imported.
- **Profile switch.** The engine does not know profiles: it sends
  `EngineNotice::SwitchProfile(name)`; the owner loads that profile and calls `update()`.

### Safety

- **Never grabs for an empty (or passthrough-only) binding set.**
- **Any I/O error** (read, emit, poll) releases every virtual key, ungrabs every device and stops
  the engine (`EngineNotice::Stopped(StopReason::Error(..))`) rather than leaving input captured.
- **Crash, panic, `kill -9`:** a grab belongs to an open file descriptor. When the process dies,
  the kernel closes it, which ends the grab, and destroys the uinput device, which releases any key
  held on it. During a panic, `Drop` also ungrabs explicitly. The release profile uses
  `panic = "abort"`; the descriptors are still closed by the kernel.
- **Emergency escape:** hold **both Ctrl keys and both Shift keys for 2 seconds** on a captured
  keyboard. The engine releases every key, ungrabs everything and stops
  (`StopReason::EmergencyEscape`). It works whatever the bindings are, because it watches the
  physical keys before remapping.
- **Recording** grabs only for its own duration (bounded by `timeout`), ends on the stop key, and
  aborts on the escape chord. It waits for held keys to be released before grabbing.

## Permissions

The engine runs as your user; it does not need root.

1. **Input devices** (`/dev/input/event*`): be in the `input` group. Most distributions already
   create event nodes as `root:input 0660`; `assets/99-steelseries.rules` also does it for
   SteelSeries devices.

   ```sh
   sudo usermod -aG input "$USER"   # then log out and back in
   ```

2. **uinput** (`/dev/uinput`): it is usually `root:root 0600`. Add a udev rule:

   ```
   # /etc/udev/rules.d/99-ssgg-uinput.rules
   KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
   ```

   ```sh
   sudo udevadm control --reload-rules && sudo udevadm trigger
   # make sure the module loads at boot
   echo uinput | sudo tee /etc/modules-load.d/uinput.conf
   sudo modprobe uinput
   ```

`start()` fails early with `Error::PermissionDenied` when `/dev/uinput` cannot be opened or no
`/dev/input/event*` device can be opened at all, and with `Error::DeviceNotFound` when
`/dev/uinput` does not exist (module not loaded).

Group membership in `input` lets a process read every keyboard on the system. That is inherent to
any evdev remapper (keyd, kmonad, input-remapper).

## Limitations

- **Wayland and X11 both work.** The engine sits below the compositor (evdev/uinput), so it needs
  no compositor protocol and works the same under GNOME, KDE, wlroots compositors and X11.
- **Keyboard layouts.** Bindings work at the evdev code level, so `key`, `combo` and `macro` behave
  the same with any layout (a binding to `KEY_Q` sends the physical Q position; an AZERTY layout
  shows it as "a"). **`text` assumes a US layout**; with another layout the typed characters
  differ.
- **Text and held modifiers.** Typing text while you hold a modifier combines with it (holding
  Ctrl while text types "c" sends Ctrl+C).
- **Lock-key LEDs.** A grab blocks other readers, not writers; compositors that sync lock LEDs
  across keyboards should still light Caps Lock on the captured keyboard. Not verified.
- **New key codes after start-up.** The virtual device's capabilities are fixed when it is created.
  Keys of a device first connected later, outside keys 1..=255 and mouse buttons, are dropped.
- **Mixed hi-res mice** connected after start-up may scroll at a different speed.
- **Hardware macro keys** handled by keyboard firmware (`SteelSeriesKey`, volume wheel) are not
  remappable here unless the device reports them as evdev keys.
- **Two engines** cannot capture the same device; the second grab fails and that device is skipped.

## Tests

```sh
cargo test --locked --lib input
```

Covers key parsing and aliases, serde (JSON and TOML), validation, text conversion, the remapper
(remap, combo, macro timing with a fake clock, repeat counts and rate limit, while-held stop,
toggle, disabled, passthrough, release trigger, profile switch and launch requests, overlapping
keys, binding changes), the escape chord, recording conversion, device filtering and grab
decisions, and `Error::Unsupported` on non-Linux hosts.
