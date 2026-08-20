# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 4

**Date:** 2026-08-20T08:21:57Z
**Session number:** 4
**Prior handoffs:** None. Sessions 1–3 ended without one; their state survives only in `progress.json` and the git log.

## Current state

- Epic: `6dc81018` — state: `backlog` (never claimed by the lead; claim it on resume)
- Wave in progress: wave 10 of 10 — `50b47eae`'s re-measurement, not yet dispatched
- Children summary: 11 done, 0 in_progress, 1 ready (`50b47eae`), 0 rejected
- Active claims: `50b47eae` claimed as `agent:worker` since 2026-08-20 ~05:30Z — release or reuse on resume
- Open escalations: none awaiting input. DEC-A…DEC-D and the c42720ce counter reset are all answered and recorded in `progress.json`.
- Progress file: `dev/active/6dc81018-field-capability-dispatch/progress.json`

**The epic cannot close yet.** REQ-04 is unmet: the pinned non-regression comparison currently records `RESULT: FAIL`.

## What just happened

- Resumed mid-flight: session 3 was cut immediately after `278acf3a`'s doc-review failed, with the gate-fail record uncommitted. Recovered, committed at `9bda623e`.
- `278acf3a` — closed after 1 rework. The baseline run itself was correct first time; the rework recorded the across-build falsification in plan §4 and named a tracked owner for the deferred consequence.
- Escalated the falsified noise model → **DEC-C** (owner, option B): plan v1 stays frozen, `50b47eae` additionally measures a control build whose ratios enter no verdict.
- Filed `51058f8e` for that predeclaration; closed with 0 rework. It declared **both** control-build hash outcomes in advance, which is what made the later excursion adjudicable.
- Escalated `5ecc9bf8`'s stale harness path → owner approved amending the Background to `tuning_calibration.rs`.
- `5ecc9bf8` — closed after one lead-directed correction. Calibration moved three fields: `simd_min_words` 8→4, `div_rem_fast_min_len` 2048→1024, `subproduct_min_len` 4096→512. Six falsification records, incl. two in-tree tuning notes contradicted.
- **DEC-B6**: `karatsuba_min_degree` is not sweepable (both arms private; `poly.rs:72` documents the omission deliberately). The emitted profile OMITS the field per design §5 condition 5 rather than claiming an unmeasured value. Tracked forward as `389aa4de`.
- `0d819b62` — closed after 1 rework. R1 edited `f35daec0`'s committed test and reworded a measured claim without recording the contradicting receipt.
- `697fc55b` — closed after 1 rework. R1's tests compared mathematically equivalent algorithms, so they passed even if the dispatchers ignored the profile. Fixed with a route-observation API all five dispatchers consume.
- **DEC-B7**: `@/inv/present-tense-prose` and `@/inv/falsification-preserved` bind the same rustdoc; satisfied together by stating the measurement as a present fact, not by trading one off.
- `50b47eae` first session — the comparison **FAILS**: `popcount/words=1` ratio 1.182311, `popcount/words=8` ratio 1.064214, both > τ_cell = 5 %. τ_set holds at 0.998853. Control arm reads 0.972476 / 1.002702, so the predeclared rule attributes the cost to the cutover.
- Escalated → **DEC-D** (owner): remove the cost; do not widen the tolerance, do not amend REQ-04. Filed `c42720ce`.
- `c42720ce` — closed after 3 rework rounds and one owner-granted counter reset. Final mechanism: threshold cached in an `AtomicUsize` beside an `AtomicBool` flag, published Relaxed-then-Release, read Acquire-then-Relaxed.

## What to do next

- [ ] Claim the epic: `jit issue claim 6dc81018 agent:jit-execution-lead`.
- [ ] Dispatch `50b47eae`'s re-measurement. Full spec was written at `/tmp/.../scratchpad/dispatch-50b47eae-rerun.md`; if that scratchpad is gone, rebuild it from these invariants:
      - Run the plan procedure **unmodified**, comparing this session's post-fix arm against `2026-08-19-pre-cutover-baseline.md` — **not** against the previous post-cutover arm.
      - Include the control arm again, same lock session, control worktree already prepared at `.agents/worktrees/control-0c072d73` (detached at `0c072d73`).
      - Install no profile. Counterbalance arm ordering. Verify 1-minute load is idle first.
      - New receipt at `dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt-2.md`; report the two previously failing cells with old ratio beside new.
      - Benchmark-mode work goes to a **Claude opus agent in the main checkout**, never codex.
- [ ] Review, gate `doc-review`, close `50b47eae` **only if the tolerance holds**. If it fails again, preserve it and escalate — do not re-run until it agrees.
- [ ] Then Section 10: reconcile `surfaced_pitfalls` against the epic's criteria, run `jit gate evaluate-all 6dc81018` (repo-validate, doc-review, holistic-review), write the completion report, close.
- [ ] Reclaim stale worktrees: `agent-265997f9`, `agent-34d85cb9`, `agent-0d819b62`, `agent-697fc55b`, `agent-c42720ce`, and `control-0c072d73` once the re-run is done.

## Traps — do not repeat these

- **Do NOT treat `ListAgents` emptiness or a quiet working tree as a dead worker.** `ListAgents` returned "No reachable agents" while two dispatched agents were provably alive and mid-task, and a clean `git status` 80 seconds before an agent's first write looked identical to death. Acting on both signals, this session re-dispatched duplicates over live workers; only the duplicate's own refusal to write on a mismatched premise prevented two agents racing on the same two paths. A dispatched agent's first write can lag its spawn by 10+ minutes. Probe artifacts and `.jit/events.jsonl` over a window instead. (The initial theory that `ScheduleWakeup` tore the agents down was **wrong** and is retracted.)
- **Do NOT run a background CI command relying on a `cd` from an earlier tool call.** A backgrounded `./scripts/cargo-ci.sh` executed in `/home/vkaskivuo/Projects/gf2` on `main` instead of the worktree under review and reported all four stages green. That pass never touched the branch. Always `cd <worktree> && ./scripts/cargo-ci.sh` in one command, and echo `pwd` and `git rev-parse --abbrev-ref HEAD` first.
- **Do NOT reserve `usize::MAX` as a sentinel for any `_min_` threshold.** `dev/active/220cab0b/design.md` §2.1: "Both endpoints of the `usize` range are admissible and meaningful: for a `_min_` field, $t = \texttt{usize::MAX}$" — it is a valid `simd_min_words` meaning "never use SIMD". Rejecting it in `try_new` to free a sentinel turned a loadable v1 profile into a loader error and cost a full rework round. This was the **lead's** instruction, not a worker error. Fix publication races with `Release`/`Acquire` ordering, which reserves nothing.
- **Do NOT publish shared state through two relaxed atomics.** `c42720ce` R2 stored value-then-flag with both `Relaxed`, so a reader could observe the flag set with the stale value and select the conservative route after an install. `cargo-ci` was green throughout; only `code-review` caught it.
- **Do NOT let a worker "prove" a dispatcher change with equivalence assertions.** `697fc55b` R1 asserted `mul_fast(a,b) == a.mul_ntt(b)` — two mathematically equivalent algorithms — which passes even if the dispatcher ignores the profile entirely. Require route observation through production code the dispatcher itself calls, never a test-local copy of the comparison.
- **Do NOT let a worker edit another issue's committed tests.** Both codex workers independently reached for `f35daec0`'s `tuning_profile_install.rs` / `tuning_profile_default.rs` instead of adding their own binary; `0d819b62` also narrowed one's `required-features`. Name this prohibition explicitly in every dispatch. Note the distinction that was applied: *adding* assertions to a default-profile binary is acceptable; *changing* existing assertions or feature gates is not.
- **Do NOT dispatch benchmark-mode work to codex.** Its sandbox blocks sccache (EPERM) and crates.io DNS, and the bench wrapper's privileged `nice`/affinity. Every codex dispatch this session produced work it could not build; the lead ran CI each time. Workaround the worker found: `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh`.
- **Do NOT trust a single red CI run on `gf2-sim gpu::awgn`.** `test_gpu_awgn_matches_cpu_within_1_ulp` (SIGABRT) and `test_gpu_box_muller_within_1_ulp_over_1024_frames` (TIMEOUT) failed once and passed on an immediate re-run of the **identical tree** with the GPU at 0 % use. Re-run once before treating it as a regression. This flake is unfiled — see open questions.
- **Do NOT edit closed-issue records under `dev/active/` non-additively.** Their citations are anchored (`220cab0b/design.md` at `05bc9c14`, `classification.md` at `2f2cbb37`). A prior session's codex rewrote three such documents' citations to the post-change tree, making them self-contradictory, and the lead reverted all three. Amendments must be appended and must change no pre-existing line.
- **Do NOT invoke `codex exec` without `< /dev/null`.** It hangs forever on "Reading additional input from stdin...", producing no work and no error, indistinguishable from a slow worker. Write the prompt file in a separate shell call from the dispatch.
- **Do NOT expect codex to commit.** Its sandbox cannot write `.git/index.lock`. It leaves work uncommitted; the lead commits on the worker branch, then merges. It also violated the no-`.jit`-writes rule once and recorded a gate run — revert those files before committing.

## Open questions needing invoker input

- Question: should the flaky `gf2-sim gpu::awgn` tests be filed as a repository issue?
  - Context: two GPU tests abort/time out intermittently and pass on re-run against an identical tree with the GPU idle; this cost one false-red CI cycle this session.
  - Options: (A) file outside the epic under `86b9c719` as prior follow-ons were; (B) leave unfiled.
  - Recommendation: (A). It is a real defect and `@/inv/no-deferred-defects` points that way; it was left unfiled only because it is outside this epic's scope and the session was closing.

## Reference artefacts

- Epic: `jit issue show 6dc81018`
- Open child: `jit issue show 50b47eae`
- Design: `dev/active/220cab0b/design.md` (anchored at `05bc9c14`; §2.1 ranges, §2.5 defaults, §2.7 calibration, §4.1 amended by `c42720ce`, §5 extensibility)
- Classification: `dev/active/6dc81018-field-capability-dispatch/classification.md` (anchored at `2f2cbb37`)
- Frozen procedure: `dev/benchmarks/tuning_profiles/selector-non-regression-plan-v1.md`
- Predeclaration: `dev/benchmarks/tuning_profiles/across-build-control-arm-v1.md`
- Baseline: `dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.md` (+ `.csv`)
- Failing post-cutover receipt (preserved, do not modify): `dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-receipt.md` (+ 2 `.csv`)
- Calibration receipt: `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`
- Proof sketch: linked to `1ac74567`
- Out-of-epic follow-ons: `a6636671`, `4dd5372a`, `99c92597`, `389aa4de` (all under `86b9c719`)
