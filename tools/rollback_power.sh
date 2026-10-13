#!/bin/sh
set -eu
[ "$(id -u)" = 0 ] && [ "$#" = 1 ] || { echo 'Usage as administrator: rollback_power.sh OPERATOR' >&2; exit 1; }
operator=$1
operator_uid=$(id -u "$operator")
backup=/var/lib/daimon-integrators/power-rollback
[ -f "$backup/manifest" ] || { echo 'Power rollback manifest missing' >&2; exit 1; }
runuser -u "$operator" -- env XDG_RUNTIME_DIR="/run/user/$operator_uid" /usr/local/bin/daimon-integrators power auto
systemctl disable --now daimon-integrators-power.service
while IFS=' ' read -r action path; do
    case "$action" in
        restore) cp -p "$backup/$(basename "$path")" "$path";;
        remove) rm -f "$path";;
        *) exit 1;;
    esac
done < "$backup/manifest"
systemctl daemon-reload
previous=/var/lib/daimon-integrators/power-overlay-before/daimon-integrators
if [ -f "$previous" ]; then
    install -m 755 "$previous" /usr/local/bin/daimon-integrators.new
    mv /usr/local/bin/daimon-integrators.new /usr/local/bin/daimon-integrators
    runuser -u "$operator" -- env XDG_RUNTIME_DIR="/run/user/$operator_uid" systemctl --user restart daimon-integrators-panel.service
fi
echo 'Power grant/unit/selection, background quotas and retained previous panel restored.' 
