# Kernel entry points with a caller condition: inventory, checks and cost (jit:bfc2ceab)

> **Diátaxis Type:** Research

This record covers the inventory of REQ-01, the checked form of every listed
entry point (REQ-02), its contract test and the behavioural suites (REQ-03),
and the measurement family that prices the checks (REQ-04). Every command runs
from the worktree root; script paths are relative to this directory. No
command here takes a timing window, the host lock or an ignored test.

## Documents

- [`inventory.md`](inventory.md) and [`inventory.json`](inventory.json): the
  entry points, their conditions, the check each holds, the unchecked access
  each check guards, and every caller.
- [`decisions.json`](decisions.json): the invoker's dated decision per cost
  class, which the generator reads.
- [`decision-table.md`](decision-table.md): one row per pointer type, grouped
  by cost class, with the decision, the instruction counts of the check and
  the interface's cost rows of both stages; for the sparse row class, the
  grains at which the check can stand, the P-20 arithmetic of a further
  confirmation and the measured cost of the per-product check.
- [`measurement-plan.md`](measurement-plan.md): the family, its cells and its
  committed stages.
- [`tables.md`](../../../bench_results/bfc2ceab/tables.md): each stage's cells
  with pair count, interval and outcome, the lane witness, the family ledger
  and the source digests with the acceptance finding counts.
- [`spmm-check-tables.md`](../../../bench_results/bfc2ceab/spmm-check-tables.md):
  the same for the stages of the family `sparse-product-check-cost`.
- [`confirmation-freeze-refusal-spmm-check.json`](confirmation-freeze-refusal-spmm-check.json):
  the refused freeze of that family's confirmation, as its launcher reports it.

## Review record and generators

[`survey/review.py`](survey/review.py) holds the reviewer's reading as data:
the source claims, a verdict for each safe function that holds an `unsafe`
block, the conditions and witnesses of each entry point, the assignment of
every use of a published pointer field, and the grains of the sparse row
check.

| Command | Writes |
|---|---|
| `python3 -B survey/make-source-evidence.py` | [`survey/source-evidence.json`](survey/source-evidence.json) |
| `python3 -B survey/regen-prototype-listing.py` | [`survey/check-prototype.asm.txt`](survey/check-prototype.asm.txt) |
| `python3 -B survey/make-instruction-counts.py` | [`survey/instruction-counts.json`](survey/instruction-counts.json) |
| `python3 -B survey/run-witnesses.py` | [`survey/witness-runs.json`](survey/witness-runs.json) |
| `python3 -B survey/make-tables.py` | `tables.md` beside the family ledger, through the shared `campaign_tables.py` |
| `python3 -B survey/make-spmm-freeze-refusal.py` | [`confirmation-freeze-refusal-spmm-check.json`](confirmation-freeze-refusal-spmm-check.json), from the launcher's `freeze` action |
| `python3 -B survey/make-spmm-tables.py` | `spmm-check-tables.md` beside the family ledger, through the shared `campaign_tables.py` |
| `python3 -B survey/make-inventory.py` | `inventory.md`, `inventory.json`, `decision-table.md` and `measurement-plan.md` |

`make-inventory.py` states under which conditions it fails before writing; its
`scan_rule` and `use_rule` fields state what the coverage means. Two
consecutive runs of the record generators leave `git diff --exit-code` clean,
and so do two runs of `regen-prototype-listing.py`, which removes the banner
lines that carry the commit and time of a run.
`make-instruction-counts.py --self-test` checks the counting rule on a
fixture.

`python3 dev/scripts/verify-source-evidence.py survey/source-evidence.json
--project gf2 --frame tree` exits 0 with class `tree`. The ledger tracks the
tree: a change to a cited kernel or caller source is followed by a rerun of the
generators. A claim about the tree the family measured resolves in the
producing snapshot of the confirmation receipt.

## Coverage of the inventory (REQ-01)

The inventory starts from the boundary scanner of the kernel safety-contract
inventory ([`7d44b71f`](../7d44b71f/verification.md)), run on the working tree.
Each safe function that holds an `unsafe` block outside test code is read
against the kernel it calls and is either an entry point or cleared with a
basis and a source claim; the `Counts` table of `inventory.md` holds the
totals. The pointer types the issue names are in the inventory under the
`sparse-row-index`, `ple-panel` and `packed-lengths` classes. The
`dense-shape-product` class is the reviewer's addition: these kernels assert
their shapes in release builds, and the assertion compares a slice length with
a product that wraps there, so dimensions that overflow pass the kernel's own
assertion. The release profile sets no overflow check (`release-profile`).

A condition on values alone is outside the inventory: a non-canonical residue,
a composite modulus or a wrong Barrett constant yields a wrong field element
through arithmetic that no memory access depends on. The one value condition
inside it is the PLE pivot, which indexes the inverse table.

## Checked entry points (REQ-02)

The decision is the invoker's, recorded in [`decisions.json`](decisions.json);
[`decision-table.md`](decision-table.md) states it per class.

- Packed, PLE and dense classes. Each safe entry runs a named assertion of
  `gf2_kernels_simd::entry_check`, in the form of
  `m4rm::assert_gray_build_lengths`, ahead of the `unsafe` kernel call, and the
  pointer type lists the conditions under Panics. The PLE check is the hoisted
  form: the window shape, an inverse table of at least `p` entries and one pass
  that requires every window entry below `p`; the kernel keeps every entry
  below `p`, so each pivot indexes inside the table. The dense entries form
  their shape products with `checked_mul` ahead of the kernel's own
  assertions; the two Gray-PAM wrappers hold the checked product as a
  statement ahead of the length assertion that compares with it.
- Sparse row class. The pointer of each lane is a whole-product entry
  (`SmallPrimeSpmmFn`, `MediumPrimeSpmmFn`): it checks the output shape, the
  value and index lengths and the largest column index once, then holds the
  row loop and the `unsafe` call of the row kernel, which the bundle also
  publishes as the `unsafe fn` pointer `spmm_row_unchecked_fn` with the column
  bound under Safety. `gf2-core` converts the operands and
  the product between the field type and the lane type around one call. The
  sparse matrix type holds the column bound in every constructor (the
  constructor table of `decision-table.md`), so the check does not fire for a
  product that starts at `SparseFieldMatrix::matmat`; the check stays because
  the hook that reaches the entry is a safe public trait method over slices.
- No file under `src/x86` changes, so no kernel listing changes.
  `gf2-core`, `gf2-algebra` and `gf2-coding` deny `unsafe_code`.

### Witness calls

[`survey/check-prototype/examples/witness.rs`](survey/check-prototype/examples/witness.rs)
calls an entry point through its published safe pointer with arguments that
break its condition; `run-witnesses.py` runs each call in its own release-build
process and records how the process ends. The `Witness call` column of
`inventory.md` holds the outcome per entry point, and the record's
`entries_without_a_witness` field lists the rest, the dense entry points whose
recorded witness tuple holds an operand of at least $2^{32}$ elements.

### Default build against the anchor

`python3 -B survey/make-default-build-comparison.py record after` and
`compare`, under Rust 1.95, write the
[comparison](survey/default-build-comparison.json) of the functions a release
build of the kernel crate emits between the
[anchor tree](survey/kernel-anchor-baseline.json), which precedes the checks,
and the working tree, under `crate_functions.RULE`. Its `function_count` and
`differing_function_count` total the step, which spans every change to the
crate since the anchor.

### Prototypes and instruction counts

[`survey/check-prototype`](survey/check-prototype/src/lib.rs) is a standalone
crate outside the production workspace. For each pointer type a `*_current`
function forwards to the published pointer and a `*_checked` function runs the
interface's check first, so the pair's listings differ by the instructions of
the check. The listing is the shared `regen-asm.sh` output for those functions
under Rust 1.95, and `instruction-counts.json` holds the counts under its
`rule` beside the loop bodies of the kernels a committed listing records.

Two limits apply to the counts. A count is no timing. The dense prototypes
repeat the length comparison the kernel makes with a wrapping product, so they
overstate what the checked product adds.

## Contract test and suites (REQ-03)

`crates/gf2-kernels-simd/tests/entry_conditions.rs` holds one test per pointer
type. Each test calls every implementation the type publishes with arguments
that meet the conditions and then with arguments that break each condition,
and requires a panic that opens with the kernel's name and names the
condition. The two sparse lanes share one contract through a macro. The
product of two slice lengths that the Gray-PAM wrappers check wraps only for
operands of at least $2^{32}$ elements, so their test covers the length
assertion the product feeds and a unit test of `entry_check::checked_cells`
covers the overflow branch.

`dev/scripts/run-kernel-suites.sh
dev/active/1a379447-zen3-cpu-performance/bfc2ceab/survey/test-logs` runs the
fast-tier suites that reach the kernel crate under Rust 1.95 with and without
the `simd` feature; each log opens with its toolchain and command and closes
with its exit status.

| `simd` | without `simd` |
|---|---|
| [log](survey/test-logs/simd-build.txt) | [log](survey/test-logs/scalar-build.txt) |

## Measurement family (REQ-04)

[`measurement-plan.md`](measurement-plan.md) states the family; its frozen
pilot addendum is
[`addendum-v4-entry-check-pilot.json`](addendum-v4-entry-check-pilot.json),
written by `survey/make-addendum.py`, and its frozen confirmation addendum
[`addendum-v4-entry-check-confirmation.json`](addendum-v4-entry-check-confirmation.json),
derived from the pilot receipt by the canonical freezer with the
[derivation record](confirmation-derivation-entry-check.txt) beside it.

- Evidence. Both stages are committed receipts under
  `dev/bench_results/bfc2ceab/`, and the family ledger beside them holds one
  line per stage. `python3 -B dev/scripts/verify-campaign-log.py --log
  <stage>/execution.log --receipt <stage>/receipt.json --plan
  <stage>/plan.json` exits 0 for the confirmation: every declared cell starts,
  is checkpointed and completes once at its declared pairs, and the journal
  ends in `complete`. The verdict, the outcome of each cell and the finding
  count are those of the stage's acceptance summary, which
  [`tables.md`](../../../bench_results/bfc2ceab/tables.md) projects.
- Measured tree. The family measures a tree that holds every entry point in
  two forms, selected by `--cfg gf2_entry_checked`; the two arms are one arm
  package built from that tree with and without the flag. Each receipt's
  `inputs/producing` snapshot holds that tree with the arm, the plan
  projection and the launcher. The live copies of the arm package
  (`survey/entry-check-arm`), `survey/make-plan.py`, `survey/smoke-arms.sh`,
  the two smoke records, `survey/producing-inputs.json` and the launcher are
  those producing inputs as the stages recorded them, except the arm's test of
  the form a build publishes, which is removed: the working tree holds one
  form of each entry point, so its two builds of the arm do not differ.
- Outcome. The confirmation records a regression for the per-row column check
  of both sparse row interfaces and no regression for the other retained
  cells; the rows are in the `Confirmation` section of the tables. The
  decision rule the confirmation addendum freezes refers the sparse row class
  to the invoker and records the measured check of each other class as an
  accepted measured cost. The regression contradicts the non-regression
  criterion the family freezes for the per-row form; the invoker's decision
  replaces that form by the per-product entry.
- Per-product entry. The family above cannot price it: its ledger admits no
  further confirmatory cell under P-20, and `decision-table.md` holds the
  arithmetic with the claims of the two rules it applies. The family
  `sparse-product-check-cost` prices it instead, as the next section states.
- Plan projector. The shared `dev/scripts/campaign_plan.py` has the general
  entry `project`, which takes the two arms of each cell from the family, so a
  pair may be two executables with their own RUSTFLAGS; the lane-comparison
  entry `main` is a caller of `project`.
  `python3 -B survey/make-plan-projection-comparison.py` projects every frozen
  addendum of the two lane families through the anchor projector and through
  the working tree's with fixed arguments and writes the
  [comparison](survey/plan-projection-comparison.json); its `plan_count` and
  `differing_plan_count` total it.
- Tables. `dev/scripts/campaign_tables.py` writes a family's tables with or
  without a receipt pin; a family without one gets no agreement section.

## Family `sparse-product-check-cost` (REQ-04, per-product entry)

The frozen pilot addendum is
[`addendum-v4-spmm-check-pilot.json`](addendum-v4-spmm-check-pilot.json),
written by `survey/make-spmm-addendum.py`; its family description states the
arms, the cells, the confirmation's selection, the resolution rule and the
decision rule. The decision table names the family's committed stages.

- Arms. Both arms are the one executable of
  [`survey/spmm-check-arm`](survey/spmm-check-arm/src/lib.rs), selected by the
  environment. The candidate is `SparseFieldMatrix::matmat`. The baseline is
  the dispatcher's row loop held in the arm over the kernel crate's unchecked
  row kernel, which each lane's bundle publishes as the `unsafe fn` pointer
  `spmm_row_unchecked_fn` with the column bound under Safety; the arm's
  `// SAFETY:` comment discharges it from the sparse matrix type's
  constructors. The arm's tests require both forms to equal a scalar
  reference on both lanes.
- Tooling. `survey/make-spmm-plan.py` projects the plan through the shared
  `campaign_plan.py`, `survey/make-spmm-producing-inputs.py` writes the
  producing manifest through the shared `campaign_inputs.py`,
  `survey/smoke-spmm-arms.sh pilot` drives both arms of every cell through
  the shared `benchmark-ab-runner smoke` and writes
  [`survey/spmm-pilot-smoke.json`](survey/spmm-pilot-smoke.json), and
  `dev/bench_results/bfc2ceab/run-spmm-check.sh` has the actions `build`,
  `window pilot|pilot2|confirmation` and `freeze`; `window` refuses outside a
  benchmark window and refuses a campaign input that differs from its
  committed bytes. `survey/make-spmm-pilot-pairs.py` sizes the second pilot
  trial from the first pilot's receipt and writes
  [`survey/spmm-second-pilot-pairs.json`](survey/spmm-second-pilot-pairs.json).
- Admissibility. A first confirmatory attempt of this family spends half the
  family alpha over its four cells, which P-20's tail-support bound admits;
  the pilot addendum's family description carries the arithmetic.
- Outcome. The family holds the two pilot trials its search budget admits and
  no confirmation: the `freeze` action refuses the receipt of the second
  trial, and `survey/make-spmm-freeze-refusal.py` records the refusal. The
  measured cost of the per-product check is therefore exploratory; the
  decision table holds each stage's rows and the load each receipt records at
  the start of its session. REQ-04 asks for "before/after evidence under the
  measurement contract" or a record of "the measured cost as accepted"; the
  family's rule records an accepted cost from confirmatory cells only, so the
  acceptance is the invoker's: the last ruling of
  [`decisions.json`](decisions.json) records the exploratory cost as accepted
  and closes the family without a further measurement.

## Checks run

- `python3 dev/scripts/check-family-producing-closures.py`,
  `python3 dev/scripts/check-receipt-input-snapshots.py`,
  `python3 dev/scripts/check-campaign-producing-closure.py` and
  `python3 dev/scripts/check-addendum-schema-versions.py`: exit 0.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check` and `cargo doc -p gf2-kernels-simd -p gf2-core
  --all-features --no-deps` with `RUSTDOCFLAGS='-D warnings'`: exit 0.
- `cargo clippy --release --all-targets -- -D warnings` and `cargo fmt --check`
  in `survey/check-prototype` under `RUSTUP_TOOLCHAIN=1.95.0`: exit 0.
- `python3 -B survey/make-instruction-counts.py --self-test`: exit 0.
- `./scripts/asm-artefact-present.sh`: exit 0, no SIMD source file changes.
