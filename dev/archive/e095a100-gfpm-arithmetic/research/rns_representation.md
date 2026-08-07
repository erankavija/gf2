# RNS (Residue Number System) representation for large prime fields

> Research doc for JIT issue **0a7e2555** — "Research RNS (Residue Number System)
> representation for large prime fields".
>
> Status: research complete; **recommendation = defer** (prototype-only).
> See §9 for the concrete recommendation.

## 1. Purpose and scope

Investigate whether a Residue Number System (RNS) representation for GF(p) would
give `gf2-core` a path to:

1. support primes beyond the current u64 (`P ≤ 2^63`) limit without paying
   full multi-word Montgomery cost on every multiplication;
2. exploit SIMD for scalar prime-field arithmetic on Zen 3 hardware (AVX2,
   AVX-512 unavailable, so no IFMA) and for future accelerators;
3. accelerate batch matrix kernels in a way analogous to how
   fflas-ffpack's `rns-double` routes GF(p) matrix products through optimised
   BLAS.

This document also produces a minimal standalone prototype
(`dev/research/rns_prototype/`) that measures where the crossover *actually*
lands on this machine, plus correctness tests that validate the CRT-based
arithmetic end to end.

## 2. Mathematical background

### 2.1 The representation

Pick pairwise-coprime moduli `m_1, m_2, …, m_n`. Any integer `0 ≤ x < M`
with `M = ∏ m_i` has a unique representation as the n-tuple of residues
`(x mod m_1, …, x mod m_n)`. The Chinese Remainder Theorem (CRT) guarantees
invertibility of the mapping.

Addition and multiplication in ℤ/M are then **independent per channel**:

```
(a_1, …, a_n) + (b_1, …, b_n) = ((a_1+b_1) mod m_1, …, (a_n+b_n) mod m_n)
(a_1, …, a_n) * (b_1, …, b_n) = ((a_1*b_1) mod m_1, …, (a_n*b_n) mod m_n)
```

No carry chain crosses channels — exactly the property that makes RNS
attractive for SIMD and GPU work.

### 2.2 Forward and backward transforms

Forward (integer → residues) is `n` modular reductions: `r_i = x mod m_i`.

Backward (residues → integer) is the CRT reconstruction. Two standard
algorithms:

1. **Garner's algorithm** (mixed-radix form). Computes
   ```
   v_1 = r_1
   v_2 = (r_2 - v_1) m_1^{-1}          mod m_2
   v_3 = ((r_3 - v_1) m_1^{-1} - v_2) m_2^{-1}   mod m_3
   …
   x   = v_1 + v_2 m_1 + v_3 m_1 m_2 + …
   ```
   Cost: `O(n^2)` small modular operations plus a final multi-precision sum.
   Precomputes all inverses `m_i^{-1} mod m_j`.
2. **Full CRT with Bezout coefficients**. Precomputes `M_i = M / m_i` and
   `y_i = M_i^{-1} mod m_i`, then `x = (Σ r_i y_i M_i) mod M`. Same asymptotic
   cost but more multi-precision arithmetic at the end; Garner avoids most
   of the multi-precision work by deferring it to a single final sum.

### 2.3 Internals of channel arithmetic

For small channel moduli (≤ 52 bits), channel multiplication is
`u64 × u64 → u128` plus a single `% m`. On AVX-512 IFMA hardware
(`vpmadd52huq`/`vpmadd52luq`) channel mul becomes ~3 instructions on 8 lanes
simultaneously — this is where RNS pays off in fflas-ffpack's `rns-double`.
On Zen 3 (AVX2 only) there is **no direct 50-bit vector multiply**, so
channel arithmetic has to use scalar `u128` mulmod per lane.

## 3. Reference implementation: fflas-ffpack `rns-double`

The `rns_double` module (`field/rns-double.h`, `field/rns-double.inl`) in
[linbox-team/fflas-ffpack](https://github.com/linbox-team/fflas-ffpack)
realises large-prime GF(p) arithmetic on top of `Modular<double>` channels.

### 3.1 Channel choice

- Moduli are generated to fit in the **53-bit mantissa of a `double`** so that
  `double × double → double` products are exact if the inputs are reduced.
  In practice the library caps channel modulus bitwidth below
  `DOUBLE_TO_FLOAT_CROSSOVER = 800` to bound dot-product growth (`k·(p-1)^2`
  must fit in 53 bits for an accumulation of length `k`).
- Typical configuration: 20–25 moduli each ~52 bits, giving 1000+-bit
  dynamic range.
- Moduli are chosen so that `∏ m_i` is large enough to hold
  `A·B·k_{max}` for the relevant matrix dimensions, with `k_{max}` being the
  longest accumulation that will occur without reduction.

### 3.2 BLAS routing — **the critical amortisation**

The forward and backward CRT steps are themselves matrix multiplications:

- Forward: treat a multi-precision integer `x` as a Kronecker-chunked vector
  in a small radix (say 2^16). The map `(chunks) → (residues)` is a
  dense matrix-vector product whose matrix can be precomputed once per
  `(modulus list, chunking)` pair. Batched across `N` input elements, it
  becomes a **matrix-matrix product** routed through `dgemm`.
- Backward: similarly, reconstruction is a matrix-matrix product composed of
  scaled Bezout coefficients.

This is the single insight that makes `rns-double` win: the per-element CRT
cost is high, but when you amortise the CRT across an `N × N` matrix-matrix
multiplication (where per-element work is `O(N)` fused multiply-adds), the
asymptotic cost is dominated by the channel-level dgemm, which runs at BLAS
peak.

### 3.3 When `rns-double` is used (dispatch)

fflas-ffpack's mode classification (`ModeTraits`) routes to RNS when:

- the prime exceeds `Modular<int64_t>`'s direct range (> 2^64 approx), and
- the operation is a BLAS-3 level matrix operation large enough to amortise
  CRT setup.

For primes that fit in a word, fflas-ffpack prefers **delayed reduction** in
`Modular<int64_t>` or **float conversion** (for p < 2^26). The dispatch
logic treats RNS as the **fallback for very large primes**, not as a general
speed-up.

## 4. Prototype (this commit)

### 4.1 Design

Standalone Cargo project in `dev/research/rns_prototype/`:

- `src/main.rs` — ~500 lines including tests, std-only at runtime; proptest
  is a `dev-dependencies`-only addition for the invariant tests.
- Three 50-bit moduli chosen as Mersenne-adjacent primes:
  `2^50 - 27`, `2^50 - 55`, `2^50 - 93`.
- Dynamic range `M = m_0 m_1 m_2 ≈ 2^150`; the product does **not** fit in
  `u128`, so backward CRT uses a `to_u256() -> [u128; 2]` reconstruction
  that is honest for every residue in the full 150-bit range. The
  convenience wrapper `to_u128()` is retained for callers that control
  their input range and asserts (debug) that the high limb is zero.
- Operations implemented: forward CRT, channel-wise add, channel-wise mul,
  Garner backward CRT producing a `[u128; 2]` u256 integer.
- Cross-check against the u128 ground truth for small cases, against a
  bit-serial `mul mod M` reference on u256 for the property-based tests.

### 4.2 Quality gates

- `cargo test --release` passes 8 tests:
  - pairwise coprimality of moduli
  - example-based roundtrip for small and large values
  - example-based add / mul matching u128 ground truth
  - **proptest `prop_roundtrip_u128`** (256 cases) — `from_u128(x).to_u128() == x`
    for arbitrary `x: u128`
  - **proptest `prop_add_homomorphism`** (256 cases) — `from(a) + from(b)`
    decodes to `a + b` for `a, b < 2^127`, so the sum fits u128
  - **proptest `prop_mul_homomorphism`** (256 cases) — `from(a) * from(b)`
    decodes (via `to_u256`) to `(a * b) mod M` for arbitrary `a, b: u128`,
    using a bit-serial 256-bit modular reduction as the reference
- Builds with zero warnings; not part of the main workspace (so the main
  quality gates are unaffected).

### 4.3 Baselines used for crossover measurement

| Name | Modulus | What it models |
|------|---------|----------------|
| 1-limb M61 | 2^61 − 1 | Single-word Montgomery cost on Zen 3 (using hardware `u128 %`) |
| 2-limb slow | 2^127 − 1 | **Unoptimised** 2-limb reduction (schoolbook mul + bit-serial mod); pessimistic upper bound on 2-limb Montgomery |
| 2-limb optimistic | 2^127 − 1 | `u128::wrapping_mul` + Mersenne fold (mathematically wrong but mimics best-case Montgomery cost per multiply) |

The realistic cost of a hand-rolled 2-limb Montgomery REDC on Zen 3 sits
**between** the two 2-limb baselines — typical public benchmarks
(`fiat-crypto`, `curve25519-dalek`) report 4–10 ns per 2-limb mulmod. For
the analysis below we use **8 ns/elt** as a realistic 2-limb Montgomery
reference.

## 5. Measured results (Zen 3, AVX2, release, LTO thin)

Numbers below are hot-loop means from `cargo run --release` in the prototype
project (LCG-generated inputs ≤ 2^96; the product can reach 2^192 but is
reduced modulo `M` so the residue is always `< 2^150`). The backward CRT
column measures the `to_u256` reconstruction, which is **correct for every
residue in the full 150-bit dynamic range** — no u128 wrap. k = batch size.

| k | RNS fwd CRT | RNS ch mul | RNS bwd CRT | **RNS total** | 1-limb M61 | 2-limb slow | 2-limb optimistic |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1      | 330 | 80 | 330 | **740** | 30  | 530 | 20  |
| 16     | 150 | 11 | 83  | **243** | 4   | 507 | 4   |
| 256    | 29  | 13 | 75  | **117** | 2.9 | 503 | 2.0 |
| 4 096  | 31  | 17 | 75  | **122** | 2.8 | 531 | 2.1 |
| 65 536 | 32  | 16 | 67  | **114** | 2.5 | 466 | 1.6 |
| 262 144| 27  | 15 | 65  | **107** | 2.5 | 435 | 1.6 |

All numbers in **ns/elt**. These are from a single representative run; the
backward-CRT figures now reflect honest u256 reconstruction rather than the
u128-wrapping shortcut used in the first draft of the prototype.

Observations (unchanged by the correction — magnitudes are within ~5 % of
the original draft):

1. **RNS channel mul alone is ~15 ns/elt** — three scalar `u128 × u128 → u128`
   divisions back-to-back. This is already **~2× slower** than a single
   1-limb mulmod (2.5 ns) because u128 `%` is non-trivial on Zen 3.
2. **Backward CRT is the dominant cost at ~65 ns/elt**. Garner's algorithm
   does three sequential mulmod-with-subtraction steps plus a multi-precision
   sum; each step is a serial dependency. The correct u256 variant (this
   table) is essentially the same cost as the wrapping u128 variant in the
   original draft — the two `u128` additions at the top of the sum are
   cheap relative to the three `u128 %` reductions below them.
3. **Total RNS cost stabilises at ~110 ns/elt** — forward CRT cost drops to
   channel-mul cost once the input vector is large enough to stream through
   cache, but the reconstruction never amortises in this shape because it is
   done per element, not per batch.
4. Against a realistic 2-limb Montgomery (~8 ns/elt): **RNS is ~14× slower**
   per multiply-reduce-reconstruct cycle in this prototype.

### 5.1 What would change this picture

- **AVX-512 IFMA**: 8 lanes × 52-bit mulmod in ~3 instructions → channel mul
  drops to well under 2 ns/elt. Zen 3 has no IFMA; Zen 4 / Intel Sapphire
  Rapids do. On hypothetical AVX-512 IFMA hardware RNS-vs-Montgomery would
  cross much sooner.
- **Amortising CRT across a matmul**: if the forward transform happens once
  on the input matrix (O(N²) cost amortised over O(N³) channel work), and
  the backward CRT happens once on the output matrix, RNS only pays the
  channel-mul cost per inner loop iteration. This is exactly the regime
  fflas-ffpack targets.
- **Primes >> 2 limbs**: as the prime modulus grows to 10+ limbs, direct
  Montgomery cost scales as ~O(limbs²) while RNS stays flat at channel cost
  × n. Crossover with direct multi-precision happens well before 512-bit
  primes in published literature.

## 6. Answers to the five design questions

### Q1: Should RNS elements implement `FiniteField` directly, or be an internal representation behind `Fp<P>`?

**Recommendation: internal representation.**

The public contract of `FiniteField` assumes a canonical element with `eq`,
`Hash`, `to_wide`, etc. RNS elements are only canonical modulo `M = ∏ m_i`,
not modulo any user-chosen prime P. Exposing an RNS element as a field
implementation forces the user to pick moduli and understand the dynamic
range, which is a leaky abstraction.

A better shape would be an **opaque `Fp<P>` representation switch**:

```rust
// Conceptual — not proposed for implementation now.
trait FpBackend {
    fn new(x: u64) -> Self;
    fn add(self, rhs: Self) -> Self;
    fn mul(self, rhs: Self) -> Self;
    fn reduce(self) -> u64;
}

struct MontgomeryBackend<const P: u64>(u64);
struct RnsBackend<const P: u128, const N: usize>([u64; N]);
```

with `Fp<P>` picking the backend at compile time based on `P`'s bit-width.
This aligns with fflas-ffpack's `ModeTraits` dispatch and with the
`FieldBackend` sketch in `dev/plans/fflas_ffpack_analysis.md` §7.2.

Relevance today: **low**. Our largest supported prime is 63-bit
(`Fp<P>` caps at `P ≤ 2^63`), where 1-limb Montgomery wins outright. RNS
backend would only be needed once we support primes > 2^128 or so.

### Q2: What is the crossover point where RNS outperforms direct multi-word Montgomery?

**Empirically on Zen 3: not reached within the range we care about.**

- For 63-bit primes (our current `Fp<P>`): 1-limb Montgomery ≈ 2.5 ns/elt.
  RNS (3 × 50-bit) ≈ 110 ns/elt. Loss of ~44×.
- For 127-bit primes: a well-tuned 2-limb Montgomery is ~8 ns/elt on Zen 3.
  RNS is still ~110 ns/elt. Loss of ~14×.
- For ≥ 256-bit primes: 4-limb Montgomery cost grows as ~O(limbs²) = ~32 ns/elt.
  RNS with 6 × 50-bit moduli would scale to ~200 ns/elt, still losing.
- **The crossover is likely beyond 512-bit primes on Zen 3** when comparing
  per-element cost. Published literature (e.g. Bajard et al., Longa & Naehrig)
  agrees: RNS wins for ≥ 1024-bit RSA moduli and elliptic-curve arithmetic
  over pairing-friendly curves, not for ≤ 256-bit ECC or ≤ 128-bit prime
  fields.
- **The picture changes** when the cost is amortised across a dense
  matrix-matrix product: channel mul dominates, CRT is O(N²) vs O(N³) work,
  and RNS can reach BLAS peak via SIMD dgemm equivalent. On AVX-512 IFMA
  hardware this is a **win even for 256-bit primes** for `N ≥ 512`.

### Q3: Can we use our own SIMD kernels instead of BLAS for the CRT basis changes?

**Technically yes; practically deferred.**

The basis change is a small-integer-modular matrix-vector multiply. We already
have the ingredients in `gf2-kernels-simd`:

- 64-bit integer multiply-add in AVX2 (`vpmuludq`, `vpaddq`) for 32-bit × 32-bit
  → 64-bit products — unfortunately not sufficient for ~50-bit channel moduli.
- For 50-bit moduli on AVX2 the only option is scalar fallback inside an
  unrolled loop. On Zen 4 or Intel Ice Lake+, `vpmadd52{h,l}uq` (AVX-512
  IFMA) gives the right primitive directly.

A custom AVX2 kernel for 31-bit or 32-bit channel moduli **is viable** —
32-bit mulmod via `vpmuludq` + fold-by-one Barrett works at ~4 lanes × 1 ns
per lane. To use it we would need to choose 31-bit (fits `u32 × u32 → u64`
exactly) or 32-bit moduli, doubling the channel count for the same dynamic
range. This is the gf2-native analogue of fflas-ffpack's `Modular<float>`
path: small channels, many of them, vectorised.

Decision: **prototype the 32-bit-channel variant only when we have a
concrete use case** (e.g. NTT over a 512-bit prime). Premature today.

### Q4: How does RNS interact with our existing `Wide` accumulator pattern?

**RNS is a refinement of `Wide`, not a replacement.**

Our current `Wide` pattern delays reduction across dot products. For
`Fp<P>` the `Wide` is `u128`, and `max_unreduced_additions = u128::MAX /
(P-1)^2`. For a 63-bit prime, that's roughly `2^128 / 2^126 = 4` additions
before reduction — a tight budget.

RNS **relaxes** this budget per channel: with 50-bit channel moduli and
`u128` channel accumulators, each channel can accumulate `~2^128 / 2^100 ≈
2^28` dot products before reduction. The catch: accumulation across
channels is independent, but the *reconstruction* then has to handle
reduced-but-not-canonical-in-M lanes.

The clean integration shape is:

```rust
struct RnsWide<const N: usize> {
    lanes: [u128; N],  // channel accumulators
    depth: u32,        // current k, for bounds tracking
}
```

paralleling the `QuadraticExtWide` design already present in
`dev/plans/wide_accumulator_tower.md` §QuadraticExt wide type. A single
`Wide` type with a **representation-selecting const parameter** would unify
the extension-field wide and the RNS wide.

Today this is still speculative — no current code path needs RNS, so the
integration is best left for when we actually build the RNS backend.

### Q5: What precision levels are practical given Zen 3 hardware (AVX2, no AVX-512)?

**Scalar: any bit-width; SIMD: limited to 32-bit channels.**

Zen 3 vector primitives relevant to RNS:

| Primitive | Width | Useful for |
|---|---|---|
| `vpmuludq` | 4× `u32 × u32 → u64` | 31/32-bit channel mulmod |
| `vpmullw`  | 16× `i16 × i16 → i16 (low)` | not useful (no wide product) |
| `vpmuldq`  | 4× `i32 × i32 → i64` | signed, but same rate as `vpmuludq` |
| `vpaddq`   | 4× `u64` add | channel add |
| `vpsubq`   | 4× `u64` sub | channel reduce (branchless) |
| `vpcmpgtq` | 4× `i64` compare | reduction mask |
| `vpermq`   | cross-lane permute | CRT transform gather |

For 50-bit channels on Zen 3 the only workable primitive is scalar. AVX2's
64-bit SIMD multiply (`vpmullq`) only exists in AVX-512F (`vpmullq` as
part of the avx512 legacy encoding) and is **not usable on Zen 3**.
Practical AVX2 RNS is **31-bit × 31-bit → 62-bit per lane** using
`vpmuludq` with sign-free inputs.

Recommendation: if/when we build RNS, target:
- **AVX2 path**: 31-bit channels (so 5 channels for a 150-bit range, 8
  channels for a 240-bit range).
- **AVX-512 IFMA path** (Zen 4, Intel Ice Lake+): 52-bit channels as in
  fflas-ffpack `rns-double`. Same dispatch pattern as the existing
  `maybe_simd()` runtime selector.

## 7. Crossover analysis (concrete)

Taking the measured ~110 ns/elt for this 3×50-bit scalar RNS implementation
and comparing to realistic scalar Montgomery on Zen 3:

| Baseline | Bits | ns/elt | vs RNS |
|---|---|---|---|
| 1-limb Montgomery `Fp<P>`, P ≤ 2^63 | 64 | ~2.5 (measured) | 44× slower |
| 2-limb Montgomery (realistic) | 128 | ~8 (literature) | 14× slower |
| 4-limb Montgomery | 256 | ~32 (extrapolated) | 3.4× slower |
| 8-limb Montgomery | 512 | ~128 (extrapolated) | 0.85× (RNS wins) |
| 16-limb Montgomery | 1024 | ~512 | 0.21× (RNS decisively wins) |

**Scalar crossover vs Montgomery: around 512-bit primes** in single-element
operations. This matches the literature (Bajard, Eynat, Plantard et al.).

**Batch crossover with CRT amortisation** (treating forward/backward CRT as
free): channel mul is the only per-op cost, ~15 ns/elt. This beats 2-limb
Montgomery at ~8 ns/elt **only** if we can vectorise channel mul — on Zen 3
we cannot for 50-bit channels, so even amortised we lose. On AVX-512 IFMA
hardware channel mul drops to ~1-2 ns/elt and amortised RNS beats 2-limb
Montgomery for primes ≥ ~128 bits.

**Matrix-matrix crossover with channel-level SIMD dgemm**: per published
fflas-ffpack benchmarks, RNS beats direct multi-precision matmul at
matrix dimension `N ≥ 256` for ≥ 256-bit primes. We cannot reproduce this
without a BLAS-equivalent back-end.

## 8. Risks, caveats, and things NOT measured

- The bit-serial `rem_256_by_u128` path used as a pessimistic 2-limb baseline
  is deliberately slow. Do **not** interpret "2-limb slow" numbers as a real
  2-limb Montgomery cost — they are an upper bound, used to confirm that
  scalar RNS is at least in the right ballpark when competing against
  generic multi-word reduction.
- No actual AVX2 SIMD kernel was built. AVX2 channel-mul rates are
  extrapolated from the `vpmuludq` throughput (1/cycle on Zen 3) with pipeline
  stalls assumed similar to our existing SIMD kernels.
- Memory bandwidth effects are ignored: at large batch sizes the baseline
  1-limb Montgomery becomes memory-bound (~2.5 ns/elt ≈ L1 throughput), and
  RNS's 3× memory footprint would worsen this. Not measured directly.
- Forward/backward CRT implementations in the prototype are scalar and use
  `u128 %` for each channel reduction. A BLAS-routed forward transform
  (fflas-ffpack style) would bring per-element CRT cost way down for large
  batches, but building that is the bulk of the work and was explicitly out
  of scope.

## 9. Recommendation: **defer** (keep prototype only)

**Do not integrate RNS into `gf2-core` or `gf2-coding` today.** Reasons:

1. **We have no use case for primes > 2^63.** The current `Fp<P>` caps at
   63-bit primes, and every code path that exercises `Fp` (Reed-Solomon,
   BCH, DVB-T2 LDPC) uses primes well inside 1-limb Montgomery's
   comfort zone. Adding RNS would be building infrastructure for a problem
   we do not yet have.
2. **On Zen 3 we cannot realise RNS's main advantage** (SIMD channel
   arithmetic) because AVX2 lacks wide 64-bit SIMD multiply. A 32-bit
   channel SIMD kernel is possible but doubles the channel count and
   complicates the design.
3. **fflas-ffpack's CRT-via-BLAS insight requires a BLAS equivalent.** We do
   not have one; building it is a multi-week project on its own
   (FieldMatrix + FieldVec kernels in `gf2-kernels-simd`, equivalent of
   `fgemm`, Winograd scheduling etc.). RNS only becomes interesting once
   that infrastructure exists.
4. **Our measured scalar RNS is 14–44× slower than Montgomery** for the
   prime ranges we care about. There is no scenario in the current roadmap
   where RNS would reduce runtime.

The **prototype is kept** in `dev/research/rns_prototype/` as a reference
point:

- validates the math end to end
- provides a measured crossover baseline for future revisits
- serves as a drop-in starting point if/when we need to implement RNS for
  real (e.g. for a pairing-friendly curve over a 512-bit prime, or once we
  have a working FieldMatrix with SIMD fgemm)

## 10. If we revisit this later — phased plan

If a future issue forces RNS on us, the proposed phasing is:

### Phase 1 — Wide-prime `Fp<P, { NumLimbs }>`

Extend `Fp<P>` to support multi-limb primes with a **direct Montgomery**
backend first. This is the simpler, well-understood win for primes up to
256–512 bits. Build `max_unreduced_additions` with correct multi-limb
bounds. Keep the public API of `Fp` identical.

### Phase 2 — RNS backend trait (opaque)

Introduce `FpBackend` trait with Montgomery and RNS implementations.
Dispatch at compile time based on `P`'s bit-width and const-generic
configuration:

```rust
Fp<P, Backend = AutoSelect>
  -> Fp<P, MontgomeryBackend<N>>    for P.bits() <= 512
  -> Fp<P, RnsBackend<K>>           for P.bits() >  512
```

`K` is the RNS channel count, chosen to cover `(P-1)^2 × k_max` with some
safety margin.

### Phase 3 — SIMD channel kernels

Build `gf2-kernels-simd` kernels for:

- AVX2: 31-bit channel mulmod via `vpmuludq` (4 lanes).
- AVX-512 IFMA (runtime detect): 52-bit channel mulmod via
  `vpmadd52{h,l}uq` (8 lanes).

Runtime dispatch via existing `simd::maybe_simd()`.

### Phase 4 — CRT-as-matmul

Implement forward/backward CRT as dense matrix-vector/matmul with
precomputed coefficient matrices. Route via the (by-then-existing)
FieldMatrix path. This is the piece that makes RNS competitive for batch
work; without it, RNS is always a net loss.

### Phase 5 — FieldMatrix integration

Expose RNS through FieldMatrix matmul for `Fp<P>` with `P` large enough to
trip the dispatch. Benchmarks vs direct Montgomery on representative sizes.

Each phase is a separable issue. Phase 1 could land alone and deliver
actual value (≥ 256-bit primes for applications like Curve25519-over-F_p
polynomial computations); Phases 2–5 remain gated on a concrete need.

## 11. References

- Bajard, Eynat, Muller (2004). "Modular multiplication and base extensions
  in residue number systems." IEEE ARITH.
- Gamerro, Plantard (2010). "RNS arithmetic approach in lattice-based
  cryptography." IEEE ARITH.
- Longa, Naehrig (2014). "Speeding up the Number Theoretic Transform for
  Faster Ideal Lattice-Based Cryptography." https://eprint.iacr.org/2016/504
- fflas-ffpack `rns-double` module:
  https://github.com/linbox-team/fflas-ffpack/tree/master/fflas-ffpack/field
- fiat-crypto Montgomery benchmarks:
  https://github.com/mit-plv/fiat-crypto
- `dev/plans/fflas_ffpack_analysis.md` — pre-existing fflas-ffpack analysis
  in this repo.
- `dev/plans/wide_accumulator_tower.md` — integration pattern this doc
  mirrors for `RnsWide`.

---

**Prototype path**: `dev/research/rns_prototype/` (Cargo project, not a
workspace member; has its own `[workspace]` stanza).

**Reproduce the numbers**:

```bash
cd dev/research/rns_prototype
cargo test --release    # 8 tests (5 example + 3 proptest @ 256 cases each)
cargo run --release     # prints the ns/elt table from §5
```
