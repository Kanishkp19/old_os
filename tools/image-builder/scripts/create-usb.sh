#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: create-usb.sh --image PATH --target DEVICE [--yes] [--dry-run]
       create-usb.sh --list
  --image    Raw image or ISO to write
  --target   Whole USB disk (for example /dev/sdb or /dev/rdisk2)
  --list     List removable/USB disks
  --yes      Skip the YES prompt
  --dry-run  Validate and show the planned write without changing a disk
EOF
}
fatal() { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

image='' target='' list=0 yes=0 dry_run=0
while (($#)); do
  case "$1" in
    --image|--target)
      (($# >= 2)) || fatal "Missing value for $1"
      if [[ $1 == --image ]]; then image=$2; else target=$2; fi
      shift 2 ;;
    --list) list=1; shift ;;
    --yes) yes=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fatal "Unknown argument: $1" ;;
  esac
done

platform=$(uname -s)
[[ $platform == Linux || $platform == Darwin ]] || fatal "Unsupported host: $platform"
if ((list)); then
  if [[ $platform == Linux ]]; then
    command -v lsblk >/dev/null || fatal 'lsblk is required'
    lsblk -d -o NAME,TRAN,RM,SIZE,MODEL | awk 'NR == 1 || $0 ~ /[[:space:]]usb[[:space:]]/ || $0 ~ /[[:space:]]1[[:space:]]/'
  else
    diskutil list external physical
  fi
  exit 0
fi

[[ -n $image ]] || fatal 'Missing --image'
[[ -f $image && -r $image && -s $image ]] || fatal "Image is missing, unreadable, or empty: $image"
[[ -n $target ]] || fatal 'Missing --target'
image_size=$(wc -c < "$image" | tr -d '[:space:]')

if [[ $platform == Linux ]]; then
  command -v findmnt >/dev/null && command -v lsblk >/dev/null || fatal 'findmnt and lsblk are required'
  target=$(readlink -f "$target") || fatal 'Cannot resolve target device'
  root_source=$(findmnt -n -o SOURCE /) || fatal 'Cannot identify root disk'
  if [[ $target == "$root_source" ]]; then fatal 'Refusing to write to system root disk'; fi
  if [[ $root_source == /dev/* ]]; then
    root_ancestors=$(lsblk -sn -o NAME "$root_source" 2>/dev/null || true)
    target_name=${target##*/}
    if printf '%s\n' "$root_ancestors" | grep -Fxq "$target_name"; then
      fatal 'Refusing to write to system root disk'
    fi
  fi
  if ((dry_run)) && [[ $target == /dev/null ]]; then
    model='synthetic dry-run target'; vendor='test'; size='unlimited'; mounted=''
  else
    [[ $target == /dev/* && -b $target ]] || fatal "Target is not a block device: $target"
    [[ $(lsblk -dn -o TYPE "$target") == disk ]] || fatal 'Target must be a whole disk'
    [[ $(lsblk -dn -o TRAN "$target" | tr '[:upper:]' '[:lower:]') == usb ]] || fatal 'Target must be a USB disk'
    model=$(lsblk -dn -o MODEL "$target" | xargs)
    vendor=$(lsblk -dn -o VENDOR "$target" | xargs)
    size=$(lsblk -bdn -o SIZE "$target" | tr -d '[:space:]')
    ((size >= image_size)) || fatal 'Image is larger than target disk'
    mounted=$(lsblk -nrpo MOUNTPOINT "$target" | sed '/^[[:space:]]*$/d')
  fi
  block_size=4M
else
  root_source=$(mount | awk '$3 == "/" {print $1}')
  [[ -n $root_source ]] || fatal 'Cannot identify root disk'
  target_disk=${target/#\/dev\/rdisk/\/dev\/disk}
  [[ $target_disk != "$root_source" ]] || fatal 'Refusing to write to system root disk'
  root_whole=$(diskutil info "$root_source" 2>/dev/null | awk -F': *' '/^[[:space:]]*Part of Whole:/ {print $2}' || true)
  [[ -z $root_whole || ${target_disk##*/} != "$root_whole" ]] || fatal 'Refusing to write to system root disk'
  if ((dry_run)) && [[ $target == /dev/null ]]; then
    model='synthetic dry-run target'; vendor='test'; size='unlimited'; mounted=''
  else
    [[ $target == /dev/disk* || $target == /dev/rdisk* ]] || fatal 'Target must be a disk device'
    info=$(diskutil info "$target_disk") || fatal "Unknown disk: $target"
    [[ $(printf '%s\n' "$info" | awk -F': *' '/^[[:space:]]*Whole:/ {print $2; exit}') == Yes ]] || fatal 'Target must be a whole disk'
    [[ $(printf '%s\n' "$info" | awk -F': *' '/^[[:space:]]*Internal:/ {print $2; exit}') == No ]] || fatal 'Target must be an external disk'
    [[ $(printf '%s\n' "$info" | awk -F': *' '/^[[:space:]]*Protocol:/ {print $2; exit}') == USB ]] || fatal 'Target must be a USB disk'
    model=$(printf '%s\n' "$info" | awk -F': *' '/^[[:space:]]*Device \/ Media Name:/ {print $2; exit}')
    vendor=$(printf '%s\n' "$info" | awk -F': *' '/^[[:space:]]*Vendor:/ {print $2; exit}')
    [[ -n $vendor ]] || vendor='unknown'
    size=$(printf '%s\n' "$info" | awk '/Disk Size:/ {if (match($0, /\([0-9,]+ Bytes\)/)) {s=substr($0,RSTART+1,RLENGTH-2); sub(/ Bytes/,"",s); gsub(/,/,"",s); print s}}')
    [[ $size =~ ^[0-9]+$ ]] || fatal 'Cannot determine target disk size'
    ((size >= image_size)) || fatal 'Image is larger than target disk'
    mounted=$(diskutil list "$target_disk" | grep -E 'disk[0-9]+s[0-9]+' || true)
  fi
  block_size=4m
fi

printf 'Target: %s\nModel: %s\nVendor: %s\nSize: %s\nImage: %s (%s bytes)\n' \
  "$target" "$model" "$vendor" "$size" "$image" "$image_size"
[[ -z $mounted ]] || printf 'Warning: target has partitions or mounted volumes; they will be unmounted before writing.\n'
if ((dry_run)); then
  printf 'Dry run: validation passed; would write and verify %s bytes on %s.\n' "$image_size" "$target"
  exit 0
fi
if ((!yes)); then
  printf "Type 'YES' to confirm erasing %s (%s) and writing %s: " "$target" "$size" "$image"
  IFS= read -r answer || fatal 'Confirmation required'
  [[ $answer == YES ]] || fatal 'Confirmation declined'
fi

if [[ $platform == Linux ]]; then
  while IFS= read -r partition; do
    [[ -n $partition ]] || continue
    if [[ -n $(lsblk -nro MOUNTPOINT "$partition" | sed '/^[[:space:]]*$/d') ]]; then
      umount "$partition" || fatal "Could not unmount $partition"
    fi
  done < <(lsblk -nrpo NAME "$target")
else
  diskutil unmountDisk "$target_disk" || fatal "Could not unmount $target_disk"
fi

if command -v sha256sum >/dev/null; then
  hash_command=(sha256sum)
else
  hash_command=(shasum -a 256)
fi
source_hash=$("${hash_command[@]}" "$image" | awk '{print $1}')
dd if="$image" of="$target" bs="$block_size"
sync

# Read complete blocks, then only the remaining bytes. This avoids hashing
# trailing disk contents when the image length is not a multiple of 4 MiB.
full_blocks=$((image_size / 4194304))
tail_bytes=$((image_size % 4194304))
readback_hash=$({
  if ((full_blocks)); then dd if="$target" bs="$block_size" count="$full_blocks" 2>/dev/null; fi
  if ((tail_bytes)); then dd if="$target" bs=1 skip="$((full_blocks * 4194304))" count="$tail_bytes" 2>/dev/null; fi
} | "${hash_command[@]}" | awk '{print $1}')
[[ $source_hash == "$readback_hash" ]] || fatal 'Verification failed: hash mismatch'
printf 'Verification passed. Bootable USB created successfully.\n'
