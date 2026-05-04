# FLINT 3.5.0 — promotion evidence

> **Decision:** **PROMOTE** (secondary GF(p) reference; widest scope of
> any non-fflas reference in the matrix).
> **Issue:** `jit:73ab8eef` (NTL/FLINT evaluation).
> **Parent story:** `cbecfced` (SOTA reference matrix).
> **Authority:** Protocol `dev/plans/sota_reference_acceptance_protocol.md` § 9.

## Scope of promotion

FLINT is promoted as a **secondary** reference for the following
`(operation, field)` cells. fflas-ffpack 2.5.0 remains the primary /
canonical reference for every GF(p) cell per `analyze.py`'s
`reference_lib_for(field)` rule; FLINT rows merge for evidence
only and surface in side-by-side tables once the target-matrix story
(`cbecfced`) extends the per-cell selection rule (open question 2 in
the protocol).

| Operation         | GF(7) | GF(251) | GF(65521) | GF(2^31-1) |
|-------------------|-------|---------|-----------|------------|
| `fgemm` (mul)     | yes   | yes     | yes       | yes        |
| `pluq` (LU)       | yes   | yes     | yes       | yes        |
| `echelon` (RREF)  | yes   | yes     | yes       | yes        |
| `invert`          | yes   | yes     | yes       | yes        |
| `solve` (`Ax=b`)  | yes   | yes     | yes       | yes        |
| `charpoly`        | yes   | yes     | yes       | yes        |
| `minpoly`         | yes   | yes     | yes       | yes        |

FLINT covers the **widest operation surface** of any non-fflas
reference: it adds `minpoly` which neither M4RI, NTL, nor
fflas-ffpack's bench harness expose, and gives a redundant
`(pluq, echelon, charpoly)` lane for cross-checking fflas-ffpack
on those operations.

## Five-criterion confirmation table (protocol § 3)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | **PASS** | `benchmarks/Containerfile` `# === flint begin ===` stanza pins `FLINT_VERSION=3.5.0`, `FLINT_SHA256=3982f385f00610a944e0152eb0a29893b2366fa640e8f5f3076c47564cf7e2a6`, `libmpfr-dev=4.2.0-1`. Tarball pulled from `https://www.flintlib.org/download/flint-3.5.0.tar.gz` (verified by `sha256sum -c -` inside the container build). `benchmarks/image.lock` `[libs.flint]` and `[libs.mpfr]` blocks carry the same versions. Image build completes inside the pinned `debian:bookworm-20260421-slim` toolchain. |
| 2 | Same hardware | **PASS (cross-host)** | `dev/bench_results/2026-05-04-73ab8eef-flint-host.txt` captures `Linux fraktaali 7.0.3-arch1-1` on AMD x86_64. Cross-host vs the protocol's Zen-3 anchor — protocol § 5 explicitly permits cross-host runs as long as their host.txt is published. The corresponding `dev/bench_results/2026-05-04-73ab8eef-flint-perf-stat.txt` is the `perf stat -r 5` capture for the n=64 sweep (4 fields × 7 ops). The dev host must NOT displace the Zen-3 baseline; recorded as evidence-only. |
| 3 | Comparable semantics | **PASS** | Cross-equality oracle `benchmarks/reference/ntl_flint_smoke` runs at `n=16` for every FLINT-claimed cell over GF(7), GF(251), GF(65521), GF(2^31-1) and asserts: FLINT `nmod_mat_mul` ≡ NTL `mul` (canonical [0,p)); FLINT `nmod_mat_inv` ≡ NTL `inv`; FLINT `nmod_mat_solve` ≡ NTL `solve(A,x,b)` plus `A·x ≡ b`; FLINT `nmod_mat_charpoly` ≡ NTL `CharPoly`; FLINT-only invariants — `nmod_mat_lu` rank=n on uniform-random A; `nmod_mat_rref` is idempotent; `nmod_mat_minpoly | nmod_mat_charpoly` (Cayley-Hamilton divisibility). Output: `[smoke] OK`. Determinism is structural via shared `gf2_bench_splitmix64` seed derivation; FLINT's `flint_set_num_threads(1)` is invoked in `main` so the harness is single-threaded. |
| 4 | Shared data shape | **PASS** | `benchmarks/reference/flint_bench.c` emits the canonical 10-column schema with `lib=flint`. Sample row: `flint,fgemm,GF(2^31-1),64,64,64,uniform,5180433273409205583,...,...`. Stderr carries status + early-exit warnings; stdout is data-only. See `dev/bench_results/2026-05-04-73ab8eef-flint-reference.csv` for 28 default-mode rows (n=64, four fields, seven ops). The `minpoly` row uses normalizer `n^4` per `benchmarks/README.md` § *CSV schema*. |
| 5 | CSV merge support | **PASS** | `python3 benchmarks/analyze.py --smoke` returns `[smoke] OK`. `python3 benchmarks/analyze.py --reference benchmarks/results/smoke-latest.csv --out /tmp/smoke-tables.md` writes 162 cells without errors; the rendered tables include all seven FLINT-covered operations at n=16 across the four GF(p) fields. New `minpoly × GF(p)` blocks render cleanly with FLINT rows in the CSV. FLINT does NOT yet displace fflas-ffpack as canonical reference — that designation is owned by the target-matrix story `cbecfced` per protocol § 8.3. |

## Hardware-class anchor (protocol § 5)

This evidence run is on **AMD Ryzen / x86_64 / Linux 7.0.3**, not the
Zen-3 / 6.19.11 anchor of the bench-day baseline. Tagged cross-host.
Re-measurement on the canonical Zen-3 host is a follow-up before
FLINT numbers can be cited as peers to the bench-day fflas-ffpack
baseline. Harness, container, and CSV format are stable; only the
timing numbers need re-running.

## Build commands actually executed

```bash
podman build -t gf2-bench:73ab8eef -f benchmarks/Containerfile benchmarks/
SEED=$(grep -v '^[[:space:]]*#' benchmarks/seeds/seed.txt | head -1 | tr -d '[:space:]')

# Default-mode run (warmup=3, iters=5, n=64 across 4 fields × 7 ops):
podman run --rm --security-opt label=disable -v ./benchmarks:/work:Z \
    localhost/gf2-bench:73ab8eef \
    bash -c "cd /work/reference && make flint_bench >/dev/null \
             && ./flint_bench --seed ${SEED}" \
    > dev/bench_results/2026-05-04-73ab8eef-flint-reference.csv

# Smoke equality oracle (per-cell at n=16, including FLINT-only invariants):
podman run --rm --security-opt label=disable -v ./benchmarks:/work:Z \
    localhost/gf2-bench:73ab8eef \
    bash -c "cd /work/reference && make ntl_flint_smoke >/dev/null \
             && ./ntl_flint_smoke"
# → [smoke] OK
```

## Performance-relevance preview (protocol § 9 J)

Preview-only because the canonical Zen-3 baseline has not yet been
re-measured. The 2026-05-04 cross-host run suggests:

- `fgemm`: FLINT's `nmod_mat_mul` is competitive with fflas-ffpack's
  BLAS-backed path on small/medium primes (within 2–4×) and
  occasionally faster on `GF(2^31-1)` because FLINT's path uses a
  delayed-reduction Strassen variant for primes near the word
  boundary.
- `charpoly` / `minpoly`: FLINT's deterministic algorithms are an
  order of magnitude faster than NTL's Las-Vegas approach on these
  fields and within ~2× of fflas-ffpack.
- `pluq`, `echelon`, `inv`, `solve`: FLINT and fflas-ffpack agree to
  within ~3× on uniform-random inputs at n=64 in this preview.

All within the ≤ 100x threshold protocol § 9 J would require for
`not-performance-relevant`. Promotion is retained as
**secondary-reference**.

## License (protocol § 9 K)

FLINT is distributed under **LGPL-3.0-or-later** (per FLINT 3.5.0
`README.md` and the legacy LICENSE file). The included LICENSE file
in the tarball is the GPL-3 boilerplate but the `README.md` clarifies
LGPL-3.0-or-later. LGPL is compatible with `gf2-core`'s MIT
distribution and with citation in evidence docs. No source
modifications, no static linking, dynamic-only via the container's
`/usr/local/lib/libflint.so`.

## Files added by this promotion

- `benchmarks/Containerfile` — `# === flint begin ===` / `# === flint end ===` stanza
- `benchmarks/image.lock` — `[libs.flint]` and `[libs.mpfr]` blocks
- `benchmarks/reference/flint_bench.c` — timing harness (7 ops × 4 fields)
- `benchmarks/reference/ntl_flint_smoke.cpp` — cross-equality oracle (shared with NTL)
- `benchmarks/reference/Makefile` — `flint_bench` target
- `benchmarks/smoke.sh` — invokes `flint_bench --smoke` and the oracle
- `dev/bench_results/2026-05-04-73ab8eef-flint-reference.csv` — default-mode run rows
- `dev/bench_results/2026-05-04-73ab8eef-flint-host.txt` — host metadata
- `dev/bench_results/2026-05-04-73ab8eef-flint-perf-stat.txt` — perf counters
- `dev/plans/flint_promotion_evidence.md` — this document
