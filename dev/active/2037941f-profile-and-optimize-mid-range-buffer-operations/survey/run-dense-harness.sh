#!/usr/bin/env bash
# Launcher for the dense-parity measurement harness (jit:e1f9a78f).
#
# Subcommands:
#   build [--m4ri]                     build the arms, run the crate contract and the oracle
#   cells --family <id> --issue <hex8> --frozen-utc <t> --output <path>
#                                      transcribe one family into a campaign addendum
#   smoke [--m4ri]                     deterministic untimed release smoke (no receipt sample)
#   window --family <id> --addendum <path> --run-id <id> [--m4ri]
#                                      timed campaign, benchmark window only
#
# Every numeric setting comes from the frozen addendum or the protocol's shared
# settings; this launcher adds none. `~/.cargo/bin` is exported here because the
# benchmark-window unit has no login shell.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

STORY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
SURVEY="${STORY}/survey"
MANIFEST="${REPO}/${SURVEY}/dense-harness/Cargo.toml"
PRODUCING="${SURVEY}/dense-producing-inputs.json"
VALIDATION="${SURVEY}/dense-harness-validation.txt"
SMOKE_RECORD="${SURVEY}/dense-runner-smoke.txt"
LAUNCHER="${SURVEY}/run-dense-harness.sh"
GF2_TARGET="${REPO}/target/e1f9a78f-arms"
SCALAR_TARGET="${REPO}/target/e1f9a78f-scalar-arm"
M4RI_TARGET="${REPO}/target/e1f9a78f-m4ri-arm"
GF2_ARM="${GF2_TARGET}/release/dense-arm"
SCALAR_ARM="${SCALAR_TARGET}/release/dense-arm"
M4RI_ARM="${M4RI_TARGET}/release/dense-m4ri-arm"
CAMPAIGN_TOOL="${GF2_TARGET}/release/dense-campaign"
ORACLE="${GF2_TARGET}/release/dense-oracle"
SCALAR_ORACLE="${SCALAR_TARGET}/release/dense-oracle"
M4RI_GENERATION=qualified-v3

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

# The qualified M4RI install is shared under the primary checkout's
# .agents/ext; it is discovered, never rebuilt destructively. `build.rs`
# verifies the installed library against the digest the qualification record
# pins, so a different install fails the build.
m4ri_prefix() {
    if [[ -n "${GF2_M4RI_PREFIX:-}" ]]; then
        printf '%s\n' "${GF2_M4RI_PREFIX}"
        return
    fi
    local common primary candidate
    common="$(git -C "${REPO}" rev-parse --path-format=absolute --git-common-dir)"
    primary="$(dirname "${common}")"
    candidate="${primary}/.agents/ext/92385645/prefix-${M4RI_GENERATION}"
    [[ -f "${candidate}/lib/libm4ri.so" ]] || {
        echo "no qualified M4RI install at ${candidate}; see ${SURVEY}/run-m4ri-matvec-probe.sh" >&2
        exit 2
    }
    printf '%s\n' "${candidate}"
}

build_gf2() {
    CARGO_TARGET_DIR="${GF2_TARGET}" \
        ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}" --bins
    # The addendum's reference arm is the same configuration with the `simd`
    # feature removed, so it is a separate build of the same sources.
    CARGO_TARGET_DIR="${SCALAR_TARGET}" \
        ./scripts/cargo-budget.sh cargo build --release --no-default-features \
        --manifest-path "${MANIFEST}" --bin dense-arm --bin dense-oracle
}

build_m4ri() {
    local prefix
    prefix="$(m4ri_prefix)"
    GF2_M4RI_PREFIX="${prefix}" CARGO_TARGET_DIR="${M4RI_TARGET}" \
        ./scripts/cargo-budget.sh cargo build --release --features m4ri \
        --manifest-path "${MANIFEST}" --bin dense-m4ri-arm
    printf '%s\n' "${prefix}"
}

cmd_build() {
    local with_m4ri=0 prefix='' invocation
    # The record states the invocation that produced it, so the launcher's own
    # arguments are passed through rather than reconstructed.
    invocation="${LAUNCHER} build${*:+ $*}"
    [[ "${1:-}" == "--m4ri" ]] && with_m4ri=1
    build_gf2
    # The survey harness is its own workspace, so the repository CI contract
    # never reaches it: this command is the only thing that runs its contract.
    local contract passed
    contract="$(CARGO_TARGET_DIR="${GF2_TARGET}" ./scripts/cargo-budget.sh --test \
        cargo test --release --manifest-path "${MANIFEST}" --tests 2>&1)"
    echo "${contract}"
    passed="$(printf '%s\n' "${contract}" |
        awk '/^test result: ok\./ { total += $4 } END { print total + 0 }')"
    [[ "${with_m4ri}" == 1 ]] && prefix="$(build_m4ri)"

    local version llvm host
    version="$(rustc --version | sed 's/^rustc //')"
    host="$(rustc --version --verbose | sed -n 's/^host: //p')"
    llvm="$(rustc --version --verbose | sed -n 's/^LLVM version: //p')"
    {
        echo '# Dense-parity harness non-timed validation (jit:e1f9a78f)'
        echo "# command: ${invocation}"
        echo '# timing: none; the command builds release binaries and runs semantic validators only'
        echo "# rustc: ${version}; LLVM ${llvm}; ${host}"
        "${CAMPAIGN_TOOL}" pins | sed 's/^/# /'
        echo "PASS harness contract: ${passed} tests pass"
        "${ORACLE}"
        echo '# scalar-reference build: the same sources without the simd feature'
        "${SCALAR_ORACLE}"
        if [[ "${with_m4ri}" == 1 ]]; then
            echo "# m4ri prefix: ${prefix}"
            "${M4RI_ARM}" --backend | sed 's/^/# /'
            "${M4RI_ARM}" --oracle
            printf '# dense-m4ri-arm sha256: %s\n' \
                "$(sha256sum "${M4RI_ARM}" | cut -d' ' -f1)"
            printf '# libm4ri.so sha256: %s\n' \
                "$(sha256sum "${prefix}/lib/libm4ri.so" | cut -d' ' -f1)"
        fi
        local binary
        for binary in dense-arm dense-campaign dense-oracle; do
            printf '# %s sha256: %s\n' \
                "${binary}" "$(sha256sum "${GF2_TARGET}/release/${binary}" | cut -d' ' -f1)"
        done
        printf '# dense-arm (scalar-reference) sha256: %s\n' \
            "$(sha256sum "${SCALAR_ARM}" | cut -d' ' -f1)"
    } >"${VALIDATION}"
    cat "${VALIDATION}"
    echo "harness validation record: ${VALIDATION}" >&2
}

cmd_cells() {
    [[ -x "${CAMPAIGN_TOOL}" ]] || build_gf2
    "${CAMPAIGN_TOOL}" pins >/dev/null
    "${CAMPAIGN_TOOL}" cells "$@"
}

# Drives one family's stage to completion, one cell per session, and fails
# unless at least one session paused and the last one completed.
smoke_family() {
    local family="$1" stage="$2" plan="$3" addendum="$4"
    local session=0 status=0 paused=0
    echo "campaign execution log: ${stage}/execution.log" >&2
    while :; do
        session=$((session + 1))
        set +e
        "${CAMPAIGN_TOOL}" smoke --plan "${plan}" --addendum "${addendum}" --stage "${stage}"
        status=$?
        set -e
        [[ "${session}" == 1 ]] && cp "${stage}/execution.log" \
            "$(dirname "${stage}")/execution.session-1.log"
        case "${status}" in
            0) break ;;
            3) paused=$((paused + 1)) ;;
            *) echo "${family}: session ${session} exited ${status}" >&2; exit 2 ;;
        esac
    done
    (( paused > 0 )) || {
        echo "${family}: no session paused, so resume was not exercised" >&2
        exit 2
    }
}

cmd_smoke() {
    local with_m4ri=0 invocation
    # The record states the invocation that produced it, so the launcher's own
    # arguments are passed through rather than reconstructed.
    invocation="${LAUNCHER} smoke${*:+ $*}"
    [[ "${1:-}" == "--m4ri" ]] && with_m4ri=1
    build_gf2
    [[ "${with_m4ri}" == 1 ]] && build_m4ri >/dev/null
    local smoke
    smoke=target/e1f9a78f-campaigns/smoke
    rm -rf "${smoke}"
    mkdir -p "${smoke}"
    "${CAMPAIGN_TOOL}" pins

    # Semantics: the deterministic untimed oracle, whose cases the smoke record
    # counts, from both gf2 builds. It emits no timing sample.
    "${ORACLE}" >"${smoke}/oracle.txt"
    "${SCALAR_ORACLE}" >>"${smoke}/oracle.txt"
    if [[ "${with_m4ri}" == 1 ]]; then
        "${M4RI_ARM}" --oracle >>"${smoke}/oracle.txt"
    fi

    local families=(
        2037941f-dense-isolated-fused-parity
        2037941f-dense-allocated-matvec
    )
    [[ "${with_m4ri}" == 1 ]] && families+=(2037941f-dense-matvec-vs-m4ri)

    local family frozen
    frozen="$("${CAMPAIGN_TOOL}" pins | sed -n 's/^addendum_frozen_utc=//p')"
    for family in "${families[@]}"; do
        mkdir -p "${smoke}/${family}"
        # Cell generation is deterministic: two transcriptions of one family
        # are byte-identical, and each validates against the version-4 schema.
        "${CAMPAIGN_TOOL}" cells --family "${family}" --issue e1f9a78f \
            --frozen-utc "${frozen}" \
            --output "${smoke}/${family}/addendum.a.json" >/dev/null
        "${CAMPAIGN_TOOL}" cells --family "${family}" --issue e1f9a78f \
            --frozen-utc "${frozen}" \
            --output "${smoke}/${family}/addendum.b.json" >/dev/null
        cmp -s "${smoke}/${family}/addendum.a.json" "${smoke}/${family}/addendum.b.json"
        "${CAMPAIGN_TOOL}" verify --family "${family}" \
            --addendum "${smoke}/${family}/addendum.a.json" >/dev/null
    done
    # Every family transcribes, including one whose arms this smoke does not
    # drive, so cell generation is validated for the whole frozen addendum.
    "${CAMPAIGN_TOOL}" cells --family 2037941f-dense-matvec-vs-m4ri --issue e1f9a78f \
        --frozen-utc "${frozen}" --output "${smoke}/comparator-cells.json"
    "${CAMPAIGN_TOOL}" verify --family 2037941f-dense-matvec-vs-m4ri \
        --addendum "${smoke}/comparator-cells.json"

    # The throwaway family rewrites only the ledger path, so the smoke names no
    # committed ledger and the three family ledgers stay at genesis.
    python3 -B "${SURVEY}/make-dense-smoke-addenda.py" --stage "${smoke}" \
        --families "${families[@]}"

    # The smoke never takes the benchmark lock: it collects no timing sample,
    # so it neither needs a quiet host nor may pretend to have had one.
    for family in "${families[@]}"; do
        local stage="${smoke}/${family}/stage"
        local plan="${smoke}/${family}/plan.json"
        local addendum="${smoke}/${family}/smoke-addendum.json"
        local external=()
        [[ "${family}" == *matvec-vs-m4ri ]] && external=(--m4ri-executable "${M4RI_ARM}")
        "${CAMPAIGN_TOOL}" plan --family "${family}" \
            --addendum "${addendum}" \
            --campaign-id "e1f9a78f-${family}-wire-smoke" \
            --campaign-seed 20260917 \
            --label smoke \
            --lock "$(realpath "${smoke}/lock")" \
            --gf2-executable "${GF2_ARM}" \
            --scalar-executable "${SCALAR_ARM}" \
            "${external[@]}" \
            --producing-manifest "${PRODUCING}" \
            --max-cells-per-session 1 \
            --output "${plan}"
        smoke_family "${family}" "${stage}" "${plan}" "${addendum}"
    done

    local m4ri_flag=()
    [[ "${with_m4ri}" == 1 ]] && m4ri_flag=(--m4ri-arm "${M4RI_ARM}")
    python3 -B "${SURVEY}/check-dense-smoke.py" --stage "${smoke}" --record "${SMOKE_RECORD}" \
        --command "${invocation}" \
        --oracle "${smoke}/oracle.txt" \
        --gf2-arm "${GF2_ARM}" \
        --scalar-arm "${SCALAR_ARM}" \
        "${m4ri_flag[@]}" \
        --families "${families[@]}"
    cat "${SMOKE_RECORD}"
    echo "smoke record: ${SMOKE_RECORD}" >&2
}

cmd_window() {
    local family='' addendum='' run_id='' with_m4ri=0
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --family) family="$2"; shift 2 ;;
            --addendum) addendum="$2"; shift 2 ;;
            --run-id) run_id="$2"; shift 2 ;;
            --m4ri) with_m4ri=1; shift ;;
            *) echo "unknown window argument $1" >&2; exit 2 ;;
        esac
    done
    [[ -n "${family}" && -n "${addendum}" && -n "${run_id}" ]] || {
        echo 'window needs --family, --addendum and --run-id' >&2
        exit 2
    }
    [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
        echo 'timed dense-parity measurement runs only in the scheduled benchmark window' >&2
        exit 2
    }

    local ledger
    ledger="$("${CAMPAIGN_TOOL}" pins | sed -n "s/^family=${family} ledger=\\([^ ]*\\).*/\\1/p")"
    [[ -n "${ledger}" ]] || { echo "${family} is not a frozen family" >&2; exit 2; }

    # Refuse unfrozen inputs: the prose addendum matches the harness pin and
    # the campaign JSON is the harness transcription of it.
    "${CAMPAIGN_TOOL}" pins
    "${CAMPAIGN_TOOL}" verify --family "${family}" --addendum "${addendum}"

    # Every executable this run launches is rebuilt from the current tree
    # before the closure is checked, so no build can follow the check and no
    # prebuilt arm can carry bytes the check never saw.
    build_gf2
    local external=()
    if [[ "${with_m4ri}" == 1 ]]; then
        build_m4ri >/dev/null
        external=(--m4ri-executable "${M4RI_ARM}")
    fi
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-ab-runner --bin benchmark-acceptance

    # The closure is a snapshot of the tree it was enumerated from, so a source
    # added to a measured crate since the last regeneration is absent from it
    # and the guard below would never look at it.
    python3 -B "${SURVEY}/make-dense-producing-inputs.py" --check

    # The producing-input closure is the manifest every receipt snapshots:
    # harness sources, measured crate sources, campaign-support sources, every
    # Cargo manifest and lock the executables above are built from, the closure
    # manifest itself, the contract, the protocol and the frozen addendum. A
    # path the closure names that git does not track, or whose bytes differ
    # from the committed content, refuses the run. This is the last step before
    # the launch.
    "${CAMPAIGN_TOOL}" inputs --producing-manifest "${PRODUCING}" \
        --also "${addendum}" --also "${ledger}"

    local runner acceptance campaign stage plan out lock launch
    runner="$(realpath target/release/benchmark-ab-runner)"
    acceptance="$(realpath target/release/benchmark-acceptance)"
    campaign="${run_id}-${family}"
    stage="${REPO}/target/e1f9a78f-campaigns/${campaign}"
    plan="${stage}.plan.json"
    out="${REPO}/dev/bench_results/2037941f/${family}/${run_id}-pilot"
    lock="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
    mkdir -p "$(dirname "${stage}")" "$(dirname "${out}")"
    touch "${lock}"
    lock="$(realpath "${lock}")"

    "${CAMPAIGN_TOOL}" plan --family "${family}" --addendum "${addendum}" \
        --campaign-id "${campaign}" --campaign-seed 20260917 --label pilot \
        --lock "${lock}" --gf2-executable "${GF2_ARM}" \
        --scalar-executable "${SCALAR_ARM}" "${external[@]}" \
        --producing-manifest "${PRODUCING}" --max-cells-per-session 2 \
        --output "${plan}.projected"
    if [[ -e "${plan}" ]]; then
        cmp -s "${plan}.projected" "${plan}" || {
            echo "${plan} differs from the current projection; resume is refused" >&2
            exit 2
        }
        rm "${plan}.projected"
    else
        mv "${plan}.projected" "${plan}"
    fi

    launch="${stage}.launcher.log"
    {
        echo "# command: $0 $*"
        echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "# gf2 revision (informational): $(git rev-parse HEAD)"
        "${CAMPAIGN_TOOL}" pins | sed 's/^/# /'
        echo "# campaign addendum: ${addendum} sha256=$(sha256sum "${addendum}" | cut -d' ' -f1)"
        echo "# producing manifest: ${PRODUCING} sha256=$(sha256sum "${PRODUCING}" | cut -d' ' -f1)"
        echo "# gf2 arm: ${GF2_ARM} sha256=$(sha256sum "${GF2_ARM}" | cut -d' ' -f1)"
        echo "# scalar arm: ${SCALAR_ARM} sha256=$(sha256sum "${SCALAR_ARM}" | cut -d' ' -f1)"
        [[ "${with_m4ri}" == 1 ]] &&
            echo "# m4ri arm: ${M4RI_ARM} sha256=$(sha256sum "${M4RI_ARM}" | cut -d' ' -f1)"
        echo "# plan: ${plan} sha256=$(sha256sum "${plan}" | cut -d' ' -f1)"
        echo "# toolchain: $(rustc --version --verbose | tr '\n' ';')"
    } >>"${launch}"
    echo "campaign execution log: ${stage}/execution.log" >&2

    stage_complete() {
        [[ -f "${stage}/execution.log" ]] && python3 - "${stage}/execution.log" <<'PY'
import json, sys
events = [json.loads(line)["event"] for line in open(sys.argv[1])]
terminal = [e for e in events if e in ("complete", "failed", "paused", "budget-exhausted")]
raise SystemExit(0 if terminal and terminal[-1] == "complete" else 1)
PY
    }

    local session=0 rc
    while ! stage_complete; do
        session=$((session + 1))
        set +e
        GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
            "${runner}" run "${stage}" "${plan}" | tee -a "${launch}"
        rc=${PIPESTATUS[0]}
        set -e
        echo "# session ${session} exit: ${rc}" >>"${launch}"
        case "${rc}" in
            0) break ;;
            3) ;;
            *) echo "session ${session} failed with ${rc}" >&2; exit "${rc}" ;;
        esac
    done

    "${runner}" finalize "${stage}" "${out}" | tee -a "${launch}"
    cp "${launch}" "${out}/launcher.log"
    set +e
    "${acceptance}" "${out}" | tee -a "${out}/launcher.log"
    local verdict=${PIPESTATUS[0]}
    set -e
    echo "# acceptance exit: ${verdict}" >>"${out}/launcher.log"
    echo "exploratory receipt: ${out}" >&2
    exit "${verdict}"
}

case "${1:-}" in
    build) shift; cmd_build "$@" ;;
    cells) shift; cmd_cells "$@" ;;
    smoke) shift; cmd_smoke "$@" ;;
    window) shift; cmd_window "$@" ;;
    *) sed -n '2,13p' "${BASH_SOURCE[0]}" >&2; exit 2 ;;
esac
