# Complete core tuning calibration: premeasurement protocol

Status: authoritative campaign declaration for issue `eaae1b56`.

This document fixes the campaign before implementation or timing. It applies
the calibration convention in
[`7d824b2f/design.md`](../7d824b2f/design.md), including Amendment A7, and
extends the executed pilot recorded in
[`2026-09-01-389aa4de.md`](../../benchmarks/tuning_profiles/2026-09-01-389aa4de.md).
The current harness and composer remain the only calibration and publication
tools. The campaign adds no loader, forcing hook, artifact representation, or
parallel-execution convention.

## 1. Fixed protocol and accounting

Every grid/arm cell runs one untimed equivalence-and-route probe followed by
five independent timed executions. Each timed execution contains five 250 ms
target windows. Fixtures are built before timing and paired arms receive
identical inputs. The current 5 executions x 5 repetitions x 250 ms choice is
retained because it already produced auditable pilot evidence on the target
host.

Fifteen selectors contribute `15 x 9 x 2 = 270` grid/arm cells. The shared
interpolation selector contributes two separately timed dispatcher variants,
or `2 x 9 x 2 = 36` cells. The exact campaign therefore has 306 cells, 306
probe children, 1,530 timed children, 1,836 total fresh-process launches, and
`1,530 x 5 = 7,650` raw windows. Nominal target-window time is
`7,650 x 250 ms = 1,912.5 s = 31 min 52.5 s`. The calibrator has a hard
3,600 s wall-clock budget under one outer full-host lock; composition follows
under the same lock. Expected calibration elapsed time is 38--55 minutes after
child startup, call calibration, fixture setup, validation, and serialization.
A timeout invalidates the campaign and emits no publishable partial result.

Measurements use Rust 1.95 and `RAYON_NUM_THREADS=4`, matching the repository's
four-thread contract. The probe/controller has explicit features
`parallel,simd,test-support,tuning-profile`; the separately built normal timed
worker has exactly `parallel,simd,tuning-profile`. Separate target directories
prevent the crate's self dev-dependency from feature-unifying `test-support`
into the timed bytes. A directly executed binary receives the thread setting
explicitly. `Fp<251>` is the byte-lane matrix carrier, `Fp<65537>` is the
polynomial and SoA carrier, and bit-matrix fixtures have no field carrier.

## 2. Deterministic fixtures

The seed root and derivation remain the pilot protocol:

```text
root = 0x5ecc9bf800000000
value = root XOR (role * 0x9e3779b97f4a7c15 modulo 2^64)
for word in [field_tag, scalar_grid_value]:
    value = value XOR word
    value = rotl(value * 0xbf58476d1ce4e5b9 modulo 2^64, 27)
            + 0x94d049bb133111eb modulo 2^64
seed = value XOR (value >> 31)
```

Pilot tags 0--4 and their role values stay unchanged. Follow-on tags 5--15
are assigned in selector order below. Their stream roles are fixed:

| Tag | Fixture streams and role values |
|---:|---|
| 5 | transpose matrix `0x100` |
| 6 | quadratic lhs/rhs `0x200`/`0x201`; cubic lhs/rhs `0x202`/`0x203` |
| 7 | M4RM-wide lhs/rhs `0x300`/`0x301` |
| 8 | M4RM-tiled lhs/rhs `0x310`/`0x311` |
| 9 | GF(2) unit-lower/unit-upper `0x400`/`0x401` |
| 10 | field unit-lower/unit-upper `0x500`/`0x501` |
| 11 | solve unit-lower/unit-upper/rhs `0x600`/`0x601`/`0x602` |
| 12 | PLE unit-lower/unit-upper `0x700`/`0x701` |
| 13 | back-sub designated-nonzero/non-designated/row-mix `0x800`/`0x801`/`0x802` |
| 14 | GEMM lhs/rhs `0x900`/`0x901` |
| 15 | interpolation coefficients/point offset `0xa00`/`0xa01` |

The scalar seed input is the listed selector value; GEMM uses volume rather
than cube dimension. Every follow-on stream has exactly eight banks. Bank `b`
uses `seed(field, scalar_grid_value, role + (b << 16))` for `0 <= b < 8`;
the addition is wrapping `u64` arithmetic. The pilot keeps its existing seed
and bank derivation byte-for-byte. No ambient entropy, repetition, execution,
process, or worker identity enters seed derivation.

Each seed initializes `gf2_core::rng::Lcg`. One draw first updates
`state = state * 6364136223846793005 + 1442695040888963407` modulo `2^64` and
then returns that state. Consumers traverse banks ascending, then the stream
order in the role table, then components ascending, then row-major matrix cells
or low-to-high polynomial coefficients. A bit-matrix row consumes one draw per
stored `u64` word; the returned word is stored directly and the final word is
masked to zero tail padding. A scalar GF(2) triangular entry uses
`draw & 1`. An `Fp<P>` cell uses `Fp::new(draw % P)`; a required nonzero cell
uses `Fp::new(1 + draw % (P-1))`. No rejection sampling or unused draw is
permitted.

Call-count calibration begins at logical index zero. For a timed window,
`start = (execution * 5 + repetition) & 7`; call `c` uses logical index
`start + c`. Unary fixtures use bank `index & 7`. Binary fixtures use lhs bank
`index & 7` and rhs bank `(index + 3) & 7`. An SoA composite uses that same
bank pair for all four calls. The untimed probe uses the corresponding index
zero banks. Both arm children reconstruct these banks independently and must
emit identical operand digests before comparison.

### 2.1 Exact follow-on constructions

- **Transpose:** fill each `(64x) x (64x)` `BitMatrix` bank by stored words in
  row-major order from role `0x100`.
- **SoA:** use `Fp<65537>` with quadratic and cubic `ExtConfig::NON_RESIDUE = 3`.
  For each of the four roles, consume coefficients component-major and element
  ascending to build length-`x` `BatchExtField` banks. A composite returns, in
  order, quadratic multiply(lhs,rhs), quadratic square(lhs), cubic
  multiply(lhs,rhs), and cubic square(lhs).
- **M4RM wide and tiled:** independently fill lhs `64 x 512` and rhs
  `512 x (64x)` bit matrices by stored words in row-major order from the row's
  two roles.
- **GF(2) inverse:** construct unit-lower `L` and unit-upper `U` in row-major
  order, drawing one bit only for each strict-triangle cell. Form `A=L*U` with
  the harness's scalar loops ordered row, column, inner index; fixture
  construction calls no production dispatcher.
- **Field inverse:** construct `Fp<251>` unit-lower `L` and unit-upper `U` in
  the same order with `draw % 251`, then form `A=L*U` with scalar field loops in
  row, column, inner-index order.
- **TRSM:** construct `A=L*U` by the field-inverse rule and fill rhs `B` as an
  `x x x` row-major `Fp<251>` matrix from role `0x602`.
- **PLE panel:** construct the full-rank `Fp<251>` matrix `A=L*U` by the
  field-inverse rule using roles `0x700` and `0x701`.
- **PLE back-substitution:** let `r=floor(x/2)` and designated independent
  columns `p_i=2i`, `0 <= i < r`. Build an `r x x` rank-seed matrix `E`: each
  `E[i,p_i]` is a nonzero draw from role `0x800`; every non-designated column
  cell is a `draw % 251` from role `0x801`, traversed row then column; all other
  designated-column cells are zero. Embed `E` in the first `r` rows of an `x x x` zero
  matrix. Build an `x x x` unit-lower row mixer `L` from role `0x802` and set
  `A=L*E` using scalar row, column, inner-index loops. The nonzero entries on
  distinct `p_i` prove `rank(A)=r`; since `0<r<x`, every fixture has pivots
  and free columns and cannot take the pivot-free bypass.
- **GEMM:** fill `Fp<251>` lhs and rhs `d x d` matrices row-major from roles
  `0x900` and `0x901`; `d` is the cube dimension mapped from the volume grid.
- **Interpolation:** consume `x` nonzero coefficient draws from role `0xa00`
  in low-to-high degree order. One role-`0xa01` draw supplies
  `offset = draw % 65536`; point `i` is
  `Fp<65537>::new((offset + i*1000003) % 65536 + 1)`. Compute each ordinate by
  descending-coefficient Horner evaluation. This gives a degree-`x-1`
  polynomial and `x` distinct nonzero points.

## 3. Follow-on selector matrix

Unless a row names a companion control, only the studied selector differs from
`CoreTuning::CONSERVATIVE`. `x` is the listed scalar grid value and `d` the
listed GEMM cube dimension. All forcing values are within current codec ranges.

| Selector (tag) | Ordered grid; exact fixture shape | Production entry point | Exact forced arms, route, and exclusion |
|---|---|---|---|
| `bit_matrix.transpose_simple_max_blocks` (5) | `[2, 4, 8, 15, 16, 17, 32, 64, 128]`; square `(64x) x (64x)` seeded `BitMatrix` | `BitMatrix::transpose` | Simple threshold `x`; MacroTiled `x-1`. `transpose_route(x,x)` is exactly `Simple`/`MacroTiled`; macro extent stays conservative. |
| `soa_batch.parallel_min_len` (6) | `[4096, 8192, 16384, 32767, 32768, 32769, 65536, 131072, 262144]`; length-`x` quadratic and cubic SoA batches over `Fp<65537>` | In fixed order, `BatchExtField::batch_mul_quadratic`, `batch_square_quadratic`, `batch_mul_cubic`, `batch_square_cubic`, inside one local dedicated four-thread pool per child | Sequential `x+1`; Parallel `x`. `soa_parallel_route(x)` is exactly `Sequential`/`Parallel`. Inside the dedicated pool, `rayon::current_num_threads()` is exactly 4, the production gate fact. Parallel records `last_effective_soa_chunk=16384` after each call; sequential records none. Chunk length stays conservative. |
| `m4rm.wide_tier_min_stride_words` (7) | `[2, 4, 8, 15, 16, 17, 32, 64, 128]`; `64 x 512` by `512 x (64x)` bit matrices | `alg::m4rm::multiply` | SmallN `x+1`; Wide `x`. `m4rm_schedule_route(512,64x)` reports the exact tier and panel width. Both arms set tiled threshold to `usize::MAX` as a recorded companion control, excluding tiled execution. |
| `m4rm.tiled_min_stride_words` (8) | domain-clipped `[4, 5, 6, 8, 12, 16, 24, 32, 64]`; `64 x 512` by `512 x (64x)` bit matrices | `alg::m4rm::multiply` | RowWise `x+1`; RegisterTiled `x`. Reporter admission must be false/true and the authorized test-support effective observer must report exactly RowWise/RegisterTiled. `m=64>=8`; the existing SIMD tile capability is required. |
| `dense_inverse.m4ri_min_dim` (9) | `[1, 2, 4, 7, 8, 9, 16, 32, 64]`; GF(2) `A=L*U`, shape `x x x` | `alg::gauss::invert` | Scalar `x+1`; M4ri `x`. `invert_route(x)` is exactly `Scalar`/`M4ri`. |
| `dense_inverse.blocked_min_dim` (10) | `[2, 4, 8, 15, 16, 17, 32, 64, 128]`; `Fp<251>` `A=L*U`, shape `x x x` | `FieldMatrix::inv` | ScalarPle `x+1`; BlockedPanelized `x`. `inv_route(x)` is exactly the named arm. |
| `triangular.trsm_blocked_min_dim` (11) | `[8, 16, 32, 63, 64, 65, 96, 128, 256]`; invertible `Fp<251>` `A=L*U` and dense rhs `B`, each `x x x` | `FieldMatrix::solve_batch` | Recursive `x+1`; Blocked `x`. `trsm_route(x)` is exact; blocked observes `last_effective_trsm_panel_rows=64`, recursive none. The Blocked arm requires the Fp251 whole-GEMM capability; Recursive records but does not require it. Panel rows are pinned at 64. |
| `ple.panel_base_max_cols` (12) | `[16, 32, 64, 96, 127, 128, 129, 160, 256]`; full-rank `Fp<251>` `A=L*U`, shape `x x x` | `FieldMatrix::ple` | PanelBase `x`; SubPanelRecursion `x-1`. Carrier lane is `Byte`, lane ceiling is 256, and `ple_panel_route(Byte,x)` is exact. `RecursiveSplit` is forbidden. Reset/read `max_effective_panel_dispatch_cols` must report exactly `x`/`x-1`. |
| `ple.blocked_back_sub_min_dim` (13) | `[16, 32, 64, 96, 127, 128, 129, 192, 256]`; exact-rank `Fp<251>` matrix, shape `x x x` | `FieldMatrix::rref` | Scalar `x+1`; Blocked `x`. `back_sub_route(x,x)` is exact. Zero-rank and full-rank/no-free-column bypasses are forbidden. |
| `gemm.axpy_fast_path_min_volume` (14) | volumes `[64, 512, 1728, 3375, 4096, 4913, 8000, 13824, 32768]`, mapped from `d=[4,8,12,15,16,17,20,24,32]` to `Fp<251>` `d x d` by `d x d` | Public production calibration entry `run_gemm_axpy_dispatch` with caller-supplied output | PerCell `d^3+1`; WholeGemm `d^3`. `gemm_axpy_route(d,d,d)` and the probe-only `last_gemm_axpy_dispatch_route` are exactly the named arm. Fp251 whole-GEMM capability is required for the WholeGemm arm; the PerCell arm remains executable when that capability is absent. |
| `polynomial.interpolate_fast_min_points` (15) | `[2, 4, 8, 15, 16, 17, 32, 64, 128]`; degree-`x-1` `Fp<65537>` polynomial sampled at `x` distinct points | Separately, `interpolate_auto` and `interpolate_auto_two_adic` | For each variant: Barycentric `x+1`; SubproductTree `x`; `interpolate_route(x)` is exact. Each variant has its own samples and crossover. Publish the smallest point from which both fast arms win monotonically. If either independently measured variant has a fully comparable measured-default outcome, the shared selector retains the measured default. If either is unreachable, missing, or otherwise uncomparable, the shared selector is uncalibrated and omitted, and authoritative publication aborts. |

The SoA cell is one composite timed operation containing the four public batch
dispatchers in table order; its digest is the ordered result tuple. The two
interpolation variants are not composite: they are independently probed and
timed because generic `interpolate_fast` and two-adic `interpolate_fast_auto`
have different work. Neither represents the other.

The M4RM tiled effective observer is evidence instrumentation at the existing
`multiply_with_k_block` branch. It is compiled only for `cfg(test)` or feature
`test-support`, has reset/read semantics for one fresh-child call, reports
`RegisterTiled` only after `resolve_m4rm_tile8xn()` returns a kernel and the
tiled callee is selected, and otherwise reports `RowWise`. A focused test
target, `crates/gf2-core/tests/m4rm_tiled_effective_no_simd.rs`, installs
`tiled_min_stride_words=4`, multiplies `64 x 512` by `512 x 256`, requires the
reporter's stride admission, and requires the effective observation
`RowWise`. It runs exactly as:

```sh
./scripts/cargo-budget.sh --test cargo +1.95.0 nextest run -p gf2-core \
  --no-default-features \
  --features tuning-profile,test-support \
  --test m4rm_tiled_effective_no_simd \
  --cargo-profile ci-test --profile ci
```

The command omits the `simd` feature, so `resolve_m4rm_tile8xn()` compiles to
`None` deterministically even though the stride predicate is true. It uses no
setter or capability-forcing hook. The observer is not a selector, route
reporter, forcing hook, or normal-build API.

Every named route is a closed vocabulary. Missing, stale, unexpected, or third
routes fail before timing. M4RM tiled, PLE panel, TRSM, GEMM, and SoA preflights
run in each probe child. A probe returns the typed outcome `Complete` or
`Unavailable`; the latter carries only a closed host/capability reason and an
omission-specific fallback or `unavailable_before_dispatch` observation. It
receives no timing. A conservative arm that does not need the absent
accelerator remains complete, while its unavailable paired arm still makes the
grid comparison incomplete. The authoritative prepared-host campaign publishes
only when all sixteen selectors are measured; `--capability-report` may
describe omissions but cannot time or emit the owner.

## 4. Pilot rerun

The authoritative core section has one measurement provenance. All five pilot
values are selected from this campaign's samples, using their existing tags,
roles, fixtures, and exact grids:

| Selector | Exact grid | Production comparison and forcing |
|---|---|---|
| `bit_backend.simd_min_words` | `[1, 2, 4, 7, 8, 9, 16, 32, 64]` | Direct `ScalarBackend` versus detected concrete SIMD backend in separate children, both installing value `x`; because this selector is baked, exact backend/capability identity replaces runtime route evidence. |
| `polynomial.karatsuba_min_degree` | `[4, 8, 16, 31, 32, 33, 64, 128, 256]` | `FieldPoly::mul`: Schoolbook `usize::MAX`, Karatsuba `x`; `mul_route(x,x)` is exactly the named arm, preserving the pilot's single-split production shape. |
| `polynomial.karatsuba_max_out_len` | `[15, 31, 63, 127, 129, 191, 255, 383, 511]` | `field::poly::mul_fast`: Karatsuba `x`, NTT `x-1`; `mul_fast_route(x)` is exact, and balanced operand lengths sum minus one to the grid output length. |
| `polynomial.div_rem_fast_min_len` | `[64, 128, 256, 512, 1024, 2047, 2048, 2049, 4096]` | `FieldPoly::div_rem_auto`: Schoolbook `x+1`, Fast `x`; `div_rem_auto_route` is exactly the named arm for the emitted dividend/divisor shape. |
| `polynomial.subproduct_min_len` | `[128, 256, 512, 1024, 2048, 4095, 4096, 4097, 8192]` | `FieldPoly::batch_evaluate_auto`: Horner `x+1`, SubproductTree `x`; `batch_evaluate_auto_route(x,x)` is exactly the named arm. |

The raw receipt records derived operand dimensions rather than mislabelling a
scalar selector as an operand length.

Per-field fallback is explicit. Transpose, M4RM wide, both inverse fields,
back-substitution, and the four polynomial pilot dispatchers admit no third
arm: any mismatch is invalid evidence. SoA omits when the dedicated pool does
not report `rayon::current_num_threads() == 4`; this asserts the production
thread gate and makes no claim about per-call worker participation. Its receipt
records pool width and effective chunk, while the scalar arm records no chunk.
M4RM tiled omits when the tile kernel is unavailable; PLE panel
omits when `Fp<251>` lacks the Byte lane or its panel kernel declines; the
asymptotic TRSM and GEMM arms omit when the `Fp<251>` whole-GEMM kernel is
unavailable or declines, while their conservative arms remain executable; the
baked SIMD pilot omits when no concrete SIMD backend exists. Interpolation
keeps the conservative default when either fully comparable variant has a
measured-default outcome, but is uncalibrated when either variant lacks a
comparison. Each omission aborts authoritative publication, while reporting
mode records the capability and reason.

## 5. Fresh-child and digest contract

All 1,836 launches are fresh OS processes. The probe/controller performs the
306 probes; the reporter-free normal worker performs the 1,530 timed
executions. The parent sends one canonical case on standard input to the same
guarded child implementation; the command line accepts no case body. Before
any dispatcher, each child:

1. constructs the forced typed `CoreTuning` from the conservative codec value;
2. canonically encodes a format-2 envelope, strictly reopens it through the
   current registry, and verifies canonical re-encoding;
3. installs the reopened `PreparedEnvelope`, never the pre-serialization value;
4. reads every forced/control field from `tuning::active()`, requires
   resolution `Installed`, and compares the active values to the case;
5. builds the deterministic fixture and validates its operand/result digest.

Probe children additionally pass equivalence, route, carrier, and
effective-execution checks. Timed children share the fixture, dispatcher,
result, equivalence, and timing implementation, but compile without
`test-support`; they emit no synthetic effective/capability observation and
their production calls contain no test-support observer stores.

Every child emits its binary role, `test_support` status, embedded clean build
HEAD, profile/section/schema/harness IDs, all active values, section-wrapper
and envelope-content SHA-256, fixture shape, seed inventory, and operand digest.
Complete outcomes add result and equivalence digests; probe-complete outcomes
also add route/effective and carrier/capability/worker observations, while
timed-complete outcomes add calibrated call count and raw windows. Unavailable
probe outcomes add their requested route, closed fallback/pre-dispatch token,
capability observation, and omission reason. The parent accepts one canonical
prefixed result. The parent and composer never install.

Paired arms have identical operand digests and semantically equal results.
Domain-separated SHA-256 covers carrier, shape, and canonical elements.
Compound outputs use ordered length-prefixed tuples: PLE covers `P,L,E,rank`,
RREF `X,R`, division quotient/remainder, and SoA all four results. Checks include
`A*A^-1=I`, `A*X=B`, `P*(L*E)=A`, `X*A=R`, reconstruction of both interpolation
variants, and exact arm equality elsewhere. A route without result equivalence,
or equivalence without route/effective evidence, is invalid.

The harness schema bumps from `tuning-calibration-v2` because process, fixture,
seed-inventory, output, and publication-coverage semantics change. Serialized
raw-sample and fresh-child schemas bump when their shapes change. Historical
bytes remain evidence but are not accepted as output from the current harness.

## 6. Selection and publication

The standing lower-bound crossover scans the ordered grid from small to large.
A tie, conservative-arm win, or uncertainty failure is a non-qualifying point
permitted before the first strict, uncertainty-qualified asymptotic-arm win.
That first win is a candidate only if every later comparison is also a strict,
uncertainty-qualified asymptotic-arm win. A tie, loss, or uncertainty failure
at or after the first win makes the sweep non-monotone. If no qualified win
appears, including an all-tie grid, a fully comparable sweep has a measured
no-win outcome and retains the default. An upper-bound selector applies the
same rule in its existing reversed selection direction.

A field is fully comparable when both arms execute with valid route,
equivalence, nonzero timing, and raw-sample evidence at every predeclared point.
A fully comparable no-win sweep, or a candidate invalidated by a later tie,
loss, or uncertainty failure, is a measured outcome: it retains the
conservative default and the campaign records that fallback under this run's
measurement provenance. An unreachable arm, missing child/result/comparison,
or any other absence of comparable evidence makes the field uncalibrated. An
uncalibrated field is omitted rather than filled with the default, and the
authoritative campaign aborts publication.

A missing comparison anywhere, including after a candidate win, therefore
invalidates the candidate and makes the field uncalibrated rather than a
measured non-monotone outcome.

For interpolation, both dispatcher variants must be fully comparable. If
either has a measured-default outcome, the shared selector has a measured
default outcome; if either is uncalibrated, the shared selector is
uncalibrated. Raw samples, median, dispersion/confidence interval, selecting
margin, ties, no-win and non-monotone outcomes, measured fallback, seeds,
routes, and omissions are retained in the log and receipt.

The exact inventory derives from
`CoreTuningCodec::encode_body(CoreTuning::CONSERVATIVE)` through `SectionCodec`.
For a publishable run, the measured set equals the codec-derived sixteen names
here, including any fully comparable measured-default outcome; omissions are
its exact 21-field complement in the current 37-field codec. An uncalibrated
campaign has no authoritative owner output. Missing, duplicate, unknown,
defaulted, frozen, malformed, zero-call, zero-duration, non-finite,
route-mismatched, result-mismatched, or
noncanonical evidence fails closed.

Core owner and complete envelope reopen strictly and canonically. The
complete envelope's `gf2-core/selectors` raw wrapper is byte-identical to the
owner wrapper. Its typed and raw `gf2-algebra/selectors` section is byte-identical
to `crates/gf2-algebra/data/tuning-profiles/conservative.json`. The composer
changes assembly provenance only and never installs. The receipt records owner
and complete wrapper/content hashes separately, source revision and clean
status, tool/binary hashes, host facts, feature/thread configuration, every
route/selection/fallback/omission decision, uncertainty, raw-log hash, and the
M4RM boundary clipping.

`MeasurementProvenance::Calibrated.binary_sha256` identifies the reporter-free
timed worker that produces samples. `AssemblyProvenance.tool_sha256` identifies
the probe/controller that validates, selects, and encodes. Both hashes are
captured before the campaign and checked again after it, before any artifact
write; either binary changing aborts publication.

## 7. Build, run, copy, and checksum

Implementation and premeasurement tests precede this exact procedure. Builds
finish before the exclusive lock; the locked driver contains no Cargo command.

```sh
set -eu
RUN_STAMP=$(date -u +%Y%m%d-%H%M%S)-$$
STAGE=/tmp/gf2-eaae1b56-$RUN_STAMP
mkdir "$STAGE"
BUILD_HEAD=$(git rev-parse HEAD)
export BUILD_HEAD
test -z "$(git status --porcelain --untracked-files=all)"
printf '%s\n' "$BUILD_HEAD" >"$STAGE/build-head"
GF2_TUNING_BUILD_HEAD="$BUILD_HEAD" \
CARGO_TARGET_DIR="$STAGE/target-controller" \
./scripts/cargo-budget.sh cargo +1.95.0 bench -p gf2-core \
  --no-default-features \
  --features parallel,simd,test-support,tuning-profile \
  --bench tuning_calibration --no-run --message-format=json \
  >"$STAGE/controller-build.json" 2>"$STAGE/controller-build.stderr"
jq -r -s '[.[] | select(.reason == "compiler-artifact" and
  .target.name == "tuning_calibration" and .executable != null) |
  .executable] | unique | if length == 1 then .[0] else
  error("expected exactly one controller executable") end' \
  "$STAGE/controller-build.json" >"$STAGE/controller-build-path"
GF2_TUNING_BUILD_HEAD="$BUILD_HEAD" \
CARGO_TARGET_DIR="$STAGE/target-timed" \
./scripts/cargo-budget.sh cargo +1.95.0 build --release -p gf2-core \
  --no-default-features --features parallel,simd,tuning-profile \
  --bin tuning_calibration_timed --message-format=json \
  >"$STAGE/timed-build.json" 2>"$STAGE/timed-build.stderr"
jq -r -s '[.[] | select(.reason == "compiler-artifact" and
  .target.name == "tuning_calibration_timed" and .executable != null) |
  .executable] | unique | if length == 1 then .[0] else
  error("expected exactly one reporter-free timed executable") end' \
  "$STAGE/timed-build.json" >"$STAGE/timed-build-path"
CARGO_TARGET_DIR="$STAGE/target-composer" \
./scripts/cargo-budget.sh cargo +1.95.0 build --release \
  --manifest-path dev/tools/tuning-profile-compose/Cargo.toml \
  --message-format=json >"$STAGE/composer-build.json" \
  2>"$STAGE/composer-build.stderr"
jq -r -s '[.[] | select(.reason == "compiler-artifact" and
  .target.name == "tuning-profile-compose" and .executable != null) |
  .executable] | unique | if length == 1 then .[0] else
  error("expected exactly one composer executable") end' \
  "$STAGE/composer-build.json" >"$STAGE/composer-path"
cp --no-clobber "$(cat "$STAGE/controller-build-path")" \
  "$STAGE/tuning-calibration-controller"
cp --no-clobber "$(cat "$STAGE/timed-build-path")" \
  "$STAGE/tuning-calibration-timed"
cp --no-clobber "$(cat "$STAGE/composer-path")" \
  "$STAGE/tuning-profile-compose"
CALIBRATOR=$(realpath "$STAGE/tuning-calibration-controller")
TIMED_WORKER=$(realpath "$STAGE/tuning-calibration-timed")
COMPOSER=$(realpath "$STAGE/tuning-profile-compose")
test -x "$CALIBRATOR" && test -x "$TIMED_WORKER" && test -x "$COMPOSER"
test "$CALIBRATOR" != "$TIMED_WORKER"
sha256sum "$CALIBRATOR" "$TIMED_WORKER" "$COMPOSER" \
  >"$STAGE/binaries-sha256.txt"
test "$(git rev-parse HEAD)" = "$BUILD_HEAD"
test -z "$(git status --porcelain --untracked-files=all)"
test "$(cat "$STAGE/build-head")" = "$BUILD_HEAD"
```

`cargo bench --no-run` intentionally has no redundant `--release`. The build
path files contain plain text read with `cat`; they are never parsed as JSON.
The staged binaries are the exact bytes executed under the lock.

```sh
RUN_ID=gf2-eaae1b56-$RUN_STAMP
RECEIPT=dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.md
CORE_OUT="$STAGE/$RUN_ID-core.json"
COMPLETE_OUT="$STAGE/$RUN_ID-complete.json"
RAW_LOG="$STAGE/$RUN_ID-calibration.log"
STDERR_LOG="$STAGE/$RUN_ID-calibration.stderr"
COMPOSER_LOG="$STAGE/$RUN_ID-composer.log"
COMPOSER_STDERR="$STAGE/$RUN_ID-composer.stderr"
HASHES="$STAGE/$RUN_ID-sha256.txt"
env GF2_BENCH=1 RUSTUP_TOOLCHAIN=1.95.0 RAYON_NUM_THREADS=4 \
  BUILD_HEAD="$BUILD_HEAD" RUN_ID="$RUN_ID" RECEIPT="$RECEIPT" \
  CALIBRATOR="$CALIBRATOR" TIMED_WORKER="$TIMED_WORKER" \
  COMPOSER="$COMPOSER" CORE_OUT="$CORE_OUT" \
  COMPLETE_OUT="$COMPLETE_OUT" RAW_LOG="$RAW_LOG" \
  STDERR_LOG="$STDERR_LOG" COMPOSER_LOG="$COMPOSER_LOG" \
  COMPOSER_STDERR="$COMPOSER_STDERR" HASHES="$HASHES" \
  ./dev/scripts/ccx1-bench-flock.sh --full-host sh -eu -c '
    expected_head=$BUILD_HEAD
    test "$(git rev-parse HEAD)" = "$expected_head"
    test -z "$(git status --porcelain --untracked-files=all)"
    for path in "$CORE_OUT" "$COMPLETE_OUT" "$RAW_LOG" "$STDERR_LOG" \
      "$COMPOSER_LOG" "$COMPOSER_STDERR" "$HASHES"; do test ! -e "$path"; done
    timeout -k 30s 3600s "$CALIBRATOR" \
      --executions 5 --repetitions 5 --target-ms 250 \
      --out "$CORE_OUT" --profile-id "$RUN_ID" \
      --timed-worker "$TIMED_WORKER" \
      --lock-wrapper dev/scripts/ccx1-bench-flock.sh --receipt "$RECEIPT" \
      >"$RAW_LOG" 2>"$STDERR_LOG"
    test "$(git rev-parse HEAD)" = "$expected_head"
    test -z "$(git status --porcelain --untracked-files=all)"
    assembled_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    composer_sha=$(sha256sum "$COMPOSER" | cut -d " " -f 1)
    "$COMPOSER" complete "$CORE_OUT" \
      crates/gf2-algebra/data/tuning-profiles/conservative.json \
      "$COMPLETE_OUT" "$RUN_ID" "$assembled_at" "$expected_head" \
      false "$composer_sha" >"$COMPOSER_LOG" 2>"$COMPOSER_STDERR"
    test "$(git rev-parse HEAD)" = "$expected_head"
    test -z "$(git status --porcelain --untracked-files=all)"
    sha256sum "$CALIBRATOR" "$TIMED_WORKER" "$COMPOSER" \
      "$CORE_OUT" "$COMPLETE_OUT" \
      "$RAW_LOG" "$STDERR_LOG" "$COMPOSER_LOG" "$COMPOSER_STDERR" \
      >"$HASHES"
  '
```

After independent validation of hashes, strict reopens, wrapper identity,
coverage, routes, samples, source binding, and receipt arithmetic, publication
copies only to unique absent destinations:

```sh
CORE_DST="crates/gf2-core/data/tuning-profiles/$RUN_ID.json"
COMPLETE_DST="dev/reference_data/tuning-profiles/$RUN_ID.json"
RAW_DST="dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56-calibration.log"
MANIFEST_DST="dev/benchmarks/tuning_profiles/2026-09-01-eaae1b56.sha256"
for path in "$CORE_DST" "$COMPLETE_DST" "$RAW_DST" "$MANIFEST_DST"; do
  test ! -e "$path"
done
cp --no-clobber "$CORE_OUT" "$CORE_DST"
cp --no-clobber "$COMPLETE_OUT" "$COMPLETE_DST"
cp --no-clobber "$RAW_LOG" "$RAW_DST"
cmp "$CORE_OUT" "$CORE_DST"
cmp "$COMPLETE_OUT" "$COMPLETE_DST"
cmp "$RAW_LOG" "$RAW_DST"
sha256sum "$CORE_DST" "$COMPLETE_DST" "$RAW_DST" >"$MANIFEST_DST"
sha256sum -c "$MANIFEST_DST"
```

The receipt is authored from the validated log and committed with exact
artifacts, manifest, code, and tests. Every live reader and baked citation cuts
over only when the candidate bytes and validation tests are present in the same
commit. The 389 core-owner and complete JSON bytes remain at their committed
paths with their receipt, raw log, and manifest as immutable historical
evidence; they are not a compatibility alias or a current representation.

## 8. Ownership and gates

Issue `eaae1b56` owns the harness and bench metadata; the harness-schema token;
focused route, installation, child, and artifact tests; the M4RM effective
observation at the existing branch under `cfg(test)` or feature `test-support`;
the authoritative owner, complete envelope, raw log, manifest, and receipt; and
exact current-reader cutover. Production dispatch behavior, generic registry,
composer, algebra owner, and kernel interfaces remain unchanged. A falsified
reachability premise stops and preserves evidence for owner review.

The source-path boundary is exact:

- harness behavior and schema:
  `crates/gf2-core/benches/tuning_calibration.rs`,
  `crates/gf2-core/src/bin/tuning_calibration_timed.rs`,
  `crates/gf2-core/Cargo.toml`, and
  `crates/gf2-core/src/tuning/mod.rs`;
- production calibration entry and successful PLE evidence:
  `crates/gf2-core/src/field/matrix.rs`,
  `crates/gf2-core/src/field/ple.rs`,
  `crates/gf2-core/tests/tuning_profile_gemm_install.rs`,
  `crates/gf2-core/tests/tuning_profile_gemm_install_above.rs`, and
  `crates/gf2-core/tests/ple_panel_effective_no_simd.rs`;
- effective M4RM evidence only:
  `crates/gf2-core/src/alg/m4rm.rs` and
  `crates/gf2-core/tests/m4rm_tiled_effective_no_simd.rs`;
- current measured-owner readers and baked-value consumers:
  `crates/gf2-core/src/tuning/baked.rs`,
  `crates/gf2-core/src/kernels/backend.rs`,
  `crates/gf2-core/tests/support/measured_format2.rs`,
  `crates/gf2-core/tests/tuning_profile_committed.rs`,
  `crates/gf2-core/tests/backend_selection_baked.rs`,
  `crates/gf2-core/tests/field_vec_baked.rs`,
  `crates/gf2-core/tests/gemm_tiles_baked.rs`, and
  `crates/gf2-core/tests/prime_route_baked.rs`;
- complete-envelope composition and raw-wrapper validation:
  `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`;
- the current permanent citation:
  `crates/gf2-core/docs/KERNEL_OPTIMIZATION.md`;
- append-only executed-state projection at the convention source:
  `dev/active/7d824b2f/design.md`;
- generated evidence destinations under
  `crates/gf2-core/data/tuning-profiles/`,
  `dev/reference_data/tuning-profiles/`, and
  `dev/benchmarks/tuning_profiles/`.

The baked-test support changes from whole-family omission to codec-derived
per-field omission because the measured `gemm` family contains the runtime
volume selector while its tile extents stay omitted. Dated receipts, raw logs,
manifests, active-design amendments, probe material, and archive records remain
immutable historical evidence. In particular,
`crates/gf2-core/data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json`
and
`dev/reference_data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json`
remain byte-identical and resolvable by the 389 receipt and manifest. A
current-reader grep classifies rather than rewrites historical hits.

Before measurement, one canonical metadata suite covers all 16 fields, 17
sweeps, 153 points, and 306 grid/arm forcing cases: literal grids, exact
selector and companion values, codec admissibility, codec-derived complement,
bank-major seed inventories, the binary `+3 mod 8` schedule, and direct
`306/306/1530/1836/7650` accounting. Focused fresh-process production witnesses
cover the smallest exact point for both arms/variants of each family without
turning the fast tier into 306 substantive operations. They cover deterministic
fixtures/digests, strict reopen-before-install, `Installed` resolution, closed
routes and forbidden thirds, effective/capability observations, paired result
equality, malformed/defaulted/frozen/nonzero fail-closed behavior, absent
destinations, and raw-wrapper identity. Synthetic verifier tests mutate every
closed omission and report component. The M4RM observer test includes admitted
stride plus declined kernel capability and requires RowWise.

Premeasurement validation runs harness self-check/list-grid without measuring,
then `git diff --check`, wrapped `cargo fmt`, focused harness/composer/artifact
tests through `cargo-budget --test`, wrapped
`cargo doc --workspace --all-features --no-deps`, and `./scripts/cargo-ci.sh`.
The measurement runs once only after these gates pass.
