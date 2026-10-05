# Caller-trusted kernel entry points: inventory and decision material (jit:bfc2ceab)

> **Diátaxis Type:** Research

This record covers the inventory of REQ-01, the material for the choice REQ-02
and REQ-04 depend on, the checked form of every entry point compiled under
`--cfg gf2_entry_checked`, its contract test (REQ-03) and the measurement
family that prices it. A build without the cfg emits the instruction text it
emitted before the checked form existed. Every command runs from the worktree
root; script paths are relative to this directory. No command here takes a
timing window, the host lock or an ignored test; the family's timed stages are
queue lines of the benchmark window.

## Documents

- [`inventory.md`](inventory.md) and [`inventory.json`](inventory.json): the
  entry points, their conditions, where a debug build checks each, the
  unchecked access each guards, and every caller with how it meets the
  condition.
- [`decision-table.md`](decision-table.md): one row per pointer type, grouped
  by cost class, with both options, the instruction counts and the
  recommendation.
- [`measurement-plan.md`](measurement-plan.md): the candidate before/after
  cells for the checks that stay on a hot path.

## Review record and generators

[`survey/review.py`](survey/review.py) holds the reviewer's reading as data:
the source claims, a verdict for each safe function that holds an `unsafe`
block, the conditions and witnesses of each entry point, the assignment of
every use of a published pointer field, and the recommendations.

| Command | Writes |
|---|---|
| `python3 -B survey/make-source-evidence.py` | [`survey/source-evidence.json`](survey/source-evidence.json) |
| `python3 -B survey/regen-prototype-listing.py` | [`survey/check-prototype.asm.txt`](survey/check-prototype.asm.txt) |
| `python3 -B survey/make-instruction-counts.py` | [`survey/instruction-counts.json`](survey/instruction-counts.json) |
| `python3 -B survey/run-witnesses.py` | [`survey/witness-runs.json`](survey/witness-runs.json) |
| `python3 -B survey/make-inventory.py` | the three documents above and `inventory.json` |

`make-inventory.py` states under which conditions it fails before writing; its
`scan_rule` and `use_rule` fields state what the coverage means. Two
consecutive runs of the four record generators leave `git diff --exit-code`
clean, and so do two runs of `regen-prototype-listing.py`, which removes the
banner lines that carry the commit and time of a run.
`make-instruction-counts.py --self-test` checks the counting rule on a
fixture.

`python3 dev/scripts/verify-source-evidence.py survey/source-evidence.json
--project gf2 --frame tree` exits 0 with class `tree`. The ledger tracks the
tree: a change to a cited kernel or caller source is followed by a rerun of the
generators.

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
a product that wraps there, so a caller whose dimensions overflow passes it.
The release profile sets no overflow check (`release-profile`); a debug build
panics on the product.

A condition on values alone is outside the inventory: a non-canonical residue,
a composite modulus or a wrong Barrett constant yields a wrong field element
through arithmetic that no memory access depends on. The one value condition
inside it is the PLE pivot, which indexes the inverse table.

## Witness calls

[`survey/check-prototype/examples/witness.rs`](survey/check-prototype/examples/witness.rs)
calls an entry point through its published safe pointer with arguments that
break its condition; `run-witnesses.py` runs each call in its own release-build
process and records how the process ends. The `Witness call` column of
`inventory.md` holds the outcome per entry point, and the record's
`entries_without_a_witness` field lists the rest, the dense entry points whose
recorded witness tuple holds an operand of at least $2^{32}$ elements. Such a
call has undefined behaviour, so an outcome describes one build on one host
and a clean exit would not show a call to be sound.

## Prototypes and instruction counts

[`survey/check-prototype`](survey/check-prototype/src/lib.rs) is a standalone
crate outside the production workspace. For each pointer type a `*_current`
function forwards to the published pointer and a `*_checked` function runs the
check first, in the form of `m4rm::assert_gray_build_lengths`. The listing is
the shared `regen-asm.sh` output for those functions under Rust 1.95, and
`instruction-counts.json` holds the counts under its `rule` beside the loop
bodies of the kernels a committed listing records.

Three limits apply to the counts. A count is no timing. The dense prototypes
repeat the length comparison the kernel makes with a wrapping product,
so they overstate what replacing that product adds. The PLE index check
belongs inside the kernel, at the pivot read; the `pivot inverse read` pair
shows that read alone, and the `entry scan` pair the alternative that keeps the
whole check in the safe entry point.

## Checked form under `gf2_entry_checked`

Each entry point has a checked wrapper beside the wrapper it publishes today:
a named assertion of `gf2_kernels_simd::entry_check`, in the form of
`m4rm::assert_gray_build_lengths`, ahead of the same `unsafe` kernel call.
Under `--cfg gf2_entry_checked` the detection function publishes the checked
wrapper; the two Gray-PAM wrappers hold the checked product as a statement
under the cfg, ahead of the length assertion that compares with it. The `Checked form` column of `inventory.md` names each. No file under
`src/x86` changes, so no kernel listing changes.

Every check stands ahead of the kernel's loops. The sparse row check takes
the largest column index of the row once per call. The PLE check is the
hoisted form: the window shape, an inverse table of at least `p` entries and
one pass that requires every window entry below `p`; the kernel keeps every
entry below `p`, so each pivot then indexes inside the table and the kernel's
unchecked read stays as it is. The dense wrappers form the three shape
products with `checked_mul` ahead of the kernel's own assertions.

### Default build

`python3 -B survey/make-default-build-comparison.py compare` writes the
[comparison](survey/default-build-comparison.json) of the functions a release
build of the kernel crate emits without the cfg, between the
[anchor tree](survey/kernel-anchor-baseline.json) and the tree with the checked
wrappers, under `crate_functions.RULE`. Its `function_count` and
`differing_function_count` total the step; every row carries
`instruction_text: same`. The script's docstring states how the two records
are made.

### Contract test (REQ-03)

`crates/gf2-kernels-simd/tests/entry_conditions.rs` is compiled under the cfg
and holds one test per pointer type. Each test calls every implementation the
type publishes with arguments that meet the conditions and then with arguments
that break each condition, and requires a panic that opens with the kernel's
name and names the condition. The product of two slice lengths that the
Gray-PAM wrappers check wraps only for operands of at least $2^{32}$ elements,
so their test covers the length assertion the product feeds and a unit test of
`entry_check::checked_cells` covers the overflow branch.

`RUSTFLAGS="--cfg gf2_entry_checked" ./scripts/cargo-budget.sh --test cargo
nextest run -p gf2-kernels-simd --cargo-profile ci-test --profile ci -E
'binary(entry_conditions) | test(entry_check)'` exits 0.

### Shared suites in both builds

`survey/run-suites.sh` runs the shared `dev/scripts/run-kernel-suites.sh` once
without a flag and once with `RUSTFLAGS="--cfg gf2_entry_checked"`; each log
opens with its command and closes with its exit status, and `rustflags.txt`
beside the logs states the flags.

| Build | `simd` | without `simd` |
|---|---|---|
| current | [log](survey/test-logs/current/simd-build.txt) | [log](survey/test-logs/current/scalar-build.txt) |
| checked | [log](survey/test-logs/checked/simd-build.txt) | [log](survey/test-logs/checked/scalar-build.txt) |

## Measurement family

[`measurement-plan.md`](measurement-plan.md) states the family; its frozen
pilot addendum is
[`addendum-v4-entry-check-pilot.json`](addendum-v4-entry-check-pilot.json),
written by `survey/make-addendum.py`.

- Arms. Both arms are `survey/entry-check-arm` built from one tree with one
  command line; the candidate adds `RUSTFLAGS="--cfg gf2_entry_checked"` and a
  target directory of its own. `survey/make-plan.py` records the flags in each
  arm's `rustflags` field, the runner digests each executable, and the
  launcher's log states both digests and flag sets. Each execution reports the
  entry-point form observed from the linked kernel crate and refuses a form
  other than the plan's; the `selected_path` rows of
  [`survey/pilot-smoke.json`](survey/pilot-smoke.json) show `entry-form=current`
  on the baseline arm and `entry-form=checked` on the candidate arm of every
  cell.
- Shared forms. The runner, its smoke, the acceptance tool, the campaign log
  checker, the producing-manifest writer, the shared smoke script, the
  canonical freezer and the plan projector `dev/scripts/campaign_plan.py` are
  shared; `survey/make-plan.py` declares the family's two arms and its case and
  calls the projector, as each lane family's `make-plan.py` does. The arm
  package is the family's own, over `tuning-campaign-support`.
- Plan projector. `campaign_plan.py` gains the general entry `project`, which
  takes the two arms of each cell from the family, so a pair may be two
  executables with their own RUSTFLAGS; the lane-comparison entry `main` is a
  caller of `project` and takes its environment variable names as arguments
  with the GF(2^8) names as defaults.
  `python3 -B survey/make-plan-projection-comparison.py` projects every frozen
  addendum of the two lane families through the anchor projector and through
  the generalised one with fixed arguments and writes the
  [comparison](survey/plan-projection-comparison.json); its `plan_count` and
  `differing_plan_count` total it, and every row carries `plan_bytes: same`.
  The two families' smoke records are not regenerated: a smoke plan carries
  the absolute lock path of the worktree that ran it. Their producing
  manifests are regenerated by their own generators and gain the kernel
  crate's new source file.
- Launcher. `dev/bench_results/bfc2ceab/run-entry-check.sh` has the actions
  `build`, `window pilot|confirmation` and `freeze`; `window` refuses outside
  a benchmark window and refuses a campaign input that differs from its
  committed bytes. Its `build_binaries` function compiles the runner and the
  acceptance tool into `target/bfc2ceab-tools`, a target directory only this
  launcher writes, and `window` runs `runner check` on the projected plan
  before the first session, so a campaign uses the tools this tree builds.

## Checks run

- `python3 -B survey/make-instruction-counts.py --self-test`: exit 0.
- `cargo clippy --release --all-targets -- -D warnings` and `cargo fmt --check`
  in `survey/check-prototype` under `RUSTUP_TOOLCHAIN=1.95.0`: exit 0.
- `python3 dev/scripts/check-receipt-input-snapshots.py`: exit 0.
- `python3 dev/scripts/check-campaign-producing-closure.py`: exit 0.

- `cargo clippy -p gf2-kernels-simd --all-targets --all-features -- -D warnings`
  without the cfg and with it: exit 0.
- Both `--check` modes of the mid-range story's producing closures: exit 0
  after regeneration for the new kernel-crate files.
- `survey/smoke-arms.sh pilot` and `run-entry-check.sh build`: exit 0.
- `cargo fmt --all -- --check` and `./scripts/asm-artefact-present.sh`: exit 0.
- `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh`: exit 0 with every step
  reported `ok`, on the tree before the plan projector's generalisation; the
  merged tree's run covers the generalisation.
