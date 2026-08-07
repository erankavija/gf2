# Handoff — Continue gf2-core SOTA catch-up (026fc832) — session 16

**Date:** 2026-05-31
**Session number:** 16 (continuation of session 15; see handoff-15)
**Prior handoffs:** sessions 1–15 in `dev/active/026fc832-handoff*.md`. Read every prior handoff's **Traps** section — all carry forward.

## TL;DR — epic is 15/15 children done; closing is blocked ONLY by an external DVB-T2 test regression on shared `main`

This session landed the last remaining child (`b0fa00af`) to `done`. **All 15 children are now done.** The epic itself (`026fc832`) auto-advanced `backlog → ready`. The epic's own `cargo-ci` gate is the sole remaining blocker to `jit issue update 026fc832 --state done`, and it fails for a reason **outside this epic's scope**: two DVB-T2 tests (the other project lead's domain) time out under full-suite parallel contention. Per user decision (session-16 escalation), we **defer to the DVB-T2 lead** to fix their tests; do NOT edit their files and do NOT bypass the gate.

## What session 16 did

1. **Closed `b0fa00af`** (terminal SOTA scorecard, the epic's SC#2 deliverable):
   - The `code-review` gate's transient Copilot `CAPIError` glitch (handoff-15's trap) had cleared. On re-run the reviewer extracted a real VERDICT and surfaced **one substantive finding (SC#6)**: the v2 scorecard's JIT doc **attachment label** was stale — it still read "…except rows 4-5 (→bdf60780)", contradicting the finalized scorecard where rows 4-5 are PASS and SC#5 is satisfied.
   - **Fix applied:** relabeled the v2 attachment (`dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md`) to the final state, and relabeled the interim `2026-05-25-...-final.md` attachment as "SUPERSEDED by v2" (one-pass sweep to avoid a second one-finding-at-a-time round). No scorecard *content* changed — labels only.
   - Re-ran `code-review` → **PASS**. `cargo-ci` was already PASS (at commit `2702def8`). `jit issue update b0fa00af --state done`. Committed as `edbcf718`.

2. **Epic close-out attempt (Section 10):**
   - `jit graph deps 026fc832` → **15/15 complete.**
   - `jit gate pass 026fc832 cargo-ci` → **FAIL.** Two tests TIMEOUT:
     - `gf2-coding::ldpc_systematic_encoding dvb_t2_encoding_tests::test_dvb_t2_normal_all_rates`
     - `gf2-coding::ldpc_systematic_encoding dvb_t2_encoding_tests::test_dvb_t2_short_all_rates`

## The blocker, characterized precisely (do not misdiagnose next session)

- These two tests live in `crates/gf2-coding/tests/ldpc_systematic_encoding.rs` (lines ~198 and ~225) and are **plain `#[test]` with no `#[ignore]` marker**.
- **In isolation they PASS** at **3.98s / 4.02s** (`cargo nextest run -p gf2-coding --release --profile ci -E 'test(test_dvb_t2_normal_all_rates) + test(test_dvb_t2_short_all_rates)'`). Under the **full 4079-test fast-tier suite** they get CPU-starved by parallel contention and tip to **5.04s → nextest 5s hard-kill TIMEOUT**.
- Root cause is the **DVB-T2 lead's** commit `599b868b` ("retire stale RU-preprocessing notes, **lift slow markers**"), which removed the `#[ignore = "slow"]` markers from these tests betting the new linear-time IRA encoder (`cb4e3a08`, jit:82dd7384) made them safely fast. They are borderline (~4s isolated) and break the shared fast-tier gate under load.
- The fast tier was **green at this epic's terminal commit `2702def8`** (handoff-15); the regression entered with the intervening DVB-T2 commits (`cb4e3a08`, `599b868b`, `52207ce9`), all `jit:82dd7384`.

## User decision (session-16 escalation) — DEFER TO DVB-T2 LEAD

Escalated via `AskUserQuestion`. User chose **"Defer to DVB-T2 lead"**: wait for the other project lead to fix their two timing-out tests (re-add `#[ignore = "slow:..."]` or further optimize). **No cross-domain edits. No gate bypass.** Once `main`'s fast tier is green again, the next session re-runs the epic gate and closes.

## Next session, do exactly this

1. Re-run the fast tier to check whether the DVB-T2 lead has fixed it:
   `cargo nextest run --workspace --all-features --release --profile ci` — look for the two `dvb_t2_encoding_tests` timeouts.
2. **If green:** `jit gate pass 026fc832 cargo-ci` (should pass), then `jit gate pass 026fc832 code-review` (may hit the same transient Copilot `CAPIError` — low-frequency retry per handoff-15 trap; it cleared this session). Then Section 10 close-out: write the completion report (`references/completion-report-template.md`; SC mapping is prepared in handoff-15 §"Epic success-criteria mapping" — it remains accurate), `jit issue update 026fc832 --state done`, archive the progress file.
3. **If still red:** the DVB-T2 lead hasn't fixed it yet. Do NOT edit `crates/gf2-coding/tests/ldpc_systematic_encoding.rs` (it's their domain and reverting `599b868b` collides with their active work). Re-confirm with the user only if it persists across multiple sessions.

## Epic success-criteria mapping (still accurate — prepared for the completion report)

See handoff-15 §"Epic success-criteria mapping". Summary: SC#1–SC#5 **MET**; SC#6 (aspirational, the 11 EXCLUDED §6.3 cells) **PARTIALLY MET** (8 GF(2) + 3 GF(2^4) cells remain EXCLUDED; the GF(2^4) cells still need a `Gf2mWide<u4>` follow-up that was never filed — aspirational, does not block close; report honestly).

## Traps — do not repeat these

**Carry forward** (link, don't copy): sessions 1–15 handoffs' Traps. Especially handoff-15's (transient Copilot `CAPIError` on code-review = infra not content, low-frequency retry, never bypass; 300s cargo-ci `status:error exit:None` = transport timeout, warm cache locally first).

**New session-16 traps:**

- **The epic `cargo-ci` failure is NOT this epic's bug.** It is the DVB-T2 lead's two `dvb_t2_encoding_tests` (`test_dvb_t2_normal_all_rates`, `test_dvb_t2_short_all_rates`) timing out under full-suite contention (~4s isolated, >5s under load) after their commit `599b868b` lifted the slow markers. Do not start debugging gf2-core/SOTA kernels chasing this — it is `jit:82dd7384` territory. Do not re-add the ignore markers yourself (user said defer; reverting their deliberate decision collides with their active work).
- **Shared `.jit/` state with the DVB-T2 lead.** The working tree carries the DVB-T2 lead's uncommitted `.jit/issues/82dd7384-…json` and their appends to the shared append-only `.jit/events.jsonl`. This session committed ONLY my own issue files (`b0fa00af`, `026fc832`) plus the shared `events.jsonl`; `82dd7384.json` was left untouched for them to commit. Next session: stage your specific `.jit/issues/<my-id>.json` files explicitly; never `git add .jit/issues/82dd7384*.json`.
- **`jit validate` reports an isolated issue `a7b1bb21` (DVB-T2 §6.1.4 demuxer).** That is the DVB-T2 lead's issue, not ours — ignore it; do not wire or delete it.

## Reference artefacts

- Epic `jit issue show 026fc832` (now `ready`, 15/15 children done). Terminal deliverable `b0fa00af` (`done`, `edbcf718`).
- v2 scorecard: `dev/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md` (all A8 cells PASS/AMENDED/EXCLUDED; SC#5 satisfied; attachment label now correct).
- Completion report template: `.claude/skills/project-lead/references/completion-report-template.md`.
- Reference host: AMD Ryzen 9 5900X (Zen 3), AVX2+FMA, no AVX-512.

## Main HEAD at end of session 16

`edbcf718` (chore(jit:b0fa00af): close terminal scorecard; fix stale v2 attachment label). After this handoff is committed, HEAD will be the handoff commit. No worker processes running.
