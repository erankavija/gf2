/-
  Gf2Core.Proofs.BchGenerator — obligation O-4 of the algebraic-foundations
  proof sketch (issue b1bd75ca): generator base-field membership and root
  correctness.

  Binding mode: abstract model plus refinement, with no extraction anchor.
  `gf2-coding` appears in no Charon invocation of `scripts/verify-lean.sh`, so
  no lemma here is a statement about extracted code. The model is stated over
  Mathlib: fields `B ⊆ E` with `E` finite, a root `α : E` of exact
  multiplicative order `n`, the base cardinality `q = Nat.card B` coprime to
  `n`, and a `μ_q`-closed defining set `T : Finset (ZMod n)`. The embedding, the
  relative Frobenius, the fixed-field characterisation and the conjugate orbit
  come from `Gf2Core.Proofs.RelativeExtension` (O-1's L1.3, L1.5 and L1.8); the
  exponent orbits, their least representatives and the closure come from
  `Gf2Core.Proofs.CyclotomicClosure` (O-3's L3.2 and L3.5). Neither is restated
  here.

  The production path is `BchCode::construct`
  (`crates/gf2-coding/src/bch/spec.rs:633`). Its supporting core functions are
  `minimal_polynomial` (`crates/gf2-core/src/field/extension.rs:2390`) and
  `conjugates` (`:2329`), together with `FieldPoly::lcm`
  (`crates/gf2-core/src/field/poly.rs:1934`), `FieldPoly::div_rem` (`:1779`) and
  `FieldPoly::eval` (`:1144`).

  Every model lemma names its refinement anchor: an executable Rust check that
  decides the same statement on that path. The anchors live in the production
  module's own test module and in the extension module's. One anchor is specific
  to this obligation: `the_generator_vanishes_at_every_defining_set_root`
  (`crates/gf2-coding/src/bch/spec.rs:2129`) lifts each constructed generator
  into `E` through `FieldExtension::embed`
  (`crates/gf2-core/src/field/extension.rs:2558`) and decides that it vanishes at
  `root ^ j` for exactly the exponents `j` of the defining set, over the binary,
  GF(5)-base and GF(9)-base codes the suite builds. Its shared generic helper is
  `assert_generator_vanishes_exactly_on_the_defining_set`
  (`crates/gf2-coding/src/bch/spec.rs:2102`).

  Scope: L4.7 and L4.8 characterise the witnessed run, a combinatorial property
  of `T`. The step from a run of `δ - 1` consecutive exponents to a
  minimum-distance claim is the classical Vandermonde argument, out of scope by
  the sketch's register row A-09 and risk R-02. `α` having exact order `n` is a
  hypothesis of the model (row A-07), discharged in production by
  `validate_length_divides_unit_group`
  (`crates/gf2-coding/src/bch/spec.rs:1271`), `resolve_root` (`:1320`),
  `multiplicative_order` (`:1354`) and `has_exact_order`
  (`crates/gf2-core/src/field/extension.rs:1678`). The model is
  representation-free, so the typed size rejections row A-10 names are outside
  it.

  Axiom footprint: this module declares no axiom and contains no `sorry`. Every
  declaration rests on `propext`, `Classical.choice` and `Quot.sound` alone, and
  several on fewer. The dependency on `Gf2Core.Proofs.RelativeExtension` reaches
  only its Section 1 model, which is stated over Mathlib, so the external-type
  axiom `Aeneas.Std.core.fmt.Formatter` that the extracted carriers of that
  module carry does not reach any declaration here.

  Every lemma statement is the sketch's. Four steps of the sketch's proof
  strategy are reached by a different Mathlib route:

  * L4.2 — the sketch names `lcm` in the `NormalizedGCDMonoid` structure of
    `B[X]`. `generator` is the product over the coset representatives, and
    `minpoly_dvd_generator` with `generator_dvd_of_forall_minpoly_dvd` proves
    the two halves of the least-common-multiple property, which is the contract
    `FieldPoly::lcm` (`crates/gf2-core/src/field/poly.rs:1934`) implements. This states the same
    fact without depending on which `GCDMonoid` instance `B[X]` carries.
  * L4.5 — the sketch names a separability-and-descent argument through
    `Polynomial.map_modByMonic`. `prod_X_sub_C_dvd` reaches the same conclusion
    from `Polynomial.prod_multiset_X_sub_C_dvd`: the roots of `g` are `|T|`
    distinct roots of `X ^ n - 1`, so the product of their linear factors
    divides it, and `Polynomial.map_dvd_map'` descends the divisibility to
    `B[X]` without a separate separability step.
  * L4.6 — the sketch derives `deg g = |T|` from the coprimality of the minimal
    polynomials of distinct cosets. `map_generator` gives the stronger identity
    `ι_* g = ∏_{j ∈ T} (X - α ^ j)` directly, so the degree is the cardinality
    and the coprimality argument is not needed.
  * L4.7 — the sketch names `Fin n` combinatorics. `runLen` is `Nat.find` on the
    scan's own stopping condition over `ZMod n`, mirroring
    `CyclotomicClosure.cosetLen`, because the production loop computes exactly
    that minimum (`crates/gf2-coding/src/bch/spec.rs:1554-1556`).

  Every line this module cites under `crates/gf2-core/` is also the line the
  sketch cites. The `crates/gf2-coding/src/bch/spec.rs` citations have drifted by several
  hundred lines since the sketch was written; the sketch's own O-4 section
  carries the remapped lines, and the citations here are the ones that hold at
  this revision.
-/
import Mathlib.FieldTheory.Minpoly.Field
import Mathlib.RingTheory.RootsOfUnity.PrimitiveRoots
import Mathlib.RingTheory.Polynomial.Cyclotomic.Basic
import Gf2Core.Proofs.CyclotomicClosure

set_option maxHeartbeats 1600000
set_option linter.unusedSectionVars false

noncomputable section

attribute [local instance 0] Classical.propDecidable

namespace BchGenerator

open Polynomial RelativeExtension

variable {B E : Type*} [Field B] [Field E] [Algebra B E] [Finite E]
variable {n : ℕ} [NeZero n]

/-! ## Section 1 — powers of the root -/

/-- `α ^ j` for an exponent `j` of `ZMod n`. -/
def alphaPow (α : E) (j : ZMod n) : E := α ^ j.val

/-- `α ^ m` depends on `m` only through its residue, because `α ^ n = 1`. -/
theorem pow_mod {α : E} (hord : orderOf α = n) (m : ℕ) : α ^ (m % n) = α ^ m := by
  conv_rhs => rw [← Nat.div_add_mod m n]
  rw [pow_add, pow_mul, ← hord, pow_orderOf_eq_one, one_pow, one_mul]

/-- A natural exponent names the power of `α` its residue names. -/
theorem alphaPow_natCast {α : E} (hord : orderOf α = n) (m : ℕ) :
    alphaPow α ((m : ZMod n)) = α ^ m := by
  show α ^ ((m : ZMod n)).val = α ^ m
  rw [ZMod.val_natCast]
  exact pow_mod hord m

/-- The root `α` is a primitive `n`-th root of unity, the form of `ord(α) = n`
Mathlib's roots-of-unity API consumes. -/
theorem isPrimitiveRoot {α : E} (hord : orderOf α = n) : IsPrimitiveRoot α n :=
  IsPrimitiveRoot.iff_orderOf.2 hord

/-- Distinct exponents of `ZMod n` name distinct powers of `α`. -/
theorem alphaPow_injective {α : E} (hord : orderOf α = n) :
    Function.Injective (alphaPow α : ZMod n → E) := by
  intro i j hij
  have := (isPrimitiveRoot hord).pow_inj (ZMod.val_lt i) (ZMod.val_lt j) hij
  exact (ZMod.val_injective n) this

/-- The zeroth power is one. -/
@[simp]
theorem alphaPow_zero (α : E) : alphaPow α (0 : ZMod n) = 1 := by
  simp [alphaPow]

/-- Exponents add, so `alphaPow` is a monoid homomorphism from `ZMod n`. -/
theorem alphaPow_add {α : E} (hord : orderOf α = n) (i j : ZMod n) :
    alphaPow α (i + j) = alphaPow α i * alphaPow α j := by
  have h : alphaPow α i * alphaPow α j = α ^ (i.val + j.val) := by
    rw [pow_add]; rfl
  rw [h, ← alphaPow_natCast hord (i.val + j.val), Nat.cast_add,
    ZMod.natCast_zmod_val, ZMod.natCast_zmod_val]

/-- **L4.4 (the bridge lemma).** The relative Frobenius acts on powers of `α` as `μ_q` acts on
their exponents. This is what ties the exponent combinatorics of O-3 to the field statement
of this obligation, and it is where `ord(α) = n` is used.

Production path: `derive_generator` (`crates/gf2-coding/src/bch/spec.rs:1439`) raises the root
to each coset representative and hands the result to `minimal_polynomial`
(`crates/gf2-core/src/field/extension.rs:2390`), whose orbit is generated by the relative
Frobenius.

Refinement anchor: the closure assertion of `assert_construction_is_consistent`
(`crates/gf2-coding/src/bch/spec.rs:1948-1953`), which recomputes `exponent * q mod n` for every
member of the defining set, and `the_generator_vanishes_at_every_defining_set_root`
(`crates/gf2-coding/src/bch/spec.rs:2129`), which decides the field statement directly. -/
theorem relFrobenius_alphaPow {α : E} (hord : orderOf α = n) (j : ZMod n) :
    relFrobenius B (alphaPow α j) = alphaPow α (CyclotomicClosure.mu (Nat.card B) j) := by
  rw [relFrobenius_apply]
  show (α ^ j.val) ^ Nat.card B = _
  rw [← pow_mul, ← alphaPow_natCast hord (j.val * Nat.card B), CyclotomicClosure.mu_apply,
    Nat.cast_mul, ZMod.natCast_zmod_val, mul_comm]

/-! ## Section 2 — L4.1: the conjugate product is the minimal polynomial -/

variable (B)

/-- The conjugate orbit of `x` as a finite set. -/
def orbitFinset (x : E) : Finset E :=
  (Finset.range (orbitPeriod B x)).image fun i => (relFrobenius B : E → E)^[i] x

/-- Orbit membership in the form the `conjugates` loop advances
(`crates/gf2-core/src/field/extension.rs:2329`): reachable from `x` by fewer than `ℓ`
Frobenius steps. -/
theorem mem_orbitFinset {x y : E} :
    y ∈ orbitFinset B x ↔ ∃ i < orbitPeriod B x, (relFrobenius B : E → E)^[i] x = y := by
  simp [orbitFinset]

/-- The orbit holds `ℓ` distinct conjugates, by O-1's L1.8. -/
theorem card_orbitFinset (x : E) : (orbitFinset B x).card = orbitPeriod B x := by
  rw [orbitFinset, Finset.card_image_of_injOn, Finset.card_range]
  intro i hi j hj hij
  exact orbit_injOn B x (Finset.mem_range.1 hi) (Finset.mem_range.1 hj) hij

/-- `x` is its own first conjugate, the entry the loop pushes first. -/
theorem self_mem_orbitFinset (x : E) : x ∈ orbitFinset B x :=
  (mem_orbitFinset B).2 ⟨0, orbitPeriod_pos B x, rfl⟩

/-- One more Frobenius step stays inside the orbit. -/
theorem relFrobenius_mem_orbitFinset {x y : E} (hy : y ∈ orbitFinset B x) :
    relFrobenius B y ∈ orbitFinset B x := by
  obtain ⟨i, hi, rfl⟩ := (mem_orbitFinset B).1 hy
  refine (mem_orbitFinset B).2
    ⟨(i + 1) % orbitPeriod B x, Nat.mod_lt _ (orbitPeriod_pos B x), ?_⟩
  rw [(iterate_eq_iterate_iff_modEq B x ((i + 1) % orbitPeriod B x) (i + 1)).2
      (Nat.mod_modEq _ _), Function.iterate_succ_apply']

/-- `φ_B` permutes the conjugate orbit, which is what makes the product over
it `φ_B`-invariant. -/
theorem image_relFrobenius_orbitFinset (x : E) :
    (orbitFinset B x).image (relFrobenius B) = orbitFinset B x := by
  refine Finset.eq_of_subset_of_card_le (fun y hy => ?_) ?_
  · obtain ⟨z, hz, rfl⟩ := Finset.mem_image.1 hy
    exact relFrobenius_mem_orbitFinset B hz
  · rw [Finset.card_image_of_injective _ (relFrobenius_injective B)]

/-- `m_x` as computed in `E`: the product over the conjugate orbit. -/
def conjProd (x : E) : Polynomial E := ∏ y ∈ orbitFinset B x, (X - C y)

/-- The conjugate product is monic, being a product of monic linear factors. -/
theorem conjProd_monic (x : E) : (conjProd B x).Monic :=
  monic_prod_of_monic _ _ fun y _ => monic_X_sub_C y

/-- The conjugate product has the degree of the orbit it runs over. -/
theorem natDegree_conjProd (x : E) : (conjProd B x).natDegree = orbitPeriod B x := by
  rw [conjProd, natDegree_prod_of_monic _ _ fun y _ => monic_X_sub_C y]
  simp [card_orbitFinset]

/-- The conjugate product vanishes at `x`, because `x` is one of its roots. -/
theorem eval_conjProd_self (x : E) : (conjProd B x).eval x = 0 := by
  rw [conjProd, eval_prod]
  exact Finset.prod_eq_zero (self_mem_orbitFinset B x) (by simp)

/-- Applying `φ_B` coefficientwise permutes the factors cyclically and so
leaves the conjugate product unchanged. This is the step that puts the
coefficients in `ι(B)`. -/
theorem map_relFrobenius_conjProd (x : E) :
    (conjProd B x).map (relFrobenius B) = conjProd B x := by
  have hinj : Set.InjOn (relFrobenius B) (↑(orbitFinset B x) : Set E) :=
    fun a _ b _ h => relFrobenius_injective B h

  calc (conjProd B x).map (relFrobenius B)
      = ∏ y ∈ orbitFinset B x, (X - C (relFrobenius B y)) := by
        rw [conjProd, Polynomial.map_prod]
        exact Finset.prod_congr rfl fun y _ => by simp
    _ = ∏ z ∈ (orbitFinset B x).image (relFrobenius B), (X - C z) :=
        (Finset.prod_image (f := fun z : E => (X : Polynomial E) - C z) hinj).symm
    _ = conjProd B x := by rw [image_relFrobenius_orbitFinset, conjProd]

/-- **L4.1 (b).** Every coefficient of `m_x` is `φ_B`-fixed, hence in `ι(B)`. This is what
discharges the coefficient restriction inside `minimal_polynomial`
(`crates/gf2-core/src/field/extension.rs:2398`): `restrict_invariant` (`:2507`) never
panics.

Refinement anchor: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4104`), whose base-membership assertion restricts every
coefficient. -/
theorem conjProd_coeff_mem_range (x : E) (k : ℕ) :
    (conjProd B x).coeff k ∈ Set.range (algebraMap B E) := by
  rw [← fixedPoints_eq_range B (E := E)]
  show relFrobenius B ((conjProd B x).coeff k) = (conjProd B x).coeff k
  conv_rhs => rw [← map_relFrobenius_conjProd B x]
  rw [coeff_map]

/-- The conjugate product lifts to a monic polynomial over `B` of the same degree, the
polynomial `minimal_polynomial` (`crates/gf2-core/src/field/extension.rs:2390`) returns. -/
theorem exists_lift_conjProd (x : E) :
    ∃ m : Polynomial B, m.Monic ∧ m.map (algebraMap B E) = conjProd B x ∧
      m.natDegree = orbitPeriod B x := by
  obtain ⟨m, hmap, _, hmonic⟩ :=
    Polynomial.lifts_and_degree_eq_and_monic
      ((Polynomial.lifts_iff_coeff_lifts _).2 (conjProd_coeff_mem_range B x))
      (conjProd_monic B x)
  exact ⟨m, hmonic, hmap, by rw [← natDegree_conjProd B x, ← hmap, hmonic.natDegree_map]⟩

/-- The image of `minpoly B x` in `E[X]` is `φ_B`-invariant, because its
coefficients come from `ι(B)`. -/
theorem map_relFrobenius_map_minpoly (x : E) :
    ((minpoly B x).map (algebraMap B E)).map (relFrobenius B)
      = (minpoly B x).map (algebraMap B E) := by
  rw [Polynomial.map_map]
  congr 1
  exact RingHom.ext fun a => relFrobenius_algebraMap B a

/-- `φ_B` commutes with evaluation of a `φ_B`-invariant polynomial. -/
theorem eval_relFrobenius {M : Polynomial E} (hM : M.map (relFrobenius B) = M) (y : E) :
    M.eval (relFrobenius B y) = relFrobenius B (M.eval y) := by
  conv_lhs => rw [← hM]
  rw [eval_map, eval₂_hom]

/-- Every conjugate of `x` is a root of the image of `minpoly B x`, because
`φ_B` fixes the coefficients. -/
theorem eval_map_minpoly_iterate (x : E) (i : ℕ) :
    ((minpoly B x).map (algebraMap B E)).eval ((relFrobenius B : E → E)^[i] x) = 0 := by
  induction i with
  | zero =>
      show ((minpoly B x).map (algebraMap B E)).eval x = 0
      rw [Polynomial.eval_map, ← Polynomial.aeval_def]
      exact minpoly.aeval B x
  | succ i ih =>
      rw [Function.iterate_succ_apply', eval_relFrobenius B (map_relFrobenius_map_minpoly B x),
        ih, map_zero]

/-- The image of a monic minimal polynomial is nonzero. -/
theorem map_minpoly_ne_zero (x : E) : (minpoly B x).map (algebraMap B E) ≠ 0 := by
  refine fun h => ?_
  have hmonic : (minpoly B x).Monic := minpoly.monic (IsIntegral.of_finite B x)
  exact (hmonic.map (algebraMap B E)).ne_zero h

/-- The `ℓ` distinct conjugates bound the degree of `minpoly B x` from below. -/
theorem orbitPeriod_le_natDegree_minpoly (x : E) :
    orbitPeriod B x ≤ (minpoly B x).natDegree := by
  set M := (minpoly B x).map (algebraMap B E) with hM
  have hsub : orbitFinset B x ⊆ M.roots.toFinset := by
    intro y hy
    obtain ⟨i, _, rfl⟩ := (mem_orbitFinset B).1 hy
    exact Multiset.mem_toFinset.2
      ((Polynomial.mem_roots (map_minpoly_ne_zero B x)).2 (eval_map_minpoly_iterate B x i))
  calc orbitPeriod B x = (orbitFinset B x).card := (card_orbitFinset B x).symm
    _ ≤ M.roots.toFinset.card := Finset.card_le_card hsub
    _ ≤ Multiset.card M.roots := Multiset.toFinset_card_le _
    _ ≤ M.natDegree := Polynomial.card_roots' M
    _ = (minpoly B x).natDegree := (minpoly.monic (IsIntegral.of_finite B x)).natDegree_map _

/-- **L4.1 (d).** The conjugate product is the image of `minpoly B x`, so `minpoly B x` is the
model's `m_x` over the base field and parts (a) and (c) read off it.

Production path: `minimal_polynomial` (`crates/gf2-core/src/field/extension.rs:2390`), which
forms the product over `conjugates` (`:2329`) with `FieldPoly::from_roots`
(`crates/gf2-core/src/field/poly.rs:1455`) and restricts the coefficients.

Refinement anchor: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4104`), driven by
`prop_binary_minimal_polynomial_and_relative_laws` (`:4171`),
`prop_odd_prime_minimal_polynomial_and_relative_laws` (`:4195`) and
`prop_odd_tower_minimal_polynomial` (`:4208`). -/
theorem conjProd_eq_map_minpoly (x : E) :
    conjProd B x = (minpoly B x).map (algebraMap B E) := by
  obtain ⟨m, hmonic, hmap, hdeg⟩ := exists_lift_conjProd B x
  have hint : IsIntegral B x := IsIntegral.of_finite B x
  have haeval : Polynomial.aeval x m = 0 := by
    rw [Polynomial.aeval_def, ← Polynomial.eval_map, hmap]
    exact eval_conjProd_self B x
  obtain ⟨c, hc⟩ : minpoly B x ∣ m := minpoly.dvd B x haeval
  have hcmonic : c.Monic := (minpoly.monic hint).of_mul_monic_left (hc ▸ hmonic)
  have hdeg' : (minpoly B x).natDegree + c.natDegree = orbitPeriod B x := by
    rw [← hdeg, hc, (minpoly.monic hint).natDegree_mul hcmonic]
  have hle := orbitPeriod_le_natDegree_minpoly B x
  have hc1 : c = 1 := eq_one_of_monic_natDegree_zero hcmonic (by omega)
  rw [← hmap, hc, hc1, mul_one]

/-- **L4.1 (a).** `m_x` is monic of degree `ℓ`, the length of the conjugate orbit.

Refinement anchor: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4104`), whose degree and monicity assertions decide
both halves. -/
theorem minpoly_monic_natDegree (x : E) :
    (minpoly B x).Monic ∧ (minpoly B x).natDegree = orbitPeriod B x := by
  refine ⟨minpoly.monic (IsIntegral.of_finite B x), ?_⟩
  rw [← natDegree_conjProd B x, conjProd_eq_map_minpoly B x,
    (minpoly.monic (IsIntegral.of_finite B x)).natDegree_map]

/-- **L4.1 (c).** `m_x` vanishes at `x`.

Refinement anchor: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4104`), whose root assertion evaluates the returned
polynomial at `x`. -/
theorem aeval_minpoly_self (x : E) : Polynomial.aeval x (minpoly B x) = 0 :=
  minpoly.aeval B x

/-! ## Section 3 — root sets and products of linear factors -/

/-- A nodup multiset of roots sits inside the root multiset. -/
theorem nodup_le_roots {p : Polynomial E} (hp : p ≠ 0) {S : Multiset E}
    (hnd : S.Nodup) (hroot : ∀ a ∈ S, p.eval a = 0) : S ≤ p.roots := by
  refine Multiset.le_iff_count.2 fun a => ?_
  by_cases ha : a ∈ S
  · rw [Multiset.count_eq_one_of_mem hnd ha]
    exact Multiset.one_le_count_iff_mem.2 ((Polynomial.mem_roots hp).2 (hroot a ha))
  · simp [Multiset.count_eq_zero_of_notMem ha]

/-- The product of the distinct linear factors named by a finite set of roots
divides the polynomial. -/
theorem prod_X_sub_C_dvd {p : Polynomial E} (hp : p ≠ 0) {S : Finset E}
    (hroot : ∀ a ∈ S, p.eval a = 0) : (∏ a ∈ S, (X - C a)) ∣ p := by
  have hle : S.val ≤ p.roots := nodup_le_roots hp S.nodup (by simpa using hroot)
  have : (S.val.map fun a => X - C a).prod ∣ (p.roots.map fun a => X - C a).prod :=
    Multiset.prod_dvd_prod_of_le (Multiset.map_le_map hle)
  exact this.trans (Polynomial.prod_multiset_X_sub_C_dvd p)

/-- A product of distinct linear factors is monic. -/
theorem prod_X_sub_C_monic (S : Finset E) : (∏ a ∈ S, (X - C a)).Monic :=
  monic_prod_of_monic _ _ fun a _ => monic_X_sub_C a

/-- A product of linear factors has the degree of the index set. -/
theorem natDegree_prod_X_sub_C (S : Finset E) : (∏ a ∈ S, (X - C a)).natDegree = S.card := by
  rw [natDegree_prod_of_monic _ _ fun a _ => monic_X_sub_C a]
  simp

/-! ## Section 4 — L4.2 and L4.3: the generator over the base field -/

variable {α : E}

/-- **L4.4 (iterated).** Repeated conjugation multiplies the exponent by a
power of `q`, which is the form L4.3 consumes. -/
theorem iterate_relFrobenius_alphaPow (hord : orderOf α = n) (c : ZMod n) (i : ℕ) :
    (relFrobenius B : E → E)^[i] (alphaPow α c)
      = alphaPow α ((Nat.card B : ZMod n) ^ i * c) := by
  induction i with
  | zero => simp
  | succ i ih =>
      rw [Function.iterate_succ_apply', ih, relFrobenius_alphaPow hord,
        CyclotomicClosure.mu_apply, ← mul_assoc, ← pow_succ']

/-- The conjugate orbit of `α ^ c` has the length of the `μ_q`-orbit of `c`. -/
theorem orbitPeriod_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    (c : ZMod n) : orbitPeriod B (alphaPow α c) = CyclotomicClosure.cosetLen hq c := by
  have key : ∀ t : ℕ, (relFrobenius B : E → E)^[t] (alphaPow α c) = alphaPow α c ↔
      CyclotomicClosure.cosetLen hq c ∣ t := by
    intro t
    rw [iterate_relFrobenius_alphaPow B hord, ← CyclotomicClosure.pow_mul_eq_self_iff_dvd hq c t]
    exact ⟨fun h => alphaPow_injective hord h, fun h => congrArg _ h⟩
  exact Nat.dvd_antisymm ((iterate_eq_self_iff_dvd B _ _).1 ((key _).2 dvd_rfl))
    ((key _).1 (relFrobenius_iterate_orbitPeriod B _))

/-- One emitted cyclotomic coset as a finite set of exponents. -/
def cosetFinset {q : ℕ} (hq : Nat.Coprime q n) (c : ZMod n) : Finset (ZMod n) :=
  (CyclotomicClosure.cosetList hq c).toFinset

/-- Coset membership unfolds to reachability from `c` under `μ_q`. -/
theorem mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) (c x : ZMod n) :
    x ∈ cosetFinset hq c ↔ ∃ t : ℕ, (q : ZMod n) ^ t * c = x := by
  rw [cosetFinset, List.mem_toFinset, CyclotomicClosure.mem_cosetList_iff,
    CyclotomicClosure.mem_orbit_iff_exists_pow]

/-- A representative lies in its own coset, the head the emitting pass pushes. -/
theorem self_mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) (c : ZMod n) :
    c ∈ cosetFinset hq c :=
  (mem_cosetFinset hq c c).2 ⟨0, by simp⟩

/-- A coset is determined by any of its members, so the representative the
scan enters at names the same set. -/
theorem cosetFinset_eq_of_mem {q : ℕ} (hq : Nat.Coprime q n) {c x : ZMod n}
    (hx : x ∈ cosetFinset hq c) : cosetFinset hq x = cosetFinset hq c := by
  ext y
  rw [cosetFinset, cosetFinset, List.mem_toFinset, List.mem_toFinset,
    CyclotomicClosure.mem_cosetList_iff, CyclotomicClosure.mem_cosetList_iff,
    CyclotomicClosure.orbit_eq_of_mem hq ((CyclotomicClosure.mem_cosetList_iff hq c x).1
      (List.mem_toFinset.1 hx))]

/-- The conjugate orbit of `α ^ c` is the image of the coset of `c`. -/
theorem orbitFinset_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    (c : ZMod n) :
    orbitFinset B (alphaPow α c) = (cosetFinset hq c).image (alphaPow α) := by
  ext y
  rw [mem_orbitFinset B, Finset.mem_image]
  constructor
  · rintro ⟨i, hi, rfl⟩
    exact ⟨_, (mem_cosetFinset hq c _).2 ⟨i, rfl⟩,
      (iterate_relFrobenius_alphaPow B hord c i).symm⟩
  · rintro ⟨j, hj, rfl⟩
    obtain ⟨t, rfl⟩ := (mem_cosetFinset hq c j).1 hj
    refine ⟨t % CyclotomicClosure.cosetLen hq c, ?_, ?_⟩
    · rw [orbitPeriod_alphaPow B hord hq]
      exact Nat.mod_lt _ (CyclotomicClosure.cosetLen_pos hq c)
    · rw [iterate_relFrobenius_alphaPow B hord, CyclotomicClosure.pow_mul_mod]

/-- The coset representatives of an exponent set: the least member of each
orbit, which is the head of each emitted coset. -/
def reps {q : ℕ} (hq : Nat.Coprime q n) (T : Finset (ZMod n)) : Finset (ZMod n) :=
  T.filter fun c => ((CyclotomicClosure.orbitMin hq c : ℕ) : ZMod n) = c

/-- Membership in the representative set unfolds to being the least member of
its own orbit. -/
theorem mem_reps {q : ℕ} (hq : Nat.Coprime q n) (T : Finset (ZMod n)) (c : ZMod n) :
    c ∈ reps hq T ↔ c ∈ T ∧ ((CyclotomicClosure.orbitMin hq c : ℕ) : ZMod n) = c := by
  simp [reps]

/-- A `μ_q`-closed exponent set holds the whole coset of each of its members,
which is what O-3's closure guarantees about the defining set. -/
theorem mem_of_mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) {T : Finset (ZMod n)}
    (hclosed : ∀ j ∈ T, CyclotomicClosure.mu q j ∈ T) {c x : ZMod n} (hc : c ∈ T)
    (hx : x ∈ cosetFinset hq c) : x ∈ T := by
  obtain ⟨t, rfl⟩ := (mem_cosetFinset hq c x).1 hx
  exact CyclotomicClosure.pow_mul_mem_of_closed (U := (↑T : Set (ZMod n)))
    (fun y hy => hclosed y hy) hc t

/-- Distinct representatives name disjoint cosets, by O-3's L3.2. -/
theorem cosetFinset_pairwiseDisjoint {q : ℕ} (hq : Nat.Coprime q n) (T : Finset (ZMod n)) :
    (↑(reps hq T) : Set (ZMod n)).PairwiseDisjoint (cosetFinset hq) := by
  intro a ha b hb hab
  refine Finset.disjoint_left.2 fun x hxa hxb => hab ?_
  have h : cosetFinset hq a = cosetFinset hq b :=
    (cosetFinset_eq_of_mem hq hxa).symm.trans (cosetFinset_eq_of_mem hq hxb)
  have horb : CyclotomicClosure.orbit hq a = CyclotomicClosure.orbit hq b := by
    ext y
    have := congrArg (fun s : Finset (ZMod n) => y ∈ s) h
    simpa [cosetFinset, List.mem_toFinset, CyclotomicClosure.mem_cosetList_iff] using this
  have hmin := CyclotomicClosure.orbitMin_eq_of_orbit_eq hq horb
  rw [← ((mem_reps hq T a).1 ha).2, ← ((mem_reps hq T b).1 hb).2, hmin]

/-- The defining set is the disjoint union of the cosets of its representatives, which is what
`derive_generator` (`crates/gf2-coding/src/bch/spec.rs:1444`) iterates over. -/
theorem biUnion_cosetFinset_reps {q : ℕ} (hq : Nat.Coprime q n) {T : Finset (ZMod n)}
    (hclosed : ∀ j ∈ T, CyclotomicClosure.mu q j ∈ T) :
    (reps hq T).biUnion (cosetFinset hq) = T := by
  ext x
  rw [Finset.mem_biUnion]
  constructor
  · rintro ⟨c, hc, hx⟩
    exact mem_of_mem_cosetFinset hq hclosed ((mem_reps hq T c).1 hc).1 hx
  · intro hx
    refine ⟨((CyclotomicClosure.orbitMin hq x : ℕ) : ZMod n), ?_, ?_⟩
    · have hmem : ((CyclotomicClosure.orbitMin hq x : ℕ) : ZMod n) ∈ cosetFinset hq x :=
        List.mem_toFinset.2 ((CyclotomicClosure.mem_cosetList_iff hq x _).2
          (CyclotomicClosure.orbitMin_mem hq x))
      refine (mem_reps hq T _).2 ⟨mem_of_mem_cosetFinset hq hclosed hx hmem, ?_⟩
      exact congrArg (fun m : ℕ => (m : ZMod n)) (CyclotomicClosure.orbitMin_eq_of_orbit_eq hq
        (CyclotomicClosure.orbit_eq_of_mem hq (CyclotomicClosure.orbitMin_mem hq x)))
    · have hmem : ((CyclotomicClosure.orbitMin hq x : ℕ) : ZMod n) ∈ cosetFinset hq x :=
        List.mem_toFinset.2 ((CyclotomicClosure.mem_cosetList_iff hq x _).2
          (CyclotomicClosure.orbitMin_mem hq x))
      rw [cosetFinset_eq_of_mem hq hmem]
      exact self_mem_cosetFinset hq x

/-- **L4.2 (base-field membership).** The generator: the product of the minimal
polynomials of the coset representatives. It is a polynomial over `B` by
typing, which is the membership half of this obligation, because L4.1 (b) has
already put each factor there.

Production path: `derive_generator` (`crates/gf2-coding/src/bch/spec.rs:1439`), which folds
`FieldPoly::lcm` (`crates/gf2-core/src/field/poly.rs:1934`) over the coset minimal polynomials.

Refinement anchors: `prime_base_primitive_construction_derives_a_base_field_generator`
(`crates/gf2-coding/src/bch/spec.rs:2051`) and
`extension_base_primitive_construction_derives_a_base_field_generator`
(`crates/gf2-coding/src/bch/spec.rs:2071`), which assert `base_field_id()` against the intended
base. -/
def generator (hq : Nat.Coprime (Nat.card B) n) (α : E) (T : Finset (ZMod n)) : Polynomial B :=
  ∏ c ∈ reps hq T, minpoly B (alphaPow α c)

/-- **L4.2 (monic).** The generator is monic, so it has a degree and the division that forms the
dimension is exact.

Refinement anchor: `assert_construction_is_consistent`
(`crates/gf2-coding/src/bch/spec.rs:1928`), whose `degree()` call rejects a zero generator. -/
theorem generator_monic (hq : Nat.Coprime (Nat.card B) n) (α : E) (T : Finset (ZMod n)) :
    (generator B hq α T).Monic :=
  monic_prod_of_monic _ _ fun _ _ => minpoly.monic (IsIntegral.of_finite B _)

/-- The image of the generator in `E[X]` is the product of the linear factors
named by the defining set. -/
theorem map_generator (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (generator B hq α T).map (algebraMap B E) = ∏ j ∈ T, (X - C (alphaPow α j)) := by
  rw [generator, Polynomial.map_prod]
  have h1 : ∀ c ∈ reps hq T, (minpoly B (alphaPow α c)).map (algebraMap B E)
      = ∏ j ∈ cosetFinset hq c, (X - C (alphaPow α j)) := by
    intro c _
    rw [← conjProd_eq_map_minpoly B, conjProd, orbitFinset_alphaPow B hord hq]
    exact Finset.prod_image (fun a _ b _ h => alphaPow_injective hord h)
  rw [Finset.prod_congr rfl h1, ← Finset.prod_biUnion (cosetFinset_pairwiseDisjoint hq T),
    biUnion_cosetFinset_reps hq hclosed]

/-- **L4.3 (root correctness).** The generator vanishes at `α ^ j` for every
exponent `j` of the defining set. This is the root-correctness half of the
obligation.

Production path: `derive_generator` (`crates/gf2-coding/src/bch/spec.rs:1439`) together with
`assemble` (`:1488`), which stores the defining set the generator's roots are
indexed by.

Refinement anchor: `the_generator_vanishes_at_every_defining_set_root`
(`crates/gf2-coding/src/bch/spec.rs:2129`), whose shared helper
`assert_generator_vanishes_exactly_on_the_defining_set` (`:2102`) lifts the
generator into `E` and decides `eval(root ^ j).is_zero()` against defining-set
membership for every exponent below `n`. -/
theorem aeval_generator_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    {j : ZMod n} (hj : j ∈ T) :
    Polynomial.aeval (alphaPow α j) (generator B hq α T) = 0 := by
  rw [Polynomial.aeval_def, ← Polynomial.eval_map, map_generator B hord hq hclosed, eval_prod]
  exact Finset.prod_eq_zero hj (by simp)

/-! ## Section 5 — L4.2 (least common multiple), L4.5 and L4.6 -/

/-- The image of the monic generator is nonzero. -/
theorem map_generator_ne_zero (hq : Nat.Coprime (Nat.card B) n) (α : E) (T : Finset (ZMod n)) :
    (generator B hq α T).map (algebraMap B E) ≠ 0 :=
  ((generator_monic B hq α T).map (algebraMap B E)).ne_zero

/-- L4.3 restated as an evaluation in `E`, the form the anchor test decides. -/
theorem eval_map_generator_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    {j : ZMod n} (hj : j ∈ T) :
    ((generator B hq α T).map (algebraMap B E)).eval (alphaPow α j) = 0 := by
  rw [Polynomial.eval_map, ← Polynomial.aeval_def]
  exact aeval_generator_alphaPow B hord hq hclosed hj

/-- **L4.2 (lower bound).** Every coset minimal polynomial divides the
generator, so the generator is a common multiple of the family — the first half
of the `lcm` contract.

Production path: the `FieldPoly::lcm` fold of `derive_generator`
(`crates/gf2-coding/src/bch/spec.rs:1439-1447`).

Refinement anchor: `the_generator_vanishes_at_every_defining_set_root`
(`crates/gf2-coding/src/bch/spec.rs:2129`), which fails as soon as one coset's roots are missing
from the generator. -/
theorem minpoly_dvd_generator (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    {j : ZMod n} (hj : j ∈ T) :
    minpoly B (alphaPow α j) ∣ generator B hq α T := by
  rw [← Polynomial.map_dvd_map' (algebraMap B E), ← conjProd_eq_map_minpoly B, conjProd]
  refine prod_X_sub_C_dvd (map_generator_ne_zero B hq α T) fun y hy => ?_
  rw [orbitFinset_alphaPow B hord hq] at hy
  obtain ⟨k, hk, rfl⟩ := Finset.mem_image.1 hy
  exact eval_map_generator_alphaPow B hord hq hclosed
    (mem_of_mem_cosetFinset hq hclosed hj hk)

/-- **L4.2 (upper bound).** The generator divides every common multiple of the coset minimal
polynomials. With the lower bound this is the least-common- multiple property, which is what
`FieldPoly::lcm` (`crates/gf2-core/src/field/poly.rs:1934`) computes and what keeps the
generator's degree from exceeding `|T|`.

Refinement anchor: `assert_construction_is_consistent`
(`crates/gf2-coding/src/bch/spec.rs:1940`), whose `one root per generator degree` assertion
rejects a generator carrying a factor more than once. -/
theorem generator_dvd_of_forall_minpoly_dvd (hord : orderOf α = n)
    (hq : Nat.Coprime (Nat.card B) n) {T : Finset (ZMod n)}
    (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) {h : Polynomial B}
    (hh : ∀ j ∈ T, minpoly B (alphaPow α j) ∣ h) : generator B hq α T ∣ h := by
  rcases eq_or_ne h 0 with rfl | hne
  · exact dvd_zero _
  have hmap : h.map (algebraMap B E) ≠ 0 :=
    fun hz => hne (Polynomial.map_eq_zero_iff (algebraMap B E).injective |>.1 hz)
  rw [← Polynomial.map_dvd_map' (algebraMap B E), map_generator B hord hq hclosed]
  have himg : ∏ j ∈ T, (X - C (alphaPow α j))
      = ∏ y ∈ T.image (alphaPow α), (X - C y) :=
    (Finset.prod_image (f := fun y : E => (X : Polynomial E) - C y)
      (fun a _ b _ hab => alphaPow_injective hord hab)).symm
  rw [himg]
  refine prod_X_sub_C_dvd hmap fun y hy => ?_
  obtain ⟨j, hj, rfl⟩ := Finset.mem_image.1 hy
  obtain ⟨c, hc⟩ := hh j hj
  have : Polynomial.aeval (alphaPow α j) h = 0 := by
    rw [hc, map_mul, minpoly.aeval, zero_mul]
  rw [Polynomial.eval_map, ← Polynomial.aeval_def]
  exact this

/-- **L4.5 (divisibility).** The generator divides `X ^ n - 1` over the base field.

Production path: `derive_generator` re-checks exactly this at run time
(`crates/gf2-coding/src/bch/spec.rs:1458-1465`) against `cyclic_polynomial` (`:1470`) and
reports `BchError::GeneratorNotDivisorOfCyclicPolynomial` when it fails.

Refinement anchors: `generator_divides_cyclic_polynomial`
(`crates/gf2-coding/src/bch/spec.rs:1893`) through `assert_construction_is_consistent`
(`:1930`), and `binary_generators_divide_the_cyclic_polynomial` (`:2041`). -/
theorem generator_dvd_X_pow_sub_one (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    generator B hq α T ∣ (X ^ n - 1 : Polynomial B) := by
  have hmapc : ((X ^ n - 1 : Polynomial B)).map (algebraMap B E)
      = (X ^ n - 1 : Polynomial E) := by
    simp
  have hmonic : (X ^ n - 1 : Polynomial E).Monic := by
    simpa using monic_X_pow_sub_C (1 : E) (NeZero.ne n)
  rw [← Polynomial.map_dvd_map' (algebraMap B E), hmapc, map_generator B hord hq hclosed]
  have himg : ∏ j ∈ T, (X - C (alphaPow α j))
      = ∏ y ∈ T.image (alphaPow α), (X - C y) :=
    (Finset.prod_image (f := fun y : E => (X : Polynomial E) - C y)
      (fun a _ b _ hab => alphaPow_injective hord hab)).symm
  rw [himg]
  refine prod_X_sub_C_dvd hmonic.ne_zero fun y hy => ?_
  obtain ⟨j, _, rfl⟩ := Finset.mem_image.1 hy
  have hαn : α ^ n = 1 := by rw [← hord]; exact pow_orderOf_eq_one α
  have hpow : (alphaPow α j) ^ n = 1 := by
    show (α ^ j.val) ^ n = 1
    rw [← pow_mul, mul_comm, pow_mul, hαn, one_pow]
  simp [hpow]

/-- **L4.6 (degree).** The generator has one root per exponent of the defining set, so its
degree is `|T|`.

Production path: `assemble` (`crates/gf2-coding/src/bch/spec.rs:1488`) reads the degree off the
generator and the defining set off the closure.

Refinement anchor: `assert_construction_is_consistent`
(`crates/gf2-coding/src/bch/spec.rs:1940`), the `one root per generator degree` assertion. -/
theorem natDegree_generator (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (generator B hq α T).natDegree = T.card := by
  rw [← (generator_monic B hq α T).natDegree_map (algebraMap B E),
    map_generator B hord hq hclosed,
    natDegree_prod_of_monic _ _ fun j _ => monic_X_sub_C (alphaPow α j)]
  simp

/-- The defining set has at most `n` exponents, because it lives in `ZMod n`. -/
theorem card_le_length (T : Finset (ZMod n)) : T.card ≤ n := by
  calc T.card ≤ Fintype.card (ZMod n) := Finset.card_le_univ T
    _ = n := ZMod.card n

/-- **L4.6 (dimension).** `k = n - |T|` is a genuine subtraction, so the `checked_sub` that
forms the dimension (`crates/gf2-coding/src/bch/spec.rs:1505`) is total and `0 ≤ k ≤ n`.

Refinement anchor: `assert_construction_is_consistent`
(`crates/gf2-coding/src/bch/spec.rs:1929`), the `k must be n - deg(g)` assertion. -/
theorem dimension_add_natDegree (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (n - (generator B hq α T).natDegree) + (generator B hq α T).natDegree = n := by
  rw [natDegree_generator B hord hq hclosed]
  exact Nat.sub_add_cancel (card_le_length T)

/-! ## Section 6 — L4.7 and L4.8: the witnessed run -/

section Run

variable (T : Finset (ZMod n))

/-- The run from any exponent stops: `n` is always a stopping index. -/
theorem exists_run_stop (s : ZMod n) : ∃ r : ℕ, r = n ∨ (s + (r : ZMod n)) ∉ T :=
  ⟨n, Or.inl rfl⟩

/-- The run length from `s`: the number of consecutive exponents of `T` from `s` onwards, capped
at `n`.

Production path: the inner loop of `witness_longest_run`
(`crates/gf2-coding/src/bch/spec.rs:1554-1556`), whose guard `run < length && present[(start +
run) % length]` stops at the first absent exponent or at `length`. -/
def runLen (s : ZMod n) : ℕ := Nat.find (exists_run_stop T s)

/-- A run never exceeds the code length, the `run < length` half of the loop
guard. -/
theorem runLen_le (s : ZMod n) : runLen T s ≤ n := Nat.find_le (Or.inl rfl)

/-- Every exponent the run passes is present. -/
theorem mem_of_lt_runLen (s : ZMod n) {i : ℕ} (hi : i < runLen T s) : s + (i : ZMod n) ∈ T := by
  have h := Nat.find_min (exists_run_stop T s) hi
  simp only [not_or, not_not] at h
  exact h.2

/-- A run shorter than the whole cycle stops at an absent exponent. -/
theorem notMem_of_runLen_lt (s : ZMod n) (h : runLen T s < n) :
    s + ((runLen T s : ℕ) : ZMod n) ∉ T := by
  rcases Nat.find_spec (exists_run_stop T s) with heq | hmem
  · exact absurd heq (Nat.ne_of_lt h)
  · exact hmem

/-- A block of `m` present exponents forces the run to reach `m`, or the whole
cycle. -/
theorem le_runLen_of_forall {s : ZMod n} {m : ℕ} (h : ∀ i < m, s + (i : ZMod n) ∈ T) :
    min m n ≤ runLen T s := by
  by_cases hlt : runLen T s < m
  · rcases Nat.find_spec (exists_run_stop T s) with heq | hmem
    · exact le_trans (min_le_right m n) (le_of_eq heq.symm)
    · exact absurd (h _ hlt) hmem
  · exact le_trans (min_le_left m n) (Nat.not_lt.1 hlt)

/-- An absent exponent starts no run. -/
theorem runLen_eq_zero_of_notMem {s : ZMod n} (hs : s ∉ T) : runLen T s = 0 := by
  by_contra h
  exact hs (by simpa using mem_of_lt_runLen T s (Nat.pos_of_ne_zero h))

/-- With every exponent present every run wraps the whole cycle. -/
theorem runLen_eq_length_of_univ (hT : ∀ x : ZMod n, x ∈ T) (s : ZMod n) : runLen T s = n := by
  refine le_antisymm (runLen_le T s) ?_
  have h := le_runLen_of_forall T (s := s) (m := n) fun i _ => hT (s + (i : ZMod n))
  rwa [min_self] at h

/-- The scan's guard: `s` starts a run when `s` is present and `s - 1` is not.

Production path: the `continue` guard of `witness_longest_run`
(`crates/gf2-coding/src/bch/spec.rs:1552`), `!present[start] || present[(start + length - 1) %
length]`. -/
def IsRunStart (s : ZMod n) : Prop := s ∈ T ∧ s - 1 ∉ T

/-- The exponents at which the scan opens a run: the candidates the outer loop
of `witness_longest_run` (`crates/gf2-coding/src/bch/spec.rs:1551`) does not skip. -/
def runStarts : Finset (ZMod n) := Finset.univ.filter fun s => IsRunStart T s

/-- Membership in the candidate set is the scan's guard. -/
theorem mem_runStarts {s : ZMod n} : s ∈ runStarts T ↔ IsRunStart T s := by
  simp [runStarts]

/-- The longest cyclic run of consecutive members of `T`: the `best_run` the
scan accumulates (`crates/gf2-coding/src/bch/spec.rs:1558-1562`).

Refinement anchor: `assert_witnessed_run_is_maximal` (`crates/gf2-coding/src/bch/spec.rs:1960`),
whose closing sweep at `:1994-2003` recomputes the run from every present
exponent and rejects a longer one. -/
def bestRun : ℕ := (runStarts T).sup (runLen T)

/-- **L4.7 (maximality).** No cyclic run of consecutive members of `T` is longer than the
witnessed one. Walking backwards from any exponent reaches a run start whose run covers it,
which is why scanning only the run starts loses nothing.

Refinement anchors: `assert_witnessed_run_is_maximal`
(`crates/gf2-coding/src/bch/spec.rs:1994-2003`) and
`the_witnessed_run_is_present_and_maximal_in_the_defining_set` (`:2646`). -/
theorem runLen_le_bestRun (hne : ∃ y : ZMod n, y ∉ T) (s : ZMod n) :
    runLen T s ≤ bestRun T := by
  rcases Nat.eq_zero_or_pos (runLen T s) with h0 | hpos
  · omega
  have hsT : s ∈ T := by simpa using mem_of_lt_runLen T s hpos
  obtain ⟨y, hy⟩ := hne
  have hex : ∃ d : ℕ, s - ((d : ℕ) : ZMod n) - 1 ∉ T := by
    refine ⟨(s - y - 1).val, ?_⟩
    rwa [ZMod.natCast_zmod_val, show s - (s - y - 1) - 1 = y by ring]
  set d := Nat.find hex with hd
  have hback : ∀ k ≤ d, s - ((k : ℕ) : ZMod n) ∈ T := by
    intro k hk
    match k with
    | 0 => simpa using hsT
    | (e + 1) =>
      have he := Nat.find_min hex (show e < d by omega)
      rw [show s - ((e + 1 : ℕ) : ZMod n) = s - ((e : ℕ) : ZMod n) - 1 by push_cast; ring]
      exact not_not.1 he
  have hstart : IsRunStart T (s - ((d : ℕ) : ZMod n)) := by
    refine ⟨hback d le_rfl, ?_⟩
    exact Nat.find_spec hex
  have hcover : ∀ i < d + runLen T s,
      (s - ((d : ℕ) : ZMod n)) + ((i : ℕ) : ZMod n) ∈ T := by
    intro i hi
    rcases lt_or_ge i d with hid | hid
    · rw [show s - ((d : ℕ) : ZMod n) + ((i : ℕ) : ZMod n) = s - (((d - i : ℕ)) : ZMod n) by
        rw [Nat.cast_sub (le_of_lt hid)]; ring]
      exact hback _ (Nat.sub_le _ _)
    · rw [show s - ((d : ℕ) : ZMod n) + ((i : ℕ) : ZMod n) = s + (((i - d : ℕ)) : ZMod n) by
        rw [Nat.cast_sub hid]; ring]
      exact mem_of_lt_runLen T s (by omega)
  have hmin := le_runLen_of_forall T hcover
  have hle := runLen_le T s
  have hsup : runLen T (s - ((d : ℕ) : ZMod n)) ≤ bestRun T :=
    Finset.le_sup ((mem_runStarts T).2 hstart)
  have : min (d + runLen T s) n ≤ bestRun T := le_trans hmin hsup
  omega

/-- Some run start attains the longest run, so the scan's maximum is reached. -/
theorem exists_bestRun_start (hs : (runStarts T).Nonempty) :
    ∃ m : ℕ, IsRunStart T ((m : ℕ) : ZMod n) ∧ runLen T ((m : ℕ) : ZMod n) = bestRun T := by
  obtain ⟨t, ht, hsup⟩ := Finset.exists_mem_eq_sup (runStarts T) hs (runLen T)
  exact ⟨t.val, by rwa [ZMod.natCast_zmod_val, ← mem_runStarts T],
    by rw [ZMod.natCast_zmod_val, bestRun, hsup]⟩

/-- The witnessed start: the least exponent that opens a longest run, which is
the tie-break the scan's strict `run > best_run` comparison implements
(`crates/gf2-coding/src/bch/spec.rs:1563`). -/
def bestStart (hs : (runStarts T).Nonempty) : ℕ := Nat.find (exists_bestRun_start T hs)

/-- The witnessed start opens a run. -/
theorem bestStart_isRunStart (hs : (runStarts T).Nonempty) :
    IsRunStart T ((bestStart T hs : ℕ) : ZMod n) :=
  (Nat.find_spec (exists_bestRun_start T hs)).1

/-- The run from the witnessed start has the witnessed length. -/
theorem runLen_bestStart (hs : (runStarts T).Nonempty) :
    runLen T ((bestStart T hs : ℕ) : ZMod n) = bestRun T :=
  (Nat.find_spec (exists_bestRun_start T hs)).2

/-- **L4.7 (least tie).** No smaller exponent opens a run of the witnessed
length, so the witness is reproducible.

Refinement anchor: `first_root_flavor_witnesses_the_run_it_actually_has`
(`crates/gf2-coding/src/bch/spec.rs:2150`), whose expected first root is the least start of the
longest run rather than the requested one. -/
theorem bestStart_le (hs : (runStarts T).Nonempty) {m : ℕ}
    (hm : IsRunStart T ((m : ℕ) : ZMod n)) (hrun : runLen T ((m : ℕ) : ZMod n) = bestRun T) :
    bestStart T hs ≤ m :=
  Nat.find_le ⟨hm, hrun⟩

/-- **L4.7 (the witness is present).** Every exponent of the witnessed run lies
in the defining set.

Refinement anchor: `assert_witnessed_run_is_maximal` (`crates/gf2-coding/src/bch/spec.rs:1981`),
the `witnessed exponent {exponent} is absent` assertion. -/
theorem mem_of_lt_bestRun (hs : (runStarts T).Nonempty) {i : ℕ} (hi : i < bestRun T) :
    ((bestStart T hs : ℕ) : ZMod n) + (i : ZMod n) ∈ T :=
  mem_of_lt_runLen T _ (by rw [runLen_bestStart T hs]; exact hi)

/-- **L4.7 (the witness is maximal).** A witnessed run shorter than the whole cycle is bounded
on both sides by absent exponents.

Refinement anchor: `assert_witnessed_run_is_maximal`
(`crates/gf2-coding/src/bch/spec.rs:1983-1992`), the two `if run < n` assertions. -/
theorem bestRun_boundaries (hs : (runStarts T).Nonempty) (hlt : bestRun T < n) :
    ((bestStart T hs : ℕ) : ZMod n) - 1 ∉ T ∧
      ((bestStart T hs : ℕ) : ZMod n) + ((bestRun T : ℕ) : ZMod n) ∉ T := by
  refine ⟨(bestStart_isRunStart T hs).2, ?_⟩
  have := notMem_of_runLen_lt T ((bestStart T hs : ℕ) : ZMod n)
    (by rw [runLen_bestStart T hs]; exact hlt)
  rwa [runLen_bestStart T hs] at this

/-- **L4.7 (base case `T = ∅`).** With no member present no exponent opens a run and every run
length is zero, so the scan cannot name a start. This is the first early return of
`witness_longest_run` (`crates/gf2-coding/src/bch/spec.rs:1534-1540`), which reports no
first root, a run of `0` and a bound of `1`.

Refinement anchor: `assert_witnessed_run_is_maximal`
(`crates/gf2-coding/src/bch/spec.rs:1973-1976`), the branch that asserts an empty defining set
against a `None` first root. -/
theorem run_of_empty (h : T = ∅) : runStarts T = ∅ ∧ ∀ s : ZMod n, runLen T s = 0 := by
  subst h
  exact ⟨by simp [runStarts, IsRunStart], fun s => runLen_eq_zero_of_notMem _ (by simp)⟩

/-- **L4.7 (base case `|T| = n`).** With every exponent present the whole cycle is one run, so
the canonical witness is start `0` with run `n`. No exponent opens a run, which is why the
scan cannot reach this case and `witness_longest_run` decides it up front
(`crates/gf2-coding/src/bch/spec.rs:1541-1547`).

Refinement anchor: `assert_witnessed_run_is_maximal`
(`crates/gf2-coding/src/bch/spec.rs:1978-1981`), which finds every witnessed exponent present.
-/
theorem run_of_univ (h : ∀ x : ZMod n, x ∈ T) :
    T.card = n ∧ runStarts T = ∅ ∧ (∀ i < n, (0 : ZMod n) + (i : ZMod n) ∈ T) ∧
      ∀ s : ZMod n, runLen T s ≤ n := by
  refine ⟨?_, ?_, fun i _ => h _, runLen_le T⟩
  · rw [Finset.eq_univ_iff_forall.2 h, Finset.card_univ, ZMod.card]
  · refine Finset.eq_empty_of_forall_notMem fun s hs => ?_
    exact ((mem_runStarts T).1 hs).2 (h _)

/-- **L4.8 (consecutive flavors).** A run of `δ - 1` consecutive seeds inside the defining set
forces the witnessed run to be at least that long, so the reported bound `run + 1`
(`crates/gf2-coding/src/bch/spec.rs:531`) is at least `δ`. The seeds are in the defining set
by O-3's L3.5, because `consecutive_seeds` (`crates/gf2-coding/src/bch/spec.rs:1407`) feeds
them to the closure.

Refinement anchors: `primitive_narrow_sense_agrees_with_the_current_binary_generators`
(`crates/gf2-coding/src/bch/spec.rs:2009`),
`first_root_flavor_witnesses_the_run_it_actually_has` (`:2150`) and
`narrow_sense_is_the_first_root_flavor_at_exponent_one` (`:2175`). -/
theorem le_bestRun_of_consecutive (hne : ∃ y : ZMod n, y ∉ T) (b : ZMod n) {m : ℕ}
    (hm : m ≤ n) (hseeds : ∀ i < m, b + (i : ZMod n) ∈ T) : m ≤ bestRun T := by
  have h1 := le_runLen_of_forall T hseeds
  have h2 := runLen_le_bestRun T hne b
  omega

end Run

end BchGenerator
