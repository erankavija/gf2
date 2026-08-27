# eBCH(128,64) OSD campaign worker-scaling receipt

Issue: `258be082`. This receipt backs the speedup claim REQ-05 requires for the
parallel OSD campaign protocol, and independently witnesses the worker-count
invariance REQ-01 requires.

## Protocol

Each trial starts from no checkpoint and samples exactly 120000 blocks in the
campaign's first cell (order 2, $E_b/N_0 = 1.55$ dB), then stops interrupted:
`--target-block-errors` is set to $10^9$, beyond any reachable count, so the
sampled work per trial is fixed and depends on neither the worker count nor any
sampled outcome. Wall clock is measured around the binary invocation. Three
trials run at each of 1, 2, 8, and 24 workers, in that order.

Timing a bounded interrupted prefix rather than a completed cell is deliberate:
a completed cell stops at its $K$-th block error, so its sample count is a
random stopping time and its wall clock would confound decode throughput with
that cell's error rate.

Every trial's resulting checkpoint is reduced to its cell evidence — the
`cell_results`, `cells`, `campaign_seed`, and `target_block_errors` payload
fields, excluding the invocation provenance that records each run's argument
vector — and hashed, so this receipt states directly whether worker count
changed the evidence.

The runs were serialized through the repository benchmark mutex with
`dev/scripts/ccx1-bench-flock.sh --full-host`, which holds the canonical lock
and deliberately omits the CCX1 `taskset` pinning because the measured
configuration is the full processor. Renicing was attempted and denied for the
non-root user; the host was otherwise idle.

## Results

Cell evidence hash, all 12 trials:
`8c95f21e2200c92cefd17717c390c69a7f34bdc74bc5ecdd81b0cb2ceac987a8`.

Every worker count produced byte-identical cell evidence. This is an
end-to-end witness on the real eBCH/OSD evaluator, independent of the scripted
in-process invariance tests in `crates/gf2-sim/tests/osd_campaign_protocol.rs`.

| workers | trial 1 (s) | trial 2 (s) | trial 3 (s) | mean (s) | speedup | parallel efficiency |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 77.74 | 76.98 | 78.31 | 77.68 | 1.00× | 100.0% |
| 2 | 39.78 | 41.15 | 41.51 | 40.81 | 1.90× | 95.2% |
| 8 | 12.33 | 12.40 | 12.39 | 12.37 | 6.28× | 78.5% |
| 24 | 6.18 | 6.22 | 6.40 | 6.27 | 12.40× | 51.6% |

Throughput rises from 1545 blocks/s at one worker to 19139 blocks/s at 24.

Efficiency falls off in the expected place: the host has 12 physical cores with
2 SMT threads each, so 24 workers occupy 12 cores. Measured against the 12
physical cores rather than the 24 logical threads, the 24-worker run holds
103% — the decode loop is compute-bound, and SMT contributes the residual 0.4×
above linear physical-core scaling. The 51.6% figure in the table is efficiency
per logical thread and understates the result for that reason.

## Interpretation and scope

This measures the campaign's dominant inner loop — per-block encode, BI-AWGN
transmission, and order-2 OSD reprocessing — at a single grid cell. Order-2
per-block cost is approximately independent of $E_b/N_0$, because the
reprocessing candidate list is fixed by the order and the code parameters
rather than by the received values, so this cell's speedup is expected to carry
to the campaign's dominant order-2 cell at 5.23 dB. That expectation is an
estimate; this receipt does not measure the 5.23 dB cell.

Worker counts above the host's 24 logical threads are correct but serialize the
excess and were not measured. The wave size constant
`OSD_WAVE_BLOCKS_PER_WORKER = 16` was not swept; it changes how much
speculative work a cell performs past its stopping block, never the committed
result.

## Provenance and regeneration

- Git revision: `1e2d488581ca30e8d2d03b40bad1bca9f6739d98`
- Binary: `target/release/ebch_osd_awgn_campaign`, SHA-256
  `b1015246dfff6a4459921bfc658fee257a4e542bbf439e2f5000924d64ea7f86`
- CPU model: `AMD Ryzen 9 5900X 12-Core Processor` (12 physical cores, 24
  logical threads)
- Toolchain: `rustc 1.97.0 (2d8144b78 2026-07-07)`; `cargo 1.97.0 (c980f4866
  2026-06-30)`
- Campaign seed: `0xC8322EFF` (the binary's pinned default)

Regenerate one cell of the table with:

```sh
cargo build --release --bin ebch_osd_awgn_campaign
./dev/scripts/ccx1-bench-flock.sh --full-host \
    ./target/release/ebch_osd_awgn_campaign \
    --checkpoint "$(mktemp -d)/cp.json" --receipt "$(mktemp -d)/rc.json" \
    --seed 0xC8322EFF --max-samples 120000 \
    --target-block-errors 1000000000 --workers <N>
```

Timing is wall clock around that invocation; the cell-evidence hash is the
SHA-256 of the checkpoint payload's `cell_results`, `cells`, `campaign_seed`,
and `target_block_errors` fields serialized as compact JSON with sorted keys.
