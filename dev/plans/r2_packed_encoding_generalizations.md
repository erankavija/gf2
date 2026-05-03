# Packed encoding generalizations — cross-prime conceptual map

> Companion to JIT issue **f10152f6** (R2 $\mathbb{F}_7$ encoding research).
> Captures the conceptual structure underlying the R-decisions for
> $\mathbb{F}_3$, $\mathbb{F}_5$, $\mathbb{F}_7$, with empirical extension
> to $\mathbb{F}_{17}$ as a test of the prediction. Used to motivate the
> revised epic §8 fallback rule and the relationship to existing
> `gf2-core` machinery.

## 1. Two algebraic gifts

Every per-element op-count win we measured comes from one of two
specific algebraic properties of the prime $p$:

### Gift 1 — Fermat-like: $p - 1 = 2^m$

If $\mathbb{F}_p^*$ has order $2^m$, the $(\mathit{zero}, \log)$ encoding's
mul reduces to **bit-parallel $m$-bit log addition with no conditional
subtract** (truncated-carry add = mod $2^m$ for free). Available primes:
$p \in \{3, 5, 17, 257, 65537\}$ — the primes where $p - 1$ is a pure
power of two. Candidate B's domain.

### Gift 2 — Mersenne: $p = 2^k - 1$

If $p$ is Mersenne, the **Mersenne fold** $x \bmod p = (x \mathrel{\&} p) + (x \gg k)$
collapses bit-parallel addition into a constant-cost reduction step.
Available primes: $p \in \{3, 7, 31, 127, 8191, \ldots\}$. Candidate D's
"fold" advantage.

### F_3 is the unique double-gift prime

$\mathbb{F}_3$ is the only prime that gets **both** gifts:
$3 = 2^2 - 1$ (Mersenne) **and** $3 - 1 = 2^1$ (Fermat-like). Bipedal
$\mathbb{F}_3$ is what $(z, \log)$ and Mersenne fold collapse to when
both apply simultaneously: 1 bit of zero-flag (`mag`), 1 bit of log =
sign (`sgn`), addition uses Mersenne fold over the $(\mathit{mag}, \mathit{sgn})$
representation. The 22× headline isn't methodological — it's a unique
structural property. **It does not generalize to any other prime.**

## 2. Cross-prime map

| $p$    | Mersenne (cheap add) | Fermat-like (cheap mul) | Profile |
|--------|---------------------|-------------------------|---------|
|  3     | ✓ (k=2)             | ✓ (m=1)                | both — bipedal sweeps |
|  5     | ✗                   | ✓ (m=2)                | mul cheap, add per-element |
|  7     | ✓ (k=3)             | ✗ (p−1 = 6)            | add cheap, mul O(p²) |
| 11     | ✗                   | ✗                        | no gift — LUT only |
| 13     | ✗                   | ✗                        | no gift — LUT only |
| **17** | ✗                   | ✓ (m=4)                | mul cheap, add per-element |
| 19     | ✗                   | ✗                        | no gift — LUT only |
| 31     | ✓ (k=5)             | ✗ (p−1 = 30)           | add cheap, mul prohibitive |
| 257    | ✗                   | ✓ (m=8)                | LUT slot ≥ 9 bits — see §6 |

## 3. Empirical confirmation across measured primes

All numbers in ns/element, median of 5 runs, AMD Ryzen 9 5900X
(Zen 3, AVX2-only), single-thread, release profile, MSRV 1.95.
65 536 elements per op.

| Prime | Encoding | add | sub | mul | div |
|-------|----------|----:|----:|----:|----:|
| $\mathbb{F}_3$ | naive `Vec<u8>` | 0.074 | 0.080 | 0.112 | 0.882 |
| $\mathbb{F}_3$ | LUT-A           | 0.220 | 0.219 | 0.219 | 0.230 |
| $\mathbb{F}_3$ | **bipedal**     | **0.010** | **0.010** | **0.017** | **0.018** |
| $\mathbb{F}_5$ | LUT-A (winner R1 → revised → D) | 0.230 | 0.230 | 0.230 | 0.230 |
| $\mathbb{F}_5$ | B (z, log)      | 10.3  | 10.0  | 0.015 | 0.014 |
| $\mathbb{F}_5$ | **D bit-sliced** | **0.19** | **0.19** | **0.21** | **0.21** |
| $\mathbb{F}_7$ | **A LUT (winner R2)** | **0.230** | **0.230** | **0.230** | **0.230** |
| $\mathbb{F}_7$ | B (z, log)      | 8.43  | 8.39  | 0.020 | 0.020 |
| $\mathbb{F}_7$ | D Mersenne fold | 0.020 | 0.020 | 0.380 | 0.390 |
| $\mathbb{F}_{17}$ | naive `Vec<u8>` | 0.073 | 0.080 | 0.090 | 0.918 |
| $\mathbb{F}_{17}$ | LUT-A (8-bit slot) | 0.488 | 0.481 | 0.477 | 0.479 |
| $\mathbb{F}_{17}$ | **B (z, log) mod 16** | 7.86 | 8.18 | **0.037** | **0.025** |

The pattern matches §2 exactly:

- $\mathbb{F}_3$: both gifts → bipedal wins by 22× over LUT.
- $\mathbb{F}_5$: Fermat-like only → B's mul is 15× faster than A; B's add
  is 50× slower → B loses overall under any reasonable workload weighting.
  **D bit-sliced wins by uniform small margin (1.10–1.20×) over A.**
- $\mathbb{F}_7$: Mersenne only → D's add is 9.5× faster than A; D's mul
  is 1.7× slower than A → D loses on Ryser. A wins.
- $\mathbb{F}_{17}$: Fermat-like only ($m = 4$, the cleanest available) → B's
  mul is 13× faster than A; B's add is 16× slower than A. **Under
  Ryser-weighted load (mul-heavy), B wins decisively** — see §4.

### Why F_17 LUT-A is 2× slower than F_5/F_7 LUT-A

$\mathbb{F}_5$ and $\mathbb{F}_7$ fit in 4-bit slots, so LUT-A there packs
**16 elements per `u64`**. $\mathbb{F}_{17}$ needs ≥ 5 bits per element;
the natural variant is 8-bit slots packing **8 elements per `u64`**.
That halves the throughput per LUT load, cleanly visible in the 0.48
vs 0.23 ns/elem result. This is structural, not methodological — any
$p > 16$ pays the same density penalty for the LUT path.

## 4. Workload-weighted analysis for $\mathbb{F}_{17}$

Gray-code Ryser at $n = 36$ does ~35 packed muls + 1 packed add per
Gray step ≈ **97% mul, 3% add**. Weighted ns/op:

| Encoding   | weighted ns/op (n=36) | speedup vs LUT-A |
|------------|----------------------:|-----------------:|
| LUT-A      | 0.480                 | 1.00× (baseline) |
| **B**      | **0.272**             | **1.76×**        |

At smaller $n$ the picture shifts:

| Workload                   | LUT-A | B    | B speedup |
|----------------------------|------:|-----:|----------:|
| $n=10$ (~90% mul)         | 0.48  | 0.81 | 0.59×     |
| $n=20$ (~95% mul)         | 0.48  | 0.43 | 1.12×     |
| $n=24$ (~96% mul)         | 0.48  | 0.36 | 1.34×     |
| $n=36$ (~97% mul)         | 0.48  | 0.27 | 1.76×     |
| $n=64$ (~98% mul)         | 0.48  | 0.20 | 2.41×     |

So **for $\mathbb{F}_{17}$ Ryser, B wins above $n \approx 18$ and wins by
~2× at the epic's headline $n = 36$**. F_17 is the cleanest empirical
demonstration of the Fermat-like trick winning in production.

This matters for the §8 rule (next section): a "no per-op regression"
rule would reject F_17-B because B regresses 16× on add. But the rule
shouldn't be blind to workload weighting when the dominant op gives a
massive win.

## 5. The revised §8 fallback rule

The original epic §8 rule was "≥ 1.5× speedup or fallback to LUT-A".
The cross-prime data shows this rule is mis-calibrated:

- It rejects F_5-D (uniform 1.10–1.20×) even though D dominates A on
  every op.
- It rejects F_17-B at $n < 36$ even though B wins decisively at the
  epic's target $n = 36$.

**Revised rule**: a candidate $C$ wins over LUT-A iff **either**

1. $C$ has no per-op regression vs LUT-A AND is faster on at least one
   op, **OR**
2. $C$ achieves $\ge 1.10\times$ speedup on a Gray-code-Ryser-weighted
   mix at the epic's target $n$ (currently $n = 36$).

If both rules disqualify every candidate, fall back to LUT-A.

Application to the three measured primes:

- $\mathbb{F}_3$: bipedal wins by clause 1 (uniform 13–22× over LUT-A).
- $\mathbb{F}_5$: D wins by clause 1 (uniform 1.10–1.20× over A, no
  regression). **R1 amended.**
- $\mathbb{F}_7$: D fails clause 1 (regresses 1.7× on mul) and fails
  clause 2 (Ryser-weighted is 0.62× of A). A wins. **R2 ratified.**
- $\mathbb{F}_{17}$: B fails clause 1 (regresses 16× on add) but **wins
  clause 2** (Ryser-weighted at $n = 36$ is 1.76×). For a future epic
  targeting $\mathbb{F}_{17}$, B would be the choice.

Clause 2's threshold of $1.10\times$ is calibrated to capture F_5-D's
uniform-but-small advantage in the rare case where it's the only
candidate that beats A; in practice clause 1 catches F_5-D first.
Clause 2 is what makes F_17-B win when the per-op picture is mixed.

## 6. Generalization beyond $p \le 17$

**Boring primes** (no gift): $p \in \{11, 13, 19, 23, 29, \ldots\}$ are
expected to fall back to LUT-A. For $p \le 16$ the 4-bit-slot LUT-A
applies; for $16 < p \le 256$, 8-bit-slot LUT-A applies (with the 2×
density penalty). Beyond $p = 256$, packed encoding becomes
uneconomical and `gf2-core::gfp::Fp<P>` Montgomery arithmetic is the
right answer.

**Fermat-like primes**: $\{17, 257, 65537\}$ are predicted to follow
the F_17 pattern — Candidate B's $(z, \log)$ encoding wins on
mul-heavy workloads. F_257 specifically is interesting for lattice
crypto (small enough for small-prime arithmetic, $p - 1 = 2^8$ is a
clean byte-aligned log).

**Mersenne primes** (other than 3): $\{7, 31, 127\}$ where Mersenne
fold gives cheap add. For $p = 31$, the cross-product mul circuit
becomes prohibitive ($31^2 = 961$ cells) and a hybrid encoding becomes
mandatory: D's bit-planes for add, separate LUT for mul. Untested
here; would be a follow-up issue.

## 7. Relationship to existing `gf2-core` machinery

The packed encodings are **orthogonal to**, not reductions of, the
existing finite-field machinery:

- **`gfp::Fp<P>` (Montgomery)** is scalar — one element, one arithmetic
  op. Generic for any odd prime up to 64-bit. Different optimization
  target: no lane parallelism. Used as the ground-truth reference
  (`ref_*` functions in the prototypes call out to scalar `% p`
  arithmetic that mirrors `Fp<P>` semantics).
- **`gf2m` (GF(2^m) via CLMUL)** is lane-parallel by accident: `u64`
  packs 64 F_2 elements directly because the field is characteristic
  2. Mul uses hardware `pclmulqdq`. Algebraically very different
  (polynomial ring over F_2, not prime field), so the techniques don't
  transfer.
- **`gfpn` (tower extensions)** sits above `gfp` — F_(p²) and F_(p³) via
  polynomial representation. Doesn't currently use packed encodings,
  but **could**: a F_(p^n) coefficient vector could be stored as `n`
  parallel packed-F_p columns. That would be a natural follow-on to
  the gf2-algebra epic.
- **`dev/research/rns_prototype/`** (Residue Number System) is the
  closest in-tree relative — RNS computes over
  $\mathbb{F}_{p_1 \cdot p_2 \cdots p_k}$ by parallelising scalar
  `Fp<P_i>` ops over the moduli. Same "lane-parallel small-prime
  arithmetic" theme as our packed encodings, but the lanes are
  different primes (CRT) rather than different elements of the same
  prime.

## 8. Closest external relative

**Bit-sliced AES** is structurally identical to our Candidate D: the
AES S-box is a function $\mathbb{F}_2^8 \to \mathbb{F}_2^8$ represented
as a Boolean circuit on 8 bit-planes. Our F_5-D / F_7-D mul uses the
same pattern — express the field op as a Boolean circuit on
$\lceil \log_2 p \rceil$ canonical bit-planes via truth-table cross-
product. The bit-sliced-AES literature (Käsper-Schwabe 2009 onward)
gives the optimisation playbook for this kind of code.

The "packed prime field" technique itself doesn't seem to have a
single canonical reference outside the Scheinerman F_3 paper. The
closest body of work is in lattice cryptography (small-prime modular
arithmetic for RLWE / CRYSTALS), which uses $(z, \log)$ + Fermat-like
primes ($p = 2^{16} + 1$ chosen specifically because Fermat-like).
That's the same trick, applied at scale.

## 9. Provenance

- **Standalone prototypes** (each with `[workspace]` empty marker so
  they don't enter the gf2 workspace):
  - `dev/research/f3_bipedal/` — naive, LUT-A, bipedal.
  - `dev/research/f5_packing/` — A, B, C, D (R1).
  - `dev/research/f7_packing/` — A, B, C, D (R2).
  - `dev/research/f17_packing/` — naive, LUT-A, B (this doc's headline).
- **Sibling decision docs**:
  - `dev/plans/r1_f5_encoding_decision.md`
  - `dev/plans/r2_f7_encoding_decision.md`
  - `dev/plans/r2_f3_f5_f7_cross_prime_comparison.md`
- **Paper**: arxiv 2407.20205v2, Scheinerman, "Fast permanents over
  $\mathbb{F}_3$ via $\mathbb{F}_2$ arithmetic" (referenced from
  `dev/plans/gf2_algebra_permanent.md`).
