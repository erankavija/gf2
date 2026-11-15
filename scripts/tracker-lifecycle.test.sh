#!/bin/sh
# Tests tracker mounting and locked snapshots in scratch repositories.
# Usage: sh scripts/tracker-lifecycle.test.sh; exits 0 on success, 1 on failure.
# Isolates inherited git and tracker settings; caller repositories stay untouched.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/tracker-lifecycle-test.XXXXXX")
trap 'rm -rf "$TMP"' 0
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
expect() { [ "$2" = "$3" ] || fail "$1: got [$2], expected [$3]"; }

unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CEILING_DIRECTORIES \
  GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export JIT_DATA_DIR="$TMP/unrelated-data" JIT_WORKTREE_ROOT="$TMP/unrelated-root"
export JIT_AGENT_ID=agent:outer
TEST_GIT=$(command -v git)
TEST_LOCK="$TMP/lock-held"
TEST_CALLS="$TMP/jit-calls"
export TEST_GIT TEST_LOCK TEST_CALLS
mkdir -p "$TMP/bin" "$TMP/sp ace'q"
BASE=$(cd "$TMP/sp ace'q" && pwd -P)

cat >"$TMP/bin/jit" <<'SH'
#!/bin/sh
set -eu
[ "$JIT_DATA_DIR" = "$TEST_PRIMARY/.jit" ] || exit 90
[ "$JIT_WORKTREE_ROOT" = "$TEST_CODE" ] || exit 91
[ "$JIT_AGENT_ID" = agent:outer ] || exit 92
case $* in
  validate)
    [ "$(pwd -P)" = "$TEST_CODE" ] || exit 93
    printf 'validate\n' >>"$TEST_CALLS"
    if [ "${TEST_VALIDATE_FAIL:-0}" = 1 ]; then
      echo 'synthetic invalid tracker' >&2
      exit 23
    fi
    ;;
  'worktree exec -- '*)
    printf 'lock\n' >>"$TEST_CALLS"
    [ "${TEST_REFUSE_LOCK:-0}" = 0 ] || exit 24
    shift 3
    : >"$TEST_LOCK"
    trap 'rm -f "$TEST_LOCK"' 0
    rc=0
    "$@" || rc=$?
    exit "$rc"
    ;;
  *) echo "unexpected jit invocation: $*" >&2; exit 94 ;;
esac
SH
cat >"$TMP/bin/git" <<'SH'
#!/bin/sh
set -eu
if [ "${TEST_CHECK_LOCK:-0}" = 1 ]; then
  for arg do
    case $arg in
      add|commit) [ -f "$TEST_LOCK" ] || exit 95 ;;
    esac
  done
fi
exec "$TEST_GIT" "$@"
SH
chmod +x "$TMP/bin/jit" "$TMP/bin/git"
PATH="$TMP/bin:$PATH"
export PATH

init_repo() {
  git init -q --initial-branch=main "$1"
  git -C "$1" config user.name test
  git -C "$1" config user.email test@example.invalid
  git -C "$1" config commit.gpgsign false
}
make_code() {
  init_repo "$1"
  mkdir -p "$1/scripts" "$1/sub/dir"
  cp "$ROOT/scripts/tracker-env" "$1/scripts/"
  for name in tracker-bootstrap tracker-snapshot; do
    if [ -f "$ROOT/scripts/$name" ]; then
      cp "$ROOT/scripts/$name" "$1/scripts/"
    fi
  done
  printf '/.jit/\n' >"$1/.gitignore"
  : >"$1/code"
  git -C "$1" add .
  git -C "$1" commit -qm init
}
bootstrap() { (cd "$TEST_CODE/sub/dir" && "$TEST_CODE/scripts/tracker-bootstrap" "$@"); }
snapshot() { (cd "$TEST_CODE/sub/dir" && TEST_CHECK_LOCK=1 "$TEST_CODE/scripts/tracker-snapshot"); }

TRACKER="$BASE/tracker"
init_repo "$TRACKER"
printf '{}\n' >"$TRACKER/index.json"
printf 'old\n' >"$TRACKER/record"
: >"$TRACKER/remove-me"
cat >"$TRACKER/.gitignore" <<'EOF'
# jit:data-root-ignore:begin
/tmp/
# jit:data-root-ignore:end

*.lock
gate-runs/
EOF
git -C "$TRACKER" add .
git -C "$TRACKER" commit -qm init
CODE="$BASE/code"
make_code "$CODE"
TEST_PRIMARY=$CODE TEST_CODE=$CODE
export TEST_PRIMARY TEST_CODE

# A valid mount must run validation through the entry point from the code root.
bootstrap "$TRACKER" >"$TMP/findings" 2>&1 ||
  fail "valid mount: $(cat "$TMP/findings")"
expect 'mounted origin' "$(git -C "$CODE/.jit" remote get-url origin)" "$TRACKER"
expect 'mount validation' "$(cat "$TEST_CALLS")" validate
[ -f "$CODE/.jit/index.json" ] || fail 'mount lost the store index'
rm -rf "$CODE/.jit"
: >"$TEST_CALLS"

rc=0
bootstrap >"$TMP/findings" 2>&1 || rc=$?
[ "$rc" -ne 0 ] || fail 'mount without an argument succeeded'
[ ! -e "$CODE/.jit" ] || fail 'mount without an argument created a store'

for form in directory file symlink; do
  case $form in
    directory) mkdir "$CODE/.jit"; : >"$CODE/.jit/keep" ;;
    file) printf 'keep\n' >"$CODE/.jit" ;;
    symlink) ln -s "$TMP/missing" "$CODE/.jit" ;;
  esac
  before=$(git -C "$CODE" status --porcelain --ignored -uall)
  rc=0
  bootstrap "$TRACKER" >"$TMP/findings" 2>&1 || rc=$?
  [ "$rc" -ne 0 ] || fail "mount over existing $form succeeded"
  expect "existing $form unchanged" "$(git -C "$CODE" status --porcelain --ignored -uall)" "$before"
  case $form in
    directory) [ -f "$CODE/.jit/keep" ] || fail 'existing directory changed' ;;
    file) expect 'existing file' "$(cat "$CODE/.jit")" keep ;;
    symlink) expect 'existing symlink' "$(readlink "$CODE/.jit")" "$TMP/missing" ;;
  esac
  rm -rf "$CODE/.jit"
done

# Minimal linked-checkout metadata exercises git discovery without worktree commands.
LINKED="$BASE/linked"
mkdir -p "$LINKED/sub/dir" "$CODE/.git/worktrees/linked"
cp -R "$CODE/scripts" "$LINKED/scripts"
printf 'gitdir: %s/.git/worktrees/linked\n' "$CODE" >"$LINKED/.git"
printf '%s/.git\n' "$LINKED" >"$CODE/.git/worktrees/linked/gitdir"
printf '../..\n' >"$CODE/.git/worktrees/linked/commondir"
printf 'ref: refs/heads/main\n' >"$CODE/.git/worktrees/linked/HEAD"
TEST_CODE=$LINKED
rc=0
bootstrap "$TRACKER" >"$TMP/findings" 2>&1 || rc=$?
[ "$rc" -ne 0 ] || fail 'mount in linked checkout succeeded'
[ ! -e "$CODE/.jit" ] && [ ! -e "$LINKED/.jit" ] || fail 'linked mount changed a checkout'
[ ! -s "$TEST_CALLS" ] || fail 'refused mount invoked jit'
TEST_CODE=$CODE

FAILED="$BASE/invalid-mount"
make_code "$FAILED"
TEST_PRIMARY=$FAILED TEST_CODE=$FAILED
rc=0
TEST_VALIDATE_FAIL=1 bootstrap "$TRACKER" >"$TMP/findings" 2>&1 || rc=$?
expect 'validation failure status' "$rc" 23
grep -q 'synthetic invalid tracker' "$TMP/findings" || fail 'validation findings hidden'
[ -f "$FAILED/.jit/index.json" ] || fail 'failed validation removed the mounted store'
git -C "$FAILED/.jit" rev-parse --verify HEAD >/dev/null

TEST_PRIMARY=$CODE TEST_CODE=$CODE
mkdir "$CODE/.jit"
: >"$CODE/.jit/index.json"
head=$(git -C "$CODE" rev-parse HEAD)
before=$(git -C "$CODE" status --porcelain --ignored -uall)
rc=0
snapshot >"$TMP/findings" 2>&1 || rc=$?
[ "$rc" -ne 0 ] || fail 'snapshot accepted a store inside the code repository'
expect 'code HEAD after refusal' "$(git -C "$CODE" rev-parse HEAD)" "$head"
expect 'code status after refusal' "$(git -C "$CODE" status --porcelain --ignored -uall)" "$before"
rm -rf "$CODE/.jit"
bootstrap "$TRACKER" >"$TMP/findings" 2>&1 || fail 'remount failed'
git -C "$CODE/.jit" config user.name test
git -C "$CODE/.jit" config user.email test@example.invalid
git -C "$CODE/.jit" config commit.gpgsign false

# HEAD differs from main, so the subject must resolve the named code ref.
main=$(git -C "$CODE" rev-parse --short=12 refs/heads/main)
git -C "$CODE" symbolic-ref HEAD refs/heads/topic
printf 'topic\n' >"$CODE/code"
git -C "$CODE" add code
git -C "$CODE" commit -qm topic

head=$(git -C "$CODE/.jit" rev-parse HEAD)
snapshot >"$TMP/findings" 2>&1 || fail 'clean snapshot failed'
expect 'clean snapshot HEAD' "$(git -C "$CODE/.jit" rev-parse HEAD)" "$head"

printf 'new\n' >"$CODE/.jit/record"
: >"$CODE/.jit/added"
rm "$CODE/.jit/remove-me"
mkdir -p "$CODE/.jit/tmp" "$CODE/.jit/gate-runs" "$CODE/.jit/nested"
for local in write.lock nested/write.lock tmp/local gate-runs/local; do
  : >"$CODE/.jit/$local"
done
rc=0
TEST_REFUSE_LOCK=1 snapshot >"$TMP/findings" 2>&1 || rc=$?
expect 'lock refusal status' "$rc" 24
expect 'lock refusal HEAD' "$(git -C "$CODE/.jit" rev-parse HEAD)" "$head"
git -C "$CODE/.jit" diff --cached --quiet || fail 'lock refusal staged changes'

cat >"$CODE/.jit/.git/hooks/pre-commit" <<'SH'
#!/bin/sh
[ -f "$TEST_LOCK" ] || exit 96
[ "${TEST_COMMIT_FAIL:-0}" = 0 ] || exit 25
SH
chmod +x "$CODE/.jit/.git/hooks/pre-commit"
rc=0
TEST_COMMIT_FAIL=1 snapshot >"$TMP/findings" 2>&1 || rc=$?
[ "$rc" -ne 0 ] || fail 'commit failure returned success'
expect 'failed commit HEAD' "$(git -C "$CODE/.jit" rev-parse HEAD)" "$head"

# Snapshots do not validate branch-dependent document links.
TEST_CODE=$LINKED
start=$(date -u +%Y-%m-%dT%H:%M:%SZ)
TEST_VALIDATE_FAIL=1 snapshot >"$TMP/findings" 2>&1 ||
  fail "changed snapshot: $(cat "$TMP/findings")"
end=$(date -u +%Y-%m-%dT%H:%M:%SZ)
subject=$(git -C "$CODE/.jit" log -1 --format=%s)
printf '%s\n' "$subject" | grep -Eq "^snapshot [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z main $main$" ||
  fail "snapshot subject: $subject"
stamp=${subject#snapshot }
stamp=${stamp% main *}
expect 'UTC timestamp in invocation interval' \
  "$(printf '%s\n%s\n%s\n' "$start" "$stamp" "$end" | LC_ALL=C sort | sed -n 2p)" "$stamp"
expect 'snapshot parent' "$(git -C "$CODE/.jit" rev-parse HEAD^)" "$head"
expect 'snapshot contents' "$(git -C "$CODE/.jit" ls-tree --name-only HEAD)" \
  "$(printf '.gitignore\nadded\nindex.json\nrecord')"
expect 'snapshot changed contents' "$(git -C "$CODE/.jit" show HEAD:record)" new
git -C "$CODE/.jit" diff --quiet || fail 'snapshot left unstaged changes'
git -C "$CODE/.jit" diff --cached --quiet || fail 'snapshot left staged changes'
head=$(git -C "$CODE/.jit" rev-parse HEAD)
snapshot >"$TMP/findings" 2>&1 || fail 'snapshot with only ignored files failed'
expect 'ignored-only snapshot HEAD' "$(git -C "$CODE/.jit" rev-parse HEAD)" "$head"

echo 'tracker lifecycle self-test passed'
