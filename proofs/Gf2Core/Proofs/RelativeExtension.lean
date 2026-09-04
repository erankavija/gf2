/-
  Gf2Core.Proofs.RelativeExtension — obligation O-1 of the algebraic-foundations
  proof sketch (issue 41090b8d): extension embedding, restriction, and
  relative-Frobenius laws.

  Binding mode: abstract model plus refinement, with an extracted-carrier
  instance. Section 1 states the model over Mathlib: fields B and E with
  `[Field B] [Field E] [Finite E] [Algebra B E]`, the embedding
  `ι = algebraMap B E`, the relative degree `r = Module.finrank B E`, the base
  cardinality `q = Nat.card B`, and the relative Frobenius `φ_B x = x ^ q`.
  Section 2 instantiates it at the extracted `QuadraticExt` and `CubicExt`
  carriers, whose `Field` instances come from `Gf2Core.Proofs.QuadraticExtField`
  and `Gf2Core.Proofs.CubicExtField`.

  Every lemma names its refinement anchor: an executable Rust check that decides
  the same statement on the production path
  `crates/gf2-core/src/field/extension.rs`. The anchors live in the shared
  conformance harness `crates/gf2-core/src/field/axiom_tests.rs` and in the
  production module's own test module. No new Rust test is required for this
  obligation.

  Axiom footprint: this module declares no axiom and contains no `sorry`. The
  Section 1 model rests on Lean's `propext`, `Classical.choice`, and `Quot.sound`
  alone. The Section 2 instantiations additionally carry
  `Aeneas.Std.core.fmt.Formatter`, the opaque external type axiom that
  `Gf2Core/TypesExternal.lean` regenerates each extraction run and that the
  `QuadraticExtField` and `CubicExtField` instances they build on already carry.
-/
import Mathlib.Algebra.CharP.Lemmas
import Mathlib.Algebra.Polynomial.Roots
import Mathlib.FieldTheory.Finite.Basic
import Mathlib.FieldTheory.Finiteness
import Mathlib.LinearAlgebra.Dimension.Constructions
import Gf2Core.Proofs.ExtAlgebra
import Gf2Core.Proofs.QuadraticExtField
import Gf2Core.Proofs.CubicExtField

set_option maxHeartbeats 1600000

noncomputable section

attribute [local instance] Classical.propDecidable

namespace RelativeExtension

/-! ## Section 1 — the abstract relative-extension model

`B ⊆ E` is carried as `[Algebra B E]` with `E` finite. The base field is then
finite too, so `q = Nat.card B` and `r = Module.finrank B E` are the model's
counterparts of `FieldExtension::base_order`
(`crates/gf2-core/src/field/extension.rs:2597`) and
`FieldExtension::relative_degree` (`:2592`). -/

/-- The base field of a finite extension is finite, because `ι` embeds it in `E`.

This is the model's counterpart of the certificate invariant that a
`FieldExtension` names two finite fields
(`ExtensionCertificate`, `crates/gf2-core/src/field/extension.rs:1418`). -/
theorem finite_base (B E : Type*) [Field B] [Field E] [Algebra B E] [Finite E] :
    Finite B :=
  Finite.of_injective (algebraMap B E) (algebraMap B E).injective

/-- `q = |B|` is nonzero. -/
theorem baseCard_ne_zero (B E : Type*) [Field B] [Field E] [Algebra B E] [Finite E] :
    Nat.card B ≠ 0 := by
  haveI : Finite B := finite_base B E
  exact Nat.card_ne_zero.mpr ⟨inferInstance, inferInstance⟩

/-- `q = |B| > 1`, since a field is nontrivial. -/
theorem one_lt_baseCard (B E : Type*) [Field B] [Field E] [Algebra B E] [Finite E] :
    1 < Nat.card B := by
  haveI : Finite B := finite_base B E
  exact Finite.one_lt_card

variable (B : Type*) {E : Type*} [Field B] [Field E] [Algebra B E] [Finite E]

/-- The characteristic `p` is prime, is shared by `B` and `E`, and `|B| = p ^ d_B`
for the absolute base degree `d_B`.

Model counterpart of `FieldExtension::characteristic`
(`crates/gf2-core/src/field/extension.rs:2575`) and
`FieldExtension::base_degree` (`:2580`), which read both numbers off the shared
`FieldId` (`:621`).

Refinement anchor: `check_extension_structure`
(`crates/gf2-core/src/field/axiom_tests.rs:1498`), which asserts that base and
extension report the same characteristic. -/
theorem exists_char_pow :
    ∃ p dB : ℕ, Nat.Prime p ∧ CharP E p ∧ Nat.card B = p ^ dB := by
  haveI : Finite B := finite_base B E
  haveI : Fintype B := Fintype.ofFinite B
  haveI : CharP B (ringChar B) := ringChar.charP B
  obtain ⟨n, hp, hcard⟩ := FiniteField.card B (ringChar B)
  refine ⟨ringChar B, n, hp, ?_, ?_⟩
  · exact charP_of_injective_algebraMap (algebraMap B E).injective (ringChar B)
  · rw [Nat.card_eq_fintype_card]; exact hcard

/-! ### L1.1 — the embedding is an injective ring homomorphism -/

omit [Finite E] in
/-- **L1.1 (embedding).** `ι = algebraMap B E` sends `0` to `0` and `1` to `1`,
commutes with addition and multiplication, and is injective.

Production path: `FieldExtension::embed`
(`crates/gf2-core/src/field/extension.rs:2558`).

Refinement anchors: `check_embedding_homomorphism`
(`crates/gf2-core/src/field/axiom_tests.rs:1549`) and
`check_embedding_injective` (`crates/gf2-core/src/field/axiom_tests.rs:1573`). -/
theorem embedding_laws :
    algebraMap B E 0 = 0 ∧ algebraMap B E 1 = 1 ∧
      (∀ a b : B, algebraMap B E (a + b) = algebraMap B E a + algebraMap B E b) ∧
      (∀ a b : B, algebraMap B E (a * b) = algebraMap B E a * algebraMap B E b) ∧
      Function.Injective (algebraMap B E) :=
  ⟨map_zero _, map_one _, fun a b => map_add _ a b, fun a b => map_mul _ a b,
    (algebraMap B E).injective⟩

/-! ### L1.2 — orders and degrees -/

/-- **L1.2 (orders).** `|E| = q ^ r`.

Production path: `FieldExtension::ext_order`
(`crates/gf2-core/src/field/extension.rs:2602`), which reads `|E|` off the
extension `FieldId`.

Refinement anchor: `check_extension_structure`
(`crates/gf2-core/src/field/axiom_tests.rs:1498`), whose `|E| = |B| ^ r`
assertion sits at `crates/gf2-core/src/field/axiom_tests.rs:1512-1530`. -/
theorem card_ext_eq_baseCard_pow_finrank :
    Nat.card E = Nat.card B ^ Module.finrank B E := by
  haveI : Finite B := finite_base B E
  haveI : Fintype B := Fintype.ofFinite B
  haveI : Fintype E := Fintype.ofFinite E
  rw [Nat.card_eq_fintype_card, Nat.card_eq_fintype_card]
  exact Module.card_eq_pow_finrank

omit [Finite E] in
/-- **L1.2 (unit group).** `|E*| = |E| - 1`.

Production path: `FieldExtension::ext_unit_group_order`
(`crates/gf2-core/src/field/extension.rs:2607`).

Refinement anchor: `check_extension_structure`
(`crates/gf2-core/src/field/axiom_tests.rs:1498`). -/
theorem card_units_ext : Nat.card Eˣ = Nat.card E - 1 :=
  Nat.card_units E

/-- The relative degree is positive; the production certificate carries the same
fact as exact divisibility of `d_E` by `d_B`
(`FieldExtension::relative_degree`,
`crates/gf2-core/src/field/extension.rs:2592`). -/
theorem finrank_pos : 0 < Module.finrank B E := by
  rcases Nat.eq_zero_or_pos (Module.finrank B E) with h | h
  · exfalso
    have hcard := card_ext_eq_baseCard_pow_finrank B (E := E)
    rw [h, pow_zero] at hcard
    have : 1 < Nat.card E := Finite.one_lt_card
    omega
  · exact h

/-- **L1.2 (degrees).** `[E : F_p] = d_B · r`, stated representation-free as
`|E| = p ^ (d_B · r)`.

Production path: `FieldExtension::ext_degree`
(`crates/gf2-core/src/field/extension.rs:2585`), whose value the certificate
ties to `base_degree () * relative_degree ()`.

Refinement anchor: `check_extension_structure`
(`crates/gf2-core/src/field/axiom_tests.rs:1498`), degree-product assertions at
`crates/gf2-core/src/field/axiom_tests.rs:1512-1530`. -/
theorem card_ext_eq_char_pow {p dB : ℕ} (hcard : Nat.card B = p ^ dB) :
    Nat.card E = p ^ (dB * Module.finrank B E) := by
  rw [card_ext_eq_baseCard_pow_finrank B (E := E), hcard, ← pow_mul]

/-! ### L1.3 — the relative Frobenius is a ring endomorphism -/

/-- The relative Frobenius is additive, because `q` is a power of the
characteristic. -/
theorem relFrobenius_add' (x y : E) :
    (x + y) ^ Nat.card B = x ^ Nat.card B + y ^ Nat.card B := by
  obtain ⟨p, dB, hp, hchar, hcard⟩ := exists_char_pow B (E := E)
  haveI := hchar
  haveI : Fact p.Prime := ⟨hp⟩
  rw [hcard, add_pow_char_pow]

/-- **L1.3 (relative Frobenius).** `φ_B : E → E`, `x ↦ x ^ q`, bundled as a ring
endomorphism of `E`.

Production path: `FieldExtension::relative_frobenius`
(`crates/gf2-core/src/field/extension.rs:2637`) at `k = 1`.

Refinement anchor: `check_relative_frobenius`
(`crates/gf2-core/src/field/axiom_tests.rs:1643`). -/
def relFrobenius : E →+* E where
  toFun x := x ^ Nat.card B
  map_one' := one_pow _
  map_mul' x y := mul_pow x y _
  map_zero' := zero_pow (baseCard_ne_zero B E)
  map_add' := relFrobenius_add' B

@[simp]
theorem relFrobenius_apply (x : E) : relFrobenius B x = x ^ Nat.card B := rfl

/-- **L1.3 (base is fixed).** `φ_B` fixes the image of `ι` pointwise.

Refinement anchor: `check_relative_frobenius`
(`crates/gf2-core/src/field/axiom_tests.rs:1643`). -/
theorem relFrobenius_algebraMap (a : B) :
    relFrobenius B (algebraMap B E a) = algebraMap B E a := by
  haveI : Finite B := finite_base B E
  haveI : Fintype B := Fintype.ofFinite B
  rw [relFrobenius_apply, ← map_pow, Nat.card_eq_fintype_card, FiniteField.pow_card]

/-! ### L1.4 and L1.7 — iteration count and period -/

/-- Iterating `y ↦ y ^ p` exactly `n` times computes `x ↦ x ^ p ^ n`. This is the
shape of the production loop body. -/
theorem iterate_pow_apply {M : Type*} [Monoid M] (p n : ℕ) (x : M) :
    (fun y : M => y ^ p)^[n] x = x ^ p ^ n := by
  induction n generalizing x with
  | zero => simp
  | succ m ih => rw [Function.iterate_succ_apply, ih, ← pow_mul, pow_succ']

/-- `φ_B^[k] x = x ^ q ^ k`. -/
theorem relFrobenius_iterate_apply (k : ℕ) (x : E) :
    (relFrobenius B : E → E)^[k] x = x ^ Nat.card B ^ k := by
  induction k generalizing x with
  | zero => simp
  | succ m ih =>
      rw [Function.iterate_succ_apply, ih, relFrobenius_apply, ← pow_mul, pow_succ']

/-- `φ_B^[r] = id`, the first half of L1.4. -/
theorem relFrobenius_iterate_finrank (x : E) :
    (relFrobenius B : E → E)^[Module.finrank B E] x = x := by
  haveI : Fintype E := Fintype.ofFinite E
  rw [relFrobenius_iterate_apply, ← card_ext_eq_baseCard_pow_finrank B (E := E),
    Nat.card_eq_fintype_card]
  exact FiniteField.pow_card x

/-- Any multiple of `r` iterations acts as the identity. -/
theorem relFrobenius_iterate_mul_finrank (j : ℕ) (x : E) :
    (relFrobenius B : E → E)^[Module.finrank B E * j] x = x := by
  induction j with
  | zero => simp
  | succ m ih =>
      rw [Nat.mul_succ, Function.iterate_add_apply, relFrobenius_iterate_finrank, ih]

/-- **L1.4 (period).** `φ_B^[k] = φ_B^[k mod r]`, so reducing `k` modulo the
relative degree before iterating is sound.

Production path: `FieldExtension::relative_frobenius`
(`crates/gf2-core/src/field/extension.rs:2637`), which reduces `k` modulo
`relative_degree ()` first.

Refinement anchor: `check_relative_frobenius`
(`crates/gf2-core/src/field/axiom_tests.rs:1643`). -/
theorem relFrobenius_iterate_mod (k : ℕ) (x : E) :
    (relFrobenius B : E → E)^[k] x =
      (relFrobenius B : E → E)^[k % Module.finrank B E] x := by
  conv_lhs => rw [← Nat.mod_add_div k (Module.finrank B E)]
  rw [Function.iterate_add_apply, relFrobenius_iterate_mul_finrank]

/-- **L1.7 (iteration count).** The production loop — `d_B · (k mod r)`
applications of `pow (characteristic)` — computes `φ_B^[k]`, without ever
forming `q ^ k`.

Production path: `FieldExtension::relative_frobenius`
(`crates/gf2-core/src/field/extension.rs:2637`), whose body sets
`steps = base_degree () * (k % relative_degree ())` and then raises to the
characteristic `steps` times.

Refinement anchors: `test_extension_laws_gf9_in_gf81`
(`crates/gf2-core/src/field/axiom_tests.rs:2042`) and
`tower_relative_frobenius_takes_two_absolute_steps`
(`crates/gf2-core/src/field/extension.rs:4353`), the two cases with `d_B > 1`. -/
theorem production_loop_eq_relFrobenius {p dB : ℕ} (hcard : Nat.card B = p ^ dB)
    (k : ℕ) (x : E) :
    (fun y : E => y ^ p)^[dB * (k % Module.finrank B E)] x =
      (relFrobenius B : E → E)^[k] x := by
  rw [iterate_pow_apply, relFrobenius_iterate_mod B k x, relFrobenius_iterate_apply,
    hcard, ← pow_mul]

/-! ### L1.5 and L1.6 — the fixed field and the restriction round trip -/

/-- **L1.5 (fixed field).** The `φ_B`-fixed points of `E` are exactly the image
of `ι`.

This is the load-bearing statement: `FieldExtension::contains`
(`crates/gf2-core/src/field/extension.rs:2652`) decides base membership by this
test alone.

Refinement anchor: `check_relative_frobenius`
(`crates/gf2-core/src/field/axiom_tests.rs:1679`), the `phi_x == x` versus
`contains (x)` assertion. -/
theorem fixedPoints_eq_range :
    {x : E | relFrobenius B x = x} = Set.range (algebraMap B E) := by
  haveI : Finite B := finite_base B E
  haveI : Fintype B := Fintype.ofFinite B
  haveI : Fintype E := Fintype.ofFinite E
  have hq1 : 1 < Nat.card B := one_lt_baseCard B E
  set q := Nat.card B with hq
  set P : Polynomial E := Polynomial.X ^ q - Polynomial.X with hP
  have hP0 : P ≠ 0 := FiniteField.X_pow_card_sub_X_ne_zero E hq1
  have hdeg : P.natDegree = q := FiniteField.X_pow_card_sub_X_natDegree_eq E hq1
  have hmem : ∀ x : E, x ∈ P.roots.toFinset ↔ x ^ q = x := by
    intro x
    rw [Multiset.mem_toFinset, Polynomial.mem_roots hP0]
    simp [hP, Polynomial.IsRoot, sub_eq_zero]
  have hScard : P.roots.toFinset.card ≤ q := by
    refine le_trans (Multiset.toFinset_card_le _) ?_
    exact le_trans (Polynomial.card_roots' P) (le_of_eq hdeg)
  have hTcard : (Finset.image (algebraMap B E) Finset.univ).card = q := by
    rw [Finset.card_image_of_injective _ (algebraMap B E).injective, Finset.card_univ, hq,
      Nat.card_eq_fintype_card]
  have hTS : Finset.image (algebraMap B E) Finset.univ ⊆ P.roots.toFinset := by
    intro x hx
    rw [Finset.mem_image] at hx
    obtain ⟨a, -, rfl⟩ := hx
    rw [hmem]
    exact congrArg (fun z : E => z) (relFrobenius_algebraMap B a)
  have hTS' : Finset.image (algebraMap B E) Finset.univ = P.roots.toFinset :=
    Finset.eq_of_subset_of_card_le hTS (by omega)
  ext x
  constructor
  · intro hx
    have hx' : x ∈ P.roots.toFinset := (hmem x).mpr hx
    rw [← hTS', Finset.mem_image] at hx'
    obtain ⟨a, -, rfl⟩ := hx'
    exact ⟨a, rfl⟩
  · rintro ⟨a, rfl⟩
    exact relFrobenius_algebraMap B a

/-- **L1.6 (membership matches restriction).** `contains (x)` holds exactly when
`try_restrict (x)` answers.

Production paths: `FieldExtension::contains`
(`crates/gf2-core/src/field/extension.rs:2652`) and
`FieldExtension::try_restrict` (`:2562`).

Refinement anchor: `check_membership_matches_restriction`
(`crates/gf2-core/src/field/axiom_tests.rs:1610`). -/
theorem relFrobenius_fixed_iff_mem_range (x : E) :
    relFrobenius B x = x ↔ ∃ a : B, algebraMap B E a = x := by
  have := fixedPoints_eq_range B (E := E)
  constructor
  · intro hx
    have : x ∈ Set.range (algebraMap B E) := this ▸ hx
    exact this
  · rintro ⟨a, rfl⟩
    exact relFrobenius_algebraMap B a

/-- **L1.6 (restriction round trip).** A `φ_B`-fixed element has exactly one
preimage under `ι`, so `try_restrict` is a genuine partial inverse of `embed`.

Production paths: `FieldExtension::embed`
(`crates/gf2-core/src/field/extension.rs:2558`),
`FieldExtension::try_restrict` (`:2562`), and `FieldExtension::restrict`
(`:2661`).

Refinement anchor: `check_restriction_round_trip`
(`crates/gf2-core/src/field/axiom_tests.rs:1588`). -/
theorem existsUnique_restrict {x : E} (hx : relFrobenius B x = x) :
    ∃! a : B, algebraMap B E a = x := by
  obtain ⟨a, ha⟩ := (relFrobenius_fixed_iff_mem_range B x).mp hx
  exact ⟨a, ha, fun b hb => (algebraMap B E).injective (hb.trans ha.symm)⟩

/-! ### L1.8 — the conjugate orbit -/

/-- Every element returns to itself after `r` applications of `φ_B`, so the set
of positive return times is nonempty. -/
theorem exists_return_time (x : E) :
    ∃ i : ℕ, 0 < i ∧ (relFrobenius B : E → E)^[i] x = x :=
  ⟨Module.finrank B E, finrank_pos B (E := E), relFrobenius_iterate_finrank B x⟩

/-- The orbit period `ℓ` of `x`: the least positive `i` with `φ_B^[i] x = x`.

Production counterpart: the length of the vector `conjugates`
(`crates/gf2-core/src/field/extension.rs:2329`) returns. -/
def orbitPeriod (x : E) : ℕ := Nat.find (exists_return_time B x)

theorem orbitPeriod_pos (x : E) : 0 < orbitPeriod B x :=
  (Nat.find_spec (exists_return_time B x)).1

theorem relFrobenius_iterate_orbitPeriod (x : E) :
    (relFrobenius B : E → E)^[orbitPeriod B x] x = x :=
  (Nat.find_spec (exists_return_time B x)).2

/-- Iterating a multiple of the orbit period fixes `x`. -/
theorem relFrobenius_iterate_mul_orbitPeriod (x : E) (j : ℕ) :
    (relFrobenius B : E → E)^[orbitPeriod B x * j] x = x := by
  induction j with
  | zero => simp
  | succ m ih =>
      rw [Nat.mul_succ, Function.iterate_add_apply, relFrobenius_iterate_orbitPeriod, ih]

/-- `φ_B^[i] x = x` exactly when the orbit period divides `i`. -/
theorem iterate_eq_self_iff_dvd (x : E) (i : ℕ) :
    (relFrobenius B : E → E)^[i] x = x ↔ orbitPeriod B x ∣ i := by
  constructor
  · intro hi
    by_contra hdvd
    have hpos : 0 < i % orbitPeriod B x :=
      Nat.pos_of_ne_zero fun h => hdvd (Nat.dvd_of_mod_eq_zero h)
    have hlt : i % orbitPeriod B x < orbitPeriod B x :=
      Nat.mod_lt _ (orbitPeriod_pos B x)
    have hfix : (relFrobenius B : E → E)^[i % orbitPeriod B x] x = x := by
      have hsplit : (relFrobenius B : E → E)^[i] x =
          (relFrobenius B : E → E)^[i % orbitPeriod B x] x := by
        conv_lhs => rw [← Nat.mod_add_div i (orbitPeriod B x)]
        rw [Function.iterate_add_apply, relFrobenius_iterate_mul_orbitPeriod]
      exact hsplit ▸ hi
    exact absurd ⟨hpos, hfix⟩ (Nat.find_min (exists_return_time B x) hlt)
  · rintro ⟨j, rfl⟩
    exact relFrobenius_iterate_mul_orbitPeriod B x j

/-- **L1.8 (period divides the relative degree).** `ℓ ∣ r`.

Refinement anchors: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4053`) and
`const_ext_relative_frobenius_agrees_with_the_conjugate`
(`crates/gf2-core/src/field/extension.rs:4340`). -/
theorem orbitPeriod_dvd_finrank (x : E) : orbitPeriod B x ∣ Module.finrank B E :=
  (iterate_eq_self_iff_dvd B x _).mp (relFrobenius_iterate_finrank B x)

/-- `ℓ ≤ r`, so the `0..=r` loop in `conjugates` always finds its repeat. -/
theorem orbitPeriod_le_finrank (x : E) : orbitPeriod B x ≤ Module.finrank B E :=
  Nat.le_of_dvd (finrank_pos B (E := E)) (orbitPeriod_dvd_finrank B x)

/-- `φ_B` is injective, because `φ_B^[r]` is the identity and `r > 0`. -/
theorem relFrobenius_injective : Function.Injective (relFrobenius B : E → E) := by
  intro a b hab
  obtain ⟨m, hm⟩ := Nat.exists_eq_add_of_lt (finrank_pos B (E := E))
  have key : ∀ z : E, (relFrobenius B : E → E)^[m] (relFrobenius B z) = z := by
    intro z
    have := relFrobenius_iterate_finrank B z
    rwa [hm, Nat.zero_add, Function.iterate_succ_apply] at this
  rw [← key a, ← key b, hab]

theorem relFrobenius_iterate_injective (i : ℕ) :
    Function.Injective ((relFrobenius B : E → E)^[i]) :=
  Function.Injective.iterate (relFrobenius_injective B) i

/-- **L1.8 (orbit is a cycle).** `φ_B^[i] x = φ_B^[j] x` exactly when
`i ≡ j (mod ℓ)`. -/
theorem iterate_eq_iterate_iff_modEq (x : E) (i j : ℕ) :
    (relFrobenius B : E → E)^[i] x = (relFrobenius B : E → E)^[j] x ↔
      i ≡ j [MOD orbitPeriod B x] := by
  have key : ∀ m n : ℕ, m ≤ n →
      ((relFrobenius B : E → E)^[m] x = (relFrobenius B : E → E)^[n] x ↔
        orbitPeriod B x ∣ n - m) := by
    intro m n hmn
    have hsplit : (relFrobenius B : E → E)^[n] x =
        (relFrobenius B : E → E)^[m] ((relFrobenius B : E → E)^[n - m] x) := by
      rw [← Function.iterate_add_apply]
      congr 1
      omega
    rw [hsplit]
    constructor
    · intro h
      exact (iterate_eq_self_iff_dvd B x (n - m)).mp
        (relFrobenius_iterate_injective B m h.symm)
    · intro h
      rw [(iterate_eq_self_iff_dvd B x (n - m)).mpr h]
  rcases le_total i j with hij | hij
  · rw [key i j hij, Nat.modEq_iff_dvd' hij]
  · rw [eq_comm, key j i hij, ← Nat.modEq_iff_dvd' hij]
    exact ⟨Nat.ModEq.symm, Nat.ModEq.symm⟩

/-- **L1.8 (distinct orbit members).** The prefix
`x, φ_B x, …, φ_B^[ℓ-1] x` has `ℓ` distinct members. -/
theorem orbit_injOn (x : E) :
    Set.InjOn (fun i : ℕ => (relFrobenius B : E → E)^[i] x) (Set.Iio (orbitPeriod B x)) := by
  intro i hi j hj hij
  have hmod : i ≡ j [MOD orbitPeriod B x] :=
    (iterate_eq_iterate_iff_modEq B x i j).mp hij
  have hi' : i % orbitPeriod B x = i := Nat.mod_eq_of_lt hi
  have hj' : j % orbitPeriod B x = j := Nat.mod_eq_of_lt hj
  rw [Nat.ModEq, hi', hj'] at hmod
  exact hmod

/-- **L1.8 (the `conjugates` loop is safe).** Three facts discharge both
assertions of `conjugates` (`crates/gf2-core/src/field/extension.rs:2329`) as
unreachable: no repeat occurs before index `ℓ`; the value at index `ℓ` equals the
value at index `0`, so the reported first-repeat position is `0`
(`crates/gf2-core/src/field/extension.rs:2336`); and `ℓ ≤ r`, so the
`0..=r` loop reaches index `ℓ` and never falls through to the panic
(`crates/gf2-core/src/field/extension.rs:2346`).

Refinement anchors: `assert_minimal_polynomial_properties`
(`crates/gf2-core/src/field/extension.rs:4053`) and
`const_ext_relative_frobenius_agrees_with_the_conjugate`
(`crates/gf2-core/src/field/extension.rs:4340`). -/
theorem conjugates_loop_is_safe (x : E) :
    (∀ i < orbitPeriod B x, ∀ j < i,
        (relFrobenius B : E → E)^[j] x ≠ (relFrobenius B : E → E)^[i] x) ∧
      (relFrobenius B : E → E)^[orbitPeriod B x] x = (relFrobenius B : E → E)^[0] x ∧
      orbitPeriod B x ≤ Module.finrank B E := by
  refine ⟨fun i hi j hji hEq => ?_, ?_, orbitPeriod_le_finrank B x⟩
  · have : j = i := orbit_injOn B x (lt_trans hji hi) hi hEq
    omega
  · simpa using relFrobenius_iterate_orbitPeriod B x

/-! ### L1.9 — trace and norm land in the base field -/

/-- `Tr_{E/B} (x) = Σ_{i<r} φ_B^[i] x`, the model of `relative_trace`
(`crates/gf2-core/src/field/extension.rs:2439`). -/
def relTrace (x : E) : E :=
  ∑ i ∈ Finset.range (Module.finrank B E), (relFrobenius B : E → E)^[i] x

/-- `N_{E/B} (x) = Π_{i<r} φ_B^[i] x`, the model of `relative_norm`
(`crates/gf2-core/src/field/extension.rs:2486`). -/
def relNorm (x : E) : E :=
  ∏ i ∈ Finset.range (Module.finrank B E), (relFrobenius B : E → E)^[i] x

theorem relFrobenius_relTrace (x : E) :
    relFrobenius B (relTrace B x) = relTrace B x := by
  have hshift : ∀ i : ℕ, relFrobenius B ((relFrobenius B : E → E)^[i] x) =
      (relFrobenius B : E → E)^[i + 1] x := fun i =>
    (Function.iterate_succ_apply' (relFrobenius B : E → E) i x).symm
  have hends : (relFrobenius B : E → E)^[Module.finrank B E] x =
      (relFrobenius B : E → E)^[0] x := by
    simpa using relFrobenius_iterate_finrank B x
  rw [relTrace, map_sum]
  simp only [hshift]
  have h1 := Finset.sum_range_succ' (fun i => (relFrobenius B : E → E)^[i] x)
    (Module.finrank B E)
  have h2 := Finset.sum_range_succ (fun i => (relFrobenius B : E → E)^[i] x)
    (Module.finrank B E)
  rw [h1] at h2
  rw [hends] at h2
  exact add_right_cancel h2

theorem relFrobenius_relNorm (x : E) :
    relFrobenius B (relNorm B x) = relNorm B x := by
  have hshift : ∀ i : ℕ, relFrobenius B ((relFrobenius B : E → E)^[i] x) =
      (relFrobenius B : E → E)^[i + 1] x := fun i =>
    (Function.iterate_succ_apply' (relFrobenius B : E → E) i x).symm
  rw [relNorm, map_prod]
  simp only [hshift]
  rcases eq_or_ne x 0 with rfl | hx
  · have hzero : ∀ i : ℕ, (relFrobenius B : E → E)^[i] (0 : E) = 0 := fun i =>
      Function.iterate_fixed (map_zero (relFrobenius B)) i
    simp only [hzero]
  · have hends : (relFrobenius B : E → E)^[Module.finrank B E] x =
        (relFrobenius B : E → E)^[0] x := by
      simpa using relFrobenius_iterate_finrank B x
    have h1 := Finset.prod_range_succ' (fun i => (relFrobenius B : E → E)^[i] x)
      (Module.finrank B E)
    have h2 := Finset.prod_range_succ (fun i => (relFrobenius B : E → E)^[i] x)
      (Module.finrank B E)
    rw [h1, hends] at h2
    simp only [Function.iterate_zero_apply] at h2
    exact mul_right_cancel₀ hx h2

/-- **L1.9 (trace lands in `B`).** `Tr_{E/B} (x)` is `φ_B`-fixed, hence in the
image of `ι`. This discharges the `restrict_invariant` panic
(`crates/gf2-core/src/field/extension.rs:2510`) for `relative_trace`.

Refinement anchor: `assert_trace_norm_laws`
(`crates/gf2-core/src/field/extension.rs:4082`). -/
theorem relTrace_mem_range (x : E) : ∃ a : B, algebraMap B E a = relTrace B x :=
  (relFrobenius_fixed_iff_mem_range B _).mp (relFrobenius_relTrace B x)

/-- **L1.9 (norm lands in `B`).** `N_{E/B} (x)` is `φ_B`-fixed, hence in the
image of `ι`. This discharges the `restrict_invariant` panic
(`crates/gf2-core/src/field/extension.rs:2510`) for `relative_norm`.

Refinement anchor: `assert_trace_norm_laws`
(`crates/gf2-core/src/field/extension.rs:4082`). -/
theorem relNorm_mem_range (x : E) : ∃ a : B, algebraMap B E a = relNorm B x :=
  (relFrobenius_fixed_iff_mem_range B _).mp (relFrobenius_relNorm B x)

/-! ### L1.10 — the trivial extension -/

/-- **L1.10 (trivial extension).** `r = 1` makes `ι` bijective.

Production path: `TrivialExt` (`crates/gf2-core/src/field/extension.rs:3124`).

Refinement anchors: `test_extension_laws_trivial_fp7`
(`crates/gf2-core/src/field/axiom_tests.rs:2124`) and
`trivial_ext_is_the_identity_on_every_operation`
(`crates/gf2-core/src/field/extension.rs:4365`). -/
theorem algebraMap_bijective_of_finrank_eq_one (h : Module.finrank B E = 1) :
    Function.Bijective (algebraMap B E) := by
  refine (Nat.bijective_iff_injective_and_card _).mpr ⟨(algebraMap B E).injective, ?_⟩
  rw [card_ext_eq_baseCard_pow_finrank B (E := E), h, pow_one]

/-- **L1.10 (trivial extension).** `r = 1` makes `φ_B` the identity on `E`.

Refinement anchors: `test_extension_laws_trivial_fp7`
(`crates/gf2-core/src/field/axiom_tests.rs:2124`) and
`trivial_ext_is_the_identity_on_every_operation`
(`crates/gf2-core/src/field/extension.rs:4365`). -/
theorem relFrobenius_eq_id_of_finrank_eq_one (h : Module.finrank B E = 1) (x : E) :
    relFrobenius B x = x := by
  have := relFrobenius_iterate_finrank B x
  rwa [h, Function.iterate_one] at this

/-- **L1.10 at the identity extension.** `B ⊆ B` has `r = 1`, so `ι` is bijective
and `φ_B` is the identity. This is the model of `TrivialExt`
(`crates/gf2-core/src/field/extension.rs:3124`), whose `embed` (`:3159`) and
`try_restrict` (`:3163`) are the identity and `Some`.

Refinement anchors: `test_extension_laws_trivial_fp7`
(`crates/gf2-core/src/field/axiom_tests.rs:2124`) and
`trivial_ext_is_the_identity_on_every_operation`
(`crates/gf2-core/src/field/extension.rs:4365`). -/
theorem trivial_self (F : Type*) [Field F] [Finite F] :
    Function.Bijective (algebraMap F F) ∧ ∀ x : F, relFrobenius F x = x := by
  have h : Module.finrank F F = 1 := Module.finrank_self F
  exact ⟨algebraMap_bijective_of_finrank_eq_one F h,
    relFrobenius_eq_id_of_finrank_eq_one F h⟩

end RelativeExtension

/-! ## Section 2 — the extracted-carrier instantiation

The model of Section 1 is instantiated at the two carriers the Charon/Aeneas
pipeline extracts transparently, `gfpn::quadratic::QuadraticExt` and
`gfpn::cubic::CubicExt`. Their `Field` instances are the transferred ones from
`Gf2Core.Proofs.QuadraticExtField` and `Gf2Core.Proofs.CubicExtField`; the
`Algebra` structure below is the model of `FieldExtension::embed`
(`crates/gf2-core/src/field/extension.rs:2558`) at those carriers, sending a
base element `a` to the coefficient vector `(a, 0)` resp. `(a, 0, 0)`.

`ValidExtConfig` (`Gf2Core/Proofs/ExtDefs.lean:65`) carries no finiteness, so
these instantiations take `[Finite BF]` as an added hypothesis, exactly as the
sketch's binding section states.

L1.1 through L1.4 instantiate at both carriers below. L1.10 governs `r = 1`,
which `quad_finrank` and `cubic_finrank` refute for these two carriers; its
instantiation is `RelativeExtension.trivial_self`, the identity extension that
models `TrivialExt` (`crates/gf2-core/src/field/extension.rs:3124`). -/

namespace ExtractedExtension

open Aeneas Aeneas.Std Result gf2_core

variable {C BF Char Wide : Type} [Field BF]
variable {inst : gfpn.ext_config.ExtConfig C BF Char Wide}

/-! ### The extracted quadratic carrier -/

omit [Field BF] in
/-- Componentwise extensionality for the extracted quadratic carrier. -/
theorem quadExt_ext {a b : gfpn.quadratic.QuadraticExt C BF Char Wide}
    (h0 : a.c0 = b.c0) (h1 : a.c1 = b.c1) : a = b := by
  cases a; cases b; simp_all

/-- The extracted quadratic carrier is its pair of coefficients. -/
def quadEquivProd : gfpn.quadratic.QuadraticExt C BF Char Wide ≃ BF × BF where
  toFun a := (a.c0, a.c1)
  invFun p := ⟨p.1, p.2⟩
  left_inv a := by cases a; rfl
  right_inv p := by cases p; rfl

instance instFiniteQuadraticExt [Finite BF] :
    Finite (gfpn.quadratic.QuadraticExt C BF Char Wide) :=
  Finite.of_equiv _ (quadEquivProd (C := C) (BF := BF) (Char := Char) (Wide := Wide)).symm

omit [Field BF] in
/-- `|E| = q ^ 2` for the extracted quadratic carrier. This is the Lean
counterpart of `order_eq_base_squared`
(`Gf2Core/Proofs/QuadraticExtField.lean:89`), which pins the same cardinality on
the extracted `order` implementation. -/
theorem quad_card [Finite BF] :
    Nat.card (gfpn.quadratic.QuadraticExt C BF Char Wide) = Nat.card BF ^ 2 := by
  rw [Nat.card_congr (quadEquivProd (C := C) (BF := BF) (Char := Char) (Wide := Wide)),
    Nat.card_prod, sq]

/-- The model of `FieldExtension::embed`
(`crates/gf2-core/src/field/extension.rs:2558`) at the extracted quadratic
carrier: `a ↦ (a, 0)`. -/
@[reducible]
noncomputable def quadAlgebra (hv : ValidExtConfig inst) :
    letI := QuadraticExtField.instField hv
    Algebra BF (gfpn.quadratic.QuadraticExt C BF Char Wide) :=
  letI := QuadraticExtField.instField hv
  RingHom.toAlgebra
    { toFun := fun a => ⟨a, 0⟩
      map_one' := rfl
      map_zero' := rfl
      map_mul' := by
        intro a b
        refine quadExt_ext ?_ ?_
        · show a * b = a * b + hv.getNonResidue * (0 * 0); ring
        · show (0 : BF) = a * 0 + 0 * b; ring
      map_add' := by
        intro a b
        refine quadExt_ext rfl ?_
        show (0 : BF) = 0 + 0; ring }

/-- **L1.1 at the extracted quadratic carrier.** -/
theorem quad_embedding_laws (hv : ValidExtConfig inst) :
    letI := QuadraticExtField.instField hv
    letI := quadAlgebra hv
    algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) 0 = 0 ∧
      algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) 1 = 1 ∧
      (∀ a b : BF, algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) (a + b) =
        algebraMap BF _ a + algebraMap BF _ b) ∧
      (∀ a b : BF, algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) (a * b) =
        algebraMap BF _ a * algebraMap BF _ b) ∧
      Function.Injective (algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide)) :=
  letI := QuadraticExtField.instField hv
  letI := quadAlgebra hv
  RelativeExtension.embedding_laws BF

/-- **L1.2 at the extracted quadratic carrier.** The relative degree is two. -/
theorem quad_finrank (hv : ValidExtConfig inst) [Finite BF] :
    letI := QuadraticExtField.instField hv
    letI := quadAlgebra hv
    Module.finrank BF (gfpn.quadratic.QuadraticExt C BF Char Wide) = 2 := by
  letI := QuadraticExtField.instField hv
  letI := quadAlgebra hv
  have hcard := RelativeExtension.card_ext_eq_baseCard_pow_finrank BF
    (E := gfpn.quadratic.QuadraticExt C BF Char Wide)
  rw [quad_card] at hcard
  have hq : 2 ≤ Nat.card BF :=
    RelativeExtension.one_lt_baseCard BF (gfpn.quadratic.QuadraticExt C BF Char Wide)
  exact (Nat.pow_right_injective hq hcard).symm

/-- **L1.3 and L1.4 at the extracted quadratic carrier.** The relative Frobenius
`x ↦ x ^ q` is a ring endomorphism, it fixes the embedded base pointwise, and its
square is the identity. -/
theorem quad_relFrobenius_laws (hv : ValidExtConfig inst) [Finite BF] :
    letI := QuadraticExtField.instField hv
    letI := quadAlgebra hv
    (∀ x y : gfpn.quadratic.QuadraticExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (x + y) =
          RelativeExtension.relFrobenius BF x + RelativeExtension.relFrobenius BF y) ∧
      (∀ x y : gfpn.quadratic.QuadraticExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (x * y) =
          RelativeExtension.relFrobenius BF x * RelativeExtension.relFrobenius BF y) ∧
      (∀ a : BF, RelativeExtension.relFrobenius BF
          (algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) a) =
        algebraMap BF (gfpn.quadratic.QuadraticExt C BF Char Wide) a) ∧
      (∀ x : gfpn.quadratic.QuadraticExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (RelativeExtension.relFrobenius BF x) = x) := by
  letI := QuadraticExtField.instField hv
  letI := quadAlgebra hv
  refine ⟨fun x y => map_add _ x y, fun x y => map_mul _ x y,
    RelativeExtension.relFrobenius_algebraMap BF, fun x => ?_⟩
  have h := RelativeExtension.relFrobenius_iterate_finrank BF
    (E := gfpn.quadratic.QuadraticExt C BF Char Wide) x
  rw [quad_finrank hv] at h
  simpa using h

/-! ### The extracted cubic carrier -/

omit [Field BF] in
/-- Componentwise extensionality for the extracted cubic carrier. -/
theorem cubicExt_ext {a b : gfpn.cubic.CubicExt C BF Char Wide}
    (h0 : a.c0 = b.c0) (h1 : a.c1 = b.c1) (h2 : a.c2 = b.c2) : a = b := by
  cases a; cases b; simp_all

/-- The extracted cubic carrier is its triple of coefficients. -/
def cubicEquivProd : gfpn.cubic.CubicExt C BF Char Wide ≃ BF × BF × BF where
  toFun a := (a.c0, a.c1, a.c2)
  invFun p := ⟨p.1, p.2.1, p.2.2⟩
  left_inv a := by cases a; rfl
  right_inv p := by obtain ⟨_, _, _⟩ := p; rfl

instance instFiniteCubicExt [Finite BF] :
    Finite (gfpn.cubic.CubicExt C BF Char Wide) :=
  Finite.of_equiv _ (cubicEquivProd (C := C) (BF := BF) (Char := Char) (Wide := Wide)).symm

omit [Field BF] in
/-- `|E| = q ^ 3` for the extracted cubic carrier. -/
theorem cubic_card [Finite BF] :
    Nat.card (gfpn.cubic.CubicExt C BF Char Wide) = Nat.card BF ^ 3 := by
  rw [Nat.card_congr (cubicEquivProd (C := C) (BF := BF) (Char := Char) (Wide := Wide)),
    Nat.card_prod, Nat.card_prod]
  ring

/-- The model of `FieldExtension::embed`
(`crates/gf2-core/src/field/extension.rs:2558`) at the extracted cubic carrier:
`a ↦ (a, 0, 0)`. -/
@[reducible]
noncomputable def cubicAlgebra (hv : ValidCubicExtConfig inst) :
    letI := CubicExtField.instField hv
    Algebra BF (gfpn.cubic.CubicExt C BF Char Wide) :=
  letI := CubicExtField.instField hv
  RingHom.toAlgebra
    { toFun := fun a => ⟨a, 0, 0⟩
      map_one' := rfl
      map_zero' := rfl
      map_mul' := by
        intro a b
        refine cubicExt_ext ?_ ?_ ?_
        · show a * b = a * b + hv.toValidExtConfig.getNonResidue * (0 * 0 + 0 * 0); ring
        · show (0 : BF) = a * 0 + 0 * b + hv.toValidExtConfig.getNonResidue * (0 * 0); ring
        · show (0 : BF) = a * 0 + 0 * 0 + 0 * b; ring
      map_add' := by
        intro a b
        refine cubicExt_ext rfl ?_ ?_
        · show (0 : BF) = 0 + 0; ring
        · show (0 : BF) = 0 + 0; ring }

/-- **L1.1 at the extracted cubic carrier.** -/
theorem cubic_embedding_laws (hv : ValidCubicExtConfig inst) :
    letI := CubicExtField.instField hv
    letI := cubicAlgebra hv
    algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) 0 = 0 ∧
      algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) 1 = 1 ∧
      (∀ a b : BF, algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) (a + b) =
        algebraMap BF _ a + algebraMap BF _ b) ∧
      (∀ a b : BF, algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) (a * b) =
        algebraMap BF _ a * algebraMap BF _ b) ∧
      Function.Injective (algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide)) :=
  letI := CubicExtField.instField hv
  letI := cubicAlgebra hv
  RelativeExtension.embedding_laws BF

/-- **L1.2 at the extracted cubic carrier.** The relative degree is three. -/
theorem cubic_finrank (hv : ValidCubicExtConfig inst) [Finite BF] :
    letI := CubicExtField.instField hv
    letI := cubicAlgebra hv
    Module.finrank BF (gfpn.cubic.CubicExt C BF Char Wide) = 3 := by
  letI := CubicExtField.instField hv
  letI := cubicAlgebra hv
  have hcard := RelativeExtension.card_ext_eq_baseCard_pow_finrank BF
    (E := gfpn.cubic.CubicExt C BF Char Wide)
  rw [cubic_card] at hcard
  have hq : 2 ≤ Nat.card BF :=
    RelativeExtension.one_lt_baseCard BF (gfpn.cubic.CubicExt C BF Char Wide)
  exact (Nat.pow_right_injective hq hcard).symm

/-- **L1.3 and L1.4 at the extracted cubic carrier.** The relative Frobenius
`x ↦ x ^ q` is a ring endomorphism, it fixes the embedded base pointwise, and its
cube is the identity. -/
theorem cubic_relFrobenius_laws (hv : ValidCubicExtConfig inst) [Finite BF] :
    letI := CubicExtField.instField hv
    letI := cubicAlgebra hv
    (∀ x y : gfpn.cubic.CubicExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (x + y) =
          RelativeExtension.relFrobenius BF x + RelativeExtension.relFrobenius BF y) ∧
      (∀ x y : gfpn.cubic.CubicExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (x * y) =
          RelativeExtension.relFrobenius BF x * RelativeExtension.relFrobenius BF y) ∧
      (∀ a : BF, RelativeExtension.relFrobenius BF
          (algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) a) =
        algebraMap BF (gfpn.cubic.CubicExt C BF Char Wide) a) ∧
      (∀ x : gfpn.cubic.CubicExt C BF Char Wide,
        RelativeExtension.relFrobenius BF (RelativeExtension.relFrobenius BF
          (RelativeExtension.relFrobenius BF x)) = x) := by
  letI := CubicExtField.instField hv
  letI := cubicAlgebra hv
  refine ⟨fun x y => map_add _ x y, fun x y => map_mul _ x y,
    RelativeExtension.relFrobenius_algebraMap BF, fun x => ?_⟩
  have h := RelativeExtension.relFrobenius_iterate_finrank BF
    (E := gfpn.cubic.CubicExt C BF Char Wide) x
  rw [cubic_finrank hv] at h
  simpa using h

end ExtractedExtension

end
