# Current project context

Human-authorized goal: implement and reversibly deploy the first Rust local CompAII
panel on the existing Raspberry Pi, following Cluster #212/#213. A dedicated public
code repository was selected; private inventory/access stays in Shared Resources.

The pilot already proved remote shell, real camera/audio capture and human-confirmed
TV video playback. The panel has partial user-level hardware qualification, detailed in docs/qualification.md. Preserve the
existing login and desktop; deployment does not authorize administrative changes.

Telecodex dashboard #70 has a real separate deployed provider. Its observed branch
source is 747a2588eaf4e1c1e3a7f23b7afee229b679b331, with a sessions-dashboard/v1
projection. The optional bridge uses the qualified receiving copy of that provider;
it exports aggregate activity only and does not duplicate native Stop.

Current obstacle: SSH key login works, but sudo requires the rotated user password.
Its original root-only recovery storage was insufficient for subsequent independent
administrator access. Recover through the human's existing local authorization; do
not bypass authentication or alter policy. Continue source work independently.

The user preview is running and human-confirmed above Chromium kiosk. Compact
edge mode and the maintained Star Trek / Matrix visual tokens are now selected
by Nicolás. Preserve icon plus explicit state markers; compact mode should become
the final default after qualification. Root observer/login deployment remains
pending administrator recovery; unknown idle states are intentional in preview.

Code is published; full privileged observation/deployment remains pending independently recovered administrator access. The visible preview intentionally shows unknown idle when process coverage is incomplete.

## Current correction and next component — 2026-10-13

The preview/recovery statements above describe earlier preparation. Independent
administrator access was recovered; the full observer and panel were installed
and actual receiving rollback/reinstallation qualified (docs/qualification.md).
The current receiving hardware is Pi 5, with SSD system and an optional content
SD; the TV is now 1080p60. Nicolás confirmed the panel works; compact resizing
for this resolution is #2. Private deployment and access remain in Shared Resources.

Nicolás selected **WiggleControl**, executable `wigglecontrol`, for a lightweight
Rust camera-to-system-input component. Its goal is mouse and later multitouch
device behavior across existing applications, not only a browser interface.
Before implementation he authorized focused state-of-the-art research.
[The research](wigglecontrol-research.md) compares existing implementations,
native perception engines, uinput/Wayland behavior and proposed qualification.
It informs #3; no device injection, new camera trial or startup inference was
performed by the research. Reuse ProyectoRPi/Barehands and selected HarMoCAP
methods, preserve attribution, and measure rather than promise efficiency.

## System metrics and selectable quiet modes — 2026-10-13

Nicolas requested actual CPU percentage, SoC temperature and the selected silent
mode in the existing overlay. The panel now samples those local resources offline,
shows compact CPU/TEMP/MODE badges and an expanded detail line, and detects mode
from live fan enable, CPU cap and thermal thresholds. Compact presentation is
scaled down for 1080p. Existing observation/capture and layer-shell behavior remain.

Power CLI selects auto (standard thermal curve/2.4 GHz), quiet (later thermal
steps/2 GHz), or silent (forced fan off/1.5 GHz). Exact fixed profile helpers have
a narrow sudo grant; the UI stays unprivileged. A selected mode is saved and restored
by a finite boot oneshot. No model/camera startup is implied. Receiving-selected
background units can have a mode-specific CPU budget; auto restores original quota.
See docs/power-profiles.md and qualification evidence, not the old manual script.

WiggleControl now has its own owning repository and issue; its native model probe
found one concrete palm Resize shape failure in the pinned tract candidate.
Continue its end-to-end goal there after this human-directed local dashboard update.

## Supervised system-input sessions

The panel's existing managed process lifecycle now optionally supervises finite
WiggleControl sessions through `input SECONDS`. Preparation must be acknowledged
by the UI before acquisition; Stop ends only its owned process group. The LOCAL
badge distinguishes input from recording. Observed camera/microphone activity
remains independent and works offline. WiggleControl retains gesture/input
ownership and must be installed separately; no session starts at login.

Receiving qualification observed preparation, input activity and camera-open with
microphone inactive; Stop reaped the session and the camera closed. This proves
session control, not hand recognition accuracy. See docs/qualification.md.
