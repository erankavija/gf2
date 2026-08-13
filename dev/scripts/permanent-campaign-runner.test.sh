#!/usr/bin/env bash
# Self-test: fake harness and fake flock wrapper; no build or GPU.
set -euo pipefail
THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$THIS_DIR/permanent-campaign-runner.sh"
ROOT="$(cd "$THIS_DIR/../.." && pwd)"
WORK="$(mktemp -d)"
BACKUP="$WORK/README.md"
cp "$ROOT/README.md" "$BACKUP"
trap 'cp "$BACKUP" "$ROOT/README.md"; rm -rf "$WORK"' EXIT
PASS=0

assert_rc() { [[ "$1" -eq "$2" ]] || { echo "FAIL: $3 (expected $1, got $2)"; return 1; }; }
assert_has() { [[ "$1" == *"$2"* ]] || { echo "FAIL: $3 (missing $2)"; return 1; }; }

stub_harness() {
    local path="$1"
    cat > "$path" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
step="$1"; out=''; execution_id=''
for ((i=1; i<=$#; i++)); do
    [[ "${!i}" == --out ]] && { j=$((i + 1)); out="${!j}"; }
    [[ "${!i}" == --execution-id ]] && { j=$((i + 1)); execution_id="${!j}"; }
done
if [[ "${CAMPAIGN_TEST_FAIL_STEP:-}" == "$step" || "${CAMPAIGN_TEST_FAIL_EXECUTION_ID:-}" == "$execution_id" ]]; then exit 42; fi
mkdir -p "$(dirname "$out")"; echo "stub $step" > "$out"
STUB
    chmod +x "$path"
}

stub_flock() {
    local path="$1"
    cat > "$path" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --full-host ]] || exit 91
shift; exec "$@"
STUB
    chmod +x "$path"
}

manifest() {
    local path="$1" harness="$2" hash="$3"
    printf 'manifest_version=1\nharness|%s|\nbinary|%s|%s\n' "$harness" "$harness" "$hash" > "$path"
}

measure() {
    local m="$1" h="$2" f="$3" s="$4" id="$5"
    CAMPAIGN_MANIFEST="$m" CAMPAIGN_HARNESS_BIN="$h" CAMPAIGN_FLOCK_WRAPPER="$f" \
    CAMPAIGN_STUDY_ROOT="$s" CAMPAIGN_RUN_ID="$id" "$SCRIPT" measure
}

t1() {
    local d="$WORK/t1" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    printf '\nrunner self-test marker\n' >> "$ROOT/README.md"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" dirty 2>&1); r=$?; set -e
    cp "$BACKUP" "$ROOT/README.md"
    assert_rc 2 "$r" t1; assert_has "$o" 'clean tracked worktree' t1
    echo 'PASS: dirty refusal'; PASS=$((PASS + 1))
}

t2() {
    local d="$WORK/t2" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(printf '%064d' 0)"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" hash 2>&1); r=$?; set -e
    assert_rc 2 "$r" t2; assert_has "$o" 'binary hash mismatch' t2
    echo 'PASS: hash mismatch refusal'; PASS=$((PASS + 1))
}

t3() {
    local d="$WORK/t3" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(CAMPAIGN_TEST_FAIL_STEP=grid measure "$d/m" "$h" "$f" "$d/s" censor 2>&1); r=$?; set -e
    assert_rc 7 "$r" t3
    local sum; sum=$(find "$d/s" -name '*.run-summary.txt' -print -quit)
    local text; text=$(cat "$sum")
    assert_has "$text" 'step=grid execution_id=' t3
    assert_has "$text" 'step=gray-update execution_id=' t3
    assert_has "$text" 'step=horizontal-product execution_id=' t3
    assert_has "$o" 'campaign censored' t3
    echo 'PASS: censoring continues and exits 7'; PASS=$((PASS + 1))
}

t4() {
    local d="$WORK/t4" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" success 2>&1); r=$?; set -e
    assert_rc 0 "$r" t4; assert_has "$o" 'all pipeline steps completed' t4
    local sum; sum=$(find "$d/s" -name '*.run-summary.txt' -print -quit)
    [[ "$(grep -c 'status=completed' "$sum")" -eq 4 ]] || { echo 'FAIL: t4 completion count'; return 1; }
    [[ "$(grep 'execution_id=' "$sum" | awk -F'execution_id=' '{print $2}' | awk '{print $1}' | sort -u | wc -l)" -eq 4 ]] || { echo 'FAIL: t4 IDs'; return 1; }
    echo 'PASS: success exit and distinct IDs'; PASS=$((PASS + 1))
}

t5() {
    local d="$WORK/t5" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(CAMPAIGN_TEST_FAIL_EXECUTION_ID=5001 measure "$d/m" "$h" "$f" "$d/s" equivalence-censor 2>&1); r=$?; set -e
    assert_rc 7 "$r" t5
    local text; text=$(find "$d/s" -name '*.run-summary.txt' -exec cat {} \;)
    [[ "$(printf '%s\n' "$text" | grep -c 'q=5 step=equivalence .*status=failed')" -eq 1 ]] || { echo 'FAIL: t5 equivalence failure'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -c 'q=5 .*status=skipped_equivalence_failed')" -eq 3 ]] || { echo 'FAIL: t5 skipped timing steps'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -c 'q=3 .*status=completed')" -eq 4 ]] || { echo 'FAIL: t5 q=3 completed count'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -c 'q=7 .*status=completed')" -eq 4 ]] || { echo 'FAIL: t5 q=7 completed count'; return 1; }
    echo 'PASS: equivalence failure skips one field and continues'; PASS=$((PASS + 1))
}

t1
t2
t3
t4
t5
echo "PASS: $PASS/5 permanent-campaign-runner tests"
