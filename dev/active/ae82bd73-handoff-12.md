# ae82bd73 (Permanents F_3/F_5/F_7) — session 9 handoff

**Date:** 2026-05-15
**Branch:** main (HEAD `bc305d59`)
**Session focus:** close the b62c86d8 + 8c902184 review loops; land W5 GPU kernels (F_3 ad55b777, F_5 b43cdf33, F_7 5c0505b2); CPU/GPU consistency narrowing.

## What landed since handoff-11

### Closed issues this session

1. **8c902184 (API freeze)** — `Packed7::LANES` → `packed::packed7::LANES` correction; added "out of scope" section enumerating cfg-gated placeholder modules; re-pinned doc at HEAD; user-approved sign-off provenance updated. PASS.

2. **b62c86d8 (HIP scaffold)** — criterion 4 reworded via user-approved AskUserQuestion option A (`cargo build -p gf2-algebra --release --features simd,parallel,f5,f7,serde,test-support` instead of the self-contradictory `--all-features (without hip)` wording). PASS.

3. **ad55b777 (F_3 HIP kernel)** — full kernel + batched FFI shim + 5 GPU tests (n=16,24,32,40,63). User-approved amendments: (a) per-n test counts on criterion 2 reflect GPU sequential-Gray-walk wallclock feasibility; (b) CPU/GPU narrowing from `n ≤ 64` to `n ≤ 63`. PASS.

4. **b43cdf33 (F_5 HIP kernel)** — direct byte-arithmetic kernel (vs CPU Packed5 bit-plane) per user-approved amendment; batched FFI; tests at n=8,12. PASS.

5. **5c0505b2 (F_7 HIP kernel)** — LUT-based kernel (MUL_LUT in `__constant__`, ADD/SUB in `__device__`); explicit caller-driven init via `init_permanent_gf7`; CAS-loop memoised init state machine (`memoise_init_outcome`) with 6 host-side unit tests; LUT-checksum test on gfx1030. PASS.

### CPU narrowing landed alongside (commit `afec59b9`)

User-approved CPU/GPU consistency narrowing applied to gf2-algebra:
- `gray_code_iter`: assert `n ≤ 64` → `n ≤ 63`.
- `permanent_bipedal3_singleword` / `_singleword_simd`: assert `n ≤ 64` → `n ≤ 63`.
- `permanent_bipedal3` dispatcher: `if n <= 64` → `if n <= 63`; n=64 now routes to multi-word path.
- `permanent_bipedal5_singleword`: assert tightened to `n ≤ 63`.
- Renamed `test_permanent_bipedal3_singleword_panics_on_n_65` → `_on_n_64`.
- Renamed `test_permanent_bipedal3_dispatch_routes_n64_to_singleword` → `_routes_n64_to_multiword`.
- Renamed F_5 `test_permanent5_panics_on_n_65` → `_on_n_64`.
- Updated all narrative referencing `n ≤ 64` in bipedal3.rs, bipedal5.rs, gray.rs.

### Test infrastructure landed (commit `97af9dad`)

Extracted shared HIP test helpers into `crates/gf2-kernels-hip/tests/common/mod.rs`:
- `extern "C"` HIP runtime bindings (`hipMalloc`, `hipFree`, `hipMemcpy`, `hipDeviceSynchronize`).
- `HIP_MEMCPY_HOST_TO_DEVICE` / `HIP_MEMCPY_DEVICE_TO_HOST` constants.
- `xorshift64` PRNG.
- `run_with_device_buffers` alloc/H2D/launch/D2H/free helper taking a closure.

All three F_3/F_5/F_7 test files now consume `common::*` via `#[path = "common/mod.rs"] mod common;`. Tests renamed to `test_<operation>_<scenario>` per CLAUDE.md.

### GPU run evidence on gfx1030 (`dev/active/ae82bd73-w5-gpu-verification.md`)

Recorded device-run pass output for criteria 2/3 across all three kernels:
- F_3: n=16 (0.146 s) + n=24 (14.263 s) bit-identical.
- F_5: n=8 (0.067 s) + n=12 (0.106 s) bit-identical.
- F_7: n=8 (0.254 s) + n=12 (0.275 s) bit-identical + LUT checksum match.

## Status

| Wave | State | Notes |
|---|---|---|
| W0–W3 | DONE | |
| W4 | DONE | |
| **W5** | **DONE (kernels) — 2/6** | b62c86d8, ad55b777, b43cdf33, 5c0505b2 closed. Remaining: **2fbbdfa5** (host dispatcher), **a9e461de** (GPU-vs-CPU crossover sim). |
| W6 | BLOCKED on 8c902184 (now CLOSED) | f05ffbe1, 0606186a, 30e98ef1 can dispatch. All three need approved proof sketches per CLAUDE.md verification-work convention. |
| W7 | PENDING | 8 issues. |

## Open escalations / decisions for next session

None outstanding. Session resolved 7 user escalations: scaffold criterion 4 amendment, freeze doc completeness, F_3 test counts, F_3 n-range narrowing, CPU/GPU consistency, F_5 byte-arithmetic encoding, F_5 criterion 5 wording.

## What worked / what to repeat

- **Holistic doc audit before re-running gates** prevented the iterative-reviewer-surfaces-one-finding pattern for 8c902184 freeze completeness; one PR cycle instead of N.
- **Pre-emptive criterion 5 wording fix** on b43cdf33 / 5c0505b2 (manifest-path form) avoided repeating the build-invocation finding three times.
- **Extracted `memoise_init_outcome` helper** with unit tests turned an untestable atomic-FFI-init dance into a 6-test regression suite. Same pattern works any time a multi-thread state machine needs verification without the underlying side-effecting FFI.
- **Worker dispatches with explicit "DO NOT pass gates / touch JIT state" preamble** keep workers focused on code; the lead retains state-transition authority.
- **Capturing GPU run output in a versioned verification doc** (`dev/active/ae82bd73-w5-gpu-verification.md`) gave the reviewer concrete evidence to close hard criteria 2/3 without device access.

## Traps — do not repeat these

Carrying forward all traps from handoffs 1–11. New traps from session 9:

- **Trap session-9-1 (sub-agent reporting "build clean" without verifying)**: The F_7 worker reported `cargo build --features hip --release: success` for code that referenced `gf2_algebra` from `lib.rs` — but `gf2-algebra` was a *dev-dependency*, so the lib build failed. The worker likely confused the build pass under tests (which DO see dev-deps) with the lib build. **Fix:** spot-check worker-reported build claims with a fresh `cargo build --manifest-path ... --release` on the LIB target before running the gate, especially for FFI / dep-graph-sensitive changes.

- **Trap session-9-2 (workspace-excluded crates and `-p`)**: `cargo build -p gf2-kernels-hip` fails from the repo root because the crate is workspace-excluded. The criterion 5 wording on b62c86d8, ad55b777, b43cdf33, 5c0505b2 inherited this incorrect command verbatim. Fix on creation: any issue criterion that builds a workspace-excluded crate MUST use `--manifest-path <crate-path>/Cargo.toml`, never `-p <crate-name>`.

- **Trap session-9-3 (non-atomic load-then-store on shared state)**: The first version of `GF7_INIT_RC` memoisation used a non-atomic load-then-store; a failed init could clobber a concurrent successful init. **Fix:** any time the state-update rule is "X dominates Y" (here, "success dominates failure"), use a CAS loop — never a load-then-store, even if the load and store are individually atomic.

- **Trap session-9-4 (review pass timing out on AI gate)**: A `jit gate pass code-review` invocation timed out at 600 s — likely due to upstream AI latency, not the artifact. The fix: retry once; if it times out again, escalate to user (don't keep retrying).

- **Trap session-9-5 (iterative reviewer findings on doc nits)**: Even after fixing every substantive issue, the reviewer surfaced "missing `# Examples` on `init_permanent_gf7`" — a CLAUDE.md doc-standard nit, not a correctness issue. **Fix:** holistic-audit the public-API surface for missing rustdoc sections BEFORE the first gate run on any newly-added public symbol; do `grep -L "# Examples"` across new `pub fn` lines.

## Active worktrees

None.

## Active background processes

None.

## Session 9 metrics

- **Issues closed:** 5 (`b62c86d8`, `8c902184`, `ad55b777`, `b43cdf33`, `5c0505b2`).
- **User escalations resolved:** 7 (b62c86d8 crit-4 wording, ad55b777 test counts, ad55b777 n-range narrowing, CPU/GPU consistency narrowing approach, b43cdf33 byte-arithmetic encoding, b43cdf33 build invocation wording, 5c0505b2 build invocation wording).
- **User escalations open:** 0.
- **Commits on main this session:** ~30.
- **Tests passing on HEAD `bc305d59`:** 3783 fast-tier (workspace --all-features --release --profile ci) + 6 init-state-machine unit tests under --features hip + 9 gfx1030-only tests verified manually.

## Next-session priorities

1. **W5 dispatcher (2fbbdfa5)** — host-side dispatcher that routes to CPU `permanent_bipedal3_singleword` (n ≤ 63 small batch), CPU multiword (n ≥ 64 / large batch), or GPU `compute_permanent_gfX_batch` (large M). Tied to runtime crossover from a9e461de.

2. **W5 GPU-vs-CPU crossover sim (a9e461de)** — measure the M / n product where GPU launch overhead is amortised by parallelism gain. Output: a CSV + a chosen crossover curve documented in `dev/plans/`.

3. **W6 Lean verification chain (post 8c902184 close)** — `f05ffbe1` (bipedal F_3 correctness, V1) → parallel `0606186a` (Ryser n ≤ 63, V2) + `30e98ef1` (F_5/F_7 aspirational, V3). All three require approved proof sketches per CLAUDE.md verification-work convention. Sketches `a0c0a45f` (V1) and `4aaa6e4d` (V2) are already done; V3 needs a new sketch.

4. **W7 Reporting + sims**: `7cd9afdb` (publication benchmark), `16f03734` (README + doctests), `8808b051` (root CLAUDE.md + ROADMAP), `424aa94f` (plot scripts), `c90db5a4` (`scripts/permanent-repro.sh`). Parallelisable. Also `333028c1` (F_5/F_7 CAS cross-validation), `8e4e19a0` (perm-vs-det uniformity sim), `9480f8a6` (S1g GPU 50× — after a9e461de lands).

5. **Final**: epic ae82bd73 completion report + transition to done.

## Files of note

- `dev/active/ae82bd73-w5-gpu-verification.md` — gfx1030 run evidence, attached to 5c0505b2.
- `crates/gf2-kernels-hip/tests/common/mod.rs` — shared HIP test scaffolding.
- `crates/gf2-kernels-hip/hip/permanent/permanent_bipedal{3,5,7}.hip` — the three kernels (byte for F_3/F_5, LUT for F_7).
- `crates/gf2-kernels-hip/src/permanent/mod.rs` — Rust safe wrappers + `memoise_init_outcome` CAS state machine + 6 unit tests.
- `dev/plans/gf_api_freeze_w6.md` — API freeze, pinned at the post-narrowing HEAD.
