# Performance evidence

This page names, for each comparison gf2 makes against another implementation,
the reference and the build configuration that makes it the reference's
fastest, the gf2 path measured, the harness and the command that reproduces it.
Measured ratios, intervals and result tables are recorded in the issue each
entry cites and on the dashboard, under `@/inv/committed-evidence-budget`.
A comparison uses the reference's fastest configuration at matched output
(`@/inv/performance-deliverable`).

A result is a run inside the benchmark window, which exports
`GF2_BENCH_WINDOW=1`; outside it the harnesses refuse or label their table an
indication. Each harness's README states its timing method and statistic.
Each command is a queue line's command, run from the root of a worktree at the revision under test. Comparator trees and input bundles are
staged outside the repository and named by the environment variables shown;
`GF2_EXT_ROOT` is the staging directory of the host. Every harness prints the
host and the SHA-256 of each measured executable and comparator library, and
writes its table below `target/`. [Reproduce a performance
measurement](../how-to/reproduce-performance-evidence.md) walks through one
harness end to end.

## Comparisons

<a id="ldpc-decode-aff3ct"></a>

### LDPC decoding against AFF3CT

- **Issues:** @/issue/07ca8585 (check-node update and decode cells),
  @/issue/55c3e433 (adopted routes), @/issue/1358d6ab (integrated tree),
  @/issue/de311215 (QC route).
- **Reference:** AFF3CT v4.7.0 (`@/citation/Cassagne2019`) normalized min-sum
  decoders. The fastest configuration is the best over scalar and inter-frame
  SIMD, flooding and horizontal-layered and the layered ONMS decoder, in f32 and
  i16, each pinned per worker; the flooding f32 scalar decoder is the matched
  algorithm and the other configurations bound it from above.
- **gf2 path:** `LdpcBatchDecoder` as shipped, and its single-frame, lane, QC
  and layered routes, on the DVB-T2 rate-$1/2$ normal frame
  (`@/citation/Etsi2015`) and the NR base graph 1 mother code at lifting
  $Z = 384$ (`@/citation/ThreeGpp2017`), recorded BPSK-AWGN LLRs, flooding
  normalized min-sum with factor $0.75$, syndrome stopping.
- **Harness:** [`experiments/aff3ct-ldpc-gaps`](../../experiments/aff3ct-ldpc-gaps/README.md),
  with `aff3ct-fast-bench` and `gf2-fast-bench` as the two arms. Both build with
  `-C target-cpu=native`; AFF3CT's static library builds from its source tree.
  The comparison reports best-gf2 over best-AFF3CT frames per second, overall
  and over flooding decoders only, beside frame errors.
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
- **Reference:** bchlib (`@/citation/Bchlib2026`) `bch_encode` in its
  runtime-parameter build and in the `CONFIG_BCH_CONST_PARAMS` build fixed to
  each cell's $(m, t)$, the configuration its source documents as the faster
  encoder; AFF3CT v4.7.0 `Encoder_BCH` and `Encoder_BCH_inter` at int8 and
  int32; for generator-matrix materialization, M4RI `mzd_echelonize_m4ri` and
  `mzd_echelonize_pluq`. Every reference builds `-O3 -march=native`, and a
  table row reports the fastest configuration of each cell.
- **gf2 path:** `DvbT2BchCode::encode_into` over the rate-$1/2$ outer codes
  $(7200, 7032)$ and $(32400, 32208)$, and `BchCode::encode_batch` at batch
  $1$, $16$, $256$ and $4096$, against the pre-cutover `BchEncoder`. Messages
  are seeded `BitVec`s; every configuration reproduces gf2's codewords before
  timing.
- **Harness:** [`experiments/encode-compare`](../../experiments/encode-compare/README.md).
- **Command:**

  ```sh
  GF2_SRSRAN_ROOT="$GF2_EXT_ROOT/c077a88b/srsran" \
  GF2_AFF3CT_ROOT="$GF2_EXT_ROOT/c077a88b/aff3ct" \
  GF2_BCHLIB_ROOT="$GF2_EXT_ROOT/040bc2b8/bchlib" \
      bash experiments/encode-compare/compare.sh bch
  ```

  The cutover comparison runs `cutover.sh` with `GF2_M4RI_ROOT` naming the M4RI
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
- **Reference:** srsRAN Project 25.10 (`@/citation/Srsran2026`) encoder with the
  rate matcher its "auto" factory selects, and AFF3CT v4.7.0
  `Encoder_LDPC_QC_fast` with `Puncturer_5G` at int8 and int32 bits, all built
  `-O3 -march=native` (AFF3CT also `-funroll-loops` in the baseline campaign).
- **gf2 path:** rate-matched encoding per `@/citation/ThreeGpp2017` from $k$
  information bits to $n$ transmitted bits in `BitVec` form, redundancy version
  $0$, one bit per symbol, built `-C target-cpu=native`, at one and twelve
  workers. Every configuration passes a bit-exact equivalence gate; whole
  consumer, warm.
- **Harness:** [`experiments/encode-compare`](../../experiments/encode-compare/README.md).
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
  reentrant pool, built once for every `prefix-*` install under `GF2_GF2X_ROOT`
  (tuned and native, each `-O3 -march=native` where it applies); the table
  reports the fastest configuration per size. A full wide-field product runs
  gf2's Barrett reduction on both arms.
- **gf2 path:** the carry-less long product `clmul_slice_overwrite` with the
  `simd` feature at $16$ to $2048$ words, and `Gf2mWide::mul_ref` over $4$-word
  and $9$-word fields. Operands are seeded; the arms agree word for word before
  timing. One core, warm.
- **Harness:** [`experiments/clmul-compare`](../../experiments/clmul-compare/README.md).
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
  `0x11D`; a row reports the fastest reference configuration.
- **gf2 path:** `Gf2mField::gf256_axpy` and `Gf2mField::gf256_gemm`, and the
  generic `FieldVec::axpy` and `FieldMatrix` product over `Gf256`, on one pinned
  core: region multiply-add at 4 KiB, 64 KiB, 1 MiB and 16 MiB and square
  products at $n = 64$, $256$ and $1024$. Both arms draw inputs from one seeded
  stream and agree by checksum before a cell counts.
- **Harness:** [`experiments/gf256-baselines`](../../experiments/gf256-baselines/README.md).
- **Command:**

  ```sh
  GF256_BASELINE_PREFIX="$GF2_EXT_ROOT/6c6b09b1/prefix" GF256_BASELINE_CORE=8 \
      bash experiments/gf256-baselines/run.sh
  ```

<a id="f3-permanent-batched-avx2"></a>

### Batched $\mathbb{F}_3$ permanent kernel

- **Issue:** @/issue/ddd0c6ee.
- **Reference:** the scalar one-word kernel and the direct single-matrix AVX2
  kernel of the same tree.
- **gf2 path:** the four-matrix batched AVX2 kernel on Ryser permanents over
  $\mathbb{F}_3$ in the bipedal encoding (`@/citation/Scheinerman2024`) at
  $n \in \{8, 12, 16, 20, 24, 28\}$, four matrices per call, $32$
  deterministic fixture groups from seed root `0xddd0c6ee00000000`. The three
  backends' outputs are checked equal before each size is timed, and an
  untimed doubling calibration sets each backend's call count. A speedup is a
  ratio of pooled rates (matrices over elapsed time); the harness reports
  within- and across-execution coefficients of variation and no interval.
- **Harness:** the `batched_f3_permanent` bench of
  [`experiments/permanent-benchmarks`](../../experiments/permanent-benchmarks/benches/batched_f3_permanent.rs),
  five fresh executions of five $250$ ms repetitions on CPUs 6 to 11.
- **Command:** the queue command of
  [Reproduce a performance measurement](../how-to/reproduce-performance-evidence.md#schedule-a-measurement).
