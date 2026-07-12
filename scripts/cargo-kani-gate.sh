#!/usr/bin/env bash
set -euo pipefail

# Keep JIT gate records compact while retaining the complete cargo-kani log as
# a compressed, machine-local audit artifact.

repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
artifact_dir="$repo_root/.jit/gate-artifacts/cargo-kani"
mkdir -p "$artifact_dir"

issue_id=${JIT_ISSUE_ID:-unknown-issue}
stamp=$(date -u +%Y%m%dT%H%M%SZ)
artifact_rel=".jit/gate-artifacts/cargo-kani/${issue_id}-${stamp}-$$.log.gz"
artifact="$repo_root/$artifact_rel"
raw_log=$(mktemp)
excerpt=$(mktemp)
trap 'rm -f "$raw_log" "$excerpt"' EXIT

set +e
cargo kani >"$raw_log" 2>&1
status=$?
set -e

bytes=$(wc -c <"$raw_log" | tr -d ' ')
lines=$(wc -l <"$raw_log" | tr -d ' ')
digest=$(sha256sum "$raw_log" | cut -d' ' -f1)
gzip -c "$raw_log" >"$artifact"

if [ "$status" -eq 0 ]; then
  echo "cargo kani: PASSED"
  tail_lines=40
else
  echo "cargo kani: FAILED (exit $status)"
  tail_lines=200
fi

max_excerpt_bytes=${CARGO_KANI_JIT_MAX_EXCERPT_BYTES:-65536}
tail -n "$tail_lines" "$raw_log" >"$excerpt"
excerpt_bytes=$(wc -c <"$excerpt" | tr -d ' ')

echo "full log: $artifact_rel"
echo "uncompressed: ${bytes} bytes, ${lines} lines, sha256:${digest}"
echo "--- last ${tail_lines} lines ---"
if [ "$excerpt_bytes" -gt "$max_excerpt_bytes" ]; then
  echo "[excerpt byte-capped: showing final ${max_excerpt_bytes} of ${excerpt_bytes} bytes]"
  tail -c "$max_excerpt_bytes" "$excerpt"
else
  cat "$excerpt"
fi

exit "$status"
