# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 15

**Date:** 2026-05-28
**Session number:** 15 (continuation of session 14; see also handoff-14)
**Prior handoffs:** sessions 1–14 in `dev/active/026fc832-handoff*.md`. Read every prior handoff's **Traps** section — all carry forward.

## TL;DR — the epic is ONE gate-pass away from done

All work is complete. The epic closes as soon as **`b0fa00af`'s `code-review` gate passes**, which is currently blocked **only by a transient Copilot reviewer API glitch** (`CAPIError: 400 Duplicate item found with id fc_call_...`). The reviewer's *content* returns APPROVE; the harness just can't extract the VERDICT token while the API is glitching. This is NOT a code/content problem and must NOT be bypassed.

**Next session, do exactly this:**
1. Re-run `jit gate pass b0fa00af code-review` (via JIT CLI/MCP). It will pass once the API has recovered — exactly as it did for `bdf60780` this session (cleared after ~3 attempts). **Retry at LOW frequency only** (see Traps — the user explicitly said "don't spam, wait it out", and the auto-mode classifier blocks rapid retry loops). One attempt every ~20–30 min, or just try once when you start and wait if it's still glitching.
2. When it passes: `jit issue update b0fa00af --state done`; commit `.jit` state.
3. **Epic close-out (Section 10)** — see the prepared SC mapping below. Run `jit gate check-all 026fc832`; pass the epic's gates (cargo-ci, code-review, doc-review — code-review may also hit the same transient glitch; low-frequency retry). Write the completion report (`references/completion-report-template.md`), `jit issue update 026fc832 --state done`, archive the progress file.

## Current state (verified at session-15 end)

- `0749dbad` (f64 GEMM cascade, Phase 6e) — **DONE** (commit `207f0403`). GF(65521)/n=4096 = 1.283×.
- `98336ab4` (n=4096 fgemm consolidated re-bench, Wave 14) — **DONE** (commit `79ac3a83`). All 6 primes PASS at n=4096; GF(251) = 1.490× isolated.
- `bdf60780` (matmul GF(2) small-n M4RI parity, Wave 16 — the successor filed this session) — **DONE** (commit `af493766`). **Real closure**: n=64 = 1.213×, n=256 = 1.070× vs canonical M4RI (from a true starting gap of 2.73×/2.54×), via AVX2 Gray-table builders + small-n k_block heuristic + lowered register-tile gate. No regression at n=1024/4096; proptests green; unsafe isolated; asm artefact regenerated. All 3 gates passed (code-review cleared after the same transient CAPIError glitch, ~3 attempts).
- `b0fa00af` (terminal scorecard, Wave 15) — **in_progress; gates: cargo-ci ✓, doc-review ✓, code-review ✗ (transient API only).** The v2 scorecard `dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md` is FINALIZED: **every A8 cell is PASS / AMENDED-with-citation / EXCLUDED — zero bare-FAIL; SC#5 SATISFIED.** rows 4-5 flipped to PASS citing bdf60780 (`[Ebdf]`). Finalization commit `2702def8`.
- Epic `026fc832` — `backlog`, transitively blocked only on `b0fa00af`. Closes next session once b0fa00af's code-review lands.

## Epic success-criteria mapping (prepared for the completion report)

- [hard] SC#1 — all five named follow-ups (`615db3b9`, `52cce970`, `27bb2f75`, `aaa847cf`, `5ce13bae`) done with scorecard cells PASS or user-approved amendment → **MET** (all done; dispositions in the v2 scorecard).
- [hard] SC#2 — updated scorecard supersedes `2026-05-08-2cfc4372` recording new measurements → **MET** (`2026-05-28-b0fa00af-sota-scorecard-final.md`).
- [hard] SC#3 — no regression on predecessor-PASSing cells (≤5%) → **MET** (per-issue non-regression + the scorecard's § 6 downstream-inheritance; all ≤5%).
- [hard] SC#4 — no unsafe leaks outside the two kernel crates → **MET** (held throughout; bdf60780's new SIMD is in gf2-kernels-simd, gf2-core stays `#![deny(unsafe_code)]`).
- [hard] SC#5 — bit-exact correctness across touched kernels (proptests at boundary lengths) → **MET** (proptests in 98336ab4, bdf60780, and the Phase-6 issues).
- [aspirational] SC#6 — the 11 EXCLUDED §6.3 cells re-enter as PASS → **PARTIALLY MET / aspirational**: the 8 GF(2) pluq/solve §6.3 cells and the 3 GF(2^4) matmul §6.3 cells remain **EXCLUDED** in the v2 scorecard (the GF(2^4) cells still need a `Gf2mWide<u4>` follow-up, never filed). Aspirational → does NOT block epic close. Note this honestly in the completion report (do not claim it as fully met).

## Traps — do not repeat these

**Carry forward** (link, don't copy): sessions 1–14 handoffs' Traps, especially handoff-14's (genuine 6s cargo-ci; isolated-vs-consolidated bench fairness; warmup-matched non-regression for SC#3; `974a85bd` never dispositioned matmul GF(2) n=64,256).

**New session-15 traps:**

- **Transient Copilot reviewer `CAPIError: Duplicate item found with id fc_call_...`.** During this session the `code-review` gate (`scripts/ai-review.sh`, Copilot gpt-5.x) intermittently failed at VERDICT-extraction because the model's function-call stream produced duplicate call IDs that the API rejected. Signature: `stderr` shows `ERROR: Could not extract VERDICT ... Treating as failure` + `CAPIError: 400 Duplicate item found`; the `stdout` often contains a complete APPROVE/PASS review. **This is transient infra, not a content failure.** It cleared for `bdf60780` after ~3 attempts but persisted ~30+ min for `b0fa00af`. **Handling rules:** (1) NEVER bypass or manually-attest the code-review gate to work around it (gate integrity is inviolable). (2) Re-run the gate via JIT CLI/MCP — it passes once the API recovers. (3) **Retry at LOW frequency only** — the user said "Lets not spam that. We must wait it out," and the auto-mode classifier *blocks* tight retry loops. A 150s-interval loop was denied. Use ~20–30 min spacing, or single manual attempts. Do not relaunch a rapid retry loop.

- **A 300s `status: error, exit_code: None` cargo-ci is a transport TIMEOUT, not a failure.** After SIMD-kernel changes (bdf60780), the full-workspace rebuild (debug check + release nextest + clippy --all-targets) exceeded the JIT gate's 300s ceiling from a cold-ish cache. Fix: run `./scripts/cargo-ci.sh` locally first to warm the cache (no 300s ceiling), then re-run `jit gate pass <id> cargo-ci` — it completes in ~6s on the warm cache. (Distinguish from the code-review CAPIError above, which is `status: Failed, exit_code: 1` with the CAPIError in stderr.)

## What is NOT needed next session

- No new implementation. No new benches. No scorecard edits (it's finalized). No amendments. Just land the b0fa00af code-review gate (wait out the API), close b0fa00af, close the epic, write the completion report.

## Reference artefacts

- Epic `jit issue show 026fc832`. Terminal deliverable `b0fa00af`. Final blocker (now DONE) `bdf60780`.
- v2 scorecard: `dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md` (all A8 cells dispositioned; SC#5 satisfied).
- bdf60780 evidence: `dev/bench_results/2026-05-28-bdf60780-matmul-gf2-smalln.md`.
- Completion report template: `.claude/skills/project-lead/references/completion-report-template.md`.
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512.

## Main HEAD at end of session 15

`2702def8` (finalize v2 scorecard — rows 4-5 PASS, SC#5 satisfied). After this handoff is committed, HEAD will be the handoff commit. No worker processes running; host quiet.

## Session 14–15 commit chain (high-impact)

- `207f0403`: close 0749dbad
- `b9127b2f`/`865383a1`/`5001465a`/`4f63dce9`: 98336ab4 worker (bench + proptest + isolated re-measure + warmup-matched non-reg)
- `79ac3a83`: close 98336ab4
- `bc6c9055`/`5941b133`: b0fa00af v2 scorecard + downstream CSV
- `b54fdd1c`: file bdf60780 successor + scorecard RESOLUTION note
- `714ac16a`: session-14 handoff
- `b718ba0b`/`8eff2223`: bdf60780 worker (kernel/dispatch + evidence)
- `af493766`: close bdf60780
- `2702def8`: finalize v2 scorecard (rows 4-5 PASS, SC#5 satisfied)

**Session 14–15 summary:** 3 issues closed (0749dbad, 98336ab4, bdf60780). Terminal scorecard published and finalized with every A8 cell PASS/AMENDED/EXCLUDED. The one escalation (matmul GF(2) n=64,256 bare-FAIL) was resolved by the user choosing real closure (option B) → `bdf60780`, which genuinely closed both cells to ≤1.5× M4RI. The epic is complete in substance; only `b0fa00af`'s code-review gate remains, blocked solely by a transient Copilot API glitch to be waited out at low frequency next session.
