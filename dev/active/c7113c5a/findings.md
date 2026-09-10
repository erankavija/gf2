# Equivalent polynomial multiplication baselines

> **Diátaxis Type:** Explanation

Survey findings for jit issue `c7113c5a` under epic `1a379447`. Every number
below traces to a committed artifact named beside it. No production code
changes; the survey measures the current code and reports what it selects.

## Question

Raw independent carry-less products, reduced field dot products and long binary
polynomial multiplication are three different operations, and a comparison that
mixes them is not a comparison. This survey asks three things:

1. Which gf2 consumers perform an operation that gf2x [GfTwoX2026] also
   performs, once field reduction is separated from the unreduced product?
2. For each such operation, what does the current gf2 code actually run on this
   Ryzen 9 5900X, and how far is it from gf2x measured under the frozen
   protocol?
3. Where a gap exists, what accounts for it?

Adoption is out of scope by construction: the issue excludes a gf2x production
dependency and any change to a gf2 multiplication algorithm, so every external
comparison is a `comparator-gap` cell whose outcome is an attribution task, not
a selection.

## Methodology

Measurements follow the frozen protocol `zen3-benchmark-protocol` version 1
(`dev/active/f547c394/protocol.md`) and the epic's measurement contract
(`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md`). Both are
pinned by content digest inside each receipt, along with the family addendum.

### Arms

`dev/active/c7113c5a/survey/gf2-side/` is a standalone cargo project outside the
gf2 workspace. It consumes `gf2-core` and `gf2-kernels-simd` by path as an
external user would and reuses `tuning-campaign-support` for the canonical
child-v2 framing and the window timing, so the survey adds no parallel
measurement mechanism. Both arms of every cell share the fixture generation,
bank rotation, worker fan-out and conversion accounting in
`survey/gf2-side/src/lib.rs` and differ only in the `Backend` that performs the
arithmetic, so a measured ratio attributes to the kernel rather than to the
harness.

The gf2x binding lives in the two binaries that need it rather than in the
shared library, so the gf2 arm links nothing external and carries an independent
executable digest. Each gf2x-bearing binary holds an rpath to its pinned prefix
with `--disable-new-dtags`, and asserts at start-up, by reading
`/proc/self/maps`, that the object it mapped is that prefix. The assertion is
load-bearing rather than defensive: this host also carries a system
`/usr/lib/libgf2x.so.3`, which an unqualified link resolves to and which carries
none of the pinned build's provenance. Both the validation report and every
receipt record the library path each arm actually mapped.

### Correctness before timing

`survey/run-validation.sh` runs `poly-validate` once per build variant and
writes `survey/validation.json`. `survey/run-baseline.sh` refuses to start a
timed run unless that file records a pass. The report pins the digest of every
executable it checked; its `native` entries equal the `gf2-native` and
`gf2x-native` arm digests in the confirmatory receipt, so the binaries that were
validated are the binaries that were measured.

### Behavioural identity of the arm crate across receipts

The arm crate carries a separated field-reduction probe, two type aliases and
the two largest validated operand lengths that the pilot's executables do not,
so the pilot's arm executables carry different digests from the ones the
confirmation uses. Those revisions are confined to `Backend::conversion`, which
runs outside every timed window, and to the validator; `Backend::run`, the
fixture generation, the bank rotation and the window protocol are the same
apart from line breaks. The pilot therefore measures the same arithmetic the
confirmation does, which is what
[`@/inv/behavioral-evidence-validity`](../../../AGENTS.md) makes the test of
whether committed evidence stays valid, and the pilot's role is to supply a
measurement-resolution figure rather than a performance claim.

## External baseline

| Fact | Value |
|---|---|
| Project | gf2x [GfTwoX2026] |
| Source | `https://gitlab.inria.fr/gf2x/gf2x.git` |
| Tag | `gf2x-1.3.0` |
| Commit | `27ba588f03bf6e1e74763903bab25e6e8bb6d0f0` |
| License | GPL-3.0-or-later |
| Measured entry point | `gf2x_mul_r` |

The license is the GPL branch rather than the LGPL branch: gf2x's `README` makes
the choice conditional on whether `toom-gpl.c` is a real file or a placeholder,
and this archive's copy is 61146 bytes against the placeholder's 1384.
`survey/fetch-build.sh` refuses to build a tree whose `toom-gpl.c` is under
10000 bytes, and `survey/record-build-evidence.sh` records the exact size, so
the licence is established from the archive rather than assumed.

### Selected basecase and accelerated instructions

`configure` resolves `hwdir=x86_64_pclmul` in all three variants and links the
`already_tuned/x86_64_pclmul/` copies of `gf2x-thresholds.h` and `gf2x_mul1.h`
through `gf2x_mul9.h`. The selection is not inferred from build flags: it is the
`config.status` link record in each variant's `configure.stdout.log`, extracted
into `survey/gf2x-build-evidence.txt` by `survey/record-build-evidence.sh`.

`configure` appends `-msse2 -msse3 -mssse3 -msse4.1 -mpclmul` to `CFLAGS`
whenever the compiler accepts them (`config/acinclude.m4`), so gf2x compiles its
PCLMUL basecases even in the variant built with a bare `-O2`. Disassembly of each
installed library confirms it: the conservative variant emits 100
legacy-encoded `pclmul*` instructions and the tuned and native variants emit 109
VEX-encoded `vpclmul*` instructions.

Every carry-less multiply gf2x emits on this host takes `xmm` operands. The
count of `pclmul` instructions with a `ymm` operand is zero in all three
variants, so gf2x 1.3.0 uses 128-bit PCLMULQDQ only and never the 256-bit
VPCLMULQDQ form, on a CPU that reports the feature and even at `-march=native`.
The symbols carrying the instruction are `gf2x_mul_basecase_inner`,
`gf2x_addmul_1_n`, `gf2x_mul4`, `gf2x_mul_tc4`, `gf2x_mul_tc3` and `gf2x_mul9`.

The same evidence file records the algorithm thresholds the installed
`gf2x-thresholds.h` sets, identically in all three variants and in 64-bit words:
Karatsuba at 10, Toom-W at 21, unbalanced Toom at 51, Toom-4 at 346, unbalanced
Toom unconditionally at 1880, Toom-4 unconditionally at 5683, three-way Toom at
9948, and an FFT table whose smallest tabulated size beyond its leading entry is
4344. The measured cell sizes 4, 9, 64, 256 and 2048 words therefore cross
gf2x's basecase, Karatsuba, Toom-W, unbalanced-Toom and Toom-4 regimes and reach
no FFT.

### Build adaptation

The stock `CC_FOR_BUILD` candidate `c89` cannot compile
`lowlevel/gen_bb_mul_code.c` under this host's GCC, which rejects that file's
C++-style comments in C90; `configure`'s own build-compiler probe also fails
because it calls `exit` without declaring it. All three variants therefore
configure with `CC_FOR_BUILD="gcc -std=gnu99 -Wno-implicit-function-declaration"`.
That generator emits the bit-by-bit fallback basecases used when PCLMUL is
absent, so it contributes no code to any measured path.

## Operation mapping

gf2x offers one operation: `gf2x_mul_r(c, a, an, b, bn, pool)`, the unreduced
product in $\mathrm{GF}(2)[x]$ of a polynomial spanning `an` words by one
spanning `bn` words, written as `an + bn` words. Its word and bit layout is the
canonical little-endian numbering gf2 uses, so bit $i$ of word $j$ is the
coefficient of $x^{64j + i}$ on both sides and there is no representation
conversion to charge either arm.

Three gf2 consumers map onto it, and only the first maps onto it as a single
equivalent call.

| gf2 operation | gf2x form | Equivalence | Reduction |
|---|---|---|---|
| `clmul_wide_slice::<N>`, and the dispatched wide kernels behind `Gf2mWide::mul_ref` | one `gf2x_mul_r` on `N`-word operands | equivalent external arm | none; both sides unreduced |
| `clmul_batch` | `n` independent `gf2x_mul_r` calls on one-word operands | composed reference, not equivalent | none; both sides unreduced |
| `FieldVec::<Gf2mElement>::simd_dot_product` | `n` one-word `gf2x_mul_r` calls, XOR-accumulated | composed reference, not equivalent | gf2's `BarrettReducer` on both arms |

The long product is the genuinely equivalent case: `clmul_wide_slice::<N>` and
`gf2x_mul_r` compute the same `2N`-word polynomial from the same `N`-word
operands in one call each. The kernels `gf2_kernels_simd::gf2m_wide` dispatches
for `N = 4` and `N = 9` compute that same product, so the `GF(2^256)` and
`GF(2^571)` field consumers have an external equivalent for their unreduced
stage, with the Barrett reduction that follows it excluded from the cell.

### The raw independent-product batches are an internal baseline

gf2x exposes no batch entry point. The only way to reach the operation
`clmul_batch` performs is a loop of `n` calls to the public dispatching entry
point, which is what `Gf2xBackend::run` does for both the batch case and the
dot-product case. That composition is not an equivalent external arm, and the
receipt's own numbers say why: a one-word product through `gf2x_mul_r` costs
7.128 ns and a four-word product costs 13.917 ns, so quadrupling the operand
length costs 1.95 times the time. A fixed per-call cost dominates both figures,
and a cell built from `n` such calls measures that cost rather than gf2x's
basecase kernel.

REQ-04 therefore holds in its default branch: no genuinely equivalent external
arm is verified for the raw independent-product batches, and
`clmul-batch-1024-1core` and `gf2m-dot-1024-1core` are retained as **internal
baselines of the current gf2 code**, separately labelled from the long-product
cells. Their gf2x column is a composed reference that bounds what a consumer
reaching gf2x through its public API would pay, not a measurement of gf2x's
kernel and not evidence that gf2 beats gf2x at a 64-by-64 product. A comparison
that reads them as an equivalent gf2x arm overstates the gf2 result.

The dot-product cell is the only whole-consumer cell. gf2 computes it as `n`
unreduced products, one XOR accumulation and one Barrett reduction; the gf2x arm
composes `n` `gf2x_mul_r` calls with the identical gf2 `BarrettReducer`, because
gf2x supplies no field reduction. Both arms therefore reduce with the same code
and the cell isolates the product-and-accumulate stage. Setup, packing, batch
fill, dispatch and unpacking are timed outside the measured windows by both arms
and reported per execution in the receipt.

### What has no equivalent

gf2's `BarrettReducer` and `BarrettReducerWide` have no counterpart in gf2x, so
no reduction cell exists and none is fabricated. gf2x's `gf2x_mul_toom` and its
FFT interface are reachable only below `gf2x_mul_r`, which selects among them by
size; the survey measures the documented top-level entry point rather than
pinning an internal routine the library chooses for itself.

## Correctness evidence

`survey/gf2-side/src/bin/poly-validate.rs` checks both libraries against a
canonical definition of multiplication in $\mathrm{GF}(2)[x]$: the XOR of $b$
shifted left by every set bit position of $a$. That reference uses no carry-less
multiply instruction, no gf2 kernel and no gf2x routine, so agreement with it is
evidence rather than a restatement.

`survey/validation.json` records the outcome for each of the three build
variants: 8930 checks each, no failures, each against its own pinned gf2x
library. The five checks are

- **bit order and coefficients**: single-term operands $x^k$ and $x^l$ for
  $k, l$ in 0, 1, 62, 63, 64, 65, 126, 127, 128, 191 and 255, whose product must
  be exactly $x^{k + l}$ and nothing else. These are the 0, 1, 63, 64 and 65
  boundary cases the engineering contract names, taken at bit rather than word
  granularity, so a shift that loses a coefficient across a word boundary fails.
- **lengths and complete outputs**: random operands at 1, 2, 3, 4, 5, 8, 9, 16,
  63, 64, 65, 127, 128, 256, 1024 and 2048 words, with every one of the $2N$
  product words compared, and a final trial per length whose operands have a
  zero top word so that a truncated write into genuinely zero product words is
  caught. Every operand length any cell measures appears in that list, so no
  size is timed before it is checked.
- **dispatched wide kernels**: the four-limb and nine-limb kernels
  `gf2_kernels_simd::gf2m_wide` resolves, against the canonical reference and
  against gf2x.
- **independent 64-by-64 products**: every one of 1024 products per trial,
  checked individually.
- **whole-consumer dot product**: gf2's `simd_dot_product` and the composed gf2x
  arm against an independently reduced canonical dot product, where the
  reference reduces by long division rather than by the Barrett path either arm
  uses.

The paths the checks exercised are recorded in the same file. On this host they
are `wide256:avx2+vpclmulqdq-ymm`, `wide571:avx2+vpclmulqdq-ymm` and
`clmul_batch:sequential-pclmulqdq (vpclmulqdq=1 avx512vl=0)`.

### The batch dispatch predicate this host selects

`clmul_batch` takes its 256-bit VPCLMULQDQ body only when both `vpclmulqdq` and
`avx512vl` are detected (`crates/gf2-kernels-simd/src/x86/clmul.rs:104`). This
Zen 3 host reports `vpclmulqdq` and does not report `avx512vl`, which the
receipt's own host observation records in its CPU flags, so the current code
takes the sequential PCLMULQDQ branch. The arm re-evaluates those two detections
and reports the branch it observed rather than inferring it, and every measured
`clmul_batch` figure in this survey is that sequential branch. Repairing the
predicate belongs to a separate issue and is not attempted here.

## Confirmatory results

The receipt is
`dev/bench_results/c7113c5a/2026-09-07-c7113c5a-polynomial-confirmation/`,
campaign `confirmation-c7113c5a-20260907t194909z`, receipt digest
`63eb171e787ec773e934ef1bcf205f4f2f68e88dcba77c84fcabf3530018277e`. It ran on
`fraktaali`, an AMD Ryzen 9 5900X on Linux 7.2.2-arch1-1 under
`dev/scripts/ccx1-bench-flock.sh --full-host`, in two resumed sessions, and
terminated `complete`. The receipt's `host` field describes the second of those
sessions only: `benchmark-ab-runner.rs:215` binds hostname and CPU ids across a
resume and finalization keeps the last observation, so the host description
covering the five session-1 cells lives in `execution.log` rather than in the
receipt field the protocol checks. That is a limitation of this evidence and is
stated in full below; no cell is re-run to avoid it.
`benchmark-acceptance` records verdict **accepted**,
qualifies for production selection **false**, and no error or warning finding.
`qualifies` requires every non-exploratory cell to pass, so a survey that adopts
nothing and declares every cell a `comparator-gap` cannot qualify and is not
meant to: the flag reports that no arm here is a production candidate, not that
the receipt is deficient.
Ten confirmatory cells, 24 pairs and 240 windows each, family-wise alpha 0.05
over 10 comparisons, so each interval is a percentile bootstrap at confidence
0.995 over 10000 resamples.

The host was not idle. `launcher.log` records a one-minute load average of 29.67
when the launcher started, and the receipt's own host observations record 8.27
at the first session's start and 7.91 at the second session's, so processes that
do not respect the CCX1 mutex ran alongside the campaign. The operative
stability evidence is therefore the per-arm window dispersion rather than the
load figure: no window in the receipt exceeds 1.21 times its own arm's median,
which the flagged-window section tabulates cell by cell.

### Reading the direction of a cell

The estimator is `median(baseline) / median(candidate)`, and every cell names
gf2 as baseline and gf2x as candidate. A value **above one means gf2x is that
many times faster**; a value below one means gf2 is. The acceptance vocabulary
follows the same convention, so in this family `improved` means gf2x won the
cell and `fail` means gf2x lost it. Four cells that gf2 wins therefore carry the
outcome `fail`.

### Cells

Projected from the receipt by `survey/summarize-receipt.py --table cells`:

| Cell | gf2 median | comparator median | speedup of medians | interval | pairs | decision | outcome | gf2 path |
|---|---:|---:|---:|---|---:|---|---|---|
| `poly-mul-4w-1core` | 8.48 ns | 13.9 ns | 0.6085 | [0.604, 0.6151] at 0.995 | 24 | regressed | fail | `wide256:avx2+vpclmulqdq-ymm` |
| `poly-mul-9w-1core` | 26.4 ns | 39.8 ns | 0.6625 | [0.6507, 0.6677] at 0.995 | 24 | regressed | fail | `wide571:avx2+vpclmulqdq-ymm` |
| `poly-mul-64w-1core` | 154 us | 919 ns | 167.4 | [165.6, 169.1] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-1core` | 2.69 ms | 9 us | 299.4 | [294, 303] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `poly-mul-2048w-streaming-1core` | 180 ms | 210 us | 855.2 | [847.6, 865.3] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-6core` | 146 ms | 806 us | 180.4 | [168.5, 188.8] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-12core` | 137 ms | 806 us | 172.8 | [161.5, 182.8] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-24smt` | 186 ms | 1.11 ms | 167 | [163.6, 171.1] at 0.995 | 24 | improved | unstable | `clmul_wide_slice:schoolbook` |
| `clmul-batch-1024-1core` | 1.09 us | 7.3 us | 0.1491 | [0.1483, 0.1503] at 0.995 | 24 | regressed | fail | `clmul_batch:sequential-pclmulqdq (vpclmulqdq=1 avx512vl=0)` |
| `gf2m-dot-1024-1core` | 1.2 us | 7.81 us | 0.1536 | [0.1514, 0.1559] at 0.995 | 24 | regressed | fail | `FieldVec::simd_dot_product` |

The first two rows and the last two are the cells gf2 wins; the middle six are
the cells gf2x wins. Both directions are stated here because both are measured.

### Where gf2 wins, and by how much

Reciprocals of the same intervals, so the number is how many times faster gf2 is:

| Cell | gf2 faster by | interval at 0.995 | equivalent arm? |
|---|---:|---|---|
| `poly-mul-4w-1core` | 1.643 | [1.626, 1.656] | yes: one `gf2x_mul_r` call |
| `poly-mul-9w-1core` | 1.510 | [1.498, 1.537] | yes: one `gf2x_mul_r` call |
| `clmul-batch-1024-1core` | 6.708 | [6.653, 6.742] | no: 1024 composed calls |
| `gf2m-dot-1024-1core` | 6.509 | [6.415, 6.603] | no: 1024 composed calls |

The two long-product cells are equivalent-arm results: at four and nine words
the dispatched `avx2+vpclmulqdq-ymm` kernels behind `Gf2mWide::mul_ref` beat gf2x's
`gf2x_mul4` and `gf2x_mul9` basecases outright, on operands below gf2x's
Karatsuba threshold of 10 words where gf2x runs its own tuned basecase and
nothing else. The two internal-baseline cells are not: gf2x's per-call cost
inflates their factor of about 6.6, so that figure is an upper bound on any
kernel-level advantage rather than a measurement of one, as the mapping section
states.

### Where gf2x wins, and by how much

Every long-product cell at 64 words and above. gf2x is 167.4 times faster at 64
words, 299.4 at 256 words and 855.2 at 2048 words, one core each, and 180.4,
172.8 and 167.0 at 256 words on six physical cores, twelve physical cores and
twenty-four logical CPUs. Every interval is given in the cells table and none
contains 1.

The issue's premise is that a gf2x survey supplies honest polynomial
comparisons, and it does. **The comparison it supplies contradicts the working
assumption that gf2's carry-less paths are the accelerated ones:** beyond nine
words gf2 has no accelerated long product at all, its cost per coefficient
product is that of a scalar bit-by-bit loop, and the measured gap grows with
operand length instead of staying bounded. The result is recorded here in those
words rather than aggregated away.

### One scale for every cell

Projected by `survey/summarize-receipt.py --table products`, which divides each
arm's measured call by the number of one-word coefficient products the
schoolbook definition of that cell's operation performs. For gf2x that column is
a normalised rate on the schoolbook scale, because gf2x avoids most of those
products by recursion; for gf2 at 64 words and above it is the true cost of one
scalar carry-less product.

| Cell | operand words | workers | schoolbook products per call | gf2 ns per product | comparator ns per product |
|---|---:|---:|---:|---:|---:|
| `poly-mul-4w-1core` | 4 | 1 | 16 | 0.5299 | 0.8698 |
| `poly-mul-9w-1core` | 9 | 1 | 81 | 0.3257 | 0.4919 |
| `poly-mul-64w-1core` | 64 | 1 | 4096 | 37.54 | 0.2244 |
| `poly-mul-256w-1core` | 256 | 1 | 65536 | 40.98 | 0.1374 |
| `poly-mul-2048w-streaming-1core` | 2048 | 1 | 4194304 | 42.98 | 0.0501 |
| `poly-mul-256w-6core` | 256 | 6 | 12582912 | 11.56 | 0.06409 |
| `poly-mul-256w-12core` | 256 | 12 | 25165824 | 5.459 | 0.03203 |
| `poly-mul-256w-24smt` | 256 | 24 | 50331648 | 3.699 | 0.02215 |
| `clmul-batch-1024-1core` | 1 | 1 | 1024 | 1.065 | 7.128 |
| `gf2m-dot-1024-1core` | 1 | 1 | 1024 | 1.175 | 7.631 |

Two facts follow directly. gf2's cost per coefficient product is flat at 37.5 to
43.0 ns across 64, 256 and 2048 words, which is the signature of an $O(N^2)$
schoolbook whose inner operation does not change with size. gf2x's falls from
0.2244 to 0.0501 ns over the same range, which is the signature of subquadratic
recursion. The kernel-backed rows at four and nine words show what gf2's own
hardware path costs when it is reached: 0.53 and 0.33 ns per coefficient
product, which is 77 and 126 times cheaper than the 40.98 ns the same crate
spends per coefficient product once it falls back to the scalar loop.

### Attributing the long-product gap

The 299-fold gap at 256 words decomposes into an instruction factor and an
algorithmic factor, both derived from cells of this receipt. These two quotients
are estimates computed from measured medians, not separately measured
quantities.

| Quantity | Value | Source |
|---|---:|---|
| gf2 hardware carry-less product, including the `u128` unpack | 1.0647 ns | `clmul-batch-1024-1core` gf2 median over 1024 products |
| gf2 scalar carry-less product inside the schoolbook | 40.9843 ns | `poly-mul-256w-1core` gf2 median over $256^2$ products |
| Instruction factor | 38.49 | quotient of the two rows above |
| Measured gap at 256 words | 299.38 | `poly-mul-256w-1core` interval estimate |
| Algorithmic residue | 7.78 | measured gap divided by the instruction factor |

So roughly 38 of the 299 comes from using a scalar bit-by-bit loop where the
hardware instruction is available, and the remaining factor of about 7.8 comes
from gf2x's subquadratic algorithms against gf2's $O(N^2)$ schoolbook. The
algorithmic residue grows with $N$ and the instruction factor does not: divided
by the same 38.49, the 855.2 measured at 2048 words leaves a residue of 22.2.

### Multicore behaviour

One call of a multicore cell is `inner` products on each of `workers` threads
plus two barrier round trips, so its per-product column is aggregate throughput,
not latency. Relative to the one-core 256-word cell, gf2's per-product cost
falls by 3.54, 7.51 and 11.08 times at six, twelve and twenty-four workers, and
gf2x's by 2.14, 4.29 and 6.20. gf2 scales the better of the two, which is why
the gap narrows from 299.4 on one core to 167.0 on twenty-four logical CPUs.
These ratios compare cells that differ in `inner` as well as in worker count and
are not paired observations, so they are descriptive rather than an interval
estimate. The arms also differ in what they occupy, which the receipt's own
topology observation resolves at run time: the six-worker arm takes six distinct
physical cores inside a single L3 domain, the twelve-worker arm twelve distinct
cores across both L3 domains, and the twenty-four-worker arm both SMT siblings of
those same twelve cores, so its per-product figure divides by 24 threads on 12
cores. What the reduced gap does not mean is any change in the single-core
conclusion.

### Adapter, conversion and reduction costs

Both arms report these outside their timed windows, with the field meanings
`survey/gf2-side/src/lib.rs` documents on `Conversion`. Projected by
`survey/summarize-receipt.py --table conversion`:

| Cell | arm | setup | pack | batch fill | dispatch | unpack |
|---|---|---:|---:|---:|---:|---:|
| `poly-mul-4w-1core` | `gf2-native` | 30 ns | 0 ns | 0 ns | 90 ns | 0 ns |
| `poly-mul-4w-1core` | `gf2x-native` | 825 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-9w-1core` | `gf2-native` | 40 ns | 0 ns | 0 ns | 90 ns | 0 ns |
| `poly-mul-9w-1core` | `gf2x-native` | 955 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-64w-1core` | `gf2-native` | 50 ns | 0 ns | 0 ns | 90 ns | 0 ns |
| `poly-mul-64w-1core` | `gf2x-native` | 835 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-256w-1core` | `gf2-native` | 60 ns | 0 ns | 0 ns | 90 ns | 0 ns |
| `poly-mul-256w-1core` | `gf2x-native` | 840 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-2048w-streaming-1core` | `gf2-native` | 290 ns | 0 ns | 0 ns | 90 ns | 0 ns |
| `poly-mul-2048w-streaming-1core` | `gf2x-native` | 810 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-256w-6core` | `gf2-native` | 80 ns | 0 ns | 0 ns | 110 ns | 0 ns |
| `poly-mul-256w-6core` | `gf2x-native` | 775 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-256w-12core` | `gf2-native` | 90 ns | 0 ns | 0 ns | 110 ns | 0 ns |
| `poly-mul-256w-12core` | `gf2x-native` | 730 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `poly-mul-256w-24smt` | `gf2-native` | 85 ns | 0 ns | 0 ns | 110 ns | 0 ns |
| `poly-mul-256w-24smt` | `gf2x-native` | 690 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `clmul-batch-1024-1core` | `gf2-native` | 20 ns | 0 ns | 0 ns | 90 ns | 700 ns |
| `clmul-batch-1024-1core` | `gf2x-native` | 830 ns | 0 ns | 0 ns | 0 ns | 0 ns |
| `gf2m-dot-1024-1core` | `gf2-native` | 90 ns | 15495 ns | 3355 ns | 90 ns | 0 ns |
| `gf2m-dot-1024-1core` | `gf2x-native` | 780 ns | 2995 ns | 6760 ns | 0 ns | 1 ns |

gf2's runtime capability detection costs 90 to 110 nanoseconds once per process
and gf2x's scratch-pool initialisation 690 to 955 nanoseconds once per worker,
so neither is visible in a per-call figure. Two entries are material.

**gf2's `u128` unpacking in the batch cell**: 700 ns of the 1090 ns call. It is
inside the timed window on purpose, because the contract requires both arms to
produce the same output representation, and gf2 still leads that cell by 6.7
times with it included.

**Field reduction, measured separately**: for the whole-consumer dot-product
cell `unpack_ns` is one Barrett reduction, run through
`amortised_probe_ns` over `AMORTISED_PROBE_REPEATS = 4096` repetitions because a
single reduction is below `Instant` resolution, and truncated to whole
nanoseconds by integer division. Both arms construct the identical
`BarrettReducer` over $x^8 + x^4 + x^3 + x + 1$ and call
`reduce_with_clmul` with the same `clmul`. The figure is 0 ns on the gf2 arm and
1 ns on the gf2x arm in every one of the 24 pairs, which bounds one reduction
below 1 ns on the first and inside [1, 2) ns on the second; the two arms run the
same reduction code, and the truncation is too coarse to resolve whether the
sub-nanosecond difference between the probes is real. Against calls of 1203 ns
and 7814 ns, field reduction is under 0.2% of the dot-product cell on either
arm: the cell's factor of 6.5 is the product-and-accumulate stage, and
separating the reduction does not move it. Packing costs 15495 ns on the gf2
arm, which builds two `FieldVec` operands, and 2995 ns on the gf2x arm, which
masks the fixture words to the field; both are charged outside the timed window.

## The `unstable` outcome on the six long-product cells

A cell whose flagged fraction exceeds `max_flagged_fraction` is recorded
`unstable` and, by the protocol's outlier policy, must be re-run. Six cells here
flag 116 to 119 of 240 windows, close to exactly half. That is not host noise,
and re-running cannot clear it.

### What the rule computes

`abtest::flagged_windows` counts windows above `flagged_window_factor` times a
median, and its own contract names the *execution* median. Its caller in
`receipt.rs` accumulates the windows of **both arms of every pair** into one
vector (`receipt.rs:1252` and `receipt.rs:1315`) and passes that pooled vector
(`receipt.rs:1382`), and the flagged fraction gates the outcome ahead of the
decision (`receipt.rs:1543`).

With 120 windows per arm, the pooled median of a cell whose arms differ by more
than a factor of two falls between the two clusters, and the threshold — twice
that median — falls inside the slower arm's own spread. Every window of the
slower arm above that point is flagged, so the flagged fraction is close to 0.5
by construction. It falls one to four short of 120 because that many of the slow
arm's own fastest windows sit just below the threshold.

### The same data recomputed within each arm

Projected by `survey/summarize-receipt.py --table flagged`, which applies the
protocol's own factor twice: once pooled, as `receipt.rs` does, and once within
each arm against that arm's own median.

| Cell | ratio of medians | pooled flagged | per-arm flagged | widest window within its arm | windows |
|---|---:|---:|---:|---:|---:|
| `poly-mul-4w-1core` | 0.6092 | 0 | 0 | 1.18 | 240 |
| `poly-mul-9w-1core` | 0.6621 | 0 | 0 | 1.21 | 240 |
| `poly-mul-64w-1core` | 167.3 | 119 | 0 | 1.06 | 240 |
| `poly-mul-256w-1core` | 298.4 | 118 | 0 | 1.07 | 240 |
| `poly-mul-2048w-streaming-1core` | 857.8 | 116 | 0 | 1.07 | 240 |
| `poly-mul-256w-6core` | 180.4 | 119 | 0 | 1.21 | 240 |
| `poly-mul-256w-12core` | 170.5 | 119 | 0 | 1.18 | 240 |
| `poly-mul-256w-24smt` | 167 | 117 | 0 | 1.16 | 240 |
| `clmul-batch-1024-1core` | 0.1494 | 0 | 0 | 1.07 | 240 |
| `gf2m-dot-1024-1core` | 0.1539 | 0 | 0 | 1.08 | 240 |

The `ratio of medians` column is each arm's median over all 240 windows, which
is not quite the estimator the confirmatory table above reports:
`abtest::speedup_of_medians` takes the median of the 24 per-pair `ns_per_call`
values, each of which is itself the median of that pair's five windows. The two
agree to within 1.4 percent, and to within 0.4 percent in nine of the ten cells.
The column is here so that the ratio sits beside flagged counts computed from
the same 240 windows; the speedup of record is the one above.

Not one window in the whole receipt exceeds twice its own arm's median. The
widest single window anywhere is 1.21 times its arm's median. The pooled count
is zero in all four cells whose arms are within a factor of seven and near half
in all six whose arms differ by 167 times or more. Flagging tracks the arm gap
and nothing else, and the host was quiet throughout.

### Where the artifact begins, as a function of the arm ratio

Other families in this epic have to know whether their own cells are exposed, so
the threshold is worth stating in closed form rather than as "large gaps".

With 120 windows from each arm and the two arms' windows disjoint — which they
are in all ten cells here, even the one whose arms differ by only 1.51 times —
the pooled median is the mean of the two middle order statistics, one drawn from
each cluster. The rule's threshold is therefore exactly

$$
T = 2 \cdot \operatorname{median}(\text{pooled}) = \max(\text{fast}) + \min(\text{slow}),
$$

and this identity holds to machine precision in every cell of the receipt. Two
consequences follow immediately. No window of the faster arm can ever be
flagged, because $T \geq \max(\text{fast})$. And a window of the slower arm is
flagged exactly when it exceeds $T$.

Write $R$ for the ratio of the two arms' medians, $D$ for the slower arm's full
window range relative to its own median, and $a$ for the faster arm's excess of
maximum over median. The slowest arm's largest window clears the threshold only
when $R(1 + c) > (1 + a) + R(1 - b)$ with $b + c = D$, so **no window is flagged
while**

$$
R \;\leq\; \frac{1 + a}{D}.
$$

Projected by `survey/summarize-receipt.py --table onset`, with the closed form's
predicted count beside the count the rule actually produced:

| Cell | arm ratio | slow-arm range | fast-arm excess | critical ratio | pooled flagged | closed form |
|---|---:|---:|---:|---:|---:|---:|
| `poly-mul-4w-1core` | 1.64 | 0.2164 | 0.0853 | 5.0 | 0 | 0 |
| `poly-mul-9w-1core` | 1.51 | 0.1543 | 0.2053 | 7.8 | 0 | 0 |
| `poly-mul-64w-1core` | 167.28 | 0.0822 | 0.0625 | 12.9 | 119 | 119 |
| `poly-mul-256w-1core` | 298.37 | 0.0945 | 0.0662 | 11.3 | 118 | 118 |
| `poly-mul-2048w-streaming-1core` | 857.77 | 0.0751 | 0.0699 | 14.2 | 116 | 116 |
| `poly-mul-256w-6core` | 180.43 | 0.3410 | 0.1610 | 3.4 | 119 | 119 |
| `poly-mul-256w-12core` | 170.45 | 0.3184 | 0.1459 | 3.6 | 119 | 119 |
| `poly-mul-256w-24smt` | 166.99 | 0.2163 | 0.0652 | 4.9 | 117 | 117 |
| `clmul-batch-1024-1core` | 6.69 | 0.0752 | 0.0658 | 14.2 | 0 | 0 |
| `gf2m-dot-1024-1core` | 6.50 | 0.0961 | 0.0796 | 11.2 | 0 | 0 |

The closed form reproduces the observed count in **all ten cells exactly**, at
separations from 1.51 to 858 times. That is what makes this an account of the
mechanism rather than a correlation with the effect size.

Three things a family planning its own cells can take from it.

**There is no universal ratio.** The critical ratio is a property of the cell's
own dispersion, and across these ten cells it ranges from 3.4 to 14.2. A family
whose arms hold a five-percent window range starts flagging near $R = 21$; one
whose arms hold a thirty-percent range starts near $R = 3.5$.

**Noisier arms make the rule misfire sooner.** $R^\ast$ falls as $D$ rises, so
the pooled rule reports spurious instability soonest on exactly the cells whose
windows genuinely vary most — the opposite of what a stability rule should do.
The three multicore cells here have the widest windows and the lowest critical
ratios of the ten.

**The onset is gradual in count, not a step.** Just above $R^\ast$ a handful of
the slow arm's windows clear the threshold; far above it, every slow window
except the few nearest its own minimum does, which is the 116 to 119 of 240 seen
here.

What this receipt cannot do is locate the onset empirically: its cells cluster at
$R \leq 6.69$, where nothing flags, and at $R \geq 167$, where almost everything
does, with nothing measured in between. The bracket from measurement alone is
therefore only "between 6.69 and 167". The closed form is what narrows it, and
it does so per cell rather than globally. A neighbouring survey's report of 3 of
60 flagged windows at a separation near 6 times, against 0 of 60 at 2.2 times,
is consistent with this: 3 of 60 is the just-above-$R^\ast$ regime, and it puts
that family's $D$ near 0.17, in the same range as the multicore cells here.

### What is reported, and why

These cells are reported with the `unstable` outcome retained and this
limitation stated beside every one of their numbers. Three reasons:

1. **The instability is a property of the flagging statistic under these arms
   and these frozen settings, not of the measurement.** The estimator is a ratio
   of medians resampled over whole pairs; the flagged count enters no part of
   it, and `benchmark-acceptance` independently recomputes every interval and
   decision from the raw pairs under P-20 and agrees with the runner's claim in
   all ten cells. A third recomputation of the same ten ratios of medians and
   their percentile intervals, run outside the campaign tooling with an
   unrelated pseudo-random generator, reproduces every point estimate to four
   decimals and every interval endpoint to within its Monte-Carlo error. The
   per-arm dispersion above is the direct evidence that the windows behind those
   intervals are clean.
2. **A re-run cannot clear it.** The flagged fraction is a deterministic
   function of the two arms' separation under `flagged_window_factor = 2`. Any
   re-run of a cell whose arms differ by 167 to 855 times, on any host however
   quiet, reproduces a flagged fraction near 0.5. The protocol's remedy for
   `unstable` does not apply to this cause.
3. **The addendum is not the problem, so it is not re-frozen.** Its cells,
   effect rule, equivalence margin, material-gap threshold and stopping rule are
   satisfied, and the question it was frozen to answer — how far the current gf2
   long product is from gf2x, and why — is answered above with intervals and
   sample counts. What the frozen protocol cannot deliver for a
   `comparator-gap` cell whose arms separate by more than its critical ratio is
   a `pass` outcome, because the flagged-fraction test gates the outcome ahead
   of the decision. Freezing a second addendum after seeing these results and re-running
   under it would be a protocol violation, and nothing here needs it.

### The corrected outcome, cell by cell

With the flagged fraction computed per arm it is 0 in every cell, so the
outcome ladder in `receipt.rs` falls through the `unstable` test to the decision
it already recorded. Nothing else in the ladder intervenes: the receipt declares
no settings deviation, every cell's `unresolved_settings` is empty, no cell is
exploratory and the family has no decoder cell, so the outcome becomes the
decision's `comparator-gap` mapping. The six affected cells resolve as:

| Cell | speedup | interval at 0.995 | decision | outcome as recorded | corrected outcome |
|---|---:|---|---|---|---|
| `poly-mul-64w-1core` | 167.4 | [165.6, 169.1] | improved | unstable | **pass** |
| `poly-mul-256w-1core` | 299.4 | [294.0, 303.0] | improved | unstable | **pass** |
| `poly-mul-2048w-streaming-1core` | 855.2 | [847.6, 865.3] | improved | unstable | **pass** |
| `poly-mul-256w-6core` | 180.4 | [168.5, 188.8] | improved | unstable | **pass** |
| `poly-mul-256w-12core` | 172.8 | [161.5, 182.8] | improved | unstable | **pass** |
| `poly-mul-256w-24smt` | 167.0 | [163.6, 171.1] | improved | unstable | **pass** |

All six become `pass`, and in this family's direction convention that reads
"gf2x is materially faster than gf2 by more than the 1.25 material-gap
threshold, at family confidence" — a `pass` here is a gf2 loss, not a gf2 win.
The four cells gf2 wins are unaffected: their pooled flagged count is already 0,
so per-arm flagging leaves `poly-mul-4w-1core`, `poly-mul-9w-1core`,
`clmul-batch-1024-1core` and `gf2m-dot-1024-1core` at `fail`.

The receipt-level verdict does not move. `qualifies` requires every
non-exploratory cell to pass; four cells decide `regressed`, so the family
cannot qualify for production selection under either flagging rule, which is
the right answer for a survey that adopts nothing.

Correcting the six outcomes therefore changes no number, no interval and no
conclusion in this document. What it changes is the reading of the receipt: six
cells whose measurement was sound were labelled as though the host had been
disturbed.

The defect is in the protocol's tooling, at the call site rather than in
`abtest::flagged_windows` itself, and belongs to issue `f547c394`. The fix is to
count flagged windows per arm against that arm's own median, which is what the
function's contract already states and what the `per-arm flagged` column above
computes. An independent methodology review of `f547c394` reaches the same call
site from the other direction, recording that pooling makes the rule's
sensitivity depend on effect size; this survey's contribution is the measured
demonstration on ten cells whose separations span 0.15 to 855 times. This survey patches no tooling and forks no private runner
(`@/inv/convention-convergence`); it publishes the receipts as the accepted tool
produced them.

## Limitations this receipt inherits from protocol version 1

An independent methodology review of `f547c394` records seven blocking defects
in the version-1 campaign tooling. Four of them touch this family. Each is
stated here as a limitation of the evidence; none is worked around, and no cell
is re-run to avoid one.

### The pooled flagged-window rule (receipt.rs:1315)

Diagnosed in full above. Outlier detection pools both arms' windows under one
absolute median threshold, so its sensitivity depends on the effect size rather
than on host quiet. At the 167 to 855 times separations of the six long-product
cells the pooled median sits between the two arms' distributions and near half
the windows flag by construction: pooled counts of 116 to 119 of 240 against
**zero** per-arm flags in every arm of every cell of the receipt. This is a
property of the statistic, not of gf2, gf2x or the host.

### The resume identity binds hostname and CPU ids only (benchmark-ab-runner.rs:215)

`ResumeIdentity` carries the protocol digest, source and executable digests, the
ordered work manifest, process descriptors, feature and thread contracts, and a
`host_identity` that is the hostname joined to the CPU id list — for this
campaign, `fraktaali;cpus=[0,…,23]`. Governor, SMT state, clocks and load are
not bound, so a resumed session could in principle continue under different host
conditions without the identity changing.

This campaign resumed once, and the two sessions split the family exactly in
half:

| Session | Host observed | One-minute load | Cells measured |
|---|---|---|---|
| 1 | 2026-09-07T19:56:11Z | 8.27 | `poly-mul-4w-1core`, `poly-mul-9w-1core`, `poly-mul-64w-1core`, `poly-mul-256w-1core`, `poly-mul-2048w-streaming-1core` |
| 2 | 2026-09-07T20:00:27Z | 7.91 | `poly-mul-256w-6core`, `poly-mul-256w-12core`, `poly-mul-256w-24smt`, `clmul-batch-1024-1core`, `gf2m-dot-1024-1core` |

Finalization keeps only the last session's observation, so the receipt's `host`
field describes session 2 and **does not describe the host under which the five
session-1 cells were measured**. The information is not lost: both observations
are journalled in `execution.log`, whose digest the receipt pins and whose replay
acceptance checks under P-10, and the table above is read from that log. What is
missing is a host record inside the `host` field covering every cell, and P-06
checks only the single stored observation. Comparing the two observations field
by field, they are identical in `hostname`, `cpu_model`, `cpu_flags`,
`os_kernel`, `governors`, `smt_active`, `affinity` and `topology`, and differ in
exactly three fields: `observed_utc`, `load_average` and
`available_memory_kib` (49.1 against 52.2 GiB). Nothing that would change a
measurement moved between the sessions, so the missing coverage is a gap in the
record rather than a known discrepancy.

### Calibration runs the workload before the first window (timing.rs:176)

`calibrated_calls` doubles its call count from one until a probe target is
reached, executing the workload each round, and only then does the first timed
window start. A cell declaring `cache_state: cold` therefore does not measure a
cold cache. **This family declares no `cold` cell**: nine cells declare `warm`
and `poly-mul-2048w-streaming-1core` declares `streaming`, and the arm applies
and reports each policy, which P-17 checks. The defect does not reach these
results. For the streaming cell the calibration pass does leave the fixture banks
resident before the first window, which is immaterial across five windows of
100 ms.

### The prior-trial ledger has no independent chain (receipt.rs:1565)

Acceptance verifies that every prior trial an addendum declares exists with its
recorded digest, but nothing outside the addendum establishes that the list is
complete, so a family's trial count is self-declared. This family declares
`prior_confirmatory_trials: 0` and an empty `prior_trials`, with
`max_confirmatory_attempts_per_candidate: 1`, so there is no chain to break —
and equally, the claim that this is the family's first and only confirmatory
attempt rests on that declaration rather than on an independently verifiable
record. The campaign journal supports it: `execution.log` opens with one
`campaign-start` and holds exactly one `cell-start` and one `cell-complete` per
cell across both sessions, which is what P-11 checks.

### What a version-2 re-run would and would not change

A re-run under protocol version 2 would replace the `unstable` outcomes with
whatever the per-arm flagging rule decides, and would give the family a host
record and a trial ledger that hold across a resume. It would not change the
measured quantities: the arms, their correctness validation, the operation
mapping, the build and assembly evidence and the field-reduction probe are
version-independent, and the ratios of medians and their intervals recompute
from the raw pairs under any flagging rule, because the flagged count enters
neither the estimator nor the bootstrap. The conclusions in this document are
therefore stated as they stand and are not held pending that re-run.

## Exploratory pilot

`dev/bench_results/c7113c5a/2026-09-07-c7113c5a-polynomial-pilot/`, digest
`22d2663f89f32bc0cccbd6baebad6c1b054ac04f408311a97b79790af7ecaed1`, is the
receipt the confirmatory addendum names as its resolution evidence. It is
labelled exploratory, every cell carries the outcome `pilot`, and **no cell of
it supports an adoption decision or a confirmed performance claim**. Six cells,
six pairs each, confidence 0.95:

| Cell | gf2 median | comparator median | speedup of medians | interval | pairs | decision | outcome | gf2 path |
|---|---:|---:|---:|---|---:|---|---|---|
| `poly-mul-4w-native-pilot` | 7.24 ns | 12.1 ns | 0.5987 | [0.5966, 0.6004] at 0.95 | 6 | regressed | pilot | `wide256:avx2+vpclmulqdq-ymm` |
| `poly-mul-256w-native-pilot` | 2.36 ms | 7.82 us | 302.1 | [301, 317.5] at 0.95 | 6 | improved | pilot | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-conservative-pilot` | 2.42 ms | 8.31 us | 291.3 | [290.7, 291.8] at 0.95 | 6 | improved | pilot | `clmul_wide_slice:schoolbook` |
| `poly-mul-256w-tuned-pilot` | 2.45 ms | 8.02 us | 305.7 | [304.4, 307.2] at 0.95 | 6 | improved | pilot | `clmul_wide_slice:schoolbook` |
| `poly-mul-4w-public-api-pilot` | 581 ns | 7.62 ns | 76.46 | [74.99, 79.38] at 0.95 | 6 | improved | pilot | `clmul_wide_slice:schoolbook` |
| `clmul-batch-1024-native-pilot` | 992 ns | 6.28 us | 0.1576 | [0.1524, 0.1766] at 0.95 | 6 | regressed | pilot | `clmul_batch:sequential-pclmulqdq (vpclmulqdq=1 avx512vl=0)` |

It supplies the measurement resolution 0.08 the confirmatory addendum freezes
against, and it carries two observations the confirmatory family does not cover.

### Host targeting: the three build variants

The pilot is the only receipt that times the conservative and tuned variants.
Its three 256-word rows agree within five percent of one another, so the gap is
not an artefact of compiler flags. That reading rests on an exploratory receipt
and is not confirmed.

The structural half of the same question needs no timing and is confirmed by
build evidence. `survey/fetch-build.sh` gives both sides the same three build
ladders: gf2x is configured with `CFLAGS` `-O2`, `-O3 -march=x86-64-v3` and
`-O3 -march=native`, and the gf2 arms of the same three variants are compiled
with `RUSTFLAGS` empty, `-C target-cpu=x86-64-v3` and `-C target-cpu=native`,
one variant at a time. gf2x's own `configure`
appends `-mpclmul` in every variant, so gf2x compiles its PCLMUL basecases even
under the bare `-O2`; gf2's carry-less paths reach the instruction through
runtime detection rather than through `-march`, and the receipt records the path
each arm resolved at run time. Neither side is handicapped by the portable
settings, and neither gains a capability the other is denied. The one asymmetry
runs against gf2x and is a property of gf2x 1.3.0 rather than of the build: it
emits no 256-bit VPCLMULQDQ in any variant, including `-march=native`, while
gf2's wide kernels do.

### The public long-product API misses the kernels gf2 already has

`clmul_wide` calls `clmul_wide_slice` directly
(`crates/gf2-core/src/gf2m/wide.rs:2006`, `:2052`), so a caller of the
documented public long-product API gets the scalar schoolbook even at four
words, where the dispatched kernel exists. The pilot measures that gap at 76
times. gf2's own field arithmetic is unaffected, because `Gf2mWide::mul_ref`
uses the crate-private dispatching helper instead
(`crates/gf2-core/src/gf2m/wide.rs:917`, `:2072`); no caller inside `crates/`
uses the public function. The confirmatory family does not repeat this cell, so
the factor of 76 is exploratory.

## Criterion outcomes

| Criterion | Outcome | Evidence |
|---|---|---|
| REQ-01 | met | Confirmatory receipt accepted with no finding; contract, protocol and addendum pinned by digest in `inputs/`; every cell reported above with its outcome: the four cells gf2 wins, the six it loses, and the `unstable` label the six losing cells carry |
| REQ-02 | met | `survey/gf2x-build-evidence.txt`: source pin, license test, `config.status` basecase link records, installed thresholds and the disassembly census |
| REQ-03 | met | Mapping table above, `survey/validation.json` (8930 checks per variant, no failures, executables pinned), reduction measured separately at under 0.2% of the dot-product cell |
| REQ-04 | met | Confirmatory receipt and the addendum frozen before it; the two raw independent-product cells labelled internal baselines with the non-equivalence stated |
| REQ-05 | partial | Host-targeting symmetry and the three-variant build evidence are confirmed; the conservative and tuned **timings** exist only in the exploratory pilot. `dev/active/c7113c5a/addendum-polynomial-baselines-2.json` is frozen and ready to close it |

Every criterion above is judged against a protocol-version-1 receipt whose four
inherited limitations the section above states. None of them changes a measured
quantity or a criterion outcome.

REQ-05 is partial in one specific way. The confirmatory family declares
`native` against `external` in all ten cells, so the only committed
confirmatory comparison of the conservative and tuned variants is the build
evidence, not a timing. Nothing in the survey's conclusions depends on it: the
gap the three pilot variants bracket is 291 to 306, inside which the
confirmatory native figure of 299.4 falls.

### The addendum frozen to close it

`dev/active/c7113c5a/addendum-polynomial-baselines-2.json` supersedes
`addendum-polynomial-baselines.json` and is frozen and committed without having
been run. It carries the ten cells above unchanged and adds a six-cell build
ladder — four-word and 256-word operands, each under the conservative, tuned and
native build of both sides:

| Cell | gf2 build | gf2x build | seed |
|---|---|---|---:|
| `poly-mul-4w-conservative-1core` | `conservative-portable`, no `RUSTFLAGS` | `-O2` | 211 |
| `poly-mul-4w-tuned-1core` | `tuned-portable`, `-C target-cpu=x86-64-v3` | `-O3 -march=x86-64-v3` | 212 |
| `poly-mul-4w-native-1core` | `native`, `-C target-cpu=native` | `-O3 -march=native` | 213 |
| `poly-mul-256w-conservative-1core` | `conservative-portable`, no `RUSTFLAGS` | `-O2` | 214 |
| `poly-mul-256w-tuned-1core` | `tuned-portable`, `-C target-cpu=x86-64-v3` | `-O3 -march=x86-64-v3` | 215 |
| `poly-mul-256w-native-1core` | `native`, `-C target-cpu=native` | `-O3 -march=native` | 216 |

Four design points, each of which a reviewer would otherwise have to reconstruct.

**Two sizes, not one.** Four words is where gf2's dispatched kernel leads and
256 words is where gf2x's subquadratic path leads, so the ladder tests whether
host targeting changes the size of either gap or the direction of the small one.

**The addendum cannot name the gf2x variant.** Its `build_identity` vocabulary
has a single `external` value, so every cell declares
`builds.candidate: external` and the variant pairing lives in the plan:
`run-baseline.sh confirmation-2` pairs `gf2-conservative` with
`gf2x-conservative`, `gf2-tuned` with `gf2x-tuned` and `gf2-native` with
`gf2x-native`, and the receipt's arm descriptors record the `CFLAGS`, the
`RUSTFLAGS` and the library each arm mapped. The equivalent host-targeting opportunity REQ-05
asks for is the pairing of the two sides' own optimisation knobs at the matching
level: `RUSTFLAGS` for gf2 against `CFLAGS` for gf2x — none against `-O2`,
`-C target-cpu=x86-64-v3` against `-O3 -march=x86-64-v3`, `-C target-cpu=native`
against `-O3 -march=native`. Both arms of a ladder cell additionally carry the
same `rustflags` for their Rust shell, so the harness around the C library is
not the thing that differs. Each arm's description records its flags, so the
symmetry is checkable in the receipt rather than only in the launcher.

**The native legs are deliberate duplicates.** `poly-mul-4w-native-1core` and
`poly-mul-256w-native-1core` repeat the operand sizes of `poly-mul-4w-1core` and
`poly-mul-256w-1core` so that the three legs of each ladder differ only in build
identity rather than also in seed and position; the duplication buys a
same-campaign replicate of two existing cells, which is direct evidence about
run-to-run reproducibility that this family does not otherwise have.

**One campaign, one correction.** The family declares the version-1
confirmation as its one prior confirmatory trial, pinned by receipt path and
digest, so the Bonferroni family is sixteen confirmatory cells plus one prior
trial: seventeen comparisons at per-comparison confidence 0.997059 against the
0.995 of the ten-cell family. Intervals widen slightly, which is the honest
price of measuring more cells and of having measured this family once already.
The campaign runs in four bounded sessions rather than two, so the exclusive
mutex is released three times mid-run.

**Why it waits for version 2 rather than running now.** Re-measuring the ten
carried cells is a second confirmatory attempt on the same candidate identity,
and `max_confirmatory_attempts_per_candidate` is 1. The protocol's family
section states the escape: a further attempt needs a new candidate identity or a
new protocol version, and counts in the family either way. This campaign takes
the second route, so it is blocked on version 2 landing rather than merely
waiting for it. Re-measuring is the right call over citing the version-1
receipt, because that receipt records six of the ten as `unstable` under the
rule version 2 replaces, and because one campaign gives the ladder and the ten
cells one host, one session ledger and one correction.

The addendum targets protocol version 1, because that is the version whose
schema exists: `addendum.schema.json` fixes `protocol.version` to the constant
1. Running it under a protocol version 2 needs the same content re-stamped with
the new version and freeze time, before any measurement. That is a re-freeze
ahead of a run rather than after a result, which is the distinction the freezing
rule draws; the two files should differ in `protocol.version` and
`frozen.frozen_utc` and in nothing else, and that is checkable by diff.
`run-baseline.sh confirmation-2` enforces the wait rather than leaving it to
prose: it exits 2 while the addendum still declares protocol version 1, before
reaching any build or any lock.

No gf2x production dependency exists and no gf2 multiplication algorithm
changes, as REQ-05 requires. The survey's own crate is outside the workspace and
the gf2x link lives only in its two arm binaries.

## Recommendations

None of these is performed here: the issue excludes production changes, and each
is a candidate experiment for a separate issue with its own before-and-after
receipt.

1. **Route the long product through the hardware carry-less multiply.**
   `clmul_wide_slice` multiplies word pairs with the scalar bit-by-bit loop even
   though `gf2_kernels_simd::gf2m` resolves a hardware kernel on this host. The
   confirmatory receipt bounds the available factor at 38.5 for the inner
   operation, and the pilot measures 76 for the four-word public API. This is
   the largest single lever and it changes no algorithm.
2. **Give `clmul_wide` and `clmul_wide_slice` the dispatch that
   `Gf2mWide::mul_ref` already has.** The two entry points differ only in
   whether they consult `clmul_wide_slice_product`, and only the private one
   does.
3. **Treat a subquadratic long product as a later question.** After the
   instruction factor is recovered, the measured residue against gf2x is 7.8 at
   256 words and about 22 at 2048. That is an algorithm change, explicitly out
   of scope for this survey, and worth its own issue only once the cheaper lever
   is taken.
4. **Do not add a gf2x dependency.** gf2x is GPL-3.0-or-later in the
   configuration that has a real `toom-gpl.c`, against gf2's MIT licence, and
   gf2 already leads the two long-product operations it has kernels for.

## Open questions

- The protocol's outlier rule flags pooled across arms rather than per arm, so a
  cell is recorded `unstable` on a quiet host once its arms separate by more
  than the critical ratio the onset section derives, and no re-run can clear it.
  The fix belongs to `f547c394`, where the epic's review record carries it as
  finding F8 (`dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r3.md`)
  and elevates it from advisory to blocking on the evidence measured here. What
  remains open is empirical rather than analytic: the closed form predicts every
  flag count in this receipt exactly, but the receipt measures no cell between
  6.69 and 167 times, so the onset is confirmed by derivation and by agreement at
  ten points rather than observed directly at its boundary. A family that wants
  it observed should place a cell where its own arms sit a little above
  $(1 + a) / D$.
- `Gf2mWide::mul_ref`'s Complexity rustdoc names `clmul_wide::<N, {2 * N}>`
  as the mechanism (`crates/gf2-core/src/gf2m/wide.rs:877`) while the body calls
  `clmul_wide_slice_product::<N>`, which dispatches to the SIMD kernels for
  `N = 4` and `N = 9`. Correcting production rustdoc is outside a survey that
  changes no production code; it needs its own tracked issue.
- gf2x 1.3.0 emits no 256-bit VPCLMULQDQ on this host even at `-march=native`,
  while gf2's wide kernels do. Whether gf2x's basecases would gain from the
  wider form is a question about gf2x, not about gf2, and is not measured here.
- The multicore cells define one call as `workers` times `inner` independent
  products executed in parallel, so their per-call figure is aggregate
  throughput under contention rather than a latency. Whether a shared-operand
  workload would contend differently is unmeasured.
- No confirmatory cell measures the conservative or tuned build variants, and no
  cell measures the public `clmul_wide` API. Both exist only in the exploratory
  pilot. `addendum-polynomial-baselines-2.json` closes the first of the two and
  leaves the second open; a public-API cell is not added there because the pilot
  measures that gap against the private path rather than against gf2x, which is
  a gf2-internal dispatch question rather than a comparator gap.
