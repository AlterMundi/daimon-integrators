# Local system readings and power profiles

The existing floating panel samples `/proc/stat` and local thermal/fan sysfs once
per second. CPU usage is normalized across all cores (0–100%); guest counters are
not counted twice. A first sample, unreadable counter or regressed counter remains
unknown. Temperature is the local kernel SoC sensor; fan RPM is tachometer data.
These readings work offline and do not open camera/microphone or request a model.

Compact CPU/TEMP/MODE badges and the expanded line show actual measurements.
Power mode is inferred from live fan-enable, CPU cap and thermal thresholds;
unknown/custom configurations are explicit rather than inferred from a stale
saved toggle. Tooltips include fan RPM and the CPU maximum.

## Commands

After installing the scoped power grant, use:

```sh
daimon-integrators power silent
daimon-integrators power quiet
daimon-integrators power auto
daimon-integrators status
```

- `silent`: force fan disabled; CPU maximum 1.5 GHz. Compact indicator `Zz`.
- `quiet`: temperature-governed fan with steps 60/70/77/80 °C; CPU maximum 2 GHz.
  Compact indicator `ECO`. This can still become audible during sustained heat.
- `auto`: restore standard 50/60/67.5/75 °C thermal steps and 2.4 GHz CPU maximum.
  Compact indicator `AUTO`.

The Pi's pre-existing firmware thermal protections remain in place. Fanless
operation under sustained encoding can heat up and lead to firmware throttling;
the dashboard shows actual temperature, without promising a silent mode remains
at a fixed temperature or clock speed. These profiles are Pi 5 specific.

A root-owned saved selection is restored by a finite systemd oneshot at boot.
It starts no capture or inference and does nothing without a saved selection.
The old experimental firmware CPU/fan block is removed only by its exact owned
markers so the saved profile can select every supported frequency on later boots.
Its prior contents and live settings remain in receiving rollback storage.
The earlier raw fan-off experiment was this-boot only; this selectable feature
adds explicit persisted mode restoration.

## Receiving installation and reversal

Build the ordinary receiving checkout and retain the previous working binary
before replacement. Install the binary through the existing deployment route;
`tools/install_power.sh SOURCE_DIR OPERATOR` installs the power oneshot and only
three exact passwordless helper commands. No general passwordless sudo or broad
shell grant is added. Invalid mode strings are rejected before privileged work.
The user panel remains unprivileged and keeps NoNewPrivileges.

The installer snapshots existing mode/unit/grant files before changes. Reversal:
select `auto`, disable the power oneshot, restore its retained manifest entries
and the previous working panel binary, then reload/restart only the local panel.
Restoring the original firmware file is a separate explicit rollback choice.
Keep SSH, desktop, other services and unrelated captures unchanged. Use the
receiving private overlay for exact retained backup locations and access.

Source tests verify CPU deltas and actual-state mode classification. Real
receiving command adoption, UI visibility and human perception require their
own observed evidence.

## Optional background-work budget

A receiving-owned `power-workloads.json` contains explicitly selected same-user
service names and their original normal CPU quota. Silent mode limits each selected
background unit to 100% (one core); quiet mode to 200% (two cores); auto restores
the recorded original quota. Only these configured units are affected, through
the existing user service manager. No service is started, discovered or killed.
The selected budget is reapplied when the local panel starts after login.

This follows a concrete receiving finding: fan-off at 1.5 GHz still reached
about 90 C during four-core video conversion. Restricting the selected background
encoder is necessary to reduce that sustained load; a frequency cap alone did not
establish quiet thermal equilibrium. Actual duration and temperature remain
measurements, not promised bounds. The unit binding and original quota stay in
the receiving private configuration.
