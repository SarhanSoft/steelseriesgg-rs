# SteelSeries Device Reference

All SteelSeries devices use VID `0x1038`.

PIDs in this file are sourced from GG firmware folder names, which encode `(VID << 16) | PID`.
The tables list the low 16-bit USB PID only.
Verified against: GG 111.0.0 firmware directory + connected hardware (Apex Pro TKL 2023, 2026-05-26).

---

## Keyboards — ssgg Registry

| Constant | PID | Name | Zones | Per-Key RGB | Actuation |
|----------|-----|------|-------|-------------|----------|
| `APEX_PRO` | 0x1610 | Apex Pro | 1 | No | No |
| `APEX_PRO_TKL` | 0x1614 | Apex Pro TKL | 1 | No | No |
| **`APEX_PRO_TKL_2023`** | **0x1628** | **Apex Pro TKL (2023)** | **9** | **0x40 ✅** | **0x2D ✅** |
| `APEX_PRO_TKL_2023_WIRELESS` | 0x1632 | Apex Pro TKL (2023) Wireless | 9 | ? | ? |
| `APEX_PRO_TKL_2023_WIRELESS_2` | 0x1630 | Apex Pro TKL (2023) Wireless (dongle) | 9 | ? | ? |
| `APEX_3` | 0x161A | Apex 3 | 10 | No | No |
| `APEX_3_TKL` | 0x1622 | Apex 3 TKL | 9 | No | No |
| `APEX_5` | 0x161C | Apex 5 | 1 | No | No |
| `APEX_7` | 0x1612 | Apex 7 | 1 | No | No |
| `APEX_7_TKL` | ~~0x1616~~ **→ 0x1618** | Apex 7 TKL | 1 | No | No |

**Connected hardware**: Apex Pro TKL 2023 (PID 0x1628). All others unverified.

**Zone count note**: `APEX_PRO`, `APEX_PRO_TKL`, `APEX_5`, `APEX_7`, `APEX_7_TKL` all return 1 zone in
`zone_count_for_product_id()`. These devices likely have more — pending USB capture or GG.Models.dll decompilation.

---

## Headsets — ssgg Registry [EXPERIMENTAL]

The source of truth is `src/devices/headsets/models.rs`. Every PID below has one `HeadsetModel`
entry there, and `device_type_from_product_id` / `device_name_from_product_id` read that table.
No headset has been tested on hardware by this project. Every setting is
`Verification::Reference` (bytes taken from a published driver) unless the table says *Guess*.

Wireless models are controlled through the dongle or base-station PID (GG "tx"). The
headset-side "rx" PIDs carry no control protocol and are not listed.

Sources, used for facts only (no code copied): HeadsetControl `lib/devices/steelseries_*.hpp`
(GPL-3.0), OpenRGB `SteelSeriesArctis5Controller` and `SteelSeriesArctisNova3Controller`
(GPL-2.0), nova-chatmix-linux (0BSD).

### Models and capabilities

Ranges are the device's native steps. "save" is an explicit `Action` for models whose reference
leaves some settings unsaved.

| Family | PID — constant | Interface / usage page | Settings | Status readings | Source |
|--------|----------------|------------------------|----------|-----------------|--------|
| Arctis 1 | 0x12B3 `ARCTIS_1_WIRELESS`, 0x12B6 `ARCTIS_1_WIRELESS_XBOX`, 0x12D5 `ARCTIS_7P`, 0x12D7 `ARCTIS_7X` | 3 / 0xFF43 | sidetone 0-18, inactive_time 0-90 min | battery, link | HeadsetControl `steelseries_arctis_1.hpp` |
| Arctis 7 | 0x1260 `ARCTIS_7`, 0x12AD `ARCTIS_7_2019`, 0x1252 `ARCTIS_PRO`, 0x1280 `ARCTIS_PRO_GAMEDAC` | 5 | sidetone 0-18, inactive_time 0-90 min, lights | ChatMix; battery on 0x1260 and 0x12AD only | HeadsetControl `steelseries_arctis_7.hpp` |
| Arctis 5 | 0x1250 `ARCTIS_5_2017`, 0x12AA `ARCTIS_5` | 5 | led_colors (left, right) | — | OpenRGB `SteelSeriesArctis5Controller` |
| Arctis 9 | 0x12C2 `ARCTIS_9` | 0 | sidetone 0-61, inactive_time 0-255 min | battery, charging, link, ChatMix | HeadsetControl `steelseries_arctis_9.hpp` |
| Arctis Pro Wireless | 0x1290 `ARCTIS_PRO_WIRELESS` | 0 | sidetone 0-9, inactive_time 0-250 min in steps of 10 | battery, link | HeadsetControl `steelseries_arctis_pro_wireless.hpp` |
| Arctis Nova Pro Wireless | 0x12E0 `ARCTIS_NOVA_PRO_WIRELESS`, 0x12E5 `ARCTIS_NOVA_PRO_WIRELESS_XBOX` | 4 | sidetone 0-3, lights, inactive_time (0, 1, 5, 10, 15, 30, 60 min), equalizer_preset (4 device slots), equalizer 10 bands ±10 dB, chatmix_dial, sonar_icon, save | battery, charging, link, ChatMix (from dial messages once chatmix_dial is on) | HeadsetControl `steelseries_arctis_nova_pro_wireless.hpp`; nova-chatmix-linux |
| Arctis 7+ | 0x220E `ARCTIS_7_PLUS`, 0x2212 `ARCTIS_7_PLUS_PS5`, 0x2216 `ARCTIS_7_PLUS_XBOX`, 0x2236 `ARCTIS_7_PLUS_DESTINY` | 3 / 0xFFC0 | sidetone 0-3, inactive_time 0-90 min, equalizer_preset (flat, bass, smiley, focus), equalizer 10 bands ±12 dB | battery, charging, link, ChatMix | HeadsetControl `steelseries_arctis_7_plus.hpp` |
| Arctis Nova 3 | 0x12EC `ARCTIS_NOVA_3` | 4 / 0xFFC0 | sidetone 0-3, mic_volume 0-10, mic_mute_led_brightness 0-3, equalizer_preset, equalizer 6 bands ±6 dB, led_colors, led_effect, led_effect_speed 1-10, save | — | HeadsetControl `steelseries_arctis_nova_3.hpp`; OpenRGB `SteelSeriesArctisNova3Controller` |
| Arctis Nova 3P / 3X Wireless | 0x2269 `ARCTIS_NOVA_3P_WIRELESS`, 0x226D `ARCTIS_NOVA_3X_WIRELESS` | 3 / 0xFFC0 | sidetone 0-10, inactive_time (0, 1, 5, 10, 15, 30, 45, 60, 75, 90 min), mic_volume 0-14, equalizer_preset, equalizer 10 bands ±12 dB in 0.1 dB | battery, charging, link | HeadsetControl `steelseries_arctis_nova_3p_wireless.hpp` |
| Arctis Nova 5 / 5X | 0x2232 `ARCTIS_NOVA_5`, 0x2253 `ARCTIS_NOVA_5X` | 3 / 0xFFC0 | sidetone 0-10, inactive_time 0-255 min, mic_volume 0-15, mic_mute_led_brightness (off, low, medium, high), volume_limiter, equalizer_preset, equalizer 10 bands ±10 dB, save | battery, charging, link, ChatMix | HeadsetControl `steelseries_arctis_nova_5.hpp` |
| Arctis Nova 7 | battery in 4 steps: 0x2202 `ARCTIS_NOVA_7`, 0x2206 `ARCTIS_NOVA_7X`, 0x223A `ARCTIS_NOVA_7_DIABLO_IV`, 0x227A `ARCTIS_NOVA_7_WOW`, 0x22A4 `ARCTIS_NOVA_7X_ALT`; battery in %: 0x22A1 `ARCTIS_NOVA_7_V2`, 0x2258 `ARCTIS_NOVA_7X_V2`, 0x229E `ARCTIS_NOVA_7X_GEN2`, 0x22A5 `ARCTIS_NOVA_7X_ALT_V2`, 0x22A9 `ARCTIS_NOVA_7_DIABLO_IV_V2`, 0x22AD `ARCTIS_NOVA_7X_V2_ALT` | 3 / 0xFFC0 | sidetone 0-3, inactive_time 0-255 min, equalizer_preset (flat, bass, focus, smiley), equalizer 10 bands ±10 dB in 1 dB, mic_mute_led_brightness 0-3, mic_volume 0-7, volume_limiter, bluetooth_when_powered_on, bluetooth_call_volume, save (*Guess*) | battery, charging, link, ChatMix | HeadsetControl `steelseries_arctis_nova_7.hpp` |
| Arctis Nova 7 Gen 2 | 0x227E `ARCTIS_NOVA_7_GEN2` | 3 / 0xFFC0 | as Nova 7; sidetone is saved and save is Reference | as Nova 7, plus the sidetone level (`extra["sidetone"]`) | HeadsetControl `steelseries_arctis_nova_7.hpp` |
| Arctis Nova 7P | battery in 4 steps: 0x220A `ARCTIS_NOVA_7P`; battery in %: 0x22A7 `ARCTIS_NOVA_7P_V2`, 0x2298 `ARCTIS_NOVA_7P_GEN2` | 3 / 0xFFC0 | as Nova 7 without sidetone | battery, charging, link | HeadsetControl `steelseries_arctis_nova_7p.hpp` |
| Arctis GameBuds | 0x230A `ARCTIS_GAMEBUDS` | 3 / 0xFFC0 | — | battery of the lower active bud, per-bud battery (`extra["battery_left"]`, `extra["battery_right"]`), link | HeadsetControl `steelseries_arctis_gamebuds.hpp` |
| Name only | 0x12CB `ARCTIS_NOVA_PRO_WIRED`, 0x2290 `ARCTIS_NOVA_PRO_OMNI` | 3 (not from a reference) | — | — | GG firmware registry |

Discovery opens only the model's interface. On that interface the named usage page wins, then
any vendor-defined page (`headset_control_score` in `src/devices/discovery.rs`).

### Wire format per family

Bytes are exactly what goes to `hid_write` (out) or `hid_send_feature_report` (feature); the
first byte is the report ID. Reports are zero-padded to the frame length. "+ save" means the
family's save sequence follows.

| Family | Frame | Save sequence | Sidetone | Auto-off | Other commands and status |
|--------|-------|---------------|----------|----------|---------------------------|
| Arctis 1 | out 31 | `06 09` after every setting | `06 35 01 00 lv`; 0 sends `06 35` | `06 53 min` | status `06 12` → [2] `01` offline, [3] battery % |
| Arctis 7 | out 31 | `06 09` after every setting | as Arctis 1 | `06 51 min` | lights `06 55 01 02` / `06 55 01 00`; battery `06 18` → [2] %; ChatMix `06 24` → [2] game, [3] chat (191-255, 0 = full) |
| Arctis 5 | out 37 | none | — | — | per cup: `06 81 43 01 22`, `06 81 43 01 23`, `06 8A 42 00 20 41 00 R G B FF 32 C8 C8`, `06 8A 42 00 20 41 08 zone 01`, `06 8A 42 00 20 60 zone`, `06 8A 42 00 20 05` |
| Arctis 9 | out 31 | `90 00` after every setting | `06 00 (C0 + lv)` | `04 00 secHi secLo` | status `00 20` → [3] battery 0x64-0x9A, [4] `01` charging / `FF` offline, [9] [10] ChatMix 0-19 |
| Arctis Pro Wireless | out 31 | `90 AA` after every setting | `39 AA lv` | `3C AA min/10` | link `41 AA` → [0] `02` offline; battery `40 AA` → [0] 0-4 |
| Nova Pro Wireless | out 31; out 63 for dial and icon | `06 09` after sidetone, lights, auto-off, preset | `06 39 lv` | `06 C1 idx` | lights `06 BF 0A` / `06 BF 01`; preset `06 2E idx`; custom EQ `06 2E 04` (3 bytes) then `06 33` + 10 × (0x14 + 2·dB), unsaved; dial `06 49 b`; icon `06 8D b`; status `06 B0` → [6] 0-8, [15] `01` offline / `02` charging; dial message `07 45 game chat` |
| Arctis 7+ | out 64 | `00 09` after every setting | `00 39 lv` | `00 A3 min` | preset and EQ `00 33` + 10 × (0x18 + 2·dB); status `00 B0` → [1] `01` offline, [2] 0-4, [3] `01` charging, [4] [5] ChatMix 0-100 |
| Nova 3 | feature 64; lighting out 521 / 64 | `06 09` (feature) after all but custom EQ | `06 39 lv` | — | mic `06 37 lv`; mute LED `06 AE lv`; preset and EQ `06 33` + 6 × (0x14 + 2·dB); lighting `06 AA` effect packet, `06 A5 zone state`, apply `06 09` (right) / `06 A3` (left) |
| Nova 3P Wireless | out 64 | `00 09` after every setting | `00 39 lv` | `00 A3 min` | mic `00 37 lv`; preset and EQ `00 33` + 10 × (freq LE, `01`, gain in 0.1 dB two's complement, Q 1414 LE); status `00 B0` → [1] `02` offline, [3] %, [4] `01` charging |
| Nova 5 | out 64 | `00 09` then `00 35 01`, after all but auto-off | `00 39 lv` | `00 A3 min` | mic `00 37 lv`; mute LED `00 AE 00/01/04/0A`; limiter `00 27 b`; preset and EQ `00 33` + 10 × (freq LE, flag `01` / `04` first band / `05` last band, 20 + 2·dB, Q LE); status as Nova 3P plus [5] [6] ChatMix |
| Nova 7 | out 64 | none, except `00 09` after sidetone on the Gen 2 and `06 09` after Bluetooth power-up | `00 39 lv` | `00 A3 min` | preset and EQ `00 33` + 10 × (0x14 + dB); mute LED `00 AE lv`; mic `00 37 lv`; limiter `00 3A b`; Bluetooth power-up `00 B2 b`; call volume `00 B3 0/1/2`; status `00 B0` → [2] battery, [3] `00` offline / `01` `02` charging, [4] [5] ChatMix; Gen 2 sidetone `00 20` → `20 .. lv` |
| GameBuds | — | — | — | — | status `00 B0` → [3] [4] bud state (`03` active), [5] [6] bud battery % |

Status requests use a 100 ms read timeout and skip unsolicited reports, so a silent headset
never blocks the daemon. Nova-family answers are matched on their leading `B0` byte.

ChatMix is reported as the reference reads it: two volumes of 0-100. At the dial's centre both
are 100, so they do not sum to 100.

Equalizer band frequencies are display labels. The device receives only gains, except on the
Nova 3P and Nova 5, whose packet carries the frequency. The Nova 3's six labels are
placeholders, since the reference gives no frequencies for it.

None of the references defines voice prompts, rotate-to-mute, ANC / transparency or a mic-mute
reading for a SteelSeries model, so the code offers none of them. The parametric equalizer of
the Nova 3P and Nova 5 is not offered either: the settings model has no parametric kind, and the
custom equalizer above uses the reference's fixed bands.

### PID corrections

- `0x12AD` is the Arctis 7 (2019), now `ARCTIS_7_2019`. It used to be labelled `ARCTIS_1`. The
  Arctis 1 Wireless is `0x12B3`.
- `0x12E0` is the Nova Pro Wireless base station, now `ARCTIS_NOVA_PRO_WIRELESS` (was
  `ARCTIS_NOVA_PRO`). The wired Nova Pro is `0x12CB`, `ARCTIS_NOVA_PRO_WIRED`.
- `ARCTIS_NOVA_5` is `0x2232`, the Nova 5 base station. `0x12EA` was wrong.
- `0x12CF` (old `ARCTIS_7_2019`), `0x12E4` (old `ARCTIS_NOVA_PRO_WIRELESS`), `0x12EA` and
  `0x12EE` (old `ARCTIS_NOVA_1`) had no source and are no longer headsets.
- The rx PIDs `0x2200`, `0x2204`, `0x2208`, `0x2230` and `0x2267` are no longer listed.

---

## Confirmed Bugs in ssgg `product_ids`

### Bug 1 — `APEX_7_TKL` has wrong PID (HIGH SEVERITY)

```rust
// Wrong:
pub const APEX_7_TKL: u16 = 0x1616;  // 0x1616 is apex_150 in GG firmware

// Correct:
pub const APEX_7_TKL: u16 = 0x1618;  // 0x1618 is apex_7_tkl in GG firmware
// Also add:
pub const APEX_150: u16 = 0x1616;
```

---

## Full GG Keyboard PID Table

All Apex keyboard PIDs from GG firmware, including devices not yet in ssgg:

| PID | GG firmware name | ssgg const |
|-----|-----------------|----------|
| 0x1200 | apex-raw | — |
| 0x1202 | apex | — |
| 0x1206 | apex-350 | — |
| 0x1208 | apex-300 | — |
| 0x1600 | apex_m800 | — |
| 0x1607 | apex_m500 | — |
| 0x160C | apex_m400 | — |
| 0x160E | apex_100 | — |
| 0x1610 | apex_pro | `APEX_PRO` |
| 0x1612 | apex_7 | `APEX_7` |
| 0x1614 | apex_pro_tkl | `APEX_PRO_TKL` |
| 0x1616 | apex_150 | `APEX_7_TKL` ❌ (wrong device) |
| 0x1618 | apex_7_tkl | — ❌ (missing) |
| 0x161A | apex_3 | `APEX_3` |
| 0x161C | apex_5 | `APEX_5` |
| 0x161E | apex_pro_mini | — |
| 0x1620 | apex_9_mini | — |
| 0x1622 | apex_3_tkl | `APEX_3_TKL` |
| 0x1624 | apex_pro_mini_wireless_dongle | — |
| 0x1626 | apex_pro_mini_wireless | — |
| 0x1628 | apex_pro_tkl_2022 | `APEX_PRO_TKL_2023` ✅ |
| 0x1630 | apex_pro_tkl_wireless_dongle | `APEX_PRO_TKL_2023_WIRELESS_2` ✅ |
| 0x1632 | apex_pro_tkl_wireless | `APEX_PRO_TKL_2023_WIRELESS` ✅ |
| 0x1634 | apex_9_tkl | — |
| 0x1640 | apex_pro_2024 | — |
| 0x1642 | apex_pro_tkl_2024 | — |
| 0x1644 | apex_pro_tkl_wireless_2024_dongle | — |
| 0x1646 | apex_pro_tkl_wireless_2024 | — |
| 0x1648 | apex-pro-mini-2024 | — |
| 0x1650 | apex_5_2024 | — |
| 0x1652 | apex_7_2024 | — |

> GG firmware internally calls PID 0x1628 "apex_pro_tkl_2022" regardless of the purchase year branding.

---

## Full GG Headset PID Table

| PID | GG firmware name | ssgg const |
|-----|-----------------|----------|
| 0x1240 | siberia_840 | — |
| 0x1250 | arctis_5 (2017) | `ARCTIS_5_2017` |
| 0x1252 | arctis_pro | `ARCTIS_PRO` |
| 0x1260 | arctis_7_dongle | `ARCTIS_7` |
| 0x1261 | arctis_7 (2017) | — |
| 0x1290 | arctis_pro_wireless | `ARCTIS_PRO_WIRELESS` |
| 0x1292 | arctis_pro_wireless_headset | — |
| 0x12AA | arctis_5_2018 | `ARCTIS_5` |
| 0x12AD | arctis_7_2018_tx | `ARCTIS_7_2019` |
| 0x12AE | arctis_7_2018_rx | — |
| 0x12B1 | arctis_9x | — |
| 0x12B3 | arctis_1w_tx | `ARCTIS_1_WIRELESS` |
| 0x12B4 | arctis_1w_rx | — |
| 0x12B6 | arctis_1x_tx | `ARCTIS_1_WIRELESS_XBOX` |
| 0x12B7 | arctis_1x_rx | — |
| 0x12C0 | arctis_9_rx | — |
| 0x12C2 | arctis_9_tx | `ARCTIS_9` |
| 0x12CB | arctis_nova_pro | `ARCTIS_NOVA_PRO_WIRED` (name only) |
| 0x12CD | arctis_nova_pro_xbox | — |
| 0x12CF | (unknown) | — (removed: no source) |
| 0x12D5 | arctis_7p_tx | `ARCTIS_7P` |
| 0x12D6 | arctis_7p_rx | — |
| 0x12D7 | arctis-7x-tx | `ARCTIS_7X` |
| 0x12D8 | arctis-7x-rx | — |
| 0x12E0 | arctis_nova_pro_wireless_tx | `ARCTIS_NOVA_PRO_WIRELESS` |
| 0x12E2 | arctis_nova_pro_wireless_rx | — |
| 0x12E5 | arctis_nova_pro_wireless_xbox_tx | `ARCTIS_NOVA_PRO_WIRELESS_XBOX` |
| 0x12E8 | arctis_nova_pro_wireless_xbox_rx | — |
| 0x12EC | arctis_nova_3 | `ARCTIS_NOVA_3` |
| 0x12F0 | arctis_nova_4_rx | — |
| 0x12F2 | arctis_nova_4_tx | — |
| 0x12F4 | arctis_nova_4x_rx | — |
| 0x12F6 | arctis_nova_4x_tx | — |
| 0x12FA | arctis_nova_pro_v2 | — |
| 0x2200 | arctis_nova_7_rx | — |
| 0x2202 | arctis_nova_7_tx | `ARCTIS_NOVA_7` |
| 0x2204 | arctis_nova_7x_rx | — |
| 0x2206 | arctis_nova_7x_tx | `ARCTIS_NOVA_7X` |
| 0x2208 | arctis_nova_7p_rx | — |
| 0x220A | arctis_nova_7p_tx | `ARCTIS_NOVA_7P` |
| 0x220C | arctis_7_plus_rx | — |
| 0x220E | arctis_7_plus_tx | `ARCTIS_7_PLUS` |
| 0x2210 | arctis_7p_plus_rx | — |
| 0x2212 | arctis_7p_plus_tx | `ARCTIS_7_PLUS_PS5` |
| 0x2214 | arctis_7x_plus_rx | — |
| 0x2216 | arctis_7x_plus_tx | `ARCTIS_7_PLUS_XBOX` |
| 0x2230 | arctis_nova_5_rx | — (rx; no control protocol) |
| 0x2232 | arctis_nova_5_tx | `ARCTIS_NOVA_5` |
| 0x2238 | arctis_nova_7_diablo_iv_rx | — |
| 0x2240 | arctis_nova_pro_wireless_v2_rx | — |
| 0x2244 | arctis_nova_elite_tx | — |
| 0x2249 | arctis_nova_elite_rx | — |
| 0x2251 | arctis_nova_5x_rx | — |
| 0x2253 | arctis_nova_5x_tx | `ARCTIS_NOVA_5X` |
| 0x2267 | arctis_nova_3_wireless_rx | — |
| 0x2269 | arctis_nova_3_wireless_tx | `ARCTIS_NOVA_3P_WIRELESS` |
| 0x2288 | arctis_nova_7p_gen2_rx | — |
| 0x2290 | arctis_nova_pro_omni_tx | `ARCTIS_NOVA_PRO_OMNI` (name only) |
| 0x2296 | arctis_nova_pro_omni_rx | — |
| 0x2298 | arctis_nova_7p_gen2_tx | `ARCTIS_NOVA_7P_GEN2` |
| 0x227C | arctis_nova_7_gen2_rx | — |
| 0x227E | arctis_nova_7_gen2_tx | `ARCTIS_NOVA_7_GEN2` |
| 0x229C | arctis_nova_7x_gen2_rx | — |
| 0x229E | arctis_nova_7x_gen2_tx | `ARCTIS_NOVA_7X_GEN2` |
| 0x230A | arctis_gamebuds_dongle | `ARCTIS_GAMEBUDS` |
| 0x230C | arctis_gamebuds_case | — |

HeadsetControl also lists PIDs this GG registry snapshot lacks: 0x1280 (`ARCTIS_PRO_GAMEDAC`), 0x2236 (`ARCTIS_7_PLUS_DESTINY`), 0x223A (`ARCTIS_NOVA_7_DIABLO_IV`), 0x2258 (`ARCTIS_NOVA_7X_V2`), 0x226D (`ARCTIS_NOVA_3X_WIRELESS`), 0x227A (`ARCTIS_NOVA_7_WOW`), 0x22A1, 0x22A4, 0x22A5, 0x22A7, 0x22A9 and 0x22AD (Nova 7 family revisions). All of them are in `headsets::MODELS`.

---

## Key Mapping — Apex Pro TKL 2023

Source: Prism `zone_cache`, device_id=242, 2026-05-26.

`src/devices/key_mapping.rs` TKL 2023 mapping is largely correct with one discrepancy:

| HID Code | ssgg mapping | Prism zone_cache | Status |
|----------|-------------|-----------------|--------|
| 101 | `Menu` | Absent | ⚠️ Key doesn't exist on this keyboard |
| 240 | `SteelSeriesKey` (FN) | Present | ✅ |
| 50, 100, 133, 135–139 | In `tkl_hid_codes` | Absent | International layout only |

Prism confirms **84 addressable keys** on the US layout (the migration file lists 87 — the 3 extra are international-only keys absent on US keyboards).

`zone_count_for_product_id()` returning 9 for `APEX_PRO_TKL_2023` is correct — these are the 9 logical RGB zones, not the total key count.

### Open questions

- Which keyboards in the registry support per-key RGB? (requires GG.Models.dll decompilation)
- What are the correct zone counts for `APEX_PRO`, `APEX_PRO_TKL`, `APEX_5`, `APEX_7`, `APEX_7_TKL`?
- Should `Menu (HID 101)` be removed from the TKL 2023 key mapping?

---

## Prioritized ssgg Updates

1. **Fix `APEX_7_TKL` PID** (0x1616 → 0x1618) — correctness bug affecting all Apex 7 TKL users
2. **Add Apex Pro 2024 variants** (0x1640–0x1652)
3. **Add Apex Pro Mini variants** (0x161E, 0x1620, 0x1624, 0x1626)
