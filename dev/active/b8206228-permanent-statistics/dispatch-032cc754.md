# Task: jit issue 032cc754 — Processor backend selection for campaign cells

Repository: /home/vkaskivuo/Projects/gf2 (Rust workspace, MSRV 1.95).
You are a worker dispatched by the epic-b8206228 execution lead. Read
`AGENTS.md` first; it is the binding engineering contract (architecture
boundaries, correctness/test policy, invariants). Follow it literally.

## Worker constraints (non-negotiable)

- **Do not commit. Do not stage anything (`git add` of any form is forbidden).**
  Leave the converged tree uncommitted; the lead reviews the diff and commits.
- **Do not touch `.jit/` in any way** — no `jit` state mutation, no gate
  evaluation, no issue transitions. Reading issue text is fine.
- Touch only the files named below plus whatever compile errors force you to
  fix. If you believe another file must change, say so in your report instead
  of widening the change silently.
- Do not run GPU work, benchmarks, or `./scripts/cargo-ci.sh` (host-locked; the
  lead runs it). Do not run two cargo invocations concurrently.

## Success criteria (verbatim from the issue — these are checked literally)

- [hard] REQ-01: A cell's backend is read from the campaign manifest's
  per-$(q,n)$ table, and no code path selects a backend from a value observed
  during the run.
- [hard] REQ-02: The scalar single-word path is selectable for $q \in \{3,5,7\}$
  where the manifest names it.
- [hard] REQ-03: The batch-parallel path is selectable for $q \in \{3,5,7\}$
  where the manifest names it.
- [hard] REQ-04: The intra-matrix parallel path is selectable for $\mathbb{F}_3$
  where the manifest names it.
- [hard] REQ-05: The generic Ryser path is selectable for any $(q,n)$ the packed
  kernels do not cover, so a cell above the packed $\mathbb{F}_7$ ceiling has a
  correct processor path.
- [hard] REQ-06: A backend the manifest names but the build does not provide
  halts that cell with an error naming the cell and the missing backend, rather
  than substituting another backend and continuing.

Issue notes (context, already settled — do not re-litigate): halting on an
unavailable backend is the explicit-failure arm of
`@/inv/accelerator-safe-fallback`, not a breach of it. Cross-backend
behavioural agreement is certified by a separate shared conformance suite leaf,
not re-asserted here.

## Current state and the gaps you must close

All work is in `crates/gf2-sim/src/permanent_campaign/schedule.rs` unless noted.

The manifest side is already correct: `WorkItem.backend` is copied from
`CellSpec.backend` at `schedule.rs:377`, and `CellSpec.backend` is the frozen
per-$(q,n)$ table entry (`schema.rs:542` `enum Backend { Scalar, BatchParallel,
IntraMatrixParallel, GenericRyser, Accelerator }`). Execution dispatch is
`evaluate_permanent` (`schedule.rs:~549-583`) plus the batch/serial split in
`run_shard_for_with_observer_with_worker_count` (`schedule.rs:~807-830`).

Confirmed defects:

1. **REQ-04 is unmet.** `evaluate_permanent` aliases
   `(3, Backend::IntraMatrixParallel, PackedMatrix::F3(m))` onto
   `permanent_bipedal3(m)` — the *serial* single-word kernel. The intra-matrix
   parallel kernel `gf2_algebra::permanent::permanent_bipedal3_parallel`
   (crates/gf2-algebra/src/permanent/parallel_bipedal3.rs:102, behind
   gf2-algebra's default-on `parallel` feature) is never reached. A manifest
   naming `IntraMatrixParallel` silently runs the scalar path today.

2. **REQ-06 is unmet, and the same aliasing hides it.**
   `(5, IntraMatrixParallel, F5)` and `(7, IntraMatrixParallel, F7)` are aliased
   onto the F_5/F_7 scalar kernels. There is no intra-matrix parallel
   implementation for F_5 or F_7 anywhere in the workspace, so those cells
   substitute a different backend and continue — exactly what REQ-06 forbids.
   The `Backend::Accelerator` arm errors, but its message
   ("accelerator backend is layered above the scheduler") names neither the cell
   nor the backend.

3. **Kernel-ceiling cells panic instead of halting with a named error.**
   `permanent_bipedal7` asserts `n <= Packed7::LANES` (= 16;
   crates/gf2-algebra/src/permanent/bipedal7.rs:110-124) and `permanent_bipedal5`
   asserts `n <= 63` (bipedal5.rs:108-125). A manifest naming `Scalar` or
   `BatchParallel` at $q=7, n>16$ aborts the process with an assertion instead of
   halting that cell with a named error.

4. **REQ-02 fidelity.** `Backend::Scalar` currently routes through the
   `permanent_bipedal3` wrapper, which itself dispatches on `n` (`n <= 63` →
   `permanent_bipedal3_singleword`, else the multiword path). The criterion names
   the *single-word* path, and the premeasurement cohort that froze these
   selections measured `cpu_scalar` = `permanent_bipedal3_singleword` directly
   (see the same reasoning recorded at
   `dev/research/permanent-sampling-feas/src/backend.rs:1-26`). Call the
   single-word entry points directly and range-check `n` yourself.

## Required design

Replace the ad-hoc `(Q, backend, packed)` match with an explicit two-step
resolve-then-execute, so that "which kernel this cell runs" is a value that can
be asserted in a test rather than an unobservable branch:

1. Add a crate-visible `enum ProcessorPath` (in `schedule.rs`) with one variant
   per concrete kernel actually reachable:
   `Bipedal3SingleWord`, `Bipedal5SingleWord`, `Bipedal7SingleWord`,
   `Bipedal3IntraMatrixParallel`, `GenericRyser`.
   Derive `Debug, Clone, Copy, PartialEq, Eq`.

2. Add `fn resolve_processor_path(q: u8, n: u16, backend: Backend) ->
   Result<ProcessorPath, ScheduleError>`. It is a **pure function of the frozen
   cell triple** — it must not read the clock, the host, any measurement, or any
   sampled matrix. Mapping:
   - `Scalar` → the field's single-word path, subject to that kernel's
     documented `n` ceiling (F_3: `n <= 63`; F_5: `n <= 63`; F_7: `n <= 16`).
   - `BatchParallel` → the same per-field single-word path (batch parallelism is
     rayon *across* matrices with the scalar kernel per matrix — this is the
     measured `cpu_rayon_batch_scalar` backend); same ceilings.
   - `IntraMatrixParallel` → `Bipedal3IntraMatrixParallel` for `q == 3` and
     `n <= 63`; **error for `q == 5` and `q == 7`** (no such implementation) and
     for `q == 3, n > 63`.
   - `GenericRyser` → `ProcessorPath::GenericRyser` for every `q` and every `n`
     the generic Ryser routine accepts (`permanent_ryser<F: FiniteField>`,
     crates/gf2-algebra/src/permanent/ryser.rs:92). This is the arm REQ-05 names:
     `q = 7, n = 24` must resolve here and evaluate correctly.
   - `Accelerator` → error (this scheduler is the processor half; the
     accelerator half is a separate issue, `a39bb161`). The error must still
     name the cell and the backend.

3. Add `ScheduleError::BackendUnavailable { q: u8, n: u16, backend: Backend }`
   with a `Display` that names both the cell and the backend, e.g.
   `cell q=7 n=24 names backend intra_matrix_parallel, which this build does not
   provide`. Fix every exhaustive `match` on `ScheduleError` that this breaks.

4. Add `Backend::name(self) -> &'static str` (in `schema.rs`) returning the
   snake_case token (`scalar`, `batch_parallel`, `intra_matrix_parallel`,
   `generic_ryser`, `accelerator`) and use it in the error `Display`. Do **not**
   hand-maintain a second vocabulary: add a test asserting `name()` agrees with
   `serde_json::to_value(backend)` for every variant, so the two cannot drift
   (`@/inv/single-source-prose`).

5. Rewrite `evaluate_permanent` to take a resolved `ProcessorPath` and execute
   it. Resolve **once per shard**, before the matrix loop, not per matrix. Both
   the serial loop (`schedule.rs:~549-583`) and the batch loop
   (`schedule.rs:~688-700`) consume the same resolved path; delete the
   `evaluate_permanent(Backend::Scalar, ...)` hard-coding at `schedule.rs:698`
   and pass the resolved path instead.

6. Skip the `PackedMatrix::new` construction when the resolved path is
   `GenericRyser` (it consumes the row-major operand and never the packed one).
   Keep `PhaseDurations.pack` semantics coherent: a Ryser cell records zero pack
   time. Update the `PhaseDurations::pack` rustdoc if it claims packing always
   happens.

7. `run_shard_for_with_observer_with_worker_count` keeps its
   `item.backend == Backend::BatchParallel` branch for *how* matrices are
   distributed; the resolved `ProcessorPath` decides *which kernel* each matrix
   runs. Keep these two concerns separate and say so in the rustdoc.

Preserve the existing determinism contract: identical results across worker
counts and across the serial/batch split (`@/inv/deterministic-seeded-execution`).
Do not change the default sampling, streaming, or emission behaviour in any way;
this issue changes kernel routing only.

## Tests you must add

Work test-first: write each test, watch it fail against the current routing,
then implement. Put unit tests in the existing `#[cfg(test)] mod tests` in
`schedule.rs`. Name plain `#[test]` functions `test_<operation>_<scenario>`
(owner-recorded convention amendment); keep any proptest named `prop_*`.

1. `test_resolve_processor_path_reads_scalar_for_each_field` — `(3,20,Scalar)`,
   `(5,20,Scalar)`, `(7,12,Scalar)` resolve to the three single-word variants.
   (REQ-02)
2. `test_resolve_processor_path_reads_batch_parallel_for_each_field` — the same
   three cells with `BatchParallel` resolve to the same single-word kernels.
   (REQ-03)
3. `test_resolve_processor_path_reads_intra_matrix_parallel_for_f3` —
   `(3,20,IntraMatrixParallel)` resolves to `Bipedal3IntraMatrixParallel`, and it
   is **not** equal to the `(3,20,Scalar)` resolution. (REQ-04 — the inequality
   assertion is the one that fails today.)
4. `test_resolve_processor_path_reads_generic_ryser_above_the_f7_ceiling` —
   `(7,24,GenericRyser)` resolves to `GenericRyser`. (REQ-05)
5. `test_run_shard_evaluates_generic_ryser_above_the_f7_ceiling` — run an actual
   shard at `q=7, n=17..=18`, `GenericRyser`, a small `matrix_count` (≤ 8), and
   assert every permanent matches `permanent_ryser` recomputed on the same
   operands through the `run_shard_for_with_observer` seam. Must stay inside the
   5 s fast-tier per-test kill in release mode; if it cannot, mark it
   `#[ignore = "slow: ..."]` and say so in your report. (REQ-05, end to end.)
6. `test_resolve_processor_path_halts_for_intra_matrix_parallel_on_f5_and_f7` —
   both error with `BackendUnavailable`, and the rendered `Display` string
   contains the `q`, the `n`, and the backend token. (REQ-06)
7. `test_resolve_processor_path_halts_above_the_packed_f7_ceiling` —
   `(7,24,Scalar)` and `(7,24,BatchParallel)` error rather than panic. (REQ-06)
8. `test_resolve_processor_path_halts_for_accelerator` — errors naming cell and
   backend. (REQ-06)
9. `test_work_item_backend_is_copied_from_the_manifest_cell` — build a manifest
   whose cells carry two different backends, enumerate work items, and assert
   each item's backend equals its cell's; assert no work item carries a backend
   absent from the manifest. (REQ-01)
10. `test_backend_name_agrees_with_serialized_token` — the `name()`/serde
    agreement test from design point 4.
11. Determinism guard: assert that a `BatchParallel` cell and the equivalent
    `Scalar` cell over the same stream produce identical permanent values and
    identical zero counts (extend or mirror the existing worker-count
    determinism test rather than duplicating its scaffolding —
    `@/inv/convention-convergence`).

Also check whether any existing test in `schedule.rs`, `driver.rs`,
`crates/gf2-sim/tests/permanent_campaign_bin.rs`, or
`crates/gf2-sim/tests/permanent_campaign_resume.rs` asserted the old aliasing
behaviour, and update it to the new contract rather than working around it.

## Commands you must run and report verbatim

Run these from the repository root, one at a time, never concurrently:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo nextest run -p gf2-sim --all-features --release --profile ci`
4. `cargo nextest run -p gf2-algebra --all-features --release --profile ci`
5. `cargo build --workspace --all-features`
6. `cargo doc -p gf2-sim --all-features --no-deps` (rustdoc links must resolve)

Report the exact pass/fail counts. If any command fails for a reason you did not
introduce, say so explicitly and quote the failure rather than working around it.

## Report back

State, in this order:
1. The diff summary (`git diff --stat`, and the list of files touched).
2. For each of REQ-01..REQ-06: the file:line that satisfies it and the test that
   proves it.
3. The verbatim results of all six commands above.
4. Anything you left unverified, could not check, or deliberately did not do,
   and why. Do not omit this section — an empty "everything verified" claim that
   is not true costs a full review round.
5. Any defect you noticed in adjacent code but did not fix (the lead files it).

Leave the tree uncommitted and unstaged.
