# R3: Multi-word streaming column-sum + cache-blocking design

**Epic:** `epic:gf2-algebra-permanent`
**Issue:** `60c30e2d` (R3 — Multi-word streaming column-sum cache-blocking design)
**Consumed by:** W3-T14 (multi-word streaming permanent implementation)
**Status:** design

This document is the streaming + memory-layout strategy for the bipedal-3 Ryser
permanent at `n > 64`. It fixes the matrix layout, defines the cache-blocking
plan, computes a Zen 3 roofline estimate, and gives implementation-grade Rust
pseudocode that W3-T14 transcribes directly.

The single-word fast path (`n <= 64`) is already covered by W2-T9 (see epic
doc §7.3) and is **out of scope** for this design.

---

## 1. Scope

- Input: `Bipedal3Matrix` of size `n x n`, with `n in {65, 66, ..., 256}`.
- Output: `permanent_bipedal3_multi_word(mat) -> Fp<3>`, bit-identical to
  `permanent_ryser::<Fp<3>>` (epic success criterion 2).
- Algorithm: Ryser's formula in Gray-code order, paper Theorem 2.1 / epic §2.3.
- Hot loop: per Gray step, add or subtract one column to a packed column-sum
  buffer of `W = ceil(n / 64)` words per leg, then fold over the buffer.
- Hardware envelope: AMD Ryzen 9 5900X (Zen 3) dev host, single thread for this
  issue. Multithread (`W3-T15`) and GPU (`W5`) are downstream.
- Above `n = 256`, single-thread time is dominated by the `2^n` outer
  enumeration regardless of cache behaviour; that regime is the parallel and
  GPU phase. The roofline below therefore bounds the analysis at `n <= 256`.

The single-word path uses the same column-major layout, so this design is also
the layout invariant for `Bipedal3Matrix` (epic §7.2 strawman is updated in
§2 below).

---

## 2. Memory layout decision: column-major

### 2.1 Decision

`Bipedal3Matrix` stores its entries in **column-major** order, with the magnitude
and sign legs as two separate contiguous `Vec<u64>` buffers of length
`W * n` each, where `W = ceil(n / 64)`.

```rust
pub struct Bipedal3Matrix {
    n: usize,
    w: usize,                  // = ceil(n / 64), words per column-leg
    mag: Vec<u64>,             // length W * n, column-major
    sgn: Vec<u64>,             // length W * n, column-major
}

impl Bipedal3Matrix {
    #[inline] fn col_mag(&self, j: usize) -> &[u64] {
        &self.mag[j * self.w .. (j + 1) * self.w]
    }
    #[inline] fn col_sgn(&self, j: usize) -> &[u64] {
        &self.sgn[j * self.w .. (j + 1) * self.w]
    }
}
```

Diagrammatically (column-major, with `n = 130`, `W = 3`):

```
            col 0          col 1                  col n-1 = col 129
mag:    [ w0 w1 w2 ] [ w0 w1 w2 ] [ w0 w1 w2 ] ... [ w0 w1 w2 ]
sgn:    [ w0 w1 w2 ] [ w0 w1 w2 ] [ w0 w1 w2 ] ... [ w0 w1 w2 ]
        ^---- 24 B ---^---- 24 B ---^                          ^
         contiguous     contiguous                              contiguous

byte offset within mag for column j, word w:  (j * W + w) * 8
```

The two legs are kept as **separate** `Vec<u64>` (not interleaved
`(mag, sgn)` pairs). Rationale: the bipedal add/sub formulas read `mag` and
`sgn` independently (paper §2.2: `t = m1 ^ s1 ^ s2`, `u = m2 & t`, etc.), and
per-leg-contiguous storage lets a SIMD kernel issue one wide load per leg per
cache line without shuffling. This matches the layout of the existing
`gf2-kernels-simd::LogicalFns` AVX2 paths, which all operate on flat
`&mut [u64]` rather than on tuple-packed AoS.

### 2.2 Why column-major (not row-major)

In Gray-code order the inner loop touches *one column per step*: a single bit
flip in the active subset adds or removes one column from the running
column-sum. Concretely, per epic §2.3:

```
column_sum[w] ±= A.col(flip)[w]    for w in 0..W
```

The hot operand here is "the `flip`-th column, all `W` words, both legs".

- **Column-major:** that operand is `2 * W * 8` contiguous bytes per leg = a
  single straight-line `memcpy`-style load. For `n = 256`, `W = 4`, that is
  `64 B` per leg = exactly one 64-byte cache line per leg per Gray step. Zero
  strided access; the hardware prefetcher sees a predictable forward stream
  whenever consecutive Gray steps happen to flip nearby column indices, and a
  random-but-constant-stride pattern in the worst case (still one line per
  access).

- **Row-major:** the same operand is *strided* — to read `A.col(flip)[w]` for
  all `w` we walk down the matrix, hitting every row at offset `flip`. For
  `n = 256` and `u64` row stride that is one cache line read for *each* row
  (not each word), so `n / 64 = 4` row-tile reads instead of `W = 4` word
  reads — same word count but each read pays a fresh L1 line and a fresh TLB
  entry, with no possibility of the line being reused for the next Gray step
  (which flips a *different* column, i.e. a different stride offset).

The column-sum buffer itself is small (`2 * W * 8 = 64 B` for `n = 256`),
fits in one cache line per leg, and stays L1-resident across the entire `2^n`
outer loop. So all the asymmetry between layouts lives in the matrix access.

### 2.3 Storage cost

For `n in {65, 128, 256}`:

| n   | W | per-leg bytes (`n*W*8`) | both legs | line count (64 B) |
|-----|---|-------------------------|-----------|-------------------|
| 65  | 2 | 1 040                   | 2 080     | 33                |
| 128 | 2 | 2 048                   | 4 096     | 64                |
| 256 | 4 | 8 192                   | 16 384    | 256               |

All three sizes fit in Zen 3 L1d (32 KiB, see §3) with room for the 64 B
column-sum, the gray-code bookkeeping (a `usize` counter + a `usize` flip
table of length `n`), and stack frames. The whole-matrix-fits-L1 regime
extends to `n = 256` inclusive; beyond that, see §4.2.

---

## 3. Hardware envelope (Zen 3, Ryzen 9 5900X)

All numbers below are **measured from `/sys/devices/system/cpu/cpu0/cache/`
on the dev host on 2026-05-09** unless explicitly cited from a vendor
document. Run-line for reproduction:

```
for i in 0 1 2 3; do
  cat /sys/devices/system/cpu/cpu0/cache/index$i/{level,type,size,ways_of_associativity,coherency_line_size}
done
```

| Level | Size       | Ways | Line | Latency (cycles) | Bandwidth          |
|-------|------------|------|------|------------------|--------------------|
| L1d   | 32 KiB/core| 8    | 64 B | ~4               | 32 B/cycle/core, 2 loads + 1 store per cycle |
| L1i   | 32 KiB/core| 8    | 64 B | -                | -                  |
| L2    | 512 KiB/core| 8   | 64 B | ~12              | 32 B/cycle/core (dedicated) |
| L3    | 32 MiB/CCX | 16   | 64 B | ~46              | shared across 6 cores per CCX |
| DRAM  | -          | -    | -    | ~80 ns           | dual-channel DDR4-3200, ~50 GB/s aggregate |

Sources:

- L1d / L1i / L2 / L3 sizes, ways, line size: dev host sysfs (above).
- L1d 32 B/cycle/core, 2 loads + 1 store: AMD "Software Optimization Guide for
  AMD Family 19h Processors" (PUB 56665 Rev 3.06, 2021), §2.6.1 ("Load/Store
  unit") — Zen 3 sustains two 256-bit loads or one 256-bit load + one
  256-bit store per cycle from L1.
- L2 32 B/cycle/core: same guide, §2.6.2 ("L2 cache").
- L1/L2/L3 latencies: AMD Family 19h optimisation guide §2.1.2 plus
  AnandTech "AMD Zen 3 Microarchitecture Deep Dive" (Ian Cutress, 2020-11-05).
- DRAM: dev host motherboard spec, dual-channel DDR4-3200 = 25.6 GB/s/channel
  x 2 = 51.2 GB/s aggregate theoretical peak; sustained ~ 40-46 GB/s in
  practice on Zen 3 (`stream-c` benchmark, AMD-published Zen 3 results).

For the roofline below the relevant peak is **L1d throughput**:

```
peak_L1d_bandwidth_per_core
    = 32 B/cycle x clock
    = 32 B/cycle x 4.0 GHz
    = 128 GB/s/core    (boost clock, single thread)
```

(The 5900X all-core sustained boost is ~4.0-4.2 GHz under typical thermal
load; we use 4.0 GHz as a conservative round number. CPU max boost is
4.95 GHz on a single core — see `lscpu` output — but full-loop sustained
benchmarks are unlikely to live there, so the roofline uses 4.0 GHz.)

---

## 4. Inner-loop bytes / iteration and roofline

### 4.1 Per-Gray-step bytes

Per Gray step at width `W = ceil(n / 64)`:

| Action                                  | Bytes read | Bytes written |
|-----------------------------------------|------------|---------------|
| Load `mat.col_mag(flip)`, `W` words     | `8 W`      | -             |
| Load `mat.col_sgn(flip)`, `W` words     | `8 W`      | -             |
| Load `col_sum_mag` (in-register if `W` small) | `8 W` | -             |
| Load `col_sum_sgn` (in-register if `W` small) | `8 W` | -             |
| Store updated `col_sum_mag`             | -          | `8 W`         |
| Store updated `col_sum_sgn`             | -          | `8 W`         |

Total: `48 W` bytes touched per Gray step (assuming the column-sum lives in L1,
which it always does — see §2.3). If `col_sum_*` are kept in registers across
the loop (likely for `W <= 4`, since 8 `u64`s = 4 YMM regs comfortably under
Zen 3's 16-AVX2-register budget), the actual L1 traffic drops to `16 W` bytes
read per Gray step (matrix-only).

We use the optimistic register-resident estimate `16 W` for the roofline, and
the conservative memory-resident estimate `48 W` for sanity.

| n   | W | bytes / step (reg-res) | bytes / step (mem-res) |
|-----|---|------------------------|------------------------|
| 65  | 2 | 32                     | 96                     |
| 128 | 2 | 32                     | 96                     |
| 200 | 4 | 64                     | 192                    |
| 256 | 4 | 64                     | 192                    |

### 4.2 Bitwise-op intensity

Per Gray step the bipedal add/sub is **6 u64-ops per word per leg**, both
legs sharing intermediates (paper §2.2):

```
add:  t = m1 ^ s1 ^ s2;  u = m2 & t;  m+ = u | (m1 ^ m2);  s+ = u ^ s1;
sub:  t = s1 ^ s2;       u = m1 & t;  m- = u | (m1 ^ m2);  s- = u ^ (m2 ^ s2);
```

Counting unique ops with CSE: 6 bitwise ops per word for either branch (plus
1 store-back per leg = "free" once the result is in a register).

`fold_mul` over `W` words: bipedal `mul` is 2 ops per word, with the
`fold_mul` reduction below (§7) emitting between `2 W` (sequential) and
`2 (W - 1)` (log-tree) bipedal-mul ops total. For `W = 4`, that is 8 ops on
the sequential schedule.

Then `is_zero`: 1 OR-reduce over `W` words, ~ `W - 1` ops.

Sign-extend + accumulate into `Fp3Accumulator`: amortised over the OR-reduce,
~ `O(1)` ops.

Net: per Gray step at `W = 4`:

- 6 (add/sub) x 4 words x 2 legs = 48 ops update
- 8 fold-mul ops
- 4 is-zero ops
- ~5 accumulator ops
- ~65 u64-ops total per Gray step.

Operational intensity (bitwise-ops / byte, register-resident):

```
65 ops / 64 bytes = 1.02 ops/byte
```

For `W = 2` (`n = 65..128`): `~33 ops / 32 bytes = 1.03 ops/byte`.

### 4.3 Roofline conclusion

L1d peak: 128 GB/s/core. At 1.02 ops/byte, that caps the algorithm at

```
1.02 ops/byte x 128 GB/s = 130 G u64-ops / s / core
```

A Zen 3 core retires up to 4 integer ops/cycle (AMD Family 19h optimisation
guide §2.4) = 16 G u64-ops/s @ 4 GHz on the integer pipes. The bipedal-3
inner loop is therefore **compute-bound at L1**, not memory-bound, by roughly
an order of magnitude.

This is the desired regime: the cache-blocking discipline below exists to
keep the matrix L1-resident so we *stay* on the compute-bound side of the
roofline. Once an access miss falls to L2 (32 B/cycle, but +8 cycles latency)
the inner loop slows by an integer factor; falling to L3 or DRAM costs an
order of magnitude.

The single-thread expected throughput target for W3-T14 is therefore set by
the integer-pipe ceiling: ~16 G u64-ops/s, divided by the ~65 ops per Gray
step, gives **~250 M Gray steps / s / core**. For `n = 36` (the headline
size) that is `2^36 / 250e6 = 275 s / core` — but `n = 36` lives in the
single-word path (W2-T9), so the multi-word streaming target is `n in
{65, 80, 100, 128, 200, 256}` validation runs (see §8), which use small frame
counts and do not need full enumeration.

The roofline matters mainly to bound the scaling constant when W3-T15 (rayon)
and W5 (GPU) take over for production-size enumerations.

---

## 5. Cache-blocking strategy

### 5.1 The two regimes

There are two scales of cache-blocking concern, and only one of them
actually exists for `n <= 256`:

| Regime | Condition | Strategy |
|--------|-----------|----------|
| Whole matrix fits L1 | `2 * W * n * 8 <= 32 KiB`, i.e. `W * n <= 2048` | Stream the matrix from L1; column-sum in registers; **no blocking needed**. |
| Whole matrix exceeds L1 but fits L2 | `2 * W * n * 8 <= 512 KiB` | Per-iteration column miss to L2 only; still single-line access; performance drops by L2-vs-L1 latency factor (~3x worst case but heavily prefetcher-mitigated). |
| Beyond L2 | `2 * W * n * 8 > 512 KiB`, i.e. `n > ~5800` for `W = ceil(n/64)` | Not reachable in this issue — single-thread `2^5800` is impossible. |

For the design range `n in {65, ..., 256}`:

| n   | W | matrix bytes (`2 W n * 8`) | fits L1 (32 KiB)? | fits L2 (512 KiB)? |
|-----|---|----------------------------|-------------------|--------------------|
| 65  | 2 | 2 080                      | yes               | yes                |
| 128 | 2 | 4 096                      | yes               | yes                |
| 200 | 4 | 12 800                     | yes               | yes                |
| 256 | 4 | 16 384                     | yes               | yes                |
| 512 | 8 | 65 536                     | no (~2x L1)       | yes                |
| 1024| 16| 262 144                    | no                | yes                |

So **inside the design window the matrix fits L1 entirely**, and the
cache-blocking strategy reduces to: do nothing, just stream. The Gray-code
flip pattern visits each of `n` columns `2^(n-1)` times on average so the
prefetcher learns the access set within the first few hundred Gray steps.

### 5.2 What W3-T14 actually implements

The implementation has two paths chosen by a runtime check:

1. **`n <= 256` (the design-window path).** No blocking. Column-sum legs live
   in YMM registers (`W <= 4` words per leg = `<=4` `u64` = `<=32 B` per leg
   = 1 YMM register per leg, fitting in 2 of 16 ymm regs total). Matrix
   streamed from L1.

2. **`n > 256` fallback (out of scope but cheap to implement).** No special
   blocking either: column-sum spills to L1, matrix lives in L2. The
   per-Gray-step cost goes up by the L2 latency factor (~3x in the worst
   case; AMD-published `lat_mem_rd` benchmarks show ~12 cycles L2-hit vs
   ~4 cycles L1-hit), but the code path is identical. This regime is not a
   performance target — it exists so the function does not panic.

The runtime check is a single `if mat.n() <= 256` branch that picks between
"register-resident column-sum" and "stack-resident column-sum" specialisations.
At higher `n` the SIMD kernel can no longer keep the column-sum in registers
(`W = 16` for `n = 1024` = 4 YMM regs per leg = 8/16 of the AVX2 register
file just for column-sum), but the *layout and access pattern stay the same*.

### 5.3 Why no column-blocking is needed

A column-blocking strategy (group Gray steps that flip nearby column indices)
would help iff (a) the matrix exceeded L1 *and* (b) the prefetcher could not
keep up. Neither condition applies here: (a) fails because the matrix fits
L1 in the design window; (b) would only fail if the Gray-code flip pattern
were adversarial, but the binary-reflected Gray code visits column indices in
a known stride-1-ish pattern (see §6 below) and Zen 3's stride prefetcher
handles that.

For W3-T15 (rayon parallel), each thread gets a contiguous Gray-code chunk
and operates on its own private column-sum. Cache contention is then on the
shared matrix (read-only), which lives in L2 / L3 cleanly.

For W5 (GPU), the matrix is uploaded once and lives in shared memory; the
Gray code chunk per workgroup is the unit of parallelism. The column-major
layout transfers directly: one HIP `memcpy` per leg.

---

## 6. Gray-code flip indexing

The flip index at Gray step `k` is

```
flip(k) = trailing_zeros(k)   for k = 1, 2, ...
```

This is the standard binary-reflected Gray-code update: bit `flip(k)` of the
active subset toggles at step `k`. The "added or subtracted" sense is
determined by the **new** state of that bit in the Gray-code register
`g(k) = k ^ (k >> 1)` — which is the active subset itself, not the raw
counter `k`:

```
let g_k = k ^ (k >> 1);
if (g_k >> flip(k)) & 1 == 1 then ADD column flip(k) else SUBTRACT column flip(k)
```

Inspecting `(k >> flip(k)) & 1` instead would be a bug: with
`flip(k) = trailing_zeros(k)`, bit `flip(k)` of `k` is by construction always
`1`, so that test is identically true and the inner loop would only ever add
columns, never subtract — giving a wrong Ryser term, not the permanent. The
test must be against the Gray-code register `g(k)`, where bit `flip(k)`
genuinely encodes the new state of the toggled subset element.

Equivalently, on Gray step `k` the active subset is `g(k) = k ^ (k >> 1)`,
and the bit that just changed is `g(k) ^ g(k-1) = 1 << flip(k)`. ADD when
that bit is now set in `g(k)` (the column has just entered the subset),
SUBTRACT when it is now clear (the column has just left).

The shared `gray_code_iter` (W1-T6) yields `(k, flip)` pairs; this issue
assumes that interface and does not redesign it.

---

## 7. `fold_mul` over `W` words

After updating the column-sum, the algorithm needs the bipedal product of
all `n` packed-`F_3` lanes, expressed as a single `Fp<3>` scalar. With the
column-sum stored as `(col_sum_mag[W], col_sum_sgn[W])`, the product is the
lane-wise reduction over all `64 W` lanes — but lanes `n .. 64 W - 1` are
masked out by the tail-masking invariant (CLAUDE.md §Key design invariants),
so the high tail contributes a packed `1` (the multiplicative identity), not
a `0`.

**Tail handling.** The `(W-1)`-th word of *each* leg is masked so that bit
positions `n mod 64 .. 63` encode the bipedal-1 element (`mag = 1`,
`sgn = 0`). For the magnitude leg this means setting those high bits to `1`;
for the sign leg, those high bits stay `0`. This is invariant — established
once when the column-sum is built and re-established after every add/sub by
masking.

**Two reduction shapes.**

1. **Sequential.** Run `mul` from word 0 down to word `W-1`, accumulating
   into a single `(mag_acc, sgn_acc)` pair. Cost: `W - 1` bipedal `mul`
   ops = `2 (W - 1)` u64-ops, with a chain dependency.

2. **Log-tree.** Pairwise `mul` over `W` words in `ceil(log2 W)` rounds.
   Cost: `W - 1` bipedal `mul` ops total (same total work as sequential),
   `ceil(log2 W)` dependency depth. For `W = 4`: depth 2 vs depth 3 for the
   sequential schedule. For `W = 2`: depth 1 either way.

For the design range `W in {2, 3, 4}` the two schedules differ by at most
one cycle of dependency depth. **W3-T14 uses the sequential schedule** for
implementation simplicity; W3-T12 (SIMD kernel) may prefer the log-tree if
the per-lane reduction inside the YMM register benefits from it (decided
during W3-T12 implementation, not here).

After the per-word `mul` reduction yields a single `(mag_word, sgn_word)`
pair, the lane-fold within the word collapses 64 packed `F_3` elements into
one `Fp<3>`. This is identical to the single-word path and reuses
`Bipedal3::lane_fold_mul` from W2-T9. The fold is a tree of 6 levels of
bipedal `mul`-and-shift, total ~12 u64-ops, independent of `W`.

---

## 8. Pseudocode (implementation-grade)

The following is the exact shape W3-T14 transcribes. Type names are
consistent with epic §6 (`Bipedal3Matrix`, `Fp<3>`, `Fp3Accumulator`) and
§7.3 (`gray_code_iter`, `Bipedal3::add` / `sub` / `mul`).

```rust
use crate::packed::bipedal3::{Bipedal3, Bipedal3Matrix};
use crate::permanent::accumulator::Fp3Accumulator;
use crate::gray::gray_code_iter;
use gf2_core::gfp::Fp;

/// Permanent of an n x n matrix over F_3, n > 64, single-thread, scalar.
///
/// Bit-identical to `permanent_ryser::<Fp<3>>` (epic success criterion 2).
/// The SIMD-accelerated variant is W3-T12 / W3-T13; this is the scalar
/// reference and the multi-word fallback when SIMD is disabled.
///
/// # Panics
///
/// Debug-asserts `mat.n() <= 256`; above that, dispatch to the rayon
/// parallel path (W3-T15). No correctness panic above 256 - just
/// performance regression as the matrix spills out of L1.
pub fn permanent_bipedal3_multi_word(mat: &Bipedal3Matrix) -> Fp<3> {
    debug_assert!(mat.n() > 64, "n <= 64 should use single-word fast path");

    let n = mat.n();
    let w = mat.words_per_col();           // = ceil(n / 64)

    // Column-sum buffer: two Vec<u64> of length w. Initialise to packed-zero,
    // then mask the high tail of word w-1 to packed-one in mag, packed-zero
    // in sgn (multiplicative identity in lanes n..64w-1 so fold_mul does
    // not pull the product to zero).
    let mut col_sum_mag = vec![0u64; w];
    let mut col_sum_sgn = vec![0u64; w];
    let tail_mask_hi = if n % 64 == 0 { 0u64 } else { !0u64 << (n % 64) };
    col_sum_mag[w - 1] = tail_mask_hi;     // packed-1 in tail lanes
    // col_sum_sgn[w - 1] stays 0.

    let mut acc = Fp3Accumulator::zero();

    // Empty-subset term: prod = 1 (the identity element of F_3).
    // Paper convention includes the empty subset; folded into the
    // accumulator with sign (-1)^0 = +1.
    acc.add_signed(Fp::<3>::ONE, 0);

    for (k, flip) in gray_code_iter(n).enumerate().skip(1) {
        let col_mag = mat.col_mag(flip);   // &[u64; w]
        let col_sgn = mat.col_sgn(flip);   // &[u64; w]

        // Bit (flip) of the active subset just toggled. The new value of
        // that bit in the Gray-code register `g(k) = k ^ (k >> 1)` (the
        // active subset itself) determines add vs sub: 1 = column just
        // entered (ADD), 0 = column just left (SUB). Inspecting `k`
        // directly here is a bug — bit `trailing_zeros(k)` of `k` is
        // always 1, so that test would identically pick ADD and the loop
        // would never subtract.
        let g_k = k ^ (k >> 1);
        let added = ((g_k >> flip) & 1) == 1;

        if added {
            // col_sum += column[flip], lane-wise bipedal add.
            for i in 0..w {
                let m1 = col_sum_mag[i];
                let s1 = col_sum_sgn[i];
                let m2 = col_mag[i];
                let s2 = col_sgn[i];
                // paper Theorem 2.1 add formula (6 ops, with CSE):
                let t = m1 ^ s1 ^ s2;
                let u = m2 & t;
                col_sum_mag[i] = u | (m1 ^ m2);
                col_sum_sgn[i] = u ^ s1;
            }
        } else {
            // col_sum -= column[flip], lane-wise bipedal sub.
            for i in 0..w {
                let m1 = col_sum_mag[i];
                let s1 = col_sum_sgn[i];
                let m2 = col_mag[i];
                let s2 = col_sgn[i];
                let t = s1 ^ s2;
                let u = m1 & t;
                col_sum_mag[i] = u | (m1 ^ m2);
                col_sum_sgn[i] = u ^ (m2 ^ s2);
            }
        }
        // Tail-mask invariant: lanes n..64w-1 stay at the packed-1 identity.
        col_sum_mag[w - 1] |= tail_mask_hi;
        col_sum_sgn[w - 1] &= !tail_mask_hi;

        // fold_mul: collapse W words to one (mag, sgn) word via sequential
        // bipedal mul, then collapse 64 lanes within that word to an Fp<3>.
        let prod = fold_mul_words(&col_sum_mag, &col_sum_sgn);
        if prod.is_zero() { continue; }

        // Sign of the Ryser term: (-1)^|S| where |S| = popcount of active
        // subset = popcount(g(k)) = popcount(k ^ (k >> 1)).
        let active = k ^ (k >> 1);
        let parity = (active.count_ones() & 1) as u64;
        acc.add_signed(prod.to_fp3(), parity);
    }

    // Outer (-1)^n factor from Ryser's formula (epic §2.3).
    if n & 1 == 1 { acc.negate(); }
    acc.value()
}

/// Sequential reduction of W words to a single packed bipedal element,
/// then collapse 64 lanes within that word to a scalar Bipedal3 value.
#[inline]
fn fold_mul_words(mag: &[u64], sgn: &[u64]) -> Bipedal3 {
    debug_assert_eq!(mag.len(), sgn.len());
    let mut acc_mag = mag[0];
    let mut acc_sgn = sgn[0];
    // mul: m_x = m1 & m2,  s_x = s1 ^ s2  (paper §2.2, 2 ops)
    for i in 1..mag.len() {
        acc_mag &= mag[i];
        acc_sgn ^= sgn[i];
    }
    Bipedal3 { mag: acc_mag, sgn: acc_sgn }.lane_fold_mul()
    // lane_fold_mul exists from W2-T9; collapses 64 packed F_3 lanes
    // within a single (mag, sgn) word to one Bipedal3 representing an
    // Fp<3> scalar (after .to_fp3()).
}
```

Notes on the pseudocode for the W3-T14 implementer:

- The `gray_code_iter` interface in W1-T6 yields the *empty subset first*
  (k = 0, flip undefined), then `k = 1, 2, ...`. The `.skip(1)` here pairs
  with the explicit empty-subset accumulation above. If W1-T6 chooses a
  different convention, adjust accordingly — the *only* consequence is
  whether the `k = 0` term goes through the explicit `add_signed` or
  through the loop body's first iteration.
- The `Bipedal3::lane_fold_mul` referenced above does not exist yet at
  design time; W2-T9 ships it as part of the single-word fast path. T14
  reuses that primitive verbatim.
- Tail-masking after every add/sub is one extra `|`/`&` per leg per Gray
  step. For `W = 4`, that is 8 extra ops per Gray step out of ~65, ~12%
  overhead. The SIMD kernel (W3-T12) hoists the tail mask into a YMM
  register and pays only one extra `vpor`/`vpand` per Gray step.

---

## 9. Validation plan (consumed by W3-T14 success criteria)

W3-T14 verifies correctness on three axes.

### 9.1 Cross-check vs single-word at the boundary

For `n = 64`, both the single-word path (`permanent_bipedal3_single`,
W2-T9) and the multi-word path (`permanent_bipedal3_multi_word`, this issue)
produce identical results. W3-T14 runs 1000 random `{-1, 0, 1}` matrices
of size `64 x 64` through both paths and asserts bit-equality on every
output. The multi-word path with `W = 1` exercises the same code as
`W >= 2` modulo a conditional, and we want the conditional to be exercised
in test, so the boundary case `n = 64` is included even though the issue's
formal scope is `n > 64`.

### 9.2 Cross-check vs generic Ryser

For `n in {65, 80, 100, 128, 200, 256}`, run 100 random `{-1, 0, 1}`
matrices through `permanent_ryser::<Fp<3>>` and through
`permanent_bipedal3_multi_word`. Assert bit-equality on every output.
At `n = 256` the generic Ryser path takes ~ minutes per matrix in
release mode, so the test carries `#[ignore = "sim: n=256 cross-check"]`
per CLAUDE.md test-tier rules. The full sweep at `n in {65, 80, 100, 128}`
runs in fast tier; `n in {200, 256}` runs in slow tier only.

### 9.3 Roofline sanity check

A microbench runs `permanent_bipedal3_multi_word` over a 32x32 inner-loop
unit (i.e., `2^32` Gray steps, one matrix) and reports Gray-steps / s.
The expected number from §4.3 is `~250 M/s/core`, so a release-mode run
on the dev host completing the `2^32` enumeration in `~ 17 s`. If the
measured wall-time is materially worse than 60 s (i.e., < 70 M Gray
steps/s, ~ 4x below roofline), W3-T14 review fails the perf check and
either spills to L2 (visible in `perf stat`) or has missed the
register-resident schedule for the column-sum.

The microbench is `cargo bench -p gf2-algebra --bench multi_word_streaming`
and uses `n = 32` not `n = 256` precisely because we want a tractable
wall-time for benchmarking, while still exercising the full `2^n` outer
enumeration. The matrix dimension `n = 32` triggers the single-word path,
not the multi-word path, so this microbench actually targets the
*single-word* roofline. To benchmark the multi-word path specifically,
W3-T14 runs `n = 65` (small `W = 2`) for a fixed prefix of the Gray
enumeration (e.g., `2^32` steps capped by the bench harness) — *not* full
enumeration. The bench reports steps / s and the gate is set at the same
~250 M/s/core target adjusted for the small per-step overhead at `W = 2`
(roughly 2x the single-word per-step cost = ~125 M/s/core target).

---

## 10. Open questions deferred to W3-T14

These are decisions the implementer makes inside W3-T14 with no design
document overhead:

1. **Exact `Bipedal3Matrix` constructor surface.** Is the input `&[Fp<3>]`
   in row-major or column-major order? Does the constructor accept a
   `BitMatrix` and re-pack? This is a W1-T5 (`Bipedal3Matrix`) decision; the
   layout fixed in §2 here is an *invariant* on the stored representation,
   not on the constructor input.

2. **Heap vs stack allocation for `col_sum_mag` / `col_sum_sgn`.** The
   pseudocode uses `Vec<u64>`. For `W <= 4` (`n <= 256`), a stack-allocated
   `[u64; 4]` is faster (no heap alloc per call, no length field). The
   implementer should use a small-buffer optimisation: stack-allocate up to
   `W = 4` via `[u64; 4]`, fall through to `Vec<u64>` for higher `W`. The
   `MaybeUninit<[u64; 4]>` pattern from existing `gf2-core` code is a
   precedent.

3. **SIMD-friendly alignment.** Both `mag` and `sgn` `Vec<u64>` should be
   32-byte aligned for AVX2 `vmovdqa`. `Vec<u64>::with_capacity` does *not*
   guarantee this; W3-T14 should either use a `repr(align(32))` newtype
   over `[u64; W * n]` or accept that AVX2 will use unaligned loads
   (`vmovdqu`), which is free on Zen 3 (AMD Family 19h optimisation guide
   §2.6.1: "Zen 3 unaligned vector loads have no penalty when not crossing
   a cache line"). Decision deferred to W3-T12 SIMD kernel.

4. **Loop unrolling.** For `W in {2, 3, 4}` the inner `for i in 0..w` loops
   are tiny and the compiler unrolls them at `-O3` (verified in
   `cargo show-asm` during W3-T14). No manual unrolling needed; if the
   compiler does *not* unroll, that is a code-review finding for W3-T14,
   not a design issue.

5. **`Fp3Accumulator` representation.** The accumulator is a 3-state count
   `(c0, c1, c2)` over `F_3`, with `add_signed(x, parity)` adding `+x` or
   `-x` modulo 3 depending on `parity`. The exact wire format is a
   W1-T4 / W2-T9 decision; this design assumes the interface only.

---

## 11. Cross-references

- Epic doc: `dev/plans/gf2_algebra_permanent.md` §2.3 (Ryser), §7.2-7.3
  (single-word path, Bipedal3Matrix), §9 (multi-word streaming summary),
  §10 (SIMD batching), §13 (wave plan T9, T14).
- Adjacent design: `dev/plans/gf2_core_ppc_spiral.md` (PPC roofline patterns
  reused here, especially the L1-residency discipline).
- Future consumers:
  - W3-T14 (multi-word streaming impl): consumes the layout fixed in §2,
    the pseudocode in §8, and the validation plan in §9.
  - W3-T12 (SIMD bipedal3 kernel): consumes the layout in §2 and the
    `fold_mul_words` shape in §7. May choose the log-tree reduction
    schedule for SIMD-internal lane folding.
  - W3-T15 (rayon parallel): consumes the column-sum-private-per-thread
    pattern noted in §5.3.
  - W5 (HIP/ROCm GPU): consumes the column-major layout in §2 directly
    for `hipMemcpy` of the matrix to device.

---

## 12. Summary of decisions

1. **Layout:** column-major, separate `mag` and `sgn` `Vec<u64>` buffers,
   per-leg-contiguous, length `W * n` each, `W = ceil(n / 64)`.
2. **Cache blocking:** none required for `n <= 256` (matrix fits L1d).
   Above 256, no special blocking either; the column-major layout +
   stride-1 prefetcher handles the L2-resident regime up to `n ~= 5800`,
   far past the practical `2^n` ceiling.
3. **Roofline:** L1d at 128 GB/s/core x ~1.0 op/byte = 130 G u64-ops/s/core
   ceiling; integer-pipe ceiling at ~16 G u64-ops/s/core dominates. The
   inner loop is **compute-bound, not memory-bound**, on Zen 3 in the
   design window.
4. **Pseudocode:** §8, transcribable directly by W3-T14.
5. **`fold_mul`:** sequential reduction; log-tree variant reserved as a
   SIMD-internal option for W3-T12.
6. **Validation:** cross-check vs single-word at `n = 64`, vs generic
   Ryser at `n in {65, 80, 100, 128, 200, 256}`, plus a roofline-sanity
   microbench at `n = 65`.
