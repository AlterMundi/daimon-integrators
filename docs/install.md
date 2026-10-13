# Install and rollback

Use an ordinary checkout on the receiving machine, build the locked source,
and review the selected installation paths. Install required distribution
packages through the authorized administrator route. Do not change SSH, passwords,
sudo policy, network configuration or existing desktop settings for this panel.

```sh
sudo apt-get install rustc cargo libgtk-3-dev libgtk-layer-shell-dev
cargo build --release --locked
sudo tools/install.sh "$PWD/target/release/daimon-integrators" "$PWD" "$USER"
systemctl --user daemon-reload
systemctl --user restart daimon-integrators-panel.service
```

The first installer invocation snapshots replaced files or records their absence
under a root-only rollback directory. The observer is a restricted system service;
the panel is an ordinary user's service with no new privileges. A new desktop
autostart entry starts it after login. No capture starts on login/restart. Existing
service/desktop/access files outside the explicitly named component are untouched.
Receiving-local device selectors stay in `/etc/daimon-integrators/devices.json`.
Check camera/audio selectors against actual hardware before explicit capture.

```sh
sudo tools/rollback.sh "$USER"
```

This stops the added services, removes/restores installed files, and preserves
media and the original rollback. Added distro build packages are intentionally
not auto-removed. If upgrading a previously active component, re-establish its
previous service activation from recorded installation evidence after rollback.

## Optional Telecodex activity

`tools/activity_bridge.py` reuses the actual qualified owner-local dashboard
`Provider`; it does not create a second session model or Stop mechanism. Its
private JSON config selects `provider_source`, pinned `provider_sha256`, existing
`dashboard_config`, `ssh_key` and `ssh_target`. Config must be owned by its runner
and mode 0600. It requires the provider's existing human authorization before
reads and every refresh; feeds only aggregate counts. SSH writes a small JSON
projection to the receiving user's private runtime directory. Source drift fails
closed until requalified. Run `--once` for qualification before a selected local
user service; remove that added service on rollback. Native reads attach to the
existing daemon through the provider helper; no inference or inbox access occurs.

## Partial user-only preview

When only user access is available, an observer can write to a selected private
runtime file via INTEGRATOR_OBSERVATION_PATH; the panel uses the same variable.
Permission-denied process coverage remains incomplete, so this does not establish
that devices are inactive. Keep preview units separate from the final system
observer/user panel units. Stop and remove only these added preview units on
rollback, retaining media and unfinished build evidence.

The UI remembers compact/expanded mode in its user-local panel settings.
`daimon-integrators view compact` and `view expanded` use the existing owner-local
control socket. The view setting grants no device control or remote authority.

View requests are asynchronous. `compact` is requested mode; `ui_compact` confirms the widget mode was applied. Wait for UI acknowledgment/settings persistence before restarting to test retained preference.
