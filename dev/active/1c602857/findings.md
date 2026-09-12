# Public wide carry-less product: dispatch routing and its receipt

> **Diátaxis Type:** Explanation

Issue `1c602857`. The public long-product API of `gf2-core` reaches the same
capability-dispatched kernels as the crate's own wide-field arithmetic, through
one dispatch shared by public and internal callers. No multiplication algorithm
changed: the kernels already existed, and the routing is what this issue adds.

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 4](../f547c394/amendment-v4.md) govern the receipt this issue needs.
That receipt does not exist yet: the campaign is frozen and queued for a
benchmark window. This report states no measured value; each claim points at
the artifact that holds it.

## What the code does now

`gf2m::wide::clmul_wide_dispatch` is the single place a wide carry-less product
selects its kernel. `clmul_wide`, `clmul_wide_slice`, `Gf2mWide::mul_ref` and
the wide Barrett reducer all reach it. A host with PCLMULQDQ computes the
4-word and 9-word products in the `gf2-kernels-simd` kernels whichever of those
a caller used; every other width, and every host without the capability, runs
`clmul_wide_slice_portable`.

Two consequences change how callers and tests behave:

- The slice form XOR-accumulates and the kernels overwrite, so the dispatch
  carries a write mode. Accumulating at a dispatched width costs a scratch
  product and one XOR pass; the owned form pays neither. `clmul_wide_slice`'s
  rustdoc points a caller who wants a plain 4- or 9-word product at
  `clmul_wide`.
- `clmul_wide_slice` used to be the crate's independent scalar oracle. It
  dispatches now, so the in-crate reference multiplications, the conformance
  oracle and the benchmark's scalar-baseline arms call
  `clmul_wide_slice_portable` instead and stay independent of the path they
  check.

The lane witness and the forced-fallback switch that the conformance suite
reads follow `gf2-coding`'s `force_scalar_encode_kernels`. Both compile away
outside test and `test-support` builds, and the receipt's arm crate enables
`simd` alone, so nothing instrumented is ever measured.

## Evidence

| Question | Artifact |
|---|---|
| Does the public path reach the kernels, at every width, against an independent oracle, fallback included? | `crates/gf2-core/tests/clmul_wide_conformance.rs`, recorded in [validation.json](validation.json) |
| What did the contract look like before the routing? | [conformance-before-routing.txt](conformance-before-routing.txt) |
| What does the receipt measure, and under which frozen settings? | [addendum-v4-public-clmul-pilot.json](addendum-v4-public-clmul-pilot.json) |
| What produces the measurement? | [producing-inputs.json](producing-inputs.json), [run-public-clmul.sh](run-public-clmul.sh), [make-plan.py](make-plan.py), `arms/` |

The failing behavioural evidence preceded the routing: at the pre-routing
commit the public 4-word product reported the portable lane while the host
offered its AVX2 kernel, and the `Gf2mWide` multiplication test at the same
commit passed — the asymmetry the issue describes.

The polynomial-multiplication baseline survey (`c7113c5a`) recorded the call
paths that surfaced this issue and measured the public path against the
dispatched kernel at four words. That measurement is exploratory and is cited
as motivation only; it is not the before measurement this issue requires.

## The receipt, and the decision waiting inside it

The family measures the routed public path against the path it replaced. The
baseline arm calls `clmul_wide_slice_portable`, which is the pre-change public
function word for word: a zeroed destination followed by the accumulating
schoolbook. One executable serves both arms and selects its entry point from
`GF2_CLMUL_PATH`, so a measured ratio attributes to the entry point rather than
to two builds. Six cells: the owned form at the two dispatched widths, the
accumulating slice form at the smaller of them, and the dispatch decision at
three widths without a kernel.

It runs in two windows. The pilot fixes the measurement resolution; the
confirmation is frozen against the committed pilot receipt by
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which derives the
confirmation addendum from the pilot addendum, pins the receipt by path and
SHA-256, stamps the resolution and writes a derivation record.
`run-public-clmul.sh confirmation` refuses to run before that addendum exists
and carries a digest.

**The freeze script refuses any margin that does not strictly exceed one plus
the pilot's measurement resolution.** It checks all three of the addendum's
`effect` margins — `worthwhile_speedup`, `equivalence_margin` and
`material_gap_threshold` — so the binding one is whichever is smallest, and in
this family that is `equivalence_margin`, declared for the widths without a
kernel. A pilot resolution at or above `equivalence_margin - 1` therefore stops
the freeze until that margin is replaced, through the script's
`--equivalence-margin` and `--equivalence-rationale` options; the derivation
record names every replaced value.

That decision belongs to whoever freezes the confirmation, after the pilot
resolution is committed. It is a decision about what slowdown at a
non-dispatched width would outweigh keeping one product path instead of two —
not a decision about whether the routing won. Replace the margin with a stated
rationale, or accept that the non-regression cells cannot be confirmed at this
resolution and record that. Do not lower a margin to fit a result.

Production adoption follows the frozen non-regression rule, so nothing here
claims adoption before the receipt exists.

## A recurring defect class: the arm's wire shape

The untimed smoke through the real `benchmark-ab-runner` caught this arm
serializing `cold_calls` and `decoder` as explicit nulls where the runner skips
them when absent. The canonical round trip rejects that, and the arm died in
13 ms before producing a result line.

That is the same signature that killed three queued `6c6b09b1` pilots and cost
a whole benchmark window slot: an arm whose request type does not round-trip to
the runner's own bytes. Two independent surveys have now hit it, so it is a
defect class, not one survey's mistake. The child-v2 request type is a wire
contract, not a struct that merely has to decode: `deny_unknown_fields` plus
canonical re-serialization means every optional field needs the runner's own
`skip_serializing_if`, and field order must match. Code reading does not
establish this. Run the campaign end to end through the real runner with a
throwaway plan, and reach a result line from every arm, before queuing
anything.
