#!/usr/bin/env bash
# Launcher for the logical-buffer measurement harness (jit:bb769456).
#
# Subcommands:
#   build [--isal]                     build the arms, run the crate contract and the oracle
#   cells --family <id> --issue <hex8> --frozen-utc <t> --output <path>
#                                      transcribe one family into a campaign addendum
#   smoke [--isal]                     deterministic untimed release smoke (no receipt sample)
#   window --family <id> --addendum <path> --run-id <id> [--isal]
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
MANIFEST="${REPO}/${SURVEY}/harness/Cargo.toml"
PRODUCING="${SURVEY}/logical-producing-inputs.json"
VALIDATION="${SURVEY}/logical-harness-validation.txt"
SMOKE_RECORD="${SURVEY}/logical-runner-smoke.txt"
LAUNCHER="${SURVEY}/run-logical-harness.sh"
GF2_TARGET="${REPO}/target/bb769456-arms"
ISAL_TARGET="${REPO}/target/bb769456-isal-arm"
GF2_ARM="${GF2_TARGET}/release/logical-arm"
ISAL_ARM="${ISAL_TARGET}/release/logical-isal-arm"
CAMPAIGN_TOOL="${GF2_TARGET}/release/logical-campaign"
ORACLE="${GF2_TARGET}/release/logical-oracle"
ISAL_PIN=7c3479e0a9dac17f448603ec1ad64c7c625f530c

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

# The pinned ISA-L checkout is shared under the primary checkout's .agents/ext;
# it is discovered, never rebuilt destructively.
isal_source() {
    if [[ -n "${GF2_ISAL_SOURCE:-}" ]]; then
        printf '%s\n' "${GF2_ISAL_SOURCE}"
        return
    fi
    local common primary candidate
    common="$(git -C "${REPO}" rev-parse --path-format=absolute --git-common-dir)"
    primary="$(dirname "${common}")"
    for candidate in "${primary}"/.agents/ext/*/isa-l; do
        if [[ -d "${candidate}/.git" ]] \
            && [[ "$(git -C "${candidate}" rev-parse HEAD 2>/dev/null)" == "${ISAL_PIN}" ]]; then
            printf '%s\n' "${candidate}"
            return
        fi
    done
    echo "no ISA-L checkout at ${ISAL_PIN} under ${primary}/.agents/ext; see ${STORY}/isal-comparator.md" >&2
    exit 2
}

build_gf2() {
    CARGO_TARGET_DIR="${GF2_TARGET}" \
        ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}" --bins
}

build_isal() {
    local source
    source="$(isal_source)"
    GF2_ISAL_SOURCE="${source}" CARGO_TARGET_DIR="${ISAL_TARGET}" \
        ./scripts/cargo-budget.sh cargo build --release --features isal \
        --manifest-path "${MANIFEST}" --bin logical-isal-arm
    printf '%s\n' "${source}"
}

cmd_build() {
    local with_isal=0 source=''
    [[ "${1:-}" == "--isal" ]] && with_isal=1
    build_gf2
    # The survey harness is its own workspace, so the repository CI contract
    # never reaches it: this command is the only thing that runs its contract.
    local contract passed
    contract="$(CARGO_TARGET_DIR="${GF2_TARGET}" ./scripts/cargo-budget.sh --test \
        cargo test --release --manifest-path "${MANIFEST}" --tests 2>&1)"
    echo "${contract}"
    passed="$(printf '%s\n' "${contract}" |
        awk '/^test result: ok\./ { total += $4 } END { print total + 0 }')"
    [[ "${with_isal}" == 1 ]] && source="$(build_isal)"

    local version llvm host
    version="$(rustc --version | sed 's/^rustc //')"
    host="$(rustc --version --verbose | sed -n 's/^host: //p')"
    llvm="$(rustc --version --verbose | sed -n 's/^LLVM version: //p')"
    {
        echo '# Logical-buffer harness non-timed validation (jit:bb769456)'
        echo "# command: ${LAUNCHER} build$([[ ${with_isal} == 1 ]] && echo ' --isal')"
        echo '# timing: none; the command builds release binaries and runs semantic validators only'
        echo "# rustc: ${version}; LLVM ${llvm}; ${host}"
        "${CAMPAIGN_TOOL}" pins | sed 's/^/# /'
        echo "PASS harness contract: ${passed} tests pass"
        "${ORACLE}"
        if [[ "${with_isal}" == 1 ]]; then
            echo "# isa-l: ${ISAL_PIN}"
            "${ISAL_ARM}" --backend | sed 's/^/# /'
            "${ISAL_ARM}" --oracle
            printf '# logical-isal-arm sha256: %s\n' \
                "$(sha256sum "${ISAL_ARM}" | cut -d' ' -f1)"
        fi
        local binary
        for binary in logical-arm logical-campaign logical-oracle; do
            printf '# %s sha256: %s\n' \
                "${binary}" "$(sha256sum "${GF2_TARGET}/release/${binary}" | cut -d' ' -f1)"
        done
    } >"${VALIDATION}"
    cat "${VALIDATION}"
    echo "harness validation record: ${VALIDATION}" >&2
}

cmd_cells() {
    [[ -x "${CAMPAIGN_TOOL}" ]] || build_gf2
    "${CAMPAIGN_TOOL}" pins >/dev/null
    "${CAMPAIGN_TOOL}" cells "$@"
}

cmd_smoke() {
    local with_isal=0 invocation
    # The record states the invocation that produced it, so the launcher's own
    # arguments are passed through rather than reconstructed.
    invocation="${LAUNCHER} smoke${*:+ $*}"
    [[ "${1:-}" == "--isal" ]] && with_isal=1
    build_gf2
    [[ "${with_isal}" == 1 ]] && build_isal >/dev/null
    local smoke
    smoke=target/bb769456-campaigns/smoke
    rm -rf "${smoke}"
    mkdir -p "${smoke}"
    "${CAMPAIGN_TOOL}" pins

    # Semantics: the deterministic untimed oracle, whose cases the smoke record
    # counts. It emits no timing sample.
    "${ORACLE}" >"${smoke}/oracle.txt"
    if [[ "${with_isal}" == 1 ]]; then
        "${ISAL_ARM}" --oracle >>"${smoke}/oracle.txt"
    fi

    local families=(
        2037941f-logical-isolated-xor
        2037941f-logical-public-row-xor
        2037941f-logical-nr-construction
    )
    [[ "${with_isal}" == 1 ]] && families+=(2037941f-logical-isal-base-gap)

    local family
    for family in "${families[@]}"; do
        mkdir -p "${smoke}/${family}"
        # Cell generation is deterministic: two transcriptions of one family
        # are byte-identical, and each validates against the version-4 schema.
        "${CAMPAIGN_TOOL}" cells --family "${family}" --issue bb769456 \
            --frozen-utc "$("${CAMPAIGN_TOOL}" pins |
                sed -n 's/^addendum_frozen_utc=//p')" \
            --output "${smoke}/${family}/addendum.a.json" >/dev/null
        "${CAMPAIGN_TOOL}" cells --family "${family}" --issue bb769456 \
            --frozen-utc "$("${CAMPAIGN_TOOL}" pins |
                sed -n 's/^addendum_frozen_utc=//p')" \
            --output "${smoke}/${family}/addendum.b.json" >/dev/null
        cmp -s "${smoke}/${family}/addendum.a.json" "${smoke}/${family}/addendum.b.json"
        "${CAMPAIGN_TOOL}" verify --family "${family}" \
            --addendum "${smoke}/${family}/addendum.a.json" >/dev/null
    done

    # The throwaway family rewrites only the ledger path, so the smoke names no
    # committed ledger and the four family ledgers stay at genesis.
    python3 -B "${SURVEY}/make-smoke-addenda.py" --stage "${smoke}" \
        --families "${families[@]}"

    # The smoke never takes the benchmark lock: it collects no timing sample,
    # so it neither needs a quiet host nor may pretend to have had one.
    for family in "${families[@]}"; do
        local stage="${smoke}/${family}/stage"
        local plan="${smoke}/${family}/plan.json"
        local addendum="${smoke}/${family}/smoke-addendum.json"
        local isal_flag=()
        [[ "${family}" == *isal-base-gap ]] && isal_flag=(--isal-executable "${ISAL_ARM}")
        "${CAMPAIGN_TOOL}" plan --family "${family}" \
            --addendum "${addendum}" \
            --campaign-id "bb769456-${family}-wire-smoke" \
            --campaign-seed 20260916 \
            --label smoke \
            --lock "$(realpath "${smoke}/lock")" \
            --gf2-executable "${GF2_ARM}" \
            "${isal_flag[@]}" \
            --producing-manifest "${PRODUCING}" \
            --max-cells-per-session 1 \
            --output "${plan}"
        echo "campaign execution log: ${stage}/execution.log" >&2
        # Session one pauses after its first cell; session two completes the
        # stage from the checkpoint without repeating it.
        set +e
        "${CAMPAIGN_TOOL}" smoke --plan "${plan}" --addendum "${addendum}" --stage "${stage}"
        local first=$?
        cp "${stage}/execution.log" "${smoke}/${family}/execution.session-1.log"
        "${CAMPAIGN_TOOL}" smoke --plan "${plan}" --addendum "${addendum}" --stage "${stage}"
        local second=$?
        set -e
        [[ "${first}" == 3 ]] || {
            echo "${family}: the first session exited ${first} rather than pausing" >&2
            exit 2
        }
        [[ "${second}" == 0 ]] || {
            echo "${family}: the resumed session exited ${second}" >&2
            exit 2
        }
    done

    python3 -B "${SURVEY}/check-smoke.py" --stage "${smoke}" --record "${SMOKE_RECORD}" \
        --command "${invocation}" \
        --oracle "${smoke}/oracle.txt" \
        --gf2-arm "${GF2_ARM}" \
        $([[ "${with_isal}" == 1 ]] && echo "--isal-arm ${ISAL_ARM}") \
        --families "${families[@]}"
    cat "${SMOKE_RECORD}"
    echo "smoke record: ${SMOKE_RECORD}" >&2
}

cmd_window() {
    local family='' addendum='' run_id='' with_isal=0
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --family) family="$2"; shift 2 ;;
            --addendum) addendum="$2"; shift 2 ;;
            --run-id) run_id="$2"; shift 2 ;;
            --isal) with_isal=1; shift ;;
            *) echo "unknown window argument $1" >&2; exit 2 ;;
        esac
    done
    [[ -n "${family}" && -n "${addendum}" && -n "${run_id}" ]] || {
        echo 'window needs --family, --addendum and --run-id' >&2
        exit 2
    }
    [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
        echo 'timed logical-buffer measurement runs only in the scheduled benchmark window' >&2
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
    local isal_flag=()
    if [[ "${with_isal}" == 1 ]]; then
        build_isal >/dev/null
        isal_flag=(--isal-executable "${ISAL_ARM}")
    fi
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-ab-runner --bin benchmark-acceptance

    # The producing-input closure is the manifest every receipt snapshots:
    # harness sources, measured crate sources, campaign-support sources, the
    # contract, the protocol and the frozen addendum. A path the closure names
    # that git does not track, or whose bytes differ from the committed
    # content, refuses the run. This is the last step before the launch.
    "${CAMPAIGN_TOOL}" inputs --producing-manifest "${PRODUCING}" \
        --also "${addendum}" --also "${ledger}"

    local runner acceptance campaign stage plan out lock launch
    runner="$(realpath target/release/benchmark-ab-runner)"
    acceptance="$(realpath target/release/benchmark-acceptance)"
    campaign="${run_id}-${family}"
    stage="${REPO}/target/bb769456-campaigns/${campaign}"
    plan="${stage}.plan.json"
    out="${REPO}/dev/bench_results/2037941f/${family}/${run_id}-pilot"
    lock="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
    mkdir -p "$(dirname "${stage}")" "$(dirname "${out}")"
    touch "${lock}"
    lock="$(realpath "${lock}")"

    "${CAMPAIGN_TOOL}" plan --family "${family}" --addendum "${addendum}" \
        --campaign-id "${campaign}" --campaign-seed 20260916 --label pilot \
        --lock "${lock}" --gf2-executable "${GF2_ARM}" "${isal_flag[@]}" \
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
        [[ "${with_isal}" == 1 ]] &&
            echo "# isal arm: ${ISAL_ARM} sha256=$(sha256sum "${ISAL_ARM}" | cut -d' ' -f1)"
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
