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

## The family spends its confirmatory attempt without a valid confirmation

Two confirmatory campaigns exist and neither yields a usable confirmation.

The first, `nr-encode-confirmation-12fdeb5b-20260913t064237z`, reserved the
family's six comparisons and both candidate identities, measured four of its six
cells at the confirmatory pair count, and was killed by its operator because its
addendum declared the resolution at the pilot's own alpha rather than under the
rule above. Its stage is preserved whole at
[`2026-09-13-12fdeb5b-nr-encode-confirmation-abandoned`](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation-abandoned/execution.log),
and
[`v4-abandoned-confirmation-attempt.json`](../../bench_results/12fdeb5b/v4-abandoned-confirmation-attempt.json)
records what it reserved, measured and spent. The protocol spends a reservation
whether or not a receipt follows: a crashed, interrupted or failed confirmation
spends its full reservation, and completion never removes or discounts one.

That reservation was then removed from the family ledger by hand, and the second
campaign, `nr-encode-confirmation-12fdeb5b-20260913t065640z`, reserved against
the shortened chain. On the restored ledger it is a second confirmatory
reservation for candidate identities that had already spent their one
protocol-v4 attempt, and the chain rejects it: its line repeats sequence 6 and
its predecessor still names line 5. **Its receipt is therefore not a valid
confirmatory attempt under P-22 and the one-attempt cap, and no claim in this
report rests on it.**

Its [receipt](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/receipt.json)
and
[acceptance summary](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation/acceptance-summary.md)
stay committed as measured data with this contradiction beside them. The
acceptance tool still reports the receipt accepted, because P-22 validates the
ledger prefix the receipt itself pinned and that snapshot is exactly the
shortened chain; the tool reads no live ledger, so its verdict does not reach
the defect. The "Void confirmation campaign" section of the
[tables](../../bench_results/12fdeb5b/tables.md) carries its cells under that
label.

Both candidate identities have now spent their single protocol-v4 confirmatory
attempt, so none of the six cells can be confirmed again under this protocol
version. A confirmation of this family needs a later protocol version, and that
is a separate issue's work, not a deferral inside this one.

## Measured outcomes

The family's baseline evidence is the accepted
[pilot](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-pilot/receipt.json).
Its cells are exploratory: each carries the `pilot` outcome, none yields a pass
and none decides adoption. Every figure below is a pointer to the "Pilot
campaign" section of the
[tables](../../bench_results/12fdeb5b/tables.md) and to the pilot's
[acceptance summary](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-pilot/acceptance-summary.md).

**srsRAN encodes this consumer faster than gf2 on every configuration the pilot
measures, by a ratio that grows with block length.** All four
`-gap-native-vs-srsran` rows read a speedup above one, the smallest base-graph-2
and the largest base-graph-1 configuration bounding the range, and each interval
lies clear of the family's material-gap threshold. The gap is a whole-consumer
one and includes each arm's representation conversion, which the "Per-arm call
time and adapter stages" table decomposes per arm; those medians carry no
interval. At exploratory standing this is an observation, not a confirmed
decision.

**AFF3CT splits.** `nr-enc-bg1-n2560-k2048-gap-native-vs-aff3ct` reads a speedup
above one, so AFF3CT leads on the larger base-graph-1 configuration.
`nr-enc-bg2-n256-k121-gap-native-vs-aff3ct` reads the decision `regressed`:
AFF3CT is slower than gf2 on the smallest base-graph-2 configuration, and the
tables' "gf2 faster by (gap cells)" column reports the reciprocal of that
interval. The void confirmation records the same cell as a `fail`; that
outcome is stated as the evaluator assigned it and is not reinterpreted, and it
supports no claim here.

**The identity floor and the build control.** The pilot's `-null-` cell reads
the noise floor between two launches of the same executable, and its
`-control-portable-vs-native` cell compares the conservative-portable build with
the native one and reads `inconclusive`. Neither supports a claim about gf2's
build targeting; the portable control is also the cell whose width sizes the
family's resolution.

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
- AFF3CT is slower than gf2 on the smallest base-graph-2 configuration. The
  pilot reads `regressed` there and the void confirmation records it as a
  `fail`; both stay committed under the outcomes the evaluator assigned.
- gf2 is behind srsRAN on every configuration this survey measures. That is the
  survey's principal observation, and it stands whether or not any later issue
  acts on it. It rests on exploratory cells, so it is not a confirmed decision.
- The family spends both candidate identities' single protocol-v4 confirmatory
  attempt without producing a valid confirmation. That negative accounting
  outcome stays in the ledger and in the attempt record rather than being
  cleared.

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
| REQ-01 | partially met | The smoke and [pilot](../../bench_results/12fdeb5b/2026-09-13-12fdeb5b-nr-encode-pilot/receipt.json) receipts are accepted with zero findings under protocol version 4, each pinning the contract, protocol, schema and its addendum by digest, and the family's baseline evidence rests on the pilot. Comparison validity is not met at confirmatory standing: the ledger was tampered with between the two confirmatory campaigns, so the second receipt is void (*The family spends its confirmatory attempt without a valid confirmation*) and both candidate identities have spent their one protocol-v4 attempt. Every negative outcome, including that accounting failure, is preserved. |
| REQ-02 | met | [build evidence](survey/build-evidence.json), [source-evidence ledger](survey/source-evidence.json), [plan](plan.md) |
| REQ-03 | met | [validation record](survey/nr-encode-validation.json), projected in the tables |
| REQ-04 | partially met | The [pilot](addendum-nr-encode-pilot.json) and [confirmatory](addendum-nr-encode-confirmation.json) addenda are frozen and every receipt, including the abandoned stage and the void one, is committed, over base-graph-1 and base-graph-2 block sizes and rates with conversion costs inside every timed call. Unavailable and non-equivalent arms are recorded in the ["Bit-exact equivalence outcomes"](../../bench_results/12fdeb5b/tables.md) table and in *Where the projects diverge*. The baseline receipts that stand are exploratory; no confirmatory receipt of this family is valid. |

## Citations

- [Cassagne2019] AFF3CT.
- [Srsran2026] srsRAN Project LDPC encoder and rate matcher.
- [ThreeGpp2017] 3GPP TS 38.212 V15.0.0.
