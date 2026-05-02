# R2 — $\mathbb{F}_7$ packed encoding decision

> Decision document for JIT issue **f10152f6** under
> `epic:gf2-algebra-permanent`. Settles the encoding for the W4 implementation
> tasks (T19/T21: $\mathbb{F}_7$ packed type + ops, plus the W4 SIMD kernel).

## 1. Summary

**Winner — Candidate A (3-bit + $2^{16}$-entry LUT, baseline).**

Hard-fallback applies as documented in
`dev/plans/gf2_algebra_permanent.md` §8: no novel candidate beats Candidate A
by the required ≥ 1.5× margin on a workload representative of Gray-code
Ryser permanent evaluation. **Candidate A is the chosen encoding for W4.**

| Op  | Per-element ns (Candidate A) | Per-`u64` work (16 elements) |
|-----|------------------------------|-------------------------------|
| add | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| sub | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| mul | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| div | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |

LUT footprint: 4 × 64 KiB = 256 KiB total (fits comfortably in L2; one LUT at
a time fits in L1d for hot loops). The per-element wall-clock is statistically
indistinguishable from R1's $\mathbb{F}_5$ Candidate A — the LUT shape is
identical and only the LUT contents differ.

The most interesting near-miss is **Candidate D** (bit-sliced 3-plane with
Mersenne-fold add): D is **9.5× faster than A on add** but **0.53× of A on
mul** (i.e., ~1.9× slower on mul). Under any plausible Ryser weighting
(mul-dominated), D's mul deficit dominates D's add advantage and A wins. See
§6 for the calculation.

## 2. Candidates considered

| ID | Encoding | Storage |
|----|----------|---------|
| A  | 3-bit value at 4-bit-aligned slots; 16 elements per `u64`; $2^{16}$-entry LUT keyed on `(a_pair << 8) \| b_pair` (2 elements per lookup). | `Vec<u64>`. |
| B  | $(z, \log)$ split exploiting cyclic $\mathbb{F}_7^* = \langle 3 \rangle$ of order 6; 4 separate bit-planes carrying the zero-flag and 3 log bits. Bit-sliced log-add mod 6 for mul/div; per-element fallback for add/sub. | four parallel `Vec<u64>`. |
| C  | Same 4-bit slot as A; per-nibble-pair $2^8$-entry LUT (1 element per lookup, 16 lookups per `u64`). | `Vec<u64>`. |
| D  | Bit-sliced 3-plane canonical $(b_0, b_1, b_2)$; **Mersenne-fold** add/sub exploiting $7 = 2^3 - 1$; 7-way decode + cross-product cell ANDs for mul/div. | three parallel `Vec<u64>`. |

## 3. Methodology

**Hardware.** AMD Ryzen 9 5900X (Zen 3), 12 cores, AVX2 (no AVX-512), L1d
32 KiB / core, L2 512 KiB / core, L3 64 MiB shared. Frequency was ~3.5–4.0
GHz on idle threads during benchmarking; benches run on a single core. (Same
host as R1, so R1/R2 numbers are directly comparable.)

**Build.** `cargo build -p f7-packing-prototype --release` with
`opt-level = 3, lto = "thin", codegen-units = 1`, MSRV 1.95. Standalone Cargo
package; not a member of the gf2 workspace.

**Bench.** `cargo run -p f7-packing-prototype --release` from
`dev/research/f7_packing/`. For each candidate × `{add, sub, mul, div}`, the
harness times one full pass over `N = 65 536` packed elements with
`std::time::Instant`, reports the **median of 5 runs** as ns/element. LUTs
are warmed once before the timed regions; both operands are pre-packed and
held in cache. Output operand is allocated fresh per repeat (a `clone()`)
so each measurement covers the op only, not setup.

**Correctness.** `cargo test --release` runs 41 tests: per-candidate
exhaustive 7×7 verification of `add`, `sub`, `mul`, `div(b≠0)` against
`(a OP b) % 7`, plus `proptest` cases (64 each) on length-up-to-256 vectors
for every op of every candidate, plus `pack ∘ unpack = id`. Candidate D adds
an explicit Mersenne-fold spot-check on all 49 (a, b) pairs. All 41 pass.
Correctness is `[hard]` per CLAUDE.md "Verification" section and is met
by every candidate that we benchmarked.

## 4. Per-candidate measurements

Median ns/element across four full runs of the harness on the bench host
(stable to ~5 % between runs). Numbers are per single `(a OP b)` over the
full 65 536-element batch. Reported figures are the centre of the observed
range with half-range error rounded to two decimals.

| Encoding                                | add     | sub     | mul     | div     |
|-----------------------------------------|---------|---------|---------|---------|
| A: 3-bit + 2^16 LUT (baseline)          |  0.23   |  0.23   |  0.23   |  0.23   |
| B: (z, log) split, F_7* cyclic          |  8.43   |  8.39   |  0.02   |  0.02   |
| C: 4-bit + 2^8 LUT (nibble-pair)        |  0.47   |  0.47   |  0.47   |  0.48   |
| D: bit-sliced 3-plane (Mersenne fold)   |  0.02   |  0.02   |  0.38   |  0.39   |

Speedup vs Candidate A (>1.0 = faster than A):

| Encoding                                | add      | sub      | mul     | div     |
|-----------------------------------------|----------|----------|---------|---------|
| B: (z, log) split, F_7* cyclic          |  0.03×   |  0.03×   | **9.8×** | **10.5×** |
| C: 4-bit + 2^8 LUT (nibble-pair)        |  0.46×   |  0.46×   |  0.46×  |  0.46×  |
| D: bit-sliced 3-plane (Mersenne fold)   | **9.8×** | **8.8×** |  0.58×  |  0.57×  |

## 5. Bitwise op-count derivation — Candidate A (chosen winner)

Layout: each `u64` packs 16 elements at 4-bit-aligned slots; the high bit of
each slot is reserved (canonical values are `0..=6`, never `≥ 7`). Binary
ops use a single 64 KiB lookup table keyed by a packed 16-bit
`(a_byte) | (b_byte << 8)` index, where each input byte holds two adjacent
4-bit-slot elements. Each lookup yields a 1-byte result containing two
packed 4-bit results. Per `u64`: 8 lookups produce 16 element results.

Per `u64` (16 F_7 ops), excluding the loop control:

| Step                                                   | Ops |
|--------------------------------------------------------|----:|
| Extract `a_byte_i` from `a`: `(a >> (8·i)) & 0xff`     |  16 |
| Extract `b_byte_i` from `b`: `(b >> (8·i)) & 0xff`     |  16 |
| Form key: `a_byte_i \| (b_byte_i << 8)`                |  16 |
| LUT load (memory op)                                   |   8 |
| Splice into result `r \|= load << (8·i)`               |  16 |

Total per `u64` = **16 elements**:
- **8 LUT loads** (memory)
- **64 ALU ops** (shifts/ANDs/ORs across the 8 unrolled iterations; LLVM
  shares the shift counts and exit-byte placements, so the compiled code is
  shorter than the table suggests, but 64 is the hand-counted upper bound).

Per element: **0.5 LUT loads** and **4 ALU ops**. The same shape applies to
add, sub, mul, and div — only the LUT contents differ.

The per-element op count is identical to R1's $\mathbb{F}_5$ Candidate A
because the encoding shape is the same; the only difference is that
$\mathbb{F}_7$ has one redundant 4-bit codepoint (`7`) where $\mathbb{F}_5$
had three (`5, 6, 7`). The unused entries in the LUT are zero in both cases.

## 6. Why each rejected candidate was rejected

**Candidate B (rejected despite ~10× mul speedup).** B's mul/div use
bit-sliced log addition mod 6 (~21 ops per `u64`-quad processing 64
elements ≈ 0.33 raw bitops/element), which on this hardware drops to
~0.02 ns/element. The blocker is **add/sub**: in $(z, \log)$ form there
is no bit-parallel addition (log space adds correspond to multiplications
in the underlying field). The prototype falls back to a per-element
extract-canonical-add-repack loop at ~8.4 ns/element, which is **~37×
slower than A's add**.

For the Gray-code Ryser kernel, each Gray-code transition does 1 packed
add/sub (column-sum update) and ~`n−1` packed muls (row-product update).
Per Gray-code step at $n=36$, the mul:add ratio is 35:1 ≈ 97% mul, 3%
add. With B vs A on a mul-weighted mix:
$0.97 \cdot 0.02 + 0.03 \cdot 8.43 = 0.272$ ns/op for B vs $0.23$ ns/op for
A. **B is ~1.18× slower overall** even on a mul-dominated workload despite
the headline mul speedup, because the rare add is so brutal it taxes the
average. Under a balanced (50/50) workload B is ~18× slower. Either way B
loses to A.

**Candidate C (rejected, strictly worse than A).** Same packed layout as A,
but uses a 256-byte per-nibble-pair LUT — 16 lookups per `u64` against a
tiny LUT instead of A's 8 lookups against a 64 KiB LUT. A's 64 KiB LUT
fully resides in L2 and the hot pages stay in L1d during streaming, so
the larger-LUT cache penalty never materialises on this hardware. C pays
2× the lookup count for a footprint advantage that does not benefit a
sustained inner loop. Result: uniform ~2× slowdown across all four ops
(matches R1's F_5-C behaviour).

**Candidate D (9.5× faster than A on add; 0.58× on mul).**
The Mersenne-fold add genuinely delivers — `(a+b) mod 7` for
`a, b ∈ {0..=6}` reduces to a 4-bit ripple add followed by a single
conditional subtract, totalling 31 bitwise ops per `u64`-triple over 64
elements ≈ 0.48 ops/element. LLVM auto-vectorises the hot loop to AVX2
256-bit lanes, and per-element wall-clock collapses to ~0.02 ns/element.

D's mul, however, uses a 7-way decode + 7×7 cross-product table (~100
bitwise ops per `u64`-triple over 64 elements ≈ 1.56 ops/element) and
runs at ~0.38 ns/element — about 1.7× slower than A's LUT mul.

Plausible workload weightings:

| Workload                          | A weighted ns/op | D weighted ns/op | A vs D |
|-----------------------------------|-----------------:|-----------------:|-------:|
| Balanced 50/50 (add/mul)          | 0.23             | 0.20             | D wins by 1.13× |
| Ryser-weighted at n=36 (≈97% mul) | 0.23             | 0.37             | A wins by 1.61× |
| Ryser-weighted at n=64 (≈98% mul) | 0.23             | 0.37             | A wins by 1.62× |

D needs to clear 1.5× **on the workload that matters** — Gray-code Ryser
is mul-dominated, and D loses on mul. Hard-fallback rule: A wins.

D is the most interesting "near miss" of R2 (and structurally cleaner than
F_5-D, since the Mersenne fold makes add genuinely free): the speedup on
add is ~9× — not just 1.2× as in R1's F_5-D — and would be larger after
SIMD widening. **D should be revisited once the W4 SIMD kernel is up**
(sibling issue `c7542983`); if AVX2 gather throughput on Zen 3 turns out
worse than expected for A's 64 KiB LUT, the SIMD-batching decision may
switch to D — particularly if the W4 kernel can interleave bipedal-style
add (cheap on D) with rare mul calls. The bit-sliced D layout also
SIMD-widens trivially (each bit-plane is a `u64` → `__m256i`), unlike A
where 64 KiB-LUT gather is harder to vectorise on Zen 3. **This is not a
re-decision authority for T19; T19 ships with A regardless.**

## 7. Forward implications for W4 (T19, T21, V3)

- **T19 (F_7 packed type + ops)** instantiates the encoding from §1: 16
  elements per `u64` at 4-bit-aligned slots; LUTs lazy-built via
  `OnceLock` (or hard-coded as `static` arrays generated at build time).
  The implementation should mirror the R1 outcome's T17 shape so the
  $\mathbb{F}_5$/$\mathbb{F}_7$ packed code has uniform structure.
- **T21 (SIMD kernel for F_5 / F_7)** should re-bench A vs D on AVX2 once
  the kernel skeleton exists. R2's D-vs-A scalar gap on add is much
  larger than R1's was (9× vs 1.2×), so the SIMD picture for $\mathbb{F}_7$
  is more interesting — the D path may become competitive on a
  mul-light workload (e.g., column-sum streaming kernels) even if it
  loses on Ryser's main loop. This is **not** a re-decision authority
  for T19; T19 ships with A regardless.
- **V3 (Lean F_5 / F_7 correctness)** — the $\mathbb{F}_7$ proof sketch
  will target Candidate A: prove that the LUT entries are correct against
  `Fp<7>` arithmetic, then prove that the binary-op kernel reduces to
  16 LUT lookups whose composition matches `(a OP b) % 7` per element.
  This is structurally the same as the F_3 bipedal proof but lemma-shaped
  rather than circuit-shaped — and almost identical to the planned
  $\mathbb{F}_5$ proof, with `7` substituted for `5` throughout.

## 8. Recommendation

Adopt **Candidate A — 3-bit value at 4-bit-aligned slots with $2^{16}$-entry
binary-op LUTs** as the $\mathbb{F}_7$ packed encoding for the gf2-algebra
permanent implementation phase (W4). This matches the documented
hard-fallback in the epic design doc and is the empirical winner on a
mul-weighted Ryser workload. The LUT footprint (4 × 64 KiB) is acceptable;
per-element wall-clock is ~0.23 ns symmetrically across all four ops, which
matches R1's $\mathbb{F}_5$ Candidate A on the same hardware.

The prototype, benchmark harness, and all four candidate implementations
remain in `dev/research/f7_packing/` for future re-evaluation under SIMD
or different cache regimes — particularly Candidate D, whose Mersenne-fold
add is the strongest scalar add of any candidate measured for either
$\mathbb{F}_5$ or $\mathbb{F}_7$.

## 9. User sign-off

(populated by the lead after the user comments approval on JIT f10152f6
per the success-criterion "user has signed off on the chosen encoding")
