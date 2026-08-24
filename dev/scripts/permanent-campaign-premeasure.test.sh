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
orders=''
admit_only=false
for ((i=1; i<=$#; i++)); do
    [[ "${!i}" == --out ]] && { j=$((i + 1)); out="${!j}"; }
    [[ "${!i}" == --execution-id ]] && { j=$((i + 1)); execution_id="${!j}"; }
    [[ "${!i}" == --orders ]] && { j=$((i + 1)); orders="${!j}"; }
    [[ "${!i}" == --admit-only ]] && admit_only=true
done
printf '%s\n' "$execution_id" >> "$CAMPAIGN_TEST_LOG"
if [[ "$admit_only" == true ]]; then
    if [[ "${CAMPAIGN_TEST_ADMISSION_FAIL_N:-}" == "$orders" ]]; then
        echo "stub admission rejected n=$orders" >&2
        exit 43
    fi
    exit 0
fi
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
    CAMPAIGN_MANIFEST="$MANIFEST"
    CAMPAIGN_FLOCK_WRAPPER="$FLOCK"
    CAMPAIGN_HARNESS_BIN="$HARNESS"
    CAMPAIGN_TEST_LOG="$LOG")
run_pre() {
    local target="$1" run_id="$2" cap="$3"
    CAMPAIGN_TARGET_ROOT="$target" CAMPAIGN_RUN_ID="$run_id" CAMPAIGN_PREMEASURE_PLAN="$PLAN" CAMPAIGN_TEST_SLEEP=0.03         "${BASE[@]}" "$SCRIPT" premeasure --session-cap "$cap"
}
run_pre_plan() {
    local target="$1" run_id="$2" cap="$3" plan="$4" fail_n="$5"
    CAMPAIGN_TARGET_ROOT="$target" CAMPAIGN_RUN_ID="$run_id" CAMPAIGN_PREMEASURE_PLAN="$plan" CAMPAIGN_TEST_ADMISSION_FAIL_N="$fail_n" CAMPAIGN_TEST_SLEEP=0.03         "${BASE[@]}" "$SCRIPT" premeasure --session-cap "$cap"
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
    local r2="$target/premeasure-resume/processes/process-0002-3-4-B"
    mkdir -p "$r0" "$r1" "$r2"
    printf 'status: completed\nsession_id: old-completed\n' > "$r0/receipt.txt"; printf '0\n' > "$r0/exit.status"; : > "$r0/scratch.csv"
    printf 'status: failed\nsession_id: old-failed\nold-byte: preserve-me\n' > "$r1/receipt.txt"; printf '101\n' > "$r1/exit.status"
    printf -- '--only q=3,n=4,backend=stub matched no cell in the grid\n' > "$r1/harness.log"
    printf 'failed receipt bytes\n' > "$r1/falsification.bin"
    printf 'status: failed\nsession_id: old-measure-failed\n' > "$r2/receipt.txt"; printf '42\n' > "$r2/exit.status"
    printf 'harness died mid-measurement\n' > "$r2/harness.log"
    set +e
    local o
    o=$(run_pre "$target" resume 1 2>&1)
    local rc=$?
    set -e
    [[ "$rc" -eq 0 || "$rc" -eq 7 ]]
    assert_has "$o" 'premeasure skip: process=0' t2
    assert_has "$o" 'premeasure supersede: process=1' t2
    [[ "$o" != *'premeasure skip: process=1'* ]]
    assert_has "$o" 'premeasure skip: process=2' t2
    [[ "$o" != *'premeasure supersede: process=2'* ]]
    grep -qx '0' "$LOG" && return 1
    grep -qx '2' "$LOG" && return 1
    grep -qx '1' "$LOG"
    local superseded="$target/premeasure-resume/superseded/process-0001-3-4-B-old-failed"
    [[ -f "$superseded/falsification.bin" ]] && grep -qx 'failed receipt bytes' "$superseded/falsification.bin"
    grep -q -- '--orders 4' "$target/premeasure-resume/processes/process-0001-3-4-B/receipt.txt"
    grep -q '^status: completed$' "$target/premeasure-resume/processes/process-0001-3-4-B/receipt.txt"
    grep -q '^session_id: old-measure-failed$' "$r2/receipt.txt"
    grep -q '^session_id: old-completed$' "$r0/receipt.txt"
    echo 'PASS: refusal receipt superseded and rerun with --orders; completed and measurement-failed receipts stay final'
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

SCHEDULE="$WORK/schedule.csv"
tail -n +2 "$PLAN" | sort -t, -k1,1n -k2,2n -k3,3 | awk -F, '
    BEGIN { process_index = 0 }
    {
        key = $1 ":" $2
        if (key != previous) { previous = key; arm = 0 }
        q[arm] = $1; n[arm] = $2; token[arm] = $3; backend[arm] = $4; batch[arm] = $5
        arm++
        if (arm == 2) {
            split("0 1 1 0", order, " ")
            for (cycle = 0; cycle < 6; cycle++) {
                for (slot = 1; slot <= 4; slot++) {
                    a = order[slot]
                    code = a == 0 ? "A" : "B"
                    print q[a] "," n[a] "," code "," token[a] "," backend[a] "," batch[a] "," process_index
                    process_index++
                }
            }
        }
    }
' > "$SCHEDULE"

PLAN_SHA="$(sha256sum "$PLAN" | awk '{print $1}')"
OBSERVED_GIT_SHA=1111111111111111111111111111111111111111
OBSERVED_HARNESS_SHA=2222222222222222222222222222222222222222
OBSERVED_DEPS_SHA=3333333333333333333333333333333333333333
OBSERVED_BINARY_SHA=4444444444444444444444444444444444444444444444444444444444444444

write_session() {
    local root="$1" file_tag="$2" session_id="$3" run_id="$4" plan_sha="$5"
    local actual_sha="$6" expected_sha="$7"
    local path="$root/sessions/session-$file_tag.provenance.txt"
    mkdir -p "$root/sessions"
    {
        printf 'schema_version: 1\n'
        printf 'campaign_run_id: %s\n' "$run_id"
        printf 'session_id: %s\n' "$session_id"
        printf 'mode: premeasure\n'
        printf 'started_utc: 2026-08-17T00:00:00Z\n'
        printf 'source_revision: session-source-%s\n' "$file_tag"
        printf 'session_cap_seconds: 43200\n'
        printf 'manifest: prepared, manifest "locator"\n'
        printf 'binary_hash: %s %s\n' "$WORK/timed binary, quoted" "$actual_sha"
        printf 'binary_manifest_hash: %s %s\n' "$WORK/timed binary, quoted" "$expected_sha"
        printf 'wrapper_invocation: wrapper --full-host, identity "quoted"\n'
        printf 'exact_plan: %s\n' "$PLAN"
        printf 'plan_sha256: %s\n' "$plan_sha"
    } > "$path"
}

write_scratch() {
    local path="$1" q="$2" n="$3" backend="$4" batch="$5"
    local binary_sha="$6" mode="${7:-normal}" gpu='card0, GPU "quoted"'
    [[ "$mode" != unavailable ]] || gpu=unavailable
    {
        if [[ "$mode" == empty ]]; then
            printf '# git_sha: \n'
        elif [[ "$mode" != missing ]]; then
            printf '# git_sha: %s\n' "$OBSERVED_GIT_SHA"
            [[ "$mode" != duplicate ]] || printf '# git_sha: duplicate-value\n'
        fi
        printf '# git_worktree_dirty: false\n'
        printf '# harness_source_sha: %s\n' "$OBSERVED_HARNESS_SHA"
        printf '# harness_source_dirty: false\n'
        printf '# deps_source_sha: %s\n' "$OBSERVED_DEPS_SHA"
        printf '# deps_source_dirty: false\n'
        printf '# binary_sha256: %s\n' "$binary_sha"
        printf '# rustc: rustc execution-version\n'
        printf '# cargo: cargo execution-version\n'
        printf '# cpu: Test CPU, revision "A"\n'
        printf '# logical_cpus: 24\n'
        printf '# rayon_threads: 24\n'
        printf '# avx2: true, avx512f: false\n'
        printf '# cpu_governor: performance\n'
        printf '# gpu: %s\n' "$gpu"
        printf '# rocm: 7.test\n'
        printf '# hip_feature: true\n'
        printf '# kernel: test-kernel\n'
        printf '# timestamp_utc: 2026-08-17T00:00:01Z\n'
        printf '# invocation: harness grid, label "quoted"\n'
        if [[ "$mode" == shape ]]; then
            printf 'q,n,wrong,outcome,batch_size,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s\n'
        else
            printf 'q,n,backend,outcome,batch_size,reps,matrices,zeros,total_s,gen_s,eval_s,reduce_s,store_s,composite_matrices_per_s,eval_matrices_per_s\n'
        fi
        printf '%s,%s,%s,measured,%s,5,480,10,5,1,3,0.5,0.5,96,160\n' "$q" "$n" "$backend" "$batch"
    } > "$path"
}

write_receipt() {
    local root="$1" run_id="$2" session_id="$3" process_index="$4"
    local q="$5" n="$6" code="$7" token="$8" backend="$9" batch="${10}"
    local status="${11}" receipt_exit="${12}"
    local file_exit="${13:-$receipt_exit}"
    local shape="${14:-normal}"
    local receipt="$root/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code"
    local csv="$receipt/scratch.csv"
    mkdir -p "$receipt"
    {
        printf 'schema_version: 1\n'
        printf 'run_id: %s\n' "$run_id"
        [[ "$session_id" == - ]] || printf 'session_id: %s\n' "$session_id"
        printf 'schedule_position: %s\n' "$process_index"
        printf 'q: %s\nn: %s\nconfig_code: %s\n' "$q" "$n" "$code"
        printf 'manifest_backend: %s\nharness_backend: %s\nbatch_size: %s\n' "$token" "$backend" "$batch"
        printf 'machine_warmup: skipped\n'
        printf 'started_utc: 2026-08-17T00:00:01Z\n'
        printf 'command: harness grid, identity "quoted"\n'
        printf 'scratch_csv: %s\n' "$csv"
        printf 'status: %s\n' "$status"
        case "$shape" in
            signal)
                printf 'failure: interrupted by runner signal\n'
                ;;
            orphan)
                printf 'exit_status: %s\n' "$receipt_exit"
                printf 'failure: interrupted before final receipt\n'
                ;;
            omit_exit)
                printf 'finished_utc: 2026-08-17T00:00:02Z\n'
                ;;
            omit_finished)
                printf 'exit_status: %s\n' "$receipt_exit"
                ;;
            duplicate_exit)
                printf 'exit_status: %s\n' "$receipt_exit"
                printf 'exit_status: %s\n' "$receipt_exit"
                printf 'finished_utc: 2026-08-17T00:00:02Z\n'
                ;;
            duplicate_finished)
                printf 'exit_status: %s\n' "$receipt_exit"
                printf 'finished_utc: 2026-08-17T00:00:02Z\n'
                printf 'finished_utc: 2026-08-17T00:00:02Z\n'
                ;;
            normal)
                printf 'exit_status: %s\n' "$receipt_exit"
                printf 'finished_utc: 2026-08-17T00:00:02Z\n'
                [[ "$status" != failed ]] || printf 'failure: failed, outcome "preserved"\n'
                ;;
            *) return 1 ;;
        esac
    } > "$receipt/receipt.txt"
    printf '%s\n' "$file_exit" > "$receipt/exit.status"
}

run_collect() {
    local target="$1" run_id="$2"
    CAMPAIGN_TARGET_ROOT="$target" CAMPAIGN_RUN_ID="$run_id" \
        CAMPAIGN_PREMEASURE_PLAN="$PLAN" "${BASE[@]}" "$SCRIPT" premeasure-collect
}

declare -a CSV_FIELDS
parse_csv_row() {
    local line="$1" field='' char next i=1
    CSV_FIELDS=()
    [[ "${line:0:1}" == '"' ]]
    while (( i < ${#line} )); do
        char="${line:i:1}"
        if [[ "$char" == '"' ]]; then
            next="${line:i+1:1}"
            if [[ "$next" == '"' ]]; then
                field+='"'
                i=$((i + 2))
            else
                CSV_FIELDS+=("$field")
                field=''
                i=$((i + 1))
                [[ "${line:i:1}" != ',' ]] || i=$((i + 2))
            fi
        else
            field+="$char"
            i=$((i + 1))
        fi
    done
}

csv_field() {
    local path="$1" data_index="$2" column="$3" line header_index=-1 i
    local -a headers
    IFS=, read -r -a headers < "$path"
    for ((i=0; i<${#headers[@]}; i++)); do
        [[ "${headers[$i]}" != "$column" ]] || header_index=$i
    done
    [[ "$header_index" -ge 0 ]]
    line="$(sed -n "$((data_index + 2))p" "$path")"
    parse_csv_row "$line"
    printf '%s\n' "${CSV_FIELDS[$header_index]}"
}

t5() {
    local target="$WORK/bounded-empty" run_id=bounded-empty
    local root="$target/premeasure-bounded-empty"
    mkdir -p "$root/processes"
    set +e
    run_collect "$target" "$run_id" >/dev/null 2>&1
    local rc=$?
    set -e
    [[ "$rc" -eq 7 ]]
    [[ "$(tail -n +2 "$root/premeasure-ledger.csv" | wc -l)" -eq 1440 ]]
    [[ "$(tail -n +2 "$root/premeasure-candidates.csv" | wc -l)" -eq 0 ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 ledger_state)" == pending_missing ]]
    echo 'PASS: incomplete collector publishes a bounded 1440-position ledger and empty candidates file'
    PASS=$((PASS + 1))
}

t6() {
    local bad_plan="$WORK/bad-plan.csv"
    cp "$PLAN" "$bad_plan"
    set +e
    local o
    o=$(run_pre_plan "$WORK/admission" admission 1 "$bad_plan" 4 2>&1)
    local rc=$?
    set -e
    [[ "$rc" -eq 2 ]]
    assert_has "$o" 'inadmissible rows' t6
    assert_has "$o" 'q=3,n=4' t6
    [[ ! -d "$WORK/admission/premeasure-admission/processes" ]]
    echo 'PASS: pre-flight uses binary admission and rejects an inadmissible plan row'
    PASS=$((PASS + 1))
}

t7() {
    local target="$WORK/health"
    run_pre "$target" health 1 >/dev/null 2>&1 || true
    local prov health
    prov=$(find "$target/premeasure-health/sessions" -name '*.provenance.txt' -print -quit)
    health=$(find "$target/premeasure-health/sessions" -name '*.gpu-health.txt' -print -quit)
    [[ -n "$prov" && -n "$health" ]]
    assert_has "$(cat "$prov")" 'gpu_health_snapshot_start:' t7
    assert_has "$(cat "$prov")" 'rocm_smi_health_command:' t7
    assert_has "$(cat "$health")" 'kfd_process_exists:' t7
    echo 'PASS: provenance and session-end receipt carry GPU health observations'
    PASS=$((PASS + 1))
}

t8() {
    local target="$WORK/full-collect" run_id=full-collect
    local root="$target/premeasure-full-collect"
    local session_one='session,one"quoted' session_two=session-two
    write_session "$root" one "$session_one" "$run_id" "$PLAN_SHA" \
        "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    write_session "$root" two "$session_two" "$run_id" "$PLAN_SHA" \
        "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    local q n code token backend batch process_index session status receipt_exit
    while IFS=, read -r q n code token backend batch process_index; do
        session="$session_one"
        [[ "$process_index" -lt 720 ]] || session="$session_two"
        status=completed
        receipt_exit=0
        if [[ "$process_index" -eq 539 ]]; then
            status=failed
            receipt_exit=130
        fi
        local receipt_shape=normal
        [[ "$process_index" -ne 539 ]] || receipt_shape=signal
        write_receipt "$root" "$run_id" "$session" "$process_index" "$q" "$n" \
            "$code" "$token" "$backend" "$batch" "$status" "$receipt_exit" \
            "$receipt_exit" "$receipt_shape"
        if [[ "$status" == completed ]]; then
            write_scratch "$root/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code/scratch.csv" \
                "$q" "$n" "$backend" "$batch" "$OBSERVED_BINARY_SHA"
        fi
    done < "$SCHEDULE"
    # Deliberately drift both collector-time sources after the fake executions.
    # Collection must not substitute either value or fail manifest verification.
    printf 'collector head drift\n' >> "$REPO/README.md"
    git -C "$REPO" add README.md
    git -C "$REPO" commit --quiet -m 'collector drift'
    printf '# collector harness drift\n' >> "$HARNESS"
    set +e
    local o
    o=$(run_collect "$target" "$run_id" 2>&1)
    local rc=$?
    set -e
    [[ "$rc" -eq 7 ]]
    [[ "$(tail -n +2 "$root/premeasure-ledger.csv" | wc -l)" -eq 1440 ]]
    [[ "$(tail -n +2 "$root/premeasure-candidates.csv" | wc -l)" -eq 1439 ]]
    [[ "$(awk -F'","' 'NR > 1 && $9 == "3" && $10 == "26" { n++ } END { print n + 0 }' "$root/premeasure-candidates.csv")" -eq 23 ]]
    [[ "$(awk -F'","' 'NR > 1 && $2 == "539" { n++ } END { print n + 0 }' "$root/premeasure-candidates.csv")" -eq 0 ]]
    local position_539_state position_539_reasons
    position_539_state="$(csv_field "$root/premeasure-ledger.csv" 539 ledger_state)"
    position_539_reasons="$(csv_field "$root/premeasure-ledger.csv" 539 validity_reasons)"
    [[ "$position_539_state" == censored ]] \
        || { echo "FAIL: position 539 state=$position_539_state reasons=$position_539_reasons"; return 1; }
    [[ "$(csv_field "$root/premeasure-ledger.csv" 539 candidate_emitted)" == false ]]
    [[ -z "$(csv_field "$root/premeasure-ledger.csv" 539 receipt_exit_status)" ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 539 receipt_exit_agreement)" == unavailable ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 539 receipt_failure)" == 'interrupted by runner signal' ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 539 receipt_structurally_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 539 row_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 receipt_session_id)" == "$session_one" ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 observed_git_sha)" == "$OBSERVED_GIT_SHA" ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 observed_harness_source_sha)" == "$OBSERVED_HARNESS_SHA" ]]
    assert_has "$o" 'completeness 3:26:A: expected=12 completed=11 terminal=12 invalid=0 censored=1 candidates=11' t8
    assert_has "$o" 'completeness 3:26:B: expected=12 completed=12 terminal=12 invalid=0 censored=0 candidates=12' t8
    assert_has "$o" 'completeness 3:25:A: expected=12 completed=12 terminal=12 invalid=0 censored=0 candidates=12' t8
    assert_has "$o" 'completeness 3:27:B: expected=12 completed=12 terminal=12 invalid=0 censored=0 candidates=12' t8
    grep -Fq '"session,one""quoted"' "$root/premeasure-ledger.csv"
    grep -Fq '"harness grid, identity ""quoted"""' "$root/premeasure-ledger.csv"
    local ledger_sha candidates_sha
    ledger_sha="$(sha256sum "$root/premeasure-ledger.csv" | awk '{print $1}')"
    candidates_sha="$(sha256sum "$root/premeasure-candidates.csv" | awk '{print $1}')"
    set +e
    run_collect "$target" "$run_id" >/dev/null 2>&1
    rc=$?
    set -e
    [[ "$rc" -eq 7 ]]
    [[ "$(sha256sum "$root/premeasure-ledger.csv" | awk '{print $1}')" == "$ledger_sha" ]]
    [[ "$(sha256sum "$root/premeasure-candidates.csv" | awk '{print $1}')" == "$candidates_sha" ]]
    [[ -z "$(find "$root" -maxdepth 1 -name '.premeasure-*.tmp.*' -print -quit)" ]]
    echo 'PASS: 1440 terminal rows preserve 11+1 versus 12, position 539, quoting, drift isolation, and atomic deterministic reruns'
    PASS=$((PASS + 1))
}

t9() {
    local target="$WORK/legacy-collect" run_id=legacy-collect
    local root="$target/premeasure-legacy-collect"
    local session=only-legacy-session
    write_session "$root" only "$session" "$run_id" "$PLAN_SHA" \
        "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    local q n code token backend batch process_index
    IFS=, read -r q n code token backend batch process_index < "$SCHEDULE"
    write_receipt "$root" "$run_id" - "$process_index" "$q" "$n" "$code" "$token" \
        "$backend" "$batch" completed 0
    write_scratch "$root/processes/process-0000-$q-$n-$code/scratch.csv" \
        "$q" "$n" "$backend" "$batch" "$OBSERVED_BINARY_SHA"
    set +e
    run_collect "$target" "$run_id" >/dev/null 2>&1
    local rc=$?
    set -e
    [[ "$rc" -eq 7 ]]
    [[ "$(tail -n +2 "$root/premeasure-candidates.csv" | wc -l)" -eq 1 ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 session_resolution)" == legacy_unique_fallback ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 row_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 receipt_plan_sha256)" == unavailable ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 receipt_prepared_manifest_sha256)" == unavailable ]]
    echo 'PASS: legacy unique-session fallback remains usable without fabricated receipt plan or manifest hashes'
    PASS=$((PASS + 1))
}

t10() {
    local target="$WORK/edge-collect" run_id=edge-collect
    local root="$target/premeasure-edge-collect"
    local s1=session-one s2=session-two s3=bad-chain s4=bad-plan
    write_session "$root" one "$s1" "$run_id" "$PLAN_SHA" "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    write_session "$root" two "$s2" "$run_id" "$PLAN_SHA" "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    write_session "$root" chain "$s3" "$run_id" "$PLAN_SHA" "$OBSERVED_BINARY_SHA" 5555555555555555555555555555555555555555555555555555555555555555
    write_session "$root" plan "$s4" "$run_id" 6666666666666666666666666666666666666666666666666666666666666666 "$OBSERVED_BINARY_SHA" "$OBSERVED_BINARY_SHA"
    local q n code token backend batch process_index session mode binary file_exit index=0
    local status shape
    while IFS=, read -r q n code token backend batch process_index && [[ "$index" -le 20 ]]; do
        session="$s1"; mode=normal; binary="$OBSERVED_BINARY_SHA"; file_exit=0
        status=completed; shape=normal
        case "$index" in
            1) session="$s2" ;;
            2) session=missing-session ;;
            3) session=- ;;
            4) session="$s3" ;;
            5) binary=7777777777777777777777777777777777777777777777777777777777777777 ;;
            6) mode=missing ;;
            7) mode=duplicate ;;
            8) mode=empty ;;
            9) mode=unavailable ;;
            10) session="$s4" ;;
            11) mode=shape ;;
            12) file_exit=9 ;;
            13) shape=omit_exit ;;
            14) shape=omit_finished ;;
            15) status=failed; file_exit=125; shape=orphan ;;
            16) status=failed; file_exit=42; shape=normal ;;
            17) shape=duplicate_exit ;;
            18) shape=duplicate_finished ;;
            19) status=failed; file_exit=42; shape=omit_exit ;;
            20) status=failed; file_exit=42; shape=omit_finished ;;
        esac
        write_receipt "$root" "$run_id" "$session" "$process_index" "$q" "$n" "$code" \
            "$token" "$backend" "$batch" "$status" "$file_exit" "$file_exit" "$shape"
        if [[ "$status" == completed ]]; then
            write_scratch "$root/processes/process-$(printf '%04d' "$process_index")-$q-$n-$code/scratch.csv" \
                "$q" "$n" "$backend" "$batch" "$binary" "$mode"
        fi
        index=$((index + 1))
    done < "$SCHEDULE"
    set +e
    run_collect "$target" "$run_id" >/dev/null 2>&1
    local rc=$?
    set -e
    [[ "$rc" -eq 7 ]]
    [[ "$(tail -n +2 "$root/premeasure-ledger.csv" | wc -l)" -eq 1440 ]]
    [[ "$(tail -n +2 "$root/premeasure-candidates.csv" | wc -l)" -eq 13 ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 0 session_resolution)" == exact ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 1 session_resolution)" == exact ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 2 session_resolution)" == absent ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 3 session_resolution)" == ambiguous ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 4 session_binary_chain_match)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 5 scratch_binary_matches_session_actual)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 6 scratch_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 7 validity_reasons)" == *duplicate_preamble_git_sha* ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 8 validity_reasons)" == *empty_preamble_git_sha* ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 9 observed_gpu)" == unavailable ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 9 scratch_structurally_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 10 recovery_plan_matches_session)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 11 validity_reasons)" == *header_column_2_mismatch* ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 12 receipt_exit_agreement)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 13 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 13 row_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 13 candidate_emitted)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 14 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 14 row_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 14 candidate_emitted)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 15 receipt_structurally_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 15 row_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 15 ledger_state)" == censored ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 16 receipt_structurally_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 16 row_valid)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 16 ledger_state)" == censored ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 17 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 17 candidate_emitted)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 18 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 18 candidate_emitted)" == true ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 19 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 19 row_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 20 receipt_structurally_valid)" == false ]]
    [[ "$(csv_field "$root/premeasure-ledger.csv" 20 row_valid)" == false ]]
    grep -Fq '"failed, outcome ""preserved"""' "$root/premeasure-ledger.csv"
    for index in 0 1 2 3 4 5 9 10 12 13 14 17 18; do
        [[ "$(awk -F'","' -v wanted="$index" 'NR > 1 && $2 == wanted { n++ } END { print n + 0 }' "$root/premeasure-candidates.csv")" -eq 1 ]]
    done
    echo 'PASS: session, binary, preamble, plan, shape, unavailable, and exit reconciliation facts do not suppress valid scratch candidates'
    PASS=$((PASS + 1))
}

trap 'rm -rf "$WORK"' EXIT
t1
t2
t3
t4
t5
t6
t7
t8
t9
t10
echo "PASS: $PASS/10 premeasure tests"
