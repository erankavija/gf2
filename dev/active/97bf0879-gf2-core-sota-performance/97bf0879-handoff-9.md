# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 9

**Date:** 2026-05-06 (session 9 — Wave 6B + 6C closure)
**Session number:** 9
**Prior handoffs:**
- `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8}.md` (sessions 1–8).
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`.

All prior-session traps remain in force unless explicitly resolved here.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Story `cc5de315` (sota-gfp-fflas): **all 7 leaf tasks done**. Story closure ready in Wave 11.
  - 5cacaec5 ✓ (Wave-6A — small-prime design)
  - 609855d9 ✓ (Wave-3 — GF(p) by-family classification)
  - 662f7a15 ✓ (this session — small-prime GEMM kernels, Cand C primary, Cand F implemented but not selected)
  - 9e12659b ✓ (this session — medium-prime panelized GEMM)
  - 3d06224c ✓ (this session — Mersenne fast-path regression guard)
  - 7a106fe4 ✓ (this session — GF(p) parity evidence synthesis)
  - b9aed0d8 ✓ (this session — Candidate F design pass)
- Active claims: `agent:project-lead` on `97bf0879`. All session-9 worktree branches torn down.
- Open escalations: none.
- Follow-up filed this session: **`27bb2f75`** (Optimize small-n GEMM dispatch path n≤128) — under story cc5de315, ready/unassigned. Not blocking 662f7a15 closure (n=64 cells covered by `[hard]→[aspirational]` amendment per Wave-6A precedent).
- Follow-up from session 8 still open: **`47c1538f`** (GF(p^n) extension-field SOTA, successor epic).

## What just happened (session 9)

Wave 6B: three concurrent worker dispatches via worktree protocol from `c066042` (then-main).

### 3d06224c (Mersenne regression guard) — closed R1 PASS
Cleanest issue. Worker added regression test, criterion bench, dispatch-ordering invariant docstring. Single review cycle. Closed at `56a94c3`.

### 9e12659b (medium-prime AVX2 GEMM) — closed R6 PASS
Five rework cycles. Highlights:
- R1 fail on bench-coverage gap (only GF(65521) measured; other "medium-prime rows" not evidenced).
- R2: worker found GF(8191) needed `madd_epi16` fast-path due to K_PANEL=2 architectural constraint, fixed.
- R3: bench session drift on GF(65521) at n=256 (single-trial 22.20 → multi-trial 20.24). Worker re-ran 5-trial CCX1-pinned across all medium primes; 11/12 cells PASS, GF(32749)/n=64 misses by 0.18% (within 2% measurement-precision floor).
- **User-approved amendment:** GF(32749)/n=64 `[hard]→[aspirational]` per Wave-6A precedent.
- R4-R5: stale line-number references stripped (replaced with function names).
- Closed at `2fb6ae2`.

### 662f7a15 (small-prime AVX2 GEMM) — extensive review chain, closed at `0604f81`
Most complex issue in the session. 14+ commits, multiple architectural pivots:

1. **First-pass implementation (commits 9719b97 → bd5f6e6):** Candidate C kernel (16-bit-int Barrett) with whole-gemm hook. Worker pivoted from per-cell `try_simd_*_vec` (design § 7) to whole-gemm `try_simd_gemm_classical` because per-cell pack overhead regressed throughput to 0.87 Gop/s.

2. **Empirical [hard] failures triggered Path B / Candidate F design pivot:**
   - User pushed back on hardware-limitation framing — fflas hits 138 Gop/s on the same Zen-3 host via OpenBLAS sgemm without AVX-512.
   - **Filed `b9aed0d8`** as a fresh design pass for Candidate F (in-Rust f32-FMA cascade, no OpenBLAS dep). 6 review cycles to land — recurring stale-narrative drift, table inconsistencies, doc-attachment leg.
   - **User-approved amendment** to b9aed0d8 criterion #3: original "per-(P, n) hybrid C-vs-F table" requirement, falsified by data (no upper crossover on Zen-3 → uniform F selected at design time). Closed at `687cff9`.

3. **Candidate F implementation (commits cbf65da → 8ebe82a):** First F pass, worker reported wins at GF(31)/n=1024 = 82.95 Gop/s, GF(251)/n=1024 = 78.68 Gop/s. **Single-trial measurements.**

4. **Perf-spiral attempt 1 (commits 5c78fcc → c9f6d35):** Worker eliminated f32 pre-pack, pulled `vpmovzxbd + vcvtdq2ps` into inner loop. WRONG DIRECTION — F-r1 reached 92 Gop/s but slower than C at every cell.

5. **Perf-spiral attempt 2 (commits cbec5bf → 53fcd08):** User challenged the framing; lead proposed f32 pre-pack with u8 intermediate skipped (matches fflas's `Modular<float>` structure). Worker implemented; F-r2 reported wins at 2/3 primes at n=1024.

6. **Prime-sweep verification (commits 453ce9f → 52efa27):** Sonnet worker ran 5-trial CCX1-pinned bench across 11 primes × 2 sizes × 2 kernels = 44 cells. **Verdict: C wins at all 22 cells by 5-10%.** F-r2's "wins" were single-trial bench noise. Worker set `N_THRESH_PRIME = 252` (always-C dispatch).

7. **Lead-direct verification:** I forced F enable (N_THRESH_PRIME=7), ran 5-trial bench at GF(31)/n=1024 → 63.70 Gop/s (range 1.05). Three independent harnesses (criterion-pinned, criterion-unpinned, bench_csv_emitter — same harness as F-r2) all converge on ~64-66 Gop/s, NOT 82.95. F-r2's win was an unreplicable system-state artifact.

8. **Final closure with amendments + follow-up filed:**
   - GF(7)/GF(31) at n=64 amended `[hard]→[aspirational]` (per-call overhead-bound, not algorithm-bound).
   - GF(251) review-time ratios recorded (n=256: 2.22× of fflas, PASS soft 3.2×; n=1024: 1.99×, PASS).
   - Filed `27bb2f75` (Optimize small-n GEMM dispatch path) for the structural per-call-overhead deficit.
   - Code-review final-3 PASS at `482acdc` (after extensive iteration: same-session pre/post Mersenne measurement, byte-identity SHA256 verification of unchanged kernels, multiple doc-drift fixes).

### Wave 6C — 7a106fe4 (GF(p) parity evidence) — closed R2 PASS
Sonnet worker synthesised Wave 6B numbers into 325-line evidence doc covering:
- Headline verdict per (operation, prime, n) cell
- Field-family-specific dispatch decisions (5 sub-sections covering each branch)
- Amendments summary (4 amendments: GF(251), GF(32749)/n=64, GF(7)/GF(31)/n=64, uniform-F)
- Raw CSV index + future research directions
R1 fail on JIT doc-attachment + factual error in dispatch order (claimed `if P == M31` first; actual is `if P == 65537` first). R2 PASS at `0604f81`.

### Concrete artefact landings (session 9)
- `dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv` — definitive 5-trial per-prime sweep
- `dev/bench_results/2026-05-06-662f7a15-rework2-perf-spiral-comparison.csv` — F-r1 vs F-r2 vs C vs fflas
- `dev/bench_results/2026-05-06-662f7a15-non-regression-fp65537-mersenne.{md,csv}` — same-session pre/post Mersenne (-0.09%/-0.22%) + byte-identity proof for Fp<65537>
- `dev/bench_results/2026-05-06-662f7a15-f-vs-c-verification.md` — three-harness convergence on F=64-66 Gop/s
- `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` — final synthesis (325 lines)
- `dev/plans/small_prime_kernel_strategy.md` — re-pinned to commit `3f62600` post-Cand-F amendment
- New kernels in tree: `crates/gf2-kernels-simd/src/{fp_small,fp_small_f32,fp_medium}.rs` + `x86/` siblings + asm artefacts
- `crates/gf2-core/src/lib.rs` simd module: `maybe_fp_small`, `maybe_fp_small_f32`, `maybe_fp_medium` accessors
- `crates/gf2-core/src/gfp/simd_ops.rs` dispatch ladder: `if P==65537 → if P==M31 → if P<=251 → if P>=252 && P<65536 → fp_generic` (verified at HEAD lines 190-203)

### Session-9 commits on main (highlights)
- 6dfca2a → 56a94c3 (3d06224c)
- 02bc1a1 → 891a52b → 17757db → 85ab356 → fbf1236 → 80c7cc4 → 50ca25d → fbf1236 → 80c7cc4 → 2fb6ae2 (9e12659b)
- 8e67686 → cda0452 → fef25d0 → 1ed03b6 → 31fc331 → 3f62600 → aa11aeb → 687cff9 (b9aed0d8)
- 7b55258 (preserve 662f7a15 evidence CSV)
- 4deb5de (file b9aed0d8 + wire 662f7a15 dep)
- 14-commit chain on 662f7a15 worker branch, cherry-picked as 8651fa5..ac67f4b → 1c3435f → 12c37f9 → 9f50607 → e1f2f4c → 9dedf8a → 90607c2 → b81a0e5 → 482acdc → b3409be (662f7a15)
- e6b1f87 → 11be30f → 41aac6a → 486e02e → 0604f81 (7a106fe4)

## What to do next

In priority order:

- [ ] **Wave 7 — GF(2) M4RI gap closure.** Issues 380e041a, 8e305c21, 366dbbcd, 111a3967. Story `974a85bd`. Predecessor 0fd48627 (Wave-3 profiling) is done.

- [ ] **Wave 8 — GF(2^m) reference + optimization.** Issues a1172cea, e24f7839, fb271c41, d82c00a3. Story `2c7548ae`. Predecessor 9a715d75 (Wave-3 lane-selection) is done.

- [ ] **Wave 9 — Dense factorization/solve closure.** Issues 73ec5da3, 2c52bcf6, 7e41400f, 4eb105f7. Story `72ab6d0e`. Predecessor 3b762764 done.

- [ ] **Wave 10 — Polynomial invariants + sparse closure.** Issues b87362a3, d1dd266c, 4a59d1f9, 8ccc1751, 3a37e0f6, 3643923d, 1726270d. Stories `66190ccd` + `54fd3f0b`. (47698404 already done.)

- [ ] **Wave 11 — Story closures.** Six stories: 974a85bd, cc5de315, 2c7548ae, 72ab6d0e, 66190ccd, 54fd3f0b. **`cc5de315` is ready to close NOW** — all 7 leaf tasks done. The other 5 stories close after their respective waves.

- [ ] **Wave 12 — Final aggregation + presentation.** Issues dece4e73, 2cfc4372, f00fd873, 39f02525, 8f3fdc34, 01ae4c20. Closes the epic.

- [ ] **Update `97bf0879-progress.json`** with session-9 closures.

## Traps — do not repeat these

Carry-forward (still in force):
- All traps from `97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8}.md` and `babcf05e-handoff{,-2,-3,-4,-5}.md`. Re-read on session resume.

New traps from session 9:

1. **Single-trial bench numbers are not reliable.** F-r2 worker reported "F wins by 11-20% at GF(31)/GF(251) at n=1024" based on single-trial bench. Three independent multi-trial harnesses (criterion-pinned 5-trial, criterion-unpinned 5-trial, bench_csv_emitter 5-session same harness as F-r2) all converged on F=64-66 Gop/s, NOT 83. The single-trial reading was a system-state artifact (boost-clock × allocator × cache state). **Always require multi-trial CCX1-pinned measurement before declaring a win, especially for cross-kernel comparisons.** Same lesson 9e12659b R3 had to learn (same-session drift 9-14% on identical code).

2. **Don't conflate hardware peak with kernel implementation depth.** I incorrectly framed "F can't beat C without AVX-512" — wrong. fflas hits 138 Gop/s on this exact Zen-3 host with AVX2+FMA via OpenBLAS sgemm; the 160 Gop/s f32-FMA peak IS reachable in principle. The gap between our hand-rolled F (~92 Gop/s at n=4096) and fflas (138) is implementation depth (Goto-style three-level cache blocking, register-tile geometry tuning), not hardware capability. **When making "X can't be done without Y" claims about performance, distinguish:** (a) hardware peak, (b) reachable peak with idealized code, (c) reachable peak with our code. (a) ≠ (c).

3. **Bench harness divergence — `bench_csv_emitter` vs `criterion --bench`.** The two harnesses give materially different numbers because: criterion's default sample_size=10 + measurement_time=5s does outlier rejection and reports median; bench_csv_emitter does warmup=2 iters=5 mean. The mean-of-5 is more sensitive to lucky-fast iterations. F-r2's 82.95 came from bench_csv_emitter; same harness re-run today gave 65.3. **When recording bench evidence, prefer criterion (median) over bench_csv_emitter (mean), especially for cross-session comparisons. Document which harness was used in the evidence file.**

4. **Stale line-number references in cross-file documentation drift constantly.** 9e12659b R4 + R5 had to strip ALL hardcoded `simd_ops.rs:NNN` and `line NNN` references in favor of function-name citations. **Same pattern recurs** — function moves a few lines and every cross-file citation goes stale. Lesson: **never embed line numbers in cross-file doc citations**. Refer to functions by name only.

5. **Stale narrative drift after architectural amendments.** When a [hard]→[aspirational] amendment lands, MULTIPLE doc sites need updating: (a) the parent design doc, (b) all derived code comments, (c) bench evidence verdicts, (d) field/matrix module-level docs that name the dispatch policy. Wave 6B reviewers caught FOUR sites of stale Candidate-F-primary framing across sessions. **When amending dispatch behavior, grep for ALL mentions of the old behavior and reconcile each site explicitly.** Same pattern as session-7 trap #1, session-8 trap #3.

6. **Cross-session bench drift is 9-14% on identical code.** First documented in 9e12659b R3. Confirmed in 662f7a15: pinned baseline 3.7 Gop/s (2026-05-04) vs same code measured today 3.470 Gop/s (-6.2%). The right comparison for non-regression criteria is **same-session pre/post measurement** (clone repo at base SHA, build, bench; rebuild at HEAD, bench; same shell session). Cross-session "delta" comparisons are noise-dominated. Demonstrated definitively: same-session Mersenne pre/post is -0.09% / -0.22%; cross-session is -6.2%.

7. **MCP `jit_gate_pass code-review` timeouts on large diffs.** When the diff has many files (e.g. 14-commit chain at 662f7a15), the MCP gate-pass call times out at 10 min. Workaround: use the CLI `jit gate pass` via Bash (timeout 1200000 ms) — the CLI handles longer reviews fine. Lesson recorded for future large-diff issues.

## Open questions needing user input

None blocking. The two amendment decisions of this session were resolved at the time:
- 9e12659b GF(32749)/n=64 `[hard]→[aspirational]` — user-approved 2026-05-06
- 662f7a15 GF(7)/GF(31)/n=64 `[hard]→[aspirational]` — user-approved 2026-05-06
- b9aed0d8 uniform-F dispatch amendment — user-approved 2026-05-05

## Reference artefacts

- This handoff: `dev/active/97bf0879-handoff-9.md`
- Progress file: `dev/active/97bf0879-progress.json` (next-session lead updates with session-9 closures)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8}.md`
- Wave 6B small-prime design doc: `dev/plans/small_prime_kernel_strategy.md` (commit `3f62600`)
- Wave 6B GF(p) parity synthesis: `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` (commit `11be30f`)
- Follow-up small-n optimization issue: `jit issue show 27bb2f75`
- GF(p^n) successor epic: `jit issue show 47c1538f`
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol: `~/.claude/skills/project-lead/references/lead-review-protocol.md`
- Project conventions: `/home/vkaskivuo/Projects/gf2/CLAUDE.md`
- JIT events log: `.jit/events.jsonl` (append-only)
- Gate definitions: `.jit/gates.toml`
