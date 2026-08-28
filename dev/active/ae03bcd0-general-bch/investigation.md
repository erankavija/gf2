# Current-system investigation: `ae03bcd0`

This is a read-only investigation of the checkout at HEAD `c7834d96`. The epic
contract is in `.jit/issues/ae03bcd0-8881-4308-99f5-cad2b3c717d1.json:1-45`;
the planning brief is in `dev/plans/ae03bcd0-general-bch/planning-brief.md:10-66`.
The brief is planning input, not evidence of implementation. No Cargo build,
test suite, JIT command, source edit, or `.jit` mutation was performed.

## Claim classification (numbered verdicts)

Verdicts use `already-done`, `valid-and-open`, and `invalid-as-stated`.

1. **`valid-and-open` (with an exactness caveat).** `BchCode::new` accepts
   `n`, `k`, `t`, and a `Gf2mField`; its rustdoc says that `n` must divide
   `2^m-1`, but the implementation only asserts `n < field.order()` and
   `n > k`, with `t > 0` (`crates/gf2-coding/src/bch/core.rs:98-147`). It
   stores the supplied `k` and `t` after constructing the generator and does
   not assert `k == n - generator.degree()` (`core.rs:137-147`). The alternate
   `from_generator` path also accepts the supplied dimensions and only checks
   the same coarse inequalities (`core.rs:151-198`). These constructors panic
   through `assert!`/`expect` on invalid or unavailable field inputs rather than
   returning a typed construction error (`core.rs:126-135,201-232`).

2. **`valid-and-open`.** The hard-decision decoder returns only a `BitVec` and
   flips the positions returned by Chien search before extracting the first `k`
   bits (`crates/gf2-coding/src/bch/core.rs:946-991`). The syndrome path only
   distinguishes the all-zero case from the BM/Chien path (`core.rs:560-624`),
   and `chien_search` returns positions without a post-correction syndrome
   verification contract (`core.rs:1085-1145`). The batch and HIP decode paths
   repeat the same unverified correction shape (`core.rs:843-943`). No BCH
   `NoErrors`/`Corrected`/`Uncorrectable` result type is present; the existing
   `DecoderResult` is the soft-decoder accounting result in `traits.rs:111-189`.

3. **`invalid-as-stated` (the underlying gap is real).** `BlockEncoder` is
   hard-coded to `BitVec` (`crates/gf2-coding/src/traits.rs:198-219`) and
   `GeneratorMatrixAccess::generator_matrix` returns `BitMatrix`
   (`traits.rs:47-107`). However, caching is not mandated by the shared trait;
   the concrete `BchCode` implementation caches its first computed matrix in an
   `Arc<Mutex<Option<BitMatrix>>>` (`crates/gf2-coding/src/bch/core.rs:69-96,264-320`).
   The precise finding is therefore “bit-specific shared interfaces plus
   unconditional BCH caching,” not “the trait unconditionally caches.”

4. **`invalid-as-stated`.** `FieldVec<F>` exists as a generic finite-field
   vector (`crates/gf2-core/src/field/vec.rs:85-186`) and a dense generic
   `FieldMatrix<F: FiniteField>` already exists (`crates/gf2-core/src/field/matrix.rs:303-308`).
   Its current surface includes constructors and shape/accessors
   (`matrix.rs:312-460,641-780`), row/column views and elementary row
   operations (`matrix.rs:829-921,1064-1218`), transpose/diagonal/trace/sparse
   conversion (`matrix.rs:1221-1388`), `matvec` and transpose-matvec
   (`matrix.rs:1394-1554`), `MatrixLike` implementations (`matrix.rs:1557-1593`),
   and generic `gemm` (`matrix.rs:2774-2815`). It is not merely a design issue
   left by `ab791e27`, `bb85c68a`, or `cdcebf6a`; the completion report records
   the dense/sparse delivery (`dev/archive/bb85c68a-field-linear-algebra/active/bb85c68a-completion-report.md:1-32`).

5. **`valid-and-open`.** The current concrete extension configuration is
   `ExtConfig` with an associated base field and non-residue, not a relative
   extension relationship (`crates/gf2-core/src/gfpn/ext_config.rs:36-109`).
   `Gf2mField` independently carries its runtime binary-field parameters and
   elements (`crates/gf2-core/src/gf2m/field.rs:135-188,214-250`). No canonical
   public trait was found that supplies relative degree, checked membership,
   restriction, or relative Frobenius across two field types. The present
   `FiniteFieldExt::frobenius` is a same-field convenience trait, not a
   base-to-extension relationship (`crates/gf2-core/src/field/mod.rs:1-12`).

6. **`valid-and-open`.** `ExtConfig` documents only the binomials
   `x^2-beta` and `x^3-beta` (`crates/gf2-core/src/gfpn/ext_config.rs:36-78`),
   and the concrete types are `QuadraticExt<C>` and `CubicExt<C>` with two and
   three coefficients (`crates/gf2-core/src/gfpn/quadratic.rs:201-232`; 
   `crates/gf2-core/src/gfpn/cubic.rs:221-254`). The configuration is a
   compile-time marker/associated-constant design. No arbitrary-degree
   polynomial quotient type or runtime polynomial-modulus extension type was
   found under `crates/gf2-core/src`.

7. **`valid-and-open`.** `Gf2mField::primitive_element` is available only
   when tables exist (`crates/gf2-core/src/gf2m/field.rs:438-455`), and the
   element method `minimal_polynomial` is explicitly over GF(2)
   (`field.rs:973-1012`). `verify_primitive` is a u64-specialized test of the
   defining binary polynomial/root (`field.rs:659-721`), not a reusable exact
   order constructor for an arbitrary finite field or subfield. No generic API
   deriving an element of exact multiplicative order `n`, or a minimal
   polynomial over an arbitrary base field, was found.

8. **`invalid-as-stated`, but the general requirement remains open.** There is
   no broad Conway-polynomial table, but the registry is not Conway-free:
   `PrimitivePolynomialDatabase::standard` contains a documented single
   GF(2^32) Conway entry sourced to Frank Lübeck’s Conway database
   (`crates/gf2-core/src/primitive_polys.rs:79-119`). The registry covers
   primitive entries for `m=2..=16` and that `m=32` entry, while
   `standard_u128` covers `m=64..=127` with irreducibility-only guarantees
   (`primitive_polys.rs:1-37,200-340`). The tests verify primitive entries in
   the small range and irreducibility in the u128 range
   (`primitive_polys.rs:480-620`); `new_verified` compares to the registry and
   warns on conflicts but permits unknown polynomials (`gf2m/field.rs:804-834`).
   This is not a verified Conway database plus deterministic fallback search.

9. **`valid-and-open`.** The generic polynomial module exposes multiplication,
   Euclidean division, GCD, evaluation, root construction, and related batch
   operations (`crates/gf2-core/src/field/poly.rs:161-193,1440-1475`). There is
   no public `FieldPoly::lcm`; BCH has a private `lcm_poly` copy using GCD and
   exact remainder checking (`crates/gf2-coding/src/bch/core.rs:201-232`).

10. **`valid-and-open` (with format detail).** `BitMatrix::save_to_file` opens
   the destination with `File::create` before writing (`crates/gf2-core/src/io/matrix.rs:18-39`),
   so replacement is non-atomic. The binary payload carries the format header,
   BitMatrix type tag, row/column dimensions, version, and row-major words
   (`matrix.rs:42-110`); load validates the magic/type/length and version but
   does not checksum the payload or carry a field identity
   (`matrix.rs:168-223`). The format header has checksum flags in its schema,
   but the flags are explicitly “not implemented yet” (`crates/gf2-core/src/io/mod.rs:18-32,100-104`).
   Current `.gf2` consumers are binary matrices, so there is no field identity
   to validate today.

11. **`already-done` (partial decoder scope).** HIP has a BCH syndrome kernel
   (`crates/gf2-kernels-hip/hip/bch_syndrome.hip:84-160`) and a safe wrapper
   exporting `GpuBchSyndrome`/`BchFieldTables` (`crates/gf2-kernels-hip/src/lib.rs:53-67`;
   `src/launch_bch_syndrome.rs:144-185`). `BchDecoder::compute_syndromes_batch_gpu`
   repacks `BitVec` frames, uploads the live `Gf2mField` exp/log tables, and
   returns GPU syndromes (`crates/gf2-coding/src/bch/core.rs:699-841`).
   `decode_batch_gpu` uses those syndromes but leaves BM, Chien, correction, and
   extraction on the CPU (`core.rs:843-943`). Field-level GPU tests cover GF(2^4)
   multiplication, GF(2^14)/GF(2^16) table upload, and a BCH(15) Horner fixture
   (`crates/gf2-kernels-hip/tests/gpu_bch_syndrome_field.rs:1-18,51-182`); the
   simulation tests cover 200-frame Short/Normal CPU-vs-GPU syndrome identity
   and decode equivalence (`crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs:1-18,99-164`).
   The throughput executable measures syndrome evaluation, not a new GPU encoder
   (`crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs:1-25,148-180`).

12. **`already-done` for the existing pipeline, qualified for GF(2^m).** The
   documented pipeline verifies `gfp/` and `gfpn/` arithmetic and extracts
   field-trait declarations with opaque bodies (`docs/lean4-verification-pipeline.md:18-25`).
   The current extraction surface makes `gfp/` and `gfpn/` transparent, starts
   from raw GF(2^m) multiplication, and marks runtime `gf2m::field`,
   `gf2m::generation`, `field`, `matrix`, `primitive_polys`, and related
   modules opaque (`scripts/verify-lean.sh:94-131`). The committed proof root
   imports the prime-field, quadratic/cubic, and GF(2^m) proof modules
   (`proofs/Gf2Core.lean:1-19`); the GF(2^m) proofs include raw multiply/add/
   inverse progress obligations (`proofs/Gf2Core/Proofs/Gf2mDefs.lean:1-27`;
   `Gf2mProgress.lean:1-15,69-114`). `scripts/verify-lean.sh` feeds the Charon /
   Aeneas extraction and then the Lean build; the committed build gate is
   `cd proofs && lake build` (`docs/lean4-verification-pipeline.md:73-85,180-191`).
   Therefore “binary-extension arithmetic is proven/extracted” is true for the
   raw bounded path, not for the runtime field-carrying implementation or
   `FieldPoly`; the latter remains behind the opaque `field` surface.

13. **`already-done` for dispatch/primitives; BCH batch encoding is open.**
   `gf2-kernels-simd` exposes safe function-pointer dispatch with a scalar
   fallback (`crates/gf2-kernels-simd/src/lib.rs:1-10,80-108`), and x86 detection
   selects AVX2 via `is_x86_feature_detected!("avx2")` (`src/x86/mod.rs:37-43`).
   The AVX2 function table and scalar-tail wrappers are in `src/x86/avx2.rs:560-660`.
   Core’s `SimdBackend::detect`/`maybe_simd` is the higher-level plug-in seam
   (`crates/gf2-core/src/kernels/simd/mod.rs:12-95`). Existing GF(2^m)
   primitives include PCLMULQDQ function pointers and batch dispatch
   (`crates/gf2-kernels-simd/src/gf2m.rs:1-31,77-124`; `gf2m_batch.rs:1-15,44-139`)
   and wide carry-less multiply helpers (`crates/gf2-core/src/gf2m/wide.rs:2006-2114`).
   `BitSlice` is a view/get/set abstraction, not a BCH remainder kernel
   (`crates/gf2-core/src/bitslice.rs:1-130`). BCH’s own batch encoder is still a
   sequential map and carries a `ComputeBackend` TODO (`crates/gf2-coding/src/bch/core.rs:357-400`).

14. **`already-done`, with coexistence/cutover still open.** `ExtendedBchCode`
   exists and is a wrapper around a `LinearBlockCode` (`crates/gf2-coding/src/bch/extended.rs:55-80`).
   `from_bch(&BchCode)` materializes an extended generator and parity-check
   matrix, appends the overall parity bit, and retains `base_t`
   (`extended.rs:82-175`). It has fixed convenience constructors for the
   supported binary eBCH families (`extended.rs:252-275` and following
   constructors), and consumers use its parity-check matrix in GRAND/fading
   paths (for example `crates/gf2-coding/src/grand/sogrand.rs:1530` and
   `src/fading.rs:1351`). The epic’s generic one-symbol transformation is not
   present; the current eBCH surface is binary, matrix-materializing, and
   specifically coupled to `BchCode`.

15. **`invalid-as-stated`.** Criterion BCH benches already exist. The
   `bch_parallel` target benchmarks batch decode and single-vs-batch behavior
   (`crates/gf2-coding/benches/bch_parallel.rs:1-14,52-101`), and
   `batch_operations` benchmarks BCH batch and sequential-vs-batch encoding
   (`crates/gf2-coding/benches/batch_operations.rs:57-75,120-162`). The open
   evidence gap is a field-generic/AVX2 encoder and generator-materialization
   survey with committed external receipts, not the absence of all Criterion
   BCH throughput coverage.

## Prior art

- `dev/plans/ae03bcd0-general-bch/planning-brief.md:22-47` fixes the intended
  semantic shape: `BchSpec` as the independent-input model, one
  `BchCode::construct` path, generic derived wrappers, a diagnostic decoder,
  and separate algorithm-family versus AVX2 dispatch. It also explicitly keeps
  stronger Hartmann–Tzeng/Roos bounds out of scope (`brief.md:10-18`).
- `dev/plans/field_poly_module_overview.md:14-26,37-59` is the current reader
  map for `FieldPoly`, `FieldVec`, batch polynomial operations, and the shared
  axiom-test harness. It identifies `FieldPoly` as the single source of truth
  and records the existing generic division/GCD/evaluation/product surface.
- The FieldMatrix design records the delivered abstraction, `MatrixLike`
  ownership choices, views, sparse conversion, and the decision to preserve
  `BitMatrix` specialization (`dev/archive/bb85c68a-field-linear-algebra/active/ab791e27-design-fieldmatrix-f-finitefield-dense-matrix-ty/ab791e27-design.md:1-73,129-153`).
  The expression-template document is design-only and describes a possible
  fused algebra rather than a missing base matrix type
  (`dev/archive/bb85c68a-field-linear-algebra/plans/cdcebf6a-design-fieldmatrix-expression-template-algebra-p/expression_templates_design.md:1-39`).
  The completion report states that dense/sparse FieldMatrix, decomposition,
  polynomial, and Criterion benchmark work was delivered
  (`.../bb85c68a-completion-report.md:1-32,42-78`).
- `dev/archive/806eb14e-hip-gpu-prototype/active/9012f8a0/gpu-batch-bch-syndrome-plan.md:14-30,242-320`
  already specifies the implemented HIP split: device syndrome evaluation,
  CPU BM/Chien, exact small fixtures, simulation identity tests, and a manual
  throughput executable. It should be treated as prior implementation evidence,
  not as a plan to rediscover.
- `dev/plans/sota_target_matrix.md:9-24,126-145,170-182` establishes the
  benchmark-receipt convention and the pinned external-library landscape. It
  also records that GF(2^m) external reference coverage is incomplete for many
  matrix operations (`sota_target_matrix.md:226-244`). The FLINT and NTL
  promotion material is therefore useful for evidence conventions and selected
  GF(2^m)/GF(p) baselines, but it does not supply a BCH encoder oracle.
- `dev/archive/e095a100-gfpm-arithmetic/plans/gfpn_planning_session.md:1-8`
  records the earlier motivation for general finite-field arithmetic while
  retaining binary-specific performance paths. The implemented `gfpn` result is
  the narrower quadratic/cubic tower seen in `gfpn/mod.rs`, not arbitrary
  polynomial quotients.
- `dev/plans/70972f06_audit.md:1-35` records that BCH’s
  `alpha_power.minimal_polynomial()` call remains a Gf2mElement operation. It
  is useful cutover evidence: moving this operation to a reusable relative-field
  API must preserve the current call’s semantics while removing the BCH-private
  dependency.

## Consumer inventory

The following inventory is the complete current-tree path set returned by exact
symbol/API searches, grouped so that implementation consumers are separated
from historical and research references. Paths are included even when the match
is rustdoc, a fixture, or a development artifact because REQ-10 cutover must
account for those references. Direct BCH implementation and test anchors are
`crates/gf2-coding/src/bch/core.rs:1-24`, `src/bch/mod.rs:38-42`,
`tests/bch_tests.rs:1-10`, and `tests/dvb_t2_bch_verification.rs:1-20`.

### `BchCode`, the `bch` module, and eBCH

Direct code/tests/benches/examples and simulation consumers:

`crates/gf2-coding/src/bch/core.rs`, `src/bch/mod.rs`,
`src/bch/dvb_t2/mod.rs`, `src/bch/dvb_t2/generators.rs`,
`src/bch/dvb_t2/params.rs`, `src/bch/extended.rs`,
`src/ldpc/dvb_t2/concat.rs`, `src/lib.rs`, `src/product/mod.rs`,
`src/product/chase_pyndiah.rs`, `src/grand/orbgrand.rs`,
`src/grand/sogrand.rs`, `src/fading.rs`, `src/gldpc/mod.rs`,
`src/bcjr/mod.rs`, `src/bin/sim_runner.rs`,
`benches/bch_parallel.rs`, `benches/batch_operations.rs`,
`examples/dvb_t2_bch_demo.rs`, `tests/bch_tests.rs`,
`tests/dvb_t2_bch_verification.rs`, `tests/ebch_128_64_reference.rs`,
`tests/data/ebch_128_64_reference.json`, `tests/backend_integration.rs`,
`tests/grand_phase1_smoke.rs`, `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`,
`crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs`,
`crates/gf2-sim/tests/ebch_osd_campaign_cli.rs`,
`crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs`,
`crates/gf2-sim/tests/gpu_byte_identity.rs`,
`crates/gf2-sim/tests/osd_campaign_protocol.rs`, and
`crates/gf2-kernels-hip/tests/gpu_cpu_crosscheck.rs`.

The coupling points are visible in `dvb_t2/mod.rs:59-105` (inherent
`BchCode::dvb_t2`), `extended.rs:48-55,109-175` (`from_bch`), and the GPU
simulation test’s construction/decode path (`gpu_bch_syndrome_byte_identity.rs:99-140`).

Documentation, campaign, data, presentation, and development references:

`crates/gf2-coding/README.md`, `ROADMAP.md`, `docs/PARALLELIZATION.md`,
`docs/SYSTEMATIC_ENCODING_CONVENTION.md`,
`docs/archive/QUALITY_AUDIT_REPORT.md`,
`crates/gf2-core/docs/PRIMITIVE_POLYNOMIALS.md`,
`dev/campaigns/cp_ebch_sanity.toml`, `dev/campaigns/ebch32_vs_drm32.toml`,
`dev/campaigns/ebch_bcjr_compare.toml`, all `dev/campaigns/phase*.toml`,
`dev/campaigns/test_cp_ebch.toml`, `dev/reference_data/fig_*ebch*.csv`,
`dev/reference_data/osd_ebch_128_64_fossorier1994.csv`,
`dev/reference_data/osd_ebch_128_64_fossorier1994.md`,
`dev/reference_data/osd_ebch_128_64_fossorier1994_digitization/README.md`,
`extract.py`, and `yue2022_fig1_receipt.json` in that directory,
`dev/simulation_results/fig7_comparison_report.txt`,
`dev/simulation_results/phase1_comparison_report.md`,
`dev/simulation_results/phase4_comparison_report.md`, every file under
`dev/simulation_results/osd-ebch-128-64/`,
`dev/bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md`,
`docs/presentations/6efb756b-grand-sogrand/talk.html`,
`docs/presentations/figures/fig5_ebch_64_57_comparison.svg`,
`fig6_ebch_16_7_comparison.svg`, and
`generate_grand_comparison_plots.py`.

Historical JIT/development references are also present in
`.jit/events.jsonl`, the BCH-related `.jit/issues/*.json` records, and these
tracked development documents: `dev/active/DOCUMENTATION_AUDIT.md`,
`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md`,
`dev/active/fa787f85-documentation-overhaul/fa787f85-rustdoc-example-verdicts.tsv`,
the `dev/archive/6efb756b-grand/active/` handoffs/completions, the
`dev/archive/806eb14e-hip-gpu-prototype/active/9012f8a0/` plan and related
handoff, the `dev/archive/b7157be6-osd/active/` investigation/plan/handoffs,
and the `dev/archive/f9717e7e-gf2-sim/active/ec530af9/` pipeline design.

### `BlockEncoder`, `GeneratorMatrixAccess`, and parity-check surfaces

There is no `ParityCheckAccess`/`ParityCheckMatrixAccess` trait in the current
trait module. The shared trait declarations are only `GeneratorMatrixAccess`,
`BlockEncoder`, `HardDecisionDecoder`, soft/streaming traits
(`crates/gf2-coding/src/traits.rs:47-75,198-240,254-385`). Parity-check access is
currently inherent `parity_check` on `LinearBlockCode` and several code types,
including `LinearBlockCode::parity_check` (`crates/gf2-coding/src/linear.rs:112-197`).

Exact source/test/bench/example consumers of `BlockEncoder` or
`GeneratorMatrixAccess` are:

`crates/gf2-coding/src/traits.rs`, `src/linear.rs`, `src/bch/core.rs`,
`src/bch/mod.rs`, `src/bch/extended.rs`, `src/bch/dvb_t2/mod.rs`,
`src/bcjr/mod.rs`, `src/crc.rs`, `src/drm.rs`, `src/dvb_t2_bicm_harness.rs`,
`src/gldpc/mod.rs`, `src/ldpc/core.rs`, `src/ldpc/dvb_t2/concat.rs`,
`src/ldpc/encoding/cache.rs`, `src/ldpc/encoding/ira.rs`,
`src/ldpc/nr_5g/mod.rs`, `src/osd/generator.rs`,
`src/product/mod.rs`, `src/grand/orbgrand.rs`, `src/grand/sogrand.rs`,
`src/simulation.rs`, `src/bin/check_encoding.rs`,
`src/bin/sim_checkpoint_helper.rs`, `src/bin/sim_runner.rs`,
`src/bin/validate_ldpc_cache.rs`, `benches/batch_operations.rs`,
`benches/bch_parallel.rs`, `benches/ldpc_throughput.rs`,
`benches/linear_codes.rs`, `benches/profile_ldpc_encode.rs`,
`benches/quick_parallel.rs`, `benches/sparse_preprocessing.rs`,
`examples/block_code_intro.rs`, `examples/dvb_t2_bch_demo.rs`,
`examples/hamming_basic.rs`, `examples/ldpc_cache_file_io.rs`,
`examples/ldpc_encoding_with_cache.rs`, `examples/ldpc_mother_check.rs`,
`examples/sogrand_crc_probe.rs`, `examples/visualize_large_matrices.rs`,
and the corresponding coding tests in `tests/backend_integration.rs`,
`bch_tests.rs`, `dvb_t2_bch_verification.rs`,
`dvb_t2_ldpc_verification.rs`, `dvb_t2_ldpc_verification_suite.rs`,
`ebch_128_64_reference.rs`, `grand_phase1_smoke.rs`,
`ira_encoder_correctness.rs`, `ldpc_encoding_tests.rs`,
`ldpc_systematic_encoding.rs`, `linear_codes_integration.rs`,
`nr5g_diag_test.rs`, `nr5g_external_vectors.rs`, `nr5g_regression.rs`,
`ldpc_cache_io.rs`, and `verify_cached_generator.rs`.

HIP/SIMD direct trait references are limited to
`crates/gf2-kernels-hip/tests/gpu_cpu_crosscheck.rs` and
`crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs`; the SIMD kernel
crate itself does not implement these coding traits. Simulation-facing matches
are in `crates/gf2-sim/benches/nr_5g_realtime.rs`,
`src/stages/nr_5g.rs`, `src/gpu/ldpc_bp.rs`, and the GPU byte-identity tests.

### BitMatrix save/load and `.gf2` files

Runtime/read-write consumers are:

`crates/gf2-core/src/io/matrix.rs` (format implementation),
`crates/gf2-core/src/io/bitvec.rs`, `src/io/sparse.rs` (adjacent IO modules),
`crates/gf2-coding/src/ldpc/encoding/cache.rs:317-429` (writes parity
matrices and reloads them), `crates/gf2-coding/tests/ldpc_cache_io.rs:38-79`,
`crates/gf2-coding/tests/common/mod.rs:84-93` (cache location/gating),
`crates/gf2-coding/docs/SDR_INTEGRATION.md`, and `benchmarks/analyze.py`.
The only exact production `.gf2` round trip is the LDPC encoding cache;
`BitMatrix`’s own tests live in `crates/gf2-core/src/io/matrix.rs`.

The remaining exact matches are proof/generated or documentation references:
`dev/active/7d7c647c/probes/AS1_lean/Funs.lean` and
`dev/active/fa787f85-documentation-overhaul/fa787f85-rustdoc-example-verdicts.tsv`.
No HIP/SIMD crate, BCH matrix path, or simulation checkpoint uses the `.gf2`
BitMatrix format.

### Gf2m primitive/minimal-polynomial APIs

Direct call-site consumers of `.minimal_polynomial()`, `.primitive_element()`,
or `.verify_primitive()` are:

`crates/gf2-coding/src/bch/core.rs:207-217,607-614,811-819,1098-1102`,
`crates/gf2-coding/tests/bch_primitive_verification.rs:1-35`,
`crates/gf2-coding/tests/bch_tests.rs:35-45,650-665`,
`crates/gf2-core/src/gf2m/field.rs` (implementation and canonical tests),
`crates/gf2-core/src/gf2m/generation.rs:248-401`,
`crates/gf2-core/benches/polynomial.rs:245`,
`crates/gf2-core/benches/primitive_poly.rs:98-253`,
`crates/gf2-core/examples/primitive_polynomial_verification.rs:1-32`,
`crates/gf2-core/docs/PRIMITIVE_POLYNOMIALS.md:70-80,270-360`,
`crates/gf2-core/docs/archive/GF2M_POLY_UTILITIES_REQUIREMENTS.md:171`,
`dev/plans/70972f06_audit.md:28-29`, and
`dev/archive/806eb14e-hip-gpu-prototype/active/9012f8a0/gpu-batch-bch-syndrome-plan.md:73`.
The HIP field test additionally consumes `primitive_element` to form evaluation
points (`crates/gf2-kernels-hip/tests/gpu_bch_syndrome_field.rs:39-48`). No
consumer exists in `gf2-kernels-simd` or `gf2-sim` for these exact APIs; their
matches are through BCH/eBCH consumers listed above.

## Primitive verification

- **Atomic save pattern.** The BitMatrix save path is not atomic
  (`crates/gf2-core/src/io/matrix.rs:18-39`). The reusable repository pattern
  is `gf2-sim`’s `atomic_write_json`: write a PID-tagged sibling, `sync_all`,
  rename, then sync the directory (`crates/gf2-sim/src/checkpoint/mod.rs:174-212`).
  A repository-wide `rename(` search found no other production atomic-replace
  implementation in the crate sources.
- **Checksums.** `gf2-core`’s IO schema describes a possible BLAKE3 trailer but
  checksum flags are not implemented (`crates/gf2-core/src/io/mod.rs:18-32,100-104`).
  `gf2-sim` has two reusable integrity precedents: BLAKE3 configuration hashes
  in checkpoint envelopes (`crates/gf2-sim/src/checkpoint/mod.rs:205-212` and
  `src/snr_checkpoint.rs:204-268`), and SHA-256 sidecar files with typed digest,
  normalized-path, duplicate, and external `sha256sum -c` validation in the
  permanent-campaign provenance module (`src/permanent_campaign/provenance.rs:961-1024,1814-1837`).
  Neither is a generic `BitMatrix` serializer checksum.
- **FieldPoly division.** `FieldPoly::div_rem` is Euclidean long division and
  returns both quotient and remainder; it asserts only that the divisor is
  nonzero (`crates/gf2-core/src/field/poly.rs:1745-1788`). It is not an
  exact-division-only API and does not silently truncate: callers must check
  the returned remainder. BCH’s private LCM does check it (`bch/core.rs:224-232`).
  The separate `div_rem_fast` also reconstructs a remainder from the quotient,
  rather than changing the exactness contract (`field/poly.rs:3217-3239,3324-3339`).
- **`with_tables` in BCH.** `Gf2mField::with_tables` generates log/exp tables
  only for `m <= 16` and otherwise returns the field unchanged
  (`crates/gf2-core/src/gf2m/field.rs:388-436`). The current BCH constructor
  calls `with_tables` before generator construction (`crates/gf2-coding/src/bch/core.rs:126-147`),
  generator construction requires `primitive_element` and element minimal
  polynomials (`core.rs:201-217`), and syndrome evaluation also requires a
  primitive element (`core.rs:607-614`). Thus the current BCH hot path requires
  table-backed fields even though raw field arithmetic itself does not.
- **Determinism and test budgets.** New mathematical/back-end suites must test
  observable semantics, use shared field/codec contracts, and keep seeded
  results identical across workers, scheduling, checkpoint/resume, and fallback
  paths (`AGENTS.md:62-75,129-143`). Bit-packed suites must include the 0, 1,
  63, 64, and 65 word-boundary cases and preserve little-endian indexing/tail
  padding (`AGENTS.md:66-69,129`). The fast tier is release-mode with a
  five-second per-test kill and 60-second suite budget; longer tests need
  descriptive `slow:`/`sim:` ignores, and the nightly ignored tier is capped at
  600 seconds with host-data/benchmark self-gating where needed
  (`AGENTS.md:83-90,143`). No `dev/TESTING.md` exists in this checkout; the
  repository-wide test contract is the cited `AGENTS.md`.

## Architecture fit

- **Ownership and dependencies.** `gf2-core` owns bit storage, dense/sparse
  linear algebra, finite-field abstractions/arithmetic, and safe dispatch;
  `gf2-coding` owns codes/modems/channels and depends inward on core;
  `gf2-algebra` owns packed F_3/F_5/F_7/permanent work; `gf2-sim` owns CPU/GPU
  orchestration; `proofs/` owns selected Lean paths
  (`AGENTS.md:38-58`). The Cargo manifests confirm `gf2-coding` depends on
  core and optionally HIP while core depends on the isolated SIMD kernel
  (`crates/gf2-coding/Cargo.toml:10-16,48-59`; `crates/gf2-core/Cargo.toml:10-19`).
  The two kernel crates are the only production unsafe boundary
  (`AGENTS.md:49-52`; `crates/gf2-kernels-hip/src/lib.rs:1-5,26-33`).
- **Library-first placement.** Relative extension, quotient-field,
  exact-order, minimal-polynomial-over-subfield, `FieldPoly::lcm`, and generic
  `FieldMatrix`/serialization primitives belong in `gf2-core`; BCH semantic
  construction, code transformations, binary decoder outcomes, and generic
  code-domain traits belong in `gf2-coding`. Binaries/simulation remain thin
  consumers. This follows the inward dependency and library-first invariants
  (`AGENTS.md:54-58,129-146`) and avoids copying the current BCH-private LCM or
  eBCH matrix wrapper into another layer.
- **Reuse points.** The strongest existing reusable substrates are
  `FieldVec` (`crates/gf2-core/src/field/vec.rs:85-186,425-492,592-667`),
  `FieldMatrix` and `gemm` (`field/matrix.rs:303-308,1394-1554,2774-2815`),
  `FieldPoly` (`field/poly.rs:161-193`), `FiniteField`/`ConstField`
  (`field/mod.rs:1-12,64-76`), the shared field axiom suite, and the existing
  SIMD function-pointer/fallback dispatch (`gf2-kernels-simd/src/lib.rs:80-108`).
- **Encoding/kernel seam.** A BCH batch implementation can compose a
  profile-driven algorithm-family selector with the existing AVX2/scalar
  function-pointer seam. The current BCH code has no such selector and is
  sequential (`crates/gf2-coding/src/bch/core.rs:357-400`); generic CLMUL and
  wide polynomial primitives are in core/kernel crates, while unsafe code must
  remain isolated in `gf2-kernels-simd` (`gf2-core/src/gf2m/wide.rs:2006-2114`;
  `AGENTS.md:49-58`).
- **MSRV and feature detection.** The workspace crates declare Rust `1.95`
  (`crates/gf2-core/Cargo.toml:10`, `crates/gf2-coding/Cargo.toml:10`,
  `crates/gf2-sim/Cargo.toml:10`, `crates/gf2-kernels-hip/Cargo.toml:7`), and
  the repository contract requires Rust 1.95 verification for intrinsic work
  plus a scalar fallback (`AGENTS.md:108-114`). Existing AVX2 detection is the
  `is_x86_feature_detected!("avx2")` branch returning an AVX2 function table or
  `None`, with safe scalar selection above it (`gf2-kernels-simd/src/x86/mod.rs:37-43`;
  `src/lib.rs:80-108`).
- **Research evidence.** Use the SOTA target matrix’s pinned reference and
  receipt convention (`dev/plans/sota_target_matrix.md:9-24,155-170`) and the
  existing Criterion/CSV benchmark harnesses rather than embedding hand-written
  performance claims. Existing BCH benches are a baseline, while the current
  HIP benchmark is a manually invoked simulation executable
  (`crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs:21-34`).

## Open unknowns

- The current source does not establish a canonical representation for relative
  extension identity/embedding when compile-time and runtime quotient fields
  coexist; the exact identity and conversion semantics remain a design/proof
  decision.
- No current code establishes a deterministic verified irreducible-polynomial
  search policy, Conway coverage beyond the single GF(2^32) registry entry, or
  materialization/error behavior beyond `usize`/`u128`. These are requirements
  in the epic contract, not existing behavior (`.jit/issues/ae03bcd0-8881-4308-99f5-cad2b3c717d1.json:11-17`).
- The inventory finds no generic parity-check access trait, but the exact
  generic interface shape and compatibility boundary for existing code families
  must be decided from their many inherent `parity_check` methods.
- It is not yet verified which external libraries can serve as independent
  *BCH encoder* and *generator-materialization* oracles; the SOTA matrix mostly
  covers linear algebra and explicitly records GF(2^m) oracle gaps
  (`dev/plans/sota_target_matrix.md:226-244`).
- The current HIP path is binary GF(2^m), table-backed, and CPU-completes the
  decoder. Equivalence after a future typed decoder result and canonical
  construction cutover is not yet demonstrated; current tests establish only
  the existing status-less behavior (`crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs:1-18,133-164`).
- No repository test/build was run during this investigation, so current
  compile/test health and host-specific benchmark numbers remain unverified.
