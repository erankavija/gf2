# D1c — gf2-algebra Cargo feature-gate matrix

**JIT issue:** `4fced99b` (W0 / D1c)
**Epic:** `epic:gf2-algebra-permanent`
**Predecessor decisions:**
- D1a (`6e20133d`, `dev/plans/d1a_gf2_algebra_boundary.md`) — fixes the home crate `gf2-algebra` and its dependency edges; pre-commits the default set to `["simd", "parallel"]` (D1a §4.5).
- D1b (`9fe275d3`, `dev/plans/d1b_packed_field_api.md`) — fixes the `PackedField` / `PackedFieldVec` / `Permanent` trait surface that the feature gates are wrapped around.
**Status:** decision
**Date:** 2026-05-09

## 1. Scope

This document fixes the Cargo feature-gate matrix for the new `gf2-algebra`
crate (W1-T1 of the `gf2-algebra-permanent` epic). It enumerates each feature
flag, its default state, the code paths it gates, and the cross-feature
compatibility properties the W1-T1 implementer must preserve. It also
fixes the verification approach for the matrix: running `cargo check`
over all 64 cells at the W1-T1 crate-skeleton stage. A separate
14-cell subset is recommended for ongoing CI as a cost optimisation
licensed by the orthogonality the full sweep establishes; the subset
is not the verification approach.

In scope:

- The six features called out in the strawman (`simd`, `parallel`, `hip`,
  `f5`, `f7`, `serde`).
- Cross-feature dependency rules: how `simd` is wired through to
  `gf2-core` and to the runtime SIMD kernel crate; whether `hip` requires
  `simd`; the default-on / default-off status of each flag.
- The verbatim `[features]` block to be transcribed into
  `crates/gf2-algebra/Cargo.toml` at W1-T1.
- The verification approach for the matrix: full 64-cell `cargo check`
  sweep at W1-T1 acceptance.
- A separate ongoing-CI recommendation (14-cell subset) for use after
  the W1-T1 full sweep has established the orthogonality property.

Out of scope (deferred to W1-T1 or later):

- The full `[dependencies]` block (only the dependency edges named by D1a
  are pre-committed; sub-dependency choices like `serde_json`-yes-or-no
  are W1-T1 decisions).
- AVX2 / AVX-512 sub-flags within `simd`. Detection is runtime per
  `gf2-kernels-simd` precedent; there is no plan to surface per-ISA
  Cargo flags on `gf2-algebra`. (See §5.1.)

## 2. Feature catalogue

The committed feature set. Names follow the existing project precedent
(`gf2-coding` and `gf2-core` already define `simd`, `parallel`, with
identical semantics).

| Feature   | Default | Pulls in                                    | Gates                                                                                                                    |
|-----------|---------|---------------------------------------------|--------------------------------------------------------------------------------------------------------------------------|
| `simd`    | on      | `gf2-core/simd`                             | Runtime SIMD dispatch hooks in `permanent::bipedal3` (and W4 `bipedal{5,7}`); the `gf2-kernels-simd` link is always present, this only enables the dispatch wiring. |
| `parallel`| on      | `dep:rayon`, `gf2-core/parallel`            | The `parallel.rs` module: `permanent_bipedal3_par`, rayon chunked Gray-code work-stealing schedule, chunk-size sweep harness. |
| `hip`     | off     | `dep:gf2-kernels-hip`                       | The `gpu.rs` module: HIP host-side dispatcher, gfx1030 device-kernel handles re-exported from `gf2-kernels-hip::permanent`. |
| `f5`      | off     | (no extra dep)                              | Compilation of `packed::bipedal5`, `permanent::bipedal5` modules (and their tests / benches). Body-shape decision is W4 per R1.|
| `f7`      | off     | (no extra dep)                              | Compilation of `packed::bipedal7`, `permanent::bipedal7` modules (and their tests / benches). Body-shape decision is W4 per R2.|
| `serde`   | off     | `dep:serde` (with `derive` feature)         | `Serialize` / `Deserialize` impls on `Bipedal3` / `Bipedal3Vec` / `Bipedal3Matrix` (and W4 the `Bipedal{5,7}*` analogues).      |

Six features. 2^6 = 64 cells in the full compatibility matrix (§4).

### 2.1 Why these six and no more

Each row above corresponds to a separate consumer-visible build axis. We
considered three candidate additions and rejected each:

- **`avx2` / `avx512` sub-flags.** Rejected. `gf2-kernels-simd` already
  exposes `avx2` and `avx512` Cargo features but its lib crate uses
  runtime detection via `is_x86_feature_detected!` (verified in
  `crates/gf2-kernels-simd/src/lib.rs` and exercised in
  `dev/plans/d4_intrinsic_feasibility.md` §3). Adding a parallel flag
  surface on `gf2-algebra` would only confuse callers. The W3-T12 SIMD
  kernel issue may add `avx2` / `avx512` Cargo features to
  `gf2-kernels-simd` for build-gate purposes if it grows them; that is
  internal to the kernel crate and does not require a `gf2-algebra`
  surface flag.
- **`gpu-debug` / `hip-prof`.** Rejected for now. GPU profiling and
  debug builds will use the `gf2-kernels-hip` crate's own feature
  surface when it grows one; routing through `gf2-algebra::hip` is not
  needed at W1.
- **`f3` flag.** Rejected. F_3 is the headline workload and is the
  reason the crate exists; gating it would mean the default build does
  nothing useful. F_3 packed types are unconditionally compiled.

## 3. Default-feature selection

**Decision:** `default = ["simd", "parallel"]`.

This matches the strawman in the issue body and the pre-commit in D1a
§4.5. Rationales:

1. **Existing project precedent.** `gf2-coding` ships
   `default = ["simd"]` (verified `crates/gf2-coding/Cargo.toml` line 36).
   `gf2-coding` does not default-enable `parallel` because much of its
   surface is single-frame (one Hamming / BCH / LDPC decode at a time)
   and `parallel` would pull in rayon for a workload that does not
   benefit. **`gf2-algebra` is different**: the headline workload
   (`permanent_bipedal3` at $n=36$) is a roughly $2^{36}$-step Gray-code
   walk where parallel scaling is the entire point of W3 and a `[hard]`
   epic success criterion (parent §14 #5). Defaulting `parallel` on is
   the right call for the typical user.
2. **`simd` is cheap to default on.** It only flips a runtime-dispatch
   bit; the SIMD kernel crate itself is always linked (D1a §3 verified
   that `gf2-core` non-optionally depends on `gf2-kernels-simd` because
   the latter hosts the SSOT scalar `clmul_u64_scalar`). On a host
   without AVX2, `is_x86_feature_detected!` returns `false` and the
   scalar fallback is selected at runtime — the feature being on costs
   nothing.
3. **`hip` stays off** because hipcc / ROCm is not present on most
   developer hosts. The HIP crate is in workspace `exclude` for the
   same reason (root `Cargo.toml` line 9-11).
4. **`f5` / `f7` stay off until W4 lands.** The W1-T1 skeleton creates
   the crate before R1 / R2 outcomes are wired up; defaulting `f5`
   on at W1 would mean either (a) the modules are empty stubs that
   compile trivially but produce no useful output, or (b) callers see
   non-functional API surfaces. Per CLAUDE.md issue-lifecycle rule, we
   do not default-enable APIs whose bodies are empty. The W4 issue
   that lands `permanent_bipedal5` is the trigger to revisit; see §8.
5. **`serde` stays off** because most callers that actually compute
   permanents do not also serialise the matrices. Adding serde to the
   default would pull in a heavy dep on `serde` for users who are not
   using it. `gf2-coding` makes a different choice (it has serde as a
   non-optional dependency for its parameter-table wiring), but
   `gf2-algebra` does not have an analogous always-needed use.

The default set `["simd", "parallel"]` is consistent with the §7 Cargo
fragment and with the §6 ongoing-CI subset (the `default` cell is
one of the 14 in the recommended subset, and it is also exercised by
the §6.1 full 64-cell sweep at W1-T1 acceptance).

## 4. Compatibility matrix (2^6 = 64 cells)

Every combination must build cleanly. The matrix below uses a 6-bit
bitmap `(simd, parallel, hip, f5, f7, serde)` where bit `b_i = 1` means
feature `i` is enabled. The columns are:

- **Bitmap.** 6-bit string in feature order above.
- **`--features` arg.** What you pass to `cargo build` to reproduce.
  `default` is shown for the `110000` cell since that is what
  `cargo build` defaults to with no flags.
- **Build behaviour.** `clean` = builds without external prerequisites.
  `clean (no-op f5/f7)` = builds clean but the F_5 / F_7 module trees
  are empty until W4. `needs hipcc + ROCm` = requires the HIP toolchain
  and a gfx1030-class GPU on the host machine.

Rows are sorted by bitmap value (binary count-up).

| #  | Bitmap   | `--features`                               | Build behaviour              |
|----|----------|--------------------------------------------|------------------------------|
| 0  | `000000` | `--no-default-features`                    | clean                        |
| 1  | `000001` | `--no-default-features --features serde`   | clean                        |
| 2  | `000010` | `--no-default-features --features f7`      | clean (no-op f7)             |
| 3  | `000011` | `--no-default-features --features f7,serde`| clean (no-op f7)             |
| 4  | `000100` | `--no-default-features --features f5`      | clean (no-op f5)             |
| 5  | `000101` | `--no-default-features --features f5,serde`| clean (no-op f5)             |
| 6  | `000110` | `--no-default-features --features f5,f7`   | clean (no-op f5/f7)          |
| 7  | `000111` | `--no-default-features --features f5,f7,serde` | clean (no-op f5/f7)      |
| 8  | `001000` | `--no-default-features --features hip`     | needs hipcc + ROCm           |
| 9  | `001001` | `--no-default-features --features hip,serde` | needs hipcc + ROCm         |
| 10 | `001010` | `--no-default-features --features hip,f7`  | needs hipcc + ROCm; no-op f7 |
| 11 | `001011` | `--no-default-features --features hip,f7,serde` | needs hipcc + ROCm; no-op f7 |
| 12 | `001100` | `--no-default-features --features hip,f5`  | needs hipcc + ROCm; no-op f5 |
| 13 | `001101` | `--no-default-features --features hip,f5,serde` | needs hipcc + ROCm; no-op f5 |
| 14 | `001110` | `--no-default-features --features hip,f5,f7` | needs hipcc + ROCm; no-op f5/f7 |
| 15 | `001111` | `--no-default-features --features hip,f5,f7,serde` | needs hipcc + ROCm; no-op f5/f7 |
| 16 | `010000` | `--no-default-features --features parallel` | clean                       |
| 17 | `010001` | `--no-default-features --features parallel,serde` | clean                 |
| 18 | `010010` | `--no-default-features --features parallel,f7` | clean (no-op f7)         |
| 19 | `010011` | `--no-default-features --features parallel,f7,serde` | clean (no-op f7)   |
| 20 | `010100` | `--no-default-features --features parallel,f5` | clean (no-op f5)         |
| 21 | `010101` | `--no-default-features --features parallel,f5,serde` | clean (no-op f5)   |
| 22 | `010110` | `--no-default-features --features parallel,f5,f7` | clean (no-op f5/f7)   |
| 23 | `010111` | `--no-default-features --features parallel,f5,f7,serde` | clean (no-op f5/f7) |
| 24 | `011000` | `--no-default-features --features parallel,hip` | needs hipcc + ROCm      |
| 25 | `011001` | `--no-default-features --features parallel,hip,serde` | needs hipcc + ROCm |
| 26 | `011010` | `--no-default-features --features parallel,hip,f7` | needs hipcc + ROCm; no-op f7 |
| 27 | `011011` | `--no-default-features --features parallel,hip,f7,serde` | needs hipcc + ROCm; no-op f7 |
| 28 | `011100` | `--no-default-features --features parallel,hip,f5` | needs hipcc + ROCm; no-op f5 |
| 29 | `011101` | `--no-default-features --features parallel,hip,f5,serde` | needs hipcc + ROCm; no-op f5 |
| 30 | `011110` | `--no-default-features --features parallel,hip,f5,f7` | needs hipcc + ROCm; no-op f5/f7 |
| 31 | `011111` | `--no-default-features --features parallel,hip,f5,f7,serde` | needs hipcc + ROCm; no-op f5/f7 |
| 32 | `100000` | `--no-default-features --features simd`    | clean                        |
| 33 | `100001` | `--no-default-features --features simd,serde` | clean                     |
| 34 | `100010` | `--no-default-features --features simd,f7` | clean (no-op f7)             |
| 35 | `100011` | `--no-default-features --features simd,f7,serde` | clean (no-op f7)       |
| 36 | `100100` | `--no-default-features --features simd,f5` | clean (no-op f5)             |
| 37 | `100101` | `--no-default-features --features simd,f5,serde` | clean (no-op f5)       |
| 38 | `100110` | `--no-default-features --features simd,f5,f7` | clean (no-op f5/f7)       |
| 39 | `100111` | `--no-default-features --features simd,f5,f7,serde` | clean (no-op f5/f7) |
| 40 | `101000` | `--no-default-features --features simd,hip` | needs hipcc + ROCm          |
| 41 | `101001` | `--no-default-features --features simd,hip,serde` | needs hipcc + ROCm    |
| 42 | `101010` | `--no-default-features --features simd,hip,f7` | needs hipcc + ROCm; no-op f7 |
| 43 | `101011` | `--no-default-features --features simd,hip,f7,serde` | needs hipcc + ROCm; no-op f7 |
| 44 | `101100` | `--no-default-features --features simd,hip,f5` | needs hipcc + ROCm; no-op f5 |
| 45 | `101101` | `--no-default-features --features simd,hip,f5,serde` | needs hipcc + ROCm; no-op f5 |
| 46 | `101110` | `--no-default-features --features simd,hip,f5,f7` | needs hipcc + ROCm; no-op f5/f7 |
| 47 | `101111` | `--no-default-features --features simd,hip,f5,f7,serde` | needs hipcc + ROCm; no-op f5/f7 |
| 48 | `110000` | (no flags — `default = ["simd","parallel"]`) | clean                      |
| 49 | `110001` | `--features serde`                          | clean                       |
| 50 | `110010` | `--features f7`                             | clean (no-op f7)            |
| 51 | `110011` | `--features f7,serde`                       | clean (no-op f7)            |
| 52 | `110100` | `--features f5`                             | clean (no-op f5)            |
| 53 | `110101` | `--features f5,serde`                       | clean (no-op f5)            |
| 54 | `110110` | `--features f5,f7`                          | clean (no-op f5/f7)         |
| 55 | `110111` | `--features f5,f7,serde`                    | clean (no-op f5/f7)         |
| 56 | `111000` | `--features hip`                            | needs hipcc + ROCm          |
| 57 | `111001` | `--features hip,serde`                      | needs hipcc + ROCm          |
| 58 | `111010` | `--features hip,f7`                         | needs hipcc + ROCm; no-op f7|
| 59 | `111011` | `--features hip,f7,serde`                   | needs hipcc + ROCm; no-op f7|
| 60 | `111100` | `--features hip,f5`                         | needs hipcc + ROCm; no-op f5|
| 61 | `111101` | `--features hip,f5,serde`                   | needs hipcc + ROCm; no-op f5|
| 62 | `111110` | `--features hip,f5,f7`                      | needs hipcc + ROCm; no-op f5/f7 |
| 63 | `111111` | `--features hip,f5,f7,serde` (`--all-features`) | needs hipcc + ROCm; no-op f5/f7 |

Three observations:

- **Independence.** No feature requires another. `simd`, `parallel`,
  `hip`, `f5`, `f7`, `serde` are all individually flippable.
- **`hip` shifts the build precondition** (32 cells need hipcc), but
  does not change Rust-side compatibility with any other flag. The
  `gf2-kernels-hip` crate has zero Cargo features of its own (verified
  `crates/gf2-kernels-hip/Cargo.toml` lines 1-21), so there is no
  internal hip×simd interaction at the Cargo level.
- **`f5` / `f7` are no-ops at the W1 skeleton stage** — the modules
  exist but their bodies are empty placeholders until W4 lands the
  encodings. Builds pass; nothing computes a permanent over those
  fields. This will flip to "clean (functional)" in W4 (§8).

**Verification approach for the matrix (issue success criterion 3):**
running `cargo check -p gf2-algebra <combo>` over all 64 cells above
at the W1-T1 crate-skeleton stage. This is the contract that satisfies
the criterion. The mechanics, wall-clock budget, and substitution
rule for `hip`-bearing cells on non-ROCm hosts are fixed in §6.1.

A separate **ongoing-CI optimisation** (14-cell subset, §6.2) reduces
per-PR build cost after W1-T1 by leveraging the orthogonality property
that the full sweep establishes. The subset is **not** the verification
approach for the matrix; it is a downstream cost-control measure that
is only valid because §6.1 already verified all 64 cells.

## 5. Cross-feature dependency rules

This section resolves the four explicit questions in success criterion 4.

### 5.1 Does `simd` propagate to `gf2-kernels-simd/simd`?

**Answer: there is no `simd` feature on `gf2-kernels-simd` to propagate
to.** The propagation target is `gf2-core/simd`.

Verified facts (from reading the actual Cargo.toml files):

- `gf2-kernels-simd` features (verified `crates/gf2-kernels-simd/Cargo.toml`
  lines 15-19): `default = []`, `avx2`, `avx512`. There is no `simd`
  feature. Detection of which kernel to use is runtime via
  `is_x86_feature_detected!`.
- `gf2-coding` propagates its `simd` feature to `gf2-core/simd`:
  `simd = ["gf2-core/simd"]` (verified
  `crates/gf2-coding/Cargo.toml` line 37). It does **not** propagate to
  any feature on `gf2-kernels-simd`, because there isn't one.
- `gf2-core/simd` itself (verified `crates/gf2-core/Cargo.toml` lines
  41-44) only gates the runtime-detection wiring; the
  `gf2-kernels-simd` rlib is always linked because it hosts the SSOT
  scalar `clmul_u64_scalar` that `gf2-core::gf2m::barrett` calls
  unconditionally.

**Decision:** `gf2-algebra/simd = ["gf2-core/simd"]`. The phrasing
"aligned across crates so users opt out of SIMD as a stack" still
holds: turning off `gf2-algebra/simd` turns off `gf2-core/simd` too, so
the runtime SIMD-detection wiring is suppressed crate-wide. The link to
`gf2-kernels-simd` remains (it has to, for the scalar `clmul_u64_scalar`
SSOT), but no SIMD code path is exercised.

If the W3-T12 SIMD kernel issue grows a `simd` umbrella feature on
`gf2-kernels-simd` that turns on AVX2 + AVX-512 jointly (currently each
ISA has its own flag), `gf2-algebra/simd` should be amended to also
propagate to that umbrella. That is a non-breaking amendment that the
W3-T12 issue can land alongside its SIMD work. It does not need to be
pre-committed here.

### 5.2 Does `hip` require `simd`?

**Answer: no.** `hip` and `simd` are independent.

Verified: `gf2-coding` already has `hip = ["dep:gf2-kernels-hip"]`
(verified `crates/gf2-coding/Cargo.toml` line 43) — no `simd`
prerequisite. `gf2-kernels-hip` has zero Cargo features (verified its
Cargo.toml). The HIP host-side dispatcher in `gf2-algebra::gpu` is a
thin handle around `gf2-kernels-hip::permanent::*`; it does not invoke
the CPU SIMD path. The CPU fallback inside the GPU dispatcher (when
GPU is unavailable at runtime) goes through the **scalar**
`permanent_bipedal3_single` / `_multi` paths, which compile without
`simd`. (Whether the runtime fallback is `simd`-accelerated when both
flags are on is an internal optimisation; the Cargo-level requirement
is just that `--features hip` builds cleanly with `--no-default-features`.)

**Decision:** `gf2-algebra/hip = ["dep:gf2-kernels-hip"]`. No `"simd"`
in the value. The `simd,hip` and `hip` (no-simd) cells in §4 both
build clean.

### 5.3 Default-on status of `f5` / `f7`

**Answer: off until W4 lands the implementations.**

Rationale recapped from §3 #4: at W1 the bodies are empty stubs.
Defaulting them on would expose API surfaces with no implementation,
which CLAUDE.md issue-lifecycle policy disallows.

The flip-to-on criterion is documented in §8. In short: when both
`permanent_bipedal5` and `permanent_bipedal7` pass their cross-check
tests against `permanent_ryser` over `Fp<5>` and `Fp<7>` (epic success
criterion 6 in `gf2_algebra_permanent.md` §14), and W4 is closing,
the W4-completion issue toggles `default = ["simd", "parallel", "f5",
"f7"]` in a separate dispatched edit. This is a non-breaking change for
existing callers (they were all building with `--features f5,f7`
explicitly anyway, since the issue title for W4 is "F_5 / F_7 packed
permanent").

### 5.4 Other cross-feature notes

- `parallel` does not require `simd` (rayon dispatches to whichever
  scalar / SIMD path is selected by the inner `permanent_bipedal3_*`
  call). The `parallel` (no-simd) cells in §4 build clean.
- `serde` does not require any other flag. The serde impls are
  boilerplate `Serialize` / `Deserialize` derives over `(Vec<u64>,
  Vec<u64>)`-shaped fields. They are independent of the dispatch
  layer. `serde,hip` builds clean (assuming hipcc + ROCm).
- All combinations of `f5` + `f7` + the dispatch flags are clean
  because at W1 the f5 / f7 modules are empty.

## 6. Verification approach

The verification approach for the 2^6 = 64-cell compatibility matrix
(issue success criterion 3) is the **W1-T1 full-matrix sweep** in §6.1
below: `cargo check -p gf2-algebra <combo>` over every one of the 64
cells in §4 at the crate-skeleton stage. §6.2 layers a 14-cell
ongoing-CI subset on top, as a cost-control optimisation for per-PR
runs after W1-T1; that subset is not the verification approach
itself, only a downstream optimisation licensed by §6.1's outcome.

### 6.1 W1-T1 acceptance gate: full 64-cell sweep (the verification approach)

**Contract.** Before the W1-T1 (`gf2-algebra` crate skeleton) issue
can mark its `code-review` gate passed, the implementer runs
`cargo check -p gf2-algebra <flags>` once for every one of the 64
cells in §4. Every cell must build clean. This is the verification
of the matrix that satisfies the issue's hard success criterion
"running `cargo check` over the matrix at the crate-skeleton stage."
There is no subset substitution at this stage; all 64 cells are
exercised.

**Mechanics.** A small driver script (suggested name
`scripts/check-feature-matrix.sh`, authored as part of W1-T1)
iterates the 64 `--features` arg strings from the §4 table and
invokes `cargo check -p gf2-algebra` for each. It fails fast on the
first non-zero exit. The script and its log are committed alongside
the W1-T1 skeleton.

**Wall-clock estimate.** `cargo check -p gf2-algebra` on a populated
crate is roughly 3 s on a warm cache (per `gf2-coding` baseline) and
~12 s on a cold cache. At skeleton stage the bodies are mostly empty
so the warm-cache cost will be lower, but for budgeting purposes the
worst case is `64 * 12 s = ~13 min cold`, dropping to `64 * 3 s =
~3 min` once the workspace cache is warm. Realistic full-sweep
wall-clock on the dev host is in the 30 to 60 minute range when
including the first cold compilation per crate-graph variant. This
is a one-time cost; it does not recur on subsequent PRs.

**Hosts without hipcc / ROCm.** Of the 64 cells, 32 carry the `hip`
flag and need hipcc plus a gfx1030-class GPU. On non-ROCm dev hosts
those 32 cells substitute the equivalent flag combination with `hip`
removed (e.g. `--features simd,hip,f5` becomes `--features
simd,f5`). The substitution still exercises every Rust-side feature
combination; only the `gf2-kernels-hip` link step is skipped. The
W1-T1 acceptance log records which cells ran natively and which ran
under the substitution.

**Output of the W1-T1 sweep.** A pass/fail line per cell, attached
to the W1-T1 issue as a `verification` document. The orthogonality
observation (no feature gates a code path that depends on another)
is the empirical justification for §6.2 below.

### 6.2 Ongoing CI: 14-cell representative subset (cost optimisation, not verification)

Post-W1-T1, every PR runs a 14-cell subset of the matrix (not the
full 64) on the standard CI lane. **This is a CI-cost optimisation,
not the verification approach for the matrix.** The verification
approach is §6.1's full 64-cell sweep, which has already established
that every cell builds clean. The 14-cell subset only guards against
regression of that property in the per-PR loop, and is licensed by
the orthogonality observed during the §6.1 full sweep: because no
feature gates a code path that depends on another, a regression in a
non-subset cell would surface as a regression in one of the
cross-pair cells C12 to C14 below. The full sweep is re-run (per §6.3)
if a feature is added, removed, or renamed, or if a CI subset cell
flags an interaction the subset cannot localise.

The 14 cells exercise every individual feature in isolation, plus
the empty and all-features extremes, plus three cross-pair cells:

| Cell | Bitmap   | `--features`                          | Why |
|------|----------|---------------------------------------|-----|
| C1   | `000000` | `--no-default-features`               | Empty: catches feature-required-by-default-on bugs. |
| C2   | `100000` | `--no-default-features --features simd` | `simd` solo. |
| C3   | `010000` | `--no-default-features --features parallel` | `parallel` solo. |
| C4   | `001000` | `--no-default-features --features hip`| `hip` solo (skipped on non-ROCm runners; see below). |
| C5   | `000100` | `--no-default-features --features f5` | `f5` solo. |
| C6   | `000010` | `--no-default-features --features f7` | `f7` solo. |
| C7   | `000001` | `--no-default-features --features serde` | `serde` solo. |
| C8   | `110000` | (no flags — defaults)                  | Default build. |
| C9   | `110110` | `--features f5,f7`                     | Defaults + future-functional flags (the post-W4 default). |
| C10  | `110111` | `--features f5,f7,serde`               | Defaults + f5 + f7 + serde (the most likely "everything CPU" build). |
| C11  | `111111` | `--all-features`                       | Full sweep (skipped on non-ROCm; superseded by C12). |
| C12  | `110011` | `--features f7,serde`                  | Cross-pair: serde + f7 only. |
| C13  | `100100` | `--no-default-features --features simd,f5` | Cross-pair without parallel. |
| C14  | `010010` | `--no-default-features --features parallel,f7` | Cross-pair without simd. |

C4 and C11 are skipped on the standard CI runner (which lacks hipcc /
ROCm) and run only on the dedicated ROCm CI lane the epic adds in W5
(parent §13 W5). On non-ROCm runners, these two cells degrade to
`cargo check --no-default-features` and `cargo check
--features simd,parallel,f5,f7,serde` (the "all features minus hip"
form) respectively.

Ongoing-CI cost estimate: 14 incremental `cargo check -p gf2-algebra`
runs, each ~3 s on a warm cache (per `gf2-coding` baseline), ~12 s on
a cold cache. Total: ~45 s warm, ~3 min cold. This is well under the
60 s test-suite wall-clock target in CLAUDE.md §Performance rules. The
matrix runs in a separate CI job that does not block test execution.

The subset exercises every individual feature in isolation (C2-C7),
the default build (C8), the post-W4 default (C9), the most-likely
production combination (C10), the maximum sweep (C11), and three
cross-pair cells that catch dispatch-layer interactions (C12-C14).
The remaining 50 cells are not re-run on every PR; per §6.1 they
were verified clean once at W1-T1, and the orthogonality property
established there (no feature gates a code path that depends on
another) means a regression in any of them would surface as a
regression in one of C12 to C14 first. If the W1-T1 sweep ever fails
or is invalidated by a structural change to the feature graph, the
full sweep is re-run before the subset can be trusted again.

### 6.3 Re-running the full sweep

The full 64-cell sweep is also re-run (not just the 14-cell subset)
when any of the following occurs:

- A feature is added, removed, or renamed in `gf2-algebra/Cargo.toml`.
- A propagation rule changes (e.g. `simd` starts pulling in a new
  upstream feature).
- One of the 14 CI subset cells fails in a way that the subset
  cannot localise to a specific feature pair.

The re-run lands as part of the issue that triggered it (no separate
issue is needed), and its log is attached to that issue as a
`verification` document.

## 7. Cargo.toml skeleton fragment

The exact `[features]` block to be transcribed verbatim into
`crates/gf2-algebra/Cargo.toml` at W1-T1:

```toml
[features]
default = ["simd", "parallel"]

# Enable runtime SIMD dispatch hooks. Mirrors gf2-coding's pattern: the
# gf2-kernels-simd rlib is always linked (gf2-core needs it for the SSOT
# scalar `clmul_u64_scalar`), so this feature only flips the runtime
# detection wiring in gf2-core. On hosts without AVX2, runtime
# `is_x86_feature_detected!` returns false and the scalar fallback is
# selected — no functional change with this flag off, just a slightly
# smaller compiled-out detection table.
simd = ["gf2-core/simd"]

# Rayon-based parallel permanent. Pulls in rayon and the parallel
# helpers in gf2-core. On by default because the headline workload
# (permanent_bipedal3 at n=36) is a 2^36-step Gray-code walk where
# parallel scaling is the entire point of W3.
parallel = ["dep:rayon", "gf2-core/parallel"]

# HIP/ROCm GPU dispatch (gfx1030+). Off by default — matches
# gf2-coding's "hip" feature, since hipcc/ROCm is not present on most
# developer hosts. Pulls in the gf2-kernels-hip crate (which is in
# workspace `exclude`).
hip = ["dep:gf2-kernels-hip"]

# F_5 packed permanent. Off until W4 lands the encoding (per R1 outcome
# in dev/plans/r1_f5_encoding_decision.md). Compiling the modules
# without this flag means the public surface omits the F_5 types
# entirely — callers see no API.
f5 = []

# F_7 packed permanent. Off until W4 lands the encoding (per R2 outcome
# in dev/plans/r2_f7_encoding_decision.md). Same lifecycle as `f5`.
f7 = []

# Serde Serialize/Deserialize for packed types. Off by default because
# permanent computation does not typically involve serialisation; users
# who need it (e.g. caching matrices for reproducibility benchmarks) opt
# in. Pulls in serde with the derive feature.
serde = ["dep:serde"]
```

The `[dependencies]` block (handled at W1-T1, not pre-committed here)
must declare `gf2-core` non-optionally, `rayon` and `gf2-kernels-hip`
as `optional = true`, and `serde` as `optional = true` with
`features = ["derive"]`. The exact block is W1-T1's choice.

The MSRV line `rust-version = "1.95"` is fixed per CLAUDE.md §MSRV and
D1a §5 checklist item 1. The `edition = "2021"` matches the rest of
the workspace.

## 8. Migration / API stability notes

### 8.1 The W4 default-flip event

When the W4 issue suite (parent §13: T16-T21 plus S4) closes, the
`f5` and `f7` features move from "compile but module bodies empty" to
"compile and the permanent functions return correct results matching
`permanent_ryser`". The trigger to flip `default` from
`["simd", "parallel"]` to `["simd", "parallel", "f5", "f7"]` is:

- Epic success criterion 6 (parent §14 #6) is verified: F_5 / F_7
  packed permanents match `permanent_ryser` over `Fp<5>` / `Fp<7>` on
  1000 random matrices for $n \in \{1, \ldots, 14\}$.
- The CI matrix subset cell C9 (`--features f5,f7`) and C10
  (`--features f5,f7,serde`) builds clean and passes the W4
  cross-check tests at the same wall-clock budget as C8 (no
  regression).
- The W4-closing issue dispatches a one-line edit changing the
  `default` array in `crates/gf2-algebra/Cargo.toml`.

The flip is **non-breaking for existing callers**. Anyone explicitly
opting into `f5` / `f7` (the only callers using those modules at all,
since the modules are empty in the no-flag build) sees no behavioural
change. New callers building with the defaults gain access to the
F_5 / F_7 surface without changing their `Cargo.toml`.

The flip does **not** affect `simd` / `parallel` / `hip` / `serde`. The
default for those four does not change in W4.

### 8.2 What does not change at W6 api-freeze

The W6 `gate:api-freeze` (parent §13 W6, D1b §8) freezes the
`PackedField` / `PackedFieldVec` / `Permanent` trait surfaces against
Charon extraction churn. It does **not** freeze the Cargo feature
matrix — the feature names and propagation rules are independent of
the trait surface, and feature-matrix amendments do not break Lean
extraction. New features (e.g. an `avx512` umbrella feature, or a
debugging flag) can be added post-W6 if needed.

### 8.3 Deprecation policy

If a feature is later removed, it must follow the standard semver
convention: keep the name as a no-op alias for one minor release,
then remove. We do not anticipate removal of any of the six features
in §2 within the lifetime of the epic.

## 9. Open questions deferred to W1-T1

The following are out of scope for this feature-matrix decision and
land in W1-T1:

1. **`[dependencies]` block contents.** Exact dep names + versions
   (e.g. is `serde` declared from workspace.dependencies? what minor
   version of `rayon`?). D1a §3 fixed the dependency edges; W1-T1
   chooses the precise dep manifest entries.
2. **`[dev-dependencies]` block.** proptest, criterion, etc. — chosen
   by W1-T1 based on the test/bench wiring it adds.
3. **`[[bench]]` and `[[example]]` declarations.** The crate skeleton
   may carry empty bench / example shells (à la `gf2-coding`'s
   `bench_csv_emitter`) or defer them until W2-T10 lands the criterion
   suite. W1-T1 picks.
4. **Workspace-level CI workflow file changes.** The §6 CI subset
   describes what to run; the exact GitHub Actions YAML edit is a
   W1-T1 chore (or the W3-S3 issue for the cross-CPU portability
   sweep).
5. **`avx2` / `avx512` umbrella feature on `gf2-kernels-simd`.** The
   W3-T12 SIMD kernel issue may add this. If it does,
   `gf2-algebra/simd` propagation should be amended at that point
   (one-line change).
6. **HIP feature flags on `gf2-kernels-hip`.** That crate currently
   has no features; if W5 grows a `gpu-debug` flag (or similar),
   `gf2-algebra` may grow a passthrough at that time.

These items are tracked in the W1-T1 issue spec when it is created
from the epic's wave plan.
