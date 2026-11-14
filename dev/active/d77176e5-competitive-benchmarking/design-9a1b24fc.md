# Design: one batch-encoding contract with reusable workspaces

Design for @/issue/9a1b24fc, using the owner's REQ-01 through REQ-06.
`BatchEncoder` owns batch encoding in declared coordinates. Shared shortening
supplies charter D-7's caller-buffer contract (`dev/research/2caf738d-charter.md`).

## Verified basis

The worktree basis is `52904933d`; main is `9464e4ee0`. Source inspection
includes `git diff main...<branch>` for `worktree-agent-6379e04e` at
`22378849b`, `worktree-agent-7fbd1042` at `c97b4c3d0`, and
`worktree-agent-243d1160` at `55ca8205c`. Paths below exist at the worktree
basis unless a branch is stated. Interface definitions labelled additions
are the implementation contract, not APIs supplied by those snapshots.

`traits::block::BlockEncoder` in `crates/gf2-coding/src/traits.rs` supplies scalar
encoding. BCH batch methods and `LayoutView` live in `crates/gf2-coding/src/bch/encode.rs`. In
`crates/gf2-coding/src/bch/dvb_t2/mod.rs`, `DvbT2BchCode` is
`Shortened<LayoutView<BinaryPrimeExt, BitVec, BitMatrix>>`; the mother declares
`DVB_T2_LAYOUT`. The shared transform stores `SystematicRestriction`,
`message_runs`, `codeword_runs`, `CopyRun` and `local_map`.

The batch method in `crates/gf2-coding/src/ldpc/core.rs` belongs to `LdpcEncoder`;
`LdpcCode` supplies code data and matrix access. Its encoder chooses IRA or
`RuEncodingMatrices`. `LdpcEncoder` implements the bit-only compatibility trait;
RU has inherent scalar `encode`. The design includes `LdpcCode` and actual
`LdpcEncoder` callers without requiring a scalar compatibility cutover.

@/issue/6379e04e supplies the stored encode stamp, hoisted typed-field check,
rank-index invalidation guard and short-message block fold. Its
`clmul_fold_accumulators(redundancy, dimension)` returns `usize`, including
zero. @/issue/243d1160 supplies inline `BitVec` storage and a batch allocation
witness. @/issue/7fbd1042's snapshot contains tests referring to absent
`clmul_fold_frames`, selector frame overrides and `fold_frames` definitions;
these are integration requirements, not callable APIs at that snapshot.
The shared `SystematicKernel::encode_batch_family` seam is the encode path.

## Public interface (REQ-01, REQ-05)

The final addition to `gf2_coding::traits::block` uses `std::num::NonZeroUsize`.
No inherent alias exposes these methods. The `BlockCode` supertrait lets code
values keep preprocessing in scratch while scalar encoders keep it in themselves;
batch encoding is an independent capability beside `BlockEncoder`.

```rust
/// Batch encoding in the code's declared symbol field and coordinate layout.
/// Results preserve input order and equal the corresponding scalar encoder.
/// Invalid shapes, runtime fields and incompatible workspaces return errors
/// before output changes. Other execution errors can leave a written prefix.
/// Empty batches are valid; parallel calls still require a workspace.
/// Implementations document preparation, complexity, compatibility and allocation.
/// Valid calls do not panic.
pub trait BatchEncoder: BlockCode {
    type EncodeWorkspace;
    type EncodeError: std::error::Error + From<CodeError>;

    fn encode_workspace(&self) -> Result<Self::EncodeWorkspace, Self::EncodeError>;
    fn encode_workspaces(&self, workers: NonZeroUsize)
        -> Result<Vec<Self::EncodeWorkspace>, Self::EncodeError>;
    fn encode_batch_granularity(&self) -> NonZeroUsize;
    fn encode_batch(&self, messages: &[Self::Symbols])
        -> Result<Vec<Self::Symbols>, Self::EncodeError>;
    fn encode_batch_into(
        &self, messages: &[Self::Symbols],
        workspace: &mut Self::EncodeWorkspace, codewords: &mut [Self::Symbols],
    ) -> Result<(), Self::EncodeError>;
    fn encode_batch_parallel(
        &self, messages: &[Self::Symbols], workers: NonZeroUsize,
    ) -> Result<Vec<Self::Symbols>, Self::EncodeError>
    where Self: Sync, Self::Symbol: Send + Sync, Self::Symbols: Send + Sync,
          Self::EncodeWorkspace: Send, Self::EncodeError: Send;
    fn encode_batch_parallel_into(
        &self, messages: &[Self::Symbols],
        workspaces: &mut [Self::EncodeWorkspace], codewords: &mut [Self::Symbols],
    ) -> Result<(), Self::EncodeError>
    where Self: Sync, Self::Symbol: Send + Sync, Self::Symbols: Send + Sync,
          Self::EncodeWorkspace: Send, Self::EncodeError: Send;
}
```

Only `encode_workspace` and `encode_batch_into` require implementation.
Provided constructors independently prepare workers and propagate preparation
errors. Owning calls construct outputs and scratch; granularity defaults to
one, an accepted scheduling unit rather than a preferred kernel tile size.
Parallel defaults validate the whole batch and every workspace (through an
empty `_into` call) before writes, then use balanced contiguous partitions,
Rayon with `parallel`, and index order otherwise. Empty workspace slices return
`CodeError::BufferLengthMismatch`; excess workspaces remain unused. Common
shape/field validation and partitioning live in `traits.rs`, not per encoder.

Rustdoc specifies message length `k()`, output length `n()`, equal slice counts,
field identity and workspace compatibility. Errors convert `CodeError`; allocation
failure has Rust's allocator behavior. Construction occurs outside measurements.
Output reuse does not promise allocation freedom. Owning cost includes output
construction and preparation; `_into` costs validation plus encodes. Parallel cost
adds worker validation and scheduling. Execution errors leave scratch reusable.

`BchCode<X,S,M>` and `LayoutView<X,S,M>` implement the trait with
`BchEncodeWorkspace<S::Word>` and `BchError`, retaining the existing
`FieldExtension`, `SystematicKernel` and `SymbolMatrix` bounds. Constructors
return `Ok` around prepared scratch. BCH uses the default layout, the view
uses `self.layout()`; both call shared private plan-based routines. Owning
BCH encoding retains thread scratch, including the allocation behavior used
by @/issue/243d1160. BCH parallel overrides preserve one family selection for
the whole batch and existing worker behavior, under the trait's Send/Sync
bounds; the helper transports whole workspaces rather than imposing stronger
bounds on serial implementations.

Placement of every BCH layout-taking batch API is fixed:

- `encode_batch`, `encode_batch_into`, `encode_batch_parallel` and
  `encode_batch_parallel_into` are trait methods, with no layout parameter.
- `encode_workspace` and `encode_workspaces` are trait methods too.
- `encode_batch_family_into(family, messages, workspace, codewords)`,
  `encode_family_available(family)` and `selected_encode_family(batch_len)`
  remain BCH-specific inherent operations on both `BchCode` and `LayoutView`,
  using their declared layout. Families describe BCH kernels, so they do not
  enter the field-generic trait. These signatures replace the layout-taking
  signatures; no forwarding aliases preserve them.

A non-default layout uses the owning `LayoutView::new(code, layout)` outside
timing; callers retaining the original clone once. Import `BatchEncoder`, build
`code.encode_workspace()?`, then call
`code.encode_batch_into(messages, &mut workspace, outputs)?` for every implementor.
Scalar layout methods and the representation-level `SystematicKernel` stay distinct.

## Encoding algorithm and complexity (REQ-03, REQ-04)

Add `ShortenedEncodeWorkspace<C>` in
`crates/gf2-coding/src/transform/mod.rs` and implement `BatchEncoder` for
`Shortened<C>` where `C: BatchEncoder + BlockEncoder`. The error is
`C::EncodeError`. The workspace holds the mother workspace, preallocated
mother-message and mother-codeword vectors, and an immutable zero message.
`encode_batch_granularity()` is one; there is no new kernel sizing interface.
The default staging capacity is one. The added inherent constructor
`encode_workspace_with_batch_capacity(frames: NonZeroUsize)` returns
`Result<ShortenedEncodeWorkspace<C>, C::EncodeError>` and permits actual
mother batches. Scheduling granularity and staging capacity are distinct.
The cutover caller sizes staging for its batch; rustdoc states the singleton default.

For `SystematicRestriction`, construction zeroes staging messages once.
Each call validates every external shape/field and checks the mother workspace
through an empty mother batch before any output write. For each bounded tile,
copy only `message_runs` into zero staging, call the mother's trait `_into`
on active prefixes, and copy only `codeword_runs` into the caller's outputs.
Restore written message runs from the zero message on success and on a
returned mother error. Mother outputs are overwritten rather than cleared.
Removed coordinates exist only inside workspace storage. No mother-size
clearing, message cloning or intermediate allocation occurs per codeword.
Use immediate-mother runs and `local_map`, so nested shortening composes.

Private scratch checks mother compatibility (BCH's stored stamp), staging
dimensions and field identity. Zero-between-tiles staging permits different
restrictions of one compatible mother without a shortening fingerprint. `RankDerived` uses
the stored dense generator through the scalar multiplication helper shared
with `encode_into`; it needs no mother encode or new generator materialization.

For mother parameters $k_m,n_m$, shortened parameters $k_s,n_s$ and capacity
$T$, rustdoc states $O(k_s+n_s)=O(n_s)$ adaptation per message plus one mother
encode, and $O(T(k_m+n_m)+k_m)$ scratch symbols plus mother scratch. Fixed
capacity bounds storage independently of batch length. Packed runs use
`SymbolSequence::copy_range`, including unaligned ends. Rank-derived encoding
costs $O(k_s n_s)$ field operations. Constructor cost includes staging
initialization and mother preparation. Family selection sees each active
tile length. Table, bitslice, fold and reference kernels remain dispatched
through the shared mother batch seam, including scalar fallback. Structural
selectors require no allocation on first use; fold overrides use BCH's
preparation boundary before repeated-call measurements.

## LDPC and RU workspaces (REQ-05)

Add `BlockCode` and `BatchEncoder` impls for `LdpcCode`, `LdpcEncoder` and
`RuEncodingMatrices`, with `Fp<2>` and `BitVec`. Keep scalar behavior intact;
`LdpcCode`'s batch oracle is `LdpcEncoder` for that code. RU's declared layout
is its `systematic_cols`/`parity_cols`, not an assumed message prefix.
Remove both inherent batch methods, including RU's backend-taking signature.

Add `LdpcEncodeError` in `ldpc/core.rs`: wrapped `CodeError`, wrapped
`PreprocessError`, and workspace incompatibility. `LdpcCode`'s fallible
workspace constructor chooses IRA or fallible RU preprocessing once, storing
that prepared implementation and its scratch in `LdpcEncodeWorkspace`.
A private typed identity shared by code clones binds cached preprocessing
to its code; equal dimensions alone do not establish compatibility.
`LdpcEncoder` uses the same workspace type and prepared implementation;
its constructor reuses `Arc`-held preparation. Errors return without the panicking
`LdpcEncoder::new` path; scalar and batch construction share the fallible builder.
Preparation costs $O(\operatorname{nnz}(H))$ for IRA and
$O(mn\min(m,n))$ for RU's row reduction, plus scratch initialization.

IRA scratch holds the syndrome and accumulated parity buffers, each of
length `m()`. Factor a buffer-writing helper in
`crates/gf2-coding/src/ldpc/encoding/ira.rs`, shared by scalar and batch
encoding. Copy information bits and write accumulated parity into sized
outputs. `_into` is allocation-free after construction on IRA; per-message
work is $O(\operatorname{nnz}(H)+n)$ and scratch is $O(m)$ bits.

Add `RuEncodeWorkspace` in
`crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs`, re-exported by
`crates/gf2-coding/src/ldpc/encoding/mod.rs`. It holds dimensions and an
`Arc<dyn ComputeBackend>`; coefficients and column maps stay in the encoder.
The default backend is `CpuBackend::new()`. Add
`RuEncodeWorkspace::set_backend(Arc<dyn ComputeBackend>)`; LDPC scratch
exposes its RU workspace for the same configuration. Workers each own
scratch and can share the backend's Arc. RU scratch is compatible with
matching dimensions because it holds no code-specific coefficients.
RU's error is `CodeError` (`ShapeMismatch` for incompatible workspace dimensions);
enclosing LDPC wraps it. No backend argument appears on a batch call.

RU calls `ComputeBackend::batch_matvec_transpose` and scatters its parities
into caller outputs. That API in `crates/gf2-core/src/compute/backend.rs`
returns `Vec<BitVec>`; `crates/gf2-core/src/compute/cpu.rs` allocates that
vector and parity results. RU `_into`, including `LdpcCode`/`LdpcEncoder`'s
RU arm, therefore allocates even with reused workspace and preallocated
codewords. REQ-03 applies to DVB-T2 BCH only. Dense CPU RU per-message work
is bounded by $O(k\lceil(n-k)/64\rceil+n)$ word/scatter operations; a batch
of $b$ also holds $O(b(n-k))$ transient parity bits. Other backends document
their product cost. The workspace does not turn backend panics into errors.

## Caller migration and stacked implementation (REQ-06)

The inventory covers `git grep -n 'encode_batch'`, workspace/family queries
and branch diffs. A owns the trait, BCH/view/shortening and BCH callers;
B stacks on A with LDPC/RU and their callers. Tags assign active callers;
paths are repository-relative, including embedded library tests.

| Group | Branch | Caller paths |
|---|---|---|
| Library | A | `crates/gf2-coding/src/bch/encode.rs` (rustdoc/tests), `crates/gf2-coding/src/test_support.rs` |
| Library | B | `crates/gf2-coding/src/ldpc/core.rs` (embedded tests/internal RU call), `crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs` |
| Tests | A | `crates/gf2-coding/tests/bch_encode_dispatch.rs`, `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs`, `crates/gf2-coding/tests/support/fresh_tuning_process.rs` |
| Tests | A; `worktree-agent-243d1160` only | `crates/gf2-coding/tests/bch_encode_batch_allocation.rs` |
| Tests | A+B | `crates/gf2-coding/tests/backend_integration.rs` (BCH/LDPC sections) |
| Benches | A | `crates/gf2-coding/benches/bch_encode_w1.rs` |
| Benches | A+B | `crates/gf2-coding/benches/batch_operations.rs` (BCH/LDPC sections) |
| Benches | B | `crates/gf2-coding/benches/ldpc_throughput.rs`, `crates/gf2-coding/benches/quick_parallel.rs` |
| Examples | A | `crates/gf2-coding/examples/bch_binary_quickstart.rs` |
| Experiments | A | `experiments/encode-compare/src/cutover/current.rs` (primitive views and DVB-T2; preparation outside timing) |
| Docs | A | `docs/how-to/select-acceleration.md`, `docs/reference/performance-evidence.md`, `experiments/encode-compare/README.md` |
| Docs | A | Interface prescriptions in `dev/active/d77176e5-competitive-benchmarking/design-5e71e993.md`, `dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md` |

Imports, fallible constructors, errors and dispatch-record labels use the trait. Evidence names
`BatchEncoder::encode_batch_into` on `LayoutView`/`DvbT2BchCode` for D-7 and
`BatchEncoder::encode_batch` for owning output. BCH family overrides retain
their declared-layout diagnostic names. `experiments/encode-compare/cutover/pre.rs`
compiles against its pinned comparison revision, so its revision-specific
API and evidence labels stay explicit. `benches/linear_codes.rs` has only a
benchmark name matching the search, not a batch-method caller. Charter
`dev/research/2caf738d-charter.md`, handoffs and tracker records describe
versioned decisions/evidence rather than executable interface prescriptions.

A also changes `crates/gf2-coding/src/traits.rs`,
`crates/gf2-coding/src/transform/mod.rs`,
`crates/gf2-coding/src/bch/dvb_t2/mod.rs` and
`crates/gf2-coding/tests/dvb_t2_bch_verification.rs`. B also changes
`crates/gf2-coding/src/ldpc/encoding/ira.rs`,
`crates/gf2-coding/src/ldpc/encoding/mod.rs` and
`crates/gf2-coding/tests/ldpc_encoding_tests.rs`. These, plus their tagged
caller files, are the implementation file sets; B needs no trait-file edit.

B overlaps A in exactly `tests/backend_integration.rs` and
`benches/batch_operations.rs`, both under `crates/gf2-coding/`. Keep those
edits sequential on the stack; file-disjoint delivery is not claimed.
B touches no file in the inspected BCH chain diffs. A overlaps
@/issue/6379e04e and @/issue/7fbd1042 in `src/bch/encode.rs`,
`tests/bch_encode_dispatch.rs` and `tests/support/fresh_tuning_process.rs`
under that crate; lockstep also touches the dispatch allocation test.
A overlaps @/issue/243d1160 in the encoder, shared dispatch/support tests and
its branch-only allocation test. Integrate those branch callers with the
trait cutover; preserve their behavioral and allocation assertions.

## Conformance and allocation tests (REQ-01 through REQ-06)

Implementation starts with failing behavioral cases. A adds one generic batch
contract helper alongside `traits::block::conformance` in `traits.rs`; all
implementors, including B's LDPC/RU tests, run it rather than copy a ladder.
It accepts a scalar-oracle closure and uses lengths 0, 1, 63, 64, 65.

- REQ-01: extend embedded tests in `traits.rs`, `bch/encode.rs` and
  `transform/mod.rs` with generic/UFCS calls, both BCH layouts, per-worker
  workspaces, input order and invalid-shape/field/workspace atomicity. Rustdoc
  workflows import the trait; review verifies no public inherent aliases or
  layout-taking batch signatures remain.
- REQ-02: extend `tests/bch_encode_dispatch.rs` for every configuration in
  `src/bch/dvb_t2/params.rs`, both frame sizes and lengths 0, 1, 63, 64, 65.
  Compare owning/reused/worker outputs with scalar `encode`. Include zero,
  ones, seeded and single-set-bit messages at 0, 1, 63, 64 and the final bit.
  Iterate `EncodeFamily::REGISTERED`; fresh profiles in
  `tests/support/fresh_tuning_process.rs` select each available family,
  assert selection and repeat with `test_support::force_scalar_encode_kernels`.
  Restore the kernel setting. Test capacities one and larger tiles, partial
  tiles, changed inputs, nonzero outputs and canonical tail padding.
- REQ-03: extend `tests/bch_encode_dispatch_allocation.rs`; install/reuse
  `thread_allocations::CountingAllocator` and `count` from
  `dev/tools/thread-allocations/src/lib.rs`. Count only `_into` after code,
  messages, outputs and workspaces exist. Assert zero heap requests on first
  structural use and repeats across the DVB-T2 family/scalar/batch matrix;
  comparisons and assertions stay outside counting. Prepare fold overrides
  outside their repeated-call count. Pointer stability alone is insufficient.
- REQ-04: extend embedded `transform/mod.rs` tests with an observing mother
  that counts encodes and copied ranges, rejects a tile, then permits reuse.
  Cover compatible restriction changes, noncontiguous removals, nesting,
  nonbinary symbols, rank-derived and zero-dimensional cases. Review the
  run-only loop and rustdoc bounds; retain the host-gated TP04/TP05 oracle in
  `tests/dvb_t2_bch_verification.rs` and compare batch output there too.
- REQ-05: extend `tests/ldpc_encoding_tests.rs`, LDPC portions of
  `tests/backend_integration.rs` and embedded `ldpc/core.rs` tests with the
  shared ladder on IRA, non-IRA and non-prefix systematic columns. Exercise
  `LdpcCode`, `LdpcEncoder`, RU, owning/reused/worker forms, changed messages,
  zero tail padding, preprocessing errors and incompatible workspaces. A
  recording backend confirms RU batch-product dispatch and configured backend
  reuse; scalar equality and zero syndrome establish semantics. Allocation
  assertions apply to IRA only; RU's allocating contract stays explicit.
- REQ-06: inventory review covers all paths above, branch-only callers and
  rustdoc links; each executable caller imports the trait and handles Result.
  Check both feature configurations, benches/examples and experiment consumers
  when validating the implementation stack; evidence names the timed contract.

Abbreviated source/test paths above are under `crates/gf2-coding/`. Cases respect
fast-tier budgets; heavy matrices carry descriptive slow-tier ignores with full coverage.
This revision uses source/diff inspection; execution and evidence belong to implementation.
