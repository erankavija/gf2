# gf2-kernels-simd

Isolated unsafe SIMD kernels (AVX2/AVX-512/AArch64) consumed by `gf2-core` via
the `OnceLock`-dispatched `LogicalFns` boundary. All workspace `unsafe` lives
here; the rest of the workspace is `#![deny(unsafe_code)]`. See `CLAUDE.md`
for crate layout, MSRV (1.80), and the apex dispatch constraint.

## ASM-inspection convention (PPC-spiral I3)

Each kernel module ships a sibling assembly artefact so reviewers can confirm
that SIMD/ILP transforms emitted the expected mnemonics (`vpxor`,
`vpclmulqdq`, `vpternlogq`, `vpermq`, ...) without regenerating disassembly.

The convention:

```
crates/gf2-kernels-simd/src/x86/<module>.rs
crates/gf2-kernels-simd/src/x86/asm/<module>.asm.txt        # full module
crates/gf2-kernels-simd/src/x86/asm/<module>_<fn>.asm.txt   # per-function (optional)
```

Regenerate after every spiral-step edit that changes a SIMD kernel:

```bash
./dev/scripts/regen-asm.sh gf2-kernels-simd \
    gf2_kernels_simd::x86::avx2::avx2_xor_into \
    crates/gf2-kernels-simd/src/x86/asm/avx2_xor.asm.txt \
    [extra-symbol ...]
```

The script writes a header banner (crate, symbols, rustc version, host triple,
target-cpu, RUSTFLAGS, commit short SHA, regeneration date) followed by the
disassembly for each requested symbol.

### Tooling

The script uses [`cargo-show-asm`](https://github.com/pacak/cargo-show-asm).
Install with:

```bash
cargo install cargo-show-asm --locked
```

If it is unavailable, set `REGEN_ASM_FALLBACK=1` for a `cargo rustc
--emit=asm` fallback (whole-crate dump, noisier). Override `TARGET_CPU` to
pin a baseline (`native`, `x86-64-v3`, ...); the default is unset so the
artefact reflects each function's `#[target_feature(...)]` attribute.

## `asm-artefact-present` gate

A JIT post-check gate (`scripts/asm-artefact-present.sh`) inspects the latest
commit and fails the build if any
`crates/gf2-kernels-simd/src/x86/<module>.rs` (excluding `mod.rs`) was
modified without a matching `crates/gf2-kernels-simd/src/x86/asm/<module>.asm.txt`
(or `<module>_<fn>.asm.txt`) being regenerated in the same commit. The gate
vacuously passes when no SIMD source changed.

See `dev/plans/gf2_core_ppc_spiral.md` (sections I3 and the per-kernel
execution protocol) for the rationale and the list of mnemonics each Tier
A–D kernel is expected to emit.

## `criterion-1.5x` gate (per-kernel speedup vs pinned baseline)

A second JIT post-check gate (`scripts/criterion-1.5x.sh`) enforces that
the geomean speedup of the kernel under test against its pinned criterion
baseline is at least 1.5×. The harness behind it
(`dev/benchmarks/ppc-compare.sh`) is kernel-id-positional, so the wrapper
extracts the kernel-id from the JIT issue's labels at gate time.

Convention: every issue gated on `criterion-1.5x` must carry exactly one
`ppc-kernel:<id>` label, where `<id>` is one of the keys in
`dev/benchmarks/ppc-baselines.json` (currently `A1, A2, A3, B1, B2, B3,
C1, C2, C3, C4, C5, D1, D2`). Lead applies the label when defining or
dispatching the issue:

```bash
jit issue update <issue-id> --add-label ppc-kernel:A1
```

The wrapper requires `--pass-context` and reads the issue context JSON
from `$JIT_CONTEXT_FILE`. If the label is missing, it exits 2 (infra not
ready, distinct from a kernel FAIL). Exit codes are forwarded transparently
from `ppc-compare.sh`: 0 = PASS (>= 1.5x), 1 = FAIL (< 1.5x), 2 = infra
error, 3 = baseline still `TBD-...` (pending `jit:b2ecd2ff`).
