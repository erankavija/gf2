#!/usr/bin/env bash
# Independent re-evaluation of this issue's protocol-v3 receipts (jit:eda07788).
#
# Usage: dev/bench_results/eda07788/reevaluate-v3.sh
#
# Builds `benchmark-acceptance` from the current tree under `--release`, then
# regenerates `acceptance-summary.json` and `acceptance-summary.md` beside each
# protocol-v3 receipt. No timing runs. For each receipt directory the log
# records the digest of `receipt.json` and a digest over every other file
# except the two summaries, before and after evaluation, so an unchanged
# receipt is checked rather than assumed. The evaluator is identified by its
# executable digest and by a digest over its crate sources.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
RESULTS=dev/bench_results/eda07788
LOG="$RESULTS/reevaluation-v3.log"
RECEIPTS=(
  "$RESULTS/2026-09-08-eda07788-dvb-t2-v3-pilot"
  "$RESULTS/2026-09-08-eda07788-dvb-t2-v3-pilot-r2"
  "$RESULTS/2026-09-08-eda07788-dvb-t2-v3-confirmation"
  "$RESULTS/2026-09-10-eda07788-nr-derate-pilot"
  "$RESULTS/2026-09-10-eda07788-nr-derate-confirmation"
)

./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-acceptance
ACCEPTANCE=target/release/benchmark-acceptance

# Digest over the sorted per-file digests of a directory, excluding the files
# the evaluator writes.
tree_digest() {
  (cd "$1" && find . -type f ! -name acceptance-summary.json \
      ! -name acceptance-summary.md -print0 | LC_ALL=C sort -z \
    | xargs -0 sha256sum | sha256sum | cut -d' ' -f1)
}

{
  echo "started=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "gf2_revision_informational=$(git rev-parse HEAD)"
  echo "evaluator_sha256=$(sha256sum "$ACCEPTANCE" | cut -d' ' -f1)"
  echo "evaluator_source_sha256=$(git ls-files -z dev/tools/tuning-campaign-support \
    | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -d' ' -f1)"
  echo "evaluator_source_method=sha256 of sorted sha256sum lines over git ls-files dev/tools/tuning-campaign-support"
} >"$LOG"

status=0
for dir in "${RECEIPTS[@]}"; do
  receipt_before=$(sha256sum "$dir/receipt.json" | cut -d' ' -f1)
  tree_before=$(tree_digest "$dir")
  set +e
  output=$("$ACCEPTANCE" "$dir")
  rc=$?
  set -e
  receipt_after=$(sha256sum "$dir/receipt.json" | cut -d' ' -f1)
  tree_after=$(tree_digest "$dir")
  {
    echo
    echo "receipt_dir=$dir"
    echo "receipt_sha256_before=$receipt_before"
    echo "inputs_tree_sha256_before=$tree_before"
    echo "command=$ACCEPTANCE $dir"
    echo "$output"
    echo "exit=$rc"
    echo "receipt_sha256_after=$receipt_after"
    echo "inputs_tree_sha256_after=$tree_after"
    echo "summary_json_sha256=$(sha256sum "$dir/acceptance-summary.json" | cut -d' ' -f1)"
    echo "summary_md_sha256=$(sha256sum "$dir/acceptance-summary.md" | cut -d' ' -f1)"
  } >>"$LOG"
  if [[ "$rc" -ne 0 || "$receipt_before" != "$receipt_after" || "$tree_before" != "$tree_after" ]]; then
    status=1
  fi
done
echo "finished=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$LOG"
cat "$LOG"
exit "$status"
