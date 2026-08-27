# Handoff — Ordered-statistics decoding as the generator-matrix and syndrome soft-decision baseline (b7157be6) — session 2

**Date:** 2026-08-25T23:15+03:00
**Session number:** 2
**Prior handoffs:** `dev/active/b7157be6-osd/handoff.md`

## Current state

- Epic: `b7157be6` — state: backlog (container; assigned `agent:jit-execution-lead`)
- Wave in progress: waves 1–5 of 6 COMPLETE; wave 6 not dispatched
- Children summary: bracket P/B done; 14 of 14 waves-1–5 impl issues done; 3 ready (wave 6: d76ffd12 bp-osd-decoder, 5194f470 osd-discard-thresholds, cef1ae5f osd-ebch-campaign)
- Active claims: none for wave 6 (wave-5 claims released on completion); epic assign-only
- Open escalations: none (ESC-01..03 in progress.json all resolved by owner this session)
- Progress file: `progress.json` here (statuses, rework counts, pitfalls PIT-01..07, NOTE-01..05, ESC-01..03)

## What just happened

- Wave 1 closed: 5dd3539f, 377a7a62, 6beaf008, bebe485c, 80cead18, a82f2dd9 done. 80cead18 took 4 code-review rounds (stale archive-audit prose surfaced piecemeal; closed by a full-file semantic audit, not grep-chasing). a82f2dd9 took 5 research-review rounds — see Traps.
- Wave 2 closed: 835e15fb, dd6f1665, ac78aff8, f0d6fb9a done. ac78aff8's session-1 worker WIP was verified and committed by the lead; its NaN-panic rustdoc took 4 rounds (missing doc → overstated on zero-iteration paths gldpc/turbo/chase — qualify with "and at least one iteration runs").
- Wave 3 closed: abd48d99 (shared OSD reprocessing engine, native Opus) done. `OsdWork::tested_candidates` semantics resolved by review: counts every adapter-evaluated candidate (accepts() invoked), not accepted ones. Engine footprint amendment: patterns.rs touch (OsdTermination::InconsistentTransform) recorded in manifest.
- Wave 4 closed: c97b2961 (generator-matrix OSD decoder, Sol xhigh) done, all gates round 1.
- Wave 5 closed: 583ec31c (syndrome adapter, native Opus; adds `solve()` beyond REQ-01's literal inputs — justified, reviewers passed it round 1), c8322eff (campaign binary, Luna; adds rand08/rand_chacha08 aliases per the documented workspace rand-0.8 exception), b3cab18e (segmentation, Luna; 5 code-review rounds — see Traps) all done.
- a82f2dd9 research-review converged via the gate's `## Decisions` mechanism: DEC-01..03 recorded on the issue (owner-approved), generalized to all digitized third-party sources (Fossorier1994 AND Yue2022). Dataset renamed to `osd_ebch_128_64_fossorier1994.{csv,md}` (D-21 pin); committed digitization receipt (extract.py + calibration.json, re-extraction max delta 0.0035 decades, sensitivity spread ≤0.0288), Yue2022 receipt, access audit (fresh Unpaywall JSON + ResearchGate false-positive identification: the advertised full text is the five-author 1999 IEICE multilevel-signaling paper, registered as `@/citation/Isaka1999`; the 1995 article remains unobtained; D-21 premise stands).
- Owner-approved amendments this session: citation-format brackets on a82f2dd9/cef1ae5f (+drop cites:Yue2022 from cef1ae5f), DEC-01..03 + generalization, b3cab18e Background exports sentence + D-16 landing wording, engine/segmentation footprint touches. All mirrored in breakdown.json, manifest validate + render green.
- Owner ruling (NOTE-05): plan-review/breakdown-review are breakdown-instantiation stamps; do NOT re-run them for execution-time amendments. Their completion-time passes stand; late advisory runs on P/B that flagged D-16 serialization are superseded — wave-6 dispatch order serializes the shared osd/mod.rs writers operationally.
- External re-home per plan D-02: cce5da8c (qldpc epic) now depends on 583ec31c instead of the whole OSD epic. c0bb2ab1 wired onto 5dd3539f (clears jit-validate isolation error; orphan-leaf label warning remains, advisory).
- Neighbor coordination (mailbox `agent:claude`, listener armed via `~/Projects/forum-poc/forum.sh recv`): codex lead (epic 6dc81018, tuning-ownership cutover) — exchanged architecture reviews; granted two main/JIT windows (their commits 87db4f3b, 95def717, and design work on branch worktree-agent-3fa7c9d0 — DO NOT TOUCH their worktree). Their bench-slot request is deferred until their cutover lands; they will ask on the forum.
- Host state: all OSD worktrees reclaimed (cache pool `/data/gf2-osd-cache-pool` freshly harvested, warm). Worker branches worktree-agent-* retained. Main clean, cargo-ci green at HEAD.

## What to do next

- [ ] Dispatch wave 6 per progress.json model plan: d76ffd12 bp-osd-decoder (Sol xhigh), 5194f470 osd-discard-thresholds (Luna xhigh), cef1ae5f osd-ebch-campaign (native Opus, research classification, gates research-review + doc-review). d76ffd12 and 5194f470 both touch gf2-coding/osd — worktrees mandatory; cef1ae5f runs the actual campaign (long simulations; coordinate host time with agent:codex first, they also want a 45-min bench slot).
- [ ] cef1ae5f prompt must include: BER metric (D-21), dataset receipts as comparison target, campaign binary `ebch_osd_awgn_campaign` flags (--checkpoint --receipt --seed --max-samples --target-errors), REQ-02 interval acceptance rule, and its `## Decisions`-aware research-review (the gate binds DEC items; cef1ae5f has bracketed [Fossorier1995] and no cites:Yue2022 label after amendment).
- [ ] 5194f470 consumes b3cab18e's OsdComplexityPolicy/segmented surface (see NOTE re advisory: engine.rs:372 self-referential # Errors links — fix opportunistically in that issue's footprint if reviewers repeat it).
- [ ] After wave 6: Section 10 epic completion — reconcile surfaced_pitfalls/NOTES against epic criteria, evaluate epic gates (repo-validate, doc-review, holistic-review), completion report, archive.
- [ ] Check forum mailbox `agent:claude` at session start and re-arm the listener; codex may have replied about their design rework (my last review: B1 EnvelopeAssembly undefined, B2 child-guard unstated, S1 ExplicitSubset over-engineering).

## Traps — do not repeat these

- **Do NOT let the shell sit in a worktree while running lead operations.** cwd persists across Bash calls; `cd <worktree> && ...` in one command silently reroutes later relative-path git/jit commands. Evidence: this session a `git merge` executed inside agent-b3cab18e (self-merge no-op caught late), and `jit gate status` read codex's stale worktree `.jit`, which looked like a wiped gate history. Instead: prefix lead commands with `cd /home/vkaskivuo/Projects/gf2 &&` or use `git -C`.
- **Do NOT chain a gate re-evaluation behind an unfixed tree.** A code-review re-run launched before the fix commit re-reviews the old HEAD and burns a round with the identical finding (b3cab18e R2). Commit first, then evaluate.
- **Do NOT re-run plan-review/breakdown-review for execution-time amendments** (owner ruling, NOTE-05). Amendment stamps are: breakdown_manifest.py validate (with the full --known-source list incl. D-21) + render --check, plus the amended issue's own gates. Session-1's trap prescribing bracket-gate re-runs applied to the pre-execution phase only.
- **Do NOT resolve research-review failures by adding data the source does not report.** The gate's convergence lever is the issue's `## Decisions` section (DEC-NN items bind the reviewer). Word DEC items to cover ALL digitized sources in the dataset, not just the pinned one — DEC-01 scoped to "the 1994 source" cost a full round when the rubric re-fired on Yue2022 (R4).
- **Do NOT trust grep sweeps to close stale-prose finding classes.** 80cead18 burned rounds R2/R3 on differently-phrased stale claims ("8 traits", "1 deprecation") that literal greps missed. After the first stale-prose finding, read the whole cited document and fix every semantic instance.
- **Do NOT dispatch a worker whose footprint forbids the file its REQ needs.** b3cab18e's prompt said "TOUCH engine.rs and patterns.rs ONLY", so the worker could not re-export its required public surface through mod.rs → guaranteed code-review FAIL. Check REQ-required public surfaces against the manifest footprint before dispatch; amend the footprint first, not after.
- **The codex sandbox cannot see /dev/kfd** — gf2-sim gpu dispatcher test fails with HIP code 100 inside codex workers. Re-verify cargo-ci on the host before treating it as real. Chronic 5s-budget flakes (hybrid_*, executor_oom_fallback) still flip under load — never run two cargo-ci invocations concurrently and re-run quiet before attributing.
- **Codex workers cannot git-commit in the primary checkout either** (sandbox blocks .git writes: "index.lock: Read-only file system"). Renames arrive as unstaged deletes+untracked; the lead stages and commits. Prompts must say leave-in-tree.
- (Carried from handoff 1, still in force: worktree cwd discipline for codex dispatch; in-worktree target/ caches only, never tmpfs; reclaim via reclaim-worker-worktree.sh only; codex prompt via pre-written file with stdin from /dev/null; jit state mutations only from the primary; manifest is authoritative for descriptions.)

## Open questions needing invoker input

None. (All session-2 escalations resolved; wave 6 is fully specified.)

## Reference artefacts

- Epic: `jit issue show b7157be6`; bracket P=312200e4, B=8d650253 (both done; gate stamps per NOTE-05)
- Plan/manifest: `dev/active/b7157be6-osd/plan.md`, `breakdown.json` (amendments through session 2 applied and validated)
- Dataset + receipts: `dev/reference_data/osd_ebch_128_64_fossorier1994.{csv,md}`, `..._digitization/` (extract.py, calibration.json, yue2022_fig1_receipt.json), `..._access_audit/`
- Campaign binary: `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs` (+ CLI tests)
- OSD module: `crates/gf2-coding/src/osd/{mod,patterns,engine,generator,syndrome}.rs`
- Progress: `dev/active/b7157be6-osd/progress.json`
- Neighbor: codex lead epic 6dc81018; worktree agent-3fa7c9d0 and branches — DO NOT TOUCH; forum mailbox `agent:claude` (listener must be re-armed each session)
