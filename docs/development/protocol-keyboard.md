# Apex Pro HID Protocol Reference

**VID:** `0x1038` (all SteelSeries devices)

---

## Supported Devices

Every keyboard PID maps to one lighting family in `src/devices/keyboards/protocol.rs`
(`keyboard_profile`). `GenericKeyboard` encodes reports by family. Only the Apex Pro TKL (2023)
was tested by this project; every other row is `[EXPERIMENTAL]`.

Verification uses `devices::settings::Verification`: **Hardware** = tested by this project,
**Reference** = byte layout from a published driver (OpenRGB unless stated), **Guess** = placed in
the family of its nearest sibling.

| Model | PID | Family | Lighting packet | Verification | Settings |
|-------|-----|--------|-----------------|--------------|----------|
| **Apex Pro TKL (2023)** | **`0x1628`** | legacy (+ feature-gated 0x40) | `0x21` zones, `0x22` brightness | **Hardware** | brightness (Hardware), actuation 0x2D (Guess), actuation_live 0x38 (Reference, apex-web) |
| Apex Pro TKL (2023) Wireless | `0x1630`, `0x1632` | per-key Gen 3 | `0x4B` init, `0x61` frame, 643 B | Reference | brightness if read-back answers, actuation (Guess) |
| Apex Pro TKL Gen 3 | `0x1642` | per-key Gen 3 | `0x4B` init, `0x40` frame, 643 B | Reference | as above |
| Apex Pro TKL Wireless Gen 3 | `0x1644`, `0x1646` | per-key Gen 3 | `0x4B` init, `0x61` frame, 643 B | Reference | as above |
| Apex Pro | `0x1610` | per-key Gen 1 | `0x3A` frame, 643 B | Reference | brightness if read-back answers, actuation (Guess) |
| Apex Pro TKL | `0x1614` | per-key Gen 1, Gen 3 from FW 1.19.7 | `0x3A` (or `0x4B` + `0x40`) | Reference | brightness if read-back answers, actuation (Guess), actuation_live 0x31 0x47 (Reference, apex-control), rapid_tap (Reference) |
| Apex Pro 3 (GG: "Apex Pro 2024") | `0x1640` | per-key Gen 1 | `0x3A` frame | Reference | brightness if read-back answers, actuation (Guess) |
| Apex 5 / Apex 7 / Apex 7 TKL | `0x161C` / `0x1612` / `0x1618` | per-key Gen 1 | `0x3A` frame | Reference | brightness if read-back answers |
| Apex 9 TKL / Apex 9 Mini | `0x1634` / `0x1620` | per-key Gen 2, Gen 3 from FW 1.19.7 | `0x40` frame, **513 B** | Reference | brightness if read-back answers |
| Apex Pro Mini | `0x161E` | per-key Gen 2 | `0x40` frame | Guess | brightness if read-back answers, actuation (Guess) |
| Apex Pro Mini Wireless | `0x1624`, `0x1626` | per-key Gen 3 wireless | `0x4B` + `0x61` | Guess | as above |
| Apex Pro Mini (2024) | `0x1648` | per-key Gen 3 | `0x4B` + `0x40` | Guess | as above |
| Apex 3 TKL | `0x1622` | 8-zone | `0x21 0xFF` + 8 x RGB, `0x23` brightness | Reference | brightness 0-16 |
| Apex 3 | `0x161A` | tri-zone (10 zones) | 33 B: `0x0B` colours, `0x0A` brightness | Reference | brightness 0-100, save_to_device |
| Apex M750 | `0x0616` | Apex M | 513 B grid frame | Reference | none |
| Apex (OG) / Fnatic, Apex 350 | `0x1202`, `0x1206` | old Apex (5 zones) | 33 B: `0x07` + 5 x RGBA | Reference | brightness 1-8 (apexctl), polling_rate (apexctl) |
| Apex 150, Apex 5 (2024), Apex 7 (2024) | `0x1616`, `0x1650`, `0x1652` | legacy | `0x21`, `0x22` | Guess | brightness 0-100 (Guess) |

> GG firmware calls PID `0x1628` "apex_pro_tkl_2022" regardless of purchase year.

- `0x0616`, `0x1202` and `0x1206` were added to `product_ids` from OpenRGB `SteelSeriesDevices.h`.
- OpenRGB does not detect the Apex Pro Mini PIDs, the Apex 150 or the 2024 Apex 5 / 7. The Pro
  Mini rows follow the Pro Mini SKUs in `SteelSeriesApexRegions.h` (Gen 2 wired) and the 2023 TKL
  wireless pair (Gen 3 wireless).
- OpenRGB applies no Gen 2 / Gen 3 quirk to `0x1640`, so it receives the Gen 1 `0x3A` frame.
- The 2023 wireless / Gen 3 TKL rows keep the transport of PR #290 (643-byte `0x4B` init, 50 ms
  pause, `send_feature` via interface 3). The PR states the Gen 3 wireless pair was tested on
  hardware by its contributor; this project has not tested it.

---

## Report Framing (all families)

Every report built by `HidReportBuilder` for a keyboard starts with the report ID `0x00`.

- **OpenRGB families** (per-key, 8-zone, tri-zone, M750, old Apex) write the built buffer as is
  (`GenericKeyboard::write_output_exact`, `send_feature_to_control`). The command byte is the first
  byte the keyboard receives, as with OpenRGB's `hid_write` / `hid_send_feature_report`.
- **Legacy family** (`0x1628` and the unreferenced PIDs) writes through `write_padded_report(...,
  65, true)`, which puts one more `0x00` in front of the built report and truncates to 65 bytes.
  On the wire this is `[0x00][cmd]...`: the command arrives as the second payload byte. This path
  is unchanged because the protocol notes record it as tested on `0x1628`. Apex 3 TKL (issue #173)
  used this path until it moved to the 8-zone family.

---

## OpenRGB Apex Families [EXPERIMENTAL]

Source: OpenRGB `Controllers/SteelSeriesController/` (GPL-2.0; facts only, no code copied). Rust:
`src/devices/hid_reports.rs` (commands), `src/devices/keyboards/protocol.rs` (PID table, frame),
`src/devices/keyboards/mod.rs` (`GenericKeyboard` encoding).

### Per-key family (`SteelSeriesApexController`)

**Direct frame** — feature report, `[0x00][packet][count][hid R G B] x count`, zero-padded:

| Generation | Packet | Init | Length |
|------------|--------|------|--------|
| Gen 1 (2019-22 case design) | `0x3A` | none | 643 |
| Gen 2 (2023 USB-C, Apex 9) | `0x40` (wireless `0x61`) | none | 643 (Apex 9: 513) |
| Gen 3 (Omnipoint 3.0, or Gen 1/2 with FW >= 1.19.7) | `0x40` (wireless `0x61`) | `[0x00][0x4B]` feature report, 65 B | 643 |

- The frame always carries all 112 LEDs of `APEX_LED_TABLE` in LED order (OpenRGB `keys[]`); the
  firmware ignores keys a model does not have. The Gen 1 layout matches apex7tkl_linux
  `send_colors` (`0x3A`, 642-byte payload, interface 1).
- **Firmware check** (`0x1614`, `0x1634`, `0x1620` only): output `[0x00][0x90]`, answer is ASCII
  `major.minor.patch` (leading `0x90` echo dropped). From 1.19.7 the model uses Gen 3.
- **Brightness**: output `[0x00][0x23][level]`, level `0..=10`. Offered only when the read-back
  answers: output `[0x00][0xA3]`, answer `[0xA3][0x00][level]`; status `0xFF` means unsupported.
  It applies to the on-board lighting; direct frames carry absolute colours.
- **Apply**: none. `GenericKeyboard::apply` is a no-op for every non-legacy family.
- `set_color`, `set_zone_colors` (one zone: the whole board), `set_key_colors` (partial update),
  `set_key_color(s)_direct`, `set_key_region`, `clear_per_key_rgb` and the new
  `Keyboard::set_all_key_colors` (full frame, unlisted keys off) all send this frame.

### 8-zone family (`SteelSeriesApex8ZoneController`, Apex 3 TKL)

65-byte output reports, interface 1, usage page `0xFFC0`:

| Command | Layout |
|---------|--------|
| Colours | `[0x00][0x21][0xFF][R G B] x 8` |
| Rainbow wave | `[0x00][0x22][0xFF]` (not used) |
| Brightness | `[0x00][0x23][0x00..=0x10]` |
| Brightness read | `[0x00][0xA3]`, answer `[0xA3][level]` (not used) |

OpenRGB writes brightness after a colour update whenever it differs from the last level written;
its cache starts unset, so the first colour update is followed by brightness (full, `0x10`, by
default). `GenericKeyboard` does the same. OpenRGB never sends `0x09` and notes the keyboard has
no persistent memory (a replug resets it to rainbow wave).

### Tri-zone family (`SteelSeriesApexTZoneController`, Apex 3)

33-byte output reports, interface 3. Every colour update sends brightness first:

| Command | Layout |
|---------|--------|
| Brightness | `[0x00][0x0A][0x00][0..=100]` |
| Colours | `[0x00][0x0B][0x00][R G B] x 10` |
| Save | `[0x00][0x06][0x00][0x08]` then `[0x00][0x09][0x00][0x00]` (setting `save_to_device`) |

### Apex M750 (`SteelSeriesApexMController`)

513-byte feature reports, interface 2.

- Enable (sent once): `00 00 00 00 01 00 85`, `00 00 00 00 03 01 00 FF`, `00 00 00 00 01 00 85 FF`
  (OpenRGB reuses the buffer, so byte 7 keeps `0xFF` in the third report).
- Direct frame: `00 00 00 01 8E 01 03 06 16`, then 132 slots (6 rows x 22, bottom row first) of
  `R G B`; slots without a key are `FF 32 00`. Slot positions: `APEX_M750_GRID`. OpenRGB's
  `keys_m` numpad indices predate the Japanese keys in its LED list and point five LEDs too low;
  this crate matches slots by the table's key labels.

### Old Apex (`SteelSeriesOldApexController`, Apex OG / Fnatic / 350)

33-byte output report, interface 0: `[0x00][0x07][0x00][R G B A] x 5`, zones QWERTY, ten-key,
function keys, MX keys, logo. `A` is the zone brightness: OpenRGB sends `0x08`; apexctl
(Apache-2.0) documents 1 = off, 2 = dimmest, 8 = brightest.

apexctl also documents a keyboard-side polling rate: feature report `[0x04][0x00][0..=3]`
(125 / 250 / 500 / 1000 Hz; `0x04` is the report ID). Setting `polling_rate`.

### Interface selection

OpenRGB matches Apex 3 (interface 3), Apex M750 (2) and old Apex (0) by interface number alone.
`control_score` in `discovery.rs` ranks that interface above every usage-page tier for those PIDs.

---

## Live Actuation and Rapid Tap [EXPERIMENTAL]

Both tools below were tested by their authors on one model each. Neither is tested by this
project. Both frames address the same 68 HID usages in the same order (`APEX_PRO_ACTUATION_KEYS`):
`0x04-0x28`, `0x2A-0x39`, `0x64`, `0x87-0x8B`, `0xE0-0xE7`, `0xF0` (no Escape, F-row, arrows or
navigation block).

| Model | Frame | Value encoding | Source |
|-------|-------|----------------|--------|
| Apex Pro TKL (2023) `0x1628`, FW 1.19.7 | feature, 645 B: `[0x00][0x38][0x61][68 LE16][hid A B] x 68` | `A = round(3.5696 + 11.2840x + 4.6982x^2 + 1.5643x^3)` clamped 5..=224 (x in mm); `B` = same curve at x - 0.1 mm (min 0.1) | apex-web (MIT), `index.html` `bloc0x38`, `capture-data.js` `KEY_ORDER` |
| Apex Pro TKL `0x1614`, FW 4.16.8 | feature, 643 B: `[0x00][0x31][0x47][hid lo hi] x 68` | u16 LE table captured from GG for 0.1-3.1, 3.3-3.6 and 4.0 mm (3.2 mm caused key-repeat spam); ISO / JP keys always `0x1F23` | apex-control (MIT), `src/ApexControl.Core/Actuation.cs` |

- apex-web: the `0x38` frame is live (lost on unplug) and must be sent alone; wrapping it in other
  commands cancels it. Setting `actuation_live` (Range 1..=40, 0.1 mm).
- apex-control: setting `actuation_live` is a Choice of the 36 captured depths (id = 0.1 mm).
- **Rapid Tap** (apex-control, `0x1614`): output `[0x00][0x1A][0x00 | 0x01]` switches it live.
  The key pairs live in the stored profile. Setting `rapid_tap`.
- No public source documents `0x2D` on any model. The `actuation` setting keeps the `0x2D` path
  and is labelled Guess everywhere.

### Rapid Trigger, Protection mode, per-key actuation

- Rapid Trigger and Protection mode exist only in the stored profile of the Apex Pro TKL (2023)
  (apex-web `PROTOCOL.md`): mode table at blob offsets 1446-1513 (0 normal, 2 Rapid Trigger,
  3 Protection, 4 both), sensitivity at 1516-1583 (mm x 10). Writing it means reading the
  12,500-byte profile blob, editing it and writing it back in 25 chunks with an STM32-style CRC-32.
  This crate does not implement that write.
- Per-key actuation: both frames above take one value per key; the settings send one value to
  every key.
- No byte-level reference was found for Rapid Trigger on Gen 3 / Mini models, for "dual actuation"
  / "2-in-1 action keys", or for a keyboard-side polling rate on modern Apex models.

---

## USB Interface Layout

The keyboard exposes 5 USB interfaces:

| Interface | Description | Used for |
|-----------|-------------|----------|
| MI_00 | Standard HID keyboard | Keypress events |
| **MI_01** | Vendor-defined HID | **RGB, actuation, all config** |
| MI_02 | Secondary HID | Additional keycodes |
| MI_03 | HID mouse | Mouse emulation keys |
| MI_04 | USB input device | Media/additional keys |

All configuration commands go to **MI_01** (usage page `0xFF00`, vendor-defined).

---

## Report Format

Standard keyboard output/feature reports are 65 bytes:

```
[0x00] [CMD] [DATA...] [0x00 padding...]
  ^      ^     ^-- up to 63 bytes of command data
  |      +-- command byte
  +-- report ID (always 0x00)
```

The per-key RGB command (`0x40`) uses a 645-byte feature report (see below).

---

## Command Reference

### Confirmed (hardware-tested on Apex Pro TKL 2023)

| Byte | Name | Format |
|------|------|--------|
| `0x09` | Apply/Save | `[0x00 0x09 0x00…]` — must follow any config command |
| `0x21` | Zone RGB | `[0x00 0x21 zone R G B R G B…]` |
| `0x22` | Brightness | `[0x00 0x22 brightness 0x00…]` — 0–100 |
| `0x24` | OSD Navigation | `[0x00 0x24 position 0x00…]` — opens actuation menu on OLED |
| `0x25` | Reactive Mode | `[0x00 0x25 0x01/0x00 0x00…]` — confirmed on Apex 3 TKL; compatible |
| `0x26` | Color Shift | `[0x00 0x26 R1 G1 B1 R2 G2 B2 speed 0x00…]` — speed 0–100 |
| `0x2D` | Actuation Write | `[0x00 0x2D value 0x00…]` — write only; read-back unknown |

**`0x21` zone selector:**
- `0xFF` = all zones simultaneously
- `0x00`–`0x08` = individual zone 0–8

**`0x2D` actuation encoding** (0.1 mm increments):
- `1` = 0.1 mm, `8` = 0.8 mm (default), `20` = 2.0 mm, `40` = 4.0 mm (max)
- GG UI exposes 0.4–3.6 mm; firmware accepts the full 0.1–4.0 mm range

---

### Confirmed — Per-Key RGB (`0x40`, Apex Pro TKL 2023)

Captured via `IOCTL_HID_SET_FEATURE` (code `0x000B0191`) from `SteelSeriesEngine.exe`, 2026-05-27.

**Packet layout — 645 bytes total:**

```
[0x00][0x40][0x54]  [hid R G B] × 84  [0x00 × 306]
  ^     ^     ^      ^----- 336 bytes ------^
  |     |     +-- count byte, always 0x54 = 84 (fixed)
  |     +-- command byte 0x40
  +-- report ID 0x00
```

- `hid`: USB HID keyboard Usage ID (e.g. `0x04` = A, `0x28` = Enter, `0xE0` = L Ctrl)
- `R G B`: 0–255 each
- Keys are in **physical layout order**, not ascending Usage ID order
- Trailing entries after the last key are zero-padded to reach 645 bytes
- Sent via `DeviceIoControl`/`NtDeviceIoControlFile` with `IOCTL_HID_SET_FEATURE`, **not** `WriteFile`
- Rust implementation: `Apex2023DirectCommand` in `src/devices/hid_reports.rs` (feature `experimental-apex-2023`)

---

### Placeholder (unconfirmed)

| Byte | Name | Notes |
|------|------|-------|
| `0x23` | Per-Key RGB | Placeholder, sent only by the legacy family (`0x1628` without the feature flag, and unreferenced PIDs). |

> ⚠️ UNVERIFIED: `0x23` is a placeholder command code. OpenRGB documents `0x23` as the
> brightness command of the Apex per-key and 8-zone families (`[0x00][0x23][level]`), so a
> `PerKeyRgbCommand` sent to those keyboards would be read as a brightness write with the key
> count as level. The per-key command of those families is the direct frame described above.
> Do not treat the `PerKeyRgbCommand` packet layout as confirmed hardware behavior.

---

### Unknown — Actuation Read-Back and Rapid Trigger

`discover_actuation` binary scanned codes `0x00`–`0xFF` on 2026-05-27:
- **No read-back command found** — no HID input response to any GET_FEATURE probe
- Read-back likely requires a query-then-interrupt-read pattern, or a different report type

apex-web reads the stored profile of `0x1628` with output `83 01 <0x80|slot> F4 01 <offset LE16>
00 00` followed by GET_REPORT; Rapid Trigger lives in that profile (see "Live Actuation and Rapid
Tap" above).

---

## Zone Mapping — Apex Pro TKL 2023

9 logical zones used with the `0x21` zone command:

| Zone | Position | Physical Area |
|------|----------|---------------|
| 0 | MainKeys (left) | Left side keys |
| 1 | MainKeys (left-center) | Left-center area |
| 2 | MainKeys (center) | Center area |
| 3 | MainKeys (right-center) | Right-center area |
| 4 | MainKeys (right) | Right side keys |
| 5 | FunctionRow | F1–F12 |
| 6 | Custom(0) — NumberRow | Number row (1–0) |
| 7 | Custom(1) — WASD | WASD cluster |
| 8 | ArrowKeys | Arrow key cluster |

Prism DB (`zone_cache`) confirms **84 individually addressable keys** on the US layout. The migration file lists 87 — the 3 extra are international-only keys (HID 50, 100, 133, 135–139) absent on US keyboards.

---

## Zone Mapping — Apex 3 TKL (`0x1622`, Issue #173)

`ssgg rgb solid` reports success on the Apex 3 TKL but nothing changes (multiple
users, Issue #173). The Apex 3 TKL is a zone keyboard, so the missing per-key
map is expected; the suspect is the zone-color command path.

[EXPERIMENTAL] (Reference: OpenRGB `SteelSeriesApex8ZoneController`). The Apex 3 TKL now uses the
8-zone family. Compared with the code that issue #173 was reported against:

| Item | Before | Now |
|------|--------|-----|
| Zones written | 9 (`RgbZoneCommand` with 9 colours) | 8 (`STEELSERIES_8Z_LED_COUNT`) |
| Framing | `[0x00][0x00][0x21][0xFF]...` (legacy extra `0x00`) | `[0x00][0x21][0xFF]...` as built |
| Brightness | `0x22`, never sent with colours | `0x23`, 0-16, sent after the first colour update and on change |
| `initialize()` / `apply()` | `0x09` apply | nothing (OpenRGB never sends `0x09` here) |
| Per-key / direct-address calls | `0x23` placeholder packets (read as brightness) | `Error::Unsupported` |

The zone names in `zone_mapping.rs` are this crate's approximation; OpenRGB names the eight zones
"LED 0" to "LED 7" in one linear zone. None of this is tested on an Apex 3 TKL.

---

## Key Addressing (USB HID Usage IDs)

Per-key RGB uses **USB HID Usage IDs** (keyboard page 7), not row/column matrix.

**OpenRGB LED table** (`APEX_LED_TABLE`, `src/devices/key_mapping.rs`): 112 LEDs in the order of
OpenRGB's `keys[]` / `led_names[]`, including Print Screen, Scroll Lock, Pause (`0x46-0x48`), ISO
`#` (`0x32`) and `\` (`0x64`), the Japanese keys (`0x87-0x8B`), the SteelSeries key (`0xF0`) and
the TKL media LED (`0xFB`). Key mappings per form factor (`ApexFormFactor`):

| Form factor | LEDs | Models |
|-------------|------|--------|
| Full size | 111 (all but `0xFB`) | Apex Pro, Apex 7, Apex 5, Apex Pro 3 |
| TKL | 92 (no numpad, no Print Screen / Scroll Lock / Pause, plus `0xFB`) | Apex Pro TKL, Apex 7 TKL, Apex 9 TKL, 2023 wireless and Gen 3 TKL |
| Mini | 68 (TKL without F-row, backtick, navigation block, arrows, `0xFB`) | Apex 9 Mini, Apex Pro Mini (all PIDs) |
| Apex M750 | 104 (labelled slots of `APEX_M750_GRID`) | Apex M750 |

Every set includes the 7 ISO / Japanese LEDs: OpenRGB picks the region from the serial number,
which this crate does not read, and the firmware ignores absent keys. These mappings replace the
GG-derived Apex Pro mapping (which listed a Menu key, `0x65`, that OpenRGB does not have on these
boards) and the empty Apex Pro TKL placeholder. The Apex Pro TKL (2023) keeps its GG-derived
mapping below.

**TKL key list** (from `apex_7+pro.migration`; US layout uses 84 of these):
```
4–69, 73–82, 100, 133, 135–139, 224–231, 240
```

**HID code → key reference:**

| Code | Key | Code | Key | Code | Key |
|------|-----|------|-----|------|-----|
| 4 | A | 29 | Z | 58 | F1 |
| 5 | B | 30 | 1 | 59 | F2 |
| 6 | C | 31 | 2 | 60 | F3 |
| 7 | D | 32 | 3 | 61 | F4 |
| 8 | E | 33 | 4 | 62 | F5 |
| 9 | F | 34 | 5 | 63 | F6 |
| 10 | G | 35 | 6 | 64 | F7 |
| 11 | H | 36 | 7 | 65 | F8 |
| 12 | I | 37 | 8 | 66 | F9 |
| 13 | J | 38 | 9 | 67 | F10 |
| 14 | K | 39 | 0 | 68 | F11 |
| 15 | L | 40 | Enter | 69 | F12 |
| 16 | M | 41 | Escape | 73 | Insert |
| 17 | N | 42 | Backspace | 74 | Home |
| 18 | O | 43 | Tab | 75 | Page Up |
| 19 | P | 44 | Space | 76 | Delete |
| 20 | Q | 45 | - _ | 77 | End |
| 21 | R | 46 | = + | 78 | Page Down |
| 22 | S | 47 | [ { | 79 | Right → |
| 23 | T | 48 | ] } | 80 | Left ← |
| 24 | U | 49 | \ \| | 81 | Down ↓ |
| 25 | V | 51 | ; : | 82 | Up ↑ |
| 26 | W | 52 | ' " | 224 | L Ctrl |
| 27 | X | 53 | ` ~ | 225 | L Shift |
| 28 | Y | 54 | , < | 226 | L Alt |
| 57 | Caps Lock | 55 | . > | 227 | L GUI (Win) |
| | | 56 | / ? | 228 | R Ctrl |
| | | | | 229 | R Shift |
| | | | | 230 | R Alt |
| | | | | 231 | R GUI (Win) |
| | | | | 240 | SteelSeries FN key |

Source: `SteelSeriesGG107.0.0Setup.exe` configuration migration files (`apex_7+pro.migration`), extracted 2026-03-27.

---

## How to Capture Unknown Commands

1. Open Wireshark; select USBPcap interface for the root hub containing `VID_1038/PID_1628`
2. Start capture
3. Trigger **one** action in SteelSeries GG (e.g. set actuation to 1.0 mm)
4. Stop capture; filter: `usb.transfer_type==0x01 && usb.endpoint_address.direction==0`
5. Extract `usb.capdata` from HID OUT reports
6. Hub path: `USB\ROOT_HUB30\4&5375334&0&0`

---

## Implementation Status

| Feature | Status | Rust location |
|---------|--------|---------------|
| Zone RGB (`0x21`) | ✅ Working | `src/devices/hid_reports.rs::RgbZoneCommand` |
| Brightness (`0x22`) | ✅ Working | `src/devices/hid_reports.rs::BrightnessCommand` |
| Apply (`0x09`) | ✅ Working | `src/devices/hid_reports.rs::ApplyCommand` |
| Actuation write (`0x2D`) | ✅ Experimental | `src/devices/keyboards/apex_pro_tkl_2023.rs` |
| Reactive mode (`0x25`) | ✅ Working | `src/devices/keyboards/apex.rs::Apex3Tkl` |
| Color shift (`0x26`) | ✅ Working | `src/devices/keyboards/apex.rs::Apex3Tkl` |
| Per-key RGB (`0x40`, Apex 2023) | ✅ Confirmed | `src/devices/hid_reports.rs::Apex2023DirectCommand` (feature `experimental-apex-2023`) |
| Per-key RGB (`0x23`, legacy family) | ⚠️ Placeholder | `src/devices/hid_reports.rs::PerKeyRgbCommand` |
| Per-key direct frame (`0x3A` / `0x40` / `0x61`) | [EXPERIMENTAL] Reference | `hid_reports.rs::ApexDirectCommand`, `keyboards/protocol.rs::ApexFrame` |
| Gen 3 init (`0x4B`), firmware / brightness read-back (`0x90` / `0xA3`) | [EXPERIMENTAL] Reference | `hid_reports.rs::ApexInitCommand`, `ApexQuery` |
| Illumination brightness (`0x23`) | [EXPERIMENTAL] Reference | `hid_reports.rs::IlluminationBrightnessCommand` |
| 8-zone / tri-zone / M750 / old Apex | [EXPERIMENTAL] Reference | `hid_reports.rs::{RgbZoneCommand, TriZone*, ApexM*, OldApex*}` |
| Live actuation (`0x38`, `0x31 0x47`), Rapid Tap (`0x1A`) | [EXPERIMENTAL] Reference | `hid_reports.rs::{ActuationLive2023Command, ActuationLiveGen1Command, RapidTapCommand}` |
| Settings per keyboard | [EXPERIMENTAL] | `Configurable` on `GenericKeyboard`, `Apex3Tkl`, `ApexProTkl2023` (ids in `keyboards::setting_ids`) |
| Actuation read-back | ❌ Unknown | Needs USB capture |
| Rapid Trigger | ❌ Not implemented | Profile write only (apex-web); see above |

---

## References

- [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) `Controllers/SteelSeriesController/` —
  source of the per-key, 8-zone, tri-zone, M750 and old Apex layouts (GPL-2.0, facts only)
- [apex-web](https://github.com/trottyva/apex-web) — Apex Pro TKL 2023 live actuation, profile
  blob layout (MIT)
- [apex-control](https://github.com/zunuza/apex-control) — Apex Pro TKL live actuation and Rapid
  Tap (MIT)

- [GameSense SDK](https://github.com/SteelSeries/gamesense-sdk) — official high-level API
- [apex-tux](https://github.com/not-jan/apex-tux) — Rust OLED support for Apex keyboards
- [apex7tkl_linux](https://github.com/FrankGrimm/apex7tkl_linux) — Python RGB + OLED for Apex 7 TKL
- [msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb) — detailed per-key protocol docs (MSI/SteelSeries)
- [apexctl](https://github.com/AstroSnail/apexctl) — C tool using hidapi-hidraw
- [USB HID 1.11 Specification](https://www.usb.org/sites/default/files/documents/hid1_11.pdf)
