#!/usr/bin/env bash
#
# Regenerates the committed BCH oracle fixtures and writes the run receipt.
#
# Stages, in order:
#   1. build and run the corpus emitter, producing `corpus.json`;
#   2. run the SageMath oracle over that corpus, producing `sage.json`;
#   3. run the GAP/GUAVA oracle over that corpus, producing `gap.json`;
#   4. render `oracle-receipt.md` from what this run observed.
#
# Every figure in the receipt is read back from the artifacts this run wrote or
# from the host at run time; nothing is hand-maintained. Re-running from the
# recorded revision rewrites the three fixtures byte for byte, so `git status`
# reports only the receipt's own timestamp.
#
# Usage: dev/active/ae03bcd0-general-bch/oracle/run.sh
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$script_dir" rev-parse --show-toplevel)"
cd "$root"

fixtures="crates/gf2-coding/tests/data/bch_oracle"
sage_script="dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py"
gap_script="dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g"
receipt="dev/active/ae03bcd0-general-bch/oracle-receipt.md"

corpus="$fixtures/corpus.json"
sage_out="$fixtures/sage.json"
gap_out="$fixtures/gap.json"

started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
revision="$(git rev-parse HEAD)"
if [ -z "$(git status --porcelain)" ]; then
  tree_state="clean"
else
  tree_state="modified"
fi

build_cmd="./scripts/cargo-budget.sh cargo build --release -p gf2-coding --features test-support --example bch_oracle_messages"
emit_cmd="./target/release/examples/bch_oracle_messages $corpus"
sage_cmd="python3 $sage_script $corpus $sage_out"
gap_cmd="gap -q -A -o 4g -c 'CORPUS:=\"$corpus\"; OUTPUT:=\"$gap_out\";' $gap_script"

echo "== build =="
eval "$build_cmd"

echo "== corpus =="
emit_start=$SECONDS
eval "$emit_cmd"
emit_seconds=$((SECONDS - emit_start))

echo "== SageMath =="
sage_start=$SECONDS
eval "$sage_cmd"
sage_seconds=$((SECONDS - sage_start))

echo "== GAP with GUAVA =="
gap_start=$SECONDS
eval "$gap_cmd"
gap_seconds=$((SECONDS - gap_start))

finished="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

sage_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"]["version"])' "$sage_out")"
sage_interpreter="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"]["interpreter"])' "$sage_out")"
gap_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"]["gap_version"])' "$gap_out")"
guava_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"]["guava_version"])' "$gap_out")"
sonata_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"]["sonata_version"])' "$gap_out")"
cpu="$(awk -F': ' '/^model name/ {print $2; exit}' /proc/cpuinfo)"
kernel="$(uname -srmo)"
rust_version="$(rustc --version)"

{
  echo "# Receipt: external BCH oracle fixture generation"
  echo
  echo "Written by [\`oracle/run.sh\`](oracle/run.sh) from the run it describes."
  echo "Every value below was observed during that run."
  echo
  echo "| Field | Value |"
  echo "|---|---|"
  echo "| Issue | \`3f7edef1\` |"
  echo "| Run start (UTC) | $started |"
  echo "| Run end (UTC) | $finished |"
  echo "| gf2 revision | \`$revision\` |"
  echo "| Working tree at generation | $tree_state |"
  echo "| Host CPU | $cpu |"
  echo "| Kernel | $kernel |"
  echo "| Rust | $rust_version |"
  echo "| SageMath | $sage_version |"
  echo "| SageMath interpreter | CPython $sage_interpreter |"
  echo "| GAP | $gap_version |"
  echo "| GUAVA | $guava_version |"
  echo "| SONATA | $sonata_version |"
  echo
  echo "## Stages"
  echo
  echo "| Stage | Exact invocation | Wall clock (s) | Output |"
  echo "|---|---|---|---|"
  echo "| build | \`$build_cmd\` | | |"
  echo "| corpus | \`$emit_cmd\` | $emit_seconds | [corpus.json](../../../$corpus) |"
  echo "| SageMath | \`$sage_cmd\` | $sage_seconds | [sage.json](../../../$sage_out) |"
  echo "| GAP with GUAVA | \`$gap_cmd\` | $gap_seconds | [gap.json](../../../$gap_out) |"
  echo
  echo "## Fixture hashes"
  echo
  echo "| File | SHA-256 | Bytes |"
  echo "|---|---|---|"
  for file in "$corpus" "$sage_out" "$gap_out"; do
    printf '| `%s` | `%s` | %s |\n' \
      "$file" "$(sha256sum "$file" | cut -d' ' -f1)" "$(stat -c%s "$file")"
  done
  echo
  echo "## Generating source"
  echo
  echo "| File | SHA-256 |"
  echo "|---|---|"
  for file in \
    "crates/gf2-coding/examples/bch_oracle_messages.rs" \
    "crates/gf2-coding/src/test_support.rs" \
    "$sage_script" "$gap_script" \
    "dev/active/ae03bcd0-general-bch/oracle/run.sh"; do
    printf '| `%s` | `%s` |\n' "$file" "$(sha256sum "$file" | cut -d' ' -f1)"
  done
} > "$receipt"

echo "== receipt =="
cat "$receipt"
