#!/usr/bin/env bash
# End-to-end sandbox verification (loopback only, throwaway data dir).
# Boots the hub, pairs via the dashboard pairing API, uploads with resume,
# runs the fault-injection gauntlet, and asserts DB / file-system outcomes.
#
# Usage: tools/e2e_sandbox.sh [data-dir]
set -uo pipefail

HUB_DIR="$(cd "$(dirname "$0")/../hub" && pwd)"
TOOLS="$HUB_DIR/target/debug/hh-tools"
SVC="$HUB_DIR/target/debug/hh-service"
DATA="${1:-/tmp/hh-e2e}"
DASH=http://127.0.0.1:47801
HUB=127.0.0.1:47800
SQLITE="sqlite3"

pass=0; fail=0
ok()   { echo "PASS  $1"; pass=$((pass+1)); }
bad()  { echo "FAIL  $1"; fail=$((fail+1)); }
check(){ if [ "$2" = "$3" ]; then ok "$1"; else bad "$1 (expected [$3] got [$2])"; fi; }

cleanup() { [ -n "${HUB_PID:-}" ] && kill "$HUB_PID" 2>/dev/null; wait "$HUB_PID" 2>/dev/null; }
trap cleanup EXIT

# ---------- 1. Boot the hub ----------
rm -rf "$DATA"; mkdir -p "$DATA"
"$SVC" --console --data-dir "$DATA" --library-root "$DATA/lib" >"$DATA/stdout.log" 2>&1 &
HUB_PID=$!
for i in $(seq 1 30); do
  curl -s -o /dev/null "$DASH/" && break
  sleep 0.5
done
curl -s -o /dev/null "$DASH/" && ok "hub boots: dashboard reachable on loopback" || { bad "hub boot"; tail -5 "$DATA/stdout.log"; exit 1; }
check "db created with WAL sidecars" "$(ls "$DATA"/hub.db-wal >/dev/null 2>&1 && echo yes)" "yes"
check "CA + server certs created" "$(ls "$DATA"/ca.cert.pem "$DATA"/server.cert.pem >/dev/null 2>&1 && echo yes)" "yes"

# ---------- 2. Pair a fake device through the dashboard API ----------
PAGE=$(curl -s "$DASH/")
# index.html embeds the per-start session secret as `window.HH_TOKEN="<token>"`.
TOKEN=$(printf '%s' "$PAGE" | sed -n 's/.*window\.HH_TOKEN="\([^"]*\)".*/\1/p' | head -1)
[ -n "$TOKEN" ] || { bad "dashboard token extraction"; exit 1; }
H="-H x-hh-local:$TOKEN"

curl -s $H -X POST "$DASH/api/pair/open" -o /dev/null
QR_JSON=$(curl -s $H "$DASH/api/pair/qr")
QR_PAYLOAD=$(printf '%s' "$QR_JSON" | jq -r .payload)
check "pairing QR issued (payload prefix)" "$(printf '%s' "$QR_PAYLOAD" | cut -c1-17)" "homehub://pair?h="
check "manual 6-digit code present" "$(printf '%s' "$QR_JSON" | jq -r .manual_code | grep -cE '^[0-9]{6}$')" "1"

# Sandbox rule: always pair over loopback, whatever LAN hints the QR carries.
QR_PAYLOAD=$(printf '%s' "$QR_PAYLOAD" | sed 's/a=[^&]*/a=127.0.0.1:47802/')

BUNDLE="$DATA/device.identity.pem"
"$TOOLS" fake-client pair --qr "$QR_PAYLOAD" --name e2e-phone --identity "$BUNDLE" >/dev/null 2>&1 \
  && ok "fake-client pairs (CSR → device cert issued)" || { bad "fake-client pair"; exit 1; }

# Split the identity bundle (key + device cert + CA, in that order) for curl mTLS.
mkdir -p "$DATA/id"
rm -f "$DATA/id"/*
awk -v d="$DATA/id" 'BEGIN{f=""; cert_seen=0}
  /-----BEGIN PRIVATE KEY-----/ {f="key.pem"}
  /-----BEGIN CERTIFICATE-----/ {f = cert_seen ? "ca.pem" : "cert.pem"; cert_seen=1}
  f!="" {print > (d "/" f)}
  /-----END / {f=""}
' "$BUNDLE"
ls "$DATA/id/key.pem" "$DATA/id/cert.pem" "$DATA/id/ca.pem" >/dev/null 2>&1 \
  && ok "identity bundle split for mTLS client" || bad "identity bundle split"

DEVS=$(sqlite3 "file:$DATA/hub.db?mode=ro" "SELECT COUNT(*) FROM devices WHERE status='active'")
check "device row persisted (active)" "$DEVS" "1"

# ---------- 3. Upload with verification + dedupe ----------
head -c 52428800 /dev/urandom > "$DATA/big.bin"   # 50 MB
UP=$("$TOOLS" fake-client upload --file "$DATA/big.bin" --identity "$BUNDLE" 2>&1)
printf '%s\n' "$UP" | grep -q "uploaded ✓ verified" && ok "50 MB chunked upload verified (BLAKE3)" || bad "50 MB upload: $UP"

ROWS=$(sqlite3 "file:$DATA/hub.db?mode=ro" "SELECT COUNT(*) FROM files WHERE name='big.bin' AND deleted_at IS NULL")
check "files row committed" "$ROWS" "1"
PARTS=$(find "$DATA" -name '*.part' | wc -l | tr -d ' ')
check "no .part residue after finalize" "$PARTS" "0"
STORED=$(sqlite3 "file:$DATA/hub.db?mode=ro" "SELECT rel_path FROM files WHERE name='big.bin' LIMIT 1")
# library_dir() = library_root/Library, so rel_path resolves under that.
[ -f "$DATA/lib/Library/$STORED" ] && ok "stored file exists at $STORED" || bad "stored file missing at $DATA/lib/Library/$STORED"
CHUNKS=$(sqlite3 "file:$DATA/hub.db?mode=ro" "SELECT COUNT(*) FROM file_chunks c JOIN files f ON f.id=c.file_id WHERE f.name='big.bin'")
check "chunk hashes persisted (50MB/4MiB)" "$CHUNKS" "13"

UP2=$("$TOOLS" fake-client upload --file "$DATA/big.bin" --identity "$BUNDLE" 2>&1)
printf '%s' "$UP2" | grep -q "already on hub (dedupe)" && ok "re-upload short-circuits (dedupe TR-09)" || bad "dedupe: $UP2"

# ---------- 4. Resume: crash mid-transfer, resume from server bitmap ----------
head -c 12582912 /dev/urandom > "$DATA/resume.bin"  # 12 MB = 3 chunks
ROOT=$("$TOOLS" hash "$DATA/resume.bin")
TID=$(curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" \
  -X POST "https://$HUB/v1/transfers" -H 'Content-Type: application/json' \
  -d "{\"name\":\"resume.bin\",\"size\":12582912,\"kind\":\"send\",\"client_item_id\":\"e2e-resume\",\"root_hash\":\"$ROOT\",\"chunk_size\":4194304}" | jq -r .transfer_id)
for i in 0 1; do
  dd if="$DATA/resume.bin" bs=4194304 skip=$i count=1 2>/dev/null > "$DATA/c$i"
  CH=$("$TOOLS" hash "$DATA/c$i")
  curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" \
    -X PUT "https://$HUB/v1/transfers/$TID/chunks/$i" -H "X-Chunk-Hash: $CH" \
    --data-binary @"$DATA/c$i" -o /dev/null
done
BV=$(curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" \
  "https://$HUB/v1/transfers/$TID" | jq -r .bytes_verified)
check "resume bitmap: 2 of 3 chunks verified (8388608 B)" "$BV" "8388608"
dd if="$DATA/resume.bin" bs=4194304 skip=2 count=1 2>/dev/null > "$DATA/c2"
CH2=$("$TOOLS" hash "$DATA/c2")
curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" \
  -X PUT "https://$HUB/v1/transfers/$TID/chunks/2" -H "X-Chunk-Hash: $CH2" --data-binary @"$DATA/c2" -o /dev/null
CODE=$(curl -s -o /dev/null -w '%{http_code}' --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" \
  -X POST "https://$HUB/v1/transfers/$TID/complete" -H 'Content-Type: application/json' -d "{\"root_hash\":\"$ROOT\"}")
check "resumed transfer completes 200" "$CODE" "200"

# ---------- 5. Fault-injection gauntlet ----------
FI=$(B3SUM="$TOOLS hash" bash "$(dirname "$0")/fault_injection.sh" "$HUB" "$DATA/id" 2>&1) || true
printf '%s\n' "$FI" | grep -E "^(PASS|FAIL)"
FP=$(printf '%s\n' "$FI" | grep -c '^PASS')
FF=$(printf '%s\n' "$FI" | grep -c '^FAIL')
check "fault injection assertions ($FP/$((FP+FF)))" "$FF" "0"

# ---------- 6. API invariants on the live hub ----------
CODE=$(curl -s -o /dev/null -w '%{http_code}' "https://$HUB/v1/info" --cacert "$DATA/id/ca.pem")
[ "$CODE" != "200" ] && ok "unauthenticated /v1/info refused ($CODE)" || bad "unauth /v1/info accepted"
INFO=$(curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" "https://$HUB/v1/info")
check "mTLS /v1/info succeeds (api_max=1)" "$(printf '%s' "$INFO" | jq -r .api_max)" "1"
HEALTH=$(curl -s --cacert "$DATA/id/ca.pem" --cert "$DATA/id/cert.pem" --key "$DATA/id/key.pem" "https://$HUB/v1/storage/health")
printf '%s' "$HEALTH" | jq -e '.disks' >/dev/null && ok "storage health endpoint serves disk view" || bad "storage health"
DASHOV=$(curl -s $H "$DASH/api/overview")
check "dashboard overview shows 1 device" "$(printf '%s' "$DASHOV" | jq -r .devices)" "1"

# ---------- 7. M5 metrics.sql runs against a real hub DB ----------
M=$($SQLITE "file:$DATA/hub.db?mode=ro" < "$(dirname "$0")/../docs/m5/metrics.sql" 2>&1)
if [ $? -eq 0 ] && ! printf '%s' "$M" | grep -qi 'error'; then
  ok "metrics.sql executes read-only against live-shaped DB"
else
  bad "metrics.sql: $(printf '%s' "$M" | head -2)"
fi

echo
echo "e2e sandbox: $pass passed, $fail failed"
exit $([ "$fail" = "0" ] && echo 0 || echo 1)
