#!/usr/bin/env bash
set -u
set -o pipefail

# Regenerate the narrow architecture compile receipt. The default claimed
# scope is intentionally gfx1030; extra targets are recorded as attempts but
# never become supported merely because they compile.
crate_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
repo_dir=$(cd "$crate_dir/../../.." && pwd)
rocm_path=${ROCM_PATH:-/opt/rocm}
evidence_arches_csv=${PERMANENT_WAVE_GPU_EVIDENCE_ARCHES:-gfx1030}
source_revision=$(git -C "$repo_dir" rev-parse HEAD)
worktree_status=$(git -C "$repo_dir" status --porcelain=v1 --untracked-files=all)
if [[ -n "$worktree_status" ]]; then
    echo "architecture evidence requires a fully clean tracked+untracked worktree:" >&2
    printf '%s\n' "$worktree_status" >&2
    exit 2
fi
worktree_status_sha256=$(printf '%s' "$worktree_status" | sha256sum | awk '{print $1}')

# gfx1030 is the claimed/measured baseline. An override can add attempts, but
# cannot accidentally produce a receipt that omits the baseline attempt.
if [[ ",$evidence_arches_csv," != *,gfx1030,* ]]; then
    evidence_arches_csv="gfx1030,$evidence_arches_csv"
fi

output_root=$(mktemp -d "${TMPDIR:-/tmp}/permanent-wave-gpu-evidence.XXXXXX")
target_root="$output_root/targets"
mkdir -p "$target_root"
receipt_tmp="$output_root/architecture_compile_evidence-v1.md"
log_tmp="$output_root/architecture_compile_evidence-v1.log"
trap 'rm -rf "$output_root"' EXIT

{
    echo "# Permanent-wave GPU architecture compile evidence"
    echo
    echo "schema_version: 1"
    echo "source_revision: $source_revision"
    echo "worktree_clean: true"
    echo "worktree_status_command: git status --porcelain=v1 --untracked-files=all"
    echo "worktree_status: empty"
    echo "worktree_status_sha256: $worktree_status_sha256"
    echo "rocm_path: $rocm_path"
    echo "claimed_architectures: gfx1030"
    echo
    echo "## Toolchain"
    "$rocm_path/bin/hipcc" --version || true
    cargo +1.95.0 --version || true
    rustc +1.95.0 --version --verbose || true
    echo
    echo "## HIP source inventory"
    while IFS= read -r source; do
        sha256sum "$crate_dir/$source"
    done < <(cd "$crate_dir" && find hip -type f -name '*.hip' -print | sort)
    echo
    echo "## Architecture attempts"
} > "$receipt_tmp"
: > "$log_tmp"

IFS=',' read -r -a arches <<< "$evidence_arches_csv"
seen_arches=()
for raw_arch in "${arches[@]}"; do
    arch=${raw_arch//[[:space:]]/}
    if [[ ! "$arch" =~ ^gfx[0-9A-Za-z]+$ ]]; then
        echo "invalid architecture: $raw_arch" >&2
        exit 2
    fi
    already_seen=false
    for seen_arch in "${seen_arches[@]}"; do
        if [[ "$seen_arch" == "$arch" ]]; then
            already_seen=true
            break
        fi
    done
    if [[ "$already_seen" == true ]]; then
        continue
    fi
    seen_arches+=("$arch")

    arch_log="$target_root/$arch.log"
    {
        echo "===== $arch ====="
        printf 'command_q: PERMANENT_WAVE_GPU_OFFLOAD_ARCHES=%q CARGO_TARGET_DIR=%q' "$arch" "$target_root/$arch"
        printf ' %q' cargo +1.95.0 build --manifest-path "$crate_dir/Cargo.toml" --release --features hip -vv
        printf '\n'
        set +e
        PERMANENT_WAVE_GPU_OFFLOAD_ARCHES="$arch" \
            CARGO_TARGET_DIR="$target_root/$arch" \
            cargo +1.95.0 build \
            --manifest-path "$crate_dir/Cargo.toml" --release --features hip -vv
        status=$?
        set -e
        echo "exit_status: $status"
        if [[ "$status" -eq 0 ]]; then
            echo "outcome: passed (compile evidence only)"
        else
            echo "outcome: failed; excluded from supported scope"
        fi
    } > "$arch_log" 2>&1
    cat "$arch_log" >> "$log_tmp"
    printf '%s\n' "- architecture: $arch" >> "$receipt_tmp"
    if grep -q '^outcome: passed' "$arch_log"; then
        echo "  outcome: passed (compile evidence only)" >> "$receipt_tmp"
    else
        echo "  outcome: failed; excluded_from_supported_scope: true" >> "$receipt_tmp"
    fi
    echo "  raw_log: hip/architecture_compile_evidence-v1.log" >> "$receipt_tmp"
done

raw_log_sha256=$(sha256sum "$log_tmp" | awk '{print $1}')
cat >> "$receipt_tmp" <<EOF

raw_log_sha256: $raw_log_sha256

The claimed set is not widened automatically by this receipt. A failed
attempt remains part of the raw log, including the exact toolchain diagnostic.
Compile success does not establish runtime portability or performance.
EOF

# Publish canonical files only after all attempts and provenance hashes exist.
mkdir -p "$crate_dir/hip"
mv "$receipt_tmp" "$crate_dir/architecture_compile_evidence-v1.md"
mv "$log_tmp" "$crate_dir/hip/architecture_compile_evidence-v1.log"
echo "wrote $crate_dir/architecture_compile_evidence-v1.md and $crate_dir/hip/architecture_compile_evidence-v1.log"
