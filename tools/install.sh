#!/bin/sh
# Root installation only; source and host inputs are explicit.
set -eu
[ "$(id -u)" = 0 ] || { echo 'Run as root after building on the receiving machine' >&2; exit 1; }
[ "$#" = 3 ] || { echo 'Usage: install.sh BINARY SOURCE_DIR USER' >&2; exit 1; }
binary=$1
source_dir=$2
panel_user=$3
panel_home=$(getent passwd "$panel_user" | cut -d: -f6)
[ -n "$panel_home" ] && [ -f "$binary" ] && [ -d "$source_dir/packaging" ]
backup_dir=/var/lib/daimon-integrators/rollback
install -d -m 700 "$backup_dir"
# First installation snapshot persists across retries and subsequent builds.
if [ ! -e "$backup_dir/manifest" ]; then
    : > "$backup_dir/manifest"
    for path in /usr/local/bin/daimon-integrators /etc/systemd/system/daimon-integrators-observer.service /etc/daimon-integrators/devices.json "$panel_home/.config/systemd/user/daimon-integrators-panel.service" "$panel_home/.config/autostart/daimon-integrators.desktop"; do
        if [ -e "$path" ]; then
            cp -p "$path" "$backup_dir/$(basename "$path")"
            printf 'restore %s\n' "$path" >> "$backup_dir/manifest"
        else
            printf 'remove %s\n' "$path" >> "$backup_dir/manifest"
        fi
    done
fi
install -m 755 "$binary" /usr/local/bin/daimon-integrators
install -m 644 "$source_dir/packaging/daimon-integrators-observer.service" /etc/systemd/system/
install -d -m 755 /etc/daimon-integrators
if [ ! -e /etc/daimon-integrators/devices.json ]; then
    # Stable device selectors are receiving-local values; never commit them.
    camera=$(find /dev/v4l/by-id -name '*video-index0' -print 2>/dev/null | head -n 1)
    [ -n "$camera" ] || camera=/dev/video0
    python3 - "$camera" <<'PY'
import json,sys
from pathlib import Path
Path('/etc/daimon-integrators/devices.json').write_text(json.dumps({'camera':sys.argv[1],'audio':'plughw:CARD=USB,DEV=0'})+'\n')
PY
fi
install -d -o "$panel_user" -g "$panel_user" -m 755 "$panel_home/.config/systemd/user" "$panel_home/.config/autostart"
install -o "$panel_user" -g "$panel_user" -m 644 "$source_dir/packaging/daimon-integrators-panel.service" "$panel_home/.config/systemd/user/"
install -o "$panel_user" -g "$panel_user" -m 644 "$source_dir/packaging/daimon-integrators.desktop" "$panel_home/.config/autostart/"
systemctl daemon-reload
systemctl enable --now daimon-integrators-observer.service
echo 'Observer installed. As the desktop user: systemctl --user daemon-reload; systemctl --user restart daimon-integrators-panel.service'
