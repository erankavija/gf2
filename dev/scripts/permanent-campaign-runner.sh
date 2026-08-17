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
  measure  Refuse tracked worktree changes or hash drift, then run the
           overnight receipt campaign while holding the canonical full-host
           benchmark mutex for the entire run.
  premeasure [--session-cap SECONDS]
           Run the resumable 60-cell premeasurement schedule under one
           canonical full-host lock. The default session cap is 43200 seconds.
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
    git -C "$REPO_ROOT" diff --quiet \
        && git -C "$REPO_ROOT" diff --cached --quiet \
        && [[ -z "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]
}

assert_tracked_worktree_clean() {
    if ! tracked_worktree_clean; then
        echo "ERROR: measure requires a clean worktree (tracked and untracked)" >&2
        git -C "$REPO_ROOT" status --short --untracked-files=all >&2
        exit 2
    fi
}

assert_premeasure_worktree_clean() {
    if ! tracked_worktree_clean; then
        echo "ERROR: premeasure requires a clean worktree (tracked and untracked)" >&2
        git -C "$REPO_ROOT" status --short --untracked-files=all >&2
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
            ''|manifest_version=*|source_revision=*|tracked_worktree_dirty=*|resource_receipt=*|rust_toolchain=*|build_rustc=*) ;;
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
        echo "source_revision: $(git -C "$REPO_ROOT" rev-parse HEAD)"
        if tracked_worktree_clean; then echo "tracked_worktree_dirty: false"; else echo "tracked_worktree_dirty: true"; fi
        echo "tracked_dirty_check: git diff --quiet && git diff --cached --quiet && git status --porcelain --untracked-files=all"
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
    mkdir -p "$SAMPLING_TARGET_DIR" "$WAVE_TARGET_DIR" "$TARGET_ROOT"
    echo "building permanent-sampling-feas (HIP)"
    cargo +1.95.0 build --manifest-path "$SAMPLING_MANIFEST" --release --features hip --target-dir "$SAMPLING_TARGET_DIR"
    echo "building permanent_wave_gpu (HIP)"
    cargo +1.95.0 build --manifest-path "$WAVE_MANIFEST" --release --features hip --target-dir "$WAVE_TARGET_DIR"

    local resource_root="$TARGET_ROOT/hip-resource-usage-$RUN_ID"
    mkdir -p "$resource_root"
    {
        echo "schema_version: 1"
        echo "source_revision: $(git -C "$REPO_ROOT" rev-parse HEAD)"
        echo "tracked_worktree_dirty=$(if tracked_worktree_clean; then echo false; else echo true; fi)"
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
        echo "source_revision=$(git -C "$REPO_ROOT" rev-parse HEAD)"
        echo "tracked_worktree_dirty=$(if tracked_worktree_clean; then echo false; else echo true; fi)"
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
    grep -E '^(manifest_version|source_revision|rust_toolchain|build_rustc|resource_receipt|binary\|)' "$MANIFEST_PATH"
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
}

premeasure_run_dir() {
    printf '%s\n' "$TARGET_ROOT/premeasure-$RUN_ID"
}

write_premeasure_provenance() {
    local run_dir="$1" session_id="$2" warmup_state="$3" cap="$4"
    local provenance="$run_dir/sessions/session-$session_id.provenance.txt"
    mkdir -p "$run_dir/sessions"
    {
        echo "schema_version: 1"
        echo "campaign_run_id: $RUN_ID"
        echo "session_id: $session_id"
        echo "mode: premeasure"
        echo "started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "source_revision: $(git -C "$REPO_ROOT" rev-parse HEAD)"
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

run_premeasure_process() {
    local run_dir="$1" session_id="$2" process_index="$3" q="$4" n="$5" code="$6" token="$7" backend="$8" batch="$9" warmup="${10}"
    local receipt
    receipt="$run_dir/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
    local csv="$receipt/scratch.csv" log="$receipt/harness.log" rc status skip=false
    mkdir -p "$receipt"
    PREMEASURE_CURRENT_RECEIPT="$receipt"
    if [[ "$warmup" == skipped ]]; then skip=true; fi
    local -a command=("$HARNESS_BIN" grid --out "$csv" --only "q=$q,n=$n,backend=$backend" --batch-size "$batch" --execution-id "$process_index")
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
        if premeasure_process_is_final "$receipt"; then
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

PREMEASURE_COLLECT_HEADER="record_type,process_index,schedule_position,raw_file,config_id,config_code,q,n,backend,batch_size,outcome,fresh_process,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s,timestamp_utc,machine_warmup,lock_mode,lock_path,git_sha,binary_sha256,exit_status,status"

premeasure_collect() {
    load_premeasure_schedule
    local run_dir tmp row q n code token backend batch process_index receipt csv status data
    local -A completed failures
    run_dir=$(premeasure_run_dir)
    [[ -d "$run_dir/processes" ]] || die "premeasurement run directory not found: $run_dir"
    tmp="$run_dir/premeasure-candidates.csv.tmp.$$"
    printf '%s\n' "$PREMEASURE_COLLECT_HEADER" > "$tmp"
    local source_revision binary_hash
    source_revision="$(git -C "$REPO_ROOT" rev-parse HEAD)"
    binary_hash="$(hash_file "$HARNESS_BIN")"
    for row in "${PREMEASURE_SCHEDULE[@]}"; do
        IFS=, read -r q n code token backend batch process_index <<< "$row"
        receipt="$run_dir/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
        if ! premeasure_process_is_final "$receipt"; then
            failures["$q:$n:$code"]="missing or interrupted receipt"
            continue
        fi
        status=$(sed -n 's/^status: //p' "$receipt/receipt.txt" | tail -n 1)
        if [[ "$status" != completed ]]; then
            failures["$q:$n:$code"]="exit $(cat "$receipt/exit.status")"
            continue
        fi
        csv="$receipt/scratch.csv"
        data=$(grep -v '^#' "$csv" | tail -n 1)
        [[ -n "$data" ]] || { failures["$q:$n:$code"]="completed without a data row"; continue; }
        local hq hn hbackend houtcome hm hrep hmat hzeros htotal hgen heval hreduce hstore hrate hevalsec rest started warmup
        IFS=, read -r hq hn hbackend houtcome hm hrep hmat hzeros htotal hgen heval hreduce hstore hrate hevalsec rest <<< "$data"
        completed["$q:$n:$code"]=$(( ${completed["$q:$n:$code"]:-0} + 1 ))
        started=$(sed -n 's/^started_utc: //p' "$receipt/receipt.txt" | tail -n 1)
        warmup=$(sed -n 's/^machine_warmup: //p' "$receipt/receipt.txt" | tail -n 1)
        printf 'execution,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,True,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,ccx1-bench-flock --full-host,/tmp/gf2-ccx1.lock,%s,%s,%s,completed\n' \
            "$process_index" "$process_index" "$csv" "q${q}_n${n}" "$code" "$hq" "$hn" "$hbackend" "$hm" "$houtcome" "$hrep" "$hmat" "$hzeros" "$htotal" "$hgen" "$heval" "$hreduce" "$hstore" "$hrate" "$hevalsec" "$started" "$warmup" "$source_revision" "$binary_hash" "$(cat "$receipt/exit.status")" >> "$tmp"
    done
    local zero=0 key count
    for key in "${PREMEASURE_CONFIG_KEYS[@]}"; do
        for code in A B; do
            local config_key
            config_key="$key:$code"
            count="${completed[$config_key]:-0}"
            if [[ -n "${failures[$config_key]:-}" ]]; then
                echo "completeness $config_key: $count/12 failures=${failures[$config_key]}"
            else
                echo "completeness $config_key: $count/12"
            fi
            if [[ "$count" -eq 0 ]]; then zero=$((zero + 1)); fi
        done
    done
    if [[ "$zero" -ne 0 ]]; then
        rm -f "$tmp"
        die "refusing to aggregate $zero configuration(s) with zero completed processes"
    fi
    mv "$tmp" "$run_dir/premeasure-candidates.csv"
    echo "wrote $run_dir/premeasure-candidates.csv"
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
    assert_premeasure_worktree_clean
    verify_manifest
}

# Re-runs measure's refusals inside the lock-held child, before any step. The
# pre-lock checks fail fast without waiting, but the canonical mutex is shared
# with every other worker on this host and a run can sit on it for a long time;
# a commit, an untracked file, or a rebuilt binary landing during that wait
# would otherwise reach the campaign unchecked (REQ-02). smoke deliberately
# tolerates a dirty tree — its outputs are plumbing evidence, not campaign
# evidence — so it revalidates nothing here.
revalidate_under_lock() {
    local smoke="$1"
    if [[ "$smoke" == true ]]; then
        return 0
    fi
    require_command git
    assert_tracked_worktree_clean
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
        # repeats both refusals once the lock is held.
        require_command git
        assert_tracked_worktree_clean
        verify_manifest
        run_campaign false
        ;;
    premeasure)
        require_command git
        require_command sha256sum
        assert_premeasure_worktree_clean
        verify_manifest
        run_premeasure "$(parse_session_cap "${@:2}")"
        ;;
    premeasure-collect)
        require_command git
        require_command sha256sum
        verify_manifest
        premeasure_collect
        ;;
    *) usage >&2; exit 2 ;;
esac
