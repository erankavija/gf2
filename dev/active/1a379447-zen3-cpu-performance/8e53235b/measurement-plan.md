# Measurement plan: word-wise against bit-serial codeword assembly

> **Diátaxis Type:** Explanation

Issue `@/issue/8e53235b` measures one candidate, the word-wise assembly of
packed systematic BCH codewords, under the
[frozen pilot addendum](addendum-v4-codeword-assembly-pilot.json) and the
[frozen confirmation addendum](addendum-v4-codeword-assembly-confirmation.json)
of family `8e53235b-bch-codeword-assembly`. The
[measurement contract](../measurement-contract.md) and the
[shared protocol](../f547c394/protocol.md) at
[version 4](../f547c394/amendment-v4.md) govern every receipt. This page
states how the two arms are reached, which cells are frozen, what decides, and
what the recorded outcome does to the candidate.

## How the arms are reached

Both arms are routes of one executable, `consumer-arm` of the bit-storage
consumer harness (`04b85d10/survey/gf2-side`), built once from the tree each
receipt pins under `inputs/producing`; the plan selects a route through
`GF2_CONSUMER_PATH`.

| Arm | Workspace assembly in the pinned tree | Entry |
|---|---|---|
| baseline | `CodewordAssembly::BitSerial` | `BchCode::encode_batch_into`, or `encode_batch_family_into` in a family-pinned cell |
| candidate | `CodewordAssembly::WordWise` | the same entry on the same messages |

In the pinned tree `CodewordAssembly::BitSerial` is the default of every
workspace and the only assembly an entry point without a workspace uses, so
the baseline arm is the pinned pre-change consumer, and the candidate is
reachable only through `BchEncodeWorkspace::set_codeword_assembly`. The pair
differs in that selection alone: one build, one entry point, one message
batch.

The candidate copies the message as one packed run and places the parity as
shifted runs through `BitVec::write_bits`, the bit-run write this issue adds
to `gf2-core`.

Correctness precedes timing. In the pinned tree the `gf2-coding` unit tests
hold both assemblies to the same bits and the harness's `consumer-verify`
requires every family under both assemblies to write the reference family's
bits on the three declared mother codes; the launcher runs it before the first
timed session. The tree that adopts the candidate keeps the bit-serial writer
as the unit tests' reference: the assembly function is held to it at
dimensions and redundancies of 0, 1, 63, 64, 65, 127, 128 and 129 bits under
both layouts with zero tail padding checked, and every registered family is
held to the serial reduction followed by that writer at the batch lengths 0,
1, 63, 64, 65 and 129 on the boundary codes and the construction suite's
parameter points.

## Frozen cells

The addendum declares five whole-consumer cells, single-core and warm, all
exploratory in the pilot.

| Cell | Objective | Role in the decision |
|---|---|---|
| `assembly-bch-encode-current-m14-b256-1core` | improvement | decides: the consumer the 04b85d10 profile ranks ([findings](../04b85d10/findings.md), § Ranked candidate experiments, entry 1) |
| `assembly-bch-encode-current-m16-b256-1core` | non-regression | guard: the normal-frame mother code |
| `assembly-bch-encode-fold-m14-b256-1core` | non-regression | guard: the fold family pinned in both arms |
| `assembly-bch-encode-bitslice-m14-b256-1core` | non-regression | guard: the bit-sliced family pinned in both arms |
| `assembly-bch-encode-current-m8-b1-1core` | non-regression | guard: one message on the smallest declared code, where a fixed per-call cost shows most |

The conversion-share table of 1d4fd63d
([tables](../../../bench_results/1d4fd63d/tables.md), § Conversion share of
the whole BCH consumer) bounds what a faster bit-slice conversion gives the
same consumer; this family measures the assembly, which that table leaves
outside the conversion.

A first confirmatory attempt admits six comparisons under P-20, so the
confirmation retains all five cells; the addendum's family description
carries the arithmetic.

## Sessions

Every timed step is a line of the benchmark-window queue; no session runs
one.

1. **Pilot.** `run-codeword-assembly.sh window pilot` measures the five cells
   at the frozen pilot maximum, the confirmatory sample size.
   [`survey/pilot-plan-smoke.json`](survey/pilot-plan-smoke.json) is the
   shared smoke's record of that plan: one untimed dispatch per arm and cell.
2. **Confirmation.** `run-codeword-assembly.sh freeze` derives the
   [confirmation addendum](addendum-v4-codeword-assembly-confirmation.json)
   from the committed pilot receipt with the canonical freezer, and the
   [derivation record](confirmation-derivation.txt) states the resolution and
   the pilot pin. `run-codeword-assembly.sh window confirmation` measures it;
   [`survey/confirmation-plan-smoke.json`](survey/confirmation-plan-smoke.json)
   is the shared smoke's record of that plan.

## Decision

The addendum's frozen decision rule reads the confirmation's acceptance
summary.

| Outcome | What happens to the candidate |
|---|---|
| *adopt*: accepted receipt, every confirmatory cell records pass | The packed kernel assembles every codeword word-wise. `CodewordAssembly`, the `assembly` register field and the workspace setter are removed, the harness routes with them; the bit-serial writer stays as the reference of the unit tests. |
| *no change*: any other recorded outcome | Production keeps the bit-serial assembly. The word-wise function, `CodewordAssembly`, the register field, the setter and the harness routes are removed; the receipts and the family ledger stay as the recorded no-win, each cell reported as recorded. |
| pilot places the candidate behind the baseline on the deciding cell | A recorded no-win without a confirmation; the *no change* removal applies. |

Either outcome leaves one assembly behind the existing entry points and no
selectable parallel path. `BitVec::write_bits` stays under *adopt* and is
removed with its tests under *no change*.

## Outcome

[`tables.md`](../../../bench_results/8e53235b/tables.md), written by
[`survey/make-tables.py`](survey/make-tables.py) from the committed receipts
and the family ledger, is the authoritative statement of every measured cell;
its Source table carries each receipt's digest and acceptance finding count.

- **Pilot.** The receipt is accepted and exploratory (tables, § Pilot). Its
  execution log records every declared cell started, checkpointed and
  completed once at its declared pairs across two sessions, with a terminal
  `complete` record. Every cell's interval, the deciding cell's included,
  places the candidate ahead of the baseline, so the decision table's
  pilot no-win row does not apply.
- **Confirmation.** The addendum is frozen with every pilot cell
  confirmatory; the frozen resolution lies below both margins (derivation
  record). The receipt is accepted and qualifies for production selection
  ([acceptance summary](../../../bench_results/8e53235b/r1-codeword-assembly-confirmation/acceptance-summary.md);
  tables, § Confirmation). Its execution log records every declared cell
  started, checkpointed and completed once at its declared pairs with status
  `measured` across two sessions, with a terminal `complete` record. Every
  confirmatory cell, the deciding cell and the four guards, records the
  outcome `pass` with the decision `improved`.
- **Decision.** The *adopt* row applies. The packed kernel assembles every
  codeword word-wise through the existing entry points; `CodewordAssembly`,
  the `assembly` register field, the workspace setter and the harness routes
  are removed, and the bit-serial writer is the reference of the `gf2-coding`
  unit tests.

## Limits

The family holds one cache state, one worker, three mother codes and two
batch lengths. The parallel entry point reaches the assembly through its
workspaces and is covered by the unit tests, not by a timed cell. The
launcher, the plan projection and the smoke script name the candidate routes
of the pinned harness; the adopting tree's harness carries one assembly and
no such route, so they reproduce the campaigns only on the tree the receipts
pin.
