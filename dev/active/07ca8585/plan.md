# Measurement plan for the min-sum update change

> **Diátaxis Type:** Explanation

How `07ca8585` measures the change the [findings](findings.md) describe. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [protocol](../f547c394/protocol.md) supply every rule; this plan states
only what is specific to this issue.

## Arms

The change is a production change, so a current pinned pre-change baseline is
owed. Both arms are the `3be770d5` steady-state harness built against the two
generations of `gf2-coding`: the **before** arm against the revision the
`before` claims of [source-evidence.json](survey/source-evidence.json) pin, and
the **after** arm against the changed tree. A candidate identity is the
executable's bytes, so each arm's digest is recorded before its first timed
call and every campaign refuses an arm whose digest differs.

The AFF3CT [Cassagne2019] arm is the one `3be770d5` measured, at the pinned
digest its
[build identity](../../bench_results/3be770d5/preparation/build-identity.json)
records. It is unchanged by this issue and is rebuilt only if absent.

## Families

| Family | Question | Arms | Decides |
|---|---|---|---|
| `ldpc-update-before-after-v1` | Does the changed update decode the frozen workloads faster than the path it replaces, at the same decisions? | gf2 before, gf2 after | Adoption |
| `ldpc-update-comparator-v1` | Where does the changed decoder sit against the compatible decoder, per REQ-10? | gf2 after, AFF3CT | Nothing; descriptive |

The decision family is the before/after one: its cells carry the frozen
worthwhile-effect and equivalence margins, and adoption follows them. The
comparator family publishes the REQ-10 comparisons and selects nothing, which
keeps the two questions in separate multiple-comparison families as the
protocol requires.

Both families keep an append-only ledger from genesis under
`dev/bench_results/07ca8585/`. Each family runs a pilot first; its confirmation
addendum is derived from the committed pilot receipt by the canonical freezer
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which pins the pilot by
path and digest, takes the resolution from the pilot's widest relative bootstrap
half-width at the confirmation's corrected alpha, and sets margins strictly
above `1 + resolution`. P-20 admits at most six confirmatory cells on a
family's first attempt; the cell count each family freezes is recomputed from
`dev/tools/tuning-campaign-support` for that family's ledger rather than
assumed.

## Cells

Workloads are the frozen DVB-T2 and NR bundles `c077a88b` recorded, at the
identities and digests the `3be770d5` addenda pin. No workload is added and
none is re-recorded.

REQ-10 names three granularities over those same workloads. One of them is
measured; the other two are blocked, and the block is a property of the
evidence base rather than a choice:

- **full decoding** — the `3be770d5` steady-state operation unchanged: per-worker
  batches of recorded frames through a reused decoder, including conversion,
  dispatch and decision extraction, with syndrome stopping on. This is what the
  families above measure.
- **full iterations** — decoding at the iteration cap with syndrome stopping
  disabled, so both arms perform an equal, declared number of iterations. The
  measured harness reports the prepared `c077a88b` quality evidence and refuses
  an arm whose settings differ from the settings that evidence was produced
  under, and that evidence is produced under syndrome stopping. A fixed-stopping
  cell therefore needs its own prepared quality corpus, with its own predeclared
  tolerance and its own BER and FER sample counts, which is a quality
  preparation campaign rather than a cell of this family.
- **check-node updates** — one flooding check-node update pass over a prepared
  message array, reported as an isolated kernel. Neither the measured harness nor
  the pinned AFF3CT shim exposes the update rule outside a whole decode, so this
  needs a new arm on each side: a gf2 binary that times the update pass alone,
  and a shim entry point that instantiates AFF3CT's update rule over the same
  prepared array. Without the second, the comparison would not be matched.

Both blocked granularities are reported as unmet rather than approximated by a
whole-decode cell relabelled, because a whole-decode cell measures the
termination rule and the conversion as well as the update.

Scaling arms are one worker and the physical-core and SMT arms the protocol
resolver returns at run time. Allocation counts, degree distributions and the
iteration and early-exit distributions are recorded per cell from the run rather
than assumed, and are projected beside the timings.

## Reproduction

`dev/bench_results/07ca8585/run-campaign.sh` is the launcher; it adds no numeric
setting, takes every one from the committed addendum and the protocol, and runs
each bounded session inside one `dev/scripts/ccx1-bench-flock.sh --full-host`
acquisition. Tables are regenerated by the committed generator from the
committed receipts and reproduce byte for byte.
