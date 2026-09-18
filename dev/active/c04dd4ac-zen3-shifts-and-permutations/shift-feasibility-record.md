# Residual `BitVec` shift: planning-time feasibility record

> **Diátaxis Type:** Explanation

**Issue:** 85fc5ff4
**Barrier:** REQ-05, the planning-time Rust 1.95 compile and assembly record

The [workload profile](shift-profile.md) records a material disposition and
nominates two forms. REQ-05 holds the profile open until a Rust 1.95 compile
and assembly record proves each nominated form feasible, with a runtime-gated
scalar fallback for every capability-gated form. This artifact is that record.

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
portable funnel the production paths run today, compiled with no
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

## Where REQ-05 continues

The amended [plan](plan.md) rests on this record. It creates the candidate
implementation leaf for the BMI2-gated scalar funnel (`f8dd4dde`) and its A/B
confirmation leaf (`00dd43c3`), makes them depend on this profile, and re-homes
publication behind the confirmation in the instantiated dependency graph. The
AVX2 lane-crossing funnel stays feasible and unselected in this epic; the
plan's decision table records the ruling. This record creates no issue itself.

## Scope proposal for the candidate leaves

Input to that amendment, held to what the evidence above supports. The leaves'
own criteria govern where they differ from it.

Every claim below about the production shift paths is pinned in
[`survey/shift-source-evidence.json`](survey/shift-source-evidence.json)
rather than by a line number here.

**Candidate implementation leaf.** The kernel lives in `gf2-kernels-simd`
beside the existing word-shift kernels, since it is the only production crate
that may contain `unsafe` and the AVX2 form needs it; the BMI2 form needs a
`#[target_feature]` scope but no raw pointers, so it can be written safely
there too. The dispatch point is the `bit_shift != 0` branch of
`BitVec::shift_left` and `BitVec::shift_right`, which today falls straight
into the scalar funnel while the `bit_shift == 0` branch already consults
`crate::simd::maybe_simd()`; the residual branch gains the same consultation,
behind the same `simd` cargo feature and the same runtime detection, with the
present loop as the fallback. Evidence supports building the BMI2 form first:
it reaches its intended instructions from the expression the production loop
already contains, so it costs a target-feature scope and a gate rather than a
new kernel. The leaf's behavioural suite is the existing `BitVec` shift tests
extended with the corpus shape this record's prototype uses, run once per
form through the repository's dispatch-forcing convention, with the
process-global override serialized as `crates/gf2-core/tests/prime_route_dispatch.rs` does.

**Confirmation leaf.** An A/B family over the same arms the profile measured,
residual against residual: the current scalar funnel as A and the gated kernel
as B, at the profile's material cells, so the confirmation answers the
question the profile left open rather than re-running the word-aligned
control. Its cell count is whatever P-20 admits for this family's ledger at
the time it is frozen, computed from the campaign support sources, and its
addendum is derived from the committed pilot receipt by the canonical
freezer. A family whose ledger admits no confirmatory cell records that
arithmetic as its outcome.

Both leaves depend on this profile. Neither is authorized by this record.
