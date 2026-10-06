#!/usr/bin/env bash
# usage: 7039d680-checks.sh state|builds|tests|selfchecks <outfile>
# Checks of the entries 26465e6c, eda07788 and 5cbb6545 wherever they live
# under dev/active/; inputs/ snapshot directories are outside every listing.
# builds, tests and selfchecks print a path without dev/active/ and the epic
# directory.
#   state       `--check` result of each live closure generator; the relative
#               Cargo path dependencies and build-script paths that leave or
#               name an entry; entry code that counts directory levels.
#   builds      per harness manifest of an entry: exit status of dependency
#               resolution and of the build on Rust 1.95, first error line.
#   tests       `cargo test` result lines and failed test names per harness
#               manifest of an entry that has a tests/ directory.
#   selfchecks  exit status of each script test under dev/active/ and of each
#               `--self-test` an entry script offers.
# The eda07788 gf2-side build script reads the xdsopl checkout staged beside
# the primary checkout.
set -u
cd "$(git rev-parse --show-toplevel)"
primary=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
export GF2_XDSOPL_INCLUDE="$primary/.agents/ext/c077a88b/xdsopl-ldpc" CARGO_CI_NO_SCCACHE=1
entries='26465e6c|eda07788|5cbb6545'
name() { local m=${1#dev/active/}; echo "${m#1a379447-zen3-cpu-performance/}"; }
manifests() { git ls-files -- ':(glob)dev/active/**/Cargo.toml' | grep -E "/($entries)/" | grep -v /inputs/; }
case $1 in
state)
  echo '# closure generators'
  git ls-files -- ':(glob)**/make-*-producing-inputs.py' ':(exclude,glob)**/inputs/**' | sort |
  while read -r g; do
    grep -q '"--check"' "$g" || continue
    echo "$g exit=$(python3 -B "$g" --check >/dev/null 2>&1; echo $?)"
  done
  echo '# relative Cargo and build-script paths'
  git grep -n -E '"\.\./' -- ':(glob)**/Cargo.toml' ':(glob)**/build.rs' ':(exclude,glob)**/inputs/**' | grep -E "($entries)"
  echo '# code that counts directory levels'
  git grep -n -E 'parents\[[0-9]\]|\.\./\.\.|ancestors\(\)|CARGO_MANIFEST_DIR' -- ':(glob)dev/active/**/*.py' ':(glob)dev/active/**/*.sh' ':(glob)dev/active/**/*.rs' ':(exclude,glob)**/inputs/**' |
    grep -E "^dev/active/([^/]*/)?($entries)/" ;;
builds)
  manifests | while read -r m; do
    ./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --manifest-path "$m" >/dev/null 2>&1
    resolve=$?
    log=$(./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path "$m" 2>&1)
    echo "$(name "$m") resolve=$resolve build=$? $(grep -m1 '^error' <<<"$log" | sed -E 's# \(/[^)]*\)##')"
  done | sort ;;
tests)
  manifests | while read -r m; do
    [ -d "$(dirname "$m")/tests" ] || continue
    ./scripts/cargo-budget.sh --test cargo +1.95.0 test --no-fail-fast --manifest-path "$m" 2>&1 </dev/null |
      grep -E '^test result|^test .* FAILED$| Running |Doc-tests ' |
      sed -E "s#^#$(name "$m") #; s# \(.*##; s#; finished in .*##"
  done ;;
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
esac > "$2"
