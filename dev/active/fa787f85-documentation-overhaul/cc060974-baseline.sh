#!/usr/bin/env bash
# usage: cc060974-baseline.sh <outfile>
# Runs the script tests and each Python generator of the entries 00dd43c3 and
# c04dd4ac that needs no build, in a scratch clone of HEAD under target/, from
# the clone's root. Entry directories are located by file name.
#   stdout line       the command's standard output and error, scratch prefix
#                     removed, test durations removed.
#   out <name>        a file the command writes outside the tree: SHA-256 of
#                     its bytes (raw) and of its bytes with each entry path
#                     under the epic directory rewritten to dev/active/<entry>
#                     (flat).
#   against <file>    standard output compared with a committed file: `same`
#                     or the count of differing lines; raw and flat as above.
#   last section      `git diff --numstat` of the clone: each committed file a
#                     generator rewrites in place with other bytes, and the
#                     flat SHA-256 of what it writes.
set -u
top=$(git rev-parse --show-toplevel)
out=$(realpath "$1")
s=$top/target/cc060974-scratch
rm -rf "$s"; git clone -q --shared "$top" "$s"; cd "$s" || exit 1
t=$s/.git/cc060974; mkdir -p "$t"
Z=$(dirname "$(git ls-files -- ':(glob)dev/active/**/test_pilot_freeze.py')")
C=$(dirname "$(git ls-files -- ':(glob)dev/active/**/test-profile-intervals.py')")
ZA=$(dirname "$Z"); CA=$(dirname "$C")
B=dev/bench_results/c04dd4ac/dvb-interleave-profile
flat() { sed -E 's#dev/active/1a379447-zen3-cpu-performance/(00dd43c3|c04dd4ac-zen3-shifts-and-permutations)#dev/active/\1#g' "$1" | sha256sum | cut -d' ' -f1; }
sums() { echo "raw=$(sha256sum "$1" | cut -d' ' -f1) flat=$(flat "$1")"; }
run() { # run <script> [args]: output and exit status
  echo "## ${*//$t\//}"
  python3 -B "$@" 2>&1 | sed -E "s#$s/##g; s#$t/##g; s/ in [0-9.]+s\$//"
  echo "exit=${PIPESTATUS[0]}"
}
written() { echo "out $(basename "$1") $(sums "$1")"; }
against() { # against <committed> <script> [args]: stdout compared with a file
  local committed=$1 n; shift
  echo "## $* > $committed"
  python3 -B "$@" > "$t/stdout" 2> "$t/stderr"
  echo "exit=$? $(tail -n 1 "$t/stderr")"
  n=$(diff "$t/stdout" "$committed" | grep -c '^[<>]')
  echo "against $committed $([ "$n" = 0 ] && echo same || echo "diff-lines=$n") $(sums "$t/stdout")"
}
frozen() { sed -n 's/.*"frozen_utc": "\(.*\)".*/\1/p' "$1"; }
{
  run "$Z/test_pilot_freeze.py"
  run "$C/test-profile-intervals.py"
  run "$C/make-shift-producing-inputs.py" --check
  run "$Z/freeze-pilot.py" --check
  run "$Z/freeze-pilot.py"
  run "$Z/make-plan.py" "$t/plan-00dd43c3.json" /tmp/shifts-arm /tmp/unused-shift-lock \
    --label pilot --campaign-id test-residual-pilot
  written "$t/plan-00dd43c3.json"
  run "$Z/make-producing-inputs.py"
  run "$Z/freeze-confirmation.py"
  run "$C/freeze-shift-addendum.py" --check
  run "$C/freeze-shift-addendum.py"
  run "$C/inspect-shift-consumers.py"
  run "$C/make-dvb-addendum.py" "$(frozen "$CA/dvb-profile-addendum.json")"
  run "$C/make-dvb-plan.py" --addendum "$CA/dvb-profile-addendum.json" --campaign-id test-dvb \
    --campaign-seed 1 --lock /tmp/unused-dvb-lock --executable /tmp/dvb-arm \
    --producing-manifest "$C/dvb-producing-inputs.json" --output "$t/plan-dvb.json"
  written "$t/plan-dvb.json"
  against "$CA/dvb-interleave-profile-tables.md" "$C/make-dvb-tables.py" "$B/v4-r2-pilot/receipt.json" \
    "$B/v4-r2-dynamic-profile" "$C/dvb-source-evidence.json" "$CA/dvb-profile-addendum.json" \
    "$B/v4-r3-dynamic-profile"
  run "$C/make-producing-inputs.py"
  for p in v4-r2-dynamic-profile v4-r3-dynamic-profile; do
    against "$B/$p-provenance.md" "$C/make-profile-provenance.py" "$B/$p"
    against "$B/$p/profile-summary.md" "$C/summarize-profile.py" "$B/$p"
  done
  against "$CA/publication-tables.md" "$C/make-publication-tables.py"
  run "$C/make-shift-plan.py" "$t/plan-shift.json" /tmp/shifts-arm /tmp/unused-shift-lock \
    --campaign-id test-shift
  written "$t/plan-shift.json"
  run "$C/make-shift-producing-inputs.py"
  run "$C/make-shift-source-evidence.py"
  run "$C/make-source-evidence.py"
  run "$C/make-voided-attempt.py"
  run "$C/make-voided-attempt-record.py"
  run "$C/summarize-shift-receipt.py"
  echo '## files rewritten in place with other bytes: added, deleted, path, flat'
  git diff --numstat | while read -r a d f; do echo "$a $d $f flat=$(flat "$f")"; done
  git status --porcelain | grep -v '^ M' | sed 's/^/untracked or other: /'
} > "$out"
cd "$top" && rm -rf "$s"
