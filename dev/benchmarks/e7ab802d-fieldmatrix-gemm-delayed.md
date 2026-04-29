# e7ab802d — FieldMatrix GF(p) delayed-reduction fgemm evidence

## Implementation measured

The production `FieldMatrix::gemm` path remains the public API. For `Fp<P>`,
the shared delayed dot-product kernel now accumulates raw storage products in
`u128` and performs one modular reduction per safe chunk:

- canonical/specialized storage: `Σ aᵢ bᵢ mod P`;
- Montgomery storage: `Σ (aᵢR)(bᵢR) ≡ R²Σaᵢbᵢ`, then one REDC yields
  `RΣaᵢbᵢ`, the correct Montgomery element.

The bound is unchanged from the existing Dumas–Pernet delayed-reduction
contract: each raw storage word is `< P`, so each product is `< (P - 1)²`, and
`Fp<P>::max_unreduced_additions() = floor(u128::MAX / (P - 1)²)` is a safe
chunk length.

## Fast development benchmark

Command:

```bash
CARGO_TARGET_DIR=target/agent-e7ab802d cargo bench -p gf2-core \
  --bench fieldmatrix_gemm_delayed --features rand -- Fp_7/.*64
CARGO_TARGET_DIR=target/agent-e7ab802d cargo bench -p gf2-core \
  --bench fieldmatrix_gemm_delayed --features rand -- 'Fp_(251|65521)/.*64'
```

Host/toolchain: local Linux worktree agent run, release/criterion bench profile.
The benchmark compares a public eager scalar triple loop (field reduction every
MAC) with the production cache-blocked delayed path.

| 64c88ae4-style fgemm cell | eager scalar | delayed blocked | speedup |
| --- | ---: | ---: | ---: |
| `Fp<7>`, 64×64×64 | 336.39 µs, 779.28 Melem/s | 146.06 µs, 1.7947 Gelem/s | 2.30× |
| `Fp<251>`, 64×64×64 | 337.12 µs, 777.59 Melem/s | 147.92 µs, 1.7722 Gelem/s | 2.28× |
| `Fp<65521>`, 64×64×64 | 722.67 µs, 362.74 Melem/s | 308.59 µs, 849.48 Melem/s | 2.34× |

## Deferred cells

The full `64c88ae4` suite includes n = 256/1024/4096 square cases and
1024×1024×{32,8} rectangular cases across all fields. Those remain in the
existing `fieldmatrix_gemm` harness and should run in slow/nightly benchmarking
or a dedicated performance session; the full sweep is explicitly documented as
multi-minute in that bench. The new `fieldmatrix_gemm_delayed` bench provides
fast pre/post cells and a smoke target for CI/development.

## Notes

These numbers improve the scalar Rust path but do not demonstrate the
aspirational "within 10× fflas-ffpack for n ≥ 256" target. That should be
amended or kept as a follow-up until the full fflas-comparable n = 256+ cells
are measured on the reference host.
