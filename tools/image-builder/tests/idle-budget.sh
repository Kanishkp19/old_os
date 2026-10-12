#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
binary="$repo_root/hub/target/debug/hh-tools"
cargo build --manifest-path "$repo_root/hub/Cargo.toml" --bin hh-tools

"$binary" os-metrics --help >/dev/null

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT
mock_procfs="$tmp_dir/compliant"
bad_procfs="$tmp_dir/violation"
mkdir -p "$mock_procfs/1234" "$bad_procfs"

cat > "$mock_procfs/meminfo" <<'EOF'
MemTotal:        4194304 kB
MemFree:         3500000 kB
MemAvailable:    4044304 kB
Buffers:           50000 kB
Cached:           400000 kB
SReclaimable:      50000 kB
EOF
cat > "$mock_procfs/stat" <<'EOF'
cpu  1000 100 500 98400 0 0 0 0 0 0
EOF
cat > "$mock_procfs/1234/status" <<'EOF'
Name:   hh-service
VmRSS:     45000 kB
EOF
printf 'hh-service\n' > "$mock_procfs/1234/comm"

compliant_output=$("$binary" os-metrics --procfs "$mock_procfs" --sample-seconds 1 --json)
[[ "$compliant_output" == *'"overall_pass": true'* ]]
[[ "$compliant_output" == *'"system_memory_pass": true'* ]]

cat > "$bad_procfs/meminfo" <<'EOF'
MemTotal:        4194304 kB
MemFree:         1000000 kB
MemAvailable:    3894304 kB
EOF
cp "$mock_procfs/stat" "$bad_procfs/stat"
status=0
violation_output=$("$binary" os-metrics --procfs "$bad_procfs" --sample-seconds 1 --json) || status=$?
[[ "$status" -eq 1 ]]
[[ "$violation_output" == *'"system_memory_pass": false'* ]]
[[ "$violation_output" == *'"overall_pass": false'* ]]

printf 'Idle budget and OS metrics test suite passed successfully.\n'
