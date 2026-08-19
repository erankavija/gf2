# Investigation: field capability traits and profile-driven dispatch

Container `6dc81018` — current-system findings, read-only investigation. The
report separates shipped production code, proof/extraction artifacts, research
stubs, and historical decisions. No code or `.jit/` state was changed for this
investigation.

## 1. Claim classification

### Claim 1 — already-done

`gf2-core` has the two-tier hierarchy. `FiniteField` is defined at
`crates/gf2-core/src/field/traits.rs:44`; its delayed-reduction associated type
is `type Wide` at `:68-70`. `ConstField` is `FiniteField + Copy` at `:995-1000`.
`FiniteFieldExt` is a convenience trait at `:1039-1046` and is blanket-implemented
for every `T: FiniteField` at `:1135`.

The production `FiniteField` implementations, including their `Wide` values,
are:

- `Gf2mElement_<V>` — `crates/gf2-core/src/gf2m/field.rs:1319-1324`, with
  `Wide = Self`.
- `Gf2mWide<N, Cfg>` — `crates/gf2-core/src/gf2m/wide.rs:1355-1362`, with
  `Wide = Self`.
- `Fp<P>` — `crates/gf2-core/src/gfp/mod.rs:511-514`, with `Wide = u128`.
- `GoldilocksFp` — `crates/gf2-core/src/gfp/specialized.rs:986-989`, with
  `Wide = u128`.
- `QuadraticExt<C>` — `crates/gf2-core/src/gfpn/quadratic.rs:582-585`, with
  `Wide = QuadraticExtWide<<C::BaseField as FiniteField>::Wide>`.
- `CubicExt<C>` — `crates/gf2-core/src/gfpn/cubic.rs:633-636`, with the
  corresponding `CubicExtWide` accumulator.

The production `ConstField` implementations are `Fp<P>` at
`crates/gf2-core/src/gfp/mod.rs:972-995`, `GoldilocksFp` at
`crates/gf2-core/src/gfp/specialized.rs:1105`, `QuadraticExt<C>` at
`crates/gf2-core/src/gfpn/quadratic.rs:702`, `CubicExt<C>` at
`crates/gf2-core/src/gfpn/cubic.rs:757`, and `Gf2mWide<N, Cfg>` at
`crates/gf2-core/src/gf2m/wide.rs:1818`. There is no `ConstField` impl block
for `Gf2mElement_<V>`; the complete production impl sites above are the
positive inventory, while its `FiniteField` impl is at
`crates/gf2-core/src/gf2m/field.rs:1319-1324`.

There are two test-only implementations that must not be mistaken for shipped
field families: `OpCount` in `crates/gf2-core/src/field/batch_ops.rs:670-673`
and `RuntimeFp7` in `crates/gf2-algebra/src/permanent/ryser.rs:495-499`.

### Claim 2 — invalid-as-stated

The packed traits are shipped production code, but they are defined in
`gf2-algebra`, not `gf2-core` and not only in the research stub. The canonical
definitions are `PackedField` at
`crates/gf2-algebra/src/packed/mod.rs:88-107` and `PackedFieldVec` at
`:374-385`; the module documentation explicitly calls `gf2-algebra` their
workspace home at `:1-22`. The D1b decision names the same home at
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md:33-39`.

Production implementations are:

- `ScalarPackedFp3` / `ScalarPackedFp3Vec` —
  `crates/gf2-algebra/src/packed/scalar.rs:92` and `:203`.
- `Bipedal3` / `Bipedal3Vec` —
  `crates/gf2-algebra/src/packed/bipedal3.rs:482` and `:1863`.
- `Packed5` / `Packed5Vec` —
  `crates/gf2-algebra/src/packed/packed5.rs:428` and `:989`.
- `Packed7` / `Packed7Vec` —
  `crates/gf2-algebra/src/packed/packed7.rs:581` and `:1056`.

The archived research stub is at `dev/archive/packed_field_stub/src/lib.rs`:
it redeclares both traits at `:80` and `:331`, and has stub implementations at
`:774` and `:968`. Its `fold_mul` still returns zero at `:1389-1392` and
`:1619-1624`; it is a historical demonstration, not the production
implementation.

The approved D1b decisions are reflected in the shipped trait surface:

- `const LANES` is present at `crates/gf2-algebra/src/packed/mod.rs:104`.
- `splat` is a trait method at `:155-158`.
- `fold_mul` is intentionally absent from `PackedFieldVec`; D1b records that
  decision at `dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md:159-189`,
  and production `Bipedal3Vec::fold_mul` is inherent at
  `crates/gf2-algebra/src/packed/bipedal3.rs:1592-1627`.
- `Eq` is canonical-decode equality, not raw storage equality, in the shipped
  contract at `crates/gf2-algebra/src/packed/mod.rs:60-69` and `:303-305`.
- `Bipedal3` documents and implements alternative-zero canonicalisation at
  `crates/gf2-algebra/src/packed/bipedal3.rs:99-145`; `lane` decodes zero from
  either zero codeword at `:695-748`, and `with_lane` writes a canonical sign
  bit at `:775-790`.
- The analogous `Packed5` reserved-code decode and equality are at
  `crates/gf2-algebra/src/packed/packed5.rs:215-260`, with canonical lane writes
  at `:653-699`; `Packed7` documents the same boundary at
  `crates/gf2-algebra/src/packed/packed7.rs:993-1021` and `:753-782`.

The surface is historically frozen by the W6 freeze document at
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/8c902184/gf_api_freeze_w6.md:23-51`.
That freeze is documentation-only, not an automated source-level gate; its
notes say so at `:135-142`.

### Claim 3 — already-done

The free batch-operation API exists in `crates/gf2-core/src/field/batch_ops.rs`:

| Function | Definition | Bound |
|---|---:|---|
| `batch_inverse` | `:115` | `F: FiniteField` |
| `batch_inverse_in_place` | `:157` | `F: FiniteField` |
| `batch_inverse_with_scratch` | `:210-214` | `F: FiniteField` |
| `batch_inverse_skip_zeros` | `:275` | `F: FiniteField` |
| `batch_inverse_skip_zeros_in_place` | `:309` | `F: FiniteField` |
| internal `batch_inverse_core` | `:363-367` | `F: FiniteField` |

The public module re-exports the five public functions at
`crates/gf2-core/src/field/mod.rs:65-66`. None requires `ConstField`; the
`ConstField` in the first doctest import at `batch_ops.rs:85` is not a function
bound.

### Claim 4 — valid-and-open

The system already contains all named dispatch forms, but they are not one
reconciled capability vocabulary.

`Gf2mField_<V>` stores runtime strategy state in `FieldParams_` at
`crates/gf2-core/src/gf2m/field.rs:164-187`: table handles, `simd_mul_fn`,
`clmul_fn`, `clmul_barrett_fn`, and an optional `BarrettReducer`. The constructor
selects and caches these at `:267-310`; `with_tables` builds the table strategy
at `:388-438`. Multiplication then tries table, combined CLMUL+Barrett, split
CLMUL+Barrett, legacy SIMD, and finally schoolbook scalar in priority order at
`:1095-1190`.

`gf2-core` also has safe centralized function-table accessors. The SIMD module
defines `OnceLock<Option<...>>` tables at `crates/gf2-core/src/lib.rs:81-117`
and `maybe_gf2m`, `maybe_gf2m_batch`, `maybe_gf2m_gemm`, prime-field accessors,
and wide-field accessors at `:119-389`. The disabled-SIMD module returns
`None` for the same accessors later in the file. The safe `SimdBackend` wrapper
and its function-pointer calls are in `crates/gf2-core/src/kernels/simd/mod.rs:24-70`.

There is a separate generic bit-buffer selector in
`crates/gf2-core/src/kernels/backend.rs:82-102`: `SelectedBackend` and
`select_backend_for_size` use `_SIMD_THRESHOLD = 8`. The deprecated compatibility
kernel surface is in `crates/gf2-core/src/kernels/mod.rs:27-60`.

Finally, `FiniteField` carries many `#[doc(hidden)]` default hooks with safe
`None`/`false` fallbacks. Representative hook groups are
`try_gf2m_u64_batch_dot_product` at `traits.rs:324-348`, prime-field packing and
dot hooks at `:351-444`, GEMM hooks at `:449-508`, reducer/chain/AXPY hooks at
`:532-597`, matvec/SpMM hooks at `:599-694`, and PLE hooks at `:959-986`.
These are function hooks, not algebraic laws, and their strategy predicates are
distributed between trait impls and callers.

Therefore the premise that strategy selection is embedded in constructors and
hooks is true, but the conclusion that there is already a single canonical
dispatch mechanism is not established. Reconciliation remains open.

### Claim 5 — invalid-as-stated

CLMUL and PCLMULQDQ paths exist; GFNI does not appear in the production source
search. The function-pointer types and table are in
`crates/gf2-kernels-simd/src/gf2m.rs:19-74`. x86 detection selects PCLMULQDQ
functions at `:89-100`, and the safe wrappers call the target-feature functions
at `:105-124`.

The batch path detects AVX2 plus VPCLMULQDQ at
`crates/gf2-kernels-simd/src/gf2m_batch.rs:90-100`; wide-field XMM/YMM paths
are defined at `crates/gf2-kernels-simd/src/x86/gf2m_wide.rs:36-90` and the
selection guard is at `:333-344`. GEMM detection uses AVX2, VPCLMULQDQ, and
PCLMULQDQ at `crates/gf2-kernels-simd/src/x86/gf2m_gemm.rs:241-246`.

Selection is runtime feature detection into cached function tables, then
field-level caching/priority selection: core exposes the tables at
`crates/gf2-core/src/lib.rs:142-176`, `Gf2mField` caches the selected pointers at
`gf2m/field.rs:267-310`, and its `Mul` implementation consumes them at
`:1121-1155`. Wide fields use the same core accessors and an explicit scalar
`clmul_wide_slice` fallback at `crates/gf2-core/src/gf2m/wide.rs:2064-2113`.

The historical GFNI study confirms that GFNI was evaluated but not implemented:
`dev/archive/97bf0879-gf2-core-sota-performance/plans/fb271c41/gf2m_avx512_gfni_evaluation.md:17-29`
and records the host as having no GFNI at `:33-57`. Thus “CLMUL/PCLMULQDQ/GFNI
paths exist” is false as written; the CLMUL/PCLMULQDQ portion is already done.

### Claim 6 — already-done (absence), with an open inventory obligation

No versioned tuning-profile type, committed profile artifact, profile loader, or
offline calibration-to-profile path was found in `gf2-core`, `gf2-algebra`, or
`gf2-coding`. Existing selection is source constants, compile-time field shape,
runtime CPU features, and occasional environment controls. The clearest current
examples are `select_backend_for_size`'s `_SIMD_THRESHOLD = 8` at
`crates/gf2-core/src/kernels/backend.rs:96-102`, and the prime-field route
selectors `N_THRESH_PRIME = 251` and `n >= 512` at
`crates/gf2-core/src/gfp/simd_ops.rs:524-571` and `:1604-1628`.

The performance-selection constants that a tuning profile would plausibly
subsume are:

- Core backend/matrix execution: `_SIMD_THRESHOLD = 8`
  (`crates/gf2-core/src/kernels/backend.rs:96`),
  `MATVEC_SIMD_MIN_WORDS = 8` (`crates/gf2-core/src/matrix.rs:13`),
  `TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS = 16` and local
  `MACRO_TILE_BLOCKS = 8` (`crates/gf2-core/src/matrix.rs:1115-1118` and
  `:1170-1177`), and SoA parallel `CHUNK_LEN = 16 KiB` plus
  `MIN_LEN = 2 * CHUNK_LEN` (`crates/gf2-core/src/compute/field.rs:17-36`).
- M4RM table/tile policy: `B2_GRAY_TILE_WORDS = 8`,
  `B2_GRAY_MAX_TILES = 4`, `M4RM_DEFAULT_TABLE_BYTES = 64 KiB`,
  `M4RM_DEFAULT_MAX_K = 8`, `M4RM_MID_TABLE_BYTES = 128 KiB`,
  `M4RM_WIDE_TABLE_BYTES = 256 KiB`, `M4RM_WIDE_MAX_K = 9`, tile dimensions,
  and minimum strides at `crates/gf2-core/src/alg/m4rm.rs:20-65` and `:96`.
- Linear-algebra recursion and blocking: `INVERT_M4RI_THRESHOLD = 8`
  (`crates/gf2-core/src/alg/gauss.rs:29`),
  `BLOCKED_INVERT_THRESHOLD = 16`
  (`crates/gf2-core/src/field/inverse.rs:85`),
  `TRSM_BLOCKED_PANEL_SIZE = 64`
  (`crates/gf2-core/src/field/triangular.rs:243`),
  `PLE_PANEL_RECURSIVE_BASE = 128` and `BLOCKED_BACK_SUB_MIN_DIM = 128`
  (`crates/gf2-core/src/field/ple.rs:642` and `:1684`),
  `GEMM_ROW_TILE = 32`, `GEMM_COL_TILE = 64`, and
  `GEMM_AXPY_FAST_PATH_THRESHOLD = 16^3`
  (`crates/gf2-core/src/field/matrix.rs:2498-2508` and `:2965-2978`).
- Additional generic execution cutovers: the `FieldVec` SIMD chunk size
  `CHUNK = 256` at `crates/gf2-core/src/field/vec.rs:1022`, and the disabled
  Keller–Gehrig dispatch sentinel `KG_DISPATCH_MIN_N = usize::MAX` at
  `crates/gf2-core/src/field/charpoly.rs:276`. The latter is a strategy
  selector even though its current value disables that route.
- Trait-level algorithm thresholds: `WINOGRAD_THRESHOLD = 128` and
  `TRI_BASE_THRESHOLD = 8` at `crates/gf2-core/src/field/traits.rs:807-825`
  and `:827-858`; `PLE_BASE_COLS` and `PLE_PANEL_COLS` at `:892-926`.
  These are currently trait-associated strategy constants, not a profile.
- Polynomial algorithm crossovers: `KARATSUBA_THRESHOLD = 32`,
  `SUBPRODUCT_THRESHOLD = 4096`, `NTT_THRESHOLD = 128`, and
  `DIV_REM_THRESHOLD = 2048` at
  `crates/gf2-core/src/field/poly.rs:2147`, `:2192`, `:2699-2711`, and
  `:2898-2909`; interpolation uses `INTERPOLATE_THRESHOLD = 16` at
  `crates/gf2-core/src/field/poly_interpolate.rs:115`.
- Prime-field SIMD routing: `N_THRESH_PRIME = 251` and the two `n >= 512`
  route guards at `crates/gf2-core/src/gfp/simd_ops.rs:537`, `:562-571`, and
  `:1621-1628`; the PLE panel field-family choices `256`, `128`, and `1` are
  embedded in `crates/gf2-core/src/gfp/mod.rs:885-918`.
- Packed permanent strategy constants: `N_MAX_MULTIWORD = 255`,
  `L1D_BYTES = 32 KiB`, and `MAX_MATRIX_BYTES_FOR_L1` at
  `crates/gf2-algebra/src/permanent/bipedal3_multiword.rs:64-92`; parallel
  `CHUNK_SUBSETS = 1 << 16` at
  `crates/gf2-algebra/src/permanent/parallel_bipedal3.rs:49`.

The retry limits and verification-size constants in `field/charpoly.rs:285`,
`:1341`, and `:1679` are algorithm reliability policy rather than backend
crossovers, so they are recorded as excluded from a tuning profile. Likewise,
packed `LANES` and `n <= 63`/`n <= 255` bounds are representation or algorithm
domain limits, not host tuning values; the representative bounds are at
`crates/gf2-algebra/src/packed/mod.rs:104` and
`crates/gf2-algebra/src/permanent/bipedal3.rs:407-419`.

The `gf2-coding` search found no finite-field backend or kernel crossover
constant. Its thresholds are decoder stopping criteria, code parameters, or
simulation progress controls; for example list-BLER threshold behavior is
explicitly a protocol/configuration concern at
`crates/gf2-coding/src/product/mod.rs:710-765`, while the GPU demapper is a
prototype whose module documentation says it only supports a separately tracked
CPU/GPU measurement at `crates/gf2-coding/src/modem/gpu_demapper.rs:1-26`.
Those coding-domain thresholds should not be placed in a field-kernel tuning
profile.

This is why the claim is “already-done” for absence of a profile, but not a
completed inventory: the current values are numerous and span algebraic
algorithm cutovers, cache blocking, parallel scheduling, and backend selection.

### Claim 7 — valid-and-open

The current extraction is real and succeeds for concrete arithmetic, but it does
not demonstrate extraction of a Rust function generic over the reconciled field
traits.

The proof README states the currently extracted areas are `gfp`, `gfpn`, and
`gf2m` arithmetic at `proofs/README.md:5-13`. The core Charon invocation starts
from `gf2_core::gfp`, `gf2_core::gfpn`, and `gf2_core::gf2m::mul_raw`, while
opaquing `gf2_core::field`, at `scripts/verify-lean.sh:91-131`. The algebra
invocation starts from concrete packed modules and Gray-code functions, while
opaquing `gf2_core::field` and `gf2_algebra::permanent::ryser`, at
`scripts/verify-lean.sh:140-203`; its comment says the targets are the inherent
`{Bipedal3,Packed5,Packed7}::{add,sub,mul,neg}_inherent` wrappers at `:150-166`.
The Rust wrappers themselves say they exist to avoid trait-dispatch indirection
for Charon at `crates/gf2-algebra/src/packed/bipedal3.rs:387-405`.

The generated Lean files do contain trait dictionaries and generic-looking
trait declarations: `proofs/Gf2Core/Types.lean:70-125` and
`proofs/Gf2Algebra/Types.lean:70-181`. That is not evidence that a generic Rust
algorithm body was extracted. The strongest contrary evidence is the existing
Ryser proof record: `proofs/Gf2Algebra/Proofs/RyserBounded.lean:57-85` records
that Charon did not monomorphise generic `permanent_ryser<F>` from a
non-generic start root and that the `gf2-core` dependency arithmetic remained
uninterpreted; the extracted-Rust binding was descoped. The file proves an
abstract `CommRing` theorem at `:87-105`, not the Rust trait-generic function.

Therefore the historical premise “V1 targets inherent methods” is true, and
“some generic trait declarations appear in generated Lean” is true, but “a
trait-generic Rust algorithm has already been extracted” is not verified. The
REQ-05 target remains open and has a known extraction risk.

### Claim 8 — already-done

The canonical-representation boundaries exist in both named families.

For `Bipedal3`, the encoded zero and alternative-zero behavior is documented at
`crates/gf2-algebra/src/packed/bipedal3.rs:16-26` and the canonical decode/equality
boundary is implemented at `:99-145`. `lane` is the decode boundary
(`:695-748`), and `with_lane` is the canonical encode boundary
(`:775-790`). The vector representation additionally enforces zero tail padding
through `mask_tail` at `:1563-1590`.

For `Fp<P>`, the public `new()`/`value()` boundary is documented as canonical
`[0,P)` values while storage may be Montgomery or canonical at
`crates/gf2-core/src/gfp/mod.rs:93-109`. `new` converts into the selected storage
at `:152-177`; `value` decodes at `:180-195`; `raw_storage` and
`from_raw_storage` are explicitly crate-private representation backdoors at
`:198-232`. The Montgomery constants and conversion support are in
`crates/gf2-core/src/gfp/montgomery.rs:8-23` and imported by `gfp/mod.rs:58`.

This claim is done as a representation-boundary inventory. It does not imply
that Bipedal3 canonicalisation and Montgomery storage should become one trait;
they are representation refinements with different algebraic domains.

## 2. Prior-art sweep

The following documents contain facts that constrain this container. Paths and
line numbers are from the current tree; historical documents are evidence of
decisions, not proof that every surrounding implementation still matches them.

| Document | Constraint recorded |
|---|---|
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md:14-39` | `PackedField`, `PackedFieldVec`, and `Permanent` are the public packed/permanent abstractions; their final home is `gf2-algebra`. |
| Same, `:111-157` | `LANES` is an associated const and `splat` is a trait method. |
| Same, `:159-189` | `fold_mul` is intentionally inherent on concrete vector types. |
| Same, `:192-218` and `:260-303` | `Eq` is canonical-decode equality; Bipedal3 alternative-zero is canonicalised at public boundaries; tail masking is part of the vector invariant. |
| Same, `:491-502` | The research stub is standalone but checks real `gf2-core` field bounds; it is a demonstration artifact, not the production home. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/6e20133d/d1a_gf2_algebra_boundary.md:1-40` | The crate boundary deliberately puts packed traits in `gf2-algebra` and keeps `gf2-core` as the inward dependency. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/8c902184/gf_api_freeze_w6.md:8-21` and `:23-70` | The W6 proof surface is frozen for Charon/Aeneas; the freeze is a documentation-only change-control checkpoint. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/a0c0a45f/d2_lean_bipedal3_sketch.md:1-20` and the production wrapper note at `crates/gf2-algebra/src/packed/bipedal3.rs:387-405` | V1 proof targets a fixed inherent Bipedal3 arithmetic surface. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/30e98ef1/d5_lean_packed5_sketch.md:250-270` and `d6_lean_packed7_sketch.md:185-205` | F5/F7 proof sketches use concrete inherent wrappers forwarding to the packed implementations. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/c7542983/r4_simd_batching_decision.md:41-102` | SIMD strategy choices are tied to an explicit host, MSRV 1.95, release build, deterministic inputs, and pinned measurement protocol. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/plans/r2_packed_encoding_generalizations.md:10-38`, `:40-94`, and `:125-158` | F3/F5/F7 encoding choices are representation/algorithm decisions; workload-weighted measurements and fallback rules are separate from the field-law trait. |
| `dev/archive/e095a100-gfpm-arithmetic/plans/gfpn_groundwork_analysis.md:33-43`, `:93-145`, and `:147-180` | The two-tier field hierarchy and `Wide` were implemented as a deliberate solution to runtime-vs-const field identity; the kernel layer is separate. |
| `dev/archive/e095a100-gfpm-arithmetic/plans/d11b769a/wide_accumulator_tower.md:1-20` | Wide accumulator propagation through extension towers is a distinct representation/refinement concern. |
| `dev/archive/e095a100-gfpm-arithmetic/plans/2ce2a757/karatsuba_cross_verification.md:120-130` | `FiniteFieldExt::square` is a default convenience operation, not a second field capability hierarchy. |
| `dev/archive/ae82bd73-gf2-algebra-permanent/active/0606186a/0606186a-impl-handoff.md:1-80` and `proofs/Gf2Algebra/Proofs/RyserBounded.lean:57-85` | Generic Rust Ryser extraction was empirically falsified and the direct extracted binding was descoped; the abstract proof remains. |
| `dev/archive/97bf0879-gf2-core-sota-performance/active/97bf0879-handoff-10.md:41` and `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/73ec5da3/2026-05-07-73ec5da3-ple-trsm-tuning.md:45-60` | Trait-associated thresholds have already caused Lean SSOT synchronization issues and are selected by benchmark sweeps. |
| `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/9e12659b/2026-05-05-9e12659b-medium-prime-gemm.md:340-360` | Hidden `FiniteField` SIMD hooks were introduced to amortise packing; default `None` is the fallback contract. |
| `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/111a3967/2026-05-06-111a3967-gf2-parity-evidence.md:60-75` | M4RM table tiers and `k` bounds are empirical backend policy. |
| `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-05-07-4eb105f7-dense-la-parity-evidence.md:140-155` | `PLE_BASE_COLS` is a field-level strategy knob whose selected value is performance-dependent. |
| `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/41096af5/2026-05-25-41096af5-route-selection-decision.md:15-35` and `:72-90` | Route selection uses explicit benchmark decision rules and a measured `n >= 512` crossover; it is not a host-independent algebraic law. |
| `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/0749dbad/2026-05-27-0749dbad-fp-medium-f64-cascade.md:76-104` | The f64 cascade uses a safe feature-detect table and a size selector; the existing u16 path remains the fallback. |
| `dev/archive/97bf0879-gf2-core-sota-performance/plans/fb271c41/gf2m_avx512_gfni_evaluation.md:17-29`, `:33-57`, and `:148-159` | GFNI/AVX512 was an evaluated but unimplemented future direction; the measured architecture target uses AVX2/CLMUL. |
| `dev/benchmarks/gf2_algebra_permanent/README.md:1-37` | Historical permanent CSVs lack immutable executable-source provenance and cannot alone establish a backend choice. |
| `dev/benchmarks/permanent_campaign/backend-ordering.md:1-7`, `:38-72`, and `:144-169` | Current benchmark receipts record exact configurations, schedule, seed addresses, source/build identity, host, and toolchain. |

## 3. Consumer sweep

The lists below are the complete text consumers found in the tree for the
named surfaces. A path may be both a definition and a consumer; it is listed
once in the relevant category. Rustdoc examples are included because the token
is compiled or documented at the source location. Generated Lean and script
files are listed separately from Rust production consumers.

### 3.1 `FiniteField`

Definitions and production `gf2-core` consumers:

`crates/gf2-core/src/field/traits.rs:44`; `field/axiom_tests.rs:1`;
`field/batch_ops.rs:1`; `field/charpoly.rs:2`; `field/expr.rs:100`;
`field/extension_wiedemann.rs:69`; `field/inverse.rs:2`; `field/matrix.rs:1`;
`field/mod.rs:8`; `field/ntt.rs:84`; `field/ple.rs:2`; `field/poly.rs:1`;
`field/poly_interpolate.rs:1`; `field/sparse_matrix.rs:1`;
`field/test_random_matrix.rs:35`; `field/triangular.rs:75`;
`field/two_adic.rs:68`; `field/vec.rs:4`; `field/winograd.rs:2`;
`gf2m/field.rs:1319`; `gf2m/poly_helpers.rs:7`; `gf2m/wide.rs:17`;
`gfp/mod.rs:34`; `gfp/simd_ops.rs:6`; `gfp/specialized.rs:91`;
`gfpn/batch.rs:95`; `gfpn/cubic.rs:36`; `gfpn/ext_config.rs:115`;
`gfpn/mod.rs:20`; `gfpn/quadratic.rs:30`; and `compute/field.rs:15`.

Additional `gf2-core` tests and benches:

`crates/gf2-core/tests/gfpn_nested_towers.rs:38`,
`tests/gfpn_wide_nested.rs:13`, `tests/karatsuba_cross_verify.rs:38`;
`benches/batch_inverse.rs:9`, `fieldmatrix_charpoly.rs:31`,
`fieldmatrix_gemm.rs:139`, `fieldmatrix_ple.rs:35`,
`fieldmatrix_solve.rs:32`, `fp_montgomery.rs:4`, `fp_specialized.rs:21`,
`sparse_field_matmul.rs:30`, `sparse_rref.rs:3`,
`sparse_rref_scorecard.rs:18`, `sparse_spmv.rs:41`,
`strassen_threshold.rs:12`, `triangular.rs:30`, and
`fieldmatrix_gf2m_batch_gemm.rs:15`.

`gf2-algebra` consumers and its tests/bench:

`crates/gf2-algebra/src/lib.rs:9`, `src/packed/mod.rs:22`,
`src/permanent/rank.rs:69`, `src/permanent/ryser.rs:4`,
`src/testutil.rs:9`, `tests/permanental_rank.rs:45`,
`tests/rank_event_rates.rs:12`, and `benches/determinant_companion.rs:21`.
The direct generic algorithm is `permanent_ryser<F: FiniteField>` at
`crates/gf2-algebra/src/permanent/ryser.rs:92`.

### 3.2 `ConstField`

`crates/gf2-core/src/field/traits.rs:995`; `field/axiom_tests.rs:22`;
`field/batch_ops.rs:85` (doctest import only); `field/charpoly.rs:1917`;
`field/expr.rs:70`; `field/inverse.rs:161`; `field/matrix.rs:8` and `:461`;
`field/ple.rs:390`; `field/poly.rs:1241`; `field/sparse_matrix.rs:33`;
`field/traits.rs:995`; `field/vec.rs:7`; `field/winograd.rs:86`;
`gf2m/wide.rs:18`; `gf2m/wide_config.rs:13`; `gfp/mod.rs:34`;
`gfp/specialized.rs:91`; `gfpn/batch.rs:125`; `gfpn/cubic.rs:4`;
`gfpn/ext_config.rs:34`; and `gfpn/quadratic.rs:4`.

Additional consumers are `crates/gf2-core/benches/fieldmatrix_gf2m_batch_gemm.rs:15`,
`benches/fp_montgomery.rs:4`, `benches/fp_specialized.rs:21`,
`crates/gf2-core/tests/gfpn_nested_towers.rs:38`,
`tests/gfpn_wide_nested.rs:13`, `tests/karatsuba_cross_verify.rs:38`,
`crates/gf2-algebra/src/permanent/rank.rs:283`,
`src/permanent/ryser.rs:78`, and `tests/rank_event_rates.rs:12`.

### 3.3 `FiniteFieldExt`

The definition and blanket implementation are
`crates/gf2-core/src/field/traits.rs:1039` and `:1135`. Other source/doc-code
consumers are `field/axiom_tests.rs:22`, `field/two_adic.rs:68`,
`gfp/mod.rs:34`, and `crates/gf2-core/tests/karatsuba_cross_verify.rs:38`.
No `gf2-algebra` or `gf2-coding` production consumer of this extension trait
was found.

### 3.4 `Wide`

The associated type and implementations are at
`crates/gf2-core/src/field/traits.rs:68-70`, `gf2m/field.rs:1323`,
`gf2m/wide.rs:1359-1362`, `gfp/mod.rs:513`, `gfp/specialized.rs:988`,
`gfpn/quadratic.rs:584`, and `gfpn/cubic.rs:635`.

Generic consumers are `crates/gf2-core/src/field/axiom_tests.rs:14`,
`field/batch_ops.rs:672` (test type), `field/expr.rs:445`,
`field/matrix.rs:1470` and `:2516`, `field/sparse_matrix.rs:42`,
`field/traits.rs:26`, `field/triangular.rs:18`, `field/vec.rs:426`,
`field/winograd.rs:55`, `gfp/simd_ops.rs:1034`, and `gfpn/mod.rs:14`.
Nested-tower consumers are `crates/gf2-core/src/gfpn/cubic.rs:65`,
`gfpn/quadratic.rs:58`, `tests/gfpn_nested_towers.rs:3`, and
`tests/gfpn_wide_nested.rs:1`. The sole non-core source consumer is the
test-only `RuntimeFp7` declaration at
`crates/gf2-algebra/src/permanent/ryser.rs:497`.

### 3.5 `PackedField` and `PackedFieldVec`

Definitions and all production implementations:

`crates/gf2-algebra/src/packed/mod.rs:88` and `:374`;
`packed/scalar.rs:92` and `:203`; `packed/bipedal3.rs:482` and `:1863`;
`packed/packed5.rs:428` and `:989`; `packed/packed7.rs:581` and `:1056`.

Production algorithm consumers are `crates/gf2-algebra/src/permanent/bipedal3.rs:60`,
`permanent/bipedal3_multiword.rs:48`, `permanent/bipedal5.rs:39`,
`permanent/bipedal7.rs:8`, `permanent/parallel_bipedal3.rs:35`, and
`permanent/ryser.rs:4`. Kernel test consumers are
`crates/gf2-kernels-simd/src/bipedal/packed5.rs:1189` and
`src/bipedal/packed7.rs:864`.

The archived research stub records declarations at
`dev/archive/packed_field_stub/src/lib.rs:80` and `:331`; production research
consumers are `dev/research/permanent-sampling-feas/src/gray_update.rs`,
`dev/research/permanent-sampling-feas/src/backend.rs`,
`dev/research/permanent_wave_gpu/src/f5_candidates.rs`, and
`dev/research/permanent_wave_gpu/src/wave.rs`.

### 3.6 Batch-operation consumers

The API definition/re-export and its complete self-test surface are
`crates/gf2-core/src/field/batch_ops.rs:115-363` and `:422-566`, plus the
re-export at `crates/gf2-core/src/field/mod.rs:65-66`. The external Rust call
sites are exactly:

- `crates/gf2-core/src/field/poly_interpolate.rs:473` — `batch_inverse` for
  interpolation denominators.
- `crates/gf2-core/src/field/poly_interpolate.rs:736` — `batch_inverse` for
  derivative values.
- `crates/gf2-core/benches/batch_inverse.rs:1-36` — benchmark driver.

No batch-operation function is referenced by the proof extraction configuration.
The core Charon command opaques `gf2_core::field` at
`scripts/verify-lean.sh:109`, and the generated Lean source contains no
`batch_inverse` definition or call. The `batch_ops` module’s test-only `OpCount`
implementation at `batch_ops.rs:670-673` is the only additional field consumer.

### 3.7 Proof, extraction, documentation, and non-Rust references

The extraction/configuration consumers are `scripts/verify-lean.sh:26-28`,
`:36-48`, `:58-75`, `:94-131`, and `:140-203`; the post-processors
`scripts/fix-aeneas-dupes.py:5-14`, `scripts/fix-aeneas-sorrys.py:244-320`, and
`scripts/fix-aeneas-gf2algebra.py` (its generated-impl replacements); and the
generated/handwritten proof files:

- `proofs/Gf2Core/Types.lean:70-125`, `Funs.lean`,
  `FunsExternal_Template.lean:168-246`, `FunsExternal.lean`,
  `Proofs/ExtDefs.lean:21-52`, `Proofs/QuadraticExtField.lean`,
  `Proofs/CubicExtField.lean`, and `Proofs/ExtProgress.lean`.
- `proofs/Gf2Algebra/Types.lean:70-181`, `Funs.lean`,
  `FunsExternal_Template.lean:430-471`, `FunsExternal.lean`,
  `Proofs/Bipedal3Correctness.lean:10-20`,
  `Proofs/Packed5Correctness.lean:10-20`,
  `Proofs/Packed7Correctness.lean:13-23`, and
  `Proofs/RyserBounded.lean:121-137`.

The direct documentation/code-reference consumers include
`docs/lean4-verification-pipeline.md:24`, `crates/gf2-core/README.md:15`,
`crates/gf2-algebra/README.md:5`, `crates/gf2-core/docs/GF2M.md:16`,
`dev/plans/field_poly_module_overview.md:19`,
`dev/plans/bdf95060_breakdown.md:8`, `dev/plans/small_prime_kernel_strategy.md:310`,
and the historical field/packed/Lean documents listed in the prior-art sweep.
These are prose or proof inputs, not additional Cargo consumers. The audit also
found the following additional non-`.jit` textual references; entries already
listed in the Rust/proof lists above are not repeated:

- Cargo/docs and bench registration: `crates/gf2-algebra/Cargo.toml:13`,
  `crates/gf2-core/Cargo.toml:174`, `crates/gf2-core/ROADMAP.md:198`, and
  `crates/gf2-core/benches/strassen_threshold_results.md:4`.
- Current active artifacts: `dev/active/0de41c82/bipedal-f5-f7-representation-study.md:256`,
  `dev/active/0de41c82/investigation.md:223`,
  `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md:150`,
  `dev/active/b8206228-permanent-statistics/investigation.md:228`,
  `dev/active/fa787f85-documentation-overhaul/fa787f85-rustdoc-example-verdicts.tsv:17`,
  and `dev/active/charon-patch-backup-2026-05-15/hrtb-associated-types.rs:4`.
- Research and plans: `dev/research/rns_representation.md:235`,
  `dev/plans/16283d6f-fieldmatrix-gpu/gpu_fieldmatrix_sketch.md:37`,
  `dev/plans/6fb4abad_breakdown.md:10`, `dev/plans/70972f06_audit.md:74`,
  `dev/plans/bdf95060_breakdown.md:8`, and
  `dev/plans/field_poly_module_overview.md:19`.
- Historical field-linear-algebra references:
  `dev/archive/bb85c68a-field-linear-algebra/active/ab791e27-design-fieldmatrix-f-finitefield-dense-matrix-ty/ab791e27-design.md:4`,
  `dev/archive/bb85c68a-field-linear-algebra/active/bb85c68a-handoff-2.md:19`,
  `dev/archive/bb85c68a-field-linear-algebra/active/bb85c68a-handoff-3.md:21`,
  `dev/archive/bb85c68a-field-linear-algebra/active/bb85c68a-handoff-4.md:100`,
  `dev/archive/bb85c68a-field-linear-algebra/active/bb85c68a-handoff.md:91`,
  `dev/archive/bb85c68a-field-linear-algebra/active/c3f8c1cb-implement-ple-decomposition-and-echelon-forms-ov/c3f8c1cb-ple-replan.md:12`,
  `dev/archive/bb85c68a-field-linear-algebra/plans/armadillo_ux_mapping.md:19`,
  `dev/archive/bb85c68a-field-linear-algebra/plans/cdcebf6a-design-fieldmatrix-expression-template-algebra-p/expression_templates_design.md:62`,
  `dev/archive/bb85c68a-field-linear-algebra/plans/dumas_pernet_takeaways.md:37`,
  and `dev/archive/bb85c68a-field-linear-algebra/plans/fflas_ffpack_analysis.md:64`.
- Historical arithmetic references:
  `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-completion-report.md:17`,
  `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-handoff-2.md:50`,
  `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-handoff-3.md:18`,
  `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-handoff.md:55`,
  `dev/archive/e095a100-gfpm-arithmetic/plans/0203bebd/non_residue_config_trait.md:15`,
  `dev/archive/e095a100-gfpm-arithmetic/plans/3f3d6c23/cubic_ext.md:35`,
  `dev/archive/e095a100-gfpm-arithmetic/plans/8d2863e6/quadratic_ext.md:25`,
  `dev/archive/e095a100-gfpm-arithmetic/plans/d11b769a/wide_accumulator_tower.md:1`,
  and `dev/archive/e095a100-gfpm-arithmetic/plans/fflas_ffpack_analysis.md:64`.
- Historical performance references:
  `dev/archive/026fc832-gf2-core-sota-stretch/active/24a93e4e/24a93e4e-blocked-echelon-design.md:362`,
  `dev/archive/026fc832-gf2-core-sota-stretch/active/2e8c5a29/2e8c5a29-panelized-ple-design.md:180`,
  `dev/archive/026fc832-gf2-core-sota-stretch/active/873cbec1/873cbec1-extension-field-matrix-gemm-design.md:213`,
  `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/5ce13bae/2026-05-24-5ce13bae-markowitz-sparse-rref.md:178`,
  `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/6823c8a0/2026-05-26-6823c8a0-panelized-ple.md:154`,
  `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/6823c8a0/2026-05-26-6823c8a0-r1-recursive-pluq.md:255`,
  `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/9e12659b/2026-05-05-9e12659b-medium-prime-gemm.md:347`,
  `dev/archive/026fc832-gf2-core-sota-stretch/plans/sparse_benchmark_corpus.md:173`,
  `dev/archive/97bf0879-gf2-core-sota-performance/active/97bf0879-handoff-10.md:41`,
  `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/111a3967/2026-05-06-111a3967-gf2-parity-evidence.md:66`,
  `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md:369`,
  `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md:70`,
  `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/54fd3f0b-sota-sparse-fieldmatrix/2026-05-07-3a37e0f6-sparse-layout.md:47`,
  `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/73ec5da3/2026-05-07-73ec5da3-ple-trsm-tuning.md:52`,
  and `dev/archive/97bf0879-gf2-core-sota-performance/plans/sparse_benchmark_corpus.md:173`.
- Historical packed/Lean references not already detailed in prior art:
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/0606186a/0606186a-path1-spike-changes.patch:38`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-1.md:50`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-2.md:31`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-3.md:15`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-4.md:29`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-6.md:113`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/active/ae82bd73-handoff-9.md:26`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/plans/0606186a/d3_v2_path1_sketch.md:34`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/plans/4fced99b/d1c_feature_matrix.md:7`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/plans/d3_lean_ryser_sketch.md:167`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent.md:18`,
  `dev/archive/ae82bd73-gf2-algebra-permanent/plans/gf2_algebra_permanent_completion.md:54`,
  and `dev/studies/0dffa759/findings.md:1022`.
- User-facing docs and proof maintenance: `docs/lean4-verification-pipeline.md:24`,
  `docs/presentations/ae82bd73-gf2-algebra-permanent/talk.html:65`,
  `docs/presentations/bb85c68a-fieldmatrix/talk.html:84`,
  `proofs/WORKAROUNDS.md:5`, `scripts/fix-aeneas-dupes.py:4`,
  `scripts/fix-aeneas-gf2algebra.py:7`,
  `scripts/fix-aeneas-sorrys.py:244`, and
  `scripts/verify-lean.sh:26`.

The search also found incidental prose matches that are not consumers of these
field surfaces: `crates/gf2-sim/src/parallel/mod.rs:1020` uses “Wide” for a
statistical tolerance, and the many `gf2-coding` threshold fields are protocol
controls rather than field traits. They are excluded from the semantic consumer
inventory for that reason. `.jit/` references are tracker data, not build or
proof consumers, and were intentionally not changed.

## 4. Primitive verification

### Runtime dispatch is safe and centralized — partially true, not a completed invariant

The safe part is real: production unsafe code is kept in the kernel crates by
the crate policy, `gf2-core` imports safe function bundles and `OnceLock` tables
at `crates/gf2-core/src/lib.rs:81-117`, and callers receive `Option` tables
through `:119-389`. The SIMD wrapper’s unsafe boundary is inside
`gf2-kernels-simd`, while `gf2-core/src/kernels/simd/mod.rs:24-70` only calls
safe function pointers.

Centralization is incomplete. `Gf2mField` separately caches table/CLMUL/Barrett
strategy state at `crates/gf2-core/src/gf2m/field.rs:164-187` and
`:267-310`; `FiniteField` has an independent collection of hidden hooks at
`crates/gf2-core/src/field/traits.rs:324-694` and `:959-986`; and the generic
bit backend has its own selector at `crates/gf2-core/src/kernels/backend.rs:82-102`.
The code has safe boundaries and tested fallbacks, but not one profile-driven
selection authority.

### Scalar fallback exists for every SIMD path — supported by the main paths, but the blanket assertion is too broad

Concrete evidence is strong: default hidden hooks return `None`/`false` and
document caller fallback at `crates/gf2-core/src/field/traits.rs:351-380`,
`:416-447`, `:577-597`, and `:623-694`; `Gf2mField::mul` reaches schoolbook
after all accelerated priorities at `crates/gf2-core/src/gf2m/field.rs:1121-1190`;
and wide CLMUL falls back to `clmul_wide_slice` at
`crates/gf2-core/src/gf2m/wide.rs:2064-2113`.

The kernel-facing documentation also states `None`/scalar fallback for GF2M
batch operations at `crates/gf2-core/src/gf2m/batch.rs:55-63` and `:141-147`.
However, this investigation did not execute every feature-specific backend
test, and “every SIMD path” includes future/optional kernel modules outside the
field API. The safe, evidence-backed statement is: every inspected production
field dispatch path has an explicit fallback contract; the universal invariant
still needs a path-by-path conformance test.

### The packed trait surface is frozen at api-freeze — true as a recorded decision, not mechanically enforced

The freeze document lists the traits and concrete types at
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/8c902184/gf_api_freeze_w6.md:23-51`
and says the checkpoint is approved at `:124-133`. The shipped module carries
the same trait definitions at `crates/gf2-algebra/src/packed/mod.rs:88-107` and
`:374-385`, and the production implementations match the frozen type list.
The freeze explicitly says there is no automated checker at `:135-142`; the
archived research stub’s parallel declarations at
`dev/archive/packed_field_stub/src/lib.rs:80-331` preserve the historical
evidence that the repository had an unfrozen research copy. The accurate
primitive claim is “production API matches a documentation freeze,” not
“the repository contains only one enforced declaration.”

## 5. Architecture fit

Existing primitives to reuse or preserve:

- The `OnceLock<Option<FunctionTable>>` and `maybe_*` accessors in
  `crates/gf2-core/src/lib.rs:81-389` are the established safe runtime-dispatch
  shape. The associated audit receipt says the five dispatch tables share this
  structure at `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-04-27-asm-audit.md:70-80`.
- `gf2-kernels-simd` safe wrappers and feature detection are the existing kernel
  boundary: `crates/gf2-kernels-simd/src/gf2m.rs:89-124` and the wide XMM/YMM
  guards at `src/x86/gf2m_wide.rs:333-344`.
- `FiniteField` default hooks already encode “try accelerator, return
  `None`/`false`, caller continues” semantics. They are reusable as an
  integration seam, but their current names and thresholds should not be
  mistaken for a capability classification. See
  `crates/gf2-core/src/field/traits.rs:324-444` and `:577-694`.
- The benchmark corpus already records pinned release/MSRV/host/seed/source
  provenance. The permanent receipt README rejects source-less historical CSVs
  at `dev/benchmarks/gf2_algebra_permanent/README.md:9-21`; the current backend
  ordering receipt records the exact build, host, source identity, and seed
  address protocol at `dev/benchmarks/permanent_campaign/backend-ordering.md:38-72`
  and `:131-169`.
- Existing route receipts show how to record a crossover with a predeclared
  decision rule and a fallback, rather than treating one host’s number as a law:
  `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/41096af5/2026-05-25-41096af5-route-selection-decision.md:15-35`
  and `:72-90`.

Layer constraints:

- `gf2-core` is the canonical home for `FiniteField`, `ConstField`, `Wide`,
  algebraic field-law tests, generic linear algebra, and safe dispatch. It has
  no production dependency on `gf2-algebra`; therefore a packed lane trait
  defined in `gf2-algebra` cannot be consumed by a new outward dependency from
  `gf2-core` without violating the repository contract.
- `gf2-algebra` owns the shipped F3/F5/F7 lane representations and permanent
  algorithms. Its current inward use of `gf2-core::FiniteField` is visible at
  `crates/gf2-algebra/src/packed/mod.rs:22` and `src/permanent/ryser.rs:4`.
- `gf2-kernels-simd` and `gf2-kernels-hip` are the only production unsafe
  locations. The lane algebra or field laws must not be moved into a kernel
  crate merely to make dispatch generic; kernel crates should remain strategy
  implementations behind safe boundaries.
- The current `Fp<P>` public API hides Montgomery/canonical storage behind
  `new`/`value` at `crates/gf2-core/src/gfp/mod.rs:93-109` and `:152-195`.
  A capability inventory should preserve that as a representation-refinement
  boundary rather than promoting `MontgomeryParams` to a field law.

## 6. Architectural-invariant check

| Capability class | Existing examples | Boundary/invariant result |
|---|---|---|
| Algebraic law | Field addition/multiplication/inversion laws, identity behavior, and delayed-reduction correctness represented by `FiniteField` and its axiom harness at `crates/gf2-core/src/field/traits.rs:44-80` and `field/axiom_tests.rs:114-156`. | Belongs in `gf2-core` and must not depend on `gf2-algebra` or kernels. Moving it outward would violate `crate-dependency-direction`; putting unsafe proof-sensitive law code in kernels would violate `unsafe-kernel-isolation`. |
| Algorithm | Batch inversion (`batch_ops.rs:115-363`), Karatsuba/Barrett/table algorithms in `gf2m/field.rs:267-310` and `:1095-1190`, polynomial/matrix cutovers in `field/poly.rs:2147-2909`, and permanent Ryser in `gf2-algebra/src/permanent/ryser.rs:92`. | Generic finite-field algorithms belong in `gf2-core` when they consume only field laws; packed permanent algorithms remain in `gf2-algebra`. Moving a core algorithm to algebra would force an outward dependency from core if core consumers needed it. |
| Representation refinement | `FiniteField::Wide` (`traits.rs:68-70`), extension `Wide` types (`gfpn/quadratic.rs:582-585`, `gfpn/cubic.rs:633-636`), Bipedal3 canonical decode/encode (`packed/bipedal3.rs:695-790`), and Montgomery `new`/`value` (`gfp/mod.rs:152-195`). | These are not automatically laws or kernel strategies. `Wide` and prime-field storage stay in `gf2-core`; packed lane representation stays in `gf2-algebra` per the approved boundary. A shared abstraction must have one home; duplicating the packed trait in core would violate `convention-convergence` and the D1a boundary. |
| Kernel strategy | Runtime function tables (`gf2-core/src/lib.rs:81-389`), Gf2m cached strategy state (`gf2m/field.rs:164-187`), hidden `FiniteField` hooks (`field/traits.rs:324-694`, `:959-986`), and x86 CLMUL detection (`gf2-kernels-simd/src/gf2m.rs:89-124`). | Strategy may be selected by safe core dispatch and implemented in isolated kernel crates. Moving unsafe target-feature bodies into `gf2-core` violates `unsafe-kernel-isolation`; making `gf2-core` depend on `gf2-algebra` to discover packed strategies violates `crate-dependency-direction`. A profile should be data consumed by the existing safe boundary, not a new outward crate edge. |

The main architectural finding is therefore a real reconciliation problem, but
not greenfield: the algebraic hierarchy, packed API, batch algorithms, kernel
tables, hidden hooks, representations, and proof artifacts already exist in
different ownership and abstraction forms. The load-bearing unresolved edges
are canonical ownership of any shared packed capability, classification of
strategy versus law, and whether the Charon/Aeneas toolchain can extract a
generic algorithm without repeating the already-falsified Ryser path.
