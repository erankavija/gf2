# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 4

**Date:** 2026-05-04 (session 4 evening, after rate-limit truncation)
**Session number:** 4
**Prior handoffs:**
- `dev/active/97bf0879-handoff.md` (session 1, 2026-04-30) — Wave 1 closure.
- `dev/active/97bf0879-handoff-2.md` (session 2, 2026-05-04 morning) — Wave 2 closure.
- `dev/active/97bf0879-handoff-3.md` (session 3 + 3b extension, 2026-05-04) — Wave 3 closure (7/9), b13799ac/9a715d75 held open per user.
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md` — read at minimum the **Traps** sections of `babcf05e-handoff-5.md`.

All prior-session traps remain in force unless explicitly resolved here.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Children summary: **10 done**, 2 in_progress (worker-side: `47698404`, held-open: `9a715d75`, `b13799ac`), 44 backlog/ready, 0 rejected (out of 56 total).
- Wave 1: closed (5 issues).
- Wave 2: closed (4 issues).
- Wave 3 + impl follow-ups: 7 of 9 done; 2 held open (`9a715d75`, `b13799ac`).
- **`d0ca9482` closed this session (lead-direct, Wave-1 leftover, baseline-scorecard child).** Commit `af6b7bc`.
- **`47698404` worker WIP preserved this session.** Worker hit Anthropic rate limit (resets 11:30pm Helsinki = 2026-05-04T20:30Z); two clean commits on `worktree-agent-47698404` plus a `wip(jit:47698404)` lead-preserve commit at `7251db4`. WIP is substantive — the next session can resume.
- Branch state: `main` clean at `fa5f1f6` (`chore(jit:97bf0879): record 47698404 worker claim (lead-preserve session)`).
- Open escalations: none new this session. Two carry-forward (`9a715d75`, `b13799ac`) chained on `47698404` closure.

## What just happened (session 4)

### `d0ca9482` lead-direct closure (3 minutes)
- Both `[hard]` criteria already satisfied by repository state: `a9ab0a4f` is `done` with all 3 gates passed; no scope creep on the closure path (only doc/metadata cleanup commits).
- Ran `cargo-ci` → PASS (warm cache, ~6s).
- Ran `code-review` (claude --model sonnet -p) → PASS (verbose review at `14a16a7`; reviewer cross-referenced dependency states + scope-creep checks).
- Ran `doc-review` → manual lead attestation.
- `jit issue update --state done` → committed at `af6b7bc`.

### `47698404` dispatch + worker truncation
- Dispatched in worktree `worktree-agent-47698404` anchored at main HEAD `af6b7bc` per `references/worktree-dispatch-protocol.md` (project-agnostic skill script `scripts/dispatch-worker-worktree.sh 47698404`).
- Worker built substantive infrastructure across two clean commits before the rate limit:
  - **`2bc4a70`** — `crates/gf2-coding/examples/bench_sparse_csv_emitter.rs` (1077 lines) covering all four sparse op classes × all 7 fields × CSR/CSC/block-CSR/RCM/prefetch-d8 layout variants × random / structured / coding-theory corpus classes. Gated behind a new `bench-csv` feature on `gf2-coding`.
  - **`2fb5423`** — three new C++ harnesses in `benchmarks/reference/`:
    - `fflas_sparse_bench.cpp` (397 lines) — fflas-ffpack `fspmv`/`fspmm` over GF(p) on `Modular<int64_t>` and `Modular<float>`.
    - `linbox_sparse_bench.cpp` (364 lines) — LinBox `SparseMatrix::apply` (spmv) and `GaussDomain::NoReordering` (sparse-elim) over GF(2) and the four GF(p) primes.
    - `sparse_smoke.cpp` (204 lines) — protocol § 6 cross-equality oracle at n=16, exits non-zero on bitwise mismatch. Verified passing inside the pinned container for all five fields.
    - `Makefile` extended with three new targets.
  - Initial bench output: `dev/bench_results/2026-05-04-47698404-sparse{,-extended}.csv`.
- Worker noticed and FIXED a density-format bug in their uncommitted WIP — `fmt_density_c()` shim mirrors C `printf("%.6e", ...)` so Rust-side regime strings byte-match the fflas/linbox harness output (the cause of the `e-3` vs `e-03` join-key drift in tables.md).
- Worker committed-but-not-pushed: smoke.sh wiring, the format-fix, regenerated `sparse.csv`, freshly-built `sparse-host.txt` + `sparse-reference.csv` + initial `sparse-tables.md`.
- Worker did NOT produce the actual scorecard markdown (`dev/bench_results/2026-05-04-47698404-sparse-scorecard.md`) before the rate limit fired.
- Lead-preserve commit `7251db4` on `worktree-agent-47698404` captures all of the above (excluding built binaries — added to `.gitignore` in the same commit).

### What's left for `47698404` to actually close

In priority order:
1. **Re-run analyze.py** (or hand-merge) so `sparse-tables.md` no longer carries `PENDING` rows from the (already-fixed-but-not-yet-regenerated) e-3/e-03 mismatch. The fix landed in the lead-preserve commit; the tables.md file in the worker branch is the *pre-fix* output and needs regeneration after a fresh emitter run.
2. **Write the scorecard markdown** at `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` with the § 0–§ 7 structure from the dispatch contract:
   - § 0 TL;DR
   - § 1 Acceptance for `[hard]` criteria (CSR/CSC/block-CSR/RCM/prefetch coverage table; feasible-vs-no-go cell list)
   - § 2 Methodology
   - § 3 Per-cell scorecard (table per `(operation, field-family)`)
   - § 4 Feasible CPU gaps
   - § 5 No-go cases (with protocol § 8/§ 9 exclusion class cited)
   - § 6 Five-criterion confirmation
   - § 7 Document-attach checklist
3. **Run the bench day inside the pinned container** if the worktree-host quick run is insufficient. The worker reports `--quick` mode (n=1024, d=10/n) ran cleanly on the worktree host; verify scope decision.
4. **Run cargo-ci on main** after merging the worker branch. Note: `cargo fmt --all -- --check` may fail inside the worktree because `gf2-kernels-hip` is workspace-excluded (handoff-3 trap #8) — main-side `cargo-ci.sh` is authoritative.
5. **`jit doc add 47698404 <scorecard.md>`** + supplementary CSVs (host.txt, reference.csv).
6. **Run the gates: cargo-ci → code-review → doc-review → mark done.**

### Choice for next session

Two viable paths to close `47698404`:

- **(A) Re-dispatch a continuation worker** after the 11:30pm Helsinki rate-limit reset. Pass the worker the worktree path (`.claude/worktrees/agent-47698404`), the lead-preserve commit `7251db4`, and the "outstanding" list above. Risk: another rate-limit interruption, another lead-preserve cycle.
- **(B) Lead-finalize.** Lead writes the scorecard markdown + regenerates tables.md + runs gates. Risk: lead context-window eats the scorecard work; harder to escape if review surfaces findings.

**Recommendation: (A) re-dispatch a continuation worker** with a tight, focused scope (regen tables, write scorecard, run gates). The worker WIP is high-quality and the remaining work is mechanical/synthesis; a fresh worker session has plenty of headroom.

## Wave 4 readiness (after `47698404` closes)

- `47698404` closing → unblocks **`b13799ac` re-review** per session-3b user decision.
- `b13799ac` closes → unblocks **`9a715d75` re-review** per session-3 user decision.
- `9a715d75` closes → unblocks **`4c0d0202` (Wave 4 SOTA target matrix design doc)** as the only original Wave-3 dep still in_progress.
- After `4c0d0202` closes: `cbecfced` story closure (sota-reference-matrix) — Wave 5.
- `b0434149` (sota-baseline-scorecard) is also ready to close — all 5 of its task children are done as of `d0ca9482` closure this session.

### `b13799ac` re-review nuances (still load-bearing)

The strict reviewer's three persistent findings on `b13799ac`:
1. **Direct vs transitive smoke** (criterion #3 protocol literal). `47698404`'s `sparse_smoke.cpp` uses the same transitive pattern (fflas-ffpack ↔ scalar reference), establishing project convention.
2. **CSV path convention** (`dev/bench_results/` vs protocol's `benchmarks/results/`). Project convention precedent established.
3. **SSOT** — Conway poly hard-coded in 4 places + scalar `ref_gf2pow32_mul` duplicated C++/Rust. **Not addressed by `47698404` closing.** Lead-direct extraction to a shared header (e.g., `crates/gf2-core/src/primitive_polys.rs` exporting via a tiny `gf2_pow32_constants.h` header generated by `build.rs`, included by both `ntl_gf2pow32_smoke.cpp` and `ntl_bench.cpp`; the Rust test inlines the same constant) is the cleanest fix. **Either do this as a lead-direct cleanup before re-review, OR escalate to user with a fresh option set if SSOT remains a strict-reviewer FAIL after the smoke + path-convention findings clear.**

## Traps — do not repeat these

Carry-forward (still in force):
- All traps from `babcf05e-handoff-5.md`, `97bf0879-handoff{,-2,-3}.md`. Re-read on session resume.

New traps from session 4:

1. **Worker WIP density-format mismatch was self-fixed mid-task.** The worker noticed and added `fmt_density_c()` to mirror C `printf %.6e` zero-padded exponent. The mismatch caused `tables.md` rows to fail to merge (Rust `9.765625e-3` vs C `9.765625e-03`). The fix is in the lead-preserve commit `7251db4`. **The bench-output files in that commit are the PRE-fix output** — regenerate them after a fresh emitter run before treating tables.md as authoritative. Do NOT cite the existing tables.md PENDING rows as evidence of a missing measurement.

2. **`cargo-ci` continuation workers should rebuild the bench binaries before treating CSV output as authoritative.** The worktree's `make sparse_smoke` etc. use the local g++ (Arch) toolchain, not the pinned container. The smoke run in the worker's commit message ("all five fields pass") was on the local toolchain. The protocol § 5 contract requires container builds. The continuation worker must `./benchmarks/run.sh --skip-build=0 ...` (or equivalent) inside the pinned image before the scorecard's numbers count as "measured" rather than "qualified".

3. **Don't commit built bench binaries.** The lead-preserve session 4 added `benchmarks/reference/{fflas_sparse_bench, linbox_sparse_bench, sparse_smoke}` to `.gitignore`. They are 46–90 KB each and should never live in the repo.

## Reference artefacts (this session)

- This handoff: `dev/active/97bf0879-handoff-4.md`
- Progress file: `dev/active/97bf0879-progress.json` (lead updates next)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2,-3}.md`
- Lead-preserve commit: `7251db4` on `worktree-agent-47698404` (do not merge to main until session 5+ closes the issue)
- Closed-this-session: `d0ca9482` at `af6b7bc`
- Worker commits (not yet on main): `2bc4a70`, `2fb5423`, `7251db4` on `worktree-agent-47698404`
- Worker scope spec: `dev/plans/sparse_benchmark_corpus.md` (a3412e15)
- Protocol: `dev/plans/sota_reference_acceptance_protocol.md`
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`

## Open questions for the next session

None blocking. The next session lead can choose path (A) re-dispatch continuation or (B) lead-finalize per the *Choice for next session* block above.
