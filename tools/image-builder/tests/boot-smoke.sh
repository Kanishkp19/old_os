#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
image="$root_dir/target/homehub-os.img"
log="$root_dir/target/boot-serial.log"
timeout_s=${BOOT_TIMEOUT_SECONDS:-180}
firmware=${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE.fd}

[[ -s "$image" ]] || { echo "missing image: $image" >&2; exit 1; }
[[ -f "$firmware" ]] || { echo "missing OVMF firmware: $firmware" >&2; exit 1; }
command -v qemu-system-x86_64 >/dev/null || { echo 'missing qemu-system-x86_64' >&2; exit 1; }
command -v timeout >/dev/null || { echo 'missing timeout' >&2; exit 1; }
mkdir -p "$(dirname "$log")"
: > "$log"

timeout --signal=TERM --kill-after=5 "$timeout_s" qemu-system-x86_64 \
  -machine q35 -m 2048 -smp 2 -display none \
  -vga virtio -serial "file:$log" -monitor none \
  -drive "if=pflash,format=raw,readonly=on,file=$firmware" \
  -drive "if=virtio,format=raw,readonly=on,file=$image" \
  -no-reboot || status=$?
if [[ ${status:-0} -ne 0 && ${status:-0} -ne 124 ]]; then
  echo "QEMU exited with status $status" >&2
  exit "$status"
fi
grep -Eq 'Linux version 6\.6\.' "$log" || { echo 'kernel boot marker missing' >&2; exit 1; }
grep -Eq 'systemd\[[0-9]+\]|Welcome to.*Home Hub' "$log" || { echo 'systemd boot marker missing' >&2; exit 1; }
grep -Eq 'homehub login:|Home Hub OS login:' "$log" || { echo 'login prompt missing' >&2; exit 1; }
printf 'QEMU boot smoke passed; serial log: %s\n' "$log"
