# Receipt: external BCH oracle fixture generation

Written by [`oracle/run.sh`](oracle/run.sh) from the run it describes.
Every value below was observed during that run.

| Field | Value |
|---|---|
| Issue | `3f7edef1` |
| Run start (UTC) | 2026-09-02T06:55:24Z |
| Run end (UTC) | 2026-09-02T06:55:40Z |
| gf2 revision | `f0685f7587a171116cb24177ca79c0cd674264d7` |
| Working tree at generation | clean |
| Host CPU | AMD Ryzen 9 5900X 12-Core Processor |
| Kernel | Linux 7.1.11-arch1-1 x86_64 GNU/Linux |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |
| SageMath | SageMath version 10.9, Release Date: 2026-05-04 |
| SageMath interpreter | CPython 3.14.7 |
| GAP | 4.16dev |
| GUAVA | 3.21 |
| SONATA | 2.9.8 |

## Stages

| Stage | Exact invocation | Wall clock (s) | Output |
|---|---|---|---|
| build | `./scripts/cargo-budget.sh cargo build --release -p gf2-coding --features test-support --example bch_oracle_messages` | | |
| corpus | `./target/release/examples/bch_oracle_messages crates/gf2-coding/tests/data/bch_oracle/corpus.json` | 0 | [corpus.json](../../../crates/gf2-coding/tests/data/bch_oracle/corpus.json) |
| SageMath | `python3 dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py crates/gf2-coding/tests/data/bch_oracle/corpus.json crates/gf2-coding/tests/data/bch_oracle/sage.json` | 3 | [sage.json](../../../crates/gf2-coding/tests/data/bch_oracle/sage.json) |
| GAP with GUAVA | `gap -q -A -o 4g -c 'CORPUS:="crates/gf2-coding/tests/data/bch_oracle/corpus.json"; OUTPUT:="crates/gf2-coding/tests/data/bch_oracle/gap.json";' dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | 2 | [gap.json](../../../crates/gf2-coding/tests/data/bch_oracle/gap.json) |

## Fixture hashes

| File | SHA-256 | Bytes |
|---|---|---|
| `crates/gf2-coding/tests/data/bch_oracle/corpus.json` | `02d195b78fe82c8cc784c79b1adf5f7b02e381d5e43de8aa7266b4a7c7f3db05` | 40729 |
| `crates/gf2-coding/tests/data/bch_oracle/sage.json` | `a50ab94e03456f336ba51741d00faa5e9f4308b60fc3d64c231b02d563937b01` | 89261 |
| `crates/gf2-coding/tests/data/bch_oracle/gap.json` | `e646b29a74598d871df28f22b5dd34b9406e3505da9a4033c088e71ca0e298f1` | 81718 |

## Generating source

| File | SHA-256 |
|---|---|
| `crates/gf2-coding/examples/bch_oracle_messages.rs` | `79469f78c2d921bc3e92d0a77c74aeb095bbf97bbfe82024ef069fa985c9148c` |
| `crates/gf2-coding/src/test_support.rs` | `aa6b32cf41bcac1413c367703ff4aa11ebd9eb4b633e140c6e7f9d3c8891c25f` |
| `dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py` | `809364de1e14fe43acda695be6a631e63529bedb4bba10e583a53cdabab9ad88` |
| `dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | `5e28bc9e4d3775eaa7b389e537bddb129581705f588611ac01e02cff939820a6` |
| `dev/active/ae03bcd0-general-bch/oracle/run.sh` | `02a131717235860650c2e010764f994278210d954a0069ea55d70986f4eeb5df` |
