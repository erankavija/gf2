# R1 — $\mathbb{F}_5$ packed encoding decision

> Decision document for JIT issue **6b3f6054** under
> `epic:gf2-algebra-permanent`. Settles the encoding for the W4 implementation
> tasks (T17–T20: $\mathbb{F}_5$ packed type + ops, plus the W4 SIMD kernel).

## 1. Summary

**Winner — Candidate A (3-bit + $2^{16}$-entry LUT, baseline).**

Hard-fallback applies as documented in
`dev/plans/gf2_algebra_permanent.md` §8: no novel candidate beats Candidate A
by the required ≥ 1.5× margin on a balanced add+mul workload representative
of Gray-code Ryser permanent evaluation. **Candidate A is the chosen encoding
for W4.**

| Op  | Per-element ns (Candidate A) | Per-`u64` work (16 elements) |
|-----|------------------------------|-------------------------------|
| add | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| sub | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| mul | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| div | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |

LUT footprint: 4 × 64 KiB = 256 KiB total (fits comfortably in L2; one LUT at
a time fits in L1d for hot loops).

## 2. Candidates considered

| ID | Encoding | Storage |
|----|----------|---------|
| A  | 3-bit value at 4-bit-aligned slots; 16 elements per `u64`; `2^16`-entry LUT keyed on `(a_pair << 8) \| b_pair` (2 elements per lookup). | `Vec<u64>`. |
| B  | $(z, \log)$ split exploiting cyclic $\mathbb{F}_5^* = \langle 2 \rangle$; 3 separate bit-planes carrying the zero-flag and 2 log bits. | three parallel `Vec<u64>`. |
| C  | Same 4-bit slot as A; per-nibble-pair `2^8`-entry LUT (1 element per lookup, 16 lookups per `u64`). | `Vec<u64>`. |
| D  | Bit-sliced 3-plane canonical `(b_0, b_1, b_2)`; ops via 5-way decode + cross-product Boolean circuit derived from the F_5 truth tables. | three parallel `Vec<u64>`. |

## 3. Methodology

**Hardware.** AMD Ryzen 9 5900X (Zen 3), 12 cores, AVX2 (no AVX-512), L1d
32 KiB / core, L2 512 KiB / core, L3 64 MiB shared. Frequency was ~3.5–4.0
GHz on idle threads during benchmarking; benches run on a single core.

**Build.** `cargo build -p f5-packing-prototype --release` with
`opt-level = 3, lto = "thin", codegen-units = 1`, MSRV 1.95. Standalone Cargo
package; not a member of the gf2 workspace.

**Bench.** `cargo run -p f5-packing-prototype --release` from
`dev/research/f5_packing/`. For each candidate × `{add, sub, mul, div}`, the
harness times one full pass over `N = 65 536` packed elements with
`std::time::Instant`, reports the **median of 5 runs** as ns/element. LUTs
are warmed once before the timed regions; both operands are pre-packed and
held in cache. Output operand is allocated fresh per repeat (a `clone()`)
so each measurement covers the op only, not setup.

**Correctness.** `cargo test --release` runs 39 tests: per-candidate
exhaustive 5×5 verification of `add`, `sub`, `mul`, `div(b≠0)` against
`(a OP b) % 5`, plus `proptest` cases (64 each) on length-up-to-256 vectors
for every op of every candidate, plus `pack ∘ unpack = id`. All 39 pass.
Correctness is `[hard]` — see CLAUDE.md "Verification" section — and is met
by every candidate that we benchmarked.

## 4. Per-candidate measurements

Median ns/element across three full runs of the harness on the bench host
(stable to ~5 % between runs). Numbers are per single `(a OP b)` over the
full 65 536-element batch.

| Encoding                                | add     | sub     | mul     | div     |
|-----------------------------------------|---------|---------|---------|---------|
| A: 3-bit + 2^16 LUT (baseline)          |  0.23   |  0.23   |  0.23   |  0.23   |
| B: (z, log) split, F_5* cyclic          | 10.3    | 10.0    |  0.015  |  0.014  |
| C: 4-bit + 2^8 LUT (nibble-pair)        |  0.46   |  0.46   |  0.46   |  0.46   |
| D: bit-sliced 3-plane Boolean           |  0.19   |  0.19   |  0.21   |  0.21   |

Speedup vs Candidate A (>1.0 = faster than A):

| Encoding                                | add     | sub     | mul     | div     |
|-----------------------------------------|---------|---------|---------|---------|
| B: (z, log) split, F_5* cyclic          |  0.02×  |  0.02×  | **15×** | **18×** |
| C: 4-bit + 2^8 LUT (nibble-pair)        |  0.50×  |  0.50×  |  0.50×  |  0.50×  |
| D: bit-sliced 3-plane Boolean           |  1.20×  |  1.20×  |  1.10×  |  1.10×  |

## 5. Bitwise op-count derivation — Candidate A (chosen winner)

Layout: each `u64` packs 16 elements at 4-bit-aligned slots; the high bit of
each slot is reserved (canonical values are `0..=4`, never `≥ 5`). Binary
ops use a single 64 KiB lookup table keyed by a packed 16-bit
`(a_byte) | (b_byte << 8)` index, where each input byte holds two adjacent
4-bit-slot elements. Each lookup yields a 1-byte result containing two
packed 4-bit results. Per `u64`: 8 lookups produce 16 element results.

Per `u64` (16 F_5 ops), excluding the loop control:

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

Compared with the F_3 bipedal encoding (`2` ops/mul, `6` ops/add over
**64** elements per `u64`), Candidate A is structurally heavier per element
because F_5 needs 3 value bits with one redundant codeword vs. F_3's
2 value bits and a much richer redundant-codeword set. The wall-clock
difference is small in practice (~0.23 ns/elem here vs. an expected
~0.05 ns/elem for F_3 bipedal mul), and the LUT path keeps the four ops
fully symmetric — no awkward asymmetric add/sub/div paths.

## 6. Why each rejected candidate was rejected

**Candidate B (rejected despite 15× mul speedup).** B's mul/div are
genuinely fast — 5 ops on a `u64`-triple processing 64 elements
simultaneously, ~0.08 raw bitops/element — and the result on this hardware
is 0.015 ns/element. The blocker is **add/sub**: in `(z, log)` form there
is no bit-parallel addition (log space adds correspond to multiplications
in the underlying field). The prototype falls back to a per-element
extract-canonical-add-repack loop at ~10 ns/element, which is **40-50×
slower than A's add**.

For the Gray-code Ryser kernel, each Gray-code transition does `n` add/subs
(updating row sums) and `n` muls (updating the row-product). Per Gray-code
step, add/sub and mul are roughly balanced. With B vs A the balanced cost
becomes `(0.02 + 10) / 2 ≈ 5 ns/op` vs `0.23 ns/op` — **B is ~22× slower
overall on the permanent workload**, despite the headline mul speedup.

A bit-parallel B-add could be hand-crafted via a 5-way decode + 25-cell
DNF rebuild (estimated ~70 bitwise ops per `u64`-triple ≈ 1.1 ops/element,
i.e. ≥ 14× worse than A's add LUT path on this hardware). The prototype
did not pursue that derivation because A already lower-bounds B-add even
under that optimistic estimate.

**Candidate C (rejected, strictly worse than A).** Same packed layout as A,
but uses a 256-byte per-nibble-pair LUT — 16 lookups per `u64` against a
tiny LUT instead of A's 8 lookups against a 64 KiB LUT. A's 64 KiB LUT
fully resides in L2 and the hot pages stay in L1d during streaming, so
the larger-LUT cache penalty never materialises on this hardware. C pays
2× the lookup count for a footprint advantage that does not benefit a
sustained inner loop. Result: uniform ~2× slowdown across all four ops.

**Candidate D (1.10–1.20× faster than A on add/sub; below the 1.5× bar).**
A genuine bit-sliced 3-plane Boolean implementation. Decode + cross-product
runs in ~50–60 bitwise ops per `u64`-triple over 64 elements ≈ 0.8–1.0
ops/element. Modestly faster than A on add/sub; tied on mul/div.

D is the most interesting "near miss": the speedup is real (1.20× on add)
and would be larger after SIMD widening (each `u64` plane SIMD-widens
one-for-one to AVX2/AVX-512 lanes), since the LUT-based A path is harder
to SIMD-port on AMD Zen 3 (no AVX-512 vpermb/IFMA available, and
gather-style loads against a 64 KiB table do not vectorise cleanly). For
the **scalar prototype** measured here, D does not clear the ≥ 1.5× bar
on any op, so the hard-fallback rule selects A. **D should be revisited
once the W4 SIMD kernel is up** (sibling issue `c7542983`); if AVX2
gather throughput on Zen 3 turns out worse than expected for A, the
SIMD-batching decision may switch to D.

## 7. Forward implications for W4 (T17–T20)

- **T17 (F_5 packed type + ops)** instantiates the encoding from §1: 16
  elements per `u64` at 4-bit-aligned slots; LUTs lazy-built via
  `OnceLock` (or hard-coded as `static` arrays generated at build time).
- **T21 (SIMD kernel for F_5 / F_7)** should re-bench A vs D on AVX2 once
  the kernel skeleton exists — the 1.20× scalar gap may invert under
  vectorisation. This is **not** a re-decision authority for T17; T17
  ships with A regardless.
- **V3 (Lean F_5 / F_7 correctness)** — the F_5 proof sketch will target
  Candidate A: prove that the LUT entries are correct against
  `Fp<5>` arithmetic, then prove that the binary-op kernel reduces to
  16 LUT lookups whose composition matches `(a OP b) % 5` per element.
  This is structurally the same as the F_3 bipedal proof but lemma-shaped
  rather than circuit-shaped.

## 8. Recommendation

Adopt **Candidate A — 3-bit value at 4-bit-aligned slots with `2^16`-entry
binary-op LUTs** as the F_5 packed encoding for the gf2-algebra permanent
implementation phase (W4). This matches the documented hard-fallback in
the epic design doc and is the empirical winner on a balanced add+mul
workload representative of Gray-code Ryser. The LUT footprint (4 × 64 KiB)
is acceptable; per-element wall-clock is ~0.23 ns symmetrically across all
four ops, which is within ~5× of the F_3 bipedal encoding on the same
hardware.

The prototype, benchmark harness, and all four candidate implementations
remain in `dev/research/f5_packing/` for future re-evaluation under SIMD
or different cache regimes.

## 9. User sign-off

(populated by the lead after the user comments approval on JIT 6b3f6054
per the success-criterion "user has signed off on the chosen encoding")
