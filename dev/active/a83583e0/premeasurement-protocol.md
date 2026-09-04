# Crate-owned extent calibration: premeasurement protocol

Status: premeasurement declaration for review. The epic lead has selected the
in-scope architecture in §1. Implementation follows acceptance of this exact
declaration; measurement follows committed implementation and premeasurement
validation. Issue `a83583e0` supplies the success criteria.

This declaration applies the selector convention in
[`7d824b2f/design.md`](../7d824b2f/design.md), its appended extent amendment,
and the ownership, strict-envelope, installation, and composition contract in
[`3fa7c9d0/design.md`](../3fa7c9d0/design.md). The retained core threshold
experiment is the exact experiment in
[`eaae1b56/premeasurement-protocol.md`](../eaae1b56/premeasurement-protocol.md)
and its executed [receipt](../../benchmarks/tuning_profiles/2026-09-01-eaae1b56.md).
Those historical artifacts remain immutable. This campaign takes fresh samples;
it does not copy their measured values into a different measurement wrapper.

## 1. Architecture and decisions before implementation

The core producer remains `crates/gf2-core/benches/tuning_calibration.rs`.
The algebra producer is `crates/gf2-algebra/benches/tuning_calibration.rs`.
Each owns its case vocabulary, fixtures, selectors, forcing, installed-section
checks, route/effectiveness checks, and owner codec. Core contains no permanent
field, algebra codec, or algebra producer invocation. The repository driver
invokes both producers and the existing both-codec composer. No production
crate acquires a reverse dependency.

The epic lead selected the following architecture after independent audits of
the baked mechanism and shared harness support. The alternatives are recorded
to make the decisions reviewable; neither is a pending owner question.

- **D1 — finite production const-generic specializations.** Selected:
  factor each existing tiled production body over const row/column extents,
  and the existing SIMD dot-product body over a const scratch extent. Ordinary
  APIs call these same bodies with their existing cfg-selected constants.
  Typed, `test-support`-only candidate selectors choose a finite specialization
  once in the child before its timing loop. They do not change runtime profile
  semantics. Separately built production variants would require an additional
  build-variant manifest and counterbalancing protocol; the selected mechanism
  measures every candidate from one binary. Installing candidate values alone
  cannot measure them: these three fields are baked and installation does not
  move them.
- **D2 — one crate-neutral development support library.** Selected:
  a small `publish = false` workspace member at
  `dev/tools/tuning-campaign-support`, used only as a dev-dependency by the
  two owner crates. It has no gf2 dependency and owns only common child
  transport/framing, timing windows, seed mixing, empirical statistics,
  append-only execution logging, checkpoint I/O, and neutral atomic-file
  helpers. Owner types and selector policy never enter it. Existing core
  implementations of these mechanisms move to it rather than remaining beside
  copies. A cross-owner source include would put neutral authority under one
  owner; the workspace development library gives it an explicit neutral home.

All production generics retain their natural field/type parameters. The finite
candidate restriction belongs to the calibration selector, not the general
library algorithm. No heap-backed replacement for the dot scratch arrays,
ambient cfg/environment selector, global candidate setter, benchmark-local
copy of a production loop, private tuning authority, or capability-forcing hook
is permitted. No unsafe code is added.

### 1.1 Exact baked candidate mechanism

The GEMM selector represents exactly the Cartesian product
`row_tile = {16,32,64}` and `col_tile = {32,64,128}`. The dot selector represents
exactly `dot_chunk_len = {128,256,512}`. Parsing rejects every other candidate
before preparing a child. A candidate resolves outside timing to a function
pointer or equivalent monomorphized call closure; the specialization executes
the same complete body as the corresponding ordinary API, including its
existing validation, allocation, packing, transpose, and fallback behavior.
The child records both the installed candidate values and the const candidate
identity; the latter is the mechanism that selects the baked value.

Each of the seven existing `GemmTileSite` bodies in `field/matrix.rs` and
`field/expr.rs` has one implementation. Its ordinary caller instantiates
`GEMM_ROW_TILE` and `GEMM_COL_TILE`; its test-support adapter instantiates the
chosen pair. The adapter does not skip the body's dispatch checks or replace
an expression loop with ordinary GEMM. A whole-GEMM early return that omits
the expected tile observation makes the cell unavailable.

The conservative GEMM declarations are unconditional
`GEMM_ROW_TILE_DEFAULT` and `GEMM_COL_TILE_DEFAULT`. Their values are the
existing conservative row and column values; this change separates their
definition from selection without changing either default. The typed
`CoreTuning::CONSERVATIVE` names only these unconditional declarations.
`GEMM_ROW_TILE` and `GEMM_COL_TILE` remain the separate cfg-selected production
aliases: without `gf2_tuning_baked` they name the unconditional defaults;
with it they name the committed baked values. Ordinary API specializations
consume those selected aliases. A nondefault bake therefore cannot alter the
semantic conservative section, its omission fallback, or the calibration
control. This separation is implemented and tested before measurement.

The SIMD dot helper retains `[u64; CHUNK]`, `[u64; CHUNK]`, and
`[u128; CHUNK]` stack storage. The three candidates use 4, 8, and 16 KiB of
scratch respectively. Before measurement, its per-chunk
`MAX_EFFECTIVE_DOT_CHUNK_LEN.fetch_max` is replaced by a local widest-executed
chunk accumulator and one post-walk publication. Reset/read semantics and the
observed maximum are preserved. This correction prevents the observation
itself imposing an atomic cost once per candidate-dependent chunk. There is
no timed run against the per-chunk atomic implementation.

### 1.2 TRSM observation-neutrality correction

Before measurement, `triangular_route_resolved` becomes a pure selector. It
returns the same route and performs no atomic compare-exchange or store.
Calling the public route reporter alone therefore records no executed work.
Each public triangular operation publishes its top-level executed route once
at the outer boundary, outside recursion, preserving the reset/read probe
contract while removing candidate-dependent recursive observation costs.

The timed solve uses a compile-time observation policy on the shared
`solve_batch`, its PLE preparation, recursive/blocked TRSM, and nested GEMM
update bodies. Ordinary
APIs retain their existing algorithm, capability gates, profile reads,
validation, PLE preparation, and numerical results. Test-support probes use
the recording specialization. A test-support-only timing adapter binds a
zero-sized no-op observation specialization before timing and calls the same
complete `solve_batch` body; it does not time a private substitute for the
public operation. The policy propagates through PLE's triangular/GEMM calls,
both upper/lower solve branches,
every recursive or blocked panel, and their GEMM calls. The no-op
specialization performs no triangular-route, panel-width, GEMM tile, or GEMM
route atomics in that candidate-dependent subtree. Selection is compile-time;
there is no mutable global toggle, runtime observation branch, or second
production implementation. Ordinary builds compile the same no-observation
body where the test-support instrumentation is absent.

Every TRSM timed child first makes an untimed recorded call, verifies
`Installed`, actual route, panel extent, capabilities, and solve semantics,
then runs one untimed call of the no-op specialization and verifies its result
against that call and the scalar oracle. Observer reset/read tests establish
that the no-op call publishes no candidate-dependent observations. Only the
no-op specialization enters the five timing windows. Both specializations
share every selector branch and computational body; code review verifies
that the observation policy controls evidence publication only. The receipt
records the probe and timed observation modes explicitly and never presents
an untimed observation as a separately observed timed event.

This correction applies to the new `trsm_panel_rows` extent experiment and
the cumulative `triangular.trsm_blocked_min_dim` threshold experiment,
including its recursive arm. The latter retains its exact grids, forcing,
fixtures, sampling, and crossover estimator while taking fresh samples from
the corrected observation-neutral implementation. Existing historical timing
bytes remain unchanged. The behavioral identity includes this correction.

## 2. Fixed field inventory, grids, and controls

The following twelve fields are the complete extent scope. Values are decimal
integers; `KiB` means exactly 1024 bytes. Defaults are read from each owner's
canonical conservative typed section and are required to occur in the declared
grid. A changed default or codec range invalidates preflight; it does not
silently regenerate the grid. The values below describe protocol candidates,
not additional declarations of library defaults.

Except for the studied candidate and the explicit controls in the table, every
runtime selector is conservative. Each child serializes and reports the full
typed forced section, so an omitted control is not inferred from prose.
Ordinary baked companion constants are observed and recorded. Measurements
are conditional on these controls; no claim of a universal or global optimum
is made.

| Tag | Owner / extent | Ordered candidates | Fixed shapes, in order | Forced context and required execution |
|---:|---|---|---|---|
| 16 | core `bit_matrix.transpose_macro_tile_blocks` | `[2,4,8,16,32]` | square block counts `[32,64,128]`, each block 64 rows/columns | `transpose_simple_max_blocks=0`; public `BitMatrix::transpose`; effective `MacroTiled` and consumed candidate extent |
| 17 | core `soa_batch.parallel_chunk_len` | `[4096,8192,16384,32768,65536]` | element counts `[65536,131072,262144]` | `parallel_min_len=0`; the four-operation SoA composite from §3 in one dedicated four-thread pool; each operation observes the candidate chunk |
| 18 | core `m4rm.default_table_bytes` | `[16384,32768,65536,131072,262144]` | `64 x 512` by `512 x (64s)`, `s=[16,24,31]` | Wide tier, narrow budget band; other M4RM extents conservative; RowWise C-update |
| 19 | core `m4rm.mid_table_bytes` | `[32768,65536,131072,262144,524288]` | same matrix form, `s=[32,48,63]` | Wide tier, middle budget band; other M4RM extents conservative; RowWise C-update |
| 20 | core `m4rm.wide_table_bytes` | `[65536,131072,262144,524288,1048576]` | same matrix form, `s=[64,96,128]` | Wide tier, wide budget band; other M4RM extents conservative; RowWise C-update |
| 21 | core `m4rm.wide_max_k` | `[4,5,6,7,8,9,10]` | same matrix form, `s=[16,24,31,32,48,63,64,96,128]` | Wide tier; all three budgets conservative; RowWise C-update |
| 22 | core `m4rm.small_n_max_k` | `[4,5,6,7,8,9,10]` | `64 x 2048` by `2048 x n`, `n=[512,768,960]` | SmallN tier; other extents conservative; RowWise C-update |
| 23 | core `triangular.trsm_panel_rows` | `[8,16,32,64,128]` | square dimensions `[129,193,257]` for both coefficient matrix and RHS | `trsm_blocked_min_dim=0`; shared public `FieldMatrix::solve_batch` body with §1.2's recorded probe and compile-time no-op timed specialization; Blocked solve and consumed candidate panel rows; Fp251 whole-GEMM capability required |
| 24, 25 | core `gemm.row_tile`, `gemm.col_tile` jointly | rows `[16,32,64]` x columns `[32,64,128]`, row-major candidate order | logical `(m,k,n)=[(65,64,129),(129,128,257),(193,192,385)]`, every shape at all seven sites | `gemm.axpy_fast_path_min_volume=usize::MAX`; `Fp<65537>`; exact const pair observed at the studied blocked site; no whole-GEMM early return |
| 26 | core `field_vec.dot_chunk_len` | `[128,256,512]` | lengths `[4097,16385,65537]` | runtime `Gf2mField::gf256()`; actual batched CLMUL and Barrett reduction; maximum executed chunk equals candidate |
| 27 | algebra `permanent.gray_chunk_subsets` | `[4096,16384,65536,262144,1048576]` | square dimensions `[20,22,24]` | public `permanent_bipedal3_parallel` inside the canonical dedicated four-thread pool; algebra `Installed`; requested-chunk forwarding plus effective partition from §3; serial permanent equality |

For all M4RM extent and combination cells, the explicit companion controls are
`wide_tier_min_stride_words=16` and
`tiled_min_stride_words=usize::MAX`. These hold the tier split and exclude
register-tiled C-updates while the table schedule varies. The width bands
come from the production `production_table_budget` branches; neither the
bench nor the support library reproduces their selection as execution code.
The reporter and effective observation record the selected band, active byte
budget/cap, panel width, tier, and completed C-update. Every listed candidate
has panel width at least two on these shapes; a one-row-XOR bypass is invalid.

GEMM row and column values are selected jointly from the measured nine pairs.
There is no independently selected, unmeasured row/column combination.
The fields outside this extent inventory are not implicitly covered by this
table: retained core thresholds have their
own cumulative experiment in §4, and all remaining leaves are omissions.

## 3. Fixtures, seeds, semantic witnesses, and execution scope

All new extent fixtures use eight banks. Their seed root is
`0x5ecc9bf800000000`, and the existing `gf2-calibration-seed-v1` mixer is
retained exactly. Starting from the root XOR the wrapping product of `role`
and `0x9e3779b97f4a7c15`, mix the field tag and shape key in that order: XOR
the word, wrapping-multiply by `0xbf58476d1ce4e5b9`, rotate left 27, then
wrapping-add `0x94d049bb133111eb`. Finish with XOR of the value shifted right
31. Bank index is added to the role as `bank << 16`, with wrapping arithmetic.
Candidate, execution, repetition, worker identity, wall time, PID, and schedule
never enter fixture derivation. Every candidate therefore receives the same
operands for a given field/site/shape.

The shape key is the zero-based shape index in §2, except M4RM: all its extent
and combination cells use fixture tag 18 and key `s` for the `k=512` shapes,
and key `0x10000+n` for the `k=2048` shapes. GEMM uses fixture tag 24 for both
fields and every site. Tag 25 remains the semantic column-field identity and
does not create a different operand stream. All arithmetic operates on `u64`.

| Fixture | Ordered roles | Exact construction |
|---|---|---|
| Transpose | matrix `0x100` | bit-matrix stored words, row-major; mask the last word of every row |
| SoA | quadratic lhs/rhs `0x200/0x201`, cubic lhs/rhs `0x202/0x203` | `Fp<65537>`, non-residue 3, component-major then element order, as in the retained threshold protocol |
| M4RM, including joint validation | lhs/rhs `0x300/0x301` | stored `u64` words, row-major, with canonical zero tail padding |
| TRSM | unit-lower/unit-upper/RHS `0x600/0x601/0x602` | `Fp<251>` strict-triangle draws in row-major order, diagonal one; form `A=L*U` with the scalar fixture oracle; dense row-major RHS |
| GEMM | logical lhs/rhs/addend `0x900/0x901/0x902` | `Fp<65537>` row-major matrices; transposed-A sites receive the prebuilt physical transpose of the same logical lhs |
| Dot | lhs/rhs `0xb00/0xb01` | row-order runtime GF256 elements from `draw & 255` |
| Permanent | matrix `0xc00` | row-major `Fp<3>::new(draw % 3)` cells, then `Bipedal3Matrix::from_row_major` |

Each stream initializes the canonical `gf2_core::rng::Lcg`. A draw first
updates state by wrapping multiply `6364136223846793005` plus
`1442695040888963407`, then returns that state. An `Fp<P>` cell is
`Fp::new(draw % P)`; only specified strict-triangle cells consume triangular
draws. There is no rejection sampling or skipped draw. Fixture construction
does not call a tuning dispatcher before installation. Independent scalar
oracles are correctness witnesses only and never timed candidate algorithms.

Call-count calibration begins at logical bank index zero. Timed execution
`e` and window `r` begin at `(e*5+r)&7`; logical call `c` uses bank
`(start+c)&7`, with the right operand at `(start+c+3)&7`. A third addend uses
the left bank. Probes use start zero. Each child verifies all eight banks
before timing. Candidate-paired operand and oracle digests must agree.

The SoA timed operation is the ordered tuple quadratic multiply, quadratic
square, cubic multiply, cubic square. Every constituent executes in the same
dedicated four-thread pool, created once outside timing; the existing
`run_in_dedicated_parallel_pool` is the only pool helper. Pool width four is
an observation of the production thread gate, not a count of workers that
participate in a particular call. Algebra uses this same helper, with the
required core test-support feature enabled through its dev build.

For permanent dimension $n$ and requested chunk $q$, the subset count is
$S=2^n-1$. The existing `last_effective_chunk()` accessor observes the requested
value received by `permanent_bipedal3_parallel_with_chunk`, so it must equal
$q$; it does not report a truncated chunk length. The effective schedule tuple
additionally records maximum chunk length $\min(q,S)$, chunk count
$N=\lceil S/q\rceil$, and last chunk length $S-(N-1)q$. Its post-walk
test-support observation derives from the same production partition used by
the completed walk, with no per-chunk atomic or timing-loop instrumentation.
In particular, $n=20$, $q=1{,}048{,}576$ executes one chunk of 1,048,575
subsets. The receipt distinguishes requested forwarding from the actual
partition. Equal partition tuples are schedule plateaus; candidate spelling
alone does not make their executed schedules distinct.

GEMM site order is `GemmTileSite::ALL`. The ordinary and into-view sites
compute `A*B`; AXPY computes `3*A*B+5*C`; implicit-diagonal AXPY uses those
same scalars and both `UnitDiag::Implicit` flags, with the scalar oracle
replacing each logical diagonal by one. Expression beta computes `A*B+5*C`;
expression transpose computes logical `A*B`; expression transpose-beta
computes `3*A*B+5*C`. Every operation owns a fresh initialized output on each
timed call. Output allocation or copying required by that adapter is inside
the timed body and identical for all candidates of the site. Input generation,
oracle work, and candidate binding are outside. Results are black-boxed. The
timed sites remain independent observations, not one composite that can hide
a site-specific reversal.

Required semantic witnesses are entrywise transpose correctness and involution,
the four SoA outputs against scalar field arithmetic, M4RM against scalar GF2
multiplication, solve reconstruction $A X=B$, all seven GEMM formulas against
scalar field multiplication, SIMD dot against scalar dot, and parallel
permanent against the public serial permanent. All candidates use identical
scalar oracle results. Input/output SHA-256 is domain-separated by carrier,
shape, role, and logical contents; compound outputs use ordered length-prefixed
tuples. No witness relies only on equality between routes computing the same
mathematical operation.

Effective observers are reset immediately before each probe invocation and
read immediately afterward in the same fresh process. Zero/missing chunk,
missing tile site, wrong extent, unexpected tier/band/panel, third route,
declined required capability, or noncanonical result makes the cell unavailable
or invalid before any timed windows. Observations must be published once per
operation or after a loop, never once per candidate-dependent iteration. The
normal build has no new instrumentation or candidate selection branch.

For the permanent field, a requested-value mismatch or any mismatch in the
effective partition tuple is invalid. A maximum executed chunk shorter than
the requested value is valid exactly when the declared subset count requires
it; it is not classified as unavailable or mistaken for an ignored selector.

## 4. Cumulative core coverage and algebra ownership

This campaign reruns every one of the sixteen retained core threshold fields
using exactly the grids, fixtures, tags 0–15, companion controls, direct versus
dispatched entry points, five-by-five 250 ms windows, effective observers,
uncertainty-qualified crossover rules, and interpolation reconciliation in
the retained threshold protocol, with the explicitly declared TRSM observation
correction in §1.2. That is 306 grid/arm cells. Its crossover
selection remains separate from the extent argmin rule below. Common support
extraction must pass equivalence tests against the existing timing/selection
semantics; it does not silently replace threshold uncertainty with extent
uncertainty.

Every retained threshold value is derived from this campaign's raw samples.
There is no merge of values measured at different revisions into one section
measurement. The eleven core extents plus those sixteen thresholds form this
campaign's measured core set. The exact schema inventory comes from
`CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE)`; the omission set is
its complement. At the reviewed inventory this is 27 measured of 37 core
leaves, with 10 omissions. These numbers are campaign assertions checked
against codec output, not a second schema registry.

The algebra owner measures its sole present selector through
`AlgebraTuningCodec`. It has its own producing executable hash, harness path,
behavior identity, timing interval, and measurement wrapper. The algebra owner
does not inherit a calibrated core wrapper merely to make a complete artifact.
Current permanent execution reads only algebra tuning; its child installs the
strict algebra-owner envelope and requires algebra `Installed`. Core remains
untouched in that process. If implementation finds any core tuning access on
the timed algebra path, measurement stops: the child must instead receive a
strict complete envelope and verify both installed sections under an amended
predeclaration. It must never time `DefaultedMissing` core context.

After all measured decisions, each owner builds its full retained measured
section and one section-level provenance. A fully comparable default-retention
decision is measured evidence. An unexecuted or invalid extent is omitted in
diagnostic output and invalidates authoritative campaign publication; it is
never inserted as a measured conservative value.

## 5. Exact extent estimator and argmin rule

Each cell has one untimed probe and five timed fresh-child executions. Each
timed child calibrates a call count using the existing doubling protocol:
begin at one call, double until at least 20 ms elapsed or `1<<32` calls, then
scale to the 250 ms target and clamp to `[1,1<<32]`. It records five windows
of that count. Calls and elapsed nanoseconds must both be positive. Every
window is retained; there is no trimming, outlier deletion, optional stopping,
or extra sample after seeing a result.

Let $t_{cse}$ be the median ns/call of the five windows for candidate $c$,
shape/site stratum $s$, and execution index $e$. Let $d$ denote the
conservative candidate. The execution-level aggregate is

$$
G_{ce}=\exp\left(\frac{1}{|S|}\sum_{s\in S}
\log\frac{t_{cse}}{t_{dse}}\right).
$$

Every shape has equal weight; GEMM gives every one of its 21 shape/site
strata equal weight. No operation count, matrix size, runtime, or observed
variance changes those weights. The reported central score is the median of
the five $G_{ce}$. The report also contains all five values, nearest-rank IQR,
and minimum/maximum. These are empirical dispersion summaries, not a
confidence interval or a probability estimate.

Candidate $a$ strictly beats $b$ only when $G_{ae}/G_{be}<1$ for every one
of the five paired execution indices. Equality, or a ratio range containing
one, is unresolved. No numerical epsilon or post-hoc tolerance turns an
unresolved comparison into a win. Non-finite arithmetic invalidates evidence.
The median chooses the provisional minimum; the strict comparison against
every distinct competitor decides whether that minimum is qualified.

### 5.1 Schedule plateaus, ties, and curve shape

Before looking at timings, adjacent candidates with identical observed
effective schedule tuples at every stratum are collapsed into one schedule
class. In particular, an M4RM byte budget or cap can vary while the panel
width, tier, and executed C-update remain identical. Requested byte budget and
cap are retained in provenance but are not differences in executed schedule.
Permanent classes use the effective partition tuple, not the requested chunk
value; the requested value remains a separate forwarding witness.
All raw candidates are still timed and retained. A class uses the conservative
candidate as its representative if present, otherwise its smallest numeric
candidate. No samples are pooled or selected by speed within a class: only
the preselected representative contributes the class score. Nonadjacent equal
schedule tuples are a malformed/non-monotone schedule mapping and retain the
default, with the complete tuple sequence recorded.

For a one-dimensional ordered class sequence, compare each adjacent pair by
the strict five-execution rule. Label a qualified decrease `-`, a qualified
increase `+`, and an unresolved comparison `0`. Remove zeros. A sequence
consisting of zero or more `-` followed by zero or more `+` is unimodal; an
interior U-shaped minimum is valid. Any `+` followed later by `-` is a genuinely
non-monotone curve. Apply this check to the aggregate curve and separately to
each shape/site curve. A non-monotone curve anywhere retains the conservative
field value. An unresolved rising tail alone is not a second minimum.

If the curves are unimodal, a single schedule class must strictly beat every
other class. A tied/uncertain minimum retains the conservative default. Any
winning class containing more than one distinct raw candidate is a structural
tie and retains the conservative default with reason `structural-schedule-tie`,
including an all-one-class plateau and a winning plateau that excludes the
default. The representative is used only to compare and describe schedules;
it never breaks that structural tie. Only a singleton nondefault winning class
can select a nondefault value. A singleton conservative winner retains the
default as the measured minimum. An endpoint minimum is admissible and
explicitly reported as grid-boundary-limited.

No selected candidate may have a qualified regression relative to the default
in any individual shape/site stratum. Such a conflict gives
`cross-stratum-conflict` and retains the default even if its aggregate is best.
Every fallback records its specific reason and all contradicting observations.

For GEMM, apply the same rule to all nine measured pairs, without collapsing
distinct pairs: every dimension exceeds every tile candidate. Unimodality is
required for each fixed-row column slice and each fixed-column row slice,
both in aggregate and at each stratum. There must be one pair that strictly
beats every other pair, subject to the no-qualified-stratum-regression check.
Any tie, uncertainty, or non-monotone slice retains the conservative pair as
a unit. The receipt reports both selected coordinates and all nine scores.

### 5.2 M4RM final-combination acceptance

The five M4RM one-factor sweeps produce conditional candidates with the other
four extents conservative. They do not establish a global five-dimensional
optimum. The driver then measures their combined vector against the entire
conservative vector at all twelve M4RM shapes: the nine wide-band shapes plus
the three SmallN shapes in §2. Controls, fixtures, seeds, oracles, probes,
children, and timing windows are unchanged. These 24 additional cells are
mandatory even when the selected vector is conservative; duplicate vectors
remain separately labeled controls and are never presented as independent
algorithmic behavior.

Accept the proposed vector only when it has an aggregate strict win in all
five executions and no execution-level regression at any of the twelve
strata. If the vector equals the conservative vector, retain it without making
a speed claim. Otherwise any failed or unresolved aggregate/stratum check
retains all five conservative extents together and reports
`joint-validation-default`. Invalid or missing comparison evidence aborts
publication. The recorded output is the accepted conditional vector, not a
global joint optimum. All one-factor suggestions and their rejection remain
in the receipt.

## 6. Campaign order, arithmetic, budgets, and resume

The driver runs the retained core threshold phase, the core extent phase in
tag order, M4RM combination validation, the algebra extent phase, owner
emission/reopen, and complete composition/reopen. Core extent ordering is
field, shape, site where applicable, then execution block. A probe block visits
each candidate in ascending order. Timed block `e` visits candidates in ascending order rotated
left by `e % candidate_count`; odd `e` reverses that rotated list. GEMM uses
the row-major nine-pair order as its base. M4RM combination validation uses
conservative/proposed as its base. Every execution block finishes all
candidates before advancing. The retained threshold phase keeps its committed
ordering exactly.

| Phase | Cell calculation | Cells |
|---|---:|---:|
| Retained core thresholds | exact retained protocol | 306 |
| Transpose extent | 5 candidates x 3 shapes | 15 |
| SoA extent | 5 x 3 composite shapes | 15 |
| M4RM byte budgets | 3 fields x 5 x 3 | 45 |
| M4RM wide cap | 7 x 9 | 63 |
| M4RM SmallN cap | 7 x 3 | 21 |
| TRSM panel | 5 x 3 | 15 |
| GEMM tile pair | 9 pairs x 3 shapes x 7 sites | 189 |
| Dot chunk | 3 x 3 | 9 |
| M4RM combination | 2 vectors x 12 shapes | 24 |
| Algebra permanent chunk | 5 x 3 | 15 |
| Complete campaign | sum | 717 |

The core contribution is 702 cells and the algebra contribution 15. A complete
accepted campaign has 717 probes, 3,585 timed children, 4,302 total accepted
fresh-child results, and 17,925 raw timing windows. Nominal target-window time
is 4,481.25 seconds (74 minutes 41.25 seconds). Setup, process startup,
calibration, oracles, checkpoint writes, and strict validation add elapsed
time; this nominal arithmetic is not a runtime prediction.

Each child has a hard 120-second wall limit, followed by a 5-second kill
grace. Each outer-lock session has a 10,800-second active budget, including
probes, timing, and composition but excluding builds and idle waiting. The
driver checks that session's budget before every child and after every result,
never starts a child with less than 125 seconds remaining including kill
grace, and emits `budget-exhausted` without authoritative publication when
the session cannot finish. It preserves every accepted checkpoint, releases
the outer lock, and may resume in a later session with the same 10,800-second
budget and unchanged campaign/protocol identity. No session receives a larger
budget or changed grid. There is no aggregate campaign deadline that makes
completed work unusable; cumulative active elapsed time, session boundaries,
attempts, and accepted results remain part of the campaign accounting. The
initial run may finish in one lock hold; completion does not require that.

An accepted bounded unit is one complete verified child result, keyed by
protocol identity, owner, phase, field, stratum, candidate, task, and execution
index. Each unit is written atomically to a previously absent checkpoint and
flushed/synced before being marked complete. Every later launch first skips
verified completed units. An interrupted child contributes no accepted unit;
its attempt and partial diagnostics remain in the execution log. Attempt
counts, accepted counts, and interruption records are separate; the arithmetic
above counts accepted results exactly once.

Resume verifies the checkpoint manifest, canonical case/result identity,
protocol digest, immutable producing revision, executable hashes, feature and
thread contract, host identity, and every completed unit's digest/evidence.
It restores the original candidate/execution ordering and seeds, including
after a budget-exhausted session. It never
changes an accepted sample, re-times a losing completed cell, or merges a
different producer's output. An interruption may resume the same unfinished
child. A completed child with invalid output, wrong route, or semantic failure
is a hard failure requiring diagnosis; it is not an automatic resampling
opportunity. Source or measurement-behavior changes require a new campaign
identity and preserve the old campaign's terminal failure evidence.

## 7. Durable execution log and host discipline

The launcher creates a unique stage directory before any bounded work,
resolves its absolute canonical path, and opens `execution.log` there with
create-new plus append semantics. Before launching the first probe or timed
child, it prints exactly one readily visible line:

```text
GF2_CAMPAIGN_EXECUTION_LOG=/absolute/canonical/stage/execution.log
```

It immediately appends and flushes a campaign-start record with run identity,
source and binary identities, declared counts, and checkpoint directory. The
file is the authoritative execution record under
`@/inv/campaign-execution-logging`. Console output, `tee`, or `tail` is only
a view; none consumes or replaces the record. The driver never uses
`producer | tail` as its persistence mechanism.

The common logger appends phase-start/complete, cell-start/complete, execution
and window progress outside the timing interval, child PID/exit/timeout,
checkpoint acceptance, omission/fallback, owner-write/reopen,
composition/reopen, lock hold/release, and terminal state. Records have UTC
timestamps, campaign/session identity, a monotonic sequence, and structured
case identity. Progress is flushed immediately; accepted checkpoints and
terminal records are synced to disk. A single writer at each sequential
phase serializes records; worker threads do not write the log. Child stdout
has exactly one canonical result; progress travels through the common
out-of-band logging channel and never weakens result framing.

Normal completion appends `complete`; caught errors append `failed` with the
cause and exit nonzero; intentional interruption appends `paused`; a spent
session budget appends `budget-exhausted`. Resume appends to the same file, never
truncates it, and records a new session boundary. If a prior process was
uncatchably killed before a terminal record, recovery appends `interrupted`
for that session before restarting unfinished work. Monitoring selects the
current session/sequence, never an earlier terminal line.

The stage path is `/tmp/gf2-a83583e0-<UTC-stamp>-<launcher-pid>`. Stage logs
and checkpoints survive agent/session exit and are not removed before durable
publication or a preserved failure record. Every terminal failure preserves
the execution log and diagnostic manifests under the issue's evidence area;
failed attempts never overwrite successful evidence.

All Rust builds, tests, self-checks, and binary staging finish before the
timed host reservation. They use Rust 1.95.0 and the repository Cargo budget
wrapper. The core and algebra benchmark targets use explicit
`parallel,simd,tuning-profile,test-support`; the composer is built `--release`.
`cargo bench --no-run` has no redundant `--release`. Each build's
`--message-format=json` is checked to yield exactly one executable for its
named target. Path files are plain text, not JSON. Executables are copied
to stage, hashed, and invoked from their staged paths.

The build phase records clean source HEAD and porcelain status before and
after all builds. The preflight rejects any source movement or dirty state.
No source, JIT state, handoff, or commit changes between final build and the
measurement phase. The only permitted writes during measurement are outside
the repository in the stage. Narrative-only changes after completion do not
invalidate evidence; their producing-behavior identity remains pinned.

The lead observes the actual host, including load, competing CPU/GPU work,
governor, CPU model/features, OS, affinity, and available memory. A sandbox's
isolated process listing is not host-idle evidence. A held mutex alone is not
host-idle evidence. The driver records these observations and requires the
prepared host to have no competing substantial work before entry.

The driver invokes `dev/scripts/ccx1-bench-flock.sh --full-host` once around
every calibration and composition action in that session. Core, algebra,
support code, and composer acquire no inner lock. There is no Cargo command
under the lock. All directly executed binaries receive `GF2_BENCH=1`,
`RUSTUP_TOOLCHAIN=1.95.0`, and `RAYON_NUM_THREADS=4` explicitly. The lock path
and inherited held-lock evidence are runtime-observed and emitted. Resume
reacquires that same outer mutex and records a new hold interval before any
unfinished work; completed units are never replayed. Composition occurs under
the final hold. There is no alternate lock domain or unlocked component step.

## 8. Fresh-child, codec, and fail-closed contract

Common calibration transport preserves the current reviewed sentinel
`GF2_TUNING_FRESH_CASE=child-v2` and canonical compact stdin case framing.
Benchmark CLI and libtest adapter entry modes remain explicit; `cfg(test)`
does not distinguish them. The case includes its owner protocol identity,
candidate, context, log/checkpoint channel, and task. Child mode cannot spawn
another child. The parent checks benchmark authorization before timed child
launch. Non-measuring self-check/list-grid modes neither install nor emit
profile artifacts.

Every probe and timed child, including direct baked candidate calls:

1. constructs a typed forced owner section through its owner codec and
   validating constructors;
2. encodes its format-2 owner envelope with the owner-only registry, strictly
   reopens it, and verifies canonical re-encoding and wrapper/content hashes;
3. installs the reopened `PreparedEnvelope` exactly once before any relevant
   dispatcher or tuning access;
4. requires `SectionResolution::Installed` and exact active values for every
   studied or affecting section, then verifies route, effective candidate,
   capability, pool width, operands, and scalar semantics;
5. returns one canonical `GF2_TUNING_RESULT=` result with the complete evidence
   and, for timed tasks, exactly five windows.

No parent, driver, validator, or composer installs a profile. Reopen/inspection
uses prepared typed entries without accessing process-global tuning. A late
install, `DefaultedMissing`, `FrozenBeforeInstall`, nonzero child, duplicate
or missing result, noncanonical case/result, wrong protocol/owner, unknown
field, bad digest, zero-call/zero-duration sample, route/capability mismatch,
or failed oracle prevents checkpoint acceptance and authoritative output.
Unsupported hosts may produce explicit unavailable diagnostics; they cannot
publish a partial owner or silently substitute scalar timings for required
accelerated evidence.

### 8.1 Structured evidence schema

The owner case/result behavior identities are `core-tuning-campaign-v4` and
`algebra-tuning-campaign-v1`. New extent seed inventories use
`fixture-seeds-v3`; retained threshold inventories retain `fixture-seeds-v2`
with their exact derivation. New campaign raw-window records use
`raw-timing-samples-v3`; this changes representation, not the retained
threshold sampling or decision rule. The execution journal and checkpoint
manifest identities are `tuning-campaign-journal-v1` and
`tuning-campaign-checkpoint-v1`.

An owner case contains the protocol identity and constants, campaign/phase,
typed field or joint-vector identity, stratum shape and site, exact candidate
and controls, probe or timed-execution task, seed inventory, and out-of-band
log/checkpoint identity. Unknown fields or enum variants reject. Raw results
repeat that identity and add the installed owner/section/schema/profile IDs,
active values and resolution, envelope/content/wrapper digests, fixture
shape and operands digest, requested and observed route, effective schedule,
capability/pool facts, semantic result/oracle digests, and outcome. A complete
probe has zero windows; a complete timed result has exactly five records with
integer `execution`, `repetition`, `calls`, and `elapsed_ns`. Unavailable
results have a closed omission reason and actual observations, with no timing
samples. Owner result validation verifies every field against the requested
case and independently reconstructed fixture contract.

The human receipt's required sections are campaign identity/protocol;
section-specific provenance and assembly; grids/controls/seed allocation;
coverage/accounting/resume history; effective routes and semantic witnesses;
raw-sample projection and uncertainty; argmin/threshold decisions and
contradictions; owner/composite strict validation; and limitations. Each
numeric table row has a resolvable raw-result key. The receipt does not add a
second manually maintained selector inventory or claim unmeasured optima.

## 9. Behavior identity, artifacts, and atomic cutover

The core behavior token is `tuning-calibration-v4`, because the producing tool
adds extent evaluation, output/coverage semantics, and resumable execution.
The first algebra format-2 producer uses the already owner-reserved
`algebra-tuning-calibration-v1` token. Envelope format and both selector-section
versions remain unchanged. New owner-specific case/raw-result schema tokens
identify their exact field shapes; no historical result is relabeled.

The behavior-source manifest hashes each owner harness and every extracted
support source module it uses, as well as the committed candidate production
bodies, dot observation correction, and shared TRSM/GEMM observation-policy
specializations. Runtime provenance records that manifest's
digest and the executable digest in the execution log and receipt. No new
wire field is added to the generic measurement wrapper. The support extraction therefore cannot
change measurement behavior while retaining an unqualified harness-only source
identity. Protocol/narrative hashes identify the declaration separately from
the producing behavior; documentation-only changes do not retroactively
invalidate committed measurements.

To build clean producing source while current readers still use the v3 owner,
the premeasurement commit permits v3 only through the named
`PREPUBLICATION_HARNESS_SCHEMA` boundary in the core codec. Its tracked removal
condition is the single a835 publication commit: measured v4 owner, algebra
owner, complete envelope, every current reader, baked constants/citations,
and v3 rejection test land together. No v3 compatibility token remains in the
final codec. Historical eaae/389 envelopes remain immutable evidence at their
committed paths and do not become current-codec fixtures.

Publication updates the selected GEMM bake only. The unconditional
`GEMM_ROW_TILE_DEFAULT`/`GEMM_COL_TILE_DEFAULT` declarations and the typed
conservative section retain their premeasurement values in ordinary and
`gf2_tuning_baked` builds. A cfg-independent conservative-table witness and
the baked production-site witnesses jointly verify this distinction, including
when the selected tile pair differs from the conservative pair.

The run ID is `gf2-a83583e0-<UTC-stamp>-<launcher-pid>`, fixed before launch.
Repository destinations are derived from that one ID:

| Artifact | Destination |
|---|---|
| Core-owner envelope | `crates/gf2-core/data/tuning-profiles/<run-id>.json` |
| Algebra-owner envelope | `crates/gf2-algebra/data/tuning-profiles/<run-id>.json` |
| Complete envelope | `dev/reference_data/tuning-profiles/<run-id>.json` |
| Receipt | `dev/benchmarks/tuning_profiles/<run-id>.md` |
| Execution log | `dev/benchmarks/tuning_profiles/<run-id>-execution.log` |
| Raw accepted results and checkpoints | `dev/benchmarks/tuning_profiles/<run-id>-results/` |
| Build/provenance/composition/validation record | `dev/benchmarks/tuning_profiles/<run-id>-session/` |
| Repository-relative checksum manifest | `dev/benchmarks/tuning_profiles/<run-id>.sha256` |

Each owner writes to an absent stage destination by create-new temporary file,
flush/sync, atomic rename, and strict owner-only reopen. The result must have
exactly its owner ID. No artifact is publishable until both owners and all
717 cells validate. A driver interruption may leave a stage candidate, but
the publication manifest has no accepted terminal state until the complete
bundle passes; consumers never select a stage file as current authority.

The existing composer strictly reads both owner artifacts with both codecs,
rejects duplicate or unexpected IDs, records each canonical section wrapper
and SHA-256, emits the complete format-2 envelope, and strictly reopens it.
It verifies exactly both IDs and byte-identical complete wrapper identity
for each owner: schema, measurement, selectors, and hash. It changes only
assembly provenance and the envelope content digest. It does not remeasure,
retag, fill omitted fields, or manufacture an inherited algebra section.

The receipt reports protocol and producing commits; clean-state evidence;
separate core/algebra executable and behavior identities; toolchain, features,
host/governor/affinity/thread facts; observed lock holds; seed root, mixer,
stream inventories and operand digests; complete grids and controls; every
raw window and its calls; per-execution medians, ratio ranges and IQRs;
schedule classes; unique minima, ties, non-monotone curves, cross-stratum
conflicts, joint M4RM decisions, and all fallbacks/omissions; exact accepted
and attempted accounting; interruptions/resume provenance; semantic and
effective-route evidence; owner/content/wrapper/artifact hashes; and separate
measurement versus assembly timestamps/revisions. The raw records are the
source of numerical tables; the receipt cites or projects them.

Independent validation recomputes the accounting, deterministic seed/operand
identities, every extent estimator and threshold decision, schedule plateaus,
all fallbacks, measured-set complement, canonical wrapper equality, and every
hash. It does not treat a scratch validator's own untested assertions as
evidence. Rust strict reopen remains authoritative for codec semantics.
Validation also verifies log/checkpoint completeness and the absence of a
publishable partial campaign. Successful stage files are copied byte-for-byte
to unique absent repository destinations, compared with stage, checksummed,
and committed atomically with the current-reader cutover. Existing evidence
is never overwritten.

## 10. Exact implementation boundary and premeasurement checks

The following paths form the complete boundary for the selected architecture.
A newly discovered necessary path is surfaced to the lead
before edits; it is not silently taken from another issue's completed scope.

- Workspace/dev-only support wiring: `Cargo.toml`, `Cargo.lock`,
  `dev/tools/tuning-campaign-support/Cargo.toml`, and its
  `src/lib.rs`, `src/transport.rs`, `src/timing.rs`, `src/seed.rs`,
  `src/statistics.rs`, `src/journal.rs`, plus focused tests within that crate.
- Owner producers and their canonical tests:
  `crates/gf2-core/benches/tuning_calibration.rs`,
  `crates/gf2-core/tests/tuning_calibration_harness.rs`,
  `crates/gf2-core/Cargo.toml`,
  `crates/gf2-algebra/benches/tuning_calibration.rs`,
  `crates/gf2-algebra/tests/tuning_calibration_harness.rs`,
  `crates/gf2-algebra/Cargo.toml`.
- Baked shared production bodies and candidate/effective witnesses:
  `crates/gf2-core/src/field/matrix.rs`,
  `crates/gf2-core/src/field/expr.rs`,
  `crates/gf2-core/src/field/vec.rs`,
  `crates/gf2-core/tests/tuning_extent_candidates.rs`,
  `crates/gf2-core/tests/tuning_conservative_cfg.rs`,
  `crates/gf2-core/tests/field_vec_dot_chunk.rs`,
  `crates/gf2-core/tests/gemm_tiles_baked.rs`, and
  `crates/gf2-core/tests/field_vec_baked.rs`.
- Runtime extent observations and focused semantic witnesses only:
  `crates/gf2-core/src/matrix.rs`,
  `crates/gf2-core/src/alg/m4rm.rs`,
  `crates/gf2-core/src/compute/field.rs`,
  `crates/gf2-core/src/field/triangular.rs`,
  `crates/gf2-core/src/field/inverse.rs`,
  `crates/gf2-core/src/field/ple.rs`,
  `crates/gf2-core/tests/tuning_extent_runtime.rs`,
  `crates/gf2-core/tests/tuning_profile_triangular_base_install.rs`,
  `crates/gf2-core/tests/tuning_profile_triangular_base_install_above.rs`,
  `crates/gf2-algebra/src/permanent/parallel_bipedal3.rs`, and
  `crates/gf2-algebra/tests/tuning_profile_permanent_install.rs`.
- Behavior-token acceptance and current measured readers:
  `crates/gf2-core/src/tuning/mod.rs`,
  `crates/gf2-core/src/tuning/baked.rs`,
  `crates/gf2-core/src/kernels/backend.rs`,
  `crates/gf2-core/tests/tuning_envelope_v2.rs`,
  `crates/gf2-core/tests/support/measured_format2.rs`,
  `crates/gf2-core/tests/tuning_profile_committed.rs`,
  `crates/gf2-core/tests/backend_selection_baked.rs`,
  `crates/gf2-core/tests/prime_route_baked.rs`,
  `crates/gf2-algebra/src/tuning.rs`,
  `crates/gf2-algebra/tests/tuning_section.rs`, and
  `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`.
- Driver, independent evidence validation, and CI integration:
  `dev/scripts/tuning-extent-campaign.sh`,
  `dev/scripts/validate-tuning-extent-campaign.py`, and
  `scripts/cargo-ci.sh`.
- Present-tense API/provenance citations:
  `crates/gf2-core/docs/KERNEL_OPTIMIZATION.md` and rustdoc in the above
  changed production modules; append-only amendments to
  `dev/active/7d824b2f/design.md` and `dev/active/3fa7c9d0/design.md`;
  this protocol; the exact generated evidence destinations in §9.

The generic profile registry/process cell, composer algorithm, isolated kernel
interfaces, finite-field semantics, conservative values, threshold
comparison meanings, and runtime-versus-baked classifications are unchanged.
The GEMM declaration/selected-alias separation in §1.1 preserves those
conservative values under either cfg and is included in this boundary.
The composer executable is rebuilt against the current codecs and verified;
an implementation defect requiring a composer source change is surfaced before
altering its boundary. No proof code or extraction model changes are planned.

Test-first evidence establishes every candidate's production path, all seven
GEMM sites, dot fallback behavior, the observer cost correction, and the
cfg-independent conservative section. The conservative-table witness runs
under both ordinary and `gf2_tuning_baked` builds, compares the typed section
relationally with the canonical conservative owner, and verifies that the
conservative GEMM entries name the unconditional defaults. A nondefault
candidate/bake witness proves that production selection can differ while that
section stays identical; it does not duplicate a field-name or literal-value
inventory. Both cfg invocations are required CI steps. Shared
behavior suites cover candidate equivalence and bit boundaries 0, 1, 63, 64,
65 where applicable. Ordinary tests use small fixtures, preserve the fast
tier budget, and do not run the full large campaign. Large fixture oracle and
capability checks belong to the explicit prepared-host action.

Focused triangular tests prove that route reporting alone leaves executed
observations empty, every public triangular operation records its top-level
route once, and recorded/no-op solve specializations return the same result
for recursive and blocked routes and several panel extents. The no-op test
resets and inspects all triangular/GEMM observers around the selected solve
subtree and requires no publications there. Structural/codegen review checks
that recursive helpers are pure with respect to the global observers and
that the timed specialization contains no candidate-dependent observation
atomics. The retained threshold harness tests exercise the corrected timing
adapter as well as its unchanged forcing/crossover semantics.

Before measurement, meaningful tests cover owner admissibility, exact grid
and control manifests, candidate and field identity, deterministic stream
allocation, all raw-window/statistic rules, plateaus, U shapes, multiple minima,
endpoint minima, cross-stratum conflicts, coupled GEMM selection, failed M4RM
combination fallback, strict reopen-before-install, non-installed and malformed
child rejection, explicit CLI/libtest modes, incomplete result/checkpoint
recovery, budget exhaustion followed by a same-identity resumed session,
cumulative session accounting, append-only log path announcement, terminal records, and exact
codec-derived coverage. Existing suites receive only the changes required by
this authorized mechanism or atomic current-reader migration.

Validation runs `git diff --check`, wrapped formatting, focused release-profile
tests with `cargo-budget.sh --test`, workspace API documentation, and the
complete repository CI contract. Then the exact Rust 1.95 release executables
run untimed `--self-check`, `--list-grid`, and `--capability-report`, whose
output explicitly states whether it checks representative capabilities or the
full grid. All checks and design review pass before the first timed child.
The campaign does not silently retry until an empirical result changes.
