# Handoff -- Close gf2-core SOTA performance gaps (`97bf0879`) -- session 13 entry

**Date:** 2026-05-07
**From:** session 12 (continued from handoff-10 entry)
**To:** session 13
**Branch:** main @ `52fb4a0` (docs: d1dd266c performance gap analysis)
**Lead:** project-lead

---

## TL;DR

Wave 10 partially closed. Closed 4a59d1f9 (Keller-Gehrig reassessment, R3
PASS), 3a37e0f6 (sparse layout/traversal — Path A + Path B SpMM kernel landed,
all 8 GF(p) sparse cells PASS), 1726270d (sparse parity evidence, R3 PASS).

Open and held:
- **d1dd266c** (Tune minimal polynomial path): Wiedemann O(n^3) implementation
  landed at `bda98cd` for large-cardinality fields. 4/8 reference cells PASS
  the 1.5x contract; 4 cells still fail. **User directive (2026-05-07): no
  scope-narrowing, no aspirational amendments — close the gaps with specialist
  work.** Performance gap analysis written to
  `dev/plans/d1dd266c-minpoly-performance-gaps.md` and attached to the issue.
  The user will dispatch a specialist to plan the cleanup. Hold d1dd266c open.
- **54fd3f0b** (sparse story closure): R1 code-review FAILED at run
  `0d3aaecd-...` (verdict not yet inspected — see Wave-11-A below). Needs
  re-review fix in next session.

In-flight at session close:
- 54fd3f0b code-review R1 verdict needs inspection in next session (the
  result.json is recorded; lead did not have time to read findings before
  user requested the handoff).

---

## What landed in session 12 (commits on main, in order)

```
52fb4a0 docs(jit:d1dd266c): performance gap analysis for unclosed minpoly cells
bda98cd perf(jit:d1dd266c): implement Wiedemann O(n³) minpoly for large fields
7e274fd chore: complete issue 1726270d (Publish sparse parity evidence)
c59f0ca docs(jit:1726270d): R2 fix — correct GF(2^31-1) ratios + 43-row baseline count
dc564db docs(jit:1726270d): R1 fix — replace remaining transient CSV paths with permanent ones
9f3cca4 docs(jit:1726270d): pin sparse-evidence CSVs to permanent dev/bench_results/ paths
b6a4bf2 docs(jit:1726270d): publish sparse parity evidence for story 54fd3f0b
36cde8e chore: complete issue 3a37e0f6 (Optimize sparse layout and traversal)
3783c95 chore(jit:97bf0879): record 1726270d ready promotion (3a37e0f6 close unblocked)
fb4e8f2 perf(jit:3a37e0f6): packed-int SpMM kernel for Fp<P> Montgomery primes
ce40993 chore: complete issue 4a59d1f9 (Reassess Keller-Gehrig crossover)
4e1d178 docs(jit:4a59d1f9): R2 fix — n=512 ratio + bench-helper note in evidence doc
32daafb chore(jit:4a59d1f9): record bohhcyfog R1 gate runs (cargo-ci pass, code-review fail)
baa6d9b fix(jit:4a59d1f9): factor dispatch-arms helper + refresh stale 173x notes
222e464 research(jit:4a59d1f9): reassess Keller-Gehrig vs cubic crossover post-Wave-9
```

3 issues closed (4a59d1f9, 3a37e0f6, 1726270d). 1 issue partial-impl + open
(d1dd266c). 1 issue R1 fail in flight (54fd3f0b).

---

## Wave 11 — entry state for next session

| Story | State | Notes |
|---|---|---|
| 974a85bd (GF(2) M4RI) | done (closed in handoff-10 epoch) | |
| cc5de315 (GF(p) fflas) | done | |
| 2c7548ae (GF(2^m) m4rie) | done | |
| 72ab6d0e (dense LA) | ready | All deps done. Can close once 54fd3f0b R1 cleared. |
| 66190ccd (charpoly minpoly) | backlog | Blocked on 8ccc1751 (which is blocked on d1dd266c + b87362a3). |
| **54fd3f0b (sparse FieldMatrix)** | **in_progress R1 FAIL** | code-review verdict at run `1ec45ace-...` — see "Wave-11-A R1" below. |

Wave 10 still has b87362a3 (Implement winning charpoly dispatch) ready and
8ccc1751 (Publish polynomial parity evidence) blocked on d1dd266c +
b87362a3. b87362a3 is mostly already done in spirit (4a59d1f9 confirmed cubic
always wins; KG_DISPATCH_MIN_N = usize::MAX is correct) — just needs
measurement evidence linking cubic charpoly to fflas-ffpack reference at the
target sizes + criterion verdict.

---

## Wave-11-A R1 (54fd3f0b — sparse story closure) — verdict pending inspection

54fd3f0b code-review R1 ran at gate-run `0d3aaecd-...` (latest in
`.jit/gate-runs/` for this issue) — verdict FAILED but lead did not read
findings before user requested handoff. Action for next session:

1. `for d in $(ls -t .jit/gate-runs/); do if grep -q '"issue_id":"54fd3f0b' .jit/gate-runs/$d/result.json && grep -q '"gate_key":"code-review"' .jit/gate-runs/$d/result.json; then echo $d; cat .jit/gate-runs/$d/result.json | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['stdout'][-3000:])"; break; fi; done`
2. Read findings, fix lead-direct (the story description / linked-doc set is
   the artifact under review — likely a doc-link or attribution issue, since
   the code is unchanged on the story-closure path).
3. Re-run cargo-ci + code-review.
4. doc-review attestation + state=done.

---

## d1dd266c — open, awaiting specialist

The Wiedemann implementation is sound (proptest annihilation passes; 1922
nextest tests pass). The 4 unclosed cells need:

1. **§ 2.3 SIMD matvec for medium primes** (closes GF(65521)/256 from 2.26x).
   Mirrors the existing `fp_medium_spmm_row` kernel in
   `crates/gf2-kernels-simd/src/x86/fp_medium.rs`. Probably 2 days.
2. **§ 2.2 small-prime arithmetic + SIMD matvec** (closes GF(251)/64 from
   4.90x). 3-5 days.
3. **§ 2.1 block Wiedemann for q ≤ n** (closes GF(7)/64+256, GF(251)/256).
   Coppersmith block Wiedemann or Dumas-Saunders-Villard. 1-2 weeks.

All detail in `dev/plans/d1dd266c-minpoly-performance-gaps.md` (attached to
the issue at session close).

The user said: "Create a dev doc linked to the issue that details what we
need and where our performance is lacking. I'll then dispatch a specialist
to plan this properly."

**Do NOT** dispatch a worker on d1dd266c in session 13 — wait for the user
to dispatch the specialist.

---

## Traps — do not repeat these

(All traps from handoff-10 carry forward unless explicitly resolved here.)

1. **Don't run multiple cargo bench / cargo nextest in parallel** — they
   compete for the build cache and lock. Multiple cargo-ci gate-passes
   serially are safe; concurrent worker bench + main cargo-ci is dangerous.
   Resolved by killing the orphan `charpoly --test --bench` from session
   start (PID 1611647) at session 12 close.

2. **Do not let bench dispatches run unbounded.** d1dd266c worker burned ~55
   min on a single `charpoly/minpoly --measurement-time 5` invocation with
   `sample_size=10` because the per-iteration cost at large n exceeded the
   measurement budget by 10x+. When dispatching impl workers that include
   benches, **explicitly bound** measurement-time and per-cell wall budget in
   the prompt. Suggest: `--measurement-time 2`, `sample_size(10)`, n ≤ 512
   for cells where the per-call cost can exceed a few seconds, and analytic
   extrapolation for the largest sizes.

3. **Do not introduce new exclusion classes without user approval.** d1dd266c
   worker introduced `algorithm-limitation` as a new protocol § 9 class
   without escalating; lead caught it before dispatching code-review. Per
   the user's directive, treat the failing cells as open work and document
   the gap in a planning doc instead of inventing a class.

4. **`[hard]→[aspirational]` amendments are not lead-dispatch authority.**
   The d1dd266c worker tagged GF(65521)/256 (2.26x) as `[aspirational]`
   without escalation. Same as #3 — must escalate to user, even when the
   precedent (Wave-6/8/9) is well-established.

5. **Worker evidence docs frequently cite ephemeral worktree paths.** Three
   of the 1726270d review cycles were caused by `.claude/worktrees/...`
   references in the evidence doc that wouldn't survive worktree cleanup.
   In future evidence-doc dispatch prompts, **explicitly require** that
   every CSV / artifact reference resolve to a path under `dev/...` (or
   another permanent location) at HEAD of the worker's final commit. Tell
   the worker to copy raw bench CSVs from runtime `bench_results/` to
   `dev/bench_results/` with date-prefixed names BEFORE writing the
   evidence doc.

6. **Worker evidence-doc arithmetic.** 1726270d R2 failed because the
   GF(2^31-1) ratios in the evidence doc (1.30x, 1.57x) didn't match what
   the cited CSV values (1.07 Gops/s vs 661 Mops/s; 1.199 Gops/s vs 702
   Mops/s) actually compute to (1.62x, 1.71x). Always verify
   doc-table-arithmetic vs CSV ground truth in Tier 2 before dispatching
   code-review.

---

## Lead state on close

- All d1dd266c worktree changes are landed on main at `bda98cd` + the
  performance gap doc at `52fb4a0`.
- 54fd3f0b R1 review FAIL recorded in JIT meta but JIT changes uncommitted —
  the .jit/issues/54fd3f0b-...json + .jit/events.jsonl are dirty in the
  working tree at session close.
- d1dd266c worktree at `.claude/worktrees/agent-d1dd266c` should be retained
  (worker's branch `worktree-agent-d1dd266c` has the same content as main
  HEAD, but the worktree may be useful for the specialist's continuation).
- d1dd266c minpoly tuning evidence has been attached to the issue:
  `jit doc add d1dd266c dev/plans/d1dd266c-minpoly-performance-gaps.md`
  + `jit doc add d1dd266c dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md`.
- 1726270d: 8 docs/CSVs attached to story 54fd3f0b via `jit doc add` (sparse
  evidence + reference CSVs + 3a37e0f6 layout doc + GPU handoff plan + 3
  raw bench CSVs).

## Next session first action

1. Read this handoff fully.
2. Inspect the 54fd3f0b R1 code-review verdict (gate-run `1ec45ace-...` or
   most recent). Fix lead-direct if doc-only; otherwise dispatch rework.
3. Close 54fd3f0b once R-fix passes.
4. Wait for the user to dispatch a minpoly specialist for d1dd266c.
5. Continue Wave 11 closure for 72ab6d0e (dense LA story) once 54fd3f0b is
   done and the user signals the d1dd266c specialist is in-flight.
6. Wave 10 b87362a3 (Implement winning charpoly dispatch) is still ready;
   after d1dd266c is closed by the specialist, dispatch b87362a3 → 8ccc1751
   → 66190ccd story closure.
7. Wave 12 (final aggregation + epic close) only after all stories close.
