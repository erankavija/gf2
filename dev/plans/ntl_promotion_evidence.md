# NTL 11.6.0 — promotion evidence

> **Decision:** **PROMOTE** (secondary GF(p) reference).
> **Issue:** `jit:73ab8eef` (NTL/FLINT evaluation).
> **Parent story:** `cbecfced` (SOTA reference matrix).
> **Authority:** Protocol `dev/plans/sota_reference_acceptance_protocol.md` § 9.

## Scope of promotion

NTL is promoted as a **secondary** reference for the following
`(operation, field)` cells. fflas-ffpack 2.5.0 remains the primary /
canonical reference for every GF(p) cell per `analyze.py`'s
`reference_lib_for(field)` rule; NTL rows are merged for evidence
only and surface in side-by-side tables once the target-matrix story
(`cbecfced`) extends the per-cell selection rule (open question 2 in
the protocol).

| Operation | GF(7) | GF(251) | GF(65521) | GF(2^31-1) |
|---|---|---|---|---|
| `fgemm` (mul)  | yes | yes | yes | yes |
| `invert`       | yes | yes | yes | yes |
| `solve`        | yes | yes | yes | yes |
| `charpoly`     | yes | yes | yes | yes |

Operations explicitly **not** covered by NTL in this harness:

- `pluq` — NTL's `mat_zz_p` API does not expose a public PLUQ /
  PLE entry-point at the level fflas-ffpack does. Internal LU is
  used by `inv` / `solve` but is not callable directly. Out of
  scope for this harness; covered by FLINT's `nmod_mat_lu`.
- `echelon` (RREF) — same limitation; NTL's `gauss(M)` returns
  a rank but does not expose the full RREF used by
  `protocol §6` on cell `echelon`. Covered by FLINT's
  `nmod_mat_rref`.
- `minpoly` — NTL provides `MinPolyMod(...)` for polynomials but
  not a direct `MinPoly(mat_zz_p)` at the user-facing API level.
  Covered by FLINT's `nmod_mat_minpoly`.

## Five-criterion confirmation table (protocol § 3)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | **PASS** | `benchmarks/Containerfile` `# === ntl begin ===` stanza pins `NTL_VERSION=11.6.0`, `NTL_SHA256=bc0ef9aceb075a6a0673ac8d8f47d5f8458c72fe806e4468fbd5d3daff056182`. Tarball pulled from `https://libntl.org/ntl-11.6.0.tar.gz` (verified by `sha256sum -c -` inside the container build). `benchmarks/image.lock` `[libs.ntl]` block carries the same version/source/sha256. Image build completes inside the pinned `debian:bookworm-20260421-slim` toolchain (`gcc-12.2.0-14+deb12u1`). |
| 2 | Same hardware | **PASS (cross-host)** | `dev/bench_results/2026-05-04-73ab8eef-ntl-host.txt` captures `Linux fraktaali 7.0.3-arch1-1` on AMD x86_64. This is a **cross-host run** vs the protocol's Zen-3 anchor — protocol § 5 explicitly permits cross-host runs as long as their host.txt is published. The corresponding `dev/bench_results/2026-05-04-73ab8eef-ntl-perf-stat.txt` is the `perf stat -r 5` capture for the n=64 sweep. The dev host must NOT displace the Zen-3 baseline; it is recorded as evidence-only. |
| 3 | Comparable semantics | **PASS** | Cross-equality oracle `benchmarks/reference/ntl_flint_smoke` runs at `n=16` for every NTL-claimed cell over GF(7), GF(251), GF(65521), GF(2^31-1) and asserts: NTL `mul` ≡ FLINT `nmod_mat_mul` (canonical [0,p)); NTL `inv` ≡ FLINT `nmod_mat_inv`; NTL `solve(d, A, x, b)` (column-vector convention `A·x = b`) gives `x_ntl ≡ x_flint` and both satisfy `A·x = b`; NTL `CharPoly` ≡ FLINT `nmod_mat_charpoly` (monic, canonical). Output: `[smoke] OK`. Determinism is structural: matrix entries come from the shared `gf2_bench_splitmix64` / `gf2_bench_derive_seed` from `benchmarks/reference/seed_helpers.h`, and NTL's `SetSeed` is wired so `CharPoly`'s internal Las-Vegas randomness is reproducible at the same master seed. |
| 4 | Shared data shape | **PASS** | `benchmarks/reference/ntl_bench.cpp` emits the canonical 10-column schema (`lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops`) with `lib=ntl`. Sample row: `ntl,fgemm,GF(2^31-1),64,64,64,uniform,5180433273409205583,...,...`. Stderr carries status + early-exit warnings; stdout is data-only. See `dev/bench_results/2026-05-04-73ab8eef-ntl-reference.csv` for 16 default-mode rows (n=64, four fields, four ops). |
| 5 | CSV merge support | **PASS** | `python3 benchmarks/analyze.py --smoke` returns `[smoke] OK`. `python3 benchmarks/analyze.py --reference benchmarks/results/smoke-latest.csv --out /tmp/smoke-tables.md` writes 162 cells without errors; the rendered tables include all four `fgemm × GF(p)`, `invert × GF(p)`, `solve × GF(p)`, and `charpoly × GF(p)` blocks with NTL rows present. NTL does NOT yet displace fflas-ffpack as canonical reference — that designation is owned by the target-matrix story `cbecfced` per protocol § 8.3. |

## Hardware-class anchor (protocol § 5)

This evidence run is on **AMD Ryzen / x86_64 / Linux 7.0.3**, not the
Zen-3 / 6.19.11 anchor of the bench-day baseline. The evidence is
therefore tagged as cross-host. Re-measurement on the canonical Zen-3
host is a follow-up requirement before NTL numbers can be cited as
peers to the bench-day fflas-ffpack baseline. The harness, container,
and CSV format are stable; only the timing numbers need re-running on
the canonical host.

## Build commands actually executed

```bash
podman build -t gf2-bench:73ab8eef -f benchmarks/Containerfile benchmarks/
SEED=$(grep -v '^[[:space:]]*#' benchmarks/seeds/seed.txt | head -1 | tr -d '[:space:]')

# Default-mode run (warmup=3, iters=5, n=64 across 4 fields × 4 ops):
podman run --rm --security-opt label=disable -v ./benchmarks:/work:Z \
    localhost/gf2-bench:73ab8eef \
    bash -c "cd /work/reference && make ntl_bench >/dev/null \
             && ./ntl_bench --seed ${SEED}" \
    > dev/bench_results/2026-05-04-73ab8eef-ntl-reference.csv

# Smoke equality oracle (per-cell at n=16):
podman run --rm --security-opt label=disable -v ./benchmarks:/work:Z \
    localhost/gf2-bench:73ab8eef \
    bash -c "cd /work/reference && make ntl_flint_smoke >/dev/null \
             && ./ntl_flint_smoke"
# → [smoke] OK
```

## Performance-relevance preview (protocol § 9 J)

This is **preview-only** because the canonical Zen-3 baseline has not
yet been re-measured. The 2026-05-04 cross-host run on Arch / x86_64
suggests:

- For `fgemm`, NTL's `mat_zz_p` mul is materially slower than
  fflas-ffpack's BLAS-backed path on the same primes. At n=64
  GF(2^31-1) on Arch 7.0.3: NTL ≈ 1.7 GFlops vs fflas (per
  `2026-04-26-reference.csv`) on Zen-3 ≈ 12 GFlops — roughly an
  order of magnitude gap, but NTL stays within the ≤ 100x threshold
  protocol § 9 J would require for `not-performance-relevant`.
- For `charpoly`, NTL's `CharPoly` is broadly competitive with
  fflas-ffpack at small n; deeper comparison deferred to the
  target-matrix story.

The promotion is therefore retained on the **secondary-reference**
basis: the rows merge, the harness is reproducible, and they provide
a cross-check / triangulation reference for cells where fflas-ffpack
might silently drift.

## License (protocol § 9 K)

NTL is distributed under **LGPL-2.1-or-later**
(`ntl-11.6.0/doc/copying.txt`). LGPL is compatible with `gf2-core`'s
MIT distribution and with citation in evidence docs. No source
modifications, no static linking, dynamic-only via the container's
`/usr/local/lib/libntl.so`.

## Files added by this promotion

- `benchmarks/Containerfile` — `# === ntl begin ===` / `# === ntl end ===` stanza
- `benchmarks/image.lock` — `[libs.ntl]` block
- `benchmarks/reference/ntl_bench.cpp` — timing harness (4 ops × 4 fields)
- `benchmarks/reference/ntl_flint_smoke.cpp` — cross-equality oracle
- `benchmarks/reference/Makefile` — `ntl_bench` and `ntl_flint_smoke` targets
- `benchmarks/smoke.sh` — invokes `ntl_bench --smoke` and the oracle
- `dev/bench_results/2026-05-04-73ab8eef-ntl-reference.csv` — default-mode run rows
- `dev/bench_results/2026-05-04-73ab8eef-ntl-host.txt` — host metadata
- `dev/bench_results/2026-05-04-73ab8eef-ntl-perf-stat.txt` — perf counters
- `dev/plans/ntl_promotion_evidence.md` — this document
