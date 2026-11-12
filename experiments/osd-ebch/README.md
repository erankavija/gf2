# eBCH OSD campaign

`ebch_osd_awgn_campaign` runs the pinned eBCH(128,64,22) BPSK AWGN grid.
The experiment owns the deterministic block protocol, stopping rule, intervals,
and receipt format. It consumes `gf2-sim::checkpoint`, worker scheduling and
`gf2-coding` encoders, channels and OSD decoders.

```bash
cargo run -p osd-ebch --release --bin ebch_osd_awgn_campaign -- \
    --checkpoint /tmp/osd/checkpoint.json --receipt /tmp/osd/receipt.json
```

`provenance.rs` defines the campaign serialization under @/issue/779d1926, using identity types from
`experiments-common`. The invocation vector is non-identity metadata retained
for every contributing invocation.
