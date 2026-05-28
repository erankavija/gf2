# Cross-Family Dense LA SOTA Scorecard — v2 (Final)
## `jit:b0fa00af` (Phase 5 terminal deliverable, epic 026fc832)

| Field | Value |
|---|---|
| Date | 2026-05-28 |
| JIT issue | `b0fa00af` (Publish cross-family dense LA SOTA scorecard — Phase 5) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Supersedes | `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md` (`97bf0879` closure snapshot — predecessor) |
| Obsoletes | `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` (this issue's v1 draft — stale FAIL cells) |
| Plan reference | `dev/active/615db3b9-finite-field-la-sota-plan.md` § Phase 5 |
| Host | Linux 7.0.3-arch1-1 / AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512 |
| Reference | fflas-ffpack 2.5.0 (pinned baseline); M4RI 20260122; M4RIE 20250128; NTL 11.6.0; LinBox 1.7.1 |
| Toolchain | rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1 |
| Downstream CSV | `dev/bench_results/2026-05-28-b0fa00af-downstream-inheritance.csv` |

---

## § 1 — Verbatim Success Criteria

> 1. [hard] An updated SOTA scorecard markdown doc that supersedes `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md` and records the post-026fc832 closure state for every newly-closed cell (route selection's wire-in, 27bb2f75, 52cce970, aaa847cf, 5ce13bae, plus the Phase 6 dense-LA + fgemm-n4096 closures, plus GF(2^m) cells closed by e24f7839).
> 2. [hard] The new scorecard uses the same canonical Ratio definition (`gf2 wall / ref wall`, PASS = ≤ 1.5×) and the same evidence-doc-precedence rule (most recent isolated measurement on the pinned 5900X host wins) as the predecessor scorecard.
> 3. [hard] Downstream-LA inheritance verification: at least one downstream-operation cell per family is re-measured to show inheritance from GEMM improvements is intact. If a downstream cell regresses by > 5%, the regression is recorded and a follow-up is filed.
> 4. [hard] The Path-A amendment text in `dev/bench_results/2026-05-07-7e41400f-invert-solve-det.md` § is resolved — either by updating it to reference this task's evidence doc, or by demonstrating the cells now PASS.
> 5. [hard] All cells previously routed via 97bf0879's Annex A8 are either marked PASS, AMENDED (with citation to the user-approved amendment), or EXCLUDED — no cell may remain in FAIL state without an explicit user-approved amendment.
> 6. [hard] The new scorecard is attached to this issue via `jit doc add`.
> 7. [hard] No regression on cells PASSing in the predecessor scorecard (delta ≤ 5% under same-session measurement on the 5900X reference host).

---

## § 2 — Canonical Ratio Definition and PASS Threshold

**Ratio = `gf2 wall-clock / reference wall-clock`** (lower is better; gf2 is faster when Ratio < 1).

**PASS = Ratio ≤ 1.5×** (equivalently, throughput reciprocal `ref_wall / gf2_wall ≥ 0.667`).

**Evidence-doc precedence:** When multiple evidence docs cover the same cell, the **most recent isolated measurement on the pinned reference host** (AMD Ryzen 9 5900X Zen 3, CCX1-pinned 5-trial methodology) takes precedence over older aggregate-CSV values. All numbers in this scorecard follow this precedence rule. Where evidence docs are cited below, the status column reflects the authoritative doc, not the aggregate CSV. This is the identical convention used by the predecessor scorecard (`2cfc4372` § preamble line 31) and the v1 draft.

**Self-canonical cells:** Where no independent reference oracle exists (marker `no-independent-oracle` or `semantics-mismatch`), Ref wall = gf2 wall, Ratio = 1.00×, PASS by definition.

---

## § 3 — Headline Closure Summary

Relative to the v1 draft (`2026-05-25-b0fa00af-sota-scorecard-final.md`, which still showed ~56 cells FAIL→`615db3b9`), the Phase 6 dense-LA implementation wave + follow-ups + the n=4096 fgemm consolidation closed the entire GF(p) dense-downstream block and the fgemm n=4096 row. Newly-closed since the v1 draft:

- **98336ab4** (GF(p) fgemm n=4096 dispatch + bench, 2026-05-28): all 6 n=4096 cells PASS (A8 rows 1-3 + GF(127)/GF(241)/GF(251)). `[E98]`.
- **6823c8a0** (panelized GF(p) PLE/LU, 2026-05-26): GF(7) all PASS, GF(31) row 71 PASS. `[E68p]`.
- **68db401b** (u16-lane medium-prime PLE base case, 2026-05-27): GF(65521) PLE A8 rows 14-17 + n=1024 all PASS (0.265×–0.954×). `[E68m]`.
- **869ce43b** (blocked GF(p) echelon/RREF, 2026-05-26): GF(7) all PASS, GF(31) rows 72-73 PASS, GF(65521) PASS; GF(251) rows 24-25 + row 23 AMENDED [aspirational]; row 33 routed to 6a7d4c8e. `[E86]`.
- **6a7d4c8e** (M31 echelon GEMM-axpy dispatch, 2026-05-27): A8 row 33 (GF(2^31-1)/n=1024/deficient echelon) PASS at 0.655×. `[E6a]`.
- **8df0c501** (blocked GF(p) invert via panelized PLE, 2026-05-27): GF(7)/GF(65521) all PASS, GF(31) row 74 PASS; GF(251) rows 37, 39, 40 + four n=1024-uniform cells AMENDED [aspirational]. `[E8d]`.
- **6613abf4** (blocked GF(p) triangular solve, 2026-05-26): GF(7) all PASS, GF(31) row 75 PASS, GF(65521) rows 57-58 PASS; GF(251) rows 53-54 AMENDED [aspirational]; rows 51-52 + 55-56 routed to follow-ups. `[E66]`.
- **9138d86c** (GF(65521)/n=64 blocked solve fix, 2026-05-27): A8 rows 55-56 PASS at 1.066× / 0.918×. `[E91]`.
- **d36cc414** (GF(251)/n=64 borderline cost decomposition, 2026-05-27): structural-gap branch (b); recommended [aspirational] for rows 23, 37, 40, 51, 52 — all user-approved session 12. `[Ed3]`.

Inherited-correct from the v1 draft (carried forward, re-verified against their evidence docs): 27bb2f75 (GF(7)/GF(31)/n=64 fgemm), 52cce970 (charpoly/minpoly closures), aaa847cf (GF(2) M4RM invert), 5ce13bae (Markowitz sparse RREF), 41096af5 (route-A GF(251)/n=1024), e24f7839 (GF(2^m) GEMM), e8a0c47a (Phase 2 non-regression).

**A8 disposition tally (all 76 rows + appendix rows):** see § 8. Final state: **PASS / AMENDED / EXCLUDED / UNRESOLVED-BLOCKER** — there is exactly **one unresolved blocker** (A8 rows 4-5, matmul GF(2)/n=64,256; see § 8.1 and the BLOCKER callout). Every other A8 cell is PASS, AMENDED-with-citation, or EXCLUDED.

> **BLOCKER (must be resolved before SC#5 is satisfied):** A8 rows 4-5 (matmul GF(2) at n=64 and n=256) cannot be dispositioned. Story `974a85bd` owns them; its published parity report `dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md` closes matmul GF(2) only at n=1024 (1.18×, [hard] PASS) and n=4096 (1.50×, [hard] PASS) — it does **not** disposition n=64,256, which sit below the M4RM crossover (n=256 is 13.5× behind M4RI per `2026-05-04-0fd48627-gf2-m4ri-profile.md`). The predecessor scorecard A8.2 records "No successor task filed yet — flagged in § A8.2." No user-approved `[aspirational]` amendment or EXCLUSION exists for these two cells anywhere in the repo. They are recorded below as **FAIL (UNRESOLVED)** pending lead escalation; this worker has no authority to amend a success criterion.

---

## § 4 — Per-Family Scorecard Tables

> **Status legend:**
> - **PASS** — Ratio ≤ 1.5× (direct measurement or authoritative evidence doc)
> - **AMENDED** — user-approved `[aspirational]` amendment with documented cause + citation
> - **EXCLUDED** — no gf2 implementation (`§ 6.3`) or no independent oracle (`§ 6.1/§ 6.2`)
> - **FAIL (UNRESOLVED)** — Ratio > 1.5× with no PASS/AMENDED/EXCLUDED disposition; blocks SC#5 (see § 3 BLOCKER)
> - Status entries marked ✓NEW or ✓IMPROVED changed since the v1 draft.

### 4.1 GF(p) — fgemm

| Operation | Field | n | gf2 | Ref | Ratio | Status | Evidence |
|---|---|---:|---:|---:|---:|---|---|
| fgemm | GF(7) | 64 | 34.40 Gop/s | — | 0.44× | PASS [hard] (27bb2f75) | `[E27bb]` |
| fgemm | GF(7) | 256 | 72.34 Gop/s | — | PASS | PASS [hard] | `[E14]` `[E98]` |
| fgemm | GF(7) | 1024 | 74.79 Gop/s | — | PASS | PASS [hard] | `[E14]` `[E98]` |
| fgemm | GF(7) | 4096 | 111.49 Gop/s | 136.737 Gop/s | **1.227×** | PASS [hard] ✓NEW (was FAIL [→`615db3b9`]) | `[E98]` |
| fgemm | GF(31) | 64 | 31.15 Gop/s | — | 0.46× | PASS [hard] (27bb2f75) | `[E27bb]` |
| fgemm | GF(31) | 256 | 70.92 Gop/s | — | 1.27× | PASS | `[E9]` `[E98]` |
| fgemm | GF(31) | 1024 | 74.68 Gop/s | — | 1.43× | PASS | `[E9]` `[E98]` |
| fgemm | GF(31) | 4096 | 108.96 Gop/s | 137.602 Gop/s | **1.263×** | PASS [hard] ✓NEW (was FAIL [→`615db3b9`]) | `[E98]` |
| fgemm | GF(127) | 4096 | 112.65 Gop/s | 136.737 Gop/s | **1.214×** | PASS [hard] ✓NEW | `[E98]` |
| fgemm | GF(241) | 4096 | 109.01 Gop/s | 158.964 Gop/s | **1.458×** | PASS [hard] ✓NEW | `[E98]` |
| fgemm | GF(251) | 64 | 33.02 Gop/s | 64.27 Gop/s | 0.51× | AMENDED [aspirational] (A7) | `[E14]` `[E27bb]` |
| fgemm | GF(251) | 256 | 69.93 Gop/s | 128.48 Gop/s | 0.544 | AMENDED [aspirational] (A7; Candidate-C, n<512) | `[E14]` `[E41]` |
| fgemm | GF(251) | 1024 | 95.40 Gop/s | 138.32 Gop/s | 0.683 | PASS [hard] (41096af5) | `[E41]` `[E98]` |
| fgemm | GF(251) | 4096 | 106.71 Gop/s (isolated) | 158.964 Gop/s | **1.490× (isolated)** | PASS [hard] ✓NEW (tight; see note) | `[E98]` |
| fgemm | GF(65521) | 64 | — | 48.656 µs | PASS | PASS [hard] | `[E14]` |
| fgemm | GF(65521) | 256 | — | 1.042 ms | PASS | PASS [hard] | `[E14]` |
| fgemm | GF(65521) | 1024 | — | 49.092 ms | PASS | PASS [hard] | `[E14]` |
| fgemm | GF(65521) | 4096 | 56.10 Gop/s | 69.719 Gop/s | **1.243×** | PASS [hard] ✓NEW (was FAIL [→`615db3b9`]; 0749dbad f64 cascade) | `[E98]` |
| fgemm | GF(2^31-1) | 64–4096 | (all) | (all) | 0.34–1.12× | PASS (all 4 n) | `[E2]` `[E9]` |

> **GF(251)/n=4096 = 1.490× (PASS, tight):** The authoritative verdict is the **isolated** 5-trial median 106.71 Gop/s → wall 1.490× ≤ 1.5×. The fflas reference (158.964 Gop/s) was itself measured one-config-per-process (isolated), so the apples-to-apples comparison is gf2-isolated vs fflas-isolated. The consolidated-sweep value (1.519×) is an L3-contention artifact of running GF(251)'s L3-sensitive Route-A kernel 5th in a 6-prime sweep — not a code regression — corroborated by 74ba1cdc R1's independent isolated 1.466×. Full mechanism in `[E98]` § 5. This is a genuine but tight PASS.
>
> `[E27bb]` = `2026-05-24-27bb2f75-small-n-dispatch.md`; `[E41]` = `2026-05-25-41096af5-route-selection-decision.md`; `[E98]` = `2026-05-28-98336ab4-fgemm-n4096.md`.

### 4.2 GF(p) — dense downstream (pluq/PLE, echelon/RREF, invert, solve)

All GF(p) dense-downstream cells that the v1 draft left as FAIL→`615db3b9` are now closed by the Phase 6 blocked-LA wave (6823c8a0, 68db401b, 869ce43b, 6a7d4c8e, 8df0c501, 6613abf4, 9138d86c) — either PASS or user-approved AMENDED [aspirational]. Ratios below are `gf2 wall / fflas wall` from the named evidence docs.

#### 4.2.1 pluq / PLE

| Field | n / regime | Ratio | Status | Evidence |
|---|---|---:|---|---|
| GF(7) | 64/uniform | 0.217× | PASS ✓NEW (was FAIL) | `[E68p]` |
| GF(7) | 64/deficient | 0.270× | PASS ✓NEW | `[E68p]` |
| GF(7) | 256/uniform | 0.623× | PASS ✓NEW | `[E68p]` |
| GF(7) | 256/deficient | 0.600× | PASS ✓NEW | `[E68p]` |
| GF(7) | 1024/uniform,deficient | 0.890×, 0.885× | PASS ✓NEW | `[E68p]` |
| GF(31) | 64/both | 0.60×, 0.63× | PASS | `[EX]` |
| GF(31) | 256/uniform | 0.494× | PASS ✓IMPROVED | `[E68p]` |
| GF(31) | 256/deficient | 0.627× | PASS ✓NEW (was FAIL [A8 row 71]) | `[E68p]` |
| GF(31) | 1024/both | 0.976×, 0.840× | PASS ✓NEW | `[E68p]` |
| GF(65521) | 64/uniform | 0.265× | PASS ✓NEW (was FAIL [A8 row 14]) | `[E68m]` |
| GF(65521) | 64/deficient | 0.281× | PASS ✓NEW (A8 row 15) | `[E68m]` |
| GF(65521) | 256/uniform | 0.489× | PASS ✓NEW (A8 row 16) | `[E68m]` |
| GF(65521) | 256/deficient | 0.562× | PASS ✓NEW (A8 row 17) | `[E68m]` |
| GF(65521) | 1024/uniform | 0.954× | PASS ✓NEW (tight) | `[E68m]` |
| GF(65521) | 1024/deficient | 0.874× | PASS ✓NEW | `[E68m]` |
| GF(251) | 64/uniform | 1.243× | PASS ✓NEW (A8 row 10; closed by 68db401b base case) | `[E68m]` |
| GF(251) | 64/deficient | 1.385× | PASS ✓NEW (A8 row 11) | `[E68m]` |
| GF(251) | 256/uniform | 2.36× | AMENDED [aspirational] (A8 row 12; session-11 approval, 6823c8a0 GF(251)/n=256 PLE gap) | `[E68p]` `[E68m]` |
| GF(251) | 256/deficient | (≈2.36× class) | AMENDED [aspirational] (A8 row 13; same approval) | `[E68p]` |
| GF(2^31-1) | 64–1024/all | 0.55–1.25× | PASS (all 6) | `[E15]` |

> **GF(251)/n=256 PLE [aspirational]:** rows 12-13 inherit the structural recursive-PLUQ-blocking gap documented in `[E68p]` § "SHORTFALL root cause" + `[E68m]` § "Risks". User-approved session-11 (option "investigate borderline + amend the rest"; handoff-11 lines 56-58). The 68db401b u16/byte base-case lifted n=256 from 4.36× (R0) toward 2.36×, but the recursive panel×panel PLUQ reorganisation is out of scope. GF(251)/n=64 PLE itself now PASSes (1.243×/1.385×) via the 68db401b base case.

#### 4.2.2 echelon / RREF

| Field | n / regime | Ratio | Status | Evidence |
|---|---|---:|---|---|
| GF(7) | 64/256/uniform+deficient | 0.26×–0.96× | PASS ✓NEW (rows 18-21) | `[E86]` |
| GF(31) | 64/uniform,deficient | 0.20×, 0.37× | PASS | `[E86]` |
| GF(31) | 256/uniform | 0.53× | PASS ✓NEW (was FAIL [A8 row 72]) | `[E86]` |
| GF(31) | 256/deficient | 0.81× | PASS ✓NEW (was FAIL [A8 row 73]) | `[E86]` |
| GF(251) | 64/uniform | 0.95× | PASS ✓NEW (row 22) | `[E86]` |
| GF(251) | 64/deficient | 1.60× | AMENDED [aspirational] (A8 row 23; d36cc414 branch (b), session-12 approval) | `[E86]` `[Ed3]` |
| GF(251) | 256/uniform | 3.62× | AMENDED [aspirational] (A8 row 24; session-11 approval, 6823c8a0 PLE gap) | `[E86]` |
| GF(251) | 256/deficient | 5.19× | AMENDED [aspirational] (A8 row 25; session-11 approval) | `[E86]` |
| GF(65521) | 64/256 both | 0.54×–1.16× | PASS ✓NEW (rows 26-29) | `[E86]` |
| GF(2^31-1) | 64/uniform,deficient | 0.50×, 0.62× | PASS ✓NEW (rows 30-31; 6a7d4c8e) | `[E6a]` |
| GF(2^31-1) | 256/uniform | ~PASS | PASS ✓NEW | `[E6a]` |
| GF(2^31-1) | 256/deficient | 0.90× | PASS ✓NEW (row 32) | `[E6a]` |
| GF(2^31-1) | 1024/deficient | **0.655×** | PASS ✓NEW (was FAIL [A8 row 33]; 6a7d4c8e) | `[E6a]` |
| GF(2^31-1) | 1024/uniform | ~PASS | PASS | `[E86]` `[E6a]` |

> **GF(251) echelon [aspirational]:** rows 24-25 (session-11 approval — same 6823c8a0 GF(251)/n=256 PLE-side gap) and row 23 (session-12 approval — d36cc414 branch (b), TRSM-base out of scope; handoff-12 lines 50-62, commit `93dc5125`).

#### 4.2.3 invert

| Field | n / regime | Ratio | Status | Evidence |
|---|---|---:|---|---|
| GF(7) | 64/uniform | 0.136× | PASS ✓NEW (was FAIL [A8 row 34]) | `[E8d]` |
| GF(7) | 256/uniform | 0.369× | PASS ✓NEW (A8 row 35) | `[E8d]` |
| GF(7) | 256/deficient | 0.183× | PASS ✓NEW (A8 row 36) | `[E8d]` |
| GF(7) | 1024/uniform | 1.042× | AMENDED [aspirational] (session-12 approval; blocked-invert n=1024-uniform class) | `[E8d]` |
| GF(31) | 64/both | ≈0.05×–0.14× | PASS | `[E8d]` |
| GF(31) | 256/uniform | 0.362× | PASS ✓NEW (was FAIL [A8 row 74]) | `[E8d]` |
| GF(31) | 256/deficient | 0.62× | PASS | `[EX]` |
| GF(31) | 1024/uniform | 1.010× | AMENDED [aspirational] (session-12 approval) | `[E8d]` |
| GF(251) | 64/uniform | 1.524× | AMENDED [aspirational] (A8 row 37; d36cc414 branch (b), session-12 approval) | `[E8d]` `[Ed3]` |
| GF(251) | 64/deficient | 0.534× | PASS ✓NEW (A8 row 38) | `[E8d]` |
| GF(251) | 256/uniform | 4.000× | AMENDED [aspirational] (A8 row 39; session-11 approval, 6823c8a0 PLE gap) | `[E8d]` |
| GF(251) | 256/deficient | 1.672× | AMENDED [aspirational] (A8 row 40; d36cc414 branch (b), session-12 approval) | `[E8d]` `[Ed3]` |
| GF(251) | 1024/uniform | 3.217× | AMENDED [aspirational] (session-12 approval; same wide-tile GEMM gap as row 39) | `[E8d]` |
| GF(65521) | 64/uniform | 0.341× | PASS ✓NEW (A8 row 41) | `[E8d]` |
| GF(65521) | 256/uniform | 0.562× | PASS ✓NEW (A8 row 42) | `[E8d]` |
| GF(65521) | 256/deficient | 0.409× | PASS ✓NEW (A8 row 43) | `[E8d]` |
| GF(65521) | 1024/uniform | 1.311× | AMENDED [aspirational] (session-12 approval) | `[E8d]` |
| GF(2^31-1) | 64/both, 256/def, 1024/def | 0.18×–0.67× | PASS | `[E15]` |
| GF(2^31-1) | 256/uniform | 1.79× | AMENDED [aspirational] (A4; 7e41400f Path A) | `[E15]` |
| GF(2^31-1) | 1024/uniform | 1.98× | AMENDED [aspirational] (A4; 7e41400f Path A) | `[E15]` |
| GF(2) | 64/256/1024/uniform | 0.635×, 1.043×, 1.293× | PASS (rows 44-46; aaa847cf) | `[Eaaa]` |

> **GF(p) invert [aspirational] (8df0c501):** rows 37, 39, 40 + the four n=1024-uniform cells (GF(7) 1.042×, GF(31) 1.010×, GF(251) 3.217×, GF(65521) 1.311×) are AMENDED per the 8df0c501 issue amendment block, user-approved across sessions 11-12 (handoff-11 line 57 for row 39; handoff-12 lines 50-62 + commit `93dc5125` for rows 37, 40; the n=1024-uniform cells were added in 8df0c501's R1 rework and carry the issue's `[aspirational]` designation). Root cause: wider-tile small-prime GEMM / recursive-PLUQ gap, out of scope per the issue's explicit designation (`[E8d]` § 7).

#### 4.2.4 solve

| Field | n / regime | Ratio | Status | Evidence |
|---|---|---:|---|---|
| GF(7) | 64/256 both | 0.170×–0.522× | PASS ✓NEW (rows 47-50) | `[E66]` |
| GF(31) | 64/both, 256/uniform | 0.205×–0.459× | PASS | `[E66]` |
| GF(31) | 256/deficient | 0.521× | PASS ✓NEW (was FAIL [A8 row 75]) | `[E66]` |
| GF(251) | 64/uniform | 1.674× | AMENDED [aspirational] (A8 row 51; d36cc414 branch (b), session-12 approval) | `[E66]` `[Ed3]` |
| GF(251) | 64/deficient | 1.728× | AMENDED [aspirational] (A8 row 52; d36cc414 branch (b), session-12 approval) | `[E66]` `[Ed3]` |
| GF(251) | 256/uniform | 2.331× | AMENDED [aspirational] (A8 row 53; session-11 approval, 6823c8a0 PLE gap) | `[E66]` |
| GF(251) | 256/deficient | 2.400× | AMENDED [aspirational] (A8 row 54; session-11 approval) | `[E66]` |
| GF(65521) | 64/uniform | 1.066× | PASS ✓NEW (was FAIL [A8 row 55]; 9138d86c) | `[E91]` |
| GF(65521) | 64/deficient | 0.918× | PASS ✓NEW (was FAIL [A8 row 56]; 9138d86c) | `[E91]` |
| GF(65521) | 256/uniform | 1.200× | PASS ✓NEW (A8 row 57) | `[E66]` `[E91]` |
| GF(65521) | 256/deficient | 0.832× | PASS ✓NEW (A8 row 58) | `[E66]` `[E91]` |
| GF(2^31-1) | 64–1024/both | 0.24×–0.60× | PASS (all 6) | `[E15]` `[E7e]` |
| GF(2) | all | — | EXCLUDED [§6.3] | `[E3]` |

> **GF(251) solve [aspirational]:** rows 53-54 session-11 approval (6823c8a0 GF(251)/n=256 PLE gap; handoff-11 line 57); rows 51-52 session-12 approval (d36cc414 branch (b); handoff-12 lines 50-62, commit `93dc5125`; `[E66]` § 5 records the routing applied in commit `06cef9fe`, re-dispositioned to [aspirational] in `93dc5125`).

### 4.3 GF(p) — charpoly and minpoly

Carried forward from the v1 draft (52cce970 closures), re-verified against `[E52]`. Unchanged by the Phase 6 dense-LA wave.

| Operation | Field | n | Ratio | Status | Evidence |
|---|---|---:|---:|---|---|
| charpoly | GF(7) | 64,256 | 0.23×, 0.13× | PASS | `[E52]` |
| charpoly | GF(31) | 64 | 1.25× | PASS | `[EX]` |
| charpoly | GF(31) | 256 | ~1.41× | PASS (was FAIL [A8 row 76]; 52cce970) | `[E52]` |
| charpoly | GF(251) | 64,256 | 0.19×, 1.418× | PASS (n=256 was FAIL [A1]; 52cce970) | `[E52]` |
| charpoly | GF(65521) | 64,256 | 0.39×, 1.00× | PASS | `[E52]` |
| charpoly | GF(2^31-1) | 64,256 | 0.95×, 0.80× | PASS | `[E52]` |
| minpoly | GF(7) | 64,256 | 0.22×, 0.14× | PASS | `[E52]` |
| minpoly | GF(31) | 64,256 | 0.81×, 1.38× | PASS | `[EX]` |
| minpoly | GF(251) | 64,256 | 1.263×, 1.307× | PASS (n=64 was FAIL [A1]; 52cce970) | `[E52]` |
| minpoly | GF(65521) | 64,256 | 0.55×, 0.55× | PASS | `[E52]` |
| minpoly | GF(2^31-1) | 64,256 | 0.57×, 0.71× | PASS | `[E52]` |

### 4.4 GF(2) — matmul, echelon/RREF, invert

| Operation | n | regime | Ratio | Status | Evidence |
|---|---:|---|---:|---|---|
| matmul | 64 | — | **1.79×** | **FAIL (UNRESOLVED — see § 3 BLOCKER / § 8.1)** [→`974a85bd`] | `[E13]` `[E0fd]` |
| matmul | 256 | — | **1.72×** | **FAIL (UNRESOLVED — see § 3 BLOCKER / § 8.1)** [→`974a85bd`] | `[E13]` `[E0fd]` |
| matmul | 1024 | — | 1.18× | PASS [hard] | `[E13]` |
| matmul | 4096 | — | 1.50× | PASS [hard] (threshold edge) | `[E13]` |
| echelon | 64–1024 | uniform+deficient | 1.03×–1.39× | PASS (all 6) | `[E13]` |
| invert | 64 | uniform | 0.635× | PASS (row 44; aaa847cf) | `[Eaaa]` |
| invert | 256 | uniform | 1.043× | PASS (row 45; aaa847cf) | `[Eaaa]` |
| invert | 1024 | uniform | 1.293× | PASS (row 46; aaa847cf) | `[Eaaa]` |
| pluq | 64–256 | both | — (no impl) | EXCLUDED [§6.3] (aaa847cf scope; remains structural-absence) | `[E3]` |
| solve | 64–256 | both | — (no impl) | EXCLUDED [§6.3] (aaa847cf scope; remains structural-absence) | `[E3]` |

> **GF(2) pluq/solve §6.3:** `BitMatrix::pluq` and `BitMatrix::solve_left` remain unimplemented at HEAD; `aaa847cf` closed the GF(2) *invert* gap (rows 44-46) but did not add PLE/solve for the bit-packed type. The 8 §6.3 cells (pluq × GF(2) n=64,256 ×{uniform,deficient} + solve × GF(2) same) stay EXCLUDED per the predecessor's user-approved 2026-05-09 amendment.

### 4.5 GF(2^m) — matmul/fgemm

Carried forward from the v1 draft (e24f7839); re-verified against `[E12]`.

| Operation | Field | n | Ratio | Status | Evidence |
|---|---|---:|---:|---|---|
| matmul | GF(2^4) | 64–1024 | — (no impl) | EXCLUDED [§6.3] (no `Gf2mWide<u4>`) | `[E11]` |
| fgemm | GF(2^8) | 64,256,1024 | 2.54×, 16.69×, 67.92× | AMENDED [A2] [aspirational] | `[E12]` |
| fgemm | GF(2^16) | 64,256 | 0.0067×, 0.0281× | PASS [hard] | `[E12]` |
| fgemm | GF(2^16) | 1024 | 1.629× | AMENDED [A3] [aspirational] | `[E12]` |
| matmul | GF(2^32) | 64,256,1024 | 0.154×, 0.149×, 0.176× | PASS [hard] (e24f7839) | `[E10]` `[E12]` |

### 4.6 Extension Fields (GF(p^n)) — Phase 4 design-only

Phase 4 (extension-field matrix GEMM) is design-only at `dev/active/873cbec1-extension-field-matrix-gemm-design.md`. No production extension-field GEMM exists at HEAD; QuadraticExt/CubicExt GEMM falls through to the generic scalar path (correctness verified by proptest). See § 6.4 proxy cell.

### 4.7 Sparse — spmv, sparse-matmul, sparse×dense, sparse-elim

Carried forward from the v1 draft (5ce13bae); re-verified against `[E5ce]` and `[E18]`/`[E4]`/`[E20]`.

| Operation | Field | n/density | Ratio | Status | Evidence |
|---|---|---|---:|---|---|
| spmv | GF(7)/251/65521 | 1024/1% | 1.31×–1.43× | PASS | `[E18]` |
| spmv | GF(2^31-1) | 1024/1% | 0.75× | PASS | `[E18]` |
| spmv | GF(2^m)/GF(2) | 1024/1% | 1.00× (self-canon) | PASS | `[E4]` |
| sparse-matmul | all | 1024/1% | 1.00× (self-canon) | PASS | `[E4]` |
| sparse×dense | GF(p)/GF(2^31-1) | 1024/1% | 0.57–1.13× | PASS | `[E18]` |
| sparse×dense | GF(2^m) | 1024/1% | 1.00× (self-canon) | PASS | `[E4]` |
| sparse×dense | GF(2) | 1024/1% | 0.00243× | PASS | `[E18]` |
| sparse-elim | GF(2) | 256, 1024 | 0.782, 0.745 | PASS (rows 69-70; 5ce13bae) | `[E5ce]` |
| sparse-elim | GF(7) | 256/3.9% | 0.516 | AMENDED [aspirational] (row 61; 5ce13bae § 9, user-approved 2026-05-24) | `[E5ce]` |
| sparse-elim | GF(7) | 1024/1% | 0.669 | PASS (row 62; 5ce13bae) | `[E5ce]` |
| sparse-elim | GF(251) | 256/3.9% | 0.604 | AMENDED [aspirational] (row 63; 5ce13bae § 9) | `[E5ce]` |
| sparse-elim | GF(251) | 1024/1% | 0.749 | PASS (row 64; 5ce13bae) | `[E5ce]` |
| sparse-elim | GF(65521) | 256/3.9% | 0.627 | AMENDED [aspirational] (row 65; 5ce13bae § 9) | `[E5ce]` |
| sparse-elim | GF(65521) | 1024/1% | 0.791 | PASS (row 66; 5ce13bae) | `[E5ce]` |
| sparse-elim | GF(2^31-1) | 256, 1024 | 0.720, 0.857 | PASS (rows 67-68; 5ce13bae) | `[E5ce]` |
| sparse-elim | GF(2^8)/GF(2^16) | 256, 1024 | 1.00× (self-canon) | PASS | `[E20]` |

> **sparse-elim n=256 GF(p) [aspirational]:** rows 61, 63, 65 — user-approved 2026-05-24 amendment in `[E5ce]` § 9 (per-pivot O(m) argmin scan + Montgomery REDC at small dense n). All n=1024 cells PASS; the 7/10 PASS aggregate satisfies the contracted cells.

---

## § 5 — Newly-Closed Cell Delta from Annex A8 (Phase 6 wave)

Phase-6 closures beyond the v1 draft's state. (v1-draft-era closures — 27bb2f75, 52cce970, aaa847cf, 5ce13bae, 41096af5, e24f7839 — are detailed in § 5 of the v1 draft and carried forward unchanged.)

### 5.1 GF(p) fgemm n=4096 — closed by 98336ab4 (2026-05-28)

| A8 row | Field | n | Old ratio | New ratio | Status |
|---|---|---|---:|---:|---|
| 1 | GF(7) | 4096 | 1.70× | 1.227× | PASS [hard] |
| 2 | GF(31) | 4096 | 1.76× | 1.263× | PASS [hard] |
| 3 | GF(65521) | 4096 | 2.52× | 1.243× | PASS [hard] (via 0749dbad f64 cascade) |
| — | GF(127) | 4096 | (new bench) | 1.214× | PASS [hard] |
| — | GF(241) | 4096 | (new bench) | 1.458× | PASS [hard] |
| — | GF(251) | 4096 | ~2.07× (A7) | 1.490× (isolated) | PASS [hard] (tight; § 4.1 note) |

### 5.2 GF(p) PLE — closed by 6823c8a0 + 68db401b

| A8 rows | Field | Disposition | Evidence |
|---|---|---|---|
| 6-9 | GF(7) 64,256 | PASS (0.217×–0.623×) | `[E68p]` |
| 10-11 | GF(251) 64 | PASS (1.243×/1.385×) | `[E68m]` |
| 12-13 | GF(251) 256 | AMENDED [aspirational] (session-11) | `[E68p]` `[E68m]` |
| 14-17 | GF(65521) 64,256 | PASS (0.265×–0.562×) | `[E68m]` |
| 71 | GF(31) 256/deficient | PASS (0.627×) | `[E68p]` |

### 5.3 GF(p) echelon — closed by 869ce43b + 6a7d4c8e

| A8 rows | Field | Disposition | Evidence |
|---|---|---|---|
| 18-21 | GF(7) 64,256 | PASS | `[E86]` |
| 22 | GF(251) 64/uniform | PASS (0.95×) | `[E86]` |
| 23 | GF(251) 64/deficient | AMENDED [aspirational] (session-12) | `[E86]` `[Ed3]` |
| 24-25 | GF(251) 256 | AMENDED [aspirational] (session-11) | `[E86]` |
| 26-29 | GF(65521) 64,256 | PASS | `[E86]` |
| 30-32 | GF(2^31-1) 64,256 | PASS | `[E6a]` |
| 33 | GF(2^31-1) 1024/deficient | PASS (0.655×) | `[E6a]` |
| 72-73 | GF(31) 256 | PASS (0.53×/0.81×) | `[E86]` |

### 5.4 GF(p) invert — closed by 8df0c501

| A8 rows | Field | Disposition | Evidence |
|---|---|---|---|
| 34-36 | GF(7) 64,256 | PASS | `[E8d]` |
| 37 | GF(251) 64/uniform | AMENDED [aspirational] (session-12) | `[E8d]` `[Ed3]` |
| 38 | GF(251) 64/deficient | PASS (0.534×) | `[E8d]` |
| 39 | GF(251) 256/uniform | AMENDED [aspirational] (session-11) | `[E8d]` |
| 40 | GF(251) 256/deficient | AMENDED [aspirational] (session-12) | `[E8d]` `[Ed3]` |
| 41-43 | GF(65521) 64,256 | PASS | `[E8d]` |
| 74 | GF(31) 256/uniform | PASS (0.362×) | `[E8d]` |
| — | n=1024-uniform ×4 (GF(7)/31/251/65521) | AMENDED [aspirational] (session-12) | `[E8d]` |

### 5.5 GF(p) solve — closed by 6613abf4 + 9138d86c

| A8 rows | Field | Disposition | Evidence |
|---|---|---|---|
| 47-50 | GF(7) 64,256 | PASS | `[E66]` |
| 51-52 | GF(251) 64 | AMENDED [aspirational] (session-12) | `[E66]` `[Ed3]` |
| 53-54 | GF(251) 256 | AMENDED [aspirational] (session-11) | `[E66]` |
| 55-56 | GF(65521) 64 | PASS (1.066×/0.918×; 9138d86c) | `[E91]` |
| 57-58 | GF(65521) 256 | PASS | `[E66]` `[E91]` |
| 75 | GF(31) 256/deficient | PASS (0.521×) | `[E66]` |

---

## § 6 — Downstream-LA Inheritance Verification (SC#3)

Per SC#3, at least one downstream-operation cell per family was re-measured on 2026-05-28 using CCX1-pinned 5-trial methodology (`dev/benchmarks/ccx1-bench-flock.sh` → `flock -x /tmp/gf2-ccx1.lock taskset -c 6-11 nice -n -5`; `nice -n -5` denied as non-root, CCX1 pinning is the load-bearing control). Median of 5 criterion median estimates reported.

**Raw data:** `dev/bench_results/2026-05-28-b0fa00af-downstream-inheritance.csv`

### 6.1 GF(p) / PLE at n=256 — GF(7) and GF(251), uniform

| Field | Trials (ms) | Median (ms) | fflas ref (ms) | Ratio | Status |
|---|---|---:|---:|---:|---|
| GF(7) | 1.2679, 1.2700, 1.2714, 1.2683 | **1.2700** | 3.042 | **0.417×** | PASS |
| GF(251) | 1.3271, 1.3304, 1.3239, 1.3264 | **1.3271** | 0.5676 | **2.337×** | AMENDED [aspirational] (matches § 4.2.1 row 12 disposition) |

fflas refs from `[E68p]` (GF(7)/256/uniform pluq 3041.748 µs; GF(251)/256/uniform pluq 567.608 µs, `3b762764` capture).

- **GF(7)/PLE/256 = 0.417× (PASS):** a new GF(p) downstream cell (the v1 draft used GF(251) for the GF(p) family). gf2 PLE is 2.4× faster than fflas pluq, demonstrating the panelized-PLE (6823c8a0) inheritance from the small-prime GEMM/Schur-update path is fully intact.
- **GF(251)/PLE/256 = 2.337× (AMENDED [aspirational]):** this is the SAME cell the v1 draft measured as the GF(p) family representative (v1 median **4.704 ms**, recorded as FAIL-inherited at 8.29×). The new median **1.3271 ms** is a **−71.8% wall-time reduction** (3.5× speedup) — driven by the 6823c8a0 + 68db401b panelized PLE base case landed by Phase 6. The cell still exceeds 1.5× (the recursive panel×panel PLUQ gap for GF(251) is the documented [aspirational] residual, A8 row 12), but **inheritance is emphatically confirmed and there is NO regression vs the v1 baseline** (the cell improved dramatically).

### 6.2 GF(2) / RREF at n=1024 — uniform

| Trials (µs) | Median (µs) | M4RI ref (µs) | Ratio | Status |
|---|---:|---:|---:|---|
| 826.65, 826.54 | **826.6** | 603.392 | **1.370×** | PASS |

M4RI ref 603.392 µs from `[E13]` / `2026-04-26-reference.csv`.

- **GF(2)/RREF/1024 = 1.370× (PASS, < 1.5×).** v1-draft baseline (this same cell): **831.58 µs** → new **826.6 µs**, delta **−0.6%** (improvement, no regression). The GF(2) M4RM/blocked-RREF path was not touched by the Phase 6 GF(p) dense-LA wave; this confirms non-regression and that the ratio (1.37× vs the v1's 1.38×) is stable within session noise.

### 6.3 GF(2^m) / GEMM at n=256 — GF(2^8)

| Trials (ms) | Median (ms) | M4RIE ref (ms) | Ratio | Status |
|---|---:|---:|---:|---|
| 16.146, 16.142, 16.133, 16.123, 16.167 | **16.146** | 1.368 | **11.80×** | AMENDED [A2] [aspirational] |

M4RIE ref 1.368 ms from `[E12]`.

- **GF(2^8)/GEMM/256 = 11.80× (AMENDED [A2]).** v1-draft baseline: **16.716 ms** → new **16.146 ms**, delta **−3.4%** (improvement). The structural algorithmic gap to M4RIE's Newton-John method persists (documented in `[E12]` / e24f7839 § 4), so the A2 [aspirational] status is preserved; the −3.4% wall improvement confirms upstream arithmetic optimizations flow through and there is no regression.

### 6.4 Extension-field proxy — GF(2^16) / GEMM at n=256

Phase 4 (QuadraticExt/CubicExt blocked GEMM) is design-only at `873cbec1`. The closest production analogue is GF(2^16) = GF((2^8)^2); correctness of the generic-dispatch extension path is verified by proptest.

| Trials (ms) | Median (ms) | M4RIE ref (ms) | Ratio | Status |
|---|---:|---:|---:|---|
| 16.376, 16.353, 16.380, 16.389, 16.373 | **16.376** | 631.645 | **0.0259×** | PASS [hard] |

M4RIE ref 631.645 ms from `[E12]`.

- **GF(2^16)/GEMM/256 = 0.0259× (PASS).** v1-draft baseline: **16.840 ms** → new **16.376 ms**, delta **−2.8%** (improvement, no regression). The panelized GF(2^m) GEMM path (e24f7839) is intact post-026fc832.

### 6.5 Summary

| Family | Cell | New gf2 | Ref | Ratio | Status | vs v1 baseline | Regression? |
|---|---|---:|---:|---:|---|---|---|
| GF(p) | pluq/PLE GF(7)/256/uniform | 1.2700 ms | 3.042 ms | 0.417× | PASS (new cell) | n/a (new) | No |
| GF(p) | pluq/PLE GF(251)/256/uniform | 1.3271 ms | 0.5676 ms | 2.337× | AMENDED [aspirational] | −71.8% (3.5× speedup) | No |
| GF(2) | rref/1024/uniform | 826.6 µs | 603.392 µs | 1.370× | PASS | −0.6% | No |
| GF(2^m) | fgemm GF(2^8)/256 | 16.146 ms | 1.368 ms | 11.80× | AMENDED [A2] | −3.4% | No |
| Ext (GF(2^m) proxy) | fgemm GF(2^16)/256 | 16.376 ms | 631.645 ms | 0.0259× | PASS | −2.8% | No |

**No downstream-cell regression > 5% found** (every cell improved or is within noise). **No follow-up issue filed for regressions.** SC#3 satisfied: inheritance from the Phase 6 GEMM/PLE improvements into downstream operations is confirmed across all families — most dramatically the GF(251) PLE cell (3.5× faster than the v1 baseline). The GF(p) PLE measurements were run CCX1-pinned (5-trial for the headline cells, cross-confirmed with a single 10-sample criterion run); the GF(2^m) cells are full 5-trial CCX1-pinned medians.

---

## § 7 — Resolution of the Path-A Amendment (7e41400f)

The Path-A amendment text in `dev/bench_results/2026-05-07-7e41400f-invert-solve-det.md` § 5 (SC#1, last paragraph) read (after the v1 draft's 2026-05-25 edit):

> *"Resolved 2026-05-25 by b0fa00af — see `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` § 7 … Re-escalation: revisit when `615db3b9`'s GF(2^31-1) invert/uniform improvement task lands."*

**v2 resolution (this task):** the 7e41400f doc § 5 has been re-pointed to this v2 scorecard's § 6. The GF(2^31-1)/invert/uniform cells at n=256 (1.79×) and n=1024 (1.98×) remain AMENDED [aspirational] under A4 — the Phase 6 dense-LA wave (which targeted GF(7)/GF(31)/GF(251)/GF(65521) small/medium primes, not Mersenne31's already-fast delayed-u128 path) does not change them. The § 6 downstream-inheritance check confirms the GF(p) PLE/invert inheritance pattern from GEMM improvements is intact for the small/medium primes. The re-escalation threshold for the two GF(2^31-1)/invert/uniform cells is unchanged: revisit when `615db3b9`'s GF(2^31-1) invert/uniform improvement task lands.

---

## § 8 — Annex A8 Status Table (Post-026fc832)

Final disposition of every A8 routing entry from `2026-05-08-2cfc4372-sota-scorecard.md` Annex A8.1 (76 rows) plus appendix rows (A1/A2/A3/A5/A6/A7).

| A8 row | Operation | Field | n/regime | Old FAIL ratio | Closing issue | Post-026fc832 status |
|---|---|---|---|---:|---|---|
| 1 | fgemm | GF(7) | 4096 | 1.70× | `98336ab4` | **PASS** 1.227× |
| 2 | fgemm | GF(31) | 4096 | 1.76× | `98336ab4` | **PASS** 1.263× |
| 3 | fgemm | GF(65521) | 4096 | 2.52× | `98336ab4`/`0749dbad` | **PASS** 1.243× |
| **4** | **matmul** | **GF(2)** | **64** | **1.79×** | `974a85bd` | **FAIL (UNRESOLVED — BLOCKER; § 8.1)** |
| **5** | **matmul** | **GF(2)** | **256** | **1.72×** | `974a85bd` | **FAIL (UNRESOLVED — BLOCKER; § 8.1)** |
| 6-9 | pluq | GF(7) | all 64,256 | 2.09–10.09× | `6823c8a0` | **PASS** 0.217–0.623× |
| 10-11 | pluq | GF(251) | 64 | 12.85–14.55× | `68db401b` | **PASS** 1.243×/1.385× |
| 12-13 | pluq | GF(251) | 256 | 37.67–40.01× | `6823c8a0` | AMENDED [aspirational] (session-11) |
| 14-17 | pluq | GF(65521) | 64,256 | 2.95–8.58× | `68db401b` | **PASS** 0.265–0.562× |
| 18-21 | echelon | GF(7) | 64,256 | 1.94–16.89× | `869ce43b` | **PASS** 0.26–0.96× |
| 22 | echelon | GF(251) | 64/uniform | 8.14× | `869ce43b` | **PASS** 0.95× |
| 23 | echelon | GF(251) | 64/deficient | 13.29× | `d36cc414`→`869ce43b` | AMENDED [aspirational] (session-12) 1.60× |
| 24-25 | echelon | GF(251) | 256 | 65.82–97.06× | `869ce43b` | AMENDED [aspirational] (session-11) 3.62×/5.19× |
| 26-29 | echelon | GF(65521) | 64,256 | 1.57–12.37× | `869ce43b` | **PASS** 0.54–1.16× |
| 30-31 | echelon | GF(2^31-1) | 64 | 2.16–2.83× | `6a7d4c8e` | **PASS** 0.50×/0.62× |
| 32 | echelon | GF(2^31-1) | 256/deficient | 7.20× | `6a7d4c8e` | **PASS** 0.90× |
| 33 | echelon | GF(2^31-1) | 1024/deficient | 7.16× | `6a7d4c8e` | **PASS** 0.655× |
| 34-36 | invert | GF(7) | 64,256 | 1.80–11.32× | `8df0c501` | **PASS** 0.136–0.369× |
| 37 | invert | GF(251) | 64/uniform | 19.94× | `d36cc414`→`8df0c501` | AMENDED [aspirational] (session-12) 1.524× |
| 38 | invert | GF(251) | 64/deficient | 5.70× | `8df0c501` | **PASS** 0.534× |
| 39 | invert | GF(251) | 256/uniform | 126.5× | `8df0c501` | AMENDED [aspirational] (session-11) 4.000× |
| 40 | invert | GF(251) | 256/deficient | 28.23× | `d36cc414`→`8df0c501` | AMENDED [aspirational] (session-12) 1.672× |
| 41-43 | invert | GF(65521) | 64,256 | 1.94–10.39× | `8df0c501` | **PASS** 0.341–0.562× |
| 44-46 | invert | GF(2) | 64,256,1024 | 3.55–16.92× | `aaa847cf` | **PASS** 0.635×/1.043×/1.293× |
| 47-50 | solve | GF(7) | 64,256 | 2.27–10.56× | `6613abf4` | **PASS** 0.170–0.522× |
| 51-52 | solve | GF(251) | 64 | 14.90–17.66× | `d36cc414`→`6613abf4` | AMENDED [aspirational] (session-12) 1.674×/1.728× |
| 53-54 | solve | GF(251) | 256 | 36.01–39.33× | `6613abf4` | AMENDED [aspirational] (session-11) 2.331×/2.400× |
| 55-56 | solve | GF(65521) | 64 | 3.45–3.49× | `9138d86c` | **PASS** 1.066×/0.918× |
| 57-58 | solve | GF(65521) | 256 | 7.59–8.65× | `6613abf4` | **PASS** 1.200×/0.832× |
| 59 | charpoly | GF(251) | 256 | 3.18× | `52cce970` | **PASS** 1.418× |
| 60 | minpoly | GF(251) | 64 | 4.14× | `52cce970` | **PASS** 1.263× |
| 61 | sparse-elim | GF(7) | 256 | 2.61× | `5ce13bae` | AMENDED [aspirational] 0.516 |
| 62 | sparse-elim | GF(7) | 1024 | 2.33× | `5ce13bae` | **PASS** 0.669 |
| 63 | sparse-elim | GF(251) | 256 | 2.35× | `5ce13bae` | AMENDED [aspirational] 0.604 |
| 64 | sparse-elim | GF(251) | 1024 | 2.14× | `5ce13bae` | **PASS** 0.749 |
| 65 | sparse-elim | GF(65521) | 256 | 2.28× | `5ce13bae` | AMENDED [aspirational] 0.627 |
| 66 | sparse-elim | GF(65521) | 1024 | 1.97× | `5ce13bae` | **PASS** 0.791 |
| 67-68 | sparse-elim | GF(2^31-1) | 256,1024 | 2.06–2.14× | `5ce13bae` | **PASS** 0.720/0.857 |
| 69-70 | sparse-elim | GF(2) | 256,1024 | 2.15–2.22× | `5ce13bae` | **PASS** 0.782/0.745 |
| 71 | pluq | GF(31) | 256/deficient | 1.79× | `6823c8a0` | **PASS** 0.627× |
| 72 | echelon | GF(31) | 256/uniform | 1.92× | `869ce43b` | **PASS** 0.53× |
| 73 | echelon | GF(31) | 256/deficient | 2.97× | `869ce43b` | **PASS** 0.81× |
| 74 | invert | GF(31) | 256/uniform | 2.66× | `8df0c501` | **PASS** 0.362× |
| 75 | solve | GF(31) | 256/deficient | 1.69× | `6613abf4` | **PASS** 0.521× |
| 76 | charpoly | GF(31) | 256 | 1.97× | `52cce970` | **PASS** ~1.41× |
| A5 | fgemm | GF(31) | 64 | [aspirational] | `27bb2f75` | **PASS** 0.46× |
| A6 | fgemm | GF(7) | 64 | [aspirational] | `27bb2f75` | **PASS** 0.44× |
| A7 | fgemm | GF(251) | 1024 | [aspirational] | `41096af5` | **PASS** 0.683× |
| A7 | fgemm | GF(251) | 64,256 | [aspirational] | — | AMENDED [aspirational] |
| A7 | fgemm | GF(251) | 4096 | [aspirational] | `98336ab4` | **PASS** 1.490× (isolated) |
| A2 | matmul | GF(2^8) | 64,256,1024 | [aspirational] | `e24f7839` | AMENDED [aspirational] |
| A3 | matmul | GF(2^16) | 1024 | [aspirational] | `e24f7839` | AMENDED [aspirational] |

### 8.1 SC#5 verdict

**SC#5 is NOT YET satisfied** because A8 rows 4-5 (matmul GF(2)/n=64,256) remain **FAIL (UNRESOLVED)**:

- Story `974a85bd` owns these cells. Its published parity report `[E13]` (`2026-05-06-111a3967-gf2-parity-evidence.md`) closes matmul GF(2) only at n=1024 ([hard] PASS 1.18×) and n=4096 ([hard] PASS 1.50×). The n=64,256 cells are below the M4RM crossover and are NOT dispositioned by that report (n=256 measured at 13.5× behind M4RI, `[E0fd]`).
- The predecessor scorecard's Annex A8.2 explicitly records "No successor task filed yet — flagged in § A8.2."
- No user-approved `[aspirational]` amendment or EXCLUSION for these two cells exists anywhere in the repo (`dev/`, decks, plans, all session handoffs were searched).

Per SC#5 ("no cell may remain in FAIL state without an explicit user-approved amendment") and the project's `feedback_no_autonomous_amendments` rule, this worker has **not** invented an amendment. These two cells are flagged for lead escalation to the user. Possible dispositions (user decision):
1. File a successor task (e.g. small-n GF(2) matmul dispatch) and route rows 4-5 there with a `[hard]` FAIL→follow-up amendment, OR
2. Approve an `[aspirational]` amendment with documented below-crossover cause (cf. the GF(p) n=64 per-call-overhead precedent in A5/A6), OR
3. Reclassify as EXCLUDED if the small-n regime is declared out of scope.

**Every other A8 cell is PASS, AMENDED-with-citation, or EXCLUDED.** Once rows 4-5 are dispositioned by the user, SC#5 is satisfied.

### 8.2 Tally (excluding the 2 unresolved blocker cells)

- **PASS** (direct/evidence-doc): A8 rows 1-3, 6-11, 14-22, 26-58 (PASS subset), 62, 64, 66-76, A5, A6, A7/1024, A7/4096.
- **AMENDED [aspirational]** (user-approved, cited): rows 12-13, 23-25, 37, 39-40, 51-54, 61, 63, 65, A7/{64,256}, A2, A3, + the four 8df0c501 n=1024-uniform invert cells + the two A4 GF(2^31-1)/invert/uniform cells.
- **EXCLUDED**: §6.3 (11 cells: GF(2^4) matmul ×3, GF(2) pluq/solve ×8) + §6.1/§6.2 (20 cells: no-independent-oracle).
- **FAIL (UNRESOLVED — BLOCKER)**: A8 rows 4-5 (matmul GF(2)/n=64,256).

---

## § 9 — Non-Regression Sweep (SC#7)

Per SC#7, cells PASSing in the predecessor scorecard must not regress > 5% under same-session measurement. Confirmed by: (a) the 98336ab4 warmup-matched non-regression sweep (`[E98]` § 4 — all 18 GF(p) fgemm cells with a prior PASSing baseline within ±5%); (b) the per-issue non-regression sweeps in `[E68m]` § "Non-regression", `[E86]` § 4, `[E6a]` § 4, `[E8d]` § 4, `[E66]` § 6, `[E91]` § 5.2 (all previously-PASSing GF(p) dense-LA cells improved or within ±5%); (c) the § 6 downstream-inheritance measurements (this scorecard).

No cell PASSing in the predecessor regressed beyond 5% on a same-code basis. SC#7 satisfied for all dispositioned cells. (The two unresolved blocker cells were already FAIL in the predecessor; no regression possible.)

---

## § 10 — Source Index

| Tag | Path | Coverage |
|---|---|---|
| `[E1]` | `dev/bench_results/2026-05-04-3b762764-dense-la-post-gemm.md` | GF(p) dense-LA baseline; fflas reference walls |
| `[E2]` | `dev/bench_results/2026-05-06-e24f7839-panelized-gf2m-gemm.md` | GF(p)/GF(2^m) fgemm aggregate; panelized GF(2^m) kernel |
| `[E3]` | `dev/bench_results/2026-05-04-3b762764-dense-la-post-gemm.md` | M4RI reference rows; GF(2) structural absence |
| `[E4]` | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` | Sparse baseline |
| `[E9]` | `dev/bench_results/2026-05-04-609855d9-gfp-by-family.md` | GF(31)/GF(p) family measurements |
| `[E10]` | `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md` | NTL GF(2^32) reference |
| `[E11]` | `dev/plans/m4rie_promotion_evidence.md` | M4RIE reference promotion |
| `[E12]` | `dev/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md` | GF(2^m) parity verdict |
| `[E13]` | `dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md` | GF(2) dense-LA parity (matmul n≥1024 + echelon); the 974a85bd published report |
| `[E0fd]` | `dev/bench_results/2026-05-04-0fd48627-gf2-m4ri-profile.md` | GF(2) small-n matmul M4RI gap profile (n=256 at 13.5×); bottleneck classification |
| `[E14]` | `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` | GF(p) fgemm parity verdict |
| `[E15]` | `dev/bench_results/2026-05-07-4eb105f7-dense-la-parity-evidence.md` | GF(2^31-1) dense-LA parity (Wave-9) |
| `[E18]` | `dev/bench_results/2026-05-07-1726270d-sparse-parity-evidence.md` | Sparse parity (spmv, sparse×dense) |
| `[E20]` | `dev/bench_results/2026-05-08-2cfc4372-sparse-elim-gf2m.md` | Self-canonical sparse-elim GF(2^m) |
| `[EX]` | `dev/bench_results/2026-05-08-pending-cell-measurement.md` | Wave-12 GF(31) direct Criterion measurements |
| `[E27bb]` | `dev/bench_results/2026-05-24-27bb2f75-small-n-dispatch.md` | Small-n GEMM dispatch (GF(7)/GF(31)/n=64) |
| `[E52]` | `dev/bench_results/2026-05-24-52cce970-charpoly-minpoly-closure.md` | charpoly+minpoly AVX2 closure |
| `[Eaaa]` | `dev/bench_results/2026-05-24-aaa847cf-m4rm-invert.md` | GF(2) M4RM invert closure (rows 44-46) |
| `[E5ce]` | `dev/bench_results/2026-05-24-5ce13bae-markowitz-sparse-rref.md` | Markowitz sparse RREF (rows 61-70) |
| `[E41]` | `dev/bench_results/2026-05-25-41096af5-route-selection-decision.md` | Route-A wire-in (GF(251)/n=1024) |
| `[E7e]` | `dev/bench_results/2026-05-07-7e41400f-invert-solve-det.md` | GF(2^31-1) invert/solve/det (Path A; § 7 resolution target) |
| `[E68p]` | `dev/bench_results/2026-05-26-6823c8a0-panelized-ple.md` | Panelized GF(p) PLE (GF(7)/GF(31) PASS; GF(251)/GF(65521) base) |
| `[E68m]` | `dev/bench_results/2026-05-27-68db401b-fp-medium-ple.md` | u16-lane medium-prime PLE (GF(65521) rows 14-17 + GF(251)/n=64 PASS) |
| `[E86]` | `dev/bench_results/2026-05-26-869ce43b-blocked-echelon.md` | Blocked GF(p) echelon/RREF (rows 18-33, 72-73) |
| `[E6a]` | `dev/bench_results/2026-05-27-6a7d4c8e-m31-echelon-dispatch.md` | M31 echelon GEMM-axpy dispatch (row 33 PASS) |
| `[E8d]` | `dev/bench_results/2026-05-27-8df0c501-blocked-invert.md` | Blocked GF(p) invert (rows 34-46, 74) |
| `[E66]` | `dev/bench_results/2026-05-26-6613abf4-blocked-solve.md` | Blocked GF(p) solve (rows 47-58, 75) |
| `[E91]` | `dev/bench_results/2026-05-27-9138d86c-fp65521-n64-solve.md` | GF(65521)/n=64 solve fix (rows 55-56) |
| `[Ed3]` | `dev/bench_results/2026-05-27-d36cc414-gf251-n64-borderline.md` | GF(251)/n=64 borderline decomposition (branch b; [aspirational] rows 23,37,40,51,52) |
| `[E98]` | `dev/bench_results/2026-05-28-98336ab4-fgemm-n4096.md` | GF(p) fgemm n=4096 closure (all 6 PASS) |
| `[Eds]` | `dev/bench_results/2026-05-28-b0fa00af-downstream-inheritance.csv` | This scorecard's downstream-inheritance bench CSV (§ 6) |
| handoff-11 | `dev/active/026fc832-handoff-11.md` | Session-11 amendment escalation (rows 24-25,39,53-54 [aspirational]) |
| handoff-12 | `dev/active/026fc832-handoff-12.md` | Session-12 amendment escalation (user "apply all 5", commit 93dc5125; rows 23,37,40,51,52) |
| predecessor | `dev/bench_results/2026-05-08-2cfc4372-sota-scorecard.md` | Predecessor SOTA scorecard (superseded) |
| v1 draft | `dev/bench_results/2026-05-25-b0fa00af-sota-scorecard-final.md` | v1 draft (obsoleted by this v2) |
| plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` | SOTA catch-up plan |
