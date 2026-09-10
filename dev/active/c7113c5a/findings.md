# Equivalent polynomial multiplication baselines

> **Diátaxis Type:** Explanation

## Result

gf2's dispatched long-product kernels beat gf2x [GfTwoX2026] at the two operand
sizes they cover; beyond them gf2 has only a scalar schoolbook and gf2x is two
to three orders of magnitude faster. The confirmatory host-targeting family
(24 pairs per cell, intervals at confidence 0.99375) finds gf2 1.452
[1.448, 1.456] times faster than gf2x at 4 words with both libraries built
conservatively and 1.586 [1.581, 1.589] times faster with both built tuned,
and gf2x 288.4 [287.6, 289.8] and 304.3 [303.8, 305.2] times faster at 256
words. The native family's fresh cells agree in direction with every cell of
the accepted protocol-v1 confirmation and lie within 3.7 percent of its point
estimates, but they are `not-confirmatory`: they are that family's second
attempt. The documented public long-product API runs the scalar schoolbook at
every size, a gf2-core defect tracked as `1c602857`. For GF(2^256) and
GF(2^571) the separated Barrett reduction costs more than either library's
unreduced product. No production code changes.

## Question and evidence

Raw independent carry-less products, reduced field dot products and long
binary-polynomial products are different operations. The survey asks which gf2
consumers perform an operation gf2x also performs once field reduction is
separated, how far current gf2 is from gf2x on this Ryzen 9 5900X under the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md), and what accounts for each
gap. The issue excludes adoption, a gf2x production dependency and any gf2
multiplication change, so every timed cell is a `comparator-gap` cell with gf2
as baseline and gf2x as candidate: a speedup of medians below 1 means gf2 is
faster.

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | gf2x export and three builds, three arm builds, under `target/c7113c5a-ext/` |
| `survey/run-validation.sh` | `survey/validation-v3.json`: correctness of every build, executable and library digests |
| `survey/record-build-evidence.sh`, `survey/probe-stock-build.sh` | `survey/gf2x-build-evidence-v3.txt`, `survey/gf2x-stock-build-probe.txt` |
| `survey/inspect-sources.py` | `survey/source-evidence.json`: every code claim below, cited by claim ID in backticks |
| `../../bench_results/c7113c5a/run-polynomial-v3.sh` | the four v3 campaigns; each receipt's `launcher.log` records every session command |
| `survey/freeze-confirmation.py` | confirmation addenda and `resolution-v3-baselines.txt`, `resolution-v3-host-targeting.txt` |
| `survey/summarize-v3.py`, `survey/reevaluate.sh` | [the generated tables](../../bench_results/c7113c5a/tables.md), `v3-reevaluation.txt`, `v1-reevaluation.txt` |

Every number below appears in those tables, an acceptance summary or a named
file. The host is `fraktaali`, an AMD Ryzen 9 5900X on Linux 7.2.2-arch1-1
with all 24 CPUs in the mask, SMT active and the `powersave` governor, as each
receipt's per-session host observation records. Timed work ran under
`dev/scripts/ccx1-bench-flock.sh --full-host`; arms and runner are Rust 1.95.0
release builds. All four campaigns share one producing closure of 184 files,
equal to the committed tree.

## Operation mapping

gf2x exposes one operation, `gf2x_mul_r`, the unreduced product of two
word-array polynomials (`gf2x-public-entry`). Its words and bits follow gf2's
canonical little-endian numbering, so neither arm converts representation.

| gf2 consumer | Unreduced stage | gf2x form | Status |
|---|---|---|---|
| `Gf2mWide<4>`, `Gf2mWide<9>::mul_ref` (GF(2^256), GF(2^571)) | dispatched YMM kernel (`mul-ref-uses-helper`, `dispatch-helper-n4`, `dispatch-helper-n9`, `cached-wide-detection`) | one `gf2x_mul_r` | equivalent; the reduction (`mul-ref-reduces`) is measured separately |
| public `clmul_wide`, `clmul_wide_slice` | scalar schoolbook at every size (`public-clmul-wide-delegates`, `clmul-wide-slice-word-product`, `barrett-clmul-is-scalar`) | one `gf2x_mul_r` | equivalent |
| `Gf2mWide<N>` for other N, `BarrettReducerWide` products | scalar schoolbook through the fallback (`dispatch-helper-fallback`, `barrett-wide-uses-helper`) | one `gf2x_mul_r` | equivalent at 64, 256 and 2048 words |
| raw carry-less batch (`raw-batch-default-sequential`) | 1024 independent 64x64 products | 1024 one-word calls | internal baseline; composed, not equivalent |
| `FieldVec::simd_dot_product`, GF(2^8) (`dot-product-uses-raw-batch`, `dot-product-reduces-once`) | batch, XOR accumulation, one reduction | 1024 calls plus gf2's reducer | internal baseline; composed, not equivalent |

gf2x has no batch entry point. A one-word `gf2x_mul_r` call costs 6.184 ns and
a four-word call, which does nine word multiplications, 12.04 ns, so per-call
overhead dominates the composed arm. The two `internal-` cells bound what a
consumer reaching gf2x through its public API pays; they measure neither gf2x's
basecase nor a gf2 advantage at a 64x64 product. Polynomials over GF(2^m) with
m > 1 have no gf2x counterpart.

## Pinned comparator

gf2x 1.3.0 is tag `gf2x-1.3.0`, commit `27ba588f03bf6e1e74763903bab25e6e8bb6d0f0`,
exported with `git archive` and generated with Autoconf 2.73, Automake 1.18.1
and Libtool 2.6.2. It is GPL-3.0-or-later: `configure.ac` defines
`GPL_CODE_PRESENT` from `toom-gpl.c`'s contents (`gf2x-gpl-condition`), all
three builds define it as 1, and without it every size uses Karatsuba
(`gf2x-kara-only-without-gpl`).

The stock configuration fails on this host's GCC 16.2.1: configure rejects
`gcc` as build compiler because its probe calls `exit` undeclared, falls back
to `c89`, and `make` stops because `c89` rejects C++ comments in
`lowlevel/gen_bb_mul_code.c` (`survey/gf2x-stock-build-probe.txt`). Every build
therefore passes `CC_FOR_BUILD=gcc -std=gnu99
-Wno-implicit-function-declaration`, which the v1 builds also used but the v1
script omitted. That generator emits only the bit-by-bit basecases used
without PCLMUL.

What gf2x runs is established from build, source and assembly
(`survey/gf2x-build-evidence-v3.txt`):

- **Hardware directory.** `configure` resolves `hwdir=x86_64_pclmul` in all
  builds and appends `-mpclmul` even under a bare `-O2` (`gf2x-appends-mpclmul`);
  its word basecase is the 128-bit PCLMULQDQ intrinsic (`gf2x-mul1-header-target`,
  `gf2x-mul1-pclmul`).
- **Algorithm per size.** Below 10 words `gf2x_mul_r` takes the basecase
  (`gf2x-basecase-branch`): `gf2x_mul4` is a Karatsuba of three `mul2` calls,
  9 word multiplications (`gf2x-mul4-karatsuba`); `gf2x_mul9` uses 30
  (`gf2x-mul9-thirty`). Larger balanced operands skip the FFT, whose tuned
  table selects K = 1 at every measured size (`gf2x-fft-gate`), and `gf2x_mul_toom`
  reads the tuned `GF2X_BEST_TOOM_TABLE` rather than comparing thresholds
  (`gf2x-toom-selector`, `gf2x-best-toom-table`): Karatsuba at 64 words,
  Toom-3W at 256, Toom-4 at 2048, in every build.
- **Instructions.** The conservative library holds 100 legacy `pclmul*`
  instructions and the tuned and native libraries 109 VEX `vpclmul*`, none with
  a `ymm` operand: gf2x 1.3.0 never uses 256-bit VPCLMULQDQ.

gf2's 4- and 9-limb kernels perform all 16 and 81 schoolbook word products,
two per 256-bit VPCLMULQDQ (`wide256-kernel-pairs-16-products`,
`wide571-kernel-81-products`). Every gf2x execution in the v3 receipts reports
the library path and SHA-256 it mapped, and all 540 equal their build's digest
in `survey/validation-v3.json`.

## Correctness

`survey/validation-v3.json` passes on all three builds with no failure, against
a canonical product, the XOR of `b` shifted by every set bit of `a`, that
shares no code with either library: single-term operands at bit positions 0,
1, 62 to 65, 126 to 128, 191 and 255 (242 cases per build); random operands of
every measured length from 1 to 2048 words with complete outputs and a zero top
word in the last trial (232); the dispatched kernels against the reference and
gf2x (256); 1024-product raw batches element by element (8192); the wide-field
decomposition, where `Gf2mWide::mul_ref`, the dispatched kernel plus
`BarrettReducerWide` and gf2x plus the same reducer each equal the canonical
product reduced by long division (402); and the GF(2^8) dot product of both
arms against a long-division reference (8). The file pins every validated
executable and library; they equal the receipts' arm and library digests. The
selected gf2 paths are `wide256:avx2+vpclmulqdq-ymm`,
`wide571:avx2+vpclmulqdq-ymm` and `clmul_batch:pclmulqdq-scalar-xmm`
(`raw-batch-lane-tag`).

## Two families

**`polynomial-multiplication-baselines`** repeats the v1 question under
protocol v3: how far is current gf2, built native, from gf2x built native,
across sizes, warm and streaming caches and 1, 6, 12 and 24 CPUs. Its ledger
imports the v1 pilot (zero comparisons) and the accepted v1 confirmation (ten)
before any v3 campaign (`../../bench_results/c7113c5a/v3-baselines-ledger-origin.json`).
The v3 confirmation is attempt $t = 2$ with cumulative $m = 21$: it spends
$\alpha_2 = 0.05/6$, runs each comparison at
$\alpha_c = \alpha_2/21 \approx 3.97 \times 10^{-4}$ and expects
$10000\,\alpha_c/2 \approx 1.98$ draws per tail against P-20's 20. No cell set
could be confirmatory at $t = 2$ after ten spent cells. The v3 cells are
fresh-sample measurements; the v1 confirmation stays the family's confirmatory
attempt.

**`polynomial-host-targeting`** asks a different question: with equivalent
targeting below native, conservative (no `RUSTFLAGS`; `CFLAGS -O2`) and tuned
(`-C target-cpu=x86-64-v3`; `-O3 -march=x86-64-v3`), how far is gf2 from gf2x
at a dispatched-kernel size (4 words) and a schoolbook-versus-Toom size
(256 words)? Its hypotheses concern executables no confirmatory attempt
measured: the v1 confirmation declared `native` against `external` in all ten
cells, and the only earlier measurement is two exploratory v1 pilot cells, whose
pilot the ledger imports. The cells reuse the native family's operand fixtures
and omit native legs, which would re-test that family's hypotheses outside its
ledger. The superseded v1 addendum `addendum-polynomial-baselines-2.json`
bundled both questions to pay one v1 correction; it was never measured and
reserved nothing (`../../bench_results/c7113c5a/v3-host-targeting-ledger-origin.json`).
The confirmation is attempt $t = 1$ with $m = 4$: $\alpha_c = 0.00625$, 31.25
expected draws per tail.

Resolution follows the v3 pilots: the host-targeting pilot's widest relative
half-width is 0.0093, frozen as 0.01 with margins 1.15 and 1.25; the native
pilot's is 0.2378, set by its twelve-core cell, frozen as 0.24, so the native
confirmation raises the equivalence margin to 1.25, the smallest two-decimal
margin above $1.24$.

## Results

### Native baselines

`v3-r1-baselines-confirmation`: 24 pairs per cell, intervals at confidence
0.99960, every outcome `not-confirmatory`; v1 estimates at confidence 0.995.

| Cell | gf2 median | gf2x median | Speedup | Interval | v1 speedup |
|---|---:|---:|---:|---|---:|
| 4 words | 7.348 ns | 12.04 ns | 0.6104 | [0.6081, 0.6159] | 0.6085 |
| 9 words | 22.62 ns | 34.55 ns | 0.6546 | [0.6518, 0.6578] | 0.6625 |
| 64 words | 134.7 µs | 808.9 ns | 166.5 | [164.9, 167.4] | 167.4 |
| 256 words | 2.323 ms | 7.744 µs | 300.0 | [299.0, 301.3] | 299.4 |
| 2048 words, streaming | 156.0 ms | 180.1 µs | 866.4 | [860.5, 871.8] | 855.2 |
| 256 words, 6 cores | 84.19 ms | 484.6 µs | 173.7 | [158.4, 181.3] | 180.4 |
| 256 words, 12 cores | 102.2 ms | 614.1 µs | 166.5 | [138.8, 190.0] | 172.8 |
| 256 words, 24 CPUs | 140.5 ms | 839.7 µs | 167.3 | [159.6, 171.5] | 167.0 |
| 4 words, public API | 555.7 ns | 12.12 ns | 45.85 | [45.67, 46.13] | - |
| internal raw batch | 973.8 ns | 6.332 µs | 0.1538 | [0.1532, 0.1567] | 0.1491 |
| internal GF(2^8) dot | 1.060 µs | 6.758 µs | 0.1568 | [0.1532, 0.1589] | 0.1536 |

### Host targeting

| Size | Level | gf2 median | gf2x median | Speedup | Interval | Outcome |
|---|---|---:|---:|---:|---|---|
| 4 words | conservative | 7.546 ns | 10.96 ns | 0.6886 | [0.6867, 0.6907] | fail |
| 4 words | tuned | 7.559 ns | 11.99 ns | 0.6304 | [0.6291, 0.6325] | fail |
| 256 words | conservative | 2.358 ms | 8.177 µs | 288.4 | [287.6, 289.8] | pass |
| 256 words | tuned | 2.378 ms | 7.812 µs | 304.3 | [303.8, 305.2] | pass |

`fail` means gf2 is faster; `pass` means gf2x is faster by more than the 1.25
material-gap threshold. Host targeting changes neither direction nor order of
magnitude. gf2's times move by at most 2.9 percent across the three levels
because its kernels are selected at run time; gf2x's four-word basecase is
fastest under `-O2` (10.96 ns against 11.99 and 12.04 ns), and its 256-word
Toom-3W gains 4.5 to 5.3 percent from `-O3 -march`. Comparisons across levels
are between separate cells and descriptive; the native legs are the native
family's `not-confirmatory` cells.

### What accounts for the gaps

The tables' derived estimates divide medians of different cells and carry no
interval. gf2's scalar schoolbook costs 32.88, 35.45 and 37.19 ns per word
product at 64, 256 and 2048 words, flat as a quadratic algorithm with a fixed
inner cost; gf2x's normalised rate falls from 0.1975 to 0.04293 ns, the
signature of subquadratic recursion. A hardware word product costs 0.951 ns in
the sequential raw batch and 0.4592 ns in the YMM 4-limb kernel. Scalar over
sequential hardware products is an instruction factor of 37.28; the 256-word
gap divided by it leaves 8.048, and the 2048-word gap divided by its own
factor 22.15, the share attributable to gf2x's algorithms. Under 6, 12 and 24
workers gf2's aggregate throughput rises 5.3, 8.73 and 12.7 times and gf2x's
3.07, 4.84 and 7.08 times, which narrows the gap from 300.0 to 166.5 to 173.7;
the multicore intervals are wide (twelve cores: [138.8, 190.0]). At 4 and 9
words gf2 performs more word multiplications than gf2x (16 and 81 against 9
and 30) yet wins, two per 256-bit instruction against gf2x's one per 128-bit
instruction; this is a source-level account, not a profile.

### Separated reduction and adapter costs

Each arm reports costs outside its timed windows. The amortised, rounded-up
reduction a `Gf2mWide` consumer adds is 41.5 ns (gf2 arm) and 40 ns (gf2x arm)
for GF(2^256) and 85 and 89 ns for GF(2^571), against unreduced products of
7.348 and 12.04 ns and 22.62 and 34.55 ns: the reduction is 72 to 85 percent of
a composed field multiplication on either arm, so gf2's unreduced lead is the
minor term of a wide-field multiplication. gf2's capability detection costs
120 to 170 ns and gf2x's scratch pool 695 to 850 ns, once per process or
worker. The pack, batch-fill and raw-batch unpack probes are single cold passes
that include first-touch effects (the raw-batch unpack probe reads 980 ns
against a 973.8 ns timed call that already contains it); they are not
steady-state costs. The dot product's GF(2^8) reduction is 1 to 2 ns of a
1.06 µs call.

## The public long-product API misses gf2's kernels

The source establishes the defect at the pinned gf2 commit. A caller of the
documented public API, `clmul_wide` or `clmul_wide_slice`, gets the scalar
bit-by-bit schoolbook at every size, including 4 and 9 words
(`public-clmul-wide`, `public-clmul-wide-delegates`,
`clmul-wide-slice-word-product`, `barrett-clmul-is-scalar`,
`scalar-clmul-bit-loop`). The dispatching helper is crate-private
(`dispatch-helper-crate-private`); `Gf2mWide::mul_ref` and the wide Barrett
reducer use it, so gf2's own field arithmetic is unaffected, and the recorded
search finds no production caller of the public functions. `mul_ref`'s
rustdoc still names `clmul_wide` as its mechanism
(`mul-ref-rustdoc-names-public-path`).

The measured size of the defect is not confirmatory. The only paired
public-versus-dispatched comparison is the exploratory v1 pilot cell, which
found the public path 76.46 [74.99, 79.38] times slower than the dispatched
kernel at 4 words (6 pairs, confidence 0.95, outcome `pilot`). The v3
`poly-mul-4w-public-api-1core` cell finds gf2x 45.85 [45.67, 46.13] times
faster than the public path (24 pairs, `not-confirmatory`); its median is
75.62 times the dispatched kernel's, a quotient of two cells. The fix is
tracked as `1c602857`, which uses this cell as its pre-change baseline; the
survey changes no production code.

## Protocol-v1 history

The v1 pilot and confirmation, their snapshots and summaries are unchanged. The
v1 confirmation is accepted with four `fail` cells, which gf2 wins, and six
long-product cells recorded `unstable` by v1's pooled flagging rule; recounted
under v3's per-execution rule no window of any v1 cell is flagged, and the
current evaluator reproduces every v1 verdict, decision and outcome
(`v1-reevaluation.txt`). The v1 evidence cannot serve v3: its resolution pilot
names another family and its producing closure bound neither the gf2 crates nor
the arm sources. The v1 confirmation's pinned `Cargo.lock` snapshot, left
uncommitted on the v1 branch, is now committed. An interrupted v1 confirmation
launch and an abandoned pilot launch, recorded only in `/tmp`, are preserved in
`../../bench_results/c7113c5a/v1-launch-history/`; neither shows a runner
announcement, and the origin record states why neither holds a ledger line.

## Limits and follow-up

The native family's v3 cells are not confirmatory, and its confirmatory v1
evidence carries the six `unstable` labels described above. gf2x runs its
shipped `x86_64_pclmul` tuning tables; a host-retuned gf2x is unmeasured. No
cell measures a cold cache; the 2048-word streaming cell rotates eight 64 KiB
banks, which does not show eviction from any cache level. The toolchain is
Rust 1.95; the v1 receipts used 1.97. The family addenda name gf2x 1.3.0; its
commit, flags and digests live in the plan and receipt arm records and the
validation report. No absolute optimum is claimed. Candidate follow-ups, none
performed here beyond the tracked `1c602857`: hardware word products beyond 9
words (instruction factor 37.28); a subquadratic product for the remaining
factor of 8.048 at 256 words and 22.15 at 2048; and a profile of
`BarrettReducerWide`, which dominates GF(2^256) and GF(2^571) multiplication.
A gf2x dependency is not recommended: gf2x is GPL-3.0-or-later in this
configuration and gf2 is MIT.

## Criterion status

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET | Four accepted v3 receipts pinning contract, protocol, addendum, producing closure and ledger prefix; no production change needs before/after; every negative and not-confirmatory outcome retained; v1 history preserved |
| REQ-02 | MET | `survey/gf2x-build-evidence-v3.txt`, `survey/gf2x-stock-build-probe.txt`, `survey/source-evidence.json` |
| REQ-03 | MET | Operation mapping above; `survey/validation-v3.json`; separated reduction and adapter costs in the tables |
| REQ-04 | MET | `v3-r1-baselines-confirmation` and `addendum-v3-baselines-confirmation.json`; `internal-` raw-batch cells |
| REQ-05 | MET | Matching build ladders on both sides; confirmatory `v3-r1-host-targeting-confirmation`; no gf2x dependency or algorithm change |
