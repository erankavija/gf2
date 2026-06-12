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
  # Quick mode writes its (low-resolution) outputs under scratch/quick so it
  # NEVER clobbers the committed full-resolution deliverables in $HERE.
  DVB_RANGE="-1.5:-1.0:0.25"; DVB_FRAMES=300;  DVB_ERR=30
  NR_RANGE="-4.5:-4.3:0.1";   NR_FRAMES=300;   NR_ERR=30
  OUTDIR="$SCRATCH/quick"; mkdir -p "$OUTDIR"
else
  # Full sweep: spans FER ~1e-1 -> ~1e-3 across the steep part of each
  # waterfall with enough frames for a stable 1e-2 estimate (>= ~200 frame
  # errors at the FER=1e-2 crossing). Brackets located by bisection:
  #   DVB-T2 r1/2 (rate 0.50) waterfall ~ -1.35 dB Es/N0 at FER 1e-2.
  #   NR BG1 Z=384 mother (rate 0.32) waterfall ~ -4.3 dB Es/N0 (very steep).
  DVB_RANGE="-1.8:-0.8:0.2";  DVB_FRAMES=12000; DVB_ERR=200
  NR_RANGE="-4.6:-4.0:0.1";   NR_FRAMES=12000;  NR_ERR=200
  # Full mode writes the COMMITTED deliverables directly into the comparison dir.
  OUTDIR="$HERE"
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
# aff3ct v4.4.0 has NO per-point frame or wall-clock cap (only -e max frame
# errors), so a point with FER ~ 0 would run forever trying to reach -e errors.
# We therefore drive aff3ct ONE Es/N0 point at a time under a per-point
# `timeout`; a point that cannot accumulate -e errors within the cap is
# recorded with whatever FER it reached, or skipped if it produced no data
# line. The merge step left-joins on Es/N0, so any point aff3ct skips simply
# appears with an empty aff3ct column (the curves are compared where BOTH
# sides have data). AFF_POINT_TIMEOUT bounds each point.
AFF_POINT_TIMEOUT="${AFF_POINT_TIMEOUT:-240}"
run_aff3ct() {
  local alist="$1" n="$2" k="$3" range="$4" err="$5" out="$6"
  echo "es_n0_db,aff3ct_bler,aff3ct_fps" > "$out"
  # Enumerate the Es/N0 points start:stop:step (inclusive).
  local start stop step
  start="${range%%:*}"; local rest="${range#*:}"; stop="${rest%%:*}"; step="${rest##*:}"
  # Use awk to generate the point list (floating-point safe).
  local pts
  pts="$(awk -v a="$start" -v b="$stop" -v s="$step" \
    'BEGIN{ n=int((b-a)/s + 0.5); for(i=0;i<=n;i++) printf "%.4f\n", a+i*s }')"
  local esn0 raw rc
  raw="$(mktemp)"
  while IFS= read -r esn0; do
    [[ -z "$esn0" ]] && continue
    set +e
    timeout "$AFF_POINT_TIMEOUT" "$AFF_BIN" --sim-type BFER -C LDPC \
      --dec-h-path "$alist" -K "$k" -N "$n" \
      --enc-type AZCW --src-type AZCW --mdm-type BPSK --chn-type AWGN \
      --sim-noise-type ESN0 -R "$esn0" \
      --dec-type BP_FLOODING --dec-implem NMS --dec-norm 0.75 --dec-ite "$MAX_ITER" \
      --dec-h-reorder NONE -e "$err" --sim-seed "$SEED" --ter-freq 0 > "$raw" 2>/dev/null
    rc=$?
    set -e
    if [[ "$rc" -eq 124 ]]; then
      echo ">> aff3ct Es/N0=$esn0 dB timed out (FER ~ 0, no -e errors reached); skipped" >&2
      continue
    fi
    grep -vE '^#' "$raw" | grep -E '[0-9]' | sed 's/|/ /g' \
      | awk -v n="$n" '{ fps = ($8+0) * 1e6 / n; printf "%s,%s,%.3f\n", $1, $7, fps }' \
        >> "$out"
  done <<< "$pts"
  rm -f "$raw"
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
  # Full outer join on the Es/N0 key (rounded to 2 dp to dodge float-format
  # drift), preserving EVERY point present in either file, sorted ascending by
  # Es/N0. gf2-only points (e.g. an FER ~ 0 point aff3ct skipped on timeout)
  # keep an empty aff3ct column; the comparison is read where both columns are
  # present.
  awk -F, '
    FNR==1 { next }                            # skip per-file header
    NR==FNR {                                  # first file = gf2 sweep
      key=sprintf("%.2f",$1+0); gb[key]=$2; gf[key]=$3; seen[key]=1; next
    }
    {                                          # second file = aff3ct sweep
      key=sprintf("%.2f",$1+0); ab[key]=$2; af[key]=$3; seen[key]=1
    }
    END {
      n=0; for (k in seen) keys[++n]=k+0          # numeric copy for sorting
      for (i=1;i<=n;i++) for (j=i+1;j<=n;j++)
        if (keys[j] < keys[i]) { t=keys[i]; keys[i]=keys[j]; keys[j]=t }
      for (i=1;i<=n;i++) {
        k=sprintf("%.2f", keys[i])
        printf "%s,%s,%s,%s,%s\n", k,
          (k in gb?gb[k]:""), (k in ab?ab[k]:""),
          (k in gf?gf[k]:""), (k in af?af[k]:"")
      }
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
merge_csv "$SCRATCH/gf2_dvb.csv" "$SCRATCH/aff_dvb.csv" "$OUTDIR/dvb-t2-r12-16qam-vs-aff3ct.csv"

echo ">> === 5G NR BG1 Z=384 (mother code N=26112, K=8448) ==="
run_gf2 nr-bg1-r12 "$NR_RANGE" "$NR_FRAMES" "$NR_ERR" "$SCRATCH/gf2_nr.csv"
run_aff3ct "$SCRATCH/nr_bg1_r12.alist" 26112 8448 "$NR_RANGE" "$NR_ERR" "$SCRATCH/aff_nr.csv"
merge_csv "$SCRATCH/gf2_nr.csv" "$SCRATCH/aff_nr.csv" "$OUTDIR/nr-5g-bg1-r12-vs-aff3ct.csv"

echo ">> === Plots ==="
python3 "$HERE/plot.py" --csv "$OUTDIR/dvb-t2-r12-16qam-vs-aff3ct.csv" \
  --title "DVB-T2 r1/2 LDPC (N=64800) vs aff3ct $AFF_TAG" \
  --output "$OUTDIR/dvb-t2-r12-vs-aff3ct.png"
python3 "$HERE/plot.py" --csv "$OUTDIR/nr-5g-bg1-r12-vs-aff3ct.csv" \
  --title "5G NR BG1 Z=384 LDPC (N=26112) vs aff3ct $AFF_TAG" \
  --output "$OUTDIR/nr-5g-bg1-r12-vs-aff3ct.png"

echo ">> Done. Outputs written under: $OUTDIR"
echo "   dvb-t2-r12-16qam-vs-aff3ct.csv  + .png"
echo "   nr-5g-bg1-r12-vs-aff3ct.csv     + .png"
if [[ "$QUICK" == "1" ]]; then
  echo ">> (--quick: low-resolution smoke outputs in scratch/quick; the committed"
  echo ">>  full-resolution deliverables in the comparison dir were NOT touched.)"
fi
