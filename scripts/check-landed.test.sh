#!/bin/sh
# Tests that the landed gate holds a code-producing issue's dependents until
# its code is on main, with the real jit over a scratch code repository whose
# one store two leads share: lead one in a linked worktree without a store,
# lead two in the primary checkout, both through scripts/tracker-env. A
# trivial executed-command gate stands in for cargo-ci.
# Usage: sh scripts/check-landed.test.sh; exits 0 on success or, printing the
# reason, when jit is not on PATH; 1 on failure.
set -eu

if ! command -v jit >/dev/null 2>&1; then
  echo 'check-landed integration test skipped: jit is not on PATH'
  exit 0
fi

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/check-landed-test.XXXXXX")
trap 'rm -rf "$TMP"' 0
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
expect() { [ "$2" = "$3" ] || fail "$1: got [$2], expected [$3]"; }

unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CEILING_DIRECTORIES \
  GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS
unset JIT_DATA_DIR JIT_WORKTREE_ROOT JIT_AGENT_ID JIT_ISSUE_ID
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.invalid

BASE=$(cd "$TMP" && pwd -P)
CODE="$BASE/code"
ONE="$BASE/lead-one"
git init -q --initial-branch=main "$CODE"
mkdir "$CODE/scripts"
for name in check-landed landing_gates.py tracker-env; do
  cp "$ROOT/scripts/$name" "$CODE/scripts/"
done
printf '/.jit/\n__pycache__/\n' >"$CODE/.gitignore"
: >"$CODE/code"
git -C "$CODE" add .
git -C "$CODE" commit -qm init
(cd "$CODE" && jit init >/dev/null)
git -C "$CODE" worktree add -q -b topic "$ONE"
[ ! -e "$ONE/.jit" ] || fail 'the linked worktree holds a store'

one() { (cd "$ONE" && scripts/tracker-env jit "$@"); }
two() { (cd "$CODE" && JIT_AGENT_ID=agent:lead-two scripts/tracker-env jit "$@"); }
new_issue() {
  two issue create "$@" --json | python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])'
}
state() {
  two issue show "$1" --json | python3 -c 'import json, sys; print(json.load(sys.stdin)["state"])'
}
available() {
  two query available --json |
    python3 -c 'import json, sys; print(" ".join(i["id"] for i in json.load(sys.stdin)["issues"]))'
}
is_available() {
  case " $(available) " in *" $1 "*) return 0 ;; esac
  return 1
}
run_commit() {
  two gate status "$1" "$2" --json | python3 -c 'import json, sys; print(json.load(sys.stdin).get("commit"))'
}

two gate define ci --title ci --description 'stand-in for cargo-ci' \
  --checker-command true -q
two gate define landed --title landed --description 'code is on main' \
  --checker-command scripts/check-landed -q
cat >>"$CODE/.jit/gates.toml" <<'EOF'

[[gates]]
version = 1
key = "val"
title = "val"
description = "built-in issue validation"
stage = "postcheck"
mode = "auto"
priority = 100

[gates.checker]
type = "issue_validation"

[gates.reserved]
EOF

# Lead one lands issue $1; lead two watches its dependent. $2 lists the gates
# the issue requires beside ci and landed, passed before ci.
land() {
  extra=
  for gate in $2; do extra="$extra --gate $gate"; done
  issue=$(new_issue "code-producing $1" --gate ci --gate landed $extra)
  dependent=$(new_issue "dependent $1")
  two dep add "$dependent" "$issue" -q
  one issue claim "$issue" agent:lead-one -q
  printf '%s\n' "$1" >>"$ONE/code"
  git -C "$ONE" commit -qam "change $1"
  tip=$(git -C "$ONE" rev-parse HEAD)

  for gate in $2; do
    one gate evaluate "$issue" "$gate" -q || fail "$1: gate $gate failed"
  done
  one gate evaluate "$issue" ci -q || fail "$1: ci failed"
  expect "$1: ci commit" "$(run_commit "$issue" ci)" "$tip"
  one issue update "$issue" --state done -q >/dev/null 2>&1 || :
  expect "$1: state after the last ordinary gate" "$(state "$issue")" gated
  ! is_available "$dependent" || fail "$1: dependent available before the merge"

  rc=0
  one gate evaluate "$issue" landed -q >/dev/null 2>&1 || rc=$?
  expect "$1: landed before the merge" "$rc" 4
  expect "$1: state after a refused landing" "$(state "$issue")" gated
  ! is_available "$dependent" || fail "$1: dependent available after a refused landing"

  git -C "$CODE" merge -q --no-ff -m "merge $1" topic
  ! is_available "$dependent" || fail "$1: dependent available before the landed pass"
  one gate evaluate "$issue" landed -q || fail "$1: landed failed after the merge"
  expect "$1: state after landing" "$(state "$issue")" done
  is_available "$dependent" || fail "$1: dependent unavailable after landing"
}

land ci-only ''
land built-in val

echo 'check-landed integration test passed'
