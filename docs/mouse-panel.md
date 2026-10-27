# Manual virtual mouse controls

When `~/.local/bin/wigglecontrol-mouse` is installed, both panel layouts expose
Mouse ON, Mouse OFF and Calibrate. The helper receives `open`, `stop` or
`calibrate` as one argument. Commands are reaped off the GTK thread; panel
startup never invokes them. Without this helper the existing Beta1 launcher
retains its idle-open behavior, and calibration controls are hidden.

ON explicitly enables the saved one-hand mouse through the selected independent
headless detector. It does not open the beta workbench or camera feedback.
OFF stops only the selected WiggleControl units. Calibrate first stops the mouse,
then opens the existing fullscreen provider calibration with live camera feedback;
its own Start and Save remain explicit. A saved calibration is imported through
WiggleControl's `prepare-provider-config` before the next activation.

The receiving helper owns local service names, verified package selection,
profile paths and rollback. Maintain those in the Shared Resources private host
record rather than hardcoding them in this panel. Recording stays disabled.
The existing CAM observation remains the actual device activity indicator;
button labels are actions, not proof of acquisition or input delivery.

## Select the complete receiving profile

A recent binary does not enable optional gestures omitted by an older settings
file. Cuevanico's2026-10-27 inspection found its dashboard still selecting the
basic profile, despite a binary containing the accepted action implementations.
Select the accepted right-click/detail/Escape/grab-scroll settings in the helper's
template paths as well as in the current source unit. Otherwise importing a newly
saved calibration can silently return to the older gesture configuration.

Before activation, use the selected binary's existing
`provider-camera-until-stop-preflight` and require `right_click:true` plus non-null
`detail`, `escape` and `scroll`, with `recording_configured:false`. The manual idle
source's lightweight `/provider/config` then confirms `detail_configured`,
`escape_configured`, `scroll_configured` and `right_click`. Preserve the calibrated
workspace and cursor settings. This verifies selection without Prepare, camera
capture or input injection; physical gesture acceptance is separate. Keep source,
worker and manual acquisition separate, including during profile upgrades.

## Persistent desktop visibility

Reuse the installed `daimon-integrators.desktop` graphical autostart entry and
the selected `daimon-integrators-panel.service`. A static user unit is expected:
the desktop starts it after the Wayland session exists. Enabling it in the
user's boot target is not a substitute for graphical autostart. The installed
GTK panel uses layer-shell's overlay layer (3), above ordinary and fullscreen
windows; it does not need a browser always-on-top extension or labwc window rule.

For a receiving host that wants the panel to remain available, use a user-unit
drop-in with `[Service]`, `Restart=always` and `RestartSec=3`. Reload user units
after changing this policy. This recovers process exits, including normal exits;
an explicit `systemctl --user stop` still stops the panel for maintenance.
Keep the chosen executable, Wayland environment, saved compact mode and existing
autostart entry. Opening the panel does not enable the camera or virtual mouse.

Verify on-screen controls as well as process status: an active PID alone did
not establish visibility on Cuevanico on2026-10-27. A scoped panel restart restored
the compact Mouse ON/OFF/Calibrate rail; the preceding loss of visibility was
observed, but its cause was not established. Do not describe Restart=always as
a visibility watchdog or a proven HDMI reconnect repair. Reuse the owner-local
status command and a desktop screenshot without activating sensors. Never
restart an active input trial just to test panel lifecycle. Actual reboot and
TV hotplug acceptance are separate from configuration and current visibility.
