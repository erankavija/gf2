#!/usr/bin/env bash
# permanent-campaign-runner.sh — deterministic overnight receipt campaigns.
#
# One-shot scheduling (2026-08-14 02:00, canonical repository):
#   systemd-run --user --on-calendar='2026-08-14 02:00' --unit=gf2-permanent-campaign.timer /bin/bash -lc '/home/vkaskivuo/Projects/gf2/dev/scripts/permanent-campaign-runner.sh measure >> /home/vkaskivuo/Projects/gf2/target/permanent-campaign/systemd-measure.log 2>&1'
#   systemctl --user list-timers 'gf2-permanent-campaign*'
#   systemctl --user status gf2-permanent-campaign.timer
#   systemctl --user stop gf2-permanent-campaign.timer
#   systemctl --user stop gf2-permanent-campaign.service  # stop an already-running instance
#
# at(1) alternative:
#   echo '/home/vkaskivuo/Projects/gf2/dev/scripts/permanent-campaign-runner.sh measure >> /home/vkaskivuo/Projects/gf2/target/permanent-campaign/at-measure.log 2>&1' | at 02:00 2026-08-14
#
# The outer logs live under target/, which prepare creates and git ignores, so
# their creation cannot make the measure pre-flight reject its own run. The
# per-step logs and CSV outputs are created under dev/studies/ only after the
# same pre-flight check has passed.

set -euo pipefail

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"
# Tests may point repository checks at a clean fixture while executing this
# working copy of the runner. Production uses the script's repository root.
REPO_ROOT="${CAMPAIGN_REPO_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"

SAMPLING_MANIFEST="$REPO_ROOT/dev/research/permanent-sampling-feas/Cargo.toml"
WAVE_MANIFEST="$REPO_ROOT/dev/research/permanent_wave_gpu/Cargo.toml"
TARGET_ROOT="${CAMPAIGN_TARGET_ROOT:-$REPO_ROOT/target/permanent-campaign}"
SAMPLING_TARGET_DIR="${CAMPAIGN_SAMPLING_TARGET_DIR:-$TARGET_ROOT/permanent-sampling-feas-hip}"
WAVE_TARGET_DIR="${CAMPAIGN_WAVE_TARGET_DIR:-$TARGET_ROOT/permanent-wave-gpu-hip}"
MANIFEST_PATH="${CAMPAIGN_MANIFEST:-$TARGET_ROOT/manifest-v1.txt}"
HARNESS_BIN="${CAMPAIGN_HARNESS_BIN:-$SAMPLING_TARGET_DIR/release/permanent_sampling_feas}"
FLOCK_WRAPPER="${CAMPAIGN_FLOCK_WRAPPER:-$SCRIPT_DIR/ccx1-bench-flock.sh}"
STUDY_ROOT="${CAMPAIGN_STUDY_ROOT:-$REPO_ROOT/dev/studies}"
CAMPAIGN_SIM_BINARY="${CAMPAIGN_SIM_BINARY:-}"
CAMPAIGN_SIM_MANIFEST="${CAMPAIGN_SIM_MANIFEST:-}"
CAMPAIGN_SIM_OUTPUT="${CAMPAIGN_SIM_OUTPUT:-}"
CAMPAIGN_SIM_FIELD="${CAMPAIGN_SIM_FIELD:-}"
CAMPAIGN_SIM_WORKERS="${CAMPAIGN_SIM_WORKERS:-1}"
CAMPAIGN_SIM_ACCELERATOR_COSTS="${CAMPAIGN_SIM_ACCELERATOR_COSTS:-}"
CAMPAIGN_SIM_ACCELERATOR_CAP_MS="${CAMPAIGN_SIM_ACCELERATOR_CAP_MS:-}"
ROCM_PATH="${ROCM_PATH:-/opt/rocm}"
ARCH="${PERMANENT_CAMPAIGN_ARCH:-gfx1030}"
if [[ "${1:-}" == premeasure || "${1:-}" == premeasure-collect || "${1:-}" == __locked-premeasure ]]; then
    RUN_ID="${CAMPAIGN_RUN_ID:-premeasure-v1}"
else
    RUN_ID="${CAMPAIGN_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
fi
PREMEASURE_PLAN="${CAMPAIGN_PREMEASURE_PLAN:-$REPO_ROOT/dev/benchmarks/permanent_campaign/premeasure-plan-v1.csv}"
PREMEASURE_DEFAULT_SESSION_CAP=43200

# The production permanent kernels the campaign measures, listed as the
# translation units crates/gf2-kernels-hip/build.rs hands to hipcc. build.rs
# owns that set, so the receipt sweep enumerates it instead of globbing the
# directory; assert_crate_permanent_inventory fails the run when the two drift.
CRATE_PERMANENT_DIR="$REPO_ROOT/crates/gf2-kernels-hip/hip/permanent"
CRATE_BUILD_RS="$REPO_ROOT/crates/gf2-kernels-hip/build.rs"
CRATE_PERMANENT_TU_ROOTS=(
    gray_update_micro.hip
    permanent_bipedal3.hip
    permanent_bipedal5.hip
    permanent_bipedal7.hip
)
# `<fragment>|<translation unit root that includes it>`. A fragment has no
# translation unit of its own: permanent_bipedal7.hip includes
# horizontal_product_micro.hip textually so the F_7 lookup circuit reads that
# unit's established __constant__ d_MUL_LUT, and compiling the fragment alone
# fails on that undeclared symbol. hipcc therefore never sees it as a source,
# in the crate build or here, and its kernels' resource remarks appear in the
# permanent_bipedal7.hip log its receipt entry cites.
CRATE_PERMANENT_FRAGMENTS=(
    "horizontal_product_micro.hip|permanent_bipedal7.hip"
)

# Recorded in place of an execution id for the steps that have none. The
# harness parses --execution-id under grid alone (src/usage.txt; GridOptions is
# the only parser): equivalence, gray-update, and horizontal-product draw from
# fixed stream addresses of the form (seed_root, purpose, index) that their own
# CSV preambles record. A numeric id on those lines would assert a per-execution
# stream block the harness never reserved, so their summary lines name the fixed
# addressing instead and the runner passes them neither --execution-id nor
# --skip-machine-warmup.
FIXED_STREAM_EXECUTION_ID=fixed-streams

# Exit 2 means a runner/preparation refusal or infrastructure error. Exit 7
# means the campaign ran to completion but at least one harness step failed.
CENSORED_EXIT=7

usage() {
    cat <<'USAGE'
Usage: permanent-campaign-runner.sh <prepare|smoke|measure|premeasure|premeasure-collect>

Subcommands:
  prepare  Build both HIP harnesses, capture kernel resource receipts, and
           write the binary-hash manifest used by measure and smoke.
  smoke    Run the complete four-step pipeline for q=3,5,7 with tiny inputs;
           outputs are isolated below each study's smoke/ directory.
  measure  Verify prepared binary hashes, then run the overnight receipt
           campaign while holding the canonical full-host
           benchmark mutex for the entire run.
  premeasure [--session-cap SECONDS]
           Verify prepared binary hashes and run the resumable 60-cell
           premeasurement schedule under one canonical full-host lock. The
           default session cap is 43200 seconds.
  premeasure-collect
           Collect completed premeasurement process receipts and report
           per-configuration completeness; no pooled means are calculated.
USAGE
}

die() {
    echo "ERROR: $*" >&2
    exit 2
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

hash_file() {
    sha256sum "$1" | awk '{print $1}'
}

tracked_worktree_clean() {
    git -C "$REPO_ROOT" diff --quiet -- crates/ Cargo.lock \
        && git -C "$REPO_ROOT" diff --cached --quiet -- crates/ Cargo.lock \
        && [[ -z "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all -- crates/ Cargo.lock)" ]]
}

source_closure_revision() {
    git -C "$REPO_ROOT" log -1 --format=%H -- crates/ Cargo.lock
}

assert_tracked_worktree_clean() {
    if ! tracked_worktree_clean; then
        echo "ERROR: prepare requires a clean source closure (crates/ and Cargo.lock; tracked and untracked)" >&2
        git -C "$REPO_ROOT" status --short --untracked-files=all -- crates/ Cargo.lock >&2
        exit 2
    fi
}

capture_block() {
    local title="$1"
    shift
    printf '%s\n' "$title"
    if "$@" 2>&1; then
        :
    else
        printf 'command_exit_status: %s\n' "$?"
    fi
}

study_dir_for_q() {
    case "$1" in
        3) printf '%s\n' "$STUDY_ROOT/047b62ed" ;;
        5) printf '%s\n' "$STUDY_ROOT/91605d4d" ;;
        7) printf '%s\n' "$STUDY_ROOT/6c7fcb38" ;;
        *) die "unsupported field q=$1" ;;
    esac
}

print_manifest_binary_hashes() {
    local kind path expected
    while IFS='|' read -r kind path expected; do
        [[ "$kind" == "binary" ]] || continue
        [[ -n "$path" ]] || continue
        if [[ ! -f "$path" ]]; then
            printf 'binary_missing: %s\n' "$path"
            continue
        fi
        printf 'binary_hash: %s %s\n' "$path" "$(hash_file "$path")"
        printf 'binary_manifest_hash: %s %s\n' "$path" "$expected"
    done < "$MANIFEST_PATH"
}

verify_manifest() {
    [[ -f "$MANIFEST_PATH" ]] || die "binary manifest not found; run prepare first: $MANIFEST_PATH"
    local kind path expected actual
    HARNESS_BIN=""
    while IFS='|' read -r kind path expected; do
        case "$kind" in
            harness) HARNESS_BIN="$path" ;;
            binary)
                [[ -n "$path" && -n "$expected" ]] || die "malformed binary manifest line"
                [[ -x "$path" ]] || die "manifest binary is not executable: $path"
                actual=$(hash_file "$path")
                [[ "$actual" == "$expected" ]] || die "binary hash mismatch: $path (manifest $expected, actual $actual)"
                ;;
            ''|manifest_version=*|repository_revision=*|source_closure_revision=*|source_closure_dirty=*|source_closure_dirty_check=*|resource_receipt=*|rust_toolchain=*|build_rustc=*) ;;
            *) die "unknown manifest line: $kind|$path|$expected" ;;
        esac
    done < "$MANIFEST_PATH"
    [[ -n "$HARNESS_BIN" ]] || die "manifest does not name a harness binary"
    [[ -x "$HARNESS_BIN" ]] || die "manifest harness is not executable: $HARNESS_BIN"
}

manifest_value() {
    local key="$1"
    sed -n "s/^${key}=//p" "$MANIFEST_PATH" | head -n 1
}

write_provenance() {
    local provenance="$1"
    local study_dir="$2"
    mkdir -p "$study_dir"
    {
        echo "schema_version: 1"
        echo "campaign_run_id: $RUN_ID"
        echo "repository_revision: $(git -C "$REPO_ROOT" rev-parse --verify HEAD)"
        echo "source_closure_revision: $(source_closure_revision)"
        if tracked_worktree_clean; then echo "source_closure_dirty: false"; else echo "source_closure_dirty: true"; fi
        echo "source_closure_dirty_check: git diff --quiet -- crates/ Cargo.lock && git diff --cached --quiet -- crates/ Cargo.lock && git status --porcelain --untracked-files=all -- crates/ Cargo.lock"
        echo "rust_toolchain: $(manifest_value rust_toolchain)"
        echo "build_rustc: $(manifest_value build_rustc)"
        echo "binary_hashes: see $MANIFEST_PATH and the harness CSV preambles"
        print_manifest_binary_hashes
        echo
        echo "# The harness CSV preamble is the canonical source for facts it embeds."
        echo "# This block records command outputs rather than a second hand-written inventory."
        capture_block "cpu_model_command: lscpu" lscpu
        capture_block "gpu_model_uuid_command: $ROCM_PATH/bin/rocm-smi --showproductname --showuniqueid" "$ROCM_PATH/bin/rocm-smi" --showproductname --showuniqueid
        capture_block "rocm_hipcc_version_command: $ROCM_PATH/bin/hipcc --version" "$ROCM_PATH/bin/hipcc" --version
        capture_block "amd_clang_version_command: $ROCM_PATH/llvm/bin/clang --version" "$ROCM_PATH/llvm/bin/clang" --version
        capture_block "ambient_rustc_command: rustc -V" rustc -V
        capture_block "kernel_version_command: uname -r" uname -r
        echo
        echo "harness_csv_preamble_reference: each CSV under $study_dir has the authoritative # preamble"
        if [[ "${CAMPAIGN_MODE:-measure}" == "smoke" ]]; then
            echo "contention_caveat: smoke ran while other workers may have compiled or used shared resources; timing values are plumbing evidence, not campaign evidence."
        fi
        echo
        echo "exact_commands_executed:"
    } > "$provenance"
}

record_command() {
    local provenance="$1"
    shift
    local rendered
    printf -v rendered '%q ' "$@"
    printf 'command: %s\n' "${rendered% }" >> "$provenance"
}

crate_permanent_source_is_listed() {
    local name="$1" candidate entry
    for candidate in "${CRATE_PERMANENT_TU_ROOTS[@]}"; do
        if [[ "$candidate" == "$name" ]]; then return 0; fi
    done
    for entry in "${CRATE_PERMANENT_FRAGMENTS[@]}"; do
        if [[ "${entry%%|*}" == "$name" ]]; then return 0; fi
    done
    return 1
}

# Refuses to capture receipts once the crate's translation unit set no longer
# matches the lists above, so a renamed, added, or newly self-contained kernel
# source stops the campaign instead of losing its receipt unnoticed.
assert_crate_permanent_inventory() {
    local listed compiled entry fragment root path name
    listed=$(printf '%s\n' "${CRATE_PERMANENT_TU_ROOTS[@]}" | sort)
    compiled=$(sed -n 's/.*\.file("hip\/permanent\/\([A-Za-z0-9_]*\.hip\)").*/\1/p' "$CRATE_BUILD_RS" | sort)
    if [[ "$listed" != "$compiled" ]]; then
        die "gf2-kernels-hip translation unit drift: build.rs compiles [$(tr '\n' ' ' <<< "$compiled")], this runner captures [$(tr '\n' ' ' <<< "$listed")]"
    fi
    for entry in "${CRATE_PERMANENT_FRAGMENTS[@]}"; do
        fragment="${entry%%|*}"
        root="${entry##*|}"
        [[ -f "$CRATE_PERMANENT_DIR/$fragment" ]] || die "listed fragment is missing: $CRATE_PERMANENT_DIR/$fragment"
        grep -qF "#include \"$fragment\"" "$CRATE_PERMANENT_DIR/$root" \
            || die "$root no longer includes $fragment; its kernels are not in that translation unit's resource log"
    done
    for path in "$CRATE_PERMANENT_DIR"/*.hip; do
        [[ -f "$path" ]] || die "no HIP sources under $CRATE_PERMANENT_DIR"
        name="${path##*/}"
        crate_permanent_source_is_listed "$name" \
            || die "unswept kernel source $path: list it as a translation unit root or as a fragment of one"
    done
}

# Compiles $source as its own translation unit under the resource-usage remark
# flag, with the extra hipcc flags given after it, and appends its receipt
# entry. Leaves the captured artifacts in the CAPTURED_* globals so a fragment
# of this unit can cite the same object and log.
capture_translation_unit() {
    local resource_root="$1" source="$2"
    shift 2
    local object log status source_hash rendered
    local -a command
    object="$resource_root/${source##*/}.o"
    log="$resource_root/${source##*/}.resource.log"
    if [[ -e "$object" ]]; then
        die "resource capture would overwrite $object: two measured sources share a basename"
    fi
    command=("$ROCM_PATH/bin/hipcc" "--offload-arch=$ARCH" "$@" -Rpass-analysis=kernel-resource-usage -c "$source" -o "$object")
    echo "capturing resource usage: $source"
    set +e
    "${command[@]}" 2> "$log"
    status=$?
    set -e
    [[ "$status" -eq 0 ]] || die "resource capture failed for $source (exit $status)"
    source_hash=$(hash_file "$source")
    printf -v rendered '%q ' "${command[@]}"
    CAPTURED_COMMAND="${rendered% }"
    CAPTURED_STATUS="$status"
    CAPTURED_OBJECT="$object"
    CAPTURED_OBJECT_SHA256="$(hash_file "$object")"
    CAPTURED_LOG="$log"
    CAPTURED_LOG_SHA256="$(hash_file "$log")"
    append_receipt_entry "$resource_root" "$source" "$source_hash" "$source" root
}

# One per-source receipt entry. `source` is the measured kernel source;
# `translation_unit` is the source hipcc compiled, which is the measured source
# itself for a root and the including root for a fragment.
append_receipt_entry() {
    local resource_root="$1" source="$2" source_hash="$3" translation_unit="$4" role="$5"
    {
        printf 'source: %s\nsource_sha256: %s\n' "$source" "$source_hash"
        printf 'command: %s\n' "$CAPTURED_COMMAND"
        printf 'exit_status: %s\nobject: %s\nobject_sha256: %s\nresource_log: %s\nresource_log_sha256: %s\n' \
            "$CAPTURED_STATUS" "$CAPTURED_OBJECT" "$CAPTURED_OBJECT_SHA256" "$CAPTURED_LOG" "$CAPTURED_LOG_SHA256"
        printf 'translation_unit: %s\ntranslation_unit_role: %s\n\n' "$translation_unit" "$role"
    } >> "$resource_root/receipt.txt"
}

# Every kernel the campaign measures: the prototype candidates from
# permanent_wave_gpu, and the production permanent and micro-measurement
# kernels the prepared harness launches from gf2-kernels-hip. The crate's
# unrelated coding and modem kernels are deliberately outside this sweep.
capture_resource_receipts() {
    local resource_root="$1"
    local source root entry fragment
    # Each wave source is a self-contained translation unit, compiled here with
    # the flags permanent_wave_gpu's build script uses for it.
    while IFS= read -r source; do
        capture_translation_unit "$resource_root" "$source" -O3
    done < <(find "$REPO_ROOT/dev/research/permanent_wave_gpu/hip" -type f -name '*.hip' -print | sort)
    # -O3 -fPIC are the flags build.rs hands hipcc for the crate sources, so
    # these receipts describe the kernels as the crate actually builds them.
    assert_crate_permanent_inventory
    for root in "${CRATE_PERMANENT_TU_ROOTS[@]}"; do
        capture_translation_unit "$resource_root" "$CRATE_PERMANENT_DIR/$root" -O3 -fPIC
        for entry in "${CRATE_PERMANENT_FRAGMENTS[@]}"; do
            [[ "${entry##*|}" == "$root" ]] || continue
            fragment="${entry%%|*}"
            echo "capturing resource usage: $CRATE_PERMANENT_DIR/$fragment (no standalone translation unit; captured from $root)"
            append_receipt_entry "$resource_root" "$CRATE_PERMANENT_DIR/$fragment" \
                "$(hash_file "$CRATE_PERMANENT_DIR/$fragment")" "$CRATE_PERMANENT_DIR/$root" included-fragment
        done
    done
}

prepare() {
    require_command cargo
    require_command hipcc
    require_command sha256sum
    require_command git
    require_command rustc
    assert_tracked_worktree_clean
    mkdir -p "$SAMPLING_TARGET_DIR" "$WAVE_TARGET_DIR" "$TARGET_ROOT"
    echo "building permanent-sampling-feas (HIP)"
    cargo +1.95.0 build --manifest-path "$SAMPLING_MANIFEST" --release --features hip --target-dir "$SAMPLING_TARGET_DIR"
    echo "building permanent_wave_gpu (HIP)"
    cargo +1.95.0 build --manifest-path "$WAVE_MANIFEST" --release --features hip --target-dir "$WAVE_TARGET_DIR"

    local resource_root="$TARGET_ROOT/hip-resource-usage-$RUN_ID"
    mkdir -p "$resource_root"
    {
        echo "schema_version: 1"
        echo "repository_revision: $(git -C "$REPO_ROOT" rev-parse --verify HEAD)"
        echo "source_closure_revision=$(source_closure_revision)"
        echo "source_closure_dirty=$(if tracked_worktree_clean; then echo false; else echo true; fi)"
        echo "source_closure_dirty_check=crates/ Cargo.lock"
        echo "architecture: $ARCH"
        echo "hipcc: $ROCM_PATH/bin/hipcc"
        echo "resource_flag: -Rpass-analysis=kernel-resource-usage"
    } > "$resource_root/receipt.txt"
    capture_resource_receipts "$resource_root"

    local binaries=(
        "$SAMPLING_TARGET_DIR/release/permanent_sampling_feas"
        "$WAVE_TARGET_DIR/release/wave-gf3-device-evidence"
        "$WAVE_TARGET_DIR/release/f5-wave-device-evidence"
        "$WAVE_TARGET_DIR/release/wave-gf7-device-evidence"
    )
    local binary
    local manifest_tmp="$MANIFEST_PATH.tmp.$$"
    {
        echo "manifest_version=1"
        echo "repository_revision=$(git -C "$REPO_ROOT" rev-parse --verify HEAD)"
        echo "source_closure_revision=$(source_closure_revision)"
        echo "source_closure_dirty=$(if tracked_worktree_clean; then echo false; else echo true; fi)"
        echo "source_closure_dirty_check=crates/ Cargo.lock"
        echo "rust_toolchain=1.95.0"
        echo "build_rustc=$(rustc +1.95.0 -V)"
        echo "resource_receipt=$resource_root/receipt.txt"
        echo "harness|$SAMPLING_TARGET_DIR/release/permanent_sampling_feas|"
        for binary in "${binaries[@]}"; do
            [[ -x "$binary" ]] || die "expected prepared binary missing: $binary"
            printf 'binary|%s|%s\n' "$binary" "$(hash_file "$binary")"
        done
    } > "$manifest_tmp"
    mv "$manifest_tmp" "$MANIFEST_PATH"
    echo "prepared manifest: $MANIFEST_PATH"
    grep -E '^(manifest_version|repository_revision|source_closure_revision|source_closure_dirty|rust_toolchain|build_rustc|resource_receipt|binary\|)' "$MANIFEST_PATH"
}

run_step() {
    local q="$1" step="$2" execution_id="$3" out="$4" log="$5" summary="$6" provenance="$7" smoke="$8" skip_warmup="$9"
    local -a command=("$HARNESS_BIN" "$step" --out "$out")
    case "$step" in
        equivalence)
            # The measure run covers the harness's whole committed order table,
            # whose per-order sample counts were chosen against this budget, so
            # it narrows nothing. Smoke keeps its plumbing evidence cheap: the
            # largest orders cost minutes to hours per matrix on the device.
            [[ "$smoke" == true ]] && command+=(--matrices 1 --n 8)
            ;;
        grid)
            if [[ "$smoke" == true ]]; then command+=(--only "q=$q,n=12"); else command+=(--only "q=$q"); fi
            # grid alone parses --only, --execution-id, and
            # --skip-machine-warmup (usage.txt). Its execution id reserves a
            # disjoint stream-index block for this fresh process, and it owns
            # the 90 s whole-machine warm-up that the later steps inherit under
            # the held lock. The other steps are handed neither flag: they read
            # fixed stream addresses, and their permissive parsers would accept
            # the flags into the invocation preamble while ignoring them.
            command+=(--execution-id "$execution_id")
            [[ "$skip_warmup" == true ]] && command+=(--skip-machine-warmup)
            ;;
        gray-update|horizontal-product)
            # Both isolates default --n to the harness's own timing-grid order
            # set, so the measure run passes no order flag and covers every grid
            # order for this field; the orders that ran are recorded in the
            # step's CSV preamble. Smoke narrows to a single tiny order.
            command+=(--q "$q")
            if [[ "$smoke" == true ]]; then
                command+=(--n 1)
                case "$step" in
                    gray-update) command+=(--steps 1) ;;
                    horizontal-product) command+=(--samples 1) ;;
                esac
            fi
            ;;
        *) die "unknown pipeline step: $step" ;;
    esac
    record_command "$provenance" "${command[@]}"
    local rendered rc status
    printf -v rendered '%q ' "${command[@]}"
    {
        echo "# command: ${rendered% }"
        echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        set +e
        "${command[@]}"
        rc=$?
        set -e
        echo "# exit_status: $rc"
    } > "$log" 2>&1
    if [[ "$rc" -eq 0 && ! -s "$out" ]]; then
        rc=2; status=failed
        echo "missing output after successful harness step" >> "$log"
    elif [[ "$step" == equivalence && -s "$out" ]] && grep -q '^q,n,reference,backend,matrices,mismatches,zeros_reference,zeros_backend,status$' "$out"; then
        # The harness writes the complete CSV before returning nonzero for a
        # value mismatch. Keep that evidence usable for per-field gating;
        # an exit failure with no equivalence CSV remains an infrastructure
        # failure and censors every field below.
        status=completed
    elif [[ "$rc" -eq 0 ]]; then
        status=completed
    else
        status=failed
    fi
    RUN_STEP_STATUS="$status"
    printf 'q=%s step=%s execution_id=%s status=%s exit=%s log=%s out=%s\n' "$q" "$step" "$execution_id" "$status" "$rc" "$log" "$out" >> "$summary"
    if [[ "$status" != completed ]]; then FAILURE_COUNT=$((FAILURE_COUNT + 1)); fi
}

# grid is the only timing step whose stream block depends on an execution id;
# the others record the fixed-stream marker whether they run or are censored.
timing_execution_id() {
    local step="$1" base="$2" step_index="$3"
    if [[ "$step" == grid ]]; then
        printf '%s\n' "$((base + step_index))"
    else
        printf '%s\n' "$FIXED_STREAM_EXECUTION_ID"
    fi
}

record_equivalence_skip() {
    local q="$1" step="$2" execution_id="$3" summary="$4"
    printf 'q=%s step=%s execution_id=%s status=skipped_equivalence_failed exit=- log=- out=-\n' \
        "$q" "$step" "$execution_id" >> "$summary"
}

equivalence_field_failed() {
    local q="$1" csv="$2"
    local row_q mismatches status
    while IFS=, read -r row_q _ _ _ _ mismatches _ _ status; do
        [[ "$row_q" == "$q" ]] || continue
        if [[ "$mismatches" =~ ^[1-9][0-9]*$ || "$status" == "MISMATCH" ]]; then
            return 0
        fi
    done < <(grep -v '^#' "$csv" | tail -n +2)
    return 1
}

record_shared_equivalence() {
    local execution_id="$1" status="$2" exit_status="$3" log="$4" out="$5" summary="$6"
    printf 'shared_equivalence=execution_id=%s status=%s exit=%s log=%s out=%s\n' \
        "$execution_id" "$status" "$exit_status" "$log" "$out" >> "$summary"
}

run_locked_pipeline() {
    local smoke="$1"
    CAMPAIGN_MODE="$([[ "$smoke" == true ]] && echo smoke || echo measure)"
    export CAMPAIGN_MODE
    local q study run_dir summary provenance base step_index execution_id skip_warmup
    local shared_equivalence_status shared_equivalence_exit
    local shared_equivalence_csv shared_equivalence_log shared_equivalence_out
    local idx shared_run shared_summary shared_provenance
    local timing_status equivalence_falsified=false
    local -a field_runs field_summaries field_provenances
    local first_grid=true
    FAILURE_COUNT=0
    # Set up all field summaries before the single global equivalence check so
    # that its execution, CSV, and verdict are recorded in all three fields.
    idx=0
    for q in 3 5 7; do
        study=$(study_dir_for_q "$q")
        if [[ "$smoke" == true ]]; then run_dir="$study/smoke/$RUN_ID"; else run_dir="$study"; fi
        mkdir -p "$run_dir"
        summary="$run_dir/permanent-campaign-$RUN_ID.run-summary.txt"
        provenance="$run_dir/permanent-campaign-$RUN_ID.provenance.txt"
        : > "$summary"
        {
            echo "schema_version: 1"
            echo "campaign_run_id: $RUN_ID"
            echo "mode: $CAMPAIGN_MODE"
        } > "$summary"
        write_provenance "$provenance" "$run_dir"
        field_runs[idx]="$run_dir"
        field_summaries[idx]="$summary"
        field_provenances[idx]="$provenance"
        idx=$((idx + 1))
    done

    shared_run="${field_runs[0]}"
    shared_summary="${field_summaries[0]}"
    shared_provenance="${field_provenances[0]}"
    shared_equivalence_out="$shared_run/permanent-campaign-$RUN_ID-shared-equivalence.csv"
    shared_equivalence_csv="$shared_equivalence_out"
    shared_equivalence_log="$shared_run/permanent-campaign-$RUN_ID-shared-equivalence.log"
    # The run id and mode in each summary header identify this equivalence
    # execution; the CSV preamble records the fixed stream address it read.
    run_step 3 equivalence "$FIXED_STREAM_EXECUTION_ID" "$shared_equivalence_out" \
        "$shared_equivalence_log" "$shared_summary" "$shared_provenance" "$smoke" false
    shared_equivalence_status="$RUN_STEP_STATUS"
    shared_equivalence_exit=$(awk -F'exit_status: ' 'END {print $2}' "$shared_equivalence_log")
    for idx in 0 1 2; do
        record_shared_equivalence "$FIXED_STREAM_EXECUTION_ID" \
            "$shared_equivalence_status" "$shared_equivalence_exit" \
            "$shared_equivalence_log" "$shared_equivalence_csv" "${field_summaries[$idx]}"
    done

    if [[ "$shared_equivalence_status" != completed ]]; then
        # No usable global CSV means no field-level verdict exists. Preserve
        # grid's canonical per-field execution id while censoring all timing
        # steps after the failed shared pre-flight.
        idx=0
        for q in 3 5 7; do
            if [[ "$smoke" == true ]]; then base=$((q * 1000 + 10000)); else base=$((q * 1000)); fi
            for step_index in 2 3 4; do
                case "$step_index" in 2) timing_status=grid ;; 3) timing_status=gray-update ;; 4) timing_status=horizontal-product ;; esac
                record_equivalence_skip "$q" "$timing_status" \
                    "$(timing_execution_id "$timing_status" "$base" "$step_index")" "${field_summaries[$idx]}"
            done
            idx=$((idx + 1))
        done
    else
        idx=0
        for q in 3 5 7; do
            if [[ "$smoke" == true ]]; then base=$((q * 1000 + 10000)); else base=$((q * 1000)); fi
            if equivalence_field_failed "$q" "$shared_equivalence_csv"; then
                equivalence_falsified=true
                for step_index in 2 3 4; do
                    case "$step_index" in 2) timing_status=grid ;; 3) timing_status=gray-update ;; 4) timing_status=horizontal-product ;; esac
                    # Unsupported and unavailable rows do not enter this
                    # branch: only mismatches > 0 or status=MISMATCH censor.
                    record_equivalence_skip "$q" "$timing_status" \
                        "$(timing_execution_id "$timing_status" "$base" "$step_index")" "${field_summaries[$idx]}"
                done
            else
                for step_index in 2 3 4; do
                    case "$step_index" in 2) timing_status=grid ;; 3) timing_status=gray-update ;; 4) timing_status=horizontal-product ;; esac
                    execution_id=$(timing_execution_id "$timing_status" "$base" "$step_index")
                    skip_warmup=false
                    [[ "$first_grid" == false ]] && skip_warmup=true
                    run_step "$q" "$timing_status" "$execution_id" \
                        "${field_runs[$idx]}/permanent-campaign-$RUN_ID-q$q-$timing_status.csv" \
                        "${field_runs[$idx]}/permanent-campaign-$RUN_ID-q$q-$timing_status.log" \
                        "${field_summaries[$idx]}" "${field_provenances[$idx]}" "$smoke" "$skip_warmup"
                    if [[ "$timing_status" == grid ]]; then
                        # grid owns the 90 s warm-up. Once the first field's
                        # grid has run it, the later grid executions under this
                        # held lock pass --skip-machine-warmup as usage.txt
                        # directs; the steps that never parse the flag inherit
                        # the same warmed host without being handed it.
                        first_grid=false
                    fi
                done
            fi
            echo "summary: ${field_summaries[$idx]}"
            idx=$((idx + 1))
        done
    fi
    if [[ "$equivalence_falsified" == true ]]; then
        # A valid CSV mismatch is evidence rather than an infrastructure
        # failure, so its unaffected fields still run; the falsified field
        # nevertheless makes the campaign censored and returns exit 7.
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
    fi
    if [[ "$FAILURE_COUNT" -ne 0 ]]; then
        echo "campaign censored: $FAILURE_COUNT step(s) failed; see run-summary files" >&2
        return "$CENSORED_EXIT"
    fi
    echo "campaign completed: all pipeline steps completed"
}

run_campaign() {
    local smoke="$1"
    verify_manifest
    mkdir -p "$STUDY_ROOT"
    # The canonical wrapper takes /tmp/gf2-ccx1.lock with --full-host and
    # holds it around this entire internal invocation, not once per step.
    CAMPAIGN_HARNESS_BIN="$HARNESS_BIN" \
        "$FLOCK_WRAPPER" --full-host "$BASH" "$SCRIPT_PATH" __locked-pipeline "$smoke"
    run_simulation_campaign
}

run_simulation_campaign() {
    # The feasibility runner's manifest is a harness manifest. A caller that
    # also supplies the frozen JSON campaign manifest opts into the production
    # gf2-sim campaign binary. Keep this invocation in the same canonical lock
    # domain as the timed host work above.
    [[ -n "$CAMPAIGN_SIM_BINARY" ]] || return 0
    [[ -n "$CAMPAIGN_SIM_MANIFEST" ]] || die "CAMPAIGN_SIM_MANIFEST is required with CAMPAIGN_SIM_BINARY"
    [[ -n "$CAMPAIGN_SIM_OUTPUT" ]] || die "CAMPAIGN_SIM_OUTPUT is required with CAMPAIGN_SIM_BINARY"
    [[ -n "$CAMPAIGN_SIM_FIELD" ]] || die "CAMPAIGN_SIM_FIELD is required with CAMPAIGN_SIM_BINARY"
    # An accelerator cell is sized from its own measured per-matrix cost, so the
    # cost table travels with the manifest. The binary refuses an accelerator
    # manifest without it; passing it here is what lets the lock-held path run
    # accelerator work at all.
    local -a accelerator=()
    if [[ -n "$CAMPAIGN_SIM_ACCELERATOR_COSTS" ]]; then
        [[ -f "$CAMPAIGN_SIM_ACCELERATOR_COSTS" ]] \
            || die "CAMPAIGN_SIM_ACCELERATOR_COSTS not found: $CAMPAIGN_SIM_ACCELERATOR_COSTS"
        accelerator+=(--accelerator-cost-table "$CAMPAIGN_SIM_ACCELERATOR_COSTS")
        [[ -n "$CAMPAIGN_SIM_ACCELERATOR_CAP_MS" ]] \
            && accelerator+=(--accelerator-launch-cap-ms "$CAMPAIGN_SIM_ACCELERATOR_CAP_MS")
    fi
    "$FLOCK_WRAPPER" --full-host "$CAMPAIGN_SIM_BINARY" \
        --manifest "$CAMPAIGN_SIM_MANIFEST" \
        --output "$CAMPAIGN_SIM_OUTPUT" \
        --q "$CAMPAIGN_SIM_FIELD" \
        --workers "$CAMPAIGN_SIM_WORKERS" \
        "${accelerator[@]}"
}

validate_premeasure_plan() {
    require_command awk
    require_command sort
    [[ -f "$PREMEASURE_PLAN" ]] || die "premeasurement plan not found: $PREMEASURE_PLAN"
    local header rows cells bad
    header=$(head -n 1 "$PREMEASURE_PLAN")
    [[ "$header" == "q,n,manifest_backend,harness_backend,batch_size,process_count,warmup_seconds,timed_repetition_min,timed_seconds_min,per_process_cap_seconds,interleave_block,nomination_basis" ]] \
        || die "premeasurement plan has an unexpected header"
    rows=$(awk -F, 'NR > 1 && NF == 12 { n++ } END { print n + 0 }' "$PREMEASURE_PLAN")
    [[ "$rows" -eq 120 ]] || die "premeasurement plan has $rows rows; expected 120"
    cells=$(awk -F, 'NR > 1 { key=$1 ":" $2; seen[key]++; rows++ } END { for (key in seen) if (seen[key] != 2) bad=1; for (key in seen) cells++; if (bad) exit 1; print cells + 0 }' "$PREMEASURE_PLAN") \
        || die "premeasurement plan does not contain exactly two rows per cell"
    [[ "$cells" -eq 60 ]] || die "premeasurement plan has $cells cells; expected 60"
    bad=$(awk -F, 'NR > 1 {
        if ($1 !~ /^[357]$/ || $2 !~ /^[0-9]+$/ || $5 !~ /^[1-9][0-9]*$/ ||
            $6 != 12 || $7 < 3 || $8 != 5 || $9 < 5 || $10 != 120 ||
            $11 != "A B B A") bad++
        if ($4 !~ /^(cpu_rayon_batch_scalar|cpu_rayon_intra_matrix|gpu_hip|cpu_ryser_generic)$/) bad++
    } END { print bad + 0 }' "$PREMEASURE_PLAN")
    [[ "$bad" -eq 0 ]] || die "premeasurement plan has $bad invalid protocol or backend rows"
}

load_premeasure_schedule() {
    local check_admission="${1:-true}"
    validate_premeasure_plan
    local row q n token backend batch _rest key code _cycle
    local -a sorted_rows
    local -A config_token config_backend config_batch config_seen
    mapfile -t sorted_rows < <(tail -n +2 "$PREMEASURE_PLAN" | sort -t, -k1,1n -k2,2n -k3,3)
    [[ "${#sorted_rows[@]}" -eq 120 ]] || die "failed to load the 120-row premeasurement plan"
    for row in "${sorted_rows[@]}"; do
        IFS=, read -r q n token backend batch _rest <<< "$row"
        key="$q:$n"
        if [[ -z "${config_seen[$key]:-}" ]]; then
            config_seen[$key]=1
        fi
        if [[ -n "${config_token[$key:A]:-}" ]]; then
            code=B
        else
            code=A
        fi
        config_token["$key:$code"]="$token"
        config_backend["$key:$code"]="$backend"
        config_batch["$key:$code"]="$batch"
    done
    local -a cell_keys
    mapfile -t cell_keys < <(printf '%s\n' "${!config_seen[@]}" | sort -t: -k1,1n -k2,2n)
    [[ "${#cell_keys[@]}" -eq 60 ]] || die "premeasurement schedule has ${#cell_keys[@]} cells; expected 60"
    PREMEASURE_CONFIG_KEYS=("${cell_keys[@]}")
    PREMEASURE_SCHEDULE=()
    local process_index=0
    for key in "${cell_keys[@]}"; do
        IFS=: read -r q n <<< "$key"
        for _cycle in 1 2 3 4 5 6; do
            for code in A B B A; do
                PREMEASURE_SCHEDULE+=("$q,$n,$code,${config_token[$key:$code]},${config_backend[$key:$code]},${config_batch[$key:$code]},$process_index")
                process_index=$((process_index + 1))
            done
        done
    done
    [[ "${#PREMEASURE_SCHEDULE[@]}" -eq 1440 ]] || die "premeasurement schedule has ${#PREMEASURE_SCHEDULE[@]} processes; expected 1440"
    if [[ "$check_admission" == true ]]; then
        validate_premeasure_admission
    fi
}

validate_premeasure_admission() {
    local q n _token backend batch _rest message
    local -a inadmissible=()
    while IFS=, read -r q n _token backend batch _rest; do
        if ! message=$(
            "$HARNESS_BIN" grid --admit-only \
                --only "q=$q,n=$n,backend=$backend" \
                --orders "$n" --batch-size "$batch" 2>&1
        ); then
            inadmissible+=("q=$q,n=$n,backend=$backend,batch_size=$batch: ${message:-no diagnostic output}")
        fi
    done < <(tail -n +2 "$PREMEASURE_PLAN" | sort -t, -k1,1n -k2,2n -k3,3)
    if [[ "${#inadmissible[@]}" -ne 0 ]]; then
        echo "ERROR: premeasurement plan has ${#inadmissible[@]} inadmissible rows:" >&2
        printf '  %s\n' "${inadmissible[@]}" >&2
        exit 2
    fi
}

premeasure_run_dir() {
    printf '%s\n' "$TARGET_ROOT/premeasure-$RUN_ID"
}

write_gpu_health_snapshot_body() {
    local phase="$1"
    echo "phase: $phase"
    echo "observed_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    capture_block \
        "rocm_smi_health_command: $ROCM_PATH/bin/rocm-smi --showuse --showpower --showmeminfo vram" \
        "$ROCM_PATH/bin/rocm-smi" --showuse --showpower --showmeminfo vram
    echo "kfd_process_query_command: fuser -v /dev/kfd"
    if [[ ! -e /dev/kfd ]]; then
        echo "kfd_process_exists: unavailable: /dev/kfd does not exist"
        return 0
    fi
    if ! command -v fuser >/dev/null 2>&1; then
        echo "kfd_process_exists: unavailable: command not found: fuser"
        return 0
    fi
    local output rc
    set +e
    output=$(fuser -v /dev/kfd 2>&1)
    rc=$?
    set -e
    case "$rc" in
        0) echo "kfd_process_exists: true" ;;
        1) echo "kfd_process_exists: false" ;;
        *)
            echo "kfd_process_exists: unavailable"
            echo "command_exit_status: $rc"
            ;;
    esac
    if [[ -n "$output" ]]; then
        printf '%s\n' "$output"
    fi
}

write_gpu_health_snapshot() {
    local path="$1" phase="$2"
    {
        echo "schema_version: 1"
        write_gpu_health_snapshot_body "$phase"
    } > "$path"
}

write_premeasure_provenance() {
    local run_dir="$1" session_id="$2" warmup_state="$3" cap="$4"
    local provenance="$run_dir/sessions/session-$session_id.provenance.txt"
    local end_snapshot="$run_dir/sessions/session-$session_id.gpu-health.txt"
    mkdir -p "$run_dir/sessions"
    {
        echo "schema_version: 1"
        echo "campaign_run_id: $RUN_ID"
        echo "session_id: $session_id"
        echo "mode: premeasure"
        echo "started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "repository_revision: $(git -C "$REPO_ROOT" rev-parse --verify HEAD)"
        echo "session_cap_seconds: $cap"
        echo "warmup_state_file: $warmup_state"
        echo "manifest: $MANIFEST_PATH"
        echo "binary_hashes: see manifest and per-process harness CSV preambles"
        print_manifest_binary_hashes
        echo "host_identity_command: lscpu"
        capture_block "cpu_model_command: lscpu" lscpu
        echo "rocm_identity_command: $ROCM_PATH/bin/rocm-smi --showproductname --showuniqueid"
        capture_block "gpu_model_uuid_command: $ROCM_PATH/bin/rocm-smi --showproductname --showuniqueid" "$ROCM_PATH/bin/rocm-smi" --showproductname --showuniqueid
        capture_block "rocm_hipcc_version_command: $ROCM_PATH/bin/hipcc --version" "$ROCM_PATH/bin/hipcc" --version
        capture_block "kernel_version_command: uname -r" uname -r
        echo "gpu_health_end_snapshot: $end_snapshot"
        echo "gpu_health_snapshot_start:"
        write_gpu_health_snapshot_body start
        echo "wrapper_invocation: $FLOCK_WRAPPER --full-host $BASH $SCRIPT_PATH __locked-premeasure $cap"
        echo "exact_plan: $PREMEASURE_PLAN"
        echo "plan_sha256: $(hash_file "$PREMEASURE_PLAN")"
    } > "$provenance"
    PREMEASURE_SESSION_PROVENANCE="$provenance"
}

premeasure_record_interrupted() {
    [[ -n "${PREMEASURE_CURRENT_RECEIPT:-}" ]] || return 0
    local receipt="$PREMEASURE_CURRENT_RECEIPT"
    if [[ ! -f "$receipt/exit.status" ]]; then
        printf '%s\n' 130 > "$receipt/exit.status"
        printf '%s\n' "status: failed" >> "$receipt/receipt.txt"
        printf '%s\n' "failure: interrupted by runner signal" >> "$receipt/receipt.txt"
    fi
}

premeasure_process_is_final() {
    [[ -f "$1/exit.status" ]] && grep -q '^status: \(completed\|failed\)$' "$1/receipt.txt"
}

# A grid-admission refusal is the harness rejecting the cell before any
# sampling or timing, so superseding it replaces no measurement outcome.
# Every other failed receipt is final: the preregistered schedule forbids
# replacing a process based on its measurement result.
premeasure_process_is_admission_refusal() {
    [[ -f "$1/exit.status" ]] \
        && grep -q '^status: failed$' "$1/receipt.txt" \
        && [[ -f "$1/harness.log" ]] \
        && grep -q 'matched no cell in the grid' "$1/harness.log"
}

premeasure_archive_refused_receipt() {
    local run_dir="$1" receipt="$2" process_index="$3" q="$4" n="$5" code="$6"
    local old_session_id
    old_session_id=$(sed -n 's/^session_id: //p' "$receipt/receipt.txt" | tail -n 1)
    [[ -n "$old_session_id" ]] || old_session_id=unknown
    mkdir -p "$run_dir/superseded"
    mv "$receipt" "$run_dir/superseded/process-$(printf '%04d' "$process_index")-$q-$n-$code-$old_session_id"
}

run_premeasure_process() {
    local run_dir="$1" session_id="$2" process_index="$3" q="$4" n="$5" code="$6" token="$7" backend="$8" batch="$9" warmup="${10}"
    local receipt
    receipt="$run_dir/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
    local csv="$receipt/scratch.csv" log="$receipt/harness.log" rc status skip=false
    mkdir -p "$receipt"
    PREMEASURE_CURRENT_RECEIPT="$receipt"
    if [[ "$warmup" == skipped ]]; then skip=true; fi
    local -a command=("$HARNESS_BIN" grid --out "$csv" --only "q=$q,n=$n,backend=$backend" --orders "$n" --batch-size "$batch" --execution-id "$process_index")
    [[ "$skip" == true ]] && command+=(--skip-machine-warmup)
    local rendered
    printf -v rendered '%q ' "${command[@]}"
    {
        echo "schema_version: 1"
        echo "run_id: $RUN_ID"
        echo "session_id: $session_id"
        echo "schedule_position: $process_index"
        echo "q: $q"
        echo "n: $n"
        echo "config_code: $code"
        echo "manifest_backend: $token"
        echo "harness_backend: $backend"
        echo "batch_size: $batch"
        echo "machine_warmup: $warmup"
        echo "started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "command: ${rendered% }"
        echo "scratch_csv: $csv"
    } > "$receipt/receipt.txt"
    set +e
    "${command[@]}" > "$log" 2>&1
    rc=$?
    set -e
    if [[ "$rc" -eq 0 && ! -s "$csv" ]]; then rc=2; fi
    if [[ "$rc" -eq 0 ]]; then status=completed; else status=failed; fi
    printf '%s\n' "$rc" > "$receipt/exit.status"
    printf 'status: %s\nexit_status: %s\nfinished_utc: %s\n' "$status" "$rc" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$receipt/receipt.txt"
    PREMEASURE_CURRENT_RECEIPT=""
    [[ "$status" == completed ]] || PREMEASURE_FAILURE_COUNT=$((PREMEASURE_FAILURE_COUNT + 1))
}

run_locked_premeasure() {
    local cap="$1" run_dir session_id warmup_state elapsed key q n code token backend batch process_index row
    load_premeasure_schedule
    run_dir=$(premeasure_run_dir)
    mkdir -p "$run_dir/processes"
    warmup_state="$run_dir/machine-warmup.state"
    session_id="$(date -u +%Y%m%dT%H%M%SZ)-$$"
    write_premeasure_provenance "$run_dir" "$session_id" "$warmup_state" "$cap"
    PREMEASURE_FAILURE_COUNT=0
    PREMEASURE_CURRENT_RECEIPT=""
    trap 'premeasure_record_interrupted; exit 130' INT TERM HUP
    local session_start
    session_start=$(date +%s)
    local stopped_by_cap=false
    for row in "${PREMEASURE_SCHEDULE[@]}"; do
        IFS=, read -r q n code token backend batch process_index <<< "$row"
        local receipt
        receipt="$run_dir/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
        if premeasure_process_is_admission_refusal "$receipt"; then
            premeasure_archive_refused_receipt "$run_dir" "$receipt" "$process_index" "$q" "$n" "$code"
            echo "premeasure supersede: process=$process_index config=$q,$n,$code"
        elif premeasure_process_is_final "$receipt"; then
            if [[ "$process_index" -eq 0 && ! -f "$warmup_state" ]]; then
                printf 'state: resumed; first process already has a durable receipt\nutc: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$warmup_state"
            fi
            echo "premeasure skip: process=$process_index config=$q,$n,$code"
            continue
        fi
        if [[ -e "$receipt" ]]; then
            # A scratch directory without a final status is an interrupted
            # process. It is censored in place and never reused.
            printf '%s\n' 125 > "$receipt/exit.status"
            printf 'status: failed\nexit_status: 125\nfailure: interrupted before final receipt\n' >> "$receipt/receipt.txt"
            PREMEASURE_FAILURE_COUNT=$((PREMEASURE_FAILURE_COUNT + 1))
            echo "premeasure skip: process=$process_index already interrupted"
            continue
        fi
        elapsed=$(( $(date +%s) - session_start ))
        if [[ "$elapsed" -ge "$cap" ]]; then
            stopped_by_cap=true
            break
        fi
        if [[ ! -f "$warmup_state" ]]; then
            printf 'state: performed\nutc: %s\nsession_id: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$session_id" > "$warmup_state"
            warmup=full
        else
            warmup=skipped
        fi
        run_premeasure_process "$run_dir" "$session_id" "$process_index" "$q" "$n" "$code" "$token" "$backend" "$batch" "$warmup"
    done
    trap - INT TERM HUP
    write_gpu_health_snapshot "$run_dir/sessions/session-$session_id.gpu-health.txt" end
    if [[ "$stopped_by_cap" == true ]]; then
        echo "premeasure session stopped cleanly at session cap ${cap}s"
        printf 'status: stopped_by_session_cap\nfinished_utc: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$run_dir/sessions/session-$session_id.status"
    else
        echo "premeasure schedule complete"
        printf 'status: complete\nfinished_utc: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$run_dir/sessions/session-$session_id.status"
    fi
    echo "premeasure provenance: $PREMEASURE_SESSION_PROVENANCE"
    [[ "$PREMEASURE_FAILURE_COUNT" -eq 0 ]] || return "$CENSORED_EXIT"
}

PREMEASURE_COLLECT_COLUMNS=(
    record_type process_index schedule_position raw_file receipt_file scratch_file
    config_id config_code plan_q plan_n plan_manifest_backend plan_harness_backend
    plan_batch_size ledger_state candidate_emitted receipt_present
    receipt_structurally_valid scratch_present scratch_structurally_valid
    session_resolution identity_valid provenance_complete row_valid validity_reasons
    receipt_schema_version receipt_run_id receipt_session_id receipt_schedule_position
    receipt_q receipt_n receipt_config_code receipt_manifest_backend
    receipt_harness_backend receipt_batch_size receipt_machine_warmup
    receipt_started_utc receipt_finished_utc receipt_command receipt_scratch_csv
    receipt_status receipt_exit_status exit_status_file receipt_exit_agreement
    receipt_failure receipt_plan_sha256 receipt_manifest_locator
    receipt_prepared_manifest_sha256 session_provenance_file session_id
    session_campaign_run_id session_started_utc session_repository_revision
    session_plan_path session_plan_sha256 session_manifest_locator
    session_wrapper_invocation session_binary_actual_chain
    session_binary_expected_chain session_binary_chain_match
    scratch_binary_matches_session_actual recovery_plan_sha256
    recovery_plan_matches_session scratch_q scratch_n scratch_backend scratch_outcome
    scratch_batch_size scratch_reps scratch_matrices scratch_zeros scratch_total_s
    scratch_gen_s scratch_eval_s scratch_reduce_s scratch_store_s
    scratch_composite_matrices_per_s scratch_eval_matrices_per_s observed_git_sha
    observed_git_worktree_dirty observed_harness_source_sha
    observed_harness_source_dirty observed_deps_source_sha observed_deps_source_dirty
    observed_running_binary_sha256 observed_rustc observed_cargo observed_cpu
    observed_logical_cpus observed_rayon_threads observed_cpu_features
    observed_cpu_governor observed_gpu observed_rocm observed_hip_feature
    observed_kernel observed_timestamp_utc observed_invocation
)

csv_write_header() {
    local path="$1"
    shift
    local IFS=,
    printf '%s\n' "$*" > "$path"
}

csv_write_row() {
    local path="$1"
    shift
    [[ "$#" -eq "${#PREMEASURE_COLLECT_COLUMNS[@]}" ]] \
        || die "collector row has $# fields; expected ${#PREMEASURE_COLLECT_COLUMNS[@]}"
    local field escaped separator=''
    {
        for field in "$@"; do
            field="${field//$'\r'/ }"
            field="${field//$'\n'/ }"
            escaped="${field//\"/\"\"}"
            printf '%s"%s"' "$separator" "$escaped"
            separator=,
        done
        printf '\n'
    } >> "$path"
}

declare -A COLON_VALUES COLON_COUNTS

parse_colon_metadata() {
    local path="$1" line key value
    COLON_VALUES=()
    COLON_COUNTS=()
    [[ -f "$path" ]] || return 0
    while IFS= read -r line || [[ -n "$line" ]]; do
        if [[ "$line" =~ ^([A-Za-z0-9_]+):[[:space:]]?(.*)$ ]]; then
            key="${BASH_REMATCH[1]}"
            value="${BASH_REMATCH[2]}"
            COLON_COUNTS["$key"]=$(( ${COLON_COUNTS["$key"]:-0} + 1 ))
            if [[ -n "${COLON_VALUES[$key]+set}" ]]; then
                COLON_VALUES["$key"]+=" || $value"
            else
                COLON_VALUES["$key"]="$value"
            fi
        fi
    done < "$path"
}

declare -A SCRATCH_META_VALUES SCRATCH_META_COUNTS SCRATCH_DATA
SCRATCH_PRESENT=false
SCRATCH_STRUCTURALLY_VALID=false
SCRATCH_REASONS=''

scratch_reason() {
    SCRATCH_REASONS+="${SCRATCH_REASONS:+;}$1"
}

parse_premeasure_scratch() {
    local path="$1" plan_backend="${2:-}" line key value header_seen=false header_line='' data_line=''
    local second_data_line='' data_count=0 i
    local -a header_fields data_fields
    local -a expected_header=(
        q n backend outcome batch_size reps matrices zeros total_s gen_s eval_s
        reduce_s store_s composite_matrices_per_s eval_matrices_per_s
    )
    local -a required_preamble=(
        git_sha git_worktree_dirty harness_source_sha harness_source_dirty
        deps_source_sha deps_source_dirty binary_sha256 rustc cargo cpu logical_cpus
        rayon_threads avx2 cpu_governor gpu rocm hip_feature kernel timestamp_utc
        invocation
    )
    SCRATCH_META_VALUES=()
    SCRATCH_META_COUNTS=()
    SCRATCH_DATA=()
    SCRATCH_PRESENT=false
    SCRATCH_STRUCTURALLY_VALID=false
    SCRATCH_REASONS=''
    [[ -f "$path" ]] || return 0
    SCRATCH_PRESENT=true
    while IFS= read -r line || [[ -n "$line" ]]; do
        if [[ "$header_seen" == false && "$line" == \#* ]]; then
            if [[ "$line" =~ ^\#[[:space:]]([A-Za-z0-9_]+):[[:space:]]?(.*)$ ]]; then
                key="${BASH_REMATCH[1]}"
                value="${BASH_REMATCH[2]}"
                SCRATCH_META_COUNTS["$key"]=$(( ${SCRATCH_META_COUNTS["$key"]:-0} + 1 ))
                if [[ -n "${SCRATCH_META_VALUES[$key]+set}" ]]; then
                    SCRATCH_META_VALUES["$key"]+=" || $value"
                else
                    SCRATCH_META_VALUES["$key"]="$value"
                fi
            fi
            continue
        fi
        [[ -n "$line" ]] || continue
        if [[ "$line" == \#* ]]; then
            scratch_reason "preamble_after_header"
        elif [[ "$header_seen" == false ]]; then
            header_line="$line"
            header_seen=true
        else
            data_count=$((data_count + 1))
            [[ "$data_count" -ne 1 ]] || data_line="$line"
            [[ "$data_count" -ne 2 ]] || second_data_line="$line"
        fi
    done < "$path"
    if [[ "$header_seen" == false ]]; then
        scratch_reason "missing_header"
    else
        IFS=, read -r -a header_fields <<< "${header_line},__collector_end__"
        unset 'header_fields[${#header_fields[@]}-1]'
        if [[ "${#header_fields[@]}" -lt "${#expected_header[@]}" ]]; then
            scratch_reason "short_header"
        else
            for ((i=0; i<${#expected_header[@]}; i++)); do
                if [[ "${header_fields[$i]}" != "${expected_header[$i]}" ]]; then
                    scratch_reason "header_column_${i}_mismatch"
                fi
            done
        fi
    fi
    if [[ "$data_count" -eq 2 && "$plan_backend" == gpu_hip ]]; then
        # The frozen premeasure harness grid holds two gpu_hip specs per cell
        # (M=256 and M=1024); the runner's --batch-size override coerces both
        # to the plan's M before --only filters, so one gpu_hip process
        # measures its cell twice into one scratch file. Per the recorded
        # decision on jit:7a816262 (2026-08-25), the native-M grid slot — the
        # second data row — is the process measurement; the first row is a
        # recorded, unused replicate. See
        # dev/benchmarks/permanent_campaign/premeasure-v1-deviations.md.
        data_line="$second_data_line"
    elif [[ "$data_count" -ne 1 ]]; then
        scratch_reason "data_row_count_${data_count}"
        data_line=''
    fi
    if [[ -n "$data_line" ]]; then
        IFS=, read -r -a data_fields <<< "${data_line},__collector_end__"
        unset 'data_fields[${#data_fields[@]}-1]'
        if [[ "${#data_fields[@]}" -lt "${#expected_header[@]}" ]]; then
            scratch_reason "short_data_row"
        else
            for ((i=0; i<${#expected_header[@]}; i++)); do
                SCRATCH_DATA["${expected_header[$i]}"]="${data_fields[$i]}"
            done
        fi
    fi
    for key in "${required_preamble[@]}"; do
        if [[ "${SCRATCH_META_COUNTS[$key]:-0}" -eq 0 ]]; then
            scratch_reason "missing_preamble_${key}"
        elif [[ "${SCRATCH_META_COUNTS[$key]}" -ne 1 ]]; then
            scratch_reason "duplicate_preamble_${key}"
        elif [[ -z "${SCRATCH_META_VALUES[$key]}" ]]; then
            scratch_reason "empty_preamble_${key}"
        fi
    done
    [[ -n "$SCRATCH_REASONS" ]] || SCRATCH_STRUCTURALLY_VALID=true
}

session_metadata_value() {
    local composite="$1"$'\034'"$2"
    printf '%s' "${SESSION_VALUES[$composite]:-}"
}

session_metadata_count() {
    local composite="$1"$'\034'"$2"
    printf '%s' "${SESSION_COUNTS[$composite]:-0}"
}

premeasure_collect() {
    # Collection is a recovery projection over already-observed evidence. It
    # deliberately does not verify or run the current harness, inspect the
    # current repository state, or hash the current binary: none of those
    # collector-time facts describe an
    # execution that already happened.
    load_premeasure_schedule false
    local run_dir ledger_tmp candidates_tmp ledger_path candidates_path
    local row q n code token backend batch process_index receipt receipt_file csv
    local recovery_plan_sha256
    local -A expected completed terminal invalid censored candidates
    run_dir=$(premeasure_run_dir)
    mkdir -p "$run_dir"
    ledger_path="$run_dir/premeasure-ledger.csv"
    candidates_path="$run_dir/premeasure-candidates.csv"
    ledger_tmp="$run_dir/.premeasure-ledger.csv.tmp.$$"
    candidates_tmp="$run_dir/.premeasure-candidates.csv.tmp.$$"
    PREMEASURE_COLLECT_TMP_FILES=("$ledger_tmp" "$candidates_tmp")
    trap 'rm -f "${PREMEASURE_COLLECT_TMP_FILES[@]}"' EXIT
    csv_write_header "$ledger_tmp" "${PREMEASURE_COLLECT_COLUMNS[@]}"
    csv_write_header "$candidates_tmp" "${PREMEASURE_COLLECT_COLUMNS[@]}"
    recovery_plan_sha256="$(hash_file "$PREMEASURE_PLAN")"

    local -A SESSION_VALUES SESSION_COUNTS SESSION_ID_COUNT SESSION_ID_PATH
    local -A SESSION_ACTUAL_CHAIN SESSION_EXPECTED_CHAIN SESSION_CHAIN_MATCH
    local -A SESSION_ACTUAL_HASHES SESSION_FILE_REASONS
    local -a session_files=()
    local session_file line key value composite inner_id
    local valid_session_files=0 only_session_file=''
    if [[ -d "$run_dir/sessions" ]]; then
        shopt -s nullglob
        session_files=("$run_dir"/sessions/session-*.provenance.txt)
        shopt -u nullglob
    fi
    for session_file in "${session_files[@]}"; do
        local -A actual_by_path=() expected_by_path=() actual_seen=() expected_seen=()
        local -a actual_paths=() expected_paths=()
        local actual_chain='' expected_chain='' actual_hashes='' chain_status=true
        while IFS= read -r line || [[ -n "$line" ]]; do
            if [[ "$line" == "binary_hash: "* ]]; then
                value="${line#binary_hash: }"
                local observed_hash="${value##* }" observed_path="${value% *}"
                actual_chain+="${actual_chain:+ | }$observed_path=$observed_hash"
                actual_hashes+="${actual_hashes:+$'\n'}$observed_hash"
                actual_seen["$observed_path"]=$(( ${actual_seen["$observed_path"]:-0} + 1 ))
                actual_by_path["$observed_path"]="$observed_hash"
                actual_paths+=("$observed_path")
            elif [[ "$line" == "binary_manifest_hash: "* ]]; then
                value="${line#binary_manifest_hash: }"
                local expected_hash="${value##* }" expected_path="${value% *}"
                expected_chain+="${expected_chain:+ | }$expected_path=$expected_hash"
                expected_seen["$expected_path"]=$(( ${expected_seen["$expected_path"]:-0} + 1 ))
                expected_by_path["$expected_path"]="$expected_hash"
                expected_paths+=("$expected_path")
            elif [[ "$line" =~ ^([A-Za-z0-9_]+):[[:space:]]?(.*)$ ]]; then
                key="${BASH_REMATCH[1]}"
                value="${BASH_REMATCH[2]}"
                composite="$session_file"$'\034'"$key"
                SESSION_COUNTS["$composite"]=$(( ${SESSION_COUNTS["$composite"]:-0} + 1 ))
                if [[ -n "${SESSION_VALUES[$composite]+set}" ]]; then
                    SESSION_VALUES["$composite"]+=" || $value"
                else
                    SESSION_VALUES["$composite"]="$value"
                fi
            fi
        done < "$session_file"
        if [[ "${#actual_paths[@]}" -eq 0 || "${#expected_paths[@]}" -eq 0 ]]; then
            chain_status=unavailable
        elif [[ "${#actual_paths[@]}" -ne "${#expected_paths[@]}" ]]; then
            chain_status=false
        else
            for key in "${actual_paths[@]}"; do
                if [[ "${actual_seen[$key]}" -ne 1 || "${expected_seen[$key]:-0}" -ne 1 \
                    || "${actual_by_path[$key]}" != "${expected_by_path[$key]:-}" ]]; then
                    chain_status=false
                fi
            done
        fi
        SESSION_ACTUAL_CHAIN["$session_file"]="$actual_chain"
        SESSION_EXPECTED_CHAIN["$session_file"]="$expected_chain"
        SESSION_ACTUAL_HASHES["$session_file"]="$actual_hashes"
        SESSION_CHAIN_MATCH["$session_file"]="$chain_status"
        inner_id="$(session_metadata_value "$session_file" session_id)"
        if [[ "$(session_metadata_count "$session_file" session_id)" -eq 1 && -n "$inner_id" ]]; then
            SESSION_ID_COUNT["$inner_id"]=$(( ${SESSION_ID_COUNT["$inner_id"]:-0} + 1 ))
            SESSION_ID_PATH["$inner_id"]="$session_file"
            valid_session_files=$((valid_session_files + 1))
            only_session_file="$session_file"
        else
            SESSION_FILE_REASONS["$session_file"]="invalid_session_id"
        fi
    done

    local any_incomplete=false
    for row in "${PREMEASURE_SCHEDULE[@]}"; do
        IFS=, read -r q n code token backend batch process_index <<< "$row"
        receipt="$run_dir/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
        receipt_file="$receipt/receipt.txt"
        csv="$receipt/scratch.csv"
        local config_key="$q:$n:$code"
        expected["$config_key"]=$(( ${expected["$config_key"]:-0} + 1 ))
        local reasons='' receipt_present=false receipt_valid=false
        local receipt_schema='' receipt_run='' receipt_session='' receipt_position=''
        local receipt_q='' receipt_n='' receipt_code='' receipt_token=''
        local receipt_backend='' receipt_batch='' receipt_warmup='' receipt_started=''
        local receipt_finished='' receipt_command='' receipt_scratch='' receipt_status=''
        local receipt_exit='' exit_file='' receipt_exit_agreement=unavailable
        local receipt_failure='' receipt_plan_sha=unavailable receipt_manifest=unavailable
        local receipt_prepared_manifest_sha=unavailable
        parse_colon_metadata "$receipt_file"
        if [[ -f "$receipt_file" ]]; then
            receipt_present=true
            receipt_valid=true
            local required_receipt_key
            local -a required_receipt_keys=(
                schema_version run_id schedule_position q n config_code manifest_backend
                harness_backend batch_size machine_warmup started_utc command scratch_csv
                status
            )
            for required_receipt_key in "${required_receipt_keys[@]}"; do
                if [[ "${COLON_COUNTS[$required_receipt_key]:-0}" -ne 1 \
                    || -z "${COLON_VALUES[$required_receipt_key]:-}" ]]; then
                    receipt_valid=false
                    reasons+="${reasons:+;}receipt_${required_receipt_key}_count_or_value"
                fi
            done
            if [[ "${COLON_COUNTS[session_id]:-0}" -gt 1 ]]; then
                receipt_valid=false
                reasons+="${reasons:+;}receipt_duplicate_session_id"
            fi
            receipt_schema="${COLON_VALUES[schema_version]:-}"
            receipt_run="${COLON_VALUES[run_id]:-}"
            receipt_session="${COLON_VALUES[session_id]:-}"
            receipt_position="${COLON_VALUES[schedule_position]:-}"
            receipt_q="${COLON_VALUES[q]:-}"
            receipt_n="${COLON_VALUES[n]:-}"
            receipt_code="${COLON_VALUES[config_code]:-}"
            receipt_token="${COLON_VALUES[manifest_backend]:-}"
            receipt_backend="${COLON_VALUES[harness_backend]:-}"
            receipt_batch="${COLON_VALUES[batch_size]:-}"
            receipt_warmup="${COLON_VALUES[machine_warmup]:-}"
            receipt_started="${COLON_VALUES[started_utc]:-}"
            receipt_finished="${COLON_VALUES[finished_utc]:-}"
            receipt_command="${COLON_VALUES[command]:-}"
            receipt_scratch="${COLON_VALUES[scratch_csv]:-}"
            receipt_status="${COLON_VALUES[status]:-}"
            receipt_exit="${COLON_VALUES[exit_status]:-}"
            receipt_failure="${COLON_VALUES[failure]:-}"
            [[ "${COLON_COUNTS[plan_sha256]:-0}" -eq 0 ]] \
                || receipt_plan_sha="${COLON_VALUES[plan_sha256]}"
            [[ "${COLON_COUNTS[manifest]:-0}" -eq 0 ]] \
                || receipt_manifest="${COLON_VALUES[manifest]}"
            if [[ "${COLON_COUNTS[prepared_manifest_sha256]:-0}" -ne 0 ]]; then
                receipt_prepared_manifest_sha="${COLON_VALUES[prepared_manifest_sha256]}"
            elif [[ "${COLON_COUNTS[manifest_sha256]:-0}" -ne 0 ]]; then
                receipt_prepared_manifest_sha="${COLON_VALUES[manifest_sha256]}"
            fi
        else
            reasons="missing_receipt"
        fi
        if [[ -f "$receipt/exit.status" ]]; then
            exit_file="$(<"$receipt/exit.status")"
        fi
        local receipt_exit_count="${COLON_COUNTS[exit_status]:-0}"
        local receipt_finished_count="${COLON_COUNTS[finished_utc]:-0}"
        local receipt_failure_count="${COLON_COUNTS[failure]:-0}"
        local interrupted_shape=none
        if [[ "$receipt_status" == failed && "$receipt_failure_count" -eq 1 \
            && "$receipt_failure" == 'interrupted by runner signal' \
            && "$exit_file" == 130 && "$receipt_exit_count" -eq 0 \
            && "$receipt_finished_count" -eq 0 ]]; then
            interrupted_shape=signal
        elif [[ "$receipt_status" == failed && "$receipt_failure_count" -eq 1 \
            && "$receipt_failure" == 'interrupted before final receipt' \
            && "$exit_file" == 125 && "$receipt_exit_count" -eq 1 \
            && "$receipt_exit" == 125 && "$receipt_finished_count" -eq 0 ]]; then
            interrupted_shape=orphan
        fi
        if [[ "$interrupted_shape" == none ]]; then
            if [[ "$receipt_exit_count" -ne 1 || -z "$receipt_exit" ]]; then
                receipt_valid=false
                reasons+="${reasons:+;}receipt_exit_status_count_or_value"
            fi
            if [[ "$receipt_finished_count" -ne 1 || -z "$receipt_finished" ]]; then
                receipt_valid=false
                reasons+="${reasons:+;}receipt_finished_utc_count_or_value"
            fi
        fi
        local terminal_exit_consistent=false
        if [[ "$exit_file" =~ ^[0-9]+$ ]]; then
            if [[ "$receipt_status" == completed && "$exit_file" -eq 0 ]] \
                || [[ "$receipt_status" == failed && "$exit_file" -ne 0 ]]; then
                terminal_exit_consistent=true
            fi
        fi
        if [[ -n "$receipt_exit" ]]; then
            receipt_exit_agreement=false
            if [[ "$receipt_exit" =~ ^[0-9]+$ && "$receipt_exit" == "$exit_file" \
                && "$terminal_exit_consistent" == true ]]; then
                receipt_exit_agreement=true
            else
                reasons+="${reasons:+;}receipt_exit_disagreement"
            fi
        elif [[ "$terminal_exit_consistent" != true ]]; then
            reasons+="${reasons:+;}missing_terminal_exit_fact"
        fi
        if [[ "$receipt_status" =~ ^(completed|failed)$ && "$exit_file" =~ ^[0-9]+$ ]]; then
            terminal["$config_key"]=$(( ${terminal["$config_key"]:-0} + 1 ))
        fi
        if [[ "$receipt_status" == completed && "$terminal_exit_consistent" == true ]]; then
            completed["$config_key"]=$(( ${completed["$config_key"]:-0} + 1 ))
        elif [[ "$receipt_status" == failed && "$terminal_exit_consistent" == true ]]; then
            censored["$config_key"]=$(( ${censored["$config_key"]:-0} + 1 ))
        fi

        parse_premeasure_scratch "$csv" "$backend"
        local scratch_present="$SCRATCH_PRESENT" scratch_valid="$SCRATCH_STRUCTURALLY_VALID"
        local scratch_q="${SCRATCH_DATA[q]:-}" scratch_n="${SCRATCH_DATA[n]:-}"
        local scratch_backend="${SCRATCH_DATA[backend]:-}" scratch_outcome="${SCRATCH_DATA[outcome]:-}"
        local scratch_batch="${SCRATCH_DATA[batch_size]:-}" scratch_reps="${SCRATCH_DATA[reps]:-}"
        local scratch_matrices="${SCRATCH_DATA[matrices]:-}" scratch_zeros="${SCRATCH_DATA[zeros]:-}"
        local scratch_total="${SCRATCH_DATA[total_s]:-}" scratch_gen="${SCRATCH_DATA[gen_s]:-}"
        local scratch_eval="${SCRATCH_DATA[eval_s]:-}" scratch_reduce="${SCRATCH_DATA[reduce_s]:-}"
        local scratch_store="${SCRATCH_DATA[store_s]:-}"
        local scratch_rate="${SCRATCH_DATA[composite_matrices_per_s]:-}"
        local scratch_eval_rate="${SCRATCH_DATA[eval_matrices_per_s]:-}"
        if [[ "$receipt_status" == completed && "$scratch_valid" != true ]]; then
            reasons+="${reasons:+;}$SCRATCH_REASONS"
        fi

        local session_resolution=absent resolved_session='' session_valid=false
        if [[ -n "$receipt_session" && "${COLON_COUNTS[session_id]:-0}" -eq 1 ]]; then
            case "${SESSION_ID_COUNT[$receipt_session]:-0}" in
                0) session_resolution=absent ;;
                1) session_resolution=exact; resolved_session="${SESSION_ID_PATH[$receipt_session]}" ;;
                *) session_resolution=ambiguous ;;
            esac
        elif [[ "$valid_session_files" -eq 1 ]]; then
            session_resolution=legacy_unique_fallback
            resolved_session="$only_session_file"
        elif [[ "$valid_session_files" -gt 1 ]]; then
            session_resolution=ambiguous
        fi
        local session_id='' session_campaign='' session_started='' session_source=''
        local session_plan_path='' session_plan_sha='' session_manifest=''
        local session_wrapper='' session_actual_chain='' session_expected_chain=''
        local session_chain_match=unavailable scratch_binary_match=unavailable
        local recovery_plan_match=unavailable
        if [[ -n "$resolved_session" ]]; then
            session_valid=true
            local required_session_key session_count session_value
            local -a required_session_keys=(
                session_id campaign_run_id started_utc repository_revision manifest exact_plan
                plan_sha256 wrapper_invocation
            )
            for required_session_key in "${required_session_keys[@]}"; do
                composite="$resolved_session"$'\034'"$required_session_key"
                session_count="${SESSION_COUNTS[$composite]:-0}"
                session_value="${SESSION_VALUES[$composite]:-}"
                if [[ "$session_count" -ne 1 || -z "$session_value" ]]; then
                    session_valid=false
                    reasons+="${reasons:+;}session_${required_session_key}_count_or_value"
                fi
            done
            composite="$resolved_session"$'\034'session_id
            session_id="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'campaign_run_id
            session_campaign="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'started_utc
            session_started="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'repository_revision
            session_source="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'exact_plan
            session_plan_path="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'plan_sha256
            session_plan_sha="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'manifest
            session_manifest="${SESSION_VALUES[$composite]:-}"
            composite="$resolved_session"$'\034'wrapper_invocation
            session_wrapper="${SESSION_VALUES[$composite]:-}"
            session_actual_chain="${SESSION_ACTUAL_CHAIN[$resolved_session]:-}"
            session_expected_chain="${SESSION_EXPECTED_CHAIN[$resolved_session]:-}"
            session_chain_match="${SESSION_CHAIN_MATCH[$resolved_session]:-unavailable}"
            if [[ -n "$session_plan_sha" ]]; then
                if [[ "$session_plan_sha" == "$recovery_plan_sha256" ]]; then
                    recovery_plan_match=true
                else
                    recovery_plan_match=false
                    reasons+="${reasons:+;}session_plan_mismatch"
                fi
            fi
            local observed_binary="${SCRATCH_META_VALUES[binary_sha256]:-}"
            if [[ "$scratch_valid" == true && -n "$observed_binary" \
                && "$observed_binary" != unavailable ]]; then
                local binary_matches=0 session_hash
                while IFS= read -r session_hash; do
                    [[ "$session_hash" == "$observed_binary" ]] \
                        && binary_matches=$((binary_matches + 1))
                done <<< "${SESSION_ACTUAL_HASHES[$resolved_session]:-}"
                case "$binary_matches" in
                    1) scratch_binary_match=true ;;
                    0) scratch_binary_match=false; reasons+="${reasons:+;}scratch_binary_session_mismatch" ;;
                    *) scratch_binary_match=ambiguous; reasons+="${reasons:+;}scratch_binary_session_ambiguous" ;;
                esac
            fi
            if [[ "$session_chain_match" == false ]]; then
                reasons+="${reasons:+;}session_binary_manifest_mismatch"
            elif [[ "$session_chain_match" == unavailable ]]; then
                reasons+="${reasons:+;}session_binary_chain_unavailable"
            fi
        else
            reasons+="${reasons:+;}session_$session_resolution"
        fi

        local identity_valid=true
        if [[ "$receipt_present" == true ]]; then
            local expected_value observed_value identity_name
            local -a identity_names=(run position q n code token backend batch scratch)
            local -a identity_expected=(
                "$RUN_ID" "$process_index" "$q" "$n" "$code" "$token" "$backend"
                "$batch" "$csv"
            )
            local -a identity_observed=(
                "$receipt_run" "$receipt_position" "$receipt_q" "$receipt_n"
                "$receipt_code" "$receipt_token" "$receipt_backend" "$receipt_batch"
                "$receipt_scratch"
            )
            local identity_index
            for ((identity_index=0; identity_index<${#identity_names[@]}; identity_index++)); do
                identity_name="${identity_names[$identity_index]}"
                expected_value="${identity_expected[$identity_index]}"
                observed_value="${identity_observed[$identity_index]}"
                if [[ -n "$observed_value" && "$observed_value" != "$expected_value" ]]; then
                    identity_valid=false
                    reasons+="${reasons:+;}receipt_${identity_name}_mismatch"
                fi
            done
        else
            identity_valid=false
        fi
        if [[ "$receipt_status" == completed && "$scratch_valid" == true ]]; then
            if [[ "$scratch_q" != "$q" || "$scratch_n" != "$n" \
                || "$scratch_backend" != "$backend" || "$scratch_batch" != "$batch" ]]; then
                identity_valid=false
                reasons+="${reasons:+;}scratch_plan_identity_mismatch"
            fi
        fi
        if [[ -n "$resolved_session" ]]; then
            if [[ -n "$receipt_session" && "$receipt_session" != "$session_id" ]]; then
                identity_valid=false
                reasons+="${reasons:+;}receipt_session_identity_mismatch"
            fi
            if [[ -n "$session_campaign" && "$session_campaign" != "$RUN_ID" ]]; then
                identity_valid=false
                reasons+="${reasons:+;}session_run_identity_mismatch"
            fi
            if [[ "$receipt_plan_sha" != unavailable && "$receipt_plan_sha" != "$session_plan_sha" ]]; then
                identity_valid=false
                reasons+="${reasons:+;}receipt_plan_session_mismatch"
            fi
        fi

        local provenance_complete=true observed_key
        if [[ "$receipt_status" == completed && "$scratch_valid" == true ]]; then
            local -a observed_keys=(
                git_sha git_worktree_dirty harness_source_sha harness_source_dirty
                deps_source_sha deps_source_dirty binary_sha256 rustc cargo cpu logical_cpus
                rayon_threads avx2 cpu_governor gpu rocm hip_feature kernel timestamp_utc
                invocation
            )
            for observed_key in "${observed_keys[@]}"; do
                if [[ "${SCRATCH_META_VALUES[$observed_key]:-}" == unavailable ]]; then
                    provenance_complete=false
                fi
            done
        fi
        local row_valid=false
        if [[ "$receipt_valid" == true && "$terminal_exit_consistent" == true \
            && "$receipt_exit_agreement" != false \
            && "$session_valid" == true && "$identity_valid" == true \
            && "$recovery_plan_match" == true && "$session_chain_match" == true ]]; then
            if [[ "$receipt_status" == failed ]]; then
                row_valid=true
            elif [[ "$receipt_status" == completed && "$scratch_valid" == true \
                && "$scratch_binary_match" == true ]]; then
                row_valid=true
            fi
        fi
        local candidate_emitted=false
        if [[ "$receipt_status" == completed && "$scratch_valid" == true ]]; then
            candidate_emitted=true
            candidates["$config_key"]=$(( ${candidates["$config_key"]:-0} + 1 ))
        fi
        local ledger_state=structurally_invalid
        if [[ "$receipt_present" == false ]]; then
            ledger_state=pending_missing
        elif [[ "$row_valid" == true && "$receipt_status" == completed ]]; then
            ledger_state=completed
        elif [[ "$row_valid" == true && "$receipt_status" == failed ]]; then
            ledger_state=censored
        fi
        if [[ "$row_valid" != true ]]; then
            invalid["$config_key"]=$(( ${invalid["$config_key"]:-0} + 1 ))
        fi
        if [[ "$ledger_state" != completed ]]; then
            any_incomplete=true
        fi
        local raw_file="$receipt_file"
        [[ "$scratch_present" == false ]] || raw_file="$csv"
        local -a collector_row=(
            ledger "$process_index" "$process_index" "$raw_file" "$receipt_file" "$csv"
            "q${q}_n${n}" "$code" "$q" "$n" "$token" "$backend" "$batch"
            "$ledger_state" "$candidate_emitted" "$receipt_present" "$receipt_valid"
            "$scratch_present" "$scratch_valid" "$session_resolution" "$identity_valid"
            "$provenance_complete" "$row_valid" "$reasons" "$receipt_schema"
            "$receipt_run" "$receipt_session" "$receipt_position" "$receipt_q" "$receipt_n"
            "$receipt_code" "$receipt_token" "$receipt_backend" "$receipt_batch"
            "$receipt_warmup" "$receipt_started" "$receipt_finished" "$receipt_command"
            "$receipt_scratch" "$receipt_status" "$receipt_exit" "$exit_file"
            "$receipt_exit_agreement" "$receipt_failure" "$receipt_plan_sha"
            "$receipt_manifest" "$receipt_prepared_manifest_sha" "$resolved_session"
            "$session_id" "$session_campaign" "$session_started" "$session_source"
            "$session_plan_path" "$session_plan_sha" "$session_manifest" "$session_wrapper"
            "$session_actual_chain" "$session_expected_chain" "$session_chain_match"
            "$scratch_binary_match" "$recovery_plan_sha256" "$recovery_plan_match"
            "$scratch_q" "$scratch_n" "$scratch_backend" "$scratch_outcome" "$scratch_batch"
            "$scratch_reps" "$scratch_matrices" "$scratch_zeros" "$scratch_total"
            "$scratch_gen" "$scratch_eval" "$scratch_reduce" "$scratch_store" "$scratch_rate"
            "$scratch_eval_rate" "${SCRATCH_META_VALUES[git_sha]:-}"
            "${SCRATCH_META_VALUES[git_worktree_dirty]:-}"
            "${SCRATCH_META_VALUES[harness_source_sha]:-}"
            "${SCRATCH_META_VALUES[harness_source_dirty]:-}"
            "${SCRATCH_META_VALUES[deps_source_sha]:-}"
            "${SCRATCH_META_VALUES[deps_source_dirty]:-}"
            "${SCRATCH_META_VALUES[binary_sha256]:-}" "${SCRATCH_META_VALUES[rustc]:-}"
            "${SCRATCH_META_VALUES[cargo]:-}" "${SCRATCH_META_VALUES[cpu]:-}"
            "${SCRATCH_META_VALUES[logical_cpus]:-}" "${SCRATCH_META_VALUES[rayon_threads]:-}"
            "${SCRATCH_META_VALUES[avx2]:-}" "${SCRATCH_META_VALUES[cpu_governor]:-}"
            "${SCRATCH_META_VALUES[gpu]:-}" "${SCRATCH_META_VALUES[rocm]:-}"
            "${SCRATCH_META_VALUES[hip_feature]:-}" "${SCRATCH_META_VALUES[kernel]:-}"
            "${SCRATCH_META_VALUES[timestamp_utc]:-}" "${SCRATCH_META_VALUES[invocation]:-}"
        )
        csv_write_row "$ledger_tmp" "${collector_row[@]}"
        if [[ "$candidate_emitted" == true ]]; then
            collector_row[0]=candidate_execution
            csv_write_row "$candidates_tmp" "${collector_row[@]}"
        fi
    done
    local key count
    for key in "${PREMEASURE_CONFIG_KEYS[@]}"; do
        for code in A B; do
            local summary_key="$key:$code"
            echo "completeness $summary_key: expected=${expected[$summary_key]:-0} completed=${completed[$summary_key]:-0} terminal=${terminal[$summary_key]:-0} invalid=${invalid[$summary_key]:-0} censored=${censored[$summary_key]:-0} candidates=${candidates[$summary_key]:-0}"
        done
    done
    mv -f "$ledger_tmp" "$ledger_path"
    mv -f "$candidates_tmp" "$candidates_path"
    PREMEASURE_COLLECT_TMP_FILES=()
    trap - EXIT
    echo "wrote $ledger_path"
    echo "wrote $candidates_path"
    [[ "$any_incomplete" == false ]] || return "$CENSORED_EXIT"
}

run_premeasure() {
    local cap="$1"
    verify_manifest
    mkdir -p "$TARGET_ROOT"
    CAMPAIGN_HARNESS_BIN="$HARNESS_BIN" \
        "$FLOCK_WRAPPER" --full-host "$BASH" "$SCRIPT_PATH" __locked-premeasure "$cap"
}

revalidate_premeasure_under_lock() {
    require_command git
    verify_manifest
}

# Re-runs the prepared-binary check inside the lock-held child, before any step.
# The canonical mutex is shared with every other worker on this host and a run
# can sit on it for a long time; a rebuilt binary landing during that wait must
# still be refused. smoke deliberately skips this revalidation because its
# outputs are plumbing evidence rather than campaign evidence.
revalidate_under_lock() {
    local smoke="$1"
    if [[ "$smoke" == true ]]; then
        return 0
    fi
    require_command git
    verify_manifest
}

if [[ "${1:-}" == "__locked-pipeline" ]]; then
    [[ $# -eq 2 ]] || die "internal pipeline invocation has wrong arity"
    revalidate_under_lock "$2"
    run_locked_pipeline "$2"
    exit $?
fi

if [[ "${1:-}" == "__locked-premeasure" ]]; then
    [[ $# -eq 2 ]] || die "internal premeasure invocation has wrong arity"
    revalidate_premeasure_under_lock
    run_locked_premeasure "$2"
    exit $?
fi

parse_session_cap() {
    local cap="$PREMEASURE_DEFAULT_SESSION_CAP"
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --session-cap)
                [[ $# -ge 2 ]] || die "--session-cap requires seconds"
                cap="$2"
                shift 2
                ;;
            *) die "unknown premeasure option: $1" ;;
        esac
    done
    [[ "$cap" =~ ^[1-9][0-9]*$ ]] || die "session cap must be a positive integer number of seconds"
    printf '%s\n' "$cap"
}

case "${1:-}" in
    --help|-h|"") usage; exit 0 ;;
    prepare) prepare ;;
    smoke)
        require_command git
        verify_manifest
        run_campaign true
        ;;
    measure)
        # Fail fast before queueing for the host mutex; revalidate_under_lock
        # repeats the prepared-binary check once the lock is held.
        require_command git
        verify_manifest
        run_campaign false
        ;;
    premeasure)
        require_command git
        require_command sha256sum
        verify_manifest
        run_premeasure "$(parse_session_cap "${@:2}")"
        ;;
    premeasure-collect)
        require_command sha256sum
        premeasure_collect
        ;;
    *) usage >&2; exit 2 ;;
esac
