# Task: jit issue a39bb161 — Accelerator execution for campaign cells

Repository: /home/vkaskivuo/Projects/gf2 (Rust workspace, MSRV 1.95).
You are a worker dispatched by the epic-b8206228 execution lead. Read `AGENTS.md`
first; it is the binding engineering contract. Read the issue with
`jit issue show a39bb161` (read-only) for its full Background and Notes.

## Worker constraints (non-negotiable)

- **Do not commit. Do not stage (`git add` in any form is forbidden). Do not
  write `.jit/`.** Leave the tree converged and uncommitted; the lead reviews
  and commits.
- **You have no GPU.** Your sandbox cannot reach the HIP device;
  `hipGetDeviceCount` returns HIP error 100 and
  `gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates` fails. That
  is expected — do not try to fix it, do not report it as a regression, and do
  not weaken any test to make it pass. Every device-touching path you write will
  be verified by the lead on the real host. Design so that everything except the
  actual device call is testable without a device.
- Do not run `./scripts/cargo-ci.sh`, benchmarks, or `--features hip` builds
  that require ROCm linkage unless the command below says to.
- Do not run two cargo invocations concurrently.

## Success criteria (verbatim — checked literally)

- [hard] REQ-01: Accelerator execution splits a shard into launches sized from
  measured per-matrix cost so each launch's wall-clock stays under a configured
  cap, rather than from one batch size held fixed across the grid.
- [hard] REQ-02: The launch-sizing rule is exercised by a test at two sizes whose
  per-matrix costs differ by more than an order of magnitude, confirming the
  chosen launch counts differ accordingly.
- [hard] REQ-03: Accelerator work and timed host work run under the repository's
  benchmark host lock wrapper (`dev/scripts/ccx1-bench-flock.sh`), so two
  concurrent field executions never contend for one device.
- [hard] REQ-04: A cell whose manifest names an accelerator backend on a host
  without the device halts that cell with an error naming the cell and the
  missing device, and never silently falls back to a processor path, since a
  substituted backend would break the frozen selection the dataset records.
- [hard] REQ-05: A build without accelerator support compiles and runs the
  processor-only campaign, with accelerator cells refusing rather than failing
  to build.
- [hard] REQ-06: The simulation crate's `hip` feature forwards to the algebra
  crate's `hip` feature, so the accelerator batch entry points in
  `crates/gf2-algebra/src/gpu.rs` are compiled in exactly when the feature is on;
  today `gf2-sim`'s `hip` feature enables only `gf2-kernels-hip` and
  `gf2-coding/hip`, and the algebra accelerator API is unreachable from the
  campaign.

## Decisions already made by the lead — implement these, do not re-open

**D1 — the measured per-matrix cost is an input, not a manifest field.** Do
**not** add a cost field to `CellSpec` or any other type in
`crates/gf2-sim/src/permanent_campaign/schema.rs`. The frozen root manifest is
owned by a separate in-flight issue (`7a816262`) and is under a standing decision
that it is never edited or resynchronised. The launch-sizing rule therefore takes
the measured per-matrix cost as an explicit parameter, supplied by configuration.

**D2 — the device probe is a gf2-algebra API.** The issue's Notes state the
accelerator reaches the campaign only through the algebra crate's batch entry
points, so the driver never touches the kernel crate directly. `gf2-sim` must not
gain a dependency path to `gf2-kernels-hip` for this. Add a narrow device probe
to `crates/gf2-algebra/src/gpu.rs` behind its `hip` feature and call it from
`gf2-sim`. This is `@/inv/library-first-generality` applied literally.

**D3 — `Backend::Accelerator` resolves through the existing resolver.**
`resolve_processor_path` in `crates/gf2-sim/src/permanent_campaign/schedule.rs`
currently returns `BackendUnavailable` for `Backend::Accelerator`. Give it a new
`ProcessorPath::Accelerator` arm compiled in under `#[cfg(feature = "hip")]`, and
keep the `BackendUnavailable` refusal when the feature is off — that is REQ-05.
Do not restructure `resolve_processor_path`, `ProcessorPath`, or
`evaluate_permanent` beyond adding this arm; they passed review under `032cc754`.

**D4 — REQ-03 is satisfied by invoking the campaign through the wrapper.**
`dev/scripts/permanent-campaign-runner.sh` currently drives the feasibility
harness and the wave-evidence binaries, **not** the `gf2-sim` campaign binary
(`crates/gf2-sim/src/bin/permanent_campaign.rs`), so the campaign binary has no
lock integration today. Add one, invoking the binary through
`"$FLOCK_WRAPPER" --full-host` exactly as `run_campaign` (line ~627) and
`run_premeasure` (line ~991) already do. Do not invent a second lock domain and
do not acquire the flock inside the Rust binary.

## Current state you are building on

- `crates/gf2-algebra/src/gpu.rs` exposes `permanent_batch_bipedal3`,
  `permanent_batch_bipedal5`, `permanent_batch_bipedal7`, each
  `(&[PackedMatrix]) -> Vec<Fp<q>>`, all behind gf2-algebra's `hip` feature.
  Their documented panics include "any HIP runtime call returns a non-zero error
  code — this indicates no device present or a ROCm driver error". **A panic
  cannot satisfy REQ-04**, which requires a named halt. That is why D2 adds a
  probe: check for the device before dispatching, and refuse with a named error.
- `crates/gf2-sim/Cargo.toml`: `hip = ["dep:gf2-kernels-hip", "gf2-coding/hip"]`
  — REQ-06 is the missing `gf2-algebra/hip`.
- `ScheduleError` in `schedule.rs` already carries
  `BackendUnavailable { q, n, backend }` with a `Display` naming the cell and
  the backend. REQ-04 needs a *different* error: the backend is provided, the
  *device* is missing. Add a sibling variant rather than overloading that one.
- The batch execution path and its chunking constant
  `BATCH_CHUNK_MAX_MATRIX_ENTRIES` (`schedule.rs:73`) is the processor-side
  analogue of launch splitting; read it before designing the accelerator split so
  the two read as one convention (`@/inv/convention-convergence`).
- Device-gated code in this crate uses `#[cfg(feature = "hip")]` (see
  `crates/gf2-sim/src/gpu/*.rs`). Follow that.

## Required design

1. **REQ-06 first.** Add `gf2-algebra/hip` to `gf2-sim`'s `hip` feature.

2. **Device probe (D2).** In `crates/gf2-algebra/src/gpu.rs`, behind `hip`, add a
   probe that reports whether a usable accelerator is present without panicking
   — it must return a value, never abort, when no device exists. Give it rustdoc
   stating purpose, the exact condition it reports, and that it does not panic.

3. **Launch sizing (REQ-01/REQ-02).** A pure function, free of any device call
   and unit-testable without hardware:

   ```
   launch_size(per_matrix: Duration, cap: Duration, remaining: u64) -> usize
   ```

   returning `clamp(floor(cap / per_matrix), 1, remaining)`. It must never return
   0. Document the `M·n·2^n/W` reasoning from the issue Background: holding a
   batch size fixed while n rises from 20 to 24 multiplies per-launch occupancy
   by ~19.2, which is why a fixed size is wrong across the grid. A shard is then
   split into successive launches sized by this rule.

4. **Accelerator execution.** Split the shard into launches, dispatch each launch
   through the gf2-algebra batch entry point for that field, and concatenate
   results in input order. The per-matrix cost and the cap come from
   configuration (D1); give the campaign binary a flag for the cap with a
   documented default, and a way to supply the measured per-matrix cost.
   Document in the binary's usage text that the cost is derived from the cell's
   committed measurement receipt.

5. **REQ-04.** Before dispatching any launch for an accelerator cell, probe. If
   no device: return the new error naming the cell `(q, n)` and the missing
   device. Never fall back to a processor path. The issue's Notes already record
   why this is the explicit-failure arm of `@/inv/accelerator-safe-fallback`
   rather than a breach of it — resolve that invariant with
   `jit item show @/invariant/accelerator-safe-fallback` and keep your reasoning
   consistent with the Notes.

6. **Determinism.** Results must not depend on launch size:
   the same shard split into different launch sizes must produce identical
   values in identical order (`@/inv/deterministic-seeded-execution`). Stream and
   matrix assignment must be independent of the split.

## Tests you must add

Work test-first. Name plain `#[test]` functions `test_<operation>_<scenario>`;
property tests keep `prop_*`.

1. `test_launch_size_scales_inversely_with_per_matrix_cost` — REQ-02's required
   test. Two per-matrix costs differing by more than an order of magnitude (e.g.
   50 µs and 1 ms) against one cap; assert the resulting launch counts differ
   accordingly, and assert the direction (the costlier size yields the smaller
   launch). No GPU.
2. `test_launch_size_never_returns_zero` — a per-matrix cost exceeding the whole
   cap still yields 1.
3. `test_launch_size_is_capped_by_remaining_matrices` — never exceeds what is
   left in the shard.
4. `test_resolve_processor_path_refuses_accelerator_without_the_hip_feature` —
   under `#[cfg(not(feature = "hip"))]`, `Backend::Accelerator` yields
   `BackendUnavailable` naming cell and backend. REQ-05.
5. `test_resolve_processor_path_reads_accelerator_with_the_hip_feature` — under
   `#[cfg(feature = "hip")]`, it resolves to the accelerator path.
6. A REQ-04 test asserting the device-absent halt names the cell and the missing
   device, and that no processor path ran. Structure this so it is exercisable
   **without** a device — put the probe behind a seam you can drive in a test,
   mirroring how `run_field_checkpointed_with_evaluator` in `driver.rs` exposes
   the evaluator for conformance tests. State clearly in your report which part
   of REQ-04 your tests cover and which part only the lead's host run can.
7. A determinism test: one shard evaluated at two different launch sizes yields
   identical values in identical order. Drive it through the same seam so it runs
   without a device.
8. A shell test for REQ-03 in `dev/scripts/permanent-campaign-runner.test.sh`
   (follow the existing tests there), asserting the campaign binary is invoked
   through `ccx1-bench-flock.sh --full-host` and never directly.

## Commands to run and report verbatim

From the repository root, one at a time:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo nextest run -p gf2-sim --all-features --release --profile ci`
4. `cargo nextest run -p gf2-algebra --all-features --release --profile ci`
5. `cargo build --workspace --all-features`
6. `cargo doc -p gf2-sim -p gf2-algebra --all-features --no-deps`
7. `bash dev/scripts/permanent-campaign-runner.test.sh`
8. `shellcheck dev/scripts/permanent-campaign-runner.sh` (if shellcheck is present)

REQ-05 needs an explicit no-accelerator check — run it and report it:

9. `cargo build -p gf2-sim` (default features, i.e. no `hip`) and
   `cargo nextest run -p gf2-sim --release --profile ci` (no `--all-features`)

Note that `--all-features` does **not** enable a working ROCm link on your
sandbox; if command 2, 3 or 5 fails specifically on HIP linkage or device
access, report the exact error rather than working around it.

## Report back

1. `git diff --stat` and the list of files touched.
2. For each of REQ-01..REQ-06: the file:line satisfying it and the test proving
   it. Where a criterion is only partly provable without a device, say exactly
   which part remains for the lead's host verification.
3. Verbatim results of every command above.
4. Everything you left unverified or deliberately did not do, and why. Do not
   omit this section.
5. Any adjacent defect you noticed but did not fix.

Leave the tree uncommitted and unstaged.
