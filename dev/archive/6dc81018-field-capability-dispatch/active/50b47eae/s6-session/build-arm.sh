#!/usr/bin/env bash
# Build one arm's 128 ensemble members per layout-attribution-verdict-v4 C2.
# usage: build-arm.sh <arm> <checkout-dir>
set -uo pipefail

ARM="$1"; CHECKOUT="$2"
S=/tmp/gf2-ens6
STAGE="$S/$ARM"
TGT="$S/target-$ARM"
LEDGER="$S/logs/ledger-$ARM.tsv"
LOG="$S/logs/build-$ARM.log"
MEMBERS="$S/members-$ARM.tsv"
FLOOR_KB=$((3*1024*1024))

mkdir -p "$STAGE" "$S/logs"
printf 'member_j\tE\trustflags\tbytes\tsha256\ttext_vaddr\ttext_page_offset\tbuild_s\tattempts\n' > "$LEDGER"
: > "$LOG"

echo "arm=$ARM checkout=$CHECKOUT start=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
cd "$CHECKOUT" || exit 90
echo "pwd=$(pwd) head=$(git rev-parse HEAD) porcelain_lines=$(git status --porcelain --untracked-files=all | wc -l)" >> "$LOG"

build_once() {
  local flags="$1"
  rm -rf "$TGT"
  local out
  if [ -z "$flags" ]; then
    out=$(env -u RUSTFLAGS CARGO_TARGET_DIR="$TGT" cargo +1.95.0 bench -p gf2-core --features simd \
      --bench selector_non_regression --no-run --message-format=json 2>>"$LOG" \
      | jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable')
  else
    out=$(RUSTFLAGS="$flags" CARGO_TARGET_DIR="$TGT" cargo +1.95.0 bench -p gf2-core --features simd \
      --bench selector_non_regression --no-run --message-format=json 2>>"$LOG" \
      | jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable')
  fi
  [ -n "$out" ] && [ -x "$out" ] || return 1
  printf '%s' "$out"
}

text_vaddr() {
  readelf -W -S "$1" | sed 's/^ *\[ *[0-9]*\] *//' | awk '$1==".text"{print $3}'
}

# Phase probe: the arm's ordinary build fixes P and phi (v4 C2).
probe=$(build_once "")
[ -n "$probe" ] || { echo "PROBE BUILD FAILED" >> "$LOG"; exit 95; }
cp "$probe" "$STAGE/ordinary-probe.bin"
PV=$(text_vaddr "$STAGE/ordinary-probe.bin")
PHI=$(python3 "$S/gen-members.py" "$PV" "$MEMBERS") || { echo "GEN-MEMBERS FAILED" >> "$LOG"; exit 96; }
echo "ordinary_text_vaddr=0x$PV phi=$PHI" >> "$LOG"
printf '%s\n' "$PHI" > "$S/logs/phi-$ARM.txt"
rm -rf "$TGT"

while IFS=$'\t' read -r j e; do
  [ "$j" = "member_j" ] && continue
  free_kb=$(df -k --output=avail /tmp | tail -1)
  if [ "$free_kb" -lt "$FLOOR_KB" ]; then
    echo "ABORT: /tmp free ${free_kb}K below floor ${FLOOR_KB}K at member $j" >> "$LOG"; exit 91
  fi
  if [ "$e" -eq 0 ]; then
    flags=""; label="(none)"
  else
    payload=$(( 2 * (20 + 32 * e) ))
    zeros=$(printf '%0*d' "$payload" 0)
    flags="-C link-arg=-Wl,--build-id=0x$zeros"
    label="-C link-arg=-Wl,--build-id=0x<$payload zeros>"
  fi
  t0=$(date +%s); attempts=1
  echo "--- member $j E=$e attempt 1 flags=[$label] $(date -u +%H:%M:%SZ)" >> "$LOG"
  exe=$(build_once "$flags")
  if [ -z "$exe" ]; then
    attempts=2
    echo "--- member $j RETRY (v2 A8 single retry) $(date -u +%H:%M:%SZ)" >> "$LOG"
    exe=$(build_once "$flags")
  fi
  t1=$(date +%s)
  if [ -z "$exe" ]; then
    echo "FAILED-TWICE member $j" >> "$LOG"
    printf '%d\t%d\t%s\tFAILED\tFAILED\tFAILED\tFAILED\t%d\t%d\n' "$j" "$e" "$label" "$((t1-t0))" "$attempts" >> "$LEDGER"
    continue
  fi
  cp "$exe" "$STAGE/member-$j.bin"
  bytes=$(stat -c %s "$STAGE/member-$j.bin")
  sha=$(sha256sum "$STAGE/member-$j.bin" | cut -d' ' -f1)
  vaddr=$(text_vaddr "$STAGE/member-$j.bin")
  poff=$(( 16#$vaddr % 4096 ))
  printf '%d\t%d\t%s\t%d\t%s\t0x%s\t%d\t%d\t%d\n' "$j" "$e" "$label" "$bytes" "$sha" "$vaddr" "$poff" "$((t1-t0))" "$attempts" >> "$LEDGER"
  rm -rf "$TGT"
done < "$MEMBERS"
echo "arm=$ARM done=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
