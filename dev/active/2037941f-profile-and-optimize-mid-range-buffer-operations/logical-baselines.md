# Logical-buffer baselines (jit:18a87159)

Paths beginning `survey/` are relative to this directory; all others are
repository-relative.

Three frozen logical-buffer families have current pre-change baselines on the
prepared Ryzen 9 5900X: the isolated XOR kernel, the public
`BitMatrix::row_xor` consumer, and the 5G NR rate-matched construction route.
This record states what each baseline establishes and points at the committed
bytes that hold every figure. No figure is copied here; each quantitative
statement names the table row or acceptance row that carries it, with its
sample count and interval.

The issue carries no `cites:` label, so this record cites no external registry
key.

## What a pilot arm pair measures

Every cell in all three families runs a byte-identical executable on both arms
(`arms` in each `receipt.json`: `public-xor-b`, `row-xor-b` and
`nr-construct-b` are described as the pre-candidate identity of their `-a`
baseline). A pilot therefore measures the baseline against itself. Its speedup
estimates are a dispersion measurement, not a candidate result, and the
acceptance tool records every cell's outcome as `Pilot`. The frozen addenda
leave `effect.measurement_resolution` null, which is why each acceptance row
carries the note `unresolved: effect.measurement_resolution`: the resolution
that a confirmation needs is derived from these pilots by
`dev/active/c7113c5a/survey/freeze-confirmation.py`, not declared ahead of
them.

## Isolated XOR baseline (REQ-01, REQ-02, REQ-03)

- Receipt:
  `dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/receipt.json`,
  with its raw pairs under `checkpoints/units/`, its append-only journal in
  `execution.log`, its frozen inputs under `inputs/` and its runtime
  provenance in the receipt's `host`, `toolchain` and `session_hosts` records.
- Verdict: the campaign is accepted and does not qualify for production
  selection, on the header line of
  `dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/acceptance-summary.md`
  (finding count on that file's `## Findings` section). Every one of its 17
  cells is an exploratory `Pilot` outcome in that file's `## Cells` table; the
  per-cell speedup intervals and relative half-widths are the rows of the cell
  table under the `v4-r1-2037941f-logical-isolated-xor` section in
  `dev/bench_results/2037941f/logical-tables.md`, each at 6 pairs and 0.975
  confidence.

**Semantic validation (REQ-02).** Exact XOR semantics hold for aligned and
offset cells: `survey/logical-harness-validation.txt` records a `PASS` line
per oracle case with its check count, covering the aligned `a64` and offset
`o8` layouts at 7, 8, 9, 63, 64, 65 and 66 words, and the bit-boundary cases
`xor-bits-0`, `-1`, `-63`, `-64` and `-65` that pin canonical bit indexing and
the zero tail. The same file records the harness contract test count. Each
cell's `selected_path` in the receipt carries the observed `src%64` and
`dst%64` of the run, so the offset a cell claims is the offset it measured.

**Dispatch-inclusive versus resolved (REQ-02).** The family measures the
public entry point; the profile session measures both. The `## Observed route`
table in `dev/bench_results/2037941f/logical-profile/profile-summary.md`
distinguishes the `public-xor-a` cases, whose path is
`kernels::ops::xor_inplace`, from the `resolved-xor` cases, whose path is
`kernels::ops::resolve_xor_inplace`. The instruction counts per call for the
matching 8-word and 9-word cases in that file's per-call cost and
instruction-mix table give the dispatch cost directly, each as a median over
9 repetitions with its order-statistic interval.

**Cost attribution (REQ-03).** The annotated release disassembly in
`survey/asm/logical-arm/` attributes cost as follows.

- Dispatch leaves no standalone function. `index.txt` records zero text
  symbols for `gf2_core::kernels::ops::xor_inplace`,
  `gf2_core::kernels::ops::resolve_xor_inplace` and
  `gf2_core::kernels::select_backend_for_size`; each matching `.asm.txt` file
  in that directory states that the routine is inlined into its callers rather
  than leaving a silent gap.
- The executed kernel is `gf2_kernels_simd::x86::avx2::avx2_xor_into`, one
  symbol of 412 bytes, whose vector-XOR and scalar-XOR instruction counts are
  rows of `survey/asm/logical-arm/instruction-mix.txt`. No AVX-512 body is
  present in the measured binary, which
  `survey/asm/logical-arm/avx512-xor-into.asm.txt` records.
- Sampled cost agrees: the `## Sampled symbol shares` table of the profile
  summary attributes the 8-word warm public case across the driver, the
  fixture and `avx2_xor_into`, each share a median over its stated repetition
  count.
- Cache traffic and bandwidth separate the warm and streaming regimes in the
  `## Memory traffic` table of the same file: the warm cases and the
  `xor-64w-a64-streaming` case differ in L1 load-miss rate, each with its
  interval over 9 repetitions.

No search over alternatives was run, so nothing here claims an optimum.

## Public row-XOR consumer baseline (REQ-04, REQ-05, REQ-06)

- Receipt:
  `dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/receipt.json`,
  with the same structure and the frozen matrix inputs under `inputs/`.
- Verdict: accepted, does not qualify, on the header line of
  `.../2037941f-logical-public-row-xor/v4-r1-pilot/acceptance-summary.md`; all
  17 cells are exploratory `Pilot` outcomes there. The per-cell intervals are
  the `### Cells` rows under the `v4-r1-2037941f-logical-public-row-xor`
  section in `logical-tables.md`, at 6 pairs and 0.975 confidence.

**Semantic validation (REQ-05).** `survey/logical-harness-validation.txt`
carries a `PASS row-xor-<stride>-<shape>` line for each declared boundary,
with its check count and the observed `stride`, `columns`, `base%64` and the
per-row `dst:src%64` pairs. The `full` and `tail63` shapes at 7, 8, 9, 63, 64,
65 and 66 words cover the declared boundary and the offset cases; the `tail63`
shapes, whose column count is one below a word multiple, are what pins zero
tail padding, and the bit-boundary XOR cases in the same file pin canonical
bit indexing. Each cell's `selected_path` in the receipt repeats the alignment
the run observed.

**Cost attribution and candidate signal (REQ-06).** For the 8-word warm row
case the profile summary's `## Sampled symbol shares` table splits observed
cost between `gf2_core::matrix::BitMatrix::row_xor` and
`gf2_kernels_simd::x86::avx2::avx2_xor_into`, alongside the driver's own
frames, each share a median over 9 repetitions. The public row routine
survives as one 604-byte text symbol (`survey/asm/logical-arm/index.txt`), and
its call, indirect-call, scalar-XOR and stack-slot counts are its row in
`instruction-mix.txt`, which is where row access and call overhead separate
from the vector body. Memory traffic and the streaming regime are the row
cases in the profile summary's `## Memory traffic` table.

Within the frozen search budget this pilot identifies no candidate signal.
Both arms are the same executable, and every cell's outcome is `Pilot` with
its resolution unresolved, so the acceptance summary records no cell whose
interval would support a candidate. The family's search budget allows four
pilot trials per cell and one confirmatory attempt per candidate
(`search_budget` in `campaigns/logical-public-row-xor.json`); one pilot trial
per cell is spent.

## Coding-route logical baseline (REQ-07, REQ-08, REQ-09)

The selected production route is the public 5G NR rate-matched constructor,
`gf2_coding::ldpc::nr_5g::<impl QuasiCyclicLdpc>::nr_5g_rate_matched`,
measured whole: lifting selection, construction, cloning, dispatch and output
observation are all inside the timed call, which each arm's description in the
receipt states.

- Receipt:
  `dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot/receipt.json`.
- Verdict: accepted, does not qualify, on the header line of
  `.../2037941f-logical-nr-construction/v4-r1-pilot/acceptance-summary.md`;
  all 6 cells are exploratory `Pilot` outcomes there, with intervals in the
  `### Cells` rows under the `v4-r1-2037941f-logical-nr-construction` section
  in `logical-tables.md` at 6 pairs and 0.975 confidence.

**Stride proof (REQ-07).** The stride is observed, not asserted.
`logical_buffer_harness::routes::observe_nr` derives `stride_words` from the
constructed code's own parity-check matrix as `cols.div_ceil(64)`, and
`verify_nr` refuses a cell whose observed lifting factor, dense shape and
stride differ from the frozen declaration. Each cell's `PASS nr-construct-...`
line in `survey/logical-harness-validation.txt` records the resulting `stride`
and the `band=8-64w` it falls in, and the same stride appears in the
`selected_path` of every execution in the receipt and in the selected-paths
table of `logical-tables.md`. The five distinct configurations cover
strides from the lower edge of the band to the middle of it; all lie inside 8
to 64 words.

**Determinism and canonical agreement (REQ-08).** Each validation line carries
the structure digest `h-sha256` of the constructed parity-check matrix,
computed by `structure_digest` over the sorted column indices of every row.
The same digest appears in the `selected_path` of both arms of every execution
in the receipt, so the seeded construction reproduced the canonical structure
on each of the 12 independently launched processes per cell and stayed
deterministic under the measured configuration. Each cell's frozen seed is in
the addendum and repeated in the receipt's case record.

**Cost attribution and the confirmation question (REQ-09).** The profile
summary's `## Sampled symbol shares` table attributes the 8-word warm
construction case. The constructor's own body dominates;
`SpBitMatrix::from_coo`, a hash-map insert, `to_edges` and
`SpBitMatrix::transpose` follow it; the logical-buffer work,
`BitMatrix::row_xor` and `avx2_xor_into`, sits at the bottom of that table,
and `avx2_xor_into` clears the report's percent limit in only 7 of the 9
repetitions, `swap_rows` in 4 and the inner `nr_5g` in 1. The constructor's
per-call cost, instructions per call and branch-miss rate are its rows in the
`## Per-call cost and instruction mix` table, and its L1 load-miss rate at the
two profiled sizes is its row in `## Memory traffic`; the larger
configuration's much higher miss rate is where the route's memory traffic
shows.

The answer to REQ-09 is therefore negative: **logical-buffer work in this
route is not large enough to justify candidate confirmation.** The shares in
that table bound what any change confined to `row_xor` or the XOR kernel could
remove, and they are small next to the constructor's sparse-structure and
allocation work. The evidence supports confirming a candidate against the
constructor's own hot path, not against its buffer XOR. `1/(1 - share)` over
those shares would be an Amdahl ceiling and an estimate; this record states
the conclusion from the recorded shares instead.

## Confirmatory budget for bc091474

P-20 (`dev/tools/tuning-campaign-support/src/receipt.rs`) marks a cell's
bootstrap endpoints unresolved unless
`bootstrap_resamples * corrected_alpha / 2 >= 20`. Here `corrected_alpha` is
the attempt's alpha divided by the number of comparisons, and
`tuning_campaign_support::trial_ledger::attempt_alpha` sets
the attempt alpha to `family_wise.alpha / (t * (t + 1))`, where `t` is the
number of ledger entries that reserve at least one comparison, floored at one.
Exploratory reservations carry zero comparisons and spend no budget.

Each of the three ledgers holds exactly one entry, the pilot recorded here,
with `comparisons: 0`:

- `dev/bench_results/2037941f/logical-isolated-xor-ledger.jsonl`
- `dev/bench_results/2037941f/logical-public-row-xor-ledger.jsonl`
- `dev/bench_results/2037941f/logical-nr-construction-ledger.jsonl`

So for each family the first confirmatory attempt has `t = 1` and an attempt
alpha of `0.05 / 2 = 0.025`, and with the frozen 10000 resamples P-20 holds
while `10000 * (0.025 / m) / 2 >= 20`, that is `m <= 6.25`. **Each of the
three families admits at most six confirmatory cells on its first attempt.**
No family is reduced below one cell, so none has to record an admitted-nothing
outcome. The NR construction family has six frozen cells in total, so its
whole cell set fits inside its budget; the isolated XOR and public row-XOR
families have seventeen frozen cells each, so a confirmation there selects at
most six of them.

A crashed, interrupted or failed confirmatory attempt spends its reservation,
so a second attempt in any of these families would have `t = 2`, an attempt
alpha of `0.05 / 6`, and room for at most two confirmatory cells.
