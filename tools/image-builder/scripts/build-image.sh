#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
repo_dir=$(cd "$root_dir/../.." && pwd)
target_dir="$root_dir/target"
lock_file="$root_dir/sources.lock.json"
buildroot_dir=${BUILDROOT_DIR:-"$target_dir/buildroot-src"}
output_dir=${BUILDROOT_OUTPUT_DIR:-"$target_dir/buildroot-output"}

die() { printf 'error: %s\n' "$*" >&2; exit 1; }
for cmd in python3 curl tar sha256sum make gcc g++ genimage mke2fs grub-editenv truncate; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing host tool: $cmd"
done
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || die 'requires an x86-64 Linux build host'
[[ -f "$repo_dir/hub/Cargo.lock" ]] || die 'missing hub/Cargo.lock'

readarray -t source < <(python3 - "$lock_file" <<'PY'
import json, sys
with open(sys.argv[1], encoding='utf-8') as f:
    lock = json.load(f)
for field in ('version', 'url', 'sha256'):
    print(lock['buildroot'][field])
PY
)
version=${source[0]}
url=${source[1]}
expected_hash=${source[2]}
mkdir -p "$target_dir"
if [[ ! -f "$buildroot_dir/Makefile" ]]; then
  archive="$target_dir/buildroot-$version.tar.xz"
  [[ -f "$archive" ]] || curl --fail --location --retry 2 --output "$archive" "$url"
  printf '%s  %s\n' "$expected_hash" "$archive" | sha256sum --check --status || die 'Buildroot archive hash mismatch'
  mkdir -p "$buildroot_dir"
  tar -xJf "$archive" --strip-components=1 -C "$buildroot_dir"
fi

python3 - "$lock_file" "$buildroot_dir" <<'PY'
import json, pathlib, re, sys
lock = json.loads(pathlib.Path(sys.argv[1]).read_text())
root = pathlib.Path(sys.argv[2])
packages = {'mesa': 'mesa3d', 'pipewire': 'pipewire',
            'wireplumber': 'wireplumber', 'webkitgtk': 'webkitgtk',
            'labwc': 'labwc', 'systemd': 'systemd'}
errors = []
for name, package in packages.items():
    mk = root / 'package' / package / (package + '.mk')
    if not mk.exists():
        errors.append(f'{name}: package missing from pinned Buildroot')
        continue
    match = re.search(r'^' + package.upper() + r'_VERSION\s*[:?]?=\s*(\S+)',
                      mk.read_text(), re.M)
    actual = match.group(1) if match else 'unknown'
    if actual != lock[name]['version']:
        errors.append(f'{name}: Buildroot resolves {actual}, lock requests {lock[name]["version"]}')
if errors:
    sys.exit('Dependency pin gate failed:\n' + '\n'.join(errors))
PY

[[ -n ${RECOVERY_IMAGE:-} && -f ${RECOVERY_IMAGE:-} ]] || die 'set RECOVERY_IMAGE to a signed recovery partition image'
recovery_size=$(stat -c %s "$RECOVERY_IMAGE")
(( recovery_size <= 2 * 1024 * 1024 * 1024 )) || die 'recovery image exceeds 2 GiB'

make -C "$buildroot_dir" O="$output_dir" BR2_EXTERNAL="$root_dir" \
  BR2_DEFCONFIG="$root_dir/buildroot-config/homehub_x86_64_defconfig" defconfig
make -C "$buildroot_dir" O="$output_dir" BR2_EXTERNAL="$root_dir"

images="$output_dir/images"
[[ -f "$images/rootfs.squashfs" && -f "$images/bzImage" ]] || die 'Buildroot did not emit kernel and squashfs images'
root_size=$(stat -c %s "$images/rootfs.squashfs")
(( root_size * 5 <= 6 * 1024 * 1024 * 1024 * 4 )) || die 'squashfs has less than 20% slot headroom'
efi_binary="$images/efi-part/EFI/BOOT/bootx64.efi"
[[ -f "$efi_binary" ]] || die 'GRUB EFI binary missing at images/efi-part/EFI/BOOT/bootx64.efi'

input="$target_dir/genimage-input"
mkdir -p "$input/EFI/BOOT"
cp "$efi_binary" "$input/EFI/BOOT/BOOTX64.EFI"
cp "$root_dir/board/homehub/grub.cfg" "$input/EFI/BOOT/grub.cfg"
cp "$images/bzImage" "$input/bzImage"
cp "$images/rootfs.squashfs" "$input/rootfs.squashfs"
cp "$RECOVERY_IMAGE" "$input/recovery.img"
grub-editenv "$input/EFI/BOOT/grubenv" create
grub-editenv "$input/EFI/BOOT/grubenv" set active_slot=a previous_slot=b boot_counter=0
truncate -s 16G "$input/data.ext4"
mke2fs -q -F -t ext4 -L HOMEHUB_DATA "$input/data.ext4"
genimage --rootpath "$output_dir/target" --tmppath "$target_dir/genimage-tmp" \
  --inputpath "$input" --outputpath "$target_dir" \
  --config "$root_dir/board/homehub/genimage.cfg"
[[ -s "$target_dir/homehub-os.img" ]] || die 'genimage did not emit homehub-os.img'
printf 'image: %s\n' "$target_dir/homehub-os.img"
