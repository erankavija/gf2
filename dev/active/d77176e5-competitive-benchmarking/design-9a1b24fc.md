# Design: reused-workspace batch encoding through shortening

Design for @/issue/9a1b24fc. Shortening adapts the mother's batch encoder
through bounded, reusable staging. This supplies charter decision D-7's
output contract (`dev/research/2caf738d-charter.md`) for @/issue/4443de83.

## Verified basis

The main snapshot is `c5c09600a`. The implementation basis also includes
`worktree-agent-6379e04e` at `22378849b` and
`worktree-agent-7fbd1042` at `c97b4c3d0`; both diffs against main are reviewed.
The former stores the encode stamp, hoists the typed-field decision, avoids
redundant rank-index invalidation and selects block folding below one chunk.
Its `clmul_fold_accumulators(redundancy, dimension)` returns `usize`, with
zero selecting block folding. This design preserves those changes.

The latter snapshot is incomplete: tests name `clmul_fold_frames`, selector
frame overrides and a `fold_frames` kernel field whose definitions are absent.
They belong to @/issue/7fbd1042's contract, not its usable API. Integration
targets its completed `SystematicKernel::encode_batch_family` arm.

On main, `traits::block::BlockEncoder` in
`crates/gf2-coding/src/traits.rs` exposes only `encode` and `encode_into`.
`BchCode`'s batch methods and `BchEncodeWorkspace` live in
`crates/gf2-coding/src/bch/encode.rs`. In
`crates/gf2-coding/src/bch/dvb_t2/mod.rs`, `DvbT2BchCode` aliases
`Shortened<LayoutView<BinaryPrimeExt, BitVec, BitMatrix>>`; its mother declares
`DVB_T2_LAYOUT`. `ShortenedData::SystematicRestriction`, `CopyRun`,
`message_runs`, `codeword_runs` and `local_map` already exist in
`crates/gf2-coding/src/transform/mod.rs`.

## Public interface (REQ-01)

Two placements are considered:

- Inherent methods only on `Shortened<LayoutView<...>>` preserve BCH's layout
  argument but exclude other mothers and duplicate the shortening convention.
  Overriding the declared layout disagrees with its encoder and matrix traits.
- An opt-in capability beside `BlockEncoder`, implemented by `BchCode`,
  `LayoutView` and generic `Shortened<C>`, expresses batch encoding in each
  code's declared coordinates, sharing adaptation across fields and mothers
  under @/inv/convention-convergence and @/inv/library-first-generality.

Choose the capability. These signatures are **proposed additions** to
`traits::block`:

```rust
pub trait BatchEncoder: BlockEncoder {
    type EncodeWorkspace;
    type EncodeError: From<CodeError>;

    fn encode_workspace(&self) -> Self::EncodeWorkspace;
    fn encode_workspaces(&self, workers: NonZeroUsize)
        -> Vec<Self::EncodeWorkspace>;
    fn encode_batch_granularity(&self) -> NonZeroUsize;
    fn encode_batch(&self, messages: &[Self::Symbols])
        -> Result<Vec<Self::Symbols>, Self::EncodeError>;
    fn encode_batch_into(
        &self,
        messages: &[Self::Symbols],
        workspace: &mut Self::EncodeWorkspace,
        codewords: &mut [Self::Symbols],
    ) -> Result<(), Self::EncodeError>;
}
```

`NonZeroUsize` is `std::num::NonZeroUsize`. Provided bodies construct worker
workspaces independently, return granularity one and implement the owning
batch with one workspace and new outputs. Only the constructor and `_into`
are required. Implementors validate counts, lengths and runtime symbol-field
identity before writes; empty slices form a valid empty batch. Rustdoc states
these errors, workspace compatibility and absence of panics for valid inputs.

`BchCode<X, S, M>` uses `BchEncodeWorkspace<S::Word>` and `BchError` with the
existing `FieldExtension`, `SystematicKernel` and `SymbolMatrix` bounds.
Its trait methods forward to its inherent methods with
`SystematicLayout::default()`; its owning override maps `CodeError` into
`BchError`. `LayoutView<X, S, M>` forwards with `self.layout()` instead.
The existing inherent BCH methods retain their signatures and explicit
layout facility. The capability follows `BlockEncoder`'s declared-layout
convention and adds no BCH layout vocabulary to generic code.

Proposed additions in `crates/gf2-coding/src/transform/mod.rs` are:

```rust
pub struct ShortenedEncodeWorkspace<C: BatchEncoder> { /* private fields */ }

impl<C: BatchEncoder> Shortened<C> {
    pub fn encode_workspace_with_batch_capacity(
        &self, frames: NonZeroUsize,
    ) -> ShortenedEncodeWorkspace<C>;
}
```

`Shortened<C>` implements the capability with workspace
`ShortenedEncodeWorkspace<C>` and error `C::EncodeError`; its granularity
query delegates to the mother. Scratch holds the mother workspace,
two vectors of preallocated mother-sized symbol sequences,
and one immutable zero mother message. The default tile capacity is the
mother's granularity; the explicit constructor overrides it. A proposed
provided method on `SystematicKernel<F>` has this signature:

```rust
fn batch_granularity(plan: &SystematicPlan<'_, F>) -> NonZeroUsize;
```

Its default returns one; the `BitVec` override takes the maximum frame group
among its available batch families. It reuses `BITSLICE_LANES` and the
completed lockstep sizing rule, rather than copying their constants.
`BatchEncoder`'s BCH implementations delegate their query to that method.

One caller serves both `BinaryBchCode` and `DvbT2BchCode`:

```rust
fn encode<C: BatchEncoder>(
    code: &C, messages: &[C::Symbols],
    scratch: &mut C::EncodeWorkspace, output: &mut [C::Symbols],
) -> Result<(), C::EncodeError> {
    code.encode_batch_into(messages, scratch, output)
}
```

The example imports `gf2_coding::traits::block::BatchEncoder`; its caller
creates scratch with `BatchEncoder::encode_workspace(&code)` and sizes each
output to `code.n()`. Fully qualified calls such as
`BatchEncoder::encode_batch(&code, &messages)` work on both concrete types.
A concrete BCH method call resolves to its existing layout-taking inherent
method. Per worker, use
`code.encode_workspaces(workers)` and pass each disjoint input/output
partition with its own scratch to this function; scheduling is the caller's.

**Lasting interface decision for the lead:** approve declared-layout trait
calls as REQ-01's common form, including the associated error, granularity
query and capacity override. Exact unchanged concrete BCH syntax requires
a layout policy on shortened codes. Recommend the trait form; a dummy layout
argument weakens the coordinate contract.

## Encoding algorithm and complexity (REQ-04)

For `SystematicRestriction`, the constructor zeroes every staging message
once and allocates every staging output and mother workspace once. Each call
validates the complete external batch and staging shapes first. An empty
mother batch validates the mother workspace before any external output is
written. The typed-field decision follows the hoisted check of the BCH base.

For each tile, copy each shortened message through `message_runs` into its
zero mother message with `SymbolSequence::copy_range`. Pass the active
prefixes of both staging vectors directly to the mother's
`BatchEncoder::encode_batch_into`. Copy only `codeword_runs` back to caller
outputs. Restore the written message runs from the immutable zero message
before returning or reusing the tile, including on a returned mother error.
Removed message coordinates stay zero; no whole mother-message clearing or
per-message vector construction occurs. Mother outputs are overwritten, not
cleared. Use the immediate mother's runs/`local_map`, never the composed
ancestor provenance map; nested shortening therefore composes correctly.

The zero-between-tiles invariant makes staging independent of the shortening
set. Compatible workspaces can serve different restrictions of the same
mother without a shortening fingerprint. Mother workspace validation remains
the mother's responsibility, including BCH's stored stamp comparison.
Lengths and runtime field identity of staging are checked before use.
Scratch fields are private. `RankDerived` uses the existing dense-generator
multiply factored into a shared helper; it needs no mother encode and does
not switch derivations or materialize another matrix. Its constructor can
retain the same scratch representation for a uniform workspace type.

Let the mother have parameters $k_m,n_m$, the shortening $k_s,n_s$, and tile
capacity $T$. Rustdoc states: adaptation costs $O(k_s+n_s)=O(n_s)$ symbol
moves per message plus one mother encode at the selected family's batch
granularity; scratch is $O(T(k_m+n_m)+k_m)$ symbols plus the mother workspace,
independent of batch length. Packed contiguous runs use word copies including
unaligned ends. The rank-derived arm costs $O(k_s n_s)$ field operations.
The mother encoder can write removed coordinates in workspace storage; none
is allocated or copied into an external intermediate per message.

Every tile reaches the existing BCH batch path, preserving one validation
and preparation per tile, the stored-stamp check, and
`SystematicKernel::encode_batch_family`. Thus table, bit-sliced, fold and
reference families retain their shared dispatch and scalar kernel fallback;
the completed lockstep arm receives full or partial frame groups. Selection
sees tile length, not the whole external batch. The capacity override exposes
that performance tradeoff; no singleton loop silently replaces batch kernels.
Fold selector overrides retain BCH's existing one-time preparation behavior.
Under stable selectors, reused buffers allocate nothing on DVB-T2 batches.

## Conformance and allocation tests (REQ-02, REQ-03)

Tests precede implementation. Extend
`crates/gf2-coding/tests/bch_encode_dispatch.rs`. For every configuration in
`crates/gf2-coding/src/bch/dvb_t2/params.rs`, both frame sizes and batches
0, 1, 63, 64, 65, compare both batch forms against `BlockEncoder::encode` on the
actual `DvbT2BchCode`. Include zero, one, seeded random and single-set-bit
messages at message coordinates 0, 1, 63, 64 and the final coordinate.

Iterate `EncodeFamily::REGISTERED`, checking availability through the mother
code and its declared layout. Extend
`crates/gf2-coding/tests/support/fresh_tuning_process.rs` with fresh profile
cases selecting each available family, including the reference, from batch
one; assert the selected family rather than infer coverage from output.
Run each case with detected kernels and with
`test_support::force_scalar_encode_kernels(true)` from
`crates/gf2-coding/src/test_support.rs`, restoring its previous setting.
Also test capacities one and the default, capacity boundaries, changed input
on repeated calls, reused nonzero outputs, canonical tail padding, worker
partitions, foreign mother workspaces, and invalid lengths/fields leaving
outputs unchanged. An error-returning mother checks staging cleanup; reuse
across compatible shortening sets checks that removed coordinates stay zero.
Small generic mothers cover noncontiguous removals, nested shortening,
`RankDerived`, nonbinary symbols and zero-dimensional codes.

Reuse `thread_allocations::{CountingAllocator, count, Counts::is_zero}` from
`dev/tools/thread-allocations/src/lib.rs`, already a dev dependency of
gf2-coding. Extend `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs`
on the lockstep base, which installs that global allocator. Construct code,
messages, caller outputs and workspaces outside `count`; count only the
caller-buffer batch call. Assert zero allocations and reallocations, not
merely unchanged pointers. Count first use under structural selectors and
repeated calls for every family/scalar/configuration/batch case; prepare any
nonstructural fold override outside the repeated-call count, as BCH requires.
Assertions, comparisons and fresh-process output stay outside the interval.

Keep cases separate enough to respect fast-tier budgets; any case exceeding
them carries a descriptive slow-tier ignore without reducing the matrix.
The host-gated TP04/TP05 evidence in
`crates/gf2-coding/tests/dvb_t2_bch_verification.rs` remains the external
standard oracle; its encoding case also compares the new batch output when
streams are present. Seeded equality supplies behavioral coverage only.

## Implementation order and remaining choices

1. Add failing cases in the existing dispatch, allocation, fresh-process and
   DVB-T2 verification test files above; add generic shortening cases in
   `crates/gf2-coding/src/transform/mod.rs`.
2. Add the capability and rustdoc in `crates/gf2-coding/src/traits.rs`; add
   BCH/view forwarding and the representation granularity query in
   `crates/gf2-coding/src/bch/encode.rs`, on the completed lockstep base.
   Other encoders remain outside the opt-in capability.
3. Add the workspace, capacity constructor and generic capability impl in
   `crates/gf2-coding/src/transform/mod.rs`; share the rank-derived arithmetic
   helper and document both complexity arms. Add a batch workflow to
   `crates/gf2-coding/src/bch/dvb_t2/mod.rs` rustdoc. All code uses safe Rust.

Integration waits for the completed lockstep seam and sizing rule, then runs
the common family matrix. Recommend bounded tiles with a capacity override;
profile changes use the existing preparation boundary outside measurement.
The interface decision above is the only lead escalation. Matrix traits,
`ShortenedData` and `Shortened::encode_into`'s observable behavior remain
available to @/issue/92ec2e2b and @/issue/2bca2b28.
`experiments/encode-compare/src/cutover/current.rs` changes only under
@/issue/4443de83, with library scratch constructed outside its timed call.

## Cost of the single-form alternative

The alternative to the inherent-plus-trait form above is one batch form: the
`BatchEncoder` methods replace `BchCode`'s layout-taking batch methods, and a
non-default layout is reached through `LayoutView::new(code, layout)`, which
already implements `BlockEncoder` in its declared layout. Inventory at the
`worktree-agent-7fbd1042` head `c97b4c3d0`:

- `BchCode` loses the `layout` parameter, or the method, on `encode_batch`,
  `encode_batch_into`, `encode_batch_parallel`, `encode_batch_parallel_into`,
  `encode_batch_family_into`, `encode_family_available` and
  `selected_encode_family`; the last four move to `LayoutView` or take the
  declared layout. `LayoutView::new` takes the code by value, so a caller
  holding a `BchCode` in one layout and batching in another clones it once.
- @/issue/6379e04e adds `clmul_fold_accumulators` and a selector override; its
  public additions are unaffected. Its test changes in
  `crates/gf2-coding/tests/bch_encode_dispatch.rs` and
  `crates/gf2-coding/tests/support/fresh_tuning_process.rs` call the
  layout-taking methods and change with them. @/issue/7fbd1042 adds no public
  item yet; its lockstep tests change the same way.
- Callers in the library tree: `crates/gf2-coding/src/test_support.rs`, the
  benches `crates/gf2-coding/benches/batch_operations.rs` and
  `crates/gf2-coding/benches/bch_encode_w1.rs`, the example
  `crates/gf2-coding/examples/bch_binary_quickstart.rs`, and the tests
  `crates/gf2-coding/tests/backend_integration.rs`,
  `crates/gf2-coding/tests/bch_encode_dispatch.rs`,
  `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs` and
  `crates/gf2-coding/tests/support/fresh_tuning_process.rs`. Each call either
  drops a default layout argument or wraps the code in a `LayoutView`.
- Experiments: `experiments/encode-compare/src/cutover/current.rs` batches in a
  non-default layout and moves to a `LayoutView` built outside the timed call.
  `experiments/encode-compare/cutover/pre.rs` builds the pre-cutover revision
  and is unaffected.
- Documentation: `docs/how-to/select-acceleration.md` names
  `BchCode::encode_batch_family_into`, and
  `docs/reference/performance-evidence.md` names `BchCode::encode_batch` as the
  timed gf2 path.
- `LdpcCode::encode_batch` (`crates/gf2-coding/src/ldpc/core.rs`) and the
  Richardson-Urbanke `encode_batch`
  (`crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs`) are a third
  batch form: owned output without a workspace or `Result`, the latter taking a
  compute backend. Neither form implements the capability here; under a strict
  reading of @/inv/convention-convergence they become implementors or carry a
  tracked convergence condition.

Landing the single form before @/issue/6379e04e and @/issue/7fbd1042 rewrites
their tests once; landing it after rewrites the merged tests and the cutover
harness again.
