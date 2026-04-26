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
