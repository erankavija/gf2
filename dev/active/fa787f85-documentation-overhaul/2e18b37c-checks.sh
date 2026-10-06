#!/usr/bin/env bash
# usage: 2e18b37c-checks.sh state|builds|root|selfchecks|ci-step|regen <outfile>
# Checks of the entries f8dd4dde, a203a23c and 19513245 wherever they live
# under dev/active/; inputs/ snapshot directories are outside every listing.
# builds and selfchecks print a path without dev/active/ and the epic
# directory.
#   state       the relative Cargo path dependencies that leave or name an
#               entry; entry code that counts directory levels.
#   builds      per harness manifest of an entry: exit status of dependency
#               resolution and of the build on Rust 1.95, first error line.
#   root        the root `19513245/survey/run-profile.sh` resolves, relative
#               to the checkout root: its HERE and REPO assignments evaluated
#               with the script's path, nothing else of the script run.
#   selfchecks  exit status of each script test under dev/active/ and of each
#               `--self-test` an entry script offers.
#   ci-step     verdict line of the `receipt-input-snapshots` CI step without
#               its duration, and the summary line of the script it runs.
#   regen       per committed file an entry script writes again without a
#               build or a benchmark window: `same` or the count of diff
#               lines, and the exit status of the two generators. The addenda and derivation records come from
#               make-addenda.py and freeze-confirmations.sh at the frozen
#               times of the addenda, run in a scratch repository under
#               target/ that holds the tracked files of dev/active/,
#               dev/scripts/ and dev/bench_results/19513245/.
# The 6c6b09b1 crates the consumer harness links read the byte-field prefix
# and C shim staged under target/.
set -u
cd "$(git rev-parse --show-toplevel)"
export GF2_SURVEY_EXT="$PWD/target/6c6b09b1-ext" GF2_SURVEY_BUILD="$PWD/target/6c6b09b1-survey/c"
export CARGO_CI_NO_SCCACHE=1
entries='f8dd4dde|a203a23c|19513245'
name() { local m=${1#dev/active/}; echo "${m#1a379447-zen3-cpu-performance/}"; }
case $1 in
state)
  echo '# relative Cargo and build-script paths'
  git grep -n -E '"\.\./' -- ':(glob)**/Cargo.toml' ':(glob)**/build.rs' ':(exclude,glob)**/inputs/**' | grep -E "($entries)"
  echo '# code that counts directory levels'
  git grep -n -E 'parents\[[0-9]\]|\.\./\.\.|ancestors\(\)|pardir|CARGO_MANIFEST_DIR' -- ':(glob)dev/active/**/*.py' ':(glob)dev/active/**/*.sh' ':(glob)dev/active/**/*.rs' ':(exclude,glob)**/inputs/**' |
    grep -E "^dev/active/([^/]*/)?($entries)/" ;;
builds)
  git ls-files -- ':(glob)dev/active/**/Cargo.toml' | grep -E "/($entries)/" | grep -v /inputs/ |
  while read -r m; do
    ./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --manifest-path "$m" >/dev/null 2>&1
    resolve=$?
    log=$(./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path "$m" 2>&1)
    echo "$(name "$m") resolve=$resolve build=$? $(grep -m1 '^error' <<<"$log" | sed -E 's# \(/[^)]*\)##')"
  done | sort ;;
root)
  s=$(git ls-files -- ':(glob)dev/active/**/19513245/survey/run-profile.sh')
  eval "$(grep -E '^(HERE|REPO)=' "$s" | sed "s#\${BASH_SOURCE\[0\]}#$s#")"
  echo "$(name "$s") root=$(realpath --relative-to="$PWD" "$REPO")" ;;
selfchecks)
  { git ls-files -- ':(glob)dev/active/**/*_test.py' ':(glob)dev/active/**/test-*.py' \
      ':(glob)dev/active/**/test_*.py' ':(glob)dev/active/**/*.test.sh' \
      ':(glob)dev/active/**/verify-records.py' |
      grep -v -e /inputs/ -e /fd9d5416/ -e /fa787f85-documentation-overhaul/ |
    while read -r t; do
      case $t in *.sh) bash "$t" ;; *) python3 -B "$t" ;; esac >/dev/null 2>&1
      echo "$(name "$t") exit=$?"
    done
    git grep -l -e '--self-test' -- ':(glob)dev/active/**/*.py' ':(exclude,glob)**/inputs/**' |
      grep -E "/($entries)/" |
    while read -r t; do
      python3 -B "$t" --self-test >/dev/null 2>&1
      echo "$(name "$t") --self-test exit=$?"
    done
  } | sort ;;
ci-step)
  CARGO_CI_STEPS='^receipt-input-snapshots$' ./scripts/cargo-ci.sh 2>&1 |
    grep -E '^ *[^ ]+ receipt-input-snapshots:' | sed -E 's/^ *//; s/ \([0-9]+s\)$//'
  python3 dev/scripts/check-receipt-input-snapshots.py | tail -n 1 ;;
regen)
  out=$PWD/$2
  e=$(dirname "$(git ls-files -- ':(glob)dev/active/**/19513245/findings.md')")
  a=$(dirname "$(git ls-files -- ':(glob)dev/active/**/a203a23c/select-receipts.py')")
  p=dev/bench_results/19513245/r1-consumer-profile
  scratch=$PWD/target/2e18b37c-regen
  rm -rf "$scratch"; mkdir -p "$scratch"
  git ls-files -z -- dev/active dev/scripts dev/bench_results/19513245 | tar --null -T - -cf - | tar -xf - -C "$scratch"
  compare() { local n; n=$(diff "$1" "$2" | grep -c '^[<>]'); echo "$(name "$2") $([ "$n" = 0 ] && echo same || echo "diff-lines=$n")"; }
  {
    python3 -B "$e/survey/summarize-profile.py" "$p" > "$scratch/profile-summary.md"
    compare "$scratch/profile-summary.md" "$p/profile-summary.md"
    for r in timing.json timing-tmpfs-superseded.json; do
      cp "$a/timing/$r" "$scratch/$r"
      python3 -B "$a/timing/measure.py" --assess "$scratch/$r" >/dev/null 2>&1
      compare "$scratch/$r" "$a/timing/$r"
    done
    ( cd "$scratch" && git init -q . &&
      python3 -B "$e/survey/make-addenda.py" 4 "$(sed -n 's/.*"frozen_utc": "\(.*\)".*/\1/p' "$e/addendum-v4-control-pilot.json")" &&
      bash "$e/survey/freeze-confirmations.sh" "$(sed -n 's/.*"frozen_utc": "\(.*\)".*/\1/p' "$e/addendum-v4-control-confirmation.json")"
    ) >/dev/null 2>&1
    echo "make-addenda.py and freeze-confirmations.sh exit=$?"
    for f in "$e"/addendum-v4-*.json "$e"/confirmation-derivation-*.txt; do compare "$scratch/$f" "$f"; done
  } > "$out"
  rm -rf "$scratch"
  exit ;;
esac > "$2"
