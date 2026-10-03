# Coded-modulation link simulation

This tutorial simulates the DVB-T2 bit-interleaved coded modulation link of
`@/citation/Etsi2015` over an Es/N0 sweep and reads its frame error rate (FER)
with confidence intervals. The chain is the BCH and LDPC normal-frame code,
bit interleaving, 16-QAM mapping, AWGN, soft demapping and belief-propagation
decoding. The program is the campaign binary
[`dvb_t2_awgn_campaign.rs`](../../crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs),
which builds the chain with the `gf2-sim` DVB-T2 preset and checkpoints every
point. Supported code configurations are in
[Standards conformance](../reference/standards-conformance.md#dvb-t2);
interruption, resumption and the complete output layout are in
[Run a simulation campaign](../how-to/run-simulation-campaigns.md#checkpointed-dvb-t2-campaigns).

## Configure the preset pipeline

`Pipeline::dvb_t2()` is a typestate builder whose call order is checked at
compile time ([`dvb_t2_typestate.rs`](../../crates/gf2-sim/examples/dvb_t2_typestate.rs)).
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
./scripts/cargo-budget.sh cargo run --release -p gf2-sim --bin dvb_t2_awgn_campaign -- \
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
counts ([`parallel_determinism.rs`](../../crates/gf2-sim/tests/parallel_determinism.rs))
and across repeated runs
([`campaign_byte_identity.rs`](../../crates/gf2-sim/tests/campaign_byte_identity.rs)).

`dvb_r12_16qam/curve_1_2_16qam.csv` holds one row per point:

```text
es_n0_db,fer,ber,frames,errors,mean_iters,wall_seconds
5.9,0.89,0.02823956780923994,100,89,49.97,<elapsed>
6,0.248,0.002740561351217089,250,62,48.228,<elapsed>
6.1,0.006666666666666667,0.000004450240105977811,300,2,41.026666666666664,<elapsed>
6.2,0,0,300,0,35.3,<elapsed>
```

`wall_seconds` is the host's sweep wall-clock time divided by the number of
points, an unbenchmarked observation; performance claims follow
`@/inv/benchmark-backed-performance`. `ber` is a floating-point reduction and,
with `wall_seconds`, lies outside the determinism contract.

## Interpret the result

A frame error is any information-bit error in the decoded BBFRAME; `fer` is
`errors / frames`, `ber` counts bit errors over the same BBFRAME bits, and
`mean_iters` is the mean belief-propagation iteration count. The 95%
intervals below are `gf2_stats::intervals::clopper_pearson_interval(errors, frames, 0.95)`;
the [run record](https://github.com/erankavija/gf2/blob/08e424e21a6b8d88e096aabc9e435fa486f839c4/dev/active/fa787f85-documentation-overhaul/cdba4e71-link-simulation-run.md)
holds the raw outputs, toolchain and host:

| Es/N0 (dB) | Errors / frames | FER | 95% interval |
|---|---|---|---|
| 5.9 | 89 / 100 | 0.89 | [0.8117, 0.9438] |
| 6.0 | 62 / 250 | 0.248 | [0.1957, 0.3063] |
| 6.1 | 2 / 300 | 0.0067 | [0.0008, 0.0239] |
| 6.2 | 0 / 300 | 0 | [0.0000, 0.0122] |

- At 5.9 dB nearly every frame runs the iteration cap and fails. The
  waterfall lies between 5.9 and 6.1 dB, where the FER falls from 0.89 to
  0.0067.
- At 6.1 and 6.2 dB the frame budget ended the point. Their intervals bound
  the FER rather than locate it; a larger `--max-frames` narrows them.
- The interval treats `frames` as fixed. The 5.9 and 6.0 dB points stopped on
  the error target, so their intervals are approximate.

## Resume a sweep

Rerunning the command with `--resume` loads the completed checkpoints,
simulates no frames and rewrites the CSV with the same deterministic columns.
A resume with a different `--max-frames` fails with `ConfigHashMismatch`, so
a larger budget is a fresh run. The
checkpoint hash covers the run-control fields of `PipelineConfig`, listed at
`gf2_sim::snr_checkpoint::config_hash`, including the worker count, so a resume
runs on a host with the same available parallelism. It excludes the rate,
modulation, decoder and demapper; a run that changes any of them uses a new
`--output-dir`.

## Adapt it

`--rate` and `--modulation` select the other preset MODCODs listed by `--help`.
`Pipeline::nr_5g()` builds the 5G NR LDPC chain of `@/citation/ThreeGpp2017`
without a sweep plan;
[`nr_5g_quickstart.rs`](../../crates/gf2-sim/examples/nr_5g_quickstart.rs)
drives one frame of it through `TopologyExecutor::run`.
