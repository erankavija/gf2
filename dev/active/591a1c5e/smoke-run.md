# BCH bench smoke run (591a1c5e)

This file records that the two migrated benches build and run. It is a smoke
run with one-second warm-up and measurement windows and ten samples, on a
shared host. It is not a measurement receipt; `fd9d5416` and `d1b4f85e` own
measurements.

- Revision: `ed64cad847d5f8e0b7b696b167cd8678cf7e69ca` (bench sources as committed)
- Host: AMD Ryzen 9 5900X 12-Core (24 threads), Linux 7.2.6-arch2-1 x86_64, rustc 1.97.0, `--release` bench profile, no CPU pinning
- Commands, from the repository root:

```text
./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_parallel -- bch_ --warm-up-time 1 --measurement-time 1 --sample-size 10
./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench batch_operations -- bch_ --warm-up-time 1 --measurement-time 1 --sample-size 10
```

## Criterion IDs and smoke times (low / mid / high)

| ID | time |
|---|---|
| `bch_batch_decode/1` | 25.205 µs / 25.797 µs / 26.324 µs |
| `bch_batch_decode/10` | 12.157 ms / 12.201 ms / 12.225 ms |
| `bch_batch_decode/50` | 65.733 ms / 65.929 ms / 66.143 ms |
| `bch_batch_decode/100` | 130.28 ms / 131.57 ms / 132.34 ms |
| `bch_single_vs_batch/single_loop` | 62.753 ms / 63.121 ms / 63.608 ms |
| `bch_single_vs_batch/decode_into_loop` | 68.161 ms / 69.063 ms / 69.986 ms |
| `bch_encode_pns_16383_16215/1` | 65.860 µs / 65.923 µs / 66.023 µs |
| `bch_encode_pns_16383_16215/10` | 659.69 µs / 659.90 µs / 660.21 µs |
| `bch_encode_pns_16383_16215/50` | 3.2961 ms / 3.3207 ms / 3.3489 ms |
| `bch_encode_pns_16383_16215/100` | 6.5727 ms / 6.5843 ms / 6.6072 ms |
| `bch_sequential_vs_batch/sequential_loop` | 9.2116 µs / 9.2247 µs / 9.2367 µs |
| `bch_sequential_vs_batch/batch_operation` | 6.5327 µs / 6.5429 µs / 6.5530 µs |

## ID mapping against the pre-cutover baseline (`dev/bench_results/88ca7d2f/`)

| Baseline ID | Status | Reason |
|---|---|---|
| `bch_batch_decode/{1,10,50,100}` | kept | Same code (DVB-T2 short, rate 1/2), batch sizes, codewords; sequential allocating decode per codeword collecting the messages. The baseline's `decode_batch` ran sequentially (the `parallel` feature is off by default). The decoder implementation is the canonical one, which is what the comparison measures. |
| `bch_single_vs_batch/single_loop` | kept | Same code, 50 codewords, allocating decode loop. |
| `bch_single_vs_batch/batch_api` | removed | The canonical DVB-T2 decoder has no batch call. |
| `bch_single_vs_batch/decode_into_loop` | new | Allocation-free decode on a reused workspace. |
| `bch_batch/{1,10,50,100}` | renamed to `bch_encode_pns_16383_16215/{1,10,50,100}` | The baseline's BCH(16200, 16008) over GF(2^14) paired a 168-bit generator with 192 bits of redundancy and has no canonical counterpart; the measured code is the primitive narrow-sense code of length 16383 and dimension 16215. Not comparable. |
| `bch_sequential_vs_batch/{sequential_loop,batch_operation}` | kept | Same (15, 11) code over GF(2^4), 100 messages, allocating per-message encode and the batch API. |
