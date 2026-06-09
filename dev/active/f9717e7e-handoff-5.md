# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 5

**Date:** 2026-06-09
**Session number:** 5
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-4.md` (s4). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s4 remain in force** — read them, especially the s4 traps on bg3/quiet-host, gate-on-settled-load, and apples-to-apples perf comparators.

## Current state

- Epic `f9717e7e` — `in_progress` (claimed `agent:project-lead`). HEAD ~`63ace574`.
- **PHASE A COMPLETE.** Story `bcf7776d` (CPU foundation) is **done**. Its 4 child tasks (81d05bab, 48a0db6c, c09d3e95, c0b1702d) and all transitive Phase-A tasks are done.
- **DONE this session:** `48a0db6c` (A.4, the Phase-A closer) + closed story `bcf7776d`. Added `dev/benchmarks/gf2-sim/cpu-foundation-receipts.md` (the story criterion-3 rollup).
- **Host:** `bg3` was GONE the entire session — the host was quiet, so the A.4 slow sweep + all gates ran clean. It may return; always re-check before perf work.
- Open escalations: none awaiting input. (One resolved this session — see Traps #4.)

## What happened (session 5) — the A.4 closure was a large detour

A.4 looked like "merged, just run the slow sweep + gates." It became a multi-fix detour because **A.4 is the FIRST task in the epic to carry the `tests` gate** = bare `cargo test` (whole workspace, **multi-threaded, one process per crate**, runs doctests). nextest (which `cargo-ci` uses) isolates every test in its own process and had been masking latent global-state-bleed bugs across the workspace. The `tests` gate exposes them. Fixed, in order:

1. **code-review r1:** the heartbeat-resume parity test did not exercise "SIGINT at frame 100" — it ran a *separate* `max_frames=100` config then hand-cleared `loaded.completed=false` and resumed under `max_frames=200` (different `config_hash`, since the hash includes `max_frames`). Rewrote `assert_resume_parity` to use ONE 200-frame config (stable hash); it trips a **new public `gf2_sim::checkpoint::request_interrupt()`** (programmatic SIGINT equivalent) inside the frame-100 heartbeat-flush callback, then resumes under the same hash without mutating `completed`.
2. **code-review r2:** the 3 resume-parity *integration* tests share the process-wide interrupt flag with no guard → added `static RESUME_PARITY_GUARD: Mutex<()>` in `determinism.rs`. (The 4 checkpoint *unit* tests got `INTERRUPT_FLAG_GUARD` in `checkpoint/mod.rs` for the same reason.)
3. **tests gate (real test failure, not a flake):** `checkpoint_compat::test_sweep_sigint_resume_byte_identical_{awgn,rayleigh,rician}_subprocess` flaked on **fast/idle** hosts — the child finished SNR point 0 before the parent (reading buffered stdout, then `kill -INT`) could interrupt it, so "interrupted point must be mid-point" failed. Added a test-only `--block-at-first-heartbeat` flag to `bin/checkpoint_sweep.rs`: the child parks at its first within-point heartbeat flush until the real SIGINT sets the flag. Applied ONLY to the interrupted child, **NOT** to `base_args` (the `--resume` child must run to completion, not block — that was a self-inflicted hang I had to fix).
4. **tests gate (root cause, USER-ESCALATED):** `gf2-core::field::expr` kernel-trace counters were **process-wide `AtomicU64`** statics bumped by every gemm/axpy across ~15 gf2-core test files; the counting tests' `#[serial]` only mutually-excluded *each other*, not the unrelated bumpers. User decision: **"the gate stays, it must be green, fixing this takes priority."** Made the counters **thread-local** (`Cell<u64>`); `bump()` fires on the calling thread at the dispatch entry (verified no rayon between bump and the test's read), so each test sees only its own kernel calls. Removed the now-redundant `#[serial]` + import, swept stale "atomic"/"process-wide" docs (module header + `Cargo.toml` comment).

Validation: full-workspace `cargo test` 6/6 green; gf2-core lib+doc 5/5; the 3 subprocess tests 4/4 + full `--all-features` ci suite 3/3 at loadavg up to 19; slow determinism sweep 12/12. All 3 A.4 gates green at HEAD `2bc48657`.

## What to do next

- [ ] **Phase B (GPU stages, story `1f588e2a`) — the next wave, ready now.** B.1 (`36075e4c` HIP host infra) + B.2 (`f6004add` GPU AWGN) are already done. Remaining: `a930be7f` (GPU LDPC BP batch decode; perf-gated **≥3× the CPU-24 baseline 21.44 fps** from `cpu-foundation-receipts.md`/`parallelism-receipts.md`), then `d3f1616a` (GPU Gray-QAM demap), `ed575f15` (B.3 OOM auto-fallback/kernel hard-fail), `14f59c2d` (B.4 GPU-vs-CPU byte-identity harness — closes story `1f588e2a`; `mean_iters` EXCLUDED for the GPU comparison per §11/Q3). Single gfx1030 → never run two GPU test suites concurrently; needs a quiet host for the perf gate.
- [ ] **PRE-DISPATCH for `a930be7f`/`d3f1616a` (s4 trap, still critical):** confirm the perf-gate comparator is apples-to-apples (decode-vs-decode, demap-vs-demap) BEFORE the worker writes a receipt. `a930be7f`'s "≥3× CPU-24" must mean the 24-thread DECODE throughput, not full-frame. Escalate to amend if the issue body names a category-confused baseline (f6004add's criterion-3 had to be user-amended for exactly this).
- [ ] Then Phase C (`75c22fa8` scheduler → de160fc5/571c11c4/42eac5cc), D (8c8302c8/bbf6b6ee[unblocks cross-epic e4849f07]/0d9cb8e3), E (acf9b11a[per-i_LS shift tables]/e478daa8/23d3525f[≥200 Mbps real-time]/18e69a1a/110e45cc[epic close]).

## Traps — do not repeat these (NEW this session; s1–s4 traps still apply)

- **The `tests` gate (bare `cargo test`) runs the WHOLE workspace multi-threaded in one process per crate, and runs DOCTESTS.** It exposes any process-wide mutable test state as a nondeterministic flake (nextest/`cargo-ci` masks this via process-per-test). Every future task carrying the `tests` gate is exposed. RULE: any global mutable counter/flag/state touched by tests must be **thread-local** (if bumped on the calling thread) or **mutex-serialized within the test binary** (if a flag set by one test and read by another). Already fixed: gf2-core kernel counters (thread-local), gf2-core `fieldmatrix_new_count` (was already thread-local), gf2-sim interrupt flag (two mutexes). If a NEW `tests`-gate failure names an unrelated crate's count/flag assertion, it's almost certainly this class — fix the state isolation, don't chase the symptom.
- **The `tests` gate ALSO runs doctests** (`cargo test --doc`), which `cargo-ci`/nextest do NOT. A broken `# Examples` passes cargo-ci but fails `tests`. Validate new public-API doctests with `cargo test --doc -p <crate>` before gating.
- **Subprocess SIGINT/timing tests flake OPPOSITE to load** — they fail on FAST/idle hosts (child outruns the parent's buffered-read + signal), pass under load. Do NOT "fix" by waiting for load; that masks it. The deterministic fix is `--block-at-first-heartbeat` (child parks until the real signal). A perf/throughput flake fails under load; a signal-timing flake fails when idle — diagnose which before reacting.
- **`request_interrupt()` is now public** (`gf2_sim::checkpoint`) — the programmatic SIGINT equivalent (sets the same flag the `ctrlc` handler does). Use it for in-process deterministic interrupt tests; pair with `clear_interrupt()` before any subsequent run, and serialize flag-touching tests within a binary.
- **gf2-core kernel-trace counters are now THREAD-LOCAL** (`field::expr`). A test reading `kernel_counts()` sees only its own thread's bumps; no `#[serial]` needed. Do NOT reintroduce process-wide atomics for them. `inverse/triangular/ple` still carry `#[serial]` (defensive/redundant — their `fieldmatrix_new_count` is also thread-local); leave them.
- **Doc-only changes to a foundational crate (gf2-core) force a full downstream rebuild** (gf2-coding/gf2-sim recompile). Budget for it; pre-warm before gating. A one-word doc fix cost two full warm cycles this session.
- Carry-forward (still in force): pre-warm `cargo-ci.sh` after every merge/edit before gating; gate only on SETTLED loadavg (the post-build tail flakes 5 s nextest); gate BARE (never `| tail`); `jit gate pass` via the MCP/CLI runs the auto-checker atomically and returns FAIL on findings; restore lead-owned `.jit`/progress.json after a worker merge; targeted `-p` builds; the worktree-dispatch scripts the skill references don't exist (hand-roll); 5G NR LDPC needs per-i_LS shift tables (`acf9b11a`, `feedback_ldpc_shift_tables`); apples-to-apples perf comparators for a930be7f/d3f1616a.

## Open questions needing user input

None.

## Reference artefacts

- Epic: `jit issue show f9717e7e`. Design doc: `dev/active/ec530af9-pipeline-design.md` (§3 seek; §4 v2 schema; §6 HIP; §8 fallback/OOM; §11 determinism contract).
- Project plan: `dev/active/f9717e7e-project-plan.md` (§5 receipt schema, §7 CLAUDE.md touchpoints). Progress: `dev/active/f9717e7e-progress.json`.
- Receipts: `cpu-foundation-receipts.md` (Phase-A rollup), `parallelism-receipts.md` (3fcb7025 CPU-24 21.44 fps/13.22×; f6004add GPU AWGN 14.91×), `baseline-single-thread.md` (1.6216 fps).
- Key A.4 surfaces: `crates/gf2-sim/tests/{determinism.rs,common/mod.rs,checkpoint_compat.rs}`, `src/checkpoint/mod.rs` (`request_interrupt`, the two test mutexes), `src/bin/checkpoint_sweep.rs` (`--block-at-first-heartbeat`), `crates/gf2-core/src/field/expr.rs` (thread-local counters).
