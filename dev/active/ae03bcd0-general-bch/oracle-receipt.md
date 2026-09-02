# Receipt: external BCH oracle fixture generation

Written by [`oracle/run.sh`](oracle/run.sh) from the run it describes.
Every value below was observed during that run.

| Field | Value |
|---|---|
| Issue | `3f7edef1` |
| Run start (UTC) | 2026-09-02T16:25:42Z |
| Run end (UTC) | 2026-09-02T16:26:50Z |
| gf2 revision | `17111e89b09c299959129c944034d311e1d8cbc6` |
| Working tree at generation | modified |
| Host CPU | AMD Ryzen 9 5900X 12-Core Processor |
| Kernel | Linux 7.1.11-arch1-1 x86_64 GNU/Linux |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |

## Oracle identity

| Property | Value |
|---|---|
| SageMath | SageMath version 10.9, Release Date: 2026-05-04 |
| SageMath library version | 10.9 |
| SageMath interpreter | CPython 3.14.7 |
| SageMath interpreter executable | `/usr/bin/python3.14` |
| SageMath interpreter SHA-256 | `d78f9cf7178ecff09963551399855543c297f37ac207e626228bfe43cb26a70c` |
| GAP version label | 4.16dev |
| GAP kernel version | 4.16dev |
| GAP build version | 4.16.1-dirty |
| GAP build datetime | reproducible |
| GAP architecture | x86_64-pc-linux-gnu-default64-kv11 |
| GMP | 6.3.0 |
| GAP executable | `/usr/bin/gap` |
| GAP executable SHA-256 | `bbcaafb8f27537023821a786c18cb8a8d9c685d38e0607d2fd590b0af279375f` |
| GAP heap | 4g |
| GUAVA | 3.21 |
| GUAVA PackageInfo.g SHA-256 | `c27b52f877b559d2f985928a535dcb3f218ae9af4439e3bb7404bece42f21544` |
| SONATA | 2.9.8 |
| SONATA PackageInfo.g SHA-256 | `a995860a20df497891238b41112c1bc191939eb26eddd337a4b4e28b1160b64b` |

## Stages

| Stage | Exact invocation | Wall clock (s) | Output |
|---|---|---|---|
| build | `./scripts/cargo-budget.sh cargo build --release -p gf2-coding --features test-support --example bch_oracle_messages` | 0 | `./target/release/examples/bch_oracle_messages` |
| corpus | `./target/release/examples/bch_oracle_messages crates/gf2-coding/tests/data/bch_oracle/corpus.json` | 0 | [corpus.json](../../../crates/gf2-coding/tests/data/bch_oracle/corpus.json) |
| SageMath | `python3 dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py crates/gf2-coding/tests/data/bch_oracle/corpus.json crates/gf2-coding/tests/data/bch_oracle/sage.json` | 2 | [sage.json](../../../crates/gf2-coding/tests/data/bch_oracle/sage.json) |
| GAP with GUAVA | `gap -q -A -T -o 4g -c 'CORPUS:="crates/gf2-coding/tests/data/bch_oracle/corpus.json"; OUTPUT:="crates/gf2-coding/tests/data/bch_oracle/gap.json";' dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | 66 | [gap.json](../../../crates/gf2-coding/tests/data/bch_oracle/gap.json) |

## GUAVA code-object attempts

Each row calls `BCHCode(n, b, delta, F)` for GUAVA's own code object
and `GeneratorPolCode(G, n, F)` for the derivation at gf2's root, both
under the heap above; the oracle catches an attempt that exceeds it and
records what it observed. The peak resident set is the kernel's
high-water mark for the whole process, which is monotone over the run,
so it bounds an attempt from above and a row's own cost shows as its
rise over the row before it.

| Row | Code object built | `BCHCode` attempt CPU (s) | Process peak RSS after this row (KiB) |
|---|---|---|---|
| B1 | yes | 0.0 | 151788 |
| B2 | yes | 0.0 | 151788 |
| B3 | yes | 0.0 | 151788 |
| B4 | no | 22.9 | 4174060 |
| N1 | yes | 0.0 | 7610604 |
| N2 | yes | 0.0 | 7612652 |
| N3 | yes | 0.0 | 7614700 |
| N4 | yes | 0.0 | 7622892 |

Diagnostics GAP printed during the stage, one message per attempt that
exceeded the heap:

```
Error, reached the pre-set memory limit
(change it with the -o command line option)
Error, reached the pre-set memory limit
(change it with the -o command line option)
```

## Fixture hashes

| File | SHA-256 | Bytes |
|---|---|---|
| `crates/gf2-coding/tests/data/bch_oracle/corpus.json` | `02d195b78fe82c8cc784c79b1adf5f7b02e381d5e43de8aa7266b4a7c7f3db05` | 40729 |
| `crates/gf2-coding/tests/data/bch_oracle/sage.json` | `4861230ac64ab7d5a3fb8f696925ecb9db0decc95c1d31529c5570022d717f52` | 89342 |
| `crates/gf2-coding/tests/data/bch_oracle/gap.json` | `a36df8d3261bc675cc6afe9a189138395d46c20951b143f0cc22600e1566e7cd` | 90999 |

## Generating source

| File | SHA-256 |
|---|---|
| `crates/gf2-coding/examples/bch_oracle_messages.rs` | `79469f78c2d921bc3e92d0a77c74aeb095bbf97bbfe82024ef069fa985c9148c` |
| `crates/gf2-coding/src/test_support.rs` | `33c6baca50c2008353362c0586e950a720d004861f16df91249123eb2adb10c7` |
| `dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py` | `3dd7d7ef3addb690d8644ed2a6ffb2ddd3c37bef29925d2716b681be4a8bab5a` |
| `dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | `4f8f05e5b07597ad0c375075dff2c2bcb37d0ac4d17c35f1052ffcb10db30899` |
| `dev/active/ae03bcd0-general-bch/oracle/run.sh` | `08aa8b0722678cee1714b3ad63ae749cf49d0fc2e838c8413e21b4edc96a6bda` |
