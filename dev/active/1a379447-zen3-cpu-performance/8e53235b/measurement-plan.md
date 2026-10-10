# Measurement plan: word-wise against bit-serial codeword assembly

> **Diátaxis Type:** Explanation

Issue `@/issue/8e53235b` measures one candidate, the word-wise assembly of
packed systematic BCH codewords, under the
[frozen pilot addendum](addendum-v4-codeword-assembly-pilot.json) of family
`8e53235b-bch-codeword-assembly`. The
[measurement contract](../measurement-contract.md) and the
[shared protocol](../f547c394/protocol.md) at
[version 4](../f547c394/amendment-v4.md) govern every receipt. This page
states how the two arms are reached, which cells are frozen, what decides, and
what each outcome does to the candidate.

## How the arms are reached

Both arms are routes of one executable, `consumer-arm` of the bit-storage
consumer harness (`04b85d10/survey/gf2-side`), built once; the plan selects a
route through `GF2_CONSUMER_PATH`.

| Arm | Workspace assembly | Entry |
|---|---|---|
| baseline | `CodewordAssembly::BitSerial` | `BchCode::encode_batch_into`, or `encode_batch_family_into` in a family-pinned cell |
| candidate | `CodewordAssembly::WordWise` | the same entry on the same messages |

`CodewordAssembly::BitSerial` is the default of every workspace and the only
assembly an entry point without a workspace uses, so production encoding is
what `main` encodes and the baseline arm is the pinned pre-change consumer.
The candidate is reachable only through
`BchEncodeWorkspace::set_codeword_assembly`, documented as a comparator that
no encode selects. The pair differs in that selection alone: one build, one
entry point, one message batch.

The candidate copies the message as one packed run and places the parity as
shifted runs through `BitVec::write_bits`, the bit-run write this issue adds
to `gf2-core`.

Correctness precedes timing. The `gf2-coding` unit tests hold both assemblies
to the same bits: the assembly functions directly at dimensions and
redundancies of 0, 1, 63, 64, 65, 127, 128 and 129 bits under both layouts
with zero tail padding checked, every registered family at the batch lengths
0, 1, 63, 64, 65 and 129 on the boundary codes and the construction suite's
parameter points, and every workspace, batch and parallel entry point. The
harness's `consumer-verify` requires every family under both assemblies to
write the reference family's bits on the three declared mother codes, and the
launcher runs it before the first timed session.

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
   confirmation addendum from the committed pilot receipt with the canonical
   freezer; `run-codeword-assembly.sh window confirmation` measures it in a
   later window.

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

## Limits

The family holds one cache state, one worker, three mother codes and two
batch lengths. The parallel entry point reaches the candidate through its
workspaces and is covered by the unit tests, not by a timed cell. The entry
points that take no workspace use the bit-serial assembly.
