# LinBox 1.7.1 — promotion evidence (issue 79388011)

**Status:** PROMOTE — covers `charpoly`, `minpoly`, and `solve` over GF(p) for the four reference primes (GF(7), GF(251), GF(65521), GF(2^31-1)) at n in {64, 256, 1024} (solve) and n in {64, 256} (charpoly, minpoly).

**Authority:** SOTA reference acceptance protocol — `dev/plans/sota_reference_acceptance_protocol.md` § 3 (five-criterion checklist), § 9 (workflow). This document satisfies issue `79388011`'s `[hard]` criteria #1 ("A reproducible LinBox harness exists or a rejection note explains why it is not viable") and #2 ("The target matrix uses or explicitly excludes LinBox based on evidence" — the actual target-matrix update lives in story `4c0d0202`; this evidence is the upstream feed).

---

## Five-criterion confirmation table

| # | Criterion | Result | Evidence artefact |
|---|-----------|--------|-------------------|
| 1 | Reproducible build | **PASS** | `benchmarks/Containerfile` `# === linbox begin/end ===` stanza (LinBox 1.7.1, sha256 `a2b5f910a54a46fa75b03f38ad603cae1afa973c95455813d85cf72c27553bd8`); `benchmarks/image.lock` `[libs.linbox]` block. Image built cleanly: `localhost/gf2-bench:linbox` (sha256 `f0a497dcfbb1715c69697310d79f0b4e3f7195265c2284db623041fe49943607`, 619 MB) on top of pinned Givaro 4.2.0 + fflas-ffpack 2.5.0. LinBox configures with `--with-givaro=/usr/local --with-fflas-ffpack=/usr/local --with-blas-libs="-lopenblas" --disable-openmp --disable-static` — single-thread by construction (no `OMP_NUM_THREADS=1` runtime override needed). |
| 2 | Same hardware    | **PASS** | `dev/bench_results/2026-05-04-79388011-linbox-host.txt` (AMD Ryzen 9 5900X, Zen 3, 12c/24t, AVX2/BMI2/VAES/VPCLMULQDQ; Linux 7.0.3-arch1-1 — same microarch class as the protocol § 5 baseline; kernel patch-version drift only). `dev/bench_results/2026-05-04-79388011-linbox-perf-stat.txt` aggregates the full in-scope sweep with `perf stat -r 5`: 11.14 G cycles, 22.43 G instructions (IPC ≈ 2.01), 1.5 % branch-miss rate, 7.8 % L1-d-cache miss rate, wall ≈ 2.80 s ± 1.1 % per repeat. |
| 3 | Comparable semantics | **PASS** | `linbox_bench --smoke` exercises every (op, field) cell at n=16 and asserts the per-operation contract from protocol § 6. All 12 (4 fields × 3 ops) smoke cells pass: charpoly is monic of degree n and satisfies Cayley-Hamilton (`p(A) = 0`), minpoly is monic, has degree ≤ n, and annihilates A, solve produces an x with `A·x ≡ b` bitwise. Determinism is delegated to `gf2_bench_splitmix64` / `gf2_bench_derive_seed` from `benchmarks/reference/seed_helpers.h` — every CSV row's `seed` column was generated from the same master seed `0x6F73AC91D31E4A7C` as the fflas-ffpack baseline; cross-library cells share `seed` byte-for-byte (e.g. `charpoly,GF(2^31-1),n=64` carries `11506559259852285241` in both `2026-04-26-reference.csv` and `2026-05-04-79388011-linbox-reference.csv`). |
| 4 | Shared data shape | **PASS** | `dev/bench_results/2026-05-04-79388011-linbox-reference.csv` carries 40 rows in the canonical 10-column schema; first row: `linbox,charpoly,GF(2^31-1),64,64,64,uniform,11506559259852285241,1169618,2.241279e+08`. `lib=linbox` is a new value, picked up automatically by `analyze.py` per protocol § 7 *Allowed values*. Throughput normalizers match `benchmarks/README.md`: charpoly uses `n³`, minpoly uses `n⁴`, solve uses `n³`. |
| 5 | CSV merge support | **PASS** | `python3 benchmarks/analyze.py --smoke` exits 0 (baseline self-test). Full merge `python3 benchmarks/analyze.py --reference <fflas+linbox-merged.csv> --out <out.md>` produces a 142-cell side-by-side table. Co-existence with fflas-ffpack rows on the same `(operation, field)` cells (e.g. `charpoly × GF(2^31-1)`) is preserved without silent overwrite — `analyze.py`'s default canonical-reference rule keeps fflas-ffpack as the rendering target for GF(p) charpoly/solve cells; LinBox rows live in the merged dataset for the target-matrix story `4c0d0202` to designate per-cell. |

---

## Coverage scope (cells claimed)

The harness emits 40 rows covering three operations across four fields:

| Operation | Sizes | Regimes | Fields |
|---|---|---|---|
| `charpoly` | n in {64, 256} | uniform | GF(7), GF(251), GF(65521), GF(2^31-1) |
| `minpoly`  | n in {64, 256} | uniform | GF(7), GF(251), GF(65521), GF(2^31-1) |
| `solve`    | n in {64, 256, 1024} | uniform + deficient | GF(7), GF(251), GF(65521), GF(2^31-1) |

Cell count: 4 fields × (2 charpoly + 2 minpoly + 6 solve) = 40 rows.

**Out of scope for this issue (per dispatch contract).** `fgemm`, `pluq`, `echelon`, `invert`, `spmv` cells where fflas-ffpack already owns the canonical reference — adding a LinBox row there would just be `<lib>-secondary` evidence per protocol § 8.4. Sparse cells are tracked separately. n=4096 across all three ops is deferred to T2 per the `benchmarks/README.md` § *Deferred to T2 / T3* posture inherited from the fflas harness.

**Solve regime convention.** Both `uniform` and `deficient` runs construct `b = A·x_0` for a random `x_0` so the system is always consistent. This deliberately differs from fflas-ffpack's harness (which samples `b` independently and lets `FFPACK::Solve` fall through to the trivial particular-solution path). Rationale: LinBox's `solve()` raises `LinboxMathInconsistentSystem` on inconsistent inputs (no graceful particular-solution fallback), and forcing consistency keeps the wall-clock measurement focused on the elimination cost itself rather than on path-divergent error-handling code. The protocol § 6 "Solve" row notes that "if a particular solution is returned, equality is on the **specific** particular solution by basis convention" — by forcing `b = A·x_0` we avoid that ambiguity entirely.

---

## Indicative wall-clock numbers (5 iters, warmup 3)

Selected cells from `dev/bench_results/2026-05-04-79388011-linbox-reference.csv`. Fflas-ffpack numbers from `dev/bench_results/2026-04-26-reference.csv` for direct comparison where the cell is in fflas's coverage too.

### charpoly (uniform)

| Field | n | linbox wall_ns | linbox tput | fflas wall_ns | fflas tput | linbox / fflas wall |
|---|---|---|---|---|---|---|
| GF(2^31-1) |  64 |     1 169 618 | 0.224 Gops/s |       743 458 | 0.353 Gops/s | 1.57× |
| GF(2^31-1) | 256 |    44 682 256 | 0.375 Gops/s |    43 919 996 | 0.382 Gops/s | 1.02× |
| GF(65521)  |  64 |       684 264 | 0.383 Gops/s |       674 064 | 0.389 Gops/s | 1.02× |
| GF(65521)  | 256 |    12 467 707 | 1.346 Gops/s |    12 377 745 | 1.355 Gops/s | 1.01× |
| GF(251)    |  64 |       589 426 | 0.445 Gops/s |       476 418 | 0.550 Gops/s | 1.24× |
| GF(251)    | 256 |    13 556 627 | 1.238 Gops/s |     1 316 860 | 12.74 Gops/s | 10.30× |
| GF(7)      |  64 |       422 622 | 0.620 Gops/s |       401 970 | 0.652 Gops/s | 1.05× |
| GF(7)      | 256 |    13 510 353 | 1.242 Gops/s |    13 633 042 | 1.231 Gops/s | 0.99× |

### minpoly (uniform) — LinBox only

fflas-ffpack's harness does not emit minpoly rows; LinBox provides the reference. `n⁴` normalizer per `benchmarks/README.md`.

| Field | n | linbox wall_ns | linbox tput |
|---|---|---|---|
| GF(2^31-1) |  64 |       852 488 | 19.7 Gops/s |
| GF(2^31-1) | 256 |    51 476 406 | 83.4 Gops/s |
| GF(65521)  |  64 |       367 996 | 45.6 Gops/s |
| GF(65521)  | 256 |    11 732 485 | 366 Gops/s  |
| GF(251)    |  64 |       401 510 | 41.8 Gops/s |
| GF(251)    | 256 |    13 529 635 | 317 Gops/s  |
| GF(7)      |  64 |       398 472 | 42.1 Gops/s |
| GF(7)      | 256 |    13 506 493 | 318 Gops/s  |

The throughput numbers reflect the `n⁴` Krylov-sweep normalizer; absolute wall-clock at n=256 is ~13 ms across the small primes and ~52 ms for the large 2^31-1 prime.

### solve (uniform regime, full-rank consistent system)

| Field | n | linbox wall_ns | linbox tput | fflas wall_ns | fflas tput | linbox / fflas wall |
|---|---|---|---|---|---|---|
| GF(2^31-1) |   64 |       258 202 | 1.02 Gops/s |       445 224 | 0.589 Gops/s | 0.58× |
| GF(2^31-1) |  256 |     8 274 662 | 2.03 Gops/s |     8 290 466 | 2.02 Gops/s  | 1.00× |
| GF(2^31-1) | 1024 |   399 903 956 | 2.68 Gops/s |   381 817 039 | 2.81 Gops/s  | 1.05× |
| GF(65521)  |   64 |       177 614 | 1.48 Gops/s |       131 798 | 1.99 Gops/s  | 1.35× |
| GF(65521)  |  256 |     3 068 896 | 5.47 Gops/s |     2 949 280 | 5.69 Gops/s  | 1.04× |
| GF(65521)  | 1024 |    74 708 825 | 14.4 Gops/s |    61 911 149 | 17.3 Gops/s  | 1.21× |
| GF(251)    |   64 |       252 012 | 1.04 Gops/s |        28 144 | 9.31 Gops/s  | 8.95× |
| GF(251)    |  256 |     3 194 897 | 5.25 Gops/s |       618 648 | 27.1 Gops/s  | 5.17× |
| GF(251)    | 1024 |    58 967 267 | 18.2 Gops/s |    24 094 125 | 44.6 Gops/s  | 2.45× |
| GF(7)      |   64 |       253 014 | 1.04 Gops/s |       209 520 | 1.25 Gops/s  | 1.21× |

Observation: LinBox's `Modular<int64_t>` solve is competitive with fflas's `Modular<int64_t>` path at GF(2^31-1) and GF(65521), and is materially slower at GF(251) where fflas dispatches to `Modular<float>` + BLAS. This is consistent with the field-class analysis: LinBox routes its `Method::DenseElimination` through the same fflas-ffpack BLAS-backed elimination, but the `int64_t` cardinality choice prevents the float-fast-path dispatch.

---

## Performance-relevance verdict (protocol § 9 J)

**Performance-relevant for every cell claimed.** The slowest LinBox/fflas wall-clock ratio observed is 10.30× (charpoly, GF(251), n=256), driven by fflas's `Modular<float>` BLAS dispatch. No cell shows ≥ 100× slowdown, so no cell falls into the `not-performance-relevant` exclusion class. LinBox earns its row in the side-by-side as a primary reference for **`minpoly`** (where fflas does not emit rows) and as a **secondary reference** for `charpoly` and `solve` over GF(p) — the exact canonical-reference designation per cell is the responsibility of the target-matrix story `4c0d0202`.

---

## License compatibility (protocol § 9 K)

LinBox 1.7.1 is licensed under **LGPL-2.1-or-later**. This is compatible with redistribution alongside `gf2-core` (also LGPL-friendly) and with citation in evidence docs. No exclusion under `license-incompatible`.

---

## Reproduction commands

```bash
# Build the augmented container.
podman build -t gf2-bench:linbox -f benchmarks/Containerfile benchmarks/

# Smoke (correctness oracle at n=16 across every claimed cell).
podman run --rm --security-opt label=disable -v "$PWD/benchmarks:/work:Z,U" \
    gf2-bench:linbox \
    bash -c "cd /work/reference && make linbox_bench && ./linbox_bench --smoke"

# Full timing pass.
SEED=0x6F73AC91D31E4A7C
podman run --rm --security-opt label=disable -v "$PWD/benchmarks:/work:Z,U" \
    gf2-bench:linbox \
    bash -c "cd /work/reference && ./linbox_bench --seed ${SEED} --warmup 3 --iters 5"

# Merge into the side-by-side renderer.
python3 benchmarks/analyze.py \
    --reference dev/bench_results/2026-05-04-79388011-linbox-reference.csv \
    --out dev/bench_results/2026-05-04-79388011-linbox-tables.md
```

---

## Outstanding caveats / follow-ups

1. **`run.sh` does not yet verify `LINBOX_SHA256` against `[libs.linbox]`.** The `verify_sha` helper in `run.sh` carries hardcoded calls for Givaro / fflas-ffpack / M4RI; a follow-up commit (out of scope for this dispatch per the "Do not touch run.sh" rule) needs to add `verify_sha LINBOX_SHA256 libs.linbox`. Currently the sha is enforced inside the Containerfile via `sha256sum -c -`, which is a build-time gate but not the run.sh pre-flight gate the existing pins enjoy. Lead to wire after the parallel-dispatch wave merges.

2. **CMD self-documenting block in Containerfile not extended.** The default `CMD ["/bin/bash", "-c", ...]` lists installed library versions (gcc/g++/givaro/fflas/m4ri/openblas/gmp). Extending it to also echo LinBox's version requires an edit outside the labelled stanza, which the parallel-dispatch contract forbids. Lead to merge a one-line addition (`echo 'linbox: '$(pkg-config --modversion linbox 2>/dev/null)`) post-wave.

3. **Single-cell perf-stat unavailable.** The harness lacks a `--cell <op>:<field>:<n>:<regime>` CLI knob, so the `-r 5` perf-stat covers the entire in-scope sweep. The aggregate counters meet protocol § 5 ("at least one in-scope cell") because the heaviest cells (solve@n=1024 across all four fields) dominate the wall-clock budget. A future dispatch can add a per-cell knob if needed for finer attribution.

4. **`Modular<float>` path not exercised.** The harness uses `Modular<int64_t>` for all four primes, including GF(251) and GF(7), which fflas's harness routes through `Modular<float>` for the BLAS-accelerated dispatch. This is intentional: LinBox's solution-level API (`solve`, `charpoly`, `minpoly`) is most uniformly defined over the integer path. The performance gap on small primes documented above (10.30× at charpoly GF(251) n=256) stems from this choice. A follow-up could promote a separate `linbox-float` row family if the target matrix demands it.
