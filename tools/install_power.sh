#!/bin/sh
# Install only fixed profile privileges for the named existing operator.
set -eu
[ "$(id -u)" = 0 ] && [ "$#" = 2 ] || { echo 'Usage as root: install_power.sh SOURCE_DIR USER' >&2; exit 1; }
source_dir=$1
operator=$2
getent passwd "$operator" >/dev/null
case "$operator" in *[!a-zA-Z0-9_-]*|'') exit 1;; esac
backup=/var/lib/daimon-integrators/power-rollback
install -d -m 700 "$backup"
if [ ! -f "$backup/manifest" ]; then
    : > "$backup/manifest.new"
    for path in /etc/sudoers.d/91-daimon-integrators-power /etc/systemd/system/daimon-integrators-power.service /etc/daimon-integrators/power-mode.json /etc/daimon-integrators/power-workloads.json; do
        if [ -e "$path" ]; then
            cp -p "$path" "$backup/$(basename "$path")"
            printf 'restore %s\n' "$path" >> "$backup/manifest.new"
        else
            printf 'remove %s\n' "$path" >> "$backup/manifest.new"
        fi
    done
    mv "$backup/manifest.new" "$backup/manifest"
fi
rule=$(mktemp)
trap 'rm -f "$rule"' EXIT HUP INT TERM
printf '%s ALL=(root) NOPASSWD: /usr/local/bin/daimon-integrators power-apply auto, /usr/local/bin/daimon-integrators power-apply quiet, /usr/local/bin/daimon-integrators power-apply silent\n' "$operator" > "$rule"
visudo -cf "$rule"
install -m 440 "$rule" /etc/sudoers.d/91-daimon-integrators-power
install -m 644 "$source_dir/packaging/daimon-integrators-power.service" /etc/systemd/system/
systemctl daemon-reload
systemctl enable daimon-integrators-power.service
