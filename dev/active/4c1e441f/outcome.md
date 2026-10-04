# Outcome: the shipped GF(2^8) dense product against the matrix route

> **Diátaxis Type:** Explanation

Issue `@/issue/4c1e441f` measures the cached product-table route of
`field::matrix::gemm` against the route the library takes without it. The
question, the arms and the cells are in [the plan](plan.md); this page states
what the confirmation records and the decision the frozen rule yields. It
carries no measured value: every estimate, interval, pair count and window count
is in [the generated tables](../../bench_results/4c1e441f/tables.md), and every
verdict is in the confirmation's
[acceptance summary](../../bench_results/4c1e441f/r1-dense-product-confirmation/acceptance-summary.json).

## Verdict per cell

The acceptance summary records the confirmation receipt as `accepted` and as
qualifying for production selection; the summary's row in the tables' Source
section holds its finding count. Each confirmatory cell has one row in the tables' Confirmation section,
with its outcome, its pair count and its interval at the per-comparison
confidence of the family row above it, and one entry in the summary's `cells`
array, with its `status`, `decision` and `outcome`.

| Cell | Boundary | Representation | Recorded outcome |
|---|---|---|---|
| `matmul-n256-element` | kernel-isolated | runtime-context element | `pass` |
| `matmul-n512-element` | kernel-isolated | runtime-context element | `pass` |
| `matmul-n256-whole-element` | whole-matrix consumer | runtime-context element | `pass` |
| `matmul-n256-wide` | kernel-isolated | compile-time-configured value | `pass` |
| `matmul-n512-wide` | kernel-isolated | compile-time-configured value | `pass` |
| `matmul-n256-whole-wide` | whole-matrix consumer | compile-time-configured value | `pass` |

Every confirmatory cell records status `measured` and decision `improved`: its
whole interval lies above the improvement margin the summary entry carries under
`margins`. No confirmatory cell records `fail`, `not-material`, `regressed`,
`inconclusive` or `not-confirmatory`. The two whole-matrix consumer cells, whose
windows add both operand conversions and the output conversion to the product,
its scratch buffers and its restoration of the transposed operand, record the
same outcome as the kernel-isolated cells of their representation.

The two cells the confirmation drops, `matmul-n64-element` and
`matmul-n64-wide`, carry no confirmatory verdict. Their rows in the tables' Pilot
section hold exploratory estimates with outcome `pilot`, and
[the derivation record](confirmation-derivation-dense-product.txt) names them as
dropped with the frozen selection rationale.

## Decision

The frozen rule of both addenda retains the accelerated path when no
confirmatory cell records `fail` and at least one records `pass`, and reads the
acceptance summary's recorded outcomes and nothing else
([confirmation addendum](addendum-v4-dense-product-confirmation.json),
`family.description`). Applied to the outcomes above, the rule retains the
cached product-table route as the GF(2^8) dense-product path of `gf2-core`. The
production selection is the shipped one, so the decision changes no source.

The decision covers the measured shapes: both element representations, the
square dimensions the confirmation retains, warm cache, one core, on the host
the receipt's `host` field records.

## Direction agreement with the earlier matrix-family confirmation

The accepted matrix-family confirmation receipt of issue `19513245` is pinned
by path and SHA-256 in
[`pinned-matrix-confirmation.json`](pinned-matrix-confirmation.json). It
measured a prototype that rebuilt its table per product and did not write
results in place, so it is cited for its direction and contributes no sample to
this family.

Every row of the tables' "Direction agreement with the pinned matrix-family
confirmation" section reads `agrees`: for each confirmatory cell, the shipped
path and the prototype both place the table route ahead of the route without
it. The same rows show that the sizes differ between the two campaigns, which
the direction statement does not depend on.

## Lane and allocation witness

Each arm reports, after its timed windows, the lane the shipped witness
recorded, the number of product tables the process built, the route the
representation takes when the dispatch declines, and the allocating calls and
bytes one call of the cell's body makes. The tables' "Confirmation lane
witness" section holds one row per cell and position. In every row the baseline
arm names the declined lane with no table built and the candidate arm names the
product-table lane with one table built, so each pair compares the two routes
the family declares. The per-call allocation counts and bytes of both lanes are
in the same rows, which is where the design's RISK-03 scratch is observed.

## Campaign verification

Both campaigns are verified from their own execution logs with the shared
checker, which requires every declared cell to start, checkpoint and complete
once at its declared pair count with status `measured`, and the journal to end
in a terminal `complete` record:

```sh
for stage in pilot confirmation; do
    receipt=dev/bench_results/4c1e441f/r1-dense-product-${stage}
    python3 -B dev/scripts/verify-campaign-log.py --log "${receipt}/execution.log" \
        --receipt "${receipt}/receipt.json" --plan "${receipt}/plan.json"
done
```

The confirmation is one campaign identity over more than one session: its
journal carries a `paused` record at the session cell budget before the
terminal `complete`, and the summary's `sessions` and `resumed` fields record
it. No cell is measured twice; the resuming session journals each completed
cell as an omission.

The acceptance tool recomputes the committed verdict. Run on a copy of the
receipt directory, `benchmark-acceptance` writes an `acceptance-summary.json`
and `acceptance-summary.md` byte-identical to the committed ones:

```sh
copy=$(mktemp -d)
cp -a dev/bench_results/4c1e441f/r1-dense-product-confirmation "${copy}/"
target/release/benchmark-acceptance "${copy}/r1-dense-product-confirmation"
cmp "${copy}/r1-dense-product-confirmation/acceptance-summary.json" \
    dev/bench_results/4c1e441f/r1-dense-product-confirmation/acceptance-summary.json
```

The family ledger holds one reservation per campaign, each chained to its
predecessor's digest; the tables' Family ledger section lists them, and the
confirmation's line is the one whose `comparisons` equals the confirmatory cell
count. No attempt of this family is voided.

## Regenerating the tables

`python3 -B dev/active/4c1e441f/survey/make-tables.py` rewrites
[`tables.md`](../../bench_results/4c1e441f/tables.md) from the committed
receipts, acceptance summaries, family ledger and receipt pin its Source section
lists by digest. Two consecutive runs leave the file unchanged.
