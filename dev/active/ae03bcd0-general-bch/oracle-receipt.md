# Receipt: external BCH oracle fixture generation

Written by [`oracle/run.sh`](oracle/run.sh) from the run it describes.
Every value below was observed during that run.

| Field | Value |
|---|---|
| Issue | `3f7edef1` |
| Run start (UTC) | 2026-09-02T17:20:59Z |
| Run end (UTC) | 2026-09-02T17:22:09Z |
| gf2 revision | `6896a4040a141410383db4f476d90f51a2d88339` |
| Working tree at generation | clean |
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
| build | `./scripts/cargo-budget.sh cargo build --release -p gf2-coding --features test-support --example bch_oracle_messages` | 1 | `./target/release/examples/bch_oracle_messages` |
| corpus | `./target/release/examples/bch_oracle_messages crates/gf2-coding/tests/data/bch_oracle/corpus.json` | 0 | [corpus.json](../../../crates/gf2-coding/tests/data/bch_oracle/corpus.json) |
| SageMath | `python3 dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py crates/gf2-coding/tests/data/bch_oracle/corpus.json crates/gf2-coding/tests/data/bch_oracle/sage.json` | 1 | [sage.json](../../../crates/gf2-coding/tests/data/bch_oracle/sage.json) |
| GAP with GUAVA | `gap -q -A -T -o 4g -c 'CORPUS:="crates/gf2-coding/tests/data/bch_oracle/corpus.json"; OUTPUT:="crates/gf2-coding/tests/data/bch_oracle/gap.json";' dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | 66 | [gap.json](../../../crates/gf2-coding/tests/data/bch_oracle/gap.json) |

## GUAVA code-object attempts

Each row calls `BCHCode(n, b, delta, F)` for GUAVA's own code object
and `GeneratorPolCode(G, n, F)` for the derivation at gf2's root, both
under the heap above; the oracle catches an attempt that exceeds it and
records what it observed. The two marks are samples of the kernel's
high-water mark for the whole process, taken after the `BCHCode`
attempt and after the `GeneratorPolCode` attempt that follows it. That
mark is monotone over the run, so a sample says the attempts before it
did not exceed it, and a rise from one row to the next says the later
row's attempts cost at least that much.

| Row | Code object built | `BCHCode` attempt CPU (s) | Mark after the `BCHCode` attempt (KiB) | Mark after both attempts (KiB) |
|---|---|---|---|---|
| B1 | yes | 0.0 | 151820 | 151820 |
| B2 | yes | 0.0 | 151820 | 151820 |
| B3 | yes | 0.0 | 151820 | 151820 |
| B4 | no | 22.4 | 4186380 | 7604492 |
| N1 | yes | 0.0 | 7633164 | 7633164 |
| N2 | yes | 0.0 | 7635212 | 7635212 |
| N3 | yes | 0.0 | 7637260 | 7637260 |
| N4 | yes | 0.0 | 7645452 | 7649548 |

Diagnostics GAP printed during the stage, one message per attempt that
exceeded the heap:

```
Error, reached the pre-set memory limit
(change it with the -o command line option)
Error, reached the pre-set memory limit
(change it with the -o command line option)
```

## Standards vectors

The case
`the_etsi_dvb_t2_streams_encode_to_their_verified_codewords`
encodes every block of the ETSI DVB-T2 verification streams through the
canonical mother code and compares it with the verified codeword. It
prints what it read and how far the agreement went, and those lines are
quoted below as it printed them.

| Property | Value |
|---|---|
| Exact invocation | `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --test bch_oracle_agreement --cargo-profile ci-test --profile ci -E 'test(the_etsi_dvb_t2_streams_encode_to_their_verified_codewords)' --no-capture` |
| Wall clock (s) | 2 |
| nextest summary | `Summary [   1.846s] 1 test run: 1 passed, 31 skipped` |

```
dvb-vectors: resolved_directory=/home/vkaskivuo/dvb_test_vectors
dvb-vectors: stream_set=VV001-CR35
dvb-vectors: tp04=VV001-CR35_CSP/TestPoint04/VV001-CR35_TP04_CSP.txt sha256=c658dc04cacebe24a86a42a89f8ffe588f1e269506b6da05eae7d6582d8570b8
dvb-vectors: tp05=VV001-CR35_CSP/TestPoint05/VV001-CR35_TP05_CSP.txt sha256=f4aaf105b01768b1269d21accef923de09d0040cba073a73909d8f814f3ed929
dvb-vectors: frames=4 blocks_per_frame=202
dvb-vectors: blocks_compared=808 blocks_agreeing=808
```

## Fixture hashes

| File | SHA-256 | Bytes |
|---|---|---|
| `crates/gf2-coding/tests/data/bch_oracle/corpus.json` | `02d195b78fe82c8cc784c79b1adf5f7b02e381d5e43de8aa7266b4a7c7f3db05` | 40729 |
| `crates/gf2-coding/tests/data/bch_oracle/sage.json` | `4861230ac64ab7d5a3fb8f696925ecb9db0decc95c1d31529c5570022d717f52` | 89342 |
| `crates/gf2-coding/tests/data/bch_oracle/gap.json` | `654f9aa3d5d3d2a657ce2f6340f742e3088f3cf0f4a45bd8b392a9915790c73b` | 91348 |

## Generating source

| File | SHA-256 |
|---|---|
| `crates/gf2-coding/examples/bch_oracle_messages.rs` | `722348ead9f6bed44ffefbe89e0fda10e07fd5505b5de40a1998f6eb59ee4eb9` |
| `crates/gf2-coding/src/test_support.rs` | `17b7533731435c80df08b982d623b629efb0f31fdfd29da76e2a707729913389` |
| `dev/active/ae03bcd0-general-bch/oracle/sage_oracle.py` | `3dd7d7ef3addb690d8644ed2a6ffb2ddd3c37bef29925d2716b681be4a8bab5a` |
| `dev/active/ae03bcd0-general-bch/oracle/gap_oracle.g` | `3c95d7c913da8c70c81ccda0385a069038e57784782665e86abe2a87c8ce9766` |
| `dev/active/ae03bcd0-general-bch/oracle/run.sh` | `61da20d80ff3a610472f2b6ff33d4323e8dd2a6638bc0103ec50ec8269bd2028` |
| `crates/gf2-coding/tests/bch_oracle_agreement.rs` | `b465eaa9d1abcaa17f4bd87e09b72ec515b16b445dab04c6b8c86f75bfb09657` |
| `crates/gf2-coding/tests/test_vectors/mod.rs` | `f6de3f09e5801e2e81149b8200725e23d1644afdd0fe2557c21a7720f3eb3e62` |
| `crates/gf2-coding/tests/test_vectors/config.rs` | `c33a4d0d60187e5ade862c73dbb22ed18592debaf3bdcea4d32502dd69a9227f` |
| `crates/gf2-coding/tests/test_vectors/loader.rs` | `de46d27b698f73bfab583243c4eeff6956b2f913122b1b272e0cfaed211d7607` |
| `crates/gf2-coding/tests/test_vectors/parser.rs` | `6a95c8bbfaa4d30a87b0a489c9fab7713627c0fa3a62f6b557b8329ca499fbe5` |
