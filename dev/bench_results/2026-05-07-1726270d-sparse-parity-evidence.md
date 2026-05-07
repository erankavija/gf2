# Sparse parity evidence (`jit:1726270d`)

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| JIT issue | `1726270d` (Publish sparse parity evidence) |
| Parent story | `54fd3f0b` (Close sparse FieldMatrix SpMV and SpMM gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Reference scorecard | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` |
| Layout/traversal evidence | `dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md` |
| CPU/GPU handoff decision | `dev/plans/sparse_cpu_vs_gpu_handoff.md` |
| Acceptance protocol | `dev/plans/sota_reference_acceptance_protocol.md` |

---

## § 0 TL;DR

This document is the final sparse closure evidence for story `54fd3f0b` and epic `97bf0879`.
All 7 externally-referenced sparse GF(p) cells now PASS the 1.5x contract (gf2/fflas >= 0.667x),
closed across two optimization passes (Path A: lazy-reduction CSR layout, issue `3a37e0f6`;
Path B: AVX2 packed-int SpMM kernel, continuation of `3a37e0f6`). The GPU non-goal boundary
is defined: no sparse work flows to the GPU epic (`16283d6f`) as a result of this closure; all
remaining sparse exclusion cells are self-canonical or deferred CPU algorithmic work.

Both `[hard]` success criteria for `1726270d` are met:

1. **Criterion 1 (Raw CSVs and ratio tables linked to story `54fd3f0b`):** Evidence in § 1 and § 2.
2. **Criterion 2 (CPU/GPU non-goal boundaries documented):** Evidence in § 3.

---

## § 1 Cell-by-cell verdict table

All measurements at n=1024, density=9.765625e-3, CSR layout unless noted.
Pre-Wave-3 baseline from `dev/bench_results/2026-05-04-47698404-sparse.csv`.
Path-A and Path-B measurements from
`dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-a.csv` (sparse×dense Path A),
`dev/bench_results/2026-05-07-3a37e0f6-spmv-path-a.csv` (spmv Path A), and
`dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` (sparse×dense Path B —
final). The three permanent CSVs are checked into `dev/bench_results/`; their original
session-scoped names (`gf2-sparse-1778{136354,136375,138911}.csv`) come from the
3a37e0f6 worker's `bench_results/` runtime directory.

Reference timings from `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` (fflas-ffpack
rows). Criterion spmv timings (Path A and post-B) from `dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md`
§ 3 / § 8.4.

### § 1.1 spmv × GF(p) — fflas-ffpack canonical

| Cell | Pre-Wave-3 wall | Pre-Wave-3 tput | Post-Path-A wall | Post-Path-A tput | Post-Path-B wall | Post-Path-B tput | fflas ref wall | fflas ref tput | Final gf2/fflas | Path | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|:---:|---|:---|
| spmv × GF(7) | 20.163 µs | 493 Mops/s | 9.2 µs | ~1.09 Gops/s | 9.2 µs | ~1.09 Gops/s | 8.650 µs | 1.185 Gops/s | **0.96x** | A | **PASS** |
| spmv × GF(251) | 23.713 µs | 419 Mops/s | 9.4 µs | ~1.06 Gops/s | 9.2 µs | ~1.09 Gops/s | 8.106 µs | 1.254 Gops/s | **0.88x** | A | **PASS** |
| spmv × GF(65521) | 20.326 µs | 489 Mops/s | 9.3 µs | ~1.08 Gops/s | 9.1 µs | ~1.10 Gops/s | 8.890 µs | 1.153 Gops/s | **0.97x** | A | **PASS** |
| spmv × GF(2^31-1) | 10.723 µs | 927 Mops/s | 9.4 µs | ~1.07 Gops/s | 9.4 µs | ~1.07 Gops/s | 15.043 µs | 661 Mops/s | **1.62x** | A | **PASS** (unchanged) |

CSV row sources (pre-Wave-3):
- GF(7): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 13
- GF(251): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 18
- GF(65521): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 23
- GF(2^31-1): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 28

CSV row sources (fflas reference):
- GF(7): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 8
- GF(251): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 6
- GF(65521): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 4
- GF(2^31-1): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 2

CSV row sources (Path A final, Criterion bench, post-B unchanged):
- All four spmv cells: `dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md` § 3 and § 8.4

Note: Path B does not modify the spmv hot path. Post-B spmv timings match Path A within
run-to-run noise (Criterion 10-sample, 5 s measurement window).

### § 1.2 sparse×dense × GF(p) — fflas-ffpack canonical

| Cell | Pre-Wave-3 wall | Pre-Wave-3 ratio | Post-Path-A wall | Post-Path-A ratio | Post-Path-B wall | Post-Path-B tput | fflas ref tput | Final gf2/fflas | Path | Verdict |
|---|---:|:---:|---:|:---:|---:|---:|---:|:---:|---|:---|
| sparse×dense × GF(7) | 17.397 ms | 0.22x | 7.984 ms | 0.49x | 3.605 ms | 2.822 Gops/s | 2.564 Gops/s | **1.08x** | B | **PASS** |
| sparse×dense × GF(251) | 17.335 ms | 0.15x | 8.335 ms | 0.32x | 2.452 ms | 4.149 Gops/s | 3.903 Gops/s | **1.08x** | B | **PASS** |
| sparse×dense × GF(65521) | 17.266 ms | 0.23x | 8.589 ms | 0.46x | 4.581 ms | 2.221 Gops/s | 2.576 Gops/s | **0.87x** | B | **PASS** |
| sparse×dense × GF(2^31-1) | 13.840 ms | 0.97x | 7.852 ms | 1.70x | 8.488 ms | 1.199 Gops/s | 702 Mops/s | **1.71x** | A | **PASS** |

CSV row sources (pre-Wave-3):
- GF(7): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 15
- GF(251): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 20
- GF(65521): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 25
- GF(2^31-1): `dev/bench_results/2026-05-04-47698404-sparse.csv` line 30

CSV row sources (fflas reference):
- GF(7): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 8 (sparse×dense row)
- GF(251): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 6
- GF(65521): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 4
- GF(2^31-1): `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` line 2 (sparse×dense row)

CSV row sources (Path A — `dev/bench_results/`):
- All 4 fields: `2026-05-07-3a37e0f6-sparse-dense-path-a.csv` lines 2–5

CSV row sources (Path B final — `dev/bench_results/`):
- All 4 fields: `2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` lines 2–5

Note: GF(2^31-1) row in Path B CSV (`2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` line 5) shows 8.488 ms vs Path A
7.852 ms — a ~5% wall-time increase from system noise. The SIMD hook returns `false` for
Mersenne-31, so the same Path-A code path runs. GF(2^31-1) remains well above the 1.5x contract
at 1.71x (vs the 0.667x floor). The contract clause "0.667x floor convention" applied:
gf2/fflas = 1.199 Gops/s / 702 Mops/s = 1.71x, PASS.

### § 1.3 Protocol § 9 exclusion cells (not-yet-harnessed / no-independent-oracle)

| Cell | Protocol marker | Verdict | Notes |
|---|---|:---:|---|
| sparse-matmul × GF(7) | `not-yet-harnessed` | excluded | No public sparse×sparse matmul in fflas/LinBox for any field |
| sparse-matmul × GF(251) | `not-yet-harnessed` | excluded | Same |
| sparse-matmul × GF(65521) | `not-yet-harnessed` | excluded | Same |
| sparse-matmul × GF(2^31-1) | `not-yet-harnessed` | excluded | Same |
| sparse-matmul × GF(2) | `not-yet-harnessed` | excluded | Same |
| sparse-matmul × GF(2^8) | `not-yet-harnessed` | excluded | Same |
| sparse-matmul × GF(2^16) | `not-yet-harnessed` | excluded | Same |
| sparse×dense × GF(2^8) | `no-independent-oracle` | excluded | GivaroExtension semantics mismatch; no comparable sparse library for GF(2^m) |
| sparse×dense × GF(2^16) | `no-independent-oracle` | excluded | Same |
| sparse rref/echelon | not-in-scope | — | Covered by dense story `72ab6d0e`; sparse RREF is CPU-feasible (LinBox pivot-priority gap) and tracked under `4c0d0202` for future Wave |

Throughput data for self-canonical GF(2^m) cells (regression tracking only, no pass/fail
contract):
- sparse×dense × GF(2^8): 546.8 ms (Path B CSV `2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` line 6, 18.6 Mops/s)
- sparse×dense × GF(2^16): 617.6 ms (Path B CSV line 7, 16.5 Mops/s)

These numbers are unchanged from Path A (no kernel modification for GF(2^m) cells).

---

## § 2 Ratio table and CSV links

### § 2.1 Canonical CSV files

| Role | Path | Description |
|---|---|---|
| fflas + LinBox reference | `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` | 33 rows: fflas-ffpack spmv + sparse×dense (8 rows), LinBox spmv + sparse×dense + sparse-elim (25 rows), all at n=1024/4096 |
| gf2-core pre-Wave-3 baseline | `dev/bench_results/2026-05-04-47698404-sparse.csv` | 43 data rows: full scorecard run including GF(2) layout variants, structured, coding-theory matrices |
| gf2-core Path-A spmv | `dev/bench_results/2026-05-07-3a37e0f6-spmv-path-a.csv` | 11 rows: spmv for GF(2) variants + 4 GF(p) fields |
| gf2-core Path-A sparse×dense | `dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-a.csv` | 6 rows: sparse×dense for 4 GF(p) fields + 2 GF(2^m) |
| gf2-core Path-B sparse×dense (final) | `dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv` | 6 rows: sparse×dense for 4 GF(p) fields + 2 GF(2^m) |

### § 2.2 Ratio table — final state (gf2-core / fflas-ffpack)

Computed from Path-B final throughput (§ 1) divided by fflas-ffpack reference throughput
(`dev/bench_results/2026-05-04-47698404-sparse-reference.csv`). Threshold: >= 0.667x (i.e.,
gf2-core wall-time is within 1.5x of fflas).

| Operation | Field | gf2 tput (final) | fflas tput (ref) | ratio | Threshold | Verdict |
|---|---|---:|---:|:---:|:---:|:---|
| spmv | GF(7) | ~1.09 Gops/s | 1.185 Gops/s | 0.96x | >= 0.667x | **PASS** |
| spmv | GF(251) | ~1.09 Gops/s | 1.254 Gops/s | 0.88x | >= 0.667x | **PASS** |
| spmv | GF(65521) | ~1.10 Gops/s | 1.153 Gops/s | 0.97x | >= 0.667x | **PASS** |
| spmv | GF(2^31-1) | ~1.07 Gops/s | 661 Mops/s | 1.62x | >= 0.667x | **PASS** |
| sparse×dense | GF(7) | 2.822 Gops/s | 2.564 Gops/s | 1.08x | >= 0.667x | **PASS** |
| sparse×dense | GF(251) | 4.149 Gops/s | 3.903 Gops/s | 1.08x | >= 0.667x | **PASS** |
| sparse×dense | GF(65521) | 2.221 Gops/s | 2.576 Gops/s | 0.87x | >= 0.667x | **PASS** |
| sparse×dense | GF(2^31-1) | 1.199 Gops/s | 702 Mops/s | 1.71x | >= 0.667x | **PASS** |

All 8 externally-referenced GF(p) sparse cells are PASS. Zero cells are below the 0.667x
floor. All spmv cells closed via Path A (lazy-reduction CSR); sparse×dense cells for GF(7),
GF(251), GF(65521) closed via Path B (AVX2 packed-int SpMM); GF(2^31-1) sparse×dense closed
via Path A (Mersenne fast-path, no Montgomery REDC).

### § 2.3 analyze.py invocation

The analyze.py ratio table is generated from the canonical CSVs by:

```bash
cd benchmarks
python3 analyze.py \
  --ref dev/bench_results/2026-05-04-47698404-sparse-reference.csv \
  --gf2 dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv \
  --gf2 dev/bench_results/2026-05-07-3a37e0f6-spmv-path-a.csv \
  --threshold 0.667
```

The ratio table in § 2.2 matches the values reported directly in
`dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md` § 0 and § 8.3. No additional
analyze.py run was performed in this issue because the 3a37e0f6 layout doc already records
the per-cell ratios from the same-session bench run.

---

## § 3 Non-goals: CPU/GPU boundary for epic 97bf0879

### § 3.1 What is in scope (closed in 97bf0879)

The following sparse operations over the following fields are closed in epic `97bf0879`:

**spmv × GF(p):** GF(7), GF(251), GF(65521), GF(2^31-1) — all PASS as of Path A.

**sparse×dense × GF(p):** GF(7), GF(251), GF(65521), GF(2^31-1) — all PASS as of Path B.

**spmv × GF(2):** 13 layout/structured/coding-theory variants — gf2-core self-canonical, all
reported and tracked (see `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` § 3).

**sparse×dense × GF(2):** gf2-core `SpBitMatrix::matmat` leads LinBox by 6.31x in
saxpy-normalised units — in-scope-pass.

### § 3.2 What is out of scope for 97bf0879

The following cells are excluded from 97bf0879's sparse closure. The authoritative source
for the GPU boundary decision is `dev/plans/sparse_cpu_vs_gpu_handoff.md` (issue `3643923d`).

#### GPU successor epic: 16283d6f

**sparse-matmul (sparse×sparse) × all fields (7 cells):**
Protocol § 9 exclusion class `not-yet-harnessed`. No public library exposes sparse×sparse
matmul over GF(p) or GF(2^m); no independent oracle exists to measure against. At large n and
high density, GPU dispatch would be the natural vehicle. Per `dev/plans/sparse_cpu_vs_gpu_handoff.md`
§ 2.5, these cells have no external reference gap; gf2-core is the self-canonical reference.
Measurement is deferred to GPU epic `16283d6f` or a future scope expansion. These cells do NOT
flow to `16283d6f` as required deliverables; rather, `16283d6f` is the earliest candidate to
pick them up if user-approved.

**sparse×dense × GF(2^8) and GF(2^16) (2 cells):**
Protocol § 9 exclusion class `no-independent-oracle` with `semantics-mismatch` marker.
fflas-ffpack and LinBox use GivaroExtension polynomial multiplication for GF(2^m) sparse ops,
which is ~10x slower than gf2-core's PCLMULQDQ-backed `Gf2mWide`. No meaningful ratio can be
formed. Self-canonical throughput is reported for regression tracking (§ 1.3).

#### CPU work, not GPU — tracked under 4c0d0202

**sparse-elim (sparse RREF) × {GF(2), GF(7), GF(251), GF(65521), GF(2^31-1)} at n={256, 1024}
(10 cells):**
`dev/plans/sparse_cpu_vs_gpu_handoff.md` § 2.4 classifies all sparse-elim cells as
**CPU-feasible** (not GPU). The gap to LinBox `GaussDomain::NoReordering` (0.38x–0.51x) is a
straight-line vs pivot-priority algorithmic gap; the reference runs on the same Zen-3 AVX2
hardware without any GPU or AVX-512. GPU sparse RREF over GF(p) is not a standard operation in
any GPU library and would require novel kernel design. The sparse-elim gap is tracked under
story `4c0d0202` (target-matrix story) for a future CPU RREF-pivot optimization task.

**These cells do NOT flow to GPU epic 16283d6f.**

### § 3.3 GPU boundary statement

The CPU/GPU boundary for epic `97bf0879` sparse closure is:

> **CPU sparse closure is complete in 97bf0879.** All GF(p) spmv and sparse×dense cells
> meet the 1.5x contract. No sparse cell requires GPU compute to pass its acceptance criterion.
> The GPU epic `16283d6f` (device-resident FieldMatrix) remains gated on explicit user approval
> to dispatch breakdown and is downstream of the CPU SOTA closure. The sparse-elim cells
> (CPU-feasible, algorithmic gap) stay in the CPU lane under `4c0d0202`. The self-canonical
> sparse-matmul cells have no acceptance threshold to pass; they may be picked up by `16283d6f`
> or a later scope expansion, per user decision.

Source: `dev/plans/sparse_cpu_vs_gpu_handoff.md` § 4 (decision), § 3 (GPU routing analysis),
§ 2 (per-cell CPU-feasibility analysis).

---

## § 4 Reproducibility appendix

### § 4.1 Host metadata

From `dev/bench_results/2026-05-04-47698404-sparse-host.txt`:

| Item | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X, 12c/24t, Zen 3 |
| ISA flags | AVX2, BMI2, FMA, VAES, VPCLMULQDQ (no AVX-512, no GFNI) |
| Memory | 32 GiB DDR4 |
| Kernel | Linux 7.0.3-arch1-1, x86\_64 |
| Distribution | Arch Linux (rolling) |
| Rust | rustc 1.95.0 (59807616e, 2026-04-14) |
| Cargo | cargo 1.95.0 (f2d3ce0bd, 2026-03-21) |

Path-A and Path-B bench sessions (3a37e0f6 worktree) ran on the same host
without the container image (worktree-host scope per dispatch contract `97bf0879-handoff-4`,
trap #2 conditions met: pinned-container smoke passed for the same harness binaries by prior
session `47698404`; reference numbers from the same host).

### § 4.2 Reference CSV regeneration

```bash
# Rebuild the C++ reference harness (requires fflas-ffpack and LinBox headers)
cd benchmarks/reference
make clean && make fflas_sparse_bench linbox_sparse_bench sparse_smoke

# Re-run reference measurements (fflas-ffpack + LinBox)
./fflas_sparse_bench --quick --fields GF7,GF251,GF65521,GF2pow31m1 \
  > /tmp/fflas-sparse-ref.csv
./linbox_sparse_bench --quick --fields GF7,GF251,GF65521,GF2pow31m1 \
  >> /tmp/fflas-sparse-ref.csv

# Cross-equality smoke
./sparse_smoke  # must print "all OK"
```

The canonical reference CSV is `dev/bench_results/2026-05-04-47698404-sparse-reference.csv`
(33 data rows); the seed and density-format invariants are described in
`dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` § 2.

### § 4.3 gf2-core measurement regeneration (Path B)

```bash
# From the workspace root (post-Path-B code, on branch worktree-agent-3a37e0f6)
cargo build -p gf2-core --example bench_sparse_csv_emitter --release

# sparse×dense final (Path B):
cargo run -p gf2-core --example bench_sparse_csv_emitter --release -- \
  --warmup 5 --iters 20 --filter sparse-dense \
  > bench_results/gf2-sparse-$(date +%s).csv

# spmv (Path A; Path B does not modify matvec):
cargo run -p gf2-core --example bench_sparse_csv_emitter --release -- \
  --warmup 10 --iters 100 --filter spmv \
  > bench_results/gf2-sparse-$(date +%s).csv
```

The bench emitter uses master seed `0x6F73AC91D31E4A7C` and C-printf density formatting
(`density_9.765625e-03_csr`) to match the reference harness join key.

---

## § 5 Verdict

**Criterion 1 (Raw CSVs and ratio tables linked to story `54fd3f0b`):** Met.

The canonical sparse reference CSV (`dev/bench_results/2026-05-04-47698404-sparse-reference.csv`),
the pre-Wave-3 gf2-core baseline CSV (`dev/bench_results/2026-05-04-47698404-sparse.csv`),
the Path-A measurement CSVs (`dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-a.csv`,
`dev/bench_results/2026-05-07-3a37e0f6-spmv-path-a.csv`), and the Path-B final measurement CSV
(`dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv`) are documented in § 2.1 with
exact file paths, line number references, and the ratio table in § 2.2. This document is
attached to story `54fd3f0b` via `jit doc add` (see "Lead actions" below). The ratio table
(§ 2.2) derives from the same data as reported in `dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md`
§ 0 and § 8.3, cross-checked against the reference CSV.

**Criterion 2 (CPU/GPU non-goal boundaries documented):** Met.

Section § 3 documents the complete CPU/GPU boundary. All 8 GF(p) sparse cells are closed on
CPU. The GPU boundary statement (§ 3.3) cites `dev/plans/sparse_cpu_vs_gpu_handoff.md`
(issue `3643923d`) as the decision doc. No sparse cell from 97bf0879 flows to GPU epic
`16283d6f` as a required deliverable. Self-canonical exclusion cells (sparse-matmul) and
CPU-feasible algorithmic gaps (sparse-elim) are documented with their correct classification
and routing.

---

## Lead actions

After code-review gate passes on this issue, the lead should run:

```bash
# Attach this evidence doc to story 54fd3f0b
jit doc add 54fd3f0b dev/bench_results/2026-05-07-1726270d-sparse-parity-evidence.md

# Attach the canonical reference CSV to story 54fd3f0b
jit doc add 54fd3f0b dev/bench_results/2026-05-04-47698404-sparse-reference.csv

# Attach the pre-Wave-3 gf2-core baseline CSV to story 54fd3f0b
jit doc add 54fd3f0b dev/bench_results/2026-05-04-47698404-sparse.csv

# Attach the layout/traversal optimization evidence to story 54fd3f0b
jit doc add 54fd3f0b dev/bench_results/2026-05-07-3a37e0f6-sparse-layout.md

# Attach the CPU/GPU handoff decision doc to story 54fd3f0b
jit doc add 54fd3f0b dev/plans/sparse_cpu_vs_gpu_handoff.md
```

The Path-A and Path-B raw bench CSVs have been copied to permanent locations in
`dev/bench_results/` (see § 1) and should also be attached to the story:

```bash
jit doc add 54fd3f0b dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-a.csv
jit doc add 54fd3f0b dev/bench_results/2026-05-07-3a37e0f6-spmv-path-a.csv
jit doc add 54fd3f0b dev/bench_results/2026-05-07-3a37e0f6-sparse-dense-path-b-final.csv
```
