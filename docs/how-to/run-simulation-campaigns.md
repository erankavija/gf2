# Run a simulation campaign

A campaign produces BER and BLER curves over an Eb/N0 or Es/N0 sweep. Two
runners cover it: `sim_runner` in `gf2-coding` executes a TOML campaign file of
LDPC, product-code and GLDPC curves, and `dvb_t2_awgn_campaign` in `gf2-sim`
runs the DVB-T2 BICM chain with per-SNR checkpoints.

## Configure a campaign file

A campaign file holds one `[campaign]` table and one `[[curve]]` table per
curve:

```toml
[campaign]
name = "bg2_decoder_compare"
output_dir = "results/bg2"

[[curve]]
name = "bg2_nms"
type = "ldpc"
base_graph = 2
n = 1024
k = 441
algorithm = "nms"
scale = 0.75
snr = { start = 0.0, stop = 2.0, step = 1.0 }
min_errors = 20
max_frames = 2000

[[curve]]
name = "bg2_sp"
type = "ldpc"
base_graph = 2
n = 1024
k = 441
algorithm = "sum_product"
snr = { start = 0.0, stop = 2.0, step = 1.0 }
min_errors = 20
max_frames = 2000
```

`snr` is an inclusive Eb/N0 range in dB. Each point stops at `min_errors`
frame errors or `max_frames` frames, whichever comes first. `type` selects the
code family, and the keys each family accepts, including the optional
`channel` table, are the fields of `CurveConfig` in
[`sim_runner.rs`](../../crates/gf2-coding/src/bin/sim_runner.rs). The
repository's own campaign files are in
[`dev/campaigns/`](https://github.com/erankavija/gf2/tree/6776fe30ea9af85ad509ddca95926f051421676d/dev/campaigns).

## Run it

Check that the file parses and expands to the intended points:

```bash
cargo run -p gf2-coding --release --features parallel --bin sim_runner -- \
    campaign.toml --dry-run
```

Run every curve, or select curves with repeated `--curve <name>`:

```bash
cargo run -p gf2-coding --release --features parallel --bin sim_runner -- \
    campaign.toml --parallel --seed 7
```

`--seed` defaults to 42. `--parallel` simulates the SNR points of an LDPC or
GLDPC curve concurrently and the frames of a product-code curve concurrently.

## Locate the output

`output_dir` resolves against the working directory. Per curve it holds:

| File | Content |
|---|---|
| `<name>.csv` | One row per finished point: `eb_n0_db`, `ber`, `bler`, bit and frame counts, `avg_iterations`, `avg_queries_per_bit`. Appended as each point finishes and rewritten in configured point order when the curve ends. |
| `<name>.json` | The same rows as a JSON array, written when the curve ends. |
| `<name>.progress.jsonl` | Progress and `point_complete` records, one JSON object per line. Absent for product-code curves run with `--parallel`. |

## Stop and resume

`sim_runner` writes no checkpoint files; its CSV is the resume record. A
stopped run keeps the rows of finished points and loses all progress of
in-flight points. Rerunning the same command reuses every row whose
frame-error count reached `min_errors` and simulates the remaining points from
their start; a point that ended at `max_frames` is simulated again. Product-code
curves run with `--parallel` reuse no rows and recompute every point.

- On LDPC, GLDPC and sequential product-code curves, SIGINT and SIGTERM do
  not interrupt an in-flight point. With `--parallel` the run exits with
  status 1 once its in-flight points finish; a sequential run continues.
  Ending the process otherwise, for example with `kill -KILL <pid>`, flushes
  nothing and discards in-flight points as described above.
- Reused rows are keyed by Eb/N0 alone. After changing a curve's parameters,
  delete its CSV or change `output_dir`.
- With `--parallel`, each point draws from its own stream derived from
  `--seed`, so a resumed run reproduces the rows of an uninterrupted one.
  Without it, all points share one stream and resumed rows are statistically
  equivalent, not identical.

## Checkpointed DVB-T2 campaigns

`dvb_t2_awgn_campaign` takes its configuration on the command line:

```bash
cargo run -p gf2-sim --release --bin dvb_t2_awgn_campaign -- \
    --rate 1/2 --modulation 16qam --esn0-range 4.0:7.0:0.5 \
    --target-errors 100 --max-frames 10000000 \
    --output-dir dvb_r12_16qam --seed 42
```

It writes `checkpoints/snr_<index>.json` under `--output-dir` at each SNR
boundary and every `--heartbeat-frames` frames within a point. SIGINT or
SIGTERM flushes the in-progress checkpoint and ends the run. Repeat the same
command with `--resume` to continue. The checkpoints carry a BLAKE3 hash of
`--seed`, `--esn0-range`, `--target-errors`, `--max-frames`,
`--heartbeat-frames`, `--gpu`, `--strict-gpu` and the host's worker count, and a
resume that changes any of them fails with a configuration-hash mismatch. The
hash excludes `--rate`, `--modulation`, `--decoder` and `--demap`; a resume
that changes them reuses the existing checkpoints. A run without `--resume` deletes existing
checkpoints. The deterministic CSV columns of a resumed run are byte-identical
to an uninterrupted run at the same seed
([`campaign_cli_flags.rs`](../../crates/gf2-sim/tests/campaign_cli_flags.rs)).

The output directory also holds `curve_<rate>_<modulation>.csv` with columns
`es_n0_db`, `fer`, `ber`, `frames`, `errors`, `mean_iters` and `wall_seconds`,
a `tracing.jsonl` event log, and a `README.md` recording the invocation, seed
and host. `--help` lists the decoder, demapper, calibration and GPU options.
