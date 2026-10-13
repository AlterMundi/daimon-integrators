#!/bin/sh
set -eu
[ "$(id -u)" = 0 ] && [ "$#" = 1 ] || { echo 'Usage (root): rollback.sh USER' >&2; exit 1; }
panel_user=$1
panel_uid=$(id -u "$panel_user")
backup_dir=/var/lib/daimon-integrators/rollback
[ -f "$backup_dir/manifest" ]
runuser -u "$panel_user" -- env XDG_RUNTIME_DIR="/run/user/$panel_uid" systemctl --user stop daimon-integrators-panel.service || true
systemctl disable --now daimon-integrators-observer.service || true
while IFS=' ' read -r action path; do
    case "$action" in
        restore) cp -p "$backup_dir/$(basename "$path")" "$path" ;;
        remove) rm -f "$path" ;;
    esac
done < "$backup_dir/manifest"
systemctl daemon-reload
runuser -u "$panel_user" -- env XDG_RUNTIME_DIR="/run/user/$panel_uid" systemctl --user daemon-reload || true
echo 'Panel and observer removed/restored. Media, rollback snapshot and existing access preserved.'
