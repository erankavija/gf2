# External-baseline survey for BCH encoding and generator-matrix materialization

| Field | Value |
|---|---|
| JIT issue | `4e732b56` (Reproducible external-baseline survey and workload selection) |
| Epic | `ae03bcd0` (Harden and generalize BCH codes over finite fields) |
| Contract produced | [`workload-selection`](workload-selection.md) |
| Criteria | REQ-13, REQ-14 of the epic; D-04 (encoding families), D-07 (evidence protocol) |
| Harness | `dev/active/4e732b56/baseline-survey/` |
| Receipts | `dev/bench_results/4e732b56/` |
| Status | Complete. Both workloads measured, baselines selected, contract fixed; the timed-route matrix check records a discrepancy. |

## 1. Question

Two questions, both of which the epic's optimization tasks need answered before
they start, and neither of which the repository could answer:

1. **What is the strongest reproducible external implementation of large-batch
   binary BCH encoding, and of GF(2) generator-matrix materialization, on this
   host?** Without it, REQ-13's "close the gap to the best available
   implementation" has no referent and REQ-14's comparison rows have no
   opposing column.
2. **Which encoding algorithm families are worth registering behind the
   dispatch seam?** Decision D-04 requires at least one scalar reference and at
   least two registered families, and deliberately refuses to name them without
   evidence.

The survey answers both by building candidates from pinned sources, measuring
them against the same codes, and fixing the result as the
[`workload-selection`](workload-selection.md) contract.

## 2. Why the existing SOTA target matrix does not answer this

The [SOTA target matrix](../../plans/sota_target_matrix.md) is the repository's reference-selection
keystone, and this survey follows its pinning and receipt conventions. It does
not, however, contain a cell for either workload, for three reasons it records
itself:

* Its operation surface is linear-algebra-centric — `fgemm`, `pluq`,
  `echelon`, `invert`, `solve`, `charpoly`, `minpoly`, and four sparse
  operations (§ 3.1). Neither BCH encoding nor generator-matrix construction is
  an operation in it.
* It places the coding-domain reference libraries out of scope explicitly:
  "gf2-coding decoder references (AFF3CT, IT++) — separate program tracked
  outside epic `97bf0879`. No row in § 5 carries an AFF3CT/IT++ reference"
  (§ 2, *Out of scope*). This survey is that separate program's encoder half.
* Its $\mathrm{GF}(2^m)$ coverage is a documented gap. Of its 20 exclusion
  cells, 18 are over $\mathrm{GF}(2^m)$, all classed `no-independent-oracle`
  (§ 6.1, § 6.2), and $\mathrm{GF}(2^m)$ carries a canonical reference for
  `matmul` alone. The pre-cutover gf2 BCH encoder computes over
  $\mathrm{GF}(2^m)$ *field elements*, so the matrix offers it no reference at
  all.

What the matrix does supply, and this survey reuses rather than re-litigates,
is the canonical dense GF(2) reference: **M4RI 20260122**, canonical for
`matmul`, `echelon`, `invert`, `solve`, and `pluq` over $\mathrm{GF}(2)$
(§ 5.1–§ 5.5). Workload W2's output is a dense GF(2) matrix, so M4RI is a
natural candidate for it, at the pin the matrix already carries; § 7 records
where the measurement placed it.

## 3. Candidate roster

Every candidate was fetched at a named version and either built and measured,
or rejected with the evidence for the rejection. Pins and build recipes are
executable: `baseline-survey/fetch-build.sh` carries them.

| Candidate | Version | Pin | Disposition |
|---|---|---|---|
| bchlib (userspace `lib/bch.c`) | v2.1.3 | commit `8d0656ab8f37e734428635501738d360ad80eebd` | **Measured** — W1 |
| AFF3CT | v4.7.0 | commit `e8a65c5047262d97a15563b9edc961f69b2792cc` | **Measured** — W1, W2 |
| M4RI | 20260122 | tarball sha256 `7e033ca1…46404` | **Measured** — W2 |
| IT++ | 4.3.1 | tarball sha256 `50717621…3b877` | **Measured** — W1, full-length rows only |
| liquid-dsp | v1.8.2 | commit `03d052b89b543e6d5f9a882139ba19f7683bcd29` | **Rejected** — no BCH encoder |
| kodo / steinwurf | — | — | **Rejected** — not publicly retrievable |

### 3.1 bchlib v2.1.3 — table-driven remainder over packed bytes

`bchlib/src/bch.c` is a dual-target source: under `__KERNEL__` it includes
`<linux/bch.h>` and compiles as the Linux kernel's generic BCH library
(`bch.c:68-77`), and without it as a freestanding userspace translation unit.
Copyright Ivan Djelic / Parrot S.A., GPL-2.0 [Djelic2011]. The survey compiles
only `src/bch.c`; the Python extension is not built.

Its encoder is the `table-remainder` family, stated in its own header comment
at `bch.c:45-46`: "Encoding is performed by processing 32 input bits in
parallel, using 4 remainder lookup tables." Crucially it consumes **packed
bytes**, which is the representation `gf2_core::BitVec` uses, so its figures
are not inflated by a representation the gf2 side cannot adopt.

* API: `bch_init(m, t, prim_poly, swap_bits)` and
  `bch_encode(bch, data, len_bytes, ecc)`.
* Build: `gcc -std=c11 -O3 -march=native`, source compiled directly into the
  harness.
* Limitation, recorded not worked around: `bch_encode` takes its message
  length in **bytes**, so contract rows whose $k$ is not a multiple of 8 (B1
  with $k = 5$, B3 with $k = 223$) cannot be measured. B2, T2S and T2N are.
  The harness reports the skip rather than measuring a nearby shape.
* Agreement check: for B2 the library derives `ecc_bits = 63`, matching the
  contract row's $n - k = 63$ exactly, so it is encoding the contract's code
  rather than a code of similar size.

### 3.2 AFF3CT v4.7.0 — scalar and SIMD-interleaved LFSR

AFF3CT is the reference open-source forward-error-correction toolbox
[Cassagne2019]. It supplies two BCH encoders:

* `module::Encoder_BCH<int>` — the bit-serial LFSR recurrence, one frame at a
  time (`src/Module/Encoder/BCH/Encoder_BCH.cpp:46-56`). The
  `poly-remainder-scalar` family.
* `module::Encoder_BCH_inter<int>` — the same recurrence advanced across
  `mipp::N<int>()` frames at once in SIMD registers, with the batch transposed
  in and out by `Reorderer_static`
  (`src/Module/Encoder/BCH/Encoder_BCH_inter.cpp:35-68`). The
  `bitslice-interleaved` family. On this host MIPP resolves to AVX2, so the
  wave is 8 frames wide.

Both store **one 32-bit word per bit**, so their memory traffic is 32× a
packed representation at the same code. This is a property of the baseline,
not a measurement artifact. The throughput unit counts information bits rather
than the words a library moves, and § 7 weighs the representation difference
explicitly rather than normalizing it away.

**Generator-polynomial agreement.** `tools::BCH_polynomial_generator<int>`
accepts the primitive polynomial explicitly, so the harness supplies the same
polynomial `gf2-core` uses. For the two DVB-T2 rows the resulting generator
polynomial is **bit-identical to the ETSI EN 302 755 generator** [Etsi2015]:
degree 168 for the short frame and 192 for the normal frame, equal
coefficient-by-coefficient to the product of the twelve minimal polynomials
tabulated at `crates/gf2-coding/src/bch/dvb_t2/generators.rs:14-46`. The
baseline therefore encodes the same code as gf2, not merely a code of the same
shape.

**Build note, recorded because it silently corrupts otherwise.** The AFF3CT
static library is compiled `-std=gnu++11` with
`-DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING
-DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE -DNDEBUG`. Those
definitions gate class members, so a translation unit that links against the
library without them has different class layouts and the process corrupts its
heap on the first `Encoder` construction. The harness matches them exactly.

**Measurement entry point.** AFF3CT's public `encode()` routes through the
StreamPU task/socket runtime. The harness calls the protected `_encode` kernel
directly through a thin adapter, so the figures are the encoder's own cost
without the framework's scheduling. This favors the baseline, which is the
correct direction for a target.

### 3.3 M4RI 20260122 — dense GF(2) elimination

Pinned at the version and sha256 that `benchmarks/image.lock` already carries
[AlbrechtBard2026], built with the Containerfile's flags
(`CFLAGS="-O3 -march=native -fPIC"`, `./configure --disable-static`). Used for
three W2-relevant cells:

* `genmatrix-rref` — fill the $k \times n$ polynomial-form generator matrix
  (row $i$ is $g(x)$ shifted by $i$), then `mzd_echelonize_m4ri` to reach the
  systematic form. An algorithm wholly independent of encoding basis vectors.
* `echelonize` — the same elimination on a random matrix of the same shape,
  isolating the elimination from the structured fill.
* `matmul-m4rm` — `mzd_mul_m4rm` of a $B \times k$ by $k \times n$ matrix: the
  dense form of the `genmatrix-multiply` encoding family.

The dump used for correctness is produced by the same `genmatrix_rref` helper
as the timed `genmatrix-rref` route. Coordinate normalization is owned by the
verifier, not by this benchmark.

### 3.4 IT++ 4.3.1 — polynomial remainder over GF(2^m) elements

Measured, rather than dismissed, because its algorithm is the one the
pre-cutover gf2 encoder uses [ITPP2013]: `itpp::BCH::encode` builds `GFX`
polynomials over $\mathrm{GF}(n+1)$ and calls `modgfx`
(`itpp/comm/bch.cpp:150-199`), exactly as `BchEncoder::encode` builds a
`Gf2mPoly` and calls `div_rem` (`crates/gf2-coding/src/bch/core.rs:457`). It
is the control that shows what the algorithmic choice costs in a mature
library, separate from anything specific to gf2's implementation.

`itpp::BCH::encode` is natively a batch call: it splits its input into
$\lfloor \text{len} / k \rfloor$ messages.

* Limitation, recorded not worked around: `itpp::BCH` represents polynomials
  over $\mathrm{GF}(n+1)$ and asserts that $n + 1$ is a power of two
  (`itpp/comm/bch.cpp:60`). The shortened rows T2S and T2N cannot be
  constructed; B1, B2 and B3 are.
* Its constructor selects its own primitive polynomial, so its code has the
  same $(n, k, t)$ as the contract row but not necessarily the same generator.
  The comparison is of encoding work at equal shape.
* Build: `cmake -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_CXX_FLAGS="-O3 -march=native" -DHTML_DOCS=off -DLATEX_DOCS=off`. The
  policy override is required because IT++'s CMake files predate CMake 3.5 and
  current CMake refuses them outright.

### 3.5 liquid-dsp v1.8.2 — rejected, no BCH encoder

liquid-dsp [LiquidDSP2025] was considered as a large-batch encoder candidate
and carries no BCH implementation at this version:

* Its FEC scheme enumeration at `include/liquid.h:1752-1789` lists all 28
  supported schemes: none, repeat-3/5, four Hamming variants, Golay(24,12),
  three SEC-DED variants, twelve convolutional variants (via libfec), and
  `LIQUID_FEC_RS_M8`, a single Reed–Solomon (255,223). No BCH entry exists.
* `src/fec/src/` contains no BCH source file, and the string `bch` does not
  occur anywhere under `src/fec/`.

This is a coverage finding, not a performance one: liquid-dsp cannot be
compared on either workload because it implements neither.

### 3.6 kodo / steinwurf — rejected, not retrievable

A fresh retrieval attempt is recorded in the [Kodo retrieval
receipt](../../bench_results/4e732b56/2026-08-31-4e732b56-kodo-retrieval.txt):
at 2026-09-01 00:05 EEST, an unauthenticated `curl` against the repository
page returned HTTP 404, and the anonymous `git clone` exited 128, refusing to
proceed because it needed a username with terminal prompts disabled. No
version could therefore be pinned, built, or measured from this host without
credentials this survey does not have.
Independently of retrieval, the [Kodo documentation](https://kodo.steinwurf.com/)
describes an erasure-coding library centered on random linear network coding,
not BCH encoding or dense GF(2) generator-matrix materialization
([KodoSteinwurf2026]). It has no comparable API for either workload.

## 4. Measurement protocol

**Host and serialization.** All measurements run through
`dev/scripts/ccx1-bench-flock.sh`, which holds the `/tmp/gf2-ccx1.lock` mutex
exclusively for the child and pins it to the CCX1 cores (`taskset -c 6-11`).
The wrapper requested `nice -n -5`, but the request was denied; the child ran
at the inherited default priority. Host model, microarchitecture, cache topology, frequency
governor, OS, and compiler versions are captured by `run-survey.sh` into the
`host.txt` of each run. Every committed figure was taken after the shared
benchmark host was released; the harness-validation runs that preceded the
release were bounded to seconds and none of their output is committed or
quoted.

**Worker count.** Every figure in this survey is single-threaded, the worker
count the contract fixes for baseline comparison. All four baselines are
single-threaded as built: bchlib and the AFF3CT encoder kernels contain no
threading, and M4RI was configured without OpenMP. The committed [M4RI
thread evidence](../../bench_results/4e732b56/2026-08-31-4e732b56-m4ri-thread-evidence.txt)
contains the installed `m4ri_config.h` (`__M4RI_HAVE_OPENMP 0`) and `ldd`/
`readelf -d` output showing no OpenMP runtime dependency. No figure here may be
read as a multi-threaded result.

**Statistics.** Each cell takes up to 7 independent trials. A trial repeats the
measured call enough times to span at least 5 ms, so a cell whose single call
sits near the clock's resolution is still resolved, and the reported figure is
always per one call. Results are reported as the median over trials with the
observed minimum and maximum, and the spread as a percentage of the median.
The AFF3CT and M4RI W2 B1–B3 cells were re-run with repetition counts chosen so
the aggregate timed call spans at least 5 ms; those counts are recorded in
their committed logs. The two complete 2026-09-01 survey runs use the same
calibration path, and the audit below found no measured cell without a
calibrated repetition count.
Trial counts are reported per cell and are below 7 wherever a cell's wall
budget cut it short.

**Bounding.** Each cell carries a wall budget: 90 s for the encoder harnesses,
45 s for the M4RI cells. A cell whose single repetition exceeds its budget is
reported as a **projection** from a measured per-unit cost, labeled as an
estimate, never as a measurement. This is what keeps the largest shapes —
where the pre-cutover gf2 path costs minutes per call — from consuming the
measurement window.

**Determinism.** Fixtures derive from seed `0xAE03BCD0`, the epic's seed, via
splitmix64 in the C and Rust harnesses and `std::mt19937_64` in the C++ one.
Every row carries an FNV-1a digest of the cell's output, so a re-run that
produces different numbers can be distinguished from one that produced
different *results*. A seeded AFF3CT rerun checked 98 W1 and 14 W2 B1/B2
comparisons against the new survey CSVs; every digest matched in the [determinism
agreement receipt](../../bench_results/4e732b56/2026-09-01-4e732b56-determinism-agreement.txt).

**Reproduction.**

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh <out-dir> [codes] [prefix]
dev/active/4e732b56/baseline-survey/summarize.py <out-dir> --markdown
```

**What is compared.** W1 is reported in information bits per second,
$Bk/T$; W2 in matrix bits per second, $kn/T$. Both are representation-neutral:
they count the code's information content, not the bytes a particular library
chose to move.

## 5. Results

Full per-cell figures — 128 cells, comprising 126 measured cells and 2
explicit projections, with trial counts, medians, extrema, and spreads where
measured — are in the receipt at
[`dev/bench_results/4e732b56/2026-09-01-4e732b56-survey-receipt.md`](../../bench_results/4e732b56/2026-09-01-4e732b56-survey-receipt.md), rendered
from the committed CSVs. This section quotes only the cells the selection turns
on.

Dispersion across the 126 cells that took more than one trial has a median
spread of 16.71% of the median. Ninety-four cells exceed 5% and 84 exceed 10%.
The new host record remains `powersave`, and the spread is recorded rather than
smoothed away: at the selection comparison batches, bchlib has 25.4% (T2S) and
24.9% (T2N) spreads, while M4RI `genmatrix-rref` has 20.6% at B2, 21.1%
at B3, 2.7% at T2S, and 2.9% at T2N. Per-cell spreads are in the receipt.

Two cells are **estimates**, not measurements, and are marked as such
throughout: the gf2 side could not run T2N at $B = 4096$ or materialize T2N's
generator matrix inside its 90 s cell budget.

### 5.1 W1 — large-batch encoding, $B = 4096$

Median information bits per second. Most cells took the full 7 trials; a few
of the slowest took 3 when their wall budget ran out, and the receipt records
the count per cell.

| Row | $\deg g$ | bchlib `table-remainder` | AFF3CT `bitslice-interleaved` | AFF3CT `poly-remainder-scalar` | IT++ `poly-remainder-gfx` | gf2 `encode_batch` |
|---|---|---|---|---|---|---|
| B1 | 10 | not byte-aligned | 349.2 | 92.6 | 5.37 | 5.72 |
| B3 | 32 | not byte-aligned | 381.3 | 83.7 | 0.34 | 3.11 |
| B2 | 63 | **1931.9** | 162.3 | 71.8 | 0.58 | 1.71 |
| T2S | 168 | **4018.1** | 60.3 | 38.2 | shortened | 0.68 |
| T2N | 192 | **4063.8** | 54.6 | 33.7 | shortened | 0.57 *(estimate)* |

Rows are ordered by generator degree, which is what the families separate on.

At the two DVB-T2 rows — the parameters the epic actually ships — the pinned
baseline encodes **4018.1 and 4063.8 Mbit/s** where the current gf2 encoder
reaches **0.68 and 0.60**. Comparing measured cell against measured cell — T2S at
$B = 4096$ for both, T2N at $B = 256$ for both, since gf2's T2N $B = 4096$ cell
is the estimate above — the baseline leads by a factor of **5867 at T2S** and
**7024 at T2N**.

### 5.2 W2 — generator-matrix materialization

Median matrix bits per second for a full $k \times n$ systematic $G$.

| Row | Dimensions | M4RI `genmatrix-rref` | AFF3CT `basis-encode-pack` | gf2 `generator_matrix` |
|---|---|---|---|---|
| B1 | $5 \times 15$ | 267.4 | 245.5 | 5.51 |
| B2 | $64 \times 127$ | **1190.0** | 141.9 | 6.21 |
| B3 | $223 \times 255$ | **1569.9** | 104.2 | 6.45 |
| T2S | $7032 \times 7200$ | **1560.6** | 42.7 | 1.28 |
| T2N | $32208 \times 32400$ | **346.9** | 36.3 | 0.58 *(estimate)* |

B1's row is not evidence for anything: at $5 \times 15$ the call sits near the
clock's resolution, and the M4RI and AFF3CT W2 cells span 41.1% and 33.8%,
respectively; gf2 is 29.0%. That remains too wide for a fine-grained B1
ordering, while the medians provide a large separation. Every other row
separates cleanly on its median.

The timed-route matrix check records a discrepancy, not bit-exact agreement. The
M4RI dump calls the same `genmatrix_rref` helper as the timed route; the
verifier then reverses M4RI columns by $j \mapsto n-1-j$ after RREF to map its
polynomial coordinate order to the repository's row-major `[message | parity]`
layout, while leaving gf2 unchanged. For B2, B3, and T2S, dimensions match and
both raw M4RI and gf2 matrices are systematic, but the normalized matrices have
`bit_exact_identical=False` in the [generator-matrix agreement
receipt](../../bench_results/4e732b56/2026-09-01-4e732b56-generator-matrix-agreement.txt).
The bit discrepancy is retained as a falsification of the originally expected
form equivalence. The same receipt carries the decisive check one level up:
both normalized matrices are full rank with identical canonical GF(2) reduced
row echelon forms under the repository column order (`same_code=True`), so the
two routes materialize generator matrices of the *same code*. The systematic
forms differ because M4RI's RREF pivots on the polynomial-order information
set, which the $j \mapsto n-1-j$ normalization maps to the last $k$
repository coordinates rather than the first.
Reducing the polynomial-form generator matrix to reduced row echelon form beats
encoding the $k$ basis vectors by **8.4× at B2, 15.1× at B3, 36.5× at T2S,
and 9.6× at T2N**. Against the current gf2 path the ratio is **1218× at T2S**.

### 5.3 Counter profile

`perf stat` over the AFF3CT W1 sweep records 167.1 G instructions in 66.1 G
cycles — 2.53 instructions per cycle — with a 0.83% branch-miss rate and 4.6%
of cache references missing. The T2N AFF3CT sweep records 810.3 G instructions
in 280.4 G cycles — 2.89 instructions per cycle — with a 0.70% branch-miss rate
and 2.8% cache misses. The high IPC and low recorded branch/cache miss rates
are consistent with compute-bound execution on this host, but these counters do
not establish that either baseline is not memory-limited; no stall or memory-
bandwidth counters were collected.

## 6. Algorithm-family evidence

This section is what decision D-04 asked the survey to supply: evidence for
which families are worth registering, and where each one wins.

### 6.1 `table-remainder` scales with $k$, not with $\deg g$

bchlib's throughput is nearly flat across generator degrees that span 3×:
1931.9 at $\deg g = 63$, 4018.1 at 168, 4063.8 at 192. It rises with $k$
rather than falling with $\deg g$, because the method consumes 32 message bits
per step through four lookup tables and its per-step cost depends on the parity
word count, not on the tap count.

Both LFSR families do the opposite, spending $O(k \cdot \deg g)$. So the gap
widens exactly where the epic's workloads live — the table method leads the
best LFSR family by **11.9× at $\deg g = 63$, 66.7× at 168, and 74.4× at
192**.

### 6.2 `bitslice-interleaved` pays off at short generators only

AFF3CT's interleaved encoder over its own scalar encoder, at $B = 4096$:

| $\deg g$ | 10 (B1) | 32 (B3) | 63 (B2) | 168 (T2S) | 192 (T2N) |
|---|---|---|---|---|---|
| Interleaved / scalar | 3.77× | **4.55×** | 2.26× | 1.58× | 1.62× |

The advantage peaks near $\deg g = 32$ and is smaller by the DVB-T2 rows. The
plausible mechanism is that AFF3CT's scalar inner loop auto-vectorizes better as
the loop lengthens, but this survey did not inspect the generated code, so the
mechanism is an open question for `avx2-batch-kernels` rather than a finding.
What is established is the shape: **an eight-lane interleaved encoder does not
deliver an eight-fold speedup at DVB-T2 generator degrees.**

### 6.3 `genmatrix-multiply` wins only while $G$ stays small

M4RI's `mzd_mul_m4rm` at $B = 4096$, scaled to information bits so it compares
directly with the W1 column: 2402.3 (B2), 2823.1 (B3), 402.6 (T2S), 82.0 (T2N).

It is the **fastest family measured at B3** — 7.4× the best LFSR there, and the
only competitive option for a row bchlib cannot encode — and it edges bchlib at
B2 (2402.3 against 1931.9). It then collapses by a factor of 50 relative to
bchlib at T2N, where $G$ is 124 MiB and the product's work grows with $n \cdot k$
while the LFSR's grows with $k \cdot \deg g$. The selected W2 route's matrix
comparison records matching dimensions, row-space equality (`same_code=True`,
so the routes produce generator matrices of the same code), and a bit
discrepancy of the systematic forms after the documented post-RREF
normalization; the throughput comparison is between routes materializing the
same code, not a claim of identical output bits.

### 6.4 The scalar reference

`poly-remainder-scalar` is required by D-04 as the reference every other family
is checked against, not as a performance candidate. Two independent
implementations of it bracket the current gf2 encoder: AFF3CT's, at 38–100
Mbit/s, and IT++'s field-element form at 0.39–6.22 Mbit/s. gf2's current
encoder sits with IT++, which is the expected place given both compute over
$\mathrm{GF}(2^m)$ elements — and confirms the slowness is the algorithm and
representation, not a defect peculiar to gf2.

## 7. Selection

| Workload | Role | Selection | Margin over the next candidate |
|---|---|---|---|
| W1 | Primary | **bchlib v2.1.3**, `table-remainder` | 66.7–74.4× at the DVB-T2 rows |
| W1 | Secondary | **AFF3CT v4.7.0** `Encoder_BCH_inter` | strongest family bchlib cannot express |
| W2 | Primary | **M4RI 20260122**, `mzd_echelonize_m4ri` route | 9.6–36.5× over basis-vector encoding |
| W2 | Secondary | **AFF3CT v4.7.0** `Encoder_BCH`, basis-encode | like-for-like path for the current gf2 route |

**Why bchlib is primary for W1 rather than AFF3CT.** It is faster by two orders
of magnitude at the rows that matter, and it earns that on the representation
gf2 already uses — packed bytes, not one machine word per bit. Choosing AFF3CT
as the target would set a bar gf2 could clear without adopting the method that
actually matters.

**Why AFF3CT is retained as W1 secondary.** bchlib cannot encode a row whose
$k$ is not a whole number of bytes, which excludes B1 and B3. AFF3CT covers
every row, supplies the required scalar reference, and is the only measured
implementation of the interleaved family. Two baselines also keep a single
library's quirks from defining the target.

**Why M4RI is primary for W2.** It is faster at every row above B1, by 9.6–36.5×
over the basis-vector route, and the timed-route correctness check witnesses
that its route materializes a generator matrix of the same code
(`same_code=True` by canonical-RREF row-space equality for B2, B3, and T2S).
The same check found a real bit discrepancy of the systematic forms after the
documented normalization, so the selection asserts same-code speed, not
bit-identical output. This reverses the ordering the contract predeclared;
§ 8.1 records that.

**Like-for-like justification.** The W1 comparison at the DVB-T2 rows is
between implementations of the *same code*, not merely codes of equal shape.
`baseline-survey/verify-generators.py` recomputes the generator polynomial from
the minimal-polynomial table at
`crates/gf2-coding/src/bch/dvb_t2/generators.rs:14-46` and compares it against
what the AFF3CT harness emits; both DVB-T2 rows agree coefficient by
coefficient at degrees 168 and 192. The checker is invoked against the same
`generators.txt` the harnesses consume:
`2026-09-01-4e732b56-small-generator-agreement.txt`. The W2 comparison is likewise same-code: the [generator-matrix
receipt](../../bench_results/4e732b56/2026-09-01-4e732b56-generator-matrix-agreement.txt)
witnesses full rank and identical canonical RREFs (`same_code=True`) for the
M4RI and gf2 routes at B2, B3, and T2S, while recording the systematic-form
bit discrepancy rather than treating it as agreement.
The B1/B2/B3 rows and the
IT++ rows are equal-shape comparisons only, because IT++ selects its own
primitive polynomial.

## 8. Falsification record and open questions

### 8.1 The predeclared W2 baseline was wrong

Before measuring, the selection section of the workload-selection contract
named **AFF3CT's
basis-vector materialization** the primary W2 baseline and M4RI the secondary,
reasoning that the same-semantics route was the fairer target. The measurement
contradicts that: the M4RI route is faster at every row above B1, by 36.5× at
T2S. The contract has been amended to make M4RI primary and AFF3CT secondary,
and the original ordering is recorded here rather than quietly replaced.

The substantive lesson for `genmatrix-perf` is larger than the swap: **building
$G$ by encoding $k$ basis vectors is the wrong algorithm.** A structured fill
followed by four-Russians elimination is over an order of magnitude better, and
the current gf2 implementation
(`crates/gf2-coding/src/bch/core.rs:270-294`) uses the basis-vector route.

### 8.2 The host governor was `powersave`

The run's host record shows every CPU under the `powersave` scaling governor,
not `performance`. This is a deviation from the ideal benchmark posture and is
recorded rather than corrected mid-survey. Across the 126 measured cells, 94
spreads exceed 5% and 84 exceed 10%; the selection-critical W1 and W2 spreads
are quoted in § 5. The
absolute figures may still shift under a `performance` governor, so the
`perf-receipts` task should re-establish its non-regression baseline on the
governor it intends to keep rather than inheriting these numbers as absolutes.
The *ratios* between implementations, which is what the selection rests on, are
far too large to be explained by governor effects.

### 8.3 `encode_batch` is a sequential map

gf2's `encode_batch` and a plain loop over `encode` agree within noise at every
one of the 19 measured cells. This is not a surprise —
`crates/gf2-coding/src/bch/core.rs:396` is a `messages.iter().map(...)` under a
`TODO` — but it fixes the pre-cutover baseline: there is no batch-specific
overhead for the epic to preserve, and no existing parallelism to regress.

### 8.4 Open questions

* **The interleaved family's decay** (§ 6.2) is characterized but not
  explained. `avx2-batch-kernels` should confirm from generated code whether
  the scalar path is auto-vectorizing before assuming an interleaved kernel
  will scale with register width at DVB-T2 generator degrees.
* **No DRAM-bound cell exists** in the fixed batch ladder; the largest working
  set is 31.5 MiB against a 32 MiB L3. A consumer that needs a memory-bound
  measurement must amend the contract's ladder.
* **Two gf2 cells are estimates.** T2N at $B = 4096$ and T2N's generator matrix
  were projected from probe-measured per-unit costs of 56.2 ms per frame and
  1809.8 s per materialization. They remain estimates under the fixed cell
  budgets.
* **bchlib's byte alignment** excludes B1 and B3 from the primary W1 baseline.
  Those two rows are compared against AFF3CT and M4RI only.

## 9. Review-resolution record

| Finding | Resolution | Evidence at HEAD |
|---|---|---|
| R1-F1 invocation/revision provenance | Both survey runs use native per-stage headers and record the fresh gf2 revision. | `dev/bench_results/4e732b56/2026-09-01-4e732b56-survey-receipt.md` § Per-stage provenance |
| R1-F2 matrix-equality check | The check is retained; the dump shares the timed helper and the normalized comparison records a discrepancy. | `dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench.c`; `dev/bench_results/4e732b56/2026-09-01-4e732b56-generator-matrix-agreement.txt` |
| R1-F3 5 ms repetition rule | New calibrated logs retain repetition counts; the audit found no measured cell without calibrated repetitions. | `dev/bench_results/4e732b56/2026-09-01-4e732b56-*.log`; `dev/active/4e732b56/findings.md` § 4 |
| R1-F4 kodo retrieval evidence | The retrieval failure and scope evidence remain preserved. | `dev/bench_results/4e732b56/2026-08-31-4e732b56-kodo-retrieval.txt` |
| R1-F5 M4RI thread evidence | The no-OpenMP configuration and dynamic-link evidence remain preserved. | `dev/bench_results/4e732b56/2026-08-31-4e732b56-m4ri-thread-evidence.txt` |
| R1-F6 nice posture | The denied nice request is recorded in the protocol and run host records. | `dev/active/4e732b56/findings.md` § 4; `dev/bench_results/4e732b56/2026-09-01-4e732b56-small-host.txt` |
| R1-F7 dispersion prose | Exact dispersion counts and selection-critical spreads match the new receipt. | `dev/active/4e732b56/findings.md` § 5; `dev/bench_results/4e732b56/2026-09-01-4e732b56-survey-receipt.md` |
| R1-F8 counter-claim wording | The selection and falsification prose preserve the speed result without claiming matrix equivalence. | `dev/active/4e732b56/findings.md` §§ 5.2, 6.3, 8.1 |
| R1-F9 seeded determinism | The seeded AFF3CT rerun has 98 W1 and 14 W2 comparisons with zero digest mismatches against the new CSVs. | `dev/bench_results/4e732b56/2026-09-01-4e732b56-determinism-agreement.txt` |
| R2-F1 provenance-complete remeasurement | Both complete runner reruns finished before 01:15 EEST; all measurement rows use log-header provenance and the fresh gf2 revision. | `dev/bench_results/4e732b56/2026-09-01-4e732b56-survey-receipt.md` § Per-stage provenance |
| R2-F2 timed-route equality check | The timed and dump routes share `genmatrix_rref`; verifier-owned post-RREF reversal is documented, and the B2/B3/T2S discrepancy is preserved verbatim. | `dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench.c`; `dev/active/4e732b56/baseline-survey/verify-generator-matrices.py`; `dev/bench_results/4e732b56/2026-09-01-4e732b56-generator-matrix-agreement.txt` |

## 10. Citations

Every work this survey cites resolves in `.jit/references.toml`: [Cassagne2019]
(AFF3CT), [AlbrechtBard2026] (M4RI), [Djelic2011] (the kernel BCH library),
[ITPP2013], [LiquidDSP2025], [Etsi2015] (EN 302 755 V1.4.1, the edition
`crates/gf2-coding/src/ldpc/core.rs:213` pins). The registry holds the
bibliographic data; this document does not restate it. Kodo's scope is
registered as [KodoSteinwurf2026].
