# gf2-coding

Error-correcting codes and coding-theory primitives built on [`gf2-core`](../gf2-core/): Hamming and BCH algebraic codes, LDPC with belief propagation (DVB-T2 and 5G NR base graphs), convolutional/Viterbi, product codes and generalized LDPC with Chase–Pyndiah, GRAND-family decoders (ORBGRAND, SO-GRAND), a batch-oriented modem framework with Gray-QAM soft demapping, and AWGN / Rician fading channel models tied together by a simulation harness.

## What's here

### Block codes

| Family | Module | Parameters | Notes |
|---|---|---|---|
| Hamming | `linear` | (2^r − 1, 2^r − r − 1) | Syndrome-table decoder |
| BCH | `bch` | (n, k, t) over GF(2^m) | Berlekamp–Massey + Chien; extended BCH; DVB-T2 profiles validated against ETSI EN 302 755 (202/202) |
| LDPC | `ldpc` | quasi-cyclic (n, k) | Belief propagation; DVB-T2 (all 12 rates, 202/202) and 5G NR (BG1/BG2 with per-i_LS shift tables); Richardson–Urbanke encoding with file cache |
| Product | `product` | N₁ × N₂ | Row/column iteration |
| Generalized LDPC | `gldpc` | — | Chase–Pyndiah product decoder |

### Streaming and soft decoders

- `convolutional` — convolutional encoder, Viterbi decoder (NASA/CCSDS generator polynomials)
- `bcjr` — batch BCJR soft-input/soft-output decoder (CPU; HIP GPU path via `gf2-kernels-hip`)
- `grand` — `ORBGRAND` and `SO-GRAND` universal noise-centric decoders
- `llr` — `Llr` type (`f32` by default, `f64` with `llr-f64`) and min-sum / box-plus operations
- `drm` — Doubled Reed–Muller with polar transform

### Modem, channel, simulation

- `modem` — BPSK, Gray-coded QPSK / 16-QAM / 64-QAM / 256-QAM presets, plus a validated builder for arbitrary custom constellations; reference (exact log-MAP) and optimized (Gray-QAM fast) backends selected through `ModemSpec::preferred_*` factories; optional GPU demap
- `channel` — AWGN with BPSK modulation for quick BER sweeps
- `fading` — Rician channel models integrated with the modem framework (`QpskRicianChannelModel`)
- `simulation` — BER/FER harness with batched encode/transmit/decode
- `info_theory`, `crc` — capacity/mutual information helpers, CRC polynomials

## Install

```toml
[dependencies]
gf2-core   = { path = "../gf2-core" }
gf2-coding = { path = "../gf2-coding" }  # simd on by default

# Enable extras
# gf2-coding = { path = "...", features = ["parallel", "llr-f64", "hip"] }
```

## Getting started

### Hamming(7,4)

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

let code    = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

let msg = BitVec::from_bytes_le(&[0b1010]);
let mut cw = code.encode(&msg);
cw.set(2, !cw.get(2));                   // inject a single-bit error
assert_eq!(decoder.decode(&cw), msg);
```

### DVB-T2 LDPC

```rust
use gf2_coding::{CodeRate, ldpc::LdpcCode};
use gf2_core::BitVec;

let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
assert_eq!((code.k(), code.n()), (32_400, 64_800));

let zero_cw = BitVec::zeros(code.n());
assert!(code.is_valid_codeword(&zero_cw));
```

See `examples/ldpc_awgn.rs` for the full BPSK/AWGN → LLR → belief-propagation pipeline.

### 5G NR LDPC

The `ldpc::nr_5g` submodule carries BG1 and BG2 base graphs with per-i_LS shift tables. Select the lifting factor and base graph, then use the shared `LdpcCode` API for encode/decode. A single shift table across lifting sets costs ~2 dB BLER, so the per-i_LS indirection matters.

### Gray-QAM modem

```rust
use gf2_coding::modem::ModemSpec;

let modem = ModemSpec::gray_qam_16().preferred_fast();  // Gray-QAM fast backend
// see examples/modem_gray_qam_preset.rs and modem_simulation_harness.rs
```

Custom constellations go through the validated builder in `examples/modem_custom_constellation.rs`.

### GRAND

```rust
use gf2_coding::grand::OrbGrandDecoder;
// see examples/sogrand_crc_probe.rs for a CRC-aided soft-GRAND setup
```

## Acceleration

- **SIMD** (default): bit-level and RREF-stage operations go through AVX2 / AVX-512 via `gf2-core`'s SIMD layer. Word-level (64×) × SIMD (4–8×) ≈ 256–512× over naïve Gaussian elimination for LDPC preprocessing.
- **Parallel** (opt-in, `--features parallel`): Rayon-backed batch encode/decode across frames.

  ```bash
  RAYON_NUM_THREADS=8 cargo bench -p gf2-coding --bench quick_parallel --features parallel
  ```

- **GPU** (opt-in, `--features hip`): HIP/ROCm kernels on gfx1030 accelerate batched BCJR soft decoding, Gray-QAM demapping, LDPC belief propagation, and BCH syndrome evaluation (`BchDecoder::compute_syndromes_batch_gpu` / `decode_batch_gpu`: GPU Horner over GF(2^m), CPU Berlekamp-Massey + Chien). Requires `hipcc` and an AMD GPU; see [`../gf2-kernels-hip/`](../gf2-kernels-hip/). The HIP crate is excluded from the default workspace build.

See [`docs/SIMD_PERFORMANCE_GUIDE.md`](docs/SIMD_PERFORMANCE_GUIDE.md), [`docs/PARALLELIZATION.md`](docs/PARALLELIZATION.md), and [`docs/LDPC_PERFORMANCE.md`](docs/LDPC_PERFORMANCE.md) for benchmarks and methodology.

## Features

| Feature | Default | Effect |
|---|---|---|
| `simd` | ✅ | Propagates to `gf2-core/simd` (AVX2 / AVX-512) |
| `parallel` | — | Rayon batch encode/decode |
| `visualization` | — | Propagates to `gf2-core/visualization` (matrix PNG export) |
| `llr-f64` | — | Use `f64` LLRs instead of `f32` (for research / reference runs) |
| `hip` | — | Enable `gf2-kernels-hip` GPU kernels (requires ROCm/hipcc) |

## Utility binaries

```bash
cargo run --release -p gf2-coding --bin generate_ldpc_cache all
cargo run --release -p gf2-coding --bin validate_ldpc_cache
cargo run           -p gf2-coding --bin check_encoding
```

`generate_ldpc_cache` writes ~530 MB of generator-matrix caches (a one-time ~13 min preprocessing); cached encoders then load in <16 ms.

## Examples

Run with `cargo run --release -p gf2-coding --example <name>`:

| Area | Examples |
|---|---|
| Block / Hamming | `hamming_basic`, `hamming_7_4`, `block_code_intro`, `generator_from_parity_check` |
| DVB-T2 | `dvb_t2_ldpc_basic`, `dvb_t2_bch_demo` |
| LDPC | `ldpc_awgn`, `ldpc_bler_check`, `ldpc_mother_check`, `ldpc_cache_file_io`, `ldpc_encoding_with_cache`, `qc_ldpc_demo` |
| Convolutional | `nasa_rate_half_k3` |
| Modem / fading | `modem_gray_qam_preset`, `modem_custom_constellation`, `modem_simulation_harness` |
| Soft / GRAND | `llr_operations`, `sogrand_crc_probe` |
| Channel / utilities | `awgn_uncoded`, `visualize_large_matrices`, `gen_presentation_figures` |

## Testing

```bash
cargo test  -p gf2-coding --release
cargo test  -p gf2-coding --release --doc
cargo bench -p gf2-coding --bench ldpc_decode
```

Always use `--release`: debug mode is 10–100× slower on LDPC and simulation code, and the suite has a 60-second wall-clock budget.

## Documentation

- [`docs/DVB_T2.md`](docs/DVB_T2.md) — DVB-T2 implementation and reference-vector verification
- [`docs/SIMD_PERFORMANCE_GUIDE.md`](docs/SIMD_PERFORMANCE_GUIDE.md) — SIMD routing and measured speedups
- [`docs/PARALLELIZATION.md`](docs/PARALLELIZATION.md) — Rayon batch strategy
- [`docs/LDPC_PERFORMANCE.md`](docs/LDPC_PERFORMANCE.md), [`docs/LDPC_VERIFICATION_TESTS.md`](docs/LDPC_VERIFICATION_TESTS.md)
- [`docs/SDR_INTEGRATION.md`](docs/SDR_INTEGRATION.md) — using the modem from an SDR stack
- [`docs/SYSTEMATIC_ENCODING_CONVENTION.md`](docs/SYSTEMATIC_ENCODING_CONVENTION.md) — bit-order and systematic form conventions
- `src/modem/mod.rs` — module-level modem-framework guide
- Workspace overview: [`../../README.md`](../../README.md)

## Contributing

Follow TDD. Add property tests for algebraic invariants, standards test vectors where the code claims standards compliance, and benchmarks for anything performance-sensitive. See the workspace guide in [`../../CLAUDE.md`](../../CLAUDE.md) and [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).

## License

MIT — see [`../../LICENSE-MIT`](../../LICENSE-MIT).
