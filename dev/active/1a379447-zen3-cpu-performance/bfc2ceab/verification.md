# Caller-trusted kernel entry points: inventory and decision material (jit:bfc2ceab)

> **Diátaxis Type:** Research

This record covers the inventory of REQ-01 and the material for the choice
REQ-02 and REQ-04 depend on. It changes no production signature and adds no
production check. Every command runs from the worktree root; script paths are
relative to this directory. No command takes a timing window, the host lock or
an ignored test.

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
clean. The listing's banner carries the commit and time of its run, and
`make-instruction-counts.py` reads its instruction text alone;
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

## Checks run

- `python3 -B survey/make-instruction-counts.py --self-test`: exit 0.
- `cargo clippy --release --all-targets -- -D warnings` and `cargo fmt --check`
  in `survey/check-prototype` under `RUSTUP_TOOLCHAIN=1.95.0`: exit 0.
- `python3 dev/scripts/check-receipt-input-snapshots.py`: exit 0.
- `python3 dev/scripts/check-campaign-producing-closure.py`: exit 0.

The change holds Python, documents and one standalone crate under this
directory, and touches no file under `crates/` or `dev/tools/`; the workspace
build and `./scripts/cargo-ci.sh` run on the merged tree.
