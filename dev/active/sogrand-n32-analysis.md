# SOGRAND Soft-Output Quality Analysis for n=32 Codes

**Date:** 2026-04-10
**Related issues:** 92086311 (Phase 1 AWGN sims), 3ce1cd12 (Phase 3 GLDPC sim)

## Summary

Alignment tests with max_queries=1M reveal that SOGRAND produces insufficient soft-output quality for n=32 component codes (dRM(32,21) and eBCH(32,26)), causing large performance gaps vs the reference paper. The n=16 code eBCH(16,11) in Fig 3 works correctly because the smaller search space allows SOGRAND to find the correct codeword reliably.

## Alignment Results

### Fig 1: dRM(32,21) product code (1024, 441) — 1M queries

| Eb/N0 | Ours | Paper SOGRAND | Ratio | avg_iter | queries/bit |
|-------|------|---------------|-------|----------|-------------|
| 0.00  | 1.000 | 0.962 | 1.04x | 13.9 | 3468 |
| 0.25  | 0.968 | 0.879 | 1.10x | 12.9 | 3023 |
| 0.50  | 0.882 | 0.656 | 1.34x | 9.1 | 1984 |
| 0.75  | 0.811 | 0.226 | 3.58x | 7.0 | 1530 |
| 1.00  | 0.667 | 0.072 | 9.27x | 6.6 | 1331 |
| 1.25  | 0.588 | 0.018 | 33.1x | 5.8 | 1108 |
| 1.50  | 0.411 | 0.002 | 236x  | 4.6 | 868 |

Key observation: avg_iterations ~6.6 at 1.0 dB is comparable to expected convergence speed. The turbo decoder converges but to wrong codewords 67% of the time.

### Fig 7: QC-GLDPC with eBCH(32,26) — 1M queries

| Eb/N0 | Ours | Paper GLDPC | Ratio | avg_iter |
|-------|------|-------------|-------|----------|
| 1.50  | 1.000 | 0.874 | 1.14x | 50 |
| 2.00  | 0.938 | 0.130 | 7.2x  | 48 |
| 2.50  | 0.312 | 0.003 | 121x  | 20 |
| 3.00  | 0.015 | 9e-6  | 1662x | 5.0 |

Key observation: avg_iterations=48 at 2.0 dB (near max 50) means BP is NOT converging, unlike the paper.

### Fig 3: eBCH(16,11) product code (256, 121) — MATCHES PAPER

Product code outperforms LDPC at all SNR points. LDPC SP matches paper's BP curve within 0.5-1.2x ratio.

## Root Cause Analysis

### Issue 1: ORBGRAND early termination limits soft-output quality (dRM)

ORBGRAND with `list_size=4` early-terminates after finding 4 codewords. For dRM(32,21) at 1.0 dB Eb/N0, this happens after ~3200 queries, covering roughly weight-0 through weight-3 noise patterns.

The problem: at 1.0 dB, typical noise patterns have weight 4-5. The **correct codeword is often NOT in the SOGRAND list** because the correct noise pattern hasn't been tested when the list fills up. The 4 found codewords are likely incorrect, producing misleading APP LLRs.

For eBCH(16,11) with n=16, the entire search space is 2^16=65K patterns. Even 100K queries covers everything. So the correct codeword IS found.

**Fix approach:** After finding `list_size` codewords, ORBGRAND should continue accumulating patterns (without adding to the list) up to max_queries. This ensures the cumulative probability is accurate even if the correct codeword is beyond the list. Additionally, `list_size` may need to be increased for n=32 codes.

### Issue 2: SOGRAND unsuitable as SISO check-node decoder (GLDPC)

The GLDPC decoder uses SOGRAND at check nodes to produce SISO (soft-in, soft-out) information. For eBCH(32,26) (rate 0.813), the cumulative probability coverage is too low to produce meaningful extrinsic information, even with 1M queries. The "not found" probability P(C\L) dominates the APP computation, making extrinsic ~ 0.

The reference paper's GLDPC results (avg_guesses ~59 per check node, converging in ~6.6 iterations) strongly suggest it uses a **BCJR (trellis-based APP) decoder** at check nodes, not SOGRAND. BCJR produces exact APP LLRs without needing cumulative probability coverage.

### Issue 3: No BP message damping in GLDPC decoder

The GLDPC BP decoder passes raw extrinsic from check-to-variable nodes without damping. The product code turbo decoder uses `alpha=0.5` scaling. Without damping, BP messages oscillate, especially when extrinsic quality is poor.

### Issue 4: even_code flag dropped in parallel sim_runner path

`sim_runner.rs` line 399: the parallel GLDPC path drops the `even_code` flag from the SOGRAND config, defaulting to `false`. This wastes half the query budget on impossible patterns for eBCH codes. **Fixed in this session.**

## Recommended Fixes

### For 92086311 (Fig 1 product code):

1. **Modify ORBGRAND to continue pattern accumulation after list is full.** Currently, ORBGRAND stops when `list_size` codewords are found. It should continue up to `max_queries` to accumulate cumulative probability, which is needed for accurate P(C\L) and thus accurate APP LLRs. The list stays at `list_size` entries (no memory growth), but cum_prob keeps increasing.

2. **Increase list_size for n=32 codes.** `list_size=4` may be insufficient for dRM(32,21). Try 8 or 16.

3. **Re-run alignment test at 0.5-1.0 dB** to verify improvement.

### For 3ce1cd12 (Fig 7 GLDPC):

1. **Add BP message damping (alpha=0.5-0.8)** on check-to-variable messages in `GldpcDecoder`.

2. **Apply the same ORBGRAND accumulation fix** from above to improve SOGRAND soft-output at check nodes.

3. **If still insufficient:** Implement BCJR component decoder as alternative to SOGRAND. This is a larger effort but may be necessary for competitive performance.

4. **Add LLR saturation** (clamp to +/-25) in variable node update to prevent belief explosion.

## Verification Plan

After each fix, run single-SNR alignment sims at 1.0 dB (Fig 1) and 2.0 dB (Fig 7) with min_errors=30, max_frames=200 to verify improvement before full campaign runs.
