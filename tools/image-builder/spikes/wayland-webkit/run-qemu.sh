#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
target_dir="$here/target"
compositor=
for arg in "$@"; do
  case "$arg" in
    --compositor=labwc|--compositor=smithay) compositor=${arg#*=} ;;
    *) echo "usage: $0 --compositor=labwc|smithay" >&2; exit 2 ;;
  esac
done
[[ -n "$compositor" ]] || { echo 'select a compositor' >&2; exit 2; }
command -v qemu-system-x86_64 >/dev/null || { echo 'qemu-system-x86_64 is missing' >&2; exit 2; }
[[ -s "$target_dir/homehub-spike.img" && -s "$target_dir/bzImage" ]] || { echo 'run build.sh first' >&2; exit 2; }
[[ $(cat "$target_dir/image-compositor") == "$compositor" ]] || { echo 'image compositor differs from requested compositor' >&2; exit 2; }
cp "$target_dir/homehub-spike.img" "$target_dir/run-$compositor.img"
qemu-system-x86_64 \
  -machine q35 -m 2048 -smp 2 \
  -kernel "$target_dir/bzImage" \
  -append 'root=/dev/vda rw console=ttyS0' \
  -drive "file=$target_dir/run-$compositor.img,format=raw,if=virtio" \
  -device virtio-vga \
  -display "${QEMU_DISPLAY:-gtk}" \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:47801-:47801 \
  -device virtio-net-pci,netdev=net0 \
  -serial "file:$target_dir/qemu-serial.log" \
  -monitor stdio
