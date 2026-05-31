# Charon/Aeneas/Lean toolchain upgrade — migration plan

Issue: `150d7d79` — Upgrade Charon/Aeneas/Lean verification toolchain to latest compatible upstream.

## Target pair

- Aeneas: `0f99a049` (`main`, tag `nightly-2026.05.30`) — repo root `/data/aeneas-build`.
- Charon: `e069223a` — the commit declared in `/data/aeneas-build/charon-pin` by Aeneas `main`. Repo at `/data/aeneas-build/charon`.
- rustc to build Charon: `nightly-2026-02-22` (components `rustc-dev, llvm-tools-preview, rust-src, miri`).
- Lean / Mathlib: `v4.30.0-rc2` (unchanged).

Do NOT advance Charon to its own `main` HEAD (`1a639c02`); it is 23 commits beyond what Aeneas pins. The halves must move together to `e069223a`.

## Project-local Charon patches (carry forward)

All backed up at `dev/active/charon-patch-backup-2026-05-15/`:

1. HRTB-erase (3 sites) — `expand_associated_types.rs`
2. SelfClause / Local(0,0) fallback in `lookup_type_replacement` — `expand_associated_types.rs`
3. implied-clause constraint propagation — `expand_associated_types.rs`
4. obsolete-asserts removal in `DynPredicate::fmt_with_ctx` — `pretty/fmt_with_ctx.rs`

Plus 3 untracked UI test files: `hrtb-associated-types.rs`, `trait-default-method-assoc-type.rs`, `implied-clause-assoc-type-constraint.rs`.

Pre-verified 2026-05-31: the working-tree diff of patches 1-4 applies cleanly to `e069223a`. `expand_associated_types.rs` is untouched upstream over the 94-commit range; `fmt_with_ctx.rs` saw 9 upstream commits but none on the patched lines (assert block still present at target lines 422/425).

## Procedure

1. Snapshot `/data/aeneas-build` (both repos) and `proofs/` before touching anything.
2. `rustup` install `nightly-2026-02-22` + the 4 components.
3. Charon: `git checkout e069223a`; reapply the 2-file diff; restore the 3 UI test files; `cargo build`; reinstall the `charon` binary (`~/.cargo/bin/charon`).
4. Aeneas: `git checkout 0f99a049`; `make` (OCaml); reinstall `~/.cargo/bin/aeneas`. Confirm `charon-pin` now reads `e069223a`.
5. Rebuild the Lean backend (`/data/aeneas-build/backends/lean`); refresh `proofs/lake-manifest.json` if needed.
6. Run `./scripts/verify-lean.sh`. Re-extraction regenerates `Funs.lean`/`Types.lean`; fix whatever hand-written proofs break.
7. Re-evaluate each Charon patch: if upstream now fixes the behaviour, drop it; else keep. Update backup dir + script header to the final set.
8. Update banners/docs: `verify-lean.sh` header lines 8-9 + 14-23 + comments 236-237; CLAUDE.md; auto-memory Charon/Aeneas notes.

## Known risk

Aeneas Lean backend churn (+1829 / −475, 25 files) between `5fc8fdf2` and `0f99a049`: `Std/WP.lean`, `Std/Scalar/Core.lean`, `Std/Scalar/OverflowingOps/*`, `Tactic/Setup.lean`, `Tactic/Step/Step.lean`, `SimpScalar`/`SimpLists`. Likely breakage in `proofs/Gf2Core/Proofs/MontgomeryRoundtrip.lean` wrapping-op lemma chains and possibly `FunsExternal.lean` hand defs. Fix in-place; never weaken a theorem or add a sorry to make the build pass.

## Migration result (2026-05-31, completed)

- **Final pair:** Aeneas `0f99a049` (tag nightly-2026.05.30) + Charon `e069223a`. `charon-pin` reads `e069223a` and matches the installed Charon. rustc `nightly-2026-02-22` installed with `rustc-dev, llvm-tools-preview, rust-src, miri`. Lean/Mathlib unchanged at `v4.30.0-rc2`.
- **Patches surviving:** all 4 Charon patches still required and applied cleanly at `e069223a` (none made redundant by upstream). 3 untracked UI test files restored. Rebased diff saved at `dev/active/charon-patch-backup-2026-05-15/expand_and_fmt-rebased-150d7d79.patch`.
- **Extraction-pipeline fixes (3):**
  1. `verify-lean.sh`: opaqued the three `gfp::specialized::batch_*_mersenne31` functions (Aeneas dropped the slice `.zip` iterator instances). Non-proof-target batch helpers; minor coverage delta noted.
  2. `fix-aeneas-dupes.py`: added `PLE_PANEL_COLS` (new trait const, default `Self::PLE_BASE_COLS` = 1) to the const-default inline table.
  3. `proofs/Gf2Core/FunsExternal.lean`: removed the obsolete hand-written `overflowing_sub` override (now native-pure in Aeneas Std, consumed via `lift`).
- **Hand-proof files that needed fixes (4):** `Gf2Core/Proofs/Specialized.lean` (2 `overflowing_sub`-tail lemmas), `Gf2Core/Proofs/Progress.lean` (retyped `overflowing_sub_val` to the pure equation; ported `redc/mont_add/mont_sub_progress`), `Gf2Core/Proofs/MontgomeryRoundtrip.lean` (`redc_value_spec`, `mont_add_value_spec`), `Gf2Algebra/Proofs/Packed7Correctness.lean` (array safe-vs-panic index bridge via `getElem!_pos`). Root cause: Aeneas 0f99a049 made `overflowing_sub`/`wrapping_*` pure (consumed via `lift`) and `Array.index_usize`/`step` now yields the proof-carrying `[i]` form. No theorem weakened; zero new `sorry`.
- **Verification:** `./scripts/verify-lean.sh` → "All steps passed"; strict `lake-build` → "no sorry warnings in hand-written Proofs/ files" (2179 jobs).
- **Known cosmetic follow-up (not blocking):** Aeneas renamed the `progress` tactic to `step`; existing `progress` uses (e.g. `Gf2Core/Proofs/Gf2mInverse.lean`) emit deprecation warnings. Non-fatal; a future cleanup can migrate `progress` → `step`.

## Aeneas-bundled agent skills (discovered during this migration)

The Aeneas checkout ships agent skill files at `/data/aeneas-build/documentation/skills/`
(symlinked to `.claude/skills/` and `.github/instructions/`). Directly relevant to this
project's verification work: `aeneas-crypto-verification` (Montgomery/modular-arithmetic
proof strategies), `aeneas-lean-core` (translation model, pure-vs-Result, `lift`, pitfalls),
`aeneas-tactics-quickref` (`step` vs `progress`, banned tactics), `lean-lsp-mcp`
(interactive proof MCP — faster than blind `lake build` cycles), `proof-patterns`,
`launching-proof-agents` / `agent-fleet-management` / `verification-campaigns`. These were
upgraded alongside the binaries and document the exact new-Std API that broke the proofs.
Candidate integration: wire the relevant skill files into the gf2 repo and/or stand up the
lean-lsp-mcp server for future verification work (pending user approval — changes project config).
