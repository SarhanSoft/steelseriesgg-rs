# GameSense API

ssgg runs a GameSense-compatible HTTP server (`src/gamesense/`). Games post JSON to it exactly as
they would to SteelSeries Engine / GG. The server keeps the registrations, evaluates the handlers
and publishes the result as `GameSenseOutput` values on a broadcast channel. It never touches
hardware itself; device code subscribes and shows the result.

The first part of this page describes what ssgg implements. The second part keeps the reference
captures taken from GG 111.0.0.

---

## Server discovery: `coreProps.json`

Games read `%PROGRAMDATA%\SteelSeries\SteelSeries Engine 3\coreProps.json` and post to its
`address`. On start, `GameSenseServer::run()` / `serve()` writes

```json
{"address":"127.0.0.1:<port>","encrypted_address":"","ggEncryptedAddress":""}
```

to every location below and removes those files when the server stops (graceful shutdown or the
server future being dropped):

| Location | Condition |
|----------|-----------|
| `/tmp/steelseries-engine/coreProps.json` | Linux; folder must be owned by the user |
| `~/.steam/steam`, `~/.steam/root`, `~/.local/share/Steam`, Flatpak Steam: `steamapps/compatdata/*/pfx/drive_c/ProgramData/SteelSeries/SteelSeries Engine 3/` | prefix exists |
| Every Steam library listed in `steamapps/libraryfolders.vdf` or `config/libraryfolders.vdf`: same `compatdata` path | prefix exists |
| `~/.wine/drive_c/ProgramData/...` | prefix exists |
| Lutris `~/Games/*/` (prefix at the folder or its `pfx/`) | prefix exists |
| Heroic `~/Games/Heroic/Prefixes/*` and `.../Prefixes/default/*` | prefix exists |
| Bottles `~/.local/share/bottles/bottles/*`, Flatpak `~/.var/app/com.usebottles.bottles/data/bottles/bottles/*` | prefix exists |
| `%PROGRAMDATA%\SteelSeries\SteelSeries Engine 3\coreProps.json` | Windows build |

A prefix "exists" when its `drive_c/ProgramData` folder exists. Only the two SteelSeries folders
inside it are created; on removal they are deleted again if left empty. Files are written with
mode 0600, folders 0700, without following symlinks. A failing location is logged and skipped.

Public functions: `write_core_props(port) -> Result<Vec<PathBuf>>`, `remove_core_props(&paths)`,
`discover_prefix_core_props(home)`, `write_core_props_to(port, &targets)`, `core_props_targets(home)`.
`GameSenseServer::with_core_props(false)` disables writing (used by tests).

The default port is 27301 (`[gamesense] port` in the config). GG itself picks a dynamic port; a
game only ever reads the address from `coreProps.json`, so the fixed port is compatible.

---

## Endpoints

All endpoints take `POST` with `Content-Type: application/json` unless noted.

| Path | Request | Effect |
|------|---------|--------|
| `/game_metadata` | `game`, optional `game_display_name`, `developer`, `deinitialize_timer_length_ms` (clamped to 1000-60000) | Registers the game, stores metadata; the timer replaces the default idle timeout for this game |
| `/register_game_event` | `game`, `event`, optional `min_value` (0), `max_value` (100), `icon_id` (0), `value_optional` (false) | Registers the event; existing handlers are kept |
| `/bind_game_event` | same as register plus `handlers` (non-empty array of objects or stringified objects) | Registers the event and replaces its handlers. Handlers with an unknown `device-type`, or malformed ones, are skipped and listed in `warnings`; the request still succeeds |
| `/game_event` | `game`, `event`, `data` (object or stringified object) with optional `value` and `frame` | Activates the game, evaluates the handlers, publishes outputs. Unregistered games and events are registered on the fly |
| `/multiple_game_events` | `game`, `events: [{event, data}, ...]` (at most 256) | Same as `game_event` for each entry, in order. All entries are validated before any is processed |
| `/supports_multiple_game_events` | GET or POST | 200 `{}` |
| `/game_heartbeat` | `game` | Resets the idle timer. Unknown games: 200, no effect |
| `/stop_game` | `game` | Deactivates the game now: publishes `Clear { game }`, stops its screen sequences, forgets cached values |
| `/remove_game_event` | `game`, `event` | Removes the event and its handlers (error 9 if not registered) |
| `/remove_game` | `game` | Removes the game; publishes `Clear` if it was active (error 10 if not registered) |
| `/load_golisp_handlers` | `game`, `golisp` | Accepted with a warning; GoLisp is not executed |
| `/` | GET | Server info |
| anything else | | 404 `{"error": "Unknown GameSense endpoint"}` |

Successful responses echo the request under the endpoint name, for example
`{"game_event": {"game": ..., "event": ..., "data": ...}}`.

### Event value rule

For a normal event, handlers run only when `data.value` is present and differs from the last
processed value. For an event registered with `value_optional: true`, handlers run on every
update; the value defaults to the last one sent, or 0. Floats are rounded; booleans and numeric
strings are accepted.

### Activation and heartbeat

A game becomes active on its first `game_event`. It is deactivated when no event or heartbeat
arrives within its timeout: `deinitialize_timer_length_ms` from its metadata, otherwise the server
default of 15 s (`GameSenseServer::with_heartbeat_timeout`). Deactivation publishes
`Clear { game }`, stops its screen sequences and forgets cached values, so the next event shows
again even with an unchanged value. Server shutdown deactivates every active game the same way.

### Errors

Parameter errors return HTTP 400 with the SDK's code and message:
`{"error": "<message>", "code": <n>}`.

| Code | Meaning |
|------|---------|
| 0 | Game or event missing, or malformed JSON |
| 1 | Game missing (game-only endpoints) |
| 2 | Game or event contains characters other than `A-Z 0-9 - _` |
| 3 | Game contains disallowed characters (game-only endpoints) |
| 4 | `data` missing or not an object |
| 5 | Limit reached: 64 games, or 256 events per game |
| 6 | `handlers` missing or empty |
| 9 | Event not registered (remove) |
| 10 | Game not registered (remove) |

Other statuses: 403 for a non-local `Origin`, 415 when the body is not declared
`application/json`, 413 above 1 MiB, 405 for a wrong method on a known path.
Names are limited to 128 characters; an event keeps at most 64 handlers.

### Origin policy

ssgg answers CORS only for `localhost`, `127.0.0.1` and `[::1]` origins (any numeric port), and
refuses any request whose `Origin` header names another host with 403. Requiring
`application/json` means a web page cannot reach the API with a "simple" cross-site request:
the browser must send a preflight, which is refused. GG uses `Access-Control-Allow-Origin: *`;
ssgg deliberately does not.

---

## Handlers

### Device types

`keyboard`, `mouse`, `headset`, `indicator`, `rgb-zoned-device`, `rgb-N-zone` (any N from 1 to
1024, e.g. `rgb-1-zone`, `rgb-12-zone`, `rgb-103-zone`), `rgb-per-key-zones`, `screened`,
`screened-WxH`, `tactile`. Unknown types are ignored with a warning.

### Zones

Lighting zones are resolved when the handler is bound, into a `LightTarget`:

| Device type | Zone | Target |
|-------------|------|--------|
| any lighting type | `all` | `AllZones` |
| `keyboard`, `rgb-per-key-zones` | single keys: `a`-`z`, `keyboard-1`-`keyboard-0`, `f1`-`f12`, `escape`/`esc`, `return`, `backspace`, `tab`, `spacebar`, `caps`, `dash`, `equal`, `l-bracket`, `r-bracket`, `backslash`, `semicolon`, `quote`, `backquote`, `comma`, `period`, `slash`, `insert`, `home`, `pageup`, `delete`, `end`, `pagedown`, `rightarrow`, `leftarrow`, `downarrow`, `uparrow`, `keypad-*`, `l-ctrl`, `l-shift`, `l-alt`, `l-win`, `r-ctrl`, `r-shift`, `r-alt`, `r-win`, `ss-key`, `win-menu` | `Keys([KeyId])` |
| `keyboard`, `rgb-per-key-zones` | groups: `function-keys`, `number-keys`, `q-row`, `a-row`, `z-row`, `main-keyboard`, `arrows`, `keypad`, `keypad-nums` | `Keys([...])` in SDK order |
| `keyboard`, `rgb-per-key-zones` | zones containing keys without a `KeyId`: `nav-cluster`, `printscreen`, `scrolllock`, `pause`, `pound` | `HidCodes([...])` |
| `keyboard`, `rgb-per-key-zones` | `logo`, `macro-keys`, `all-macro-keys`, `m0`-`m5`, unknown names | `NamedZone(name)` |
| any lighting type | `custom-zone-keys: [hid, ...]` | `HidCodes([...])` in the given order |
| `mouse` | `wheel`, `logo`, `base` (other names pass through) | `NamedZone(name)` |
| `headset` | `earcups` | `NamedZone("earcups")` |
| `indicator`, `rgb-zoned-device` | `one`, `two`, ... | `Zone(index)`, 0-based |
| `rgb-N-zone` | `one` ... up to N (`one-hundred-three` for 103) | `Zone(index)`; zones above N reject the handler |

Key zones are defined as USB HID usage codes and converted with `hid_to_key_id` /
`key_id_to_hid` (`src/gamesense/zone_map.rs`). `ss-key` uses HID 240, as in
`KeyMappingDatabase`. `main-keyboard` is the US ANSI block without the function row.

### Color handlers

- `color`: static `{red, green, blue}`; `{"gradient": {"zero": .., "hundred": ..}}`; or ranges
  `[{low, high, color}]` whose `color` may itself be static, gradient or ranges. The legacy
  object form `{"color": [ranges]}` is accepted too.
- A top-level static color is black when the value is 0.
- A gradient is evaluated at the value's percentage of `min_value..max_value` (with the default
  0-100 this is the value itself), rounded per channel.
- Ranges compare the raw value, bounds inclusive. A value outside every range gives black.
- Modes:
  - `color`: the whole zone gets the color.
  - `percent`: on key-level zones, the first `percent` of the keys are lit, the next key is dimmed
    by the fraction, the rest are black (12 keys at 55% = 6 lit, 1 at 60%, 5 off). On other zones
    it behaves like `color`.
  - `count`: on key-level zones, `value` keys are lit and the rest black. Otherwise like `color`.
  - `context-color`: color read from `data.frame[context-frame-key]`; no output when absent.
  - `bitmap` / `partial-bitmap`: `data.frame.bitmap` (132 `[r,g,b]`, 22x6 grid from the top-left)
    becomes one `Bitmap` output. For `partial-bitmap`, `excluded-events` (from the handler, or
    overridden by `frame.excluded-events`) become the keyboard targets of those events.
- `rate`: `frequency` static or ranges (undefined range = no flashing), `repeat_limit` static or
  ranges. Becomes `flash: Some(Flash { frequency_hz, repeat_limit })` on the lit segments.

### Screen handlers

`datas` is a list of frames or of `{low, high, datas}` ranges. Frames are resolved to text, not
pixels:

- single-line form (`has-text`, `prefix`, `suffix`, `bold`, `wrap`, `has-progress-bar` on the
  frame) and multi-line form (`lines: [...]`);
- line value from the event value, `context-frame-key`, or `arg`. Supported `arg` forms: `""`
  (no value), `(value: self)`, accessor chains such as `(hp: (player: (context-frame: self)))`,
  and dotted paths `frame.player.hp`. Other GoLisp expressions are not evaluated: the event value
  is shown and the bind response carries a warning;
- `icon-id` (0 = none), `length-millis` (0 = until the next screen event), `repeats` on the last
  frame (`true`/0 = forever, `false` = once, n = n passes), `priority` (extension, default 0);
- image frames (`has-text: false`): `image-data` or `image-data-WxH` on the frame, replaced by
  `image-data-WxH` in the event frame. Bytes are raw 1 bit per pixel, MSB first. A sized handler
  keeps only its size; a generic `screened` handler gets one output per size present. Data longer
  than `ceil(W*H/8)` is truncated, shorter data drops the frame.

Multi-frame or repeating sequences are played by a server task, one `Screen` output per frame;
a newer screen event for the same game, device type and zone replaces the running sequence.

### Tactile handlers

`pattern` is a list of `{type, delay-ms}` / `{type: "custom", length-ms, delay-ms}` entries (at
most 140), or ranges of such lists. `length-ms` is capped at 2560; a `delay-ms` above 2560 is
ignored, as the SDK states. `rate` works as for color handlers.

---

## Output model

```rust
pub enum GameSenseOutput {
    Lighting { game, event, device_type: DeviceType, target: LightTarget, color: Color,
               flash: Option<Flash>, duration: Option<Duration> /* None: until replaced */ },
    Bitmap   { game, event, device_type, colors: Vec<Color> /* 132 */, excluded: Vec<LightTarget> },
    Screen   { game, event, device_type, zone: String, size: Option<ScreenSize>,
               content: ScreenContent, duration: Option<Duration>, priority: i32 },
    Tactile  { game, event, device_type, zone: String, pattern: Vec<TactileStep>, rate: Option<Rate> },
    Clear    { game },
}
pub enum LightTarget { AllZones, Zone(usize), NamedZone(String), Keys(Vec<KeyId>), HidCodes(Vec<u8>) }
pub enum ScreenContent { Lines { lines: Vec<ScreenLine>, icon_id: Option<u32> }, Bitmap(Vec<u8>) }
pub enum ScreenLine { Text { text, bold, wrap }, ProgressBar { percent } }
```

`GameSenseServer::subscribe()` returns a `tokio::sync::broadcast::Receiver<GameSenseOutput>`
(capacity 1024; a slow receiver loses the oldest outputs and sees `Lagged`).

`set_rgb_callback(Fn(&str, u8, u8, u8))` is a legacy adapter over the same channel, kept for
`src/main.rs`. It forwards only `keyboard` / `rgb-per-key-zones` lighting: `"all"` for the whole
device and for key-level targets, `"<n>"` (1-based) for numbered zones, otherwise the zone name.
Black segments of key-level targets are not forwarded; flashing, screens, tactile and `Clear`
are dropped.

---

## Differences from GG

- GoLisp handlers (`load_golisp_handlers`, GoLisp `arg` expressions) are not executed.
- Response bodies echo the request; they do not carry GG's internal fields (`is`, `enabled`,
  `options`, `data_fields`, `level`).
- The `remove_game` response key is `remove_game`; GG answers `remove_game_event`.
- CORS is localhost-only (GG: wildcard).
- The port is fixed (default 27301); GG picks one at runtime.

---

## GG 111.0.0 reference captures (2026-05-26, port 53714)

### Port discovery

GG writes the port at runtime to:

```
C:\ProgramData\SteelSeries\SteelSeries Engine 3\coreProps.json
C:\ProgramData\SteelSeries\GG\coreProps.json  (symlink/copy)
```

```json
{"encryptedAddress":"127.0.0.1:53723","ggEncryptedAddress":"127.0.0.1:6327","address":"127.0.0.1:53714"}
```

- `address`: standard GameSense HTTP endpoint (games use this)
- `encryptedAddress`: HTTPS/TLS endpoint (unknown cert chain)
- `ggEncryptedAddress`: GG-internal encrypted endpoint

### Endpoints seen

| Method | Path | GG response key | Verified |
|--------|------|-----------------|----------|
| POST | `/game_metadata` | `game_metadata` | yes |
| POST | `/bind_game_event` | `bind_game_event` | yes |
| POST | `/register_game_event` | `register_game_event` | yes |
| POST | `/game_event` | `game_event` | yes |
| POST | `/game_heartbeat` | `game_heartbeat` | yes |
| POST | `/remove_game` | `remove_game_event` | yes |
| POST | `/remove_game_event` | not tested separately | no |

### `POST /game_metadata`

Request: `{"game":"SSGG_TEST","game_display_name":"ssgg Test","developer":"test"}`

```json
{
  "game_metadata": {
    "is": "",
    "game": "SSGG_TEST",
    "game_display_name": "ssgg Test",
    "deinitialize_timer_length_ms": 0,
    "developer": "test",
    "enabled": false,
    "options": null
  }
}
```

`is` is always `""`; `enabled` is `false` until the game fires events; `options` is a JSON blob.

### `POST /bind_game_event`

```json
{
  "game": "SSGG_TEST",
  "event": "HEALTH",
  "min_value": 0,
  "max_value": 100,
  "icon_id": 0,
  "handlers": [
    {
      "device-type": "keyboard",
      "zone": "function-keys",
      "color": {"gradient": {"zero": {"red":0,"green":255,"blue":0}, "hundred": {"red":255,"green":0,"blue":0}}},
      "mode": "percent"
    }
  ]
}
```

Response: `{"bind_game_event": {game, event, icon_id, min_value, max_value, "value_optional": false, "data_fields": null, handlers}}`.
`product_id` is not a valid request field: GG answers HTTP 400.

### `POST /register_game_event`

Request: `{"game":"SSGG_TEST","event":"MANA","min_value":0,"max_value":100}`

Response: `{"register_game_event": {game, event, "icon_id": 0, min_value, max_value, "value_optional": false, "data_fields": null}}`

### `POST /game_event`

Request: `{"game":"SSGG_TEST","event":"HEALTH","data":{"value":75}}`

Response: `{"game_event": {"level": "", game, event, "data": {"value": 75}}}`. `level` is always `""`.

### `POST /game_heartbeat`

Request `{"game":"SSGG_TEST"}`, response `{"game_heartbeat":{"game":"SSGG_TEST"}}`.

### `POST /remove_game`

Request `{"game":"SSGG_TEST"}`, response `{"remove_game_event":{"game":"SSGG_TEST"}}`.

### GG CORS headers

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: POST, OPTIONS
Access-Control-Allow-Headers: Content-Type
```
