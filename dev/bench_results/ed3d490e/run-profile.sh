#!/usr/bin/env bash
# Profile series of the LDPC batch decoder (jit:2a3b479d): the `3be770d5`
# session script, unchanged, over the `ldpc-batch-profile` the build record
# beside this script pins, reached as `ldpc-profile` through a BIN link. A case
# line is `kind label arm code core_arm batch passes`, `arm` a gf2 batch arm
# of that record. The identity holds every input a session reads that the
# session script and the case file do not fix: the driver, each case's route,
# environment, quality file and bundle, and the inherited variables the
# driver reads.
#
# Usage (from the worktree root):
#   <this directory>/run-profile.sh [--record PROFILE_BUILD] [--arms-record ARMS_BUILD]
#       [--smoke] <case-file> <output-dir>
# --smoke expands the cases and checks one warm dispatch and one pass without perf.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
USAGE="usage: run-profile.sh <case-file> <output-dir>"
MODE=window
RECORD="${HERE}/preparation/build-identity.json"
ARMS_RECORD=""
while [[ "${1:-}" == --* ]]; do
    case "$1" in
        --record) RECORD="$2"; shift 2 ;;
        --arms-record) ARMS_RECORD="$2"; shift 2 ;;
        --smoke) MODE=smoke; shift ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
done
if [[ "$MODE" == window && "${GF2_BENCH_WINDOW:-}" != 1 ]]; then
    echo 'profiles require GF2_BENCH_WINDOW=1' >&2; exit 2
fi
ARMS_RECORD="${ARMS_RECORD:-${RECORD}}"
CASES="$(realpath -e "${1:?${USAGE}}")"
OUT="${2:?${USAGE}}"
DRIVER=ldpc-batch-profile
# The `3be770d5` session count, so both series' medians carry the same
# order-statistic interval.
REPETITIONS=9
SCRATCH="${REPO}/target/ldpc-batch-profile-series"
export PATH="${HOME}/.cargo/bin:${PATH}"

FILES="$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')"
# shellcheck disable=SC1090
source "${REPO}/$(python3 -B "${FILES}" file profile-series.sh)"
SESSION="${REPO}/$(series_located profile-session.sh)"
PEAK="${REPO}/$(series_located record-peak-memory.py)"
recorded() { series_recorded "${RECORD}" "$@"; }
EXPECTED="$(recorded executables "${DRIVER}" sha256)"
CATALOGUE="${REPO}/$(series_recorded "${ARMS_RECORD}" catalogue path)"
mkdir -p "${SCRATCH}"
BIN="$(mktemp -d "${SCRATCH}/bin.XXXXXX")"
trap 'rm -rf "${BIN}"' EXIT
ln -s "${REPO}/$(recorded isolation target_directory)/release/${DRIVER}" "${BIN}/ldpc-profile"

# The driver's GF2_LDPC_ and RAYON_ variables as the session runs it: the
# launching shell's, under the assignments the session script's `env` line
# makes, under the case's pairs. The quality path is pinned by its digest.
effective_environment() {
    python3 -c 'import os, re, sys
session, pairs = sys.argv[1:]
line = re.search(r"^\s*env ((?:[A-Z0-9_]+=\S+ )+)\$\{envpairs\}", open(session).read(), re.M)
if line is None:
    sys.exit("no env line assigns the driver environment in " + session)
effective = {k: v for k, v in os.environ.items() if k.startswith(("GF2_LDPC_", "RAYON_"))}
effective.update(pair.split("=", 1) for pair in line.group(1).split())
effective.update(pair.split("=", 1) for pair in pairs.split())
effective.pop("GF2_LDPC_QUALITY", None)
print(",".join(f"{k}={v}".replace(" ", "+") for k, v in sorted(effective.items())))' \
        "${SESSION}" "$1"
}
series_identity() {
    local target line kind label route bundle quality env quality_digest bundle_digest effective
    target="$(readlink -m "${BIN}/ldpc-profile")"
    if [[ "$(series_digest "${target}")" != "${EXPECTED}" ]]; then
        echo "${target} differs from ${DRIVER} in ${RECORD}" >&2
        return 2
    fi
    line="ldpc-profile=${EXPECTED} profile-session.sh=$(series_digest "${SESSION}")"
    line+=" cases=$(series_digest "${CASES}") arms.json=$(series_digest "${CATALOGUE}")"
    # The bundle manifest pins the digests of the files the driver loads.
    while IFS=$'\t' read -r kind label route bundle _ _ _ _ quality env; do
        quality_digest="$(series_digest "${quality}")" || return 2
        bundle_digest="$(series_digest "${bundle}/manifest.json")" || return 2
        effective="$(effective_environment "${env}")" || return 2
        line+=" ${kind}/${label}=route:${route},env:${effective},quality:${quality_digest},bundle:${bundle_digest}"
    done <"$1"
    echo "${line}"
}
series_cases() {
    python3 -B - "${CASES}" "${PEAK}" "${ARMS_RECORD}" "${SCRATCH}" <<'PY'
import importlib.util, json, pathlib, shutil, sys, tarfile, tempfile
cases, peak_path, record, scratch = sys.argv[1:]
spec = importlib.util.spec_from_file_location("peak", peak_path)
peak = importlib.util.module_from_spec(spec)
spec.loader.exec_module(peak)
smoke = peak.smoke
ROUTES = {"gf2-batch-single-frame-arm": "single-frame", "gf2-batch-lane-arm": "lane", "gf2-batch-tuned-arm": "production"}
build = json.loads(pathlib.Path(record).read_text(encoding="utf-8"))
codes = json.loads((smoke.ROOT / build["catalogue"]["path"]).read_text(encoding="utf-8"))["codes"]
quality = peak.quality_files(build)
# A rename publishes a whole extraction.
inputs = pathlib.Path(scratch) / f"inputs-{smoke.ARCHIVE[1]}"
if not inputs.is_dir():
    partial = pathlib.Path(tempfile.mkdtemp(dir=scratch))
    with tarfile.open(smoke.by_digest(smoke.ARCHIVE)) as archive:
        archive.extractall(partial, filter="data")
    try:
        partial.rename(inputs)
    except OSError:
        shutil.rmtree(partial)
for line in open(cases, encoding="utf-8"):
    if not line.strip() or line.startswith("#"):
        continue
    kind, label, arm, code, core_arm, batch, passes = line.rstrip("\n").split("\t")
    row = build["arms"].get(arm, {})
    if row.get("executable") not in ROUTES:
        sys.exit(f"case {label}: {arm} is not a gf2 batch arm")
    env = " ".join(f"{k}={v}" for k, v in sorted(row["environment"].items()) if v)
    print("\t".join([kind, label, ROUTES[row["executable"]], str(inputs / codes[code]["bundle"]),
                     codes[code]["code"], core_arm, batch, passes,
                     str(smoke.ROOT / quality[arm, code][0]), env]))
PY
}
if [[ "$MODE" == smoke ]]; then
    mkdir -p "${OUT}"
    series_cases >"${OUT}/cases.expanded.tsv"
    series_identity "${OUT}/cases.expanded.tsv" >"${OUT}/identity.txt"
    while IFS=$'\t' read -r kind label route bundle code core_arm batch passes quality envpairs; do
        [[ "$kind" == stat ]] || continue
        # One dispatch after warm-up, without perf or a timing window.
        env RAYON_NUM_THREADS=1 GF2_LDPC_QUALITY="${quality}" ${envpairs} \
            "${BIN}/ldpc-profile" --arm "$route" --bundle "$bundle" --code "$code" \
            --core-arm "$core_arm" --batch "$batch" --passes 1 \
            >"${OUT}/${label}.json" 2>"${OUT}/${label}.err"
    done <"${OUT}/cases.expanded.tsv"
    echo 'Untimed diagnostic smoke; timing fields are not performance evidence.' >"${OUT}/README.txt"
    exit 0
fi
profile_series "${OUT}" "${BIN}" "${SESSION}" "${REPETITIONS}" \
    "LDPC batch decoder profile host record - produced by run-profile.sh"
echo "profile written to ${OUT}" >&2
