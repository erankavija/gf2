# Recorded-AList LDPC operation preparation

> **Diátaxis Type:** Reference

This preparation governs operation `alist-v1`. Both arms read the same recorded
AList and construct their native matrix and decoder in every measured call.
The gf2 graph adapter uses `LdpcCode::from_edges`; it does not invoke a code
fixture or prepare an encoder. `build-identity.json` pins the executable bytes,
actual AFF3CT compiler/backend observations and source revisions. The complete
source and recorded-input archives are in `source-inputs/`.

## Validation

`alist-before.log` preserves the failing behavioral evidence before the graph
parser was implemented; `alist-after.log` records all four passing harness
unit tests. Coverage includes parity at lengths 0, 1, 63, 64 and 65 and
inconsistent AList adjacency sections. `build.log`, `msrv.log` and
`harness-fmt.log` record release build, Rust 1.95 compatibility and formatting.

The exact validation plan is `validation-plan.json`. Replay command:

```sh
GF2_CCX1_LOCK=/tmp/c077a88b-quality-budget.lock CARGO_CI_NO_SCCACHE=1 \
RAYON_NUM_THREADS=1 ./scripts/cargo-budget.sh taskset -c 0 \
/tmp/c077a88b-build/release/ldpc-validate \
  --plan dev/bench_results/c077a88b/2026-09-08-r4-c077a88b-preparation/validation-plan.json \
  --output dev/bench_results/c077a88b/2026-09-08-r4-c077a88b-preparation/validation.json
```

The dedicated quality-budget lock is separate from both sides of the timing
mutex. This run completed without holding the timing mutex. `quality/` contains
exact per-arm projections of `validation.json`; timing reuses them without
adding BER/FER samples. The generated survey tables carry counts and Wilson
intervals. Layered iteration distributions are native-wave ladder upper bounds.
RSS sampling includes process/runtime/observation allocations and occurs at
different phases in the two adapters, so it cannot rank decoder-owned memory.
Untimed quality latency is diagnostic rather than a timing measurement.

## Method review of preserved attempts

The r1 receipt fails protocol P-18 because the shared runner dropped child
quality. Its repair preserves one corpus report per cell across timing pairs.
The incomplete r2 attempt exposes asymmetric matrix setup. The r3 receipts
pass mechanical acceptance but fail scientific operation review: calling
`ComparisonCode::build()` for NR reaches
`crates/gf2-coding/src/ldpc/nr_5g/mod.rs::nr_5g_rate_matched`, which calls
`compute_mother_encoding` and performs dense encoding preparation. This is
unrelated to decoder construction. Both r3 families and the unused r3
confirmation addendum are preserved and excluded from resolution evidence.

The pinned AFF3CT source also explains a substantial constructor cost:
`include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` computes
an adjacency offset by repeatedly summing a degree prefix for each edge in
its constructor. The horizontal-layered constructor does not contain that
loop. This source observation motivates a reused-decoder follow-up; it is
not a measured attribution of a kernel speedup. The measured operation here
includes construction and explicitly does not represent steady-state decode.

## CI

`cargo-ci.log` preserves the direct `./scripts/cargo-ci.sh` failure caused by
the sandbox denying sccache with EPERM. `cargo-ci-no-sccache.log` records the
same repository contract with its documented `CARGO_CI_NO_SCCACHE=1` override.
It completed with exit 0: every step passed, including 5952 passing workspace
tests and 248 skipped tests. The complete verdict is preserved verbatim in the log.
