Figure 4 is **not yet aligned with the paper**. The immediate blocker is a CRC-product mismatch at $E_b/N_0 = 1.0$ dB: the LDPC baseline is only mildly off the paper, but the CRC product curve is materially worse and fails to reproduce the paper's product-beats-LDPC crossover. The purpose of this note is to preserve the paper-alignment investigation state so the next session can resume from measured evidence rather than rerunning full campaigns blindly.

## Background

Phase 2 story `45649554` runs Figures 4--6 from the SO-GRAND paper. A full `phase2_fig4.toml` run was started, then stopped after the CRC product curve had completed through $2.0$ dB because the live curve was already diverging from the paper in the waterfall region.

The paper reference data comes from `dev/reference_data/fig_prod_crc_25x15.csv`.

The canonical campaign under investigation is:

- `dev/campaigns/phase2_fig4.toml`

The active decoder configuration for the product curve is:

- component: `crc_25_15`
- turbo: `max_iterations = 20`, `alpha = 0.5`, `list_size = 4`, `max_queries = 100000`

The active LDPC baselines are:

- BG2 `(625,225)` normalized min-sum with `scale = 0.75`
- BG2 `(625,225)` sum-product

## Results Collected So Far

### Aborted full Figure 4 run

The detached full run was stopped after the CRC product curve had completed points through $2.0$ dB. The partial output in `dev/simulation_results/fig4_crc_product.csv` is **not** a final acceptance artifact and should be treated as a scratch by-product of the aborted run.

Measured CRC product BLER versus paper:

| $E_b/N_0$ (dB) | Measured BLER | Paper BLER | Ratio |
| --- | ---: | ---: | ---: |
| 0.0 | 0.8151 | 0.7856 | 1.04x |
| 0.5 | 0.3642 | 0.4095 | 0.89x |
| 1.0 | 0.0766 | 0.0561 | 1.36x |
| 1.5 | 0.0160 | 0.00774 | 2.07x |
| 2.0 | 0.00410 | 0.000865 | 4.74x |

Interpretation: low-SNR alignment is acceptable, but the CRC product curve stops tracking the paper from $1.0$ dB upward. Full Phase 2 Figure 4 acceptance work should remain blocked until this paper-alignment failure is understood.

### Targeted 1.0 dB alignment probe

A dedicated one-point probe at $1.0$ dB was run for:

- CRC product
- LDPC normalized min-sum
- LDPC sum-product

Measured against the paper:

| Curve | Measured BLER | Paper BLER | Ratio |
| --- | ---: | ---: | ---: |
| CRC product | 0.1004 | 0.0561 | 1.79x |
| LDPC sum-product | 0.0818 | 0.0682 | 1.20x |
| LDPC NMS(0.75) | 0.1748 | n/a | n/a |

This isolates the main paper-alignment failure to the CRC product side. The LDPC sum-product point is somewhat high but still close enough that it does not explain the missing crossover claimed by the paper.

### Sequential versus frame-parallel CRC product at 1.0 dB

A follow-up sweep compared sequential and `--parallel` product decoding at the same turbo settings.

| Mode | max_queries | BLER | BER | avg_iterations | avg_queries_per_bit | frames |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| sequential | 100000 | 0.0781 | 0.00837 | 4.93 | 102416.9 | 1281 |
| parallel | 100000 | 0.0887 | 0.00907 | 4.94 | 103377.6 | 1173 |

Lower `max_queries` values in parallel mode degraded BLER sharply:

| max_queries | BLER | avg_iterations |
| --- | ---: | ---: |
| 1000 | 0.6959 | 15.96 |
| 5000 | 0.2259 | 7.92 |
| 10000 | 0.2220 | 8.12 |
| 25000 | 0.1257 | 5.45 |
| 50000 | 0.1155 | 5.51 |
| 100000 | 0.0887 | 4.94 |

Interpretation for paper alignment:

- `max_queries` must stay high; low query budgets are clearly not paper-aligned.
- Sequential decoding is better than frame-parallel decoding at the same `max_queries`, but the measured gap at roughly 100 frame errors is not yet large enough to treat scheduling as the primary root cause.

## Strongest Current Clue

The current SOGRAND/ORBGRAND regime appears to be much more query-heavy than the paper while still delivering worse BLER.

In `crates/gf2-coding/src/grand/orbgrand.rs`, the SOGRAND path intentionally keeps sweeping patterns until `max_queries` or near-total cumulative probability instead of treating `list_size` as a real stopping condition:

```rust
// Test all patterns up to max_queries...
// For SOGRAND callers, max_queries is the primary budget control.
```

At the best measured sequential $1.0$ dB point:

- `avg_queries_per_bit = 102416.9`
- product information bits per frame $= 225$
- implied total queries per frame $\approx 2.30 \times 10^7$
- component decodes per frame $\approx 2 \cdot 25 \cdot 4.93 = 246.5$
- implied average queries per component decode $\approx 93484$

The paper reference for Figure 4 at $1.0$ dB reports:

- `avg_guesses = 1148.8`

So the present implementation is operating in a very different query regime from the paper while still underperforming on BLER. This is the strongest evidence that the paper-alignment failure is driven by SOGRAND stopping/search behavior or APP/turbo integration, rather than by the LDPC baseline.

## Current Assessment

1. The Figure 4 mismatch is real and visible already at $1.0$ dB.
2. LDPC sum-product is only mildly off the paper and is not the main blocker.
3. CRC product remains materially worse than the paper in both sequential and parallel modes.
4. Lowering `max_queries` makes the product curve much worse, so the answer is not a smaller search budget.
5. The next investigation should focus on **how SOGRAND search and soft-output are being used inside turbo decoding to reproduce the paper's Figure 4 curve**, not on rerunning full campaigns.

## Recommended Next Steps

1. Instrument a **single CRC component SOGRAND decode** at $1.0$ dB and record:
   - number of codewords found,
   - cumulative probability reached,
   - stop reason,
   - query count,
   - `list_bler_prediction`,
   - APP / extrinsic outputs.
2. Instrument a **single CRC product frame** at $1.0$ dB across turbo iterations and record:
   - row/column stop behavior,
   - per-half-iteration query counts,
   - whether early termination fires,
   - whether APP/extrinsic magnitudes look pathological.
3. Compare current behavior against plausible paper-aligned alternatives already exposed by `sim_runner`:
   - `no_early_termination`
   - `pyndiah_extrinsic`
   - `extrinsic_clamp`
   - `alpha_final`
4. Only after the component/turbo behavior is understood, rerun a single-point Monte Carlo check at $1.0$ dB.
5. Do **not** resume full Figure 4 / 5 / 6 runs until the $1.0$ dB CRC product point is aligned closely enough to the paper that the expected crossover story is plausible again.

## Reproduction Notes

Temporary probe TOMLs and scratch outputs were written under the session state directory during this investigation. Those files are not authoritative; the key measured values are copied into this note so the next session does not need access to transient session artifacts.

Relevant repository files for follow-up:

- `dev/campaigns/phase2_fig4.toml`
- `dev/reference_data/fig_prod_crc_25x15.csv`
- `crates/gf2-coding/src/bin/sim_runner.rs`
- `crates/gf2-coding/src/product/mod.rs`
- `crates/gf2-coding/src/grand/orbgrand.rs`
- `crates/gf2-coding/src/grand/sogrand.rs`
- `crates/gf2-coding/src/crc.rs`
