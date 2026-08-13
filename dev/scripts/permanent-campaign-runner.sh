#!/usr/bin/env bash
# permanent-campaign-runner.sh — deterministic overnight receipt campaigns.
#
# One-shot scheduling (2026-08-14 02:00, canonical repository):
#   systemd-run --user --on-calendar='2026-08-14 02:00' --unit=gf2-permanent-campaign.service /bin/bash -lc '/home/vkaskivuo/Projects/gf2/dev/scripts/permanent-campaign-runner.sh measure >> /home/vkaskivuo/Projects/gf2/dev/studies/permanent-campaign-systemd.log 2>&1'
#   systemctl --user status gf2-permanent-campaign.service
#   systemctl --user stop gf2-permanent-campaign.service
#
# at(1) alternative:
#   echo '/home/vkaskivuo/Projects/gf2/dev/scripts/permanent-campaign-runner.sh measure >> /home/vkaskivuo/Projects/gf2/dev/studies/permanent-campaign-at.log 2>&1' | at 02:00 2026-08-14

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
    git -C "$REPO_ROOT" diff --quiet && git -C "$REPO_ROOT" diff --cached --quiet
}

assert_tracked_worktree_clean() {
    if ! tracked_worktree_clean; then
        echo "ERROR: measure requires a clean tracked worktree" >&2
        git -C "$REPO_ROOT" status --short --untracked-files=no >&2
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
            ''|manifest_version=*|source_revision=*|tracked_worktree_dirty=*|resource_receipt=*) ;;
            *) die "unknown manifest line: $kind|$path|$expected" ;;
        esac
    done < "$MANIFEST_PATH"
    [[ -n "$HARNESS_BIN" ]] || die "manifest does not name a harness binary"
    [[ -x "$HARNESS_BIN" ]] || die "manifest harness is not executable: $HARNESS_BIN"
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
        echo "tracked_dirty_check: git diff --quiet && git diff --cached --quiet"
        echo "binary_hashes: see $MANIFEST_PATH and the harness CSV preambles"
        print_manifest_binary_hashes
        echo
        echo "# The harness CSV preamble is the canonical source for facts it embeds."
        echo "# This block records command outputs rather than a second hand-written inventory."
        capture_block "cpu_model_command: lscpu" lscpu
        capture_block "gpu_model_uuid_command: $ROCM_PATH/bin/rocm-smi --showproductname --showuniqueid" "$ROCM_PATH/bin/rocm-smi" --showproductname --showuniqueid
        capture_block "rocm_hipcc_version_command: $ROCM_PATH/bin/hipcc --version" "$ROCM_PATH/bin/hipcc" --version
        capture_block "amd_clang_version_command: $ROCM_PATH/llvm/bin/clang --version" "$ROCM_PATH/llvm/bin/clang" --version
        capture_block "rustc_version_command: rustc -V" rustc -V
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

prepare() {
    require_command cargo
    require_command hipcc
    require_command sha256sum
    require_command git
    mkdir -p "$SAMPLING_TARGET_DIR" "$WAVE_TARGET_DIR" "$TARGET_ROOT"
    echo "building permanent-sampling-feas (HIP)"
    cargo +1.95.0 build --manifest-path "$SAMPLING_MANIFEST" --release --features hip --target-dir "$SAMPLING_TARGET_DIR"
    echo "building permanent_wave_gpu (HIP)"
    cargo +1.95.0 build --manifest-path "$WAVE_MANIFEST" --release --features hip --target-dir "$WAVE_TARGET_DIR"

    local resource_root="$TARGET_ROOT/hip-resource-usage-$RUN_ID"
    local source object log status source_hash object_hash log_hash
    mkdir -p "$resource_root"
    {
        echo "schema_version: 1"
        echo "source_revision: $(git -C "$REPO_ROOT" rev-parse HEAD)"
        echo "tracked_worktree_dirty=$(if tracked_worktree_clean; then echo false; else echo true; fi)"
        echo "architecture: $ARCH"
        echo "hipcc: $ROCM_PATH/bin/hipcc"
        echo "resource_flag: -Rpass-analysis=kernel-resource-usage"
    } > "$resource_root/receipt.txt"
    while IFS= read -r source; do
        source_hash=$(hash_file "$source")
        object="$resource_root/$(basename "$source").o"
        log="$resource_root/$(basename "$source").resource.log"
        echo "capturing resource usage: $source"
        set +e
        "$ROCM_PATH/bin/hipcc" "--offload-arch=$ARCH" -O3 -Rpass-analysis=kernel-resource-usage -c "$source" -o "$object" 2> "$log"
        status=$?
        set -e
        [[ "$status" -eq 0 ]] || die "resource capture failed for $source (exit $status)"
        object_hash=$(hash_file "$object")
        log_hash=$(hash_file "$log")
        {
            printf 'source: %s\nsource_sha256: %s\n' "$source" "$source_hash"
            printf 'command: %q ' "$ROCM_PATH/bin/hipcc" "--offload-arch=$ARCH" -O3 -Rpass-analysis=kernel-resource-usage -c "$source" -o "$object"
            printf '\nexit_status: %s\nobject: %s\nobject_sha256: %s\nresource_log: %s\nresource_log_sha256: %s\n\n' "$status" "$object" "$object_hash" "$log" "$log_hash"
        } >> "$resource_root/receipt.txt"
    done < <(find "$REPO_ROOT/dev/research/permanent_wave_gpu/hip" -type f -name '*.hip' -print | sort)

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
        echo "resource_receipt=$resource_root/receipt.txt"
        echo "harness|$SAMPLING_TARGET_DIR/release/permanent_sampling_feas|"
        for binary in "${binaries[@]}"; do
            [[ -x "$binary" ]] || die "expected prepared binary missing: $binary"
            printf 'binary|%s|%s\n' "$binary" "$(hash_file "$binary")"
        done
    } > "$manifest_tmp"
    mv "$manifest_tmp" "$MANIFEST_PATH"
    echo "prepared manifest: $MANIFEST_PATH"
    grep -E '^(manifest_version|source_revision|resource_receipt|binary\|)' "$MANIFEST_PATH"
}

run_step() {
    local q="$1" step="$2" execution_id="$3" out="$4" log="$5" summary="$6" provenance="$7" smoke="$8" skip_warmup="$9"
    local -a command=("$HARNESS_BIN" "$step" --out "$out" --execution-id "$execution_id")
    case "$step" in
        equivalence) [[ "$smoke" == true ]] && command+=(--matrices 1) ;;
        grid)
            if [[ "$smoke" == true ]]; then command+=(--only "q=$q,n=12"); else command+=(--only "q=$q"); fi
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
    # Harness gap: only grid parses --execution-id, --skip-machine-warmup, and
    # --only; the permissive other modes retain these flags in their invocation
    # preamble for the lock-held chain.
    [[ "$skip_warmup" == true ]] && command+=(--skip-machine-warmup)
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
    elif [[ "$rc" -eq 0 ]]; then
        status=completed
    else
        status=failed
    fi
    RUN_STEP_STATUS="$status"
    printf 'q=%s step=%s execution_id=%s status=%s exit=%s log=%s out=%s\n' "$q" "$step" "$execution_id" "$status" "$rc" "$log" "$out" >> "$summary"
    if [[ "$status" != completed ]]; then FAILURE_COUNT=$((FAILURE_COUNT + 1)); fi
}

record_equivalence_skip() {
    local q="$1" step="$2" execution_id="$3" summary="$4"
    printf 'q=%s step=%s execution_id=%s status=skipped_equivalence_failed exit=- log=- out=-\n' \
        "$q" "$step" "$execution_id" >> "$summary"
}

run_locked_pipeline() {
    local smoke="$1"
    CAMPAIGN_MODE="$([[ "$smoke" == true ]] && echo smoke || echo measure)"
    export CAMPAIGN_MODE
    local q study run_dir summary provenance base step_index execution_id skip_warmup equivalence_failed
    local first_grid=true
    FAILURE_COUNT=0
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
        if [[ "$smoke" == true ]]; then base=$((q * 1000 + 10000)); else base=$((q * 1000)); fi
        step_index=0
        equivalence_failed=false
        for step in equivalence grid gray-update horizontal-product; do
            step_index=$((step_index + 1))
            execution_id=$((base + step_index))
            if [[ "$equivalence_failed" == true ]]; then
                # The harness cannot exclude one backend from the remaining
                # timing modes reliably, so censor the whole field after a
                # failed equivalence check.
                record_equivalence_skip "$q" "$step" "$execution_id" "$summary"
                continue
            fi
            skip_warmup=false
            [[ "$first_grid" == false ]] && skip_warmup=true
            run_step "$q" "$step" "$execution_id" \
                "$run_dir/permanent-campaign-$RUN_ID-q$q-$step.csv" \
                "$run_dir/permanent-campaign-$RUN_ID-q$q-$step.log" \
                "$summary" "$provenance" "$smoke" "$skip_warmup"
            if [[ "$step" == equivalence && "$RUN_STEP_STATUS" == failed ]]; then
                equivalence_failed=true
            fi
            if [[ "$step" == grid ]]; then
                # grid owns the 90 s warm-up. Once invoked, later executions
                # under this held lock use --skip-machine-warmup as usage.txt
                # directs; the lock couples the chain to that thermal state.
                first_grid=false
            fi
        done
        echo "summary: $summary"
    done
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
