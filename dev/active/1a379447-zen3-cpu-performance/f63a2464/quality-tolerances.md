# Predeclared quality tolerances for the candidate families

> **Diátaxis Type:** Reference

REQ-05 requires the quality tolerances before the quality evidence. This
document is committed before any candidate decodes a frame of the measured
cells; the [decision record](decision-record.md) fixes the search budget and the
[numerical-contract review](numerical-contract-review.md) fixes what each family
declares.

The [frozen protocol](../f547c394/protocol.md) supplies the statistics: the
independent quality sampling unit is the frame, the marginal BER interval is the
frame-bounded Hoeffding interval and the marginal FER interval is Wilson at the
frozen quality confidence, and a fastest-quality-compatible arm is certified
only by the protocol's paired bound. None of those is redeclared here.

## The certification rule

`fer_ratio_max = 1.10`. A candidate is quality-compatible with the canonical
decoder on a cell when the protocol's paired bounded-mean upper bound over the
cell's frames is at most zero at the family's corrected alpha, with the
candidate and the canonical arm decoding the same frames of the same recorded
bundle.

**Rationale.** The consumer benefit a candidate offers is throughput or
single-frame latency at the same link budget. A tenth more frame errors is
within the spread a tenth of a decibel of implementation margin covers on the
waterfall of both measured codes, and is small enough that a candidate that
buys throughput by decoding materially fewer frames cannot pass. The rule is
symmetric in the sense that matters: a candidate with lower frame errors passes
trivially, and one with higher frame errors must have that excess bounded away
by the frames themselves rather than by a wider tolerance.

**What the rule does not permit.** A bound that fails to certify is not evidence
that the candidate is worse; it is insufficient evidence, and it is recorded as
that. No cell's tolerance is revised after its bound is read, and a cell whose
bound is positive records `quality-incompatible` with its counts preserved.

## The bit-exactness rule for family QC

Family QC declares the canonical numerical contract unchanged, so its tolerance
is not a frame-error ratio. Every codeword position's posterior, after the same
iteration count on the same input, must carry the same `f32` bit pattern as the
canonical decoder's. One differing bit pattern on one position of one frame
withdraws the bit-exactness claim, and the candidate is then a changed contract
governed by the certification rule above.

This rule applies to the scalar path and to the AVX2 check kernel separately,
because a kernel that agrees with the canonical decoder on ordinary channel LLRs
and parts from it on a signed zero or a NaN has a different contract from the
one declared.

## The exploratory screen

The screen decides which of a family's configurations reach a timed pilot at
all, inside the search budget the decision record freezes. It is descriptive and
is not a certification: an admitted configuration has shown nothing about its
population frame error rate.

A configuration is admitted when, on every measured cell of the family:

1. it introduces no frame error on a frame the canonical arm decodes with no
   information-bit error, and
2. its aggregate information-bit error count over the cell does not exceed the
   canonical arm's.

A configuration that fails either condition is recorded with its counts and does
not reach a timed cell. A family whose whole configuration budget fails the
screen stops under stop rule S2 of the decision record.

## The measured cells

The two workloads are the recorded bundles `dvb-t2-r12-waterfall` and
`nr-bg1-z384-mother` that `c077a88b` froze and `3be770d5` and `07ca8585`
measured: one parity-check matrix, one set of transmitted codewords and one set
of recorded channel LLRs each, pinned by digest, at the Es/N0 and frame count
their manifests declare, with all-zero and encoded random codewords both
present. Scoring is over codeword positions `0 .. k-1`, the information window
`c077a88b` scores, so `bits` is `frames * k` with no punctured or filler
denominator substitution.

Four cell kinds are declared per family:

- **recorded** — the bundle exactly as frozen, every frame.
- **punctured** — the NR bundle with the first `2 * Z` systematic positions at
  the zero LLR, which is the mandatory systematic puncturing of the NR rate
  matching [ThreeGpp2017]. It applies to every frame, because puncturing
  constrains no transmitted bit's value.
- **filler** — the NR bundle's all-zero frames with the last `Z` systematic
  positions at the finite filler magnitude the rate-matched path uses. It
  applies only to the all-zero frames, because a filler position is a bit the
  rate matching forced to zero and only those frames carry zero there. The cell
  declares its reduced frame count rather than borrowing the bundle's.
- **difficult** — the subset of the recorded cell's frames the canonical arm
  leaves with at least one information-bit error at the iteration cap. Each
  candidate's outcome on exactly that subset is reported with its own frame
  count; the subset is defined by the canonical arm alone, so it is the same
  subset for every candidate of every family.

Mixed convergence is not a separate cell: the recorded bundles sit at the
waterfall of their codes, so the iteration distribution of every cell carries
frames that stop early, frames that reach the cap and frames between, and each
cell reports that distribution beside its counts.

## Stopping and iteration reporting

Every cell decodes under syndrome stopping at the iteration cap the frozen
decoder contract of `c077a88b` declares. Family L's unit is a layer sweep rather
than a flooding iteration; its counts are reported as sweeps and are not
compared with flooding counts.
