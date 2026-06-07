# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 1

**Date:** 2026-06-07
**Session number:** 1 (first f9717e7e-named handoff; prior session state lived in `f9717e7e-progress.json` `paused`/`resume_2026_06_07` blocks)
**Prior handoffs:** None for f9717e7e. Progress file is the prior record: `dev/active/f9717e7e-progress.json`.

## Current state

- Epic: `f9717e7e` — state: `in_progress` (claimed `agent:project-lead`)
- Wave in progress: A.2b / B.2 boundary. Phase 0 + A.1 + A.2a + B.1 complete.
- Children summary: done = 118a0091, c0b1702d, 19ae6540, ec530af9, **36075e4c** (B.1); held-open in_progress = **c09d3e95**; remaining pending = 3fcb7025 (A.2b), db9836e4 (A.2c), 81d05bab (A.2d), 5f12e7ff (A.3), 48a0db6c (A.4), f6004add/a930be7f/d3f1616a (B.2), ed575f15 (B.3), 14f59c2d (B.4), all of C/D/E.
- Active claims: `36075e4c` (agent:claude, now done — release if stale), `c09d3e95` (agent:claude, held open), epic (agent:project-lead).
- Open escalations: None awaiting input (two resolved this session — see below).
- Progress file: `dev/active/f9717e7e-progress.json` — `resume_2026_06_07` block has the authoritative next-actions + process-reminders.

## What just happened

- Fixed rust-analyzer LSP crash (pre-skill): `rustup component add rust-analyzer` (the `~/.cargo/bin/rust-analyzer` proxy had no component behind it).
- Resumed the two paused workers per the inherited plan.
- **36075e4c (HIP host infra)**: merged `a8c8361f` → `8a38c3e7` (lead-owned `.jit/`+progress.json restored to main). Adversarial pre-review PASS. code-review gate FAIL — round-6 fixed the OOM *preflight* (`device_mem_info_for`) but `DeviceBuffer::new` still `hipMalloc`'d on the *current* device, contradicting its docs. Escalated (rework past MAX). User chose lead fix → `8f990d15`: `select_device`/`restore_device` helpers, `new` scopes the alloc to `device_id`, buffer bound-then-freed on the rare restore-fail path. Re-pre-review PASS; cargo-ci + code-review GREEN. **DONE.** Unblocks B.2.
- **c09d3e95 (graph API)**: merged `4be31a38` → `85d8fd9a`. Added DVB-T2 BICM rustdoc example (`9d10ed18`, deliverable 4; validated via `cargo test --doc`, 2.56s). cargo-ci PASS. code-review FAIL on 2 findings: (1) criterion-1 references downstream preset `81d05bab` (a stub) → escalated; user chose **hold c09d3e95 open**, do NOT amend criterion. (2) `build()` panicked on malformed fallback regs → fixed `659b2249` (up-front validation → `BuildError::Disconnected`; 2 regression tests). **HELD OPEN (in_progress).**
- Recorded the `f6004add`↔`3fcb7025` CPU-24-receipt coupling (see Traps).

## What to do next

- [ ] **Dispatch `3fcb7025` (A.2b) — the true critical path.** It supplies the `WorkerCtx`/seek primitives A.2c (`db9836e4`) needs AND the CPU-24-thread baseline receipt every B.2 GPU receipt references. Perf-gated (`parallelism-pays`, ≥12× single-thread vs 1.6216 fps baseline from c0b1702d) → run on a **confirmed-quiet machine**. Opus model; quote success criteria verbatim; reference design-doc §3 (`worker_offset` seek formula, `ec530af9-pipeline-design.md:287-340`) and project-plan §5–§6 receipt schema. Receipt path per its criterion: `dev/benchmarks/gf2-sim/parallelism-receipts.md` (NB naming wrinkle vs plan's `cpu-foundation-receipts.md` — confirm before the worker writes it).
- [ ] After 3fcb7025: A.2c `db9836e4` (channels; needs 3fcb7025 WorkerCtx + 19ae6540 batch types).
- [ ] A.2d `81d05bab` (preset) — its `tests/preset_vs_graph.rs` satisfies c09d3e95 criterion-1. Resolve the c09d3e95↔81d05bab coupling here (see Traps): close c09d3e95 together with / right after 81d05bab once preset_vs_graph byte-equality is proven and c09d3e95's code-review re-gates green.
- [ ] B.2 GPU kernels (`f6004add`, `a930be7f`, `d3f1616a`) — only after 3fcb7025's CPU-24 receipt exists. Single GPU (gfx1030) → do NOT run multiple GPU workers' test suites concurrently. a930be7f + f6004add are perf-gated.
- [ ] Release stale `agent:claude` claim on 36075e4c if it lingers.

## Traps — do not repeat these

- **Do NOT dispatch `f6004add` (or `a930be7f`) before `3fcb7025` is done.** f6004add deliverable-5 / criterion-3 require the receipt to report GPU throughput vs **3fcb7025's CPU-24-thread baseline**, which does not exist until 3fcb7025 lands. The only *formal* JIT dep on f6004add is 36075e4c (done), so the DAG will NOT stop you — the coupling is soft and must be enforced by dispatch order. Dispatching early forces the worker to fabricate a missing baseline (same class as the c09d3e95 criterion-1 trap below). Evidence: `jit issue show f6004add` Deliverables §5 + Success Criteria bullet 3.
- **Do NOT mark `c09d3e95` done until `81d05bab`'s `preset_vs_graph.rs` exists.** Its `[hard]` criterion-1 ("build() matches the typestate-builder preset output") names downstream artifact `81d05bab`. The formal code-review gate REJECTED self-satisfying it against DvbT2Concat alone (even though the deferral was documented in the test) — ruling a `[hard]` criterion cannot be silently reassigned to a consumer issue. User chose to HOLD, not amend. COUPLING HAZARD: 81d05bab DAG-depends on c09d3e95, so c09d3e95 staying not-done will block 81d05bab's availability at A.2d. Resolve by treating 81d05bab's preset+equality work as the unit that finally closes c09d3e95 (or temporarily wire around the dep). Evidence: gate run `.jit/gate-runs/fba95100-*/result.json`; `crates/gf2-sim/tests/dvb_t2_chain_via_graph.rs:11-25`.
- **Do NOT trust the adversarial pre-reviewer as an oracle.** It returned PASS on BOTH 36075e4c and c09d3e95, yet the formal gate then caught a real device-scoped-alloc bug (36075e4c) and a real `build()` panic (c09d3e95). Treat the mandated pre-review as risk-reduction, not a guarantee; the formal gate is authoritative. Always still run it (it does front-load many findings).
- **`cargo nextest` / `cargo-ci` do NOT run doc tests.** A broken `# Examples` block will pass cargo-ci and only fail `cargo test --doc`. Validate any new/changed doc example with `cargo test --doc -p <crate>` BEFORE gating. (Caught proactively on the c09d3e95 DVB-T2 doc example this session.)
- **The skill's worktree-dispatch scripts are MISSING.** `scripts/dispatch-worker-worktree.sh` and `scripts/check-leak-into-main.sh` (referenced by project-lead Section 5/6) do not exist in this repo. For parallel worktree dispatch you must hand-roll: `git worktree add .claude/worktrees/agent-<id> -b worktree-agent-<id> main` anchored to current `main` HEAD, verify the SHA, dispatch the Agent WITHOUT `isolation:"worktree"` (it was observed to branch from a stale ancestor), and after the wave manually verify no worker files leaked into main's tree (`git -C <main> status`). Consider creating these scripts.
- Carry-forward (still in force, from progress.json `process_notes`): pre-warm `./scripts/cargo-ci.sh` after every merge before gating (cold HIP build > 300s gate timeout); gate BARE (never `| tail` — masks exit code); restore lead-owned `.jit/`+progress.json to main HEAD after a worker merge; workers use targeted `-p` builds (disk at 93%, ~65 GB free); 5G NR LDPC needs per-i_LS shift tables (`acf9b11a`, `feedback_ldpc_shift_tables`).

## Open questions needing user input

None. (Two escalations resolved this session: 36075e4c device-scoping fix → lead applies; c09d3e95 criterion-1 → hold open.)

## Reference artefacts

- Epic: `jit issue show f9717e7e`
- Design doc: `dev/active/ec530af9-pipeline-design.md` (§3 seek scheme, §6 HIP dispatch, §8 fallback/error mapping, §9 graph API)
- Project plan: `dev/active/f9717e7e-project-plan.md` (§2 run-book, §5–§6 receipt schema)
- Progress file: `dev/active/f9717e7e-progress.json`
- Baseline receipt (c0b1702d): `dev/benchmarks/gf2-sim/baseline-single-thread.md` (1.6216 fps single-thread)
- Latest failing c09d3e95 review: `.jit/gate-runs/fba95100-d40a-4132-bf8e-c82e85c410b3/result.json`
