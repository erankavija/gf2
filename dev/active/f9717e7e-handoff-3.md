# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 3

**Date:** 2026-06-08
**Session number:** 3
**Prior handoffs:** `f9717e7e-handoff.md` (s1), `f9717e7e-handoff-2.md` (s2). Progress file: `dev/active/f9717e7e-progress.json`. **All traps from s1 + s2 remain in force** — read them.

## Current state

- Epic `f9717e7e` — `in_progress` (claimed `agent:project-lead`).
- Phase A is nearly complete. **Closed THIS session:** `3fcb7025` (A.2b within-SNR parallelism), `db9836e4` (A.2c CPU channel stages). Already done before: ec530af9, 118a0091, c0b1702d, 19ae6540, 36075e4c (B.1).
- **`81d05bab` (A.2d DVB-T2 typestate preset): CODE MERGED to main** (merge `3669ac65`; HEAD now `80fffd95`). cargo-ci GREEN. NOT formally closed — cannot go `in_progress` (DAG-blocked by c09d3e95) and its own code-review gate not yet run. Worktree `agent-81d05bab` still present (HEAD 607ee995, fully merged).
- **`c09d3e95` (graph API): held since s1; crit-1 NOW SATISFIED** by `81d05bab`'s `tests/preset_vs_graph.rs` (the codex reviewer explicitly confirmed this 2026-06-08). BUT code-review FAILED on **two NEW graph bugs** (see What-to-do-next). cargo-ci GREEN. Worktree `agent-c09d3e95` present.
- Machine: AMD Ryzen 9 5900X, single gfx1030 GPU. Was quiet this session.

## What just happened (session 3)

- **3fcb7025 CLOSED.** Clean quiet-host throughput re-measure: 24-thread **21.44 fps ±0.22 → 13.22×** (≥12× gate). Receipt INVALID banner removed (`4002584a`). All 3 gates green (cargo-ci, parallelism-pays attested, code-review codex PASS). Worktree removed.
- **db9836e4 CLOSED.** AWGN/Rayleigh/Rician `Stage` impls. 2 code-review rounds. Round 2 had 2 findings rooted in a STALE deliverable-1 (it demanded `Stage::process()` take a `WorkerCtx`, IMPOSSIBLE vs the approved design-doc §1 trait shipped by done task 19ae6540). **USER-APPROVED AMENDMENT 2026-06-08** corrected only the impossible API sketch (seek owned by `apply_for_frame`; `Stage::process` consumes pre-seeked scratch) — NO criterion relaxed. The two genuine findings were FIXED: un-ignored the {1,4,24} byte-identity proptest (measured 0.013s → enforced fast gate), and rewrote stats to genuine 10000-FRAME mean+variance + Rician LOS mean. All gates green. Worktree removed.
- **81d05bab merged.** Opus worker. Typestate builder (`NeedsModcod→NeedsDecoder→NeedsDemap→NeedsChannel→Ready` via `with_state`+PhantomData), `Modcod::Normal{rate,mod}`+`validate` (6 in-scope), `Channel::awgn`, `Pipeline::dvb_t2()` entry, trybuild compile-fail (`tests/compile_fail/wrong_order.rs`+`.stderr`), and `tests/preset_vs_graph.rs` proving STRUCTURAL + driven-output BYTE-IDENTITY (channel scratch seeded identically, channel identified as the unique SymbolBatch→SymbolBatch stage). Lead pre-gate fix: `InvalidModcod` was misreporting out-of-scope rates via lossy NR placeholders → worker changed it to `{ rate: String, modulation: String }` reporting true values, deleted the `NrRate`/`Modulation` placeholder enums (error.rs doc had assigned them to 81d05bab).
- **Lead-direct fix `80fffd95`:** the trybuild compile-fail test (`typestate_rejects_out_of_order_calls`) TIMED OUT at the 5s fast-tier nextest hard-kill in the full `--all-features` run (trybuild spawns a fresh cargo compile; ~14.5s). Added a per-test nextest `ci`-profile timeout override (90s) in `.config/nextest.toml` — keeps it ENFORCED in the fast gate, no other test's budget changed.

## What to do next (the closure chain — DO IN ORDER)

1. **Fix the two c09d3e95 graph bugs** (in `crates/gf2-sim/src/graph/mod.rs`). Dispatch a worker (Sonnet ok; well-scoped) in a worktree:
   - **(a) `build()` leaves stale edge StageIds after topo reorder.** `ordered_stages` is built in topo `order` (graph/mod.rs:569-576) but `ordered_edges` keeps the ORIGINAL `Edge{from,to}` StageIds (:590-594) — NOT remapped. For out-of-topo-order insertion, `Pipeline::edges()` ids no longer index `Pipeline::stages()`. (DVB-T2 chains add in topo order so `order`==identity and tests pass — but the general graph API is broken.) FIX: remap each edge's `from`/`to` to the post-sort position via an `old_id → new_index` map derived from `order`, so `Pipeline::edges()` from/to are indices into `Pipeline::stages()`. Add a regression test that adds stages OUT of topo order, connects, builds, and asserts edges index the right stages.
   - **(b) Fallback validation gaps.** `build()` collects fallbacks into `HashMap<StageId,_>` (:578-587) so duplicate GPU registrations with distinct CPU fallbacks SILENTLY OVERWRITE; and there's no check that the CPU fallback has the same input/output batch types as the GPU stage. FIX: in `build()` return `BuildError` for a duplicate GPU-stage fallback registration, and validate fallback input/output type-compatibility (return a `BuildError` variant — reuse `TypeMismatch` or add one). Add tests.
   - Keep `#![deny(unsafe_code)]`, doc accuracy (no fabricated `# Errors`/`# Panics`), clippy clean. Both bugs are real (codex-confirmed); do NOT amend c09d3e95 criteria.
2. **Merge the c09d3e95 fix → pre-warm `./scripts/cargo-ci.sh` → gate `c09d3e95` cargo-ci then code-review (FOREGROUND).** crit-1 is already satisfied (preset_vs_graph.rs); once the two bugs are fixed, code-review should pass. Then `jit issue update c09d3e95 --state done`.
3. **c09d3e95 done unblocks 81d05bab.** Then `jit issue claim 81d05bab agent:claude` + `jit issue update 81d05bab --state in_progress` (now allowed) → gate `cargo-ci` then `code-review` (FOREGROUND) → if PASS `jit issue update 81d05bab --state done`. (81d05bab's code is already in main + cargo-ci green; just needs the formal code-review gate run.) Remove worktrees `agent-81d05bab` and `agent-c09d3e95`.
4. **Then Phase A finish:** A.3 `5f12e7ff` (heartbeat-checkpoint v2; needs 3fcb7025✓ + db9836e4✓) → A.4 `48a0db6c` (CPU determinism property suite + **CLAUDE.md determinism-contract update**; needs db9836e4✓ + 5f12e7ff). These close story `bcf7776d` (Phase A).
5. **Phase B is parallel + ready:** `f6004add` (GPU ChaCha20+Box-Muller AWGN) is `ready` (dep 36075e4c done) and its CPU-24 receipt coupling is now SATISFIED (3fcb7025 receipt valid, 13.22×). Then `a930be7f` (GPU LDPC BP, ≥3× CPU-24), `d3f1616a` (GPU demap), `ed575f15` (B.3 OOM), `14f59c2d` (B.4 close). **Single gfx1030 → never run two GPU test suites concurrently; perf-gated tasks need a quiet machine.** f6004add MUST match the §3 seek scheme exactly (QPSK worst case, 32-bit words, see s2 trap).
6. Then C (75c22fa8 scheduler → de160fc5/571c11c4/42eac5cc), D (8c8302c8/bbf6b6ee/0d9cb8e3), E (acf9b11a/e478daa8/23d3525f/18e69a1a/110e45cc).

## Traps — do not repeat these (NEW this session; s1+s2 traps still apply)

- **The user's standing directive (2026-06-08): "be more careful; the reviewer is right not to pass unfulfilled criteria."** Do NOT amend a `[hard]` criterion to make a red gate green when the work is genuinely incomplete. Amend ONLY when a criterion is literally IMPOSSIBLE vs already-approved architecture (like db9836e4's `process()`-takes-`WorkerCtx` vs the design-doc §1 trait) — and even then, amend only the impossible mechanism, fix everything else for real, and get user approval. When in doubt, fix the code, don't touch the criterion.
- **A backgrounded `jit gate pass` reports wrapper exit 0 even when the gate FAILS** (re-confirmed: db9836e4 round-2 reported "exit 0" while code-review FAILED). ALWAYS read the real verdict via `jit issue show <id> --json` gates_status or `jit gate check-all`. Prefer running code-review FOREGROUND.
- **trybuild tests blow the 5s fast-tier nextest limit** under the full `--all-features` workspace (they spawn a fresh cargo compile, ~14.5s). The fix is a per-test `[[profile.ci.overrides]]` timeout in `.config/nextest.toml` (done for `typestate_rejects_out_of_order_calls`), NOT `#[ignore]` (which would un-enforce it). Any future trybuild test needs the same override.
- **Workers run targeted `-p gf2-sim`, which MISSES failures that only appear in the full `--all-features` cargo-ci run** (the trybuild timeout, and potentially feature-gated test failures). ALWAYS pre-warm `./scripts/cargo-ci.sh` (full workspace) after a merge BEFORE gating — it catches these. (The trybuild timeout was invisible to the worker's `-p gf2-sim` run.)
- **`c09d3e95`↔`81d05bab` is a circular coupling**: 81d05bab DAG-depends on c09d3e95, but c09d3e95's crit-1 is closed by 81d05bab's `preset_vs_graph.rs`. JIT BLOCKS claiming/transitioning 81d05bab while c09d3e95 is not-done. Resolution (no DAG edit needed): merge 81d05bab's work → close c09d3e95 FIRST (crit-1 now satisfiable) → that unblocks 81d05bab → close it. Dispatch 81d05bab's WORK in its worktree WITHOUT a JIT claim (the state is just bookkeeping; the lead does all transitions at close).
- **The codex code-review re-reviews ALL of an issue's code on every run and surfaces DEEPER findings each round** (c09d3e95: prior rounds caught crit-1/panic; this round caught the topo-reorder edge-remap bug + fallback validation gaps that 5 prior rounds missed). Budget for this; the lead must holistically pre-audit (project-lead Tier 1.5/2.75) before each gate.

## Open questions needing user input

None currently. (One escalation resolved this session: db9836e4 criteria-vs-architecture conflict → user approved amending only the impossible API sketch.)

## Reference artefacts

- Epic: `jit issue show f9717e7e`; Design doc: `dev/active/ec530af9-pipeline-design.md` (§1 Stage trait, §3 seek, §6 HIP, §8 fallback/OOM, §9 layered API/typestate+graph examples — NB §9 uses some aspirational names that DIFFER from landed: `stages::channels::AwgnStage`→ actual `channels::Awgn`; `Pipeline::builder()`→ actual `Pipeline::dvb_t2()`).
- Project plan: `dev/active/f9717e7e-project-plan.md`; Progress: `dev/active/f9717e7e-progress.json`.
- Receipts: `dev/benchmarks/gf2-sim/parallelism-receipts.md` (3fcb7025: 13.22× clean), `baseline-single-thread.md` (1.6216 fps).
- Latest c09d3e95 code-review (the topo-reorder + fallback findings): newest `.jit/gate-runs/*/result.json` with `issue_id: c09d3e95` + `gate_key: code-review` (run `6cb02f88`, commit `80fffd95`).
- Key landed surfaces for graph/preset work: `crates/gf2-sim/src/graph/mod.rs` (Chain), `src/stages/mod.rs` (`dvb_t2_bicm_stages` factory → {forward,inverse,codec}), `src/presets/dvb_t2.rs` (typestate), `tests/preset_vs_graph.rs` + `tests/dvb_t2_chain_via_graph.rs` (the `run_pipeline_stages`/`process_any` drive pattern), `src/stage.rs` (Stage/AnyStage/erase).
