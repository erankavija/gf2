# `sparse-elim × GF(2^m)` self-canonical measurements (`jit:2cfc4372`)

| Field | Value |
|---|---|
| Date | 2026-05-08 |
| JIT issue | `2cfc4372` (Render final SOTA markdown scorecard) |
| Purpose | One-shot bench wiring the GF(2^8) and GF(2^16) sparse-elim cells the SOTA scorecard's reviewer demanded numeric values for. The existing `sparse_rref` bench (added by `eb57f944`) covered Gf2mWide<u8> and Gf2mWide<u32> at non-scorecard sizes (n=1024 d=1/n, n=4096 d=log(n)/n). This bench fills the gap with the scorecard-canonical sparse-elim cell sizes (n=256 d=3.906250e-02, n=1024 d=9.765625e-03 — same as `sparse-elim × GF(p)` rows in `2026-05-04-47698404-sparse.csv`) so § 4.4 of the scorecard can populate gf2-side numbers for the GF(2^m) self-canonical rows. |
| Host | AMD Ryzen 9 5900X (Zen 3) |
| Toolchain | rustc 1.95.0, criterion 0.5.1 |
| Bench file | `crates/gf2-core/benches/sparse_rref_scorecard.rs` |
| Build profile | `release` (`opt-level=3`, `lto=thin`, `codegen-units=1`) |
| Bench budget | `--measurement-time 5s`, `sample_size 10` |

## Measurements (Criterion medians)

| Field | Cell | gf2 wall (median) | Reference owner | Status |
|---|---|---:|---|---|
| GF(2^8) | n=256 / density 3.906250e-02 / csr | **189.77 ms** | (self-canonical, gf2-core) | PASS [self-canonical, target_matrix § 5.11 `semantics-mismatch`] |
| GF(2^8) | n=1024 / density 9.765625e-03 / csr | **12.751 s** | (self-canonical, gf2-core) | PASS [self-canonical, target_matrix § 5.11 `semantics-mismatch`] |
| GF(2^16) | n=256 / density 3.906250e-02 / csr | **219.32 ms** | (self-canonical, gf2-core) | PASS [self-canonical, target_matrix § 5.11 `semantics-mismatch`] |
| GF(2^16) | n=1024 / density 9.765625e-03 / csr | **14.647 s** | (self-canonical, gf2-core) | PASS [self-canonical, target_matrix § 5.11 `semantics-mismatch`] |

Raw Criterion estimates JSON files at:
- `target/criterion/sparse_rref_scorecard_Gf2m_u8_AES/n256_d3.906e-2_csr/new/estimates.json`
- `target/criterion/sparse_rref_scorecard_Gf2m_u8_AES/n1024_d9.766e-3_csr/new/estimates.json`
- `target/criterion/sparse_rref_scorecard_Gf2m_u16_Conway/n256_d3.906e-2_csr/new/estimates.json`
- `target/criterion/sparse_rref_scorecard_Gf2m_u16_Conway/n1024_d9.766e-3_csr/new/estimates.json`

## Cross-reference: GF(p) sparse-elim at the same cell sizes

For comparison (from `dev/bench_results/2026-05-08-dece4e73-sota-aggregate-gf2.csv`):

| Field | n=256 / d=3.9% | n=1024 / d=0.98% |
|---|---:|---:|
| GF(7) | 21.423 ms | 1.113 s |
| GF(251) | 16.761 ms | 776.362 ms |
| GF(65521) | 15.644 ms | 717.241 ms |
| GF(2^31-1) | 16.210 ms | 754.420 ms |
| GF(2) | 9.593 ms | 505.270 ms |
| **GF(2^8)** | **189.77 ms** (this doc) | **12.751 s** (this doc) |
| **GF(2^16)** | **219.32 ms** (this doc) | **14.647 s** (this doc) |

The GF(2^m) walls are ~10x larger than GF(p) at the same cell size. This is the
expected `semantics-mismatch` cost: GF(2^m) sparse-elim performs `Gf2mWide`
multiplications on every elimination step (PCLMULQDQ-backed but not SIMD-fused
across rows), whereas the GF(p) byte-canonical path uses tight u8/u16 axpy
tables. The 10x gap is a known feasible-CPU optimization gap tracked under
`4c0d0202` (sparse RREF future-Wave); not in scope for `2cfc4372` per
`sota_target_matrix.md` § 5.11 (cells are self-canonical, so PASS by design
regardless of absolute wall).

## Reproduction

```bash
cd /home/vkaskivuo/Projects/gf2
cargo bench -p gf2-core --bench sparse_rref_scorecard --features rand
```

Total wall: ~7 minutes for 4 cells × 10 samples × 5s measurement-time + warmup
on the listed host.

## Test coverage

`SparseFieldMatrix<Gf2mWide<…>>::rref` is unit-tested in
`crates/gf2-core/src/field/sparse_matrix.rs`. The bench exercises the same
production code path the scorecard timings reflect (no test-copied helper).
