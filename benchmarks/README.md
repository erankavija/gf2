# benchmarks/ — reference reproducibility harness

This directory holds the pinned-container reference harness for the
`FieldMatrix` linear-algebra benchmarking story
(`jit:64c88ae4` / `jit:bb85c68a`). The artefacts here let any host with
a container runtime reproduce the reference numbers byte-for-byte —
matrices, build flags, library versions and OS userspace are all
pinned. The gf2-side criterion benches (under `crates/gf2-core/benches/`,
delivered by sibling task `T2`) consume the same master seed so each
matrix is identical on both sides.

## Layout

```
benchmarks/
├── Containerfile            # Debian-slim + pinned gcc / OpenBLAS / GMP /
│                            # Givaro / fflas-ffpack / M4RI
├── image.lock               # version + sha256 pins (lock file)
├── run.sh                   # build + run driver (rootless podman default)
├── reference/
│   ├── fflas_bench.cpp      # fgemm, PLUQ, echelon, invert, solve, charpoly
│   ├── m4ri_bench.c         # GF(2) matmul + echelonize via Method-of-4-Russians
│   └── Makefile             # in-container build (Containerfile invokes it)
├── seeds/
│   └── seed.txt             # single 64-bit master seed shared with gf2 side
├── host.txt                 # captured at run time (CPU, OS, runtime)
└── results/
    └── <timestamp>.csv      # one CSV per run; latest.csv symlink follows
```

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
| `m4ri 20240729`                        | upstream tarball + sha256 (`image.lock`) |

The Containerfile compiles every C and C++ artefact with
`-O3 -march=native` so timings reflect what production code on the
target host can actually achieve. `-march=native` does mean numbers are
not portable across host CPUs — `host.txt` captures the CPU model,
microarchitecture flags, and cache topology so any reader can verify
the substrate before drawing conclusions.

The first time the image is built, the local content-addressable id can
be recorded into `image.lock` (the `[image].local_id` slot). The
placeholder that ships with the repo is intentionally not a real
digest; the gate-job will fill it on first build.

## CSV schema

Every row in `results/<timestamp>.csv` has exactly **ten** columns:

| Column           | Type     | Description                                                                                                                                |
|------------------|----------|--------------------------------------------------------------------------------------------------------------------------------------------|
| `lib`            | string   | Reference implementation: `fflas-ffpack` or `m4ri`.                                                                                        |
| `operation`      | string   | One of `fgemm`, `pluq`, `echelon`, `invert`, `solve`, `charpoly`, `matmul` (m4ri spelling for `fgemm`).                                    |
| `field`          | string   | Field tag, e.g. `GF(2^31-1)`, `GF(65521)`, `GF(251)`, `GF(7)`, `GF(2)`.                                                                   |
| `m`, `k`, `n`    | usize    | Matrix shape passed to the kernel. For square ops `m = k = n`. For `fgemm` rectangular variants (deferred to T2), `m`, `k`, `n` differ.    |
| `rank_regime`    | string   | `uniform` (sample i.i.d.) or `deficient` (rank exactly `n/2`, generated as L·R with shared inner dimension).                              |
| `seed`           | uint64   | The 64-bit deterministic seed for *this row*'s matrix. Derived from the master seed via `SplitMix64(tag, op_idx, size_idx, regime_idx)`. |
| `wall_ns`        | uint64   | Mean wall-clock nanoseconds per iteration (steady_clock / CLOCK_MONOTONIC, total wall time / `--iters`).                                  |
| `throughput_ops` | float    | Conventional op-count divided by `wall_ns`. `fgemm`: `2·m·k·n`. Square `n×n` factorizations and charpoly: `n³`. M4RI matmul: `2·n³`.       |

A header row is emitted by `run.sh` exactly once per file. Both
harnesses emit only data rows on stdout so they can be concatenated
without de-duplication.

### Column rationale

- `(m, k, n)` rather than just `n` because `fgemm` is genuinely
  rectangular; the schema also covers the rectangular sweep that T2
  introduces without re-versioning the CSV.
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

## What's covered in T1

Per the parent story `64c88ae4` and the R1 amendment dated 2026-04-26:

| Operation              | T1 sizes (square)    | Fields                                       | Rank regimes        |
|------------------------|----------------------|----------------------------------------------|---------------------|
| fgemm                  | 64, 256, 1024, 4096  | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform             |
| PLUQ (≡ PLE)           | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| RowEchelonForm         | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| Invert                 | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| Solve (Ax=b)           | 64, 256, 1024        | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform + deficient |
| CharPoly               | 64, 256              | GF(2^31-1), GF(65521), GF(251), GF(7)         | uniform             |
| M4RI mzd_mul (matmul)  | 64, 256, 1024, 4096  | GF(2)                                         | uniform + deficient |
| M4RI echelonize        | 64, 256, 1024        | GF(2)                                         | uniform + deficient |

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

For `n = 4096` we keep `fgemm` only across all four GF(p) fields (the
cheapest of the dense ops at that size, BLAS3 with peak microarch
utilisation) — PLUQ / echelon / invert / solve / charpoly at 4096 are
formally deferred (see below).

## Deferred to T2 / T3

The R1 amendment (2026-04-26) formalises the following deferrals:

- **PLUQ / echelon / invert / solve / charpoly at `n = 4096`**: each
  cell is multi-second per iteration on most reference fields and the
  full sweep would dominate the T1 wall-clock budget. The fgemm@4096
  point is preserved on every field; the rest will land in T2 with
  per-cell budgets relaxed.
- **Rectangular fgemm** (`m × k × n` with skew aspect ratios): the
  Winograd-crossover sweep. The CSV schema already supports it.
- **Sparse SpMV** (sparsity 1% / 5% at n = 1024, 4096): fflas-ffpack's
  sparse interface is via LinBox proper, not fflas-ffpack itself, so
  reference numbers will come from a separate harness. Tracked under
  story `8a90882e`.
- **`Modular<int8_t>` proper**: fflas-ffpack does not ship an int8
  Modular template; `Modular<float>` over `[0, 251)` is the canonical
  small-prime path used in the upstream test corpus. A native int8 path
  would require a custom field wrapper and is not on the critical path
  for the epic.
- **`Givaro::GFq` for GF(2^m)**: GF(2^m) reference numbers via Givaro
  are tracked separately. fflas-ffpack's GF(2^m) path goes through
  Modular<float> with a polynomial multiplication wrapper, which is
  not a fair comparison to gf2's PCLMULQDQ kernels. T2 will add a
  Givaro::GFq harness if a fair-comparison framing is found.

## Gate coverage

`cargo-ci` (the workspace's lint/test gate) does **not** validate the
container build or the harness binaries — the gate runner has no
`podman` daemon. The criteria

- "container builds from clean state with the pinned `Containerfile`",
  and
- "harnesses run to completion and emit CSV for every (field, op,
  size) cell"

are therefore validated **out-of-gate** by running
`./benchmarks/run.sh` once on a real host (the dev workstation, the
project lead's machine, or any contributor with rootless podman). The
generated `benchmarks/results/<timestamp>.csv` and
`benchmarks/host.txt` are the audit-trail artefacts the lead checks in
to substantiate those criteria.

For interactive verification without a full timing run, see
`benchmarks/smoke.sh` — it builds the image and runs both harnesses
with `--warmup 0 --iters 1` so a single end-to-end pass takes well
under a minute per supported field.

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

## Prerequisites the human must satisfy

The agent that produced this directory cannot itself build the image
(no privilege to invoke the container runtime in the worker sandbox).
Before the gate job can certify the success criteria, a human (or the
gate job runner) must have:

1. **Rootless podman** installed (or Docker, with the env var set).
   `which podman` should resolve.
2. **Network access** at *build time* so the Containerfile can fetch
   the upstream tarballs. Run-time of the container itself is offline.
3. **At least ~6 GiB of free disk** for the layer cache (Givaro +
   fflas-ffpack + M4RI source trees + build artefacts).
4. **A host CPU that satisfies `-march=native`** for whatever
   microarchitecture the gate runs on. The `host.txt` file at run time
   captures this.

`run.sh` automatically stamps the new `[image].local_id` into
`image.lock` after each successful build, so subsequent runs on the
same host detect environment drift if the local id no longer matches.
On a fresh checkout the slot is `sha256:TODO_FILL_AFTER_FIRST_BUILD`;
the first `./benchmarks/run.sh` invocation overwrites that with the
real id.
