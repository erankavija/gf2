import Aeneas

open Aeneas Aeneas.Std Result Aeneas.Std.WP

/-- Total correctness for Rust's failure-only effects yields an actual return
value, allowing the field models to recover pure operations. -/
theorem result_spec_exists {α : Type} {m : Result α} {P : α → Prop}
    (h : spec m P) : ∃ y, m = ok y ∧ P y := by
  cases m using Result.cases with
  | ret y => exact ⟨y, rfl, spec_ok y |>.mp h⟩
  | vis e k => exact False.elim ((spec_vis e k).mp h)
  | div => exact False.elim (spec_div.mp h)
