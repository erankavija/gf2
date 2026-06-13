#!/usr/bin/env bash
# Extension sweeps: five configs whose initial window stopped just above the
# FER=1e-4 crossing (cliff underestimated). These add the missing sub-1e-4
# points so the crossing is bracketed by MEASURED data. Output goes to
# curve_<slug>_ext/ scratch dirs; merge into the main CSV afterwards.
set -euo pipefail
OUT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$OUT" rev-parse --show-toplevel)"
BIN="$ROOT/target/release/dvb_t2_awgn_campaign"
SEED=42; TARGET_ERRORS=100; MAX_FRAMES=1200000

# Minimal: just enough points to put ONE measured sample below the FER=1e-4
# crossing (cliff slope ~22 decades/dB => one 0.05 dB step past the current
# deepest point is already below 1e-4). Stop ranges (start:stop:step) chosen so
# only the listed points are produced.
CONFIGS=(
  "2/3 16qam 9.05:9.06:0.05"
  "3/4 16qam 10.25:10.26:0.05"
  "1/2 64qam 10.55:10.56:0.05"
  "2/3 64qam 14.05:14.11:0.05"
  "3/4 64qam 15.65:15.66:0.05"
)
slug() { echo "$1" | tr '/' '_'; }
for cfg in "${CONFIGS[@]}"; do
  read -r rate mod range <<<"$cfg"
  dir="$OUT/curve_$(slug "$rate")_${mod}_ext"
  mkdir -p "$dir"
  resume=""
  if compgen -G "$dir/checkpoints/*" >/dev/null 2>&1; then resume="--resume"; fi
  echo "=== $(date -Iseconds) :: EXT $rate $mod :: $range :: ${resume:-fresh} ==="
  "$BIN" --rate "$rate" --modulation "$mod" --esn0-range "$range" \
    --target-errors "$TARGET_ERRORS" --max-frames "$MAX_FRAMES" \
    --decoder sumproduct --demap exactlogmap --gpu --seed "$SEED" \
    --output-dir "$dir" $resume
  echo "=== $(date -Iseconds) :: EXT $rate $mod DONE ==="
done
echo "=== ALL EXTENSIONS COMPLETE :: $(date -Iseconds) ==="
