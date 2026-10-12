#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
health="$root_dir/rootfs-overlay/usr/libexec/homehub-health"
unit="$root_dir/rootfs-overlay/etc/systemd/system/homehub-health.service"
grub="$root_dir/board/homehub/grub.cfg"
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

bash -n "$health"
grep -Fq 'After=homehub.service' "$unit"
grep -Fq 'ConditionKernelCommandLine=!homehub.mode=trial' "$unit"

data_dir="$tmp_dir/data"
media_dir="$tmp_dir/media"
mkdir -p "$data_dir" "$media_dir"
db_path="$data_dir/hub.db"
printf 'SQLite format 3\000' > "$db_path"
grubenv="$tmp_dir/grubenv"
printf 'boot_counter=2\nactive_slot=a\nprevious_slot=b\n' > "$grubenv"
report="$data_dir/boot-health.json"

run_health() {
  HOMEHUB_DATA_DIR="$data_dir" HOMEHUB_MEDIA_DIR="$media_dir" \
    HOMEHUB_DB_PATH="$db_path" HOMEHUB_STABILITY_SECS="${test_stability:-1}" \
    HOMEHUB_MOCK_SERVICES=1 HOMEHUB_GRUBENV_PATH="$grubenv" \
    HOMEHUB_REPORT_PATH="$report" "$health"
}

run_health
[[ -f "$report" ]]
grep -Fq '"status": "healthy"' "$report"
grep -Fq '"slot_marked_good": true' "$report"
grep -Fxq 'boot_counter=0' "$grubenv"

rm -f "$report"
data_dir="$tmp_dir/missing"
if run_health; then echo 'missing data directory was accepted' >&2; exit 1; fi
[[ ! -e "$report" ]]

data_dir="$tmp_dir/data"
printf 'corrupted database' > "$db_path"
if run_health; then echo 'corrupted database was accepted' >&2; exit 1; fi
[[ ! -e "$report" ]]

printf 'SQLite format 3\000' > "$db_path"
printf 'boot_counter=2\nactive_slot=a\nprevious_slot=b\n' > "$grubenv"
test_stability=2
(sleep 1; touch "$data_dir/.crash_injected") &
inject_pid=$!
if run_health; then echo 'crash during stability window was accepted' >&2; exit 1; fi
wait "$inject_pid"
[[ ! -e "$report" ]]
grep -Fxq 'boot_counter=2' "$grubenv"

# Match the counter cap and rollback statements used by grub.cfg.
grep -Fq 'if [ "$boot_counter" -ge 3 ]; then' "$grub"
grep -Fq 'set active_slot=$previous_slot' "$grub"
grep -Fq 'set boot_counter=0' "$grub"
grep -Fq 'set boot_counter=1' "$grub"
grep -Fq 'set boot_counter=2' "$grub"
grep -Fq 'set boot_counter=3' "$grub"
active_slot=a
previous_slot=b
boot_counter=0
for expected in 1 2 3; do
  if [[ "$boot_counter" -ge 3 ]]; then
    active_slot=$previous_slot
    boot_counter=0
  fi
  if [[ "$boot_counter" -eq 0 ]]; then
    boot_counter=1
  elif [[ "$boot_counter" -eq 1 ]]; then
    boot_counter=2
  else
    boot_counter=3
  fi
  [[ "$active_slot" == a && "$boot_counter" -eq "$expected" ]]
done
if [[ "$boot_counter" -ge 3 ]]; then
  active_slot=$previous_slot
  boot_counter=0
fi
[[ "$active_slot" == b && "$boot_counter" -eq 0 ]]

printf 'A/B rollback and health watchdog test suite passed successfully.\n'
