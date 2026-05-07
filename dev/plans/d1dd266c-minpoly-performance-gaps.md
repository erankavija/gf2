# Minpoly performance gap analysis (`jit:d1dd266c`)

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| Status | Open. d1dd266c implementation landed at `bda98cd` (Wiedemann O(n^3) for large-cardinality fields) but 4 of 8 reference cells still fail the 1.5x SOTA contract. User directive (2026-05-07): no scope-narrowing, no aspirational amendments — close the gaps with proper specialized work. |
| Parent issue | `d1dd266c` (Tune minimal polynomial path) |
| Parent story | `66190ccd` (Close charpoly and minpoly gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Related evidence | `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` (raw measurements + dispatch design) |
| Reference baseline | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` |
| Reference protocol | `dev/plans/sota_reference_acceptance_protocol.md` |

This doc enumerates the unclosed cells, their structural causes, and the
implementation work needed to close each. It does not propose any criterion
amendments — closing every cell on the 1.5x contract is the goal.

---

## § 1 Cells failing the 1.5x contract (post-d1dd266c)

| # | Cell | gf2/fflas | Cause class | Algorithm currently used |
|---|---|---:|---|---|
| 1 | minpoly × GF(7) × n=64 | 52.8x | small-field minpoly + small-field arithmetic | quartic (Wiedemann gate fails: 2^floor(log2(7))=4 ≤ 64) |
| 2 | minpoly × GF(7) × n=256 | 327x | same as #1 | quartic |
| 3 | minpoly × GF(251) × n=64 | 4.90x | small-field arithmetic on Wiedemann path | Wiedemann (gate passes: 2^7=128 > 64) |
| 4 | minpoly × GF(251) × n=256 | 4740x | small-field minpoly + small-field arithmetic | quartic (Wiedemann gate fails: 2^7=128 ≤ 256) |
| 5 | minpoly × GF(65521) × n=256 | 2.26x | scalar matvec inner loop, no SIMD | Wiedemann (gate passes: 2^15 > 256) |

The four cells where Wiedemann's textbook bound `q > n` does not hold (#1, #2,
#4 — and arguably #3 at boundary) need a different algorithm entirely. The
constant-factor cells (#3, #5) need micro-optimization of the arithmetic and
matvec inner loops.

---

## § 2 Required work, by structural cause

### § 2.1 Small-field minpoly when q ≤ n (cells #1, #2, #4)

**Problem.** When the field cardinality `q ≤ n`, scalar Wiedemann is unsafe
(no Las-Vegas guarantee — random projection probability bound `1 - n/q` becomes
non-positive). The current fallback is `find_max_minpoly_generator` which is
O(n^4): it iterates over all `n` canonical basis vectors, runs per-vector
Berlekamp-Massey on each (O(n^3)), and accumulates the LCM.

**fflas-ffpack reference.** fflas uses a deterministic O(n^3) blocked
Krylov / polynomial-lifting method that handles small fields without the
Wiedemann probability constraint. Specifically the
`FFPACK::CharPoly` / `FFPACK::MinPoly` family in `ffpack_charpoly_*.h` uses
`LasVegas` for large fields and a deterministic block-Krylov for small ones
(see `fflas-ffpack/ffpack/ffpack_charpoly_kgfast.inl` and the
`MatPolMul` machinery).

**What needs to be implemented in gf2-core.**

1. A **block Wiedemann** variant (Coppersmith 1994 *Solving Linear Equations
   over GF(2): Block Lanczos Algorithm*; Villard 1997 *A study of Coppersmith's
   block Wiedemann algorithm*). Block size `b ≥ 2` makes the per-step
   probability bound `1 - n/q^b` non-trivial even for small `q`. With `b=8`
   over GF(7), the failure probability per attempt is `≤ 64 / 5764801 ≈ 1.1e-5`
   — comfortably Las-Vegas.
2. Alternative: **Dumas-Saunders-Villard** (Dumas, Saunders, Villard, *On
   Efficient Sparse Integer Matrix Smith Normal Form Computations*, 2001) blocked
   approach with deterministic verification.
3. The dispatch in `minpoly_dispatch` (in `crates/gf2-core/src/field/charpoly.rs`)
   must route small-field cells to the new path instead of the quartic fallback.

**Estimated effort.** 1–2 weeks for a competent Rust + finite-field-algorithms
specialist. The Coppersmith block Wiedemann needs careful matrix-polynomial
multiplication; the Berlekamp-Massey generalization to matrix sequences (the
`MatBM` algorithm of Beckermann-Labahn 1994 / Giorgi-Jeannerod-Villard 2003)
is the central kernel.

**Out of scope for `97bf0879`** unless a specialist is available before the epic
closes. Should be filed as a successor task either inside `66190ccd` (if the
story can stay open) or as a new follow-up issue tagged for the
`gf2-core-finite-field-la-sota` plan in `dev/plans/615db3b9-finite-field-la-sota-plan.md`.

### § 2.2 Small-prime arithmetic on Wiedemann path (cell #3: GF(251)/64)

**Problem.** GF(251) at n=64 runs Wiedemann (gate passes), but the Wiedemann
inner loop is dominated by `2n+2 = 130` matvec calls. Each matvec is O(n^2)
field multiplications. For GF(251), each multiplication uses the same 64-bit
Montgomery path as GF(2^31-1), which is 4–8x more expensive than necessary
for an 8-bit prime that fits in a single byte.

**fflas-ffpack reference.** fflas uses `FFPACK::Modular<int8_t>` for primes
≤ 251 and `FFPACK::Modular<int16_t>` for primes ≤ 32749, packing many field
elements per CPU word and using SIMD for the inner kernel (the `fgemv` /
`fgemm` family).

**What needs to be implemented.**

1. A **`SmallPrimeFp<P>`** type (or specialization of `Fp<P>` for `P ≤ 251`)
   that uses `u8` or `u16` storage with byte-Barrett reduction (no
   Montgomery). Mirrors the Wave-8b `fp_small.rs` packed-int kernel but for
   single-element arithmetic, not just SpMM.
2. SIMD-vectorized matvec for the `SmallPrimeFp` type. Probably mirrors
   the AVX2 `fp_small_spmm_row` from `crates/gf2-kernels-simd/src/x86/fp_small.rs`
   (already exists, used by sparse Path B). The matvec variant would be a
   `fp_small_matvec_row` kernel.
3. Dispatch in `Fp<P>` so the right Montgomery / small-prime path is selected
   based on `P`.

**Estimated effort.** 3–5 days. The AVX2 small-prime SIMD machinery already
exists for the sparse Path B kernels; this is mostly a refactor + dispatch +
new entry point + bench harness extension.

### § 2.3 SIMD matvec for medium primes (cell #5: GF(65521)/256)

**Problem.** GF(65521) at n=256 runs Wiedemann correctly but the matvec is
scalar (no SIMD). 514 matvec calls × 65536 muls per call = 33.7M scalar field
operations, dominated by Montgomery REDC overhead.

**fflas-ffpack reference.** Same as § 2.2 — `Modular<int16_t>` packs 4 GF(65521)
elements per AVX2 lane; `fgemv` uses SIMD-vectorized inner loops.

**What needs to be implemented.**

1. SIMD-vectorized `fp_medium_matvec_row` kernel mirroring
   `fp_medium_spmm_row` (which already exists for Wave-8b sparse Path B,
   16-bit-lane (u16) for P ∈ (251, 65535] with u64 accumulators and
   end-of-row Barrett reduction).
2. Wire it into `FieldMatrix::matvec` for `Fp<P>` with `P ≤ 65521`. Currently
   `matvec` likely calls into the scalar `simd_ops` path or the generic loop.

**Estimated effort.** 2–3 days. The medium-prime SIMD SpMM already exists
(landed in `crates/gf2-kernels-simd/src/x86/fp_medium.rs` at commit `fb4e8f2`);
this is mostly a matvec wrapper + dispatch + bench harness extension.

---

## § 3 Implementation order recommendation

Suggested wave for a specialist:

1. **§ 2.3 first** — closes cell #5, smallest scope, leverages existing
   Wave-8b SpMM kernels. Probably 2 days. Outcome: GF(65521)/256 closes the
   1.5x contract (currently 2.26x).
2. **§ 2.2 next** — closes cell #3 (GF(251)/64). Probably 3-5 days. Builds on
   the existing `fp_small.rs` packed-int machinery. Outcome: GF(251)/64 should
   come down from 4.90x to within 1.5x. (The n=256 row also benefits but is
   still bottlenecked by the small-q-ness, which falls under § 2.1.)
3. **§ 2.1 last** — closes cells #1, #2, #4 by replacing the quartic fallback
   with block Wiedemann. Probably 1–2 weeks. Largest scope; needs careful
   correctness validation against the existing `find_max_minpoly_generator`
   on all q ≤ n cells.

Total: ~3 weeks of specialist work to close all 4 unclosed cells.

---

## § 4 Test discipline

Every change in § 2.1, § 2.2, § 2.3 must:

1. Pass the existing minpoly proptests
   (`proptest_wiedemann_minpoly_annihilates_fp_*` in `charpoly.rs`).
2. Add property-based tests for the new path that verify:
   - `mp(A) · v == 0` for random `A` and `v` (Cayley-Hamilton-style annihilation)
   - `mp | charpoly` (the minpoly divides the characteristic polynomial)
   - Per-cell bit-exact output equals the existing
     `find_max_minpoly_generator` reference on small `n` (n ≤ 16 over each
     of GF(7), GF(251), GF(65521), GF(2^31-1))
3. Re-run the `bench_minpoly_reference_sweep` Criterion group (added by
   d1dd266c at `bda98cd`) and update
   `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` with the new
   ratios.

---

## § 5 Cross-references

- `dev/plans/615db3b9-finite-field-la-sota-plan.md` — broader finite-field LA
  plan; the small-field minpoly + SIMD matvec work above belongs in this
  plan's implementation phase.
- `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` — raw measurements
  + dispatch design for the d1dd266c change that landed at `bda98cd`.
- `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` — fflas-ffpack
  / LinBox / FLINT reference baseline (canonical for analyze.py merge).
- `crates/gf2-core/src/field/charpoly.rs` — current implementation
  (`berlekamp_massey`, `wiedemann_minpoly_attempt`, `minpoly_dispatch`) at
  HEAD `bda98cd`.
- `crates/gf2-kernels-simd/src/x86/{fp_small,fp_medium}.rs` — existing AVX2
  SpMM kernels that the matvec work in § 2.2 / § 2.3 will mirror.
