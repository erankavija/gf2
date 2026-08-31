#!/usr/bin/env bash
# One-shot continuation of the frozen 02b8137c validation journal.
set -uo pipefail

: "${GF2_VALIDATION_EXPECTED_HEAD:?missing pinned execution revision}"
: "${GF2_VALIDATION_EXPECTED_DEPS_REVISION:?missing pinned dependency-source revision}"
: "${GF2_VALIDATION_EXPECTED_BINARY_SHA256:?missing validator binary identity}"
: "${GF2_VALIDATION_LOG:?missing durable log destination}"

SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIRECTORY
REPOSITORY="$(git -C "$SCRIPT_DIRECTORY" rev-parse --show-toplevel)"
readonly REPOSITORY
readonly BINARY="$REPOSITORY/target/release/permanent_validation"
readonly AUTHORIZATION="dev/active/02b8137c/pre-draw-validation-v2-continuation.json"
readonly JOURNAL="dev/active/02b8137c/validation-journal"
readonly RECEIPT="dev/active/02b8137c/pre-draw-validation-v2-receipt.json"
readonly EXPECTED_AUTHORIZATION_SHA256="dd19bb60bf07510085ab1ec06581526df710f39dbea1236e2d67273b41186094"
readonly EXPECTED_PRODUCER0_STATE_SHA256="bd108332375684992b5be6830bf278c1dd8281cb1d4e1f272d0982e30c9d31a0"

mkdir -p "$(dirname "$GF2_VALIDATION_LOG")"
exec >>"$GF2_VALIDATION_LOG" 2>&1
cd "$REPOSITORY" || exit 3

abort() {
    echo "ABORT: $*" >&2
    exit 3
}

observed_sha256() {
    sha256sum "$1" | awk '{print $1}'
}

echo "=== $(date -Is) validation continuation wrapper start ==="
echo "repository: $REPOSITORY"

[[ "$(git rev-parse HEAD)" == "$GF2_VALIDATION_EXPECTED_HEAD" ]] ||
    abort "execution revision changed"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
    abort "pinned execution worktree is dirty"
[[ "$(git log -1 --format=%H -- crates/ Cargo.lock)" == "$GF2_VALIDATION_EXPECTED_DEPS_REVISION" ]] ||
    abort "dependency-source revision changed"
[[ -x "$BINARY" ]] || abort "validator binary is missing"
[[ "$(observed_sha256 "$BINARY")" == "$GF2_VALIDATION_EXPECTED_BINARY_SHA256" ]] ||
    abort "validator binary identity changed"
[[ "$(observed_sha256 "$AUTHORIZATION")" == "$EXPECTED_AUTHORIZATION_SHA256" ]] ||
    abort "continuation authorization identity changed"
[[ "$(observed_sha256 "$JOURNAL/run-state.json")" == "$EXPECTED_PRODUCER0_STATE_SHA256" ]] ||
    abort "producer-0 run-state identity changed"
[[ ! -e "$RECEIPT" ]] || abort "canonical schema-v2 receipt already exists"

while IFS=$'\t' read -r artifact_path expected_sha256; do
    [[ "$(observed_sha256 "$JOURNAL/$artifact_path")" == "$expected_sha256" ]] ||
        abort "authorized producer-0 prefix changed at $artifact_path"
done < <(jq -r '.prefix_artifacts[] | [.path, .sha256] | @tsv' "$AUTHORIZATION")

LOCAL_HOUR="$(date +%H)"
readonly LOCAL_HOUR
((10#$LOCAL_HOUR >= 2)) || abort "device run opened before the 02:00 local window"

echo "HEAD: $(git rev-parse HEAD)"
echo "dependency source: $(git log -1 --format=%H -- crates/ Cargo.lock)"
echo "binary sha256: $(observed_sha256 "$BINARY")"
echo "authorization sha256: $(observed_sha256 "$AUTHORIZATION")"
rocm-smi --showuse --showmeminfo vram 2>/dev/null | head -20 || true
echo "=== $(date -Is) launching five missing anchors under the full-host lock ==="

dev/scripts/ccx1-bench-flock.sh --full-host \
    "$BINARY" \
    --preregistration dev/active/02b8137c/pre-draw-validation-v1-preregistration.json \
    --state-dir "$JOURNAL" \
    --receipt "$RECEIPT" \
    --workers 24 \
    --continue-producer-segment "$AUTHORIZATION"
rc=$?

echo "=== $(date -Is) validator exit code: $rc ==="
if [[ -e "$RECEIPT" ]]; then
    echo "receipt sha256: $(observed_sha256 "$RECEIPT")"
fi
exit "$rc"
