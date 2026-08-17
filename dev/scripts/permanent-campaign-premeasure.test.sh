#!/usr/bin/env bash
# Self-test for premeasure mode. The fake harness never measures hardware.
set -euo pipefail
THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$THIS_DIR/permanent-campaign-runner.sh"
ROOT="$(cd "$THIS_DIR/../.." && pwd)"
WORK="$(mktemp -d)"
REPO="$WORK/fixture"
PLAN="$ROOT/dev/benchmarks/permanent_campaign/premeasure-plan-v1.csv"
mkdir -p "$REPO"
git -C "$REPO" init --quiet
git -C "$REPO" config user.email test@example.invalid
git -C "$REPO" config user.name premeasure-test
printf 'fixture\n' > "$REPO/README.md"
git -C "$REPO" add README.md
git -C "$REPO" commit --quiet -m 'fixture'
HARNESS="$WORK/harness"
FLOCK="$WORK/flock"
LOG="$WORK/harness.log"
cat > "$HARNESS" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
out=''
execution_id=''
for ((i=1; i<=$#; i++)); do
    [[ "${!i}" == --out ]] && { j=$((i + 1)); out="${!j}"; }
    [[ "${!i}" == --execution-id ]] && { j=$((i + 1)); execution_id="${!j}"; }
done
printf '%s\n' "$execution_id" >> "$CAMPAIGN_TEST_LOG"
if [[ "${CAMPAIGN_TEST_FAIL_EXEC_ID:-}" == "$execution_id" ]]; then
    exit 42
fi
[[ -n "$out" ]]
sleep "${CAMPAIGN_TEST_SLEEP:-0}"
mkdir -p "$(dirname "$out")"
printf 'q,n,backend,outcome,batch_size,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s\n' > "$out"
printf '3,4,cpu_rayon_batch_scalar,measured,96,5,480,10,5,1,3,0.5,0.5,96,160\n' >> "$out"
STUB
chmod +x "$HARNESS"
cat > "$FLOCK" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --full-host ]]
shift
exec "$@"
STUB
chmod +x "$FLOCK"
MANIFEST="$WORK/manifest"
printf 'manifest_version=1\nharness|%s|\nbinary|%s|%s\n' "$HARNESS" "$HARNESS" "$(sha256sum "$HARNESS" | awk '{print $1}')" > "$MANIFEST"
BASE=(env
    CAMPAIGN_REPO_ROOT="$REPO"
    CAMPAIGN_PREMEASURE_PLAN="$PLAN"
    CAMPAIGN_MANIFEST="$MANIFEST"
    CAMPAIGN_FLOCK_WRAPPER="$FLOCK"
    CAMPAIGN_HARNESS_BIN="$HARNESS"
    CAMPAIGN_TEST_LOG="$LOG")
run_pre() {
    local target="$1" run_id="$2" cap="$3"
    CAMPAIGN_TARGET_ROOT="$target" CAMPAIGN_RUN_ID="$run_id" CAMPAIGN_TEST_SLEEP=0.03         "${BASE[@]}" "$SCRIPT" premeasure --session-cap "$cap"
}
assert_has() { [[ "$1" == *"$2"* ]] || { echo "FAIL: $3 (missing $2)"; return 1; }; }
PASS=0

t1() {
    local rows cells
    rows=$(awk -F, 'NR > 1 { n++ } END { print n }' "$PLAN")
    cells=$(awk -F, 'NR > 1 { c[$1 ":" $2]++ } END { for (k in c) { if (c[k] != 2) exit 1; n++ } print n }' "$PLAN")
    [[ "$rows" -eq 120 && "$cells" -eq 60 ]]
    [[ "$(awk -F, 'NR > 1 && $11 == "A B B A" { n++ } END { print n }' "$PLAN")" -eq 120 ]]
    [[ $((rows * 12)) -eq 1440 ]]
    echo 'PASS: plan parsing 120 configurations, 60 cells, 1440 processes, balanced blocks'
    PASS=$((PASS + 1))
}

t2() {
    local target="$WORK/resume"
    local r0="$target/premeasure-resume/processes/process-0000-3-4-A" r1="$target/premeasure-resume/processes/process-0001-3-4-B"
    mkdir -p "$r0" "$r1"
    printf 'status: completed\n' > "$r0/receipt.txt"; printf '0\n' > "$r0/exit.status"; : > "$r0/scratch.csv"
    printf 'status: failed\n' > "$r1/receipt.txt"; printf '42\n' > "$r1/exit.status"
    set +e
    local o
    o=$(run_pre "$target" resume 1 2>&1)
    local rc=$?
    set -e
    [[ "$rc" -eq 0 || "$rc" -eq 7 ]]
    assert_has "$o" 'premeasure skip: process=0' t2
    assert_has "$o" 'premeasure skip: process=1' t2
    grep -qx '0' "$LOG" && return 1
    grep -qx '1' "$LOG" && return 1
    grep -qx '2' "$LOG"
    echo 'PASS: completed receipt skipped and failed receipt not repeated'
    PASS=$((PASS + 1))
}

t3() {
    local target="$WORK/cap"
    local o
    o=$(run_pre "$target" cap 1 2>&1)
    assert_has "$o" 'stopped cleanly at session cap 1s' t3
    [[ -n "$(find "$target/premeasure-cap/sessions" -name 'session-*.status' -print -quit)" ]]
    echo 'PASS: session cap stops cleanly after current process'
    PASS=$((PASS + 1))
}

t4() {
    printf 'dirty\n' >> "$REPO/README.md"
    set +e
    local o
    o=$(run_pre "$WORK/dirty" dirty 1 2>&1)
    local rc=$?
    set -e
    git -C "$REPO" checkout -- README.md
    [[ "$rc" -eq 2 ]]
    assert_has "$o" 'premeasure requires a clean worktree' t4
    echo 'PASS: dirty-tree pre-flight refusal'
    PASS=$((PASS + 1))
}

make_one_receipt_per_config() {
    local target="$1" run_id="$2"
    local root="$target/premeasure-$run_id"
    mkdir -p "$root/processes" "$root/sessions"
    local cell_index=0 q n backend batch arm_count code process_index
    local previous=''
    while IFS=, read -r q n _token backend batch _count _rest; do
        [[ "$q" == q ]] && continue
        if [[ "$q:$n" != "$previous" ]]; then
            previous="$q:$n"
            cell_index=$((cell_index + 1))
            arm_count=0
        fi
        code=A
        [[ "$arm_count" -eq 1 ]] && code=B
        process_index=$(( (cell_index - 1) * 24 + (arm_count == 0 ? 0 : 1) ))
        local receipt
        receipt="$root/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
        mkdir -p "$receipt"
        printf 'schema_version: 1\nstatus: completed\nexit_status: 0\nstarted_utc: 2026-08-17T00:00:00Z\nmachine_warmup: skipped\n' > "$receipt/receipt.txt"
        printf '0\n' > "$receipt/exit.status"
        printf 'q,n,backend,outcome,batch_size,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s\n3,4,%s,measured,%s,5,480,10,5,1,3,0.5,0.5,96,160\n' "$backend" "$batch" > "$receipt/scratch.csv"
        arm_count=$((arm_count + 1))
    done < <(tail -n +2 "$PLAN" | sort -t, -k1,1n -k2,2n -k3,3)
}

t5() {
    local target="$WORK/collect" run_id=collect
    make_one_receipt_per_config "$target" "$run_id"
    local o
    o=$(CAMPAIGN_TARGET_ROOT="$target" CAMPAIGN_RUN_ID="$run_id" "${BASE[@]}" "$SCRIPT" premeasure-collect 2>&1)
    assert_has "$o" 'completeness 3:4:A: 1/12' t5
    assert_has "$o" 'completeness 7:19:B: 1/12' t5
    [[ "$(tail -n +2 "$target/premeasure-$run_id/premeasure-candidates.csv" | wc -l)" -eq 120 ]]
    local zero_target="$WORK/zero" zero_run=zero
    mkdir -p "$zero_target/premeasure-$zero_run/processes"
    set +e
    o=$(CAMPAIGN_TARGET_ROOT="$zero_target" CAMPAIGN_RUN_ID="$zero_run" "${BASE[@]}" "$SCRIPT" premeasure-collect 2>&1)
    local rc=$?
    set -e
    [[ "$rc" -eq 2 ]]
    assert_has "$o" 'zero completed processes' t5
    echo 'PASS: collector reports 1/12 completeness and refuses zero-complete configurations'
    PASS=$((PASS + 1))
}

trap 'rm -rf "$WORK"' EXIT
t1
t2
t3
t4
t5
echo "PASS: $PASS/5 premeasure tests"
