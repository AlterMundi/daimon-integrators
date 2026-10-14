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

## Native hand feedback

`daimon-integrators hand-overlay PORT SECONDS` renders WiggleControl's bounded
loopback `/state` metadata on a transparent, full-output GTK3/layer-shell surface.
It opens no camera, input device or inference runtime. Start the separately
authorized finite `wigglecontrol run-feedback` producer; after the first metadata
view is painted, the overlay sends `/ready` to start its measured input window.
`SECONDS` bounds this UI to 1–120 seconds. Nothing is installed at startup.

The skeleton surface has an empty input region and no keyboard focus. A separate
small layer-shell STOP INPUT button is the only interactive area. It sends `/stop`
to this selected local producer; the independent status panel remains the source
for actual camera use. This direct session is not owned by the panel's managed
capture Stop. Timer expiry/connection failure clears stale hand feedback.

Bones retain camera-pixel dimensions around a palm anchor mapped from the mirrored
camera workspace to the output. **This is hand position, not the relative mouse's
global cursor position.** No global Wayland cursor coordinate is guessed. Guided
workspace calibration and cursor-coincident feedback remain pending in WiggleControl.

`hand-overlay-demo SECONDS` draws a labeled fictional hand, without opening sensors
or producing input, for transparency/fullscreen qualification. The web qualification
page is now an ordinary application event observer, with no hand renderer.

Calibrated producers publish a shared absolute `screen_target` and calibration
state. The hand overlay anchors to that emitted target during active use/review;
neutral sampling shows the camera workspace. Native CALIBRATE/NEXT/CANCEL/ACCEPT/
ROLLBACK controls target the selected finite producer. Stage/sample/corner
instructions show progress, and controls are disabled outside applicable stages
or after expiry/disconnection. The measured window is not acknowledged for a
configured output size differing from the actual layer allocation.

Visual hand retention is bounded at 300 ms; aged feedback is dimmed/amber. It does
not extend the strict 150 ms input freshness or green press indication. The
independent status panel still owns device observation; source/UI fixture checks
do not prove human calibration or camera Stop cleanup.

Recognized pinch onsets also produce a mint-green double-ring pulse at the hand's
palm anchor. It expands and fades over 700 ms, with a PINCH label, even after a
brief pinch releases. The cumulative recognized-press counter triggers once per
increase; initial/reset counters and a held press do not create repeated flashes.
This is event feedback, separate from current held-button state and input safety.

The entire skeleton becomes mint green while the current fresh producer state
reports an active held pinch, and returns to cyan on release or unknown/stale
state. Retained geometry cannot preserve a green held indication. This complements
the onset pulse without changing gesture recognition or injecting input.

Feedback uses the producer's bounded `freshness_ms` metadata (50..1000 ms),
falling back to150 ms for legacy producers. The selected300 ms gesture trial
therefore shares one window with recognition and the device writer, without
changing onset-pulse duration or retaining a held indication indefinitely.

Recorded camera-only trial feedback explicitly shows RECORDING // NO INPUT.
Experimental metric3D pinch metadata displays the estimated fingertip gap in
millimetres; recognized state still drives the pulse and held skeleton color.
Recording/playback lives in WiggleControl, not this observer or panel.
