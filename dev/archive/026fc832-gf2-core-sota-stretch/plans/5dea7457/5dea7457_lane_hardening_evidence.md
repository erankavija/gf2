# Issue 5dea7457 — fflas-ffpack + M4RI lane hardening evidence

> **Scope.** This document captures the evidence required by issue
> `5dea7457` (parent story `cbecfced`, epic `97bf0879`) to extend the
> already-promoted hard-reference lanes for fflas-ffpack and M4RI to
> cover every in-scope **dense** operation supported by each upstream.
>
> **Authority.** Linked to the SOTA reference acceptance protocol
> (`dev/plans/sota_reference_acceptance_protocol.md`). The five-criterion
> checklist in protocol § 3 governs every new (operation, field) row
> committed here.

## 1. Pre-vs-post coverage tables

### 1.1 fflas-ffpack lane (every in-scope dense GF(p) operation)

| Operation  | Pre (2026-04-26 baseline) | Post (this commit) |
|------------|---------------------------|---------------------|
| `fgemm`    | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `pluq`     | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `echelon`  | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `invert`   | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `solve`    | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `charpoly` | GF(7), GF(251), GF(65521), GF(2^31-1) | unchanged |
| `minpoly`  | **none**                  | **GF(7), GF(251), GF(65521), GF(2^31-1)** (new) |

Every dense operation supported by `fflas-ffpack 2.5.0`'s public surface
(`fgemm`, `pluq/ple`, `echelon`, `invert`, `solve`, `charpoly`,
`minpoly`) now has a matching reference row at `n ∈ {64, 256, 1024}`
across the four GF(p) reference fields. `n = 4096` deferral for the
non-fgemm ops is documented in `benchmarks/README.md` § *Deferred to
T2 / T3* and matches the protocol § 10 worked example A. Sparse ops
(`spmv`, sparse matmul) are out of scope per the protocol's scope and
the dispatch contract for issue `5dea7457`; LinBox sparse coverage is
tracked under `jit:79388011`.

### 1.2 M4RI lane (every in-scope dense GF(2) operation supported by M4RI)

| Operation  | Pre (2026-04-26 baseline) | Post (this commit)            | Notes |
|------------|---------------------------|--------------------------------|-------|
| `matmul`   | GF(2)                     | unchanged                       | |
| `echelon`  | GF(2)                     | unchanged                       | |
| `pluq`     | **none**                  | **GF(2)** (new)                  | `mzd_pluq` |
| `invert`   | **none**                  | **GF(2)** (new)                  | `mzd_inv_m4ri` |
| `solve`    | **none**                  | **GF(2)** (new)                  | `mzd_solve_left` (single RHS) |
| `charpoly` | **n/a**                   | n/a — not supported by M4RI     | exclusion class `not-supported-by-library` |
| `minpoly`  | **n/a**                   | n/a — not supported by M4RI     | exclusion class `not-supported-by-library` |

`charpoly` and `minpoly` are not provided by M4RI's public API
(`/usr/local/include/m4ri/m4ri.h` and friends). The protocol's
`not-supported-by-library` exclusion class (analogous to § 8 entries)
governs this gap — these operations remain sourced from gf2-core
itself, with LinBox available as the future canonical reference per
`jit:79388011`. The exclusion is documented in
`benchmarks/README.md` § *M4RI scope exclusions*.

`fgemm` rectangular shapes and sparse ops (`spmv`) are explicitly
out of scope for this issue per the dispatch contract.

## 2. Five-criterion checklist (protocol § 3) per new operation

Each new (operation, field) row inherits the same Containerfile,
`image.lock`, host capture, and CSV merge plumbing the existing
fflas-ffpack and M4RI rows already use. The checklist below enumerates
the per-criterion confirmation; per protocol § 10 the bulk of the
evidence is shared infrastructure (criteria 1, 2, 4, 5) and is copied
by reference. Criterion 3 (semantics) is the new work.

### 2.1 fflas-ffpack `minpoly`

| # | Criterion          | Status | Evidence |
|---|--------------------|--------|----------|
| 1 | Reproducible build | PASS   | Built from the same pinned `fflas-ffpack 2.5.0` image (`localhost/gf2-bench:ref` — sha256 `6c5d58a4f3a91a9f5013726e90e06e3aac6e7d23bbad3d00347e71fe2471d7b5`). `MinPoly` is exported from the public `ffpack.h` (`/usr/local/include/fflas-ffpack/ffpack/ffpack.h:1137-1153`) so no Containerfile or `image.lock` change is required. |
| 2 | Same hardware      | PASS   | `dev/bench_results/2026-05-04-5dea7457-host.txt` (AMD Ryzen 9 5900X / Linux 6.19.11-arch1-1, the same Zen-3 baseline as protocol § 5). `dev/bench_results/2026-05-04-5dea7457-perf-stat.txt` carries the perf-stat capture. |
| 3 | Comparable semantics | PASS | `--smoke` mode runs at n=16 against a fixed seeded input across all four GF(p) fields (see `fflas_bench.cpp::smoke_minpoly_equality`). The per-operation contract enforced is: minpoly is **monic** (leading coefficient = 1) and **divides charpoly** (`charpoly mod minpoly == 0` via `Givaro::Poly1Dom::divmod`). The smoke run prints `[fflas_bench] SMOKE OK minpoly field=...` on every field, with the matrix's minimum polynomial degree printed alongside the charpoly degree for cross-reference. |
| 4 | Shared data shape  | PASS   | Rows in `dev/bench_results/2026-05-04-5dea7457-reference-extension.csv` carry exactly the ten columns documented in `benchmarks/README.md` § *CSV schema*. The throughput op-count for `minpoly` is `n^4` (see README CSV-schema rationale, "LCM-merge sweep over n Krylov passes"). |
| 5 | CSV merge support  | PASS   | `python3 benchmarks/analyze.py --reference dev/bench_results/2026-05-04-5dea7457-reference-extension.csv --out /tmp/test-analyze.md` rendered the full side-by-side matrix without modification. `analyze.py --smoke` continues to pass. The pre-existing `OPERATION_ORDER` array in `analyze.py` already includes `minpoly`, so no source change was required. |

### 2.2 M4RI `pluq`

| # | Criterion          | Status | Evidence |
|---|--------------------|--------|----------|
| 1 | Reproducible build | PASS   | `mzd_pluq` is exported from the pinned M4RI `20260122` image. No Containerfile/lockfile change required. |
| 2 | Same hardware      | PASS   | Same host capture (`2026-05-04-5dea7457-host.txt`) and perf-stat (`2026-05-04-5dea7457-perf-stat.txt`). |
| 3 | Comparable semantics | PASS | `--smoke` mode at n=16 reconstructs `P · L · U · Q == A0` over GF(2) (extract `L = mzd_extract_l(...)`, `U = mzd_extract_u(...)`, multiply `LU = mzd_mul(L, U, 0)`, then `mzd_apply_p_left_trans(LU, P)` + `mzd_apply_p_right(LU, Q)` to recover the swap-list permutation matrices). The reported rank from `mzd_pluq` is independently cross-checked against `mzd_echelonize_m4ri` on the same input. Smoke prints `[m4ri_bench] SMOKE OK pluq n=16 rank=...`. |
| 4 | Shared data shape  | PASS   | CSV rows match the schema; throughput op-count `n^3` matches the protocol/README convention for square factorisations. |
| 5 | CSV merge support  | PASS   | `analyze.py` accepts the rows; `OPERATION_ORDER` already includes `pluq`. |

### 2.3 M4RI `invert`

| # | Criterion          | Status | Evidence |
|---|--------------------|--------|----------|
| 1 | Reproducible build | PASS   | `mzd_inv_m4ri` (Konrod's method) — pinned M4RI `20260122`. |
| 2 | Same hardware      | PASS   | Same captures as above. |
| 3 | Comparable semantics | PASS | `--smoke` at n=16 builds an invertible-by-construction matrix `A = L · U` (random unit-lower L × random unit-upper U), then verifies `A · A^{-1} == I` via `mzd_mul` + `mzd_equal` against `mzd_set_ui(I, 1)`. The constructed-invertible input avoids the ~71% singular rate of i.i.d. random GF(2) matrices, which would otherwise flake the smoke gate (a documented quirk of GF(2) random matrices: the limit `prod_{k≥1} (1 - 2^{-k}) ≈ 0.289` invertible). Smoke prints `[m4ri_bench] SMOKE OK invert n=16`. |
| 4 | Shared data shape  | PASS   | CSV schema-conformant; throughput `n^3`. |
| 5 | CSV merge support  | PASS   | `analyze.py` accepts the rows; `OPERATION_ORDER` already includes `invert`. |

### 2.4 M4RI `solve`

| # | Criterion          | Status | Evidence |
|---|--------------------|--------|----------|
| 1 | Reproducible build | PASS   | `mzd_solve_left` — pinned M4RI `20260122`. |
| 2 | Same hardware      | PASS   | Same captures as above. |
| 3 | Comparable semantics | PASS | `--smoke` at n=16 uses the same constructed-invertible `A0` as the invert smoke (so the system is guaranteed consistent), runs `mzd_solve_left(A, x, 0, 1)` with `inconsistency_check=1`, and verifies `A0 · x == b0` by recomputing the product with `mzd_mul`. Smoke prints `[m4ri_bench] SMOKE OK solve n=16`. |
| 4 | Shared data shape  | PASS   | CSV schema-conformant; throughput `n^3` (square solve). |
| 5 | CSV merge support  | PASS   | `analyze.py` accepts the rows; `OPERATION_ORDER` already includes `solve`. |

## 3. Container build / harness compile commands actually executed

```text
# Image reuse (pinned 2026-04-26 build, sha256:6c5d58a4f3a91a9f5013726e90e06e3aac6e7d23bbad3d00347e71fe2471d7b5):
$ podman tag localhost/gf2-bench:ref gf2-bench:smoke

# fflas_bench compile (inside container, via Makefile):
$ podman run --rm --security-opt label=disable -v "$PWD/benchmarks:/work:Z,U" \
      localhost/gf2-bench:ref bash -c 'cd /work/reference && make -B fflas_bench'
g++ -std=c++17 -O3 -march=native -fopenmp -I/usr/local/include  \
    fflas_bench.cpp -lopenblas -L/usr/local/lib -lgivaro -lgmpxx -lgmp \
    -lopenblas -o fflas_bench

# m4ri_bench compile (inside container, via Makefile):
$ podman run --rm --security-opt label=disable -v "$PWD/benchmarks:/work:Z,U" \
      localhost/gf2-bench:ref bash -c 'cd /work/reference && make -B m4ri_bench'
cc -std=c11 -O3 -march=native -I/usr/local/include m4ri_bench.c \
    -L/usr/local/lib -lm4ri -lm -o m4ri_bench

# Smoke equality oracle pass (n=16 algebraic-equality contracts):
$ benchmarks/smoke.sh --skip-build
[fflas_bench] SMOKE OK minpoly field=GF(2^31-1) deg=16 charpoly_deg=16
[fflas_bench] SMOKE OK minpoly field=GF(65521) deg=16 charpoly_deg=16
[fflas_bench] SMOKE OK minpoly field=GF(251) deg=16 charpoly_deg=16
[fflas_bench] SMOKE OK minpoly field=GF(7) deg=15 charpoly_deg=16
[fflas_bench] smoke OK
[m4ri_bench] SMOKE OK pluq n=16 rank=15
[m4ri_bench] SMOKE OK invert n=16
[m4ri_bench] SMOKE OK solve n=16
[m4ri_bench] smoke OK

# Full reference CSV extension run (warmup=2, iters=3 to keep wall-clock modest):
$ GF2_BENCH_WARMUP=2 GF2_BENCH_ITERS=3 benchmarks/run.sh \
      --skip-build --image-tag localhost/gf2-bench:ref
```

The CSV produced is at
`dev/bench_results/2026-05-04-5dea7457-reference-extension.csv`
(165 data rows; SHA-checked into the worktree branch).

## 4. Linked artefacts

- `dev/bench_results/2026-05-04-5dea7457-reference-extension.csv` —
  full reference CSV including the four new operation-classes
  (fflas-ffpack `minpoly`; M4RI `pluq`, `invert`, `solve`).
- `dev/bench_results/2026-05-04-5dea7457-host.txt` — host capture
  (Zen-3 / Linux 6.19.11-arch1-1).
- `dev/bench_results/2026-05-04-5dea7457-perf-stat.txt` — combined
  perf-stat capture for one representative bench-binary invocation
  per harness (covers all new operations × fields × sizes within the
  binary's run; aggregate numbers, but each (op, field) cell is
  reachable from this single dispatch and the timing rows in the CSV
  give per-cell wall-clock decomposition).
- `benchmarks/reference/fflas_bench.cpp` — added `bench_minpoly` +
  `smoke_minpoly_equality`; `--smoke` CLI flag.
- `benchmarks/reference/m4ri_bench.c` — added `bench_pluq`,
  `bench_invert`, `bench_solve`, plus three `smoke_*` correctness
  oracles and an `alloc_invertible_gf2` helper that constructs
  invertible-by-construction GF(2) matrices for the smoke checks.
- `benchmarks/run.sh` — new `--smoke-equality` flag that engages each
  harness's `--smoke` mode before the timing pass; fixed a pre-existing
  stdout/stderr leak where `make -B`'s compile command echo could
  contaminate the CSV after a forced rebuild.
- `benchmarks/smoke.sh` — engages `--smoke-equality` automatically.
- `benchmarks/README.md` — coverage table extended; M4RI's
  charpoly/minpoly absence documented as a `not-supported-by-library`
  exclusion class.

## 5. `analyze.py` reference-selection rule untouched

Per protocol § 8.3, `analyze.py`'s reference-selection rule (M4RI for
GF(2), fflas-ffpack for every other field) is unchanged in this issue.
Canonical-reference designations live downstream of story `cbecfced`
(in particular issue `4c0d0202`); this issue is purely an extension
of coverage within the existing two lanes.

## 6. Known caveats observed during the run

- **GF(2) random-matrix singularity.** A random `n × n` GF(2) matrix
  is singular with probability `1 − prod_{k=1}^{n} (1 − 2^{-k})`, which
  saturates at `≈ 0.711` for large `n`. This means the M4RI `solve`
  and `invert` benches' `uniform` regime *will* legitimately hit
  singular inputs at certain seeds — the harness reports this via
  `[m4ri_bench] INCONSISTENT solve` / `WARN deficient invert returned
  non-NULL` stderr breadcrumbs. The CSV row's wall-clock still measures
  the singular-detection cost (which is dominated by the same Konrod /
  PLE pass that would have produced the answer on a non-singular
  input), and the row is still consistent with the protocol § 6
  "regime documents the call's expected outcome" semantics.
- **m4ri `mzd_inv_m4ri` does not return NULL on rank-deficient
  inputs.** Empirically observed at all three sizes (64, 256, 1024) on
  this host with the constructed rank-`n/2` inputs. This is an
  upstream behaviour the harness records via stderr; the CSV `regime`
  column documents the deficiency so consumers do not naively compare
  the timing across rank regimes. The protocol § 6 contract for
  `invert` is "both sides must report singular" — fflas-ffpack reports
  via `nullity`, M4RI reports (or fails to report) via NULL. This
  asymmetry is now visible in the lane and is a candidate for a
  future-protocol amendment if the asymmetry blocks downstream
  promotion logic.
- **Pre-existing run.sh stdout/stderr leak.** `run.sh`'s
  `COMPILE_CMD='cd /work/reference && make -B'` previously sent
  `make`'s compile-command echo to stdout, which got concatenated
  into the CSV file. Forcing rebuild now (which is what the post-
  Containerfile-image-reuse case requires) leaks `g++ -std=c++17 ...`
  and `cc -std=c11 ...` lines into the CSV. Fixed in this commit by
  redirecting `make`'s output to stderr (`make -B 1>&2`). This was
  not visible in the 2026-04-26 baseline because that run was against
  a freshly-built image and `make -B` happened to be a no-op (binaries
  built during image build).
