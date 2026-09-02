/-
  Gf2Core.Proofs.CyclotomicClosure — obligation O-3 of the algebraic-foundations
  proof sketch (issue d7749931): q-cyclotomic seed closure.

  Binding mode: abstract model plus refinement, with no extraction anchor. The
  whole of `gf2_core::field` is `--opaque` to Charon, so no lemma here is a
  statement about extracted code. The model is a transcription rather than an
  abstraction: the production functions are pure `u64` arithmetic on Z/nZ with
  no field carrier, so the model over `ZMod n` states the same equations the
  code computes, and the only gap between model and code is the `u64`/`usize`
  representation (assumptions-register row A-08).

  The model fixes `n ≥ 1` as `[NeZero n]`, a multiplier `q : ℕ` with
  `Nat.Coprime q n`, and the map `mu q : ZMod n → ZMod n`, `i ↦ q * i`. Orbits
  are the orbits of the cyclic subgroup `⟨q⟩ ≤ (ZMod n)ˣ` acting by
  multiplication, so `MulAction.orbit` supplies the partition machinery.

  Every lemma names its refinement anchor: an executable Rust check that decides
  the same statement on the production path
  `crates/gf2-core/src/field/extension.rs`. The anchors live in that module's
  own test module. `iterative_closure` (`crates/gf2-core/src/field/extension.rs:3917`)
  is an independent fixed-point implementation of the closure that the two
  property tests compare the production result against, so it is a differential
  oracle rather than a law check. One anchor is specific to this obligation:
  `base_order_mod_matches_the_hand_computed_multiplier`
  (`crates/gf2-core/src/field/extension.rs:4062`) pins the multiplier
  `q = p ^ d_B mod n` over two bases of degree `d_B > 1`, where
  `base_order_mod` (`:2281`) computes it without materializing `|B|`.

  Axiom footprint: this module declares no axiom and contains no `sorry`. Every
  declaration rests on `propext`, `Classical.choice` and `Quot.sound` alone, and
  a third of them on fewer. The dependency on `Gf2Core.Proofs.RelativeExtension`
  reaches only its Section 1 model (`RelativeExtension.exists_char_pow`,
  `proofs/Gf2Core/Proofs/RelativeExtension.lean:88`), which is stated over
  Mathlib, so the external-type axiom `Aeneas.Std.core.fmt.Formatter` that the
  extracted carriers of that module carry does not reach any declaration here.

  Every lemma statement is the sketch's. Three steps of the sketch's proof
  strategy are reached by a different Mathlib route:

  * L3.3 — the sketch names
    `MulAction.card_orbit_mul_card_stabilizer_eq_card_group` and `orderOf`; the
    orbit length is instead `Nat.find` on the return-time predicate, mirroring
    `RelativeExtension.orbitPeriod`
    (`proofs/Gf2Core/Proofs/RelativeExtension.lean:379`), because the counting
    pass computes exactly that minimum
    (`crates/gf2-core/src/field/extension.rs:2254-2262`) and the stabiliser
    cardinality is a detour around it.
  * L3.4 — the sketch names `Finset.min'` for the least member of an orbit;
    `orbitMin` is `Nat.find` over the natural representatives instead, which
    states the same minimum directly as a scan index
    (`crates/gf2-core/src/field/extension.rs:2246`).
  * L3.6 — the sketch names `ZMod.natCast_self_eq_zero`; `ZMod.natCast_mod`
    states the needed identity in one step.

  Every `path:line` the sketch cites for this obligation holds at this revision.
-/
import Mathlib.Data.Finset.Sort
import Mathlib.Data.List.Sort
import Mathlib.Data.ZMod.Basic
import Mathlib.GroupTheory.OrderOfElement
import Gf2Core.Proofs.RelativeExtension

set_option maxHeartbeats 1600000

-- The model's standing hypothesis is `n ≥ 1`, carried as `[NeZero n]` on every
-- statement of the section. The unused-section-variable linter would ask each
-- lemma that happens not to consume it to drop it, splitting one model in two.
set_option linter.unusedSectionVars false

noncomputable section

attribute [local instance 0] Classical.propDecidable

namespace CyclotomicClosure

/-! ## Section 1 — the μ_q model over `ZMod n`

`n ≥ 1` is carried as `[NeZero n]`, matching the production rejection of `n = 0`
with `FieldError::InvalidCyclotomicModulus`
(`crates/gf2-core/src/field/extension.rs:2222-2224`). The multiplier is a
natural number `q`, the production's `q_mod_n`; `mu q` is the model's
counterpart of the `modular_mul (current, q_mod_n, n)` step
(`crates/gf2-core/src/field/extension.rs:1686`). -/

variable {n : ℕ} [NeZero n]

/-- `μ_q : ZMod n → ZMod n`, `i ↦ q · i`.

Production path: `modular_mul` (`crates/gf2-core/src/field/extension.rs:1686`)
applied with the partition's stored multiplier, the single arithmetic step of
both orbit passes (`:2258`, `:2270`).

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), whose successor assertion at
`:3905-3906` recomputes `modular_mul` on every stored member. -/
def mu (q : ℕ) (x : ZMod n) : ZMod n := (q : ZMod n) * x

/-- `μ_q` is multiplication by the residue of `q`. -/
@[simp]
theorem mu_apply (q : ℕ) (x : ZMod n) : mu q x = (q : ZMod n) * x := rfl

/-- **L3.6 (multiplier normalization).** Reducing the multiplier modulo `n`
leaves `μ_q` unchanged.

Production path: `cyclotomic_cosets_mod` normalizes its argument once more with
`q_mod_n % n` (`crates/gf2-core/src/field/extension.rs:2226`), and
`cyclotomic_cosets` (`:2129`) feeds it the already reduced `base_order_mod`
result.

Refinement anchor: `cyclotomic_coset_order_is_deterministic`
(`crates/gf2-core/src/field/extension.rs:4041`), whose first assertion compares
the partitions for `q = 17` and `q = 2` modulo 15. -/
theorem mu_mod (q : ℕ) : mu (n := n) (q % n) = mu q := by
  funext x
  simp [mu, ZMod.natCast_mod]

/-! ### L3.1 — `μ_q` is a permutation exactly under coprimality -/

variable {q : ℕ}

/-- `q` as a unit of `ZMod n`, available exactly under `gcd(q, n) = 1`.

Production path: the `gcd_u64` decision at
`crates/gf2-core/src/field/extension.rs:2227-2234`, which rejects a non-coprime
multiplier with `FieldError::NonCoprimeCyclotomicParameters`. -/
def qUnit (hq : Nat.Coprime q n) : (ZMod n)ˣ := ZMod.unitOfCoprime q hq

/-- The unit's value is the residue of `q`. -/
@[simp]
theorem qUnit_val (hq : Nat.Coprime q n) : ((qUnit hq : (ZMod n)ˣ) : ZMod n) = (q : ZMod n) := rfl

/-- The cyclic subgroup `⟨q⟩ ≤ (ZMod n)ˣ` whose multiplication action on
`ZMod n` is `μ_q`. -/
def qSubgroup (hq : Nat.Coprime q n) : Subgroup (ZMod n)ˣ := Subgroup.zpowers (qUnit hq)

/-- **L3.1 (forward).** Coprimality makes `μ_q` bijective, which is what lets a
partition exist at all.

Production path: `cyclotomic_cosets_mod`
(`crates/gf2-core/src/field/extension.rs:2221`) returns a partition only past
the `gcd` check at `:2228`.

Refinement anchor: `cyclotomic_cosets_reject_invalid_moduli_and_non_coprime_parameters`
(`crates/gf2-core/src/field/extension.rs:4018`). -/
theorem mu_bijective (hq : Nat.Coprime q n) : Function.Bijective (mu (n := n) q) := by
  refine Function.bijective_iff_has_inverse.2
    ⟨fun x => (((qUnit hq)⁻¹ : (ZMod n)ˣ) : ZMod n) * x, ?_, ?_⟩
  · intro x
    show (((qUnit hq)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) * x) = x
    rw [← qUnit_val hq, ← mul_assoc, Units.inv_mul, one_mul]
  · intro x
    show (q : ZMod n) * ((((qUnit hq)⁻¹ : (ZMod n)ˣ) : ZMod n) * x) = x
    rw [← qUnit_val hq, ← mul_assoc, Units.mul_inv, one_mul]

/-- **L3.1 (reverse).** A surjective `μ_q` forces coprimality, so rejecting
`gcd ≠ 1` rejects exactly the parameters for which no partition exists.

Production path and anchor as for `mu_bijective`. -/
theorem coprime_of_mu_surjective (hs : Function.Surjective (mu (n := n) q)) :
    Nat.Coprime q n := by
  obtain ⟨x, hx⟩ := hs 1
  have hx' : (q : ZMod n) * x = 1 := hx
  exact (ZMod.isUnit_iff_coprime q n).1 (IsUnit.of_mul_eq_one _ hx')

/-- **L3.1.** `gcd(q, n) = 1` if and only if `μ_q` is a permutation. -/
theorem mu_bijective_iff_coprime :
    Function.Bijective (mu (n := n) q) ↔ Nat.Coprime q n :=
  ⟨fun h => coprime_of_mu_surjective h.2, mu_bijective⟩

/-! ### L3.2 — the orbits partition `ZMod n` -/

/-- The `μ_q`-orbit of `c`: the orbit of `⟨q⟩` acting on `ZMod n` by
multiplication.

Production path: one emitted coset of `cyclotomic_cosets_mod`
(`crates/gf2-core/src/field/extension.rs:2264-2273`).

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), whose disjointness and cover
assertions at `:3903` and `:3910-3913` decide the partition property. -/
def orbit (hq : Nat.Coprime q n) (c : ZMod n) : Set (ZMod n) :=
  MulAction.orbit (qSubgroup hq) c

/-- Orbit membership in the form the production loop advances: `x` is reachable
from `c` by finitely many `modular_mul` steps
(`crates/gf2-core/src/field/extension.rs:2270`). -/
theorem mem_orbit_iff_exists_pow (hq : Nat.Coprime q n) (c x : ZMod n) :
    x ∈ orbit hq c ↔ ∃ t : ℕ, (q : ZMod n) ^ t * c = x := by
  constructor
  · rintro ⟨g, rfl⟩
    obtain ⟨t, ht⟩ :=
      (Submonoid.mem_powers_iff (g : (ZMod n)ˣ) (qUnit hq)).1
        (mem_powers_iff_mem_zpowers.2 g.2)
    refine ⟨t, ?_⟩
    have hval : ((qUnit hq ^ t : (ZMod n)ˣ) : ZMod n) = (q : ZMod n) ^ t := by
      rw [Units.val_pow_eq_pow_val, qUnit_val]
    rw [← hval, ht]
    rfl
  · rintro ⟨t, rfl⟩
    exact ⟨⟨qUnit hq ^ t, Subgroup.npow_mem_zpowers _ t⟩, by
      show ((qUnit hq ^ t : (ZMod n)ˣ) : ZMod n) * c = (q : ZMod n) ^ t * c
      rw [Units.val_pow_eq_pow_val, qUnit_val]⟩

/-- Every residue lies in its own orbit. -/
theorem mem_orbit_self (hq : Nat.Coprime q n) (c : ZMod n) : c ∈ orbit hq c :=
  MulAction.mem_orbit_self c

/-- Two orbits coincide exactly when one contains the other's base point;
`MulAction.orbit_eq_iff` transported to the model's name. -/
theorem orbit_eq_iff (hq : Nat.Coprime q n) {a b : ZMod n} :
    orbit hq a = orbit hq b ↔ a ∈ orbit hq b :=
  MulAction.orbit_eq_iff

/-- An orbit is determined by any of its members, which is why the scan may
enter a coset at whichever member it reaches first. -/
theorem orbit_eq_of_mem (hq : Nat.Coprime q n) {a b : ZMod n} (h : a ∈ orbit hq b) :
    orbit hq a = orbit hq b :=
  (orbit_eq_iff hq).2 h

/-- **L3.2 (disjointness).** Two orbits are equal or disjoint.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the `cosets are not disjoint`
assertion at `:3903`. -/
theorem orbit_eq_or_disjoint (hq : Nat.Coprime q n) (a b : ZMod n) :
    orbit hq a = orbit hq b ∨ Disjoint (orbit hq a) (orbit hq b) := by
  by_cases h : (orbit hq a ∩ orbit hq b).Nonempty
  · obtain ⟨x, hxa, hxb⟩ := h
    exact Or.inl ((orbit_eq_of_mem hq hxa).symm.trans (orbit_eq_of_mem hq hxb))
  · exact Or.inr (Set.disjoint_iff_inter_eq_empty.2 (Set.not_nonempty_iff_eq_empty.1 h))

/-- **L3.2 (cover).** The orbits cover `ZMod n`.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the full-partition assertions
at `:3910-3913`. -/
theorem iUnion_orbit (hq : Nat.Coprime q n) : (⋃ c : ZMod n, orbit hq c) = Set.univ :=
  Set.eq_univ_of_forall fun x => Set.mem_iUnion.2 ⟨x, mem_orbit_self hq x⟩

/-- **L3.2.** Every residue lies in exactly one orbit. -/
theorem existsUnique_orbit (hq : Nat.Coprime q n) (x : ZMod n) :
    ∃! S, S ∈ Set.range (orbit hq) ∧ x ∈ S := by
  refine ⟨orbit hq x, ⟨⟨x, rfl⟩, mem_orbit_self hq x⟩, ?_⟩
  rintro S ⟨⟨c, rfl⟩, hx⟩
  exact (orbit_eq_of_mem hq hx).symm

/-- `μ_q` maps an orbit into itself, the closure half of L3.5. -/
theorem mu_mem_orbit (hq : Nat.Coprime q n) {c x : ZMod n} (hx : x ∈ orbit hq c) :
    mu q x ∈ orbit hq c := by
  obtain ⟨t, ht⟩ := (mem_orbit_iff_exists_pow hq c x).1 hx
  refine (mem_orbit_iff_exists_pow hq c _).2 ⟨t + 1, ?_⟩
  rw [pow_succ', mul_assoc, ht]
  rfl

/-! ### L3.3 — an orbit is a cycle from any member

`cosetLen` is the counting pass at
`crates/gf2-core/src/field/extension.rs:2254-2262`: the least positive number of
`modular_mul` steps that returns to the starting residue. -/

/-- The scan's counting loop terminates: some positive power of `q` fixes `c`.

The witness is `orderOf` of the unit `q`, which is positive because `(ZMod n)ˣ`
is finite. -/
theorem exists_return_time (hq : Nat.Coprime q n) (c : ZMod n) :
    ∃ t : ℕ, 0 < t ∧ (q : ZMod n) ^ t * c = c := by
  refine ⟨orderOf (qUnit hq), orderOf_pos _, ?_⟩
  have h1 : (q : ZMod n) ^ orderOf (qUnit hq) = 1 := by
    rw [← qUnit_val hq, ← Units.val_pow_eq_pow_val, pow_orderOf_eq_one, Units.val_one]
  rw [h1, one_mul]

/-- **L3.3.** `ℓ_c`, the length of the coset entered at `c`.

Production path: `coset_len` of the counting pass
(`crates/gf2-core/src/field/extension.rs:2254-2262`).

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), whose successor assertion at
`:3905-3906` forces the stored coset to close up after exactly its own
length. -/
def cosetLen (hq : Nat.Coprime q n) (c : ZMod n) : ℕ := Nat.find (exists_return_time hq c)

/-- A coset is nonempty: the counting pass runs at least one step. -/
theorem cosetLen_pos (hq : Nat.Coprime q n) (c : ZMod n) : 0 < cosetLen hq c :=
  (Nat.find_spec (exists_return_time hq c)).1

/-- The counting pass returns to its starting residue after `ℓ_c` steps, the
loop exit at `crates/gf2-core/src/field/extension.rs:2259-2261`. -/
theorem pow_cosetLen_mul (hq : Nat.Coprime q n) (c : ZMod n) :
    (q : ZMod n) ^ cosetLen hq c * c = c :=
  (Nat.find_spec (exists_return_time hq c)).2

/-- `ℓ_c` is the least such positive count, so the counting pass exits at the
first return rather than at a later one. -/
theorem cosetLen_le (hq : Nat.Coprime q n) {c : ZMod n} {t : ℕ} (ht : 0 < t)
    (heq : (q : ZMod n) ^ t * c = c) : cosetLen hq c ≤ t :=
  Nat.find_le ⟨ht, heq⟩

/-- Multiplication by `q ^ i` is injective, because `q` is a unit. -/
theorem pow_mul_left_cancel (hq : Nat.Coprime q n) (i : ℕ) {x y : ZMod n}
    (h : (q : ZMod n) ^ i * x = (q : ZMod n) ^ i * y) : x = y := by
  have hu : (((qUnit hq ^ i)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) ^ i) = 1 := by
    rw [← qUnit_val hq, ← Units.val_pow_eq_pow_val, Units.inv_mul]
  calc
    x = (((qUnit hq ^ i)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) ^ i) * x := by rw [hu, one_mul]
    _ = (((qUnit hq ^ i)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) ^ i * x) := by rw [mul_assoc]
    _ = (((qUnit hq ^ i)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) ^ i * y) := by rw [h]
    _ = (((qUnit hq ^ i)⁻¹ : (ZMod n)ˣ) : ZMod n) * ((q : ZMod n) ^ i) * y := by rw [mul_assoc]
    _ = y := by rw [hu, one_mul]

/-- Whole multiples of `ℓ_c` return to `c`. -/
theorem pow_mul_cosetLen (hq : Nat.Coprime q n) (c : ZMod n) (k : ℕ) :
    (q : ZMod n) ^ (cosetLen hq c * k) * c = c := by
  induction k with
  | zero => simp
  | succ k ih =>
    rw [Nat.mul_succ, pow_add, mul_assoc, pow_cosetLen_mul, ih]

/-- The step count matters only modulo `ℓ_c`; this is why the emitting pass
lists each member exactly once. -/
theorem pow_mul_mod (hq : Nat.Coprime q n) (c : ZMod n) (t : ℕ) :
    (q : ZMod n) ^ (t % cosetLen hq c) * c = (q : ZMod n) ^ t * c := by
  conv_rhs => rw [← Nat.div_add_mod t (cosetLen hq c), pow_add, mul_assoc,
    mul_comm ((q : ZMod n) ^ (t % cosetLen hq c)) c, ← mul_assoc, pow_mul_cosetLen]
  rw [mul_comm]

/-- **L3.3 (return).** `q ^ t · c = c` exactly on the multiples of `ℓ_c`. -/
theorem pow_mul_eq_self_iff_dvd (hq : Nat.Coprime q n) (c : ZMod n) (t : ℕ) :
    (q : ZMod n) ^ t * c = c ↔ cosetLen hq c ∣ t := by
  constructor
  · intro ht
    rcases Nat.eq_zero_or_pos (t % cosetLen hq c) with hmod | hmod
    · exact Nat.dvd_of_mod_eq_zero hmod
    · exact absurd (cosetLen_le hq hmod ((pow_mul_mod hq c t).trans ht))
        (Nat.not_le.2 (Nat.mod_lt _ (cosetLen_pos hq c)))
  · rintro ⟨k, rfl⟩
    exact pow_mul_cosetLen hq c k

/-- **L3.3 (distinctness).** The first `ℓ_c` powers of `q` send `c` to `ℓ_c`
distinct residues. -/
theorem pow_mul_injOn (hq : Nat.Coprime q n) (c : ZMod n) :
    Set.InjOn (fun t : ℕ => (q : ZMod n) ^ t * c) (Set.Iio (cosetLen hq c)) := by
  intro i hi j hj hij
  simp only [Set.mem_Iio] at hi hj
  rcases Nat.le_total i j with hle | hle
  · obtain ⟨d, rfl⟩ := Nat.exists_eq_add_of_le hle
    have hd : (q : ZMod n) ^ i * c = (q : ZMod n) ^ i * ((q : ZMod n) ^ d * c) := by
      rw [← mul_assoc, ← pow_add]
      exact hij
    have hdvd := (pow_mul_eq_self_iff_dvd hq c d).1 (pow_mul_left_cancel hq i hd).symm
    have hdlt : d < cosetLen hq c := lt_of_le_of_lt (Nat.le_add_left d i) hj
    have : d = 0 := Nat.eq_zero_of_dvd_of_lt hdvd hdlt
    omega
  · obtain ⟨d, rfl⟩ := Nat.exists_eq_add_of_le hle
    have hd : (q : ZMod n) ^ j * c = (q : ZMod n) ^ j * ((q : ZMod n) ^ d * c) := by
      rw [← mul_assoc, ← pow_add]
      exact hij.symm
    have hdvd := (pow_mul_eq_self_iff_dvd hq c d).1 (pow_mul_left_cancel hq j hd).symm
    have hdlt : d < cosetLen hq c := lt_of_le_of_lt (Nat.le_add_left d j) hi
    have : d = 0 := Nat.eq_zero_of_dvd_of_lt hdvd hdlt
    omega


/-! ### L3.3 — the emitted coset -/

/-- The list the emitting pass pushes
(`crates/gf2-core/src/field/extension.rs:2266-2271`): the cycle from `c`, one
`modular_mul` step per element.

Refinement anchor: `cyclotomic_cosets_match_the_worked_binary_vector_and_pure_form`
(`crates/gf2-core/src/field/extension.rs:3935`), whose expected vector lists
each coset in exactly this order. -/
def cosetList (hq : Nat.Coprime q n) (c : ZMod n) : List (ZMod n) :=
  (List.range (cosetLen hq c)).map fun t => (q : ZMod n) ^ t * c

/-- The emitted coset has the length the counting pass reserved for it
(`crates/gf2-core/src/field/extension.rs:2265`). -/
@[simp]
theorem cosetList_length (hq : Nat.Coprime q n) (c : ZMod n) :
    (cosetList hq c).length = cosetLen hq c := by
  simp [cosetList]

/-- The member at index `i` is `q ^ i · c`. -/
theorem cosetList_getElem (hq : Nat.Coprime q n) (c : ZMod n) (i : ℕ)
    (hi : i < (cosetList hq c).length) :
    (cosetList hq c)[i] = (q : ZMod n) ^ i * c := by
  simp only [cosetList, List.getElem_map, List.getElem_range]

/-- The emitting pass starts at the representative it was entered with
(`crates/gf2-core/src/field/extension.rs:2266`).

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the `coset[0] == representative`
assertion at `:3899`. -/
theorem cosetList_head (hq : Nat.Coprime q n) (c : ZMod n) :
    (cosetList hq c)[0]'(by simpa using cosetLen_pos hq c) = c := by
  rw [cosetList_getElem]
  simp

/-- The emitted coset holds exactly the orbit. -/
theorem mem_cosetList_iff (hq : Nat.Coprime q n) (c x : ZMod n) :
    x ∈ cosetList hq c ↔ x ∈ orbit hq c := by
  simp only [cosetList, List.mem_map, List.mem_range]
  constructor
  · rintro ⟨t, -, rfl⟩
    exact (mem_orbit_iff_exists_pow hq c _).2 ⟨t, rfl⟩
  · intro hx
    obtain ⟨t, ht⟩ := (mem_orbit_iff_exists_pow hq c x).1 hx
    exact ⟨t % cosetLen hq c, Nat.mod_lt _ (cosetLen_pos hq c), (pow_mul_mod hq c t).trans ht⟩

/-- **L3.3 (distinctness).** The emitted coset lists `ℓ_c` distinct residues.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), whose `cosets are not disjoint`
assertion at `:3903` also rejects a repeat inside one coset. -/
theorem cosetList_nodup (hq : Nat.Coprime q n) (c : ZMod n) : (cosetList hq c).Nodup := by
  refine List.Nodup.map_on ?_ (List.nodup_range)
  intro i hi j hj hij
  exact pow_mul_injOn hq c (List.mem_range.1 hi) (List.mem_range.1 hj) hij

/-- One `modular_mul` step advances the exponent and wraps modulo `ℓ_c`. -/
theorem mu_pow_mul_succ (hq : Nat.Coprime q n) (c : ZMod n) (i : ℕ) :
    mu q ((q : ZMod n) ^ i * c) = (q : ZMod n) ^ ((i + 1) % cosetLen hq c) * c := by
  rw [mu_apply, ← mul_assoc, ← pow_succ']
  exact (pow_mul_mod hq c (i + 1)).symm

/-- **L3.3 (cycle).** `μ_q` sends each member of the emitted coset to the next
and the last back to the head.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the assertion at `:3905-3906`
that `modular_mul(member, q, n)` is `coset[(index + 1) % coset.len()]`. -/
theorem mu_cosetList_getElem (hq : Nat.Coprime q n) (c : ZMod n) {i : ℕ}
    (hi : i < cosetLen hq c) :
    mu q ((cosetList hq c)[i]'(by simpa using hi)) =
      (cosetList hq c)[(i + 1) % cosetLen hq c]'(by
        simpa using Nat.mod_lt _ (cosetLen_pos hq c)) := by
  rw [cosetList_getElem hq c i (by simpa using hi),
    cosetList_getElem hq c _ (by simpa using Nat.mod_lt (i + 1) (cosetLen_pos hq c))]
  exact mu_pow_mul_succ hq c i

/-- **L3.3 (the two passes agree).** With `V` the residues visited before the
coset is entered, the emitting pass
(`crates/gf2-core/src/field/extension.rs:2267-2271`) first meets an already
visited residue after exactly `ℓ_c` steps — the count the counting pass
(`:2254-2262`) produced. The emitted vector therefore has the length that was
reserved for it.

Refinement anchor: `cyclotomic_cosets_match_the_worked_binary_vector_and_pure_form`
(`crates/gf2-core/src/field/extension.rs:3935`) together with
`assert_coset_partition_properties` (`:3890`): a disagreement between the passes
is a wrong coset length in the compared vectors. -/
theorem emit_stop_isLeast (hq : Nat.Coprime q n) (c : ZMod n) (V : Set (ZMod n))
    (hV : Disjoint (orbit hq c) V) :
    IsLeast
      {t : ℕ | (q : ZMod n) ^ t * c ∈ V ∨ ∃ s < t, (q : ZMod n) ^ s * c = (q : ZMod n) ^ t * c}
      (cosetLen hq c) := by
  constructor
  · exact Or.inr ⟨0, cosetLen_pos hq c, by rw [pow_zero, one_mul, pow_cosetLen_mul]⟩
  · rintro t (hmemV | ⟨s, hs, hst⟩)
    · exact absurd hmemV
        (Set.disjoint_left.1 hV ((mem_orbit_iff_exists_pow hq c _).2 ⟨t, rfl⟩))
    · by_contra hlt
      have hlt' : t < cosetLen hq c := Nat.not_le.1 hlt
      have hse : s = t :=
        pow_mul_injOn hq c (show s < cosetLen hq c from lt_trans hs hlt')
          (show t < cosetLen hq c from hlt') hst
      omega

/-! ### L3.4 — the scan is deterministic

The scan runs over representatives `0, 1, …, n-1`
(`crates/gf2-core/src/field/extension.rs:2246`) and skips the ones already
marked (`:2247-2249`). The invariant that carries the determinism argument is
that the visited table holds exactly the union of the orbits of the
representatives already considered. -/

/-- The residues the scan has marked once it has considered every representative
below `k` (`crates/gf2-core/src/field/extension.rs:2267-2270`). -/
def visited (hq : Nat.Coprime q n) (k : ℕ) : Set (ZMod n) :=
  {x | ∃ j < k, x ∈ orbit hq ((j : ℕ) : ZMod n)}

/-- The loop invariant, restated: a residue is marked exactly when its own orbit
is the orbit of an earlier representative. -/
theorem mem_visited_iff (hq : Nat.Coprime q n) (k : ℕ) (x : ZMod n) :
    x ∈ visited hq k ↔ ∃ j < k, orbit hq x = orbit hq ((j : ℕ) : ZMod n) := by
  constructor
  · rintro ⟨j, hj, hx⟩
    exact ⟨j, hj, orbit_eq_of_mem hq hx⟩
  · rintro ⟨j, hj, hx⟩
    exact ⟨j, hj, hx ▸ mem_orbit_self hq x⟩

/-- An orbit holds the image of some natural number, so it has a least such
representative. -/
theorem exists_natCast_mem_orbit (hq : Nat.Coprime q n) (x : ZMod n) :
    ∃ m : ℕ, ((m : ℕ) : ZMod n) ∈ orbit hq x :=
  ⟨x.val, by
    show ((x.val : ℕ) : ZMod n) ∈ orbit hq x
    rw [ZMod.natCast_zmod_val]
    exact mem_orbit_self hq x⟩

/-- The least member of an orbit, as a scan index.

Production counterpart: the representative at which `cyclotomic_cosets_mod`
enters the coset (`crates/gf2-core/src/field/extension.rs:2246`). -/
def orbitMin (hq : Nat.Coprime q n) (x : ZMod n) : ℕ :=
  Nat.find (exists_natCast_mem_orbit hq x)

/-- The entry representative lies in the orbit it enters. -/
theorem orbitMin_mem (hq : Nat.Coprime q n) (x : ZMod n) :
    ((orbitMin hq x : ℕ) : ZMod n) ∈ orbit hq x :=
  Nat.find_spec (exists_natCast_mem_orbit hq x)

/-- No member of the orbit carries a smaller residue. -/
theorem orbitMin_le (hq : Nat.Coprime q n) {x y : ZMod n} (hy : y ∈ orbit hq x) :
    orbitMin hq x ≤ y.val := by
  have hp : ((y.val : ℕ) : ZMod n) ∈ orbit hq x := by
    rw [ZMod.natCast_zmod_val]
    exact hy
  exact Nat.find_le hp

/-- The entry representative is a legal scan index. -/
theorem orbitMin_lt (hq : Nat.Coprime q n) (x : ZMod n) : orbitMin hq x < n :=
  lt_of_le_of_lt (orbitMin_le hq (mem_orbit_self hq x)) (ZMod.val_lt x)

/-- The entry representative depends on the orbit alone, which is the content of
"determined by the partition, not by scan order". -/
theorem orbitMin_eq_of_orbit_eq (hq : Nat.Coprime q n) {x y : ZMod n}
    (h : orbit hq x = orbit hq y) : orbitMin hq x = orbitMin hq y := by
  refine le_antisymm ?_ ?_
  · have hmem : ((orbitMin hq y : ℕ) : ZMod n) ∈ orbit hq x := by
      rw [h]; exact orbitMin_mem hq y
    simpa [ZMod.val_cast_of_lt (orbitMin_lt hq y)] using orbitMin_le hq hmem
  · have hmem : ((orbitMin hq x : ℕ) : ZMod n) ∈ orbit hq y := by
      rw [← h]; exact orbitMin_mem hq x
    simpa [ZMod.val_cast_of_lt (orbitMin_lt hq x)] using orbitMin_le hq hmem

/-- **L3.4 (entry point).** The scan enters a coset at representative `j`
exactly when `j` is the least member of its orbit. The outer ordering and the
inner starting point are therefore functions of the partition alone.

Refinement anchor: `cyclotomic_coset_order_is_deterministic`
(`crates/gf2-core/src/field/extension.rs:4041`) and
`cyclotomic_cosets_match_the_worked_binary_vector_and_pure_form` (`:3935`),
whose expected vector fixes both orders. -/
theorem notMem_visited_iff (hq : Nat.Coprime q n) {j : ℕ} (hj : j < n) :
    ((j : ℕ) : ZMod n) ∉ visited hq j ↔ orbitMin hq ((j : ℕ) : ZMod n) = j := by
  rw [mem_visited_iff]
  constructor
  · intro h
    refine le_antisymm ?_ ?_
    · simpa [ZMod.val_cast_of_lt hj] using
        orbitMin_le hq (mem_orbit_self hq ((j : ℕ) : ZMod n))
    · by_contra hlt
      have hlt' : orbitMin hq ((j : ℕ) : ZMod n) < j := Nat.not_le.1 hlt
      exact h ⟨orbitMin hq ((j : ℕ) : ZMod n), hlt',
        (orbit_eq_of_mem hq (orbitMin_mem hq ((j : ℕ) : ZMod n))).symm⟩
  · rintro h ⟨i, hi, heq⟩
    have hi_lt : i < n := lt_trans hi hj
    have hle : orbitMin hq ((j : ℕ) : ZMod n) ≤ i := by
      have hmem : ((i : ℕ) : ZMod n) ∈ orbit hq ((j : ℕ) : ZMod n) := by
        rw [heq]; exact mem_orbit_self hq _
      simpa [ZMod.val_cast_of_lt hi_lt] using orbitMin_le hq hmem
    omega

/-- The representatives at which the scan enters a coset
(`crates/gf2-core/src/field/extension.rs:2246-2249`), in scan order. -/
def scanHeads (hq : Nat.Coprime q n) : List ℕ :=
  (List.range n).filter fun j => decide (orbitMin hq ((j : ℕ) : ZMod n) = j)

/-- The cosets the scan emits, in emission order
(`crates/gf2-core/src/field/extension.rs:2264-2273`). -/
def scanCosets (hq : Nat.Coprime q n) : List (List (ZMod n)) :=
  (scanHeads hq).map fun j => cosetList hq ((j : ℕ) : ZMod n)

/-- The scan enters a coset at exactly the least members of the orbits. -/
theorem mem_scanHeads_iff (hq : Nat.Coprime q n) (j : ℕ) :
    j ∈ scanHeads hq ↔ j < n ∧ orbitMin hq ((j : ℕ) : ZMod n) = j := by
  simp [scanHeads, List.mem_filter, List.mem_range]

/-- **L3.4 (outer order).** The entry representatives strictly increase, so
cosets leave the scan ordered by their least member.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the `pair[0][0] < pair[1][0]`
assertion at `:3893`. -/
theorem scanHeads_sortedLT (hq : Nat.Coprime q n) : (scanHeads hq).SortedLT := by
  rw [List.sortedLT_iff_pairwise]
  exact List.Pairwise.filter _ List.pairwise_lt_range

/-- No representative enters two cosets. -/
theorem scanHeads_nodup (hq : Nat.Coprime q n) : (scanHeads hq).Nodup :=
  (List.sortedLT_iff_pairwise.1 (scanHeads_sortedLT hq)).imp Nat.ne_of_lt

/-- **L3.4 (partition).** Every residue lies in exactly one emitted coset.

Refinement anchor: `assert_coset_partition_properties`
(`crates/gf2-core/src/field/extension.rs:3890`), the disjointness and cover
assertions at `:3903` and `:3910-3913`. -/
theorem existsUnique_mem_scanCosets (hq : Nat.Coprime q n) (x : ZMod n) :
    ∃! l, l ∈ scanCosets hq ∧ x ∈ l := by
  have hxmin : ((orbitMin hq x : ℕ) : ZMod n) ∈ orbit hq x := orbitMin_mem hq x
  have horb : orbit hq ((orbitMin hq x : ℕ) : ZMod n) = orbit hq x := orbit_eq_of_mem hq hxmin
  have hhead : orbitMin hq ((orbitMin hq x : ℕ) : ZMod n) = orbitMin hq x :=
    orbitMin_eq_of_orbit_eq hq horb
  refine ⟨cosetList hq ((orbitMin hq x : ℕ) : ZMod n), ⟨?_, ?_⟩, ?_⟩
  · exact List.mem_map.2
      ⟨orbitMin hq x, (mem_scanHeads_iff hq _).2 ⟨orbitMin_lt hq x, hhead⟩, rfl⟩
  · exact (mem_cosetList_iff hq _ x).2 (by rw [horb]; exact mem_orbit_self hq x)
  · rintro l ⟨hl, hxl⟩
    obtain ⟨j, hj, rfl⟩ := List.mem_map.1 hl
    obtain ⟨hjlt, hjmin⟩ := (mem_scanHeads_iff hq j).1 hj
    have hxj : x ∈ orbit hq ((j : ℕ) : ZMod n) := (mem_cosetList_iff hq _ x).1 hxl
    have h1 : orbitMin hq ((j : ℕ) : ZMod n) = orbitMin hq x :=
      orbitMin_eq_of_orbit_eq hq (orbit_eq_of_mem hq hxj).symm
    have hjeq : j = orbitMin hq x := by rw [← h1, hjmin]
    rw [hjeq]

/-! ### L3.5 and L3.6 — the closure is the least closed superset -/

/-- The seeds reduced into `ZMod n`; production reduces each seed with `seed % n`
(`crates/gf2-core/src/field/extension.rs:2182`). -/
def seedSet (S : Set ℕ) : Set (ZMod n) := (fun s : ℕ => (s : ZMod n)) '' S

/-- `cl_q(S)`, the union of the orbits the seeds meet
(`crates/gf2-core/src/field/extension.rs:2179-2187`). -/
def closure (hq : Nat.Coprime q n) (S : Set ℕ) : Set (ZMod n) :=
  ⋃ s ∈ seedSet (n := n) S, orbit hq s

/-- Closure membership unfolds to lying in the orbit of some seed. -/
theorem mem_closure_iff (hq : Nat.Coprime q n) (S : Set ℕ) (x : ZMod n) :
    x ∈ closure hq S ↔ ∃ s ∈ seedSet (n := n) S, x ∈ orbit hq s := by
  simp [closure]

/-- Iterating `μ_q` from a member of a closed set stays inside it. -/
theorem pow_mul_mem_of_closed {U : Set (ZMod n)}
    (hclosed : ∀ x ∈ U, mu q x ∈ U) {s : ZMod n} (hs : s ∈ U) (t : ℕ) :
    (q : ZMod n) ^ t * s ∈ U := by
  induction t with
  | zero => simpa using hs
  | succ t ih => simpa [mu, pow_succ', mul_assoc] using hclosed _ ih

/-- **L3.5.** The closure is the least `μ_q`-closed superset of the reduced seed
set — the property issue `d7749931` names.

Refinement anchor: `iterative_closure`
(`crates/gf2-core/src/field/extension.rs:3917`), an independent fixed-point
implementation of the same closure, driven by `prop_binary_cyclotomic_closure_laws`
(`:3976`) and `prop_nonbinary_cyclotomic_closure_laws` (`:3997`), together with
`cyclotomic_closure_selects_complete_seed_cosets` (`:3956`). -/
theorem closure_isLeast (hq : Nat.Coprime q n) (S : Set ℕ) :
    IsLeast {U : Set (ZMod n) | seedSet S ⊆ U ∧ ∀ x ∈ U, mu q x ∈ U} (closure hq S) := by
  refine ⟨⟨?_, ?_⟩, ?_⟩
  · intro s hs
    exact (mem_closure_iff hq S s).2 ⟨s, hs, mem_orbit_self hq s⟩
  · intro x hx
    obtain ⟨s, hs, hxs⟩ := (mem_closure_iff hq S x).1 hx
    exact (mem_closure_iff hq S _).2 ⟨s, hs, mu_mem_orbit hq hxs⟩
  · rintro U ⟨hsub, hclosed⟩ x hx
    obtain ⟨s, hs, hxs⟩ := (mem_closure_iff hq S x).1 hx
    obtain ⟨t, rfl⟩ := (mem_orbit_iff_exists_pow hq s x).1 hxs
    exact pow_mul_mem_of_closed hclosed (hsub hs) t

/-- Every member of an orbit is `μ_q` of another member. -/
theorem exists_mu_eq (hq : Nat.Coprime q n) {c x : ZMod n} (hx : x ∈ orbit hq c) :
    ∃ y ∈ orbit hq c, mu q y = x := by
  obtain ⟨t, rfl⟩ := (mem_orbit_iff_exists_pow hq c x).1 hx
  refine ⟨(q : ZMod n) ^ (t + cosetLen hq c - 1) * c,
    (mem_orbit_iff_exists_pow hq c _).2 ⟨_, rfl⟩, ?_⟩
  have hL : 1 ≤ cosetLen hq c := cosetLen_pos hq c
  have hsucc : t + cosetLen hq c - 1 + 1 = t + cosetLen hq c := by omega
  rw [mu_apply, ← mul_assoc, ← pow_succ', hsucc, pow_add, mul_assoc, pow_cosetLen_mul]

/-- **L3.5 (closure).** `μ_q` permutes the closure. -/
theorem mu_image_closure (hq : Nat.Coprime q n) (S : Set ℕ) :
    mu q '' closure hq S = closure hq S := by
  refine Set.Subset.antisymm ?_ ?_
  · rintro _ ⟨x, hx, rfl⟩
    exact (closure_isLeast hq S).1.2 x hx
  · intro x hx
    obtain ⟨s, hs, hxs⟩ := (mem_closure_iff hq S x).1 hx
    obtain ⟨y, hy, hmu⟩ := exists_mu_eq hq hxs
    exact ⟨y, (mem_closure_iff hq S y).2 ⟨s, hs, hy⟩, hmu⟩

/-- **L3.6 (seed normalization).** Reducing seeds modulo `n` changes nothing, so
duplicate seeds and seeds outside `[0, n)` have no effect
(`crates/gf2-core/src/field/extension.rs:2182`).

Refinement anchor: `prop_binary_cyclotomic_closure_laws`
(`crates/gf2-core/src/field/extension.rs:3976`), which draws seeds from `0..64`
against moduli of at most 31, and
`cyclotomic_closure_selects_complete_seed_cosets` (`:3956`), whose seed `20`
lies outside `[0, 15)`. -/
theorem seedSet_mod (S : Set ℕ) : seedSet (n := n) ((· % n) '' S) = seedSet (n := n) S := by
  ext x
  simp only [seedSet, Set.mem_image]
  constructor
  · rintro ⟨_, ⟨s, hs, rfl⟩, rfl⟩
    exact ⟨s, hs, by rw [ZMod.natCast_mod]⟩
  · rintro ⟨s, hs, rfl⟩
    exact ⟨s % n, ⟨s, hs, rfl⟩, by rw [ZMod.natCast_mod]⟩

/-- **L3.6.** Closing the reduced seeds gives the same set, on the anchors
`seedSet_mod` names. -/
theorem closure_mod (hq : Nat.Coprime q n) (S : Set ℕ) :
    closure hq ((· % n) '' S) = closure hq S := by
  rw [closure, closure, seedSet_mod]

/-- **L3.6 (idempotence).** Re-closing the defining set reproduces the closure;
the `again` comparison of `prop_binary_cyclotomic_closure_laws`
(`crates/gf2-core/src/field/extension.rs:3985-3989`). -/
theorem closure_val_image (hq : Nat.Coprime q n) (S : Set ℕ) :
    closure hq (ZMod.val '' closure hq S) = closure hq S := by
  have hseed : seedSet (n := n) (ZMod.val '' closure hq S) = closure hq S := by
    ext x
    simp only [seedSet, Set.mem_image]
    constructor
    · rintro ⟨_, ⟨y, hy, rfl⟩, rfl⟩
      rwa [ZMod.natCast_zmod_val]
    · intro hx
      exact ⟨x.val, ⟨x, hx, rfl⟩, ZMod.natCast_zmod_val x⟩
  rw [closure, hseed]
  ext x
  simp only [Set.mem_iUnion, exists_prop]
  constructor
  · rintro ⟨s, hs, hxs⟩
    obtain ⟨s', hs', hss'⟩ := (mem_closure_iff hq S s).1 hs
    exact (mem_closure_iff hq S x).2 ⟨s', hs', (orbit_eq_of_mem hq hss') ▸ hxs⟩
  · intro hx
    exact ⟨x, hx, mem_orbit_self hq x⟩

/-! ### L3.7 — the multiplier from the extension -/

/-- **L3.7.** `|B| mod n = p ^ d_B mod n`, and modular exponentiation reaches it
without forming `|B|`: `Nat.pow_mod` is the identity the repeated-squaring loop
of `modular_pow_usize` (`crates/gf2-core/src/field/extension.rs:1712`) runs on,
and `RelativeExtension.exists_char_pow`
(`proofs/Gf2Core/Proofs/RelativeExtension.lean:88`) supplies `|B| = p ^ d_B`.

Production path: `base_order_mod`
(`crates/gf2-core/src/field/extension.rs:2281`).

Refinement anchor: `base_order_mod_matches_the_hand_computed_multiplier`
(`crates/gf2-core/src/field/extension.rs:4062`), which pins the derived
multiplier against hand-computed `p ^ d_B mod n` over a `GF(9)` base
(`d_B = 2`) and a `GF(2 ^ 64)` base (`d_B = 64`, where `q` exceeds
`u64::MAX`). -/
theorem exists_baseCard_mod (B : Type*) {E : Type*} [Field B] [Field E] [Algebra B E]
    [Finite E] :
    ∃ p dB : ℕ, Nat.Prime p ∧ Nat.card B = p ^ dB ∧
      Nat.card B % n = (p % n) ^ dB % n := by
  obtain ⟨p, dB, hp, _, hcard⟩ := RelativeExtension.exists_char_pow B (E := E)
  exact ⟨p, dB, hp, hcard, by rw [hcard, Nat.pow_mod]⟩

/-- **L3.7 (model multiplier).** The multiplier the model runs on is the image of
`|B|` in `ZMod n`, whichever representative the production computes. -/
theorem natCast_eq_pow {N p dB : ℕ} (hcard : N = p ^ dB) :
    ((N : ℕ) : ZMod n) = (p : ZMod n) ^ dB := by
  rw [hcard, Nat.cast_pow]

/-- The reduced multiplier is coprime to `n` exactly when `|B|` is, so the
production `gcd` check on `q_mod_n`
(`crates/gf2-core/src/field/extension.rs:2226-2234`) decides the model's
hypothesis. -/
theorem coprime_mod_iff (m : ℕ) : Nat.Coprime (m % n) n ↔ Nat.Coprime m n :=
  ZMod.coprime_mod_iff_coprime m n

/-! ### L3.8 — the partition's derived views -/

/-- The cosets of the closure: the emitted cosets a seed meets
(`crates/gf2-core/src/field/extension.rs:2179-2187`). -/
def closureCosets (hq : Nat.Coprime q n) (S : Set ℕ) : List (List (ZMod n)) :=
  (scanCosets hq).filter fun l => decide (∃ x ∈ l, x ∈ seedSet (n := n) S)

/-- A coset is selected exactly when a seed lands in it
(`crates/gf2-core/src/field/extension.rs:2180-2182`). -/
theorem mem_closureCosets_iff (hq : Nat.Coprime q n) (S : Set ℕ) (l : List (ZMod n)) :
    l ∈ closureCosets hq S ↔ l ∈ scanCosets hq ∧ ∃ x ∈ l, x ∈ seedSet (n := n) S := by
  simp [closureCosets, List.mem_filter]

/-- **L3.8 (`contains`).** A residue belongs to the closure exactly when one of
the selected cosets holds it.

Refinement anchor: `cyclotomic_closure_selects_complete_seed_cosets`
(`crates/gf2-core/src/field/extension.rs:3956`). -/
theorem mem_closure_iff_mem_closureCosets (hq : Nat.Coprime q n) (S : Set ℕ) (x : ZMod n) :
    x ∈ closure hq S ↔ ∃ l ∈ closureCosets hq S, x ∈ l := by
  constructor
  · intro hx
    obtain ⟨s, hs, hxs⟩ := (mem_closure_iff hq S x).1 hx
    obtain ⟨l, ⟨hl, hxl⟩, -⟩ := existsUnique_mem_scanCosets hq x
    obtain ⟨j, hj, rfl⟩ := List.mem_map.1 hl
    have hxj : x ∈ orbit hq ((j : ℕ) : ZMod n) := (mem_cosetList_iff hq _ x).1 hxl
    have hsj : s ∈ orbit hq ((j : ℕ) : ZMod n) := by
      rw [← orbit_eq_of_mem hq hxj, orbit_eq_of_mem hq hxs]
      exact mem_orbit_self hq s
    exact ⟨_, (mem_closureCosets_iff hq S _).2 ⟨hl, s, (mem_cosetList_iff hq _ s).2 hsj, hs⟩, hxl⟩
  · rintro ⟨l, hl, hxl⟩
    obtain ⟨hlscan, s, hsl, hs⟩ := (mem_closureCosets_iff hq S l).1 hl
    obtain ⟨j, hj, rfl⟩ := List.mem_map.1 hlscan
    have hxj : x ∈ orbit hq ((j : ℕ) : ZMod n) := (mem_cosetList_iff hq _ x).1 hxl
    have hsj : s ∈ orbit hq ((j : ℕ) : ZMod n) := (mem_cosetList_iff hq _ s).1 hsl
    refine (mem_closure_iff hq S x).2 ⟨s, hs, ?_⟩
    rw [orbit_eq_of_mem hq hsj]
    exact hxj

/-- **L3.8 (`coset_of`).** A member of the closure lies in exactly one selected
coset, so `coset_of` returns a unique index. -/
theorem existsUnique_mem_closureCosets (hq : Nat.Coprime q n) (S : Set ℕ) {x : ZMod n}
    (hx : x ∈ closure hq S) : ∃! l, l ∈ closureCosets hq S ∧ x ∈ l := by
  obtain ⟨l, ⟨hl, hxl⟩, huniq⟩ := existsUnique_mem_scanCosets hq x
  obtain ⟨l', hl', hxl'⟩ := (mem_closure_iff_mem_closureCosets hq S x).1 hx
  refine ⟨l', ⟨hl', hxl'⟩, ?_⟩
  rintro m ⟨hm, hxm⟩
  rw [huniq m ⟨((mem_closureCosets_iff hq S m).1 hm).1, hxm⟩,
    huniq l' ⟨((mem_closureCosets_iff hq S l').1 hl').1, hxl'⟩]

/-- **L3.8 (`coset_of` returns `None`).** A residue outside the closure lies in
no selected coset. -/
theorem notMem_closureCosets (hq : Nat.Coprime q n) (S : Set ℕ) {x : ZMod n}
    (hx : x ∉ closure hq S) : ∀ l ∈ closureCosets hq S, x ∉ l := fun l hl hxl =>
  hx ((mem_closure_iff_mem_closureCosets hq S x).2 ⟨l, hl, hxl⟩)

/-- The exponents of the closure, as the `u64` residues the production returns
(`crates/gf2-core/src/field/extension.rs:2036-2044`). -/
def definingFinset (hq : Nat.Coprime q n) (S : Set ℕ) : Finset ℕ :=
  (Finset.range n).filter fun m => ((m : ℕ) : ZMod n) ∈ closure hq S

/-- `CosetPartition::defining_set`
(`crates/gf2-core/src/field/extension.rs:2036`): the sorted union of the
selected cosets. -/
def definingList (hq : Nat.Coprime q n) (S : Set ℕ) : List ℕ :=
  (definingFinset hq S).sort

/-- The defining set holds exactly the closure's residues below `n`. -/
theorem mem_definingFinset_iff (hq : Nat.Coprime q n) (S : Set ℕ) (m : ℕ) :
    m ∈ definingFinset hq S ↔ m < n ∧ ((m : ℕ) : ZMod n) ∈ closure hq S := by
  simp [definingFinset, Finset.mem_filter, Finset.mem_range]

/-- **L3.8 (`defining_set` is sorted).** The returned exponents strictly
increase, so the union is sorted and duplicate-free.

Refinement anchor: `cyclotomic_closure_selects_complete_seed_cosets`
(`crates/gf2-core/src/field/extension.rs:3956`), whose expected defining set is
`[3, 5, 6, 9, 10, 12]`, and `assert_coset_partition_properties` (`:3890`), whose
disjointness assertion at `:3903` rules out a repeat. -/
theorem definingList_sortedLT (hq : Nat.Coprime q n) (S : Set ℕ) :
    (definingList hq S).SortedLT :=
  Finset.sortedLT_sort _

/-- The defining set carries no repeat. -/
theorem definingList_nodup (hq : Nat.Coprime q n) (S : Set ℕ) :
    (definingList hq S).Nodup :=
  Finset.sort_nodup _ _

/-- Membership in the sorted defining set is closure membership. -/
theorem mem_definingList_iff (hq : Nat.Coprime q n) (S : Set ℕ) (m : ℕ) :
    m ∈ definingList hq S ↔ m < n ∧ ((m : ℕ) : ZMod n) ∈ closure hq S := by
  rw [definingList, Finset.mem_sort, mem_definingFinset_iff]

/-- **L3.8 (`contains` reduces).** `contains` reduces its argument modulo `n`, so
`n + e` and `e` decide alike
(`crates/gf2-core/src/field/extension.rs:2066-2067`).

Refinement anchor: `cyclotomic_closure_selects_complete_seed_cosets`
(`crates/gf2-core/src/field/extension.rs:3956`), whose `coset_of(18)` query
reduces to exponent `3`. -/
theorem contains_iff (hq : Nat.Coprime q n) (S : Set ℕ) (e : ℕ) :
    ((e : ℕ) : ZMod n) ∈ closure hq S ↔ (e % n) ∈ definingFinset hq S := by
  rw [mem_definingFinset_iff, ZMod.natCast_mod]
  exact ⟨fun h => ⟨Nat.mod_lt _ (Nat.pos_of_ne_zero (NeZero.ne n)), h⟩, And.right⟩

end CyclotomicClosure
