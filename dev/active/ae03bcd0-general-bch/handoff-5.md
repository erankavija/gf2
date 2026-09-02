# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 8

**Date:** 2026-09-02T12:00+00:00 (interim; rewritten at session end if the session survives)
**Session number:** 8 (sessions 6 and 7 died at rate limits with no handoff; this one writes early)
**Prior handoffs:** handoff.md, handoff-2.md, handoff-3.md, handoff-4.md — their Traps sections remain in force in full.

## Current state

- Epic `ae03bcd0` — state: backlog (container); claimed by agent:jit-execution-lead.
- Wave in progress: wave 10 of 15 (`current_wave` = 10), plus two wave-11 early starts.
- Children: 40 done before this session; wave 10 open: `2b6968d3` (rework 1 in flight), `444c06bc` + `bd0edfa2` (rework 1 in flight, one worker), `3f7edef1` (rework 1 in flight), `d7749931` (fresh dispatch in flight); wave 11 early: `88ca7d2f` (in flight), `203ee826` (created this session, backlog, waits on 444c06bc).
- Active claims: epic, 2b6968d3, 444c06bc, bd0edfa2, 3f7edef1 by agent:jit-execution-lead; d7749931 and 88ca7d2f by agent:worker.
- Open escalations: none pending — two were resolved this session (R-39 Shortened fast path; B4 retry-then-amend), see below.
- Progress file: `progress.json` here (current through the B4 escalation).

## What just happened

- Reconstructed session 7's end from git log, gate records, and the eight still-listed idle teammates; committed its orphaned gate records (28fbb090). Three idle session-7 workers never answered status requests and were stopped; fresh Opus workers resumed from their worktrees.
- `444c06bc`/`bd0edfa2`: cargo-ci failure root-caused to `t2s_mother_row_matches_the_oracle_generator` at 7.85 s isolated vs the 8 s kill (R-38); code-review R1 (F1 contract cells, F2 field-generic coverage, F3 receipt) and doc-review R1 (bench rustdoc narration) → rework 1 to the live `worker-bd0edfa2-2` with ruling R-37 (dated bd0edfa2 amendment of the survey contract § 9: DVB-T2 W2 rows at mother length for this consumer only). Its draft receipt (window 11:27–11:33Z) shows the canonical path 10×–3000× over the reference.
- `3f7edef1`: merged fd8bc3d9 (lead resolved a `test_support.rs` import conflict, widened the module summary); code-review R1 FAIL (3), doc-review R1 FAIL (docs unlinked — lead's job post-rework), research-review R1 FAIL (5). Citekeys `SageMath2026`/`GapGroup2026`/`Joyner2026` registered lead-direct (aa56c3cb). Rework 1 → `worker-3f7edef1-2` (prompt files in the session scratchpad; contents summarized in progress.json). ETSI DVB-T2 streams exist on this host at `~/dvb_test_vectors/VV001-CR35_CSP` (TP04/TP05).
- `2b6968d3`: rework 1 (R-36 `clmul-fold` family) resumed by `worker-2b6968d3-3` from the session-7 draft (commit 7d545c26 + uncommitted gf2-coding edits).
- `d7749931`: fresh Opus `worker-d7749931-2` with the O-3 spec and the R-27 anchor test.
- `88ca7d2f`: Sonnet `worker-88ca7d2f` (runner committed de2202d0; measurement pending; told to take the CCX1 lock instead of waiting for load < 1).
- **Escalation 1 (resolved, R-39):** generic `Shortened<C>` builds a dense generator by nullspace + RREF and encodes by dense product → unusable for the DVB-T2 rows; owner chose option A: new task `203ee826` (systematic fast path in the wrapper) before `97410c80`. Deps wired: 203ee826 → 8aa98a25, 444c06bc; 97410c80 → 203ee826.
- **Escalation 2 (resolved):** GUAVA `BCHCode` on B4 needs an uncompressed 65343×65535 matrix (~34 GB; 40 GB probe reached 25.7 GB and was stopped). Owner: retry on the idle host with a ~56 GB heap bounded to 20 min; on failure amend the plan's evidence-protocol so B4's GUAVA result is the derivation plus polynomial map with the observed failure recorded.
- Codex weekly quota: 93% → 94% across four gate runs (resets Mon 2026-09-07 17:56 EEST). Gate runs are cheap (~0.25%/run); Luna workers are not — native workers only until the reset.

## What to do next

- [ ] Collect the five worker reports (they arrive as messages/notifications). Review each per `lead-review-protocol.md` (Tier 1.5 tables for 2b6968d3, bd0edfa2/444c06bc, 3f7edef1), merge one branch at a time with the leak check, run gates on an idle-enough host (cargo-ci is load-sensitive: five gf2-algebra tests time out under load).
- [ ] `3f7edef1` B4: when the host is idle, run `systemd-run --user --scope -p MemoryMax=52G timeout 1200 gap -q -A -o 50g -c 'LoadPackage("guava");; C:=BCHCode(65535,1,25,GF(2));; Print(Dimension(C),"\n"); QUIT;'` and send the outcome (with peak RSS) to `worker-3f7edef1-2`; then link `oracle-provenance.md` and `oracle-receipt.md` to 3f7edef1, refresh the plan.md link on the epic if the worker amended the evidence-protocol section, and re-run all four gates.
- [ ] After `444c06bc` and `bd0edfa2` pass: link the bd0edfa2 receipt and refresh the `4e732b56` link to `workload-selection.md`; dispatch wave-11 `e1e0e7ff`, `5ee83cd3`, `203ee826` (prompts drafted: `dispatch-e1e0e7ff.md`, `dispatch-5ee83cd3.md`, `dispatch-203ee826.md` in the session scratchpad — copy their substance into progress.json or re-derive if the scratchpad is gone; the design fences are in `bch-api-design.md` "Consumer migration files").
- [ ] `97410c80` waits for 203ee826. Wave 11 closes with 88ca7d2f, e1e0e7ff, 5ee83cd3, 203ee826, 97410c80.
- [ ] Standing: surfaced_pitfalls (verify-lean.sh header count; FpField.lean link gap for lean waves; proof-sketch.md:922 stale L4.3 line) — reconcile at close.

## Traps — do not repeat these

- **Do NOT wait for idle session-7 teammates to answer.** Three of them (`worker-2b6968d3-2`, `worker-3f7edef1`, `worker-d7749931`) stayed idle through two messages over 40 minutes while a fourth (`worker-bd0edfa2-2`) woke on the first message. Give one nudge, then stop them and re-dispatch fresh from the worktree.
- **Do NOT run anything heavy on the host during a worker's measurement window.** A lead-side GAP probe (25 GB, one core) ran 11:08–11:11Z while a benchmark worker was preparing; the worker's window happened to start later (11:27Z), but the overlap risk was real. Ask for the window, and keep lead-side probes for after wave close.
- **Do NOT run cargo-ci on main while workers build.** The 444c06bc failure carried five unrelated gf2-algebra timeouts under load; a re-run passed. Run cargo-ci when `uptime`'s 1-minute load is low, or accept a retry.
- **Do NOT `git add .jit` after a gate run: `.jit/gate-runs` is gitignored and the add aborts the `&&` chain.** Add the issue file, `events.jsonl`, `index.json` explicitly.
- **Do NOT assume the survey contract's measurement cells can be measured on the canonical model before cutover-dvbt2.** The DVB-T2 rows need `Shortened<C>` with the R-39 fast path (203ee826); until then, mother-length cells are covered only by the dated bd0edfa2 amendment (R-37).
- **Do NOT dispatch 97410c80 before 203ee826 is done** (DAG enforces it now).
- **`free -g` fails under the user's shell alias** (`free: Multiple unit options`); use `command free -m`. `/usr/bin/time` is not installed.
- All traps in handoff.md through handoff-4.md remain in force (codex startup-deadlock probing, gate-evaluate clean tree, `cmd | tail` exit-code masking, claim-release-reclaim, .jit porcelain checks, never reclaim agent-4e732b56, no-delegation clause in every native prompt, done-transitions respect the DAG, doc-link hash refresh, worker reports truncate ~3KB).

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; progress.json, plan.md, breakdown.json, investigation.md, bch-api-design.md, extension-design.md (this directory).
- Rework/dispatch prompt files (session scratchpad, may not survive): `rework-bd0edfa2-444c06bc-r1.md`, `rework-2b6968d3-r1.md`, `dispatch-d7749931.md`, `rework-3f7edef1-r1.md`, `rework-3f7edef1-r1-research.md`, `dispatch-203ee826.md`, `dispatch-e1e0e7ff.md`, `dispatch-5ee83cd3.md`.
- Survey contract and findings: dev/active/4e732b56/{workload-selection.md,findings.md}; baselines in KEPT worktree agent-4e732b56.
- Proof sketch: dev/active/64fd3afd/proof-sketch.md (O-3 for d7749931).
- ETSI DVB-T2 streams: ~/dvb_test_vectors/VV001-CR35_CSP (host-only, not committed).
