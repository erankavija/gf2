# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 14

**Date:** 2026-05-28
**Session number:** 14
**Prior handoffs:** sessions 1–13 in `dev/active/026fc832-handoff*.md`. Read every prior handoff's **Traps** section — all carry forward.

## Current state

- Epic `026fc832` — state `backlog` (assignee `agent:project-lead`). Transitively blocked on `bdf60780` (see below). **Does NOT close this session.**
- The concurrent agent (epic `2928ccce`) that contended for cargo in session 13 has **settled** — its `session-1 state settled` commit (`12fe678f`) was main HEAD at session-14 start, and no cargo/nextest from it is running. Session 14 had a quiet host throughout.
- **Closed this session:**
  - `0749dbad` (f64 GEMM cascade, Phase 6e) — re-gated and closed. Both session-13 blockers were already fixed on main (bipedal3 n24 flake split into fast + `#[ignore="sim"]` slow; `bit_interleaver.rs` doctest fixed by the other agent at `0c10f0f6`). All 3 gates pass. GF(65521)/n=4096 = 1.283× PASS.
  - `98336ab4` (n=4096 fgemm consolidated re-bench, Wave 14) — closed after 1 rework round. **All 6 primes PASS at n=4096.** GF(251)/4096 = **1.490× (isolated)** — see Traps for the isolated-vs-consolidated methodology. SC#3 satisfied via a warmup-matched gf2-vs-41096af5-baseline delta table.
- **In flight:**
  - `b0fa00af` (terminal scorecard, Wave 15) — v2 scorecard PUBLISHED at `dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md` and attached. Every A8 cell is PASS / AMENDED-with-citation / EXCLUDED **except** matmul GF(2) n=64,256 (A8 rows 4-5). `b0fa00af` stays `in_progress` until `bdf60780` lands. (Commits `bc6c9055`, `5941b133`; lead resolution edits in `b54fdd1c`.)
  - `bdf60780` (NEW, filed this session) — "Close matmul GF(2) small-n (n=64, n=256) to M4RI parity". Dispatched as Wave 16 (opus, background, main checkout). See "The bdf60780 escalation" below.

## What happened this session

1. Re-gated + closed `0749dbad` (commit `207f0403`). Verified the 6.3s cargo-ci is a GENUINE pass (real rustup cargo, warm cache, ~3851-4025 tests executed in ~5s across 24 threads) — not a stub false-pass (issue 941d1528). See Traps.
2. Wave 14: dispatched `98336ab4` worker (sonnet, main). R0 returned 5 PASS + 1 SHORTFALL (GF(251)/4096 = 1.519× consolidated). Resolved via isolated re-measurement (1.490× PASS). code-review R0 FAILED on SC#3 (non-regression compared to fflas instead of to the gf2 41096af5 baseline; GF(7)/256 −37.4% cold-i-cache artifact). R1 fixed with a warmup-matched gf2-vs-baseline delta table (all ≤5%). Closed (commit `79ac3a83`). Worker commits `b9127b2f`, `865383a1`, `5001465a`, `4f63dce9`.
3. Wave 15: dispatched `b0fa00af` worker (opus, main). It built the v2 scorecard, dispositioned ~55 PASS + ~24 AMENDED (all cited) + 31 EXCLUDED, ran downstream-inheritance (no >5% regression), resolved the Path-A text in `2026-05-07-7e41400f-invert-solve-det.md` — and correctly REFUSED to amend the one genuine blocker: matmul GF(2) n=64,256.
4. **Escalated the blocker to the user.** User chose **option B: file a successor task, pursue real closure** (declined amend-to-aspirational and EXCLUDE). Filed `bdf60780`, wired it as a blocker of `b0fa00af` (transitively of the epic), updated the scorecard's RESOLUTION note + rows-4-5 routing to cite it. Committed `b54fdd1c`.
5. Dispatched `bdf60780` worker (Wave 16, opus, background).

## The bdf60780 escalation (the only open decision, now resolved)

- A8 rows 4-5 = matmul GF(2) at n=64 (1.79×) and n=256 (1.72×) were routed in Annex A8 to story `974a85bd`. But `974a85bd`'s parity report (`dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md`) only dispositioned the matmul **target rows** n=1024 (1.18× PASS) and n=4096 (1.50× PASS) — it never covered n=64,256 (below the M4RM crossover). The predecessor scorecard recorded "No successor task filed yet." No amendment/exclusion existed anywhere.
- `b0fa00af` SC#5 forbids any bare-FAIL cell, so this blocked epic close. **User picked real closure (option B).** `bdf60780` carries a `[hard]` ≤1.5× target.
- **NOTE the baseline ambiguity (tell the bdf60780 reviewer):** the scorecard's "1.79×/1.72×" may be against a non-canonical M4RI number. The canonical pinned reference (`2026-04-26-reference.csv`) is M4RI matmul GF(2) n=64 uniform = **3474 ns**, n=256 uniform = **29808 ns**. If current gf2 is ~9.5µs at n=64 (per old `[E13]`), the true gap vs canonical is ~2.7×, not 1.79× — i.e. potentially HARDER than the scorecard implies. `bdf60780`'s STEP-0 establishes the real gap. Targets: gf2 n=64 ≤ 5211 ns, n=256 ≤ 44712 ns.

## What to do next

1. **Review the `bdf60780` worker output** (Wave 16). Two possible outcomes:
   - **Closed PASS** (n=64 ≤ 5211 ns AND n=256 ≤ 44712 ns, no n=1024/4096 regression, proptests green): run gates (cargo-ci → code-review → doc-review), close `bdf60780`. Then **update the b0fa00af v2 scorecard rows 4-5 → PASS** (cite bdf60780's evidence doc), flip the § 8.1 SC#5 verdict to SATISFIED, re-run b0fa00af's gates, close `b0fa00af`. Then **epic close-out (Section 10)**: map all 6 epic SCs, run epic gates, completion report, `jit issue update 026fc832 --state done`, archive progress file.
   - **Infeasible** (worker reports best ratio + measured structural cause): do NOT amend yourself — **re-escalate to the user** with the evidence (offer: [aspirational]-amend rows 4-5 with the infeasibility evidence, or EXCLUDE, or keep grinding). Only after user approval, amend the scorecard accordingly, then proceed to close.
2. **Epic SC mapping for close-out** (Section 10): SC#1 (5 follow-ups done — all done), SC#2 (scorecard supersedes predecessor — b0fa00af v2), SC#3 (no regression — downstream-inheritance + per-issue non-reg all ≤5%), SC#4 (unsafe isolation — held), SC#5 (bit-exact — proptests), SC#6 [aspirational] (11 EXCLUDED §6.3 cells: GF(2) pluq/solve owned by aaa847cf, GF(2^4) matmul ×3 need a `Gf2mWide<u4>` follow-up — aspirational, may remain partially unmet at close).

## Traps — do not repeat these

**Carry forward** (link, don't copy): sessions 1–13 handoffs' Traps. All still in force — notably [[shell-cwd-persistence]], [[merge-marker-edit-careful]], `git add -A` cross-agent sweeps, parallel cargo-ci contention.

**New session 14 traps:**

- **A 6-second cargo-ci gate is GENUINE here, not a stub false-pass.** `scripts/cargo-ci.sh` runs `cargo check` (builds everything) then `cargo nextest` (reuses the just-built artifacts to EXECUTE ~4025 fast tests in ~5s across 24 threads) then clippy + fmt. With a warm target/ the whole thing is ~6s. `cargo` is the real rustup proxy (verified `file $(which cargo)` → symlink to rustup; `cargo --version` → 1.95.0). Don't panic at the fast duration. BUT do sanity-check `which cargo`/`file` if ever suspicious (issue 941d1528 stub guard). Also: alternating `cargo clippy --all-targets` and `cargo nextest`/`cargo test` THRASHES the cache (clippy's rustc-wrapper produces different artifact fingerprints), forcing a full test rebuild on the next `cargo test` — don't be alarmed by a sudden multi-minute rebuild after a clippy run.

- **Bench fairness: isolated vs consolidated, and warmup-matched non-regression.** Two distinct measurement traps bit Wave 14:
  1. **L3-contention in multi-prime sweeps.** GF(251)/n=4096 uses Route A, whose L3-budget heuristic (tuned by 74ba1cdc to ~16MB) is violated when 5 prior n=4096 GEMMs run first in the same criterion process → reads 1.519× (SHORTFALL). The fflas reference is itself isolated (one config/process), so the fair apples-to-apples comparison is gf2-ISOLATED (filter `gemm/Fp_251/Fp_251/4096$` alone) vs fflas-isolated → 1.490× PASS. **When a SOTA cell is borderline, measure it isolated to match how the reference was measured.**
  2. **i-cache warmup in non-regression.** SC#3 means gf2-now vs the gf2 41096af5 baseline (NOT gf2-vs-fflas). The 41096af5 baseline filter was `(64|256|1024)` so GF(7)/64 warmed the Candidate-C i-cache before GF(7)/256. A filter starting at n=256 leaves GF(7)/256 cold → −37% artifact. **Re-measure non-regression with a filter matching the baseline's warmup ordering (include n=64).** The reviewer enforces the literal SC#3 comparison basis; satisfy it, don't argue it.

- **`974a85bd` did NOT disposition matmul GF(2) n=64,256.** Its parity report covers only the matmul target rows n=1024/4096 + echelon n=64..1024. The epic description's claim that "974a85bd's closure documentation owns rows 4-5" is OPTIMISTIC — those cells were never actually dispositioned there. This is the bdf60780 blocker's root. Don't assume an A8 "[→issue]" routing means the cell was actually closed by that issue — verify in the issue's evidence doc.

- **No autonomous amendments held the line.** The b0fa00af worker correctly refused to amend rows 4-5 and surfaced them; the lead escalated rather than self-authorising. Keep this discipline: a [hard] FAIL with no prior user-approved amendment is an escalation, never a lead-side amend.

## Open questions needing user input

**None currently open** — the matmul-GF(2)-smalln escalation was answered (option B: file `bdf60780` + pursue closure). The NEXT possible escalation is only if `bdf60780` proves infeasible (then re-escalate with evidence per "What to do next" item 1).

## Reference artefacts

- Epic: `jit issue show 026fc832`. Open blocker: `bdf60780`. Scorecard deliverable: `b0fa00af`.
- v2 scorecard: `dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md` (supersedes 2026-05-08 predecessor; obsoletes the 2026-05-25 v1 draft).
- Wave 14 evidence: `dev/bench_results/2026-05-28-98336ab4-fgemm-n4096.md` (+ isolated re-measure + warmup-matched non-reg CSVs).
- 0749dbad evidence: `dev/bench_results/2026-05-27-0749dbad-fp-medium-f64-cascade.md`.
- M4RI canonical reference: `dev/bench_results/2026-04-26-reference.csv` (rows `m4ri,matmul,GF(2),...`).
- 974a85bd parity report: `dev/bench_results/2026-05-06-111a3967-gf2-parity-evidence.md`.
- Worktree dispatch protocol: `.claude/skills/project-lead/references/worktree-dispatch-protocol.md`.
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512.

## Dispatch mode note (session 14)

All session-14 workers were dispatched on the **main checkout directly** (no worktrees), because each wave was a SINGLE sequential issue with no parallelism and no concurrent gf2 agent — so worktree isolation was unnecessary and main's warm build cache benefited the bench tasks. Workers were instructed: no `.jit/`, no `jit` commands, no worktrees, `git add <paths>` only (never `-A`), --release, quiet host for benches. Leak risk was checked via `git status` after each (all clean). If a FUTURE session needs PARALLEL dispatch, revert to the worktree-dispatch protocol.

## Main HEAD at end of session 14

`b54fdd1c` (file bdf60780 successor + scorecard resolution). bdf60780 Wave-16 worker running in background on main.

## Session 14 commit chain (high-impact)

- `207f0403`: close 0749dbad (f64 cascade gates pass)
- `b9127b2f`, `865383a1`, `5001465a`, `4f63dce9`: 98336ab4 worker (bench harness + proptest + consolidated bench + isolated GF(251) re-measure + warmup-matched non-reg)
- `79ac3a83`: close 98336ab4 (all 6 n=4096 PASS)
- `bc6c9055`, `5941b133`: b0fa00af v2 scorecard + downstream-inheritance CSV
- `b54fdd1c`: file bdf60780 successor + scorecard RESOLUTION note

**Session 14 summary:** 2 issues closed (0749dbad, 98336ab4). v2 terminal scorecard published — all A8 cells dispositioned PASS/AMENDED/EXCLUDED except matmul GF(2) n=64,256, which the user chose to close via new task `bdf60780` (dispatched). Epic is one task away from done; it will close once `bdf60780` lands (or after a user re-escalation if `bdf60780` proves infeasible).
