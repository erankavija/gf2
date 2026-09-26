# Logical-buffer candidate portfolio (jit:bc091474)

This portfolio is frozen before candidate code, pilot samples, or a new receipt. It applies the [frozen logical-buffer addendum](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md) and [protocol version 4](../f547c394/protocol.md) to the accepted [baseline and profile](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-baselines.md). The addendum, protocol, and baseline receipts retain their existing bytes.

## Profile decision

The [profile's sampled-symbol rows](../../bench_results/2037941f/logical-profile/profile-summary.md#sampled-symbol-shares) show AVX2 XOR work in both the isolated and public-row eight-word routes; its [memory-traffic rows](../../bench_results/2037941f/logical-profile/profile-summary.md#memory-traffic) show that the 64-word streaming route has a different, memory-heavy cost pattern. The [release assembly](../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/logical-arm/avx2-xor-into.asm.txt) identifies the existing AVX2 XOR body. These observations warrant a bounded short-loop study, but they do not themselves show a candidate speedup.

The unroll portfolio contains exactly two identities, in order: `avx2-xor-unroll-2` and `avx2-xor-unroll-4`. Factor 8 is eliminated before a pilot: eight and nine words contain only two complete AVX2 vectors, while the longer streaming route has the recorded memory pressure. The two factors use the same public dispatch and row access path as the identity baseline, each in a separate conservative-portable Rust 1.95 release executable. A compile-time experimental `--cfg` chooses the candidate body; ordinary builds retain the current body. This creates no runtime dispatcher, target-CPU override, public API, allocation, dependency, or production selection.

The [NR construction profile rows](../../bench_results/2037941f/logical-profile/profile-summary.md#sampled-symbol-shares) and [baseline conclusion](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-baselines.md#coding-route-logical-baseline-req-07-req-08-req-09) give no consumer-specific resolver-hoist signal for the permitted eight-word constructor. The NR route therefore has **no candidate** and no new pilot or confirmation. The preserved [64-word generic-hoist no-win](../04b85d10/findings.md) remains excluded. Already-hoisted M4RM, RREF, Gauss inversion, Strassen, and dense-multiply loops remain excluded.

## Fixed cells and measurement order

Each factor receives one exploratory pilot in each of the two frozen families, in this order: isolated factor 2, public row factor 2, isolated factor 4, public row factor 4. Each pilot uses all 17 family cells, in the order projected by the existing harness:

| Family | Cells |
|---|---|
| Isolated XOR | `xor-{8,9,63,64,65}w-a64-warm`; `xor-{7,66}w-a64-warm`; `xor-{8,9,63,64,65}w-o8-warm`; `xor-{8,9,63,64,65}w-a64-streaming` |
| Public row XOR | `row-xor-{8,9,63,64,65}w-full-warm`; `row-xor-{7,66}w-full-warm`; `row-xor-{8,9,63,64,65}w-tail63-warm`; `row-xor-{8,9,63,64,65}w-full-streaming` |

The existing identity pilot counts as one exploratory trial per cell; these two factors bring each cell to three, below the frozen cap of four. Resolved-function attribution stays exploratory at eight and nine words and is not repeated here. The two pilot variants share the addendum's fixed 24 paired executions, five 100 ms windows per warm or streaming execution, and other frozen cache, seed, timeout, and host settings. Every timed run goes through the benchmark window and the shared runner. Untimed oracle and runner smoke precede it.

After both pilots, select at most one identity by this rule. For each allowed dispatch band, `{8,9}` or `{63,64,65}`, a factor is eligible only if every warm, aligned anchor in that band has a point-estimate speedup above 1 in **both** families and the immediately adjacent anchor outside the band has a point estimate at least 1 in both families. For each eligible factor-band pair, score the smallest point estimate among those two families' in-band anchors. Select the pair with the largest score; break an exact score tie in favor of factor 2, then the `{8,9}` band. Point estimates only choose a confirmation target; they never establish a qualifying result. If no pair is eligible, record no candidate and stop without confirmation.

The selected factor receives exactly one confirmatory attempt in each applicable family. Each attempt includes the five frozen warm, aligned anchors, giving the required in-band and immediately adjacent out-of-band checks under the addendum's first-attempt comparison limit. The families retain their separate append-only ledgers. An unstable, inconclusive, or regressed confirmation is terminal under this protocol. There is no adaptive extra sample, alternative factor, or second confirmation.

## Acceptance and code bounds

The [addendum's effect and complexity rules](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md#effect-resolution-and-complexity-rules) govern the decision: isolated worthwhile speedup 1.05 and equivalence 1.03; public row worthwhile 1.03 and equivalence 1.02. Both bands may qualify if the selected factor satisfies each band's required cells. Pilot outcomes cannot authorize adoption. A qualifying confirmation yields a recommendation for a later production-selection issue, not a production change in this issue. All negative, not-material, unstable, inconclusive, unavailable, and no-candidate rows remain visible.

Candidate source is confined to the existing `gf2-kernels-simd/src/x86/avx2.rs` XOR body, with at most one new private target-feature XOR function and 80 added nonblank production lines. The implementation uses Rust 1.95-supported AVX2 intrinsics, keeps the scalar fallback, and observes the unsafe pointer bounds and target-feature contract. The shared XOR and row oracle must agree at logical bit lengths 0, 1, 63, 64, and 65, at aligned and eight-byte-offset word layouts, and across the 7/8/9/63/64/65/66-word and full/tail63 matrix cases. The candidate build must pass those semantic checks before any smoke or queued measurement.

The candidate plan uses the canonical harness and runner, with an explicit executable and Rust flags for each compared arm. The producing-input closure and receipt pin all source and executable bytes. The separate ISA-L gap arm remains the comparator worker's responsibility and cannot authorize this kernel candidate.
