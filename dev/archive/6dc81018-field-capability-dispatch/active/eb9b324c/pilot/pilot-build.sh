#!/usr/bin/env bash
# Pilot translation ensemble of the FIXED candidate for issue eb9b324c.
# 16 members, E = 0..15, 32-byte build-id translation steps, RUSTFLAGS
# construction copied from dev/active/50b47eae/s5-session/build-arm.sh.
# Build phase only: no lock is held and no timed work runs here.
set -uo pipefail

CHECKOUT=/home/vkaskivuo/Projects/gf2
S=/tmp/gf2-pilot-eb9b324c
STAGE="$S/cand-fixed"
TGT="$S/target"
LEDGER="$S/ledger-cand-fixed.tsv"
LOG="$S/build.log"
FLOOR_KB=$((3 * 1024 * 1024))

mkdir -p "$STAGE"
printf 'member_j\tE\trustflags\tbytes\tsha256\ttext_vaddr\ttext_page_offset\tbit5\tbuild_s\tattempts\n' > "$LEDGER"
: > "$LOG"

echo "pilot=eb9b324c arm=cand-fixed checkout=$CHECKOUT start=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
cd "$CHECKOUT" || exit 90
echo "head=$(git rev-parse HEAD) porcelain_lines=$(git status --porcelain --untracked-files=all | wc -l)" >> "$LOG"
echo "tracked_dirty_lines=$(git status --porcelain | wc -l)" >> "$LOG"

build_once() {
  local flags="$1" out
  rm -rf "$TGT"
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

for e in $(seq 0 15); do
  free_kb=$(df -k --output=avail /tmp | tail -1)
  if [ "$free_kb" -lt "$FLOOR_KB" ]; then
    echo "ABORT: /tmp free ${free_kb}K below floor ${FLOOR_KB}K at E=$e" >> "$LOG"; exit 91
  fi
  if [ "$e" -eq 0 ]; then
    flags=""; label="(none)"
  else
    payload=$((2 * (20 + 32 * e)))
    zeros=$(printf '%0*d' "$payload" 0)
    flags="-C link-arg=-Wl,--build-id=0x$zeros"
    label="-C link-arg=-Wl,--build-id=0x<$payload zeros>"
  fi
  t0=$(date +%s); attempts=1
  echo "--- member $e attempt 1 flags=[$label] $(date -u +%H:%M:%SZ)" >> "$LOG"
  exe=$(build_once "$flags")
  if [ -z "$exe" ]; then
    attempts=2
    echo "--- member $e RETRY $(date -u +%H:%M:%SZ)" >> "$LOG"
    exe=$(build_once "$flags")
  fi
  t1=$(date +%s)
  if [ -z "$exe" ]; then
    echo "FAILED-TWICE member $e" >> "$LOG"
    printf '%d\t%d\t%s\tFAILED\tFAILED\tFAILED\tFAILED\tFAILED\t%d\t%d\n' "$e" "$e" "$label" "$((t1 - t0))" "$attempts" >> "$LEDGER"
    continue
  fi
  cp "$exe" "$STAGE/member-$e.bin"
  bytes=$(stat -c %s "$STAGE/member-$e.bin")
  sha=$(sha256sum "$STAGE/member-$e.bin" | cut -d' ' -f1)
  vaddr=$(text_vaddr "$STAGE/member-$e.bin")
  poff=$((16#$vaddr % 4096))
  bit5=$(( (poff % 64) >= 32 ? 1 : 0 ))
  printf '%d\t%d\t%s\t%d\t%s\t0x%s\t%d\t%d\t%d\t%d\n' "$e" "$e" "$label" "$bytes" "$sha" "$vaddr" "$poff" "$bit5" "$((t1 - t0))" "$attempts" >> "$LEDGER"
  rm -rf "$TGT"
done
echo "done=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
column -t "$LEDGER"
