/-
  Gf2Core.Proofs.BchSystematicEncoding — obligation O-5 (issue 94597a51): systematic BCH
  encoding, field-generic and packed binary, writes under every declared layout a codeword
  that is a multiple of the generator and carries the message in its first `k` coordinates.
  Production path, refinement anchors, assumptions (A-11, A-12, A-13) and proof-route notes:
  `dev/active/ae03bcd0-general-bch/64fd3afd/proof-sketch.md`, section O-5.
-/
import Mathlib.Algebra.Polynomial.Div
import Gf2Core.Proofs.BchGenerator

set_option linter.unusedSectionVars false

noncomputable section

namespace BchSystematicEncoding

open Polynomial

variable {B : Type*} [Field B]

/-! ## Section 1 — L5.1 and L5.2: the parity specification -/

/-- The parity `p = -(X ^ ρ * m mod g)`, with `ρ = deg g`: the reference specification of the
module documentation (`crates/gf2-coding/src/bch/encode.rs:1-12`). -/
def parity (g m : B[X]) : B[X] := -((X ^ g.natDegree * m) %ₘ g)

/-- The systematic codeword `c = X ^ ρ * m + p`. -/
def codeword (g m : B[X]) : B[X] := X ^ g.natDegree * m + parity g m

/-- A monic generator has degree `ρ`. -/
theorem degree_eq_redundancy {g : B[X]} (hg : g.Monic) : g.degree = (g.natDegree : WithBot ℕ) :=
  degree_eq_natDegree hg.ne_zero

/-- **L5.1 (parity degree).** `deg p < ρ`.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem degree_parity_lt {g : B[X]} (hg : g.Monic) (m : B[X]) :
    (parity g m).degree < g.degree := by
  rw [parity, degree_neg]
  exact degree_modByMonic_lt _ hg

/-- The codeword is `g` times the quotient. -/
theorem codeword_eq_mul (g m : B[X]) : codeword g m = g * ((X ^ g.natDegree * m) /ₘ g) := by
  have h := modByMonic_add_div (X ^ g.natDegree * m) g
  rw [codeword, parity]
  linear_combination -h

/-- **L5.1 (divisibility).** `g ∣ c`.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem dvd_codeword (g m : B[X]) : g ∣ codeword g m :=
  ⟨_, codeword_eq_mul g m⟩

/-- Coefficients of the shifted message. -/
theorem coeff_shift (g m : B[X]) (d : ℕ) :
    (X ^ g.natDegree * m).coeff d = if g.natDegree ≤ d then m.coeff (d - g.natDegree) else 0 :=
  coeff_X_pow_mul' m _ d

/-- Parity coefficients vanish from degree `ρ` on. -/
theorem coeff_parity_of_le {g : B[X]} (hg : g.Monic) (m : B[X]) {d : ℕ}
    (hd : g.natDegree ≤ d) : (parity g m).coeff d = 0 := by
  have h := degree_parity_lt hg m
  rw [degree_eq_redundancy hg, degree_lt_iff_coeff_zero] at h
  exact h d hd

/-- **L5.1 (codeword degree).** `deg c < n`, given `deg m < k`.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem degree_codeword_lt {g : B[X]} (hg : g.Monic) {n : ℕ} (hρn : g.natDegree ≤ n) {m : B[X]}
    (hm : m.degree < ((n - g.natDegree : ℕ) : WithBot ℕ)) : (codeword g m).degree < n := by
  rw [degree_lt_iff_coeff_zero] at hm ⊢
  intro d hd
  rw [codeword, coeff_add, coeff_shift, coeff_parity_of_le hg m (by omega),
    if_pos (by omega), hm _ (by omega), add_zero]

/-- **L5.2 (uniqueness).** `p` is the only polynomial of degree below `ρ` with
`g ∣ X ^ ρ * m + p`.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem parity_unique {g : B[X]} (hg : g.Monic) (m : B[X]) {p : B[X]}
    (hp : p.degree < g.degree) (hdvd : g ∣ X ^ g.natDegree * m + p) : p = parity g m := by
  have hsub : g ∣ p - parity g m := by
    have := dvd_sub hdvd (dvd_codeword g m)
    rwa [codeword, add_sub_add_left_eq_sub] at this
  have hdeg : (p - parity g m).degree < g.degree :=
    lt_of_le_of_lt (degree_sub_le _ _) (max_lt hp (degree_parity_lt hg m))
  exact sub_eq_zero.1 (eq_zero_of_dvd_of_degree_lt hsub hdeg)

/-- **L5.2, the codeword is determined.** A polynomial of degree below `n` that `g` divides and
that carries the message at degrees `ρ` to `n - 1` is the systematic codeword.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem eq_codeword {g : B[X]} (hg : g.Monic) {n : ℕ} (hρn : g.natDegree ≤ n) {m : B[X]}
    (hm : m.degree < ((n - g.natDegree : ℕ) : WithBot ℕ)) {c : B[X]} (hc : c.degree < n)
    (hdvd : g ∣ c) (hmsg : ∀ i < n - g.natDegree, c.coeff (g.natDegree + i) = m.coeff i) :
    c = codeword g m := by
  have hp : (c - X ^ g.natDegree * m).degree < g.degree := by
    rw [degree_eq_redundancy hg, degree_lt_iff_coeff_zero]
    rw [degree_lt_iff_coeff_zero] at hm hc
    intro d hd
    rw [coeff_sub, coeff_shift, if_pos hd]
    by_cases hdn : d < n
    · obtain ⟨i, rfl⟩ : ∃ i, d = g.natDegree + i := ⟨d - g.natDegree, by omega⟩
      rw [hmsg i (by omega), Nat.add_sub_cancel_left, sub_self]
    · rw [hc d (by omega), hm _ (by omega), sub_zero]
  have h := parity_unique hg m hp (by rwa [add_sub_cancel])
  rw [codeword, ← h, add_sub_cancel]

/-! ## Section 2 — L5.3: the shift-register recurrence -/

/-- The message suffix from degree `j`, shifted down to degree zero:
`∑_{j ≤ i < k} m_i X ^ (i - j)`. -/
def tail (m : B[X]) (k j : ℕ) : B[X] :=
  ∑ i ∈ Finset.range (k - j), C (m.coeff (j + i)) * X ^ i

/-- The register state `s_j = X ^ ρ * tail_j mod g` of L5.3. -/
def rem (g m : B[X]) (k j : ℕ) : B[X] := (X ^ g.natDegree * tail m k j) %ₘ g

/-- The empty suffix. -/
theorem tail_self (m : B[X]) (k : ℕ) : tail m k k = 0 := by
  simp [tail]

/-- Peeling the lowest degree off a suffix is one Horner step. -/
theorem tail_succ (m : B[X]) {k j : ℕ} (hj : j < k) :
    tail m k j = X * tail m k (j + 1) + C (m.coeff j) := by
  have hk : k - j = (k - (j + 1)) + 1 := by omega
  rw [tail, hk, Finset.sum_range_succ', tail, Finset.mul_sum]
  congr 1
  · refine Finset.sum_congr rfl fun i _ => ?_
    rw [show j + (i + 1) = j + 1 + i by omega, pow_succ]
    ring
  · simp

/-- The full suffix carries the message coefficients below `k`. -/
theorem coeff_tail_zero (m : B[X]) (k d : ℕ) :
    (tail m k 0).coeff d = if d < k then m.coeff d else 0 := by
  simp only [tail, Nat.sub_zero, zero_add, finset_sum_coeff, coeff_C_mul_X_pow]
  rw [Finset.sum_ite_eq]
  simp [Finset.mem_range]

/-- The full suffix is the message, when its degree is below `k`. -/
theorem tail_zero {m : B[X]} {k : ℕ} (hm : m.degree < (k : WithBot ℕ)) : tail m k 0 = m := by
  ext d
  rw [coeff_tail_zero]
  split_ifs with h
  · rfl
  · rw [degree_lt_iff_coeff_zero] at hm
    exact (hm d (by omega)).symm

/-- The cleared register, `s_k = 0`. -/
theorem rem_self (g m : B[X]) (k : ℕ) : rem g m k k = 0 := by
  simp [rem, tail_self]

/-- **L5.3 (the recurrence).** `s_j = (X * s_{j+1} + X ^ ρ * m_j) mod g`.

Anchor: `binary_encoding_agrees_with_the_legacy_encoder`
(`crates/gf2-coding/src/bch/encode.rs:3136`). -/
theorem rem_succ {g : B[X]} (hg : g.Monic) (m : B[X]) {k j : ℕ} (hj : j < k) :
    rem g m k j = (X * rem g m k (j + 1) + X ^ g.natDegree * C (m.coeff j)) %ₘ g := by
  rw [rem, rem]
  apply modByMonic_eq_of_dvd_sub hg
  have h := modByMonic_add_div (X ^ g.natDegree * tail m k (j + 1)) g
  refine ⟨X * ((X ^ g.natDegree * tail m k (j + 1)) /ₘ g), ?_⟩
  rw [tail_succ m hj]
  linear_combination -(X * h)

/-- The register after the whole message is `X ^ ρ * m mod g`. -/
theorem rem_zero (g : B[X]) {m : B[X]} {k : ℕ} (hm : m.degree < (k : WithBot ℕ)) :
    rem g m k 0 = (X ^ g.natDegree * m) %ₘ g := by
  rw [rem, tail_zero hm]

/-- One step of the production register update, on the coefficient function `R` of the register
and the entering message symbol `a`: the feedback `fb = R_{ρ-1} + a`, then
`R'_0 = -(fb * g_0)` and `R'_i = R_{i-1} - fb * g_i` for `0 < i < ρ`
(`crates/gf2-coding/src/bch/encode.rs:1367-1371`). The descending `index` loop reads each
`R_{i-1}` before it is rewritten, so it is the simultaneous update written here. -/
def stepReg (g : B[X]) (R : ℕ → B) (a : B) : ℕ → B := fun i =>
  if i = 0 then -((R (g.natDegree - 1) + a) * g.coeff 0)
  else if i < g.natDegree then R (i - 1) - (R (g.natDegree - 1) + a) * g.coeff i
  else 0

/-- **L5.3 (the closed-form step).** Reducing `X * s + X ^ ρ * a` modulo a monic `g` of degree
`ρ ≥ 1` is the production update on the coefficients of `s`.

Anchor: `binary_encoding_agrees_with_the_legacy_encoder`
(`crates/gf2-coding/src/bch/encode.rs:3136`). -/
theorem coeff_step {g : B[X]} (hg : g.Monic) (hρ : 0 < g.natDegree) {s : B[X]}
    (hs : s.degree < g.degree) (a : B) :
    ((X * s + X ^ g.natDegree * C a) %ₘ g).coeff = stepReg g s.coeff a := by
  have hs' : ∀ d, g.natDegree ≤ d → s.coeff d = 0 := by
    rw [degree_eq_redundancy hg, degree_lt_iff_coeff_zero] at hs
    exact hs
  have hlead : g.coeff g.natDegree = 1 := hg
  have hcoeff : (X * s + X ^ g.natDegree * C a
      - C (s.coeff (g.natDegree - 1) + a) * g).coeff = stepReg g s.coeff a := by
    funext i
    rcases i with _ | i
    · simp only [coeff_sub, coeff_add, coeff_C_mul, coeff_X_pow_mul', coeff_X_mul_zero, stepReg]
      rw [if_neg (by omega)]
      simp
    · simp only [coeff_sub, coeff_add, coeff_C_mul, coeff_X_pow_mul', coeff_X_mul, stepReg]
      rw [if_neg (Nat.succ_ne_zero i)]
      rcases lt_trichotomy (i + 1) g.natDegree with h | h | h
      · rw [if_neg (by omega), if_pos h, Nat.add_sub_cancel]
        ring
      · have h1 : g.coeff (i + 1) = 1 := h ▸ hlead
        rw [if_pos (by omega), if_neg (by omega), show i + 1 - g.natDegree = 0 by omega,
          coeff_C_zero, h1, show g.natDegree - 1 = i by omega]
        ring
      · rw [if_pos (by omega), if_neg (by omega),
          coeff_eq_zero_of_natDegree_lt (by omega : g.natDegree < i + 1), hs' i (by omega),
          coeff_C, if_neg (by omega)]
        ring
  have hdeg : (X * s + X ^ g.natDegree * C a
      - C (s.coeff (g.natDegree - 1) + a) * g).degree < g.degree := by
    rw [degree_eq_redundancy hg, degree_lt_iff_coeff_zero, hcoeff]
    intro d hd
    simp only [stepReg]
    rw [if_neg (by omega), if_neg (by omega)]
  have hmod : (X * s + X ^ g.natDegree * C a) %ₘ g = (X * s + X ^ g.natDegree * C a
      - C (s.coeff (g.natDegree - 1) + a) * g) %ₘ g :=
    modByMonic_eq_of_dvd_sub hg ⟨C (s.coeff (g.natDegree - 1) + a), by ring⟩
  rw [hmod, (modByMonic_eq_self_iff hg).2 hdeg, hcoeff]

/-- The production loop: the register after `t` iterations of
`for degree in (0..k).rev()`, reading the message symbol of degree `k - 1 - t` at iteration `t`,
starting from the cleared register (`crates/gf2-coding/src/bch/encode.rs:1361-1372`). -/
def regRun (g : B[X]) (msg : ℕ → B) (k : ℕ) : ℕ → ℕ → B
  | 0 => fun _ => 0
  | t + 1 => stepReg g (regRun g msg k t) (msg (k - 1 - t))

/-- The loop reads only the message symbols of degree below `k`. -/
theorem regRun_congr (g : B[X]) {msg msg' : ℕ → B} {k : ℕ} (h : ∀ d < k, msg d = msg' d) :
    ∀ t ≤ k, regRun g msg k t = regRun g msg' k t
  | 0, _ => rfl
  | t + 1, ht => by
    simp only [regRun]
    rw [regRun_congr g h t (by omega), h _ (by omega)]

/-- **L5.3 (the loop invariant).** After `t` iterations the register holds `s_{k-t}`.

Anchor: `binary_encoding_agrees_with_the_legacy_encoder`
(`crates/gf2-coding/src/bch/encode.rs:3136`). -/
theorem regRun_eq_rem {g : B[X]} (hg : g.Monic) (hρ : 0 < g.natDegree) (m : B[X]) (k : ℕ) :
    ∀ t ≤ k, regRun g m.coeff k t = (rem g m k (k - t)).coeff
  | 0, _ => by
    funext i
    simp [regRun, rem_self]
  | t + 1, ht => by
    have hj : k - (t + 1) < k := by omega
    rw [regRun, regRun_eq_rem hg hρ m k t (by omega), rem_succ hg m hj,
      show k - (t + 1) + 1 = k - t by omega, show k - 1 - t = k - (t + 1) by omega]
    exact (coeff_step hg hρ (degree_modByMonic_lt _ hg) _).symm

/-- **L5.3, the register after the loop.** The register holds `X ^ ρ * m mod g`, so the written
parity symbol `-R_i` is the coefficient of `X ^ i` in `p`.

Anchor: `binary_encoding_agrees_with_the_legacy_encoder`
(`crates/gf2-coding/src/bch/encode.rs:3136`). -/
theorem regRun_full {g : B[X]} (hg : g.Monic) (hρ : 0 < g.natDegree) {m : B[X]} {k : ℕ}
    (hm : m.degree < (k : WithBot ℕ)) :
    regRun g m.coeff k k = ((X ^ g.natDegree * m) %ₘ g).coeff := by
  rw [regRun_eq_rem hg hρ m k k le_rfl, Nat.sub_self, rem_zero g hm]

/-- The written parity symbol `-R_i` (`crates/gf2-coding/src/bch/encode.rs:1384`) is the
coefficient of `X ^ i` in `p`. -/
theorem coeff_parity_eq_neg_regRun {g : B[X]} (hg : g.Monic) (hρ : 0 < g.natDegree) {m : B[X]}
    {k : ℕ} (hm : m.degree < (k : WithBot ℕ)) (i : ℕ) :
    (parity g m).coeff i = -regRun g m.coeff k k i := by
  rw [regRun_full hg hρ hm, parity, coeff_neg]

/-! ## Section 3 — L5.4 and L5.5: the declared layouts -/

/-- The two declared layouts of `SystematicLayout` (`crates/gf2-coding/src/bch/encode.rs:346`). -/
inductive Layout
  | messageParityAscending
  | messageParityDescending
  deriving DecidableEq

namespace Layout

variable (L : Layout) (n k : ℕ)

/-- `internal_at` (`crates/gf2-coding/src/bch/encode.rs:492`): the internal coordinate, the
degree, that user coordinate `u` carries, in the production's branch form. -/
def internalAt (u : ℕ) : ℕ :=
  match L with
  | messageParityAscending => if u + (n - k) ≥ n then u + (n - k) - n else u + (n - k)
  | messageParityDescending => n - 1 - u

/-- `user_at` (`crates/gf2-coding/src/bch/encode.rs:508`): the user coordinate presenting
internal coordinate `i`. -/
def userAt (i : ℕ) : ℕ :=
  match L with
  | messageParityAscending => if i + k ≥ n then i + k - n else i + k
  | messageParityDescending => n - 1 - i

/-- `message_at` (`crates/gf2-coding/src/bch/encode.rs:523`): the user coordinate carrying the
message coefficient of degree `d`, that is, of `X ^ (ρ + d)`. -/
def messageAt (d : ℕ) : ℕ := userAt L n k (n - k + d)

variable {L n k}

/-- **L5.4.** The ascending branch form is the sketch's rotation `(u + ρ) mod n`.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem internalAt_ascending_eq_mod {u : ℕ} (hu : u < n) :
    internalAt messageParityAscending n k u = (u + (n - k)) % n := by
  simp only [internalAt]
  split_ifs with h
  · rw [Nat.mod_eq_sub_mod h, Nat.mod_eq_of_lt (by omega)]
  · rw [Nat.mod_eq_of_lt (by omega)]

/-- **L5.4.** The ascending inverse is the rotation `(i + k) mod n`.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem userAt_ascending_eq_mod {i : ℕ} (hk : k ≤ n) (hi : i < n) :
    userAt messageParityAscending n k i = (i + k) % n := by
  simp only [userAt]
  split_ifs with h
  · rw [Nat.mod_eq_sub_mod h, Nat.mod_eq_of_lt (by omega)]
  · rw [Nat.mod_eq_of_lt (by omega)]

/-- **L5.4.** `internal_at` stays in range.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem internalAt_lt {u : ℕ} (hk : k ≤ n) (hu : u < n) : internalAt L n k u < n := by
  cases L <;> simp only [internalAt] <;> (try split_ifs) <;> omega

/-- **L5.4.** `user_at` stays in range.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem userAt_lt {i : ℕ} (hk : k ≤ n) (hi : i < n) : userAt L n k i < n := by
  cases L <;> simp only [userAt] <;> (try split_ifs) <;> omega

/-- **L5.4.** `user_at` inverts `internal_at`.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem userAt_internalAt {u : ℕ} (hk : k ≤ n) (hu : u < n) :
    userAt L n k (internalAt L n k u) = u := by
  cases L <;> simp only [internalAt, userAt] <;> (try split_ifs) <;> omega

/-- **L5.4.** `internal_at` inverts `user_at`.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem internalAt_userAt {i : ℕ} (hk : k ≤ n) (hi : i < n) :
    internalAt L n k (userAt L n k i) = i := by
  cases L <;> simp only [internalAt, userAt] <;> (try split_ifs) <;> omega

/-- **L5.4, the systematic property.** User coordinate `u` carries a message degree, at least
`ρ`, exactly when `u < k`.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
theorem lt_dimension_iff {u : ℕ} (hk : k ≤ n) (hu : u < n) :
    u < k ↔ n - k ≤ internalAt L n k u := by
  cases L <;> simp only [internalAt] <;> (try split_ifs) <;> omega

/-- **L5.4.** Each declared layout is a permutation of the `n` coordinates.

Anchor: `the_layout_mapping_is_a_bijection_placing_the_message_first`
(`crates/gf2-coding/src/bch/encode.rs:3226`). -/
def equiv (L : Layout) {n k : ℕ} (hk : k ≤ n) : Fin n ≃ Fin n where
  toFun u := ⟨internalAt L n k u, internalAt_lt hk u.2⟩
  invFun i := ⟨userAt L n k i, userAt_lt hk i.2⟩
  left_inv u := Fin.ext (userAt_internalAt hk u.2)
  right_inv i := Fin.ext (internalAt_userAt hk i.2)

/-- **L5.5.** The message symbol of degree `d < k` sits at a user coordinate below `k`.

Anchor: `every_declared_layout_round_trips_over_each_base_field`
(`crates/gf2-coding/src/bch/encode.rs:3315`). -/
theorem messageAt_lt {d : ℕ} (hk : k ≤ n) (hd : d < k) : messageAt L n k d < k := by
  rw [messageAt, lt_dimension_iff hk (userAt_lt hk (by omega)), internalAt_userAt hk (by omega)]
  omega

/-- **L5.5.** The coordinate `message_at d` presents the internal degree `ρ + d`.

Anchor: `every_declared_layout_round_trips_over_each_base_field`
(`crates/gf2-coding/src/bch/encode.rs:3315`). -/
theorem internalAt_messageAt {d : ℕ} (hk : k ≤ n) (hd : d < k) :
    internalAt L n k (messageAt L n k d) = n - k + d :=
  internalAt_userAt hk (by omega)

/-- **L5.5.** User coordinate `u < k` is `message_at` of the degree it carries.

Anchor: `every_declared_layout_round_trips_over_each_base_field`
(`crates/gf2-coding/src/bch/encode.rs:3315`). -/
theorem messageAt_internalAt {u : ℕ} (hk : k ≤ n) (hu : u < k) :
    messageAt L n k (internalAt L n k u - (n - k)) = u := by
  have h := (lt_dimension_iff (L := L) hk (by omega : u < n)).1 hu
  rw [messageAt, Nat.add_sub_cancel' h, userAt_internalAt hk (by omega)]

end Layout

open Layout

/-- The message polynomial the field-generic loop reads
(`crates/gf2-coding/src/bch/encode.rs:1366`): the coefficient of `X ^ d` is the user symbol at
`message_at d`. -/
def msgPoly (L : Layout) (n k : ℕ) (msg : ℕ → B) : B[X] :=
  ∑ d ∈ Finset.range k, C (msg (messageAt L n k d)) * X ^ d

/-- The coefficients of the message polynomial. -/
theorem coeff_msgPoly (L : Layout) (n k : ℕ) (msg : ℕ → B) (d : ℕ) :
    (msgPoly L n k msg).coeff d = if d < k then msg (messageAt L n k d) else 0 := by
  simp only [msgPoly, finset_sum_coeff, coeff_C_mul_X_pow]
  rw [Finset.sum_ite_eq]
  simp [Finset.mem_range]

/-- The message polynomial has degree below `k`. -/
theorem degree_msgPoly_lt (L : Layout) (n k : ℕ) (msg : ℕ → B) :
    (msgPoly L n k msg).degree < (k : WithBot ℕ) := by
  rw [degree_lt_iff_coeff_zero]
  intro d hd
  rw [coeff_msgPoly, if_neg (by omega)]

/-- The user codeword the field-generic `encode_systematic_with`
(`crates/gf2-coding/src/bch/encode.rs:1345`) writes: the message copied into
user coordinates `0` to `k - 1`, and `-R_i` at user coordinate `u ≥ k` with `i = internal_at u`,
where `R` is the register after the loop. With `ρ = 0` the loop is skipped and the register is
never read, which is the `if redundancy > 0` guard. -/
def encodeUser (g : B[X]) (L : Layout) (n : ℕ) (msg : ℕ → B) (u : ℕ) : B :=
  if u < n - g.natDegree then msg u
  else -regRun g (fun d => msg (messageAt L n (n - g.natDegree) d)) (n - g.natDegree)
    (n - g.natDegree) (internalAt L n (n - g.natDegree) u)

/-- **L5.5, message survival.** The first `k` user coordinates are the message, which is what
`systematic_message` (`crates/gf2-coding/src/bch/encode.rs:2494`) copies back.

Anchor: `every_declared_layout_round_trips_over_each_base_field`
(`crates/gf2-coding/src/bch/encode.rs:3315`). -/
theorem encodeUser_message (g : B[X]) (L : Layout) (n : ℕ) (msg : ℕ → B) {u : ℕ}
    (hu : u < n - g.natDegree) : encodeUser g L n msg u = msg u := by
  rw [encodeUser, if_pos hu]

/-- **L5.3 with L5.4 and L5.5, the field-generic path.** Every user coordinate the encoder
writes presents, through the layout, the coefficient of the systematic codeword `c` of
L5.1 at the degree the layout assigns it.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem encodeUser_eq_coeff_codeword {g : B[X]} (hg : g.Monic) (L : Layout) {n : ℕ}
    (hρn : g.natDegree ≤ n) (msg : ℕ → B) {u : ℕ} (hu : u < n) :
    encodeUser g L n msg u =
      (codeword g (msgPoly L n (n - g.natDegree) msg)).coeff
        (internalAt L n (n - g.natDegree) u) := by
  set k := n - g.natDegree with hkdef
  have hk : k ≤ n := Nat.sub_le _ _
  have hnk : n - k = g.natDegree := by omega
  set m := msgPoly L n k msg
  have hm := degree_msgPoly_lt L n k msg
  rw [encodeUser, codeword, coeff_add, coeff_shift]
  by_cases huk : u < k
  · have hle := (lt_dimension_iff (L := L) hk hu).1 huk
    rw [hnk] at hle
    rw [if_pos huk, if_pos hle, coeff_parity_of_le hg m hle, add_zero, coeff_msgPoly,
      if_pos (by have := internalAt_lt (L := L) hk hu; omega)]
    have h := messageAt_internalAt (L := L) hk huk
    rw [hnk] at h
    rw [h]
  · have hlt : internalAt L n k u < g.natDegree := by
      have := (lt_dimension_iff (L := L) hk hu).not.1 huk
      omega
    have hρ : 0 < g.natDegree := by omega
    rw [if_neg huk, if_neg (by omega), zero_add,
      coeff_parity_eq_neg_regRun hg hρ hm, ← hkdef]
    have hr := regRun_congr g (msg := fun d => msg (messageAt L n k d)) (msg' := m.coeff)
      (k := k) (fun d hd => by rw [coeff_msgPoly, if_pos hd]) k le_rfl
    rw [hr]

/-- **The O-5 obligation, field-generic path.** Under every declared layout, the user codeword
the encoder writes presents a multiple of `g` of degree below `n` and carries the message in its
first `k` coordinates.

Anchor: `assert_encodes_a_codeword` (`crates/gf2-coding/src/bch/encode.rs:3067`). -/
theorem encodeUser_presents_codeword {g : B[X]} (hg : g.Monic) (L : Layout) {n : ℕ}
    (hρn : g.natDegree ≤ n) (msg : ℕ → B) :
    ∃ c : B[X], g ∣ c ∧ c.degree < n ∧
      (∀ u < n, encodeUser g L n msg u = c.coeff (internalAt L n (n - g.natDegree) u)) ∧
      (∀ u < n - g.natDegree, encodeUser g L n msg u = msg u) :=
  ⟨codeword g (msgPoly L n (n - g.natDegree) msg), dvd_codeword _ _,
    degree_codeword_lt hg hρn (degree_msgPoly_lt L n _ msg),
    fun _ hu => encodeUser_eq_coeff_codeword hg L hρn msg hu,
    fun _ hu => encodeUser_message g L n msg hu⟩

/-! ## Section 4 — L5.7: the degenerate cases -/

/-- **L5.7, `ρ = 0`.** A monic generator of degree zero is `1`, the parity vanishes and the
codeword is the message.

Anchor: `the_full_space_code_encodes_the_identity_under_the_default_layout`
(`crates/gf2-coding/src/bch/encode.rs:3337`). -/
theorem codeword_of_natDegree_eq_zero {g : B[X]} (hg : g.Monic) (h0 : g.natDegree = 0)
    (m : B[X]) : codeword g m = m := by
  have h1 : g = 1 := eq_one_of_monic_natDegree_zero hg h0
  rw [codeword, parity, h1, modByMonic_one, natDegree_one, pow_zero, one_mul, neg_zero,
    add_zero]

/-- **L5.7, `ρ = 0`.** The full-space code writes the message unchanged.

Anchor: `the_full_space_code_encodes_the_identity_under_the_default_layout`
(`crates/gf2-coding/src/bch/encode.rs:3337`). -/
theorem encodeUser_of_natDegree_eq_zero (g : B[X]) (L : Layout) {n : ℕ} (h0 : g.natDegree = 0)
    (msg : ℕ → B) {u : ℕ} (hu : u < n) : encodeUser g L n msg u = msg u :=
  encodeUser_message g L n msg (by omega)

/-- **L5.7, `k = 0`.** The empty message encodes to the zero codeword.

Anchor: `the_zero_dimensional_code_rejects_a_nonempty_message`
(`crates/gf2-coding/src/bch/encode.rs:3353`). -/
theorem codeword_of_dimension_zero {g : B[X]} (L : Layout) {n : ℕ}
    (h0 : n - g.natDegree = 0) (msg : ℕ → B) :
    codeword g (msgPoly L n (n - g.natDegree) msg) = 0 := by
  rw [h0, msgPoly, Finset.range_zero, Finset.sum_empty, codeword, parity, mul_zero,
    zero_modByMonic, neg_zero, add_zero]

/-! ## Section 5 — L5.6: the packed binary path -/

namespace Packed

/-- A coefficient bit as an element of `𝔽₂`. -/
def toZ (b : Bool) : ZMod 2 := if b then 1 else 0

/-- `(x >> s) & 1 == 1`, the production's read of bit `s` of a word
(`crates/gf2-coding/src/bch/encode.rs:1640`, `:2039`). -/
def readBit (x : BitVec 64) (s : ℕ) : Bool := ((x >>> s) &&& 1#64) == 1#64

/-- The production's bit read is `BitVec.getLsbD`. -/
theorem readBit_eq (x : BitVec 64) (s : ℕ) : readBit x s = x.getLsbD s := by
  have h : (x >>> s) &&& 1#64 = if x.getLsbD s then 1#64 else 0#64 := by
    ext j hj
    simp only [BitVec.getElem_and, BitVec.getElem_ushiftRight, BitVec.getElem_one]
    by_cases hj0 : j = 0
    · subst hj0
      cases hb : x.getLsbD s <;> simp
    · cases hb : x.getLsbD s <;> simp [hj0]
  rw [readBit, h]
  cases x.getLsbD s <;> decide

/-- Bit `i` of a packed register, at word `i / 64` and position `i % 64`, the canonical bit
indexing. -/
def bit (reg : ℕ → BitVec 64) (i : ℕ) : Bool := (reg (i / 64)).getLsbD (i % 64)

/-- `packed_tail_mask` (`crates/gf2-coding/src/bch/encode.rs:1622`). -/
def tailMask (ρ : ℕ) : BitVec 64 :=
  if ρ % 64 = 0 then BitVec.allOnes 64 else (1#64 <<< (ρ % 64)) - 1#64

/-- `(1 << r) - 1` sets exactly the bits below `r`. -/
theorem getLsbD_low_mask {r : ℕ} (hr : r < 64) (j : ℕ) :
    ((1#64 <<< r) - 1#64).getLsbD j = decide (j < r) := by
  have h : (1#64 <<< r) - 1#64 = BitVec.ofNat 64 (2 ^ r - 1) := by
    interval_cases r <;> rfl
  rw [h, BitVec.getLsbD_ofNat, Nat.testBit_two_pow_sub_one]
  by_cases hj : j < r
  · simp [hj, show j < 64 by omega]
  · simp [hj]

/-- Bit `j` of the top word survives the mask exactly when its degree is below `ρ`, in both
branches of `packed_tail_mask`, the `ρ mod 64 = 0` branch included.

Anchor: `the_packed_path_holds_at_the_word_boundaries`
(`crates/gf2-coding/src/bch/encode.rs:3190`). -/
theorem getLsbD_tailMask {ρ : ℕ} (hρ : 0 < ρ) {j : ℕ} (hj : j < 64) :
    (tailMask ρ).getLsbD j = decide (64 * ((ρ + 63) / 64 - 1) + j < ρ) := by
  unfold tailMask
  split_ifs with h
  · rw [BitVec.getLsbD_allOnes]
    simp only [decide_eq_decide]
    omega
  · rw [getLsbD_low_mask (by omega)]
    simp only [decide_eq_decide]
    omega

/-- `packed_step` (`crates/gf2-coding/src/bch/encode.rs:1638`): one degree of the packed
recurrence over `W` words, with `top = ρ - 1`. The
descending in-place word loop reads each lower word before it is rewritten, so it is the
simultaneous shift written here. -/
def step (W : ℕ) (reg low : ℕ → BitVec 64) (top : ℕ) (tail : BitVec 64) (sym : Bool) :
    ℕ → BitVec 64 := fun w =>
  let fb := readBit (reg (top / 64)) (top % 64) != sym
  let sh : ℕ → BitVec 64 := fun w =>
    if w = 0 then reg 0 <<< 1
    else if w < W then (reg w <<< 1) ||| (reg (w - 1) >>> 63) else reg w
  let ms : BitVec 64 := if w = W - 1 then sh w &&& tail else sh w
  if fb && decide (w < W) then ms ^^^ low w else ms

/-- The words `reset_registers` (`crates/gf2-coding/src/bch/encode.rs:1394`) writes for the
generator's low coefficients: bit `i` is set exactly when `i < ρ` and `g_i = 1`. -/
def LowSpec (g : (ZMod 2)[X]) (W : ℕ) (low : ℕ → BitVec 64) : Prop :=
  ∀ i < 64 * W, bit low i = (decide (i < g.natDegree) && decide (g.coeff i = 1))

/-- The register is clear from degree `ρ` to the end of its words. -/
def Clean (ρ W : ℕ) (reg : ℕ → BitVec 64) : Prop := ∀ i, ρ ≤ i → i < 64 * W → bit reg i = false

/-- The register's coefficients as a function into `𝔽₂`, zero from degree `ρ` on. -/
def readout (ρ : ℕ) (reg : ℕ → BitVec 64) : ℕ → ZMod 2 := fun i =>
  if i < ρ then toZ (bit reg i) else 0

/-- An `𝔽₂` coefficient is its `is_one` bit. -/
theorem toZ_decide_eq_one (x : ZMod 2) : toZ (decide (x = 1)) = x := by
  revert x; decide

/-- Over `𝔽₂`, a conditional XOR is a multiply-subtract. -/
theorem toZ_xor_and (a f c : Bool) : toZ (a ^^ (f && c)) = toZ a - toZ f * toZ c := by
  cases a <;> cases f <;> cases c <;> decide

/-- Over `𝔽₂`, `!=` is addition. -/
theorem toZ_bne (a b : Bool) : toZ (a != b) = toZ a + toZ b := by
  cases a <;> cases b <;> decide

/-- Bit `i` of the shifted words, before masking: the shift moves each `R_{i-1}` to position `i`
and brings in zero at degree zero. -/
theorem bit_shift {W : ℕ} (reg : ℕ → BitVec 64) {i : ℕ} (hi : i < 64 * W) :
    ((fun w => if w = 0 then reg 0 <<< 1
      else if w < W then (reg w <<< 1) ||| (reg (w - 1) >>> 63) else reg w) (i / 64)).getLsbD
      (i % 64) = if i = 0 then false else bit reg (i - 1) := by
  simp only [bit]
  by_cases hw : i / 64 = 0
  · rw [if_pos hw, BitVec.getLsbD_shiftLeft]
    by_cases h0 : i = 0
    · subst h0; simp
    · rw [if_neg h0, show (i - 1) / 64 = 0 by omega, show (i - 1) % 64 = i % 64 - 1 by omega]
      simp [show i % 64 < 64 by omega, show ¬ i % 64 < 1 by omega]
  · rw [if_neg hw, if_pos (by omega), BitVec.getLsbD_or, BitVec.getLsbD_shiftLeft,
      BitVec.getLsbD_ushiftRight, if_neg (by omega)]
    by_cases hj : i % 64 = 0
    · rw [show (i - 1) / 64 = i / 64 - 1 by omega, show (i - 1) % 64 = 63 by omega, hj]
      simp
    · rw [show (i - 1) / 64 = i / 64 by omega, show (i - 1) % 64 = i % 64 - 1 by omega,
        BitVec.getLsbD_of_ge (reg (i / 64 - 1)) (63 + i % 64) (by omega)]
      simp [show i % 64 < 64 by omega, show ¬ i % 64 < 1 by omega]

/-- Bit `i < 64 W` of a packed step: the shifted bit, cleared from degree `ρ` on by the mask,
plus the feedback times the low-coefficient bit.

Anchor: `the_packed_path_holds_at_the_word_boundaries`
(`crates/gf2-coding/src/bch/encode.rs:3190`). -/
theorem bit_step {ρ : ℕ} (hρ : 0 < ρ) (reg low : ℕ → BitVec 64) (sym : Bool) {i : ℕ}
    (hi : i < 64 * ((ρ + 63) / 64)) :
    bit (step ((ρ + 63) / 64) reg low (ρ - 1) (tailMask ρ) sym) i =
      ((decide (i < ρ) && if i = 0 then false else bit reg (i - 1)) ^^
        ((bit reg (ρ - 1) != sym) && bit low i)) := by
  have hw : i / 64 < (ρ + 63) / 64 := by omega
  have hsh := bit_shift (W := (ρ + 63) / 64) reg hi
  simp only [bit, hw, if_true] at hsh ⊢
  simp only [step, readBit_eq, hw, decide_true, Bool.and_true]
  by_cases htop : i / 64 = (ρ + 63) / 64 - 1
  · have hm : (tailMask ρ).getLsbD (i % 64) = decide (i < ρ) := by
      rw [getLsbD_tailMask hρ (by omega)]
      exact decide_eq_decide.2 (by omega)
    rw [if_pos htop]
    cases hfb : ((reg ((ρ - 1) / 64)).getLsbD ((ρ - 1) % 64) != sym) <;>
      simp only [if_true, if_false, Bool.false_eq_true, BitVec.getLsbD_xor,
        BitVec.getLsbD_and, hsh, hm] <;> cases decide (i < ρ) <;> simp
  · have hlt : i < ρ := by omega
    rw [if_neg htop]
    cases hfb : ((reg ((ρ - 1) / 64)).getLsbD ((ρ - 1) % 64) != sym) <;>
      simp only [if_true, if_false, Bool.false_eq_true, BitVec.getLsbD_xor, hsh,
        decide_eq_true hlt] <;> simp

/-- **L5.6, the mask's invariant.** A packed step keeps the register clear from degree `ρ` on,
in both branches of `packed_tail_mask`.

Anchor: `the_packed_path_holds_at_the_word_boundaries`
(`crates/gf2-coding/src/bch/encode.rs:3190`). -/
theorem clean_step (g : (ZMod 2)[X]) (hρ : 0 < g.natDegree) {reg low : ℕ → BitVec 64}
    (hlow : LowSpec g ((g.natDegree + 63) / 64) low) (sym : Bool) :
    Clean g.natDegree ((g.natDegree + 63) / 64)
      (step ((g.natDegree + 63) / 64) reg low (g.natDegree - 1) (tailMask g.natDegree) sym) := by
  intro i hρi hi
  rw [bit_step hρ reg low sym hi, hlow i hi]
  simp [show ¬ i < g.natDegree by omega]

/-- **L5.6, one step.** On the coefficients below `ρ`, the packed word update is the
field-generic update `stepReg` over `𝔽₂`: negation is the identity and subtraction is XOR.

Anchor: `the_packed_path_agrees_with_the_field_generic_reference`
(`crates/gf2-coding/src/bch/encode.rs:3158`). -/
theorem readout_step (g : (ZMod 2)[X]) (hρ : 0 < g.natDegree) (reg : ℕ → BitVec 64)
    {low : ℕ → BitVec 64} (hlow : LowSpec g ((g.natDegree + 63) / 64) low) (sym : Bool) :
    readout g.natDegree
        (step ((g.natDegree + 63) / 64) reg low (g.natDegree - 1) (tailMask g.natDegree) sym) =
      stepReg g (readout g.natDegree reg) (toZ sym) := by
  funext i
  simp only [readout, stepReg]
  by_cases hiρ : i < g.natDegree
  · have hi : i < 64 * ((g.natDegree + 63) / 64) := by omega
    have hρ1 : g.natDegree - 1 < g.natDegree := by omega
    rw [if_pos hiρ, bit_step hρ reg low sym hi, hlow i hi, toZ_xor_and, toZ_bne]
    simp only [hiρ, hρ1, decide_true, Bool.true_and, toZ_decide_eq_one, if_true]
    by_cases h0 : i = 0
    · subst h0
      simp [toZ]
    · have hi1 : i - 1 < g.natDegree := by omega
      simp only [h0, hi1, if_false, if_true]
  · rw [if_neg hiρ, if_neg (by omega), if_neg hiρ]

/-- `packed_serial_reduce` (`crates/gf2-coding/src/bch/encode.rs:1744`): the packed register
after `t` iterations, from the cleared register. -/
def run (g : (ZMod 2)[X]) (low : ℕ → BitVec 64) (msg : ℕ → Bool) (k : ℕ) :
    ℕ → ℕ → BitVec 64
  | 0 => fun _ => 0#64
  | t + 1 => step ((g.natDegree + 63) / 64) (run g low msg k t) low (g.natDegree - 1)
      (tailMask g.natDegree) (msg (k - 1 - t))

/-- **L5.6, the loop.** The packed register's coefficients after `t` iterations are the
field-generic register's, and the register stays clear from degree `ρ` on.

Anchor: `the_packed_path_agrees_with_the_field_generic_reference`
(`crates/gf2-coding/src/bch/encode.rs:3158`). -/
theorem readout_run (g : (ZMod 2)[X]) (hρ : 0 < g.natDegree) {low : ℕ → BitVec 64}
    (hlow : LowSpec g ((g.natDegree + 63) / 64) low) (msg : ℕ → Bool) (k : ℕ) :
    ∀ t, readout g.natDegree (run g low msg k t) = regRun g (fun d => toZ (msg d)) k t ∧
      Clean g.natDegree ((g.natDegree + 63) / 64) (run g low msg k t)
  | 0 => by
    refine ⟨?_, fun i _ _ => by simp [run, bit]⟩
    funext i
    simp [readout, run, regRun, bit, toZ]
  | t + 1 => by
    refine ⟨?_, clean_step g hρ hlow _⟩
    rw [run, readout_step g hρ _ hlow, (readout_run g hρ hlow msg k t).1, regRun]

/-- `packed_write_codeword` (`crates/gf2-coding/src/bch/encode.rs:2024`) after
`packed_serial_reduce`: the message bits copied into user
coordinates `0` to `k - 1`, and register bit `internal_at u` at user coordinate `u ≥ k`. -/
def encodeUser (g : (ZMod 2)[X]) (low : ℕ → BitVec 64) (L : Layout) (n : ℕ) (msg : ℕ → Bool)
    (u : ℕ) : Bool :=
  if u < n - g.natDegree then msg u
  else
    let reg := run g low (fun d => msg (messageAt L n (n - g.natDegree) d)) (n - g.natDegree)
      (n - g.natDegree)
    let p := internalAt L n (n - g.natDegree) u
    readBit (reg (p / 64)) (p % 64)

/-- **L5.6, the packed path agrees.** Every user coordinate the packed encoder writes is, read
in `𝔽₂`, the symbol the field-generic encoder writes.

Anchor: `the_packed_path_agrees_with_the_field_generic_reference`
(`crates/gf2-coding/src/bch/encode.rs:3158`). -/
theorem toZ_encodeUser (g : (ZMod 2)[X]) {low : ℕ → BitVec 64}
    (hlow : LowSpec g ((g.natDegree + 63) / 64) low) (L : Layout) {n : ℕ}
    (hρn : g.natDegree ≤ n) (msg : ℕ → Bool) {u : ℕ} (hu : u < n) :
    toZ (encodeUser g low L n msg u) =
      BchSystematicEncoding.encodeUser g L n (fun v => toZ (msg v)) u := by
  simp only [encodeUser, BchSystematicEncoding.encodeUser]
  by_cases huk : u < n - g.natDegree
  · rw [if_pos huk, if_pos huk]
  · rw [if_neg huk, if_neg huk]
    have hk : n - g.natDegree ≤ n := Nat.sub_le _ _
    have hlt : internalAt L n (n - g.natDegree) u < g.natDegree := by
      have := (lt_dimension_iff (L := L) hk hu).not.1 huk
      omega
    have hρ : 0 < g.natDegree := by omega
    have h := congrFun (readout_run g hρ hlow
      (fun d => msg (messageAt L n (n - g.natDegree) d)) (n - g.natDegree)
      (n - g.natDegree)).1 (internalAt L n (n - g.natDegree) u)
    simp only [readout, if_pos hlt] at h
    simp only [readBit_eq]
    rw [ZMod.neg_eq_self_mod_two, ← h, bit]

/-- **The O-5 obligation, packed path.** Under every declared layout, the packed encoder's
output, read in `𝔽₂`, presents a multiple of `g` of degree below `n` and carries the message in
its first `k` coordinates.

Anchor: `the_packed_path_agrees_with_the_field_generic_reference`
(`crates/gf2-coding/src/bch/encode.rs:3158`). -/
theorem encodeUser_presents_codeword {g : (ZMod 2)[X]} (hg : g.Monic) {low : ℕ → BitVec 64}
    (hlow : LowSpec g ((g.natDegree + 63) / 64) low) (L : Layout) {n : ℕ}
    (hρn : g.natDegree ≤ n) (msg : ℕ → Bool) :
    ∃ c : (ZMod 2)[X], g ∣ c ∧ c.degree < n ∧
      (∀ u < n, toZ (encodeUser g low L n msg u) =
        c.coeff (internalAt L n (n - g.natDegree) u)) ∧
      (∀ u < n - g.natDegree, encodeUser g low L n msg u = msg u) := by
  obtain ⟨c, hdvd, hdeg, hcoeff, -⟩ :=
    BchSystematicEncoding.encodeUser_presents_codeword hg L hρn (fun v => toZ (msg v))
  refine ⟨c, hdvd, hdeg, fun u hu => ?_, fun u hu => ?_⟩
  · rw [toZ_encodeUser g hlow L hρn msg hu, hcoeff u hu]
  · simp only [encodeUser, if_pos hu]

end Packed

/-! ## Section 6 — the encoder over the O-4 generator -/

section Generator

open BchGenerator

variable {E : Type*} [Field E] [Algebra B E] [Finite E] {n : ℕ} [NeZero n] {α : E}

/-- The O-4 generator has degree at most `n`, so the dimension `k = n - ρ` is exact.

Anchor: `assert_construction_is_consistent` (`crates/gf2-coding/src/bch/spec.rs:1929`). -/
theorem natDegree_generator_le (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T) :
    (generator B hq α T).natDegree ≤ n := by
  rw [natDegree_generator B hord hq hclosed]
  exact card_le_length T

/-- **The O-5 obligation over a constructed code.** Encoding under the BCH generator that O-4
constructs writes, under every declared layout, a user codeword that presents a multiple of the
generator of degree below `n`, carries the message in its first `k` coordinates, and vanishes at
`α ^ j` for every exponent `j` of the defining set.

Anchor: `the_generator_vanishes_at_every_defining_set_root`
(`crates/gf2-coding/src/bch/spec.rs:2129`) with the codeword property tests. -/
theorem encodeUser_generator (hord : orderOf α = n) (hq : Nat.Coprime (Nat.card B) n)
    {T : Finset (ZMod n)} (hclosed : ∀ j ∈ T, CyclotomicClosure.mu (Nat.card B) j ∈ T)
    (L : Layout) (msg : ℕ → B) :
    ∃ c : B[X], generator B hq α T ∣ c ∧ c.degree < n ∧
      (∀ u < n, encodeUser (generator B hq α T) L n msg u =
        c.coeff (internalAt L n (n - (generator B hq α T).natDegree) u)) ∧
      (∀ u < n - (generator B hq α T).natDegree, encodeUser (generator B hq α T) L n msg u =
        msg u) ∧
      ∀ j ∈ T, Polynomial.aeval (alphaPow α j) c = 0 := by
  obtain ⟨c, ⟨q, hc⟩, hdeg, hcoeff, hmsg⟩ := encodeUser_presents_codeword
    (generator_monic B hq α T) L (natDegree_generator_le hord hq hclosed) msg
  refine ⟨c, ⟨q, hc⟩, hdeg, hcoeff, hmsg, fun j hj => ?_⟩
  rw [hc, map_mul, aeval_generator_alphaPow B hord hq hclosed hj, zero_mul]

end Generator

end BchSystematicEncoding
