# Performance evidence

Each claim below names its measurement method and states its workload,
baseline, host, build and measurement date, and links the evidence that
produced it at a fixed commit; that commit is the claim's source identity
unless its Build line names a measured source revision.
Ratios hold for the stated host and build. The page admits a claim only with
a committed receipt for its stated workload. Other pages cite these anchors
instead of restating figures.

## Methodology

<a id="paired-protocol"></a>

### Paired comparison protocol

Claims whose method line cites this section come from confirmatory cells of
the
[Zen 3 benchmark protocol](https://github.com/erankavija/gf2/blob/6776fe30ea9af85ad509ddca95926f051421676d/dev/active/f547c394/protocol.md).
Each such receipt records its protocol version and input digests; its source
revision is the rebuild pointer.
Its `receipt.json`, linked beside each summary, records the host, build,
workload, baseline and date.

- **Comparison basis.** A cell times two arms on one frozen workload with the
  same inputs and output contract, checked for equivalence before timing.
  Whole-consumer cells include conversion into and out of each library's
  representation in the timed call; kernel-isolated cells time the operation
  alone. These claims state
  $r = \tilde t_{\mathrm{gf2}} / \tilde t_{\mathrm{cmp}}$, the ratio of median
  gf2 time to median comparator time, so $r < 1$ means gf2 is faster.
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
  confirmation, and pilot samples never enter a confirmation. Brackets in
  these claims give each interval.
- **Host control.** The runner verifies an exclusive host lock before
  measuring, records CPU model and flags, kernel, per-CPU governor, SMT state,
  affinity and cache topology, and pins each worker to a resolved CPU. It
  records the governor as found; every such receipt cited here observed
  `powersave`. Arms are release executables built before timing and pinned by
  digest, each with its build kind, RUSTFLAGS and environment.

## Claims

<a id="ldpc-decode-aff3ct"></a>

### LDPC decoding against AFF3CT

- **Method:** [paired comparison protocol](#paired-protocol), version 4.
- **Result:** AFF3CT decodes faster on one core. With syndrome stopping,
  $r = 1.91\ [1.88, 1.91]$ for DVB-T2 and $1.74\ [1.67, 1.74]$ for NR. At a
  fixed $50$ iterations, $r = 2.21\ [2.15, 2.22]$ and $1.90\ [1.89, 1.90]$.
  The check-node update alone gives $r = 2.56\ [2.56, 2.57]$ and
  $2.28\ [2.27, 2.28]$.
- **Workload:** the DVB-T2 rate-$1/2$ normal frame, $n = 64800$, $k = 32400$
  (`@/citation/Etsi2015`), and the NR base graph 1 mother code at lifting
  $Z = 384$, $n = 26112$, $k = 8448$ (`@/citation/ThreeGpp2017`). Recorded
  BPSK-AWGN LLRs with seed $42$, $8$ frames per timed call. Both decoders run
  f32 flooding normalized min-sum, factor $0.75$, at most $50$ iterations,
  syndrome stopping except in the fixed-iteration cells. One worker, warm.
- **Baseline:** AFF3CT v4.7.0 (`@/citation/Cassagne2019`)
  `Decoder_LDPC_BP_flooding<NMS>`, scalar f32; for the check-node cells, its
  `Update_rule_NMS<float>` over the same prepared messages.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.2.
- **Build:** rustc 1.95.0; RUSTFLAGS `-C target-cpu=native` on both arms.
- **Date:** 2026-09-14.
- **Evidence:** at commit `cdbefa4be6bf`:
  [whole decode](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-comparator-single-worker-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-comparator-single-worker-confirmation/receipt.json)),
  [fixed iterations](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-fixed-iteration-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-fixed-iteration-confirmation/receipt.json)),
  [check-node update](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-checknode-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/cdbefa4be6bfe8815c7fce3fabf2c415edcca93b/dev/bench_results/07ca8585/v4-r1-07ca8585-ldpc-update-checknode-confirmation/receipt.json)).

<a id="integrated-ldpc-aff3ct"></a>

### Integrated LDPC decoding against AFF3CT

- **Method:** [paired comparison protocol](#paired-protocol), version 4.
- **Result:** gf2 is faster at one worker with eight frames per call:
  $r = 0.376\ [0.369, 0.377]$ for DVB-T2 and
  $0.325\ [0.313, 0.326]$ for NR. AFF3CT is faster with one
  frame per call, $r = 1.97\ [1.97, 1.98]$ and
  $1.78\ [1.77, 1.78]$, and at twelve physical cores with
  eight frames per worker, $r = 1.82\ [1.77, 1.84]$ and
  $1.54\ [1.50, 1.56]$.
- **Workload:** the DVB-T2 rate-$1/2$ normal frame, $n = 64800$, $k = 32400$
  (`@/citation/Etsi2015`), and the NR base graph 1 mother code at lifting
  $Z = 384$, $n = 26112$, $k = 8448$ (`@/citation/ThreeGpp2017`). Timed calls
  reuse the first frames of the recorded corpus; quality statistics cover all
  $128$ frames. Both decoders run f32 flooding normalized min-sum, factor
  $0.75$, at most $50$ iterations, syndrome stopping. Whole consumer, warm.
- **Baseline:** AFF3CT v4.7.0 (`@/citation/Cassagne2019`)
  `Decoder_LDPC_BP_flooding<NMS>`, scalar f32, one clone per worker. gf2 runs the
  production route of `LdpcBatchDecoder` under the committed coding envelope.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.11.
- **Build:** rustc 1.95.0; RUSTFLAGS `-C target-cpu=native` on both arms.
- **Date:** 2026-10-28.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/e18288afa5a92442eb8d8066c032520d448f2aa4/dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/e18288afa5a92442eb8d8066c032520d448f2aa4/dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/receipt.json))
  at commit `e18288afa5a9`.

<a id="integrated-bch-encode"></a>

### Integrated BCH encoding against AFF3CT and bchlib

- **Method:** [paired comparison protocol](#paired-protocol), version 4.
- **Result:** gf2 is faster than AFF3CT on the GF($2^{14}$) mother code,
  $r = 0.268\ [0.267, 0.269]$ against its scalar encoder at one
  message and $0.437\ [0.384, 0.440]$ against its
  interleaved encoder at $256$ messages. bchlib is faster on the DVB-T2 outer
  codes: $r = 106\ [105, 107]$ and
  $107\ [107, 108]$ at $n = 7200$ with $1$ and $256$
  messages, $105\ [104, 106]$ and
  $106\ [106, 107]$ at $n = 32400$.
- **Workload:** systematic encoding of seeded `BitVec` messages: the mother code
  $n = 16383$, $k = 16215$, and the DVB-T2 rate-$1/2$ outer codes $(7200, 7032)$
  and $(32400, 32208)$ (`@/citation/Etsi2015`). Every pair reports codewords equal
  to gf2's. Kernel-isolated, one core, warm.
- **Baseline:** AFF3CT v4.7.0 (`@/citation/Cassagne2019`) `Encoder_BCH` and
  `Encoder_BCH_inter`; bchlib (`@/citation/Bchlib2026`) `bch_encode`, four-table
  remainder over packed bytes; all built `-O3 -march=native`.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.11.
- **Build:** rustc 1.95.0; gf2 conservative-portable, no RUSTFLAGS.
- **Date:** 2026-10-29.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/ce0bfdccb8bde17ac0dc993b2935ee9b95f122ef/dev/bench_results/1358d6ab/bch-integration/1358d6ab-bch-integration-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/ce0bfdccb8bde17ac0dc993b2935ee9b95f122ef/dev/bench_results/1358d6ab/bch-integration/1358d6ab-bch-integration-confirmation/receipt.json))
  at commit `ce0bfdccb8bd`.

<a id="integrated-core-route"></a>

### Integrated core route: native against tuned portable

- **Method:** [paired comparison protocol](#paired-protocol), version 4. Here the
  comparator is the tuned-portable build of gf2 and $r$ is its median time over
  the native build's, so $r > 1$ means the native build is faster.
- **Result:** $r = 1.34\ [1.17, 1.35]$ for the matrix-vector
  product, $1.02\ [1.02, 1.02]$ for population count and
  $1.07\ [1.07, 1.07]$ for the nine-word long product. The first
  meets the frozen improvement margin; the others are not worse within the frozen
  equivalence margin.
- **Workload:** kernels at $1024 \times 1536$ bits, $1024$ words and nine words
  through the production dispatch. Kernel-isolated, one core, warm.
- **Baseline:** the tuned-portable release build of the same tree.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.11.
- **Build:** rustc 1.95.0; RUSTFLAGS `--cfg gf2_tuning_baked`, native arm with
  `-C target-cpu=native`.
- **Date:** 2026-10-29.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57/core/31727c57-core-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57/core/31727c57-core-confirmation/receipt.json))
  at commit `d9cbcfcddaea`.

<a id="integrated-decoder-route"></a>

### Integrated decoder route: tuned against conservative selectors

- **Method:** [paired comparison protocol](#paired-protocol), version 4. Here the
  comparator is the conservative-portable single-frame arm and $r$ is its median
  time over the tuned arm's, so $r > 1$ means the tuned arm is faster.
- **Result:** the tuned arm is not worse within the frozen equivalence margin at
  every confirmatory cell: DVB-T2 $r = 1.00\ [1.00, 1.01]$ at
  one worker with one frame and $1.01\ [1.00, 1.01]$ at six
  physical cores; NR $1.01\ [0.999, 1.02]$ at twelve
  physical cores and $1.00\ [0.998, 1.01]$ at twenty-four
  logical CPUs. Each cell's route is its `selected_path`.
- **Workload:** the LDPC workloads of
  [integrated LDPC decoding](#integrated-ldpc-aff3ct), eight frames per worker at
  more than one worker. Whole consumer, warm.
- **Baseline:** `LdpcBatchDecoder` conservative-portable single-frame arm of the
  same tree.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.11.
- **Build:** rustc 1.95.0; no native target on either arm.
- **Date:** 2026-10-29.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57/decoder/31727c57-decoder-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/d9cbcfcddaea049e001f16326e5550a9d212706d/dev/active/1a379447-zen3-cpu-performance/31727c57/decoder/31727c57-decoder-confirmation/receipt.json))
  at commit `d9cbcfcddaea`.

<a id="nr-ldpc-encode"></a>

### NR rate-matched LDPC encoding against srsRAN and AFF3CT

- **Method:** [paired comparison protocol](#paired-protocol), version 4.
- **Result:** srsRAN encodes faster at every measured size:
  $r = 6.89\ [6.83, 6.98]$ at BG2 $n = 256$, $34.6\ [34.4, 34.9]$ at BG2
  $n = 1440$, $80.4\ [79.9, 80.8]$ at BG1 $n = 2560$ and $151\ [149, 152]$ at
  BG1 $n = 8448$. Against AFF3CT, gf2 is faster at BG2 $n = 256$,
  $r = 0.475\ [0.473, 0.477]$, and slower at BG1 $n = 2560$,
  $r = 1.98\ [1.97, 1.99]$.
- **Workload:** rate-matched encoding per `@/citation/ThreeGpp2017` from $k$
  information bits to $n$ transmitted bits in `BitVec` form, redundancy
  version $0$, one bit per symbol. Every configuration passes a bit-exact
  equivalence gate. Whole consumer, one core, warm.
- **Baseline:** srsRAN Project 25.10 (`@/citation/Srsran2026`) encoder plus
  rate matcher, built `-O3 -march=native`; AFF3CT v4.7.0 QC encoder plus
  puncturer, built `-O3 -march=native -funroll-loops`.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.2.
- **Build:** rustc 1.95.0; gf2 `-C target-cpu=native`.
- **Date:** 2026-09-13.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/8896b2a339846f28a5d29bed8da346f45c730a2b/dev/bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/8896b2a339846f28a5d29bed8da346f45c730a2b/dev/bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/receipt.json))
  at commit `8896b2a33984`.

<a id="nr-llr-derate"></a>

### NR LLR de-rate-matching against AFF3CT

- **Method:** [paired comparison protocol](#paired-protocol), version 3.
- **Result:** gf2 is faster at every measured configuration,
  $0.32 \le r \le 0.35$, each interval within $[0.318, 0.351]$. A build for
  baseline x86-64 matches the native build, $r = 0.999\ [0.990, 1.003]$.
- **Workload:** de-rate-matching of one received LLR frame to the mother-code
  length per `@/citation/ThreeGpp2017`, for BG2 $n \in \{256, 1024, 1440\}$
  and BG1 $n \in \{1320, 2560\}$, with and without filler bits. Whole
  consumer, one core, warm.
- **Baseline:** AFF3CT v4.7.0 `Puncturer_5G::depuncture`, built
  `-O3 -march=native -funroll-loops`.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.2.
- **Build:** rustc 1.97.0; gf2 `-C target-cpu=native`, control arm
  `-C target-cpu=x86-64`.
- **Date:** 2026-09-10.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/f88bd137298d93f57f8867acc1f9207186ffbaca/dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/f88bd137298d93f57f8867acc1f9207186ffbaca/dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/receipt.json))
  at commit `f88bd137298d`.

<a id="gf2x-long-product"></a>

### Binary polynomial multiplication against gf2x

- **Method:** [paired comparison protocol](#paired-protocol), version 4.
- **Result:** gf2 is faster at $4$ and $9$ words, $r = 0.83\ [0.80, 0.84]$
  and $0.767\ [0.765, 0.769]$; gf2x is faster at $16$ words,
  $r = 97.1\ [96.7, 97.2]$. For a full wide-field product with gf2's Barrett
  reduction on both arms, $r = 1.392\ [1.385, 1.414]$ at $4$ words and
  $1.115\ [1.112, 1.122]$ at $9$ words, the latter below the family's $1.25$
  material-gap threshold.
- **Workload:** the carry-less long product of two $w$-word operands through
  the public `clmul_wide_slice` into a fresh destination, kernel-isolated;
  `Gf2mWide::mul_ref` over $4$-word and $9$-word fields, whole consumer. One
  core, warm.
- **Baseline:** gf2x 1.3.0 `gf2x_mul_r` (`@/citation/GfTwoX2026`), built
  `-O3 -march=native`.
- **Hardware:** AMD Ryzen 9 5900X, SMT on, Linux 7.2.2.
- **Build:** rustc 1.95.0; gf2 build kind `native`, no RUSTFLAGS recorded.
- **Date:** 2026-09-13.
- **Evidence:** [receipt summary](https://github.com/erankavija/gf2/blob/4fd96ef94d8176712b80b16662f1f067acd5c4a0/dev/bench_results/53c5a8c0/2026-09-13-53c5a8c0-polynomial-confirmation/acceptance-summary.md) ([`receipt.json`](https://github.com/erankavija/gf2/blob/4fd96ef94d8176712b80b16662f1f067acd5c4a0/dev/bench_results/53c5a8c0/2026-09-13-53c5a8c0-polynomial-confirmation/receipt.json))
  at commit `4fd96ef94d81`.

<a id="f3-permanent-batched-avx2"></a>

### Batched $\mathbb{F}_3$ permanent kernel

- **Method:** the receipt's harness protocol. Before each size is timed, the
  three backends' outputs are checked equal, and an untimed doubling
  calibration sets each backend's call count. Five fresh executions of five
  $250$ ms repetitions run under the host lock on CPUs 6 to 11. The speedup
  is a ratio of pooled rates (matrices over elapsed time); the receipt
  reports within- and across-execution coefficients of variation and no
  interval.
- **Result:** the four-matrix batched AVX2 kernel leads the scalar one-word
  kernel by $3.57\times$ at $n = 8$ and by $5.29\times$ to $6.17\times$ for
  $12 \le n \le 28$. The direct single-matrix AVX2 kernel is $2.86\times$ to
  $3.35\times$ slower than scalar.
- **Workload:** Ryser permanents over $\mathbb{F}_3$ in the bipedal encoding
  (`@/citation/Scheinerman2024`) at $n \in \{8, 12, 16, 20, 24, 28\}$, four
  matrices per call, $32$ deterministic fixture groups from seed root
  `0xddd0c6ee00000000`.
- **Baseline:** the scalar one-word kernel.
- **Hardware:** AMD Ryzen 9 5900X, Linux 7.1.6, governor `powersave`.
- **Build:** rustc 1.95.0, `cargo bench` with features `simd` and
  `test-support`; the receipt's command sets no RUSTFLAGS; measured source
  revision `88474a74ceee`.
- **Date:** 2026-08-10.
- **Evidence:** [receipt](https://github.com/erankavija/gf2/blob/a8937d14ce000cc4fbcde5b2afc0d9e6da624eef/dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md)
  at commit `a8937d14ce00`.
