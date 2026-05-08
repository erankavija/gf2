# D4: MSRV 1.95 intrinsic feasibility for bipedal SIMD kernels

**JIT issue:** `4c534d31`
**Epic:** `epic:gf2-algebra-permanent` (parent design: `dev/plans/gf2_algebra_permanent.md`)
**Status:** verified
**Date:** 2026-05-09

## 1. Scope

The bipedal `Bipedal3` arithmetic from Scheinerman 2024 §2.2 reduces every
`F_3` add / sub / mul / div to a fixed 1 to 6 op chain of bitwise AND, XOR,
and OR over wide registers (see `dev/plans/gf2_algebra_permanent.md` §2.2).
The W3 SIMD kernel issue (`T12`) needs the following intrinsics on x86_64:

**AVX2 (256-bit lane, 4 x u64):**

- `_mm256_and_si256`
- `_mm256_xor_si256`
- `_mm256_or_si256`
- `_mm256_loadu_si256`
- `_mm256_storeu_si256`

**AVX-512F (512-bit lane, 8 x u64) — `[aspirational]` per parent §4:**

- `_mm512_and_si512`
- `_mm512_xor_si512`
- `_mm512_or_si512`
- `_mm512_loadu_si512`
- `_mm512_storeu_si512`

This document records (a) the stability status of every intrinsic on the
project MSRV (rustc 1.95.0, release 2026-04-14), (b) the dev host's CPUID
flags as observed via `lscpu`, and (c) the procedural follow-through for
the prior `afac2262` rework cycle.

## 2. MSRV stability table

Status verified by `rustup run 1.95.0 cargo check --release` on the stub
crate at `dev/research/intrinsic_feasibility_stub/` (see §4). All intrinsics
listed below are exercised in functions guarded by
`#[target_feature(enable = "avx2")]` or `#[target_feature(enable = "avx512f")]`,
and the crate compiles with zero warnings on rustc 1.95.0 both with and
without `RUSTFLAGS="-C target-feature=+avx2,+avx512f"`.

The "stable since" column reflects what the empirical `cargo check` proves
on rustc 1.95.0: each intrinsic is stable at or before that version. The
column is not asserting an exact stabilisation rustc; that value would
require historical-rustc bisection out of scope for this feasibility
exercise. The MSRV-1.95 verification is what the `[hard]` criterion needs.

| Intrinsic                | Lane width | ISA       | Status on rustc 1.95.0 |
|--------------------------|------------|-----------|------------------------|
| `_mm256_and_si256`       | 256-bit    | AVX2      | stable, verified       |
| `_mm256_xor_si256`       | 256-bit    | AVX2      | stable, verified       |
| `_mm256_or_si256`        | 256-bit    | AVX2      | stable, verified       |
| `_mm256_loadu_si256`     | 256-bit    | AVX2      | stable, verified       |
| `_mm256_storeu_si256`    | 256-bit    | AVX2      | stable, verified       |
| `_mm512_and_si512`       | 512-bit    | AVX-512F  | stable, verified       |
| `_mm512_xor_si512`       | 512-bit    | AVX-512F  | stable, verified       |
| `_mm512_or_si512`        | 512-bit    | AVX-512F  | stable, verified       |
| `_mm512_loadu_si512`     | 512-bit    | AVX-512F  | stable, verified       |
| `_mm512_storeu_si512`    | 512-bit    | AVX-512F  | stable, verified       |

"Verified" here means the intrinsic, called with realistic typed arguments,
compiles in a `#[target_feature(...)]`-attributed function on rustc 1.95.0
with no `feature(...)` gate and no nightly toolchain. No `cfg` workaround
or scalar fallback is needed for compilation.

`is_x86_feature_detected!("avx2")` and `is_x86_feature_detected!("avx512f")`
are also stable on 1.95.0 and are the runtime gates the production kernels
will use, mirroring the existing pattern in
`crates/gf2-kernels-simd/src/x86/mod.rs`.

## 3. Dev host CPUID

Output from `lscpu` on the dev host (2026-05-09):

```
Architecture:                            x86_64
Model name:                              AMD Ryzen 9 5900X 12-Core Processor
Flags:                                   fpu vme de pse tsc msr pae mce cx8 apic
sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx mmxext
fxsr_opt pdpe1gb rdtscp lm constant_tsc rep_good nopl xtopology nonstop_tsc
cpuid extd_apicid aperfmperf rapl pni pclmulqdq monitor ssse3 fma cx16 sse4_1
sse4_2 x2apic movbe popcnt aes xsave avx f16c rdrand lahf_lm cmp_legacy
extapic cr8_legacy abm sse4a misalignsse 3dnowprefetch osvw ibs skinit wdt
tce topoext perfctr_core perfctr_nb bpext perfctr_llc mwaitx cpb cat_l3
cdp_l3 hw_pstate ssbd mba ibrs ibpb stibp vmmcall fsgsbase bmi1 avx2 smep
bmi2 erms invpcid cqm rdt_a rdseed adx smap clflushopt clwb sha_ni xsaveopt
xsavec xgetbv1 xsaves cqm_llc cqm_occup_llc cqm_mbm_total cqm_mbm_local
user_shstk clzero irperf xsaveerptr rdpru wbnoinvd arat npt lbrv svm_lock
nrip_save tsc_scale vmcb_clean flushbyasid decodeassists pausefilter
pfthreshold avic v_vmsave_vmload vgif v_spec_ctrl umip pku ospke vaes
vpclmulqdq rdpid overflow_recov succor smca fsrm debug_swap
```

**Observed:** `avx2` present, `vaes` and `vpclmulqdq` present (Zen 3 has the
non-AVX-512 versions), `fma` present, `bmi1`/`bmi2` present, `popcnt`
present.

**Not observed:** no `avx512f`, `avx512dq`, `avx512bw`, `avx512vl`, or any
other `avx512*` flag. This matches the parent design's §4 "Hardware envelope"
statement that the Ryzen 9 5900X (Zen 3) lacks AVX-512.

**Implication for downstream W3 issues:**

- `T12` (SIMD bipedal3 kernel) AVX2 path is `[hard]` — it must run on this host.
- `T12` AVX-512 path remains coded for portability but is `[aspirational]` per
  parent §14 success criteria. CI on this host runs only the AVX2 path; the
  AVX-512 path can be exercised only on a different host (Zen 4/5 desktop, or
  Intel Skylake-X / Icelake / Sapphire Rapids server) and that host would be
  required for any `[hard]` AVX-512 perf criterion.
- Runtime dispatch must therefore prefer AVX2 over AVX-512 only when the
  AVX-512 hardware path is verified. The `gf2-kernels-simd::detect()` pattern
  (see `crates/gf2-kernels-simd/src/lib.rs`) already implements this contract
  and W3 will follow the same shape.

## 4. Stub crate

Location: `dev/research/intrinsic_feasibility_stub/`

Layout (mirrors `dev/research/rns_prototype/`):

```
intrinsic_feasibility_stub/
  Cargo.toml          # standalone, publish = false, rust-version = "1.95"
  src/
    lib.rs            # avx2_stub + avx512_stub modules
```

The stub crate is **deliberately not a workspace member** — its `Cargo.toml`
declares an empty `[workspace]` table to detach from the parent workspace.
This isolates the experiment from the workspace's `Cargo.lock`, MSRV
inheritance, and clippy lints, and lets the W3 issue dispatch verify
intrinsic feasibility without polluting the production build.

What `lib.rs` exercises:

- `avx2_stub::loadu` → `_mm256_loadu_si256`
- `avx2_stub::storeu` → `_mm256_storeu_si256`
- `avx2_stub::bipedal_and` → `_mm256_and_si256`
- `avx2_stub::bipedal_xor` → `_mm256_xor_si256`
- `avx2_stub::bipedal_or` → `_mm256_or_si256`
- `avx2_stub::bipedal_add` → composite using all five AVX2 logical ops
- `avx512_stub::loadu` → `_mm512_loadu_si512`
- `avx512_stub::storeu` → `_mm512_storeu_si512`
- `avx512_stub::bipedal_and` → `_mm512_and_si512`
- `avx512_stub::bipedal_xor` → `_mm512_xor_si512`
- `avx512_stub::bipedal_or` → `_mm512_or_si512`
- `avx512_stub::bipedal_add` → composite using all five AVX-512 logical ops
- `run_all` → drives both stubs with deterministic 64-element inputs and uses
  `std::hint::black_box` to defeat dead-code elimination.

The `bipedal_add` shape is the paper's `Bipedal3::add` formula
`t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`,
which is the most demanding lane-wise primitive needed by `T12`.

### How to re-run the verification

From a shell with `rustup` available:

```sh
cd dev/research/intrinsic_feasibility_stub

# Default check (no target-feature override): both modules compile-gate on
# target_arch but do not require the host to have AVX2 / AVX-512 at compile time.
rustup run 1.95.0 cargo check --release

# Forced check (target-feature override): proves that target_feature gates
# compile in cleanly even when the toolchain is told to assume the features
# are present.
RUSTFLAGS="-C target-feature=+avx2,+avx512f" rustup run 1.95.0 cargo check --release

# Optional smoke run.
rustup run 1.95.0 cargo test --release
```

Both `cargo check` invocations completed in well under 1 second on the dev
host, with zero warnings (after the local `unused_unsafe` allow). The
`cargo test` smoke run reports:

```
running 1 test
test tests::run_all_compiles_and_executes ... ok
```

i.e. on the dev host the AVX2 driver returns `true` (it ran) and the AVX-512
driver returns `false` (gated out by `is_x86_feature_detected!("avx512f")`).

## 5. The `afac2262` lesson

`CLAUDE.md` §"Breakdown-time feasibility check" cites `afac2262` (an AVX-512
ZMM lane attempt) as the prior incident that motivates this whole document.
The relevant text:

> Previous incident: `afac2262` (AVX-512 ZMM lane) cost a rework cycle and
> a scope reduction because the intrinsic-feasibility check was not run
> during breakdown; the ZMM lane was requested on a host that has no
> AVX-512 hardware AND on an MSRV (then 1.80) that did not stabilise the
> required intrinsics. MSRV was bumped to 1.95 on 2026-04-27 so those
> particular intrinsics are now stable; the procedural lesson stands.

The procedural lesson is that the feasibility check must be **rerun** for
each new SIMD-touching epic, even when nothing has visibly changed, because
the hosting matrix and the intrinsic surface evolve independently. This
document is that rerun for `epic:gf2-algebra-permanent`. The verification
shows that the bipedal kernel's required intrinsic surface is now fully
stable on the project MSRV — the situation that bit `afac2262` would not
have bitten today on this surface, but the check still ran. Future
SIMD-touching breakdowns in this epic (or any other) must run an analogous
stub before dispatch.

## 6. Conclusion

All ten required intrinsics — five AVX2 and five AVX-512F — are stable on
rustc 1.95.0 (release 2026-04-14). The stub at
`dev/research/intrinsic_feasibility_stub/` compiles cleanly with both the
default toolchain settings and with `RUSTFLAGS="-C target-feature=+avx2,+avx512f"`.

The dev host (AMD Ryzen 9 5900X, Zen 3) supports AVX2 but not AVX-512. As a
consequence, downstream W3 issues will:

- treat the AVX2 path as `[hard]`, runnable on this host, and the carrier
  for the parent epic's headline single-thread perf number;
- treat the AVX-512 path as `[aspirational]` per parent §14, coded for
  portability and exercised only on hosts that report `avx512f` from
  `is_x86_feature_detected!`.

No intrinsic is unstable; no `cfg` gating beyond the standard
`#[cfg(target_arch = "x86_64")]` plus `#[target_feature(enable = "...")]`
attributes is required for compilation; runtime dispatch uses the existing
`is_x86_feature_detected!` pattern from `gf2-kernels-simd`. T12 is cleared
to dispatch from the intrinsic-feasibility angle.
