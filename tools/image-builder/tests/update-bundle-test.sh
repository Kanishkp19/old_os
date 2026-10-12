#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
script="$root_dir/scripts/update-bundle.sh"
system_conf="$root_dir/rootfs-overlay/etc/rauc/system.conf"
tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/homehub-update-test.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
expect_failure() {
  if "$script" "$@" >"$tmp_dir/output" 2>&1; then
    fail "Expected failure: $*"
  fi
}

"$script" --help >"$tmp_dir/help"
grep -q 'Usage: update-bundle.sh' "$tmp_dir/help" || fail 'CLI help'
expect_failure --dry-run
expect_failure --rootfs "$tmp_dir/missing.img" --version 0.2.0 --output "$tmp_dir/update.raucb" --dry-run

[[ -f $system_conf ]] || fail 'Missing RAUC system.conf'
grep -qx 'compatible=homehub-x86_64' "$system_conf" || fail 'Compatible tag'
grep -qx 'bootloader=grub' "$system_conf" || fail 'GRUB bootloader'
awk '/^\[slot.rootfs.0\]$/{slot=1; next} /^\[/{slot=0} slot && /^device=\/dev\/disk\/by-partlabel\/root_a$/{device=1} slot && /^bootname=a$/{boot=1} END{exit !(device && boot)}' "$system_conf" || fail 'Slot A'
awk '/^\[slot.rootfs.1\]$/{slot=1; next} /^\[/{slot=0} slot && /^device=\/dev\/disk\/by-partlabel\/root_b$/{device=1} slot && /^bootname=b$/{boot=1} END{exit !(device && boot)}' "$system_conf" || fail 'Slot B'

mock_rootfs="$tmp_dir/rootfs.squashfs"
dd if=/dev/zero of="$mock_rootfs" bs=1024 count=1024 2>/dev/null
bundle_out="$tmp_dir/update.raucb"
expect_failure --version 0.2.0 --output "$bundle_out" --dry-run
expect_failure --rootfs "$mock_rootfs" --output "$bundle_out" --dry-run
expect_failure --rootfs "$mock_rootfs" --version 0.2.0 --dry-run
"$script" --rootfs "$mock_rootfs" --version 0.2.0 --output "$bundle_out" --dry-run >"$tmp_dir/manifest" 2>"$tmp_dir/diagnostics"
grep -qx 'compatible=homehub-x86_64' "$tmp_dir/manifest" || fail 'Manifest compatible tag'
grep -qx 'version=0.2.0' "$tmp_dir/manifest" || fail 'Manifest version'
grep -qx 'format=verity' "$tmp_dir/manifest" || fail 'Manifest format'
if command -v sha256sum >/dev/null 2>&1; then
  expected_sha=$(sha256sum "$mock_rootfs" | awk '{print $1}')
else
  expected_sha=$(shasum -a 256 "$mock_rootfs" | awk '{print $1}')
fi
grep -qx "sha256=$expected_sha" "$tmp_dir/manifest" || fail 'Rootfs SHA-256'
grep -q 'Dry run: manifest validated successfully' "$tmp_dir/diagnostics" || fail 'Dry run summary'

expect_failure --rootfs "$mock_rootfs" --version 0.2.0 --output "$bundle_out"
command -v openssl >/dev/null 2>&1 || fail 'openssl is required for certificate test'
openssl req -x509 -newkey rsa:2048 -keyout "$tmp_dir/test-key.pem" -out "$tmp_dir/test-cert.pem" \
  -days 1 -nodes -subj '/CN=HomeHub Update Test' 2>/dev/null
"$script" --rootfs "$mock_rootfs" --version 0.2.0 \
  --cert "$tmp_dir/test-cert.pem" --key "$tmp_dir/test-key.pem" \
  --output "$bundle_out" --dry-run >"$tmp_dir/signed-manifest" 2>"$tmp_dir/diagnostics"
cmp -s "$tmp_dir/manifest" "$tmp_dir/signed-manifest" || fail 'Signed dry-run manifest'

if ! command -v rauc >/dev/null 2>&1; then
  "$script" --rootfs "$mock_rootfs" --version 0.2.0 \
    --cert "$tmp_dir/test-cert.pem" --key "$tmp_dir/test-key.pem" \
    --output "$bundle_out" >"$tmp_dir/output" 2>"$tmp_dir/diagnostics"
  LC_ALL=C tar -tf "$bundle_out" >"$tmp_dir/archive-list"
  grep -qx 'manifest.raucm' "$tmp_dir/archive-list" || fail 'Mock bundle manifest'
  grep -qx 'rootfs.squashfs' "$tmp_dir/archive-list" || fail 'Mock bundle rootfs'
  grep -q 'unsigned mock tar archive' "$tmp_dir/diagnostics" || fail 'Mock bundle warning'
fi

printf 'Update bundle test suite passed successfully.\n'
