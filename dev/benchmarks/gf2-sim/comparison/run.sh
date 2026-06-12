#!/usr/bin/env bash
#
# External-library comparison harness driver (JIT issue 18e69a1a).
#
# Produces side-by-side AWGN BLER curves for the gf2-sim LDPC decoder vs
# aff3ct, on the IDENTICAL parity-check matrix (exported to AList and fed to
# aff3ct via --dec-h-path) and the IDENTICAL channel (AWGN-BPSK, all-zero
# codeword), for two configurations:
#
#   1. DVB-T2 r1/2 Normal LDPC   (N=64800, K=32400, rate 1/2)
#   2. 5G NR BG1 Z=384 LDPC      (mother code, N=26112, K=8448, rate ~0.323)
#
# Both decoders run Normalized Min-Sum (NMS, normalization 0.75), flooding
# schedule, same iteration cap, same per-noise-point frame/error budgets, same
# seed. The comparison isolates channel + decoder behaviour by construction:
# same H, same channel, AZCW transmit (no encoder in the loop).
#
# aff3ct is built hermetically the FIRST time this script runs (pinned tag
# v4.4.0) into .aff3ct-build/ (gitignored). Subsequent runs reuse the binary.
#
# OUTPUTS (committed):
#   dvb-t2-r12-16qam-vs-aff3ct.csv   es_n0_db,gf2_sim_bler,aff3ct_bler,gf2_sim_fps,aff3ct_fps
#   nr-5g-bg1-r12-vs-aff3ct.csv      (same columns)
#   *.png                            (via plot.py)
#
# USAGE:
#   bash run.sh            # full sweep (committed CSVs); minutes per config
#   bash run.sh --quick    # smoke sweep (fewer frames/points); seconds
#
# Es/N0 convention: --sim-noise-type ESN0 on the aff3ct side; the gf2-sim
# sweep takes Es/N0 in dB directly (sigma = sqrt(1/(2*10^(EsN0/10))), Es=1).
set -euo pipefail

# ---------------------------------------------------------------------------
# Paths and configuration.
# ---------------------------------------------------------------------------
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Repo root: comparison/ is dev/benchmarks/gf2-sim/comparison.
REPO_ROOT="$(cd "$HERE/../../../.." && pwd)"
AFF_DIR="$HERE/.aff3ct-build"
AFF_BIN="$AFF_DIR/build/bin/aff3ct"
SCRATCH="$HERE/scratch"
AFF_TAG="v4.4.0"
AFF_REPO="https://github.com/aff3ct/aff3ct.git"

SEED=42
MAX_ITER=50

QUICK=0
if [[ "${1:-}" == "--quick" ]]; then QUICK=1; fi

if [[ "$QUICK" == "1" ]]; then
  # Smoke: tiny budgets, 3 points per config (CI-friendly, seconds).
  DVB_RANGE="-1.5:-1.0:0.25"; DVB_FRAMES=300;  DVB_ERR=30
  NR_RANGE="-4.5:-4.3:0.1";   NR_FRAMES=300;   NR_ERR=30
else
  # Full sweep: spans FER ~1e-1 -> ~1e-3 across the steep part of each
  # waterfall with enough frames for a stable 1e-2 estimate (>= a few
  # thousand frames near the bracket). Brackets located by bisection:
  #   DVB-T2 r1/2 (rate 0.50) waterfall ~ -1.35 dB Es/N0 at FER 1e-2.
  #   NR BG1 Z=384 mother (rate 0.32) waterfall ~ -4.3 dB Es/N0 (very steep).
  DVB_RANGE="-2.0:-0.8:0.2";  DVB_FRAMES=30000; DVB_ERR=300
  NR_RANGE="-4.6:-4.0:0.1";   NR_FRAMES=30000;  NR_ERR=300
fi

mkdir -p "$SCRATCH"

# ---------------------------------------------------------------------------
# Step 1: hermetic aff3ct build (once).
# ---------------------------------------------------------------------------
build_aff3ct() {
  if [[ -x "$AFF_BIN" ]]; then
    echo ">> aff3ct already built: $($AFF_BIN --version 2>&1 | head -1)"
    return
  fi
  echo ">> Hermetic aff3ct build ($AFF_TAG); this takes ~10-20 min the first time..."
  if [[ ! -d "$AFF_DIR/.git" ]]; then
    git clone --recursive --depth 1 --branch "$AFF_TAG" "$AFF_REPO" "$AFF_DIR"
  fi
  cmake -S "$AFF_DIR" -B "$AFF_DIR/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DAFF3CT_COMPILE_EXE=ON \
    -DAFF3CT_COMPILE_STATIC_LIB=OFF \
    -DCMAKE_CXX_FLAGS="-march=native"
  cmake --build "$AFF_DIR/build" --config Release -j "$(nproc)"
  echo ">> aff3ct built: $($AFF_BIN --version 2>&1 | head -1)"
}

# ---------------------------------------------------------------------------
# Step 2: export the H matrices to AList.
# ---------------------------------------------------------------------------
export_alists() {
  echo ">> Building gf2-sim comparison bins (release)..."
  ( cd "$REPO_ROOT" && cargo build -p gf2-sim --release --features test-support \
      --bin export_alist --bin ldpc_bler_sweep )
  local exp="$REPO_ROOT/target/release/export_alist"
  "$exp" --code dvb-t2-r12 --output "$SCRATCH/dvb_t2_r12.alist"
  "$exp" --code nr-bg1-r12 --output "$SCRATCH/nr_bg1_r12.alist"
  echo ">> AList SHA-256:"
  sha256sum "$SCRATCH"/*.alist
}

# ---------------------------------------------------------------------------
# Step 3a: gf2-sim sweep -> es_n0_db,gf2_sim_bler,gf2_sim_fps
# ---------------------------------------------------------------------------
run_gf2() {
  local code="$1" range="$2" frames="$3" err="$4" out="$5"
  "$REPO_ROOT/target/release/ldpc_bler_sweep" \
    --code "$code" --esn0-range "$range" \
    --max-frames "$frames" --target-errors "$err" \
    --max-iter "$MAX_ITER" --seed "$SEED" --output "$out"
}

# ---------------------------------------------------------------------------
# Step 3b: aff3ct sweep -> es_n0_db,aff3ct_bler,aff3ct_fps
# Parses the BFER terminal table (final line per noise point, --ter-freq 0):
#   Es/N0 | Eb/N0 || FRA | BE | FE | BER | FER || SIM_THR | ET/RT
# After 's/|/ /g' the '|' and '||' both collapse to whitespace, so awk fields
# are: 1=EsN0 2=EbN0 3=FRA 4=BE 5=FE 6=BER 7=FER 8=SIM_THR(Mb/s) 9=ET/RT.
# SIM_THR is BIT throughput; we record frames/s ~= SIM_THR*1e6 / N
# (informational only — wall-clock-dependent, never a contractual column).
# ---------------------------------------------------------------------------
run_aff3ct() {
  local alist="$1" n="$2" k="$3" range="$4" err="$5" out="$6"
  # Convert start:stop:step -> aff3ct -m/-M/-s.
  local m M s
  m="${range%%:*}"; local rest="${range#*:}"; M="${rest%%:*}"; s="${rest##*:}"
  echo "es_n0_db,aff3ct_bler,aff3ct_fps" > "$out"
  "$AFF_BIN" --sim-type BFER -C LDPC \
    --dec-h-path "$alist" -K "$k" -N "$n" \
    --enc-type AZCW --src-type AZCW --mdm-type BPSK --chn-type AWGN \
    --sim-noise-type ESN0 -m "$m" -M "$M" -s "$s" \
    --dec-type BP_FLOODING --dec-implem NMS --dec-norm 0.75 --dec-ite "$MAX_ITER" \
    --dec-h-reorder NONE -e "$err" --sim-seed "$SEED" --ter-freq 0 \
    | grep -vE '^#' | grep -E '[0-9]' | sed 's/|/ /g' \
    | awk -v n="$n" '{ fps = ($8+0) * 1e6 / n; printf "%s,%s,%.3f\n", $1, $7, fps }' \
      >> "$out"
}

# ---------------------------------------------------------------------------
# Step 4: merge the two single-source CSVs on es_n0_db.
# Output columns: es_n0_db,gf2_sim_bler,aff3ct_bler,gf2_sim_fps,aff3ct_fps
# ---------------------------------------------------------------------------
merge_csv() {
  local gf2="$1" aff="$2" out="$3"
  echo "# gf2-sim vs aff3ct $AFF_TAG, AWGN-BPSK, AZCW, NMS 0.75, flooding," \
       "Es/N0 (ESN0) in dB, seed $SEED, max_iter $MAX_ITER" > "$out"
  echo "es_n0_db,gf2_sim_bler,aff3ct_bler,gf2_sim_fps,aff3ct_fps" >> "$out"
  # Join on the Es/N0 key (rounded to 2 dp to dodge float formatting drift).
  awk -F, '
    FNR==1 { next }                            # skip per-file header
    NR==FNR { key=sprintf("%.2f",$1+0); gb[key]=$2; gf[key]=$3; next }
    {
      key=sprintf("%.2f",$1+0); ab=$2; af=$3
      printf "%s,%s,%s,%s,%s\n", key, (key in gb?gb[key]:""), ab,
                                  (key in gf?gf[key]:""), af
    }
  ' "$gf2" "$aff" >> "$out"
}

# ---------------------------------------------------------------------------
# Main.
# ---------------------------------------------------------------------------
build_aff3ct
export_alists

echo ">> === DVB-T2 r1/2 (N=64800, K=32400) ==="
run_gf2 dvb-t2-r12 "$DVB_RANGE" "$DVB_FRAMES" "$DVB_ERR" "$SCRATCH/gf2_dvb.csv"
run_aff3ct "$SCRATCH/dvb_t2_r12.alist" 64800 32400 "$DVB_RANGE" "$DVB_ERR" "$SCRATCH/aff_dvb.csv"
merge_csv "$SCRATCH/gf2_dvb.csv" "$SCRATCH/aff_dvb.csv" "$HERE/dvb-t2-r12-16qam-vs-aff3ct.csv"

echo ">> === 5G NR BG1 Z=384 (mother code N=26112, K=8448) ==="
run_gf2 nr-bg1-r12 "$NR_RANGE" "$NR_FRAMES" "$NR_ERR" "$SCRATCH/gf2_nr.csv"
run_aff3ct "$SCRATCH/nr_bg1_r12.alist" 26112 8448 "$NR_RANGE" "$NR_ERR" "$SCRATCH/aff_nr.csv"
merge_csv "$SCRATCH/gf2_nr.csv" "$SCRATCH/aff_nr.csv" "$HERE/nr-5g-bg1-r12-vs-aff3ct.csv"

echo ">> === Plots ==="
python3 "$HERE/plot.py" --csv "$HERE/dvb-t2-r12-16qam-vs-aff3ct.csv" \
  --title "DVB-T2 r1/2 LDPC (N=64800) vs aff3ct $AFF_TAG" \
  --output "$HERE/dvb-t2-r12-vs-aff3ct.png"
python3 "$HERE/plot.py" --csv "$HERE/nr-5g-bg1-r12-vs-aff3ct.csv" \
  --title "5G NR BG1 Z=384 LDPC (N=26112) vs aff3ct $AFF_TAG" \
  --output "$HERE/nr-5g-bg1-r12-vs-aff3ct.png"

echo ">> Done. Committed deliverables:"
echo "   $HERE/dvb-t2-r12-16qam-vs-aff3ct.csv  + .png"
echo "   $HERE/nr-5g-bg1-r12-vs-aff3ct.csv     + .png"
