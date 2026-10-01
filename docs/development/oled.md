# OLED screen `[EXPERIMENTAL]`

Drawing on the monochrome OLED screen of Apex keyboards. Nothing here has been run on hardware
by this project. The byte layouts come from published drivers; models no driver covers use the
layout of their nearest sibling and are marked `Guess`.

Code: `src/oled/` (frames, font, images, screens) and `src/devices/keyboards/oled.rs`
(per-model table, report encoders, sending).

## Models

Every screen is 128x40 pixels, one bit per pixel. Verification uses the project's
`devices::settings::Verification` levels: `Reference` = a published driver whose users report it
working sends exactly these bytes; `Guess` = no reference covers the model.

| Model | PID | Layout | Interface | Verification | Source |
|---|---|---|---|---|---|
| Apex Pro | `0x1610` | RowMajor | 1 | Reference | apex-tux README, `usb.rs` |
| Apex 7 | `0x1612` | RowMajor | 1 | Reference | apex-tux README |
| Apex Pro TKL | `0x1614` | RowMajor | 1 | Reference | apex-tux `usb.rs` (only entry above its "Never tested" marker) |
| Apex 7 TKL | `0x1618` | RowMajorApex7 | 1 | Reference | apex7tkl_linux `device.py`, `oled.py` |
| Apex 5 | `0x161C` | RowMajor | 1 | Reference | apex-tux README |
| Apex Pro TKL (2023) | `0x1628` | PageMajor | 1 | Guess | see below |
| Apex Pro TKL Wireless (2023), dongle | `0x1630` | Chunked `0x4C` | 3 | Guess | copied from `0x1644` |
| Apex Pro TKL Wireless (2023), cable | `0x1632` | Chunked `0x0C` | 3 | Guess | copied from `0x1646` |
| Apex Pro (Gen 3) | `0x1640` | PageMajor | 1 | Reference | apex-tux PR #78 |
| Apex Pro TKL (Gen 3) | `0x1642` | PageMajor | 1 | Guess | copied from `0x1640` |
| Apex Pro TKL Wireless (Gen 3), dongle | `0x1644` | Chunked `0x4C` | 3 | Reference | apex-tux PR #74 (GG Wireshark capture) |
| Apex Pro TKL Wireless (Gen 3), cable | `0x1646` | Chunked `0x0C` | 3 | Reference | apex-tux PR #74 |
| Apex 5 (2024) | `0x1650` | PageMajor | 1 | Guess | copied from `0x1640` |
| Apex 7 (2024) | `0x1652` | PageMajor | 1 | Guess | copied from `0x1640` |

No screen: Apex 3 (`0x161A`), Apex 3 TKL (`0x1622`), Apex 150 (`0x1616`), Apex 9 Mini / TKL
(`0x1620`, `0x1634`), Apex Pro Mini in every variant (`0x161E`, `0x1624`, `0x1626`, `0x1648`).

How the guesses were made:

- The references split by board type, not by year. Wired-only boards take one report on
  interface 1 (`0x1640`); boards with a wireless radio take eight chunks on interface 3, with
  `0x4C` through the dongle and `0x0C` on cable (`0x1644`, `0x1646`). The 2023 wireless boards
  already share their RGB direct packet (`0x61`, interface 3) with the Gen 3 wireless boards.
- `0x1628`: apex-tux issue #52 reports that the RowMajor layout did nothing on this PID, so the
  Gen 3 wired layout is assumed instead. GG's database stores a 1024-byte `oled_display` buffer
  for this board (`database-schemas.md`), which would fit 128x64, and a separate
  `apex_2022_oled_display_sequence` key. The panel size and upload format are therefore not
  settled for this model; a USB capture of GG drawing on it would settle both.
- `0x1650` / `0x1652`: assumed to keep the Apex 5 / Apex 7 screen.

## Frame format

`OledFrame` packs pixels row-major, most significant bit first: 16 bytes per row, byte `x / 8`,
bit 7 = leftmost pixel, 640 bytes per 128x40 frame. This is the RowMajor wire order.

PageMajor (SSD1306 order): byte `page * 128 + x` holds column `x` of rows `page * 8 ..
page * 8 + 7`, bit 0 = top row; `page = y / 8`, 5 pages.

## Report layouts

All are HID feature reports. Byte 0 is the HID report ID, as `HIDIOCSFEATURE` expects; they are
sent with `send_feature_report_raw` to the model's interface.

| Layout | Reports | Bytes |
|---|---|---|
| RowMajor | 1 x 642 | `61` + 640 row-major bytes + `00` |
| RowMajorApex7 | 1 x 642 | `00` (report ID) + `65` + 640 row-major bytes (device sees 641 bytes, wValue `0x0300`) |
| PageMajor | 1 x 642 | `61` + 640 page-major bytes + `00` |
| Chunked | 8 x 641 | `cmd 01 off_lo off_hi 50 00` + 80 page-major bytes + zero padding; `off = i * 80`, little-endian (`0x0000` .. `0x0230`) |

apex-tux drives the Apex 7 with RowMajor (`0x61`); apex7tkl_linux drives the Apex 7 and 7 TKL
with `0x65`. Only the 7 TKL uses `0x65` here, since apex-tux marks it never tested.

## Giving the screen back

No reference documents a command that returns the screen to the keyboard's own idle image.
apex-tux sends nothing on shutdown. A frame stays until it is replaced. The one firmware-UI
command known in this repo is OSD navigation `0x24` (`protocol-keyboard.md`, hardware-tested on
`0x1628`), which opens the on-board actuation menu; it is not wired into the OLED code.

## Verification

Unit tests (`cargo test --lib oled`):

- Encoders are compared byte for byte against ports of the apex-tux algorithms on random
  frames: the `FrameBuffer` bit indexing (`device.rs`, bit `x + y * 128 + 8`, `Msb0`), the
  `draw_gen3` 8x8 transpose and `CHUNK_OFFSETS` (`usb.rs`).
- Hand-checked pixels: `(0,0)`, `(9,10)` and `(127,39)` land on the documented byte and bit in
  each layout.
- Size, layout and verification lookups for every keyboard PID in `product_ids`.
- Glyph bits, packing order, Floyd–Steinberg density on a gradient, image fit and GIF delays.

To verify a model on hardware: draw `oled::render_lines(&["TEST"])` and read it on the screen.
If it shows correctly, set that model's `Verification` to `Hardware` and update the table above,
naming the device and date in the commit.
