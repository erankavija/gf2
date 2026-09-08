# Confirmation commands and producing inputs

All commands run from the repository root. The completed receipt is
`dev/bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation`.
Its saved plan, append-only execution log and input snapshots are authoritative.
These commands document the completed run; the frozen family has consumed its
one confirmatory attempt and must not be silently rerun for adoption.

The baseline tree was extracted before building:

```sh
mkdir -p /tmp/gf2-1d0da41f-baseline-continuation
tar -xzf dev/active/1d0da41f/survey/baseline-source.tar.gz \
  -C /tmp/gf2-1d0da41f-baseline-continuation
GF2_BASELINE_TREE=/tmp/gf2-1d0da41f-baseline-continuation \
  dev/bench_results/1d0da41f/run-clmul-ab.sh confirmation 2026-09-08
```

The launcher sets Rust 1.95.0 and disables the sandbox-incompatible sccache
wrapper. It builds release runner/evaluator binaries and both locked adapter
projects before requesting the timing lock. Both adapter projects use thin LTO
and one codegen unit. The baseline adapter feature is `frozen-baseline`; both
library dependency graphs use the same adapter Cargo.lock. No RUSTFLAGS or
CARGO_ENCODED_RUSTFLAGS override was present.

The launcher invokes each bounded timing session as:

```sh
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 \
  dev/scripts/ccx1-bench-flock.sh --full-host \
  target/release/benchmark-ab-runner run \
  /tmp/gf2-confirmation-1d0da41f-20260908t084330z \
  /tmp/gf2-confirmation-1d0da41f-20260908t084330z.plan.json \
  dev/active/1d0da41f/producing-inputs.json
```

The three sessions share the exact saved plan and content identity. Two paused
sessions exit 3; the final session exits 0. Finalization and independent
acceptance run after releasing the lock:

```sh
target/release/benchmark-ab-runner finalize \
  /tmp/gf2-confirmation-1d0da41f-20260908t084330z \
  dev/bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation
target/release/benchmark-acceptance \
  dev/bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation
```

The measured candidate libraries, adapter, tooling and build inputs are in the
receipt's `inputs/producing/`. Its producing manifest also pins the baseline
archive. Use those versioned source bytes for an independent reconstruction;
the working tree contains the subsequent default-selection decision and the
auxiliary unpack-diagnostic correction. The receipt's `unpack_ns` diagnostics
are not isolated costs and are excluded from interpretation. Raw paired
windows, samples, logs and checkpoints are preserved without correction.

[receipt-validation.json](receipt-validation.json) records acceptance of the
unchanged receipt and rejection of a tampered candidate-source snapshot copy.
The tampered copy is a temporary validation fixture, not a measurement trial.
