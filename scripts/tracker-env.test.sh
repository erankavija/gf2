#!/bin/sh
# Behavioral test for scripts/tracker-env. Scratch repositories live under
# $TMPDIR; a fake store index stands in for a tracker store and a fake jit
# answers lease lookups, so the real jit never runs.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
ENTRY="$ROOT/scripts/tracker-env"
TMP=$(mktemp -d "${TMPDIR:-/tmp}/tracker-env-test.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

# Isolate git from the caller's repository and configuration.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_CEILING_DIRECTORIES
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
gitq() { git -c user.name=t -c user.email=t@example.invalid -c commit.gpgsign=false "$@"; }

# Inherited tracker variables name nothing the entry point may use.
export JIT_DATA_DIR="$TMP/inherited-data" JIT_WORKTREE_ROOT="$TMP/inherited-root"

# A path with a space and a quote checks the printed assignments' quoting.
BASE="$TMP/sp ace'q"
mkdir -p "$BASE"
BASE=$(cd "$BASE" && pwd -P)

make_primary() { # $1: directory
  gitq init -q "$1"
  mkdir -p "$1/sub/dir"
  : >"$1/sub/dir/file"
  gitq -C "$1" add sub
  gitq -C "$1" commit -q -m init
}

P="$BASE/primary"
make_primary "$P"
mkdir -p "$P/.jit"
: >"$P/.jit/index.json"
# Two linked worktrees with one directory name; git registers distinct names.
mkdir -p "$BASE/a" "$BASE/b"
gitq -C "$P" worktree add -q "$BASE/a/wt" -b wt-a
gitq -C "$P" worktree add -q "$BASE/b/wt" -b wt-b
STALE="$BASE/stale"
gitq -C "$P" worktree add -q "$STALE" -b stale
mkdir -p "$STALE/.jit"
: >"$STALE/.jit/index.json"

EMPTY="$BASE/empty"
make_primary "$EMPTY"
mkdir -p "$EMPTY/.jit"
gitq -C "$EMPTY" worktree add -q "$BASE/empty-wt" -b wt

# Prints the three variables, one per line, as the command sees them.
# shellcheck disable=SC2016 # expanded by the probing shell
probe='printf "%s\n" "$JIT_DATA_DIR" "$JIT_WORKTREE_ROOT" "${JIT_AGENT_ID-<unset>}"'
empty_probe='printf "%s\n%s\n%s|%s\n" "$JIT_DATA_DIR" "$JIT_WORKTREE_ROOT" "${JIT_AGENT_ID+set}" "${JIT_AGENT_ID-<unset>}"'

# A fake jit: `claim status --issue leased --json` reports a lease held by
# agent:holder, `--issue broken` fails, any other issue has none; every other
# command prints the identity it runs as.
mkdir -p "$TMP/bin"
FAKE_JIT="$TMP/bin/jit"
cat >"$FAKE_JIT" <<'FAKE'
#!/bin/sh
if [ "$1 $2 $3" = "claim status --issue" ]; then
  case $4 in
  leased) printf '{\n  "count": 1,\n  "leases": [\n    {\n      "agent_id": "agent:holder",\n      "issue_id": "leased"\n    }\n  ]\n}\n' ;;
  broken) exit 3 ;;
  *) printf '{\n  "count": 0,\n  "leases": []\n}\n' ;;
  esac
  exit 0
fi
printf '%s\n' "${JIT_AGENT_ID-<unset>}"
FAKE
chmod +x "$FAKE_JIT"

expect() { # $1: label, $2: actual, $3: expected
  [ "$2" = "$3" ] || fail "$1: got [$2], expected [$3]"
}

empty_id_cases() { # $1: label, $2: working directory, $3: checkout root
  expected=$(printf '%s\n%s\n%s|%s' "$P/.jit" "$3" set '')
  out=$(cd "$2" && JIT_AGENT_ID= "$shell" "$ENTRY" sh -c "$empty_probe")
  expect "$shell $1 empty command" "$out" "$expected"

  printed=$(cd "$2" && JIT_AGENT_ID= "$shell" "$ENTRY")
  printf '%s\n' "$printed" | grep -Fx "export JIT_AGENT_ID=''" >/dev/null ||
    fail "$shell $1 empty print omitted export JIT_AGENT_ID=''"
  out=$(cd "$2" && JIT_AGENT_ID= "$shell" -c \
    'eval "$1" || exit 1; '"$empty_probe" _ "$printed")
  expect "$shell $1 empty eval" "$out" "$expected"
}

for shell in sh bash zsh; do
  # --- REQ-01: linked worktree without a store, from a subdirectory --------
  out=$(cd "$BASE/a/wt/sub/dir" && env -u JIT_AGENT_ID "$shell" "$ENTRY" sh -c "$probe")
  expect "$shell linked" "$out" "$(printf '%s\n' "$P/.jit" "$BASE/a/wt" agent:wt)"
  id_a=$(printf '%s\n' "$out" | sed -n 3p)

  # --- REQ-03: two linked worktrees get different identities ----------------
  out=$(cd "$BASE/b/wt/sub" && env -u JIT_AGENT_ID "$shell" "$ENTRY" sh -c "$probe")
  id_b=$(printf '%s\n' "$out" | sed -n 3p)
  expect "$shell second linked root" "$(printf '%s\n' "$out" | sed -n 2p)" "$BASE/b/wt"
  case $id_b in agent:?*) ;; *) fail "$shell second linked identity: $id_b" ;; esac
  [ "$id_a" != "$id_b" ] || fail "$shell: both linked worktrees are $id_a"

  # --- REQ-03: a caller-set identity passes through -------------------------
  out=$(cd "$BASE/a/wt/sub" && JIT_AGENT_ID="agent:caller x" "$shell" "$ENTRY" sh -c "$probe")
  expect "$shell linked caller id" "$(printf '%s\n' "$out" | sed -n 3p)" "agent:caller x"

  # --- REQ-03: an explicitly empty caller identity remains set and empty -----
  empty_id_cases linked "$BASE/a/wt/sub" "$BASE/a/wt"
  empty_id_cases primary "$P/sub" "$P"

  # --- REQ-02: primary checkout leaves JIT_AGENT_ID as the caller set it ----
  out=$(cd "$P/sub/dir" && env -u JIT_AGENT_ID "$shell" "$ENTRY" sh -c "$probe")
  expect "$shell primary" "$out" "$(printf '%s\n' "$P/.jit" "$P" '<unset>')"
  out=$(cd "$P/sub" && JIT_AGENT_ID=agent:lead "$shell" "$ENTRY" sh -c "$probe")
  expect "$shell primary caller id" "$out" "$(printf '%s\n' "$P/.jit" "$P" agent:lead)"

  # --- Print form: the same shell evaluates the assignments -----------------
  # shellcheck disable=SC2016 # expanded by the evaluating shell
  out=$(cd "$BASE/a/wt/sub" && env -u JIT_AGENT_ID "$shell" -c \
    'eval "$("$1" "$2")" || exit 1; '"$probe" _ "$shell" "$ENTRY")
  expect "$shell linked eval" "$out" "$(printf '%s\n' "$P/.jit" "$BASE/a/wt" agent:wt)"
  # shellcheck disable=SC2016 # expanded by the evaluating shell
  out=$(cd "$P" && env -u JIT_AGENT_ID "$shell" -c \
    'eval "$("$1" "$2")" || exit 1; '"$probe" _ "$shell" "$ENTRY")
  expect "$shell primary eval" "$out" "$(printf '%s\n' "$P/.jit" "$P" '<unset>')"
  printed=$(cd "$P" && env -u JIT_AGENT_ID "$shell" "$ENTRY")
  case $printed in *JIT_AGENT_ID*) fail "$shell primary printed an identity: $printed" ;; esac

  # --- a5773e5b: a gate command on a leased issue runs as the lease holder -
  run_jit() { # $1: directory, then the jit arguments; identity unset
    dir=$1
    shift
    (cd "$dir" && env -u JIT_AGENT_ID "$shell" "$ENTRY" "$FAKE_JIT" "$@")
  }
  out=$(run_jit "$BASE/a/wt/sub" gate evaluate leased cargo-ci 2>"$TMP/err")
  expect "$shell lease holder" "$out" agent:holder
  grep -qF 'agent:holder' "$TMP/err" || fail "$shell lease holder note: $(cat "$TMP/err")"
  for sub in eval evaluate-all fail add status status-all; do
    out=$(run_jit "$BASE/a/wt" gate "$sub" leased 2>/dev/null)
    expect "$shell lease holder $sub" "$out" agent:holder
  done
  out=$(run_jit "$BASE/a/wt" gate evaluate --json --by lead --gate cargo-ci leased 2>/dev/null)
  expect "$shell lease holder after options" "$out" agent:holder
  # Without a lease, a lookup failure or a gate command, the default stands.
  out=$(run_jit "$BASE/a/wt" gate evaluate free cargo-ci 2>"$TMP/err")
  expect "$shell unleased gate" "$out" agent:wt
  [ ! -s "$TMP/err" ] || fail "$shell unleased gate wrote: $(cat "$TMP/err")"
  out=$(run_jit "$BASE/a/wt" gate evaluate broken cargo-ci 2>/dev/null)
  expect "$shell failed lookup" "$out" agent:wt
  out=$(run_jit "$BASE/a/wt" issue show leased)
  expect "$shell non-gate command" "$out" agent:wt
  out=$(run_jit "$BASE/a/wt" gate evaluate-many cargo-ci leased 2>/dev/null)
  expect "$shell multi-issue gate command" "$out" agent:wt
  # A caller-set identity and the primary checkout are unchanged.
  out=$(cd "$BASE/a/wt" && JIT_AGENT_ID=agent:caller "$shell" "$ENTRY" "$FAKE_JIT" gate evaluate leased)
  expect "$shell caller id over lease" "$out" agent:caller
  out=$(run_jit "$P" gate evaluate leased 2>/dev/null)
  expect "$shell primary leased" "$out" '<unset>'

  # --- REQ-05: the command's exit status passes through ---------------------
  rc=0
  (cd "$BASE/a/wt/sub" && "$shell" "$ENTRY" sh -c 'exit 7') || rc=$?
  expect "$shell exit status" "$rc" 7

  # --- REQ-04: refusals run nothing and name the store ----------------------
  refuse() { # $1: label, $2: directory, $3: store the message names
    rm -f "$TMP/ran"
    rc=0
    (cd "$2" && "$shell" "$ENTRY" touch "$TMP/ran") 2>"$TMP/err" || rc=$?
    [ "$rc" -ne 0 ] || fail "$shell $1: exit status 0"
    [ ! -e "$TMP/ran" ] || fail "$shell $1: the command ran"
    grep -qF "$3" "$TMP/err" || fail "$shell $1: message does not name $3: $(cat "$TMP/err")"
    rc=0
    (cd "$2" && "$shell" "$ENTRY") >"$TMP/out" 2>/dev/null || rc=$?
    [ "$rc" -ne 0 ] || fail "$shell $1: print form exit status 0"
    [ ! -s "$TMP/out" ] || fail "$shell $1: print form printed $(cat "$TMP/out")"
  }
  refuse "missing primary store" "$BASE/empty-wt" "$EMPTY/.jit"
  refuse "missing primary store from primary" "$EMPTY" "$EMPTY/.jit"
  refuse "stale linked store" "$STALE" "$STALE/.jit"
done

echo "tracker-env self-test passed"
