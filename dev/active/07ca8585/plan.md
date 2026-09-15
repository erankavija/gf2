# Measurement plan for the min-sum update change

> **Diátaxis Type:** Explanation

How `07ca8585` measures the change the [findings](findings.md) describe. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [protocol](../f547c394/protocol.md) supply every rule; this plan states
only what is specific to this issue.

## Arms

The production change carries a current pinned pre-change baseline. Both arms
are the `3be770d5` steady-state harness built against the two generations of
`gf2-coding`: the **before** arm against the revision the `before` claims of
[source-evidence.json](survey/source-evidence.json) pin, and the **after** arm
against the canonical-layout implementation. A candidate identity is the
executable's bytes, so each arm's digest is recorded before its first timed
call and every campaign refuses an arm whose digest differs.

The AFF3CT [Cassagne2019] arm is the one `3be770d5` measured, at the pinned
digest its
[build identity](../../bench_results/3be770d5/preparation/build-identity.json)
records. It is unchanged by this issue and is rebuilt only if absent.

The isolated check-node granularity needs a third build, because neither the
measured harness nor the pinned AFF3CT shim exposes an update rule outside a
whole decode. Its two arms live in [survey/arms](survey/arms/), a standalone
workspace with its own target directory: the gf2 arm times one flooding
check-node pass through the canonical `EdgeLayout` runs and `min_sum_check_row`,
and the AFF3CT arm times AFF3CT's own `tools::Update_rule_NMS` over the
check-node scan order of `Decoder_LDPC_BP_flooding::_decode_single_ite`, through
a translation unit whose C entry points are disjoint from the pinned shims'. A
separate workspace is what keeps the pinned throughput executables byte-identical:
the campaigns refuse an arm whose digest differs from the preparation build
identity, and adding a target to the measured harness would rebuild them. These
two arms are pinned by
[kernel-build-identity.json](../../bench_results/07ca8585/preparation/kernel-build-identity.json).

## Families

| Family | Question | Arms | Decides |
|---|---|---|---|
| `ldpc-update-single-worker-v1` | Does the canonical update decode the frozen workloads faster than the path it replaces, at the same decisions? | gf2 before, gf2 after | Adoption on the declared single-worker domain |
| `ldpc-update-multicore-v1` | How does the selected route scale across physical cores and SMT? | gf2 before, gf2 after | Nothing; exploratory scaling |
| `ldpc-update-comparator-single-worker-v1` | Where does the canonical decoder sit against the compatible decoder at whole decoding, per REQ-10? | gf2 after, AFF3CT | Nothing; descriptive |
| `ldpc-update-comparator-multicore-v1` | How does that whole-decoding comparison scale across physical cores and SMT? | gf2 after, AFF3CT | Nothing; exploratory scaling |
| `ldpc-update-fixed-iteration-v1` | Where does it sit against the same decoder at an equal, declared iteration count, per REQ-10? | gf2 after, AFF3CT | Nothing; descriptive |
| `ldpc-update-checknode-v1` | Where does the canonical check-node update sit against AFF3CT's own update rule, isolated, per REQ-10? | gf2 check pass, AFF3CT check pass | Nothing; descriptive |

The decision family is the single-worker before/after one: its confirmation
cells carry the frozen worthwhile-effect and equivalence margins, and adoption
follows them. The three comparator families publish the REQ-10 comparisons at
the three granularities and select nothing, which keeps the questions in
separate multiple-comparison families as the protocol requires. The check-node family's purpose is
`kernel-family` and its cells declare no decoder, because an isolated check-node
pass decodes no frame; the other families are decoder families.

Every family keeps an append-only ledger from genesis under
`dev/bench_results/07ca8585/`. The four single-worker families carry a pilot and
a confirmation addendum derived from the committed pilot receipt by the
canonical freezer
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which pins the pilot by
path and digest, takes the resolution from the pilot's widest relative bootstrap
half-width at the confirmation's corrected alpha, and sets margins strictly
above `1 + resolution`. P-20 admits at most six confirmatory cells on a
family's first attempt; the cell count each family freezes is recomputed from
`dev/tools/tuning-campaign-support` for that family's ledger rather than
assumed. The two multicore families preserve exploratory pilots across all six
scaling cells and make no confirmation or adoption claim.

## Cells

Workloads are the frozen DVB-T2 and NR bundles `c077a88b` recorded, at the
identities and digests the `3be770d5` addenda pin. No workload is added and
none is re-recorded.

REQ-10 names three granularities over those same workloads, and each is a family
with its own ledger from genesis, its own pilot and its own frozen confirmation:

- **full decoding** — the `3be770d5` steady-state operation unchanged: per-worker
  batches of recorded frames through a reused decoder, including conversion,
  dispatch and decision extraction, with syndrome stopping on. This is what the
  before/after and comparator families measure.
- **full iterations** — decoding at the iteration cap with syndrome stopping
  disabled, so both arms perform an equal, declared number of iterations. The
  measured harness reports the prepared quality evidence and refuses an arm whose
  settings differ from the settings that evidence was produced under, and the
  reused `c077a88b` evidence was produced under syndrome stopping. These cells
  therefore decode against a corpus produced under exactly their own settings:
  `preparation/quality-fixed/`, one decode of every recorded frame per arm by
  `survey/arms/src/bin/ldpc-fixed-quality.rs`, untimed and committed with its
  digests in the kernel build identity, which the launcher checks before the arms
  read it. The frames, the graphs and the recorded LLRs are the frozen ones; only
  the stopping rule differs, which is the granularity.
- **check-node updates** — one flooding check-node update pass over a prepared
  variable-to-check message array, reported as an isolated kernel: no variable
  update, no syndrome, no conversion and no allocation inside the timed call. The
  prepared array is a state a flooding decode reaches, derived from the frozen
  recorded LLRs by eight flooding rounds of the production path, and one function
  derives it for both arms. Each cell freezes the checksum of that array and of
  the pass's output in the canonical check-major edge order; every worker of
  either arm reproduces both or the arm fails. The gf2 arm holds the messages
  check-major, as its production decoder does, and the AFF3CT arm holds them
  variable-major, as its decoder does, so each side reads the layout its own
  production path uses with the message on every edge identical. The arm refuses
  to run unless AFF3CT's transpose equals the canonical
  check-edge-to-variable-edge map, which is what makes that identity a check
  rather than an assumption, and the untimed
  `survey/arms/src/bin/ldpc-checknode-verify.rs` records that the two passes
  write bit-identical outputs over both workloads.

No granularity is approximated by relabelling a whole-decode cell, because a
whole-decode cell measures the termination rule and the conversion as well as the
update.

Scaling evidence covers one worker and the physical-core and SMT arms the protocol
resolver returns at run time. Allocation counts, degree distributions and the
iteration and early-exit distributions are recorded per cell from the run rather
than assumed, and are projected beside the timings.

## Reproduction

`dev/bench_results/07ca8585/run-campaign.sh` is the launcher of every family; it
adds no numeric setting, takes every one from the committed addendum and the
protocol, and runs each bounded session inside one
`dev/scripts/ccx1-bench-flock.sh --full-host` acquisition. It selects the
prepared quality corpus and the producing manifest each family reads, and checks
the executables and corpus digests that family depends on before the first timed
call. `dev/bench_results/07ca8585/run-profile-resample.sh` runs the predecessor's
profile series over the canonical-layout build, which is the predecessor's refutation rule
for the levers this issue spends. Tables are regenerated by the committed
generator from the committed receipts and reproduce byte for byte.
