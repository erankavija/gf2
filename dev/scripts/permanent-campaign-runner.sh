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
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

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
RUN_ID="${CAMPAIGN_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"

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
Usage: permanent-campaign-runner.sh <prepare|smoke|measure>

Subcommands:
  prepare  Build both HIP harnesses, capture kernel resource receipts, and
           write the binary-hash manifest used by measure and smoke.
  smoke    Run the complete four-step pipeline for q=3,5,7 with tiny inputs;
           outputs are isolated below each study's smoke/ directory.
  measure  Refuse tracked worktree changes or hash drift, then run the
           overnight receipt campaign while holding the canonical full-host
           benchmark mutex for the entire run.
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
        equivalence) [[ "$smoke" == true ]] && command+=(--matrices 1) ;;
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
        gray-update)
            command+=(--q "$q")
            [[ "$smoke" == true ]] && command+=(--n 1 --steps 1)
            ;;
        horizontal-product)
            command+=(--q "$q")
            [[ "$smoke" == true ]] && command+=(--n 1 --samples 1)
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

if [[ "${1:-}" == "__locked-pipeline" ]]; then
    [[ $# -eq 2 ]] || die "internal pipeline invocation has wrong arity"
    run_locked_pipeline "$2"
    exit $?
fi

case "${1:-}" in
    --help|-h|"") usage; exit 0 ;;
    prepare) prepare ;;
    smoke)
        require_command git
        verify_manifest
        run_campaign true
        ;;
    measure)
        require_command git
        assert_tracked_worktree_clean
        verify_manifest
        run_campaign false
        ;;
    *) usage >&2; exit 2 ;;
esac
