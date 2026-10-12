#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
script="$root_dir/scripts/create-usb.sh"
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT

expect_failure() {
  local status
  if "$script" "$@" >"$temp_dir/output" 2>&1; then
    printf 'Expected failure: %s\n' "$*" >&2
    exit 1
  else
    status=$?
  fi
  [[ $status -eq 1 ]] || { printf 'Expected exit 1, got %s\n' "$status" >&2; exit 1; }
}

bash -n "$script"
"$script" --help >"$temp_dir/output"
grep -q 'Usage: create-usb.sh' "$temp_dir/output"
expect_failure --target /dev/null
grep -q 'Missing --image' "$temp_dir/output"
expect_failure --image "$temp_dir/missing.img" --target /dev/null
grep -q 'Image is missing' "$temp_dir/output"
printf 'home hub USB test image\n' > "$temp_dir/image.img"
expect_failure --image "$temp_dir/image.img" --target /dev/null
grep -q 'Target is not a block device\|Target must be a disk device' "$temp_dir/output"
"$script" --dry-run --image "$temp_dir/image.img" --target /dev/null >"$temp_dir/output"
grep -q 'Dry run: validation passed' "$temp_dir/output"
grep -q 'synthetic dry-run target' "$temp_dir/output"
printf 'create-usb tests passed\n'
