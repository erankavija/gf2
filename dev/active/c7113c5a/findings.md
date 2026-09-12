# Equivalent polynomial multiplication baselines

> **Diátaxis Type:** Explanation

## Result

gf2's dispatched long-product kernels beat gf2x [GfTwoX2026] at the two operand
sizes they cover, 4 and 9 words; beyond them gf2 has only a scalar schoolbook,
and gf2x is two to three orders of magnitude faster. The confirmatory
host-targeting family finds gf2 faster than gf2x at 4 words and gf2x faster at
256 words, with both libraries built conservatively and with both built tuned.
The native family's fresh cells agree in direction with every cell of the
accepted protocol-v1 confirmation, but they are `not-confirmatory`: they are
that family's second attempt. The documented public long-product API runs the
scalar schoolbook at every size, a gf2-core defect tracked as `1c602857`. For
GF(2^256) and GF(2^571) the separated Barrett reduction costs more than either
library's unreduced product. The receipts' dot-product reduction values time
the reducer's early exit on an already reduced value; a repeated diagnostic
replaces them. No production code changes. The sections below point to the
evidence for each statement.

## Question and evidence

Raw independent carry-less products, reduced field dot products and long
binary-polynomial products are different operations. The survey asks which gf2
consumers perform an operation gf2x also performs once field reduction is
separated, how far current gf2 is from gf2x on this Ryzen 9 5900X under the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 3](../f547c394/amendment-v3.md) — the version every receipt pins,
snapshots and is evaluated under — and what accounts for each
gap. The issue excludes adoption, a gf2x production dependency and any gf2
multiplication change, so every v3 cell is a `comparator-gap` cell with gf2 as
baseline and gf2x as candidate: a speedup of medians below one means gf2 is
faster.

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | gf2x export, its conservative, tuned and native builds and the matching arm builds, under `target/c7113c5a-ext/` |
| `survey/run-validation.sh` | `survey/validation-v3.json`: correctness of every build, executable and library digests |
| `survey/record-build-evidence.sh`, `survey/probe-stock-build.sh` | `survey/gf2x-build-evidence-v3.txt`, `survey/gf2x-stock-build-probe.txt` |
| `survey/inspect-sources.py` | `survey/source-evidence.json`: every code claim below, cited by claim ID in backticks |
| `../../bench_results/c7113c5a/run-polynomial-v3.sh` | the v3 campaigns; each receipt's `launcher.log` records every session command |
| `../../bench_results/c7113c5a/run-dot-reduction-diagnostic.sh` | `../../bench_results/c7113c5a/v3-dot-reduction-diagnostic/`: raw values, identity and command log of the repeated dot-product reduction probe |
| `survey/freeze-confirmation.py` | confirmation addenda and `resolution-v3-baselines.txt`, `resolution-v3-host-targeting.txt` |
| `survey/summarize-v3.py`, `survey/reevaluate.sh` | [the generated tables](../../bench_results/c7113c5a/tables.md), `v3-reevaluation.txt`, `v1-reevaluation.txt` |

This report states no measured value. Each conclusion points to its source: a
section of [the generated tables](../../bench_results/c7113c5a/tables.md),
written "tables § heading" with the receipt directory shortened to its last
component, and a row named by its cell ID or row ID; a receipt or summary
field; or a named file. Speedup intervals are the acceptance tool's; every
other interval in the tables is the generator's percentile bootstrap, whose
method section states the resampling, draw count and seed. Running the
generator command recorded there reproduces the tables byte for byte.

The host is `fraktaali`, a Ryzen 9 5900X; each receipt records its CPU model,
kernel, governors, SMT state and CPU mask per session (`session_hosts`). Timed
work ran under `dev/scripts/ccx1-bench-flock.sh --full-host`; arms and runner
are Rust 1.95 release builds (`toolchain` in each receipt). All v3 campaigns
share one producing closure, `survey/producing-inputs.json`, which each receipt
pins under `source.producing`. It equals the survey branch's tree at
`38d2c091`; merged main differs from it only in rustdoc of
`crates/gf2-core/src/field/vec.rs`, and the round-1 rework's arm-source changes
bind only the diagnostic.

Seeds: each campaign seed in the launcher drives the runner's counterbalanced
pair order and the acceptance bootstrap through `Xoshiro256StarStar` seeded by
`SplitMix64`, both in tuning-campaign-support 0.1.0
(`dev/tools/tuning-campaign-support/src/abtest.rs`). Each cell's case seed in
its receipt's `plan.json` generates the operands with that crate's `SplitMix64`,
expanded per worker and bank by `banks` in `survey/gf2-side/src/lib.rs`; the
validator's fixed seeds, for the same generator, are in
`survey/gf2-side/src/bin/poly-validate.rs`.

## Operation mapping

gf2x exposes one operation, `gf2x_mul_r`, the unreduced product of two
word-array polynomials (`gf2x-public-entry`). Its words and bits follow gf2's
canonical little-endian numbering, so neither arm converts representation.

| gf2 consumer | Unreduced stage | gf2x form | Status |
|---|---|---|---|
| `Gf2mWide<4>`, `Gf2mWide<9>::mul_ref` (GF(2^256), GF(2^571)) | dispatched YMM kernel (`mul-ref-uses-helper`, `dispatch-helper-n4`, `dispatch-helper-n9`, `cached-wide-detection`) | one `gf2x_mul_r` | equivalent; the reduction (`mul-ref-reduces`) is measured separately |
| public `clmul_wide`, `clmul_wide_slice` | scalar schoolbook at every size (`public-clmul-wide-delegates`, `clmul-wide-slice-word-product`, `barrett-clmul-is-scalar`) | one `gf2x_mul_r` | equivalent |
| `Gf2mWide<N>` for other N, `BarrettReducerWide` products | scalar schoolbook through the fallback (`dispatch-helper-fallback`, `barrett-wide-uses-helper`) | one `gf2x_mul_r` | equivalent at 64, 256 and 2048 words |
| raw carry-less batch (`raw-batch-default-sequential`) | independent 64x64 products | one one-word call per product | internal baseline; composed, not equivalent |
| `FieldVec::simd_dot_product`, GF(2^8) (`dot-product-uses-raw-batch`, `dot-product-reduces-once`) | batch, XOR accumulation, one reduction | one-word calls plus gf2's reducer with the scalar `barrett::clmul` | internal baseline; composed, not equivalent |

gf2x has no batch entry point, and its per-call overhead dominates the composed
arm: a four-word call, which performs several word multiplications
(`gf2x-mul4-karatsuba`), costs far less per multiplication than a one-word call
(tables § Derived estimates: `v3-r1-baselines-confirmation`, rows
`gf2x-one-word-call` and `gf2x-four-word-call`). The two `internal-` cells bound
what a consumer reaching gf2x through its public API pays; they measure neither
gf2x's basecase nor a gf2 advantage at a 64x64 product. Polynomials over
GF(2^m) with m > 1 have no gf2x counterpart.

## Pinned comparator

gf2x 1.3.0 is tag `gf2x-1.3.0`, commit `27ba588f03bf6e1e74763903bab25e6e8bb6d0f0`,
exported with `git archive` and generated with the host autotools the build
evidence records (`survey/gf2x-build-evidence-v3.txt`, section *generated build
system*). It is GPL-3.0-or-later: `configure.ac` defines `GPL_CODE_PRESENT` from
`toom-gpl.c`'s contents (`gf2x-gpl-condition`), every build defines it (each
variant's `GPL_CODE_PRESENT` line), and without it every size uses Karatsuba
(`gf2x-kara-only-without-gpl`).

The stock configuration fails on this host's GCC (version in the build
evidence's *build host* section): configure rejects `gcc` as build compiler
because its probe calls `exit` undeclared, falls back to `c89`, and `make`
stops because `c89` rejects C++ comments in `lowlevel/gen_bb_mul_code.c`
(`survey/gf2x-stock-build-probe.txt`). Every build therefore passes
`CC_FOR_BUILD=gcc -std=gnu99 -Wno-implicit-function-declaration`, which the v1
builds also used but the v1 script omitted. That generator emits only the
bit-by-bit basecases used without PCLMUL.

What gf2x runs is established from build, source and assembly
(`survey/gf2x-build-evidence-v3.txt`, one section per build variant):

- **Hardware directory.** `configure` resolves `hwdir=x86_64_pclmul` in all
  builds and appends `-mpclmul` even under a bare `-O2` (`gf2x-appends-mpclmul`);
  its word basecase is the 128-bit PCLMULQDQ intrinsic (`gf2x-mul1-header-target`,
  `gf2x-mul1-pclmul`).
- **Algorithm per size.** Below the Karatsuba threshold `gf2x_mul_r` takes the
  basecase (`gf2x-basecase-branch`; each variant's *algorithm thresholds*):
  `gf2x_mul4` is a Karatsuba of three `mul2` calls (`gf2x-mul4-karatsuba`) and
  `gf2x_mul9` a reduced-multiplication variant (`gf2x-mul9-thirty`), both with
  fewer word multiplications than a schoolbook. Larger balanced operands skip
  the FFT, which the tuned table disables at every measured size
  (`gf2x-fft-gate`), and `gf2x_mul_toom` reads the tuned `GF2X_BEST_TOOM_TABLE`
  rather than comparing thresholds (`gf2x-toom-selector`,
  `gf2x-best-toom-table`): Karatsuba at 64 words, Toom-3W at 256, Toom-4 at
  2048, in every build (each variant's *gf2x_mul_r selection for the measured
  balanced operand sizes*).
- **Instructions.** The conservative library holds legacy `pclmul*`
  instructions and the tuned and native libraries VEX `vpclmul*`, none with a
  `ymm` operand (each variant's *carry-less multiply instruction census* and
  *ymm-operand carry-less multiplies*): gf2x 1.3.0 never uses 256-bit
  VPCLMULQDQ.

gf2's 4- and 9-limb kernels perform every schoolbook word product, paired into
256-bit VPCLMULQDQ (`wide256-kernel-pairs-16-products`,
`wide571-kernel-81-products`). Every gf2x execution in the v3 receipts reports
the library path and SHA-256 it mapped (`selected_path`), each equal to its
build's digest in `survey/validation-v3.json` (`executables.<build>.libgf2x`);
tables § gf2x libraries the candidate arms mapped lists the one digest each arm
mapped.

## Correctness

`survey/validation-v3.json` passes on every build with no failure
(`variants.<build>.passed`; each check's `cases` and `failures`), against a
canonical product, the XOR of `b` shifted by every set bit of `a`, that shares
no code with either library: single-term operands at the word-boundary bit
positions of `BIT_POSITIONS` in `survey/gf2-side/src/bin/poly-validate.rs`;
random operands of every measured length (`LENGTHS`) with complete outputs and
a zero top word in the last trial; the dispatched kernels against the reference
and gf2x; the raw batches element by element; the wide-field decomposition,
where `Gf2mWide::mul_ref`, the dispatched kernel plus `BarrettReducerWide` and
gf2x plus the same reducer each equal the canonical product reduced by long
division; and the GF(2^8) dot product of both arms against a long-division
reference. The file pins every validated executable and library; they equal
the receipts' arm and library digests. The selected gf2 paths are
`wide256:avx2+vpclmulqdq-ymm`, `wide571:avx2+vpclmulqdq-ymm` and
`clmul_batch:pclmulqdq-scalar-xmm` (`raw-batch-lane-tag`;
`variants.<build>.gf2_selected_paths`).

## Two families

**`polynomial-multiplication-baselines`** repeats the v1 question under
protocol v3: how far is current gf2, built native, from gf2x built native,
across sizes, warm and streaming caches and worker counts (the cells of tables
§ Cells: `v3-r1-baselines-confirmation`). Its ledger imports the v1 pilot and
the accepted v1 confirmation before any v3 campaign
(`../../bench_results/c7113c5a/v3-baselines-ledger-origin.json`, `sources`), so
the v3 confirmation is the family's second attempt and its cumulative
comparison count includes the v1 confirmation's cells (tables § Campaigns, row
`v3-r1-baselines-confirmation`, columns *Ledger comparisons*, *Attempt alpha*
and *Per-comparison confidence*). At that corrected alpha the frozen bootstrap
resamples leave fewer expected draws per tail than P-20 requires, for any cell
set a second attempt could have declared; the acceptance summary records a P-20
note for every cell (column *Findings*; `findings` in
`v3-r1-baselines-confirmation/acceptance-summary.json`) and every outcome is
`not-confirmatory`. The v3 cells are fresh-sample measurements; the v1
confirmation stays the family's confirmatory attempt.

**`polynomial-host-targeting`** asks a different question: with equivalent
targeting below native, conservative (no `RUSTFLAGS`; `CFLAGS -O2`) and tuned
(`-C target-cpu=x86-64-v3`; `-O3 -march=x86-64-v3`), how far is gf2 from gf2x
at a dispatched-kernel size (4 words) and a schoolbook-versus-Toom size
(256 words)? Its hypotheses concern executables no confirmatory attempt
measured: the v1 confirmation declared `native` against `external` in every
cell, and the only earlier measurement is the exploratory v1 pilot's
conservative and tuned cells (tables § Cells:
`2026-09-07-c7113c5a-polynomial-pilot`, rows `poly-mul-256w-conservative-pilot`
and `poly-mul-256w-tuned-pilot`), whose pilot the ledger imports. The cells
reuse the native family's operand fixtures and omit native legs, which would
re-test that family's hypotheses outside its ledger. The superseded v1
addendum `addendum-polynomial-baselines-2.json` bundled both questions to pay
one v1 correction; it was never measured and reserved nothing
(`../../bench_results/c7113c5a/v3-host-targeting-ledger-origin.json`,
`frozen_addenda_never_run`). The confirmation is the family's first attempt,
whose per-comparison alpha satisfies P-20 (tables § Campaigns, row
`v3-r1-host-targeting-confirmation`), so its cells can be confirmatory.

Resolution follows the v3 pilots (P-03). Each confirmation freezes
`effect.measurement_resolution` at its pilot's widest relative half-width,
rounded up (`resolution-v3-host-targeting.txt`, `resolution-v3-baselines.txt`,
lines *widest relative half-width* and *frozen measurement resolution*). The
native pilot's widest half-width, set by its twelve-core cell, left no room
under the default equivalence margin, so the native confirmation raises
`effect.equivalence_margin` to the smallest two-decimal margin strictly above
one plus that resolution (`resolution-v3-baselines.txt`, line *replaced
equivalence_margin*; `addendum-v3-baselines-confirmation.json`, `effect`).

## Results

### Native baselines

The tables' § Cells: `v3-r1-baselines-confirmation` holds every native cell's
speedup, interval and outcome, and § Cells:
`2026-09-07-c7113c5a-polynomial-confirmation` the v1 confirmation's. gf2 is
faster at 4 and 9 words and in both `internal-` cells; gf2x is faster in every
larger cell, single-core, streaming and multicore. Each v3 interval lies on the
same side of one as its v1 counterpart; the public-API cell has no v1
counterpart.

### Host targeting

The tables' § Cells: `v3-r1-host-targeting-confirmation` holds the family's
confirmatory cells, and § Host-targeting ladder sets them beside the native
legs. `fail` means gf2 is faster; `pass` means gf2x is faster by more than the
family's material-gap threshold (`effect.material_gap_threshold` in
`addendum-v3-host-targeting-confirmation.json`). Host targeting changes neither
direction nor order of magnitude: at each size every leg's interval, the
native leg's included, lies on the same side of one. The ladder's *gf2 over
native* column shows gf2 nearly insensitive to the build level, since its
kernels are selected at run time. Its *gf2x over native* column shows gf2x's
four-word basecase fastest under `-O2` and its 256-word Toom-3W slowest there
(rows `poly-mul-4w-conservative-1core` and `poly-mul-256w-conservative-1core`).
These cross-level ratios are explanatory; the native legs are the native
family's `not-confirmatory` cells.

### What accounts for the gaps

Pointers here name rows of tables § Cost per schoolbook word product:
`v3-r1-baselines-confirmation` by cell ID and of § Derived estimates:
`v3-r1-baselines-confirmation` by row ID. gf2's scalar schoolbook costs nearly
the same per word product at 64, 256 and 2048 words, as a quadratic algorithm
with a fixed inner cost predicts, while gf2x's normalised rate falls with size,
the signature of subquadratic recursion (rows `poly-mul-64w-1core`,
`poly-mul-256w-1core` and `poly-mul-2048w-streaming-1core`). A hardware word
product costs far less than a scalar one, in the sequential raw batch and less
again in the YMM 4-limb kernel (`raw-batch-word-product`, `ymm4-word-product`,
`scalar-word-product-256w`). Their ratio, the instruction factor
(`instruction-factor-256w`), is the larger multiplicative part of the 256-word
gap; what remains after dividing it out (`residual-gap-256w`) is the share
attributable to gf2x's algorithms, and it grows at 2048 words
(`residual-gap-2048w`). Under 6, 12 and 24 workers gf2's aggregate throughput
rises more than gf2x's (`throughput-6-workers-gf2` against
`throughput-6-workers-gf2x`, and likewise for 12 and 24), which narrows the gap
from the single-core cell to the multicore cells (tables § Cells:
`v3-r1-baselines-confirmation`, rows `poly-mul-256w-1core`,
`poly-mul-256w-6core`, `poly-mul-256w-12core` and `poly-mul-256w-24smt`, where
the twelve-core interval is the widest). At 4 and 9 words gf2 performs more
word multiplications than gf2x yet wins: it pairs them into 256-bit VPCLMULQDQ
where gf2x issues 128-bit PCLMULQDQ (`wide256-kernel-pairs-16-products`,
`wide571-kernel-81-products`, `gf2x-mul4-karatsuba`, `gf2x-mul9-thirty`,
`gf2x-mul1-pclmul`); this is a source-level account, not a profile.

### Separated reduction and adapter costs

Each arm reports costs outside its timed windows, one value per execution;
tables § Costs: `v3-r1-baselines-confirmation` gives their medians with
intervals and states what each probe timed. The `BarrettReducerWide`
reduction a `Gf2mWide` consumer adds (rows `poly-mul-4w-1core` and
`poly-mul-9w-1core`, column *unpack*) costs more than either library's
unreduced product for GF(2^256) and GF(2^571) (the same rows of § Cells:
`v3-r1-baselines-confirmation`), and it is the larger share of a composed field
multiplication on either arm (§ Derived estimates, rows
`reduction-share-4w-gf2`, `reduction-share-4w-gf2x`, `reduction-share-9w-gf2`
and `reduction-share-9w-gf2x`), so gf2's unreduced lead is the minor term of a
wide-field multiplication.

The receipt's dot-product reduction values (§ Costs, row
`internal-gf2m-dot-1024-1core`, column *unpack*, marked `superseded`) do not
measure the reduction and stay in the receipt unchanged: both arms' probe
reduced the dot product's already reduced output, which `reduce_with_clmul`
returns at its early exit (`barrett-early-exit`). The dot product's single
reduction receives the raw XOR accumulator (`dot-product-xor-accumulates`,
`dot-product-single-reduction`). No public gf2 path exposes that accumulator
(`dot-product-accessors-crate-private`), so the diagnostic
`v3-dot-reduction-diagnostic` rebuilds it from the same per-element products,
checks that gf2's batch kernel and gf2x yield the same value and that it
reduces to both arms' dot products, and times its reduction on the timed
cell's operands in repeated processes (tables § Dot-product reduction
diagnostic states the fixture, sample and resampling; raw values in its
`diagnostic.jsonl`). The diagnostic supersedes the receipt as the reduction
measurement. With the PCLMULQDQ multiply `simd_dot_product` passes
(`dot-product-clmul`, `bundle-clmul`, `x86-clmul-pclmulqdq`) the reduction
costs several times the superseded early exit (rows `consumer` and
`superseded`); with the scalar `barrett::clmul` the gf2x arm composes it costs
more again (row `composed`). Each is a small fraction of its own arm's timed
call (rows `consumer-share` and `composed-share`), so the arms' different
multiplies leave the cell comparing the product-and-accumulate stages.

The setup, pack, batch-fill, dispatch and gf2 raw-batch unpack values are
single passes in a fresh process, with first-touch and cold-code effects, and
several did not time what their field names; the tables state what each timed
and list them as descriptive medians, and no conclusion rests on them. The
round-1 rework's arm source measures each field as documented, the dot-product
reduction included; no campaign has run it.

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
public-versus-dispatched comparison is the exploratory v1 pilot cell (tables §
Cells: `2026-09-07-c7113c5a-polynomial-pilot`, row
`poly-mul-4w-public-api-pilot`, baseline `gf2-native-schoolbook`, candidate
`gf2-native`, outcome `pilot`), which found the public path far slower than the
dispatched kernel at 4 words. The v3 `poly-mul-4w-public-api-1core` cell finds
gf2x faster than the public path (§ Cells: `v3-r1-baselines-confirmation`,
`not-confirmatory`); the public path's median over the dispatched kernel's, a
quotient of two cells with an interval that resamples both, is in § Derived
estimates: `v3-r1-baselines-confirmation`, row `public-api-over-dispatched-4w`.
The fix is tracked as `1c602857`, which uses this cell as its pre-change
baseline; the survey changes no production code.

## Protocol-v1 history

The v1 pilot and confirmation, their snapshots and summaries are unchanged. The
v1 confirmation is accepted with `fail` cells, which gf2 wins, and long-product
cells recorded `unstable` by v1's pooled flagging rule (tables § Cells:
`2026-09-07-c7113c5a-polynomial-confirmation`, columns *Outcome* and
*Flagged*); recounted under v3's per-execution rule no window of any v1 cell is
flagged (column *Per-execution*), and the current evaluator reproduces every v1
verdict, decision and outcome (`v1-reevaluation.txt`). The v1 evidence cannot
serve v3: its resolution pilot names another family and its producing closure
bound neither the gf2 crates nor the arm sources. The v1 confirmation's pinned
`Cargo.lock` snapshot, left uncommitted on the v1 branch, is now committed. An
interrupted v1 confirmation launch and an abandoned pilot launch, recorded only
in `/tmp`, are preserved in `../../bench_results/c7113c5a/v1-launch-history/`;
neither shows a runner announcement, and the origin record states why neither
holds a ledger line.

## Limits and follow-up

The native family's v3 cells are not confirmatory, and its confirmatory v1
evidence carries the `unstable` labels described above. gf2x runs its shipped
`x86_64_pclmul` tuning tables; a host-retuned gf2x is unmeasured. No cell
measures a cold cache; the 2048-word streaming cell rotates the protocol's
fixture banks (`FIXTURE_BANKS` in tuning-campaign-support), which does not show
eviction from any cache level. The toolchain is Rust 1.95; the v1 receipts used
1.97. The family addenda name gf2x 1.3.0; its commit, flags and digests live in
the plan and receipt arm records and the validation report. No absolute
optimum is claimed. The public-API fix is tracked as `1c602857`. Candidate
follow-ups, which REQ-05 excludes from this survey, go to `53c5a8c0`
(carry-less multiplication crossovers and polynomial competitiveness), whose
measurements would decide them: hardware word products beyond 9 words
(`instruction-factor-256w`); a subquadratic product for the residual
(`residual-gap-256w`, `residual-gap-2048w`); and a profile of
`BarrettReducerWide`, which dominates GF(2^256) and GF(2^571) multiplication
(the `reduction-share-` rows). A gf2x dependency is not recommended: gf2x is
GPL-3.0-or-later in this configuration and gf2 is MIT.

## Criterion status

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET | The accepted v3 receipts (tables § Campaigns) pin contract, protocol, addendum, producing closure and ledger prefix; no production change needs before/after; every negative and not-confirmatory outcome retained; v1 history preserved |
| REQ-02 | MET | `survey/gf2x-build-evidence-v3.txt`, `survey/gf2x-stock-build-probe.txt`, `survey/source-evidence.json` |
| REQ-03 | MET | Operation mapping above; `survey/validation-v3.json`; wide-field reductions in tables § Costs and § Derived estimates; the dot-product reduction in tables § Dot-product reduction diagnostic, which supersedes the receipt's early-exit values; adapter costs as descriptive single-pass medians in tables § Costs |
| REQ-04 | MET | `v3-r1-baselines-confirmation` and `addendum-v3-baselines-confirmation.json`; `internal-` raw-batch cells |
| REQ-05 | MET | Matching build ladders on both sides; confirmatory `v3-r1-host-targeting-confirmation`; no gf2x dependency or algorithm change |
