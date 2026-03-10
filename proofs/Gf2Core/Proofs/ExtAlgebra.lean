/-
  Gf2Core.Proofs.ExtAlgebra — Pure algebraic proofs for extension fields

  Defines QExt F β (pairs representing F[u]/(u²=β)) and CExt F β (triples
  representing F[v]/(v³=β)), proves CommRing and Field instances.
  No Aeneas code — purely mathematical.
-/
import Mathlib.Algebra.Field.Basic
import Mathlib.Tactic.Ring
import Mathlib.Tactic.FieldSimp

set_option maxHeartbeats 800000

attribute [local instance] Classical.propDecidable

/-! ## QExt: Pure quadratic extension type -/

/-- Quadratic extension F[u]/(u²=β), represented as pairs (c0, c1) meaning c0 + c1·u -/
structure QExt (F : Type*) [Field F] (β : F) where
  c0 : F
  c1 : F
  deriving DecidableEq

namespace QExt

variable {F : Type*} [Field F] {β : F}

instance : Inhabited (QExt F β) := ⟨⟨0, 0⟩⟩

/-! ### Basic operations -/

def zero : QExt F β := ⟨0, 0⟩
def one : QExt F β := ⟨1, 0⟩

def add (a b : QExt F β) : QExt F β :=
  ⟨a.c0 + b.c0, a.c1 + b.c1⟩

def neg (a : QExt F β) : QExt F β :=
  ⟨-a.c0, -a.c1⟩

def sub (a b : QExt F β) : QExt F β :=
  ⟨a.c0 - b.c0, a.c1 - b.c1⟩

/-- Schoolbook multiplication: (a0+a1·u)(b0+b1·u) = (a0·b0 + β·a1·b1) + (a0·b1 + a1·b0)·u -/
def mul (a b : QExt F β) : QExt F β :=
  ⟨a.c0 * b.c0 + β * (a.c1 * b.c1), a.c0 * b.c1 + a.c1 * b.c0⟩

/-- Norm: c0² - β·c1² -/
def norm (a : QExt F β) : F :=
  a.c0 ^ 2 - β * a.c1 ^ 2

/-- Inverse using conjugate/norm formula -/
noncomputable def inv' (a : QExt F β) : QExt F β :=
  if a.norm = 0 then zero
  else ⟨a.c0 * a.norm⁻¹, -(a.c1 * a.norm⁻¹)⟩

/-! ### Karatsuba equivalence -/

/-- The Karatsuba trick computes the same result as schoolbook multiplication. -/
theorem karatsuba_eq_schoolbook (a0 a1 b0 b1 β : F) :
    let v0 := a0 * b0; let v1 := a1 * b1
    (v0 + β * v1 = a0 * b0 + β * (a1 * b1)) ∧
    ((a0 + a1) * (b0 + b1) - v0 - v1 = a0 * b1 + a1 * b0) := by
  constructor <;> ring

/-! ### Algebraic instances -/

@[ext]
theorem ext (a b : QExt F β) (h0 : a.c0 = b.c0) (h1 : a.c1 = b.c1) : a = b := by
  cases a; cases b; simp_all

instance : Zero (QExt F β) := ⟨zero⟩
instance : One (QExt F β) := ⟨one⟩
instance : Add (QExt F β) := ⟨add⟩
instance : Neg (QExt F β) := ⟨neg⟩
instance : Sub (QExt F β) := ⟨sub⟩
instance : Mul (QExt F β) := ⟨mul⟩

@[simp] theorem zero_c0 : (0 : QExt F β).c0 = 0 := rfl
@[simp] theorem zero_c1 : (0 : QExt F β).c1 = 0 := rfl
@[simp] theorem one_c0 : (1 : QExt F β).c0 = 1 := rfl
@[simp] theorem one_c1 : (1 : QExt F β).c1 = 0 := rfl
@[simp] theorem add_c0 (a b : QExt F β) : (a + b).c0 = a.c0 + b.c0 := rfl
@[simp] theorem add_c1 (a b : QExt F β) : (a + b).c1 = a.c1 + b.c1 := rfl
@[simp] theorem neg_c0 (a : QExt F β) : (-a).c0 = -a.c0 := rfl
@[simp] theorem neg_c1 (a : QExt F β) : (-a).c1 = -a.c1 := rfl
@[simp] theorem sub_c0 (a b : QExt F β) : (a - b).c0 = a.c0 - b.c0 := rfl
@[simp] theorem sub_c1 (a b : QExt F β) : (a - b).c1 = a.c1 - b.c1 := rfl
@[simp] theorem mul_c0 (a b : QExt F β) : (a * b).c0 = a.c0 * b.c0 + β * (a.c1 * b.c1) := rfl
@[simp] theorem mul_c1 (a b : QExt F β) : (a * b).c1 = a.c0 * b.c1 + a.c1 * b.c0 := rfl

/-! ### Cast instances -/

instance instNatCast : NatCast (QExt F β) := ⟨fun n => ⟨n, 0⟩⟩
instance instIntCast : IntCast (QExt F β) := ⟨fun n => ⟨n, 0⟩⟩

@[simp] theorem natCast_c0 (n : ℕ) : (↑n : QExt F β).c0 = ↑n := rfl
@[simp] theorem natCast_c1 (n : ℕ) : (↑n : QExt F β).c1 = 0 := rfl
@[simp] theorem intCast_c0' (n : ℤ) : (↑n : QExt F β).c0 = ↑n := rfl
@[simp] theorem intCast_c1' (n : ℤ) : (↑n : QExt F β).c1 = 0 := rfl

/-! ### CommRing instance -/

instance instCommRing : CommRing (QExt F β) where
  add_assoc a b c := by ext <;> simp <;> ring
  zero_add a := by ext <;> simp
  add_zero a := by ext <;> simp
  add_comm a b := by ext <;> simp <;> ring
  mul_assoc a b c := by ext <;> simp <;> ring
  one_mul a := by ext <;> simp <;> ring
  mul_one a := by ext <;> simp <;> ring
  mul_comm a b := by ext <;> simp <;> ring
  left_distrib a b c := by ext <;> simp <;> ring
  right_distrib a b c := by ext <;> simp <;> ring
  zero_mul a := by ext <;> simp <;> ring
  mul_zero a := by ext <;> simp <;> ring
  neg_add_cancel a := by ext <;> simp
  sub_eq_add_neg a b := by ext <;> simp <;> ring
  nsmul := nsmulRec
  zsmul := zsmulRec
  natCast_zero := by ext <;> simp
  natCast_succ n := by ext <;> simp <;> ring
  intCast_ofNat n := by ext <;> simp [Int.cast_natCast]
  intCast_negSucc n := by ext <;> simp [Int.cast_negSucc, add_comm]

/-! ### Field instance -/

/-- The norm form a² - β·b² is anisotropic iff every nonzero element has nonzero norm -/
theorem norm_ne_zero_of_ne_zero
    (hirr : ∀ a b : F, a ^ 2 - β * b ^ 2 = 0 → a = 0 ∧ b = 0)
    (a : QExt F β) (ha : a ≠ 0) : a.norm ≠ 0 := by
  intro hn
  have := hirr a.c0 a.c1 hn
  exact ha (QExt.ext _ _ this.1 this.2)

/-- Inverse times original equals one (given irreducibility) -/
theorem inv_mul_cancel
    (hirr : ∀ a b : F, a ^ 2 - β * b ^ 2 = 0 → a = 0 ∧ b = 0)
    (a : QExt F β) (ha : a ≠ 0) : a.inv' * a = 1 := by
  have hn : a.norm ≠ 0 := norm_ne_zero_of_ne_zero hirr a ha
  simp only [inv', hn, ite_false]
  ext
  · -- c0: a.c0 * norm⁻¹ * a.c0 + β * (-(a.c1 * norm⁻¹) * a.c1) = 1
    simp only [mul_c0, one_c0]
    rw [show a.c0 * a.norm⁻¹ * a.c0 + β * (-(a.c1 * a.norm⁻¹) * a.c1)
        = (a.c0 ^ 2 - β * a.c1 ^ 2) * a.norm⁻¹ from by ring]
    exact mul_inv_cancel₀ hn
  · -- c1: a.c0 * norm⁻¹ * a.c1 + -(a.c1 * norm⁻¹) * a.c0 = 0
    simp only [mul_c1, one_c1]
    ring

/-- QExt F β is a field when β makes the norm anisotropic -/
noncomputable instance instField
    (hirr : ∀ a b : F, a ^ 2 - β * b ^ 2 = 0 → a = 0 ∧ b = 0) :
    Field (QExt F β) where
  inv a := a.inv'
  exists_pair_ne := ⟨⟨0, 0⟩, ⟨1, 0⟩, by simp [QExt.ext_iff]⟩
  mul_inv_cancel a ha := by
    have : a.inv' * a = 1 := inv_mul_cancel hirr a ha
    rwa [mul_comm] at this
  inv_zero := by
    show QExt.inv' (0 : QExt F β) = 0
    simp [inv', norm]
    rfl
  nnqsmul := _
  qsmul := _

end QExt

/-! ## CExt: Pure cubic extension type -/

/-- Cubic extension F[v]/(v³=β), represented as triples (c0, c1, c2) meaning c0 + c1·v + c2·v² -/
structure CExt (F : Type*) [Field F] (β : F) where
  c0 : F
  c1 : F
  c2 : F
  deriving DecidableEq

namespace CExt

variable {F : Type*} [Field F] {β : F}

instance : Inhabited (CExt F β) := ⟨⟨0, 0, 0⟩⟩

def zero : CExt F β := ⟨0, 0, 0⟩
def one : CExt F β := ⟨1, 0, 0⟩

def add (a b : CExt F β) : CExt F β :=
  ⟨a.c0 + b.c0, a.c1 + b.c1, a.c2 + b.c2⟩

def neg (a : CExt F β) : CExt F β :=
  ⟨-a.c0, -a.c1, -a.c2⟩

def sub (a b : CExt F β) : CExt F β :=
  ⟨a.c0 - b.c0, a.c1 - b.c1, a.c2 - b.c2⟩

/-- Schoolbook cubic multiplication in F[v]/(v³=β) -/
def mul (a b : CExt F β) : CExt F β :=
  ⟨a.c0 * b.c0 + β * (a.c1 * b.c2 + a.c2 * b.c1),
   a.c0 * b.c1 + a.c1 * b.c0 + β * (a.c2 * b.c2),
   a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0⟩

/-- Cofactors for the cubic norm/inverse computation -/
def cofactor_s0 (a : CExt F β) : F := a.c0 ^ 2 - β * (a.c1 * a.c2)
def cofactor_s1 (a : CExt F β) : F := β * a.c2 ^ 2 - a.c0 * a.c1
def cofactor_s2 (a : CExt F β) : F := a.c1 ^ 2 - a.c0 * a.c2

/-- Cubic norm: c0·s0 + β·(c2·s1 + c1·s2) -/
def norm (a : CExt F β) : F :=
  a.c0 * a.cofactor_s0 + β * (a.c2 * a.cofactor_s1 + a.c1 * a.cofactor_s2)

/-- Inverse: cofactors scaled by norm⁻¹ -/
noncomputable def inv' (a : CExt F β) : CExt F β :=
  if a.norm = 0 then zero
  else ⟨a.cofactor_s0 * a.norm⁻¹, a.cofactor_s1 * a.norm⁻¹, a.cofactor_s2 * a.norm⁻¹⟩

@[ext]
theorem ext (a b : CExt F β) (h0 : a.c0 = b.c0) (h1 : a.c1 = b.c1) (h2 : a.c2 = b.c2) :
    a = b := by
  cases a; cases b; simp_all

instance : Zero (CExt F β) := ⟨zero⟩
instance : One (CExt F β) := ⟨one⟩
instance : Add (CExt F β) := ⟨add⟩
instance : Neg (CExt F β) := ⟨neg⟩
instance : Sub (CExt F β) := ⟨sub⟩
instance : Mul (CExt F β) := ⟨mul⟩

@[simp] theorem zero_c0 : (0 : CExt F β).c0 = 0 := rfl
@[simp] theorem zero_c1 : (0 : CExt F β).c1 = 0 := rfl
@[simp] theorem zero_c2 : (0 : CExt F β).c2 = 0 := rfl
@[simp] theorem one_c0 : (1 : CExt F β).c0 = 1 := rfl
@[simp] theorem one_c1 : (1 : CExt F β).c1 = 0 := rfl
@[simp] theorem one_c2 : (1 : CExt F β).c2 = 0 := rfl
@[simp] theorem add_c0 (a b : CExt F β) : (a + b).c0 = a.c0 + b.c0 := rfl
@[simp] theorem add_c1 (a b : CExt F β) : (a + b).c1 = a.c1 + b.c1 := rfl
@[simp] theorem add_c2 (a b : CExt F β) : (a + b).c2 = a.c2 + b.c2 := rfl
@[simp] theorem neg_c0 (a : CExt F β) : (-a).c0 = -a.c0 := rfl
@[simp] theorem neg_c1 (a : CExt F β) : (-a).c1 = -a.c1 := rfl
@[simp] theorem neg_c2 (a : CExt F β) : (-a).c2 = -a.c2 := rfl
@[simp] theorem sub_c0 (a b : CExt F β) : (a - b).c0 = a.c0 - b.c0 := rfl
@[simp] theorem sub_c1 (a b : CExt F β) : (a - b).c1 = a.c1 - b.c1 := rfl
@[simp] theorem sub_c2 (a b : CExt F β) : (a - b).c2 = a.c2 - b.c2 := rfl
@[simp] theorem mul_c0 (a b : CExt F β) :
    (a * b).c0 = a.c0 * b.c0 + β * (a.c1 * b.c2 + a.c2 * b.c1) := rfl
@[simp] theorem mul_c1 (a b : CExt F β) :
    (a * b).c1 = a.c0 * b.c1 + a.c1 * b.c0 + β * (a.c2 * b.c2) := rfl
@[simp] theorem mul_c2 (a b : CExt F β) :
    (a * b).c2 = a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0 := rfl

/-! ### Karatsuba equivalence for cubic (6-mul trick) -/

/-- The Karatsuba/Toom-Cook 6-mul trick for cubic extension produces the same
    result as schoolbook multiplication. -/
theorem cubic_karatsuba_eq_schoolbook (a0 a1 a2 b0 b1 b2 β : F) :
    let v0 := a0 * b0; let v1 := a1 * b1; let v2 := a2 * b2
    let x := (a1 + a2) * (b1 + b2) - v1 - v2  -- cross term for c0
    let y := (a0 + a1) * (b0 + b1) - v0 - v1  -- cross term for c1
    let z := (a0 + a2) * (b0 + b2) - v0 + v1 - v2  -- middle for c2
    (v0 + β * x = a0 * b0 + β * (a1 * b2 + a2 * b1)) ∧
    (y + β * v2 = a0 * b1 + a1 * b0 + β * (a2 * b2)) ∧
    (z = a0 * b2 + a1 * b1 + a2 * b0) := by
  refine ⟨?_, ?_, ?_⟩ <;> ring

/-! ### Cast instances (defined separately so projections reduce in CommRing proofs) -/

instance instNatCast : NatCast (CExt F β) := ⟨fun n => ⟨n, 0, 0⟩⟩
instance instIntCast : IntCast (CExt F β) := ⟨fun n => ⟨n, 0, 0⟩⟩

@[simp] theorem natCast_c0 (n : ℕ) : (↑n : CExt F β).c0 = ↑n := rfl
@[simp] theorem natCast_c1 (n : ℕ) : (↑n : CExt F β).c1 = 0 := rfl
@[simp] theorem natCast_c2 (n : ℕ) : (↑n : CExt F β).c2 = 0 := rfl
@[simp] theorem intCast_c0' (n : ℤ) : (↑n : CExt F β).c0 = ↑n := rfl
@[simp] theorem intCast_c1' (n : ℤ) : (↑n : CExt F β).c1 = 0 := rfl
@[simp] theorem intCast_c2' (n : ℤ) : (↑n : CExt F β).c2 = 0 := rfl

/-! ### CommRing instance for CExt -/

instance instCommRing : CommRing (CExt F β) where
  add_assoc a b c := by ext <;> simp <;> ring
  zero_add a := by ext <;> simp
  add_zero a := by ext <;> simp
  add_comm a b := by ext <;> simp <;> ring
  mul_assoc a b c := by ext <;> simp <;> ring
  one_mul a := by ext <;> simp <;> ring
  mul_one a := by ext <;> simp <;> ring
  mul_comm a b := by ext <;> simp <;> ring
  left_distrib a b c := by ext <;> simp <;> ring
  right_distrib a b c := by ext <;> simp <;> ring
  zero_mul a := by ext <;> simp <;> ring
  mul_zero a := by ext <;> simp <;> ring
  neg_add_cancel a := by ext <;> simp
  sub_eq_add_neg a b := by ext <;> simp <;> ring
  nsmul := nsmulRec
  zsmul := zsmulRec
  natCast_zero := by ext <;> simp
  natCast_succ n := by ext <;> simp <;> ring
  intCast_ofNat n := by ext <;> simp [Int.cast_natCast]
  intCast_negSucc n := by ext <;> simp [Int.cast_negSucc, add_comm]

/-! ### Cubic norm identities -/

/-- The norm equals the cubic norm form a₀³ + β·a₁³ + β²·a₂³ - 3β·a₀·a₁·a₂ -/
theorem norm_eq_cubic_form (a : CExt F β) :
    a.norm = a.c0 ^ 3 + β * a.c1 ^ 3 + β ^ 2 * a.c2 ^ 3 - 3 * β * a.c0 * a.c1 * a.c2 := by
  simp [norm, cofactor_s0, cofactor_s1, cofactor_s2]; ring

/-- Nonzero elements have nonzero norm (from irreducibility) -/
theorem norm_ne_zero_of_ne_zero
    (hirr : ∀ a b c : F,
      a ^ 3 + β * b ^ 3 + β ^ 2 * c ^ 3 - 3 * β * a * b * c = 0 →
      a = 0 ∧ b = 0 ∧ c = 0)
    (a : CExt F β) (ha : a ≠ 0) : a.norm ≠ 0 := by
  intro hn
  rw [norm_eq_cubic_form] at hn
  have := hirr a.c0 a.c1 a.c2 hn
  exact ha (CExt.ext _ _ this.1 this.2.1 this.2.2)

/-- Orthogonality: s0·a1 + s1·a0 + β·s2·a2 = 0 -/
private theorem ortho1 (a : CExt F β) :
    a.cofactor_s0 * a.c1 + a.cofactor_s1 * a.c0 + β * (a.cofactor_s2 * a.c2) = 0 := by
  simp [cofactor_s0, cofactor_s1, cofactor_s2]; ring

/-- Orthogonality: s0·a2 + s1·a1 + s2·a0 = 0 -/
private theorem ortho2 (a : CExt F β) :
    a.cofactor_s0 * a.c2 + a.cofactor_s1 * a.c1 + a.cofactor_s2 * a.c0 = 0 := by
  simp [cofactor_s0, cofactor_s1, cofactor_s2]; ring

/-- Key identity: inv' * a = 1 when norm ≠ 0 -/
theorem inv_mul_cancel
    (hirr : ∀ a b c : F,
      a ^ 3 + β * b ^ 3 + β ^ 2 * c ^ 3 - 3 * β * a * b * c = 0 →
      a = 0 ∧ b = 0 ∧ c = 0)
    (a : CExt F β) (ha : a ≠ 0) : a.inv' * a = 1 := by
  have hn : a.norm ≠ 0 := norm_ne_zero_of_ne_zero hirr a ha
  unfold inv'
  rw [if_neg hn]
  have hnorm_def : a.norm = a.c0 * a.cofactor_s0 +
      β * (a.c2 * a.cofactor_s1 + a.c1 * a.cofactor_s2) := rfl
  have h1 := ortho1 (β := β) a
  have h2 := ortho2 (β := β) a
  ext
  · -- c0
    simp only [mul_c0, one_c0]
    rw [show a.cofactor_s0 * a.norm⁻¹ * a.c0 +
        β * (a.cofactor_s1 * a.norm⁻¹ * a.c2 + a.cofactor_s2 * a.norm⁻¹ * a.c1)
        = (a.c0 * a.cofactor_s0 + β * (a.c2 * a.cofactor_s1 + a.c1 * a.cofactor_s2)) * a.norm⁻¹
        from by ring]
    rw [← hnorm_def, mul_inv_cancel₀ hn]
  · -- c1
    simp only [mul_c1, one_c1]
    rw [show a.cofactor_s0 * a.norm⁻¹ * a.c1 +
        a.cofactor_s1 * a.norm⁻¹ * a.c0 +
        β * (a.cofactor_s2 * a.norm⁻¹ * a.c2)
        = (a.cofactor_s0 * a.c1 + a.cofactor_s1 * a.c0 + β * (a.cofactor_s2 * a.c2)) * a.norm⁻¹
        from by ring]
    rw [h1, zero_mul]
  · -- c2
    simp only [mul_c2, one_c2]
    rw [show a.cofactor_s0 * a.norm⁻¹ * a.c2 +
        a.cofactor_s1 * a.norm⁻¹ * a.c1 +
        a.cofactor_s2 * a.norm⁻¹ * a.c0
        = (a.cofactor_s0 * a.c2 + a.cofactor_s1 * a.c1 + a.cofactor_s2 * a.c0) * a.norm⁻¹
        from by ring]
    rw [h2, zero_mul]

/-- CExt F β is a field when the cubic norm is anisotropic -/
noncomputable instance instField
    (hirr : ∀ a b c : F,
      a ^ 3 + β * b ^ 3 + β ^ 2 * c ^ 3 - 3 * β * a * b * c = 0 →
      a = 0 ∧ b = 0 ∧ c = 0) :
    Field (CExt F β) where
  inv a := a.inv'
  exists_pair_ne := ⟨⟨0, 0, 0⟩, ⟨1, 0, 0⟩, by simp [CExt.ext_iff]⟩
  mul_inv_cancel a ha := by
    have : a.inv' * a = 1 := inv_mul_cancel hirr a ha
    rwa [mul_comm] at this
  inv_zero := by
    show CExt.inv' (0 : CExt F β) = 0
    simp [inv', norm, cofactor_s0, cofactor_s1, cofactor_s2]
    rfl
  nnqsmul := _
  qsmul := _

end CExt
