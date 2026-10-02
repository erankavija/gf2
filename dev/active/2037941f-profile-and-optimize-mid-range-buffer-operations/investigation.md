# Mid-range logical-buffer investigation

> **Diátaxis Type:** Explanation

This is planning evidence for story `2037941f` and planning node `8d8be934`.
It makes no implementation, tuning, JIT-state, protocol, or receipt change.
All paths and line references below are to the current checkout unless they
name a committed measurement artifact.

## Scope verdict

The story is warranted, but it is not a blank-slate kernel project. The
canonical logical dispatcher already selects scalar below eight words and a
runtime-detected SIMD bundle at/above eight words; ordinary builds use eight
words while the baked build may use its recorded threshold
[`crates/gf2-core/src/kernels/backend.rs:82-120`](../../../crates/gf2-core/src/kernels/backend.rs#L82-L120).
The public XOR entry resolves that choice on every call, while callers with a
fixed width can bind the function pointer once
[`crates/gf2-core/src/kernels/ops.rs:7-65`](../../../crates/gf2-core/src/kernels/ops.rs#L7-L65).

The predecessor's accepted evidence says that an eight-word row loop benefits
from hoisting, but that the same hoist is not material at 64 words; it also
preserves the 64- and 8192-word no-win outcomes
[`dev/active/04b85d10/findings.md:100-110`](../04b85d10/findings.md#L100-L110).
Consequently, the work must measure real unhoisted 8--64-word consumers before
proposing a new helper or threshold. It must not relitigate a 64-word
dispatch-hoist change without a materially different consumer contract.

## Claim classification

| Input claim | Classification | Evidence and implication |
|---|---|---|
| 8--64-word buffers *can* be sensitive to dispatch/call overhead as well as throughput. | **valid-and-open** | The current per-call resolver is real [`crates/gf2-core/src/kernels/ops.rs:93-101`](../../../crates/gf2-core/src/kernels/ops.rs#L93-L101), and the eight-word hoist result supports the premise [`dev/active/04b85d10/findings.md:100-107`](../04b85d10/findings.md#L100-L107). The premise does not establish a gain at every width or consumer. |
| Row operations, parity generation, and dense-matrix work are relevant production consumers. | **valid-and-open** | `row_xor` is a production row operation [`crates/gf2-core/src/matrix.rs:1027-1069`](../../../crates/gf2-core/src/matrix.rs#L1027-L1069); dense `matvec` computes row parity [`crates/gf2-core/src/matrix.rs:1538-1589`](../../../crates/gf2-core/src/matrix.rs#L1538-L1589); coding routes invoke both matrix parity and row XOR [`crates/gf2-coding/src/ldpc/core.rs:131-143`](../../../crates/gf2-coding/src/ldpc/core.rs#L131-L143), [`crates/gf2-coding/src/ldpc/nr_5g/mod.rs:802-841`](../../../crates/gf2-coding/src/ldpc/nr_5g/mod.rs#L802-L841). No current receipt establishes that each is material in the 8--64-word band. |
| Measure consumers before selecting unrolling, zero-copy access, fusion, or a changed dispatch route. | **valid-and-open** | The measurement contract requires frozen per-family adoption rules and whole-consumer costs before selection [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:23-35`](../1a379447-zen3-cpu-performance/measurement-contract.md#L23-L35), [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:72-86`](../1a379447-zen3-cpu-performance/measurement-contract.md#L72-L86). |
| **REQ-01:** publish frozen 8--64-word profiles/baselines, neighboring and unaligned/boundary cases, and primary consumer cells. | **valid-and-open** | Reusable exploratory coverage exists for logical 4/8/64/8192-word rows and the profile records routes, but it is not this story's frozen protocol/addendum [`dev/active/04b85d10/findings.md:365-373`](../04b85d10/findings.md#L365-L373). The external logical remeasurement covers 8, 9, 63, 64 and 65 words, but is exploratory [`dev/active/6fb89a3c/findings.md:67-83`](../6fb89a3c/findings.md#L67-L83). |
| **REQ-02:** compare unroll, hoist, BitSlice zero-copy, and useful fusions; adopt only under a predeclared confidence/complexity rule. | **valid-and-open** | The generic hoist candidate is **already-done/no-win** at 64 words [`dev/active/04b85d10/findings.md:100-107`](../04b85d10/findings.md#L100-L107). Dense matvec already uses a no-temporary fused AND-popcount path and explicitly keeps the carry-save comparator unselected because it did not qualify [`crates/gf2-core/src/matrix.rs:1591-1608`](../../../crates/gf2-core/src/matrix.rs#L1591-L1608). Unroll and a consumer-specific fusion remain hypotheses, not justified changes. |
| **REQ-03:** compare semantically equivalent XOR/parity against ISA-L and dense-matrix work against M4RI, including adaptation costs. | **valid-and-open** | The prior ISA-L comparison maps LSB-first word XOR and includes a destination copy/pointer-array accounting, but it reaches only unvectorized `xor_gen_base`; public ISA-L `xor_gen` is unavailable without NASM [`dev/active/6fb89a3c/findings.md:37-52`](../6fb89a3c/findings.md#L37-L52). Prior M4RI work is transpose/BCH-generator framing, not a demonstrated 8--64-word logical consumer, so a matched M4RI operation must be proven before freezing a cell [`dev/active/6fb89a3c/findings.md:37-41`](../6fb89a3c/findings.md#L37-L41). |
| **REQ-04:** preserve exact semantics, little-endian indexing, zero tails, and one canonical buffer/dispatch abstraction. | **valid-and-open** | These are established current invariants, not evidence that a future change preserves them: `BitSlice` indexes word `i >> 6`, bit `i & 63` [`crates/gf2-core/src/bitslice.rs:29-37`](../../../crates/gf2-core/src/bitslice.rs#L29-L37), and the shared contract explicitly requires tails and 0/1/63/64/65 cases [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:97-112`](../1a379447-zen3-cpu-performance/measurement-contract.md#L97-L112). |
| **REQ-05:** commit annotated release assembly/profiles and split any independent implementation work. | **valid-and-open** | Existing Rust-1.95 assembly/profile evidence identifies AVX2 XOR as a load/XOR/store loop and records no frame traffic, but it is predecessor evidence, not a story-specific before/after explanation [`dev/active/04b85d10/findings.md:460-471`](../04b85d10/findings.md#L460-L471), [`crates/gf2-kernels-simd/src/x86/avx2.rs:19-35`](../../../crates/gf2-kernels-simd/src/x86/avx2.rs#L19-L35). |
| **REQ-06:** satisfy the Zen 3 measurement contract, including a current baseline and retained negative outcomes. | **valid-and-open** | The contract requires release measurements, resumable journaled runs, raw samples, and a current pre-change/after pair for production changes [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:55-70`](../1a379447-zen3-cpu-performance/measurement-contract.md#L55-L70), [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:114-129`](../1a379447-zen3-cpu-performance/measurement-contract.md#L114-L129). Historical receipts may inform the design but cannot replace that baseline. |

## Consumer sweep

### Production source

* **Logical XOR.** `BitVec::bit_xor_into` reaches the canonical operation
  [`crates/gf2-core/src/bitvec.rs:427-446`](../../../crates/gf2-core/src/bitvec.rs#L427-L446).
  `BitMatrix::row_xor` uses the same public operation; its blocked suffix
  sibling deliberately remains an inline scalar loop for 4--16-word RREF
  rows, a measured decision that directly covers part of this story's band
  [`crates/gf2-core/src/matrix.rs:1072-1115`](../../../crates/gf2-core/src/matrix.rs#L1072-L1115).
  The Gray-table slice path is also scalar [`crates/gf2-core/src/matrix.rs:1117-1145`](../../../crates/gf2-core/src/matrix.rs#L1117-L1145).
* **Already-hoisted consumers.** The degenerate dense multiply fallback,
  M4RM, RREF, Gauss inversion, and Strassen resolve once outside their fixed
  row loops [`crates/gf2-core/src/matrix.rs:225-274`](../../../crates/gf2-core/src/matrix.rs#L225-L274),
  [`crates/gf2-core/src/alg/m4rm.rs:1060-1092`](../../../crates/gf2-core/src/alg/m4rm.rs#L1060-L1092),
  [`crates/gf2-core/src/alg/rref.rs:140-152`](../../../crates/gf2-core/src/alg/rref.rs#L140-L152),
  [`crates/gf2-core/src/alg/gauss.rs:273-295`](../../../crates/gf2-core/src/alg/gauss.rs#L273-L295),
  [`crates/gf2-core/src/alg/strassen.rs:237-281`](../../../crates/gf2-core/src/alg/strassen.rs#L237-L281).
  They are no-win/not-material for a *generic hoist* change; profile them only
  as whole-consumer controls if a different fusion is proposed.
* **Dense parity fusion.** `BitMatrix::matvec` selects at eight words and its
  SIMD lane calls `and_popcnt_fn` once per row, avoiding a temporary or second
  pass [`crates/gf2-core/src/matrix.rs:12-24`](../../../crates/gf2-core/src/matrix.rs#L12-L24),
  [`crates/gf2-core/src/matrix.rs:1538-1611`](../../../crates/gf2-core/src/matrix.rs#L1538-L1611).
  The AVX2 kernel fuses the AND before nibble-lookup population accumulation
  [`crates/gf2-kernels-simd/src/x86/avx2.rs:315-347`](../../../crates/gf2-kernels-simd/src/x86/avx2.rs#L315-L347).
  It is the primary parity family; the generic carry-save alternative is
  already non-selectable, not a fresh candidate
  [`crates/gf2-kernels-simd/src/lib.rs:116-129`](../../../crates/gf2-kernels-simd/src/lib.rs#L116-L129).
* **Coding consumers.** OSD obtains a dense syndrome then XORs a candidate
  into a clone [`crates/gf2-coding/src/osd/syndrome.rs:132-150`](../../../crates/gf2-coding/src/osd/syndrome.rs#L132-L150).
  DRM and NR-5G materialization use `row_xor` directly
  [`crates/gf2-coding/src/drm.rs:644-654`](../../../crates/gf2-coding/src/drm.rs#L644-L654),
  [`crates/gf2-coding/src/ldpc/nr_5g/mod.rs:802-841`](../../../crates/gf2-coding/src/ldpc/nr_5g/mod.rs#L802-L841).
  These are candidate consumers only after their actual row stride is fixed.
* **BitSlice/zero-copy.** `BitSlice` and `BitSliceMut` are offset bit views
  over backing words, offering only bit accessors
  [`crates/gf2-core/src/bitslice.rs:1-81`](../../../crates/gf2-core/src/bitslice.rs#L1-L81).
  `from_bitslice` allocates and copies bit-by-bit
  [`crates/gf2-core/src/bitvec.rs:1094-1104`](../../../crates/gf2-core/src/bitvec.rs#L1094-L1104).
  The in-tree coding use is an ownership-taking message assertion, not a
  zero-copy logical-buffer consumer
  [`crates/gf2-coding/src/bch/extended.rs:375-383`](../../../dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/inputs/producing/crates/gf2-coding/src/bch/extended.rs#L375-L383).
  Therefore “adopt zero-copy BitSlice use” is **invalid as an existing
  primitive claim**; it is a possible new shared API, not an optimization that
  can be switched on in the current consumers.

### Tests, benchmarks, and docs

* The dispatch-hoist equivalence suite covers lengths 0--64, unaligned views,
  and scalar equivalence [`crates/gf2-core/tests/simd_equiv_dispatch_hoist.rs:1-105`](../../../crates/gf2-core/tests/simd_equiv_dispatch_hoist.rs#L1-L105).
  Matvec and count route suites provide the existing behavioral boundary for a
  fusion or selector experiment [`crates/gf2-core/tests/simd_equiv_matvec.rs:1-105`](../../../crates/gf2-core/tests/simd_equiv_matvec.rs#L1-L105),
  [`crates/gf2-core/tests/popcount_routes.rs:1-219`](../../../crates/gf2-core/tests/popcount_routes.rs#L1-L219).
* Existing benchmark coverage is useful but insufficiently frozen for REQ-01:
  `selector_non_regression` drives public `xor_inplace` and deliberately keeps
  harness dispatch outside the timed window
  [`crates/gf2-core/benches/selector_non_regression.rs:12-19`](../../../crates/gf2-core/benches/selector_non_regression.rs#L12-L19),
  [`crates/gf2-core/benches/selector_non_regression.rs:866-890`](../../../crates/gf2-core/benches/selector_non_regression.rs#L866-L890);
  dense matvec benchmarks sweep square 64--1024 matrices
  [`crates/gf2-core/benches/matrix_vector.rs:10-23`](../../../crates/gf2-core/benches/matrix_vector.rs#L10-L23);
  M4RM components bind one XOR function for a 16-word table row
  [`crates/gf2-core/benches/m4rm_components.rs:20-40`](../../../crates/gf2-core/benches/m4rm_components.rs#L20-L40).
* No permanent documentation defines a second logical-buffer or zero-copy
  abstraction. The durable architecture is instead the core dispatcher and
  kernel bundle [`crates/gf2-core/src/kernels/backend.rs:1-60`](../../../crates/gf2-core/src/kernels/backend.rs#L1-L60),
  [`crates/gf2-kernels-simd/src/lib.rs:90-143`](../../../crates/gf2-kernels-simd/src/lib.rs#L90-L143).

## Prior art and reusable evidence

The prior-art sweep is local and standards-backed: gf2 AVX2 XOR, ISA-L, M4RI,
and the predecessor consumer survey. ISA-L's pinned arm is `xor_gen_base`, a
non-vectorized byte loop; its dispatched public route was unavailable because
the host lacked NASM. M4RI is pinned and its transpose route uses general
purpose instructions; Bitshuffle is irrelevant to this story except as a
demonstration that adapters must be charged
[`dev/active/6fb89a3c/findings.md:43-52`](../6fb89a3c/findings.md#L43-L52).

Reuse the accepted 04b85d10 logical profile for workload prioritization,
assembly symbols, and the preserved no-wins; reuse 6fb89a3c only as
exploratory semantic/adaptation evidence. Its original comparisons were
withdrawn for a warm-pass defect, and corrected remeasurements remain
exploratory/not production-selectable
[`dev/active/6fb89a3c/findings.md:54-83`](../6fb89a3c/findings.md#L54-L83).
Neither is a current pre-change baseline, as the contract expressly forbids
that substitution [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:123-129`](../1a379447-zen3-cpu-performance/measurement-contract.md#L123-L129).

## Frozen questions for implementation planning

1. Freeze distinct isolated XOR cells at 8, 9, 63, 64, and 65 words plus
   deliberately offset slice views, then primary whole-consumer cells for one
   public `row_xor` route and one actually 8--64-word coding/elimination route.
   Include output/tail validation rather than treating word-aligned buffers as
   representative.
2. Freeze dense parity cells at the same stride boundaries for `matvec`, with
   output allocation included; compare only a candidate that is not the already
   retained AND-popcount fusion. Measure the scalar four-accumulator route as
   a semantic/reference arm, not as a new API.
3. Before naming ISA-L a competitive baseline, decide whether a host with NASM
   is available to build its public dispatched `xor_gen`; otherwise retain the
   explicitly labelled base-arm comparison. Before naming M4RI, prove a
   matching dense-matrix operation and charge construction/conversion/output;
   do not use transpose or BCH-generator receipts as a substitute.
4. Freeze family-specific worthwhile, equivalence, complexity, cache-regime,
   statistical, and search-budget rules before confirmation; retain every
   no-win or unavailable cell. The contract requires exactly this discipline
   [`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:37-53`](../1a379447-zen3-cpu-performance/measurement-contract.md#L37-L53).

## Architecture, invariants, and MSRV

The fit is `gf2-core` public operation/dispatch plus `gf2-kernels-simd` for
any new unsafe intrinsic, never a coding-private kernel. AVX2 detection is
runtime-gated and returns no SIMD bundle otherwise
[`crates/gf2-kernels-simd/src/x86/mod.rs:40-46`](../../../crates/gf2-kernels-simd/src/x86/mod.rs#L40-L46);
new work must preserve that scalar fallback and the current one canonical
dispatch mechanism. This satisfies the relevant architecture directions:
canonical little-endian indexing and zero tails, behavioral equivalence,
unsafe-kernel isolation, canonical cutover, and benchmark-backed performance.

Rust 1.95 is a hard feasibility gate for any intrinsic-bearing candidate:
compile and test it at that MSRV before implementation breakdown, and retain a
tested scalar fallback with explicit safety contracts
[`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:108-112`](../1a379447-zen3-cpu-performance/measurement-contract.md#L108-L112).
The existing AVX2 code demonstrates the required `#[target_feature]` boundary
[`crates/gf2-kernels-simd/src/x86/avx2.rs:19-38`](../../../crates/gf2-kernels-simd/src/x86/avx2.rs#L19-L38).

## Owner decision

One lasting architectural decision is required **only if** a measurement makes
zero-copy material: either (A) retain `BitSlice` as a bit-access view and keep
logical kernels on canonical `&[u64]` row/BitVec storage, or (B) add one
canonical, word-aligned borrowed-word view with explicit offset, alignment and
tail semantics. Option B is shared `gf2-core` infrastructure, affects public
API and every backend, and cannot be chosen by a leaf optimization task. No
other current candidate presents a lasting architecture choice.
