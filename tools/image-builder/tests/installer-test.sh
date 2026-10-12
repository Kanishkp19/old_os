#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
manifest="$repo_root/tools/image-builder/installer/Cargo.toml"
cargo build --manifest-path "$manifest" --bin homehub-installer
binary="${CARGO_TARGET_DIR:-$repo_root/tools/image-builder/installer/target}/debug/homehub-installer"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

"$binary" --help > /dev/null
"$binary" --audit --json > "$tmp/audit.json"
python3 - "$tmp/audit.json" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    audit = json.load(f)
assert "meets_ram_requirement" in audit
assert isinstance(audit["detected_disks"], list)
PY

python3 - "$tmp" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
(root / "image").write_bytes(b"HOMEHUB" * (512 * 1024))
(root / "target").write_bytes(b"X" * (16 * 1024 * 1024))
PY

if "$binary" --target "$tmp/target" --image "$tmp/missing" --allow-small-target --yes --json > /dev/null 2>&1; then
  echo "missing image was accepted" >&2; exit 1
fi
if "$binary" --target "$tmp/target" --image "$tmp/image" --allow-small-target --dry-run --json < /dev/null > /dev/null 2>&1; then
  echo "unconfirmed install was accepted" >&2; exit 1
fi

"$binary" --target "$tmp/target" --image "$tmp/image" --allow-small-target --dry-run --yes --json > "$tmp/dry.json"
python3 - "$tmp" <<'PY'
from pathlib import Path
import json, sys
root = Path(sys.argv[1])
report = json.loads((root / "dry.json").read_text())
assert report["success"] is True and report["dry_run"] is True
assert report["bytes_written"] == 0
assert (root / "target").read_bytes() == b"X" * (16 * 1024 * 1024)
PY

"$binary" --target "$tmp/target" --image "$tmp/image" --allow-small-target --yes --json --recovery-dir "$tmp/recovery" > "$tmp/write.json"
python3 - "$tmp" <<'PY'
from pathlib import Path
import hashlib, json, sys
root = Path(sys.argv[1])
report = json.loads((root / "write.json").read_text())
image = (root / "image").read_bytes()
assert report["success"] is True and report["dry_run"] is False
assert report["image_sha256"] == hashlib.sha256(image).hexdigest()
assert (root / "target").read_bytes()[:len(image)] == image
assert (root / "recovery/partition-backup.json").exists()
journal = json.loads((root / "recovery/journal.json").read_text())
assert any(entry["step"] == "backup" for entry in journal["entries"])
PY

echo "Installer test suite passed successfully."
