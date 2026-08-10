# Reclassified four-matrix $\mathbb{F}_3$ AVX2 cohort

This document reclassifies the 2026-08-10 four-matrix F_3 timing cohort as a
**non-authoritative provenance-incomplete attempt**. It is not a measurement
receipt and must not be used to claim a rate, speedup, backend ordering,
dispersion, or conclusion about the four-lane expectation.

## Why it is not authoritative

The harness used for this cohort recorded `source_dirty=false` from a partial
`git status --porcelain` query: it selected only a small set of tracked paths
and explicitly ignored untracked files. Therefore that flag cannot support the
clean-source assertion required for a permanent measurement artifact, even
though every retained row records revision
`5b15d2723fcede3ccc082e508d1223c2f54087ce` and `source_dirty=false`.

The raw rows are retained unchanged at
[`batched-f3-avx2-provenance-incomplete-2026-08-10.csv`](batched-f3-avx2-provenance-incomplete-2026-08-10.csv).
Their SHA-256 remains
`f6c5eb673e982f002ab71002f4310fc7db7f31b320850a70bd2526e2248742ee`.
No row was discarded, rewritten, pooled, or remeasured during this
reclassification. Its recorded hardware, seed, lock/affinity, toolchain, and
binary information remain historical metadata only, not sufficient provenance
for interpreting the timings.

## Fixed-harness successor

The corrected harness derives the same `source_dirty` field from the canonical
full-repository `git status --porcelain --untracked-files=all` output, after
opening its output. A new in-repository output would therefore mark the source
dirty; the next cohort will instead write first to a unique absent `/tmp` file.

The fixed-harness successor completed the locked five-execution protocol from
clean revision `88474a74ceee817040327db164c21f9fdd5ccf84`. Its raw bytes and
interpretation are separately recorded in
[`batched-f3-avx2-provenance-fixed.md`](batched-f3-avx2-provenance-fixed.md);
that receipt does not rehabilitate this incomplete cohort.

The previously preserved source-split attempt remains independently
non-authoritative: [interrupted-attempt record](batched-f3-avx2-interrupted-2026-08-10.md).
