#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
power="$root_dir/hub/crates/hh-service/src/power.rs"
service="$root_dir/hub/crates/hh-service/src/main.rs"
network="$root_dir/hub/crates/hh-net/src/lib.rs"
transfer="$root_dir/hub/crates/hh-transfer/src/lib.rs"

[[ -f "$power" && -f "$service" && -f "$network" && -f "$transfer" ]]
grep -q 'remote_input_revoked = true' "$power"
grep -q 'capture_revoked = true' "$power"
grep -q 'sleep_inhibit_active = false' "$power"
grep -q 'state.connections.drain_all()' "$service"
grep -q 'drain.notified()' "$network"
grep -q 'reconcile_on_startup' "$service"
grep -q 'pub fn reconcile_on_startup' "$transfer"

echo 'Power state, connection drain, and startup transfer recovery hooks are present.'
echo 'Physical suspend/resume, logind delay inhibition, and consent enforcement need Linux integration validation.'
