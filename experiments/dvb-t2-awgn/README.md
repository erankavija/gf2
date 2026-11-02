# DVB-T2 AWGN campaign

`dvb_t2_awgn_campaign` runs the `gf2-sim` DVB-T2 BICM pipeline with calibration
brackets, per-SNR checkpoints, CSV output and tracing. Its `hip` feature enables
`gf2-sim/hip` for GPU execution.

```bash
./scripts/cargo-budget.sh cargo run -p dvb-t2-awgn --release --bin dvb_t2_awgn_campaign -- --help
```

[Run a simulation campaign](../../docs/how-to/run-simulation-campaigns.md)
describes flags, output and resume. The
[production drivers and plotting script](../../dev/benchmarks/dvb_t2_awgn/)
consume this binary and its CSV output.
