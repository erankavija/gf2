#!/usr/bin/env bash
# Self-test: fake harness and fake flock wrapper; no build or GPU.
set -euo pipefail
THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$THIS_DIR/permanent-campaign-runner.sh"
ROOT="$(cd "$THIS_DIR/../.." && pwd)"
WORK="$(mktemp -d)"
# The suite drives the real runner against this repository root, and measure
# refuses any dirty or untracked state, so a clean tree is a prerequisite for
# every case after the two refusal tests.
if [[ -n "$(git -C "$ROOT" status --porcelain --untracked-files=all)" ]]; then
    echo "PREREQ: this self-test needs a clean worktree (tracked and untracked); commit or clean first" >&2
    rm -rf "$WORK"
    exit 2
fi
BACKUP="$WORK/README.md"
cp "$ROOT/README.md" "$BACKUP"
trap 'cp "$BACKUP" "$ROOT/README.md"; rm -rf "$WORK"' EXIT
PASS=0

assert_rc() { [[ "$1" -eq "$2" ]] || { echo "FAIL: $3 (expected $1, got $2)"; return 1; }; }
assert_has() { [[ "$1" == *"$2"* ]] || { echo "FAIL: $3 (missing $2)"; return 1; }; }
assert_matches() { [[ "$1" =~ $2 ]] || { echo "FAIL: $3 (no match for $2)"; return 1; }; }

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
# The execution-id trigger applies only when the caller set one: grid is the
# only step the runner gives --execution-id, so an unguarded comparison would
# match every other step on the empty string and fail the whole pipeline.
if [[ "${CAMPAIGN_TEST_FAIL_STEP:-}" == "$step" ]] \
    || { [[ -n "${CAMPAIGN_TEST_FAIL_EXECUTION_ID:-}" ]] && [[ "${CAMPAIGN_TEST_FAIL_EXECUTION_ID}" == "$execution_id" ]]; }; then
    exit 42
fi
if [[ "$step" == equivalence ]]; then
    echo 'q,n,reference,backend,matrices,mismatches,zeros_reference,zeros_backend,status' > "$out"
    for q in 3 5 7; do
        mismatches=0; status=identical
        if [[ "${CAMPAIGN_TEST_EQUIVALENCE_MISMATCH_Q:-}" == "$q" ]]; then
            mismatches=1; status=MISMATCH
        fi
        echo "$q,12,scalar,stub,1,$mismatches,0,0,$status" >> "$out"
    done
    exit 0
fi
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

# A wrapper that dirties the repository after the pre-lock checks have passed
# and before the lock-held child starts, standing in for a change landing while
# this run waited on the shared host mutex.
stub_flock_dirtying() {
    local path="$1"
    cat > "$path" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --full-host ]] || exit 91
touch "$CAMPAIGN_TEST_LOCK_DIRTY_MARKER"
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
    assert_rc 2 "$r" t1; assert_has "$o" 'clean worktree (tracked and untracked)' t1
    echo 'PASS: tracked dirty refusal'; PASS=$((PASS + 1))
}

t2() {
    local d="$WORK/t2" h="$WORK/h" f="$WORK/f" marker="$ROOT/.permanent-campaign-runner-untracked-$BASHPID"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    touch "$marker"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" untracked 2>&1); r=$?; set -e
    rm -f "$marker"
    assert_rc 2 "$r" t2; assert_has "$o" 'clean worktree (tracked and untracked)' t2; assert_has "$o" 'permanent-campaign-runner-untracked' t2
    echo 'PASS: untracked refusal'; PASS=$((PASS + 1))
}

t3() {
    local d="$WORK/t3" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(printf '%064d' 0)"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" hash 2>&1); r=$?; set -e
    assert_rc 2 "$r" t3; assert_has "$o" 'binary hash mismatch' t3
    echo 'PASS: hash mismatch refusal'; PASS=$((PASS + 1))
}

t4() {
    local d="$WORK/t4" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(CAMPAIGN_TEST_FAIL_STEP=grid measure "$d/m" "$h" "$f" "$d/s" censor 2>&1); r=$?; set -e
    assert_rc 7 "$r" t4
    local sum; sum=$(find "$d/s" -name '*.run-summary.txt' -print -quit)
    local text; text=$(cat "$sum")
    # grid is the only step the harness gives an execution id; the rest name
    # the fixed stream addresses their CSV preambles record.
    assert_matches "$text" 'step=grid execution_id=[0-9]+ ' t4
    assert_has "$text" 'step=gray-update execution_id=fixed-streams ' t4
    assert_has "$text" 'step=horizontal-product execution_id=fixed-streams ' t4
    assert_has "$o" 'campaign censored' t4
    echo 'PASS: censoring continues and exits 7'; PASS=$((PASS + 1))
}

t7() {
    local d="$WORK/t7" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(measure "$d/m" "$h" "$f" "$d/s" success 2>&1); r=$?; set -e
    assert_rc 0 "$r" t7; assert_has "$o" 'all pipeline steps completed' t7
    local sum; sum=$(find "$d/s" -name '*.run-summary.txt' -print -quit)
    [[ "$(grep -c 'status=completed' "$sum")" -eq 4 ]] || { echo 'FAIL: t7 completion count'; return 1; }
    # Only grid reserves a stream block per execution, so only grid lines carry
    # a numeric id, one distinct value per field. Every other step, and the
    # shared equivalence reference, records the fixed-stream marker instead.
    local text; text=$(find "$d/s" -name '*.run-summary.txt' -exec cat {} \;)
    [[ "$(printf '%s\n' "$text" | grep -Eo 'step=grid execution_id=[0-9]+' | sort -u | wc -l)" -eq 3 ]] || { echo 'FAIL: t7 grid IDs'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -Ec 'step=(equivalence|gray-update|horizontal-product) execution_id=fixed-streams ')" -eq 7 ]] || { echo 'FAIL: t7 fixed-stream markers'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -Ec 'step=(equivalence|gray-update|horizontal-product) execution_id=[0-9]')" -eq 0 ]] || { echo 'FAIL: t7 synthetic id on a fixed-stream step'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -c '^shared_equivalence=execution_id=fixed-streams ')" -eq 3 ]] || { echo 'FAIL: t7 shared equivalence marker'; return 1; }
    echo 'PASS: success exit, numeric grid ids, fixed-stream markers'; PASS=$((PASS + 1))
}

t5() {
    local d="$WORK/t5" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(CAMPAIGN_TEST_EQUIVALENCE_MISMATCH_Q=5 measure "$d/m" "$h" "$f" "$d/s" equivalence-censor 2>&1); r=$?; set -e
    assert_rc 7 "$r" t5
    local text; text=$(find "$d/s" -name '*.run-summary.txt' -exec cat {} \;)
    [[ "$(printf '%s\n' "$text" | grep -c '^shared_equivalence=')" -eq 3 ]] || { echo 'FAIL: t5 shared equivalence references'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -c 'q=5 .*status=skipped_equivalence_failed')" -eq 3 ]] || { echo 'FAIL: t5 skipped timing steps'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -Ec 'q=3 step=(grid|gray-update|horizontal-product).*status=completed')" -eq 3 ]] || { echo 'FAIL: t5 q=3 timing count'; return 1; }
    [[ "$(printf '%s\n' "$text" | grep -Ec 'q=7 step=(grid|gray-update|horizontal-product).*status=completed')" -eq 3 ]] || { echo 'FAIL: t5 q=7 timing count'; return 1; }
    assert_has "$o" 'campaign censored' t5
    echo 'PASS: global equivalence q=5 mismatch skips only q=5'; PASS=$((PASS + 1))
}

t6() {
    local d="$WORK/t6" h="$WORK/h" f="$WORK/f"; mkdir -p "$d"
    stub_harness "$h"; stub_flock "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    set +e; local o r; o=$(CAMPAIGN_TEST_FAIL_STEP=equivalence measure "$d/m" "$h" "$f" "$d/s" equivalence-failure 2>&1); r=$?; set -e
    assert_rc 7 "$r" t6
    local text; text=$(find "$d/s" -name '*.run-summary.txt' -exec cat {} \;)
    [[ "$(printf '%s\n' "$text" | grep -c 'status=skipped_equivalence_failed')" -eq 9 ]] || { echo 'FAIL: t6 all timing steps skipped'; return 1; }
    echo 'PASS: equivalence process failure without CSV skips all fields'; PASS=$((PASS + 1))
}

t8() {
    local d="$WORK/t8" h="$WORK/h" f="$WORK/f-dirtying"
    local marker="$ROOT/.permanent-campaign-runner-lock-race-$BASHPID"; mkdir -p "$d"
    stub_harness "$h"; stub_flock_dirtying "$f"; manifest "$d/m" "$h" "$(sha256sum "$h" | awk '{print $1}')"
    # The tree is clean when measure runs its pre-lock checks, so the refusal
    # below can only come from the revalidation inside the lock-held child.
    set +e; local o r
    o=$(CAMPAIGN_TEST_LOCK_DIRTY_MARKER="$marker" measure "$d/m" "$h" "$f" "$d/s" lock-race 2>&1); r=$?
    set -e
    rm -f "$marker"
    assert_rc 2 "$r" t8
    assert_has "$o" 'clean worktree (tracked and untracked)' t8
    assert_has "$o" 'permanent-campaign-runner-lock-race' t8
    # run_campaign creates the study root before taking the lock; refusing under
    # the lock must leave it without a single run summary.
    [[ -z "$(find "$d/s" -name '*.run-summary.txt' 2>/dev/null)" ]] || { echo 'FAIL: t8 ran a step after refusing'; return 1; }
    echo 'PASS: dirt landing during the lock wait is refused under the lock'; PASS=$((PASS + 1))
}

t1
t2
t3
t4
t5
t6
t7
t8
echo "PASS: $PASS/8 permanent-campaign-runner tests"
