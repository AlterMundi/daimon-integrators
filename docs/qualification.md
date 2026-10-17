# Qualification record — 2026-10-12

Software, deployment and human acceptance remain separate.

## Source checks

Rust locked build, format, tests and clippy pass. Tests cover stale/incomplete
states, bounded captures, playback vs source flow, missing UI preparation
acknowledgment, and managed process-group Stop preserving an unrelated process.
Python tests verify that only aggregate activity leaves the Telecodex provider.

## Raspberry Pi 4 preview

Compiled with the distribution Rust 1.85.1 under the user account. Temporary
extracted build dependencies were required because sudo needs administrator
credential recovery. Two supervised user services run the panel and a partial
observer. This is not the full root observer installation or reboot acceptance.

Observed hardware checks:
- Audio-only capture: microphone detected, camera untouched; completed.
- Video-only capture: camera detected, microphone untouched; completed.
- Combined capture: both detected; completed. Sampling was checked after an
  idle baseline, excluding preceding operations' still-valid last samples.
- Real pointer click on panel Stop: managed child reaped and stopped.
- Same Stop button above Chromium kiosk: child stopped; compositor still
  reported the browser active and fullscreen after interaction.
- Separate Jabra PipeWire recording: source flow detected; managed Stop left
  that external process alive. The test then explicitly ended its own capture.
- Invalid output path: failed capture state, not success.
- Observer stopped: old samples became stale; panel remained responsive.
- Remote activity feed stopped: remote counts expired while local observation
  continued. Native/owner control was unchanged.
- Layer-shell overlay visually inspected above actual Chromium --kiosk.
  Nicolás independently confirmed the panel stays above everything.

Source limits remain real: partial observer cannot prove inactive when other
users' FDs cannot be inspected, so idle is unknown. Other applications' storage
intent and model perception are not inferred from open devices.

Still to qualify: full restricted root observer, restart/login installation and
rollback on receiving hardware, mediated camera coverage, and final human readability/status-timing acceptance.
Initial ten seconds including GTK startup measured 14.75% of one CPU core plus
1.42% for the partial observer, with panel RSS 31 MiB; this is not steady idle.

No capture remains active. Captured test media stays local and unpublished.

Compact rail qualification on 2026-10-13: visually inspected with actual icons
and state markers above kiosk. Real expand click works; preference persisted
after waiting for UI acknowledgment and survived service restart. Compact Stop
click ended a real combined capture; browser remained active/fullscreen.
The browser text input also received the injected focus-test text while the
overlay remained visible. Human reaffirmed the Star Trek direction; guide tokens
are applied with system font fallback pending a qualified Space Grotesk asset.

After replacing recurring PipeWire snapshot processes with one incremental
monitor, verified source start and return to idle. A steady 10-second sample
measured panel 0.53% of one CPU core / 41728 KiB RSS, partial observer 1.99% /
10072 KiB RSS. These are dated preview measurements, not fleet capacity claims.

## Installer interruption qualification — 2026-10-13

An isolated, network-disabled bubblewrap namespace injected a snapshot copy
failure, retried installation and rolled back. The initial installer restored
the wrong binary because it published an incomplete manifest before its copies
finished. The corrected installer publishes the complete staging directory
atomically before receiving mutation and checks retained backups on retries.

The same fixture now passes interrupted-copy/retry/rollback with original
contents, owners and modes preserved; fresh installation retried twice then
removed only its installed files. Administrative SSH sentinel contents remained
unchanged throughout. Services were mocked inside the namespace: this proves
file-effect qualification, not real systemd or receiving privileged deployment.

## Full receiving observer — 2026-10-13

Administrator authentication was recovered and verified independently. The
restricted system observer and final user panel were installed. The first full
check still reported incomplete ALSA coverage: the code treated the regular
summary file `cards` as a `cardN` directory, and the actual syscall returned
ENOTDIR. Corrected directory selection and added a regression test. Subsequent
live samples report complete=true with healthy idle and PipeWire observation.
Receiving installation/rollback and final acceptance are being qualified.

## Pi 5 receiving qualification — 2026-10-13

The same SD booted on Raspberry Pi 5 Model B Rev 1.0, verified from the
device tree. Final system observer and user panel started automatically; compact
preference survived, complete observation was healthy, and no capture started.

A bounded external GStreamer PipeWire camera stream (60 buffers to fakesink,
no stored media) with target-object set to the source object serial and explicit
video/x-raw caps completed successfully. Both camera FD observation and PipeWire
flow became active, while managed recording stayed absent. Earlier attempts
without explicit video caps returned target-not-found; their cause is not inferred.

Actual receiving rollback removed the installed binary and system unit. Fresh
SSH and independent sudo authentication remained functional. Reinstallation
restored complete observation and the acknowledged compact UI. Content digests
of SSH configuration, authorized access files and sudo configuration matched
before and after; credential contents were never emitted.

Remaining human/device acceptance: physical USB disconnect/reconnect and final
compact readability/status timing on the TV. Earlier steady idle measurements
belong to Pi 4 and must not be presented as Pi 5 measurements.

## System metrics and power profiles — 2026-10-13

Source tests verify aggregate CPU counters without duplicate guest accounting,
missing/regressed samples and mode classification from live fan/frequency/thermal
configuration. Existing capture isolation tests still pass. Full-feature Clippy
passed; the no-UI test configuration retains its existing unused-UI warnings.

Receiving Rust 1.85.1 built the updated GTK/layer-shell release with one build job
and low priority. Previous working installed binary retained before replacement.
Existing observer and desktop were not restarted; only the user panel was.
Independent status receipt shows actual CPU percentage, SoC temperature, 0 RPM,
1.5 GHz cap and silent mode, with camera and microphone inactive. A right-edge
screenshot confirms compact CPU/TEMP/MODE badges visible at 1080p and the existing
compact view preserved; no screenshot/media/private binding is committed.

The exact three-command sudo grant and saved-mode boot unit are installed; a
normal-user `power silent` invocation applied the state. Reboot restoration is
configured but not yet observed. Real quiet/auto transition and final selected
workload-budget acceptance remain separately recorded below when executed.

The fan-off experiment reached about 90 C under full four-core conversion and
firmware thermal limiting was observed. A selected existing encoder was limited
to one core; measured overall CPU then fell from about 98% to 28% and temperature
fell to around 85 C during the initial cooldown. Compilation adds temporary load.
This does not prove a stable fanless equilibrium or future inference performance.

Final normal-user transitions passed: quiet reported its actual mode and selected
encoder quota 2s per second; auto restored its observed 2.4 GHz maximum and original
unlimited quota; silent reported its mode, 1.5 GHz cap and quota 1s per second.
The first auto test exposed that systemd displays `infinity` but rejects that
literal assignment; reset now uses the supported empty CPUQuota assignment and
real retest restored the original unlimited value. Final independent panel status
reported silent, 0 RPM, temperature about 84 C and fresh CPU percentage; camera/
microphone remained inactive. Ten Rust tests, Python bridge checks and isolated
installer snapshot/retry/rollback fixtures passed. Mode-specific receiving
workloads remain private. Actual boot adoption and human visual/thermal preference
acceptance remain unverified; no reboot or sensor capture was performed.

## Finite WiggleControl supervision — 2026-10-13

The receiving panel accepted `input 20`, acknowledged preparation before starting
the installed native runtime, and reported managed activity `input` separately
from recording. The independent observer reported camera-open and microphone
inactive. A screenshot confirmed the camera and LOCAL input markers above the
task-owned fullscreen qualification page. Scoped Stop ended the managed process
group; subsequent status reported stopped and actual camera use false. No stored
media or microphone capture was produced. This qualifies start/Stop and visible
ownership, not human hand accuracy or touch semantics. Hardware bindings and
binary rollback locations stay in the receiving private overlay.

Native touch mode receiving extension: `input 5 touch` and default mouse-mode
session each completed without error after the WiggleControl expiry correction.
The observer saw camera use, then closure, with microphone inactive; the matching
virtual device was absent afterward. Expanded state identifies hands-touch or
hands-mouse; input is not recording. Two-hand reference inference ran separately
without camera on Pi 5. Physical hand gesture/touch accuracy remains unqualified.


## Bounded managed input summaries — 2026-10-13

The panel retains at most twenty owner-generated numbered input logs at session
start, protecting the current log even after clock regression. A filesystem test
creates twenty-five numbered records and unrelated capture/media/archive paths,
directories and a symlink; only the expected old numbered records are removed.
Repeating the operation is stable; invalid zero retention is rejected. It passed
on the development host and receiving ARM64 Rust 1.85.1. Full-feature source tests
and Clippy passed; existing no-UI dead-code warnings remain unchanged.

The receiving release was installed with the prior full-input panel retained;
only its user service restarted. A three-second real input session completed,
then reported camera/microphone inactive, silent power mode and seven valid
numbered logs within the limit. This demonstrates wired session operation, not
that twenty-plus live logs were present. No external files or selected recordings
were pruned. Evidence/custody stays in the receiving private overlay.

## Native desktop hand surface — 2026-10-13

The receiving Rust 1.85.1 release renders a labeled fictional hand over the
existing fullscreen Chromium Wayland page. A compositor screenshot and
layer-shell configure trace confirm 1920x1080 after selecting exclusive zone -1,
which ignores other surfaces' reserved zones without reserving its own area.
Keyboard interactivity is zero and the skeleton surface repeatedly commits an
empty input region. The independent compact status panel remains visible.

Real uinput mouse events passed through the overlay into Chromium: two trusted
clicks, one completed drag, wheel delivery and producer-watchdog release.
Read-only browser observation reported focus retained, 1920x1080 and no web hand
canvas. The initial automated sample arrived before a pointer event was observed;
the bounded harness was corrected to await the first event before targeting.
No camera/microphone or inference was opened.

A process sample during the fictional rendering showed about 3.6% CPU and 36 MiB
RSS. This is an illustrative short rendering sample, not a perception benchmark
or a steady-state CPU guarantee. Source formatting, fourteen Rust tests (including
malformed/bounded response and stale feedback cases), no-UI tests and Clippy passed.

Physical hand-overlay alignment, camera-driven click/drag comfort, producer Stop
cleanup and countdown visibility still require human-supervised live acceptance.
The current visual palm workspace does not establish relative-cursor coincidence.
Receiving binaries and exact routes remain in Shared Resources' private overlay.

## Native hand presentation scheduling correction

The human reported a less fluid native skeleton than the preceding web preview.
The native UI previously polled metadata every 70 ms and scheduled drawing every
100 ms; it now polls every 30 ms and schedules drawing every 33 ms. This reduces
presentation scheduling delay, without inventing intermediate landmarks or
changing 150 ms input eligibility / 300 ms visual retention. Inference FPS,
physical fluency and live-session resource consumption remain separate measured
questions. The change affects only the optional finite hand overlay, not the
always-running compact status panel. Rust alone does not establish fluency.

## Two-diagonal calibration controls

The finite native overlay understands upper_left/lower_right sampling states and
CALIBRATE now queues the owning producer's same-origin diagonal route. NEXT
requires the same fresh sample count; corner review and explicit acceptance stay
separate. Raw sampling feedback does not ask the human to reach already-mapped
screen edges. The classic producer route remains available. Tests (including
no-UI build) and all-target Clippy passed. Physical comfort/precision are pending.
The requested last-skeleton retention during missing data is deferred.

## Guided calibration UI timing

The optional finite hand overlay now renders guided MOVE / HOLD STILL / WAIT FOR
TRACKING / completion/retry states with a producer-tied step countdown. Each step
has its own paint acknowledgment, emitted after fresh full-hand feedback is drawn
on the matching output; prior-step receipts do not acknowledge a new step. Manual
calibration controls remain insensitive during guidance, while Stop is available.
UI expiry can be up to 180 seconds to cover the separate camera-only 150-second
bound; original production panel and device-writer bounds are unchanged.

The feedback parser now accepts null blocked_reason as empty text, matching the
producer's pressed-pinch state. Previously valid pressed metadata could fail parsing.
A regression covers that case and invalid guided steps. Source tests/Clippy passed;
actual guided timing/comfort remains pending. No physical trial was started here.

## Stable geometry independent of input eligibility

Nicolás reported repeated whole-hand pink/cyan flicker and unusable pinch/drag.
Source rendering changed whole-hand color at the consumer's 150 ms freshness
boundary and removed geometry after 300 ms or a missing-landmark frame. The optional
native overlay now retains one last metadata skeleton without refreshing its
receipt time, keeps bone color cyan, and marks delayed retained geometry explicitly
as LAST KNOWN HAND / NO NEW GEOMETRY. Current live metadata alone controls press
indication/Stop/readiness; retained geometry cannot acknowledge a guided step or
keep a click eligible. It disappears at test expiry/overlay shutdown.

A regression verifies missing geometry retention, unchanged timestamp/input
eligibility and replacement by a new sequence. Normal/no-UI tests and Clippy passed.
Optional input_enabled metadata now distinguishes camera-only tracking from native
input in the UI. This addresses presentation behavior, not proved pinch accuracy;
actual gesture failure diagnosis and physical acceptance remain outstanding.

## Foreground conversation companion — 2026-10-17

The native GTK one-button client passed an Xvfb/XTest journey against an actual
TLS dashboard and Telecodex native-controller/SQLite instance. Synthetic App
Server, ASR and speech peers and FFmpeg-generated audio kept this qualification
independent of physical sensors and model inference. It delivered a finalized
voice reply to Alpha's exact native question, deferred Beta's concurrent result
while recording, and played that result afterward. Three captures each received
their selected Stop; only one was submitted. Long hold retained a recording
without sending or restarting it, and window close stopped only its selected
capture. The cold integrating native context remained uncreated.

The screenshot showed Listening and fresh Microphone in use during the synthetic
capture. An initial TLS fixture incorrectly used Starlette Mount, retaining the
public prefix in the path that the receiving proxy strips; the corrected fixture
uses the actual receiving path semantics. This was a test-mount correction, not
an authentication-policy change.

Six transport/private-file tests and eight runtime/race tests pass, including
barge-in on a new question without answering a previous one. All sixteen Python
tests, locked Rust tests with and without UI, formatting and default-feature
Clippy pass. Selected Wayland installation, actual device enrollment, physical
button/hearing and complete conversational intent/clarification acceptance remain
open; these synthetic observations do not close the device issue.

## Contextual voice intent — 2026-10-17

The companion now sends a source as context rather than an implicit question
answer. Against the contextual-capable Telecodex controller, the actual GTK/
XTest/TLS/FFmpeg journey passed with synthetic native/ASR/speech peers. A voice
original reached one integrating native context without automatically answering;
the synthetic integrator selected `question_context` and `answer_question`, and
the exact retained transcript answered Alpha. Beta's concurrent labeled result
played after capture. Three selected captures stopped and one was submitted;
hold and close retained their scoped behavior. No physical sensors or inference
were used. This qualifies the mechanism, not real semantic classification.

Seventeen transport/runtime tests and all nineteen Python tests pass. New owned
player regressions cover question→worker and worker→worker barge-in context,
partial audible questions, and integrating replies without unsupported hints.
Source review found and resolved preference for stale focus and unsupported
native-answer hints. Real model quality, the integrating model's own blocking
questions, selected receiving adoption/enrollment and audibility remain open.

## Spoken destination clarification — 2026-10-17

The existing actual GTK/XTest/TLS/controller/SQLite/FFmpeg journey now also
passes an unrouted original through a spoken destination question, a later
contextual voice response and explicit controller resolution. The busy worker
receives the unchanged original task rather than the clarification. The journey
retains its semantic native-question answer, concurrent labeled result, long
hold and close coverage: five selected captures stop and three voice clips are
submitted. Native, ASR and speech peers remain synthetic; model calls and
physical sensors remain zero. This is not real-model or hearing acceptance.

Twenty transport/runtime tests and all twenty-two Python tests pass. New
regressions cover recovery of a current clarification descriptor from a retained
terminal receipt without POST, exact descriptor-to-original identity and
contextual route-question playback/response. Default Clippy and embedded-helper
build pass. Focused independent source review found no actionable issue.
Selected receiving adoption, device enrollment and physical acceptance remain
open.
