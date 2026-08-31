# External-baseline survey for BCH encoding and generator-matrix materialization

| Field | Value |
|---|---|
| JIT issue | `4e732b56` (Reproducible external-baseline survey and workload selection) |
| Epic | `ae03bcd0` (Harden and generalize BCH codes over finite fields) |
| Contract produced | [`workload-selection`](workload-selection.md) |
| Criteria | REQ-13, REQ-14 of the epic; D-04 (encoding families), D-07 (evidence protocol) |
| Harness | `dev/active/4e732b56/baseline-survey/` |
| Receipts | `dev/bench_results/4e732b56/` |
| Status | *(filled at § 5)* |

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

The [SOTA target matrix](/dev/plans/sota_target_matrix.md) is the repository's reference-selection
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
(§ 5.1–§ 5.5). Workload W2's output is a dense GF(2) matrix, so M4RI is the
natural secondary reference for it, at the pin the matrix already carries.

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
not a measurement artifact, and § 5 reports it rather than normalizing it away.

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

`https://github.com/steinwurf/kodo-rlnc` returns HTTP 404 to an unauthenticated
request, and an anonymous `git clone` fails asking for credentials. No version
of it could be pinned, built, or measured. Independently of retrievability, the
kodo family implements random linear network coding over erasure channels
rather than BCH encoding, so it has no comparable API for either workload.

## 4. Measurement protocol

**Host and serialization.** All measurements run through
`dev/scripts/ccx1-bench-flock.sh`, which holds the `/tmp/gf2-ccx1.lock` mutex
exclusively for the child and pins it to the CCX1 cores (`taskset -c 6-11`,
`nice -n -5`). Host model, microarchitecture, cache topology, frequency
governor, OS, and compiler versions are captured by `run-survey.sh` into the
`host.txt` of each run. The survey did not begin measuring until the shared
benchmark host was released.

**Statistics.** Each cell takes up to 7 independent trials. A trial repeats the
measured call enough times to span at least 5 ms, so a cell whose single call
sits near the clock's resolution is still resolved, and the reported figure is
always per one call. Results are reported as the median over trials with the
observed minimum and maximum, and the spread as a percentage of the median.
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
different *results*.

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

*(filled from the committed receipts)*

## 6. Algorithm-family evidence

*(filled from the committed receipts)*

## 7. Selection

*(filled from the committed receipts)*

## 8. Falsification record and open questions

*(filled from the committed receipts)*

## 9. Citations

Every work this survey cites resolves in `.jit/references.toml`: [Cassagne2019]
(AFF3CT), [AlbrechtBard2026] (M4RI), [Djelic2011] (the kernel BCH library),
[ITPP2013], [LiquidDSP2025], and [Etsi2015] (EN 302 755 V1.4.1, the edition
`crates/gf2-coding/src/ldpc/core.rs:213` pins). The registry holds the
bibliographic data; this document does not restate it.
