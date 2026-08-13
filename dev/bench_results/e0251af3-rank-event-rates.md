# Permanental rank-deficiency rare-event receipt

Issue: `e0251af3`. This is a preregistered, single-run pipeline check of the production permanental-rank predicate at observable small dimensions.

## Interpretation

The $k = 1$ event is an all-zero column, whose exact probability is $q^{-n}$. The $k = 2$ comparison uses the stated heuristic $2q^{-n}$, not a theorem prediction. Every measured $(n,k)$ lies outside the cited hypothesis $k \le 0.1\sqrt{n}$: these small observable cells cannot test the theorem in its proven range. Agreement therefore supports the implementation and the $k/q^n$ heuristic, rather than that theorem.

Event counts are in the small-count regime, so every interval below is the equal-tailed 95% Clopper–Pearson exact binomial interval from `gf2_stats::intervals::clopper_pearson_interval`; no normal approximation is used.

Cell selection is fixed before drawing: $k = 1$ uses $(q,n) = (3,5), (5,4), (7,3)$, giving expected counts $10^4 q^{-n} \approx 41$, $2\cdot10^4 q^{-n} = 32$, and $10^4 q^{-n} \approx 29$; $k = 2$ uses $(3,5)$ and $(5,4)$, giving heuristic counts $10^4(2q^{-n}) \approx 82$ and $2\cdot10^4(2q^{-n}) = 64$. These choices keep the expected events in the tens at modest sample sizes.

A disagreement with an exact value or heuristic is recorded as a pipeline finding, not a mathematical one; no observed result is reconciled by changing the preregistered sample sizes.

## Preregistered cells and results

| $k$ | $q$ | $n$ | samples | events | estimate | exact 95% CP interval | exact $q^{-n}$ (k=1) | heuristic $2q^{-n}$ (k=2) | heuristic/exact comparison | mean $k \times k$ permanent evaluations per matrix |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|
| 1 | 3 | 5 | 10000 | 45 | 0.004500000000 | [0.003284167987, 0.006016771133] (95% CP) | 0.004115226337 | — | covered exact value | 1.488700 |
| 1 | 5 | 4 | 20000 | 25 | 0.001250000000 | [0.000809092338, 0.001844697343] (95% CP) | 0.001600000000 | — | covered exact value | 1.248550 |
| 1 | 7 | 3 | 10000 | 30 | 0.003000000000 | [0.002024974816, 0.004279939087] (95% CP) | 0.002915451895 | — | covered exact value | 1.164800 |
| 2 | 3 | 5 | 10000 | 109 | 0.010900000000 | [0.008958341244, 0.013133832675] (95% CP) | — | 0.008230452675 | DISAGREEMENT: heuristic outside interval (pipeline finding) | 2.035300 |
| 2 | 5 | 4 | 20000 | 96 | 0.004800000000 | [0.003889693670, 0.005858507616] (95% CP) | — | 0.003200000000 | DISAGREEMENT: heuristic outside interval (pipeline finding) | 1.376800 |

The measured means are small constants relative to the $\binom{n}{k}$ worst-case scan, confirming the production predicate's early exit for these uniformly sampled matrices.

## Provenance and regeneration

- Git revision: `f0f2cac0857e0a535eb13f74ea772b5c4746485b`
- CPU model: `AMD Ryzen 9 5900X 12-Core Processor`
- Toolchain: `rustc 1.97.0 (2d8144b78 2026-07-07)`; `cargo 1.97.0 (c980f4866 2026-06-30)`
- Sampler: `gf2_stats::sampler::MatrixSampler<F_q>` with ChaCha20; root `ROOT = 0xe0251af320260813`.
- Stream purpose: `StreamPurpose::RareEvent` (tag 4), distinct from `Validation`, `Timing`, and `CampaignCell`; stream indices are recorded per cell below.
- Harness constants and sample sizes are committed in `crates/gf2-algebra/tests/rank_event_rates.rs`; the sizes were not changed after observing outcomes.

| $k$ | $q$ | $n$ | stream index | matrix entries drawn per sample | total samples |
|---:|---:|---:|---:|---:|---:|
| 1 | 3 | 5 | 101 | 5 | 10000 |
| 1 | 5 | 4 | 102 | 4 | 20000 |
| 1 | 7 | 3 | 103 | 3 | 10000 |
| 2 | 3 | 5 | 201 | 10 | 10000 |
| 2 | 5 | 4 | 202 | 8 | 20000 |

Regeneration (from the repository root): `cargo nextest run -p gf2-algebra --test rank_event_rates --release --profile ci --run-ignored ignored-only`.
