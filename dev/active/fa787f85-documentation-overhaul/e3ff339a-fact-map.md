# Fact map for the gf2-core, gf2-coding and gf2-algebra READMEs

Each row names a fact that the README of the stated crate carries at
revision `c6d620004695` and omits in its purpose-and-links form, and the
location that states it. A Rustdoc location is the module documentation in the
named source file, which the README reaches through its Rustdoc link; a
`Cargo.toml` location is the `[features]` table the README links.

## gf2-core

| Fact | Stated at |
|---|---|
| Dense $\mathrm{GF}(2)$ types `BitVec`, `BitSlice`, `BitSliceMut` and the row-major `BitMatrix`, re-exported at the crate root | [finite-field arithmetic, choosing a field type](../../../docs/concepts/finite-field-arithmetic.md#choosing-a-field-type); Rustdoc [`lib.rs`](../../../crates/gf2-core/src/lib.rs), [`matrix.rs`](../../../crates/gf2-core/src/matrix.rs) |
| M4RM multiplication, Gauss-Jordan inversion and RREF in `alg` | Rustdoc [`alg/matmul.rs`](../../../crates/gf2-core/src/alg/matmul.rs), [`alg/gauss.rs`](../../../crates/gf2-core/src/alg/gauss.rs), [`alg/rref.rs`](../../../crates/gf2-core/src/alg/rref.rs) |
| Polar transform | Rustdoc of `BitVec::polar_transform` in [`bitvec.rs`](../../../crates/gf2-core/src/bitvec.rs) |
| Sparse $\mathrm{GF}(2)$: `SpBitMatrix` (CSR), `SpBitMatrixDual` (row and column traversal), block-CSR and row permutations in `sparse` | Rustdoc [`lib.rs`](../../../crates/gf2-core/src/lib.rs), [`sparse.rs`](../../../crates/gf2-core/src/sparse.rs) |
| $\mathrm{GF}(2^m)$ generic over storage width | [single-word runtime fields](../../../docs/concepts/finite-field-arithmetic.md#single-word-runtime-fields) |
| Barrett, table and SIMD multiplication paths for $\mathrm{GF}(2^m)$ | [multiplication strategy](../../../docs/concepts/finite-field-arithmetic.md#multiplication-strategy) |
| Karatsuba multiplication | Rustdoc [`field/mod.rs`](../../../crates/gf2-core/src/field/mod.rs) (polynomial products, including `Gf2mPoly_`); [$\mathrm{GF}(p^m)$](../../../docs/concepts/finite-field-arithmetic.md#mathrmgfpm) (quadratic extension product) |
| Wide-degree binary fields | [multi-word compile-time fields](../../../docs/concepts/finite-field-arithmetic.md#multi-word-compile-time-fields) |
| Primitive-polynomial database and generation in `primitive_polys` and `gf2m` | [polynomial database](../../../docs/concepts/finite-field-arithmetic.md#polynomial-database) |
| `Fp<const P: u64>` in `gfp`: Montgomery form for generic primes, canonical-form reduction for Mersenne and Proth primes | [$\mathrm{GF}(p)$](../../../docs/concepts/finite-field-arithmetic.md#mathrmgfp) |
| `QuadraticExt` and `CubicExt` towers over `ExtConfig`, `QuotientField` for runtime moduli, field identity and extension witnesses in `gfpn` and `field::extension` | [$\mathrm{GF}(p^m)$](../../../docs/concepts/finite-field-arithmetic.md#mathrmgfpm); Rustdoc [`field/mod.rs`](../../../crates/gf2-core/src/field/mod.rs) |
| `FieldMatrix<F>` over any `FiniteField`: classical and Strassen-Winograd GEMM, PLE, RREF, nullspace, inverse, determinant, triangular solves, characteristic polynomial | [finite-field linear algebra at scale](../../../docs/tutorials/field-linear-algebra-at-scale.md); Rustdoc [`field/matrix.rs`](../../../crates/gf2-core/src/field/matrix.rs), [`field/ple.rs`](../../../crates/gf2-core/src/field/ple.rs), [`field/inverse.rs`](../../../crates/gf2-core/src/field/inverse.rs), [`field/triangular.rs`](../../../crates/gf2-core/src/field/triangular.rs), [`field/charpoly.rs`](../../../crates/gf2-core/src/field/charpoly.rs) |
| Sparse `SparseFieldMatrix<F>` | Rustdoc [`field/sparse_matrix.rs`](../../../crates/gf2-core/src/field/sparse_matrix.rs) |
| `FieldPoly`, NTT and batch inversion in `field` | Rustdoc [`field/mod.rs`](../../../crates/gf2-core/src/field/mod.rs) |
| Runtime scalar and SIMD dispatch in `kernels` | [SIMD dispatch](../../../docs/concepts/acceleration-architecture.md#simd-dispatch); Rustdoc [`kernels/mod.rs`](../../../crates/gf2-core/src/kernels/mod.rs) |
| Rayon batch backends in `compute` | [thread parallelism](../../../docs/how-to/select-acceleration.md#thread-parallelism); Rustdoc [`compute/mod.rs`](../../../crates/gf2-core/src/compute/mod.rs) |
| Typed tuning profiles in `tuning` | [route selection](../../../docs/concepts/acceleration-architecture.md#route-selection); [install a tuning profile](../../../docs/how-to/select-acceleration.md#install-a-tuning-profile) |
| Serialization in `io`, including checksummed `FieldMatrix` files | Rustdoc [`io/mod.rs`](../../../crates/gf2-core/src/io/mod.rs), [`io/field_matrix.rs`](../../../crates/gf2-core/src/io/field_matrix.rs) |
| Every field implementation passes the shared field-law suite `field::axiom_tests` under `test-support` | [shared contracts](../../../docs/concepts/finite-field-arithmetic.md#shared-contracts) |
| The crate denies `unsafe` code; unsafe SIMD lives in `gf2-kernels-simd` | [crate layering](../../../docs/concepts/acceleration-architecture.md#crate-layering) |
| Bit `i` lives in word `i >> 6` under mask `1u64 << (i & 63)`; padding bits are zero | [$\mathrm{GF}(2)$](../../../docs/concepts/finite-field-arithmetic.md#mathrmgf2) |
| Default features | [Cargo features](../../../docs/reference/supported-configurations.md#cargo-features) |
| Effect of `rand`, `simd`, `parallel`, `visualization`, `tuning-profile`, `test-support` and `io` | comments of the `[features]` table in [`Cargo.toml`](../../../crates/gf2-core/Cargo.toml) |
| Dependency setup | [installation](../../../docs/reference/supported-configurations.md#installation) |

## gf2-coding

| Fact | Stated at |
|---|---|
| Hamming codes with syndrome-table decoding in `linear` | Rustdoc [`lib.rs`](../../../crates/gf2-coding/src/lib.rs) |
| BCH over any supported base field from independent length, designed-distance and first-root inputs; binary Berlekamp-Massey and Chien decoding; shortening through `transform`; explicit coordinate layouts and typed decode outcomes | Rustdoc [`bch/mod.rs`](../../../crates/gf2-coding/src/bch/mod.rs), [`bch/spec.rs`](../../../crates/gf2-coding/src/bch/spec.rs), [`transform/mod.rs`](../../../crates/gf2-coding/src/transform/mod.rs) |
| Quasi-cyclic and edge-list LDPC codes; min-sum, normalized min-sum, offset min-sum and sum-product belief propagation | [5G NR LDPC](../../../docs/reference/standards-conformance.md#5g-nr-ldpc); Rustdoc [`ldpc/core.rs`](../../../crates/gf2-coding/src/ldpc/core.rs), [`ldpc/min_sum.rs`](../../../crates/gf2-coding/src/ldpc/min_sum.rs) |
| Richardson-Urbanke encoding with file-cached generators | Rustdoc [`ldpc/core.rs`](../../../crates/gf2-coding/src/ldpc/core.rs), [`ldpc/encoding/cache.rs`](../../../crates/gf2-coding/src/ldpc/encoding/cache.rs) |
| Product codes with Chase-Pyndiah and SO-GRAND or BCJR turbo decoding | Rustdoc [`product/mod.rs`](../../../crates/gf2-coding/src/product/mod.rs), [`product/chase_pyndiah.rs`](../../../crates/gf2-coding/src/product/chase_pyndiah.rs) |
| Generalized LDPC | Rustdoc [`gldpc/mod.rs`](../../../crates/gf2-coding/src/gldpc/mod.rs) |
| ORBGRAND and SO-GRAND | Rustdoc [`grand/mod.rs`](../../../crates/gf2-coding/src/grand/mod.rs) |
| BCJR | Rustdoc [`bcjr/mod.rs`](../../../crates/gf2-coding/src/bcjr/mod.rs) |
| Ordered-statistics decoding and BP-OSD | Rustdoc [`osd/mod.rs`](../../../crates/gf2-coding/src/osd/mod.rs), [`osd/bp_osd.rs`](../../../crates/gf2-coding/src/osd/bp_osd.rs) |
| Convolutional encoder with caller-supplied generator polynomials and hard-decision Viterbi decoder | Rustdoc [`convolutional.rs`](../../../crates/gf2-coding/src/convolutional.rs) |
| CRC codes; Reed-Muller subcodes with polar-transform extension | Rustdoc [`crc.rs`](../../../crates/gf2-coding/src/crc.rs), [`drm.rs`](../../../crates/gf2-coding/src/drm.rs) |
| Modem: BPSK and Gray-coded square QAM presets, validated builder for custom constellations, exact log-MAP reference and Gray-QAM fast backends behind one trait layer | Rustdoc [`modem/mod.rs`](../../../crates/gf2-coding/src/modem/mod.rs) |
| AWGN and Rician fading channels, SNR and capacity helpers | Rustdoc [`channel.rs`](../../../crates/gf2-coding/src/channel.rs), [`fading.rs`](../../../crates/gf2-coding/src/fading.rs), [`info_theory.rs`](../../../crates/gf2-coding/src/info_theory.rs) |
| BER/FER harness with checkpointed campaigns in `simulation` | [run a simulation campaign](../../../docs/how-to/run-simulation-campaigns.md); Rustdoc [`simulation.rs`](../../../crates/gf2-coding/src/simulation.rs) |
| Block-code and decoder interfaces are traits in `traits`; `llr::Llr` is the soft-value type | Rustdoc [`traits.rs`](../../../crates/gf2-coding/src/traits.rs), [`lib.rs`](../../../crates/gf2-coding/src/lib.rs) |
| DVB-T2 coverage: LDPC and outer BCH for both frame sizes at every code rate, bit interleaver, BCH and LDPC concatenation | [DVB-T2 configurations](../../../docs/reference/standards-conformance.md#configurations) |
| DVB-T2 QAM mapping and BICM chain | Rustdoc [`modem/mod.rs`](../../../crates/gf2-coding/src/modem/mod.rs), [`dvb_t2_bicm_harness.rs`](../../../crates/gf2-coding/src/dvb_t2_bicm_harness.rs) |
| DVB-T2 evidence: reference streams read from `$DVB_TEST_VECTORS_PATH`, and test behavior when they are absent | [test vectors](../../../docs/reference/standards-conformance.md#test-vectors) |
| 5G NR coverage: base graphs BG1 and BG2 at every lifting size, rate matching | [5G NR LDPC](../../../docs/reference/standards-conformance.md#5g-nr-ldpc) |
| 5G NR evidence: shift tables compared with external reference tables of recorded provenance | [external evidence](../../../docs/reference/standards-conformance.md#external-evidence) |
| Default features; `simd`, `visualization` and `bench-csv` enable `gf2-core` features | [Cargo features](../../../docs/reference/supported-configurations.md#cargo-features) |
| Effect of `sim-observability`, `llr-f64`, `tuning-profile` and `test-support` | comments of the `[features]` table in [`Cargo.toml`](../../../crates/gf2-coding/Cargo.toml) |
| Effect of `parallel` | [thread parallelism](../../../docs/how-to/select-acceleration.md#thread-parallelism) |
| Effect of `hip`: batched BCJR, Gray-QAM soft demapping and BCH syndrome evaluation | [GPU offload](../../../docs/how-to/select-acceleration.md#gpu-offload) |
| `gf2-core` owns runtime SIMD dispatch for bit-level and elimination operations | [SIMD dispatch](../../../docs/concepts/acceleration-architecture.md#simd-dispatch) |
| `gf2-kernels-simd` holds the batch BCH encoding kernels; `bch::encode` selects among algorithm families with identical output under the tuning profile in `tuning` | [admit the BCH encode families](../../../docs/how-to/select-acceleration.md#admit-the-bch-encode-families); Rustdoc [`bch/encode.rs`](../../../crates/gf2-coding/src/bch/encode.rs), [`tuning.rs`](../../../crates/gf2-coding/src/tuning.rs) |
| `gf2-kernels-hip` holds the GPU kernels, sits outside the default workspace and needs `hipcc` and an AMD GPU | [GPU](../../../docs/reference/supported-configurations.md#gpu); [workspace exclusion](../../../docs/reference/supported-configurations.md#workspace-exclusion) |
| The crate denies `unsafe` code and selects among the backends | [crate layering](../../../docs/concepts/acceleration-architecture.md#crate-layering) |
| Test command | [supported toolchain and commands](../../../AGENTS.md#supported-toolchain-and-commands) |
| `generate_ldpc_cache` and `validate_ldpc_cache` binaries | Rustdoc and usage text of [`generate_ldpc_cache.rs`](../../../crates/gf2-coding/src/bin/generate_ldpc_cache.rs); Rustdoc of [`validate_ldpc_cache.rs`](../../../crates/gf2-coding/src/bin/validate_ldpc_cache.rs) |
| Examples run in release mode | [supported toolchain and commands](../../../AGENTS.md#supported-toolchain-and-commands) |
| Module documentation carries the BCH, DVB-T2 and modem guides | Rustdoc [`lib.rs`](../../../crates/gf2-coding/src/lib.rs), [`bch/mod.rs`](../../../crates/gf2-coding/src/bch/mod.rs), [`modem/mod.rs`](../../../crates/gf2-coding/src/modem/mod.rs) |
| Dependency setup | [installation](../../../docs/reference/supported-configurations.md#installation) |

## gf2-algebra

| Fact | Stated at |
|---|---|
| The crate denies `unsafe` code; SIMD and GPU kernels live in `gf2-kernels-simd` and `gf2-kernels-hip` | [crate layering](../../../docs/concepts/acceleration-architecture.md#crate-layering); Rustdoc [`lib.rs`](../../../crates/gf2-algebra/src/lib.rs) |
| Ryser's formula in Gray-code order over the bipedal encoding of `@/citation/Scheinerman2024` | Rustdoc [`lib.rs`](../../../crates/gf2-algebra/src/lib.rs); [batched $\mathbb{F}_3$ permanent kernel](../../../docs/reference/performance-evidence.md#f3-permanent-batched-avx2) |
| Permanental rank deficiency of rectangular matrices in `permanent::rank` | Rustdoc [`lib.rs`](../../../crates/gf2-algebra/src/lib.rs), [`permanent/mod.rs`](../../../crates/gf2-algebra/src/permanent/mod.rs) |
| Exact permanent-zero and determinant-singularity probabilities in `permanent::exact` | Rustdoc [`permanent/exact.rs`](../../../crates/gf2-algebra/src/permanent/exact.rs) |
| Lane counts of the packed $\mathbb{F}_3$, $\mathbb{F}_5$ and $\mathbb{F}_7$ types | [choosing a field type](../../../docs/concepts/finite-field-arithmetic.md#choosing-a-field-type) |
| `packed` holds lane-parallel vectors and matrices | [choosing a field type](../../../docs/concepts/finite-field-arithmetic.md#choosing-a-field-type); Rustdoc [`packed/mod.rs`](../../../crates/gf2-algebra/src/packed/mod.rs) |
| Permanents over any `FiniteField` serve as the correctness oracle; the $\mathbb{F}_3$ kernel extends to multi-word matrices | Rustdoc [`lib.rs`](../../../crates/gf2-algebra/src/lib.rs) |
| Scalar code is always available; `simd` adds AVX2 kernels for $\mathbb{F}_3$ with scalar fallback, `parallel` adds Rayon evaluation, `hip` adds batched GPU evaluation in `gpu` | comments of the `[features]` table in [`Cargo.toml`](../../../crates/gf2-algebra/Cargo.toml); Rustdoc [`lib.rs`](../../../crates/gf2-algebra/src/lib.rs), [`gpu.rs`](../../../crates/gf2-algebra/src/gpu.rs); [limitations](../../../docs/reference/supported-configurations.md#limitations) |
| Default features | [Cargo features](../../../docs/reference/supported-configurations.md#cargo-features) |
| Effect of `f5`, `f7`, `serde`, `tuning-profile` and `test-support`; `default-features = false` selects the scalar, $\mathbb{F}_3$-only build | comments of the `[features]` table in [`Cargo.toml`](../../../crates/gf2-algebra/Cargo.toml) |
| Test command | [supported toolchain and commands](../../../AGENTS.md#supported-toolchain-and-commands) |
| Dependency setup | [installation](../../../docs/reference/supported-configurations.md#installation) |

## Statements without a supporting source

These README statements disagree with the code and have no owning page.

| Crate | Statement | Code |
|---|---|---|
| `gf2-core` | `io` provides Serde serialization of bit containers | `BitVec`, `BitMatrix` and the sparse types carry no Serde derive; [`io/mod.rs`](../../../crates/gf2-core/src/io/mod.rs) defines a binary file format with JSON metadata |
| `gf2-algebra` | The $\mathbb{F}_7$ permanent runs over a bit-plane encoding | `Packed7` stores each element in a 4-bit slot, per [`packed/packed7.rs`](../../../crates/gf2-algebra/src/packed/packed7.rs) |
| `gf2-coding` | Every DVB-T2 vector test returns early when the streams are absent | [test vectors](../../../docs/reference/standards-conformance.md#test-vectors) states which tests pass without assertion and which are ignored |
