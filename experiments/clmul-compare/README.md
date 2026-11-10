# Carry-less product comparison

Throughput of gf2's long carry-less polynomial product against gf2x
(`@/citation/GfTwoX2026`) on identical seeded operands (@/issue/befccab3).
[`compare.sh`](compare.sh) times gf2's production long product,
`gf2_core::gf2m::wide::clmul_slice` with the `simd` feature, and gf2x's
`gf2x_mul_r` with a reentrant pool, at the operand lengths
[`src/lib.rs`](src/lib.rs) lists, on one core. Each size checks that the two
products agree word for word before timing, and a rate is the median of the
samples `src/lib.rs` states.

The comparison binary is the [`external/`](external/) crate, outside the
workspace: its build links the gf2x install prefix `GF2_GF2X_PREFIX` names.
`compare.sh` builds it once for every `prefix-*` directory under
`GF2_GF2X_ROOT`, so the table reports gf2x's fastest configuration per size.
It runs inside a benchmark window and writes the table under `target/`.
