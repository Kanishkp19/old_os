#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
units="$root_dir/rootfs-overlay/etc/systemd/system"

for unit in homehub-labwc.service homehub-shell.service; do
  [[ -f "$units/$unit" ]] || { echo "missing $unit" >&2; exit 1; }
  grep -q '^User=homehub$' "$units/$unit" || { echo "$unit must run as homehub" >&2; exit 1; }
  grep -q '^NoNewPrivileges=yes$' "$units/$unit" || { echo "$unit needs NoNewPrivileges" >&2; exit 1; }
done
grep -q '^Requires=homehub-labwc.service$' "$units/homehub-shell.service"
grep -q '^ExecStart=/usr/bin/labwc$' "$units/homehub-labwc.service"
grep -q '^ExecStart=/usr/bin/hh-shell$' "$units/homehub-shell.service"
echo 'labwc and shell unit validation passed'
