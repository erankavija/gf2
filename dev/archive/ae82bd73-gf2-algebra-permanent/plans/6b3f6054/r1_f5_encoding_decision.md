# R1 — $\mathbb{F}_5$ packed encoding decision

> Decision document for JIT issue **6b3f6054** under
> `epic:gf2-algebra-permanent`. Settles the encoding for the W4 implementation
> tasks (T17–T20: $\mathbb{F}_5$ packed type + ops, plus the W4 SIMD kernel).

## 1. Summary

**Winner — Candidate D (bit-sliced 3-plane Boolean).**

Decision review 2026-05-03: this document originally ratified Candidate A
under the original §8 "≥ 1.5× margin" hard-fallback rule. After the
cross-prime study (`dev/plans/r2_packed_encoding_generalizations.md`) the
epic §8 rule was revised to "no per-op regression vs LUT-A AND faster on
at least one op, OR ≥ 1.10× Ryser-weighted speedup at $n = 36$".
Candidate D meets clause 1 (faster on every op, no regression) — so D
wins under the revised rule. The original A vs D analysis below is
preserved as historical record. **W4 (T17) ships with Candidate D.**

| Op  | Per-element ns (Candidate D, winner) | Per-element ns (Candidate A, runner-up) | D speedup |
|-----|--------------------------------------|------------------------------------------|----------:|
| add | 0.19                                 | 0.23                                     | 1.20×     |
| sub | 0.19                                 | 0.23                                     | 1.20×     |
| mul | 0.21                                 | 0.23                                     | 1.10×     |
| div | 0.21                                 | 0.23                                     | 1.10×     |

D uses three parallel `Vec<u64>` bit-planes (b₀, b₁, b₂) carrying the
canonical 3-bit value of each F_5 element; one `u64`-triple covers 64
elements. Ops are implemented as decode-then-cross-product Boolean
circuits derived from the 5×5 truth tables, ~50–60 bitwise ops per
`u64`-triple over 64 elements ≈ 0.8–1.0 ops/element. Storage is 0 KiB
of LUT footprint — bipedal-style, but with three planes instead of
F_3 bipedal's two.

### Original Candidate A summary (preserved as historical record)

| Op  | Per-element ns (Candidate A) | Per-`u64` work (16 elements) |
|-----|------------------------------|-------------------------------|
| add | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| sub | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| mul | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |
| div | 0.22–0.24 ns                 | 8 LUT loads + 16 shift/mask/OR |

LUT footprint: 4 × 64 KiB = 256 KiB total (fits comfortably in L2; one LUT at
a time fits in L1d for hot loops). Candidate A remains a usable
fallback if D's bit-sliced kernel turns out to have unfavourable SIMD
or proof characteristics in W4.

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

## 5. Bitwise op-count derivation — Candidate D (chosen winner)

Layout: three parallel `Vec<u64>` bit-planes `(b_0, b_1, b_2)` carry the
canonical 3-bit value of each F_5 element. One `u64`-triple covers
**64 elements**. Canonical values are `0..=4`; codepoints `5..=7` are
redundant and never produced by canonical packings (decode of any
non-canonical codepoint maps to 0).

Every binary op uses a **5-way decode** of each operand into mutually-
exclusive selectors `e_0..e_4` (where `e_i = 1` iff the element equals
`i`), followed by a **5×5 cross-product** that gates `e_a[i] & e_b[j]`
into per-result-value selectors `r_0..r_4` (where `r_k = 1` iff the
result equals `k`), and finally an **encode** that combines `r_0..r_4`
into output bit-planes `(c_0, c_1, c_2)`.

Decode (per operand): 11 ops (3 NOTs + 8 ANDs, with shared sub-
expressions). 22 ops total for both operands.

Cross-product cells producing result `0` need no AND because they
contribute nothing to any output bit-plane (the canonical `0` has
all three bits zero). Out of 25 cells per op, the fraction whose
result is non-zero varies by op:

| Op  | Cells producing 0 | Cells producing ≠ 0 | Cross-product ANDs |
|-----|------------------:|--------------------:|-------------------:|
| add |                 5 |                  20 |                 20 |
| sub |                 5 |                  20 |                 20 |
| mul |                 9 |                  16 |                 16 |
| div |                 9 |                  16 |                 16 |

Per `u64`-triple (= 64 F_5 ops):

| Step                                                   | add  | sub  | mul  | div  |
|--------------------------------------------------------|-----:|-----:|-----:|-----:|
| Decode `e_a` (3 NOTs + 8 ANDs)                         |   11 |   11 |   11 |   11 |
| Decode `e_b` (same, sharing sub-expressions)           |   11 |   11 |   11 |   11 |
| Cross-product ANDs                                     |   20 |   20 |   16 |   16 |
| Result-tree ORs to assemble `r_0..r_4`                 |   16 |   16 |   12 |   12 |
| Encode `c_0 = r_1 \| r_3`, `c_1 = r_2 \| r_3`, `c_2 = r_4` |  2 |  2 |  2 |  2 |
| **Total per `u64`-triple (64 elements)**               | **60** | **60** | **52** | **52** |
| **Per element**                                        | 0.94 | 0.94 | 0.81 | 0.81 |

**LUT footprint**: 0 KiB. The 5×5 truth tables live as `[[u8; 5]; 5]`
constants (25 bytes each, four ops = 100 bytes total) which the
compiler folds into the cross-product Boolean expressions at compile
time — no runtime memory accesses.

Compared with Candidate A's **0.5 LUT loads + 4 ALU ops per element**,
D is **structurally heavier in raw ALU ops/element** (~0.9 ops/elem)
but eliminates LUT memory traffic entirely. The net wall-clock
advantage on the bench host (Zen 3, AVX2, 64 KiB L1d) is 1.10–1.20×
across the four ops — measured in §4. D's lead is expected to grow
under SIMD widening because each `u64` plane SIMD-widens trivially
to AVX2 256-bit lanes, while A's gather-against-64-KiB-LUT does not
vectorise cleanly on Zen 3.

### Op-count derivation for Candidate A (runner-up; preserved as historical record)

Each `u64` packs 16 elements at 4-bit-aligned slots; the high bit of
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

**Candidate D (winner under revised §8 rule, originally rejected by the
≥ 1.5× bar).** A genuine bit-sliced 3-plane Boolean implementation.
Decode + cross-product runs in ~50–60 bitwise ops per `u64`-triple over
64 elements ≈ 0.8–1.0 ops/element. **Faster than A on every op (1.10×
on mul/div, 1.20× on add/sub) — no per-op regression**, so under the
revised §8 rule's clause 1 it qualifies. The original "≥ 1.5× margin"
rule rejected it; that rule has been revised after the cross-prime
study (see `dev/plans/r2_packed_encoding_generalizations.md` §5).

D should still be re-benched against A on AVX2 once the W4 SIMD kernel
is up (sibling issue `c7542983`) — each `u64` plane SIMD-widens
one-for-one to AVX2/AVX-512 lanes, while the LUT-based A path is
harder to SIMD-port on AMD Zen 3 (no AVX-512 `vpermb`/IFMA available;
gather-style loads against a 64 KiB table do not vectorise cleanly).
The expectation is that D's lead grows under SIMD; A becomes a clean
fallback for portability.

## 7. Forward implications for W4 (T17–T20)

- **T17 (F_5 packed type + ops)** instantiates the encoding from §1
  (Candidate D, after revised §8): three parallel `Vec<u64>` bit-planes
  for `(b₀, b₁, b₂)` carrying the canonical 3-bit value of each F_5
  element; one `u64`-triple covers 64 elements. Ops via decode-7-then-
  cross-product Boolean circuit derived from the 5×5 truth tables.
  No `OnceLock` LUT needed.
- **T21 (SIMD kernel for F_5 / F_7)** should re-bench D vs A on AVX2
  to confirm D's lead grows under vectorisation. D's bit-plane layout
  widens trivially to AVX2 256-bit lanes; A's gather-against-64-KiB-LUT
  does not. If AVX2 measurement reverses the picture, T21 may keep A
  as a fallback path.
- **V3 (Lean F_5 / F_7 correctness)** — the F_5 proof sketch will now
  target Candidate D: prove that the decode-cross-product circuit
  matches `(a OP b) % 5` per lane against `Fp<5>` arithmetic. The
  proof is closer in shape to the F_3 bipedal proof (truth-table
  Boolean circuit) than to a LUT-correctness proof. The 5×5 truth
  table for each op gives 25 cell propositions per op; the proof
  reduces to verifying that the result-selector ORs assemble those
  cells correctly into the canonical `(c₀, c₁, c₂)` output.

## 8. Recommendation

Adopt **Candidate D — bit-sliced 3-plane Boolean** as the F_5 packed
encoding for the gf2-algebra permanent implementation phase (W4).
D wins under the revised epic §8 rule's clause 1 (faster than A on
every op, no regression). Per-element wall-clock is 0.19 ns add /
0.21 ns mul, beating A's 0.23 ns by 1.20× on add/sub and 1.10× on
mul/div on the bench host (AMD Ryzen 9 5900X, AVX2-only, scalar).

### Decision review (2026-05-03)

The original recommendation in this document was Candidate A under
the original "≥ 1.5× margin or fall back to LUT-A" rule. After the
cross-prime study
(`dev/plans/r2_packed_encoding_generalizations.md`) demonstrated that
the original rule was mis-calibrated — it rejected uniform-but-small
gains that the field structure permits — epic §8 was revised to
"no per-op regression AND faster on at least one op, OR ≥ 1.10×
Ryser-weighted speedup at $n = 36$". Candidate D meets clause 1
unambiguously. T17 now ships with D; T21 (SIMD) re-benches D vs A
under AVX2 to confirm the scalar lead extends.

Candidate A remains documented in §2 as the LUT-A baseline and is
the natural fallback if D's bit-sliced kernel turns out to have
unfavourable AVX2 or proof characteristics. The prototype, benchmark
harness, and all four candidate implementations remain in
`dev/research/f5_packing/` for future re-evaluation.

## 9. User sign-off

User signed off via direct directive 2026-05-03: "close then both F_5
and F_7 issues by doing what's recommended", following review of
the cross-prime comparison and generalization analysis.

(Historical placeholder text from before sign-off: "populated by the
lead after the user comments approval on JIT 6b3f6054 per the
success-criterion 'user has signed off on the chosen encoding'".)
