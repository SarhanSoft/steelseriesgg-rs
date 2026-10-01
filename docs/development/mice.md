# Mice [EXPERIMENTAL]

Mouse support covers every mouse in [rivalcfg](https://github.com/flozz/rivalcfg) (commit
`f16c521`): 33 device files, 76 USB product IDs. **None of it has been tested on hardware by this
project.** The protocol comes from rivalcfg, and every report is checked byte for byte against
rivalcfg's own test expectations.

Code: `src/devices/mice/`.

| File | Content |
|---|---|
| `mod.rs` | `MODELS` (one entry per PID), `Mouse` trait, `open()` |
| `profiles.rs` | One `Profile` per rivalcfg device file: commands, ranges, defaults, LED zones |
| `profile.rs` | Turns a profile into setting descriptors and reports |
| `encode.rs` | One typed encoder per rivalcfg value handler |
| `dpi_tables.rs`, `keys.rs` | rivalcfg's sensor DPI tables and button key names |
| `device.rs` | `SteelSeriesMouse`: the single implementation every model uses |
| `handler_tests.rs`, `device_spec_tests.rs` | Ports of rivalcfg's handler and device tests |

## Verification

Every setting descriptor is `Verification::Reference`: command bytes, value encodings, ranges and
defaults are rivalcfg's. The tests reproduce:

- rivalcfg's handler tests (`test/handlers/*.py`) for each encoder;
- every case of rivalcfg's device tests (`test/devices/old_specs/*.py`, `test/devices/specs/*.txt`)
  for all 33 profiles, run through the same profile setting here;
- each setting's default value and each save, battery and firmware command, as rivalcfg encodes
  them (computed by running rivalcfg, for the few commands its tests do not cover).

Three choices are this project's, not rivalcfg's:

- direct LED color frames (`Mouse::set_zone_colors_direct`) are spaced 2 ms apart, following
  OpenRGB, instead of rivalcfg's 50 ms between settings. Untested at animation rates;
- direct colors on gradient LEDs are sent as rivalcfg's one-color gradient, one feature report
  per zone;
- a `reactive_color` of black sends rivalcfg's "off" bytes, because the settings model has no
  "off" color.

## How settings map to rivalcfg

Setting ids are rivalcfg's setting names, with these additions:

- `dpi` combines DPI presets: rivalcfg's multi-stage `sensitivity` command, or its separate
  `sensitivity1` / `sensitivity2` commands (stage 1 sets preset 1, stage 2 preset 2). Mice whose
  presets are a fixed list (Rival 95/100, Kana v2, Kinzu v2) keep `sensitivity1` and
  `sensitivity2` as choices.
- `colors` sets every LED zone at once, one command per zone; each zone also keeps its own
  setting (`z1_color`, `logo_color`, ...).
- On mice whose LEDs take gradients (Rival 310/500/600/700, Sensei 310/TEN), `<zone>_color` is a
  steady color, `<zone>_gradient` a looping gradient of three colors at 0 %, 33 % and 66 %
  (rivalcfg's default positions), and `gradient_duration` the cycle time in ms.
- `buttons` takes only the buttons to change, as `button=action`, all lowercase; the others keep
  their defaults. Actions are another button, `disabled`, `dpi`, `scrollup`/`scrolldown`, a QWERTY
  key or a media key, with rivalcfg's aliases.
- `save` writes the current settings to the mouse's memory. rivalcfg's CLI sends this after every
  change; here it is a separate action, so the mouse's flash is written only when asked. Settings
  report `persists_on_device = true` when the mouse has a save command, meaning they persist once
  `save` has been run.

In 2.4 GHz mode, dual-mode mice get rivalcfg's treatment: `0x40` is OR-ed into the first command
byte and a 64-byte reply is read and discarded after each setting.

Feature reports (gradient LEDs, Rival 500/600 buttons) go through the raw `HIDIOCSFEATURE` ioctl
on Linux (`devices::send_feature_report_raw`) and hidapi elsewhere. Output reports use
`hid_write` with report ID `0x00`.

`read_status()` returns the battery level and charging flag where rivalcfg has a battery command,
and the firmware version in `extra["firmware"]` where it has a firmware command.

## Profiles

| Profile | Settings | Status read | Ported from | Tests ported from |
|---|---|---|---|---|
| `AEROX_3` Aerox 3 | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `led_brightness`, `buttons` (8), `save` | - | `rivalcfg/devices/aerox3.py` | `test/devices/old_specs/test_aerox3.py` |
| `AEROX_3_WIRELESS_WIRED` Aerox 3 Wireless (wired) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (8), `save` | battery | `rivalcfg/devices/aerox3_wireless_wired.py` | `test/devices/specs/aerox3_wireless_wired.txt` |
| `AEROX_3_WIRELESS_WIRELESS` Aerox 3 Wireless (2.4 GHz) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (8), `save` | battery | `rivalcfg/devices/aerox3_wireless_wireless.py` | `test/devices/specs/aerox3_wireless_wireless.txt` |
| `AEROX_5` Aerox 5 | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `led_brightness`, `buttons` (11), `save` | - | `rivalcfg/devices/aerox5.py` | `test/devices/old_specs/test_aerox5.py` |
| `AEROX_5_WIRELESS_WIRED` Aerox 5 Wireless (wired) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (11), `save` | battery | `rivalcfg/devices/aerox5_wireless_wired.py` | `test/devices/old_specs/test_aerox5_wireless_wired.py` |
| `AEROX_5_WIRELESS_WIRELESS` Aerox 5 Wireless (2.4 GHz) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (11), `save` | battery | `rivalcfg/devices/aerox5_wireless_wireless.py` | `test/devices/old_specs/test_aerox5_wireless_wireless.py` |
| `AEROX_9_WIRELESS_WIRED` Aerox 9 Wireless (wired) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `save` | battery | `rivalcfg/devices/aerox9_wireless_wired.py` | `test/devices/old_specs/test_aerox9_wireless_wired.py` |
| `AEROX_9_WIRELESS_WIRELESS` Aerox 9 Wireless (2.4 GHz) | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `sleep_timer`, `dim_timer`, `save` | battery | `rivalcfg/devices/aerox9_wireless_wireless.py` | `test/devices/old_specs/test_aerox9_wireless_wireless.py` |
| `KANA_V2` Kana v2 | `sensitivity1`, `sensitivity2` (choice), `polling_rate`, `led_brightness1`, `led_brightness2`, `save` | - | `rivalcfg/devices/kanav2.py` | `test/devices/old_specs/test_kanav2.py` |
| `KINZU_V2` Kinzu v2 | `sensitivity1`, `sensitivity2` (choice), `polling_rate`, `save` | - | `rivalcfg/devices/kinzuv2.py` | `test/devices/old_specs/test_kinzuv2.py` |
| `PRIME` Prime | `dpi` (5 stages), `polling_rate`, `color`, `led_brightness`, `buttons` (6), `save` | - | `rivalcfg/devices/prime.py` | `test/devices/old_specs/test_prime.py` |
| `PRIME_MINI` Prime Mini | `dpi` (5 stages), `polling_rate`, `color`, `default_lighting`, `buttons` (8), `save` | - | `rivalcfg/devices/prime_mini.py` | `test/devices/old_specs/test_prime_mini.py` |
| `PRIME_PLUS` Prime+ | `dpi` (5 stages), `polling_rate`, `color`, `led_brightness`, `buttons` (6), `save` | - | `rivalcfg/devices/prime_plus.py` | `test/devices/old_specs/test_prime_plus.py` |
| `PRIME_WIRELESS_WIRED` Prime Wireless (wired) | `dpi` (5 stages), `polling_rate`, `color`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (8), `save` | battery | `rivalcfg/devices/prime_wireless_wired.py` | `test/devices/old_specs/test_prime_wireless_wired.py` |
| `PRIME_WIRELESS_WIRELESS` Prime Wireless (2.4 GHz) | `dpi` (5 stages), `polling_rate`, `color`, `default_lighting`, `sleep_timer`, `dim_timer`, `buttons` (8), `save` | battery | `rivalcfg/devices/prime_wireless_wireless.py` | `test/devices/old_specs/test_prime_wireless_wireless.py` |
| `RIVAL_3` Rival 3 | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `logo_color`, `light_effect`, `buttons` (8), `save` | firmware | `rivalcfg/devices/rival3.py` | `test/devices/old_specs/test_rival3.py` |
| `RIVAL_3_GEN_2` Rival 3 Gen 2 | `dpi` (5 stages), `polling_rate`, `colors` + `z1_color`, `z2_color`, `z3_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `led_brightness`, `buttons` (8), `save` | - | `rivalcfg/devices/rival3_gen2.py` | `test/devices/old_specs/test_rival3_gen2.py` |
| `RIVAL_3_WIRELESS` Rival 3 Wireless | `dpi` (5 stages), `polling_rate`, `buttons` (6), `save` | battery, firmware | `rivalcfg/devices/rival3_wireless.py` | `test/devices/old_specs/test_rival3_wireless.py` |
| `RIVAL_3_WIRELESS_GEN_2` Rival 3 Wireless Gen 2 | `dpi` (5 stages), `polling_rate`, `buttons` (6), `save` | battery | `rivalcfg/devices/rival3_wireless_gen2.py` | `test/devices/old_specs/test_rival3_wireless_gen2.py` |
| `RIVAL_5` Rival 5 | `dpi` (5 stages), `polling_rate`, `colors` + `wheel_color`, `z2_color`, `z3_color`, `z4_color`, `z5_color`, `z6_color`, `z7_color`, `z8_color`, `z9_color`, `logo_color`, `reactive_color`, `rainbow_effect`, `default_lighting`, `led_brightness`, `buttons` (11), `save` | - | `rivalcfg/devices/rival5.py` | `test/devices/old_specs/test_rival5.py` |
| `RIVAL_95` Rival 95 / Rival 100 PC Bang | `sensitivity1`, `sensitivity2` (choice), `polling_rate`, `btn6_mode`, `save` | - | `rivalcfg/devices/rival95.py` | `test/devices/old_specs/test_rival95.py` |
| `RIVAL_100` Rival 100 / Rival 105 | `sensitivity1`, `sensitivity2` (choice), `polling_rate`, `color`, `light_effect`, `btn6_mode`, `save` | firmware | `rivalcfg/devices/rival100.py` | `test/devices/specs/rival100.txt` |
| `RIVAL_110` Rival 110 / Rival 106 | `dpi` (2 presets), `polling_rate`, `color`, `light_effect`, `btn6_mode`, `save` | - | `rivalcfg/devices/rival110.py` | `test/devices/old_specs/test_rival110.py` |
| `RIVAL_300` Rival 300 / Rival | `dpi` (2 presets), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_light_effect`, `wheel_light_effect`, `buttons` (6), `save` | firmware | `rivalcfg/devices/rival300.py` | `test/devices/old_specs/test_rival300.py` |
| `RIVAL_300S` Rival 300S | `dpi` (2 presets), `polling_rate`, `color`, `light_effect`, `btn6_mode`, `save` | - | `rivalcfg/devices/rival300s.py` | `test/devices/old_specs/test_rival300s.py` |
| `RIVAL_310` Rival 310 | `dpi` (2 presets), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_gradient`, `wheel_gradient`, `gradient_duration`, `buttons` (6), `save` | firmware | `rivalcfg/devices/rival310.py` | `test/devices/old_specs/test_rival310.py` |
| `RIVAL_500` Rival 500 | `dpi` (2 presets), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_gradient`, `wheel_gradient`, `gradient_duration`, `buttons` (15), `save` | firmware | `rivalcfg/devices/rival500.py` | `test/devices/old_specs/test_rival500.py` |
| `RIVAL_600` Rival 600 | `dpi` (2 presets), `polling_rate`, `colors` + `wheel_color`, `logo_color`, `z2_color`, `z3_color`, `z4_color`, `z5_color`, `z6_color`, `z7_color`, `wheel_gradient`, `logo_gradient`, `z2_gradient`, `z3_gradient`, `z4_gradient`, `z5_gradient`, `z6_gradient`, `z7_gradient`, `gradient_duration`, `buttons` (7), `save` | - | `rivalcfg/devices/rival600.py` | `test/devices/old_specs/test_rival600.py` |
| `RIVAL_650` Rival 650 Wireless | `dpi` (2 presets), `polling_rate`, `lift_off_distance`, `sleep_timer`, `buttons` (7), `save` | battery | `rivalcfg/devices/rival650.py` | `test/devices/specs/rival650.txt` |
| `RIVAL_700` Rival 700 / Rival 710 | `dpi` (2 presets), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_gradient`, `wheel_gradient`, `gradient_duration`, `save` | firmware | `rivalcfg/devices/rival700.py` | `test/devices/old_specs/test_rival700.py` |
| `SENSEI_310` Sensei 310 | `dpi` (2 presets), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_gradient`, `wheel_gradient`, `gradient_duration`, `buttons` (8), `save` | firmware | `rivalcfg/devices/sensei310.py` | `test/devices/old_specs/test_sensei310.py` |
| `SENSEI_RAW` Sensei [RAW] | `dpi` (2 presets), `polling_rate`, `light_effect`, `led_brightness`, `buttons` (8), `save` | - | `rivalcfg/devices/sensei_raw.py` | `test/devices/old_specs/test_sensei_raw.py` |
| `SENSEI_TEN` Sensei TEN | `dpi` (5 stages), `polling_rate`, `colors` + `logo_color`, `wheel_color`, `logo_gradient`, `wheel_gradient`, `gradient_duration`, `buttons` (8), `save` | firmware | `rivalcfg/devices/sensei_ten.py` | `test/devices/old_specs/test_sensei_ten.py` |

## Models

| Model | PID | Interface | Profile |
|---|---|---|---|
| Aerox 3 | `0x1836` | 3 | `AEROX_3` |
| Aerox 3 Wireless (wired) | `0x183A` | 3 | `AEROX_3_WIRELESS_WIRED` |
| Aerox 3 Wireless CS2 Dragon Lore Edition (wired) | `0x187A` | 3 | `AEROX_3_WIRELESS_WIRED` |
| Aerox 3 Wireless (2.4 GHz) | `0x1838` | 3 | `AEROX_3_WIRELESS_WIRELESS` |
| Aerox 3 Wireless CS2 Dragon Lore Edition (2.4 GHz) | `0x1878` | 3 | `AEROX_3_WIRELESS_WIRELESS` |
| Aerox 5 | `0x1850` | 3 | `AEROX_5` |
| Aerox 5 Wireless (wired) | `0x1854` | 3 | `AEROX_5_WIRELESS_WIRED` |
| Aerox 5 Wireless Destiny 2 Edition (wired) | `0x185E` | 3 | `AEROX_5_WIRELESS_WIRED` |
| Aerox 5 Wireless Diablo IV Edition (wired) | `0x1862` | 3 | `AEROX_5_WIRELESS_WIRED` |
| Aerox 5 Wireless (2.4 GHz) | `0x1852` | 3 | `AEROX_5_WIRELESS_WIRELESS` |
| Aerox 5 Wireless Destiny 2 Edition (2.4 GHz) | `0x185C` | 3 | `AEROX_5_WIRELESS_WIRELESS` |
| Aerox 5 Wireless Diablo IV Edition (2.4 GHz) | `0x1860` | 3 | `AEROX_5_WIRELESS_WIRELESS` |
| Aerox 9 Wireless (wired) | `0x185A` | 3 | `AEROX_9_WIRELESS_WIRED` |
| Aerox 9 Wireless WOW Edition (wired) | `0x1876` | 3 | `AEROX_9_WIRELESS_WIRED` |
| Aerox 9 Wireless (2.4 GHz) | `0x1858` | 3 | `AEROX_9_WIRELESS_WIRELESS` |
| Aerox 9 Wireless WOW Edition (2.4 GHz) | `0x1874` | 3 | `AEROX_9_WIRELESS_WIRELESS` |
| Kana v2 | `0x137A` | 0 | `KANA_V2` |
| Kinzu v2 | `0x1366` | 0 | `KINZU_V2` |
| Kinzu v2 | `0x1378` | 0 | `KINZU_V2` |
| Prime | `0x182E` | 0 | `PRIME` |
| Prime Rainbow 6 Siege Black Ice Edition | `0x182A` | 0 | `PRIME` |
| Prime CS:GO Neo Noir Edition | `0x1856` | 0 | `PRIME` |
| Prime Mini | `0x184D` | 3 | `PRIME_MINI` |
| Prime+ | `0x182C` | 0 | `PRIME_PLUS` |
| Prime Wireless (wired) | `0x1842` | 3 | `PRIME_WIRELESS_WIRED` |
| Prime Mini Wireless (wired) | `0x184A` | 3 | `PRIME_WIRELESS_WIRED` |
| Prime Wireless (2.4 GHz) | `0x1840` | 3 | `PRIME_WIRELESS_WIRELESS` |
| Prime Mini Wireless (2.4 GHz) | `0x1848` | 3 | `PRIME_WIRELESS_WIRELESS` |
| Rival 3 | `0x1824` | 3 | `RIVAL_3` |
| Rival 3 (firmware v0.37.0.0) | `0x184C` | 3 | `RIVAL_3` |
| Rival 3 Gen 2 | `0x1870` | 3 | `RIVAL_3_GEN_2` |
| Rival 3 Wireless (2.4 GHz) | `0x1830` | 3 | `RIVAL_3_WIRELESS` |
| Rival 3 Wireless Gen 2 (2.4 GHz) | `0x1872` | 3 | `RIVAL_3_WIRELESS_GEN_2` |
| Rival 5 | `0x183C` | 0 | `RIVAL_5` |
| Rival 5 Destiny Edition | `0x183E` | 0 | `RIVAL_5` |
| Rival 95 | `0x1706` | 0 | `RIVAL_95` |
| Rival 95 MSI Edition | `0x1707` | 0 | `RIVAL_95` |
| Rival 95 PC Bang | `0x1704` | 0 | `RIVAL_95` |
| Rival 100 PC Bang | `0x1708` | 0 | `RIVAL_95` |
| Rival 100 | `0x1702` | 0 | `RIVAL_100` |
| Rival 100 (Dell China) | `0x170A` | 0 | `RIVAL_100` |
| Rival 100 Dota 2 Edition (retail) | `0x170B` | 0 | `RIVAL_100` |
| Rival 100 Dota 2 Edition (Lenovo) | `0x170C` | 0 | `RIVAL_100` |
| Rival 105 | `0x1814` | 0 | `RIVAL_100` |
| Rival 110 | `0x1729` | 0 | `RIVAL_110` |
| Rival 106 | `0x1816` | 0 | `RIVAL_110` |
| Rival | `0x1384` | 0 | `RIVAL_300` |
| Rival Dota 2 Edition | `0x1392` | 0 | `RIVAL_300` |
| Rival 300 | `0x1710` | 0 | `RIVAL_300` |
| Rival 300 Fallout 4 Edition | `0x1712` | 0 | `RIVAL_300` |
| Rival 300 Evil Geniuses Edition | `0x171C` | 0 | `RIVAL_300` |
| Rival 300 CS:GO Fade Edition | `0x1394` | 0 | `RIVAL_300` |
| Rival 300 CS:GO Hyper Beast Edition | `0x171A` | 0 | `RIVAL_300` |
| Rival 300 CS:GO Fade Edition (stm32) | `0x1716` | 0 | `RIVAL_300` |
| Rival 300 Acer Predator Edition | `0x1714` | 0 | `RIVAL_300` |
| Rival 300 HP OMEN Edition | `0x1718` | 0 | `RIVAL_300` |
| Rival 300S | `0x1810` | 0 | `RIVAL_300S` |
| Rival 310 | `0x1720` | 0 | `RIVAL_310` |
| Rival 310 CS:GO Howl Edition | `0x171E` | 0 | `RIVAL_310` |
| Rival 310 PUBG Edition | `0x1736` | 0 | `RIVAL_310` |
| Rival 500 | `0x170E` | 0 | `RIVAL_500` |
| Rival 600 | `0x1724` | 0 | `RIVAL_600` |
| Rival 600 Dota 2 Edition | `0x172E` | 0 | `RIVAL_600` |
| Rival 650 Wireless (wired) | `0x172B` | 0 | `RIVAL_650` |
| Rival 650 Wireless (2.4 GHz) | `0x1726` | 0 | `RIVAL_650` |
| Rival 700 | `0x1700` | 0 | `RIVAL_700` |
| Rival 710 | `0x1730` | 0 | `RIVAL_700` |
| Sensei 310 | `0x1722` | 0 | `SENSEI_310` |
| Sensei [RAW] | `0x1369` | 0 | `SENSEI_RAW` |
| Sensei [RAW] Diablo III Edition | `0x1362` | 0 | `SENSEI_RAW` |
| Sensei [RAW] Guild Wars 2 Edition | `0x136D` | 0 | `SENSEI_RAW` |
| Sensei [RAW] CoD Black Ops II Edition | `0x136F` | 0 | `SENSEI_RAW` |
| Sensei [RAW] World of Tanks Edition | `0x1380` | 0 | `SENSEI_RAW` |
| Sensei [RAW] Heroes of the Storm Edition | `0x1390` | 0 | `SENSEI_RAW` |
| Sensei TEN | `0x1832` | 0 | `SENSEI_TEN` |
| Sensei TEN CS:GO Neon Rider Edition | `0x1834` | 0 | `SENSEI_TEN` |
