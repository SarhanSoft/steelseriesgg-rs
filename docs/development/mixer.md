# PipeWire mixer (Sonar replacement)

`src/mixer/` recreates SteelSeries Sonar on Linux on top of PipeWire. It is always compiled
(no Cargo feature) and links no audio libraries: it drives `pipewire`, `pactl`, `pw-dump`,
`pw-cli`, `pgrep` and `kill` at runtime through one seam, `CommandRunner`.

**Status: never run against a real PipeWire system.** Everything below follows the PipeWire
documentation and the example configs PipeWire ships, and is covered by host unit tests with a
fake runner. The section "Verification status" lists what those tests cannot show.

## What it provides

| Sonar feature | Implementation |
|---|---|
| Game / Chat / Media / Aux outputs | one `libpipewire-module-filter-chain` per channel; capture side is a virtual `Audio/Sink` |
| Mixed into one headset | every filter-chain's playback side targets the chosen physical sink |
| Per-channel volume and mute | `pactl set-sink-volume` / `set-sink-mute` on the virtual sinks |
| 10-band parametric EQ + presets | builtin `bq_lowshelf` / `bq_peaking` / `bq_highshelf` (also `bq_lowpass`, `bq_highpass`, `bq_notch`) nodes in each filter graph |
| ChatMix | Game and Chat sink volumes = user volume x dial factor |
| Microphone + ClearCast-style noise reduction + EQ | optional `libpipewire-module-echo-cancel` (WebRTC) or RNNoise LADSPA node, then the mic EQ; result is the virtual `Audio/Source` "SteelSeries Microphone" |
| Streamer mode (Monitoring + Streaming mixes) | a "SteelSeries Stream" sink plus one `libpipewire-module-loopback` per channel copying it in at its own volume |
| Per-app routing, remembered | rules in `mixer.toml`; existing streams moved at start, new streams moved by a watcher (`pactl subscribe`, polling every second as fallback) |
| Spatial audio | `sofa` spatializer on the Game channel: 7.1 in, binaural stereo out |

## Architecture

```
MixerConfig (mixer.toml) ──► Mixer::start / apply
                                  │
          detect ─► resolve devices ─► GraphPlan ─► generate_pipewire_config
                                  │                        │
                                  │             $XDG_RUNTIME_DIR/ssgg/mixer-pipewire.conf
                                  │                        │
                                  └──► spawn `pipewire -c <conf>` (child process)
                                        wait until the ssgg_* nodes exist
                                        set volumes/mutes, defaults, move streams (pactl)
                                        start the routing watcher thread
```

| File | Role |
|---|---|
| `runner.rs` | `CommandRunner` trait, `SystemRunner` (timeouts, `LC_ALL=C`), `FakeRunner` for tests, `ChildHandle` |
| `config.rs` | `MixerConfig` and its parts, validation, TOML load/save (owner-only permissions) |
| `eq.rs` | `Eq`, `EqBand`, `BandType`, the seven presets |
| `routing.rs` | `RoutingRule`, `RoutingConfig`, default rules, `*` glob matching |
| `chatmix.rs` | ChatMix factors, slider-to-`ChatMix` conversion |
| `pactl.rs` | pactl argument builders and parsers for `pactl -f json` and the text format |
| `pwconf.rs` | `GraphPlan` and the SPA-JSON config generator |
| `detect.rs` | `DetectReport`: PipeWire running, tools, plugins |
| `engine.rs` | `Mixer`: lifecycle, diffing, setters, status, routing watcher |

### One child process for every node

All mixer nodes live in one client-side PipeWire instance (`pipewire -c <generated file>`, the
same way PipeWire's own `filter-chain.conf` is run). Killing that one process removes every node,
so a crash cannot leave half a graph behind. The child is restarted only when the graph's shape
changes: output or input device, band count or band type, noise-suppression backend, spatial
audio, streamer mode. Volumes, mutes and ChatMix never restart it.

EQ changes that keep the band shapes are first sent to the running filter with
`pw-cli set-param <node id> Props '{ params = [ "eq_1:Gain" 3.0 ... ] }'` (node id from
`pw-dump`). If that fails, or `Mixer::set_live_eq_updates(false)` is set, the child restarts.

### Start sequence

1. `detect()`: `pipewire --version`, `pactl info` (must report PipeWire), `pactl -f json info`,
   `wpctl status`, tool probes, plugin file probes. Problems stop the start with the report.
2. Stale cleanup: a `mixer-state.json` left behind means the last run crashed. Its saved
   default devices are kept for restoring. `pgrep -f '^([^ ]*/)?pipewire -c <conf>$'` and the
   `application.process.id` of any leftover `ssgg_*` node owned by `pipewire` give PIDs, which
   get `kill -TERM`.
3. Resolve devices. `auto` output: a SteelSeries/Arctis sink (name, description or USB vendor id
   `0x1038`), preferring "game"/"stereo" over "chat"/"mono"; else the default sink; else the
   first. Input the same way, skipping monitors. The mixer's own nodes are never chosen.
4. Write the config, record the defaults in `mixer-state.json`, spawn the child, wait up to 5 s
   for the expected sinks/sources. If the child dies or the nodes do not appear and the plan
   uses noise suppression or spatial audio, retry once without them and report a warning.
5. Apply levels, set the default sink to `ssgg_game` and the default source to `ssgg_mic`,
   move existing streams per the rules, start the watcher.

`stop()` restores the previous defaults first (so streams on the vanishing sinks fall back to
the real device), terminates the child (SIGTERM, then SIGKILL after 2 s), and deletes the state
file. Dropping a running `Mixer` calls `stop()`.

## Node names

Every name starts with `ssgg_`; that prefix is how stale nodes and the mixer's own streams are
recognised.

| Node name | Kind | Description shown to users | Present when |
|---|---|---|---|
| `ssgg_game` | `Audio/Sink` | SteelSeries Game | always |
| `ssgg_chat` | `Audio/Sink` | SteelSeries Chat | always |
| `ssgg_media` | `Audio/Sink` | SteelSeries Media | always |
| `ssgg_aux` | `Audio/Sink` | SteelSeries Aux | always |
| `ssgg_<channel>_out` | playback stream | SteelSeries Game (output), ... | always; feeds the headset |
| `ssgg_mic` | `Audio/Source` | SteelSeries Microphone | mic enabled and an input found |
| `ssgg_mic_in` | capture stream | SteelSeries Microphone (input) | same |
| `ssgg_mic_ns_in`, `ssgg_mic_ns` | echo-cancel capture / source | ... (noise suppression input / noise suppressed, internal) | WebRTC noise suppression |
| `ssgg_mic_ns_ref`, `ssgg_mic_ns_ref_out` | echo-cancel reference sink / playback | SteelSeries Mic Echo Reference | WebRTC noise suppression |
| `ssgg_stream` | `Audio/Sink` | SteelSeries Stream | streamer mode |
| `ssgg_stream_out` | `Audio/Source` | SteelSeries Stream (source) | streamer mode |
| `ssgg_stream_<channel>_in`, `ssgg_stream_<channel>` | loopback capture / playback | SteelSeries Stream: Game, ... | streamer mode; one pair per channel incl. `mic` |

OBS captures the streaming mix with "Audio Output Capture" on SteelSeries Stream (its monitor)
or "Audio Input Capture" on SteelSeries Stream (source).

Filter-graph node names inside the config: `eq_1`..`eq_10` (one mono chain, which filter-chain
runs once per audio channel), `eql_*`/`eqr_*` (spatial Game, per ear), `sp_FL`..`sp_SR`,
`mix_l`/`mix_r`, `rnnoise`, `copy` (EQ with no bands).

## Files

| Path | Content |
|---|---|
| `<config dir>/mixer.toml` | `MixerConfig`; `MixerConfig::load()` / `save()` (callers persist; setters only update `Mixer::config()`) |
| `$XDG_RUNTIME_DIR/ssgg/mixer-pipewire.conf` | the generated PipeWire config, rewritten on every (re)start |
| `$XDG_RUNTIME_DIR/ssgg/mixer-state.json` | child PID and the defaults to restore; exists only while running |

Without `XDG_RUNTIME_DIR` the two runtime files go to `<config dir>/run/`.

## Runtime packages

Required: PipeWire with its PulseAudio server, WirePlumber, and `pactl`. The `pipewire` binary
must be on `PATH`. `pw-dump`/`pw-cli` are optional (without them every EQ change restarts the
child); `pgrep` is optional (stale-process detection then relies on node properties).

| Distro | Required | Optional |
|---|---|---|
| Arch | `pipewire pipewire-pulse wireplumber libpulse` (`libpulse` provides `pactl`) | `noise-suppression-for-voice` (RNNoise), `libmysofa` (spatial) |
| Debian / Ubuntu | `pipewire pipewire-pulse wireplumber pulseaudio-utils pipewire-bin` | `libmysofa1` for spatial; RNNoise is not packaged on every release: install `librnnoise_ladspa.so` from the noise-suppression-for-voice project into a LADSPA directory |
| Fedora | `pipewire pipewire-pulseaudio wireplumber pulseaudio-utils pipewire-utils` | `noise-suppression-for-voice`, `libmysofa` |

The WebRTC echo-cancel module (`libpipewire-module-echo-cancel.so` plus
`spa-0.2/aec/libspa-aec-webrtc.so`, built against webrtc-audio-processing) ships with the
PipeWire modules on all three. The optional package names above were not checked on each
release; confirm them with the distro's package search.

Versions assumed: PipeWire 1.0 or newer (`target.object`, echo-cancel `library.name`), WirePlumber
0.5 for `node.dont-fallback` (0.4 ignores it; see below), pactl 16+ for JSON output (older pactl
falls back to text automatically). The sofa plugin moved from
`pipewire-0.3/filter-chain/libpipewire-module-filter-chain-sofa.so` (before 1.4) to
`spa-0.2/filter-graph/libspa-filter-graph-plugin-sofa.so` (1.4+); detection looks for both.

RNNoise search path: `$LADSPA_PATH`, `/usr/lib{,64}/ladspa`, `/usr/lib/{x86_64,aarch64}-linux-gnu/ladspa`,
`/usr/local/lib/ladspa`, `/run/current-system/sw/lib/ladspa`, `~/.ladspa`.

## Verification status

The unit tests (`cargo test --lib mixer`) check: the generated config parses as SPA-JSON with
balanced brackets and quoted descriptions; node names, targets and module arguments for the
EQ, mic, WebRTC, RNNoise, streamer and spatial variants; pactl argument lists; parsing of
`pactl -f json` and text output (including pactl 16's broken JSON); ChatMix math; routing
rules; presets; config round-trips; and the engine's command sequence against `FakeRunner`
(start, stop, crash recovery, degraded retry, live EQ, restart on shape change, minimal
`apply`, routing of new streams).

What those tests cannot show, because no PipeWire system was available:

- That `pipewire -c <absolute path>` runs the generated file as a client of the user's daemon
  with only `protocol-native`, `client-node` and `adapter` loaded (the layout of PipeWire's
  `filter-chain.conf`).
- That volume and mute set on a filter-chain capture node through pipewire-pulse behave like a
  normal sink volume.
- That a filter-chain `Audio/Sink` has monitor ports, and that its monitor is taken before the
  sink volume and mute (`monitor.channel-volumes` defaults to false). The streaming mix's
  independence from the monitoring sliders and ChatMix depends on this.
- That `pw-cli set-param <id> Props '{ params = [ ... ] }'` on the filter-chain capture node
  changes the biquad controls. If it silently does nothing, EQ changes that keep the band
  shapes will not be heard; `Mixer::set_live_eq_updates(false)` forces a restart instead.
- Passive-link behaviour of the mic chain. The RNNoise path copies PipeWire's
  `source-rnnoise.conf` (capture passive, so the mic runs only while recorded). The WebRTC path
  marks only the echo-cancel capture passive; the mic may stay open while the mixer runs.
- That `node.dont-fallback` / `node.dont-reconnect` keep a filter's playback unlinked when its
  target disappears. With WirePlumber 0.4 the playback stream could fall back to the default
  sink, which is `ssgg_game` while the mixer runs.
- The WebRTC echo-cancel reference sink `ssgg_mic_ns_ref` shows up in sound settings; nothing
  is routed into it, so echo cancellation itself has no reference signal (only noise
  suppression is intended).
- Routing happens after a stream appears: WirePlumber first links a new stream to the default
  sink (Game), and the watcher moves it a moment later, so the first instant of audio can play
  on Game.
- In streamer mode the taps are not passive, so the headset may not suspend while the mixer
  runs.
- Spatial audio: the azimuth layout copies PipeWire's `spatializer-7.1.conf`; how it sounds was
  not heard by anyone.

### Manual smoke test on a real system

```sh
# 1. Inspect the generated config after a start
cat "$XDG_RUNTIME_DIR/ssgg/mixer-pipewire.conf"
# 2. Run it by hand and watch PipeWire's own errors
pipewire -c "$XDG_RUNTIME_DIR/ssgg/mixer-pipewire.conf"
# 3. In another terminal: the nodes, their links and the defaults
pactl list short sinks | grep ssgg_
pactl list short sources | grep ssgg_
pw-link -l | grep ssgg_
pactl info | grep Default
```
