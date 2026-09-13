# 5G NR rate-matched encoder baselines

> **Diátaxis Type:** Explanation

Survey for `12fdeb5b`. It measures and records; it changes no production
encoder. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) govern every timed cell; the
addenda name [version 4](../f547c394/amendment-v4.md) and each receipt is
evaluated under the version it pins. The [plan](plan.md) fixes the operation,
the grid and the campaign order. Every figure this survey publishes lives in
the generated
[evidence tables](../../bench_results/12fdeb5b/tables.md); this document states
none.

## Question

`eda07788` covers the inverse mapping, from channel LLRs onto the mother code.
The forward direction is this survey's question, and gf2's public surface
offers no bit selection of its own: its rate-matched encoder fuses the
transmitted-position gather into the encode. The comparable operation is
therefore the whole consumer, information bits in and rate-matched codeword
out.
The family description in the
[pilot addendum](addendum-nr-encode-pilot.json) is the authoritative operation
definition.

## Operation mapping

gf2 reaches the operation through `BlockEncoder::encode` on its rate-matched
code. Both comparators split it into two public halves that the adapters
compose: AFF3CT's quasi-cyclic encoder followed by its 5G puncturer, and
srsRAN's LDPC encoder followed by its rate matcher. Each claim about those
paths, with the file, line and verbatim text it rests on, is in the
[source-evidence ledger](survey/source-evidence.json).

The consumer representation is gf2's `BitVec`. Each adapter converts into its
project's representation and back inside its own timed call, so every
conversion is charged to the arm that needs it. AFF3CT takes one `int32_t` per
bit. srsRAN packs bits most significant bit first inside `uint8_t` words while
gf2 packs least significant bit first inside `u64` words, so its adapter's
conversion is a per-byte bit reversal. Modulation is one bit per symbol, where
TS 38.212 [ThreeGpp2017] Section 5.4.2.2 interleaving is the identity and
srsRAN's rate matcher reduces to selection plus packing; a higher modulation
order would add an interleave gf2 has no counterpart for.

## Comparator builds

AFF3CT [Cassagne2019] v4.7.0 is the MIT-licensed tree `c077a88b` pins, with its
`conf` submodule supplying the per lifting-set generator matrices the encoder
reads. Its 5G puncturer and fast quasi-cyclic encoder carry no intrinsic path,
so the selected backend is scalar.

srsRAN [Srsran2026] serves as a measured comparator here. `eda07788` records
its rate dematcher as unavailable because srsRAN's CMake configuration stops on
a host without MbedTLS. That still holds for the whole-project configuration
and it does not reach the LDPC encoder and rate matcher: the shim compiles
their translation units directly, and none of the layers that require MbedTLS
is on that closure. The shim reproduces srsRAN's own `"auto"` backend selection
and reports the backend it chose. srsRAN is AGPL-3.0-or-later, so the linked
binary stays in the git-ignored `.agents/ext/12fdeb5b/` of the checkout that
builds it and is neither committed nor distributed; the repository commits only
this survey's own shim source.

Revisions, licences, compiled units, compiler and toolchain versions, flags,
selected backends and arm executable digests are recorded, as the build
observes them, in the
[build evidence](survey/build-evidence.json).
`survey/nr-encode-build.sh` refuses a tree whose commit, submodule commit or
static-library digest differs from its pins.

## Equivalence

The [equivalence gate](survey/nr-encode-validation.json) encodes the same
message set through gf2 and through each adapter on every configuration and
compares the rate-matched codewords bit for bit. The message set is the
all-zero word, the all-one word and seeded random words, so an implementation
correct only on the all-zero codeword fails here. The gate runs before any
timed work, and each arm binary additionally refuses at setup a configuration
whose comparator dimensions differ from gf2's, so a non-equivalent
configuration cannot reach a timed cell.

The outcome per configuration and arm, the configurations and the code
parameters each project derives are projected in the
["Bit-exact equivalence outcomes"](../../bench_results/12fdeb5b/tables.md)
and "Configuration grid" sections of the tables. The grid spans both base
graphs, the rates TS 38.212 Section 7.2.2 assigns them, at least one lifting
size from each of the eight lifting sets of its Table 5.3.2-1, codes with and
without filler bits for both base graphs, and redundancy versions beyond 0.
Two pairs cross a lifting-size boundary on one information bit: the
base-graph-1 pair moves to the next lifting size and from a filler-free code to
one with fillers, at a rate high enough that AFF3CT's own rule also selects
base graph 1, and the base-graph-2 pair crosses the `560 < K <= 640` lifting
constant.

## Where the projects diverge

Three divergences decide which arms are timed and which are only recorded.

**Base-graph selection.** gf2 and srsRAN take the base graph from the caller.
AFF3CT derives it from the message length and the rate, so it cannot be asked
for a base graph its own rule rejects; on this grid that rule routes the
rate-1/2 base-graph-1 configurations to base graph 2. Those configurations are
recorded as non-equivalent on the AFF3CT arm with the differing parameter, and
srsRAN serves them.

**Lifting size for `560 < K <= 640`.** gf2 uses the lifting constant TS 38.212
Section 5.2.2 specifies for that range and AFF3CT uses the value of the
adjacent range, which selects a larger lifting size and a different mother
code. This reproduces `eda07788`'s finding on the forward direction and adds
the standard's own value as the arbiter: the ledger records both branches with
their verbatim source lines. The grid pins the divergence at its minimal
witness, a pair of configurations one information bit apart: at the last
message length of the lower range both projects agree, and at the first length
of the disputed range they select different lifting sizes. srsRAN is unaffected
because it takes the lifting size from the caller. The affected configurations
are recorded, not timed, on the AFF3CT arm.

**Redundancy-version starting offset.** srsRAN implements all four versions
through the TS 38.212 Table 5.4.2.1-2 shift factors. gf2 and AFF3CT carry no
redundancy-version parameter at all, which two committed negative searches
establish, so both implement the version-0 offset only. The non-zero
redundancy-version configurations are therefore recorded as non-equivalent on
srsRAN and unavailable on AFF3CT, and carry no cell. Their rows also show why
an all-zero-only validation would be worthless here: under a different starting
offset the all-zero message still produces gf2's codeword, and every other
message does not.

## Campaign order and standing

The order is smoke, then pilot, then freeze, then confirmation.

The [smoke](../../bench_results/12fdeb5b/2026-09-12-12fdeb5b-nr-encode-smoke/receipt.json)
is an accepted five-cell exploratory campaign that reaches a result line from
every arm the pilot names, on the largest and smallest configurations of the
grid. It is a functional check, not a performance result about gf2, and it
decides nothing.

Four earlier smoke campaigns of the same addendum stay committed beside it,
under `2026-09-12-12fdeb5b-nr-encode-smoke-r1` through `-r4`. Each is a
superseded functional check and none is a measurement of gf2 or of a
comparator: no figure in this survey or in the
[tables](../../bench_results/12fdeb5b/tables.md) comes from any of them, and
their exploratory cells decide nothing, exactly as the current smoke's do. Each
ran before a harness change that altered the arm executables or the launcher:
the build directory the launcher resolves, the semantic type the srsRAN
adapter's parameters carry, the configuration grid, and the comparator trees
the launcher defaults to. A superseded receipt stays committed under its own
directory rather than being replaced, so every family-ledger reservation names
a campaign whose receipt is present. All five are accepted with zero findings
and all five spend zero comparisons, so the accounting shows every attempt
without touching the family's error budget.

The [pilot](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-pilot/receipt.json)
is an accepted eight-cell exploratory campaign over both base graphs, both
comparators, the build control and the identity floor, at the protocol's
confirmatory pair count. Every cell carries the `pilot` outcome and decides
nothing; the campaign exists to size the family's measurement resolution.

The [confirmatory addendum](addendum-nr-encode-confirmation.json) declares six
of those eight cells. Six is the largest set P-20 admits for a family with no
ledger history: each bootstrap tail holds `bootstrap_resamples` times the
corrected alpha over two expected draws, and the rule requires twenty. The two
dropped cells are the identity controls, which answer a question about the
harness rather than attributing a comparator gap; the retained six are every
gf2-versus-comparator pair the pilot measured. The arithmetic, the rationale
and both lists are in
[`resolution-nr-encode-confirmation.txt`](resolution-nr-encode-confirmation.txt).

The resolution rule is `eda07788`'s: the pilot's widest relative bootstrap
half-width recomputed at the confirmation's corrected alpha, not at the pilot's
own, because the DVB-T2 family's use of the pilot's own alpha was contradicted
by a threefold wider confirmation interval. P-20's endpoint shift between seed
streams is taken alongside it and the larger of the two is rounded up.
[`pilot-resolution-nr-encode.txt`](pilot-resolution-nr-encode.txt) is that
derivation, the committed output of `eda07788`'s `bootstrap-resolution` over
this survey's pilot receipt. The freeze refuses a resolution below the width at
the pilot's own alpha, and refuses margins that do not strictly exceed one plus
the frozen value.

The [confirmation](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/receipt.json)
is accepted with no findings and does not qualify for production selection,
which is the standing a survey proposing no change should reach. Every declared
cell is measured at the protocol's confirmatory pair count, none reports an
unresolved setting, and none exceeds the flagged-window fraction. Its own widest
relative half-width, in the summary table of the "Confirmation campaign" section
of the [tables](../../bench_results/12fdeb5b/tables.md), sits far inside the
declared resolution, which the dropped portable-build control sized. It spends
the family's six comparisons and both candidate identities' single protocol-v4
confirmatory attempt, so these cells are not confirmable again under this
protocol version.

### The aborted attempt

An earlier confirmatory campaign, `nr-encode-confirmation-12fdeb5b-20260913t064237z`,
launched against an addendum that declared the resolution at the pilot's own
alpha rather than under the rule above. The executor aborted it for that
procedural defect before reading any of its results and voided it under the
[protocol's voided-attempt rule](../f547c394/protocol.md), so its reservation
does not enter the chain the confirmation reserves on and it spends no
comparison and no candidate attempt. Its stage is preserved whole at
[`2026-09-13-12fdeb5b-nr-encode-confirmation-abandoned`](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation-abandoned/execution.log),
where the executions of the four cells it reached remain available, and
[`v4-abandoned-confirmation-attempt.json`](../../bench_results/12fdeb5b/v4-abandoned-confirmation-attempt.json)
names the campaign, the addendum digest, the defect, the cells measured and
unmeasured, and the abort. No figure in this survey comes from it. The rule
reaches an attempt whose results are unread; an attempt whose results are read
spends its reservation, so it cannot retire a losing measurement.

## Measured outcomes

Every figure below is a pointer. The cell rows are the "Confirmation campaign"
section of the [tables](../../bench_results/12fdeb5b/tables.md) and the cell
entries of the
[acceptance summary](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/acceptance-summary.md);
the exploratory rows that sized them are the "Pilot campaign" section of the
same tables.

**srsRAN is ahead on every configuration measured, by a margin that grows with
block length.** All four `-gap-native-vs-srsran` cells pass: each interval lies
entirely above the material-gap threshold, and the smallest base-graph-2
configuration and the largest base-graph-1 configuration bound the range. The
gap is a whole-consumer one and includes each arm's representation conversion,
which the "Per-arm call time and adapter stages" table decomposes descriptively
per arm; those medians carry no interval and decide nothing.

**AFF3CT splits.** `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` passes: AFF3CT
is materially ahead on the larger base-graph-1 configuration.
`nr-enc-bg2-n256-k121-gap-native-vs-aff3ct` is a **`fail`**: the evaluator
records the decision `regressed`, meaning AFF3CT is materially slower than gf2
on the smallest base-graph-2 configuration. The tables' "gf2 faster by (gap
cells)" column reports the reciprocal of that cell's interval. The outcome is
recorded as the evaluator states it and is not reinterpreted.

**The identity floor and the build control are settled at exploratory
standing.** The pilot's `-null-` cell reads the noise floor between two launches
of the same executable, and its `-control-portable-vs-native` cell compares the
conservative-portable build with the native one and reads `inconclusive`.
Neither is confirmatory, so neither supports a claim about gf2's build
targeting; the portable control is also the cell whose width sizes the family's
resolution.

Every cell is a comparator-gap cell about a whole-consumer operation. None
measures a kernel, none proposes a change, and the family's
`worthwhile_speedup` stays undeclared, so no cell can qualify for adoption.

## Negative, unavailable and recorded outcomes

- No configuration where a comparator derives gf2's parameters failed the
  bit-exact gate. That is a negative result for the hypothesis that gf2's
  RREF-derived column mapping places message and parity bits differently from
  the standard's ordering: on this grid it does not.
- Redundancy versions other than 0 have no gf2 counterpart and are recorded,
  not timed.
- The configurations where AFF3CT's own derivation differs from gf2's are
  recorded, not timed, on the AFF3CT arm; the equivalence table names each one
  with the parameter that differs.
- Bit selection alone is not a cell. gf2 exposes no forward selection entry
  point, so timing a harness re-implementation would time the harness.
- The survey proposes no production change, so its addenda declare no
  worthwhile speedup and adopt nothing.
- One confirmatory cell is a `fail`: AFF3CT is materially slower than gf2 on
  the smallest base-graph-2 configuration. It stays in the confirmation and in
  the tables under the outcome the evaluator assigned.
- gf2 is behind srsRAN on every configuration this survey measures. That is the
  survey's principal result, and it stands whether or not any later issue acts
  on it.
- The aborted attempt keeps its stage and its record. Voiding it removes no
  measurement from the repository and creates no room for a second confirmatory
  attempt: the confirmation spends the candidates' one protocol-v4 attempt.

## Reproduction

```
dev/active/12fdeb5b/survey/freeze-addendum.py smoke|pilot <addendum>
dev/active/12fdeb5b/survey/freeze-addendum.py confirmation <addendum> <pilot-receipt-dir>
dev/bench_results/12fdeb5b/run-nr-encode-baselines.sh smoke|pilot|confirmation
```

The freezer's confirmation mode delegates to the canonical
`dev/active/c7113c5a/survey/freeze-confirmation.py`, supplying this family's
cell selection, its prose and the resolution derived at the confirmation's
corrected alpha; the survey owns no freezing logic of its own. The confirmation
launcher refuses an addendum, or a pinned pilot receipt, whose bytes differ
from the committed ones.

The launcher resolves the shared comparator source trees under the primary
checkout through the common git directory, so any worktree of this repository
runs it with no path of its own; `GF2_AFF3CT_ROOT` and `GF2_SRSRAN_ROOT`
override that default. Its own build output stays in the git-ignored
`.agents/ext/12fdeb5b` of the invoking checkout, which `GF2_12FDEB5B_EXT`
overrides, so two worktrees never write the same target directory. It verifies
the pins, builds the four arms, regenerates the source and build evidence, runs
the equivalence gate, projects the plan from the frozen addendum and measures
it as bounded checkpointed sessions under the CCX1 exclusive mutex. `dev/bench_results/12fdeb5b/summarize.py` regenerates the
tables from the committed records and receipts.

## Criterion outcomes

| Criterion | Standing | Evidence |
|---|---|---|
| REQ-01 | met | The smoke, [pilot](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-pilot/receipt.json) and [confirmation](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/receipt.json) receipts are accepted with zero findings under protocol version 4, each pinning the contract, protocol, schema and its addendum by digest. The `fail` cell and the exploratory `inconclusive` control stay recorded under the outcomes the evaluator assigned, and the aborted attempt keeps its stage and its record (*The aborted attempt*). |
| REQ-02 | met | [build evidence](survey/build-evidence.json), [source-evidence ledger](survey/source-evidence.json), [plan](plan.md) |
| REQ-03 | met | [validation record](survey/nr-encode-validation.json), projected in the tables |
| REQ-04 | met | The [pilot](addendum-nr-encode-pilot.json) and [confirmatory](addendum-nr-encode-confirmation.json) addenda are frozen and their receipts committed, alongside the aborted attempt's stage, over base-graph-1 and base-graph-2 block sizes and rates with conversion costs inside every timed call. Unavailable and non-equivalent arms are recorded in the ["Bit-exact equivalence outcomes"](../../bench_results/12fdeb5b/tables.md) table and in *Where the projects diverge*. |

## Citations

- [Cassagne2019] AFF3CT.
- [Srsran2026] srsRAN Project LDPC encoder and rate matcher.
- [ThreeGpp2017] 3GPP TS 38.212 V15.0.0.
