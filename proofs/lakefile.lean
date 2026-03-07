import Lake
open Lake DSL

-- Aeneas standard library (provides `Aeneas`, `Aeneas.Std`, etc.)
require aeneas from git
  "https://github.com/AeneasVerif/aeneas.git" @ "c23de9324fbe4d3630fc532e5216a0568b9beb5c" / "backends" / "lean"

package gf2core where
  leanOptions := #[
    ⟨`autoImplicit, false⟩
  ]

@[default_target]
lean_lib Gf2Core where
  srcDir := "."
