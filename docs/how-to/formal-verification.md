# Run and extend the Lean 4 proofs

`scripts/verify-lean.sh` translates selected `gf2-core` and `gf2-algebra`
functions into Lean 4 with Charon and Aeneas (@/citation/HoProtzenko2022; the
pinned pair is @/citation/AeneasVerif2026). Hand-written theorems under
`proofs/` state specifications over the translated definitions. The extraction
scope is the `--start-from` and `--opaque` flag list of the script's two Charon
invocations.

## Install the toolchain

- Lean and Mathlib: [elan](https://github.com/leanprover/elan) selects the
  toolchain in `proofs/lean-toolchain`; `proofs/lakefile.lean` pins Mathlib.
- Charon, Aeneas and the Rust nightly Charon compiles with: follow the
  `lean-verify` job in `.github/workflows/ci.yml`. It pins each commit and
  toolchain, applies `patches/charon-487f0320-implied-clause.patch` to Charon,
  and installs the OCaml packages Aeneas builds with.
- The Aeneas Lean library: `proofs/lakefile.lean` requires it from a fixed
  local Aeneas checkout path. Either place the checkout there or set
  `AENEAS_LEAN_DIR` to its `backends/lean` directory when running
  `scripts/verify-lean.sh`. That variable rewrites `proofs/lakefile.lean` and
  `proofs/lake-manifest.json` in place; keep those edits out of commits.

## Build the committed proofs

Building needs only Lean, Mathlib and the Aeneas Lean library:

```bash
cd proofs && lake build
```

The `lake-build` quality gate runs `./scripts/lake-build-strict.sh`, which also
fails on a `declaration uses 'sorry'` warning in `proofs/Gf2Core/Proofs/` or
`proofs/Gf2Algebra/Proofs/`. Generated files may carry translation `sorry`s and
are exempt. The default targets are the `Gf2Core` and `Gf2Algebra` libraries,
so a proof module is checked only when `proofs/Gf2Core.lean` or
`proofs/Gf2Algebra.lean` imports it.

## Regenerate the extraction

Run the pipeline after changing Rust code that extraction reaches:

```bash
./scripts/verify-lean.sh
git add proofs/Gf2Core/ proofs/Gf2Algebra/
```

The script runs Charon into `target/charon/`, Aeneas into `proofs/Gf2Core/` and
`proofs/Gf2Algebra/`, the post-processing passes `scripts/fix-aeneas-dupes.py`,
`scripts/fix-aeneas-sorrys.py` and `scripts/fix-aeneas-gf2algebra.py`, and
`lake build`. [`proofs/WORKAROUNDS.md`](../../proofs/WORKAROUNDS.md) states why
each pass exists. Charon compiles with `--cfg verify_lean`, so items under
`#[cfg(not(verify_lean))]` are absent from the extraction.

| File in `proofs/Gf2Core/`, `proofs/Gf2Algebra/` | Owner |
|---|---|
| `Types.lean`, `Funs.lean`, `*_Template.lean`, `TypesExternal.lean` | Regenerated on every run |
| `Gf2Algebra/FunsExternal.lean` | Copied from its template on every run |
| `Gf2Core/FunsExternal.lean` | Hand-maintained; seeded from the template only when absent |
| `Proofs/*.lean` | Hand-written |

CI reruns the pipeline and fails when `proofs/Gf2Core/Proofs/` or
`proofs/Gf2Core/FunsExternal.lean` differ from the committed files afterwards.

## Add a proof for an extracted function

1. Bring the function into scope. Its module must lie under a `--start-from`
   path and outside every `--opaque` path of the matching Charon invocation in
   `scripts/verify-lean.sh`. An `--opaque` module hides its submodules, which is
   why `gf2_core::gf2m` is made opaque submodule by submodule to keep
   `gf2_core::gf2m::mul_raw` translated.
2. Regenerate and find the definition in `Funs.lean`. Its doc comment names the
   Rust path; its Lean name is the crate-relative module path inside namespace
   `gf2_core` or `gf2_algebra`, and it returns `Result`, with a panic mapped to
   `fail`. `gf2_core::gf2m::mul_raw::gf2m_add_raw` translates to:

   ```lean
   def gf2m.mul_raw.gf2m_add_raw
     (a : Std.U64) (b : Std.U64) : Result Std.U64 := do
     ok (a ^^^ b)
   ```

3. State the theorem in a new file under `Proofs/` as success plus a property
   of the result. `proofs/Gf2Core/Proofs/Gf2mAddition.lean` proves:

   ```lean
   theorem gf2m_add_raw_correct (a b : Std.U64) :
       ∃ r, gf2m.mul_raw.gf2m_add_raw a b = ok r ∧
         r.val = add_raw_spec a.val b.val := by
     unfold gf2m.mul_raw.gf2m_add_raw
     refine ⟨_, rfl, ?_⟩
     simp [add_raw_spec, UScalar.val_xor]
   ```

   A function that calls other extracted functions is proved with Aeneas's
   `progress` tactic over `@[progress]` lemmas for its callees, as in
   `Progress.lean`, `ExtProgress.lean` and `Gf2mProgress.lean`. A translated
   loop is proved with a termination measure and an invariant, as
   `gf2m_mul_raw_loop_progress` in `Gf2mProgress.lean` does.
4. Import the module from `proofs/Gf2Core.lean` or `proofs/Gf2Algebra.lean`.
5. A standard-library call that Aeneas cannot translate appears as an `axiom`
   in `FunsExternal.lean`, and the proof trusts it. To prove through the call,
   replace the axiom with a definition in `proofs/Gf2Core/FunsExternal.lean`;
   `Gf2Algebra/FunsExternal.lean` is regenerated and keeps its axioms.
6. Run `./scripts/lake-build-strict.sh` and commit the proof with the
   regenerated files.
