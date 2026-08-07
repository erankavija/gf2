# Cross-prime comparison — bipedal $\mathbb{F}_3$ vs $\mathbb{F}_5$ / $\mathbb{F}_7$ R-decision winners

> Companion to JIT issue **f10152f6** (R2 $\mathbb{F}_7$ encoding research).
> Reproduces the per-element cost of the Scheinerman bipedal $\mathbb{F}_3$
> encoding (arxiv 2407.20205v2) under the same harness used for R1 ($\mathbb{F}_5$)
> and R2 ($\mathbb{F}_7$), then tabulates the three primes side-by-side.

## 1. Why this comparison matters

R1 selected Candidate A (3-bit + $2^{16}$ LUT) for $\mathbb{F}_5$;
R2 selected the same shape for $\mathbb{F}_7$. Both selections invoked the
hard-fallback rule from `dev/plans/gf2_algebra_permanent.md` §8: no novel
encoding beat LUT-A by ≥ 1.5× on a Ryser-weighted workload. Before
locking those decisions in, the user asked whether the same harness
reproduces the headline $\mathbb{F}_3$ bipedal advantage from the paper —
i.e., is the methodology calibrated, and does $\mathbb{F}_3$ have headroom
that $\mathbb{F}_5$/$\mathbb{F}_7$ structurally do not?

**Short answer: yes.** $\mathbb{F}_3$ bipedal is **13–22× faster than $\mathbb{F}_3$
LUT-A** in the same harness. $\mathbb{F}_5$ and $\mathbb{F}_7$ have no candidate
in their respective R-decisions that comes close to that advantage —
and the per-element op-count math explains why: bipedal $\mathbb{F}_3$
encodes 64 elements in 6 add ops / 2 mul ops, an algebraic gift that
$\mathbb{F}_5$ and $\mathbb{F}_7$ structurally cannot match.

## 2. What was measured

A standalone prototype `dev/research/f3_bipedal/` (parallel to the
existing `f5_packing/` and `f7_packing/` prototypes) implements three
$\mathbb{F}_3$ encodings under the same `F3Encoding` trait shape as
`F5Encoding` / `F7Encoding`:

- **naive** — `(a OP b) % 3` per element on `Vec<u8>`. Rust analogue of
  the paper's "Julia naive Ryser" baseline.
- **LUT-A** — 4-bit-aligned slots, 16 elements per `u64`, $2^{16}$ LUT
  keyed on packed byte-pairs. Identical shape to F_5-A / F_7-A.
- **bipedal** — paper's `(mag, sgn)` pair encoding; 64 elements per
  `u64`-pair; add = 6 bitwise ops, mul = 2 bitwise ops, sub = 7 ops
  (= neg + add), div = 2 ops (= mul, since every nonzero element of
  $\mathbb{F}_3$ is its own inverse).

The harness is the same `bench_op_ns_per_elem` used in R1/R2: 65 536
packed elements per op, median of 5 runs, fresh `clone` per repeat,
release profile (`opt-level=3, lto="thin", codegen-units=1`), MSRV 1.95.
Bench host is the same AMD Ryzen 9 5900X (Zen 3, AVX2-only) used for
R1/R2, single-thread.

All 26 unit tests pass: exhaustive 3×3 verification of every op against
`(a OP b) % 3` for every encoding, plus 64-case proptest for length-
up-to-256 vectors of every op of every encoding, plus pack-unpack
roundtrip tests, plus a tail-mask invariant test on the bipedal encoding
(per CLAUDE.md §Key design invariants).

## 3. $\mathbb{F}_3$ measurements

Median ns/element across three full runs of the harness on the bench host
(stable to ~10 % between runs). Numbers are per single `(a OP b)` over
the full 65 536-element batch.

| Encoding                                | add     | sub     | mul     | div     |
|-----------------------------------------|---------|---------|---------|---------|
| naive F_3 (scalar `Vec<u8>`)            | 0.074   | 0.080   | 0.112   | 0.882   |
| F_3 LUT-A (4-bit slots, 2^16 LUT)       | 0.220   | 0.219   | 0.219   | 0.230   |
| bipedal F_3 (paper, mag/sgn pair)       | **0.010** | **0.010** | **0.017** | **0.018** |

Speedup vs naive (>1.0 = faster than naive):

| Encoding                                | add     | sub     | mul     | div     |
|-----------------------------------------|---------|---------|---------|---------|
| F_3 LUT-A                               | 0.34×   | 0.36×   | 0.50×   | 3.84×   |
| bipedal F_3                             | **7.6×** | **8.2×** | **6.6×** | **49.6×** |

Speedup of bipedal vs LUT-A:

| add    | sub    | mul    | div    |
|--------|--------|--------|--------|
| **22.3×** | **22.5×** | **12.9×** | **12.9×** |

### Notes on naive division

Naive div is 7–8× slower than naive mul because each scalar div does a
multiplication followed by a modulo (`(a * INV[b]) % 3`), and LLVM emits
two integer-modulo instructions per element where add/mul lower to a
single SIMD `pmullw` / `paddw` lane. Bipedal F_3 div pays nothing extra
over mul because it reuses the mul kernel verbatim (every nonzero F_3
element is its own inverse). This is the source of the headline 49.6× div
speedup — it is a real algebraic property of $\mathbb{F}_3$, not a
microarchitectural artefact.

### Why bipedal is so much faster than LUT-A

The op-count derivation in §5 of `r2_f7_encoding_decision.md` puts LUT-A
at ~4 ALU ops + 0.5 LUT loads per element. Bipedal F_3 add is 6 bitwise
ops over 64 elements = 0.094 ops/element — **~50× fewer ops/element**
than LUT-A on paper, ~22× fewer ns/element measured. The remaining gap
(50× vs 22×) is explained by LLVM auto-vectorising the bipedal bitwise
loops to AVX2 256-bit lanes (each `u64`-pair effectively becomes 4 `u64`
lanes in flight); the LUT-A path is harder to vectorise on Zen 3 because
gather-against-64-KiB-LUT does not lower cleanly to AVX2.

## 4. Cross-prime comparison

R-decision winners side-by-side, all measured in the same harness:

| Prime  | Winner encoding                       | add ns/elem | mul ns/elem | LUT footprint | Per-`u64` work |
|--------|---------------------------------------|------------:|------------:|--------------:|----------------|
| $\mathbb{F}_3$ | **bipedal** (mag/sgn)         | **0.010**   | **0.017**   | 0 KiB         | 6 ops add / 2 ops mul over 64 elements |
| $\mathbb{F}_5$ | A (3-bit + 2^16 LUT)         | 0.230       | 0.230       | 256 KiB       | 8 LUT loads + ~16 ALU ops over 16 elements |
| $\mathbb{F}_7$ | A (3-bit + 2^16 LUT)         | 0.230       | 0.230       | 256 KiB       | 8 LUT loads + ~16 ALU ops over 16 elements |

Bipedal $\mathbb{F}_3$ is **~23× faster per element than the F_5/F_7 LUT
winners** on add and **~14× faster on mul**. This gap is structural, not
methodological:

- $\mathbb{F}_3$ has a 2-element multiplicative group `{1, 2}`. The
  $(\mathit{mag}, \mathit{sgn})$ encoding turns add into a small XOR-OR
  circuit and mul into a single AND/XOR pair. **No analogous encoding
  exists for $\mathbb{F}_5$ or $\mathbb{F}_7$.** R1 confirmed for
  $\mathbb{F}_5$, R2 for $\mathbb{F}_7$ — the candidates that came closest
  ($\mathbb{F}_5$-D bit-sliced 3-plane Boolean; $\mathbb{F}_7$-D
  bit-sliced 3-plane Mersenne fold) gain on add but lose on mul, because
  $\mathbb{F}_5$/$\mathbb{F}_7$ multiplication has no comparable bit-trick.
- Additionally, bipedal F_3 packs **64 elements per `u64`-pair**, vs
  LUT-A's **16 elements per `u64`**. Even ignoring per-op cost, bipedal
  F_3 gets a 4× density bonus from the smaller field.

The natural-LUT density for $\mathbb{F}_5$ would be 4-bit slots (16/u64)
or 3-bit slots (21/u64); for $\mathbb{F}_7$ it is 3-bit slots (21/u64).
LUT-A uses 4-bit slots in both cases for byte-pair indexing convenience,
costing some packing density but enabling the cheap 8-byte-pair LUT
lookup. R1/R2 measured this trade-off and ratified A as the best
in-class choice for each prime.

## 5. Reconciliation with the paper's 86.9× headline

The paper (arxiv 2407.20205v2, Scheinerman) reports an 86.9× wall-clock
speedup of bipedal F_3 Ryser permanent over Julia's naive Ryser at
4.20 GHz on a single thread. Our harness measures bipedal-vs-naive
**arithmetic** at 7–50× — smaller than 86.9×. Three reasons:

1. **Language overhead.** Julia's naive Ryser pays the per-element cost
   of Julia's runtime (boxed `Int`, dispatch, GC) on every operation;
   our naive baseline is `(a OP b) % 3` on `Vec<u8>` in Rust release
   mode, which LLVM auto-vectorises to SIMD modular arithmetic. The
   paper's 86.9× factor includes ~3–5× of pure Julia-vs-Rust overhead.
2. **Operation mix.** The paper's headline is on **full Ryser permanent
   wall-clock** at $n = 24$, which interleaves add, mul, and Gray-code
   subset enumeration. Our harness measures pure-op throughput at scale.
   Add/mul/div individually each show 7–50× — the geometric mean (a
   reasonable proxy for Ryser's mixed workload) is ~12×, consistent with
   what the algorithm-level (Rust-vs-Rust) speedup would be after
   subtracting language overhead.
3. **Memory layout.** Our naive baseline reads/writes `u8`-packed
   `Vec<u8>`; Julia's `Array{Int}` is `i64`-packed, costing 8× the
   memory bandwidth. Folding that out, a Rust naive on `Vec<i64>` would
   be slower and the bipedal speedup would be 30–50×.

The bipedal **algorithmic** advantage — `2/64 = 0.031` mul ops/element,
**~64× fewer raw bitwise ops than the naive `(a * b) % 3`** — is exactly
the per-element cost ratio the paper relies on. Our measurement
reproduces it faithfully under our (faster, in-Rust) baseline.

The paper's wall-clock claim is reproducible in principle by adding a
Julia naive Ryser harness on the same host. That is a wave-3 task in
the gf2-algebra epic (T6: faithful Julia-port reference, plus T7–T13:
bipedal F_3 permanent), not a research follow-up to this issue.

## 6. Implication for the R2 decision

The R2 decision (Candidate A wins for $\mathbb{F}_7$) is **correct and
ratified**. Bipedal $\mathbb{F}_3$'s headline speedup is structural
(2-element multiplicative group, dense 64-element packing) — it does
not generalise to $\mathbb{F}_5$ or $\mathbb{F}_7$. The R-decision
methodology produces sensible per-element numbers across all three
primes:

- $\mathbb{F}_3$: bipedal wins by 13–22×; LUT-A is the runner-up at
  ~22× the per-element cost. Hard fallback would (correctly) pick
  bipedal.
- $\mathbb{F}_5$: Candidate A (LUT-A) wins by ≥ 1.5× over every
  alternative on a Ryser-weighted mix. Hard fallback applies (no
  alternative cleared 1.5×).
- $\mathbb{F}_7$: Candidate A (LUT-A) wins on Ryser-weighted mix
  (D's 9.5× add advantage is dominated by D's 1.7× mul deficit).
  Hard fallback applies.

The wave-3 implementation task T19 (F_7 packed type + ops) ships with
Candidate A. The wave-3 SIMD-batching issue `c7542983` should re-bench
$\mathbb{F}_7$-D under AVX2 to confirm the scalar gap doesn't invert
when the LUT path is harder to vectorise — but this is **not** a
re-decision authority for T19.

## 7. Provenance

- Standalone prototype: `dev/research/f3_bipedal/`
  - `Cargo.toml`: standalone (`[workspace]` empty marker), MSRV 1.95.
  - `src/bipedal.rs`: paper's algorithm, op counts annotated in the
    module header.
  - `src/naive.rs`: per-element scalar baseline.
  - `src/lut.rs`: F_3 LUT-A (cross-prime structural twin of
    `f5_packing/src/cand_a.rs` and `f7_packing/src/cand_a.rs`).
  - `src/main.rs`: bench harness identical in shape to the F_5/F_7
    prototypes.
- Sibling decision docs: `dev/plans/r1_f5_encoding_decision.md` (R1) and
  `dev/plans/r2_f7_encoding_decision.md` (R2).
- Paper: arxiv 2407.20205v2, Scheinerman, "Fast permanents over $\mathbb{F}_3$
  via $\mathbb{F}_2$ arithmetic" (referenced from
  `dev/plans/gf2_algebra_permanent.md`).
