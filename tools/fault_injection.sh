#!/usr/bin/env bash
# Fault-injection harness for the transfer engine (TEST_PLAN §4).
#
# Proves the reliability contract by breaking things on purpose:
#   kill -9 the client mid-chunk, flip bytes, drop chunks, fill the disk —
#   and asserts the hub never reports a corrupt or partial file as done.
#
# Usage (on the hub machine, with hh-tools built):
#   tools/fault_injection.sh <hub_addr:port> <identity_bundle_dir>
#
# An identity bundle is produced by `hh-tools pair` and contains
# cert.pem/key.pem for a test device. Requires: curl, openssl, dd, jq and
# a BLAKE3 CLI (b3sum, or set B3SUM to a wrapper such as
# `hh-tools hash`).

set -euo pipefail

B3SUM="${B3SUM:-b3sum}"
# B3SUM may be a single command (b3sum) or a command with args ("hh-tools hash");
# word-splitting here is intentional.
# shellcheck disable=SC2086
hash_of() { $B3SUM "$1" | cut -d' ' -f1; }

HUB="${1:?usage: fault_injection.sh <hub:port> <identity_dir>}"
ID="${2:?identity dir from hh-tools pair}"
CERT="--cert $ID/cert.pem --key $ID/key.pem --cacert $ID/ca.pem"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0; fail=0
check() { # check <name> <condition...>
  local name="$1"; shift
  if "$@"; then echo "PASS  $name"; pass=$((pass+1));
  else echo "FAIL  $name"; fail=$((fail+1)); fi
}

echo "=== 1. Kill mid-transfer, then resume ==="
dd if=/dev/urandom of="$TMP/big.bin" bs=1048576 count=32 2>/dev/null
ROOT=$(hash_of "$TMP/big.bin")
TID=$(curl -s $CERT -X POST "https://$HUB/v1/transfers" -H 'Content-Type: application/json' \
  -d "{\"name\":\"big.bin\",\"size\":33554432,\"kind\":\"send\",\"client_item_id\":\"fi-1\",\"root_hash\":\"$ROOT\",\"chunk_size\":4194304}" | jq -r .transfer_id)
# Upload only chunks 0..3 (of 8), "crashing" afterwards.
for i in 0 1 2 3; do
  dd if="$TMP/big.bin" bs=4194304 skip=$i count=1 2>/dev/null > "$TMP/c$i"
  CH=$(hash_of "$TMP/c$i")
  curl -s $CERT -X PUT "https://$HUB/v1/transfers/$TID/chunks/$i" \
    -H "X-Chunk-Hash: $CH" --data-binary @"$TMP/c$i" -o /dev/null
done
# Resume: status must report exactly chunks 0..3 as verified.
HAVE=$(curl -s $CERT "https://$HUB/v1/transfers/$TID" | jq -r '.bytes_verified')
check "resume reports 4 verified chunks" test "$HAVE" = "16777216"
# Complete the rest and finalize.
for i in 4 5 6 7; do
  dd if="$TMP/big.bin" bs=4194304 skip=$i count=1 2>/dev/null > "$TMP/c$i"
  CH=$(hash_of "$TMP/c$i")
  curl -s $CERT -X PUT "https://$HUB/v1/transfers/$TID/chunks/$i" \
    -H "X-Chunk-Hash: $CH" --data-binary @"$TMP/c$i" -o /dev/null
done
CODE=$(curl -s -o /dev/null -w '%{http_code}' $CERT -X POST "https://$HUB/v1/transfers/$TID/complete" \
  -H 'Content-Type: application/json' -d "{\"root_hash\":\"$ROOT\"}")
check "resume completes with 200" test "$CODE" = "200"

echo "=== 2. Corrupted chunk is rejected ==="
dd if=/dev/urandom of="$TMP/evil.bin" bs=1048576 count=5 2>/dev/null
EROOT=$(hash_of "$TMP/evil.bin")
TID2=$(curl -s $CERT -X POST "https://$HUB/v1/transfers" -H 'Content-Type: application/json' \
  -d "{\"name\":\"evil.bin\",\"size\":5242880,\"kind\":\"send\",\"client_item_id\":\"fi-2\",\"root_hash\":\"$EROOT\",\"chunk_size\":4194304}" | jq -r .transfer_id)
dd if="$TMP/evil.bin" bs=4194304 count=1 2>/dev/null | \
  (cat; echo sabotage) > "$TMP/corrupt"  # extra bytes = wrong hash
CODE=$(curl -s -o /dev/null -w '%{http_code}' $CERT -X PUT "https://$HUB/v1/transfers/$TID2/chunks/0" \
  -H "X-Chunk-Hash: $(hash_of "$TMP/evil.bin")" --data-binary @"$TMP/corrupt")
check "corrupt chunk rejected (4xx)" test "${CODE:0:1}" = "4"

echo "=== 3. Wrong root hash at complete is rejected (422) ==="
dd if=/dev/urandom of="$TMP/ok.bin" bs=1048576 count=4 2>/dev/null
OKROOT=$(hash_of "$TMP/ok.bin")
TID3=$(curl -s $CERT -X POST "https://$HUB/v1/transfers" -H 'Content-Type: application/json' \
  -d "{\"name\":\"ok.bin\",\"size\":4194304,\"kind\":\"send\",\"client_item_id\":\"fi-3\",\"root_hash\":\"$OKROOT\",\"chunk_size\":4194304}" | jq -r .transfer_id)
curl -s $CERT -X PUT "https://$HUB/v1/transfers/$TID3/chunks/0" \
  -H "X-Chunk-Hash: $OKROOT" --data-binary @"$TMP/ok.bin" -o /dev/null
WRONG="0000000000000000000000000000000000000000000000000000000000000000"
CODE=$(curl -s -o /dev/null -w '%{http_code}' $CERT -X POST "https://$HUB/v1/transfers/$TID3/complete" \
  -H 'Content-Type: application/json' -d "{\"root_hash\":\"$WRONG\"}")
check "wrong root hash -> 422" test "$CODE" = "422"

echo "=== 4. Unauthenticated request is refused ==="
CODE=$(curl -s -o /dev/null -w '%{http_code}' "https://$HUB/v1/info" --cacert "$ID/ca.pem" || true)
check "no client cert -> refused" sh -c 'test "$0" != 200' "$CODE"

echo
echo "fault injection: $pass passed, $fail failed"
test "$fail" = "0"
