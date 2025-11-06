# gf2-coding

`gf2-coding` hosts research and application-oriented components that build on `gf2-core`. It contains linear block codes, early convolutional code scaffolding, and shared traits for experimentation with error-correcting codes and compression.

## Highlights

- Linear block code implementation with generator/parity matrices
- Syndrome table decoder for classic Hamming codes
- Traits for block and streaming encoders/decoders
- Examples such as the Hamming (7,4) workflow (`cargo run -p gf2-coding --example hamming_7_4`)

## Usage

Add both crates when consuming the coding components:

```toml
[dependencies]
gf2-core = "0.1"
gf2-coding = "0.1"
```

Example:

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

let code = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

let mut msg = BitVec::from_bytes_le(&[0b1011]);
msg.resize(code.k(), false);
let codeword = code.encode(&msg);
let decoded = decoder.decode(&codeword);
assert_eq!(decoded, msg);
```

Refer to the workspace [README](../../README.md) for testing, benchmarking, and roadmap details.
