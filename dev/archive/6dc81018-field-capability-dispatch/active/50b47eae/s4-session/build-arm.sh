#!/usr/bin/env bash
# Build one arm's 256 ensemble members per layout-attribution-verdict-v2 §A3.
# usage: build-arm.sh <arm> <checkout-dir>
set -uo pipefail

ARM="$1"; CHECKOUT="$2"
STAGE="/tmp/gf2-ens4/$ARM"
TGT="/tmp/gf2-ens4/target-$ARM"
LEDGER="/tmp/gf2-ens4/logs/ledger-$ARM.tsv"
LOG="/tmp/gf2-ens4/logs/build-$ARM.log"
FLOOR_KB=$((3*1024*1024))   # 3 GiB free on /tmp

. /tmp/gf2-ens4/member-flags.sh

mkdir -p "$STAGE"
printf 'member_j\tE\tG\trustflags\tbytes\tsha256\ttext_vaddr\ttext_page_offset\tbuild_s\tattempts\n' > "$LEDGER"
: > "$LOG"

echo "arm=$ARM checkout=$CHECKOUT start=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
cd "$CHECKOUT" || exit 90
echo "pwd=$(pwd) head=$(git rev-parse HEAD) porcelain_lines=$(git status --porcelain --untracked-files=all | wc -l)" >> "$LOG"

build_once() {
  local j="$1" flags="$2"
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

for j in $(seq 0 255); do
  free_kb=$(df -k --output=avail /tmp | tail -1)
  if [ "$free_kb" -lt "$FLOOR_KB" ]; then
    echo "ABORT: /tmp free ${free_kb}K below floor ${FLOOR_KB}K at member $j" >> "$LOG"; exit 91
  fi
  e=$(( j / 2 )); g=$(( ( (j/4) + j ) % 2 ))
  flags="$(member_flags "$j")"; label="$(member_label "$j")"
  t0=$(date +%s); attempts=1
  echo "--- member $j attempt 1 flags=[$label] $(date -u +%H:%M:%SZ)" >> "$LOG"
  exe=$(build_once "$j" "$flags")
  if [ -z "$exe" ]; then
    attempts=2
    echo "--- member $j RETRY (v2 A8 single retry) $(date -u +%H:%M:%SZ)" >> "$LOG"
    exe=$(build_once "$j" "$flags")
  fi
  t1=$(date +%s)
  if [ -z "$exe" ]; then
    echo "FAILED-TWICE member $j" >> "$LOG"
    printf '%d\t%d\t%d\t%s\tFAILED\tFAILED\tFAILED\tFAILED\t%d\t%d\n' "$j" "$e" "$g" "$label" "$((t1-t0))" "$attempts" >> "$LEDGER"
    continue
  fi
  cp "$exe" "$STAGE/member-$j.bin"
  bytes=$(stat -c %s "$STAGE/member-$j.bin")
  sha=$(sha256sum "$STAGE/member-$j.bin" | cut -d' ' -f1)
  vaddr=$(readelf -W -S "$STAGE/member-$j.bin" | sed 's/^ *\[ *[0-9]*\] *//' | awk '$1==".text"{print $3}')
  poff=$(( 16#$vaddr % 4096 ))
  printf '%d\t%d\t%d\t%s\t%d\t%s\t0x%s\t%d\t%d\t%d\n' "$j" "$e" "$g" "$label" "$bytes" "$sha" "$vaddr" "$poff" "$((t1-t0))" "$attempts" >> "$LEDGER"
  rm -rf "$TGT"
done
echo "arm=$ARM done=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
