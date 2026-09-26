# Dense-parity baseline preparation

> **Diátaxis Type:** Research
>
> **Owning issue:** `c73ffa25`

The two gf2 baseline families use the committed
[dense-parity addendum](dense-parity-addendum.md) and its
[harness](dense-parity-harness.md). The frozen campaign JSON retains the
harness owner's `family.issue=e1f9a78f`; the
[benchmark queue](../1a379447-zen3-cpu-performance/bench-window/queue.tsv)
assigns collection to `c73ffa25`. The external M4RI comparison belongs to
`50f0bd42`.

The queued campaigns collect the isolated fused-entry and allocated public
`BitMatrix::matvec` identity baselines. The release harness
[validation record](survey/dense-baseline-validation.txt) covers the scalar
reference, the selected SIMD lane, canonical bit indexing, output length,
zero tail padding, and the boundary cases. The shared
[staged smoke](survey/dense-baseline-smoke.txt) exercises pause and resume with
zero timing samples.

## Release assembly

The [assembly generator](survey/disassemble-dense.sh) extracts symbols from
the same release arm binaries the campaign launcher builds. Its
[SIMD index](survey/asm/dense-baseline/simd/index.txt) and
[scalar index](survey/asm/dense-baseline/scalar/index.txt) record binary
digests, toolchain and exact symbol matches.

The [public entry](survey/asm/dense-baseline/simd/public-matvec.asm.txt)
checks the dimension contract and selects the SIMD route for the declared
anchor strides. The [SIMD body](survey/asm/dense-baseline/simd/simd-matvec.asm.txt)
allocates the output, calls through the bundle once per row, folds parity,
and sets output bits. The [bundle entry](survey/asm/dense-baseline/simd/bundle-entry.asm.txt)
jumps to the [AVX2 fused body](survey/asm/dense-baseline/simd/avx2-fused.asm.txt).
That body combines vector AND, nibble lookup and four accumulation lanes. The
[scalar build's public entry](survey/asm/dense-baseline/scalar/public-matvec.asm.txt)
contains its four-accumulator row loop. These observations identify candidate
cost centers; the window profile determines their shares.

## Profile attribution

The [profile launcher](survey/run-dense-profile.sh) uses the campaign's
canonical timed request encoder and existing `dense-arm`, with the frozen
windows and fixtures. It profiles representative warm isolated and allocated
cells, the allocated streaming cell, and the scalar reference. Its output
retains actual commands, arm results, hardware counters, sampled symbols,
annotated cycle samples, runtime host facts and a digest-bound append-only log.
The [summary generator](survey/summarize-dense-profile.py) reports medians
with order-statistic intervals over the repeated counter passes. The queued
profile requires both baseline receipts before it runs.

The [portfolio](plan.md) is frozen after the baseline receipt and current
profile establish a cost center. It names at most one distinct fusion within
the frozen complexity budget, or records that the profile supports no
candidate. The established fused route and generic carry-save result remain
the prior evidence cited by the addendum.
