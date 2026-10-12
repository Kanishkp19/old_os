#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
cd "$repo_root"

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT
source_dir="$tmp_dir/source"
output_dir="$tmp_dir/output"
target_dir="$tmp_dir/target"
passphrase='migration-passphrase-test'

HH_MIGRATION_TEST_MODE=seed HH_MIGRATION_TEST_SOURCE="$source_dir" \
  cargo test --manifest-path hub/Cargo.toml -p hh-tools migration::tests::script_fixture -- --exact

cargo build --manifest-path hub/Cargo.toml -p hh-tools
tool=(hub/target/debug/hh-tools)

"${tool[@]}" migration export --data-dir "$source_dir" --output "$output_dir" --passphrase "$passphrase"
"${tool[@]}" migration verify --bundle "$output_dir/identity.bundle.enc" --passphrase "$passphrase"
if "${tool[@]}" migration verify --bundle "$output_dir/identity.bundle.enc" --passphrase 'wrong-passphrase'; then
  echo 'wrong passphrase unexpectedly verified' >&2
  exit 1
fi
"${tool[@]}" migration import --bundle "$output_dir/identity.bundle.enc" \
  --db-snapshot "$output_dir/hub.db.snapshot" --target-dir "$target_dir" --passphrase "$passphrase"

for file in ca.cert.pem ca.key.enc server.cert.pem server.key.enc hub.db; do
  [[ -s "$target_dir/$file" ]] || { echo "missing imported file: $file" >&2; exit 1; }
done

HH_MIGRATION_TEST_MODE=validate HH_MIGRATION_TEST_SOURCE="$source_dir" HH_MIGRATION_TEST_TARGET="$target_dir" \
  cargo test --manifest-path hub/Cargo.toml -p hh-tools migration::tests::script_fixture -- --exact

echo 'migration roundtrip validation passed'
