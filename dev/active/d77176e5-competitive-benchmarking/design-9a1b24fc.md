# Design: one batch-encoding contract with reusable workspaces

Design for @/issue/9a1b24fc and REQ-01 through REQ-06. Shared shortening supplies
charter D-7's caller-buffer contract (`dev/research/2caf738d-charter.md`).
All interface additions below are proposed in the named existing source files.

## Source basis and landing order

`traits::block::BlockEncoder` in `crates/gf2-coding/src/traits.rs` supplies scalar
encoding. BCH kernels, batch methods and `LayoutView` live in
`crates/gf2-coding/src/bch/encode.rs`. `DvbT2BchCode` in
`crates/gf2-coding/src/bch/dvb_t2/mod.rs` is
`Shortened<LayoutView<BinaryPrimeExt, BitVec, BitMatrix>>`, with `DVB_T2_LAYOUT`.
`crates/gf2-coding/src/transform/mod.rs` stores `SystematicRestriction`,
`RankDerived`, `message_runs`, `codeword_runs`, `CopyRun` and `local_map`.
`gf2_core::compute::for_each_partition` in
`crates/gf2-core/src/compute/partition.rs` is the shared partition primitive;
BCH consumes it; the trait defaults use it directly.
`LdpcEncoder` in `crates/gf2-coding/src/ldpc/core.rs` owns the LDPC batch method
and chooses IRA or `RuEncodingMatrices`. `LdpcCode` supplies code data.
RU's inherent scalar `encode` supplies its oracle. Batch capability does not
require cutting over the bit-only scalar compatibility trait.
The lead sequences @/issue/6379e04e, branch A, then
@/issue/243d1160 and @/issue/7fbd1042 rebased onto A. Branch B proceeds in
parallel with those two issues after A; @/issue/9a1b24fc closes after B.
This is lead sequencing, not graph order: only @/issue/6379e04e is a graph
dependency of @/issue/243d1160 and @/issue/7fbd1042.
A owns the trait, BCH/view/shortening and their callers; B owns LDPC/RU and theirs.
The stored BCH content stamp and hoisted typed-field check supplied by
@/issue/6379e04e are A's basis. @/issue/243d1160's allocating-batch witness
migrates on its rebase; its file is outside this branch's caller inventory.
@/issue/7fbd1042 integrates its lockstep frame grouping at the shared
`SystematicKernel::encode_batch_family` seam and the granularity rule below.

## Public interface (REQ-01, REQ-05)

Add this capability to `gf2_coding::traits::block`, using `NonZeroUsize` from
`std::num`. The skeleton has two required methods and the defaults below.
No inherent alias exposes batch encoding.

```rust
/// Batch encoding in the code's declared symbol field and coordinate layout.
/// Results preserve input order and equal the corresponding scalar encoder.
/// Validate all shapes, runtime fields and workspace compatibility before writes.
/// Even an empty encode_batch_into validates workspace compatibility and shape;
/// it leaves scratch usable and needs no output or allocation to do so.
/// Execution errors may leave any subset of outputs written; scratch is reusable.
/// Empty batches are valid; encode_batch_parallel_into needs a workspace.
/// Implementors document preparation, complexity, compatibility and allocation.
/// Valid calls do not panic, apart from documented backend/allocator behavior.
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
The provided worker constructor calls the workspace constructor per worker;
LDPC overrides it to share preparation as specified below. Owning defaults
construct sized outputs and scratch, propagate construction errors, and call
`_into`. Empty owning calls return an empty vector without staging allocation.
Output reuse alone does not promise allocation freedom.

Granularity defaults to one and means the preferred bounded staging tile.
BCH/view override it with the largest structural frame group among available
encode families: `gf2_kernels_simd::bch_encode::BITSLICE_LANES` when bitslicing
is available, otherwise one. Take the maximum with the active profile
`encode.bitslice_interleaved_min_batch` when bitslicing is available. Lockstep
integration contributes its structural frame group and active caller override
to the same maximum. Defaults derive from kernel groups; installed selectors
can enlarge them. Shortening uses and forwards this capacity; LDPC/RU retain one.

Common validation in `crates/gf2-coding/src/traits.rs` checks equal slice counts,
message length `k()`, output length `n()` and symbol field identity. Move BCH's
private `validate_symbol_field` there, parameterized by the code's symbol zero:
typed fields need no scan; runtime carriers check the first symbol. Mixing field
presentations within one sequence remains caller error with unspecified results.
Use that check for BCH scalar methods, all batch implementors and shortening.

Parallel defaults reject an empty workspace slice with
`CodeError::BufferLengthMismatch`, even for an empty batch. Before dispatch,
validate the full batch and use empty `_into` calls to validate every workspace.
Use `gf2_core::compute::for_each_partition` with `PartitionDispatch::Pool` only
under gf2-coding's `parallel` feature, otherwise `PartitionDispatch::CallingThread`.
It owns boundaries, excess-worker handling and index order; add no local partitioner.

The primitive's closure is infallible. A stack-allocated
`Mutex<Option<(usize, Self::EncodeError)>>` collects each partition's `_into`
error with its range start, retaining the smallest start. Each closure returns
normally; all partitions finish; the default then returns that error or `Ok(())`.
The mutex allocates nothing and requires only `EncodeError: Send`.
Error selection follows input order; outputs need not form a written prefix.
An empty output invokes no closure; the explicit validation still runs.
BCH overrides keep whole-batch family selection and the same primitive; its
validated kernel path is infallible. Serial implementations need no Send bound.

Rustdoc states validation, preparation and owning-output costs separately:
`_into` costs validation plus encodes; parallel calls add worker validation and
partition dispatch. Allocation failure follows Rust's allocator behavior.

`BchCode<X,S,M>` and `LayoutView<X,S,M>` implement the trait with
`BchEncodeWorkspace<S::Word>` and `BchError`, retaining `FieldExtension`,
`SystematicKernel` and `SymbolMatrix` bounds. Constructors wrap prepared scratch
in `Ok`. BCH declares the default layout; the view uses `self.layout()`.
Private plan routines retain BCH thread scratch and @/issue/243d1160's owning behavior.

Remove the inherent `encode_batch`, `encode_batch_into`, `encode_batch_parallel`,
`encode_batch_parallel_into`, `encode_workspace` and `encode_workspaces` forms.
Remove public `encode_batch_family_into` entirely, including any view variant;
family dispatch remains private below the trait. Retain inherent diagnostic
`encode_family_available(family)` and `selected_encode_family(batch_len)` on
BCH/view in their declared layout, without layout-taking aliases. These queries
encode nothing. A non-default layout uses `LayoutView::new(code, layout)`;
callers retaining the code clone once outside timing. Import `BatchEncoder`,
build `code.encode_workspace()?`, then use its trait `_into` for every encoder.

Retain scalar `encode_systematic_with(message, layout, workspace, codeword)`
on `BchCode`: it accepts the trait-created `BchEncodeWorkspace` and shares
validation/preparation with the private batch routines. Scalar `encode_systematic`, `encode_systematic_into` and representation-level
`SystematicKernel` also remain; the layout parameter belongs to scalar operations.

## Shortening algorithm and complexity (REQ-03, REQ-04)

Add `ShortenedEncodeWorkspace<C>` in `crates/gf2-coding/src/transform/mod.rs`
and implement `BatchEncoder` for `Shortened<C>` where
`C: BatchEncoder + BlockEncoder`, with error `C::EncodeError`.
For `SystematicRestriction`, scratch holds the mother workspace, preallocated
mother-message/codeword vectors and an immutable zero message. Default capacity
is the mother's `encode_batch_granularity()`, including active admission
thresholds, so a full tile reaches eligible multi-frame kernels. Add `encode_workspace_with_batch_capacity(NonZeroUsize)`
as a caller override returning `Result<ShortenedEncodeWorkspace<C>, C::EncodeError>`.
The default worker constructor gives each worker that structural capacity.

Shortening overrides owning serial and parallel calls to size each active
workspace to $\min(b_p,T)$, where $T$ is the preferred tile and $b_p$ the
partition's message count (the full batch for serial). Parallel sizing uses
`gf2_core::compute::partition_offset`; it does not reproduce the split rule.
Empty owning calls need no workspace. Caller-buffer batches use bounded tiles;
grouped defaults require no harness-specific setting.

Construction zeroes staging once. Before output writes, validate external
shapes/fields, staging dimensions/fields and the mother workspace through an
empty mother `_into` call. For each tile, copy only `message_runs` into zero
staging, call the mother's trait `_into` on active prefixes and copy only
`codeword_runs` to the caller. Restore written message runs from the zero
message on success and on a returned mother error. Mother outputs are overwritten.
Removed coordinates exist only in workspace storage. No per-codeword mother-size
clearing, cloning or allocation occurs. Immediate-mother runs and `local_map`
compose nested shortening; compatible restrictions reuse mother scratch without
a restriction fingerprint because staging is zero between tiles.

REQ-04's linear adaptation bound applies to `SystematicRestriction`, including
all DVB-T2 configurations: for shortened lengths $k_s,n_s$ it costs
$O(k_s+n_s)=O(n_s)$ per message plus one mother encode. For mother lengths
$k_m,n_m$, scratch holds $O(T(k_m+n_m)+k_m)$ symbols plus mother scratch,
independently of batch length. Packed runs use `SymbolSequence::copy_range`
at unaligned ends. Construction pays staging initialization and mother preparation.
`RankDerived` uses its stored dense generator via the helper shared with scalar
`encode_into`, costing $O(k_s n_s)$ field operations per message and no mother
encode or new generator materialization. Its workspace needs only shape/field validation.
Rustdoc gives it no linear bound relative to an arbitrarily fast mother.

Each active tile supplies its length to mother family selection. Full and
partial groups reach table, bitslice, fold or reference kernels, including
scalar fallback. Structural first use allocates nothing; installed fold overrides
are prepared at workspace construction before repeated-call measurements.

## LDPC/RU preparation and compatibility (REQ-05)

Add `BlockCode` and `BatchEncoder` impls for `LdpcCode`, `LdpcEncoder` and
`RuEncodingMatrices`, with `Fp<2>` and `BitVec`. `LdpcCode` uses its encoder as
scalar oracle; RU uses its `systematic_cols`/`parity_cols` layout. Remove both
inherent batch methods, including RU's backend-taking form.
Keep `LdpcEncoder`'s compatibility `crate::traits::BlockEncoder` impl. For every
concrete encoder dimension call, including scalar-impl calls, use only
`crate::traits::BlockEncoder::k(encoder)` and
`crate::traits::BlockEncoder::n(encoder)` with `encoder: &LdpcEncoder`.
The added `block::BlockCode` impl delegates to these; the compatibility impl
reads `self.code`. No bare `encoder.k()`/`encoder.n()` call relies on imports.
Add `LdpcEncodeError` in `crates/gf2-coding/src/ldpc/core.rs`, wrapping
`CodeError`, `PreprocessError` and incompatible-workspace errors.

Batch-workspace compatibility uses one content-stamp convention. Add private semantic
`EncodeStamp` and common FNV fingerprint mixing in `crates/gf2-coding/src/traits.rs`.
BCH stores that type internally; `BchEncodeWorkspace::code_stamp` and existing
error diagnostics expose its numeric value at the reporting boundary.
LDPC stamps dimensions, field and complete canonical parity-check content once,
using the same type/comparison, without instance or pointer identity.
Equal codes built independently accept compatible prepared scratch; equal
sizes with different content do not. RU stores a stamp over dimensions, parity
matrix and column maps, computed by both preprocessing and `from_components`.
Its workspace compares that stamp too. These catch mistakes; no code fingerprint is recomputed per batch.
The partial `CacheKey` hash in `crates/gf2-coding/src/ldpc/encoding/cache.rs`
and decoder `code_stamp: u64` in `crates/gf2-coding/src/bch/core.rs` remain
distinct. Under @/inv/convention-convergence, the lead files an issue to converge
`CacheKey` and `code_stamp`, titled “Converge code fingerprints across caches and
workspaces”, before completion. Cache-file identity and decoder compatibility
extend beyond batch encoding; this issue makes no repository-wide convergence claim.

Add `LdpcEncoder::try_new(code: LdpcCode) -> Result<Self, LdpcEncodeError>`.
A shared fallible builder chooses IRA or `RuEncodingMatrices::preprocess`.
Cache its `Result<EncoderImpl, PreprocessError>` in an added
`Arc<OnceLock<Result<EncoderImpl, PreprocessError>>>` on `LdpcCode`, shared by
clones. `EncoderImpl` holds preparation in Arcs. Construction serializes
initialization and clones the same prepared Arc into each `LdpcEncodeWorkspace`;
only syndrome/parity scratch is worker-local. Override `encode_workspaces` on
code and encoder to obtain preparation once, then allocate worker scratch.
Scalar and batch construction share this builder. `LdpcEncoder::new` and
`LdpcEncoder::with_cache` remain documented panicking scalar conveniences;
batch constructors call the fallible builder, never either panicking wrapper.
The encoder's existing prepared Arc seeds its code's preparation cache so workers
reuse it. Preparation costs $O(\operatorname{nnz}(H))$ for IRA or
$O(mn\min(m,n))$ for RU row reduction, paid once per code's shared cache.

IRA scratch has syndrome and accumulated parity of length `m()`. Factor a
buffer-writing helper in `crates/gf2-coding/src/ldpc/encoding/ira.rs` for scalar
and batch callers. `_into` then allocates nothing; per-message cost is
$O(\operatorname{nnz}(H)+n)$ with $O(m)$ scratch bits.

Add `RuEncodeWorkspace` in
`crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs`, re-exported by
`crates/gf2-coding/src/ldpc/encoding/mod.rs`. It holds its stamp, dimensions and
`Arc<dyn ComputeBackend>`, initially `CpuBackend::new()`. Add
`RuEncodeWorkspace::set_backend(Arc<dyn ComputeBackend>)` and
`LdpcEncodeWorkspace::ru_workspace_mut(&mut self) -> Option<&mut RuEncodeWorkspace>`
(`None` for IRA) for configuration. Workers share the backend Arc. RU returns `CodeError`
(`ShapeMismatch` on incompatible scratch); LDPC wraps it.

RU dispatches `ComputeBackend::batch_matvec_transpose` and scatters parities.
`crates/gf2-core/src/compute/backend.rs` returns `Vec<BitVec>` and
`crates/gf2-core/src/compute/cpu.rs` allocates parity storage: RU `_into` and
LDPC's RU arm allocate. REQ-03 applies to DVB-T2 BCH. Dense CPU RU costs
$O(k\lceil(n-k)/64\rceil+n)$ per message with $O(b(n-k))$ transient parity
bits for batch length $b$. Other backends document product costs and retain their panic contracts.

## Caller audit and integration (REQ-06)

The keyword audit includes batch/workspace/family calls, `for_each_partition`, tests and rustdoc.

| Group | Branch | Repository paths |
|---|---|---|
| Library | A | `crates/gf2-coding/src/bch/encode.rs`, `crates/gf2-coding/src/test_support.rs` |
| Library | B | `crates/gf2-coding/src/ldpc/core.rs`, `crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs` |
| Tests | A | `crates/gf2-coding/tests/bch_encode_dispatch.rs`, `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs`, `crates/gf2-coding/tests/support/fresh_tuning_process.rs` |
| Tests | A+B | `crates/gf2-coding/tests/backend_integration.rs` |
| Benches | A | `crates/gf2-coding/benches/bch_encode_w1.rs` |
| Benches | A+B | `crates/gf2-coding/benches/batch_operations.rs` |
| Benches | B | `crates/gf2-coding/benches/ldpc_throughput.rs`, `crates/gf2-coding/benches/quick_parallel.rs` |
| Examples | A | `crates/gf2-coding/examples/bch_binary_quickstart.rs` |
| Experiments | A | `experiments/encode-compare/src/cutover/current.rs` |
| Docs | A | `docs/how-to/select-acceleration.md`, `docs/reference/performance-evidence.md`, `experiments/encode-compare/README.md` |
| Interface plans | A | `dev/active/d77176e5-competitive-benchmarking/design-5e71e993.md`, `dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md` |

Family-forcing callers in the BCH embedded tests, shared test support, dispatch
suite and W1 benchmark all use installed tuning profiles and the trait form.
Reuse the fresh-process protocol in
`crates/gf2-coding/tests/support/fresh_tuning_process.rs`: install one family's
selectors before construction, assert the declared-layout selection, then call
trait `_into`. Move embedded family differential cases into that integration
suite; `bch_encoding_contract` checks the active family through the shared trait
contract. Reference oracles call per-message
`SystematicKernel::encode_systematic_with` with plan/registers, independently
of the installed profile; no public forced-family batch helper replaces the method.
The benchmark launches one process per family/profile, installing before Criterion
and preparation. Timed closures call only the trait; family/kernel labels describe
configuration. `docs/how-to/select-acceleration.md` prescribes installed profiles.
Shared test-support rustdoc describes pointer stability only as storage reuse;
allocation claims require the counting allocator.
Imports, Result handling, constructor calls and timed labels use `BatchEncoder`.
`docs/reference/performance-evidence.md` names `BatchEncoder::encode_batch_into`
on `LayoutView`/`DvbT2BchCode` for D-7 and `BatchEncoder::encode_batch` for owning
output. Preparation, view cloning and profile installation stay outside timing.
REQ-06 sweeps every tracked file that names or describes the pre-cutover arm,
except the named records below. Branch A removes the arm and its `pre` role;
all swept prose describes current behaviour. Re-run this tracked-content search
at A's integration and classify every hit; the generated inventory below is its
file-level projection, including record-only and unrelated receipt matches:

```sh
git grep -n -i -E 'pre\.rs|pre-cutover|CUTOVER_PRE_REVISION|legacy encoder|gf2 revision|each revision|any gf2|git archive|gf2 [[:xdigit:]]{8}|cutover::pre|mod[[:space:]]+pre([[:space:]]|;)|arms=\(pre|^[[:space:]]*pre\)|"pre"|`pre`'
```

The revision-name expression covers the pinned gf2 name without embedding a
revision identifier. Generated search: 78 matching lines across 30 files.

| Hit file | Decision |
|---|---|
| `.jit/archive-evidence/container/6dc81018-9a8e-4318-9572-148670d1c272.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/events.jsonl` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/0d819b62-f6ad-49e0-add7-fd9f3486fe42.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/1d4fd63d-c522-47cb-9448-fe8f5220f0ed.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/278acf3a-3a49-4c79-8d54-8eb6c934c834.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/4443de83-4498-4100-8549-52b6c03f8007.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/51058f8e-6bad-443c-aa5b-14d430bcd0c7.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/697fc55b-4b36-4cb5-8106-a811b1772e44.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/6fb89a3c-f1a7-439d-a397-add1782afe2b.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/88ca7d2f-5045-4356-8ee3-e4e26876847d.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/e1e0e7ff-36f3-46e4-8bcb-db406c526bcb.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `.jit/issues/fd9d5416-d529-427d-a273-10e6468b9a19.json` | Keep as record: tracker configuration/evidence; unchanged. |
| `dev/active/2caf738d-1-0/handoff.md` | Keep as record: owner handoff; unchanged. |
| `dev/active/ae03bcd0-general-bch/breakdown.json` | Keep as record: manifest of accepted work. |
| `dev/active/ae03bcd0-general-bch/handoff-8.md` | Keep as record: lead handoff and evidence decisions. |
| `dev/active/ae03bcd0-general-bch/oracle/run.sh` | Keep as record producer: oracle receipt metadata, no comparison arm. |
| `dev/active/ae03bcd0-general-bch/plan.md` | Keep as record: accepted plan and decisions; unchanged. |
| `dev/active/ae03bcd0-general-bch/progress.json` | Keep as record: decisions and evidence; unchanged. |
| `dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md` | Sweep A: remove comparison-against-`encode_batch` clause; use trait rows. |
| `dev/active/d77176e5-competitive-benchmarking/design-9a1b24fc.md` | Sweep A: this prescription governs all arm references here. |
| `dev/scripts/bench-window/queue.tsv` | Sweep A: remove `fd9d5416` command and comment; preserve done-marker keys. |
| `docs/reference/performance-evidence.md` | Sweep A: current trait entry points versus external arms only. |
| `experiments/encode-compare/README.md` | Sweep A: single current adapter/build; remove per-revision extraction and layout rationale. |
| `experiments/encode-compare/cutover.sh` | Sweep A: remove `CUTOVER_PRE_REVISION`, its build/run/digest and role; document current-only extraction. |
| `experiments/encode-compare/cutover/main.rs` | Sweep A: document only the current adapter. |
| `experiments/encode-compare/cutover/pre.rs` | Sweep A: delete the arm. |
| `experiments/encode-compare/cutover/summarize.py` | Sweep A: current/external rows and verdicts only. |
| `experiments/encode-compare/src/cutover/current.rs` | Sweep A: declare layout directly; use the trait. |
| `experiments/encode-compare/src/cutover/harness.rs` | Sweep A: current/external roles only; remove cross-revision claims, layout rationale and pinned-name example. |
| `experiments/encode-compare/src/cutover/mod.rs` | Sweep A: current adapter and external arms only. |

Recorded results stay in their issues. Other interface-audit matches need no edit:
`crates/gf2-coding/benches/linear_codes.rs` contains benchmark names;
`crates/gf2-core/src/compute/mod.rs`, `crates/gf2-core/src/compute/partition.rs`
and `crates/gf2-sim/src/parallel/mod.rs` supply/use the shared partition primitive.
`dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-4.md`,
`dev/active/d77176e5-competitive-benchmarking/handoff-6.md`,
`dev/active/d77176e5-competitive-benchmarking/progress.json` and the charter
remain records of decisions/evidence.

A also changes `crates/gf2-coding/src/traits.rs`, `crates/gf2-coding/src/bch/spec.rs`,
`crates/gf2-coding/src/transform/mod.rs`, `crates/gf2-coding/src/bch/dvb_t2/mod.rs`
and `crates/gf2-coding/tests/dvb_t2_bch_verification.rs`. B also changes
`crates/gf2-coding/src/ldpc/encoding/ira.rs`,
`crates/gf2-coding/src/ldpc/encoding/mod.rs` and
`crates/gf2-coding/tests/ldpc_encoding_tests.rs`. B uses A's stamp/trait machinery.
A/B overlap in `crates/gf2-coding/tests/backend_integration.rs` and
`crates/gf2-coding/benches/batch_operations.rs`; integrate those hunks sequentially.
The BCH issues rebase encoder/tests/W1 callers onto A with profiles and the trait.

## Conformance and allocation evidence (REQ-01 through REQ-06)

Start implementation with failing behavioral cases. Add one generic batch helper
beside `traits::block::conformance` in `crates/gf2-coding/src/traits.rs`; all
implementors run it with a scalar oracle at batch lengths 0, 1, 63, 64 and 65.
It checks owning/reused/parallel equality, order, tails, changed inputs, invalid
shape/field/workspace atomicity, empty-call workspace rejection and scratch reuse.
Exercise worker counts above/below batch length, both feature configurations,
execution errors in multiple partitions and allocation-free error transport.

BCH/view tests cover both layouts and no inherent batch aliases. Extend
`crates/gf2-coding/tests/bch_encode_dispatch.rs` for all configurations in
`crates/gf2-coding/src/bch/dvb_t2/params.rs`, both frame sizes, every available
`EncodeFamily::REGISTERED` family under fresh profiles and forced scalar kernels.
Restore kernel settings; cover zero/ones/seeded/single-set-bit messages at
0, 1, 63, 64 and the final bit. Assert default mother group sizes, capacities one
and larger overrides, partial tiles, nesting and default owning/worker staging.
Install a bitslice threshold above `BITSLICE_LANES`; assert default full tiles
meet that threshold and match whole-batch family selection when eligible.
In `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs`, count only `_into`
after construction using `thread_allocations::CountingAllocator` and `count` in
`dev/tools/thread-allocations/src/lib.rs`. Require zero allocations on structural
first use and repeats across the DVB-T2 matrix; prepare fold overrides outside
repeated-call counts. Assertions and comparisons stay outside counting.

Embedded transform tests observe copied runs, reject a mother tile then reuse
scratch, and cover changed compatible restrictions, noncontiguous removals,
nonbinary fields, rank-derived and zero-dimensional codes. Review each path's
rustdoc bound. Retain the host-gated TP04/TP05 oracle in
`crates/gf2-coding/tests/dvb_t2_bch_verification.rs` and compare batches there.
LDPC tests in `crates/gf2-coding/tests/ldpc_encoding_tests.rs`,
`crates/gf2-coding/tests/backend_integration.rs` and embedded sources run the
shared contract on IRA, non-IRA and non-prefix columns for code/encoder/RU.
Check preprocessing errors, one preparation shared across workers/clones,
independently equal codes and different equal-sized codes, configured backend
reuse and RU batch-product dispatch. Scalar equality and zero syndrome establish
semantics; allocation assertions apply to IRA only. Caller review includes all
inventory paths and rebased witnesses. Implementation validation covers benches,
examples and experiment consumers. Cases respect fast-tier budgets; heavy cases
use descriptive slow-tier ignores. Execution and benchmark evidence belong to implementation.
