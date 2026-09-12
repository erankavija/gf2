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
binary stays under the primary checkout's `.agents/ext/12fdeb5b/` and is
neither committed nor distributed; the repository commits only this survey's
own shim source.

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
decides nothing. The superseded smoke campaigns of the same addendum are
retained beside it under `-r1` to `-r3`, each one run before a harness change
that altered the arms or the launcher: the staging directory the launcher
resolves, the semantic type the srsRAN adapter's parameters carry, the
configuration grid, and the comparator trees the launcher defaults to. Every
one of them is accepted and every one holds its zero-comparison reservation in
the family ledger, so the accounting shows every attempt.

The [pilot](addendum-nr-encode-pilot.json) is frozen: eight exploratory cells
over both base graphs, both comparators, the build control and the identity
floor, at the protocol's confirmatory pair count. It is a window job and has
not run. **WAITING-ON-WINDOW.**

The confirmatory addendum is not frozen and cannot be: its
`measurement_resolution` must cite the committed pilot receipt by path and
digest, so it freezes in a later window after the pilot receipt is committed.
It will declare at most six confirmatory cells, the P-20 cap for a family with
no ledger history, spanning both base graphs, filler and filler-free codes,
both comparators and the build control. **WAITING-ON-WINDOW.**

The resolution rule the confirmation will use is `eda07788`'s: compute the
pilot's widest relative bootstrap half-width at the confirmation's corrected
alpha, not at the pilot's own, because the DVB-T2 family's use of the pilot's
own alpha was contradicted by a threefold wider confirmation interval.

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

## Reproduction

```
dev/bench_results/12fdeb5b/run-nr-encode-baselines.sh smoke|pilot|confirmation
```

The launcher resolves the staged comparator trees under the primary checkout
through the common git directory, so any worktree of this repository runs it
with no path of its own; `GF2_AFF3CT_ROOT` and `GF2_SRSRAN_ROOT` override that
default. It verifies the pins, builds the four arms, regenerates the source and
build evidence, runs the equivalence gate, projects the plan from the frozen
addendum and measures it as bounded checkpointed sessions under the CCX1
exclusive mutex. `dev/bench_results/12fdeb5b/summarize.py` regenerates the
tables from the committed records and receipts.

## Criterion outcomes

| Criterion | Standing | Evidence |
|---|---|---|
| REQ-01 | partially met, WAITING-ON-WINDOW | The smoke receipt is accepted with zero findings under protocol version 4 and pins the contract, protocol, schema and addendum by digest. The pilot and confirmation receipts follow in later windows. |
| REQ-02 | met | [build evidence](survey/build-evidence.json), [source-evidence ledger](survey/source-evidence.json), [plan](plan.md) |
| REQ-03 | met | [validation record](survey/nr-encode-validation.json), projected in the tables |
| REQ-04 | partially met, WAITING-ON-WINDOW | The pilot addendum is frozen and the smoke receipt is committed; the pilot and confirmation receipts are window jobs. Unavailable and non-equivalent arms are already recorded. |

## Citations

- [Cassagne2019] AFF3CT.
- [Srsran2026] srsRAN Project LDPC encoder and rate matcher.
- [ThreeGpp2017] 3GPP TS 38.212 V15.0.0.
