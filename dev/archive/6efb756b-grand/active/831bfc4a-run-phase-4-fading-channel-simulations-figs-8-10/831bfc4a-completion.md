# Phase 4 Fading Simulations — Completion — 2026-04-16

JIT issue `831bfc4a` delivers Figs 8, 9, and 10 from the SO-GRAND
paper over QPSK + Rician-fading channels, using the paper's
turbo-SOGRAND decoder (1-line ORBGRAND with auto-IC and per-
component `list_bler_stop_threshold = 1e-4`) throughout. All three
figures have complete result files in `dev/simulation_results/`; the
per-figure paper-alignment assessment is in
`dev/simulation_results/phase4_comparison_report.md`.

## Paper-headline 8 dB check

| Figure  | Code         | Product SOGRAND | LDPC NMS | LDPC SP  |
|--------:| ---          | --------------: | -------: | -------: |
|  Fig 8  | (1024, 441)  |         0       |    0     |    0     |
|  Fig 9  | (1024, 676)  |      0.0076     |  0.0056  |  0.0052  |
|  Fig 10 | (4096, 3249) |      0.00375    |  0.0025  |  0.00125 |

All three figures also have LDPC-SP match paper BP within 1× at 8 dB.
Product-SOGRAND matches paper SOGRAND within 1-1.3× for Fig 8 across
0-4 dB, beats paper SOGRAND at 0-2 dB on Fig 9, and trails paper
SOGRAND by ~3× at 8 dB on Fig 10 (compute-reduced). Full tables in
the comparison report.

## Delivered

### `sim_runner` Rician extension

- `ChannelToml { kind, preset }` TOML sub-table on `[[curve]]`.
- `AnyChannel` wrapper implementing `ChannelModel` over
  `BpskAwgnChannel` + `QpskRicianChannelModel::new(RicianConfig::figN())`.
- `build_channel(Option<&ChannelToml>) -> Result<AnyChannel>`.
- `run_product` / `run_product_frame_parallel` generic over
  `C: ChannelModel + Sync`.
- Three integration tests: Rician end-to-end BLER decay, TOML parse
  for `rician` / `fig8`, default `awgn` when `channel` is absent.
- Leverages the completed modem-framework epic (`d4851c3d`): the
  Rician model, QPSK mapper/demapper, and `ModemChannelAdapter` all
  ship as first-class `ChannelModel` implementors.

### Campaigns (turbo-SOGRAND, paper-aligned)

- `dev/campaigns/phase4_fig8.toml` — dRM(32, 21)² product + LDPC BG2
  (1024, 441), Rician K=5 / N_c=128 / t=4 / QPSK, SNR 0-8 dB step 1.
- `dev/campaigns/phase4_fig9.toml` — eBCH(32, 26)² product + LDPC
  BG2 (1024, 676), Rician K=8 / N_c=256 / t=2 / QPSK, SNR 0-8 dB
  step 1.
- `dev/campaigns/phase4_fig10.toml` — eBCH(64, 57)² product + LDPC
  BG1 (4096, 3249), Rician K=6 / N_c=256 / t=8 / QPSK, SNR 0-8 dB
  step 2, reduced statistics (min_errors=15, max_frames=800) because
  the 4096-bit frame + 128-state trellis is intrinsically expensive.

### Results

- Fig 8: three curves, 9 SNR points each, min_errors=30, max_frames=5000.
  Product SOGRAND reaches zero errors in 5000 frames at 8 dB.
- Fig 9: three curves, 9 SNR points each, same statistics. Product
  SOGRAND **beats** paper's SOGRAND at 0-2 dB (0.881 vs 0.893;
  0.520 vs 0.667).
- Fig 10: three curves, 5 SNR points (0, 2, 4, 6, 8 dB),
  min_errors=15, max_frames=800. LDPC SP at 8 dB matches paper BP
  within 1×. Product SOGRAND at 8 dB 0.00375 vs paper SOGRAND 0.0011
  — ~3× gap at this reduced-statistics level, documented as
  follow-on.

## Decoder choice — paper-aligned turbo-SOGRAND

All three figures now use `use_bcjr = false` (turbo-SOGRAND). An
earlier session ran a BCJR first pass while the SOGRAND pattern
enumerator was weight-tiered (not paper-aligned) and impractically
slow for `n = 32` and `n = 64`. Commit `6199244` replaced the
enumerator with 1-line ORBGRAND + auto-IC + list-BLER stopping,
making paper-aligned SOGRAND practical on CPU. The Fig 8-10
campaigns were then re-run from scratch with SOGRAND. The BCJR
first-pass CSVs have been overwritten.

## Paper-alignment assessment

See `phase4_comparison_report.md` for the full tables. Condensed:

- **Fig 8 LDPC SP vs paper BP**: within 1.0-1.2× at 0-6 dB, below
  paper at 3-6 dB.
- **Fig 8 Product SOGRAND vs paper SOGRAND**: within 1.0-1.3× at
  0-4 dB, 1.25× at 5 dB, 3× at 6 dB.
- **Fig 9 LDPC**: within 1.4× of paper BP at 4-8 dB.
- **Fig 9 Product SOGRAND**: beats paper SOGRAND at 0-2 dB; within
  1.0-1.8× at 4-8 dB.
- **Fig 10 LDPC SP at 8 dB**: matches paper BP within 1×.
- **Fig 10 Product SOGRAND at 8 dB**: ~3× above paper (reduced
  statistics).

The paper's headline "product codes decoded with turbo-SOGRAND
compete with or beat 5G NR LDPC in Rician fading" reproduces
qualitatively in the 0-8 dB window for Figs 8 and 9. Paper's
crossover at 10+ dB (Fig 9) / 10+ dB (Fig 10) is beyond our window.

## Gate summary

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
- `cargo test --workspace --all-features --release`: green — 2800+
  tests including 3 new sim_runner Rician tests.

## Follow-on (not blocking epic close)

1. **Fig 9 extended sweep to 18 dB** to capture the paper's
   product-beats-LDPC crossover.
2. **Fig 10 higher-budget rerun** (min_errors=100 across 0-12 dB)
   on GPU BCJR or a parallelised CPU farm.
3. **APP-LLR clamp + prior-constant tuning** — the `±20` clamp in
   `compute_per_bit_app_llrs` and the `(2^k-1)/(2^n-1)` vs `2^-s`
   prior detail are the most likely causes of the residual 1.3-3×
   gap at mid-SNR. A standalone issue is the clean home for this.
