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
