import Lake
open Lake DSL

-- Aeneas standard library (provides `Aeneas`, `Aeneas.Std`, etc.)
require aeneas from "/data/aeneas-build" / "backends" / "lean"

-- Mathlib (also a transitive dep via Aeneas, declared explicitly for clarity)
require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git" @ "v4.28.0-rc1"

package gf2core where
  leanOptions := #[
    ⟨`autoImplicit, false⟩
  ]

@[default_target]
lean_lib Gf2Core where
  srcDir := "."
