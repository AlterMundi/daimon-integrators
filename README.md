# Daimon Integrators

Local perceptor/actuator hubs and visible device controls, beginning with a small
Rust camera/microphone status panel for Wayland desktops. This follows the
[Matrix foundation's `/integrators`](https://github.com/AlterMundi/daimon-matrix/blob/main/docs/foundation/daimon-matrix.md)
definition: reachable hubs that integrate sensory information and coordinate action.
A hub is not a new being or an identity/capability grant.

Matrix owns identity and semantic authority. Cluster owns resource lifecycle,
operation grants and observed effects. This component observes local devices and
provides bounded local controls. Shared Resources owns the inventory and source
pointers; Telecodex owns native session activity and Stop.

## First slice

- Native Rust + system GTK3/layer-shell overlay above fullscreen/kiosk windows; no web runtime.
- Star Trek / Matrix visual tokens, expanded detail and a compact edge rail with
  device icons, state markers and scoped Stop. View preference is retained locally.
- Local CPU percentage, SoC temperature and observed auto/quiet/silent power mode.
- Microphone/camera usage indicators with explicit stale/incomplete/unknown states.
- Managed finite recording, visible preparation before acquisition, elapsed time and
  a **Stop my capture** control; arbitrary external applications are never killed.
- Optional finite WiggleControl input sessions, with visible preparation and scoped
  Stop. Input activity is shown separately from recording and observed camera use.
- Optional aggregate Telecodex activity, via the existing authorized dashboard
  provider, expiring when disconnected. Local device observation does not depend
  on the VPS. No model, Matrix inbox consumer or continuous recording is started.

See [observation limits](docs/observation.md), [installation and rollback](docs/install.md),
[qualification](docs/qualification.md), [power profiles](docs/power-profiles.md), and [current work](docs/CONTEXT.md).

## Build

Debian/Raspberry Pi OS: Rust 1.85+, `libgtk-3-dev`, `libgtk-layer-shell-dev`.
Runtime: labwc or another qualified Wayland layer-shell compositor, GTK3,
libgtk-layer-shell, PipeWire `pw-dump`, `timeout`, and ffmpeg for explicit captures.

```sh
cargo build --release --locked
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
python3 -m unittest discover -s tests
```

```text
daimon-integrators panel
daimon-integrators status
daimon-integrators input 30
daimon-integrators input 30 touch
daimon-integrators power silent
daimon-integrators power quiet
daimon-integrators power auto
daimon-integrators capture both 8 /absolute/local/clip.mp4
daimon-integrators capture audio 8 /absolute/local/audio.m4a
daimon-integrators capture video 8 /absolute/local/video.mp4
daimon-integrators stop
```

The control socket is local, owner-only. An acknowledgment is not capture/stop
completion; read `status` and observed device states. Audio/video are retained
only at the explicit local output path, never published by the panel.

`input SECONDS` requires a separately installed/configured WiggleControl and
accepts 1–120 seconds. It opens the camera only after the panel acknowledges
preparation. The same Stop control ends only the managed process group; observed
camera use remains independent. It records no media. Local session summaries use
owner-only runtime logs, cleared with the login runtime directory. No input
session is started at boot. See WiggleControl for gesture/device qualification.

At session start, only the newest 20 owned numbered input logs are retained,
including the current log even if the clock regressed. Recordings, capture logs,
archives, directories, symlinks and other owners' files are untouched. A retention
failure is reported before starting the session, rather than growing without a
limit. Copy selected diagnostic evidence elsewhere before it ages out.

`input SECONDS [mouse|touch]` selects the installed runtime's relative mouse or
direct touchscreen session. Mouse is the backward-compatible default. The
expanded managed status identifies the selected mode; touch creates no parallel
mouse stream. Touch-aware clients own gesture interpretation. Native hand accuracy
and crossing/occlusion acceptance remain WiggleControl qualification work.

Initial ownership/design: [Cluster #212](https://github.com/nicoechaniz/daimon-cluster/issues/212)
and [panel #213](https://github.com/nicoechaniz/daimon-cluster/issues/213).
