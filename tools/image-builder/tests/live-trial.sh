#!/usr/bin/env bash
# Run against disposable VM disks. The VM command must write evidence JSON at
# the supplied path after boot, edit, crash/restart and power-off exercises.
set -euo pipefail
if [[ $# -lt 3 ]]; then
  echo "usage: $0 disposable-disk-image evidence.json trial-vm-command [arguments...]" >&2
  exit 2
fi
disk=$1
evidence=$2
shift 2
if [[ ! -f $disk || $disk == /dev/* ]]; then
  echo "refusing non-file disk target" >&2
  exit 2
fi
if [[ -e $evidence ]]; then
  echo "evidence path must not exist before VM run" >&2
  exit 2
fi
before=$(sha256sum -- "$disk" | cut -d ' ' -f 1)
result=0
"$@" || result=$?
after=$(sha256sum -- "$disk" | cut -d ' ' -f 1)
if [[ $before != "$after" ]]; then
  echo "FAIL: trial changed disposable internal disk image" >&2
  exit 1
fi
echo "PASS: internal disk image unchanged ($after)"
if [[ $result -ne 0 ]]; then exit "$result"; fi
python3 - "$evidence" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as source:
    e = json.load(source)
limit = min(536870912, e["total_ram_bytes"] // 4)
assert e["overlay_filesystem"] == "tmpfs", "overlay is not RAM-backed"
assert 0 < e["overlay_limit_bytes"] <= limit, "overlay exceeds RAM quota"
assert e["overlay_used_bytes"] <= e["overlay_limit_bytes"], "overlay exceeded limit"
assert e["internal_mounts"] == [], "internal disk mounted"
assert e["persistent_swap"] is False, "persistent swap active"
assert e["lan_listeners"] == [], "LAN listener active"
assert e["lan_advertising"] is False, "LAN advertising active"
print("PASS: RAM quota, mount and LAN evidence")
PY
exit "$result"
