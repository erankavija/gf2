# Numerical contracts and MSRV intrinsic feasibility of the candidates

> **Diátaxis Type:** Explanation

REQ-03's review for `f63a2464`, written before any candidate prototype exists.
It states the canonical float contract on the six axes the criterion names, the
contract each family of the
[decision record](decision-record.md) declares against it, and what the
repository MSRV lets each family's AVX2 forms compile to. A family this review
rules out is ruled out with its evidence, not dropped.

Code claims are in [source-evidence.json](survey/source-evidence.json). The
feasibility artefact is
[feasibility.json](../../bench_results/f63a2464/intrinsic-feasibility/feasibility.json)
beside the emitted assembly and the behavioural-test log; it is written by
[record-intrinsic-feasibility.py](survey/record-intrinsic-feasibility.py) from
that run's own commands, and records only what those commands observe.

## The canonical float contract

The production decoder is `LdpcDecoder` over the canonical `EdgeLayout`, with
the shared reduction `min_sum_check_row`. Six rules fix its behaviour.

**Signed zero and ties.** A sign is taken by comparison against zero, so a
negative-zero message counts as positive and a NaN counts as negative. The
magnitude fold is `f32::min` from infinity, which skips a NaN, so an all-NaN
input set reduces to an infinite magnitude. Among equal smallest magnitudes the
strict comparison keeps the first as the smallest position, and an output takes
the second smallest exactly at that position, which is the leave-one-out minimum
including under a tie. The hard decision is a strict comparison, so a zero or
negative-zero belief decides to bit zero.

**Channel scaling.** None. A variable-to-check message starts at its variable's
channel LLR unchanged, and the decoder applies no scale of its own; the min-sum
variant's `alpha` or `beta` is the only multiplicative or additive constant in
the update.

**Saturation and clipping.** None inside the decoder. The float alphabet's only
clamp is the explicit caller-invoked `Llr::saturate`, and the only clipping in
the update is the offset rule's floor of the excluded magnitude at zero.

**Puncturing and fillers.** Not the decoder's concern: it decodes the mother
code over all `n` positions. The NR rate-matched path prepares the LLR vector
around it — every untransmitted position, punctured systematic or truncated
parity alike, enters at the zero LLR, and a filler position enters at the finite
filler magnitude. Both are ordinary channel LLRs to the decoder.

**Variable-node accumulation order.** The belief accumulates sequentially over a
variable's canonical layout slots in the parity-check matrix's column order,
starting from the channel LLR; each outgoing message is that belief minus the
edge's incoming message, so every message inherits the accumulated belief's
rounding and its order.

**Per-frame termination.** With early termination the decoder tests the syndrome
after the variable update of each iteration and stops at the first passing test;
the parity of a check is the exclusive or of its edges' hard decisions. Without
it the loop runs to the cap and the syndrome is evaluated once at the end.

## What each family declares

| Axis | Q (`i8`/`i16`) | L (layered f32) | QC (intra-frame f32) |
|---|---|---|---|
| Signed zero, NaN | No image: the alphabet has one zero and no NaN. Declared: the sign of a zero message is positive, matching the float rule's outcome on `+0.0` and `-0.0` alike. | Unchanged | Unchanged, on the comparison rule rather than the IEEE sign bit |
| Ties | Unchanged rule, on integer magnitudes | Unchanged | Unchanged |
| Channel scaling | New: `clip(round(scale * llr))` into the symmetric alphabet | Unchanged | Unchanged |
| Saturation, clipping | New: saturating add and subtract, symmetric clip | Unchanged | Unchanged |
| Puncturing, fillers | Zero and the filler magnitude map through the same scale; the filler magnitude clips when the scale places it above the limit | Unchanged | Unchanged |
| Accumulation order | Same sequential order, saturating at each step, so the order is load-bearing rather than incidental | Same order within a layer; the belief a layer reads carries the preceding layers of the same iteration | Same order per lane, lanes being distinct lifted checks |
| Termination | Unchanged syndrome rule on the integer hard decisions | Unchanged rule, tested per layer sweep rather than per flooding iteration | Unchanged |

**Q is a changed contract.** Nothing in it is a rounding of the float result, so
bit-exactness is neither claimed nor testable, and quality is the evidence.
Saturating accumulation makes the order of the variable-node sum observable: a
float sum reassociated differs in the last bits, a saturating integer sum
reassociated can differ by the whole clip excess. The prototype therefore keeps
the canonical order, and its scalar reference asserts it.

**The symmetric alphabet.** The `i8` alphabet is `[-127, 127]` and the `i16`
alphabet `[-32767, 32767]`, both excluding the type's minimum. This is a
contract clause, not an implementation convenience: `_mm256_abs_epi8` of the
type minimum returns the minimum, so a magnitude fold over an alphabet that
admits it produces a negative magnitude and the reduction loses its meaning. The
probe measures exactly this, in both directions.

**L is a changed contract.** A layered sweep reaches a different message state
from a flooding iteration, so its iteration counts are not comparable with
flooding counts and the protocol's rule against comparing unadjusted iteration
counts under different stopping contracts applies to the comparison. Quality is
evidence under the predeclared tolerance; bit-exactness is not claimed.

**QC is the unchanged contract, conditionally.** Lanes carry lifted positions of
one circulant block, which are distinct checks of the lifted code, so no
reduction inside a check is reassociated and no variable's accumulation order
changes. The condition is that the lane assignment is the lifted position and
never the edge index within a node; a prototype that vectorizes a node's own
edges would reassociate the belief sum and would be a changed contract. Under
the stated condition the candidate is bit-exact against the canonical decoder,
and REQ-05 tests that rather than asserting it.

## MSRV intrinsic feasibility

Every AVX2 form the three families need compiles at the repository MSRV, lowers
to the instruction the design assumes, and agrees with its scalar reference. The
probe crate is a standalone workspace with no dependency; each function is one
register-width operation of a candidate's inner loop, and each is paired with a
test over exhaustive or boundary-covering inputs, so a form that compiles but
computes something else fails at the MSRV rather than inside a prototype. The
feasibility record names the toolchain the run observed, the emitted assembly's
digest, the test command's exit status, and the distinct mnemonics the compiler
produced for each probe; the assembly itself is committed beside it.

The forms recorded are, for Q, saturating add and subtract in both widths, the
magnitude and two-minimum fold in both widths, the sign mask and its
masked negation, the symmetric clip, and the byte rotation a lifted block of
`i8` messages needs at a within-lane and a half-register amount; for QC, the f32
magnitude and sign extraction, the comparison-based sign rule of the canonical
contract, and the cross-lane lane rotation. L needs no intrinsic beyond the ones
a flooding f32 update already uses, so it has no separate feasibility question.

Two results are worth stating as contract findings rather than instruction
availability. The comparison-based sign rule lowers to a compare against zero,
so a QC-aware f32 kernel can follow the canonical contract exactly; extracting
the IEEE sign bit instead is what makes the existing AVX2 `boxplus_minsum_n`
kernel disagree with the scalar reference, a divergence `07ca8585` measures and
`@/issue/39cbde20` owns. And the byte rotation needs the cross-lane half-swap
before the within-lane align, because AVX2 has no cross-lane byte shuffle; the
record carries the instructions this composition actually produced at each of
the two amounts.

## Rulings

**Q proceeds, in both widths, without a matched external `i8` arm.** The
alphabet, its saturation and its intrinsic forms are feasible. The external
comparison is asymmetric and stays so: `c077a88b`'s capability screen records
that the pinned AFF3CT build [Cassagne2019] rejects `i8` for the horizontal-layered
INTER mode whose `i16` form it does measure, and it preserves the build failure
of srsRAN [Srsran2026] and OpenAirInterface [OpenAirInterface2026] on this host,
each of which decodes with `int8` messages upstream. So an `i8` candidate has an
`i16` external arm and no `i8` one, and that gap is recorded rather than filled
by substituting a different operation.

**L proceeds.** It changes no arithmetic the float contract does not already
perform, and `c077a88b` already pins and measures the matched external layered
arm it needs.

**QC proceeds on the NR workload and is unavailable on DVB-T2, with the
reason.** NR BG1 is lifted from circulant blocks, and a block places lifted
position `i` of a check block on lifted position `(i + shift) mod Z` of a
variable block [ThreeGpp2017]; that rotation is the operation the intra-frame
update performs, and the probe records its feasibility. DVB-T2 has no such
partition of its check rows [Etsi2015]: its information part groups columns into
blocks whose check indices advance by the code's step `q`, so a column block's
edges land on checks spread across the whole parity range rather than on one
check block, and its parity part is a staircase accumulator rather than a
circulant. A variable-major block form exists for its information columns, but
the check-node update — the dominant named decoder work in `3be770d5`'s lever
ranking after `07ca8585` — has no equal-degree check-block partition to
vectorize. The DVB-T2 QC cell is therefore declared unavailable with this reason
and carries no samples, which is what the measurement contract requires of an
inapplicable arm.

## Consequences for the prototypes

The prototypes of REQ-04 follow from these rulings: Q in two widths over both
workloads, L over both workloads, QC over the NR workload only. Each keeps the
canonical order its contract declares, each has a scalar reference path its
behavioural suite asserts against, and any AVX2 form lives behind a safety
contract in an isolated kernel. No production selection changes.
