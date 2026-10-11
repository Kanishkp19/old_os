#!/usr/bin/env bash
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
day=all
evidence=
for arg in "$@"; do
  case "$arg" in
    --day=1|--day=2|--day=3|--day=4|--day=5|--day=all) day=${arg#*=} ;;
    --evidence=*) evidence=${arg#*=} ;;
    *) echo "usage: $0 [--day=1|2|3|4|5|all] [--evidence=FILE]" >&2; exit 2 ;;
  esac
done
command -v jq >/dev/null || { echo 'jq is required' >&2; exit 2; }
results="$here/results.json"
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
cp "$results" "$tmp"
if [[ -n "$evidence" ]]; then
  [[ -f "$evidence" ]] || { echo "missing evidence: $evidence" >&2; exit 2; }
  # Evidence format: {"cases":{"2.labwc_boot":{"status":"pass","artifact":"target/..."}}}
  jq -e '.cases | type == "object"' "$evidence" >/dev/null
  while IFS= read -r entry; do
    case_id=$(jq -r '.key' <<< "$entry")
    case_day=${case_id%%.*}
    case_name=${case_id#*.}
    [[ "$day" == all || "$day" == "$case_day" ]] || continue
    jq -e --arg d "$case_day" --arg c "$case_name" '.days[$d].cases[$c] != null' "$tmp" >/dev/null || { echo "unknown case $case_id" >&2; exit 2; }
    status=$(jq -r '.value.status' <<< "$entry")
    artifact=$(jq -r '.value.artifact // empty' <<< "$entry")
    [[ "$status" == pass || "$status" == fail ]] || { echo "invalid status for $case_id" >&2; exit 2; }
    [[ -n "$artifact" && -f "$here/$artifact" ]] || { echo "artifact missing for $case_id: $artifact" >&2; exit 2; }
    jq --arg d "$case_day" --arg c "$case_name" --arg s "$status" --arg a "$artifact" '.days[$d].cases[$c]={status:$s,artifact:$a}' "$tmp" > "$tmp.next"
    mv "$tmp.next" "$tmp"
  done < <(jq -c '.cases | to_entries[]' "$evidence")
fi

if [[ "$day" == all || "$day" == 1 ]]; then
  jq -e '.buildroot.version and .linux.version and .mesa.version and .webkitgtk.version and .tauri.version and .labwc.version and .smithay.version' "$here/dependencies.lock.json" >/dev/null
  # Candidate versions alone cannot pass the inventory without source hashes and generated lockfiles.
fi
if [[ -s "$here/target/homehub-spike.img" ]]; then
  if command -v sha256sum >/dev/null; then
    digest=$(sha256sum "$here/target/homehub-spike.img" | awk '{print $1}')
  else
    digest=$(shasum -a 256 "$here/target/homehub-spike.img" | awk '{print $1}')
  fi
  jq --arg digest "$digest" '.image_sha256=$digest' "$tmp" > "$tmp.next"
  mv "$tmp.next" "$tmp"
fi
jq '.days |= with_entries(.value.status = (if ([.value.cases[].status] | any(. == "fail")) then "fail" elif ([.value.cases[].status] | all(. == "pass")) then "pass" else "pending" end)) | .status = (if ([.days[].status] | any(. == "fail")) then "fail" elif ([.days[].status] | all(. == "pass")) then "pass" else "pending" end)' "$tmp" > "$results"
jq '{gate,status,days:(.days | map_values(.status)),image_sha256}' "$results"
