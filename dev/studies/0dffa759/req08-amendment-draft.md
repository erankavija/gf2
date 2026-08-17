# REQ-08 amendment — archived $\mathbb{F}_7$ encoding decision

**Status: approved by the owner and applied verbatim (2026-08-17). The applied
amendment carries the owner-approved 2026-08-17 revision removing the numeric
republication of §6's weighted comparison. The archived file carries the note
and stubs below at the insertion points of §1.**

REQ-08 of JIT issue `0dffa759` requires that, where the study's receipts support
the three-plane $\mathbb{F}_7$ candidate, the archived $\mathbb{F}_7$ encoding
decision be amended at its source with the permanent-workload evidence and the
changed verdict. [`findings.md`](findings.md) §8 determines that the receipts do
support it. `dev/archive/` is permanent repository content and amending it is a
deliberate act that needs the owner's approval
([`../../active/0de41c82/plan.md`](../../active/0de41c82/plan.md):174). The
owner gave that approval on 2026-08-17 and this document records the approved
text and its insertion points.

## 1. Target file and insertion points

Target:
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/f10152f6/r2_f7_encoding_decision.md`.

Three edits, all additive. No existing sentence is reworded, deleted, or
renumbered; the original text is preserved unchanged beneath the note
(`@/inv/falsification-preserved`). This is the same shape the sibling archived
document already carries — the 2026-08-16 supersession note above
`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`
§2.5 — and it is deliberately modelled on it.

| # | Where | What |
| --- | --- | --- |
| A | Immediately above the `## 1. Summary` heading | The amendment note of §2 below |
| B | Immediately above the "**Candidate D (9.5× faster than A on add; 0.58× on mul).**" paragraph in §6 | The pointer stub of §3 below |
| C | Immediately above the `## 8. Recommendation` heading | The pointer stub of §3 below |

## 2. Amendment note (insertion A), verbatim

> **Amendment (2026-08-16) — §6's Ryser workload model does not describe the
> permanent workload, and on that workload Candidate D is the measured winner
> on the device.** The original text below is preserved unchanged rather than
> reworked (`@/inv/falsification-preserved`). What this amendment changes is
> the standing of the workload model in §6 and the scope of the verdict, not
> the ratification of Candidate A as the public packed $\mathbb{F}_7$ encoding.
>
> 1. **The weighting §6 rejects D on is falsified by measurement.** §6 states
>    that "*each Gray-code transition does 1 packed add/sub (column-sum update)
>    and ~`n−1` packed muls (row-product update)*", giving "*≈97% mul, 3% add*"
>    at $n = 36$. The $\mathbb{F}_7$ permanent kernels do not pay that: the
>    row-product reduction has an early zero exit, and the complete reduction
>    runs only on the nonzero branch, whose frequency is the exact marginal
>    $(6/7)^n$ — 0.157267 at $n = 12$, 0.045821 at $n = 20$, and 0.013350 at
>    $n = 28$, each confirmed inside a two-sided Wilson 95 % interval on 4 096
>    samples and again on 6.8–7.8 million timed operations
>    (`dev/studies/6c7fcb38/receipts.md` §9). At $n = 20$ the complete reduction
>    runs on 4.58 % of Gray steps, not on every one of them. §6's weighted
>    comparison and the margin it derives for A at $n = 36$ from that weighting
>    (§6) therefore rest on
>    a premise the permanent workload does not satisfy.
>
> 2. **Measured against each other on the permanent workload, on the device, D
>    leads A at four of five orders.** A preregistered receipt campaign runs
>    both encodings as $\mathbb{F}_7$ permanent kernels on `gfx1030` under one
>    execution mapping: `f7-lookup-table-control` is Candidate A — it retains
>    the canonical two-nibble table layout and uploads the public Rust
>    `Packed7` table byte arrays themselves
>    (`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:25-29`) —
>    and `f7-three-plane-permanent` is Candidate D. Composite throughput of D
>    over A is 0.4524, 2.0661, 20.5474, 168.1439, and 66.1022 at
>    $n = 12, 16, 20, 24, 28$ (`dev/studies/6c7fcb38/receipts.md` §4.2). Both
>    are equivalence-identical against the CPU oracle at every order, with zero
>    mismatches (§3), and D is exact above the sixteen-lane bound the packed
>    CPU kernel stops at (§14).
>
> 3. **The size of those ratios is confounded and the campaign says so.** Each
>    cell sizes its own batch from a single-matrix probe, so the ratios mix the
>    circuit effect with a device-parallelism effect spanning a factor of 5 833;
>    the $n = 12$ reversal in A's favour is itself a batch artefact, A running
>    5 833 matrices against D's 46 (`dev/studies/6c7fcb38/receipts.md` §4.5,
>    §17.5, §17.6). The one comparison at matched launch geometry is the
>    horizontal-product isolate, where D's reduction measures
>    $6.00 \times 10^{-10}$ s against A's $1.4005 \times 10^{-8}$ s at
>    $n = 24$; that campaign declines to order the two circuits from it, because
>    the partner branches are censored (§6.2, §17.8). The *ordering* of the two
>    encodings on the permanent workload is what the evidence supports; the
>    *magnitudes* are not decomposed.
>
> 4. **What this amendment does not change.** The measurements of §4 are CPU
>    measurements of a scalar Rust prototype on the Ryzen 9 5900X host, and
>    nothing in the accelerator study re-measures them: that study measures HIP
>    device kernels on `gfx1030` only, on one host
>    (`dev/studies/a9284086/receipt.md` §12.8). **Candidate A remains the
>    ratified public packed $\mathbb{F}_7$ encoding**, and `Packed7`
>    (`crates/gf2-algebra/src/packed/packed7.rs:211`) is unchanged. The
>    accelerator study's synthesis reaches the same conclusion on the same
>    grounds and defines the three-plane state as a permanent-internal kernel
>    representation under a named exception with a tracked convergence
>    condition, rather than as a second public representation
>    (`dev/studies/0dffa759/findings.md` §7).
>
> 5. **The re-decision clause.** §7 and §8 state that a re-bench "*is not a
>    re-decision authority for T19*". This amendment respects that: T19 shipped
>    with A and A stands. What is superseded is narrower and is stated in §6's
>    own terms — the workload model there does not describe the Gray-code Ryser
>    permanent as these kernels implement it, so §6's conclusion is scoped to
>    the model it assumes rather than to the permanent workload. §7's own
>    forecast that "*D should be revisited once the W4 SIMD kernel is up*" is
>    the revisit this amendment records, arriving from the device side rather
>    than the SIMD side.
>
> **This note scopes every restatement of the D-versus-A verdict in this
> document, wherever it appears** — §1's "*D fails both clauses; A wins*", §6's
> "*Hard-fallback rule: A wins*" and its plausible-workload-weighting table, and
> §8's recommendation — and each of those sites carries a pointer back here.
> What none of it disturbs is the measurement record itself: the per-element
> figures of §4 stand as measured, and the 41 correctness tests of §3 stand.
>
> Evidence: `dev/studies/6c7fcb38/receipts.md` §3, §4.2, §4.5, §6.2, §9, §14,
> §17.5–§17.8; `dev/studies/a9284086/receipt.md` §12.8; synthesis and its
> reasoning at `dev/studies/0dffa759/findings.md` §5, §7, §8.

## 3. Pointer stub (insertions B and C), verbatim

> *(See the 2026-08-16 amendment above §1: the Ryser workload model this
> conclusion rests on does not describe the permanent workload, and Candidate D
> leads Candidate A on that workload at four of five measured orders on the
> device. Candidate A remains the ratified public packed $\mathbb{F}_7$
> encoding.)*

## 4. What the owner approved

Adding the note of §2 and the two pointer stubs of §3 to a file under
`dev/archive/`. Nothing existing is edited. The amendment records a falsified
premise and a scoped changed verdict; it does not re-decide the public packed
$\mathbb{F}_7$ encoding, and no code, proof, or public type changes as a
consequence of it. The archived verdict's ratification of `Packed7` as
Candidate A stands, and the synthesis's representation-boundary choice
([`findings.md`](findings.md) §7) is consistent with it standing.
