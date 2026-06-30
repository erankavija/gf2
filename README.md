# gf2

[![CI](https://github.com/erankavija/gf2/workflows/CI/badge.svg)](https://github.com/erankavija/gf2/actions)
[![gf2-core coverage](https://raw.githubusercontent.com/erankavija/gf2/badges/gf2-core.svg)](https://github.com/erankavija/gf2/actions)
[![gf2-coding coverage](https://raw.githubusercontent.com/erankavija/gf2/badges/gf2-coding.svg)](https://github.com/erankavija/gf2/actions)
[![gf2-algebra coverage](https://raw.githubusercontent.com/erankavija/gf2/badges/gf2-algebra.svg)](https://github.com/erankavija/gf2/actions)
[![gf2-sim coverage](https://raw.githubusercontent.com/erankavija/gf2/badges/gf2-sim.svg)](https://github.com/erankavija/gf2/actions)
[![workspace coverage](https://raw.githubusercontent.com/erankavija/gf2/badges/workspace.svg)](https://github.com/erankavija/gf2/actions)

A research-grade Rust toolkit for finite field computing and modern coding theory — binary fields, prime fields, tower extensions, and error-correcting codes with SIMD/GPU kernels and machine-checked proofs. Built to explore algorithms that compete with specialized CAS (Magma, Sage) and production PHY stacks (DVB-T2, 5G NR), behind clean, composable Rust APIs.

## Highlights

- **Finite fields**: dense GF(2) linear algebra, GF(2^m) with Karatsuba/Barrett/table strategies, GF(p) via Montgomery (plus specialized Mersenne/Proth backends), and GF(p^n) quadratic/cubic tower extensions.
- **Codes**: Hamming, BCH, LDPC (belief-propagation, quasi-cyclic), convolutional/Viterbi, product codes, generalized LDPC with Chase–Pyndiah, and GRAND family (ORBGRAND, SO-GRAND).
- **Standards**: DVB-T2 LDPC + BCH validated against ETSI EN 302 755 test vectors (202/202); 5G NR LDPC BG1/BG2 base graphs with per-i_LS shift tables.
- **Modulation & channel**: BPSK + Gray-QAM (QPSK/16/64/256) with soft demapping, AWGN, Rician fading, BCJR batch decoder.
- **Acceleration**: AVX2/AVX-512 CPU kernels (runtime-dispatched) and optional HIP/ROCm GPU kernels (gfx1030) for batched BCJR, Gray-QAM demap, LDPC belief propagation, and BCH syndrome evaluation.
- **Formal verification**: Lean4 proofs of prime-field Montgomery arithmetic and bipedal F_3 arithmetic (add/sub/mul/neg) extracted from the live Rust source via Charon/Aeneas.

Active work is tracked in-repo with [jit](https://github.com/erankavija/just-in-time) under `.jit/` — run `jit status` or browse issues there for the current backlog and in-progress items.

## Workspace layout

| Crate | Purpose |
|---|---|
| [`gf2-core`](crates/gf2-core/) | Bit primitives, dense/sparse linear algebra over GF(2), GF(2^m), GF(p), and tower extensions GF(p^n). No unsafe. |
| [`gf2-coding`](crates/gf2-coding/) | Block codes, streaming codes, GRAND decoders, modem framework, channel models, simulation harness. |
| [`gf2-algebra`](crates/gf2-algebra/) | Packed F_3 / F_5 / F_7 element types and fast matrix permanents (bipedal F_3, packed F_5 / F_7) on CPU (scalar, AVX2, rayon) and HIP/ROCm GPU. |
| [`gf2-kernels-simd`](crates/gf2-kernels-simd/) | Isolated unsafe CPU kernels (AVX2, AVX-512, aarch64). |
| [`gf2-kernels-hip`](crates/gf2-kernels-hip/) | Isolated unsafe HIP/ROCm GPU kernels. Excluded from the default workspace; opt in with `--features hip` on `gf2-coding` (BCJR / Gray-QAM demap / LDPC BP / BCH syndrome eval) or `gf2-algebra` (batch permanents). |
| [`proofs/`](proofs/) | Lean4 formal-verification package for `gfp/`, `gfpn/`, and `gf2-algebra::packed::bipedal3`. |

All `unsafe` code is confined to the two kernel crates; everything else is `#![deny(unsafe_code)]`.

## Install

```toml
[dependencies]
gf2-core   = { path = "crates/gf2-core" }
gf2-coding = { path = "crates/gf2-coding" }

# Optional: SIMD (default on for gf2-coding), parallel, LLR in f64, HIP GPU
# gf2-coding = { path = "...", features = ["parallel", "llr-f64", "hip"] }
```

The crates are not yet published to crates.io.

## Tour by task

### Bit algebra & GF(2) linear algebra

```rust
use gf2_core::{BitVec, BitMatrix};

let mut a = BitVec::from_bytes_le(&[0b1010_1010]);
a.bit_xor_into(&BitVec::from_bytes_le(&[0b1100_1100]));
assert_eq!(a.count_ones(), 4);

let m = BitMatrix::identity(128);
let p = &m * &m;                     // M4RM matrix multiply
assert_eq!(p, m);
```

### GF(2^m) and GF(p^n) arithmetic

```rust
use gf2_core::gf2m::Gf2mField;

let f = Gf2mField::new(8, 0b1_0001_1101).with_tables();   // AES polynomial
let x = f.element(0x53);
let y = f.element(0xCA);
let _ = &x * &y;                     // O(1) via log/antilog
```

Prime fields (`Fp<P>`), quadratic and cubic extensions live under `gf2_core::gfp` and `gf2_core::gfpn`; both are the subject of machine-checked Lean proofs (see [proofs/](proofs/)).

### Error correction (Hamming)

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

let code    = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

let msg = BitVec::from_bytes_le(&[0b1011]);
let mut cw = code.encode(&msg);
cw.set(2, !cw.get(2));                // flip one bit
assert_eq!(decoder.decode(&cw), msg); // corrected
```

### DVB-T2 LDPC + BCH + BPSK/AWGN

```rust
use gf2_coding::{CodeRate, ldpc::LdpcCode, simulation::BpskAwgnChannel};

let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
assert_eq!((code.k(), code.n()), (32_400, 64_800));
let _channel = BpskAwgnChannel;       // full pipeline: see examples/ldpc_awgn.rs
```

See [crates/gf2-coding/README.md](crates/gf2-coding/README.md) for the full menu (5G NR LDPC, GRAND, product codes, Chase–Pyndiah, Gray-QAM modem, Rician fading, BCJR).

## Features

| Crate | Feature | Default | Effect |
|---|---|---|---|
| `gf2-core` | `rand` | ✅ | Random bit/matrix/field generators |
| `gf2-core` | `io` | ✅ | Serde (de)serialization |
| `gf2-core` | `simd` | — | Routes to `gf2-kernels-simd` (AVX2/AVX-512) |
| `gf2-core` | `parallel` | — | Rayon batch operations |
| `gf2-core` | `visualization` | — | PNG export of matrices |
| `gf2-coding` | `simd` | ✅ | Propagates to `gf2-core/simd` |
| `gf2-coding` | `parallel` | — | Rayon batch encode/decode |
| `gf2-coding` | `llr-f64` | — | f64 LLRs (default f32) |
| `gf2-coding` | `hip` | — | HIP/ROCm GPU kernels (BCJR, Gray-QAM demap, LDPC BP, BCH syndrome eval; requires hipcc) |
| `gf2-algebra` | `simd` | ✅ | AVX2 bipedal3 path (default on) |
| `gf2-algebra` | `parallel` | ✅ | Rayon batch permanents (default on) |
| `gf2-algebra` | `f5` | ✅ | F_5 packed types + permanent (default on) |
| `gf2-algebra` | `f7` | ✅ | F_7 packed types + permanent (default on) |
| `gf2-algebra` | `hip` | — | HIP/ROCm GPU batch permanent dispatcher (requires hipcc) |

Runtime SIMD dispatch: the first call probes CPU features via `OnceLock` and binds the best available backend for the process's lifetime.

## Developing

```bash
# build everything (default workspace, HIP excluded)
cargo build --workspace --all-features

# test — ALWAYS --release (debug is 10–100× slower on SIMD / simulation code)
cargo test --workspace --all-features --release

# format + lint gates (CI enforces -D warnings and fmt --check)
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# focused benches
cargo bench -p gf2-core --bench fp_montgomery
cargo bench -p gf2-coding --bench ldpc_decode

# docs
cargo doc --no-deps --open

# HIP GPU kernels (requires ROCm + hipcc, builds independently of workspace)
cargo build --manifest-path crates/gf2-kernels-hip/Cargo.toml

# Lean4 verification — needs elan only
cd proofs && lake build
# Full regeneration from Rust source — needs patched Charon + Aeneas
./scripts/verify-lean.sh
```

Test-suite wall-clock budget is 60 seconds. If it takes longer, something is wrong.

## Examples

`cargo run --release -p gf2-coding --example <name>`:

- **Block codes**: `hamming_basic`, `hamming_7_4`, `block_code_intro`, `generator_from_parity_check`
- **DVB-T2**: `dvb_t2_ldpc_basic`, `dvb_t2_bch_demo`
- **LDPC**: `ldpc_awgn`, `ldpc_bler_check`, `ldpc_mother_check`, `ldpc_cache_file_io`, `ldpc_encoding_with_cache`, `qc_ldpc_demo`
- **Convolutional / Viterbi**: `nasa_rate_half_k3`
- **Modem + fading**: `modem_gray_qam_preset`, `modem_custom_constellation`, `modem_simulation_harness`
- **Soft decoding / GRAND**: `llr_operations`, `sogrand_crc_probe`
- **Channel**: `awgn_uncoded`
- **Utilities**: `visualize_large_matrices`, `gen_presentation_figures`

`cargo run -p gf2-core --example <name>`: `bitvec_basics`, `matrix_basics`, `sparse_display`, `random_generation`, `primitive_polynomial_verification`, `visualize_matrix`.

## Formal verification

`proofs/` contains a self-contained Lean4 package that proves correctness of the Rust implementations of `Fp<P>` (Montgomery), the quadratic/cubic tower extensions, and the `gf2-algebra` bipedal F_3 arithmetic. Lean sources are auto-generated from the live Rust via a Charon/Aeneas pipeline (`scripts/verify-lean.sh`), committed to the repo, and backed by hand-written proofs under `proofs/Gf2Core/Proofs/` and `proofs/Gf2Algebra/Proofs/`. Headline theorems include Montgomery roundtrip, REDC correctness, `CommRing`/`Field` instances via equivalence with `ZMod P.val`, and all four bipedal F_3 operations (add/sub/mul/neg) correct against their `Fp<3>` reference semantics. See [proofs/README.md](proofs/README.md) for the full pipeline and prerequisites.

## Design notes

- **Tail-masking invariant** — `BitVec` guarantees that padding bits beyond `len_bits` in the final `u64` word are zero. Every mutating operation calls `mask_tail()`. This is the single most critical correctness invariant.
- **Functional at the API boundary, imperative inside kernels** — high-level code prefers pure functions and iterator combinators; `kernels/` and `compute/` use mutation and loops for speed.
- **Unsafe isolation** — the only crates allowed to use `unsafe` are `gf2-kernels-simd` and `gf2-kernels-hip`. Runtime SIMD dispatch goes through `simd::maybe_simd()`.
- **MSRV**: Rust 1.95.

## Documentation

- Crate guides: [gf2-core](crates/gf2-core/README.md), [gf2-coding](crates/gf2-coding/README.md), [proofs/](proofs/README.md)
- Deep dives under `crates/*/docs/` (benchmarks, kernel optimization, DVB-T2, SIMD, parallelization, SDR integration, systematic encoding)
- Full API docs: `cargo doc --no-deps --open`
- Strategic roadmap: [ROADMAP.md](ROADMAP.md) (subproject roadmaps under each crate)

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) and the workspace guide in [CLAUDE.md](CLAUDE.md). TDD is expected: write the test first, implement minimally, add property tests for mathematical invariants, and cover word-boundary edge cases (0, 1, 63, 64, 65 bits). Public APIs need doc examples that compile under `cargo test --doc`.

Good first areas: SIMD kernels (NEON, AVX-512 extensions), new code families, soft-decoding algorithms, Lean proof polish, and fuzzing harnesses.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
