# benchmarks/ — reference reproducibility harness

This directory holds the pinned-container reference harness for the
`FieldMatrix` linear-algebra benchmarks. Matrices, build flags, library
versions and OS userspace are all pinned, so any host with a container
runtime reruns the same reference configuration. The gf2-side criterion benches
(under `crates/gf2-core/benches/`) consume the same master seed so each
matrix is identical on both sides.

## Container runtime

The host is assumed to have **rootless podman** available. Docker is
also supported but is not the default; pass `GF2_RUNTIME=docker` to
`run.sh` or set the env var.

Rootless podman quirks worth noting:

- File ownership inside the container maps via subuid/subgid. `run.sh`
  bind-mounts `benchmarks/` into the container with `:Z,U` so the
  container can write CSV output back to the host. On Debian/Ubuntu
  hosts (no SELinux), `:Z` is harmless. On Fedora/RHEL it forces a
  private relabel.
- `--security-opt label=disable` is also passed to keep relabel-related
  surprises out of the gate path; the harness only writes into a bind
  mount we control.
- Rootless podman uses `slirp4netns` for networking, but the harness
  needs no network at run time once the image is built.

## Pinning strategy

| Pin                                    | Source                                  |
|----------------------------------------|-----------------------------------------|
| Base image (`debian:bookworm-...-slim`) | Docker Hub digest in `image.lock`       |
| `gcc-12`, `g++-12`, `libopenblas-dev`, `libgmp-dev`, `liblapack-dev`, `cmake` | Debian bookworm apt versions (`image.lock`) |
| `givaro 4.2.0`                         | upstream tarball + sha256 (`image.lock`) |
| `fflas-ffpack 2.5.0`                   | upstream tarball + sha256 (`image.lock`) |
| `m4ri 20260122`                        | upstream tarball + sha256 (`image.lock`) |

The Containerfile compiles every C and C++ artefact with
`-O3 -march=native` so timings reflect what production code on the
target host can actually achieve. `-march=native` does mean numbers are
not portable across host CPUs — `host.txt` captures the CPU model,
microarchitecture flags, and cache topology so any reader can verify
the substrate before drawing conclusions.

After a successful image build, `run.sh` records the local
content-addressable id into `image.lock` (the `[image].local_id` slot);
a mismatch with the committed id signals environment drift. A rebuild on
different hardware commits the stamped id together with its results.

## CSV schema

Every row in `results/<timestamp>.csv` has exactly **ten** columns:

| Column           | Type     | Description                                                                                                                                |
|------------------|----------|--------------------------------------------------------------------------------------------------------------------------------------------|
| `lib`            | string   | Reference implementation, e.g. `fflas-ffpack` or `m4ri`.                                                                                       |
| `operation`      | string   | One of `fgemm`, `pluq`, `echelon`, `invert`, `solve`, `charpoly`, `minpoly`, `spmv`, `matmul` (m4ri spelling for `fgemm`), or one of the sparse ops `sparse-matmul`, `sparse×dense`, `sparse-elim`.               |
| `field`          | string   | Field tag, e.g. `GF(2^31-1)`, `GF(65521)`, `GF(251)`, `GF(7)`, `GF(2)`.                                                                   |
| `m`, `k`, `n`    | usize    | Matrix shape passed to the kernel. For square ops `m = k = n`. For rectangular `fgemm`, `m`, `k`, `n` differ. |
| `rank_regime`    | string   | `uniform` (sample i.i.d.) or `deficient` (rank exactly `n/2`, generated as L·R with shared inner dimension).                              |
| `seed`           | uint64   | The 64-bit deterministic seed for *this row*'s matrix. Derived from the master seed via `SplitMix64(tag, op_idx, size_idx, regime_idx)`. |
| `wall_ns`        | uint64   | Mean wall-clock nanoseconds per iteration (steady_clock / CLOCK_MONOTONIC, total wall time / `--iters`).                                  |
| `throughput_ops` | float    | Conventional op-count divided by `wall_ns`. `fgemm`: `2·m·k·n`. Square `n×n` factorizations and `charpoly`: `n³`. `minpoly`: `n⁴` (LCM-merge sweep over `n` Krylov passes). `spmv`: `nnz` (one mul + one add per stored entry). M4RI `matmul`: `2·n³`.       |

A header row is emitted by `run.sh` exactly once per file. Both
harnesses emit only data rows on stdout so they can be concatenated
without de-duplication.

### Column rationale

- `(m, k, n)` rather than just `n` because `fgemm` is genuinely
  rectangular.
- `seed` is per-row, not per-run, because rank-deficient and uniform
  draws need independent seeds for the same operation/size cell. The
  master seed is a derivation key.
- `throughput_ops` is computed with the *dominant* term op-count rather
  than the exact constant; the goal is to give a single normalized
  yardstick across operations. Don't compare absolute throughput across
  *different* operations naively — compare gf2 vs reference within the
  same `(operation, field, n, rank_regime)` cell.

### Seed derivation (matches `gf2-core` side)

```
splitmix64(state) ::= ... see fflas_bench.cpp / m4ri_bench.c

derive(master, tag, op_idx, size_idx, regime_idx):
    s = master
    for byte in tag.bytes(): s ^= byte; splitmix64(&s)
    s ^= op_idx;     splitmix64(&s)
    s ^= size_idx;   splitmix64(&s)
    s ^= regime_idx; splitmix64(&s)
    return splitmix64(&s)
```

The exact tag strings used per row are listed in the harness sources
(`fflas_bench.cpp` and `m4ri_bench.c`). For the gf2-side benches to
produce the *same* matrix, they must use the same SplitMix64 seed
followed by the same uniform-fill routine (one `splitmix64` call per
field element, reduced mod `p`). The reduction-into-field step is
biased only for non-power-of-two fields, but the bias is identical on
both sides.

## Coverage of `fflas_bench` and `m4ri_bench`

| Operation              | Sizes (square)       | Fields                                       | Rank regimes        |
|------------------------|----------------------|----------------------------------------------|---------------------|
| fgemm                  | 64, 256, 1024, 4096  | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform             |
| PLUQ (≡ PLE)           | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| RowEchelonForm         | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| Invert                 | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| Solve (Ax=b)           | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| CharPoly               | 64, 256              | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform             |
| MinPoly                | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform             |
| M4RI mzd_mul (matmul)  | 64, 256, 1024, 4096  | GF(2)                                         | uniform + deficient |
| M4RI echelonize        | 64, 256, 1024        | GF(2)                                         | uniform + deficient |
| M4RI pluq              | 64, 256, 1024        | GF(2)                                         | uniform + deficient |
| M4RI invert            | 64, 256, 1024        | GF(2)                                         | uniform + deficient |
| M4RI solve (Ax=b)      | 64, 256, 1024        | GF(2)                                         | uniform + deficient |

`uniform`        — i.i.d. samples from the field's canonical range.
`deficient`      — exact rank `n/2`, generated as `L · R` with a shared
                   inner dimension. For `invert` and `solve` this means
                   the call hits its singular-matrix path; the timing
                   measures the work fflas-ffpack does to detect that,
                   which is the same PLUQ pass it would otherwise use
                   to compute the inverse / solution.

The harness applies a **per-cell wall-clock cap** of 30 s (configured
in `fflas_bench.cpp` as `kCellBudgetNs`). Cells that exceed the cap
emit an `early_exit` warning on stderr and a CSV row with the
measurements taken so far; downstream consumers can filter on the
warning when comparing.

**M4RI scope exclusions.** The M4RI library does not expose a
`charpoly` or `minpoly` interface, so those operations have no M4RI
lane. `m4ri_bench` covers neither `mzd_addmul` (rectangular fgemm) nor
sparse paths.

## Gate coverage

`cargo-ci` validates neither the container build nor the harness
binaries. `./benchmarks/run.sh` on a host with a container runtime
validates both and writes `benchmarks/results/<timestamp>.csv` and
`benchmarks/host.txt`. `benchmarks/smoke.sh` builds the image and runs
the harnesses with `--warmup 0 --iters 1`, without a full timing run.

## Determinism guarantees

- Re-running with the same `--seed` produces *identical* matrices on
  every (field, op, size, regime) cell. Timing of course varies.
- Re-building the image from the pinned Containerfile + `image.lock`
  on the **same host** produces an identical local image-id (stamped
  into `[image].local_id`); cross-host the digest will differ because
  every C/C++ object is built with `-O3 -march=native` and is therefore
  microarchitecture-specific. The library-source pins (base-image
  digest, tarball sha256s, Debian apt versions) are reproducible across
  hosts; only the final compiled binaries' content-id is host-specific.
  Debian's stable-release semantics guarantee no apt-side drift within
  bookworm's lifetime.
- The harnesses use `clock_gettime(CLOCK_MONOTONIC)` /
  `std::chrono::steady_clock` so wall-time timings are immune to wall-
  clock adjustments mid-run.

## Running

```bash
# Default: build the image (if not already built), capture host info,
# run both harnesses, write benchmarks/results/<timestamp>.csv.
./benchmarks/run.sh

# Override the master seed (for cross-validation with a sibling run).
./benchmarks/run.sh --seed 0xCAFEBABEDEADBEEF

# CI smoke run: M4RI only, single warm-up + iter.
GF2_BENCH_WARMUP=1 GF2_BENCH_ITERS=1 ./benchmarks/run.sh --skip-fflas

# Use Docker if podman is unavailable.
GF2_RUNTIME=docker ./benchmarks/run.sh
```

## Prerequisites

1. **Rootless podman** installed (or Docker, with the env var set).
   `which podman` should resolve.
2. **Network access** at *build time* so the Containerfile can fetch
   the upstream tarballs. Run-time of the container itself is offline.
3. **At least ~6 GiB of free disk** for the layer cache (Givaro +
   fflas-ffpack + M4RI source trees + build artefacts).
4. **A host CPU that satisfies `-march=native`**. The `host.txt` file
   at run time captures the microarchitecture.
