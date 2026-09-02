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
# `ORACLE_GAP_HEAP` sets the GAP heap the oracle runs under; the receipt records
# the invocation it was used in.
#
# Usage: dev/active/ae03bcd0-general-bch/oracle/run.sh
set -euo pipefail

gap_heap="${ORACLE_GAP_HEAP:-4g}"

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

emitter="./target/release/examples/bch_oracle_messages"
build_cmd="./scripts/cargo-budget.sh cargo build --release -p gf2-coding --features test-support --example bch_oracle_messages"
emit_cmd="$emitter $corpus"
sage_cmd="python3 $sage_script $corpus $sage_out"
gap_cmd="gap -q -A -T -o $gap_heap -c 'CORPUS:=\"$corpus\"; OUTPUT:=\"$gap_out\";' $gap_script"

echo "== build =="
build_start=$SECONDS
eval "$build_cmd"
build_seconds=$((SECONDS - build_start))

echo "== corpus =="
emit_start=$SECONDS
eval "$emit_cmd"
emit_seconds=$((SECONDS - emit_start))

echo "== SageMath =="
sage_start=$SECONDS
eval "$sage_cmd"
sage_seconds=$((SECONDS - sage_start))

# `-T` keeps a code-object attempt that exceeds the heap from ending the run,
# so the oracle records the attempt instead. GAP then exits zero either way and
# the fixture's presence is what says the run produced a result.
echo "== GAP with GUAVA =="
gap_log="target/bch-oracle-gap.log"
mkdir -p target
rm -f "$gap_out"
gap_start=$SECONDS
eval "$gap_cmd" 2>&1 | tee "$gap_log"
gap_seconds=$((SECONDS - gap_start))
if [ ! -f "$gap_out" ]; then
  echo "the GAP oracle wrote no fixture; see $gap_log" >&2
  exit 1
fi

finished="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Oracle identity, read back from what each oracle recorded about itself.
field() {
  python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["oracle"][sys.argv[2]])' "$1" "$2"
}
digest() {
  if [ -e "$1" ]; then sha256sum "$1" | cut -d' ' -f1; else echo "absent"; fi
}

sage_version="$(field "$sage_out" version)"
sage_library="$(field "$sage_out" library_version)"
sage_interpreter="$(field "$sage_out" interpreter)"
sage_exe="$(field "$sage_out" interpreter_executable)"
sage_exe_real="$(readlink -f "$sage_exe")"

gap_version="$(field "$gap_out" gap_version)"
gap_kernel="$(field "$gap_out" gap_kernel_version)"
gap_build="$(field "$gap_out" gap_build_version)"
gap_build_datetime="$(field "$gap_out" gap_build_datetime)"
gap_arch="$(field "$gap_out" gap_architecture)"
gmp_version="$(field "$gap_out" gmp_version)"
gap_heap_observed="$(field "$gap_out" heap)"
guava_version="$(field "$gap_out" guava_version)"
guava_path="$(field "$gap_out" guava_path)"
sonata_version="$(field "$gap_out" sonata_version)"
sonata_path="$(field "$gap_out" sonata_path)"
gap_exe_real="$(readlink -f "$(command -v gap)")"

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
  echo
  echo "## Oracle identity"
  echo
  echo "| Property | Value |"
  echo "|---|---|"
  echo "| SageMath | $sage_version |"
  echo "| SageMath library version | $sage_library |"
  echo "| SageMath interpreter | CPython $sage_interpreter |"
  echo "| SageMath interpreter executable | \`$sage_exe_real\` |"
  echo "| SageMath interpreter SHA-256 | \`$(digest "$sage_exe_real")\` |"
  echo "| GAP version label | $gap_version |"
  echo "| GAP kernel version | $gap_kernel |"
  echo "| GAP build version | $gap_build |"
  echo "| GAP build datetime | $gap_build_datetime |"
  echo "| GAP architecture | $gap_arch |"
  echo "| GMP | $gmp_version |"
  echo "| GAP executable | \`$gap_exe_real\` |"
  echo "| GAP executable SHA-256 | \`$(digest "$gap_exe_real")\` |"
  echo "| GAP heap | $gap_heap_observed |"
  echo "| GUAVA | $guava_version |"
  echo "| GUAVA PackageInfo.g SHA-256 | \`$(digest "$guava_path/PackageInfo.g")\` |"
  echo "| SONATA | $sonata_version |"
  echo "| SONATA PackageInfo.g SHA-256 | \`$(digest "$sonata_path/PackageInfo.g")\` |"
  echo
  echo "## Stages"
  echo
  echo "| Stage | Exact invocation | Wall clock (s) | Output |"
  echo "|---|---|---|---|"
  echo "| build | \`$build_cmd\` | $build_seconds | \`$emitter\` |"
  echo "| corpus | \`$emit_cmd\` | $emit_seconds | [corpus.json](../../../$corpus) |"
  echo "| SageMath | \`$sage_cmd\` | $sage_seconds | [sage.json](../../../$sage_out) |"
  echo "| GAP with GUAVA | \`$gap_cmd\` | $gap_seconds | [gap.json](../../../$gap_out) |"
  echo
  echo "## GUAVA code-object attempts"
  echo
  echo "Each row calls \`BCHCode(n, b, delta, F)\` under the heap above; the"
  echo "oracle catches an attempt that exceeds it and records what it observed."
  echo
  python3 - "$gap_out" <<'ATTEMPTS'
import json, sys
rows = json.load(open(sys.argv[1]))["rows"]
print("| Row | Code object built | Attempt CPU (s) | Process peak RSS after attempt (KiB) |")
print("|---|---|---|---|")
for row in rows:
    peak = row["bchcode_attempt_peak_rss_kib"]
    print("| {} | {} | {:.1f} | {} |".format(
        row["id"],
        "yes" if row["bchcode_built"] else "no",
        row["bchcode_attempt_cpu_ms"] / 1000.0,
        "unavailable" if peak is None else peak,
    ))
ATTEMPTS
  echo
  echo "Diagnostics GAP printed during the stage:"
  echo
  echo '```'
  if grep -q . "$gap_log"; then cat "$gap_log"; else echo "(none)"; fi
  echo '```'
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
