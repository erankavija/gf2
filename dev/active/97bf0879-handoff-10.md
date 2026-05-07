# Handoff -- Close gf2-core SOTA performance gaps (`97bf0879`) -- session 12

**Date:** 2026-05-06 / 2026-05-07 (session spans midnight)
**Session number:** 12
**Prior handoffs:**
- `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8,-9}.md` (sessions 1-9; sessions 10 and 11 wrote progress-file summaries only).
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`.

All prior-session traps remain in force unless explicitly resolved here.

## Current state

- Epic: `97bf0879` -- state: **in_progress**, claimed by `agent:project-lead`.
- **Wave 7 (GF(2) M4RI gap closure): DONE** -- 4/4 leaves closed.
- **Wave 8 (GF(2^m) reference + optimization): DONE** -- 4/4 leaves closed (a1172cea, e24f7839, fb271c41, d82c00a3).
- **Wave 9a (PLE/TRSM tuning + rank-deficient): DONE** -- 2/2 closed (73ec5da3, 2c52bcf6).
- **Wave 9b (close invert/solve/det rows): IN-FLIGHT** -- worker `afbd503db3f934f14` dispatched 2026-05-07 ~02:30Z on 7e41400f.
- **Wave 9c (4eb105f7 publish dense LA evidence): pending**, depends on 7e41400f.
- **Waves 10, 11, 12: pending**.
- Active claims: `agent:project-lead` on `97bf0879`; `agent:claude` on `7e41400f` (in-flight).

## What just happened (session 12)

### Wave 7 closure (111a3967)
- Doc-synthesis worker produced `dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md` (132 lines). R1 PASS. Closed at `336c9e1`.

### Wave 8a (a1172cea)
- Worker measured GF(2^m) post-PPC GEMM via `bench_csv_emitter` extended with `EmitterGf2m32Cfg`. R1 FAIL on (a) deferred GF(2^32) n=256/1024 cells, (b) inconsistent counts, (c) wrong follow-up issue routing. Path A taken: pinned container ran NTL `--large` for the missing n=256/1024 GF(2^32) rows. Surfaced bench-day anomaly: canonical 2026-05-05 GF(2^32) n=64 row is ~3.5x slow vs fresh re-measurement on same host. Fresh row used for verdicts. Closed at `cdb0e87` after R2 PASS.

### Wave 8b (e24f7839 + fb271c41 parallel via worktree)
- **fb271c41** (research): produced `dev/plans/gf2m_avx512_gfni_evaluation.md` (268 lines). Decision: AVX-512/GFNI **NOT REQUIRED** for SOTA closure on Zen-3 host class (M4RIE achieves reference numbers on AVX2; NTL uses VPCLMULQDQ also AVX2-class). Future direction documented for Zen-4+ hosts. R1 PASS. Closed at `e8c6784`.
- **e24f7839** (impl): worker built panelized GF(2^m) GEMM kernel (broadcast-multiply with I_TILE=4 row tiling, AVX2+VPCLMULQDQ). New files `crates/gf2-kernels-simd/src/{gf2m_gemm.rs, x86/gf2m_gemm.rs}`; new dispatch `crate::simd::maybe_gf2m_gemm()`. **3 cells PASS**: GF(2^32) n=64/256/1024 at 5.7x-6.7x of NTL. **4 cells [aspirational]** (user-approved Path A): GF(2^16) n=1024 (0.614 vs 0.667 threshold; close to single-core VPCLMULQDQ ceiling), GF(2^8) all sizes (M4RIE uses O(n^3/log n) Newton-John). Code-review went R1 FAIL (SSOT findings: duplicate Barrett helpers in `gf2m_gemm.rs` vs `gf2m_batch.rs`; duplicate dispatch path in `kernels/simd/mod.rs` vs `lib.rs`; evidence-doc claimed PARTIAL contradicting the JIT amendment). R2 FAIL (scalar-fallback branch in `wide.rs::try_simd_gemm_classical` lacked test coverage on SIMD-capable hosts). R3 PASS after extracting `gf2m_common.rs`, making `kernels/simd::maybe_gf2m_*` thin pass-throughs to `crate::simd`, updating evidence doc to reflect the amendment, and adding test entry point `Gf2mWide::scalar_panelized_gemm_fallback_for_test`. Closed at commit `0022a5f` + cleanup at `963d53c`.
- User-approved Path A amendment landed in `e24f7839` and `2c7548ae` JIT descriptions. The 4 [aspirational] cells are owned by the broader finite-field SOTA plan in `615db3b9` (user's separate planning task), not by duplicate impl issues under this story.
- Filed and rejected `c450f40b` (Newton-John GF(2^8) follow-up) as `resolution:duplicate` after user pointer that 615db3b9 already scopes that work.

### Wave 8c (d82c00a3)
- Doc-synthesis worker produced `dev/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md`. R1 FAIL on (a) GF(2^8) row citations off-by-one (referenced lines 31/32/33 of `e24f7839-gf2m-panelized.csv` but actual GF(2^8) rows are 32/33/34; line 31 is GF(2^31-1)), (b) doc not attached to parent story 2c7548ae. R2 PASS after lead-direct fixes. Closed at `11563a3`.

### Wave 9a (73ec5da3 + 2c52bcf6 parallel via worktree)
- **2c52bcf6** (impl): rank-deficient dense paths optimization. Threads `pivot_cols: Vec<usize>` through `ple_in_place` / `ple_in_place_window` / `ple_base_direct` so `split_compact` skips its O(rank * n) post-factorisation scan. Same-session bench: -7.7% to -43.8% wall-time wins on rank-deficient cells; +7.9% regression on full-rank uniform/64 (Vec overhead exceeds scan savings at small rank). Code-review R1 FAIL on (a) doc described old algorithm wrong (column-by-column instead of row-by-row), (b) doc said `pivot_cols.push(col_lo)` but code does `pivot_cols.push(col)`. R2 FAIL on (c) doc said criterion was `[aspirational]` -- contract-modifying language (forbidden per CLAUDE.md). R3 PASS after lead-direct doc fixes. Closed at `61e70a0`.
- **73ec5da3** (impl): PLE/TRSM block tuning. New `PLE_BASE_COLS` and updated `TRI_BASE_THRESHOLD` trait constants on `FiniteField`. Sweep selected `TRI_BASE_THRESHOLD = 8` (from `{4, 8, 16, 32, 64}`). Two pre-existing slow tests marked `#[ignore]`. R1 FAIL on missing TRSM data + missing TRI_BASE_THRESHOLD sweep. R2 FAIL on stale bench-comment "currently 32". R3 FAIL on **Lean SSOT divergence**: `proofs/Gf2Core/Funs.lean` (4 occurrences) and `scripts/fix-aeneas-sorrys.py` hardcoded TRI_BASE_THRESHOLD=32 against the new Rust value 8. R4 PASS after Lean sync + `lake build` verification (2109 jobs success). Closed at `a50afc2`.

### Concurrent user activity
- User created issue `615db3b9` ("Pursue fflas-like GF(251) GEMM performance" -> broader "Finite-field dense linear algebra SOTA catch-up plan") with 8 [hard] criteria and a comprehensive plan doc at `dev/active/615db3b9-finite-field-la-sota-plan.md`. User added a Status update and Phase 3 update to that doc this session reflecting e24f7839's closure. The plan owns deeper GF(2^m) algorithmic catch-up (Newton-John GF(2^8), GFNI/AVX-512 for Zen-4+).

### Concrete artefact landings (session 12)
- `dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md`
- `dev/bench_results/2026-05-06-a1172cea-gf2m-scorecard.md` (post-rework)
- `dev/bench_results/2026-05-06-a1172cea-gf2m-gf2-rows.csv`
- `dev/bench_results/2026-05-06-a1172cea-ntl-gf2pow32-large.csv` (path-A NTL pinned-container measurements)
- `dev/plans/gf2m_avx512_gfni_evaluation.md` (fb271c41 decision)
- `dev/bench_results/2026-05-06-e24f7839-panelized-gf2m-gemm.md`
- `dev/bench_results/2026-05-06-e24f7839-gf2m-panelized.csv`
- `dev/bench_results/2026-05-06-e24f7839-gf2pow32-panelized.csv`
- `dev/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md`
- `dev/bench_results/2026-05-07-2c52bcf6-rank-deficient-dense.md`
- `dev/bench_results/2026-05-07-73ec5da3-ple-trsm-tuning.md`
- New code: `crates/gf2-kernels-simd/src/{gf2m_gemm,x86/gf2m_gemm,x86/gf2m_common}.rs`; `Gf2mGemmFns` accessor on `crate::simd`; `try_simd_gemm_classical` hook on `Gf2mWide<1, Cfg>`; `ple_base_direct` + `pivot_cols` threading on PLE; `PLE_BASE_COLS` + updated `TRI_BASE_THRESHOLD` trait constants; Lean `Funs.lean` / `fix-aeneas-sorrys.py` synced.

### Commits on main this session
`aa585e9` (111a3967) -> `336c9e1` (close 111a3967) -> `bc4b45d` (a1172cea R1) -> `c5402de` (a1172cea R2 rework path-A) -> `cdb0e87` (close a1172cea) -> `e1bff44` (fb271c41) -> `e8c6784` (close fb271c41) -> `f899d00` (user-direct: 615db3b9 plan) -> `0022a5f` (e24f7839 panelized GEMM) -> `825c813` (615db3b9 status update + reject c450f40b) -> `7363ce5` (e24f7839 R1 rework: gf2m_common + dispatch consolidation) -> `e684fb9` (e24f7839 R2 rework: scalar-fallback test) -> `963d53c` (close fb271c41 + e24f7839) -> `919d2ab` (d82c00a3) -> `d59a24c` (d82c00a3 R1 rework) -> `11563a3` (close d82c00a3) -> `2c9de48` (Wave-8 progress.json) -> `42a6903` (2c52bcf6 base) + `851f136` + `d0c7bfd` (R1+R2 doc fixes) -> `61e70a0` (close 2c52bcf6) -> `ad6a3dd` (73ec5da3 R2 sweep + TRSM) -> `5ddf9a2` (Lean SSOT sync) -> `a50afc2` (close 73ec5da3).

## What to do next

In priority order:

- [ ] **Wave 9b finish**: 7e41400f worker (`afbd503db3f934f14`) is in-flight as of handoff write-time. When it returns, integrate, run gates, close. Worker may need rework on inheritance demonstration (does invert/solve/det actually inherit PLE/TRSM speedups? same-session pre/post is the operative bench).
- [ ] **Wave 9c**: dispatch `4eb105f7` (Publish dense LA parity evidence) once 7e41400f closes. Doc-synthesis pattern; mirror the GF(p) / GF(2) / GF(2^m) parity evidence docs.
- [ ] **Wave 10 (poly + sparse)**: 7 issues -- b87362a3, d1dd266c, 4a59d1f9, 8ccc1751, 3a37e0f6, 3643923d, 1726270d. Stories `66190ccd` + `54fd3f0b`. Several have parallel-able sub-waves.
- [ ] **Wave 11 (story closures)**: cc5de315 already done; 974a85bd ready (Wave-7 closed); 2c7548ae ready (Wave-8 closed); 72ab6d0e and 66190ccd close after Waves 9 + 10; 54fd3f0b closes after Wave 10 sparse.
- [ ] **Wave 12 (final aggregation)**: dece4e73, 2cfc4372, f00fd873, 39f02525, 8f3fdc34, 01ae4c20.

## Traps -- do not repeat these

Carry-forward (still in force):
- All traps from `97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8,-9}.md` and `babcf05e-handoff{,-2,-3,-4,-5}.md`. Re-read on session resume.

New traps from session 12:

1. **CSV row-number citations in evidence docs are off-by-one when the CSV has a header line and the doc cites `row N` meaning "data row N" rather than "line N including header".** d82c00a3 R1 FAIL: doc cited `e24f7839-gf2m-panelized.csv` rows 31/32/33 but data line 31 is GF(2^31-1), not GF(2^8). Lesson: always use absolute line numbers (header is line 1) and verify with `awk 'NR==<line>' file.csv` before committing.

2. **Aeneas/Lean post-processor scripts hardcode constants.** `scripts/fix-aeneas-sorrys.py` rewrites `TRI_BASE_THRESHOLD := field.traits.FiniteField.TRI_BASE_THRESHOLD.default ...` to a literal. When the Rust constant changes, BOTH the script's replacement string AND the already-generated `proofs/Gf2Core/Funs.lean` need updating. The Lean code is committed (not regenerated by `verify-lean.sh` on every run because Charon/Aeneas requires the build infra at `/data/aeneas-build/`). Audit recipe: `grep -rn "TRI_BASE_THRESHOLD\|WINOGRAD_THRESHOLD" scripts/ proofs/` after any change to those Rust constants.

3. **Bench-day CSVs can have host-anomaly rows.** The canonical 2026-05-05 NTL GF(2^32) n=64 row in `benchmarks/results/20260505T091600Z.csv` (7.539e7 ops/s) was reproducibly 3.5x slower than fresh re-measurement on the same host with the same image and seed (~2.67e8 ops/s). Likely thermal throttling or background contention. Lesson: when a single canonical row contradicts the trend, re-measure before treating it as the verdict baseline. The fresh row is now the operative reference; the canonical row is retained in the bench-day CSV with a caveat in `dev/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md` § 4b.

4. **Path A (NTL pinned container) is reachable from this host.** The pinned container `localhost/gf2-bench:ref` is locally cached. `ntl_bench --large` runs successfully on it. When a code-review failure cites missing reference data and the pinned container is reachable, prefer running it (Path A) over filing a follow-up issue (Path B). Worker a1172cea R1->R2 demonstrated this works.

5. **"Aspirational" framing in evidence docs is contract-modifying.** 2c52bcf6 R2 FAIL: the worker wrote "the issue success criterion `[aspirational]` states rank-deficient paths as the focus" to justify a small full-rank regression. The criterion's actual marker is `[hard]`. Even when the criterion's *scope phrase* limits coverage (here, to rank-deficient rows), the evidence doc cannot rewrite the maturity tier. Phrase it as: "the +X% regression is on a cell outside the criterion's scope phrase; the criterion is `[hard]` and is met within scope". This is the no-argue rule from session-7 trap #2 generalised to evidence docs, not just rework dispatches.

6. **Cherry-pick conflicts are likely when two parallel workers touch ple.rs / triangular.rs or any other shared file.** Wave 9a 73ec5da3 + 2c52bcf6 conflicted on `ple.rs::ple_in_place_window` because 73ec5da3 added `PLE_BASE_COLS` extraction while 2c52bcf6 threaded `pivot_cols`. Manual integration was straightforward but cost ~30 min. When dispatching parallel workers that may touch the same file, the lead can pre-plan a serialization order (impl 1 first, then impl 2 rebases) instead of cherry-picking.

7. **Worker rebase + cherry-pick interplay can confuse main's commit topology.** When a worker reports "rebased onto main HEAD `<X>`" but main has moved past `<X>` (because the lead made other commits), `git cherry-pick <worker-tip>` still works -- the cherry-pick is sequenced in the right order. But the worker's branch then contains commits that already exist on main, which makes `git diff main..worker-branch` confusing. Lesson: tear down the worktree immediately after cherry-pick.

## Open questions needing user input

None blocking at handoff time. The session-12 user decisions (Path A amendment for e24f7839, 615db3b9 plan creation, c450f40b rejection as duplicate) are all resolved.

## Reference artefacts

- This handoff: `dev/active/97bf0879-handoff-10.md`
- Progress file: `dev/active/97bf0879-progress.json` (lead updates after this handoff lands)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7,-8,-9}.md`
- Wave 8 design / evidence: `dev/plans/gf2m_avx512_gfni_evaluation.md`, `dev/bench_results/2026-05-{06,07}-{a1172cea,e24f7839,fb271c41,d82c00a3}-*`
- Wave 9a evidence: `dev/bench_results/2026-05-07-{73ec5da3-ple-trsm-tuning,2c52bcf6-rank-deficient-dense}.md`
- Concurrent user plan: `dev/active/615db3b9-finite-field-la-sota-plan.md`
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol: `~/.claude/skills/project-lead/references/lead-review-protocol.md`
- Project conventions: `/home/vkaskivuo/Projects/gf2/CLAUDE.md`
- JIT events log: `.jit/events.jsonl` (append-only)
- Gate definitions: `.jit/gates.json`
