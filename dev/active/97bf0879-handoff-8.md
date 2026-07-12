# Handoff — Close gf2-core SOTA performance gaps (`97bf0879`) — session 8

**Date:** 2026-05-05 (session 8 — Wave 4 + Wave 5 + Wave 6A closure; GF(p^n) follow-up epic placeholder filed)
**Session number:** 8
**Prior handoffs:**
- `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7}.md` (sessions 1–7).
- Predecessor PPC epic: `dev/active/babcf05e-handoff{,-2,-3,-4,-5}.md`.

All prior-session traps remain in force unless explicitly resolved here.

## Current state

- Epic: `97bf0879` — state: **in_progress**, claimed by `agent:project-lead`.
- Children summary (epic-level): 4c0d0202 done; b0434149 done; cbecfced done; 5cacaec5 done. Story-level closures: cbecfced + b0434149.
- Wave 1: closed (5/5). Wave 2: closed (4/4). Wave 3 + impl follow-ups: closed. Wave 4 (`4c0d0202` SOTA target matrix): closed (R2 PASS). Wave 5 (story closures `cbecfced` + `b0434149`): closed. **Wave 6A (`5cacaec5` design): closed R4 PASS** with user-approved `[hard]→[aspirational]` amendment on the GF(251) row.
- Wave 6B-ready: `662f7a15`, `9e12659b`, `3d06224c` — three impl tasks in `backlog`/`ready` blocked only on `5cacaec5` (now done). All three depend solely on `5cacaec5`; **they can run in parallel via worktrees**.
- Active claims: `agent:project-lead` on `97bf0879`. Worktree branch `worktree-agent-5cacaec5` torn down. No live worker claims.
- Open escalations: none.
- Follow-up placeholder epic filed: **`47c1538f` (Close gf2-core GF(p^n) extension-field SOTA gaps)** — successor to `97bf0879`, depends on `97bf0879` closing first. Not yet broken down. User-flagged scope-loss surfaced during 4c0d0202 review.

## What just happened (session 8)

### Wave 4 (4c0d0202) — closed R2 PASS
- Architect dispatched in worktree (`a6e905632dc316752`); produced `dev/plans/sota_target_matrix.md` (476 lines, 10 sections, 11 cell tables) with cells over GF(2), GF(p) per family (GF(7)/GF(31)/GF(251)/GF(65521)/Mersenne31), GF(2^m) for m∈{4,8,16,32}.
- R1 fail: 3 findings — (a) doc-attach `commit: null`, (b) GF(31) absent from `analyze.py` `FIELD_ORDER`/`FIELD_FAMILY`, (c) `charpoly × GF(2)` / `minpoly × GF(2)` marked `N/A` not `EXCLUDED:<class>`.
- R1 fix commit `47e4525`: matrix doc § 5.6/5.7 use `EXCLUDED:no-independent-oracle`, exclusion ledger 18→20 cells, § 9.3 #4 inconsistency note added; `analyze.py` extended; doc-attach re-pinned to `47e4525` for all three issues (`4c0d0202`, `cbecfced`, `97bf0879`).
- R2 PASS at commit `cd3a93d`. Closed at `8492315`.

### Wave 5 — story closures
- **`b0434149`** (post-PPC SOTA baseline scorecard story) — R1 fail: 3 token-vocab inconsistencies. R1 fix `6c2897d`: `slow/nightly`→`slow-or-nightly` (7 sites), `within 10×`→`optimization gap`, all parenthetical-qualified `measured (...)` stripped to bare 5-token vocab; `ntl_gf2pow32_smoke` added to `.gitignore`; corrected scorecard wired to legacy issues `64c88ae4` + `a9ab0a4f`. R2 PASS. Closed at `6f0e286`.
- **`cbecfced`** (reference-matrix story) — R1 fail: stale "LinBox primary for `minpoly × GF(p)`" claim in `linbox_promotion_evidence.md`. R1 fix `6106034`: amendment block added. R2 fail: matching § 4.6 in `sota_target_matrix.md` was missed (surgical-edit drift, session-7 trap #1). R2 fix `901d884`: § 4.6 LinBox profile demoted minpoly to "Secondary for". R3 PASS. Closed at `af7fd97`.

### GF(p^n) follow-up epic — `47c1538f` filed
- User flagged GF(p^n) (odd p, n>1) scope-loss during 4c0d0202 review. Codebase has partial GF(p^n) infrastructure (`gfpn/ExtConfig`, `QuadraticExt`, `CubicExt`, `BatchExtField`); max degree 3.
- `47c1538f` placeholder epic created with 9 [hard] criteria sketched, three implementation work-streams identified (algorithmic / reference-lane / matrix-amendment). DAG edge `47c1538f → 97bf0879`. Commit `88448b8`. Not broken down — `/jit-breakdown 47c1538f` runs after `97bf0879` closes.

### Wave 6A (5cacaec5) — closed R4 PASS, with criterion amendment
- Architect dispatched in worktree (`a87fd6531ec06302a`); produced `dev/plans/small_prime_kernel_strategy.md` (407→411 lines, 10 sections). Selects **Candidate C — AVX2 16-bit-lane SIMD with Barrett reduction** for GF(7), GF(31), GF(251).
- R1 fail / R2 fail / R3 fail / **R4 PASS**. Total 4 review cycles. The blocker: GF(251) downstream 1.5× target (85.3 Gop/s) exceeds AVX2-MAC peak on Zen-3 host (80 Gop/s). fflas hits 128 Gop/s only via OpenBLAS sgemm (Candidate D, rejected for pure-Rust posture).
- **User AskUserQuestion / Path A approved:** amend the GF(251) `[hard]` 1.5× target to `[aspirational]` upfront, before `662f7a15` dispatch. `[aspirational]` permitted per `CLAUDE.md` § *Success-criterion maturity markers* — empirical evidence (80 Gop/s peak vs. 85.3 Gop/s 1.5× absolute) is the falsifier.
- R2 fix `46b603d`: amended `662f7a15` description. R3 review found the parent `5cacaec5` criterion text itself still required amendment.
- R3 fix `af5c8a6`: amended `5cacaec5` description's criterion #2 to allow per-prime maturity-marker scoping; added "Amendment — 2026-05-05 (user-approved, Path A)" block in the description (mirrors Wave-3 `9a715d75`/`a3412e15` amendment-log style). R4 review found § 7 step 7 of the design doc still hardcoded the OLD pre-amendment hard-stop rule (escalate immediately if GF(251) < 85.3 Gop/s).
- R4 fix `43393aa`: § 7 step 7 reflects the amended per-prime acceptance rule (GF(7)/GF(31) [hard] 1.5× threshold; GF(251) [aspirational] re-escalates only at ~3.2× soft threshold ≈ 40 Gop/s). R4 PASS at `43393aa`. Closed at `adb833b`.

### Concrete artefact landings (session 8)
- `dev/plans/sota_target_matrix.md` (476 lines) — keystone consumer matrix. 11 cell tables, 20-cell exclusion ledger, per-story consumption guide.
- `dev/plans/small_prime_kernel_strategy.md` (411 lines) — Wave 6A selection: Candidate C SIMD-lane AVX2 byte/word kernel.
- `benchmarks/analyze.py` extended: `FIELD_ORDER` + `FIELD_FAMILY` now include GF(31).
- `dev/plans/linbox_promotion_evidence.md` § *Cells where LinBox is the primary reference* gained an amendment block — minpoly now secondary, fflas-ffpack canonical.
- `dev/bench_results/2026-04-30-post-ppc-delta-appendix.md` token vocabulary normalized to the 5-token SSOT.
- `.gitignore`: `benchmarks/reference/ntl_gf2pow32_smoke` added.
- `47c1538f` placeholder epic filed.

### Session-8 commits on main
`47e4525` → `cd3a93d` → `8492315` (Wave 4 4c0d0202 R1+close)
`6c2897d` → `6f0e286` (b0434149 Wave 5)
`6106034` → `901d884` → `af7fd97` (cbecfced Wave 5)
`88448b8` (47c1538f GF(p^n) placeholder epic)
`cd00161` → `bb0fcdb` → `a577eb1` → `46b603d` → `af5c8a6` → `43393aa` → `adb833b` (5cacaec5 Wave 6A — 4 review cycles)

## What to do next

In priority order:

- [ ] **Wave 6B dispatch — `662f7a15`, `9e12659b`, `3d06224c` in parallel.** All three depend only on `5cacaec5` (done). Use `scripts/dispatch-worker-worktree.sh 662f7a15 9e12659b 3d06224c`. The three issues touch overlapping crates (`crates/gf2-core/src/gfp/`, `crates/gf2-kernels-simd/src/x86/`); **expect cherry-pick conflicts on `gfp/simd_ops.rs` and `gf2-kernels-simd/src/x86/mod.rs`**. Plan: dispatch all three in parallel, integrate by cherry-pick in the order least-overlap-first (probably `3d06224c` first since it only adds regression tests; then `9e12659b`; then `662f7a15` last). Run `scripts/check-leak-into-main.sh` after each integration.
  - `662f7a15`: implements the design from `dev/plans/small_prime_kernel_strategy.md`. New files at `crates/gf2-kernels-simd/src/{fp_small.rs, x86/fp_small.rs}` plus `crates/gf2-core/src/lib.rs::simd::maybe_fp_small`. Estimated 520 LOC, 8 files. **GF(251) is `[aspirational]`** per the amended criterion — worker must implement the kernel for all three primes but record GF(251) numbers without escalating at the 1.5× absolute target; only re-escalate at ~3.2× soft threshold.
  - `9e12659b`: medium-prime panelized GEMM (GF(65521) lane). Different lane width (16-bit vs 8-bit) per design § 8.2 #3.
  - `3d06224c`: Mersenne fast-path regression guard. The new `if P <= 251` dispatch branch in `662f7a15` cannot reach Mersenne31, but the regression check is mechanical.

- [ ] **Wave 6C — `7a106fe4`** (Publish GF(p) parity evidence). Documentation issue; runs after 6B closes. Records the measured per-prime ratios from the benchmark sweep `662f7a15` step 7 will produce.

- [ ] **Wave 7+ planning.** Re-read `97bf0879-progress.json` for the wave 7-12 topology. The progress file's session-1 wave plan is the source of truth for ordering.

- [ ] **Final report (`01ae4c20`).** Backlog; 0/4 story deps complete (deps are `8f3fdc34`, `54fd3f0b`, `66190ccd`, `72ab6d0e` — story-level closures of waves 7-11). Don't dispatch yet.

- [ ] **Update `97bf0879-progress.json`** with session-8 closures, wave-6A done, wave-6B/6C ready.

## Traps — do not repeat these

Carry-forward (still in force):
- All traps from `97bf0879-handoff{,-2,-3,-4,-5,-6,-7}.md` and `babcf05e-handoff{,-2,-3,-4,-5}.md`. Re-read on session resume.

New traps from session 8:

1. **`code-review` gate runs sometimes timeout at 10 min before completing.** Observed once on `5cacaec5` R1 — the MCP `jit_gate_pass code-review` call returned `TIMEOUT` after 600000ms, but `jit_gate_check-all` showed the gate had *not yet run*. Re-running `jit_gate_pass` immediately afterwards completed cleanly (~5 min). Lesson: if `jit_gate_pass code-review` times out, run `jit gate check-all <id>` to inspect — if `not_run` lists the gate, just retry the `jit_gate_pass` call.

2. **`[hard]→[aspirational]` amendments must be applied to BOTH the parent design issue AND the downstream consumer issue when the criterion text on the parent is what's being amended.** Wave 6A R3 surfaced this: amending `662f7a15`'s GF(251) row was insufficient because `5cacaec5`'s OWN criterion #2 ("One strategy is selected with feasibility evidence") still implicitly required GF(251) parity. The fix was to ALSO amend `5cacaec5`'s description to add per-prime maturity-marker scoping. Lesson: when amending a `[hard]` criterion that names a downstream artefact, audit the parent issue's criterion text too — a feasibility-evidence-style criterion implicitly inherits the downstream contract.

3. **Surgical-edit drift on amendment landings.** Wave 6A R3→R4 lost a cycle because the criterion-level amendment (in `5cacaec5`/`662f7a15` JIT descriptions) was not propagated to § 7 step 7 of the design doc, which still encoded the OLD pre-amendment hard-stop rule. Lesson: when a [hard]→[aspirational] amendment lands, grep the linked design doc for **every** mention of the amended target value (here: `85.3`, `1.5×`, the now-deprecated stop conditions) and reconcile each match. Same trap as session-7 #1.

4. **Reviewer agent is strict on "feasibility evidence" — it reads "feasibility for the downstream contract" not "feasibility for implementation."** Wave 6A R1/R2 lost two cycles to my attempt to read "feasibility evidence" narrowly (architectural-implementation-feasibility) when the reviewer was reading it broadly (downstream-throughput-target-feasibility). Lesson: when an issue text has a feasibility-evidence criterion, the reviewer reads it as evidence the strategy will hit the downstream `[hard]` targets, not just evidence the strategy is implementable. Don't argue this distinction; either supply the evidence or amend the criterion via user escalation.

5. **GF(p^n) (p odd, n>1) is a deliberate scope-loss in `97bf0879`.** Codebase has partial infrastructure (`gfpn/`, max degree 3) but the SOTA target matrix `dev/plans/sota_target_matrix.md` § 3.2 explicitly excludes GF(p^n). Successor epic `47c1538f` is filed as a placeholder for follow-up. **Do not** re-amend `97bf0879` to include GF(p^n) — that's `47c1538f`'s scope.

## Open questions needing user input

None blocking. The user already approved Path A (the 5cacaec5/662f7a15 GF(251) amendment) during this session.

## Reference artefacts

- This handoff: `dev/active/97bf0879-handoff-8.md`
- Progress file: `dev/active/97bf0879-progress.json` (next-session lead updates with session-8 closures)
- Predecessor handoffs: `dev/active/97bf0879-handoff{,-2,-3,-4,-5,-6,-7}.md`
- Wave 4 design doc: `dev/plans/sota_target_matrix.md` (commit `47e4525`)
- Wave 6A design doc: `dev/plans/small_prime_kernel_strategy.md` (commit `cd00161`, with R1-R4 fixes layered through `43393aa`)
- GF(p^n) placeholder epic: `jit issue show 47c1538f`
- Wave 6B issues: `662f7a15`, `9e12659b`, `3d06224c` — all `backlog` or `ready`, deps satisfied
- Wave 6C: `7a106fe4` — `backlog`, depends on Wave 6B closure
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
- Lead review protocol: `~/.claude/skills/project-lead/references/lead-review-protocol.md`
- Project conventions: `/home/vkaskivuo/Projects/gf2/CLAUDE.md`
- JIT events log: `.jit/events.jsonl` (append-only)
- Gate definitions: `.jit/gates.toml`
