## Rework Required (attempt 1 of 2)

Your work on issue **Accelerator execution for campaign cells** (`a39bb161`) is
close, and the structure is accepted: the launch-sizing rule, the non-panicking
`has_usable_device` probe, the `AcceleratorDeviceUnavailable` error, the resolver
arm, the CLI flags, and the flock-wrapper runner mode all stand. Do not redo or
restructure them. Three defects from the lead's review must be fixed.

The lead verified on the real host what your sandbox could not:
`cargo build -p gf2-sim --features hip --release` succeeds, and
`cargo nextest run -p gf2-algebra --features hip --release --profile slow
--run-ignored all -E 'binary(gpu_dispatcher)'` passes
`test_permanent_batch_bipedal3_matches_cpu_n24` on the device. The accelerator
dispatch you route to is therefore known-good at the algebra layer. You still
have no GPU — do not attempt device work.

### Worker constraints (unchanged)

- **Do not commit. Do not stage. Do not write `.jit/`.**
- Do not run `./scripts/cargo-ci.sh` or benchmarks.
- Your `cargo nextest -p gf2-sim --all-features` will fail
  `test_dispatcher_acquires_stream_and_allocates` with HIP 100. Expected. Ignore it.

### What to fix

**F1 [blocking] — the accelerator path reports `pack: Duration::ZERO` while it
actually packs, misattributing a published dataset column.**

- Location: the accelerator shard loop in
  `crates/gf2-sim/src/permanent_campaign/schedule.rs` — the `PhaseDurations`
  literal sets `pack: Duration::ZERO`, while `dispatch_accelerator` calls
  `PackedMatrix::new` for every matrix *inside* the span you time as `evaluate`.
- Why it is wrong: `PhaseDurations.pack` is a published per-shard phase column in
  the emitted dataset. The processor path charges `PackedMatrix::new` to `pack`
  (the serial loop times it explicitly around the `PackedMatrix::new` call). The
  generic-Ryser path reports `pack: ZERO` truthfully, because it genuinely never
  packs. An accelerator cell reporting `pack: ZERO` while packing every matrix
  states something false about how its time was spent, and inflates `evaluate` by
  the packing cost. That breaks `@/inv/claims-trace-to-artifacts` for the phase
  columns and leaves three different conventions for one concept in one crate,
  against `@/inv/convention-convergence`.
- What is expected: time the packing separately in the accelerator path and
  charge it to `pack`, so `evaluate` measures the device dispatch alone and
  `pack` measures packing, exactly as the processor path does. Restructure
  `dispatch_accelerator` so packing is timed by the caller rather than buried
  inside the dispatch — for example, pack into a `Vec<PackedMatrix>` in the loop
  under a `pack` span, then hand the packed batch to the dispatch under the
  `evaluate` span.
- Then re-read the rustdoc on `PhaseDurations.pack` and fix it if it no longer
  describes every path's behaviour. Do not leave it saying packing is timed only
  "when `BatchParallel` is selected" if the accelerator now also charges it.

**F2 [medium] — the new public `has_usable_device` has no test.**

- Location: `crates/gf2-algebra/src/gpu.rs`.
- What is expected: a test in `crates/gf2-algebra/tests/gpu_dispatcher.rs`,
  following that file's existing device-gating and `#[ignore = "slow: ..."]`
  conventions exactly. Read the neighbouring tests first and match them. Assert
  that on a host with a working device the probe reports `true`, and state in the
  test's own doc comment that the false branch is only reachable on a host
  without a device. Keep it cheap — it must not become another multi-minute test.
- Note for your report: you cannot run this test. The lead will. Say so.

**F3 [low] — `dispatch_accelerator` re-derives `n` instead of being given it.**

- Location: `crates/gf2-sim/src/permanent_campaign/schedule.rs`,
  `dispatch_accelerator`, which recovers `n` with
  `(1..=63).find(|candidate| candidate * candidate == entries.len())`.
- What is wrong: `n` is a frozen manifest value and is in hand at the call site
  (the accelerator loop already computes `let n = usize::from(item.n);`).
  Re-deriving it by perfect-square search is a second source of truth for a value
  the cell already fixes, and its failure message is wrong: a non-square operand
  length reports "accelerator launch is empty", which is not what happened.
- What is expected: pass `n` in as a parameter. Keep an explicit error for a
  genuinely empty launch, separate from any shape disagreement.

### Constraints

- Fix only F1, F2, F3 plus whatever the sweep below surfaces.
- Do not change `launch_size`, the resolver arm, the error variants, the CLI
  flags, or the runner mode — they passed review.
- If you believe a required change is wrong, say so in your report rather than
  silently skipping it. Do not argue the phase-column semantics in F1: the
  processor path's convention is the one to converge on.

### Resolution table (required)

| # | Round | Source | Finding (verbatim) | Resolution (file:line at HEAD) |
|---|-------|--------|--------------------|-------------------------------|
| 1 | R1 | lead review | F1 — accelerator reports pack ZERO while packing inside the evaluate span | |
| 2 | R1 | lead review | F2 — has_usable_device has no test | |
| 3 | R1 | lead review | F3 — dispatch_accelerator re-derives n by perfect-square search | |

### Mandatory sweep

Run and paste raw results:

```
rg -n "pack" crates/gf2-sim/src/permanent_campaign/schedule.rs
rg -n "Duration::ZERO" crates/gf2-sim/src/permanent_campaign/
```

Every site that reports a phase duration it did not actually measure is the same
defect as F1. Fix all of them, and add rows for any the review did not cite.

### Commands to run and report verbatim

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo nextest run -p gf2-sim --all-features --release --profile ci`
4. `cargo nextest run -p gf2-algebra --all-features --release --profile ci`
5. `cargo build -p gf2-sim` (default features, no hip) and
   `cargo nextest run -p gf2-sim --release --profile ci`
6. `cargo doc -p gf2-sim -p gf2-algebra --all-features --no-deps`
7. `bash dev/scripts/permanent-campaign-runner.test.sh`

### Report back

1. The completed resolution table.
2. The raw sweep results.
3. Verbatim results of every command.
4. Everything left unverified and why — including every device-dependent claim.
