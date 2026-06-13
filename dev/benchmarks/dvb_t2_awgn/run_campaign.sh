#!/usr/bin/env bash
# DVB-T2 AWGN production campaign driver (e4849f07).
#
# Runs all six in-scope MODCODs sequentially on the GPU (gf2-sim hybrid
# CPU+GPU pipeline, --features hip), with SumProduct decoding + ExactLogMap
# demapping (the calibration-recommended config). Each run is resumable:
# re-invoking this script picks up each config from its last completed SNR
# checkpoint.
#
# Brackets are anchored on the waterfall knees measured by the gf2-sim
# DVB-T2 byte-identity regression (dev/benchmarks/gf2-sim/
# dvb-t2-regression-receipts.md, SumProduct+ExactLogMap) and span the
# waterfall through FER = 1e-4. Step 0.1 dB near the knee.
#
# Single GPU: configs MUST run serially (concurrent GPU campaigns contend
# on the one gfx1030 device).
set -euo pipefail

OUT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$OUT" rev-parse --show-toplevel)"
BIN="$ROOT/target/release/dvb_t2_awgn_campaign"
SEED=42
TARGET_ERRORS=100
MAX_FRAMES=1200000

# config: rate modulation esn0-range(start:stop:step)
# Windows are anchored on the per-config waterfall cliff estimated from the
# gf2-sim DVB-T2 regression FER-at-anchor (SumProduct+ExactLogMap) plus the
# measured cliff slope (~1 decade / 0.045 dB). 0.05 dB step; window stops
# ~0.05 dB past the estimated FER=1e-4 crossing — enough to plot one point
# below the crossing for the waterfall floor without grinding max-frames at
# points far below 1e-4. If a config's deepest point still shows FER>1e-4
# (cliff underestimated), extend that config's range with --resume.
CONFIGS=(
  "1/2 16qam 5.85:6.20:0.05"
  "2/3 16qam 8.70:9.00:0.05"
  "3/4 16qam 9.85:10.20:0.05"
  "1/2 64qam 10.15:10.50:0.05"
  "2/3 64qam 13.65:14.00:0.05"
  "3/4 64qam 15.25:15.60:0.05"
)

slug() { echo "$1" | tr '/' '_'; }

for cfg in "${CONFIGS[@]}"; do
  read -r rate mod range <<<"$cfg"
  dir="$OUT/curve_$(slug "$rate")_${mod}"
  mkdir -p "$dir"
  resume=""
  if [[ -d "$dir/checkpoints" ]] && compgen -G "$dir/checkpoints/*" >/dev/null; then
    resume="--resume"
  fi
  echo "=== $(date -Iseconds) :: $rate $mod :: range $range :: resume='${resume:-none}' ==="
  "$BIN" \
    --rate "$rate" --modulation "$mod" \
    --esn0-range "$range" \
    --target-errors "$TARGET_ERRORS" --max-frames "$MAX_FRAMES" \
    --decoder sumproduct --demap exactlogmap \
    --gpu --seed "$SEED" \
    --output-dir "$dir" $resume
  echo "=== $(date -Iseconds) :: $rate $mod DONE ==="
done
echo "=== ALL SIX CONFIGS COMPLETE :: $(date -Iseconds) ==="
