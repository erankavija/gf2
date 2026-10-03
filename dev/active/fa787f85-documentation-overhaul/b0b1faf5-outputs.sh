#!/usr/bin/env bash
# usage: b0b1faf5-outputs.sh <output-directory> <evidence-revision>
#
# Runs the four survey scripts that read the shared producing manifest and
# writes one capture per script into <output-directory>: command, exit status,
# stdout, stderr and the SHA-256 of every file the run writes.
#
# The three recording scripts write in place, so each runs at the root of a
# scratch copy of the tracked working tree, itself a git repository, on fixture
# arguments: stub executables, stub source trees, an AFF3CT stand-in holding
# one fixed commit, and the bundles of the committed recorded-inputs archive.
# The re-evaluation script runs in the checkout on <evidence-revision> and
# writes its record into the scratch directory.
#
# A capture names the scratch tree `<scratch>`. JSON digests are taken with the
# `observed_utc` and `evaluator_revision` values replaced, and cargo's build
# duration is replaced in stderr; no other byte is altered.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
mkdir -p "$1"
out=$(cd "$1" && pwd)
evidence=$(git -C "$root" rev-parse --verify "$2^{commit}")
cd "$root"

live() {
  git ls-files -- ":(glob)**/$1" | grep -v '/inputs/'
}

scratch=$root/target/b0b1faf5-outputs
tree=$scratch/tree
rm -rf "$scratch"
mkdir -p "$tree"
git ls-files -z | tar -c --null -T - | tar -x -C "$tree"
git -C "$tree" init -q

fixture=target/fixture
external=$fixture/external
mkdir -p "$tree/$external/inputs" "$tree/$fixture/evidence/quality" \
  "$tree/$fixture/baseline/release" "$tree/$fixture/candidate/release"
tar -xzf "$(live 'v3-preparation/source-inputs/recorded-inputs.tar.gz')" \
  -C "$tree/$external/inputs"
for name in aff3ct srsran xdsopl-ldpc oai simde; do
  mkdir -p "$tree/$external/$name"
  echo "$name" > "$tree/$external/$name/fixture.txt"
done
mkdir -p "$tree/$external/aff3ct/build/lib"
echo fixture > "$tree/$external/aff3ct/build/lib/libaff3ct-4.7.0.a"
git -C "$tree/$external/aff3ct" init -q
GIT_AUTHOR_DATE=2000-01-01T00:00:00Z GIT_COMMITTER_DATE=2000-01-01T00:00:00Z \
  git -C "$tree/$external/aff3ct" -c user.name=fixture \
  -c user.email=fixture@example.invalid -c commit.gpgsign=false \
  commit -q --allow-empty -m fixture
for name in gf2-throughput-arm aff3ct-throughput-arm ldpc-profile ldpc-plan-check \
  ldpc-alloc-census ldpc-throughput-validate; do
  echo "$name" > "$tree/$fixture/baseline/release/$name"
done
echo qc-decoder-arm > "$tree/$fixture/candidate/release/qc-decoder-arm"
echo '{}' > "$tree/$fixture/evidence/quality/fixture.json"

digest() {
  case $1 in
    *.json)
      sed -E 's/("(observed_utc|evaluator_revision)": )"[^"]*"/\1"<replaced>"/' "$1" |
        sha256sum | cut -d' ' -f1
      ;;
    *) sha256sum "$1" | cut -d' ' -f1 ;;
  esac
}

# capture <slug> <directory> <lister> <command...>: runs the command in the
# directory and digests the files the lister function prints there.
capture() {
  local slug=$1 directory=$2 written=$3 status=0
  shift 3
  local marker=$scratch/$slug.marker
  touch "$marker"
  (cd "$directory" && "$@") > "$scratch/$slug.stdout" 2> "$scratch/$slug.stderr" || status=$?
  {
    echo "command: $*"
    echo "exit: $status"
    echo "stdout:"
    cat "$scratch/$slug.stdout"
    echo "stderr:"
    sed -E 's/(Finished .* in )[0-9.]+s$/\1<duration>/' "$scratch/$slug.stderr"
    echo "written:"
    (cd "$directory" && "$written" "$marker") | LC_ALL=C sort |
      while IFS= read -r path; do
        echo "$(digest "$directory/$path")  $path"
      done
  } | sed "s|$tree|<scratch>|g; s|$root|<root>|g" > "$out/$slug.txt"
}

newer() {
  find . -newer "$1" -type f ! -path "./.git/*" ! -path "*/aff3ct/.git/*"
}

record=target/b0b1faf5-outputs/receipt-reevaluation.json
record() {
  echo "$record"
}

capture c077a88b-snapshot-inputs "$tree" newer \
  python3 -B "$(live 'c077a88b/survey/snapshot-inputs.py')" \
  --external "$external" --evidence "$fixture/evidence"

capture 3be770d5-record-preparation "$tree" newer \
  python3 -B "$(live '3be770d5/survey/record-preparation.py')" \
  --aff3ct "$external/aff3ct" --bin-dir "$fixture/baseline/release" \
  --inputs "$external/inputs"

capture f63a2464-record-preparation "$tree" newer \
  python3 -B "$(live 'f63a2464/survey/record-preparation.py')" \
  "$fixture/f63a2464" --aff3ct-root "$external/aff3ct" \
  --baseline-dir "$fixture/baseline/release" --candidate-dir "$fixture/candidate/release"

# Built ahead of the script, whose own build step then reports no compilation.
./scripts/cargo-budget.sh cargo build --locked --release \
  -p tuning-campaign-support --bin benchmark-acceptance
capture a203a23c-reevaluate-receipts "$root" record \
  "./$(live 'a203a23c/reevaluate-receipts.sh')" "$evidence" "$record"
