# Observation and control contract

A root read-only observer scans process FD **target names** for the configured
camera and ALSA capture substream state. It never reads captured samples, process
arguments, memory, native sessions or credentials. A separate unprivileged Rust
panel reconciles running physical Audio/Source and Video/Source nodes from the
local PipeWire graph through one persistent pw-dump monitor (no recurring process launches). Device FD/ALSA polling is approximately one second; a short acquisition
between polls may be missed. The configured camera is selected in a root-owned
receiving-local device file; microphones cover ALSA capture devices plus running
PipeWire sources, not only the managed capture's selected microphone.

Camera open means **device in use**, not proof of streaming, storage, a model
viewing frames or attention. ALSA RUNNING/PipeWire running means signal flow,
not proof of recording. Playback sinks and idle sources do not count as capture.
Other applications' purpose remains unknown. Device absence, observer stale over
four seconds, permissions or PipeWire observation failure remain explicit. A
recent active observation takes precedence over incomplete coverage. No claim
covers network cameras, arbitrary browser remote streams, Bluetooth inputs not
represented by these backends, or unsampled transient access.

Managed recording is separate: preparing, capturing, stopping, completed, stopped
or failed. Preparing must be acknowledged by the UI before ffmpeg acquisition.
This proves the UI updated its widgets; human-perceived timing is qualified on
hardware separately. Recording intent/process lifecycle does not substitute for
device observation. Captures have a 1–120 second limit and refuse overwrites.
The user-only socket controls one managed process group. Stop requests SIGINT,
waits for actual process exit, and escalates only that group after three seconds.
A request acknowledgment is not completed stop. Systemd also cleans the group's
children if the panel crashes. Other applications are indicated, never stopped.

Telemetry has no camera/audio data, native titles, goals or history. Optional
remote activity projects only counts/status from the established Telecodex
provider and expires locally after 15 seconds. Losing the VPS cannot disable
local device observation or allow automatic recording.
