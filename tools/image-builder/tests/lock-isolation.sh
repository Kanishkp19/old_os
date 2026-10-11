#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
unit="$root_dir/rootfs-overlay/etc/systemd/system/homehub-lock.service"
portals="$root_dir/rootfs-overlay/etc/xdg/xdg-desktop-portal/homehub-portals.conf"

[[ -f "$unit" ]] || { echo "missing homehub-lock.service" >&2; exit 1; }
grep -q '^User=homehub$' "$unit"
grep -q '^NoNewPrivileges=yes$' "$unit"
grep -q '^Restart=always$' "$unit"
[[ -f "$portals" ]] || { echo "missing homehub-portals.conf" >&2; exit 1; }
grep -q '^org.freedesktop.impl.portal.ScreenCast=labwc$' "$portals"
grep -q '^org.freedesktop.impl.portal.Screenshot=labwc$' "$portals"
echo 'lock unit and portal routing validation passed'
