# cdba4e71: coded-modulation tutorial run record

Raw outputs behind `docs/tutorials/coded-modulation-link-simulation.md`, in
[`cdba4e71-link-simulation-run/`](cdba4e71-link-simulation-run/). Source:
worktree branch based on `92fa3e22895d9f6d02f7acabaf778d4318336f20`, with no
source changes. Toolchain: `cargo 1.97.0 (c980f4866 2026-06-30)`,
`rustc 1.97.0 (2d8144b78 2026-07-07)`. Host: AMD Ryzen 9 5900X, 24 hardware
threads, shared with other sessions; the host and kernel line is in
`run-README.md`.

## Commands

Run from the repository root.

1. Fresh sweep; `dvb_r12_16qam/curve_1_2_16qam.csv` copied to
   `curve_fresh.csv` and `dvb_r12_16qam/README.md` to `run-README.md`:

   ```bash
   ./scripts/cargo-budget.sh cargo run --release -p gf2-sim --bin dvb_t2_awgn_campaign -- \
       --rate 1/2 --modulation 16qam --esn0-range 5.9:6.2:0.1 \
       --decoder sumproduct --demap exactlogmap \
       --target-errors 50 --max-frames 300 --heartbeat-frames 50 \
       --output-dir dvb_r12_16qam --seed 42
   ```

2. The same command with `--resume`; the CSV copied to `curve_resumed.csv`.
   `diff <(cut -d, -f1-6 curve_fresh.csv) <(cut -d, -f1-6 curve_resumed.csv)`
   prints nothing and exits 0.
3. The same command with `--max-frames 400 --resume`; stderr is
   `resume-max-frames-400.stderr`, exit status 1.
4. Intervals: `intervals.rs` copied to
   `crates/gf2-stats/examples/cdba4e71_intervals.rs`, then
   `./scripts/cargo-budget.sh cargo run --release -q -p gf2-stats --example cdba4e71_intervals < curve_fresh.csv > intervals.csv`;
   columns are Es/N0, errors, frames and the 95% Clopper-Pearson bounds.
