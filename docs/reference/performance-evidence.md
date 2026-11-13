# Performance evidence

This page names, for each comparison gf2 makes against another implementation,
the reference and the build configuration that makes it the reference's
fastest, the gf2 path measured, the harness and the command that reproduces it.
Measured ratios, intervals and result tables are recorded in the issue each
entry cites, under `@/inv/committed-evidence-budget`.
A comparison uses the reference's fastest configuration at matched output
(`@/inv/performance-deliverable`).

A result is a run inside the benchmark window, which exports
`GF2_BENCH_WINDOW=1`. Each entry's **Output** line states what its harness
prints, where it writes below `target/`, and its behavior outside a window.
Each harness's README states its timing method and statistic.
Each command is a queue line's command, run from the root of a worktree at the
revision under test. Comparator trees and input bundles are staged outside the
repository and named by the environment variables shown; `GF2_EXT_ROOT` is the
staging directory of the host. [Reproduce a performance
measurement](../how-to/reproduce-performance-evidence.md) walks through one
harness end to end.

## Reference builds

Each reference is built from its release source under the staging directory,
either once into a prefix or by the comparison binary's build as the table
states, and used by every entry that names it. The options below make each
the reference's fastest build on the host; a host with another instruction set
rebuilds with the same options.

| Reference | Build options | Backend selection |
|---|---|---|
| AFF3CT v4.7.0 | CMake `Release`, `CMAKE_CXX_FLAGS="-O3 -march=native -funroll-loops"`, static library on, shared library, MPI and GSL off | Compile-time SIMD from `-march=native`; the harness requests scalar or inter-frame SIMD and the precision per run |
| srsRAN Project 25.10 | [`build.rs`](../../experiments/encode-compare/external/build.rs) compiles `SRSRAN_SOURCES` and the adapter with `cc`, `-std=c++17 -O3 -march=native`, and `NDEBUG` | [`srsran_nr.cpp`](../../experiments/encode-compare/external/cpp/srsran_nr.cpp) constructs `ldpc_encoder_avx2` when `__builtin_cpu_supports("avx2")` is true and `ldpc_encoder_generic` otherwise, plus `ldpc_rate_matcher_impl`. srsRAN 25.10 has no AVX-512 LDPC encoder, so AVX2 is its fastest x86 encoder |
| bchlib | Compiled by the comparison binary's build with `-O3 -march=native`, once with run-time parameters and once with `CONFIG_BCH_CONST_PARAMS` fixed to each cell's $(m, t)$ | The faster build per cell |
| gf2x 1.3.0 | `./configure --disable-static --enable-shared` once per `prefix-*` install: `native` with `CFLAGS="-O3 -march=native"`, `tuned` with `CFLAGS="-O3 -march=x86-64-v3"`, `conservative` with `CFLAGS=-O2`; configure adds `-msse2 -msse3 -mssse3 -msse4.1 -mpclmul` to each | PCLMULQDQ low-level multiplication, detected by configure; the fastest prefix per size |
| ISA-L | `./configure --disable-static CFLAGS="-O3 -march=native -fPIC"`, assembled with NASM 2.16.03 | The multibinary entry point `gf_vect_mad` selects among its SSE, AVX, AVX2 and AVX-512 variants, GFNI ones included, by CPUID at load |
| M4RI | `./configure --disable-static CFLAGS="-O3 -march=native -fPIC"`; SSE2 enabled, OpenMP off | The library's SSE2 kernels |
| M4RIE | `./configure --disable-static --with-m4ri=<the M4RI prefix> CFLAGS="-O3 -march=native -fPIC"` | The three multiplication routines named by the entry that uses it |

ISA-L, M4RI and M4RIE install into one prefix, `$GF2_EXT_ROOT/6c6b09b1/prefix`.

## Comparisons

<a id="ldpc-decode-aff3ct"></a>

### LDPC decoding against AFF3CT

- **Issues:** @/issue/07ca8585 (check-node update and decode cells),
  @/issue/1358d6ab (integrated tree), @/issue/de311215 (QC route).
- **Reference:** AFF3CT v4.7.0 (`@/citation/Cassagne2019`) normalized min-sum
  decoders, built as in [Reference builds](#reference-builds). The fastest
  configuration is the best over scalar f32 flooding and horizontal-layered, and
  inter-frame SIMD flooding, horizontal-layered and layered ONMS at f32 and i16,
  at each worker, batch and pinning cell. The flooding f32 scalar decoder is the
  matched algorithm and the other configurations bound it from above.
- **gf2 path:** on the DVB-T2 rate-$1/2$ normal frame (`@/citation/Etsi2015`) and
  the NR base graph 1 mother code at lifting $Z = 384$
  (`@/citation/ThreeGpp2017`), over recorded f32 LLR bundles with syndrome
  stopping, the harness runs five route configurations:
  `single`, `lanes` (SIMD none and AVX2), `qc` (`QcDecoder`, one frame at a
  time; SIMD none and AVX2) and `default` (`LdpcBatchDecoder` as shipped, SIMD
  auto) are flooding normalized min-sum with factor $0.75$ and iteration cap
  $50$; `layered` is a separate i8 `LayeredDecoder` under the corpus code's
  `LayeredConfig` with cap $25$ (SIMD none and AVX2).
- **Harness:** [`experiments/aff3ct-ldpc-gaps`](../../experiments/aff3ct-ldpc-gaps/README.md),
  with `aff3ct-fast-bench` and `gf2-fast-bench` as the two arms, both built with
  `-C target-cpu=native`. Cells are workers, batch and pinning
  $\in \{(1,1), (1,8), (1,16), (12,8), (12,16), (24,16)\}$ pinned and
  $(12,16), (24,16)$ unpinned; a run whose batch is not whole native waves of its
  decoder is skipped.
- **Output:** the host line, the SHA-256 of both executables and of AFF3CT's
  static library, then per code a table of frames per second, information
  throughput, frame errors and iteration cap per decoder, and the ratio of best
  gf2 to best AFF3CT frames per second overall and over flooding decoders only,
  at `target/aff3ct-ldpc-gaps/quick-compare/<UTC>.txt`. Outside a window the
  wrapper refuses.
- **Command:**

  ```sh
  GF2_AFF3CT_ROOT="$GF2_EXT_ROOT/c077a88b/aff3ct" \
  GF2_LDPC_BUNDLES="$GF2_EXT_ROOT/c077a88b/inputs" \
      bash experiments/aff3ct-ldpc-gaps/quick-compare.sh
  ```


<a id="integrated-bch-encode"></a>

### BCH encoding against AFF3CT and bchlib

- **Issues:** @/issue/1358d6ab (DVB-T2 and mother-code comparison),
  @/issue/040bc2b8 (DVB-T2 outer codes), @/issue/fd9d5416 (cutover).
- **Reference:** bchlib (`@/citation/Bchlib2026`) `bch_encode` in both builds of
  [Reference builds](#reference-builds); in the cutover comparison also AFF3CT
  `Encoder_BCH` and `Encoder_BCH_inter` at int8 and int32 and, for
  generator-matrix materialization, M4RI `mzd_echelonize_m4ri` and
  `mzd_echelonize_pluq`. A table row reports the fastest configuration of each
  cell.
- **gf2 path, `compare.sh bch`:** `DvbT2BchCode::encode_into` over the
  rate-$1/2$ outer codes $(7200, 7032)$ and $(32400, 32208)$
  (`@/citation/Etsi2015`), cycling a pool of $1$ or $256$ distinct messages.
- **gf2 path, `../../experiments/encode-compare/cutover.sh`:** `BchCode::encode_batch` at batch $1$, $16$, $256$
  and $4096$ (workload W1) and `BchCode::generator_matrix` (W2), against the
  pre-cutover revision (`BchEncoder::encode_batch` and the legacy
  `generator_matrix`), over the five rows of `experiments/encode-compare/src/cutover/harness.rs`.
- **Both:** messages are seeded; each arm takes its library's native format,
  converted before timing, and the harness digests the output of its first,
  timed call, which must equal gf2's.
  gf2 builds with `-C target-cpu=native`.
- **Harness:** [`experiments/encode-compare`](../../experiments/encode-compare/README.md).
- **Output:** `../../experiments/encode-compare/compare.sh` prints the SHA-256 of the binary and of AFF3CT's static
  library and writes `target/encode-compare/<suite>-<UTC>.md`; outside a window
  the wrapper refuses. `cutover.sh` prints the host and the SHA-256 of each
  measured binary and of the AFF3CT and M4RI libraries, writes the table and the
  raw rounds to `target/encode-compare/cutover/<UTC>/`, and labels a run outside
  a window an indication measured on `CUTOVER_CORES`. A row gives the median and
  range over rounds; a configuration whose projected call exceeds
  `CUTOVER_CALL_BUDGET_S` is reported as a projection and not run.
- **Command:**

  ```sh
  GF2_SRSRAN_ROOT="$GF2_EXT_ROOT/c077a88b/srsran" \
  GF2_AFF3CT_ROOT="$GF2_EXT_ROOT/c077a88b/aff3ct" \
  GF2_BCHLIB_ROOT="$GF2_EXT_ROOT/040bc2b8/bchlib" \
      bash experiments/encode-compare/compare.sh bch
  ```

  The cutover comparison runs `../../experiments/encode-compare/cutover.sh` with `GF2_M4RI_ROOT` naming the M4RI
  prefix and `CUTOVER_ROUNDS=3` and `CUTOVER_CALL_BUDGET_S=2000` set:

  ```sh
  CUTOVER_CALL_BUDGET_S=2000 CUTOVER_ROUNDS=3 \
  GF2_SRSRAN_ROOT="$GF2_EXT_ROOT/c077a88b/srsran" \
  GF2_AFF3CT_ROOT="$GF2_EXT_ROOT/c077a88b/aff3ct" \
  GF2_BCHLIB_ROOT="$GF2_EXT_ROOT/040bc2b8/bchlib" \
  GF2_M4RI_ROOT="$GF2_EXT_ROOT/6c6b09b1/prefix" \
      bash experiments/encode-compare/cutover.sh
  ```

<a id="nr-ldpc-encode"></a>

### NR rate-matched LDPC encoding against srsRAN and AFF3CT

- **Issues:** @/issue/12fdeb5b (baselines), @/issue/be6d6f45 (word-wise encoder).
- **Reference:** srsRAN Project 25.10 (`@/citation/Srsran2026`), whose adapter
  constructs `ldpc_encoder_avx2` when `__builtin_cpu_supports("avx2")` is true
  and `ldpc_encoder_generic` otherwise, with `ldpc_rate_matcher_impl`, and
  AFF3CT v4.7.0
  `Encoder_LDPC_QC_fast` with `Puncturer_5G` at int8 and int32 bits, both built
  as in [Reference builds](#reference-builds).
- **gf2 path:** rate-matched encoding per `@/citation/ThreeGpp2017` from $k$
  information bits to $n$ transmitted bits in `BitVec` form, redundancy version
  $0$, one bit per symbol, built `-C target-cpu=native`, at the worker counts
  `ENCODE_COMPARE_WORKERS` lists (default one and twelve). Every configuration
  passes a bit-exact equivalence gate, and a cell where AFF3CT derives another
  base graph or lifting size is reported and skipped; whole consumer, warm.
- **Harness:** [`experiments/encode-compare`](../../experiments/encode-compare/README.md).
- **Output:** the SHA-256 of the binary and of AFF3CT's static library, then the
  table at `target/encode-compare/nr-<UTC>.md`; outside a window the wrapper
  refuses.
- **Command:**

  ```sh
  GF2_SRSRAN_ROOT="$GF2_EXT_ROOT/c077a88b/srsran" \
  GF2_AFF3CT_ROOT="$GF2_EXT_ROOT/c077a88b/aff3ct" \
  GF2_BCHLIB_ROOT="$GF2_EXT_ROOT/040bc2b8/bchlib" \
      bash experiments/encode-compare/compare.sh nr
  ```

<a id="gf2x-long-product"></a>

### Binary polynomial multiplication against gf2x

- **Issues:** @/issue/53c5a8c0 (crossovers and wide products),
  @/issue/befccab3 (long products).
- **Reference:** gf2x 1.3.0 `gf2x_mul_r` (`@/citation/GfTwoX2026`) with a
  reentrant pool, built as in [Reference builds](#reference-builds) for every
  `prefix-*` install under `GF2_GF2X_ROOT`; the table reports the fastest
  prefix per size.
- **gf2 path:** the unreduced carry-less long product `clmul_slice_overwrite`
  with the `simd` feature at $16$, $64$, $256$, $1000$, $1536$ and $2048$ words
  (`SIZES` in `experiments/clmul-compare/src/lib.rs`). Operands are seeded; the arms agree word for word
  before timing. One core, warm; a rate is the median of the samples
  `experiments/clmul-compare/src/lib.rs` states.
- **Harness:** [`experiments/clmul-compare`](../../experiments/clmul-compare/README.md);
  the harness times no reduced field product.
- **Output:** the host and the SHA-256 of each measured binary and of the gf2x
  library it loads, then a table of products per second, gf2's median over the
  per-prefix builds, gf2x's fastest prefix and their ratio, at
  `target/clmul-compare/<UTC>.md`; outside a window the wrapper refuses.
- **Command:**

  ```sh
  GF2_GF2X_ROOT="$GF2_EXT_ROOT/c7113c5a" \
      bash experiments/clmul-compare/compare.sh
  ```

<a id="gf256-region"></a>

### GF($2^8$) region and matrix products against ISA-L and M4RIE

- **Issue:** @/issue/c9cb0dd2.
- **Reference:** ISA-L `gf_vect_mad`, and M4RIE's fastest of `mzed_mul`,
  `mzed_mul_newton_john` and bitsliced `mzd_slice_mul_karatsuba`, at modulus
  `0x11D`, built as in [Reference builds](#reference-builds); a row reports the
  fastest reference configuration. The reference arm `experiments/gf256-baselines/ext/arm.c` compiles with
  `cc -O2` and links `-lisal -lm4rie -lm4ri`.
- **gf2 path:** `Gf2mField::gf256_axpy` and `Gf2mField::gf256_gemm`, and the
  generic `FieldVec::axpy` and `FieldMatrix` product over `Gf256`, on one pinned
  core: region multiply-add at 4 KiB, 64 KiB, 1 MiB and 16 MiB and square
  products at $n = 64$, $256$ and $1024$. Both arms draw inputs from one seeded
  stream and agree by checksum before a cell counts.
- **Harness:** [`experiments/gf256-baselines`](../../experiments/gf256-baselines/README.md).
- **Output:** the host line labelled `window` or `indication`, the SHA-256 of
  both measured executables and of the ISA-L and M4RIE libraries, and a row per
  cell. The run measures unlocked and labels the table an indication outside a
  window.
- **Command:**

  ```sh
  GF256_BASELINE_PREFIX="$GF2_EXT_ROOT/6c6b09b1/prefix" GF256_BASELINE_CORE=8 \
      bash experiments/gf256-baselines/run.sh
  ```

<a id="f3-permanent-batched-avx2"></a>

### Batched $\mathbb{F}_3$ permanent kernel

- **Issue:** @/issue/ddd0c6ee.
- **Reference:** the scalar one-word kernel and the direct single-matrix AVX2
  kernel of the same tree, in the same `cargo bench` release build.
- **gf2 path:** the four-matrix batched AVX2 kernel on Ryser permanents over
  $\mathbb{F}_3$ in the bipedal encoding (`@/citation/Scheinerman2024`) at
  $n \in \{8, 12, 16, 20, 24, 28\}$, four matrices per call, $32$
  deterministic fixture groups from seed root `0xddd0c6ee00000000`. The three
  backends' outputs are checked equal before each size is timed, and an
  untimed doubling calibration sets each backend's call count.
- **Harness:** the `batched_f3_permanent` bench of
  [`experiments/permanent-benchmarks`](../../experiments/permanent-benchmarks/benches/batched_f3_permanent.rs),
  one execution of five $250$ ms repetitions per backend and size.
- **Output:** one raw CSV row per execution, repetition, size and backend with
  calls, matrices, elapsed nanoseconds and nanoseconds per matrix, beside the
  host name, CPU model, kernel, governor, source revision, a dirty-source flag
  and the compiler version. The harness prints no executable digest and
  computes no ratio, dispersion or interval; the wrapper pins the command to
  the CPUs in `GF2_CCX1_CPUS` (default `6-11`) and refuses outside a window.
- **Procedure:** a fresh process per execution, five executions
  (`--execution 1` to `--execution 5`) appended to one CSV, each recording a
  different fixture start. A backend's pooled rate at a size is its matrices
  summed over all rows divided by its elapsed time summed over all rows; a
  speedup is the ratio of two pooled rates. Dispersion across executions is
  computed from the per-execution pooled rates of the CSV.
- **Command:** the queue command of
  [Reproduce a performance measurement](../how-to/reproduce-performance-evidence.md#schedule-a-measurement),
  run once for each `--execution` value with `--append` and a common
  `--output`:

  ```sh
  for execution in 1 2 3 4 5; do
      dev/scripts/ccx1-bench-flock.sh \
          cargo bench -p permanent-benchmarks \
          --bench batched_f3_permanent --features simd,test-support -- \
          --execution "$execution" --repetitions 5 --target-ms 250 \
          --append --output target/benchmarks/f3-repro.csv
  done
  ```

## Comparisons without an in-tree harness

The core route (native against tuned-portable build) and LDPC decoder route
(tuned against conservative selectors) have no harness in the tree;
@/issue/31727c57 records their methods and measured values. NR LLR
de-rate-matching against AFF3CT `Puncturer_5G::depuncture` has no harness in the
tree; @/issue/eda07788 records its method and measured values.
