/-
  Gf2Core.Proofs.BchGenerator — obligation O-4 (work in progress header).
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

@[simp]
theorem alphaPow_zero (α : E) : alphaPow α (0 : ZMod n) = 1 := by
  simp [alphaPow]

theorem alphaPow_add {α : E} (hord : orderOf α = n) (i j : ZMod n) :
    alphaPow α (i + j) = alphaPow α i * alphaPow α j := by
  have h : alphaPow α i * alphaPow α j = α ^ (i.val + j.val) := by
    rw [pow_add]; rfl
  rw [h, ← alphaPow_natCast hord (i.val + j.val), Nat.cast_add,
    ZMod.natCast_zmod_val, ZMod.natCast_zmod_val]

/-- **L4.4 (the bridge lemma).** The relative Frobenius acts on powers of `α` as
`μ_q` acts on their exponents. -/
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

theorem mem_orbitFinset {x y : E} :
    y ∈ orbitFinset B x ↔ ∃ i < orbitPeriod B x, (relFrobenius B : E → E)^[i] x = y := by
  simp [orbitFinset]

theorem card_orbitFinset (x : E) : (orbitFinset B x).card = orbitPeriod B x := by
  rw [orbitFinset, Finset.card_image_of_injOn, Finset.card_range]
  intro i hi j hj hij
  exact orbit_injOn B x (Finset.mem_range.1 hi) (Finset.mem_range.1 hj) hij

theorem self_mem_orbitFinset (x : E) : x ∈ orbitFinset B x :=
  (mem_orbitFinset B).2 ⟨0, orbitPeriod_pos B x, rfl⟩

theorem relFrobenius_mem_orbitFinset {x y : E} (hy : y ∈ orbitFinset B x) :
    relFrobenius B y ∈ orbitFinset B x := by
  obtain ⟨i, hi, rfl⟩ := (mem_orbitFinset B).1 hy
  refine (mem_orbitFinset B).2
    ⟨(i + 1) % orbitPeriod B x, Nat.mod_lt _ (orbitPeriod_pos B x), ?_⟩
  rw [(iterate_eq_iterate_iff_modEq B x ((i + 1) % orbitPeriod B x) (i + 1)).2
      (Nat.mod_modEq _ _), Function.iterate_succ_apply']

theorem image_relFrobenius_orbitFinset (x : E) :
    (orbitFinset B x).image (relFrobenius B) = orbitFinset B x := by
  refine Finset.eq_of_subset_of_card_le (fun y hy => ?_) ?_
  · obtain ⟨z, hz, rfl⟩ := Finset.mem_image.1 hy
    exact relFrobenius_mem_orbitFinset B hz
  · rw [Finset.card_image_of_injective _ (relFrobenius_injective B)]

/-- `m_x` as computed in `E`: the product over the conjugate orbit. -/
def conjProd (x : E) : Polynomial E := ∏ y ∈ orbitFinset B x, (X - C y)

theorem conjProd_monic (x : E) : (conjProd B x).Monic :=
  monic_prod_of_monic _ _ fun y _ => monic_X_sub_C y

theorem natDegree_conjProd (x : E) : (conjProd B x).natDegree = orbitPeriod B x := by
  rw [conjProd, natDegree_prod_of_monic _ _ fun y _ => monic_X_sub_C y]
  simp [card_orbitFinset]

theorem eval_conjProd_self (x : E) : (conjProd B x).eval x = 0 := by
  rw [conjProd, eval_prod]
  exact Finset.prod_eq_zero (self_mem_orbitFinset B x) (by simp)

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

/-- **L4.1 (b).** Every coefficient of `m_x` is `φ_B`-fixed, hence in `ι(B)`. -/
theorem conjProd_coeff_mem_range (x : E) (k : ℕ) :
    (conjProd B x).coeff k ∈ Set.range (algebraMap B E) := by
  rw [← fixedPoints_eq_range B (E := E)]
  show relFrobenius B ((conjProd B x).coeff k) = (conjProd B x).coeff k
  conv_rhs => rw [← map_relFrobenius_conjProd B x]
  rw [coeff_map]

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

theorem map_minpoly_ne_zero (x : E) : (minpoly B x).map (algebraMap B E) ≠ 0 := by
  refine fun h => ?_
  have hmonic : (minpoly B x).Monic := minpoly.monic (IsIntegral.of_finite B x)
  exact (hmonic.map (algebraMap B E)).ne_zero h

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

/-- **L4.1 (d).** The conjugate product is the image of `minpoly B x`, so
`minpoly B x` is the model's `m_x` and L4.1 (a) and (c) read off it. -/
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

/-- **L4.1 (a).** `m_x` is monic of degree `ℓ`. -/
theorem minpoly_monic_natDegree (x : E) :
    (minpoly B x).Monic ∧ (minpoly B x).natDegree = orbitPeriod B x := by
  refine ⟨minpoly.monic (IsIntegral.of_finite B x), ?_⟩
  rw [← natDegree_conjProd B x, conjProd_eq_map_minpoly B x,
    (minpoly.monic (IsIntegral.of_finite B x)).natDegree_map]

/-- **L4.1 (c).** `m_x` vanishes at `x`. -/
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

theorem prod_X_sub_C_monic (S : Finset E) : (∏ a ∈ S, (X - C a)).Monic :=
  monic_prod_of_monic _ _ fun a _ => monic_X_sub_C a

theorem natDegree_prod_X_sub_C (S : Finset E) : (∏ a ∈ S, (X - C a)).natDegree = S.card := by
  rw [natDegree_prod_of_monic _ _ fun a _ => monic_X_sub_C a]
  simp

/-! ## Section 4 — L4.2 and L4.3: the generator over the base field -/

variable {α : E}

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

theorem mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) (c x : ZMod n) :
    x ∈ cosetFinset hq c ↔ ∃ t : ℕ, (q : ZMod n) ^ t * c = x := by
  rw [cosetFinset, List.mem_toFinset, CyclotomicClosure.mem_cosetList_iff,
    CyclotomicClosure.mem_orbit_iff_exists_pow]

theorem self_mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) (c : ZMod n) :
    c ∈ cosetFinset hq c :=
  (mem_cosetFinset hq c c).2 ⟨0, by simp⟩

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
    exact ⟨_, (mem_cosetFinset hq c _).2 ⟨i, rfl⟩, (iterate_relFrobenius_alphaPow B hord c i).symm⟩
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

theorem mem_reps {q : ℕ} (hq : Nat.Coprime q n) (T : Finset (ZMod n)) (c : ZMod n) :
    c ∈ reps hq T ↔ c ∈ T ∧ ((CyclotomicClosure.orbitMin hq c : ℕ) : ZMod n) = c := by
  simp [reps]

theorem mem_of_mem_cosetFinset {q : ℕ} (hq : Nat.Coprime q n) {T : Finset (ZMod n)}
    (hclosed : ∀ j ∈ T, CyclotomicClosure.mu q j ∈ T) {c x : ZMod n} (hc : c ∈ T)
    (hx : x ∈ cosetFinset hq c) : x ∈ T := by
  obtain ⟨t, rfl⟩ := (mem_cosetFinset hq c x).1 hx
  exact CyclotomicClosure.pow_mul_mem_of_closed (U := (↑T : Set (ZMod n)))
    (fun y hy => hclosed y hy) hc t

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

/-- **L4.2.** The generator: the product of the minimal polynomials of the
coset representatives, a polynomial over the base field by construction. -/
def generator (hq : Nat.Coprime (Nat.card B) n) (α : E) (T : Finset (ZMod n)) : Polynomial B :=
  ∏ c ∈ reps hq T, minpoly B (alphaPow α c)

/-- **L4.2 (monic).** -/
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
exponent `j` of the defining set. -/
theorem aeval_generator_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    {j : ZMod n} (hj : j ∈ T) :
    Polynomial.aeval (alphaPow α j) (generator B hq α T) = 0 := by
  rw [Polynomial.aeval_def, ← Polynomial.eval_map, map_generator B hord hq hclosed, eval_prod]
  exact Finset.prod_eq_zero hj (by simp)

/-! ## Section 5 — L4.2 (least common multiple), L4.5 and L4.6 -/

theorem map_generator_ne_zero (hq : Nat.Coprime (Nat.card B) n) (α : E) (T : Finset (ZMod n)) :
    (generator B hq α T).map (algebraMap B E) ≠ 0 :=
  ((generator_monic B hq α T).map (algebraMap B E)).ne_zero

theorem eval_map_generator_alphaPow (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    {j : ZMod n} (hj : j ∈ T) :
    ((generator B hq α T).map (algebraMap B E)).eval (alphaPow α j) = 0 := by
  rw [Polynomial.eval_map, ← Polynomial.aeval_def]
  exact aeval_generator_alphaPow B hord hq hclosed hj

/-- **L4.2 (lower bound).** Every coset minimal polynomial divides the
generator, so the generator is a common multiple of the family. -/
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

/-- **L4.2 (upper bound).** The generator divides every common multiple of the
coset minimal polynomials, so it is their least common multiple. -/
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

/-- **L4.5 (divisibility).** The generator divides `X ^ n - 1` over the base
field, which is the invariant the construction re-checks at run time. -/
theorem generator_dvd_X_pow_sub_one (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    generator B hq α T ∣ (X ^ n - 1 : Polynomial B) := by
  have hmapc : ((X ^ n - 1 : Polynomial B)).map (algebraMap B E) = (X ^ n - 1 : Polynomial E) := by
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

/-- **L4.6 (degree).** The generator has one root per exponent of the defining
set, so its degree is `|T|`. -/
theorem natDegree_generator (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (generator B hq α T).natDegree = T.card := by
  rw [← (generator_monic B hq α T).natDegree_map (algebraMap B E),
    map_generator B hord hq hclosed,
    natDegree_prod_of_monic _ _ fun j _ => monic_X_sub_C (alphaPow α j)]
  simp

theorem card_le_length (T : Finset (ZMod n)) : T.card ≤ n := by
  calc T.card ≤ Fintype.card (ZMod n) := Finset.card_le_univ T
    _ = n := ZMod.card n

/-- **L4.6 (dimension).** `k = n - |T|` is a genuine subtraction, so the
`checked_sub` that forms the dimension is total and `0 ≤ k ≤ n`. -/
theorem dimension_add_natDegree (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (n - (generator B hq α T).natDegree) + (generator B hq α T).natDegree = n := by
  rw [natDegree_generator B hord hq hclosed]
  exact Nat.sub_add_cancel (card_le_length T)

end BchGenerator
