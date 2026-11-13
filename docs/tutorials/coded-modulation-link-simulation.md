# Coded-modulation link simulation

This tutorial simulates the DVB-T2 bit-interleaved coded modulation link of
`@/citation/Etsi2015` over an Es/N0 sweep and reads its frame error rate (FER)
with confidence intervals. The chain is the BCH and LDPC normal-frame code,
bit interleaving, cell-word demultiplexing and 16-QAM cell mapping, AWGN, soft
demapping and belief-propagation decoding. The program is the campaign binary
[`../../experiments/dvb-t2-awgn/src/bin/dvb_t2_awgn_campaign.rs`](../../experiments/dvb-t2-awgn/src/bin/dvb_t2_awgn_campaign.rs),
which builds the chain with the `gf2-sim` DVB-T2 preset and checkpoints every
point. Supported code configurations are in
[Standards conformance](../reference/standards-conformance.md#dvb-t2);
interruption, resumption and the complete output layout are in
[Run a simulation campaign](../how-to/run-simulation-campaigns.md#checkpointed-dvb-t2-campaigns).

## Configure the preset pipeline

`Pipeline::dvb_t2()` is a typestate builder whose call order is checked at
compile time ([`../../crates/gf2-sim/examples/dvb_t2_typestate.rs`](../../crates/gf2-sim/examples/dvb_t2_typestate.rs)).
The binary builds the pipeline from its arguments and then sets the sweep on
the pipeline's `PipelineConfig`:

```rust
let mut pipeline = Pipeline::dvb_t2()
    .modcod(modcod)
    .decoder(args.decoder)
    .demap(args.demap)
    .channel(Channel::awgn(channel_es_n0 as f32))
    .parallelism(parallelism)
    .seed(args.seed)
    .with_gpu(args.gpu)
    .build()
    .map_err(|e| format!("Cannot build DVB-T2 pipeline: {e:?}"))?;

let cfg = pipeline.config_mut();
cfg.esn0_db_points = esn0_points.to_vec();
cfg.target_errors = target_errors as u64;
cfg.max_frames = max_frames as u64;
cfg.seed = args.seed;
cfg.gpu_enabled = args.gpu;
cfg.strict_gpu = args.strict_gpu;
cfg.checkpoint_dir = checkpoint_dir;
cfg.heartbeat_every_frames = heartbeat_every_frames;
cfg.tracing_log_path = tracing_log_path;
```

`modcod` is `Modcod::Normal` with the requested rate and modulation, and
`parallelism` is the host's available parallelism. Each sweep point derives
its noise level from its own entry of `esn0_db_points`, in dB Es/N0, so the
`Channel::awgn` argument does not fix the sweep. `target_errors` and
`max_frames` are the per-point frame budget. The sweep simulates
`heartbeat_every_frames` frames per chunk; after each chunk it writes the
point's checkpoint under `checkpoint_dir` and checks the error target. A
library caller sets the same fields and calls `Pipeline::run_checkpointed`.

## Run the sweep

```bash
cargo run --release -p dvb-t2-awgn --bin dvb_t2_awgn_campaign -- \
    --rate 1/2 --modulation 16qam --esn0-range 5.9:6.2:0.1 \
    --decoder sumproduct --demap exactlogmap \
    --target-errors 50 --max-frames 300 --heartbeat-frames 50 \
    --output-dir dvb_r12_16qam --seed 42
```

The run decodes with sum-product belief propagation and exact log-MAP
demapping; `--help` lists the other decoders and demappers.
Each point stops after the first 50-frame chunk that brings its frame-error
count to at least 50, or at 300 frames, so the four points cost at most 1200
frames. Every frame's outcome is a function of the seed, the point index and
the frame index, so the deterministic CSV columns are identical across worker
counts ([`../../crates/gf2-sim/tests/parallel_determinism.rs`](../../crates/gf2-sim/tests/parallel_determinism.rs))
and across repeated runs
([`../../experiments/dvb-t2-awgn/tests/campaign_byte_identity.rs`](../../experiments/dvb-t2-awgn/tests/campaign_byte_identity.rs)).

`<campaign-dir>/curve_1_2_16qam.csv` under `dvb_r12_16qam` holds one row per point under
the header

```text
es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds
```

`frames` is the number of simulated frames and `errors` the number of frame
errors, a frame error being any information-bit error in the decoded
BBFRAME. `fer` is `errors / frames`, `ber` counts bit errors over the same
BBFRAME bits, and `mean_iters` is the mean belief-propagation iteration count.
`wall_seconds` is the host's sweep wall-clock time divided by the number of
points, an unbenchmarked observation; performance claims follow
`@/inv/benchmark-backed-performance`. The determinism contract covers
`es_n0_db`, `fer`, `frames`, `errors` and `mean_iters`; `ber` and
`wall_seconds` lie outside it.

## Interpret the result

A 95% Clopper-Pearson interval for each point's FER follows from its `errors`
and `frames` columns. This program prints one interval per CSV row read from
standard input:

```rust
use std::io::BufRead;

use gf2_stats::intervals::clopper_pearson_interval;

fn main() {
    for line in std::io::stdin().lock().lines().skip(1) {
        let line = line.unwrap();
        let col: Vec<&str> = line.split(',').collect();
        let (frames, errors): (u64, u64) = (col[3].parse().unwrap(), col[4].parse().unwrap());
        let (lo, hi) = clopper_pearson_interval(errors, frames, 0.95);
        println!("{} dB: FER {} in [{lo:.4}, {hi:.4}]", col[0], col[1]);
    }
}
```

Read the run's curve against its stopping rule:

- A point whose `mean_iters` approaches the decoder's iteration cap
  (`DvbT2Concat::max_ldpc_iterations`) is above the waterfall: most frames
  fail to converge. The waterfall lies between that point and the first point
  with few or no errors, and `mean_iters` falls across it.
- A point with `errors` at or above `--target-errors` stopped on the error
  target. Its stopping depended on the outcomes, so the interval, which treats
  `frames` as fixed, is approximate.
- A point with `frames` equal to `--max-frames` and fewer errors stopped on
  the frame budget. With few or no errors its interval bounds the FER from
  above rather than locating it; a larger `--max-frames` narrows it.
- If every point lies on one side of the waterfall, shift `--esn0-range`
  towards the other side.

## Resume a sweep

Rerunning the command with `--resume` loads the completed checkpoints,
simulates no frames and rewrites the CSV with the same deterministic columns.
A resume with a different `--max-frames` fails with `ConfigHashMismatch`, so
a larger budget is a fresh run;
[Run a simulation campaign](../how-to/run-simulation-campaigns.md#checkpointed-dvb-t2-campaigns)
describes the checkpoint identity.

## Adapt it

`--rate` and `--modulation` select the other preset MODCODs listed by `--help`.
`Pipeline::nr_5g()` builds the 5G NR LDPC chain of `@/citation/ThreeGpp2017`
without a sweep plan;
[`../../crates/gf2-sim/examples/nr_5g_quickstart.rs`](../../crates/gf2-sim/examples/nr_5g_quickstart.rs)
drives one frame of it through `TopologyExecutor::run`.
