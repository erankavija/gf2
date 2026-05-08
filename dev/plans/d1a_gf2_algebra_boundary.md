# D1a — gf2-algebra crate boundary decision

**Issue:** `6e20133d` (W0 / D1a)
**Epic:** `epic:gf2-algebra-permanent` (input: `dev/plans/gf2_algebra_permanent.md`)
**Status:** decision

## 1. Scope and inputs

This document settles the workspace location of every public type the gf2-algebra-permanent epic introduces in §§5–11 of the epic design doc, and fixes the inter-crate dependency edges. Inputs:

- Project conventions: `CLAUDE.md` (workspace map, unsafe-isolation rule, MSRV 1.95).
- Existing crate roots verified by reading `crates/gf2-core/src/lib.rs`, `crates/gf2-coding/src/lib.rs`, `crates/gf2-kernels-simd/src/lib.rs`, `crates/gf2-kernels-hip/src/lib.rs`.
- Workspace topology: `Cargo.toml` lists `gf2-core`, `gf2-coding`, `gf2-kernels-simd` as default members; `gf2-kernels-hip` is in `exclude` because hipcc/ROCm is not always present.

The epic introduces five clusters of public types: (a) the packed-field abstraction and its concrete `Bipedal{3,5,7}` implementations; (b) generic and specialised `permanent_*` algorithms; (c) Gray-code subset enumeration; (d) parallel and GPU dispatch glue; (e) SIMD and GPU device kernels backing the dispatch layer.

## 2. Public-type → crate assignment

Every type named in §§5–11 of the epic doc is listed below; each appears in exactly one crate.

| Type or item | Source § | Home crate | Module path |
|---|---|---|---|
| `PackedField<F>` trait | §6 | `gf2-algebra` | `packed::PackedField` |
| `PackedFieldVec<F>` trait | §6 | `gf2-algebra` | `packed::PackedFieldVec` |
| `Bipedal3` element | §7.1 | `gf2-algebra` | `packed::bipedal3::Bipedal3` |
| `Bipedal3Vec` | §7.2 | `gf2-algebra` | `packed::bipedal3::Bipedal3Vec` |
| `Bipedal3Matrix` | §7.2 | `gf2-algebra` | `packed::bipedal3::Bipedal3Matrix` |
| `Fp3Accumulator` | §7.3 | `gf2-algebra` | `packed::bipedal3::Fp3Accumulator` |
| `Bipedal5` element / `Vec` / `Matrix` | §8 | `gf2-algebra` | `packed::bipedal5::*` |
| `Bipedal7` element / `Vec` / `Matrix` | §8 | `gf2-algebra` | `packed::bipedal7::*` |
| `Permanent<F>` trait | §6 | `gf2-algebra` | `permanent::Permanent` |
| `permanent_ryser<F>` | §6, App. A | `gf2-algebra` | `permanent::ryser::permanent_ryser` |
| `permanent_mod3_reference` | §16 | `gf2-algebra` | `permanent::reference::permanent_mod3_reference` |
| `permanent_bipedal3` (single-word) | §7.3 | `gf2-algebra` | `permanent::bipedal3::permanent_bipedal3_single` |
| `permanent_bipedal3` (multi-word) | §9 | `gf2-algebra` | `permanent::bipedal3::permanent_bipedal3_multi` |
| `permanent_bipedal5` | §6 | `gf2-algebra` | `permanent::bipedal5::permanent_bipedal5` |
| `permanent_bipedal7` | §6 | `gf2-algebra` | `permanent::bipedal7::permanent_bipedal7` |
| `gray_code_iter` | §7.3, App. A | `gf2-algebra` | `gray::gray_code_iter` |
| Rayon parallel dispatcher | §10, §11 | `gf2-algebra` | `parallel` (cfg `feature = "parallel"`) |
| HIP host-side dispatcher | §11 | `gf2-algebra` | `gpu` (cfg `feature = "hip"`) |
| `BatchedBipedalLike<P,...>` framework (if R4 picks generic) | §10 | `gf2-kernels-simd` | `bipedal_kernel::generic` |
| AVX2/AVX-512 `bipedal3_kernel` (per-prime fast path) | §10 | `gf2-kernels-simd` | `bipedal_kernel::bipedal3` |
| AVX2/AVX-512 `bipedal{5,7}_kernel` | §10 | `gf2-kernels-simd` | `bipedal_kernel::bipedal{5,7}` |
| HIP device kernels (`permanent_{f3,f5,f7}.hip`) | §11 | `gf2-kernels-hip` | `permanent::*` (host-side handle types `re-exported` to `gf2-algebra::gpu`) |
| `FiniteField`, `ConstField`, `FiniteFieldExt` | (existing) | `gf2-core` | `field::*` (unchanged) |
| `Fp<P>` (incl. `Fp<3>`, `Fp<5>`, `Fp<7>`) | (existing) | `gf2-core` | `gfp::*` (unchanged) |
| `BitVec`, `BitMatrix`, `SpBitMatrix` | (existing) | `gf2-core` | re-exports at crate root (unchanged) |

No existing public type moves out of `gf2-core`. The new code is purely additive in `gf2-algebra` plus two leaf-level kernel modules in the existing kernel crates.

## 3. Dependency-edge graph

Default workspace members are bold; `gf2-kernels-hip` remains in workspace `exclude` and is reached only via `--features hip` on `gf2-algebra`. Edge directions below match the actual `Cargo.toml` deps verified by reading `crates/gf2-core/Cargo.toml` (line 19: non-optional `gf2-kernels-simd = { path = "../gf2-kernels-simd" }`) and `crates/gf2-kernels-simd/Cargo.toml` (lines 21-25: only a `[dev-dependencies]` back-edge to `gf2-core`, which does not affect rlib link order).

```
            +-----------------+
            |   gf2-coding    |  (existing, unchanged)
            +--------+--------+
                     |
                     v
            +-----------------+        +-----------------------+
            |    gf2-core     |------->|  gf2-kernels-simd     |
            |  (FiniteField,  |  non-  |  (LogicalFns, Fp_*    |
            |   Fp<P>, BitVec)|  opt   |   Fns, +bipedal_*)    |
            +--------+--------+  dep   +-----------+-----------+
                     ^                             ^
                     |                             |
                     |   feat=parallel/simd/hip    | feat=simd
                     |                             |
                     +-------+   +-----------------+
                             |   |
                          +--+---+--+
                          | gf2-    |    feat=hip      +---------------------+
                          | algebra +----------------->|  gf2-kernels-hip    |
                          | (NEW)   |                  | (excluded by default|
                          +---------+                  |  workspace; opt-in) |
                                                       +---------------------+
```

Edges (forward, no cycles):

- `gf2-algebra -> gf2-core` (always; for `FiniteField`, `Fp<P>`, `BitVec`).
- `gf2-algebra -> gf2-kernels-simd` (cfg `feature = "simd"`, default on).
- `gf2-algebra -> gf2-kernels-hip` (cfg `feature = "hip"`, default off).
- `gf2-core -> gf2-kernels-simd` (existing, non-optional path dep; the `simd` feature on `gf2-core` only gates runtime detection wiring, not the link itself, because `gf2-kernels-simd` hosts the SSOT scalar `clmul_u64_scalar` that `gf2-core::gf2m::barrett` always calls).
- `gf2-kernels-hip -> gf2-core` (existing).
- `gf2-coding` is unaffected; it does not depend on `gf2-algebra`.

The `gf2-kernels-simd` crate has only a `[dev-dependencies]` edge to `gf2-core` (used by its parity tests); dev-deps do not appear in the rlib link graph and so do not create a cycle. Adding `gf2-algebra -> gf2-core` is therefore safe: the new node sits above both `gf2-core` and `gf2-kernels-simd`, and there is no path from any of `gf2-core` / `gf2-kernels-simd` / `gf2-kernels-hip` back to `gf2-algebra`.

`gf2-algebra` is added to `[workspace] members`. `gf2-kernels-hip` stays in `exclude` per the existing pattern; gating the GPU permanent on `--features hip` matches the BCJR precedent.

## 4. Rationale for non-obvious placements

### 4.1 Why `Bipedal3` / `Bipedal5` / `Bipedal7` are NOT in `gf2-core`

Three reasons, ordered by force.

1. **A bipedal value is not a field element.** A `Bipedal3` packs 64 independent F_3 lanes into two `u64` words. It cannot implement `FiniteField` because `FiniteField::add` returns one element of one field, while `Bipedal3::add` returns 64 lane-parallel results. CLAUDE.md §gf2-core module map describes `gfp/` as "GF(p) prime field `Fp<P>` with Montgomery multiplication" — the bipedal types are structurally lane-vectors of F_3, not `Fp<P>`.
2. **`gf2-core` is the bedrock layer.** Per CLAUDE.md §Architecture, `gf2-core` houses primitives shared across coding theory, channel simulation, and any downstream algebra. The bipedal encoding is specific to fast permanent computation; it has no consumer outside `gf2-algebra`. Pulling it down into `gf2-core` would push F_3-specific bit-twiddle code into the foundation that every other crate already pays the compile cost for.
3. **Trait surface stability.** `gf2-core::field::FiniteField` is exercised by `gf2-coding` (BCH/LDPC) and verified in Lean (`proofs/Gf2Core/`). The `PackedField<F>` trait described in §6 of the epic doc is a different abstraction (lane-parallel) and is expected to evolve as F_5 / F_7 encodings settle (R1, R2). Its natural home is `gf2-algebra` next to its only impls. The api-freeze gate before W6 freezes this trait for Charon extraction in V1 / V2.

### 4.2 Where `gray_code_iter` lives

`gf2-algebra::gray::gray_code_iter`. The epic §7.3 and App. A use the iterator only inside `permanent_ryser` and `permanent_bipedal3`. There is an unrelated Gray-code construction inside `gf2-core::alg::m4rm` (the M4RM precompute table); that one is internal to M4RM matrix multiplication and is a *table over Gray-coded indices*, not a subset enumerator. Sharing one utility across the two would couple unrelated code paths. Verified by reading `crates/gf2-core/src/alg/m4rm.rs`: the existing M4RM Gray usage builds `[u64; 16]` accumulator panels, not a `0..2^n` subset stream.

Decision: keep `gray_code_iter` in `gf2-algebra::gray`. If a future epic needs it elsewhere, move *up* to `gf2-core` at that point and update the import in `gf2-algebra` only.

### 4.3 Why kernels split simd / hip / algebra (three layers)

This split mirrors the existing precedent (`Fp65537Fns` in `gf2-kernels-simd`, `GpuBcjrBatch` in `gf2-kernels-hip`, dispatched from `gf2-core::simd` / `gf2-coding::ldpc::gpu` respectively):

- **`gf2-kernels-simd`** owns `unsafe` AVX2 / AVX-512 intrinsics for bipedal arithmetic. It exposes safe function-pointer bundles (`Bipedal3Fns`, optional generic `BatchedBipedalLikeFns`) chosen at runtime via `OnceLock`, matching the existing `LogicalFns` / `Fp65537Fns` pattern.
- **`gf2-kernels-hip`** owns `unsafe` HIP / ROCm device FFI for permanent kernels. It stays in workspace `exclude` so non-ROCm hosts continue building `cargo build --workspace` cleanly.
- **`gf2-algebra`** is `#![deny(unsafe_code)]` and contains only the algorithm layer plus runtime dispatch glue (`parallel.rs`, `gpu.rs`).

This keeps the project-wide invariant from CLAUDE.md §Key design invariants point 3 intact: all `unsafe` lives in the two accelerator kernel crates, never in algorithm crates.

### 4.4 Why parallel dispatch lives in `gf2-algebra` (not `gf2-core::compute`)

`gf2-core::compute` provides a generic rayon harness over batch BitVec / BitMatrix / `Fp<P>` ops; its consumers are dense linear-algebra primitives. The permanent parallel dispatcher is a Gray-code-block work-stealing schedule specific to `permanent_bipedal*` and shares no shape with the existing batch helpers. Putting it in `gf2-algebra::parallel` keeps the scheduling logic next to the kernels it dispatches.

### 4.5 Feature-gate placement (matrix is finalised in D1c)

D1c owns the full feature-gate matrix. For boundary-correctness purposes only, this doc fixes:

- `gf2-algebra` defaults: `["simd", "parallel"]`. `simd` propagates to `gf2-core/simd` and `gf2-kernels-simd`.
- `gf2-algebra` opt-in: `"hip"` (pulls `gf2-kernels-hip`), `"f5"` / `"f7"` (TBD in D1c whether per-prime or single `extras` flag).

## 5. Validation checklist for the W1 skeleton (T1)

The W1 issue creating the skeleton must produce a crate that satisfies all of the following before D1a is honoured:

- [ ] `crates/gf2-algebra/Cargo.toml` exists with `rust-version = "1.95"`, matching MSRV across the workspace.
- [ ] `[workspace] members` in the root `Cargo.toml` is extended with `crates/gf2-algebra`. `gf2-kernels-hip` remains in `exclude` (not moved).
- [ ] `gf2-algebra` declares `gf2-core` as a path dependency. It does NOT depend on `gf2-coding` (no edge in either direction).
- [ ] `gf2-algebra/src/lib.rs` carries `#![deny(unsafe_code)]`.
- [ ] `cargo metadata --format-version 1 | jq '.resolve.nodes[] | select(.id|contains("gf2-algebra")) | .deps[].name'` shows `gf2-core`, optionally `gf2-kernels-simd` (default), and never lists itself in any reverse edge from `gf2-core` / `gf2-coding` / `gf2-kernels-simd` (no cycles).
- [ ] `cargo build -p gf2-algebra --no-default-features` succeeds with no SIMD or parallel.
- [ ] `cargo build -p gf2-algebra` (defaults `simd,parallel`) succeeds.
- [ ] `cargo build -p gf2-algebra --features hip` succeeds on a host with hipcc available, and is gated `#[cfg(feature = "hip")]` in `src/gpu.rs` so default builds stay clean on non-ROCm hosts.
- [ ] `cargo nextest run --workspace --all-features --release --profile ci` continues to pass (no regression from existing crates).
- [ ] `rustup run 1.95.0 cargo check -p gf2-algebra` passes (MSRV intrinsic-feasibility precondition per CLAUDE.md §Breakdown-time feasibility check; D4 runs the SIMD-intrinsic version of the same check).
- [ ] No public type listed in §2 above is exported from a crate other than the one named there. Enforced by inspecting `cargo public-api -p gf2-algebra` after T1–T3 land.
