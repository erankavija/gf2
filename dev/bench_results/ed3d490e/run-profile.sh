#!/usr/bin/env bash
# Profile series of the LDPC batch decoder (jit:2a3b479d).
#
# Runs the `3be770d5` session script unchanged over `ldpc-batch-profile`, the
# executable the build identity record beside this script pins. The session
# script calls `${BIN}/ldpc-profile`, so the series' BIN directory holds that
# name as a link to the recorded executable, whose digest is verified before
# every session. A case line is `kind label arm code core_arm batch passes`;
# `arm` names a gf2 batch arm of the build record, whose executable fixes the
# driver's route and whose environment and prepared quality record the case
# inherits. profile-series.sh states the execution log, resumption and
# refusals.
#
# Usage (from the worktree root):
#   dev/bench_results/ed3d490e/run-profile.sh <case-file> <output-dir>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
USAGE="usage: run-profile.sh <case-file> <output-dir>"
CASES="$(realpath -e "${1:?${USAGE}}")"
OUT="${2:?${USAGE}}"
RECORD="${HERE}/preparation/build-identity.json"
DRIVER=ldpc-batch-profile
# The `3be770d5` session count, so both series' medians carry the same
# order-statistic interval.
REPETITIONS=9
SCRATCH="${REPO}/target/ldpc-batch-profile-series"
export PATH="${HOME}/.cargo/bin:${PATH}"

FILES="$(git ls-files -- ':(glob)**/repository_files.py' ':(exclude,glob)**/inputs/**')"
located() { python3 -B "${FILES}" file "$1"; }
SESSION="${REPO}/$(located profile-session.sh)"
PEAK="${REPO}/$(located record-peak-memory.py)"
# shellcheck disable=SC1090
source "${REPO}/$(located profile-series.sh)"
recorded() { series_recorded "${RECORD}" "$@"; }
EXPECTED="$(recorded executables "${DRIVER}" sha256)"
CATALOGUE="${REPO}/$(recorded catalogue path)"
mkdir -p "${SCRATCH}"
BIN="$(mktemp -d "${SCRATCH}/bin.XXXXXX")"
trap 'rm -rf "${BIN}"' EXIT
ln -s "${REPO}/$(recorded isolation target_directory)/release/${DRIVER}" "${BIN}/ldpc-profile"

series_identity() {
    local target
    target="$(readlink -m "${BIN}/ldpc-profile")"
    if [[ "$(series_digest "${target}")" != "${EXPECTED}" ]]; then
        echo "${target} differs from ${DRIVER} in ${RECORD}" >&2
        return 2
    fi
    echo "ldpc-profile=${EXPECTED} profile-session.sh=$(series_digest "${SESSION}")" \
        "cases=$(series_digest "${CASES}") arms.json=$(series_digest "${CATALOGUE}")"
}
series_cases() {
    python3 -B - "${CASES}" "${PEAK}" "${RECORD}" "${SCRATCH}" <<'PY'
import importlib.util, json, pathlib, shutil, sys, tarfile, tempfile
cases, peak_path, record, scratch = sys.argv[1:]
spec = importlib.util.spec_from_file_location("peak", peak_path)
peak = importlib.util.module_from_spec(spec)
spec.loader.exec_module(peak)
smoke = peak.smoke
# The route each gf2 batch arm executable decodes on.
ROUTES = {"gf2-batch-single-frame-arm": "single-frame", "gf2-batch-lane-arm": "lane"}
build = json.loads(pathlib.Path(record).read_text(encoding="utf-8"))
codes = json.loads((smoke.ROOT / build["catalogue"]["path"]).read_text(encoding="utf-8"))["codes"]
quality = peak.quality_files(build)
# Bundles extract once per archive digest; a rename publishes a whole extraction.
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
profile_series "${OUT}" "${BIN}" "${SESSION}" "${REPETITIONS}" \
    "LDPC batch decoder profile host record - produced by run-profile.sh"
echo "profile written to ${OUT}" >&2
