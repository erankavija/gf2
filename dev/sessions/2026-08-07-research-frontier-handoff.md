# Session handoff — research-frontier epics + GGK feasibility (2026-08-07)

Paused mid-gate-cycle on issue `b488f02c`. This note is the resume contract.

> **Status update 2026-08-08.** Resume step 1 is **done**, though it took more
> than the one regeneration this note anticipated: the receipts were rebuilt
> several times as review found harness defects, and the end state is the
> two-binary provenance recorded as **DEC-02**. The four measurement receipts
> (throughput, sustained, envelope, zero-fraction) come from binary
> `b1fe566f…` at harness commit `0e0b0aec`; the equivalence receipt was
> regenerated later at `2bea03a4`, binary `77b52ddb…`, to add the $q = 7$,
> $n = 20$ cell. **DEC-01** fixes the measurement set: no further regeneration.
> Steps 2–3 — the gate cycle and issue completion — are in progress with the
> lead. The state described below is the state at the pause and is kept as
> written; where it and this note disagree, this note is current.

## Where things stand

### Issue b488f02c (feasibility study, claimed by agent:opus-feasibility)

- **Verdict: GO** (survives every review round so far). Frontier under the 12 h
  budget: SE 1e-3 reaches n=28/24/20 for q=3/5/7; SE 1e-4 reaches n=20/16/16.
- Gate history: doc-review round 1 (9 blockers) → fixed; research-review round 1
  (6 blockers, incl. HKS Thm 1.3 ≥ 1/q catch → q=5 anomaly RETRACTED as
  stream-reuse defect) → fixed; doc-review rounds 4–5 caught staleness layers.
- **In flight when paused:** the round-5 sweep retired the superseded censoring
  rule *from code* (`12bb1e3b`), which changes `binary_sha256`, so the worker
  was REGENERATING all receipts with the new binary. The worktree is
  intentionally dirty: `envelope/sustained/zero-fraction-2026-08-07.csv`
  deleted, `throughput-2026-08-07.csv` partially rewritten,
  `feasibility-study.md` mid-edit. Last consistent committed state:
  `12bb1e3b` (code) on top of `a9d5b1c5` (pooled counts, all 13 cells satisfy
  HKS Thm 1.3).

### Resume steps (in order)

1. Re-dispatch (or resume) a worker to finish the receipt regeneration:
   grid → sustained → envelope → zerofrac at the current committed binary,
   fold results into the study, full staleness sweep, commit `jit:b488f02c`.
   The dispatch brief is at the session scratchpad or reconstruct from the
   issue + this note; binding methodology amendments are recorded in the study.
2. `jit gate evaluate-all b488f02c` (doc-review on terra must clear the swept
   text; research-review on sol re-reviews pooled stats + retraction).
3. On double green: Workflow E — verify the four amended hard criteria,
   `jit issue update b488f02c --state done`, commit state, cascade check
   (`jit graph rdeps b488f02c`), `jit validate`.
4. Next work per study §7: campaign task 1 is the HKS ≥ 1/q per-cell acceptance
   test wired into a pre-registered q ∈ {5,7}, n ≈ 12–20 sampling plan; then
   break down epic b8206228 (jit-planning-lead) using the study's G1–G8 gap
   table (G5 = permanental-rank predicate, 1.0 d; G6 = bipedal3 dispatcher
   defect ~3× cost, file as gf2-algebra bug when breaking down).

## Standing decisions made this session (do not re-litigate)

- Citation system: `citation` item kind, registry `.jit/references.toml`,
  `cites:` labels; `@/citation/<Key>` addressable; issues carry `## References`.
- research-review gate (two-tier: deterministic citation checks then AI rubric)
  exists, pinned to `codex exec -m gpt-5.6-sol -c model_reasoning_effort=xhigh`;
  the five other AI gates pinned to `gpt-5.6-terra` at `high`. Four research
  invariants added (claims-trace-to-artifacts, uncertainty-reported,
  external-claims-cited, falsification-preserved).
- b488f02c REQ-01 censoring clause = projection-as-estimate (user-approved
  amendment a66d42f8); epic b8206228 Background/REQ-04 amended per 1364c050
  (permanental-rank event, Scheinerman prior-art, GGK regime k ≤ 0.1√n).
- 12 h campaign wall-clock budget; harness lives in
  `dev/research/permanent-sampling-feas/` (standalone crate).

## Open items beyond b488f02c

- Epics created today (all with citation labels): aed96ef9 finite-blocklength
  bounds (blocked on bug 325e5c89 — shannon_capacity misreads Eb/N0, confirmed),
  b7157be6 OSD, cce5da8c qLDPC (depends on OSD), 55087229 PAC/AED (depends on
  polar b81c239c + bounds), c7cfd37e additive NTT (design-study child 24701af9
  ready), b8206228 permanent statistics (this study's parent).
- External review of all five epics preserved at
  `dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md`
  (linked to each epic) — apply its splitting/pinning guidance at breakdown.
- Polar epic b81c239c needs planning repair before PAC work (generic criterion,
  placeholder child).
- Task 76dfd2ff: mark historical S3/S5 receipts non-authoritative (low).
- Pending user-approval wording fixes (amendment package v3, small): NTT epic
  "tower-field arithmetic" → GF(2^m)-but-not-binary-towers; PAC epic Yao claim
  → "L=128 approaches, L=256 essentially coincides".
- Citation registry: verify + add Chen et al. eprint 2026/014 (HQC additive
  FFT); retry `refdb add item --arxiv` for the four manually-entered papers
  (arXiv API timed out; beware duplicate slugs — manual entries lack the
  arxiv_id attr, so ingest would NOT dedup: prefer refdb note enrichment).
- CRC direction (Koopman DSN04) assessed as strong fit, recorded in refdb
  (`crc-selection-hd-sweeps`); slot as story under short-blocklength epic when
  wanted.
- refdb graph updated through journal entry 7 (GO verdict, anomaly retraction,
  HKS one-sided-test idea, May-era idea statuses).

## Infrastructure notes for the resuming session

- Worker dispatch: claim + in_progress BEFORE Agent dispatch; workers never run
  `jit gate`/state mutations; commits tagged `jit:<short-id>`; tell workers to
  background long runs and END TURN (notification re-invokes them) — polling
  loops were this session's main worker friction.
- Advisor: `codex exec -m gpt-5.6-sol -c model_reasoning_effort=xhigh -s
  read-only` (user-approved). Peer discussion channel:
  `~/Projects/forum-poc/forum.sh`, codex listens as `agent:codex`
  (FORUM_DIR default /tmp/jit-forum; stale queued messages possible on first
  recv).
- GPU on gfx1030: batch sizes {256, 1024, 2048} were measured at q=3, n=24, and
  M=1024 is the fastest of those three. M=4096 was attempted once; the device
  faulted and then recovered on its own, and that single fault's cause was never
  established — a watchdog timeout is one hypothesis, not a finding. Nothing
  beyond that one attempt was explored. Keep avoiding 4096 on the strength of
  what a fault costs, not of a known threshold. *(Requalified 2026-08-08 to
  match `dev/studies/b488f02c/feasibility-study.md` §4.5 and
  `gpu-hang-2026-08-07.log`, which record what was and was not captured; the
  original line asserted a watchdog limit and an unscoped optimum.)*
