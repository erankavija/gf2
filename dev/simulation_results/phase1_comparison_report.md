# Phase 1 Comparison Report: Product Codes vs 5G NR LDPC

## Canonical sources

- Campaigns: `dev/campaigns/phase1_fig1.toml`, `dev/campaigns/phase1_fig3.toml`
- Results: `dev/simulation_results/fig1_*.{csv,json}` and `dev/simulation_results/fig3_*.{csv,json}`
- Archived run log only: `dev/simulation_results/phase1_final/fig3_sogrand_stdout.log`

The repository now keeps a single authoritative Phase 1 acceptance
artifact set in `dev/simulation_results/`. The duplicate checked-in
Phase 1 campaign/result files were removed. Quick/alignment helper
campaigns in `dev/campaigns/` still exist for exploratory reruns, but
they are not part of the acceptance artifact set.

## Figure 1: dRM(32,21)^2 product code vs LDPC

Source files: `dev/simulation_results/fig1_drm_product.csv`,
`dev/simulation_results/fig1_ldpc_sp.csv`, `dev/simulation_results/fig1_ldpc_nms.csv`

| Eb/N0 (dB) | Product BLER | frame errors | LDPC SP BLER | LDPC NMS BLER | Paper LDPC BP | Paper SOGRAND |
|-----------|--------------|--------------|--------------|---------------|---------------|---------------|
| 0.50      | 0.824        | 122          | 0.4975       | 0.7299        | 0.442         | 0.656         |
| 0.75      | 0.532        | 117          | —            | —             | 0.210         | 0.226         |
| 1.00      | 0.202        | 108          | 0.0858       | 0.2336        | 0.074         | 0.072         |
| 1.25      | 0.0493       | 104          | —            | —             | 0.019         | —             |
| 1.50      | 0.0073       | 101          | 0.0037       | 0.0160        | 0.00367       | 0.00174       |
| 1.75      | 0.00043      | 86           | —            | —             | 0.000641      | 0.000190      |
| 2.00      | 0.000035     | 7            | 0.00013      | 0.00037       | 0.0000777     | 0.0000177     |

Our LDPC SP baseline still matches the paper's LDPC BP closely at the
reference points we measured (0.0858 vs 0.074 at 1.0 dB, 0.0037 vs
0.00367 at 1.5 dB).

For the checked-in baselines, the dRM product code is still worse than
our LDPC SP at 1.5 dB and better than the saved SP baseline by 2.0 dB.
The checked-in data therefore brackets the crossover between 1.5 and
2.0 dB, consistent with the paper's Figure 1 trend.

## Figure 3: eBCH(16,11)^2 product code vs LDPC

Source files: `dev/simulation_results/fig3_ebch_product.csv`,
`dev/simulation_results/fig3_ldpc_sp.csv`, `dev/simulation_results/fig3_ldpc_nms.csv`

| Eb/N0 (dB) | Product BLER | queries/bit | LDPC SP BLER | LDPC NMS BLER | Paper LDPC BP | Paper SOGRAND |
|-----------|--------------|-------------|--------------|---------------|---------------|---------------|
| 0.50      | 0.791        | 87.6K       | 0.617        | 0.758         | 0.656         | 0.551         |
| 1.00      | 0.401        | 44.0K       | 0.368        | 0.461         | 0.329         | 0.301         |
| 1.50      | 0.1405       | 16.9K       | 0.1362       | 0.1996        | 0.146         | 0.097         |
| 2.00      | 0.0309       | 6.3K        | 0.0341       | 0.0716        | 0.045         | 0.017         |
| 2.50      | 0.0045       | 3.0K        | 0.0073       | 0.0105        | 0.0075        | 0.0035        |
| 3.00      | 0.000603     | 1.8K        | 0.000883     | 0.001300      | 0.001         | 0.00052       |
| 3.50      | 0.00006      | 1.1K        | 0.000072     | 0.00009       | 0.0000816     | 0.0000678     |

Relative to the checked-in 5G NR baselines, the eBCH product code is:

- worse than LDPC SP at 1.5 dB (0.1405 vs 0.1362)
- better than LDPC NMS already at 1.0 dB
- better than both LDPC SP and LDPC NMS from 2.0 dB onward

Relative to the paper's LDPC BP curve, the product code is already
slightly better at 1.5 dB (0.1405 vs 0.146).

## Decoder configuration

- Fig 1 dRM turbo: GPU BCJR (exact APP via trellis forward-backward).
  BCJR is used because it is orders of magnitude faster for `n=32` than
  the current SOGRAND setup. The repository contains a diagnostic
  comparison against SOGRAND extrinsic behavior, but not an equality
  proof, so `queries/bit` remains 0 for the Figure 1 product curve.
- Fig 3 eBCH turbo: SOGRAND with 1-line ORBGRAND, `L = 4`, and
  `max_queries = 100K`, which preserves the paper's queries/bit metric.
- `alpha = 0.5`, `I_max = 20`, early termination on valid codeword
- dRM component: `(32,21,6)` via `DrmCode::drm_32_21()`
- eBCH component: `ExtendedBchCode::ebch_16_11()`
- LDPC baselines: 5G NR LDPC sum-product and normalized min-sum, `I_max = 50`

Acceptance criterion wording: collect at least 100 frame errors per SNR
point where that is achievable within the configured frame cap. Points
whose measured BLER drops below roughly `100/max_frames` are expected to
be frame-cap limited; the low error counts at those deep-waterfall
points are therefore documented rather than treated as missing data.

## Conclusion

The checked-in Phase 1 artifacts now tell one consistent story:

- Figure 1: the dRM product code lags the saved LDPC SP baseline at
  1.5 dB and is better by 2.0 dB, so the checked-in data brackets the
  crossover between 1.5 and 2.0 dB.
- Figure 3: the eBCH product code beats the saved LDPC SP baseline from
  2.0 dB onward and beats the saved LDPC NMS baseline earlier.
- Both figures remain qualitatively aligned with the paper's headline
  claim that product-code decoding can outperform the corresponding 5G
  NR LDPC baselines in the waterfall region.
