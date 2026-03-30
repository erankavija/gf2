# Charon/Aeneas Upstream Sync

**JIT issue**: `afc7bdaf` — Update Charon to 419f53b6 and Aeneas to 1180be60

## Goal

Update the Charon/Aeneas verification toolchain from our diverged fork to track upstream closely, reducing our patch surface and maintenance burden.

| Component | Current | Target |
|-----------|---------|--------|
| Charon | `24a17b5e` (2026-03-06) + 3 patches | `419f53b6` (2026-03-20) + 3 ported patches |
| Aeneas | `1180be60` (already pinned in CI) | `1180be60` (no change) |

Charon `419f53b6` is the commit Aeneas `1180be60` pins in its `flake.lock`, ensuring LLBC format compatibility.

## Completed (Phase 1)

### Charon patches ported to `419f53b6`

All patches are in `expand_associated_types.rs`:

**Patch #1b — `.erase()` in `compute_assoc_tys_for_impl`** (line ~848)
- Upstream PR #1055 fixed 2 of 3 `move_from_under_binder()` sites via `identity_tref()`.
- The third site (`compute_assoc_tys_for_impl`, implied_trait_refs loop) still uses `move_from_under_binder()`. Our fix: `pred.clone().erase()` instead.

**Patch #2b — Bidirectional SelfClause/Local(ZERO) fallback in `lookup_type_replacement`** (line ~902)
- NOT present upstream. When lookup fails, retry with the alternate base clause (`SelfClause` ↔ `Local(Bound(0, 0))`).

**Patch #3 — Implied clause constraint propagation in `compute_trait_modifications`** (line ~755)
- NOT present upstream. After `compute_constraint_set(&tr.generics)`, iterate `tr.implied_clauses` and merge parent trait `type_constraints` via `iter_self_paths()` + `on_tref(&parent_clause(clause_id))`.
- Uses `.erase()` on the implied clause's `trait_` to get the `TraitDeclRef`, then looks up trait modifications by `pred.id`.

### Pipeline status

- Charon extraction: succeeds, 0 "Could not compute" warnings, 13 benign "Type error after transformations"
- Aeneas translation: produces Types.lean, Funs.lean, FunsExternal_Template.lean (partial — 16 gfpn function body errors, same as before)
- `verify-lean.sh` updated to tolerate Aeneas exit code 1 when output files are present

## Reproducing Phase 1

The new patch file is checked in at `patches/charon-419f53b6-assoc-type-fixes.patch`. To rebuild the toolchain from scratch:

```bash
# Charon
git clone https://github.com/AeneasVerif/charon.git /tmp/charon
cd /tmp/charon && git checkout 419f53b6
git apply /path/to/gf2/patches/charon-419f53b6-assoc-type-fixes.patch
cd charon && RUSTUP_TOOLCHAIN=nightly-2026-02-07 cargo install --path . --locked

# Aeneas (needs opam with OCaml 5.2.1)
git clone https://github.com/AeneasVerif/aeneas.git /tmp/aeneas
cd /tmp/aeneas && git checkout 1180be60c7a0
eval $(opam env) && make setup-charon && make build-dev
cp bin/aeneas ~/.cargo/bin/

# Run pipeline
AENEAS_LEAN_DIR=/tmp/aeneas/backends/lean ./scripts/verify-lean.sh
```

## Remaining (Phase 2)

### 1. Fix Lean proof breakage

`lake build` fails in 3 files. The root cause is that the new Aeneas generates slightly different names/structures in `Types.lean` and `Funs.lean`:
- `Proofs/Defs.lean` — likely references renamed struct fields or changed type signatures
- `Proofs/ExtProgress.lean` — progress lemmas for gfpn functions whose generated bodies changed
- `Proofs/Gf2mProgress.lean` — progress lemmas for gf2m functions

**Diagnosis approach**:
```bash
diff proofs/Gf2Core/Types.lean proofs/Gf2Core.bak/Types.lean
diff proofs/Gf2Core/Funs.lean proofs/Gf2Core.bak/Funs.lean
diff proofs/Gf2Core/FunsExternal_Template.lean proofs/Gf2Core.bak/FunsExternal_Template.lean
```

**Fix order** (import chain):
1. `Types.lean` → `FunsExternal.lean` (check template signature changes) → `Funs.lean`
2. `Proofs/Defs.lean` → `Progress.lean` → `MontgomeryRoundtrip.lean` → `FpField.lean`
3. `Proofs/ExtDefs.lean` → `ExtProgress.lean` → `QuadraticExtField.lean` → `CubicExtField.lean`
4. `Proofs/Gf2mDefs.lean` → `Gf2mProgress.lean` → `Gf2mInverse.lean`

### 2. Swap patch files

The new patch is at `patches/charon-419f53b6-assoc-type-fixes.patch`. Delete old `patches/charon-hrtb-assoc-types.patch`.

### 3. Update CI

In `.github/workflows/ci.yml`:
- Charon commit: `24a17b5e` → `419f53b6`
- Cache key: update to reference new patch file
- Remove `git apply` if patch file name changes
- Aeneas stays at `1180be60`

### 4. Update docs

- `proofs/WORKAROUNDS.md`: update Charon/Aeneas versions, patch descriptions
- `proofs/README.md`: update base revision
- `scripts/verify-lean.sh` line 9: update comment

### 5. Verification

```bash
./scripts/verify-lean.sh          # full pipeline passes
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Local build locations

- Charon: `/data/aeneas-build/charon/` (checked out at `419f53b6` with patches applied in working tree)
- Aeneas: `/data/aeneas-build/` (checked out at `1180be60`)
- Backup of current generated Lean: `proofs/Gf2Core.bak/`

## Risk

Highest risk is proof breakage. If generated function bodies changed shape, `progress` lemma proofs may need line-by-line rewrites. The `ExtDefs.lean` abbreviation layer insulates most proofs from name changes.
