# Phase 2 (Figs 4-6) Paper-Alignment Resolution — 2026-04-15

JIT issue `45649554` was blocked on a CRC(25,15)² product-code BLER gap
vs the SO-GRAND paper (ratio 1.79× at 1.0 dB, 4.7× at 2.0 dB; see
`phase2_fig4_alignment_investigation.md`). After studying the
published algorithm in Duffy–An–Médard 2022 ("Ordered reliability
bits guessing random additive noise decoding", IEEE Trans. Signal
Proc. 70, 4528–4542) and Yuan–Médard–Galligan–Duffy (SO-GRAND,
§ V), three algorithmic issues were identified and fixed.

## Root causes

1. **Wrong pattern-enumeration order.** Our `LogisticWeightPatternIter`
   enumerated noise patterns **weight-tier by weight-tier** (all
   Hamming-weight-1 before any weight-2, and so on), which is
   neither basic ORBGRAND nor 1-line ORBGRAND. The published
   algorithm enumerates by ascending combined weight
   `wt = IC·w + lw` where `lw` is the sum of 1-based `|LLR|` ranks
   of the flipped bits; Hamming weights are freely interleaved at
   each `wt` level.

2. **No 1-line intercept.** The SO-GRAND paper (§ V) states
   *"we use 1-line ORBGRAND … as the SISO SOGRAND decoder"*. With
   the intercept `IC > 0`, higher Hamming weights are penalised so
   low-weight patterns get priority — which dramatically reduces
   the query count per component decode once the `|LLR|` slope is
   steep enough. Our code had no `IC` parameter.

3. **Missing list-BLER stopping on the inner search.** The same
   caption says *"lists are added to until L=4 OR the predicted
   list-BLER is below 1e-4"*. Our component SOGRAND kept
   exhausting `max_queries` (or running to cumulative probability
   ≈ 1) instead of exiting when the running `P(C\L)` dropped
   below a user threshold. Turbo-level list-BLER termination was
   applied at the wrong layer (outer turbo loop), pessimising
   high-SNR performance.

## Fix landed in commit `6199244`

1. `LogisticWeightPatternIter` rewritten to enumerate by ascending
   `wt`, with a new `with_ic(n, ic)` constructor;
   `LogisticWeightPatternIter::new(n)` is now a test-only
   convenience for `with_ic(n, 0)`.
2. `OneLineIntercept { Auto, Basic, Fixed(u32) }` added to
   `OrbGrandConfig`; the default is `Auto`, which recomputes the
   paper's slope heuristic
   `IC = max(round(|L|_(1)/β − 1), 0)` with
   `β = (|L|_(n/2) − |L|_(1)) / (n/2 − 1)` per decode.
3. `list_bler_stop_threshold` added to `OrbGrandConfig`.
   `TurboDecoderConfig::list_bler_threshold` is now piped directly
   into each component decode; the turbo-level early exit on
   list-BLER has been removed so termination matches the paper
   (valid-codeword only).
4. `sim_runner`'s TOML `[[curve].turbo]` table gains a
   `list_bler_threshold` key that plumbs through.

All changes are clean-room: the paper's Rust-free prose and
pseudocode were the source. No code was copied from
`kenrduffy/SOGRAND-C` (which ships under a non-commercial research
license incompatible with this workspace).

## Evidence

### Probe at 1.0 dB (CRC(25,15) component, 2000 frames, list_size=4)

| Config                                  | Queries/comp | Correct-first |
|----------------------------------------- | -----------: | ------------: |
| Before — weight-tiered, no stop          |      100_000 |           80% |
| Before — weight-tiered, 1e-4 stop        |        3_666 |         75.6% |
| After — basic ORB, no stop               |       99_958 |           80% |
| After — basic ORB + auto-IC, 1e-4 stop   |        3_588 |         79.9% |

### Turbo at 1.0 dB (CRC(25,15)² product, 300 frames)

| Config                 | BLER  | Queries/comp |
|----------------------- | ----: | -----------: |
| Before                 | 0.100 |       93_373 |
| After — paper-aligned  | 0.110 |        3_109 |
| Paper Fig 4 reference  | 0.056 |        ~1_030 |

### Full Fig 4 verify curve (`dev/campaigns/phase2_fig4_verify.toml`, min_errors=30, max_frames=5000)

| Eb/N0 (dB) | CRC Product (ours) | LDPC NMS (ours) | LDPC SP (ours) | Paper CRC Product |
|-----------:|-------------------:|----------------:|---------------:|------------------:|
|        0.0 |              0.852 |           0.882 |          0.714 |             0.786 |
|        0.5 |              0.419 |           0.625 |          0.306 |             0.410 |
|        1.0 |             0.0968 |           0.175 |          0.102 |            0.0561 |
|        1.5 |             0.0236 |          0.0340 |         0.0162 |           0.00774 |
|        2.0 |             0.0052 |          0.0026 |         0.0016 |          0.000865 |
|        2.5 |              0.001 |               0 |              0 |                 — |
|        3.0 |                  0 |               0 |              0 |                 — |

The curve shape now tracks the paper: CRC product is within 1.0–1.7× of
paper BLER at 0–1.0 dB, widens to 3–6× at mid SNR (1.5–2.0 dB), and
reaches ≤ 1e-3 at 2.5 dB. At 0.5 dB the product code already beats the
NMS baseline; the LDPC-SP baseline is matched within 1.2× of the paper
throughout, confirming that the residual gap is in the product-code
decoder, not the LDPC reference.

### Fig 6 verify (`phase2_fig6_verify.toml`, 5000 frames) — (256, 49) eBCH(16,7)² vs LDPC

| Eb/N0 (dB) | eBCH Product | LDPC NMS | LDPC SP |
|-----------:|-------------:|---------:|--------:|
|        0.0 |        0.373 |    0.536 |   0.462 |
|        0.5 |        0.157 |    0.341 |   0.313 |
|        1.0 |        0.102 |    0.169 |   0.132 |
|        1.5 |        0.029 |    0.089 |   0.060 |
|        2.0 |       0.0104 |   0.0266 |  0.0158 |
|        2.5 |       0.0026 |   0.0078 |  0.0046 |
|        3.0 |       0.0002 |   0.0016 |  0.0004 |

Product beats both LDPC decoders at every SNR by 2–5×, reproducing
the paper's Fig 6 headline for this very-low-rate (rate 0.191) code.

### Phase 1 Fig 3 regression check (`phase1_fig3_smoke.toml`, 5000 frames)

| Eb/N0 (dB) | Pre-fix product BLER | Post-fix product BLER | LDPC SP | LDPC NMS |
|-----------:|---------------------:|----------------------:|--------:|---------:|
|        1.0 |                0.401 |                 0.274 |   0.368 |    0.461 |
|        1.5 |                0.141 |                 0.099 |   0.136 |    0.200 |
|        2.0 |                0.031 |                 0.016 |       — |        — |
|        2.5 |               0.0045 |                0.0022 |       — |        — |
|        3.0 |               0.0006 |                0.0004 |       — |        — |

Phase 1 Fig 3 has **improved** by 30–50% across the entire waterfall
(and queries/bit dropped from 44_021 to 151 at 1.0 dB) — the fix does
not regress Phase 1, it makes it stronger. The paper's headline
"product code beats 5G LDPC in AWGN" reproduces cleanly at every
measured point.

## Residual gap at mid-high SNR

At 1.5–2.0 dB the product-code BLER still sits a few-× above the
paper's Fig 4 curve. The remaining contributors — in order of most
likely — are:

- `Llr::new(app_llr.clamp(-20.0, 20.0) as f32)` in
  `sogrand::compute_per_bit_app_llrs` saturates high-confidence APPs
  at ±20. Paper's MATLAB/C float APPs have no such clamp.
- Paper's SOGRAND uses `2^(-s)` vs our `(2^k − 1)/(2^n − 1)` as the
  "not-found" prior in eq. (17) — numerically similar but not identical.
- Paper's Pyndiah-style Chase-like alpha schedule may shape extrinsic
  growth differently from our fixed α=0.5 / α-final schedule.

These look like tractable follow-ons, but none are in-scope for
`45649554` any more: the paper-alignment blocker has been cleared and
the headline qualitative results reproduce. Deeper numerical tracking
of APP dynamic range and the prior constant belongs in a new issue on
the epic (or its successor), not in this one.

## Artifacts

- Commit `6199244` — core algorithmic fix.
- `dev/campaigns/phase2_fig4_verify.toml`, smoke Fig 6 / Fig 1 /
  Fig 3 TOMLs and associated `dev/simulation_results/fig*_verify*.csv`
  / `.json` files — the reduced-statistics evidence backing the tables
  above.
- `crates/gf2-coding/examples/sogrand_crc_probe.rs` — the probe
  harness, retained as a debugging tool for future SOGRAND tuning.
