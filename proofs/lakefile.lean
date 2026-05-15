import Lake
open Lake DSL

-- Aeneas standard library (provides `Aeneas`, `Aeneas.Std`, etc.)
require aeneas from "/data/aeneas-build" / "backends" / "lean"

-- Mathlib (also a transitive dep via Aeneas, declared explicitly for clarity)
require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git" @ "v4.30.0-rc2"

package gf2core where
  leanOptions := #[
    ⟨`autoImplicit, false⟩
  ]

@[default_target]
lean_lib Gf2Core where
  srcDir := "."

@[default_target]
lean_lib Gf2Algebra where
  srcDir := "."

-- Strict-build wrapper (issue 2e544a34): plain `lake build` is too permissive
-- because Lean's `warningAsError` is global (catches every warning, including
-- pre-existing project linter noise that's out of scope). Instead, the
-- `lake-build` quality gate invokes `scripts/lake-build-strict.sh`, which
-- runs `lake build` and then greps the captured output for
-- `declaration uses 'sorry'` warnings in hand-written `Proofs/` files.
-- The wrapper's hand-written / generated separation matches the carve-out:
-- Aeneas-emitted `Funs.lean` placeholders are tolerated (they are
-- extraction artefacts, not proof debt); `Proofs/*.lean` sorrys fail.
