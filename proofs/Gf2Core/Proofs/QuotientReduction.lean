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

end QuotientReduction

end
