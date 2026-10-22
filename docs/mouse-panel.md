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
