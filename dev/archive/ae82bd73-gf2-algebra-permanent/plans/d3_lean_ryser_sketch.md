# D3 — Lean4 proof sketch: Ryser's formula on $\mathbb{F}_3$, bounded $n \le 63$

**Issue:** `4aaa6e4d` (W0 / D3)
**Epic:** `epic:gf2-algebra-permanent` (input: `dev/plans/gf2_algebra_permanent.md` §§2.3, 7.3, 12 (V2))
**Status:** sketch — user approval required per CLAUDE.md §Verification work
**Sibling sketch:** D2 — bipedal $\mathbb{F}_3$ correctness (separate document)
**Pre-read of Mathlib confirmed:** `Matrix.permanent` exists in `Mathlib/LinearAlgebra/Matrix/Permanent.lean`; **no Ryser identity is currently in Mathlib**. The sketch therefore plans to prove the identity in this project (with a candidate upstream PR flagged as out-of-scope future work).

This document is a *sketch only*: it lists the lemmas, names the tactic per lemma in one line, fixes the extraction target, and predicts the Aeneas-generated def names. **No proof bodies are included.** The implementation issue (V2 in epic §13 W6) is dispatched only after this sketch is approved.

---

## 1. Statement

The headline theorem the V2 implementation issue is committing to:

```lean
namespace gf2_algebra.permanent.bipedal3

/-- Ryser's permanent formula, Charon-extracted from the monomorphised
    Rust entrypoint `permanent_ryser_fp3 : &Bipedal3Matrix -> Fp<3>`,
    matches Mathlib's `Matrix.permanent` over `ZMod 3`, for matrix sizes
    `n ≤ 63` (i.e. bounded so the Gray-code subset register fits in a
    single `u64`). -/
theorem permanent_ryser_fp3_correct
    {n : ℕ} (h_n : n ≤ 63) (M : Bipedal3Matrix)
    (h_dim : M.n = n) :
    decode_fp3 (permanent_ryser_fp3 M) =
      Matrix.permanent (matrix_of_bipedal3 M h_dim)
```

Auxiliary `decode_fp3 : Fp<3> → ZMod 3` and
`matrix_of_bipedal3 : (B : Bipedal3Matrix) → B.n = n → Matrix (Fin n) (Fin n) (ZMod 3)` are
defined in the proof preamble; both reduce to canonical lane-extraction
through the bipedal codec $\psi$ (paper §2.1) composed with the
`Fp<3> ↔ ZMod 3` ring isomorphism already present in `Gf2Core/Proofs/FpField.lean`
(specialised to $P = 3$).

The Aeneas extraction will produce a `Result`-monad version
`gf2_algebra.permanent.bipedal3.permanent_ryser_fp3 : Bipedal3Matrix -> Result Fp3`;
the theorem statement above silently passes through `spec_imp_exists` in the same
pattern as `MontgomeryRoundtrip.lean`.

### Why $\mathrm{ZMod}\,3$ as the target ring

`Matrix.permanent` requires `[CommSemiring R] [DecidableEq n] [Fintype n]`
(verified by reading `Mathlib/LinearAlgebra/Matrix/Permanent.lean:27,32`).
`ZMod 3` is the canonical `CommRing` with characteristic 3 in Mathlib;
`Fp<3>` is `RingEquiv` to it via the Montgomery roundtrip already proved
in `proofs/Gf2Core/Proofs/FpField.lean` for the general `ValidPrime P`
case ($P = 3$ is `ValidPrime` because `3` is prime, $1 < 3$, and $3 \le 2^{63}$).

### Index type

The theorem above takes a Mathlib-side natural `n` and a `Bipedal3Matrix`
whose `n` field is propositionally equal. The `Matrix` is over `Fin n × Fin n`
(rather than `Fin M.n` directly) so that downstream Mathlib lemmas using
`[Fintype]`/`[DecidableEq]` apply without juggling decidability instances
on the Rust-extracted struct.

---

## 2. Bounded-$n$ rationale

The bound $n \le 63$ is **not** an artefact of the Rust implementation alone
but a deliberate scope choice for the proof. Reasons, in priority order:

1. **Single-word Gray-code register.** The Gray-code subset
   $S_k \subseteq \{0,\dots,n-1\}$ at step $k$ is encoded as one `u64`
   (`g_k = k XOR (k >> 1)`). For $n \le 63$ all subsets fit in one `u64`
   and the bit-flip index $\mathrm{ctz}(k) < 64$ is always well-defined.
   `bv_omega` and `bv_decide` (via `Mathlib.Tactic.BVDecide`) discharge
   index-bound side conditions in this regime; once $n = 64$ is allowed
   the same tactics still work, but $n > 64$ requires a multi-word
   register — the sketch deliberately stops short of the $n=64$ corner
   to keep the decidability story clean.
2. **Finite-arity Ryser sum.** With $n$ a fixed `ℕ` numeral (or, in the
   generic statement, a fixed but free `ℕ` bounded by 63), the outer
   Ryser sum over $\mathcal{P}(\{0,\dots,n-1\})$ is a finite
   `Finset.sum` with $2^n \le 2^{63}$ terms. The proof uses
   `Finset.sum_bij` over a *bounded* enumeration. Unbounded $n$ would
   require formalising arbitrary-arity finite sums indexed by `Nat`-bit
   subsets — **possible but a separate epic** since none of the
   downstream Rust API depends on $n > 63$.
3. **Production code path.** The single-`u64` `permanent_bipedal3_single`
   path the proof targets has `debug_assert!(mat.n() <= 64)` (epic §7.3).
   The proof's bound is one tighter to keep `mod 64`/index-safety
   reasoning straightforward; the multi-word path
   (`permanent_bipedal3_multi`, epic §9) is **not** in scope for V2.
4. **Risk register alignment.** Risk #3 (Lean Ryser scope explosion) and
   the V3 aspirational tier in epic §12 explicitly call out this trade.

Out-of-scope follow-ups recorded in §8 below.

---

## 3. Lemma list — one-line tactic per lemma

The lemma names below are draft; the implementation issue may rename for
style consistency. Counts: 9 named lemmas (3 Gray-code, 4 Ryser-formula,
2 connecting). Plus 2 unconditionally-axiom-free auxiliaries that fall out
of `Mathlib`.

### 3.1 Gray-code traversal (auxiliary; see §6 for the deeper sketch)

| # | Name | Statement (informal) | Tactic |
|---|---|---|---|
| L1 | `gray_code_register` | `g_k = k XOR (k >> 1)` for `k : Fin (2^n)`, `n ≤ 63`. | `unfold` + `bv_decide` (or `native_decide` at fixed `n`). |
| L2 | `gray_flip_bit_lt` | The flipped-bit index `ctz(k+1) < n` whenever `k < 2^n - 1`. | `bv_omega` after `Nat.ctz`-unfolding lemma + `Nat.lt_of_succ_lt`. |
| L3 | `gray_subset_bijective` | The map `k : Fin (2^n) ↦ subset_of_bits g_k : Finset (Fin n)` is a bijection onto `(Finset.univ : Finset (Fin n)).powerset`. | `Function.Bijective.toEquiv` + induction on `n` using `gray_step_xor` lemma; cardinality check by `Finset.card_powerset` + `Fintype.card_fin`. |

L1 and L2 are pure `BitVec`/`Nat` facts and are likely candidates for
`bv_decide` (in scope on Lean 4.28-rc1 via `Mathlib.Tactic.BVDecide`,
already a Mathlib transitive dependency in `proofs/lakefile.lean`).

### 3.2 Inner-loop column-sum invariant

Production loop body (epic §7.3): at step $k$, after one bit-flip,
`col_sum` holds $\sum_{j \in S_k} \text{column}_j(M)$ as a `Bipedal3Vec` of
length $n$.

| # | Name | Statement (informal) | Tactic |
|---|---|---|---|
| L4 | `col_sum_invariant` | Loop invariant: at step $k$, `decode_bipedal3_vec col_sum = (fun i => ∑ j in S_k, M i j)` over `ZMod 3`. | Induction on $k$; the bipedal `add_in_place` / `sub_in_place` correctness reduces to D2's V1 lemma `bipedal3_add_decode`/`bipedal3_sub_decode` lane-wise; bookkeeping by `Finset.sum_insert` / `Finset.sum_erase` based on whether the flipped bit is added or subtracted. |
| L5 | `inner_prod_eq_column_product` | At step $k$, `prod := col_sum.fold_mul()` decodes to `∏ i in Finset.univ, ∑ j in S_k, M i j` over `ZMod 3`. | Induction on the log-tree fold using `bipedal3_mul_decode` (V1) at each step; `Finset.prod_eq_prod_fold` for the structural fold-vs-`Finset.prod` identification. |

The D2 sketch's per-op lemmas (`bipedal3_{add,sub,mul,div}_decode`) are
prerequisites; D3 references them by name without re-proving them.

### 3.3 Outer accumulator and Ryser identity

| # | Name | Statement (informal) | Tactic |
|---|---|---|---|
| L6 | `outer_acc_eq_alternating_sum` | After the full Gray-code walk, `decode_fp3 acc = (-1)^n · ∑ S in (univ : Finset (Fin n)).powerset, (-1)^|S| · ∏ i, ∑ j in S, M i j`, over `ZMod 3`. | `Finset.sum_bij` along the Gray-code bijection (L3); parity factor from L4 + `gray_code_parity_eq_subset_card_parity` (a one-line corollary from L3 since each Gray step toggles the parity of $|S|$). |
| L7 | `ryser_eq_permanent_zmod` | Pure-Mathlib identity (no Rust): for `M : Matrix (Fin n) (Fin n) (ZMod 3)`, $\operatorname{perm}(M) = (-1)^n \sum_S (-1)^{|S|} \prod_i \sum_{j \in S} M_{ij}$. | `Finset.sum_pow` (which is the multinomial expansion of $\prod_i \sum_j M_{ij}$, via `Mathlib/Data/Nat/Choose/Multinomial.lean`) + `Finset.sum_comm` to swap subset/permutation orderings + standard inclusion-exclusion (`Finset.inclusion_exclusion_sum_biUnion` or a direct `Finset.sum_powerset_neg_one_pow_card`-style telescoping; the latter exists at `Mathlib/Data/Nat/Choose/Sum.lean:194`). The proof of L7 is the **most algebra-heavy** lemma; expect 30–80 lines. |

L7 is general-ring (any `CommRing R`); its specialisation to `ZMod 3` is
free. It is also the natural candidate to upstream to Mathlib as
`Matrix.permanent_eq_ryser` once the project version is stable.

### 3.4 Top-level chain

| # | Name | Statement (informal) | Tactic |
|---|---|---|---|
| L8 | `permanent_ryser_fp3_value` | Spec form (`⦃...⦄`): the extracted `permanent_ryser_fp3` returns `ok r` with `decode_fp3 r = (-1)^n · Σ_S (-1)^|S| · …`. | `progress` walking each Aeneas-generated step; substitute L4, L5 at the inner-loop fold; substitute L6 at loop exit; `spec_imp_exists` to close. |
| L9 | `permanent_ryser_fp3_correct` (main) | The headline statement of §1. | `rw [L8]; rw [← L7]; rfl` (modulo the `decode_fp3 ↔ ZMod 3` isomorphism unfold; expected ≤ 10 lines). |

### 3.5 Auxiliaries (no proof obligations beyond Mathlib lookups)

- `Fp3_RingEquiv_ZMod3 : FpVal P ≃+* ZMod 3` — instantiation of the
  general `FpVal P ≃+* ZMod P.val` already present in
  `proofs/Gf2Core/Proofs/FpField.lean` at $P = 3$. Cited but not proved.
- `Bipedal3Matrix.matrix_of` — the decode function lifting a
  `Bipedal3Matrix` to `Matrix (Fin n) (Fin n) (ZMod 3)`. Definition only;
  no proof.

---

## 4. Monomorphisation — extraction target

**Charon/Aeneas extraction has known issues with generic Rust trait
dispatch.** Concretely (cited in epic §12 V2 and CLAUDE.md memory under
"Charon Fixes Applied"): generic trait methods can fail to translate when
their associated types appear in HRTBs or in implied clauses; the local
Charon build at `/data/aeneas-build/charon/` includes three patches that
unblock the prime-field tower (`gfp/`, `gfpn/`) but not arbitrary generic
arithmetic. Extracting `permanent_ryser<F: FiniteField>` is therefore
**out of scope for V2** — both because the trait-method dispatch is
fragile and because the proof would have to thread `FiniteField` axioms
through L4–L7 with no concrete benefit.

**Extraction target (single function, monomorphised at $\mathbb{F}_3$):**

```rust
// crates/gf2-algebra/src/permanent/bipedal3.rs
pub fn permanent_ryser_fp3(mat: &Bipedal3Matrix) -> Fp<3>
```

This is the **W2/T7-equivalent function specialised to $\mathbb{F}_3$
with no trait-method indirection** — every operation in its body is
either a `Bipedal3` bitwise primitive (already V1-verified per D2) or a
`Fp<3>` arithmetic call (already V0-verified via existing
`MontgomeryRoundtrip.lean`).

If the freezed Rust API at the W6 `gate:api-freeze` (epic §13) instead
exposes only a generic `permanent_ryser<F>` with `F = Fp<3>` instantiation,
the V2 implementation issue is responsible for adding a thin
`pub fn permanent_ryser_fp3 = permanent_ryser::<Fp<3>>` re-export and
extracting *that*; the inner generic is then opaque to Charon. **This is
a hard extraction-target requirement** and is recorded as a dependency
edge from V2 → `gate:api-freeze` in §7.

### Charon `--start-from` set additions

The current `scripts/verify-lean.sh` extracts `gf2_core::gfp` and
`gf2_core::gfpn`. V2 will require:

```bash
charon cargo \
  --preset aeneas \
  --start-from 'gf2_algebra::permanent::bipedal3::permanent_ryser_fp3' \
  --start-from 'gf2_algebra::packed::bipedal3' \
  --start-from 'gf2_algebra::gray::gray_code_iter' \
  --opaque 'gf2_core::field' \
  ...
```

with `gf2-core` items left opaque except for the `Fp<3>`-bearing path
already extracted (`gfp/`). The V2 implementation issue is responsible
for verifying the `--start-from` set still produces a translatable LLBC
(an extraction-feasibility smoke is part of V2's success criteria).

---

## 5. Expected Aeneas-generated def names

Following the naming pattern observed in `proofs/Gf2Core/Funs.lean` for
`gf2_core::gfp::*` (e.g. `gf2_core.gfp.montgomery.compute_r_mod_p`),
the V2 extraction will produce, in `proofs/Gf2Algebra/Funs.lean` (new
sub-tree under `proofs/`):

| Rust path | Predicted Lean def name |
|---|---|
| `gf2_algebra::permanent::bipedal3::permanent_ryser_fp3` | `gf2_algebra.permanent.bipedal3.permanent_ryser_fp3` |
| `gf2_algebra::packed::bipedal3::Bipedal3::add` | `gf2_algebra.packed.bipedal3.Bipedal3.add` |
| `gf2_algebra::packed::bipedal3::Bipedal3::sub` | `gf2_algebra.packed.bipedal3.Bipedal3.sub` |
| `gf2_algebra::packed::bipedal3::Bipedal3::mul` | `gf2_algebra.packed.bipedal3.Bipedal3.mul` |
| `gf2_algebra::packed::bipedal3::Bipedal3Vec::add_in_place` | `gf2_algebra.packed.bipedal3.Bipedal3Vec.add_in_place` |
| `gf2_algebra::packed::bipedal3::Bipedal3Vec::sub_in_place` | `gf2_algebra.packed.bipedal3.Bipedal3Vec.sub_in_place` |
| `gf2_algebra::packed::bipedal3::Bipedal3Vec::fold_mul` | `gf2_algebra.packed.bipedal3.Bipedal3Vec.fold_mul` |
| `gf2_algebra::packed::bipedal3::Fp3Accumulator::{zero,add_signed,negate,value}` | `gf2_algebra.packed.bipedal3.Fp3Accumulator.{zero,add_signed,negate,value}` |
| `gf2_algebra::gray::gray_code_iter` (and the `Iterator::next` impl) | `gf2_algebra.gray.gray_code_iter` and `gf2_algebra.gray.GrayCodeIter.next` (Aeneas materialises iterator next as a separate def) |

A new `proofs/Gf2Algebra/` directory mirrors the existing
`proofs/Gf2Core/` layout, with `Types.lean`, `Funs.lean`, `Funs/`-split
files, and a `Proofs/` sub-tree for the hand-written V2 proofs. The
`lakefile.lean` gains a second `lean_lib Gf2Algebra` entry. **This
restructuring is part of V2's deliverable, not D3's.**

### Result-monad shape

Every extracted Rust function returns `Result T` (Aeneas convention).
Progress lemmas mirror `proofs/Gf2Core/Proofs/Progress.lean`. Prediction:

```lean
@[progress]
theorem permanent_ryser_fp3_progress (M : Bipedal3Matrix) :
    gf2_algebra.permanent.bipedal3.permanent_ryser_fp3 M
      ⦃ r => decode_fp3 r = ... ⦄ := ...
```

with the `...` being L8's content. The `progress`/`spec_imp_exists`
pattern is already established in `MontgomeryRoundtrip.lean` and should
transfer with no surprises.

---

## 6. Gray-code traversal — detailed sketch

The Gray-code part is the most novel piece of the proof; the rest is
"standard Mathlib + Ryser". Mathlib does **not** currently contain a
formalisation of the binary reflected Gray code (verified by grep
2026-05-09 against the mathlib4 tree at v4.28.0-rc1 vendored in
`proofs/.lake/packages/mathlib/`; no hits for `gray.code`, `Gray`, or
`reflected.code`). V2 therefore writes a small standalone namespace
`Gf2Algebra.Proofs.Gray` with the following content.

### 6.1 Definitions

```lean
namespace Gf2Algebra.Proofs.Gray

/-- Reflected binary Gray code: `gray k = k XOR (k >>> 1)`. -/
def gray (k : Nat) : Nat := k ^^^ (k >>> 1)

/-- The bit position that flips between `gray k` and `gray (k+1)`. -/
def flipBit (k : Nat) : Nat := Nat.ctz (k + 1)

/-- The subset of `Fin n` with bits set as in `gray k`. -/
def subsetOfBits (n : Nat) (k : Nat) : Finset (Fin n) :=
  (Finset.range n).filterMap
    (fun i => if (gray k).testBit i then some ⟨i, by omega⟩ else none)
    (by intros; aesop)

end Gf2Algebra.Proofs.Gray
```

### 6.2 Key facts

| Fact | Tactic |
|---|---|
| `gray 0 = 0` and `gray (2^n - 1) = 2^(n-1)`. | `decide` at fixed n; `bv_decide` at general n ≤ 63. |
| `gray (k+1) = gray k XOR (1 <<< flipBit k)`. | `bv_decide` (single bit-flip is the defining property). |
| `flipBit k < n` whenever `k + 1 < 2^n` and `n ≤ 63`. | `bv_omega` after expanding `Nat.ctz` via `Nat.ctz_lt_of_pos`. |
| `subsetOfBits` is a bijection `Fin (2^n) ≃ (Finset.univ : Finset (Fin n)).powerset` (or, viewing the powerset as a `Finset`, `Function.Bijective`). | Induction on n: the doubling identity `2^(n+1) = 2·2^n` matches the powerset doubling `(univ : Fin (n+1)).powerset = univ.powerset ∪ (univ.powerset.image (insert ⟨n, _⟩))`. Strict-bijection version follows `Finset.card_powerset` + `Fintype.card_fin`. Probably 30–50 lines. |
| `(subsetOfBits n (k+1)).card = (subsetOfBits n k).card ± 1` (parity flip). | `bv_decide` on the single-bit-flip identity. |
| Telescoping parity: `(subsetOfBits n k).card.bodd = (gray k).popcount.bodd`. | Induction on `k` using the parity-flip fact. |

### 6.3 Why a separate namespace, not Mathlib

Two reasons: (a) we want to keep the V2 PR self-contained so it merges
without a Mathlib pre-PR; (b) the proofs use `bv_decide`/`bv_omega`
heavily and are tightly tied to `Nat.testBit` shape — Mathlib has its
own preferred subset-encoding patterns and upstream review would
require a refactor that is out of scope for D3.

The namespace is a **candidate Mathlib upstream PR after V2 closes**, but
that is recorded as a follow-up in §8, not as a V2 deliverable.

---

## 7. Risks and dependencies

### 7.1 Hard dependencies

| Dep | What | Source | Risk if missing |
|---|---|---|---|
| D1a (closed) | gf2-algebra crate boundary | `dev/plans/d1a_gf2_algebra_boundary.md` | None — already closed. |
| D2 (sketch + V1) | Bipedal $\mathbb{F}_3$ correctness | `dev/plans/d2_lean_bipedal3_sketch.md` (sibling), V1 issue | L4 and L5 cite D2's per-op decode lemmas; if V1 is not closed, V2 cannot start. **Dispatch order: V1 → V2.** |
| W2/T7 + T9 | `permanent_ryser_fp3` exists in `gf2-algebra` | `crates/gf2-algebra/src/permanent/bipedal3.rs` | If the function is generic-only, V2 either adds a monomorphic shim or escalates per CLAUDE.md memory "Hard criteria self-satisfied". |
| `gate:api-freeze` (Gf, W6) | `gf2-algebra` public surface frozen | epic §13 W6 | Charon re-extraction breaks on signature churn (CLAUDE.md memory + epic Risk #7). V2 cannot dispatch before this gate. |
| Mathlib `Matrix.permanent` API stability | Mathlib v4.28.0-rc1 | `proofs/.lake/packages/mathlib/Mathlib/LinearAlgebra/Matrix/Permanent.lean` | Verified present in pinned Mathlib at HEAD of `proofs/lakefile.lean` (line 9). Unpinned bumps elsewhere in the project would need re-verification before V2 dispatches. |

### 7.2 Risks during V2 implementation

- **L7 algebraic difficulty.** Ryser's identity proof from inclusion-exclusion is
  classical but non-trivial in Lean. Estimated 30–80 lines but could
  balloon if the correct combinatorial reorganisation is not chosen
  on attempt 1. Mitigation: V2 should write L7 first, before any
  Aeneas extraction, so that an algebraic dead-end is discovered before
  the generated-Lean glue is even attempted.
- **Aeneas extraction breaks on Bipedal3 layout.** `Bipedal3Vec` is a
  pair of `Vec<u64>` with shared length — Aeneas tends to wrap `Vec` as
  an opaque axiom (cf. `Gf2Core/TypesExternal.lean`). If `fold_mul` and
  `add_in_place` use indexed access via `Vec::get`, those become
  axiomatic in the extraction. Mitigation: V2 implementation must
  ensure the `Bipedal3Vec` API used by `permanent_ryser_fp3` is
  exclusively in terms of pair-of-`u64` (single-word) operations, with
  the multi-word path branching off cleanly. The single-word path
  *already does this* per epic §7.3 (`debug_assert!(mat.n() <= 64)`).
- **`bv_decide` performance at $n = 63$.** Some L1/L2 / Gray-code-flip
  identities are stated for general `n ≤ 63`. `bv_decide` typically scales
  past this regime, but if specific `n`-parametric lemmas time out we may
  need to prove them by induction on `n` instead. Mitigation: V2 has a
  spike day at the start to confirm `bv_decide` lands the headline
  Gray-code bit-flip identity within `maxHeartbeats 1600000`.

### 7.3 No axiomatised steps

D3 commits to **no `axiom` declarations** in V2's hand-written proofs,
beyond those already inherited from `Gf2Core/FunsExternal.lean` and
`TypesExternal.lean`. The Mathlib `Vec` opacity is the only candidate
risk above; the mitigation above keeps it confined to the single-word
path which does not use `Vec` indexing. No `sorry` either.

---

## 8. Out-of-scope

The following are **not** V2 deliverables and are flagged as candidate
follow-up issues:

1. **Unbounded $n$.** Lifting the proof to arbitrary $n$ requires a
   multi-word Gray-code register and a different decidability story;
   this is a separate epic.
2. **$\mathbb{F}_5$ / $\mathbb{F}_7$ analogues.** V3 (epic §12) is
   already `[aspirational]` and gated on R1/R2 closing. The proof
   shape will mirror V2 once those research issues pick a packed
   encoding.
3. **`gray_code` namespace upstreaming.** The Gray-code lemmas in §6
   are project-local; pushing them to Mathlib is a separate PR after
   V2 stabilises.
4. **`Matrix.permanent_eq_ryser` upstreaming.** L7 is a pure-Mathlib
   fact (general `CommRing`) and is the natural Mathlib upstream
   candidate after V2 stabilises.
5. **Gray-code-specific kernel correctness.** D2's V1 already proves
   the bipedal arithmetic correct lane-wise. The Gray-code iterator
   itself, as a Rust `Iterator` impl, has its own `Iterator`-trait
   correctness obligations that V2 *uses* via L1/L3 but does not
   re-prove from the trait law side.
6. **Multi-word streaming permanent (`permanent_bipedal3_multi`).**
   Epic §9; out of scope.
7. **Parallel and GPU permanent paths.** Epic §10–§11; not under
   verification.
8. **`Fp<5>` / `Fp<7>` field correctness in Lean.** V3 territory.

---

## 9. Approval

Per CLAUDE.md §Verification work, this sketch must be reviewed and
approved (lead, and the user where escalation applies) before V2 is
dispatched. The success criterion item 6 on the parent issue
(`4aaa6e4d`) is the user-approval gate; the lead's review is the
implicit sketch-approval pre-step before that.

This document does not modify `.jit/` state; transitioning the parent
issue to `done` is the lead's responsibility after user sign-off.
