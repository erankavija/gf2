# Receipt: pre-cutover BCH throughput baseline (jit:88ca7d2f)

Rendered by `render_receipt.py` from the run directory; every measured figure below is read from the committed Criterion `estimates.json` and `sample.json` files at render time.

| Field | Value |
|---|---|
| Issue | `88ca7d2f` |
| gf2 revision | `74871c6b73b722f35d5acb5ef68750cecd93e2ef` |
| Tree state | clean (git status --porcelain was empty) |
| Host | AMD Ryzen 9 5900X 12-Core Processor |
| Cores | full host (`nproc` = 24); serialized via the CCX1 lock wrapper's `--full-host` mode, no `taskset` pin |
| Governor | powersave |
| Kernel | Linux 7.1.11-arch1-1 x86_64 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |
| Cargo | cargo 1.97.0 (c980f4866 2026-06-30) |
| Measurement window (UTC) | 2026-09-02T15:50:31Z to 2026-09-02T15:58:28Z |
| Load average at run start | 18:50:31 up 3 days, 18:56,  2 users,  load average: 0.31, 0.22, 0.14 |

## Inclusion list

Every Criterion bench target in `crates/gf2-coding/benches/` whose measured code path is the legacy `gf2_coding::bch::{BchCode,BchEncoder,BchDecoder}` surface, found by reading `bch_parallel.rs` and `batch_operations.rs` in full and grepping the remaining bench files for `bch`/`Bch`:

| File | Included groups | Reason |
|---|---|---|
| [`bch_parallel.rs`](../../../crates/gf2-coding/benches/bch_parallel.rs) | `bch_batch_decode`, `bch_single_vs_batch` | Both groups decode DVB-T2 short-frame codewords through `BchDecoder::decode`/`decode_batch`, the legacy decode path. |
| [`batch_operations.rs`](../../../crates/gf2-coding/benches/batch_operations.rs) | `bch_batch`, `bch_sequential_vs_batch` | Both groups encode through `BchEncoder::encode`/`encode_batch` on legacy `BchCode` constructions (one DVB-T2-sized, one `BchCode::new(15, 11, 1, ...)`). The file's other two groups (`ldpc_batch_with_backend`, `ldpc_sequential_vs_batch`) measure LDPC, not BCH, and are excluded from this receipt by the runner's `bch_` filter. |
| `bch_genmatrix.rs` | none | Its `bch_genmatrix_w2` and `bch_paritycheck` groups measure `BinaryBchCode`/`BchSpec`, the canonical construction model, not the legacy `BchCode`/`BchEncoder`/`BchDecoder` path this receipt baselines. |
| `allocation_optimization.rs`, `bench_support.rs`, `cpu_dispatch_probe.rs`, `ldpc_decode.rs`, `ldpc_throughput.rs`, `linear_codes.rs`, `llr_simd.rs`, `modem_cpu.rs`, `modem_generic_vs_fast.rs`, `profile_ldpc_decode.rs`, `profile_ldpc_encode.rs`, `quick_parallel.rs`, `simulation_no_analysis_overhead.rs`, `sparse_preprocessing.rs` | none | `grep -lE "bch|Bch"` over `crates/gf2-coding/benches/*.rs` found no match in any of these files. |

## Seed statement

None of the included groups draw from a random-number generator; each constructs its inputs deterministically from loop indices in the bench source itself, so there is no seed constant to record:

- `bch_batch_decode`, `bch_single_vs_batch` (`bch_parallel.rs`): each message is an all-zero `BitVec` with bits `0..8` set from the low 8 bits of the batch index `i`; encoded once through `BchEncoder::encode` to produce the codewords the group decodes.
- `bch_batch` (`batch_operations.rs`): the message is a single all-zero `BitVec::zeros(k)`, cloned `batch_size` times.
- `bch_sequential_vs_batch` (`batch_operations.rs`): each message bit `j` of row `i` is set to `(i + j) % 2 == 0`.

## Exact invocations

Load average is `uptime`'s three-figure line, captured immediately before and after each bench target ran; it is an observed fact about host contention during this run, not a gating threshold.

| Bench target | Invocation | Started (UTC) | Load avg at start | Finished (UTC) | Load avg at end |
|---|---|---|---|---|---|
| `bch_parallel` | `CARGO_CI_NO_LOCK=1 ./dev/scripts/ccx1-bench-flock.sh --full-host ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_parallel -- bch_` | 2026-09-02T15:50:31Z |  18:50:31 up 3 days, 18:56,  2 users,  load average: 0.31, 0.22, 0.14 | 2026-09-02T15:52:20Z |  18:52:20 up 3 days, 18:58,  2 users,  load average: 1.16, 0.55, 0.27 |
| `batch_operations` | `CARGO_CI_NO_LOCK=1 ./dev/scripts/ccx1-bench-flock.sh --full-host ./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench batch_operations -- bch_` | 2026-09-02T15:52:20Z |  18:52:20 up 3 days, 18:58,  2 users,  load average: 1.16, 0.55, 0.27 | 2026-09-02T15:58:28Z |  18:58:28 up 3 days, 19:04,  2 users,  load average: 0.93, 0.51, 0.32 |

## Measured benchmarks

One row per Criterion benchmark ID, exactly as Criterion names it (`<group>/<function-or-parameter>`). Median and the 95% CI bounds are `estimates.json`'s `median` estimate (wall-clock nanoseconds per iteration); Samples is the length of `sample.json`'s `times` array.

| Benchmark ID | Group | Function/Parameter | Samples | Sampling mode | Median (ns/iter) | 95% CI lower (ns) | 95% CI upper (ns) |
|---|---|---|---|---|---|---|---|
| `bch_batch/1` | `bch_batch` | `1` | 100 | Linear | 107652.25 | 107619.51 | 107687.73 |
| `bch_batch/10` | `bch_batch` | `10` | 100 | Linear | 1081121.43 | 1078647.32 | 1233432.07 |
| `bch_batch/100` | `bch_batch` | `100` | 100 | Flat | 10792188.10 | 10790467.00 | 10796141.20 |
| `bch_batch/50` | `bch_batch` | `50` | 100 | Flat | 5387589.10 | 5386175.00 | 5388771.60 |
| `bch_batch_decode/1` | `bch_batch_decode` | `1` | 100 | Linear | 34398.07 | 34387.09 | 34418.66 |
| `bch_batch_decode/10` | `bch_batch_decode` | `10` | 100 | Flat | 22651079.83 | 22647681.50 | 22653596.33 |
| `bch_batch_decode/100` | `bch_batch_decode` | `100` | 100 | Flat | 248802731.50 | 248764061.00 | 248828896.00 |
| `bch_batch_decode/50` | `bch_batch_decode` | `50` | 100 | Flat | 123140275.00 | 123123625.00 | 123150100.00 |
| `bch_sequential_vs_batch/batch_operation` | `bch_sequential_vs_batch` | `batch_operation` | 100 | Linear | 107712.06 | 107684.21 | 107739.74 |
| `bch_sequential_vs_batch/sequential_loop` | `bch_sequential_vs_batch` | `sequential_loop` | 100 | Linear | 108114.58 | 108090.43 | 108158.59 |
| `bch_single_vs_batch/batch_api` | `bch_single_vs_batch` | `batch_api` | 100 | Flat | 123157700.00 | 123147115.00 | 123192550.00 |
| `bch_single_vs_batch/single_loop` | `bch_single_vs_batch` | `single_loop` | 100 | Flat | 123190290.00 | 123145835.00 | 123230325.00 |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-09-02-88ca7d2f-batch_operations-criterion.txt` | `c440a566b89d5130d739d16f0c82f7b12f148f39ea4addeaeffbe9a521bfe3c5` | 3173 |
| `2026-09-02-88ca7d2f-bch_parallel-criterion.txt` | `e4d57c5938cf7518c25484208c1ec0f085cca78b281b21b5c9507f9911742e05` | 2897 |
| `2026-09-02-88ca7d2f-host.txt` | `9355d648b53011be1129451df85635e7847dd240336ff1b5c0f3d29181dc856d` | 630 |
| `samples/bch_batch_1/benchmark.json` | `d214fb6354929e51938a8f4962b907ec7871b2576352de750b37d525ef4ba60d` | 164 |
| `samples/bch_batch_1/estimates.json` | `9097f6fa7f774264adab5d4e0b040baa7b0f8cb287cb9db0b7fa7c394c13a633` | 985 |
| `samples/bch_batch_1/sample.json` | `97ffb8d74db83738e85e0f1c0821ad1bd5871163af045b65e3c8efcaeede7e4f` | 1737 |
| `samples/bch_batch_10/benchmark.json` | `f24852296d64dc6b27603343adf6f091a1d41131a82e9600ff8c8b9b7deb1c53` | 169 |
| `samples/bch_batch_10/estimates.json` | `65dcee725f7f65ac4e10ca0b97228c19d785fc508dd99a9d171144854659e98a` | 976 |
| `samples/bch_batch_10/sample.json` | `6ce7d546f5a4872f7456d44a5eaddb32c861364e367ae250aec6c55b66329615` | 1652 |
| `samples/bch_batch_100/benchmark.json` | `922e48376841b6d1e4e7de005af4c664036f82646c8faa286df9e8eeb34a8cfb` | 174 |
| `samples/bch_batch_100/estimates.json` | `4d5447b343ed90c24a4ee5b8fd136fdc14fbcc95cc6ff3c7439ab48a5f82105d` | 781 |
| `samples/bch_batch_100/sample.json` | `34b7d8357c16526f0c3a757950e5e47027d60464155e215d9299f85a7e7d5eaa` | 1544 |
| `samples/bch_batch_50/benchmark.json` | `398c94b5d624eb95e914898e0328e0d6210fcd91dd4ee45ec23a7976c35057cb` | 170 |
| `samples/bch_batch_50/estimates.json` | `4c97e2486c3eb8b54768b4a9abb00d90c86b1fb4673d6621d9a993146fb2080b` | 770 |
| `samples/bch_batch_50/sample.json` | `80b532853bb18db70e74e226006090e651f6c13443dc0b19444721d901971caf` | 1644 |
| `samples/bch_batch_decode_1/benchmark.json` | `9845e3a0638c5254e4de86192737de2436fb7347d3738297f0f2072e5056625d` | 192 |
| `samples/bch_batch_decode_1/estimates.json` | `bf546a287e950c7780f9677efa9d1ec44a656be04248936d9d9bb5b2c929d9b4` | 979 |
| `samples/bch_batch_decode_1/sample.json` | `e1b279c4bd22183bd2e79ab3a11dbb6698e5e5ff9967bf0c07d09143f95000b5` | 1799 |
| `samples/bch_batch_decode_10/benchmark.json` | `8d7974570c8bf3ec1bfe2eb4694762727cfc50c1e2bd91f246351a4efab383b4` | 197 |
| `samples/bch_batch_decode_10/estimates.json` | `fe6d2366f4b7048ca248791ea7d74f7f643626cbc7b92a6c2e6e0149f6060493` | 792 |
| `samples/bch_batch_decode_10/sample.json` | `1842e6c9a0bb081d197d3908fcc5fb04f0befa2118468a2967b13b20eb0246de` | 1544 |
| `samples/bch_batch_decode_100/benchmark.json` | `9f464a1712b23db6d660984ab13738f8ba8de013e6ecd784b7aab32bc39aa703` | 202 |
| `samples/bch_batch_decode_100/estimates.json` | `e2da865ff0caa311c82190efc34c3e34e719ddac786445ae81ae0592020f7cd8` | 769 |
| `samples/bch_batch_decode_100/sample.json` | `4bc3f6df81949102912a1340cd59aee052fe1767f380a710d811c451151e9ad7` | 1644 |
| `samples/bch_batch_decode_50/benchmark.json` | `0a375b2bd004edd8a610bbf037b832be0eb3a8d2ea6b70f63bb6a53977542712` | 197 |
| `samples/bch_batch_decode_50/estimates.json` | `615136efa8386255c8463edbe57a4fc14cb92aff40ef9f1b5d89d342c4e7aad4` | 767 |
| `samples/bch_batch_decode_50/sample.json` | `d0fb50cc809c05280edc6442760b834683d0fe9ab5e193466e30a3c988898599` | 1644 |
| `samples/bch_sequential_vs_batch_batch_operation/benchmark.json` | `13408d3cf1c161c234cbc37e270974b174c91e00f08a62176863fd1eaafeb9cc` | 275 |
| `samples/bch_sequential_vs_batch_batch_operation/estimates.json` | `01f24eab33d22d6bc6e2b605a2208f702894fc9d2ff3dc144db07cee2f23d932` | 985 |
| `samples/bch_sequential_vs_batch_batch_operation/sample.json` | `b68634203a98f03ff927590c39deaa5a9d2c3a98f92dbd04ab3783fc381a8469` | 1737 |
| `samples/bch_sequential_vs_batch_sequential_loop/benchmark.json` | `d00db277423e73f8fcac65ace03c122da13364437d4569ed9e894fe215a56b34` | 275 |
| `samples/bch_sequential_vs_batch_sequential_loop/estimates.json` | `2e614dc7c50360144e732ed15cfc018ebe25f912b94989eb9c41003a5a21db4f` | 987 |
| `samples/bch_sequential_vs_batch_sequential_loop/sample.json` | `d81734e6b55b52e82417b3fb3fd519ff10dc640b1a6fc7473b7b8d99fb21cbf4` | 1737 |
| `samples/bch_single_vs_batch_batch_api/benchmark.json` | `5b1a2858c4054e56be589404e60220b00a434f299052f2de1d336818786c892a` | 226 |
| `samples/bch_single_vs_batch_batch_api/estimates.json` | `f4b2a949ebcee7ced370fea271b61c060618aba14b494b7d0a52414dd7c14be2` | 772 |
| `samples/bch_single_vs_batch_batch_api/sample.json` | `680f5647ee72eeb3e5f2ac03fb4fdbabc7bd9b525987d9c55a507b1e01860cfb` | 1644 |
| `samples/bch_single_vs_batch_single_loop/benchmark.json` | `685e5933ea4767dc8644c77cb03d5973f6312a204b7ae9da0563dc7410d32649` | 234 |
| `samples/bch_single_vs_batch_single_loop/estimates.json` | `a3c6d714b1ee39077916d5a9fd214a9f4e7a05c692b45d07e82560e08342807d` | 773 |
| `samples/bch_single_vs_batch_single_loop/sample.json` | `c52c0e8f4c4112dce10f85d855ef73ccb721df85c18d6e28e6e15dc22edbfcbe` | 1644 |

## Reproduction

```
dev/bench_results/88ca7d2f/run.sh
```
