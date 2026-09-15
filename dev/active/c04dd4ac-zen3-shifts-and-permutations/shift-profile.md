# Residual `BitVec` shift workload profile

**Issue:** 85fc5ff4
**Protocol:** `zen3-benchmark-protocol` version 4
**Status:** waiting for the scheduled benchmark window

## Question and scope

This profile measures the public in-place, zero-filling `BitVec::shift_left`
and `BitVec::shift_right` residual paths as an isolated family. The complete
production-source audit covers every Rust source below `crates/*/src` and finds
no downstream call outside the owning `bitvec.rs`; whole-consumer measurement
is therefore inapplicable. Benchmarks, examples and tests exercise the public
primitive, but none is a production consumer.

The word-aligned path is a workload control, not a semantically interchangeable
arm. Every exploratory cell holds direction, vector length, seeded layout and
cache state fixed, then measures the declared residual offset and offset 64.
Their ratio describes residual-path cost only. It cannot support substitution,
an ISA-specific implementation, or production adoption.

## Frozen method

The frozen addendum declares ten exploratory cells: both directions over 65,
257, 4,097, 65,537 and 33,554,439 bits. Residual offsets are respectively 1,
7, 8, 63 and 65; each is paired with the existing offset-64 control. The first
four sizes are warm single-core latency cells. The largest rotates eight
fixtures as a streaming-throughput cell. Each cell uses six counterbalanced
pairs, five calibrated 100 ms windows per execution, one worker and the runtime
resolved single-core affinity.

The search spends one exploratory trial per cell and no confirmatory
comparison. A descriptive residual/control gap of at least 1.20 is the
predeclared materiality trigger. It may justify planning-time feasibility work
for at most two concrete forms, but it does not itself nominate or authorize a
form. A material nomination keeps this issue open until Rust 1.95 compile,
assembly, semantic-oracle and applicable runtime-fallback evidence supports an
amended and re-reviewed bracket. Otherwise the profile records a no-candidate
or no-win disposition and retains production unchanged.

## Correctness and representation evidence

The independent zero-fill verifier covers both directions, lengths and offsets
0, 1, 7, 8, 63, 64 and 65, offsets equal to and beyond each vector length,
256-bit lane crossings, incomplete final words and the public API's in-place
aliasing. It derives expected bits from canonical little-endian indices without
calling either shift method. Every checked result must preserve length and zero
tail padding. The launcher refuses to stage measurement unless the rebuilt
Rust 1.95 benchmark executable reproduces the committed validation bytes and
the current production-source audit reproduces its committed bytes.

## Evidence map

| Evidence | Current result |
|---|---|
| `shift-profile-addendum.json` | Frozen before timing; protocol v4, ten exploratory cells |
| `shift-profile-trial-ledger.jsonl` | Empty genesis; the runner reserves the campaign before its first cell |
| `shift-profile-validation.json` | Passing: 948 independent-oracle cases and zero tail-padding failures |
| `shift-profile-consumer-audit.json` | Passing: 332 production files and zero downstream calls |
| `dev/bench_results/c04dd4ac/residual-shift-profile/` | Pending scheduled-window receipt |

## Result

The latency and throughput result, uncertainty, runtime provenance and final
materiality disposition remain pending until the scheduled benchmark-window
job completes. No production-adoption claim is made.
