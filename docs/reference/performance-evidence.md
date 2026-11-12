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
indication. Each command is a queue line's command, run from the root of a
worktree at the revision under test. Comparator trees and input bundles are
staged outside the repository and named by the environment variables shown;
`GF2_EXT_ROOT` is the staging directory of the host. Every harness prints the
host and the SHA-256 of each measured executable and comparator library, and
writes its table below `target/`. [Reproduce a performance
measurement](../how-to/reproduce-performance-evidence.md) walks through one
harness end to end.

## Methodology

<a id="paired-protocol"></a>

### Paired comparison protocol

Entries that cite this section run confirmatory cells of the
[Zen 3 benchmark protocol](https://github.com/erankavija/gf2/blob/6776fe30ea9af85ad509ddca95926f051421676d/dev/active/f547c394/protocol.md).
A receipt records its protocol version, input digests, host, build, workload
and baseline. Receipts, acceptance summaries and result tables live outside the
tracked tree; the issue named in each entry records the results.

- **Comparison basis.** A cell times two arms on one frozen workload with the
  same inputs and output contract, checked for equivalence before timing.
  Whole-consumer cells include conversion into and out of each library's
  representation in the timed call; kernel-isolated cells time the operation
  alone. A paired ratio is
  $r = \tilde t_{\mathrm{gf2}} / \tilde t_{\mathrm{cmp}}$, the ratio of median
  gf2 time to median comparator time, so $r < 1$ means gf2 is faster. The throughput tables of the
  `compare.sh` harnesses state the inverse orientation, a rate ratio above $1$
  meaning gf2 is faster, in their headers.
- **Warm-up.** Every execution is a fresh process. A warm cell makes one
  untimed pass over its working set, then calibrates the call count so each
  of five timing windows lasts about $100$ ms. The execution's value is its
  median time per call over those windows.
- **Statistical treatment.** The sampling unit is a pair of adjacent baseline
  and candidate executions. A confirmatory cell has $24$ pairs in
  counterbalanced, seed-ordered blocks. The estimate is the ratio of the arms'
  medians; its interval is a percentile bootstrap over whole pairs with
  $10\,000$ resamples (`@/citation/Efron1979`) at the cell's corrected
  confidence: Bonferroni within an attempt (`@/citation/Dunn1961`) under a
  family-wise budget of $\alpha = 0.05$ across attempts. No sample is
  discarded. A window at twice its execution's median or more is flagged, and
  a cell with more than a tenth of its windows flagged does not qualify.
  Decision margins are frozen from a pilot's measured resolution before
  confirmation, and pilot samples never enter a confirmation.
- **Host control.** The runner verifies an exclusive host lock before
  measuring, records CPU model and flags, kernel, per-CPU governor, SMT state,
  affinity and cache topology, and pins each worker to a resolved CPU. It
  records the governor as found. Arms are release executables built before
  timing and pinned by digest, each with its build kind, RUSTFLAGS and
  environment.

## Comparisons

<a id="ldpc-decode-aff3ct"></a>

### LDPC decoding against AFF3CT

- **Issues:** @/issue/07ca8585 (check-node update and decode cells),
  @/issue/55c3e433 (adopted routes), @/issue/de311215 (QC route).
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

  The confirmatory form is the [paired protocol](#paired-protocol) campaign:
  `record-gf2-arms-build.py` pins each arm, `make-family.py` materializes a plan
  from a declaration under
  [`families/`](../../experiments/aff3ct-ldpc-gaps/families/), and
  `run-family-campaign.py` runs it, as the harness README describes.

<a id="integrated-ldpc-aff3ct"></a>

### Integrated LDPC decoding against AFF3CT

- **Issue:** @/issue/1358d6ab.
- **Reference:** AFF3CT v4.7.0 flooding normalized min-sum, one decoder clone
  per worker; the configurations of [LDPC decoding](#ldpc-decode-aff3ct) bound
  its fastest build.
- **gf2 path:** the production route of `LdpcBatchDecoder` under the committed
  coding envelope, one worker with one and eight frames per call and twelve
  physical cores with eight frames per worker. Whole consumer, warm.
- **Harness:** the campaign tools of
  [`experiments/aff3ct-ldpc-gaps`](../../experiments/aff3ct-ldpc-gaps/README.md)
  with the `gf2-batch-tuned` and `aff3ct-flooding-nms-f32` arms of
  [`arms.json`](../../experiments/aff3ct-ldpc-gaps/arms.json), both built
  `-C target-cpu=native`, under the [paired protocol](#paired-protocol).
- **Command:** the `quick-compare.sh` command of
  [LDPC decoding](#ldpc-decode-aff3ct) for the indication over every
  configuration; the confirmation runs `run-family-campaign.py` on a plan
  materialized by `make-family.py` for the arms above.

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

<a id="integrated-core-route"></a>

### Integrated core route: native against tuned portable

- **Issue:** @/issue/31727c57.
- **Reference:** the tuned-portable release build of the same tree; $r$ is its
  median time over the native build's, so $r > 1$ means the native build is
  faster.
- **gf2 path:** the matrix-vector product at $1024 \times 1536$ bits, population
  count over $1024$ words and the nine-word long product through the production
  dispatch. Kernel-isolated, one core, warm.
- **Harness:** the `profile-arm` executable of the survey harness recorded at
  [`d9cbcfcddaea`](https://github.com/erankavija/gf2/tree/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57),
  under the [paired protocol](#paired-protocol). Arms: conservative-portable,
  `--cfg gf2_tuning_baked` (tuned-portable) and `--cfg gf2_tuning_baked` with
  `-C target-cpu=native` (native).
- **Command:** the campaign command recorded in that directory, run in the
  benchmark window from a worktree at the commit.

<a id="integrated-decoder-route"></a>

### Integrated decoder route: tuned against conservative selectors

- **Issue:** @/issue/31727c57.
- **Reference:** `LdpcBatchDecoder` in its conservative-portable single-frame
  arm of the same tree; $r$ is its median time over the tuned arm's.
- **gf2 path:** `LdpcBatchDecoder` of default construction under the committed
  coding envelope, portable build, on the LDPC workloads of
  [integrated LDPC decoding](#integrated-ldpc-aff3ct) at one, six and twelve
  physical cores and twenty-four logical CPUs. Whole consumer, warm.
- **Harness:** the `gf2-batch-single-frame-arm` and `gf2-batch-tuned-arm`
  executables of the survey harness recorded at
  [`d9cbcfcddaea`](https://github.com/erankavija/gf2/tree/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57),
  under the [paired protocol](#paired-protocol). Each cell's route is its
  `selected_path`.
- **Command:** the campaign command recorded in that directory, run in the
  benchmark window from a worktree at the commit.

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

<a id="nr-llr-derate"></a>

### NR LLR de-rate-matching against AFF3CT

- **Issue:** @/issue/eda07788.
- **Reference:** AFF3CT v4.7.0 `Puncturer_5G::depuncture`, built
  `-O3 -march=native -funroll-loops`, through a C-ABI shim.
- **gf2 path:** de-rate-matching of one received LLR frame to the mother-code
  length per `@/citation/ThreeGpp2017`, for BG2 $n \in \{256, 1024, 1440\}$ and
  BG1 $n \in \{1320, 2560\}$, with and without filler bits, built
  `-C target-cpu=native`; a control arm builds `-C target-cpu=x86-64`. Whole
  consumer, one core, warm.
- **Harness:** the `nr-derate` arms and `nr-derate-build.sh` of the survey
  recorded at
  [`f88bd137298d`](https://github.com/erankavija/gf2/tree/f88bd137298d/dev/active/eda07788/survey),
  under the [paired protocol](#paired-protocol), version 3.
- **Command:** from a worktree at that commit, the survey's
  `nr-derate-build.sh <aff3ct-root>` builds and gates the arms, and the
  campaign command follows the issue's addenda in the benchmark window.

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
