# Pre-draw validation preregistration v1

> **Schema-v2 output supersession.** This document remains the immutable v1
> scientific preregistration and producer-0 history. Its schema-v1 receipt
> destination below is historical and unused after the owner-authorized
> continuation. The current envelope and canonical output are defined by
> [`pre-draw-validation-v2-continuation.md`](pre-draw-validation-v2-continuation.md)
> and `dev/active/02b8137c/pre-draw-validation-v2-receipt.json`.

This document fixes the permanent-zero-fraction campaign's validation phase
before its first validation draw. Its machine-readable form is
[`pre-draw-validation-v1-preregistration.json`](pre-draw-validation-v1-preregistration.json),
which the runner consumes and whose SHA-256 the receipt records. That file is
the authority for every constant; this document states the rules that bind the
run and the evidence it must leave behind.

The binding protocol is the "Validation before campaign draws" section of
[`protocol.md`](/dev/simulation_results/permanent-zero-fraction/protocol.md).
This phase is an engineering gate. It can only prevent launch; it publishes no
scientific estimate and creates no campaign acceptance claim.

## Anchors and addresses

The validation family is the protocol's ten anchors: $q=3$ with
$n\in\{1,2,3,4\}$, $q=5$ with $n\in\{1,2,3\}$, and $q=7$ with $n\in\{1,2,3\}$.
The preregistration lists them in that address order and pairs each with the
exhaustive counts it must reproduce.

Every anchor draws from its preassigned validation address: root
`0x44534b2f00000001`, purpose `Validation` (tag 1), stream index 0, with the
field order and matrix order supplying the remaining address components. This
is the namespace already recorded in
[`exact-anchors.csv`](/dev/benchmarks/permanent_campaign/exact-anchors.csv).
Validation-purpose streams are disjoint by construction from the
`CampaignCell`-purpose streams the campaign arms draw; no validation step opens
a campaign-purpose address.

## Authorities

The preregistration records the content identity of four committed artifacts
and refuses to load when any digest disagrees with the bytes on disk:

- the frozen protocol;
- the frozen campaign manifest, whose backend union fixes the required backend
  set;
- the exact-anchor evidence, which is the independent cross-check authority for
  each anchor's zero count;
- the cell-exhaustive backend-selection equivalence evidence.

The preregistered per-anchor counts are cross-check authorities only. Each
anchor's exact test takes its null probability from the numerator and
denominator that the run's own exhaustive enumeration produces.

## Required checks per anchor

1. **Exhaustive agreement.** An independent enumerator visits all $q^{n^2}$
   matrices exactly once. The production permanent evaluator and its pooling
   path must reproduce the enumerated zero count exactly, and the production
   determinant evaluator must reproduce the finite-$n$ singular count exactly.
   Both comparisons are on integers with no tolerance.
2. **Backend agreement.** Every backend in the frozen manifest's union is
   compared per matrix against the independent oracle at each anchor its kernel
   domain covers. A backend outside that domain is recorded as unsupported. A
   backend inside it that cannot build or execute is recorded as unavailable and
   fails validation; it is never skipped.
3. **Deterministic regeneration.** Two fresh sampler instances regenerate
   exactly the first 1,024 matrices from the same recorded address. The
   `MatrixSampler` contract exposes no worker-count choice, so the protocol's
   second fresh serial pass applies and the receipt records that mode. The two
   passes must agree in address order and in every canonical row-major entry
   byte, one residue in $[0,q)$ per byte.
4. **Fixed-draw exact test.** The anchor draws exactly 400,000 matrices from its
   validation address and compares the sampled zero count with the enumerated
   probability under the probability-ordering exact two-sided binomial
   definition.

## Decision and stopping rules

The validation family has engineering budget 0.01 and Bonferroni level 0.001 per
anchor. An anchor passes exactly when its exact test value is greater than
0.001. Equality fails. The comparison is the one the exact test itself
implements on a logarithmic scale, so an underflowed floating-point probability
cannot change a verdict.

A failed anchor is not redrawn. The run preserves its evidence, blocks every
campaign arm, and requires a diagnosed correction and a new validation protocol
run. The journal enforces this durably: each address publishes a start marker,
fsynced together with its directory, before it opens a sampler. An address whose
start marker survives without a terminal record is preserved as an interruption
failure and is never reopened. Every anchor reaches a terminal record, so a
failure early in the order does not discard the remaining evidence.

An earlier failed component within one anchor stops that anchor before the next
component: an exhaustive disagreement stops it before regeneration, and a
regeneration mismatch stops it before any draw.

## Output

The run publishes one immutable receipt at
`dev/active/02b8137c/pre-draw-validation-v1-receipt.json`, in the
`ValidationReceipt` schema at version 1. Publication is atomic and
adopt-or-refuse: republishing identical evidence succeeds, and any difference is
refused rather than overwritten.

The receipt records, for every anchor, the complete stream address, the exact
enumerated counts, the per-backend comparison state, the determinant count, the
regeneration mode and digests, the sampled count, the runtime null numerator and
denominator, the exact test value and verdict, and the start and finish
timestamps. It records once, for the run, the preregistration identity and its
full content, the source-closure and producing-binary identities, the Rust
toolchain, the RNG identity, the hardware, the worker count, the invocation, and
the run's own timestamps. Every one of these is observed by the production path;
none is supplied by the caller.

The receipt also carries a before/after content inventory of the frozen campaign
directory
`dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829`,
captured durably before the first anchor and again after the last terminal
record. Any path or byte difference fails the run. The validation phase writes no
campaign shard, coordinator receipt, checkpoint, field summary, pooled summary,
or interpretation sidecar.

## Execution

```console
$ cargo +1.95.0 build --release --features hip --bin permanent_validation
$ ./target/release/permanent_validation \
    --preregistration dev/active/02b8137c/pre-draw-validation-v1-preregistration.json \
    --state-dir dev/active/02b8137c/validation-journal \
    --receipt dev/active/02b8137c/pre-draw-validation-v1-receipt.json \
    --workers N
```

Three build and invocation conditions bind the run, and the runner enforces the
first two before it opens any address:

- The producing toolchain is Rust 1.95.0. A build from another compiler is
  refused up front, because an address opened under a refused build could not
  be redrawn.
- The `hip` feature is enabled and the host has a usable accelerator device.
  The frozen manifest's backend union includes the accelerator, so an
  unavailable required backend fails validation. The runner proves every
  required backend on one constructed matrix first and refuses to start rather
  than failing an anchor that could then never be redrawn.
- A resumed run repeats the identical command from the identical clean source
  closure. The journal binds the run to its observed producer identity,
  including the argument tokens, and refuses to adopt a journal recorded under a
  different one. Resuming with a changed worker count or argument order is
  therefore not a redraw opportunity; it is a refusal.

Exit status 0 records a passing receipt, 2 a preserved failure, and 1 an error
that produced no verdict. `permanent_validation --verify-receipt PATH` re-reads a
committed receipt and revalidates it against this preregistration without
opening a sampler; that is the launch check consumed before the campaign arms
unblock.
