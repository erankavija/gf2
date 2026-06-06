# f9717e7e — gf2-sim project plan

**Status:** Active (planning + audit pass; no implementation worker dispatched yet)
**Author:** `agent:project-lead`
**Created:** 2026-06-07
**Epic:** `f9717e7e` — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim)

This plan is the executable run-book for epic `f9717e7e`. It does **not**
restate the epic body — the 15-row locked-architecture table, phase
structure, critical-path diagram, dependency surgery, and 11 success
criteria all live in the epic description (the SSOT for those facts).
Cross-references in this plan use the form `see epic f9717e7e §<heading>`.

---

## §1 SSOT artefact map

| Topic | Authoritative source |
|---|---|
| Locked architecture (15 rows) | epic `f9717e7e` description |
| Phase structure overview | epic `f9717e7e` description |
| Per-phase deliverable themes | story descriptions (`bcf7776d`, `1f588e2a`, `9e853c62`, `5d0a3fad`, `a5635da5`) |
| Per-task scope + acceptance | task descriptions |
| Phase 0 design decisions (Stage/Connector trait shapes, SoA↔AoS, ChaCha20 seek scheme, checkpoint schema, crate boundaries, multi-arch HIP, multi-GPU seams, failure-mode policy, layered API, precision strategy, migration plan) | `dev/active/ec530af9-pipeline-design.md` (to be produced by task `ec530af9`) |
| Determinism contract — operational detail | Phase 0 design doc (cited from the epic body) |
| Receipts directory layout | this plan §5–§6 |
| DVB-T2 campaign artefacts (CSV, PNG, closure note) | epic `epic:dvb-t2-awgn-campaign`, owned by `e4849f07` |
| Existing DVB-T2 PLAN.md, README.md, smoke/ | `dev/benchmarks/dvb_t2_awgn/` — pre-pipeline; a Phase D closing note will point forward |

**Drift resolution rule:** when an issue body and this plan disagree on
scope or acceptance, the issue body wins; the plan is amended. When the
plan and the Phase 0 design doc disagree on a design decision, the design
doc wins; the plan is amended. No autonomous amendment of issue bodies —
escalate to user per project-lead `references/escalation-policy.md` and
memory rule `feedback_no_autonomous_amendments`.

---

## §2 Phase-by-phase run-book

ID-map (canonical short IDs throughout the rest of this document):

| Phase | Story | Tasks |
|---|---|---|
| 0 | (single task) | `ec530af9` (design doc) |
| A | `bcf7776d` | `118a0091`, **`c0b1702d`** (baseline measurement, added 2026-06-07), `3fcb7025`, `81d05bab`, `c09d3e95`, `db9836e4`, `5f12e7ff`, `48a0db6c` |
| B | `1f588e2a` | `36075e4c`, `f6004add`, `a930be7f`, `d3f1616a`, `ed575f15`, `14f59c2d` |
| C | `9e853c62` | `75c22fa8`, `de160fc5`, `571c11c4`, `42eac5cc` |
| D | `5d0a3fad` | `8c8302c8`, `bbf6b6ee`, `0d9cb8e3` |
| E | `a5635da5` | `acf9b11a`, `e478daa8`, `23d3525f`, `18e69a1a`, `110e45cc` |

### Phase 0 — Design doc

- **Critical path:** serial; one task (`ec530af9`).
- **Wave 0.1:** `ec530af9` alone.
- **Done =** doc lands at `dev/active/ec530af9-pipeline-design.md` and is
  attached to the issue via `jit doc add`; `doc-review` gate green; user
  approves at the design-doc milestone (`§4` review point 1).
- **Dispatch:** `general-purpose` sub-agent with
  `references/architect-agent-prompt.md` template; not worktree-isolated
  (single writer, doc-only).

### Phase A — CPU foundation (parallel with B after Phase 0)

- **Critical path:** `118a0091` → `3fcb7025` → `5f12e7ff` → `48a0db6c`.
- **Branches:** `81d05bab`, `c09d3e95`, `db9836e4` fan out from
  `118a0091` and merge at `48a0db6c` (via `db9836e4`).
- **Waves**

  | Wave | Issues (in parallel) | Notes |
  |---|---|---|
  | A.1 | `118a0091`, `c0b1702d` | scaffolding + baseline measurement run in parallel; `c0b1702d` exercises only the legacy `simulation.rs` path so no conflict with `118a0091`'s new crate scaffolding |
  | A.2 | `3fcb7025`, `81d05bab`, `c09d3e95`, `db9836e4` | 4-way fan-out; **worktree isolation required** (memory rule `feedback_parallel_agent_isolation`); `3fcb7025` additionally depends on `c0b1702d`'s baseline receipt being committed |
  | A.3 | `5f12e7ff` | waits on `3fcb7025` and on `db9836e4` (audit C5 fix 2026-06-07) |
  | A.4 | `48a0db6c` | waits on `db9836e4`, `5f12e7ff`; CLAUDE.md update sub-bullet (Phase A close) |

- **Done =** all 7 tasks done with their gates green; Story `bcf7776d`
  success criteria met (determinism property test passes for ≥ 3
  configurations; receipt logged in `cpu-foundation-receipts.md`;
  CLAUDE.md updated).

### Phase B — GPU stages (parallel with A after Phase 0)

- **Critical path:** `36075e4c` → {`f6004add`, `a930be7f`, `d3f1616a`} →
  `ed575f15` → `14f59c2d`.
- **Waves**

  | Wave | Issues (in parallel) | Notes |
  |---|---|---|
  | B.1 | `36075e4c` | HIP host infra — single writer |
  | B.2 | `f6004add`, `a930be7f`, `d3f1616a` | 3-way fan-out across distinct kernels; **worktree isolation required** |
  | B.3 | `ed575f15` | waits on B.2 |
  | B.4 | `14f59c2d` | waits on `ed575f15` |

- **Done =** all 6 tasks done with their gates green; Story `1f588e2a`
  success criteria met (GPU-vs-CPU byte-identity passes; receipt logged
  in `gpu-stages-receipts.md`; CLAUDE.md HIP dispatch model + multi-arch
  target list documented).

### Phase C — Hybrid executor (after A + B converge)

- **Critical path:** `75c22fa8` → {`de160fc5`, `571c11c4`, `42eac5cc`}.
- **Waves**

  | Wave | Issues (in parallel) | Notes |
  |---|---|---|
  | C.1 | `75c22fa8` | scheduler — single writer |
  | C.2 | `de160fc5`, `571c11c4`, `42eac5cc` | 3-way fan-out; likely same module — assess at C.1 done whether worktree mode is needed |

- **Done =** all 4 tasks done with their gates green; Story `9e853c62`
  success criteria met (hybrid byte-identity passes; receipt logged in
  `hybrid-executor-receipts.md`).

### Phase D — DVB-T2 application (after C)

- **Critical path:** `8c8302c8` → `bbf6b6ee` → `0d9cb8e3`. Strictly
  serial — three sequential tasks operating on overlapping files
  (campaign binary, DVB-T2 preset module, regression-test crate).
- **Waves**

  | Wave | Issues | Notes |
  |---|---|---|
  | D.1 | `8c8302c8` | preset |
  | D.2 | `bbf6b6ee` | campaign-binary migration; **closes the cross-epic dep into `e4849f07`** |
  | D.3 | `0d9cb8e3` | byte-identity regression |

- **Cross-epic handoff:** once `bbf6b6ee` lands, `e4849f07`
  (epic `epic:dvb-t2-awgn-campaign`) is unblocked. The multi-day
  production sweep is owned by `e4849f07`, not this epic.
- **Done =** all 3 tasks done; Story `5d0a3fad` success criteria met;
  closure note linking `dev/benchmarks/dvb_t2_awgn/PLAN.md` forward to
  this epic's receipts written by `e4849f07`'s owner (not project-lead
  scope).

### Phase E — 5G NR real-time + research enablers (after D)

- **Critical path:** `acf9b11a` → `e478daa8` → `23d3525f` → `18e69a1a`;
  `110e45cc` branches from `acf9b11a`.
- **Waves**

  | Wave | Issues (in parallel) | Notes |
  |---|---|---|
  | E.1 | `acf9b11a` | base graphs + per-`i_LS` shift tables (memory `feedback_ldpc_shift_tables` is the relevant trap) |
  | E.2 | `e478daa8` | 5G NR typestate preset |
  | E.3 | `23d3525f` | kernel tuning to real-time; concrete target = BG1, `i_LS` = 8, Z = 384, rate 1/2, BLER ≤ 10⁻², ≥ 200 Mbps decoded TB throughput on gfx1030 (audit C1 fix 2026-06-07) |
  | E.4 | `18e69a1a` | comparison harness vs aff3ct / IT++ |
  | E.5 | `110e45cc` | onboarding examples + CLAUDE.md (Phase E + epic close); waits on `e478daa8` and `8c8302c8` (audit H4 fix 2026-06-07) |

- **Done =** all 5 tasks done; Story `a5635da5` success criteria met;
  receipts logged in `5g-nr-realtime.md` and `comparison/`.

### Cross-phase serial blockers (summary)

```
Phase 0 ──┬─► A (Wave A.1)
          └─► B (Wave B.1)

Phase A + Phase B ──► Phase C (Wave C.1)

Phase C ──► Phase D ──► Phase E
                    └─► e4849f07 (cross-epic dep unblocked at D.2 done)
```

---

## §3 Sub-agent dispatch policy

| Classification | Phase mapping | Prompt template |
|---|---|---|
| `design` | Phase 0 (`ec530af9`); 5G NR table verification subtask of `acf9b11a` if it warrants a design pass | `references/architect-agent-prompt.md` |
| `implementation` | majority of A, B, C, D, E | `.claude/skills/jit-parallel/references/agent-prompt-template.md` |
| `documentation` | `110e45cc` (onboarding), CLAUDE.md updates inside any story | `references/doc-agent-prompt.md` |
| `research` | (none currently) | `references/explorer-agent-prompt.md` |

**Worktree isolation** is mandatory in Waves A.2, B.2, and possibly C.2.
Use `scripts/dispatch-worker-worktree.sh` per project-lead Section 6 step
2 (do NOT use Agent's `isolation: "worktree"` parameter — known stale-
ancestor bug). Run `scripts/check-leak-into-main.sh` after each wave.

**Reviewer model choice** (memory `feedback_sonnet_for_planned_dispatches`):
use `sonnet` for transliteration / wrapper / mechanical tasks; reserve
`opus` for proofs, novel perf work, novel design.

**Pre-flight per dispatch** (memory `feedback_jit_claim_before_dispatch`):
lead claims and transitions to `in_progress` BEFORE Agent dispatch.

---

## §4 Milestone review points

1. **After Phase 0 (`ec530af9` done):** user reviews design doc; lead
   awaits explicit approval before dispatching Phase A or Phase B.
2. **After Phase A converges (`bcf7776d` done):** lead presents
   CPU-foundation receipts + determinism property-test results.
3. **After Phase B converges (`1f588e2a` done):** lead presents
   GPU-stages receipts + GPU-vs-CPU byte-identity attestation.
4. **After Phase C done (`9e853c62` done):** lead presents
   hybrid-executor receipts.
5. **After Phase D done (`5d0a3fad` done):** lead reports campaign-binary
   migration status; `e4849f07` owner takes over the multi-day production
   sweep.
6. **After Phase E done (`a5635da5` done):** lead presents 5G NR
   real-time evidence + aff3ct / IT++ comparison + onboarding examples.
7. **Epic close:** lead produces `f9717e7e-completion-report.md`
   per project-lead Section 10.

---

## §5 `parallelism-pays` gate — receipt schema

**Tasks bearing the gate (7):**
`3fcb7025`, `f6004add`, `a930be7f`, `d3f1616a`, `75c22fa8`, `bbf6b6ee`,
`23d3525f`.

**Dual-baseline reporting (post-audit refinement, 2026-06-07,
user-approved per Q4)**: every receipt MUST report both the
single-thread CPU baseline (from `c0b1702d`) AND the
CPU-24-thread baseline (from `3fcb7025`'s receipt) where
applicable. The single-thread number is the headline number for
the gate; the 24-thread number is the diagnostic number that
prevents a GPU receipt from "passing" while actually being
slower than the 24-thread CPU implementation. `a930be7f` (GPU
LDPC BP) carries an additional `[hard]` threshold: ≥ 3× the
24-thread CPU baseline.

**Receipt entry format** (per task), aggregated in
`dev/benchmarks/gf2-sim/parallelism-receipts.md`:

```markdown
## <task-short-id> — <task-title>

- **Date:** <ISO-8601>
- **Hardware:** CPU=<model>/<threads>, GPU=<model>/<arch>
- **Baseline configuration:** <single-thread | single-host | single-stream>
- **Test configuration:** <decoder, demap, n_ldpc, batch, mean_iters>
- **Observed throughput:** <frames/sec ± σ>
- **Speedup factor:** <observed / baseline>
- **Required threshold (from task body):** <e.g. ≥ 12x>
- **Verdict:** PASS / FAIL — attested by `<agent-id>` at commit `<sha>`
- **Raw artefacts:** `<paths under dev/benchmarks/gf2-sim/...>`
```

When the same speedup is also relevant to a phase-level receipt (e.g.
`cpu-foundation-receipts.md`), the phase-level file cross-references the
canonical entry in `parallelism-receipts.md` — no duplicated numbers.

---

## §6 Canonical receipts directory layout

```
dev/benchmarks/gf2-sim/
├── README.md                     -- index of receipts; updated by every phase
├── cpu-foundation-receipts.md    -- per Story A success criterion
├── gpu-stages-receipts.md        -- per Story B success criterion
├── hybrid-executor-receipts.md   -- per Story C success criterion
├── parallelism-receipts.md       -- per parallelism-pays gate (per-task entries)
├── 5g-nr-realtime.md             -- per task 23d3525f success criterion
└── comparison/
    ├── README.md
    ├── dvb-t2-r12-16qam-vs-aff3ct.csv (example name)
    └── 5g-nr-bg1-r12-vs-aff3ct.csv    (example name)
```

Per-task descriptions reference these paths verbatim. If a task body
names a different path, the plan is the SSOT and the task body wins
(escalate the mismatch via the adversarial reviewer rather than auto-
amending).

---

## §7 CLAUDE.md update touchpoints

Updates land progressively, one per story (per the per-story success
criteria). Plan-level expectations:

- **After Phase A** (`bcf7776d` done): add `gf2-sim` to the "Architecture"
  module list and the "Module map" sub-section; document the
  determinism contract; note `parallelism-pays` gate in the gates
  section.
- **After Phase B** (`1f588e2a` done): document the multi-arch HIP
  target list; document the GPU-vs-CPU byte-identity contract.
- **After Phase C** (`9e853c62` done): document the hybrid executor's
  worker/stream pairing model and the failure-mode policy.
- **After Phase D** (`5d0a3fad` done): update the DVB-T2 BICM AWGN
  campaign section to reference the new pipeline; mark
  `dev/benchmarks/dvb_t2_awgn/` legacy with forward-pointer.
- **After Phase E** (`a5635da5` done): document the 5G NR LDPC preset,
  the comparison harness, and the researcher onboarding entry points.

No story autonomously edits the table-of-contents — lead reviews each
landed update and stages a single TOC bump per phase closure.

---

## §8 Resume / handoff conventions

- **Progress file:** `dev/active/f9717e7e-progress.json` per
  project-lead Skill Section 4.6 / 9b.
- **Handoff files:** `dev/active/f9717e7e-handoff-<N>.md` per
  Section 9b; `Traps` section mandatory and forward-carrying.
- **Current planning-and-audit pass:** no progress.json yet (no waves
  dispatched); this plan + the adversarial review summary serve as the
  initial state record for the next session.

---

## §9 Escalation triggers (epic-specific)

In addition to the standard project-lead escalations in
`references/escalation-policy.md`:

| Trigger | Action |
|---|---|
| 5G NR real-time miss after kernel tuning (`23d3525f`) | escalate per memory `feedback_measurements_not_guesses`; propose amendment with data |
| DVB-T2 closure miss after campaign migration (`bbf6b6ee`) | escalate per `feedback_measurements_not_guesses` |
| Multi-GPU / NVIDIA / fading-beyond-Rician request | escalate scope expansion |
| `parallelism-pays` gate failure that doesn't yield to standard optimisation | escalate per `feedback_quality_gates`; do NOT remove the gate |
| Cross-arch HIP dispatch failures on archs we don't have hardware for | document as design-doc seam; do not block on hardware we lack |
| `[hard]` criterion appears falsified by data | escalate with data per `feedback_measurements_not_guesses`; record amendment if user approves |

---

## §10 Out of scope

See epic `f9717e7e` §Non-goals (cited verbatim, not restated): turbo /
feedback-loop decoders, multi-GPU implementation, frequency-selective /
multipath fading, NVIDIA / CUDA backend, encoder GPU offload, no code
change in existing encoders.

---

## §11 Cross-references to existing artefacts

- `dev/benchmarks/dvb_t2_awgn/PLAN.md` — pre-pipeline campaign plan;
  remains valid until Phase D landing adds a closing forward pointer to
  `dev/benchmarks/gf2-sim/cpu-foundation-receipts.md` and `bbf6b6ee`.
- `dev/benchmarks/dvb_t2_awgn/README.md` — pre-pipeline calibration
  smoke results; remains valid history.
- Memory: `feedback_no_autonomous_amendments`, `feedback_measurements_not_guesses`,
  `feedback_quality_gates`, `feedback_parallel_agent_isolation`,
  `feedback_pgrep_self_match`, `feedback_parallel_cargo_ci`,
  `feedback_shell_cwd_persistence`, `feedback_ldpc_shift_tables`,
  `feedback_jit_doc_links`, `feedback_jit_naming`,
  `feedback_post_amendment_sweep`, `feedback_jit_gate_pass_is_atomic`,
  `feedback_code_review_via_jit_cli`, `feedback_sonnet_for_planned_dispatches`,
  `feedback_quote_jit_criteria_verbatim`, `feedback_quote_ssot_formulas_verbatim`,
  `feedback_dispatch_prompt_facts`, `feedback_agents_no_gate_runs`,
  `feedback_jit_claim_before_dispatch`, `feedback_pre_dispatch_criterion_audit`.
- Epic `epic:dvb-t2-awgn-campaign` — standalone; this plan does not
  modify it. Cross-epic dep is `e4849f07` → `bbf6b6ee`.
- Epic `epic:hip-gpu-prototype` (`806eb14e`, backlog) — Phase B may pull
  in-scope tasks; document at Phase B kickoff if pulled.
- Commits to date:
  - `772dc9e3` — epic + Phase 0 task + dual-label `3fcb7025`
  - (next chore) — DAG breakdown: 5 stories + 24 tasks + 32 edges +
    `parallelism-pays` gate + `3fcb7025` amendment

---

## §12 What this planning-and-audit pass produces

1. This plan document (`dev/active/f9717e7e-project-plan.md`), attached
   to the epic via `jit doc add`.
2. The Phase 0 design doc at
   `dev/active/ec530af9-pipeline-design.md`, attached to `ec530af9`.
3. The new leaf task `c0b1702d` (single-thread baseline measurement)
   added to Phase A.
4. All 24 placeholder task bodies expanded to implementer-ready
   detail.
5. Epic body refined to remove duplicated DVB-T2 closure criteria
   (now SSOT-pinned in `e4849f07`), fix the critical-path numbering,
   and concretise the Phase E real-time target.
6. `e4849f07` body refreshed: TS-vs-TR typo corrected and cross-
   reference made explicit.
7. DAG edges added: `5f12e7ff → db9836e4`, `110e45cc → e478daa8`
   (the implied edge to `8c8302c8` is transitively reachable),
   `3fcb7025 → c0b1702d`, `bcf7776d → c0b1702d`, `c0b1702d → ec530af9`.

## §13 Post-audit refinements (2026-06-07)

The 2026-06-07 adversarial-review pass produced 23 findings; the
user-approved autonomous remediations are listed in epic
`f9717e7e`'s description (§"Post-audit refinements"). The
corresponding plan-side updates:

| Audit finding | Plan section | Resolution |
|---|---|---|
| B1, C3 (D wall-clock) | §2 Phase D | `bbf6b6ee` criterion changed to speedup-vs-legacy; wall-clock owned by `e4849f07` |
| B2 (placeholder bodies) | §1 SSOT map | All 24 task bodies expanded; placeholders gone |
| C1 (E real-time undefined) | §2 Phase E | Concrete target pinned: BG1, Z=384, r1/2, BLER ≤ 10⁻², ≥ 200 Mbps |
| C2 (Phase B baseline) | §5 Receipt schema | Dual-baseline reporting required (single-thread + CPU-24-thread); `a930be7f` ≥ 3× CPU-24 |
| C5 (heartbeat missing channel) | §2 Phase A Wave A.3 | Edge `5f12e7ff → db9836e4` added |
| C6 (epic critical-path) | §2 ID-map | Epic body diagram rewritten using story IDs; plan ID-map unchanged |
| H1, H2 (duplicated criteria) | §1 SSOT map | `e4849f07` is now the sole SSOT for the gap and frame-error criteria |
| H4 (110e45cc missing deps) | §2 Phase E Wave E.5 | Edges to `e478daa8` and `8c8302c8` added; moved to Wave E.5 |
| H5 (OOM seam) | §2 Phases B + C | GPU stages signal; executor handles fallback; codified in `ec530af9` design doc §8 |
| H6 (CLAUDE.md owners) | §7 | Folded into each phase's closing task (`48a0db6c`, `14f59c2d`, `0d9cb8e3`, `110e45cc`) |
| H7 (§12 design doc) | n/a | Phase 0 design doc §12 lists every public API that moves vs stays |
| H8 (baseline task) | §2 Phase A Wave A.1 | New task `c0b1702d` added |
| L1 (TS/TR typo) | n/a | Corrected in `e4849f07` body |
| M1 (receipts dir) | n/a | Folded into `118a0091` scaffolding |
| M2 (Wave A.2 conflicts) | §2 Phase A Wave A.2 | Phase 0 design doc §1 pre-allocates module skeleton |
| M5 (CI feature gating) | §2 Phase D Wave D.3 | `0d9cb8e3` body now mandates `#[cfg(feature = "hip")]` |

Project state after this pass: every leaf task body is
implementer-ready; the DAG is sound; the Phase 0 design contract
exists. The epic remains in `backlog` state pending child
completion. The next step, gated on user approval at the Phase 0
review milestone, is to dispatch the Phase 0 design doc review and
begin Wave A.1 + B.1.
