# XOR unroll body removal verification (jit:fa1d8733)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. Script paths are relative to this directory. No
command takes a timing window, the host lock, or an ignored test.

## Baselines

Two trees are identified by content; `survey/locate.py` names them. Each
`survey/<stage>-baseline.json` holds the SHA-256 of every file of
`gf2-kernels-simd` in that tree, and `survey/inputs/<stage>/` holds the bytes
of each path the working tree changes, checked against its baseline digest on
use. `python3 -B survey/freeze-baseline.py <stage> <commit>` writes a baseline
and its snapshots; the commit id is an informational field that no generator
reads.

- [`before`](survey/before-baseline.json): the kernel source with the unroll
  bodies, its manifest with their `check-cfg` entry, and the module listing.
- [`after`](survey/after-baseline.json): the source and manifest without them,
  and the listing that
  `python3 -B dev/scripts/regen-asm-listings.py crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt`
  writes from that source in the commit that edits it.

The banner of each side's listing records rustc 1.95.0, an empty `RUSTFLAGS`
and the symbols `regen-asm.sh` extracts.

## Readers of the configurations (REQ-01)

`python3 -B survey/find-flag-readers.py` writes the
[reader record](survey/flag-readers.json): each file with a line matching the
record's `pattern`, with its class under the record's `rule`. The `before`
object covers the kernel package of the `before` baseline and `current` every
tracked file of the working tree; each carries its totals under `counts`. The
script exits nonzero while a `current` row has the class `workspace-source` or
`workspace-manifest`.

The `before` rows are the kernel source and its manifest. `current` has no row
of either class, and the script exits 0. Each reader has one disposition:

| Reader | Disposition |
|---|---|
| `avx2.rs` of `gf2-kernels-simd` | Removed: the guard against selecting both factors, the `XOR_UNROLL` constant of each factor and the configuration-gated loop of `avx2_xor_into`. |
| `Cargo.toml` of `gf2-kernels-simd` | Removed: the two `check-cfg` entries. |
| `tests/harness_contract.rs` of the logical harness | Changed: the plan-projection test passes its candidate arm's flags as an opaque string, and that string names no kernel configuration. |
| `run-candidate.sh` and `portfolio.md` of the candidate study | Kept byte for byte: the unroll pilot receipts pin both by digest. The [corrections](../../bc091474/unroll-variant-corrections.md) give their reading on a tree without the bodies. |
| Smoke plans, candidate disassemblies and profile host records of the candidate study | Kept: each records the flags of the build it describes. |
| `survey/source-evidence.json` of `fcb04d66` | Kept: the ledger describes the files at the commits its rows record. |
| Rows of class `receipt`, `receipt-snapshot` and `tracker` | Kept. The `receipt-snapshot` rows under this directory are the `before` bytes. |
| Generators, source ledger, route inventory and calibration plan of `63bad95d` | Changed: the generators declare neither the claim on the removed manifest entry nor a selector for the configurations, the plan's selector table has no row for them, and the regenerated ledger and inventory state the current tree. Its workload routes keep the flags the unroll pilot receipts record for their arms. |
| Documents of class `record` that describe the candidates | Updated; see [Documents](#documents-req-04). |

## Instruction text (REQ-02)

`python3 -B survey/make-asm-comparison.py` writes the
[assembly comparison](survey/asm-comparison.json) for the step from `before`
to `after`. Its `sources` list classes each package path whose bytes differ
under the record's `classifier_rule`: the kernel source is `code-differs`, its
manifest `other` and the module listing `assembly-listing`. Its `listings`
list splits each listing of either side per symbol and compares the
instruction text under the record's `comparison_rule`; `symbol_count` and
`differing_symbol_count` total the step, and the script fails when a symbol
differs. Every symbol row carries `instruction_text: same`. The listing with
`regenerated: true` is the module's; it differs from its predecessor in its
banner and in cargo's status lines, which the `before` listing holds and the
rule leaves out.

The module's listing names its kernels. The wrappers nested in `avx2::fns` and
every other function the crate emits are covered per function:
`python3 -B survey/make-crate-functions.py freeze <stage>` builds the package
with `--emit=asm` on a tree that holds that baseline's digests and writes the
digest of each function's instruction text under the record's `rule`, and
`make-crate-functions.py compare` checks both records against their baselines'
source digests and joins them in the
[function comparison](survey/crate-function-comparison.json). Its
`function_count` and `differing_function_count` total the step; every row
carries `instruction_text: same`. The build passes no `--cfg`, so the records
describe the default build.

## Safety contracts

The removed lines hold no `unsafe` keyword: the gated loop sat inside
`avx2_xor_into`, whose contract covers the remaining loop.
`python3 -B ../7d44b71f/survey/make-unsafe-inventory.py` exits 0 and rewrites
the [inventory](../7d44b71f/survey/unsafe-inventory.json) with the line each
boundary below the removal occupies; its `counts` are those of the record it
replaces. `cargo clippy -p gf2-kernels-simd --all-targets --all-features -- -D
warnings -W clippy::undocumented_unsafe_blocks` exits 0.

## Shared suites and committed evidence (REQ-03)

`dev/scripts/run-kernel-suites.sh <this directory>/survey/test-logs` runs both
rows and writes the logs. Each log opens with the toolchain and command and
closes with the exit status; its nextest `Summary` line carries the test
counts.

| Build | Result |
|---|---|
| `simd` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| without `simd` | Exit 0; [log](survey/test-logs/scalar-build.txt). |

The harness contract suite of the logical harness, run with
`cargo test --release --test harness_contract` on its own manifest, exits 0
with the changed flag string.

`python3 dev/scripts/check-receipt-input-snapshots.py` exits 0.
`git log --format= --name-only --grep='(jit:fa1d8733)'` lists the paths this
task's commits touch: none lies under a receipt directory, none is a trial ledger,
and the paths under an `inputs` directory are this task's `before` snapshots.

## Documents (REQ-04)

Each of these states that the unroll bodies exist in the pilot receipts'
producing-input snapshots only:

- the [mid-range findings](../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md),
  in its file table, its preserved outcomes and its residual gaps; its
  [tables](../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-tables.md)
  are regenerated for the digests of the documents below;
- the candidate study's [selection outcome](../../bc091474/pilot-outcome.md),
  [premeasurement record](../../bc091474/readiness.md) and generated
  [candidate tables](../../bc091474/candidate-tables.md), whose generator
  checks that each pilot receipt's source snapshot names its factor's
  configuration;
- the logical-buffer [no-adoption verdict](../fcb04d66/no-change-outcome.md)
  and its [verification record](../fcb04d66/verification.md).

The frozen portfolio and launcher keep their bytes, and the
[corrections](../../bc091474/unroll-variant-corrections.md) beside them carry
the statement.

## Gates

`./scripts/asm-artefact-present.sh` exits 0 at the commit that edits the
kernel source and reports the changed SIMD source covered by its listing; the
gate inspects the latest commit only and passes vacuously at later commits.
`CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` exits 0 from the worktree root
with every step reported `ok`.
