# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 5

**Date:** 2026-05-05 (session 5, after 47698404 R4 escalation)
**Session number:** 5
**Prior handoffs:**
- `dev/active/97bf0879-handoff.md` (session 1, 2026-04-30) — Wave 1 closure.
- `dev/active/97bf0879-handoff-2.md` (session 2, 2026-05-04 morning) — Wave 2 closure.
- `dev/active/97bf0879-handoff-3.md` (session 3 + 3b extension, 2026-05-04) — Wave 3 closure (7/9), b13799ac/9a715d75 held open per user.
- `dev/active/97bf0879-handoff-4.md` (session 4, 2026-05-04 evening) — d0ca9482 closed; 47698404 worker truncated by rate limit, WIP preserved at 7251db4.
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`.

All prior-session traps remain in force unless explicitly resolved here.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Children summary: **10 done**, 1 in_progress (held-open: `47698404`), 2 in_progress (held-open downstream: `b13799ac`, `9a715d75`), 4 newly-filed sparse-impl follow-ups (ready: `0d6ca3b6`, `521390db`, `0f708b36`, `96fde7c7`), 43 backlog/ready, 0 rejected.
- Wave 1: closed (5 issues).
- Wave 2: closed (4 issues).
- Wave 3 + impl follow-ups: 7 of 9 done; 2 held open (`9a715d75`, `b13799ac`); + 47698404 newly held open with 4 fresh deps.
- **47698404 cherry-picked + cargo-ci PASS.** Worker output landed on main as commits `c21b700`, `005ceaa`, `c50e0bc`, `ce57a8b`, plus lead fmt fix `ed960bb` and three R1-R3 doc fixes (`eeaaa1d`, `c95e283`, `1b916cc`). cargo-ci passed every cycle.
- **47698404 code-review FAILED 4 rounds.** Same strict-reviewer ratchet pattern as b13799ac/a3412e15 in sessions 3-3b. User escalation 2026-05-05: chose path 1 — file follow-up impl tasks, hold 47698404 open.
- Branch state: `main` clean at `1b916cc` (`fix(jit:47698404): code-review R3 — clarify LinBox cross-check status`) plus the new JIT state edits about to be committed in the session-5 handoff commit.

## What just happened (session 5)

### 47698404 continuation worker dispatch + 4 R-cycles

1. **Worker dispatched** (agent `a21dda8b6d1792b62`) into existing `worktree-agent-47698404` at lead-preserve commit `7251db4`. Worker scope: regen bench output, write §0-§7 scorecard, return.

2. **Worker found and fixed a critical lead-preserve bug.** The session-4 lead-preserve commit `7251db4` added the `fmt_density_c()` shim function but **left all 12 format-string call sites with empty `{}` placeholders** — `cargo check` produced 12 errors. The continuation worker wired `fmt_density_c(density)` into every call site (lines 310, 364, 389, 414, 446, 473, 505, 556, 584, 631, 680, 706 in `bench_sparse_csv_emitter.rs`). This made the regen possible.

3. **Worker delivered:** regen of all bench CSVs/tables, scorecard at `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` (282 lines, §0-§7 structure). Final commit `d335231` on the worker branch.

4. **Lead cherry-picked** 4 worker commits onto main (rebase-equivalent — main was 1 chore-commit ahead of worker base). Applied `cargo fmt --all` fix at `ed960bb` (pre-existing drift from `2bc4a70`'s original landing).

5. **Lead attached 10 docs** to issue 47698404 via `jit doc add` (scorecard, tables, CSVs, host capture, harnesses).

6. **Code-review R1** failed: scorecard claimed `sparse×dense × GF(p)` was 5/5 PASS but `sparse_smoke.cpp` only had `oracle_spmv`, no `oracle_sparse_dense`. **Fix**: added `oracle_sparse_dense` template + main block driving fflas-ffpack `fspmm` against `scalar_sparse_dense` for the 4 GF(p) primes. Verified locally — all 9 oracles (5 spmv + 4 sparse_dense) pass. Commit `eeaaa1d`.

7. **Code-review R2** failed: scorecard silently dropped `sparse×dense × GF(2)` despite design doc naming LinBox `applyLeft` as canonical. **Fix**: added explicit `sparse×dense × GF(2)` § 3 subsection (PENDING / not-yet-harnessed both sides), § 5 #6 entry, § 6 verdict row with new `EXCL_both` marker. Commit `c95e283`.

8. **Code-review R3** failed: scorecard claimed LinBox cross-checks (n=16 smoke for spmv × GF(2); applyLeft for sparse×dense × GF(p)) that don't exist in any harness. **Fix**: § 3 row distinguishes bench-side LinBox at n=1024 (real measurement, ref.csv:22) from hypothetical n=16 LinBox smoke (not wired); § 6 verdicts gain footnotes ¹ and ² citing § 5 #7; new § 5 #7 documents both LinBox cross-check gaps. Commit `1b916cc`.

9. **Code-review R4** failed structurally: protocol § 6 (`sota_reference_acceptance_protocol.md:243-249`) requires `--smoke` to compare candidate-vs-gf2-core, but `sparse_smoke.cpp` compares fflas-vs-Givaro-scalar — gf2-core is not in the smoke loop at all. ALSO: design doc `sparse_benchmark_corpus.md:221` final tally claims "0 protocol-class exclusion cells" but the scorecard documents 7+ cells as `not-yet-harnessed`. **Not closeable by doc edits** — requires real C++/Rust impl work.

### User escalation 2026-05-05

Presented the R4 findings to user via `AskUserQuestion`. User chose **path 1**: file follow-up impl tasks, hold 47698404 open. Same pattern as session 3-3b on b13799ac and a3412e15.

### Filed 4 new sparse-impl follow-ups

| Short ID | Title | Priority | Owner story |
|---|---|---|---|
| `0d6ca3b6` | Wire sparse-elim emitter for GF(2) and GF(p) | normal | sota-sparse-fieldmatrix |
| `521390db` | gf2-core SpBitMatrix sparse-dense matmat for GF(2) | normal | sota-sparse-fieldmatrix |
| `0f708b36` | Extend LinBox sparse harness with sparse_dense rows + spmv smoke | normal | sota-sparse-fieldmatrix |
| `96fde7c7` | sparse_smoke gf2-core integration design + impl | high | sota-sparse-fieldmatrix |

All 4 wired as `47698404` dependencies via `jit dep add`. State: ready (no blockers from outside the new tasks).

`96fde7c7` is **verification work** and per CLAUDE.md § *Verification work* requires a pre-approved design sketch before impl is dispatched. The sketch task (`dev/plans/sparse_smoke_gf2core_integration_sketch.md`) is the first deliverable; impl follows after lead/user approval.

## Wave 4-impl readiness

The 4 new follow-ups can run in parallel — minimal file overlap:
- `0d6ca3b6` touches `bench_sparse_csv_emitter.rs` (sparse-elim wiring) + `sparse.rs` (if SpBitMatrixDual::rref needs adding) + `sparse_smoke.cpp` (smoke addition for sparse-elim).
- `521390db` touches `sparse.rs` (SpBitMatrix::matmat) + `bench_sparse_csv_emitter.rs` (a different code path).
- `0f708b36` touches `linbox_sparse_bench.cpp` and `sparse_smoke.cpp`.
- `96fde7c7` is design-first; only after sketch approval does it touch sparse_smoke.cpp / Cargo.toml / Containerfile.

**Conflict heuristic:** `0d6ca3b6` and `521390db` both touch `bench_sparse_csv_emitter.rs` — serialize them OR coordinate via worktrees. `0d6ca3b6`, `0f708b36`, and `96fde7c7` all touch `sparse_smoke.cpp` — serialize them.

Recommendation: dispatch `521390db` and `0f708b36` first wave (different files); then `0d6ca3b6`; then `96fde7c7` after the others land (sparse_smoke.cpp will have been extended by the others by then).

After all 4 close: `47698404` re-review. The lead must update the scorecard to fold:
- LinBox sparse_dense × GF(p) numbers from `0f708b36` into § 3 sparse×dense × GF(p) tables.
- LinBox spmv × GF(2) smoke into § 6 verdict (criterion #3 footnote ¹ removed).
- gf2-core integration into § 2 cross-equality oracle (replaces scalar reference, removes the protocol § 6 violation).
- Sparse-elim numbers from `0d6ca3b6` into § 3 sparse-elim tables (PENDING → real numbers).
- New `sparse×dense × GF(2)` numbers from `521390db` into § 3 (PENDING → real numbers).

## After 47698404 closes

Per session-4 handoff (still in force):
- **`b13799ac` re-review.** The strict reviewer's three persistent findings:
  1. Direct vs transitive smoke — `47698404`'s `oracle_spmv` (via scalar reference) IS a transitive pattern; project convention now established. Should be defensible.
  2. CSV path convention (`dev/bench_results/` vs protocol's `benchmarks/results/`) — convention precedent established by 47698404.
  3. SSOT — Conway poly hard-coded in 4 places + scalar `ref_gf2pow32_mul` duplicated C++/Rust. **Still pending.** Lead-direct extraction (single C++ header `gf2pow32_constants.h` + Rust test consumes `PrimitivePolynomialDatabase::standard(32)`) is the cleanest fix — see session-4 handoff for the recon. OR fresh user escalation if SSOT remains a strict-reviewer FAIL.
- **`9a715d75` re-review** after `b13799ac` closes — provides GF(2^32) reference harness, unblocks the GF(2^m) reference-lane selection.
- **Wave 4 dispatch** — `4c0d0202` (SOTA target matrix design doc).

## Traps — do not repeat these

Carry-forward (still in force):
- All traps from `babcf05e-handoff-5.md`, `97bf0879-handoff{,-2,-3,-4}.md`. Re-read on session resume.

New traps from session 5:

1. **Lead-preserve commits MUST compile.** Session 4's lead-preserve at `7251db4` added `fmt_density_c()` but left empty `{}` format-arg sites — example failed `cargo check`. The continuation worker had to fix all 12 sites before any regen could run. Lesson: when preserving a worker WIP that adds a new helper function, verify the preserve compiles (`cargo check -p <crate>`) BEFORE committing the lead-preserve. If the worker's call-site edits are uncommitted-and-broken, EITHER finish wiring them on the lead's behalf, OR revert the helper and only preserve clean state. Mid-state preservation costs the next worker's first hour.

2. **Strict-reviewer ratchet is structurally diminishing returns past R3.** R1 found the missing sparse_dense oracle (real fix). R2 found a missing cell (real fix). R3 found a wording overclaim (real fix). R4 found a structural protocol mismatch (NOT fixable by doc edits — requires real impl). The pattern: R1-R3 are productive doc clarifications; R4+ surface real engineering work that needs follow-up tasks. **Lesson**: when R3 lands and R4 looks like another structural finding, escalate to user immediately rather than attempting another doc-fix cycle. Each cycle ate ~5-10 minutes of gate runtime + context window.

3. **`cargo fmt` drift in worker commits is invisible until cargo-ci on main.** Worker `2bc4a70` introduced fmt drift (long-arg call sites that fmt would auto-wrap). The drift didn't surface in the worker's own dev cycle because the worker presumably ran `cargo check`+`cargo test` but not `cargo fmt --all -- --check`. The lead caught it on the first cargo-ci on main. **Lesson**: worker dispatch prompts should call out `cargo fmt --all` as part of the worker's pre-handback checklist, OR the lead should always be ready to apply fmt as a separate commit between cherry-pick and gate runs.

4. **Cherry-pick is the right pattern when the worker branch has no `.jit/` conflicts but its base is older than main.** Session 5 used cherry-pick (4 commits clean) rather than rebase or merge. Reason: the 4 worker commits had no overlap with main's `db40230` chore commit (the only commit between worker base and main HEAD). Cherry-pick avoided creating a merge commit and kept the history linear. **Lesson**: for clean worker handbacks, cherry-pick > merge --no-ff > rebase.

5. **Protocol § 6 strict reading vs "good enough" pragmatism.** Protocol § 6 line 245 says smoke must run "both the candidate and the gf2-core implementation". The 47698404 worker (and the prior a3412e15 harness work) used in-harness scalar references because gf2-core-from-C++ requires FFI infrastructure that doesn't exist yet. The strict reviewer caught this. **Lesson**: any future smoke-oracle work over a candidate where gf2-core has a same-operation path MUST plan FFI integration up front, OR the protocol § 6 requirement amendment must be in flight before the smoke harness lands.

## Reference artefacts (this session)

- This handoff: `dev/active/97bf0879-handoff-5.md`
- Progress file: `dev/active/97bf0879-progress.json` (lead updates after this handoff lands)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2,-3,-4}.md`
- 47698404 worker output (now on main): commits `c21b700`, `005ceaa`, `c50e0bc`, `ce57a8b`, `ed960bb`, `eeaaa1d`, `c95e283`, `1b916cc`
- 47698404 scorecard: `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md`
- 47698404 sparse harnesses: `benchmarks/reference/{fflas_sparse_bench,linbox_sparse_bench,sparse_smoke}.cpp`, `crates/gf2-coding/examples/bench_sparse_csv_emitter.rs`
- New follow-ups (ready, deps of 47698404): `0d6ca3b6`, `521390db`, `0f708b36`, `96fde7c7`
- Worker scope spec: `dev/plans/sparse_benchmark_corpus.md` (a3412e15)
- Protocol: `dev/plans/sota_reference_acceptance_protocol.md`

## Open questions for the next session

None blocking. The 4 new sparse-impl follow-ups are ready to dispatch. `96fde7c7` requires a pre-approved design sketch first (verification work).
