# Residual `BitVec` shift: planning-time feasibility record

> **Diátaxis Type:** Explanation

**Issue:** 85fc5ff4
**Barrier:** REQ-05, the planning-time Rust 1.95 compile and assembly record

The [workload profile](shift-profile.md) records a material disposition and
nominates two forms. REQ-05 requires a Rust 1.95 compile and assembly record
for each form, with a runtime-gated scalar fallback. This artifact satisfies
that planning-time barrier; the [completed confirmation](../00dd43c3/confirmation-outcome.md)
is the separate authority for the selected route's performance and retention.

It is planning evidence. It changes no production crate, adds no production
line, measures no time and claims no speedup. Its whole subject is what the
repository's MSRV compiler accepts and emits.

## What the record is made of

A standalone prototype crate,
[`survey/residual-shift-feasibility/`](survey/residual-shift-feasibility/),
carries both nominated forms, the runtime capability gate and the scalar
fallback. It sits outside the gf2 workspace behind its own `[workspace]`
table, the way the other standalone prototypes under `dev/` do, so the
`unsafe` it needs never reaches a production crate's lint surface.
[`capture-asm.sh`](survey/residual-shift-feasibility/capture-asm.sh)
regenerates every artefact below and reproduces each one byte for byte on an
unchanged tree.

| Artefact | What it holds |
|---|---|
| [`toolchain.txt`](survey/residual-shift-feasibility/toolchain.txt) | The compiler and cargo identity, the emit command, and the `--print cfg` target features of the build baseline |
| [`intrinsic-probe.txt`](survey/residual-shift-feasibility/intrinsic-probe.txt) | rustc's own answer on the double-precision shift intrinsic |
| [`asm/shift_left_funnel_avx2.s`](survey/residual-shift-feasibility/asm/shift_left_funnel_avx2.s), [`asm/shift_right_funnel_avx2.s`](survey/residual-shift-feasibility/asm/shift_right_funnel_avx2.s) | The AVX2 form's emitted hot loops |
| [`asm/shift_left_funnel_bmi2_pair.s`](survey/residual-shift-feasibility/asm/shift_left_funnel_bmi2_pair.s), [`asm/shift_right_funnel_bmi2_pair.s`](survey/residual-shift-feasibility/asm/shift_right_funnel_bmi2_pair.s) | The BMI2 form's emitted hot loops, shift-and-complement expression |
| [`asm/shift_left_funnel_bmi2_dp.s`](survey/residual-shift-feasibility/asm/shift_left_funnel_bmi2_dp.s), [`asm/shift_right_funnel_bmi2_dp.s`](survey/residual-shift-feasibility/asm/shift_right_funnel_bmi2_dp.s) | The same loops written as a 128-bit double-precision shift |

The build takes no `-C target-cpu` and no `-C target-feature`: `toolchain.txt`
shows the baseline the emit runs at, so every wide or BMI2 instruction in the
assembly comes from a `#[target_feature]` scope the runtime gate guards, not
from a build flag that would break the fallback.

## Form 1 — the AVX2 lane-crossing funnel: feasible

Every intrinsic the form needs compiles at 1.95, and the compiler emits a
single vector loop for each direction with no spill and no bound test inside
the loop body.

The lane crossing is emitted, but not as the instruction pair the source
names. The source builds the neighbouring-word vector with
`_mm256_permute2x128_si256` followed by `_mm256_alignr_epi8`; LLVM 22.1.2
rewrites that pair into `vpblendd` plus one `vpermq`, which performs the same
permutation in fewer operations. The funnel itself is emitted as intended:
`vpsllq` and `vpsrlq` taking the shift count from an `xmm` register — so the
count stays a runtime value rather than forcing a specialization per offset —
combined with `vpor`, over unaligned `vmovdqu` loads and stores. The scalar
epilogue for the words below the first full group is emitted beside the
vector loop in the same function.

The right-shift loop reaches that shape only once its group guard is a single
loop-invariant comparison; written as the two-part guard the bounds naturally
suggest, the backend keeps a bound test and a second exit branch inside the
loop. The prototype's right funnel therefore precomputes the last admissible
group base, and a candidate implementation needs the same care.

## Form 2 — the BMI2-gated scalar funnel: feasible, with a constraint

Rust 1.95 exposes no double-precision shift intrinsic: `core::arch::x86_64`
ships `_bzhi_u64`, `_pdep_u64`, `_pext_u64` and `_mulx_u64` under BMI2 and
nothing named `_shld_u64` or `_shrd_u64`. `intrinsic-probe.txt` is rustc's
verdict rather than a reading of the standard library. The form is therefore
reachable only as an expression the backend folds, which is what makes an
assembly record, not a compile check, the evidence that decides it.

Both expressions compile and both reach their intended instruction:

- The shift-and-complement pair — the expression the production loop already
  writes — becomes `shlx` and `shrx` under the `bmi2` target-feature scope,
  with the unrolled word pair kept as two independent chains joined by `or`.
  This is the BMI2 lowering the nomination asks for, and it needs no new
  expression in the kernel at all: only the target-feature scope and the gate.
- The 128-bit double-precision expression becomes `shld` and `shrd`, one
  instruction per word, on the condition that the shift count is masked to
  its low six bits at the expression. Without the mask the backend cannot
  prove the count stays below 64 — a `u128` shift is defined up to 128 while
  `shld` is not — and guards every double-precision shift with a `test` and a
  `cmov` against a wide count, alongside a redundant variable shift. With the
  mask those guards disappear. The masked form is what the committed `_dp`
  assembly shows.

## The runtime gate and the fallback

The gate is a single `is_x86_feature_detected!` check per form in the
prototype's `select`, applied before any dispatch, and each entry point
returns the form it actually ran. A form whose feature the host lacks becomes
the scalar funnel, and the caller can see that it did. The fallback is the
portable funnel the selected production route retains, compiled with no
target-feature scope and no `unsafe`. On a non-x86 target the gate resolves to
the fallback unconditionally.

`tests/equivalence.rs` asserts the gate's verdict against the host's own
feature detection for every form, so the fallback is exercised as a reachable
path rather than assumed.

## Correctness

`cargo +1.95 test --release`, run from the prototype's directory, compares
every form and the fallback against an independent bit-addressed zero-fill
reference that shares no code with them. The corpus covers the required
lengths and offsets 0, 1, 7, 8, 63, 64 and 65; offsets at the length, one past
it, twice it and far beyond it; sizes that leave an incomplete final word;
and sizes large enough for the 256-bit group loop to run and cross its lanes.
Every case asserts both the full word content and the zero tail above
`len_bits`. A third test asserts the corpus actually reaches the vector
regime, so a passing sweep cannot be vacuous.

The suite is untimed and deterministic: its inputs come from a fixed-seed
splitmix64 fill keyed by the case.

## What this record does not establish

Feasibility is not a win. This record shows both nominated forms compile at
the MSRV and emit the instructions their nomination names; it shows nothing
about how fast either runs, and the profile's own disposition section explains
why the word-aligned control does strictly less work per word than any correct
residual shift can. Any speed claim belongs to a confirmation measured under
the protocol, not here.

The prototype is also not a candidate implementation. It has no `BitVec`
dependency, no dispatch table entry and no feature flag; it exists to be
compiled and read.

## Completed bracket and route

The amended [plan](plan.md) uses this feasibility record to place the selected
BMI2-gated funnel implementation (`f8dd4dde`) and its A/B confirmation
(`00dd43c3`) behind the material profile. Both leaves are complete, and
publication follows the confirmation in the instantiated dependency graph.
The AVX2 lane-crossing form remains feasible at Rust 1.95 and unselected in
this epic for the reason in the plan's decision table. The prototype remains
planning evidence, not an implementation or a speed claim.

The [retained production route](../f8dd4dde/retention-rule.md) places the BMI2
funnel in `gf2-kernels-simd`, the owning CPU intrinsic crate. `BitVec` sends
residual offsets to [`crate::residual_shift`](../../../crates/gf2-core/src/residual_shift.rs),
which selects the bundle when the non-default `simd` cargo feature and observed
`bmi2` capability permit it, and otherwise runs the portable scalar funnel.
The shared behavioural suite checks both routes, including the fallback.

The [accepted confirmation](../00dd43c3/confirmation-outcome.md) compares the
same residual operation through the scalar and BMI2 routes, using the material
cells selected from the exploratory profile under the frozen family rule. Its
acceptance summary qualifies and its frozen retention rule keeps the BMI2 route.
The dropped lane-crossing pilot cell remains exploratory and is distinct from
the unimplemented AVX2 nomination.
