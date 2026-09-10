#!/usr/bin/env bash
# Dot-product reduction diagnostic (jit:c7113c5a).
#
# Usage (from the repository root):
#   dev/bench_results/c7113c5a/run-dot-reduction-diagnostic.sh build|run
#
# The native confirmation receipt's dot-product `unpack` values time the
# reduction of an already reduced element. This diagnostic repeats the
# reduction of the raw accumulator, with each arm's carry-less multiply, beside
# the superseded probe, on the operands the receipt's
# `internal-gf2m-dot-1024-1core` cell times; `dot-reduction-probe` documents
# the three probes.
#
# `build` compiles the survey crate natively into its own target directory, so
# the arm executables the receipts pin stay untouched. `run` executes PROCESSES
# processes of REPETITIONS repetitions each, pinned to the CPU the cell's
# single worker used, inside one hold of the canonical CCX1 mutex, and writes
# into OUT: `diagnostic.jsonl` (one raw line per process), `producing-inputs.json`
# and `identity.json` (the producing closure by the survey's own rule, every
# file digest, the executable, library, toolchain and flags) and `launcher.log`
# (every command with its time).
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=c7113c5a
SURVEY=dev/active/$ISSUE/survey
LAUNCHER=dev/bench_results/$ISSUE/run-dot-reduction-diagnostic.sh
EXT=${GF2_SURVEY_EXT:-$repo/target/$ISSUE-ext}
TARGET=$EXT/diagnostic-native
EXE=$TARGET/release/dot-reduction-probe
PREFIX=$EXT/prefix-native
PLAN=dev/bench_results/$ISSUE/v3-r1-baselines-confirmation/plan.json
CELL=internal-gf2m-dot-1024-1core
OUT=dev/bench_results/$ISSUE/v3-dot-reduction-diagnostic
PROCESSES=10
REPETITIONS=200
CPU=0
BUILD_RUSTFLAGS="-C target-cpu=native"
BUILD_CFLAGS="-O3 -march=native"
export RUSTUP_TOOLCHAIN=1.95 RAYON_NUM_THREADS=1

ACTION=${1:?build or run}
case "$ACTION" in
  build)
    CARGO_TARGET_DIR=$TARGET RUSTFLAGS=$BUILD_RUSTFLAGS GF2X_PREFIX=$PREFIX \
      GF2X_CFLAGS=$BUILD_CFLAGS ./scripts/cargo-budget.sh cargo build --release --locked \
      --manifest-path "$SURVEY/gf2-side/Cargo.toml" --bin dot-reduction-probe
    ;;
  run)
    [[ ! -e "$OUT" ]] || { echo "existing output: $OUT" >&2; exit 2; }
    [[ -x "$EXE" ]] || { echo "build first: $EXE" >&2; exit 2; }
    # The producing closure follows the survey's own rule. The recorded
    # revision names it only when every file in it is committed unchanged.
    MANIFEST=$TARGET/producing-inputs.json
    python3 "$SURVEY/make-producing-inputs.py" "$MANIFEST"
    mapfile -t SOURCES < <(python3 -c '
import json, sys
print("\n".join(json.load(open(sys.argv[1]))["build_inputs"]))
' "$MANIFEST")
    git ls-files --error-unmatch -- "${SOURCES[@]}" "$LAUNCHER" >/dev/null
    git diff --quiet HEAD -- "${SOURCES[@]}" "$LAUNCHER" \
      || { echo 'commit the producing sources first' >&2; exit 2; }
    mkdir -p "$OUT"
    cp "$MANIFEST" "$OUT/producing-inputs.json"
    LOG=$OUT/launcher.log
    CASE=$(python3 -c '
import json, sys
plan = json.load(open(sys.argv[1]))
print(json.dumps(next(c["case"] for c in plan["cells"] if c["cell_id"] == sys.argv[2])))
' "$PLAN" "$CELL")
    {
      echo "# command: $0 $*"
      echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      echo "# revision: $(git rev-parse HEAD)"
      echo "# case from $PLAN cell $CELL: $CASE"
      echo "# processes $PROCESSES, repetitions $REPETITIONS, cpu $CPU"
      echo "# toolchain: $(rustc --version)"
      echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
      echo "# producing: python3 $SURVEY/make-producing-inputs.py $MANIFEST (${#SOURCES[@]} files, committed)"
    } >>"$LOG"
    python3 - "$OUT" "$LAUNCHER" "$EXE" "$PREFIX" "$BUILD_RUSTFLAGS" "$BUILD_CFLAGS" <<'PY'
import hashlib, json, os, subprocess, sys
out, launcher, exe, prefix, rustflags, cflags = sys.argv[1:]
def digest(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()
manifest = json.load(open(f"{out}/producing-inputs.json"))
sources = sorted(set(manifest["build_inputs"]) | {launcher})
library = os.path.realpath(f"{prefix}/lib/libgf2x.so")
identity = {
    "schema": "c7113c5a-dot-reduction-identity-v1",
    "revision": subprocess.run(["git", "rev-parse", "HEAD"], check=True,
                               capture_output=True, text=True).stdout.strip(),
    "toolchain": subprocess.run(["rustc", "-vV"], check=True, capture_output=True,
                                text=True).stdout.strip().splitlines(),
    "rustflags": rustflags,
    "gf2x_cflags": cflags,
    "executable": {"path": exe, "sha256": digest(exe)},
    "gf2x_library": {"path": library, "sha256": digest(library)},
    "producing_manifest_sha256": digest(f"{out}/producing-inputs.json"),
    "sources_sha256": {path: digest(path) for path in sources},
}
with open(f"{out}/identity.json", "w") as handle:
    json.dump(identity, handle, indent=2)
    handle.write("\n")
PY
    RUN=(bash -c 'for process in $(seq 1 "$1"); do taskset -c "$2" "$3" "$4" "$process" "$5" || exit; done'
         diagnostic "$PROCESSES" "$CPU" "$EXE" "$CASE" "$REPETITIONS")
    echo "# run: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host ${RUN[*]@Q} >$OUT/diagnostic.jsonl" >>"$LOG"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host "${RUN[@]}" \
      >"$OUT/diagnostic.jsonl" 2>>"$LOG"
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ) load $(cut -d' ' -f1-3 /proc/loadavg)" >>"$LOG"
    ;;
  *) echo 'action must be build or run' >&2; exit 2 ;;
esac
