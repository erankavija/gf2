# Handoff — Ordered-statistics decoding as the generator-matrix and syndrome soft-decision baseline (b7157be6) — session 3

**Date:** 2026-08-26T06:35+03:00
**Session number:** 3
**Prior handoffs:** `dev/active/b7157be6-osd/handoff.md`, `dev/active/b7157be6-osd/handoff-2.md`

## Current state

- Epic: `b7157be6` — state: backlog (container; assigned `agent:jit-execution-lead`)
- Wave in progress: wave 6 of 6 — 2 of 3 done (d76ffd12, 5194f470); cef1ae5f in research-review rework + pending escalation
- Children summary: bracket P/B done; 16 of 17 impl issues done; cef1ae5f claimed `agent:worker`, merged to main (c59f1a8f + one rework merge pending), research-review FAILED R2 with 5 blocking findings
- Active claims: cef1ae5f (`agent:worker`); epic assign-only
- Open escalations: ESC-04 (research-review F1 interval validity + F5 delta-units contest cef1ae5f's criterion design; raised to the human immediately after this handoff)
- Progress file: `progress.json` here (waves, model plan, ESC-01..04, pitfalls, rework counts, 6 filed defect bugs)

## What just happened

- Wave 6 dispatched: d76ffd12 (codex Sol xhigh), 5194f470 (codex Luna xhigh) in SHA-anchored worktrees (673bb928); cef1ae5f (native Opus Claude subagent `worker-cef1ae5f`, worktree unseeded due to disk pressure).
- 5194f470 done: all gates round 1 (merged 04a43e45). d76ffd12 done: all gates round 1 (merged f4a5883b). Zero rework on both.
- Disk crisis mid-dispatch: 27G cache pool copied cross-fs onto 99%-full root; seeding script died mid-copy; codex removed their 30G stale control worktree; pool deleted, worktrees reclaimed post-merge; root now ~90G free.
- eBCH campaign (cef1ae5f) COMPLETE: 14/14 cells terminal, 17 attempts, receipt schema 1, seed 0xC8322EFF. Order-2 recorded verdicts: 4 of 6 simulation points outside acceptance; block-resolved sensitivity analysis (worker-derived) shows 3 of those are artifacts of an interval-width defect — one robust disagreement at 4.56 dB (the abscissa with the preserved table-vs-figure source contradiction).
- Campaign resume required detached checkout of 673bb928 (config_hash covers HEAD revision + binary sha — resume-hostile); one final 20M-sample invocation finished 5.23 dB (8,066,414 blocks) + all 7 order-1 controls.
- Worker surfaced 7 protocol defects; 6 tracked bugs filed (all `dep → c8322eff`, epic:osd label advisory, NOT epic children — must not block the epic): 9af52659 (resume hash conflation), f7844d2d (quadratic replay), cf37be3b (delta units), fd1f39e0 (control ordering), 8a908f79 (HIGH: BER intervals assume bit independence under block bursts — intervals ~3x too narrow), 14029b3f (cpu_model platform token), 52afe5ef (receipt argv truncation).
- Campaign merged c59f1a8f (data-only: 3 files under dev/simulation_results/osd-ebch-128-64/). Docs linked via jit doc.
- research-review R1 FAIL (122ms, tier-1): stale `cites:Yue2022` label on DEPENDENCY c8322eff leaked into gate context. Fixed by dropping the stale labels from c8322eff + 5194f470 per the standing ESC-01 owner ruling (bracket where text cites, drop where it doesn't); manifest mirrored + validated (validate needs `--plan` to exit 0).
- research-review R2 FAIL: F1 (high: BER interval coverage invalid under clustered errors + adaptive stopping; reviewer holds REQ-01/REQ-02 unmet, block-resolved analysis also insufficient), F2 (high: receipt lacks invocation argv), F3 (high: hardware not artifact-backed), F4 (medium: order-1 table bare BLER estimates), F5 (medium: acceptance predicate unit-invalid), F6/F7 advisory.
- Rework round 1 dispatched to worker-cef1ae5f for F2/F3/F4 only (lscpu artifact file, operator invocation ledger from its own session records, BLER CI columns). F1/F5 withheld → ESC-04.
- Manifest footprint amended (checkpoint file added to cef1ae5f creates); validated + render-checked.
- Neighbor coordination throughout: codex lead (epic 6dc81018) merged their b749 tuning cutover (several MAIN windows exchanged); their independent design+impl reviews consumed my 3fa7c9d0 design verdict (PASS + ProfileId/digest precision notes); I answered three of their architecture consults (endpoint crossover convention: option A + tracked enhancement; baked-extent const-generic measurement: OK with production-route confirmation caveat; artifact transaction: git commit as the transaction). Their full-gate quiet slot keeps failing for environment reasons (sccache sandbox IPC, then sandbox-blocked GPU test); they now need gates outside their sandbox and await a stable quiet window.

## What to do next

- [ ] Present ESC-04 to the owner (AskUserQuestion) — options prepared: (A) fix tool per 8a908f79 (+cf37be3b) and re-run campaign under valid stopping (~10x compute at high SNR for 100 block-errors; less with a reduced block-error target), (B) owner amends cef1ae5f REQ-01/REQ-02 wording to accept recorded-method intervals with defects tracked (criterion change — owner-only), (C) reject cef1ae5f / close epic partially. F5 rides along (predicate units: tool fix in cf37be3b vs criterion clarification).
- [ ] Collect worker rework (F2/F3/F4), merge its commits, content-check, re-run research-review only after ESC-04 resolves (avoid burning a round mid-escalation).
- [ ] After research-review + doc-review pass: complete cef1ae5f, then Section 10: reconcile surfaced_pitfalls + 6 filed bugs against epic criteria (epic REQ-02 "reproduces ... within simulation confidence intervals" is directly entangled with ESC-04's outcome — do NOT run holistic-review before ESC-04 resolves), evaluate epic gates (repo-validate, doc-review, holistic-review), completion report, archive, epic done.
- [ ] Announce MAIN RELEASED to agent:codex when the epic's main/JIT writes finish; they are waiting for a quiet window for their b749 gates (outside-sandbox) + 389 calibration.
- [ ] Reclaim worktree agent-cef1ae5f (reclaim script; pool at /data/gf2-osd-cache-pool is freshly rebuilt and lean) after the issue closes.

## Traps — do not repeat these

- **Do NOT seed worktrees from a multi-wave cache pool without checking its size.** The pool grew 4.8G→27G over five waves; cross-filesystem plain copies of 27G x3 onto a 99%-full root nearly caused host-wide ENOSPC and silently killed the dispatch script mid-loop (cp "File exists" error, cef1ae5f worktree never created, exit 0 masked by `| tail`). Check `du -sh` pool and `df` target first; delete the pool when bloated (protocol sanctions it).
- **Do NOT trust `pgrep -f <pattern>` where the pattern appears in your own command line** — it self-matches the wrapping shell. Evidence: false "NEW INVOCATION RUNNING" verdict during the quiet-slot handshake. Use a bracket pattern (`ebch_osd[_]awgn`) or `pgrep -x`.
- **Do NOT treat forum sends as delivered.** One handoff-critical HOST QUIET message never reached codex (~60 min of quiet host lost); worker inbox messages repeatedly crossed/lagged (three "crossed message" rounds with worker-cef1ae5f). After any handoff-critical send, expect an explicit ack and ping on timeout; for Claude subagents assume replies may cross your next instruction.
- **Do NOT run gate evaluations (or any .jit write) while the neighbor session holds a MAIN/JIT window** — and batch your own. Session-long pattern: announce window → write → announce release. Concurrent .jit writes from two sessions were never attempted; keep it that way.
- **Campaign resume is hash-gated on HEAD revision AND binary sha (9af52659).** Any commit or rebuild invalidates the checkpoint. Resume = detached checkout of the receipt's recorded revision with artifacts restored untracked + the ORIGINAL binary (verify sha256 against receipt). Never rebuild before a resume. Replay of the durable prefix costs ~95 min per resume (f7844d2d) — size --max-samples to finish in ONE invocation.
- **research-review tier-1 greps the whole gate context, including DEPENDENCY issues' labels.** A stale cites: label on a done dependency fails the gate in milliseconds before any scientific review. Sweep cites-label/text drift across an issue's dependency closure before evaluating (fix per ESC-01 ruling: drop labels the text doesn't bracket).
- **breakdown_manifest.py validate exits 1 without `--plan`** (contract_refs unchecked) while still printing advisories — a `| tail` hides the rc. Always pass `--plan dev/active/<epic>/plan.md` and check rc.
- **Do NOT let the lead hand-reconstruct operational history a worker can source from its own transcript** (invocation ledger). Reconstruction risks falsifiable inaccuracies in permanent artifacts; the worker records exact commands.
- (Carried, still in force: worktree cwd discipline; in-worktree target/ only, never tmpfs; codex prompt via file + stdin /dev/null; jit state mutations only from the primary; manifest is authoritative; commit-then-evaluate gates, never chain a re-eval behind an unfixed tree; no plan/breakdown-review re-runs for execution-time amendments (NOTE-05); codex sandbox blind to /dev/kfd; grep sweeps insufficient for stale-prose classes — read whole documents.)

## Open questions needing invoker input

- Question: How should cef1ae5f satisfy research-review F1 (interval statistical validity) and F5 (acceptance-predicate units)?
  - Context: The independent reviewer holds that REQ-01/REQ-02 are unmet while the recorded intervals lack demonstrated coverage (bit-level Clopper-Pearson under block-burst errors + adaptive stopping, ~3x too narrow) and the predicate mixes log10-decade δ with linear probabilities; both defects live in the DONE campaign tool (tracked 8a908f79, cf37be3b), and the criteria as written prescribe exactly the recorded-method comparison the reviewer rejects.
  - Options: (A) fix tool + re-run campaign under a valid stopping rule (highest rigor; ~10x compute at the two hardest cells for 100 block-errors, tunable down); (B) amend cef1ae5f criteria to accept recorded-method intervals with the defects tracked and the dual analysis disclosed (owner-only criterion change; fastest; epic REQ-02's "within simulation confidence intervals" then rests on defective intervals — holistic-review risk); (C) reject cef1ae5f and close the epic partially.
  - Recommendation: A with a reduced block-error target (e.g. 30-50 block errors/cell → valid-if-wider intervals at ~3-5x compute for the two hardest cells only, cheaper cells re-run in minutes), because epic REQ-02 is a hard criterion about statistical reproduction and both reviewers (worker's own analysis + gate) agree the recorded intervals cannot carry that claim.

## Reference artefacts

- Epic: `jit issue show b7157be6`; bracket P=312200e4, B=8d650253 (done)
- Plan/manifest: `dev/active/b7157be6-osd/plan.md`, `breakdown.json` (amendments through session 3 validated)
- Campaign evidence: `dev/simulation_results/osd-ebch-128-64/{README.md, ebch_osd_awgn.json, ebch_osd_awgn.checkpoint.json}` (merged c59f1a8f; rework commits pending merge)
- Research-review R2 findings: `jit gate status cef1ae5f research-review` (structured findings block)
- Filed defect bugs: 9af52659, f7844d2d, cf37be3b, fd1f39e0, 8a908f79, 14029b3f, 52afe5ef (+ divergence warnings are advisory)
- Progress: `dev/active/b7157be6-osd/progress.json`
- Worker: Claude subagent `worker-cef1ae5f` (idle, holds full campaign transcript); worktree `.agents/worktrees/agent-cef1ae5f` (branch has unmerged rework once it commits)
- Neighbor: codex lead epic 6dc81018; worktrees agent-b749bdfc, agent-b749bdfc-gatefix, agent-389aa4de(-v2), agent-fbe66a7a, agent-ef18c60b — DO NOT TOUCH; forum mailbox `agent:claude` (re-arm listener each session; delivery is lossy — ack-and-ping discipline)
