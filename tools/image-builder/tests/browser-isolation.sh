#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
runner="$repo/desktop/web-runner"
browser="$repo/desktop/src-tauri/src/browser.rs"

for file in Cargo.toml src/main.rs src/control.rs src/downloads.rs; do
    [[ -f "$runner/$file" ]] || { echo "missing web runner $file" >&2; exit 1; }
done
# Source contract checks; hostile-content and namespace checks require Linux VM tests.
grep -q 'PendingApproval' "$runner/src/downloads.rs"
grep -q 'create_new(true)' "$runner/src/downloads.rs"
grep -q 'approve_download' "$runner/src/downloads.rs"
grep -q 'reject_download' "$runner/src/downloads.rs"
grep -q 'is_private()' "$runner/src/control.rs"
grep -q 'is_loopback()' "$runner/src/control.rs"
grep -q '\.local' "$runner/src/control.rs"
grep -q '\.lan' "$runner/src/control.rs"
grep -q 'config.profile_dir.join(&config.app_id)' "$runner/src/main.rs"
grep -q 'app_id != "browser" && app_id != "youtube"' "$runner/src/main.rs"
grep -q 'downloads-quarantine' "$browser"
echo 'browser isolation foundation contracts passed'
