/-
  Gf2Core.Proofs.QuotientReduction — obligation O-2 of the algebraic-foundations
  proof sketch (issue 32f53280): quotient reduction preserves the represented
  field element.

  Binding mode: abstract model plus refinement, with an extracted base-carrier
  instance. Section 1 states the model over Mathlib: a field `B`, a monic
  `f : B[X]` of degree `r ≥ 1`, the quotient `Q = AdjoinRoot f`, the coefficient
  space `Fin r → B`, the padding map `pad`, the reduction `red`, and the class
  map `cls = AdjoinRoot.mk f ∘ pad`. Section 2 instantiates the model at the
  extracted `FpVal P` base carrier, whose `Field` instance comes from
  `Gf2Core.Proofs.FpField`.

  That instance carries the hypothesis `P ≠ 2`, inherent to Montgomery
  arithmetic, so the extracted instantiation covers odd prime bases only
  (assumptions-register row A-06). The Section 1 model is stated at an arbitrary
  `[Field B]` and so covers `p = 2` as well.

  Every lemma names its refinement anchor: an executable Rust check that decides
  the same statement on the production path
  `crates/gf2-core/src/gfpn/quotient.rs`. The anchors live in that module's own
  test module and in the shared conformance harness
  `crates/gf2-core/src/field/axiom_tests.rs`. Two of them are new with this
  obligation: `reduction_is_invariant_under_multiples_of_the_modulus` and
  `canonical_index_decodes_to_its_prime_coordinates`.

  Axiom footprint: this module declares no axiom and contains no `sorry`. The
  Section 1 model rests on Lean's `propext`, `Classical.choice`, and `Quot.sound`
  alone. The Section 2 instantiations additionally carry
  `Aeneas.Std.core.fmt.Formatter`, the opaque external type axiom that
  `Gf2Core/TypesExternal.lean` regenerates each extraction run and that the
  `FpField` instances they build on already carry.
-/
import Mathlib.Algebra.Polynomial.Degree.Domain
import Mathlib.Algebra.Polynomial.Div
import Mathlib.FieldTheory.Finite.Basic
import Mathlib.RingTheory.AdjoinRoot
import Gf2Core.Proofs.FpField
import Gf2Core.Proofs.RelativeExtension

set_option maxHeartbeats 1600000

noncomputable section

namespace QuotientReduction

open Polynomial

/-! ## Section 1 — the abstract quotient model

The model fixes a field `B`, a monic modulus `f : B[X]` with
`r = f.natDegree ≥ 1`, and the quotient `Q = AdjoinRoot f`. The stored
coefficient vector of `QuotientElement` (`crates/gf2-core/src/gfpn/quotient.rs:749`)
is modelled by `Fin r → B`, constant coefficient first, exactly as the
production layout stores it. -/

/-! ### Coefficient vectors and their polynomials -/

variable {B : Type*} [Field B]

/-- The polynomial `∑_{i < n} g i · X ^ i`.

This is the shape both padding maps take: `QuotientField::element_from_remainder`
(`crates/gf2-core/src/gfpn/quotient.rs:543`) and `ConstQuotient::from_remainder`
(`:1536`) copy a reduced polynomial into an `r`-slot buffer and zero-fill the
rest. -/
def ofCoeffs (n : ℕ) (g : ℕ → B) : B[X] :=
  ∑ i ∈ Finset.range n, monomial i (g i)

@[simp]
theorem coeff_ofCoeffs (n : ℕ) (g : ℕ → B) (j : ℕ) :
    (ofCoeffs n g).coeff j = if j < n then g j else 0 := by
  classical
  simp only [ofCoeffs, finset_sum_coeff, coeff_monomial]
  rw [Finset.sum_ite_eq' (Finset.range n) j g]
  simp [Finset.mem_range]

theorem degree_ofCoeffs_lt (n : ℕ) (g : ℕ → B) : (ofCoeffs n g).degree < (n : WithBot ℕ) :=
  (degree_lt_iff_coeff_zero _ _).mpr fun m hm => by
    simp [coeff_ofCoeffs, Nat.not_lt.mpr hm]

/-- The degree-`< n` truncation of a polynomial, keeping its coefficients below
`n` and discarding the rest. -/
def trunc (n : ℕ) (p : B[X]) : B[X] := ofCoeffs n p.coeff

theorem trunc_eq_self {n : ℕ} {p : B[X]} (h : p.natDegree < n) : trunc n p = p :=
  (p.as_sum_range' n h).symm

/-- Peeling one Horner step off a truncation: this is the algebraic content of
the accumulator shift in `ConstQuotient::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:1551`). -/
theorem trunc_succ (n : ℕ) (p : B[X]) :
    trunc (n + 1) p = X * trunc n p.divX + C (p.coeff 0) := by
  simp only [trunc, ofCoeffs, Finset.sum_range_succ' (fun i => monomial i (p.coeff i)) n,
    Finset.mul_sum, coeff_divX, X_mul_monomial, monomial_zero_left]

/-- Total extension of a stored coefficient vector to every index, zero above
the stored length. -/
def extend {n : ℕ} (c : Fin n → B) : ℕ → B :=
  fun i => if h : i < n then c ⟨i, h⟩ else 0

/-! ### The model: padding, reduction, and the class map -/

variable (f : B[X])

/-- `pad c` is the polynomial of the stored coefficient vector `c`, constant
coefficient first.

Production path: `QuotientElement::coefficients`
(`crates/gf2-core/src/gfpn/quotient.rs:810`) read back as a polynomial, the
inverse of the copy that `QuotientField::element_from_remainder` (`:543`)
performs. -/
def pad (c : Fin f.natDegree → B) : B[X] := ofCoeffs f.natDegree (extend c)

/-- `red a` is the stored coefficient vector `QuotientField::element`
(`crates/gf2-core/src/gfpn/quotient.rs:426`) returns for the coefficient vector
of `a`: the remainder of `FieldPoly::div_rem`
(`crates/gf2-core/src/field/poly.rs:1779`), copied into `r` slots. -/
def red (a : B[X]) : Fin f.natDegree → B := fun i => (a %ₘ f).coeff i

/-- The class map `κ = π ∘ pad` from stored vectors to the quotient. -/
def cls (c : Fin f.natDegree → B) : AdjoinRoot f := AdjoinRoot.mk f (pad f c)

variable {f}

@[simp]
theorem coeff_pad (c : Fin f.natDegree → B) (j : ℕ) :
    (pad f c).coeff j = if h : j < f.natDegree then c ⟨j, h⟩ else 0 := by
  by_cases h : j < f.natDegree <;> simp [pad, extend, h]

theorem degree_pad_lt (c : Fin f.natDegree → B) :
    (pad f c).degree < (f.natDegree : WithBot ℕ) :=
  degree_ofCoeffs_lt _ _

/-- `pad` restores a polynomial that already has degree below `r`. -/
theorem pad_red_of_degree_lt {a : B[X]} (h : a.degree < (f.natDegree : WithBot ℕ)) :
    ofCoeffs f.natDegree a.coeff = a := by
  refine Polynomial.ext fun j => ?_
  rw [coeff_ofCoeffs]
  split
  · rfl
  · exact ((degree_lt_iff_coeff_zero a f.natDegree).mp h j (Nat.not_lt.mp ‹¬ j < _›)).symm

/-! ### L2.1 — reduction is the residue -/

/-- Reducing before taking the class changes nothing: `π(a %ₘ f) = π(a)`. -/
theorem mk_modByMonic (a : B[X]) : AdjoinRoot.mk f (a %ₘ f) = AdjoinRoot.mk f a :=
  AdjoinRoot.mk_eq_mk.mpr ⟨-(a /ₘ f), by rw [modByMonic_eq_sub_mul_div]; ring⟩

theorem degree_modByMonic_lt_natDegree (hf : f.Monic) (a : B[X]) :
    (a %ₘ f).degree < (f.natDegree : WithBot ℕ) := by
  have h := degree_modByMonic_lt a hf
  rwa [degree_eq_natDegree hf.ne_zero] at h

/-- `pad` is a left inverse of coefficient extraction on the residue: the `r`
stored slots hold exactly the residue's coefficients.

Production path: `QuotientField::element`
(`crates/gf2-core/src/gfpn/quotient.rs:426`) divides by the modulus and hands the
remainder to `element_from_remainder` (`:543`). -/
theorem pad_red (hf : f.Monic) (a : B[X]) : pad f (red f a) = a %ₘ f := by
  refine Polynomial.ext fun j => ?_
  rw [coeff_pad]
  split
  · rfl
  · exact ((degree_lt_iff_coeff_zero _ f.natDegree).mp (degree_modByMonic_lt_natDegree hf a) j
      (Nat.not_lt.mp ‹¬ j < _›)).symm

/-- **L2.1 (reduction is the residue).** The stored vector `ρ(a)` differs from
`a` by a multiple of `f`, and its polynomial has degree below `r`.

Production path: `QuotientField::element`
(`crates/gf2-core/src/gfpn/quotient.rs:426`) and `ConstQuotient::reduce`
(`:1378`), which both call `FieldPoly::div_rem`
(`crates/gf2-core/src/field/poly.rs:1779`) and keep the remainder.

Refinement anchor: `reduction_is_invariant_under_multiples_of_the_modulus`
(`crates/gf2-core/src/gfpn/quotient.rs:2265`), which draws a coefficient vector
`a` and a cofactor `h` and checks that `a` and `a + h · f` reduce to one stored
vector of exactly `relative_degree()` entries. -/
theorem dvd_sub_pad_red (hf : f.Monic) (a : B[X]) : f ∣ a - pad f (red f a) := by
  rw [pad_red hf, modByMonic_eq_sub_mul_div]
  exact ⟨a /ₘ f, by ring⟩

/-- **L2.1 (degree bound).** The padded residue has degree below `r`, so it fits
the `r` stored slots. -/
theorem degree_pad_red_lt (hf : f.Monic) (a : B[X]) :
    (pad f (red f a)).degree < f.degree := by
  rw [pad_red hf]
  exact degree_modByMonic_lt a hf

/-- **L2.1 in one line.** `κ(ρ(a)) = a + (f)`: the stored vector represents the
class of its input.

Refinement anchor: `reduction_is_invariant_under_multiples_of_the_modulus`
(`crates/gf2-core/src/gfpn/quotient.rs:2265`). -/
theorem cls_red (hf : f.Monic) (a : B[X]) : cls f (red f a) = AdjoinRoot.mk f a := by
  rw [cls, pad_red hf, mk_modByMonic]

/-! ### L2.2 — canonical representatives -/

/-- Two polynomials of degree below `r` in one class are equal. This is the
uniqueness half of the canonical-representative claim. -/
theorem eq_of_degree_lt_of_mk_eq {p q : B[X]} (hp : p.degree < f.degree)
    (hq : q.degree < f.degree) (h : AdjoinRoot.mk f p = AdjoinRoot.mk f q) : p = q :=
  sub_eq_zero.mp
    (eq_zero_of_dvd_of_degree_lt (AdjoinRoot.mk_eq_mk.mp h)
      (lt_of_le_of_lt (degree_sub_le p q) (max_lt hp hq)))

theorem degree_pad_lt_degree (hf : f.Monic) (c : Fin f.natDegree → B) :
    (pad f c).degree < f.degree := by
  rw [degree_eq_natDegree hf.ne_zero]
  exact degree_pad_lt c

/-- **L2.2 (reduction fixes stored vectors).** `ρ` is the identity on vectors
already of length `r`, so a stored vector is its own canonical representative.

Production path: `ConstQuotient::new`
(`crates/gf2-core/src/gfpn/quotient.rs:1364`) states this as its contract —
every array of `R` base coefficients is canonical, so no reduction runs.

Refinement anchor: `identity_is_structural_across_instances_and_presentations`
(`crates/gf2-core/src/gfpn/quotient.rs:2236`). -/
theorem red_pad (hf : f.Monic) (c : Fin f.natDegree → B) : red f (pad f c) = c := by
  have hmod : pad f c %ₘ f = pad f c :=
    (modByMonic_eq_self_iff hf).mpr (degree_pad_lt_degree hf c)
  funext i
  simp [red, hmod, coeff_pad, i.isLt]

/-- **L2.2 (injectivity).** Distinct stored vectors name distinct classes. -/
theorem cls_injective (hf : f.Monic) : Function.Injective (cls f) := by
  intro c c' h
  have hpad : pad f c = pad f c' :=
    eq_of_degree_lt_of_mk_eq (degree_pad_lt_degree hf c) (degree_pad_lt_degree hf c') h
  rw [← red_pad hf c, ← red_pad hf c', hpad]

/-- **L2.2 (surjectivity).** Every class has a stored vector. -/
theorem cls_surjective (hf : f.Monic) : Function.Surjective (cls f) := by
  intro q
  obtain ⟨a, rfl⟩ := AdjoinRoot.mk_surjective q
  exact ⟨red f a, cls_red hf a⟩

/-- **L2.2 (canonical representatives).** `κ : B^r → Q` is a bijection: one
class, one stored vector.

Production path: `QuotientElement` (`crates/gf2-core/src/gfpn/quotient.rs:749`)
stores exactly the `r` coefficients and nothing else.

Refinement anchor: `identity_is_structural_across_instances_and_presentations`
(`crates/gf2-core/src/gfpn/quotient.rs:2236`), which pins that one algebraic
field has one stored presentation across descriptor instances. -/
theorem cls_bijective (hf : f.Monic) : Function.Bijective (cls f) :=
  ⟨cls_injective hf, cls_surjective hf⟩

/-- The bijection `B^r ≃ Q` of L2.2, with `ρ` as its inverse. -/
def clsEquiv (hf : f.Monic) : (Fin f.natDegree → B) ≃ AdjoinRoot f where
  toFun := cls f
  invFun q := red f (AdjoinRoot.mk_surjective q).choose
  left_inv c := cls_injective hf (by
    rw [cls_red hf, (AdjoinRoot.mk_surjective (cls f c)).choose_spec])
  right_inv q := by
    rw [cls_red hf, (AdjoinRoot.mk_surjective q).choose_spec]

/-- **L2.2 (derived equality decides the quotient).** Two polynomials reduce to
one stored vector exactly when they name one class, so `QuotientElement`'s
derived `PartialEq` on coefficients
(`crates/gf2-core/src/gfpn/quotient.rs:774`) decides equality in `Q`.

Refinement anchor: `identity_is_structural_across_instances_and_presentations`
(`crates/gf2-core/src/gfpn/quotient.rs:2236`). -/
theorem red_eq_iff_mk_eq (hf : f.Monic) (a b : B[X]) :
    red f a = red f b ↔ AdjoinRoot.mk f a = AdjoinRoot.mk f b := by
  constructor
  · intro h
    rw [← cls_red hf a, ← cls_red hf b, h]
  · intro h
    exact cls_injective hf (by rw [cls_red hf, cls_red hf, h])


/-! ### L2.3 — reduction is a ring homomorphism -/

/-- Stored-vector addition: `QuotientElement`'s `Add`
(`crates/gf2-core/src/gfpn/quotient.rs:982`) adds coefficient-wise without
reducing. -/
def vadd (f : B[X]) (x y : Fin f.natDegree → B) : Fin f.natDegree → B := fun i => x i + y i

/-- Stored-vector multiplication: `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:896`) convolves and reduces. -/
def vmul (f : B[X]) (x y : Fin f.natDegree → B) : Fin f.natDegree → B :=
  red f (pad f x * pad f y)

theorem pad_vadd (x y : Fin f.natDegree → B) : pad f (vadd f x y) = pad f x + pad f y := by
  refine Polynomial.ext fun j => ?_
  by_cases h : j < f.natDegree <;> simp [coeff_pad, vadd, h]

theorem degree_lt_of_natDegree_lt {p : B[X]} {n : ℕ} (h : p.natDegree < n) :
    p.degree < (n : WithBot ℕ) := by
  rcases eq_or_ne p 0 with rfl | hp
  · exact lt_of_le_of_lt (le_of_eq degree_zero) (WithBot.bot_lt_coe n)
  · exact (natDegree_lt_iff_degree_lt hp).mp h

theorem natDegree_lt_of_degree_lt {p : B[X]} {n : ℕ} (hn : 0 < n)
    (h : p.degree < (n : WithBot ℕ)) : p.natDegree < n := by
  rcases eq_or_ne p 0 with rfl | hp
  · simpa using hn
  · exact (natDegree_lt_iff_degree_lt hp).mpr h

theorem natDegree_pad_lt (hr : 0 < f.natDegree) (c : Fin f.natDegree → B) :
    (pad f c).natDegree < f.natDegree :=
  natDegree_lt_of_degree_lt hr (degree_pad_lt c)

/-- Two polynomials in one class have one residue. -/
theorem modByMonic_eq_of_mk_eq (hf : f.Monic) {p q : B[X]}
    (h : AdjoinRoot.mk f p = AdjoinRoot.mk f q) : p %ₘ f = q %ₘ f :=
  eq_of_degree_lt_of_mk_eq (degree_modByMonic_lt p hf) (degree_modByMonic_lt q hf)
    (by rw [mk_modByMonic, mk_modByMonic, h])

/-- **L2.3 (additive).** `ρ(a + b) = ρ(a) ⊕ ρ(b)`.

Production path: `QuotientElement`'s `Add`
(`crates/gf2-core/src/gfpn/quotient.rs:982`).

Refinement anchors: the addition case of `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2351`) and the field-law registrations
`test_quotient_gf125_field_axioms`
(`crates/gf2-core/src/field/axiom_tests.rs:1793`) and
`test_quotient_gf81_over_gf9_field_axioms` (`:1799`). -/
theorem red_add (a b : B[X]) : red f (a + b) = vadd f (red f a) (red f b) := by
  funext i
  simp [red, vadd, add_modByMonic]

/-- **L2.3 (multiplicative).** `ρ(a · b) = ρ(a) ⊗ ρ(b)`: reducing the operands
first is sound.

Production path: `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:896`), which convolves the two stored
vectors and folds the high terms.

Refinement anchors: the multiplication case of `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2353`) and the field-law registrations
`test_quotient_gf125_field_axioms`
(`crates/gf2-core/src/field/axiom_tests.rs:1793`) and
`test_quotient_gf81_over_gf9_field_axioms` (`:1799`). -/
theorem red_mul (hf : f.Monic) (a b : B[X]) : red f (a * b) = vmul f (red f a) (red f b) := by
  rw [vmul, pad_red hf, pad_red hf]
  refine (red_eq_iff_mk_eq hf _ _).mpr ?_
  rw [map_mul, map_mul, mk_modByMonic, mk_modByMonic]

/-- **L2.3 (the class map is additive).** -/
theorem cls_vadd (x y : Fin f.natDegree → B) :
    cls f (vadd f x y) = cls f x + cls f y := by
  rw [cls, pad_vadd, map_add, cls, cls]

/-- **L2.3 (the class map is multiplicative).** -/
theorem cls_vmul (hf : f.Monic) (x y : Fin f.natDegree → B) :
    cls f (vmul f x y) = cls f x * cls f y := by
  rw [vmul, cls_red hf, map_mul, cls, cls]

/-! ### L2.3 — the production-shaped high-to-low fold -/

/-- One iteration of the fold in `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:907-917`): the coefficient at high index
`r + d` is cleared by subtracting `c · X^d · f`, where `d = high - degree` is the
production offset (`:912`). -/
def foldStep (f : B[X]) (p : B[X]) (d : ℕ) : B[X] :=
  p - C (p.coeff (f.natDegree + d)) * (X ^ d * f)

/-- The fold loop of `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:907`), which visits the offsets
`n - 1, n - 2, …, 0` in that decreasing order. -/
def foldDown (f : B[X]) : ℕ → B[X] → B[X]
  | 0, p => p
  | (n + 1), p => foldDown f n (foldStep f p n)

theorem foldDown_zero (p : B[X]) : foldDown f 0 p = p := rfl

theorem foldDown_succ (n : ℕ) (p : B[X]) :
    foldDown f (n + 1) p = foldDown f n (foldStep f p n) := rfl

/-- Each iteration subtracts a multiple of `f`, so the class is a loop
invariant. -/
theorem foldStep_mk (p : B[X]) (d : ℕ) :
    AdjoinRoot.mk f (foldStep f p d) = AdjoinRoot.mk f p := by
  rw [foldStep, map_sub, sub_eq_self]
  exact AdjoinRoot.mk_eq_zero.mpr ⟨C (p.coeff (f.natDegree + d)) * X ^ d, by ring⟩

/-- Each iteration drops the working degree by at least one, the second half of
the loop invariant. -/
theorem degree_foldStep_lt (hf : f.Monic) {p : B[X]} {d : ℕ}
    (hp : p.degree < ((f.natDegree + d + 1 : ℕ) : WithBot ℕ)) :
    (foldStep f p d).degree < ((f.natDegree + d : ℕ) : WithBot ℕ) := by
  have hxf : (X ^ d * f).Monic := (monic_X_pow (R := B) d).mul hf
  have hnd : (X ^ d * f).natDegree = f.natDegree + d := by
    rw [natDegree_mul (pow_ne_zero d X_ne_zero) hf.ne_zero, natDegree_X_pow]
    exact Nat.add_comm d f.natDegree
  have htop : (X ^ d * f).coeff (f.natDegree + d) = 1 := by
    have := hxf.coeff_natDegree
    rwa [hnd] at this
  have hpz : ∀ k, f.natDegree + d + 1 ≤ k → p.coeff k = 0 :=
    (degree_lt_iff_coeff_zero p _).mp hp
  refine (degree_lt_iff_coeff_zero _ _).mpr fun m hm => ?_
  simp only [foldStep, coeff_sub, coeff_C_mul]
  rcases Nat.eq_or_lt_of_le hm with h | h
  · rw [← h, htop, mul_one, sub_self]
  · have hzero : (X ^ d * f).coeff m = 0 :=
      coeff_eq_zero_of_natDegree_lt (by rw [hnd]; exact h)
    rw [hpz m h, hzero, mul_zero, sub_zero]

/-- **L2.3 (the fold computes the residue).** Running the production fold over
the `n` high offsets of a working polynomial of degree below `r + n` yields the
residue `p %ₘ f`. The loop invariant is exactly the pair of lemmas above: the
working polynomial stays in the class of the input, and its degree falls below
the current high index.

Production path: the fold of `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:907-917`).

Refinement anchors: the multiplication case of `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2353`) and the field-law registrations
`test_quotient_gf125_field_axioms`
(`crates/gf2-core/src/field/axiom_tests.rs:1793`) and
`test_quotient_gf81_over_gf9_field_axioms` (`:1799`). -/
theorem foldDown_eq_modByMonic (hf : f.Monic) :
    ∀ (n : ℕ) (p : B[X]), p.degree < ((f.natDegree + n : ℕ) : WithBot ℕ) →
      foldDown f n p = p %ₘ f := by
  intro n
  induction n with
  | zero =>
      intro p hp
      rw [foldDown_zero]
      refine ((modByMonic_eq_self_iff hf).mpr ?_).symm
      rw [degree_eq_natDegree hf.ne_zero]
      simpa using hp
  | succ m ih =>
      intro p hp
      rw [foldDown_succ]
      have hstep : (foldStep f p m).degree < ((f.natDegree + m : ℕ) : WithBot ℕ) :=
        degree_foldStep_lt hf hp
      rw [ih _ hstep]
      exact modByMonic_eq_of_mk_eq hf (foldStep_mk p m)

/-- **L2.3 (the production multiply).** Convolving two stored vectors into the
`2r - 1` buffer and running the fold over its `r - 1` high offsets produces the
padded stored product `⊗`. This is the loop `QuotientElement::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:896`) runs, offsets and all.

Refinement anchors: the multiplication case of `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2353`) and the field-law registrations
`test_quotient_gf125_field_axioms`
(`crates/gf2-core/src/field/axiom_tests.rs:1793`) and
`test_quotient_gf81_over_gf9_field_axioms` (`:1799`). -/
theorem foldDown_pad_mul (hf : f.Monic) (hr : 0 < f.natDegree)
    (x y : Fin f.natDegree → B) :
    foldDown f (f.natDegree - 1) (pad f x * pad f y) = pad f (vmul f x y) := by
  have hdeg : (pad f x * pad f y).degree <
      ((f.natDegree + (f.natDegree - 1) : ℕ) : WithBot ℕ) := by
    refine degree_lt_of_natDegree_lt (lt_of_le_of_lt (natDegree_mul_le) ?_)
    have hx := natDegree_pad_lt hr x
    have hy := natDegree_pad_lt hr y
    omega
  rw [foldDown_eq_modByMonic hf _ _ hdeg, vmul, pad_red hf]


/-! ### L2.4 — the compile-time Horner form -/

/-- One step of the Horner loop in `ConstQuotient::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:1551`): multiply the accumulator by `x` —
a shift plus the rewrite `x^R = -∑_{i<R} f_i x^i`, which is division by the monic
`f` — and add a scalar multiple of the right operand. -/
def hornerStep (f b acc : B[X]) (c : B) : B[X] := (acc * X) %ₘ f + C c * b

/-- The Horner loop of `ConstQuotient::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:1551`), which walks the left operand's
coefficient indices downward. `hornerLoop f b n a` runs the `n` steps for
indices `n - 1, …, 0`, so the accumulator never leaves its `R`-wide array and no
`2R - 1` convolution buffer is formed. -/
def hornerLoop (f b : B[X]) : ℕ → B[X] → B[X]
  | 0, _ => 0
  | (n + 1), a => hornerStep f b (hornerLoop f b n a.divX) (a.coeff 0)

theorem hornerLoop_zero (b a : B[X]) : hornerLoop f b 0 a = 0 := rfl

theorem hornerLoop_succ (b : B[X]) (n : ℕ) (a : B[X]) :
    hornerLoop f b (n + 1) a = hornerStep f b (hornerLoop f b n a.divX) (a.coeff 0) := rfl

theorem trunc_zero (p : B[X]) : trunc 0 p = 0 := by
  simp [trunc, ofCoeffs]

/-- The Horner invariant: after `n` steps the accumulator represents the class of
`trunc n a · b`. Each step shifts by `x` and folds in one more coefficient. -/
theorem hornerLoop_mk (b : B[X]) :
    ∀ (n : ℕ) (a : B[X]), AdjoinRoot.mk f (hornerLoop f b n a) =
      AdjoinRoot.mk f (trunc n a) * AdjoinRoot.mk f b := by
  intro n
  induction n with
  | zero => intro a; rw [hornerLoop_zero, trunc_zero]; simp
  | succ m ih =>
      intro a
      rw [hornerLoop_succ, hornerStep, map_add, mk_modByMonic, map_mul, ih, map_mul,
        trunc_succ, map_add, map_mul]
      ring

theorem degree_hornerStep_lt (hf : f.Monic) {b : B[X]} (hb : b.degree < f.degree)
    (acc : B[X]) (c : B) : (hornerStep f b acc c).degree < f.degree := by
  refine lt_of_le_of_lt (degree_add_le _ _) (max_lt (degree_modByMonic_lt _ hf) ?_)
  rw [← smul_eq_C_mul]
  exact lt_of_le_of_lt (degree_smul_le c b) hb

theorem degree_hornerLoop_lt (hf : f.Monic) {b : B[X]} (hb : b.degree < f.degree) :
    ∀ (n : ℕ) (a : B[X]), (hornerLoop f b n a).degree < f.degree := by
  intro n a
  cases n with
  | zero =>
      rw [hornerLoop_zero, degree_zero, degree_eq_natDegree hf.ne_zero]
      exact WithBot.bot_lt_coe _
  | succ m => exact degree_hornerStep_lt hf hb _ _

/-- **L2.4 (the Horner form computes the residue).** The accumulator after `r`
steps is exactly the residue of the product, so `ConstQuotient::multiply`
(`crates/gf2-core/src/gfpn/quotient.rs:1551`) and `QuotientElement::multiply`
(`:896`) agree on every pair of operands.

Refinement anchor: `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2290`), driven by
`const_and_runtime_forms_agree_on_gf16` (`:2370`), `..._on_gf125` (`:2375`) and
`..._on_gf81_over_gf9` (`:2380`). -/
theorem hornerLoop_eq_modByMonic (hf : f.Monic) {a b : B[X]}
    (ha : a.natDegree < f.natDegree) (hb : b.degree < f.degree) :
    hornerLoop f b f.natDegree a = (a * b) %ₘ f := by
  refine eq_of_degree_lt_of_mk_eq (degree_hornerLoop_lt hf hb _ _)
    (degree_modByMonic_lt _ hf) ?_
  rw [hornerLoop_mk, trunc_eq_self ha, mk_modByMonic, map_mul]

/-- **L2.4 (the two carriers agree).** For one declaration, the Horner loop of
`ConstQuotient::multiply` (`crates/gf2-core/src/gfpn/quotient.rs:1551`) and the
convolve-and-fold loop of `QuotientElement::multiply` (`:896`) produce one
polynomial, hence one stored vector.

Refinement anchor: `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2290`), driven by
`const_and_runtime_forms_agree_on_gf16` (`:2370`), `..._on_gf125` (`:2375`) and
`..._on_gf81_over_gf9` (`:2380`). -/
theorem hornerLoop_eq_foldDown (hf : f.Monic) (hr : 0 < f.natDegree)
    (x y : Fin f.natDegree → B) :
    hornerLoop f (pad f y) f.natDegree (pad f x) =
      foldDown f (f.natDegree - 1) (pad f x * pad f y) := by
  rw [foldDown_pad_mul hf hr, hornerLoop_eq_modByMonic hf (natDegree_pad_lt hr x)
    (degree_pad_lt_degree hf y), vmul, pad_red hf]

/-- The modulus a compile-time declaration materializes:
`ConstQuotientConfig::MODULUS` (`crates/gf2-core/src/gfpn/quotient.rs:1294`)
stores the `R` low coefficients and `ConstQuotient::modulus` (`:1398`) restores
the implicit leading one. -/
def constModulus (R : ℕ) (m : Fin R → B) : B[X] := X ^ R + ofCoeffs R (extend m)

/-- **L2.4 (the declared modulus is monic).** The implicit leading one makes
every declaration monic, so every lemma of this section applies to the
compile-time carrier.

Production path: `ConstQuotientConfig::MODULUS`
(`crates/gf2-core/src/gfpn/quotient.rs:1294`), whose documentation states the
monicity this lemma proves.

Refinement anchor: `const_quotient_modulus_and_indeterminate_match_the_declaration`
(`crates/gf2-core/src/gfpn/quotient.rs:2441`). -/
theorem constModulus_monic (R : ℕ) (m : Fin R → B) : (constModulus R m).Monic :=
  monic_X_pow_add (degree_ofCoeffs_lt R (extend m))

/-- **L2.4 (the declared width is the relative degree).** A declaration of `R`
low coefficients has relative degree `R`, so the compile-time `R`-wide array and
the runtime `relative_degree()` slots are one width and
`ConstQuotient::runtime_field` (`crates/gf2-core/src/gfpn/quotient.rs:1530`)
bridges without reshaping.

Refinement anchor: `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2290`), whose `relative_degree` assertion
sits at `:2308`. -/
theorem natDegree_constModulus (R : ℕ) (m : Fin R → B) :
    (constModulus R m).natDegree = R := by
  have hdeg : (constModulus R m).degree = (R : WithBot ℕ) := by
    rw [constModulus, degree_add_eq_left_of_degree_lt, degree_X_pow]
    rw [degree_X_pow]
    exact degree_ofCoeffs_lt R (extend m)
  exact natDegree_eq_of_degree_eq_some hdeg


/-! ### L2.5 — inversion through Bezout -/

/-- Bezout for an irreducible modulus: the extended-Euclid run of
`euclid_inverse` (`crates/gf2-core/src/gfpn/quotient.rs:950`) computes exactly
this pair of coefficients, normalised so that the gcd is one. -/
theorem exists_bezout (hirr : Irreducible f) {v : B[X]} (hv : ¬ f ∣ v) :
    ∃ a b : B[X], a * f + b * v = 1 := by
  obtain ⟨a, b, hab⟩ := (hirr.coprime_iff_not_dvd).mpr hv
  exact ⟨a, b, hab⟩

/-- **L2.5 (inversion).** For an irreducible modulus and a representative `v`
outside `(f)`, there is exactly one residue `u` of degree below `r` with
`u · v ≡ 1 (mod f)`. The Bezout coefficient of `v`, reduced modulo `f`, is that
residue.

Production path: `euclid_inverse`
(`crates/gf2-core/src/gfpn/quotient.rs:950`), the single extended-Euclid
implementation both carriers use; `QuotientElement::inverse_euclid` (`:925`) and
`ConstQuotient::inverse` (`:1577`) each store its reduced output in their own
layout.

Refinement anchors: `sampled_nonzero_elements_have_multiplicative_inverses`
(`crates/gf2-core/src/gfpn/quotient.rs:2267`) and the `inv` case of
`assert_forms_agree` (`:2354`). -/
theorem existsUnique_inverse_residue (hf : f.Monic) (hirr : Irreducible f) {v : B[X]}
    (hv : ¬ f ∣ v) :
    ∃! u : B[X], u.degree < f.degree ∧ AdjoinRoot.mk f (u * v) = 1 := by
  obtain ⟨a, b, hab⟩ := exists_bezout hirr hv
  have hb1 : AdjoinRoot.mk f (b %ₘ f * v) = 1 := by
    rw [map_mul, mk_modByMonic, ← map_mul]
    have h : AdjoinRoot.mk f (a * f + b * v) = 1 := by rw [hab, map_one]
    rwa [map_add, map_mul, AdjoinRoot.mk_self, mul_zero, zero_add] at h
  refine ⟨b %ₘ f, ⟨degree_modByMonic_lt _ hf, hb1⟩, ?_⟩
  rintro u ⟨hdeg, hu⟩
  have hdvd : f ∣ (u - b %ₘ f) * v := by
    refine AdjoinRoot.mk_eq_zero.mp ?_
    have hsplit : (u - b %ₘ f) * v = u * v - b %ₘ f * v := by ring
    rw [hsplit, map_sub, hu, hb1, sub_self]
  rcases hirr.prime.dvd_mul.mp hdvd with h | h
  · exact sub_eq_zero.mp
      (eq_zero_of_dvd_of_degree_lt h
        (lt_of_le_of_lt (degree_sub_le _ _) (max_lt hdeg (degree_modByMonic_lt _ hf))))
  · exact absurd h hv

/-! ### L2.6 — field structure and its converse -/

/-- **L2.6 (field structure).** An irreducible modulus makes `Q` a field.

Production path: `QuotientField::new`
(`crates/gf2-core/src/gfpn/quotient.rs:251`) and `ConstQuotient::extension`
(`:1464`), which decide irreducibility before issuing a witness.

Refinement anchors: the axiom-harness registrations that build every in-tree
compile-time declaration through `ConstQuotient::extension`
(`crates/gf2-core/src/field/axiom_tests.rs:1817`, `:1826`, `:1835`) and run the
shared field-law suite against it. -/
@[reducible] def adjoinRootField (hirr : Irreducible f) : Field (AdjoinRoot f) :=
  haveI : Fact (Irreducible f) := ⟨hirr⟩
  AdjoinRoot.instField

/-- **L2.6 (order).** `|Q| = |B|^r`, from the bijection of L2.2.

Production path: `QuotientField::order`
(`crates/gf2-core/src/gfpn/quotient.rs:491`), which reads `|B|^r` off the
identity metadata.

Refinement anchor: `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2290`), whose identity assertions pin the
same `FieldId` on both carriers. -/
theorem card_adjoinRoot [Finite B] (hf : f.Monic) :
    Nat.card (AdjoinRoot f) = Nat.card B ^ f.natDegree := by
  rw [← Nat.card_congr (clsEquiv hf), Nat.card_fun]
  simp

/-- **L2.6 (converse).** A reducible modulus leaves a nonzero non-unit in `Q`,
so the quotient is not a field. This is what the GIGO contract of
`ConstQuotient::extension_unchecked`
(`crates/gf2-core/src/gfpn/quotient.rs:1511`) and
`QuotientField::from_certificate_unchecked` (`:357`) means precisely: a caller
that declares a reducible modulus gets a ring in which some nonzero element has
no inverse.

Refinement anchors: `validating_construction_rejects_reducible_modulus`
(`crates/gf2-core/src/gfpn/quotient.rs:2051`) and
`const_validation_rejects_a_reducible_declaration` (`:2387`). -/
theorem exists_ne_zero_not_isUnit_of_not_irreducible (hf : f.Monic) (hr : 0 < f.natDegree)
    (hirr : ¬ Irreducible f) : ∃ z : AdjoinRoot f, z ≠ 0 ∧ ¬ IsUnit z := by
  have hfu : ¬ IsUnit f := fun h => absurd (natDegree_eq_zero_of_isUnit h) (by omega)
  rw [irreducible_iff, not_and_or] at hirr
  rcases hirr with h | h
  · exact absurd hfu h
  simp only [not_forall, not_or] at h
  obtain ⟨a, b, hfab, hau, hbu⟩ := h
  have hane : a ≠ 0 := by rintro rfl; exact hf.ne_zero (by simpa using hfab)
  have hbne : b ≠ 0 := by rintro rfl; exact hf.ne_zero (by simpa using hfab)
  have hbdeg : 0 < b.natDegree := by
    by_contra hcon
    exact hbu (isUnit_iff_degree_eq_zero.mpr (by
      rw [degree_eq_natDegree hbne]
      exact_mod_cast Nat.le_zero.mp (Nat.not_lt.mp hcon)))
  have hnd : f.natDegree = a.natDegree + b.natDegree := by
    rw [hfab, natDegree_mul hane hbne]
  have hadeg : a.degree < f.degree := by
    rw [degree_eq_natDegree hf.ne_zero]
    exact degree_lt_of_natDegree_lt (by omega)
  refine ⟨AdjoinRoot.mk f a, ?_, ?_⟩
  · intro hz
    exact hane (eq_zero_of_dvd_of_degree_lt (AdjoinRoot.mk_eq_zero.mp hz) hadeg)
  · rintro ⟨u, hu⟩
    obtain ⟨w, hw⟩ := AdjoinRoot.mk_surjective (↑u⁻¹ : AdjoinRoot f)
    have hmk : AdjoinRoot.mk f (a * w) = AdjoinRoot.mk f 1 := by
      rw [map_mul, hw, ← hu, map_one]
      exact u.mul_inv
    have hdvd : f ∣ a * w - 1 := AdjoinRoot.mk_eq_mk.mp hmk
    have hone : a ∣ 1 := by
      have h2 : a ∣ a * w - 1 := (Dvd.intro b hfab.symm).trans hdvd
      simpa using dvd_sub (Dvd.intro w rfl) h2
    exact hau (isUnit_of_dvd_one hone)

theorem irreducible_iff_forall_isUnit (hf : f.Monic) (hr : 0 < f.natDegree) :
    Irreducible f ↔ ∀ z : AdjoinRoot f, z ≠ 0 → IsUnit z := by
  constructor
  · intro hirr z hz
    letI := adjoinRootField hirr
    exact isUnit_iff_ne_zero.mpr hz
  · intro hall
    by_contra hirr
    obtain ⟨z, hz, hzu⟩ := exists_ne_zero_not_isUnit_of_not_irreducible hf hr hirr
    exact hzu (hall z hz)


/-! ### L2.7 — canonical coordinates

The canonical index and the canonical prime coordinates are pure arithmetic on
ℕ, so this subsection is stated over ℕ and applies to both quotient carriers.
`p` is the characteristic, `dB` the absolute base degree, and `r` the relative
degree, so the absolute degree is `dB · r` exactly as `checked_degrees`
(`crates/gf2-core/src/gfpn/quotient.rs:633`) computes it. -/

/-- The repeated-division loop of `QuotientField::element_at_canonical_index`
(`crates/gf2-core/src/gfpn/quotient.rs:554`): `d` iterations, each pushing
`index % p` and then dividing `index` by `p`. -/
def divLoop (p : ℕ) : ℕ → ℕ → List ℕ
  | 0, _ => []
  | (d + 1), m => (m % p) :: divLoop p d (m / p)

/-- The `k`-th canonical prime coordinate of an index. -/
def digit (p k m : ℕ) : ℕ := m / p ^ k % p

/-- The canonical index of a coordinate vector, `∑_k c_k · p^k`. This is the
value `canonical_index_of_a_gf2m_element_is_its_stored_value`
(`crates/gf2-core/src/field/extension.rs:3529`) folds for the GF(2^m) carrier. -/
def coordValue (p : ℕ) {d : ℕ} (c : Fin d → ℕ) : ℕ := ∑ k : Fin d, c k * p ^ (k : ℕ)

theorem digit_zero (p m : ℕ) : digit p 0 m = m % p := by simp [digit]

theorem digit_succ (p k m : ℕ) : digit p (k + 1) m = digit p k (m / p) := by
  rw [digit, digit, pow_succ', Nat.div_div_eq_div_mul]

/-- **L2.7 (the division loop is base-`p` decoding).** The loop of
`element_at_canonical_index` (`crates/gf2-core/src/gfpn/quotient.rs:554`) writes
exactly the base-`p` digits, in the order it divides.

Refinement anchor: `canonical_index_decodes_to_its_prime_coordinates`
(`crates/gf2-core/src/gfpn/quotient.rs:2500`). -/
theorem divLoop_eq_ofFn (p : ℕ) :
    ∀ (d m : ℕ), divLoop p d m = List.ofFn (fun k : Fin d => digit p (k : ℕ) m) := by
  intro d
  induction d with
  | zero => intro m; rfl
  | succ n ih =>
      intro m
      rw [divLoop, ih, List.ofFn_succ]
      simp only [Fin.val_zero, Fin.val_succ, digit_zero]
      exact congrArg _ (by simp only [digit_succ])

/-- **L2.7 (decoding inverts encoding).** An index below `p^d` is recovered from
its `d` canonical coordinates.

Refinement anchor: `canonical_index_decodes_to_its_prime_coordinates`
(`crates/gf2-core/src/gfpn/quotient.rs:2500`). -/
theorem coordValue_digit (p : ℕ) (hp : 0 < p) :
    ∀ (d m : ℕ), m < p ^ d → coordValue p (fun k : Fin d => digit p (k : ℕ) m) = m := by
  intro d
  induction d with
  | zero =>
      intro m hm
      simp only [pow_zero, Nat.lt_one_iff] at hm
      simp [coordValue, hm]
  | succ n ih =>
      intro m hm
      have hdiv : m / p < p ^ n := by
        rw [Nat.div_lt_iff_lt_mul hp]
        rwa [pow_succ] at hm
      have hstep : ∀ k : Fin n,
          digit p ((k.succ : Fin (n + 1)) : ℕ) m * p ^ ((k.succ : Fin (n + 1)) : ℕ) =
            p * (digit p (k : ℕ) (m / p) * p ^ (k : ℕ)) := by
        intro k
        rw [Fin.val_succ, digit_succ, pow_succ']
        ring
      rw [coordValue, Fin.sum_univ_succ]
      simp only [Fin.val_zero, pow_zero, mul_one, digit_zero]
      rw [Finset.sum_congr rfl fun k _ => hstep k, ← Finset.mul_sum,
        show (∑ k : Fin n, digit p (k : ℕ) (m / p) * p ^ (k : ℕ)) =
          coordValue p (fun k : Fin n => digit p (k : ℕ) (m / p)) from rfl,
        ih _ hdiv, Nat.mod_add_div]

/-- **L2.7 (encoding inverts decoding).** A coordinate vector with every entry
below `p` is recovered from its canonical index.

Refinement anchor: `canonical_index_decodes_to_its_prime_coordinates`
(`crates/gf2-core/src/gfpn/quotient.rs:2500`). -/
theorem digit_coordValue (p : ℕ) (hp : 0 < p) :
    ∀ (d : ℕ) (c : Fin d → ℕ), (∀ k, c k < p) →
      ∀ k : Fin d, digit p (k : ℕ) (coordValue p c) = c k := by
  intro d
  induction d with
  | zero => intro c _ k; exact k.elim0
  | succ n ih =>
      intro c hc k
      set t : Fin n → ℕ := fun k => c k.succ with ht
      have hsplit : coordValue p c = c 0 + p * coordValue p t := by
        rw [coordValue, Fin.sum_univ_succ, coordValue, Finset.mul_sum]
        simp only [Fin.val_zero, pow_zero, mul_one, Fin.val_succ, ht]
        exact congrArg _ (Finset.sum_congr rfl fun j _ => by rw [pow_succ']; ring)
      have hdiv : (c 0 + p * coordValue p t) / p = coordValue p t := by
        rw [Nat.add_mul_div_left _ _ hp, Nat.div_eq_of_lt (hc 0), Nat.zero_add]
      refine Fin.cases ?_ ?_ k
      · rw [Fin.val_zero, digit_zero, hsplit, Nat.add_mul_mod_self_left,
          Nat.mod_eq_of_lt (hc 0)]
      · intro j
        rw [Fin.val_succ, digit_succ, hsplit, hdiv]
        exact ih t (fun j => hc j.succ) j

/-- The flattening of `QuotientElement::write_prime_coords`
(`crates/gf2-core/src/gfpn/quotient.rs:1205`): the `r · dB` absolute coordinates
are the `r` base blocks in stored order. -/
def flatten {r dB : ℕ} (C : Fin r → Fin dB → ℕ) : Fin (r * dB) → ℕ :=
  fun k => C (finProdFinEquiv.symm k).1 (finProdFinEquiv.symm k).2

/-- **L2.7 (flattening).** Absolute coordinate index `i · dB + j` carries the
`j`-th base coordinate of the `i`-th stored coefficient, so the base coordinate
varies fastest.

Production path: `QuotientElement::write_prime_coords`
(`crates/gf2-core/src/gfpn/quotient.rs:1205`), which concatenates the base
blocks, and `QuotientField::element_from_prime_coords` (`:568`), which splits
them back with `chunks_exact`.

Refinement anchors: the coordinate-agreement case of `assert_forms_agree`
(`crates/gf2-core/src/gfpn/quotient.rs:2336-2341`),
`const_quotient_coordinates_round_trip_through_the_runtime_carrier` (`:2474`)
and `tower_coordinates_vary_the_base_coordinate_fastest`
(`crates/gf2-core/src/field/extension.rs:3543`) for the `dB > 1` ordering. -/
theorem flatten_apply {r dB : ℕ} (C : Fin r → Fin dB → ℕ) (i : Fin r) (j : Fin dB) :
    flatten C (finProdFinEquiv (i, j)) = C i j := by
  simp [flatten]

theorem flatten_index {r dB : ℕ} (i : Fin r) (j : Fin dB) :
    ((finProdFinEquiv (i, j) : Fin (r * dB)) : ℕ) = (j : ℕ) + dB * (i : ℕ) :=
  finProdFinEquiv_apply_val _

/-- **L2.7 (canonical index of a quotient element).** The absolute index of a
stored vector is `∑_i idx_B(c_i) · q^i` for `q = p^{dB}`, the base-`q`
value of its coefficient indices. Together with the two round-trip lemmas this
makes `write_prime_coords` (`crates/gf2-core/src/gfpn/quotient.rs:1205`) and
`element_at_canonical_index` (`:554`) mutually inverse.

Refinement anchors: `const_quotient_coordinates_round_trip_through_the_runtime_carrier`
(`crates/gf2-core/src/gfpn/quotient.rs:2474`),
`tower_coordinates_vary_the_base_coordinate_fastest`
(`crates/gf2-core/src/field/extension.rs:3543`) and
`canonical_index_decodes_to_its_prime_coordinates`
(`crates/gf2-core/src/gfpn/quotient.rs:2500`). -/
theorem coordValue_flatten (p : ℕ) {r dB : ℕ} (C : Fin r → Fin dB → ℕ) :
    coordValue p (flatten C) = ∑ i : Fin r, coordValue p (C i) * (p ^ dB) ^ (i : ℕ) := by
  rw [coordValue, ← Equiv.sum_comp (finProdFinEquiv (m := r) (n := dB))
    (fun k => flatten C k * p ^ (k : ℕ)), Fintype.sum_prod_type]
  refine Finset.sum_congr rfl fun i _ => ?_
  rw [coordValue, Finset.sum_mul]
  refine Finset.sum_congr rfl fun j _ => ?_
  rw [flatten_apply, flatten_index, pow_add, pow_mul]
  ring

/-! ### L2.8 — Frobenius on the quotient -/

/-- **L2.8 (absolute period).** In a finite field of order `p^d` the `d`-fold
absolute Frobenius is the identity, so `QuotientElement::frobenius`
(`crates/gf2-core/src/gfpn/quotient.rs:865`) may reduce `k` modulo the absolute
degree before raising to the characteristic that many times.

Refinement anchor: `frobenius_has_absolute_and_relative_orders`
(`crates/gf2-core/src/gfpn/quotient.rs:2147`). -/
theorem absFrobenius_iterate_mod {E : Type*} [Field E] [Fintype E] {p d : ℕ}
    (hcard : Fintype.card E = p ^ d) (k : ℕ) (x : E) :
    (fun y : E => y ^ p)^[k] x = (fun y : E => y ^ p)^[k % d] x := by
  have hone : ∀ y : E, (fun y : E => y ^ p)^[d] y = y := by
    intro y
    rw [RelativeExtension.iterate_pow_apply, ← hcard]
    exact FiniteField.pow_card y
  have hid : ∀ (j : ℕ) (y : E), (fun y : E => y ^ p)^[d * j] y = y := by
    intro j
    induction j with
    | zero => intro y; simp
    | succ m ih =>
        intro y
        rw [Nat.mul_succ, Function.iterate_add_apply, hone, ih]
  conv_lhs => rw [← Nat.mod_add_div k d]
  rw [Function.iterate_add_apply, hid]

/-- **L2.8 (relative agreement).** `dB` absolute steps make one relative step:
the production loop that applies `pow (characteristic)` `dB · j` times computes
`x ↦ x^{q^j}` for `q = p^{dB}`, which is
`FieldExtension::relative_frobenius` at `j`.

Production path: `QuotientElement::frobenius`
(`crates/gf2-core/src/gfpn/quotient.rs:865`).

Refinement anchor: `frobenius_has_absolute_and_relative_orders`
(`crates/gf2-core/src/gfpn/quotient.rs:2147`). -/
theorem absFrobenius_iterate_baseDegree {E : Type*} [Monoid E] (p dB j : ℕ) (x : E) :
    (fun y : E => y ^ p)^[dB * j] x = x ^ (p ^ dB) ^ j := by
  rw [RelativeExtension.iterate_pow_apply, pow_mul]

/-- **L2.8 at the quotient.** An irreducible modulus makes `Q` a finite field of
order `|B|^r = p^{dB · r}`, so its absolute degree is `dB · r` — the product
`checked_degrees` (`crates/gf2-core/src/gfpn/quotient.rs:633`) computes — and
reducing `k` modulo it before iterating is sound.

Refinement anchor: `frobenius_has_absolute_and_relative_orders`
(`crates/gf2-core/src/gfpn/quotient.rs:2147`). -/
theorem adjoinRoot_frobenius_iterate_mod [Finite B] (hf : f.Monic) (hirr : Irreducible f)
    {p dB : ℕ} (hcard : Nat.card B = p ^ dB) (k : ℕ) (x : AdjoinRoot f) :
    letI := adjoinRootField hirr
    (fun y : AdjoinRoot f => y ^ p)^[k] x =
      (fun y : AdjoinRoot f => y ^ p)^[k % (dB * f.natDegree)] x := by
  letI := adjoinRootField hirr
  haveI : Finite (AdjoinRoot f) := Finite.of_equiv _ (clsEquiv hf)
  haveI : Fintype (AdjoinRoot f) := Fintype.ofFinite _
  refine absFrobenius_iterate_mod ?_ k x
  rw [← Nat.card_eq_fintype_card, card_adjoinRoot hf, hcard, ← pow_mul]


end QuotientReduction

end
