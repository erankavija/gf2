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


end QuotientReduction

end
