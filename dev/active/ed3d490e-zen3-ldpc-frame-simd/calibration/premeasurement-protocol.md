# Decode selector calibration: premeasurement protocol

> **Diátaxis Type:** Reference

Status: declared; no campaign of this declaration has run. Issue `0a77ecfc`
supplies the success criteria, and the story plan's contracts
[`tuning-owner-mechanism`, `decode-selectors` and
`calibrated-decode-profile`](../plan.md#shared-architectural-contracts) the
design.

This declaration amends the executed seam protocol
[`dbd8787d/premeasurement-protocol.md`](../../dbd8787d/premeasurement-protocol.md)
(cited below as *seam*) by reference, and through it the extent protocol
[`a83583e0/premeasurement-protocol.md`](../../6dc81018-field-capability-dispatch/a83583e0/premeasurement-protocol.md)
(cited below as *extent*) that the seam protocol amends. Every rule of both
holds unless a section here replaces it. The timed run follows the timed-run
rules of the
[worker brief](../../1a379447-zen3-cpu-performance/worker-brief.md); the
[measurement contract](../../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [frozen protocol](../../1a379447-zen3-cpu-performance/f547c394/protocol.md)
with its [version-4 amendment](../../1a379447-zen3-cpu-performance/f547c394/amendment-v4.md)
govern the protocol family that reconfirms the published profile (§9).

## 1. Scope

The campaign measures the coding owner and no other: the producer
`crates/gf2-coding/benches/tuning_calibration.rs` sweeps the decode family of
the coding tuning section `gf2-coding/encode` under the run-identifier prefix
`gf2-0a77ecfc-`. The core and algebra owners are imported from their committed
envelopes (§6), so the campaign builds and stages the coding producer, the
composer and the driver.

The owner protocol is `coding-tuning-campaign-v1` and the behaviour token
`coding-tuning-calibration-v1`, the harness schema of the coding codec, which
reopens a calibrated section only under that token. The producing manifest
[`producing-build-inputs.json`](producing-build-inputs.json), written by
`make-calibration-producing-inputs.py`, names the campaign's behaviour,
lifecycle and build inputs; [`producing-build-inputs.sha256`](producing-build-inputs.sha256)
pins each build input and the manifest by SHA-256 at the declared tree.
Preparation records each input's digest in the campaign record.

The encode family of the section is not measured. Its leaves are omitted
under the codec's omission rule and keep their conservative values.

## 2. Decode fields, grid and arms

Each field is an extent of extent §5: one untimed probe and five timed fresh
children per cell, five 250 ms windows per child, every window retained.
A cell is one candidate value of one field on one stratum.

<!-- generated:grid -->
| Tag | Field | Candidates | Conservative | Entry | Strata | Cells |
|---|---|---|---|---|---|---|
| 32 | `decode.lane_min_frames` | `1`, `2`, `3`, `4`, `6`, `8`, `usize::MAX` | `usize::MAX` | `ldpc-batch-decoder` | `nr5g-bg2-z16-f1-open`, `nr5g-bg2-z16-f2-open`, `nr5g-bg2-z16-f3-open`, `nr5g-bg2-z16-f4-open`, `nr5g-bg2-z16-f6-open`, `nr5g-bg2-z16-f8-open` | 42 |
| 33 | `decode.lane_max_edges` | `0`, `3152`, `48599`, `226799` | `0` | `ldpc-batch-decoder` | `nr5g-bg2-z16-f8-open`, `dvb-t2-short-r1-2-f8-open`, `dvb-t2-normal-r1-2-f8-open` | 12 |
| 34 | `decode.granules_per_task` | `1`, `2`, `4`, `8`, `16` | `1` | `pooled-ldpc-batch-decoder` | `nr5g-bg2-z16-f128-closed`, `nr5g-bg2-z16-f128-open`, `dvb-t2-short-r1-2-f128-closed`, `dvb-t2-short-r1-2-f128-open` | 20 |
<!-- /generated:grid -->

A stratum names its code, its batch size in frames and whether the lane
bounds the field does not measure open the lane route (`open`: a minimum of
one frame, no edge bound) or hold their conservative values (`closed`). The
cases of the two lane fields hold `decode.granules_per_task` at its
conservative value.

| Field | Operation timed | Required observation |
|---|---|---|
| `decode.lane_min_frames` | `LdpcBatchDecoder` on one wave of the stratum's occupied lanes, smallest code | the public route of the batch equals the route the forced selectors give |
| `decode.lane_max_edges` | `LdpcBatchDecoder` on one full wave of each code | as above |
| `decode.granules_per_task` | `PooledBatchDecoder` over sixteen waves in a dedicated pool of four workers, lane route open and closed | the pool's task size and the route equal those the forced selectors give |

Each child installs a forced owner profile whose decode family is the case's
selectors, requires that the section resolves as installed with exactly those
selectors, and requires the batch result of every fixture bank to equal the
single-frame `LdpcDecoder` result frame by frame before any window. The
decoder is the default configuration the grid listing states, with the
iteration cap of the table below. The lane backend is the detected one; the
capability report states it.

## 3. Fixtures and seed derivation

Seed tags 32, 33 and 34 follow the seam tags. The inventory schema is
`fixture-seeds-v3` and the mixer `gf2-calibration-seed-v1` with the extent seed
root: the seed of bank `b` is `fixture_seed(root, tag, shape key, 0x0d00 + (b << 16))`,
where the shape key is the code's index. A child verifies all eight banks and
its windows rotate through them from the start bank of extent §3.

<!-- generated:codes -->
| Field | Code | Frame length | Edges | Noise deviation (per mille) | Batch frames | Shape key | Iteration cap |
|---|---|---|---|---|---|---|---|
| `decode.lane_min_frames` | `nr5g-bg2-z16` | 832 | 3152 | 1200 | 1, 2, 3, 4, 6, 8 | 0 | 50 |
| `decode.lane_max_edges` | `nr5g-bg2-z16` | 832 | 3152 | 1200 | 8 | 0 | 50 |
| `decode.lane_max_edges` | `dvb-t2-short-r1-2` | 16200 | 48599 | 800 | 8 | 1 | 50 |
| `decode.lane_max_edges` | `dvb-t2-normal-r1-2` | 64800 | 226799 | 750 | 8 | 2 | 50 |
| `decode.granules_per_task` | `nr5g-bg2-z16` | 832 | 3152 | 1200 | 128 | 0 | 50 |
| `decode.granules_per_task` | `dvb-t2-short-r1-2` | 16200 | 48599 | 800 | 128 | 1 | 50 |
<!-- /generated:codes -->

A frame is the all-zero codeword through BPSK and AWGN, drawn from the bank's
`StdRng` stream as channel LLRs. Frame `i` of a batch scales the code's noise
deviation by entry `i mod 8` of the per-mille scale the grid listing states,
so the frames of one wave stop at different iterations. A batch of another
size on the same field and code is a prefix or an extension of the same
stream. Operand digests use the domains `gf2-coding-decode-fixture-v1` and
`gf2-coding-decode-operands-v1`, result digests `gf2-coding-decode-output-v1`.
The committed [grid listing](producer/coding-list-grid.json) holds every seed.

## 4. Extent decision

The decision is the extent estimator and argmin rule of extent §5 and §5.1,
applied per field over its strata with the conservative candidate as the
reference. Candidates whose schedule (route, task size, task count) agrees on
every stratum form one schedule class. A field is selected only when one class
of a single candidate strictly wins against every other class, no curve is
non-monotone and the conservative candidate strictly wins on no stratum.

The owner envelope states a field only when its decision selects a value other
than the conservative one. Every other outcome, a measured conservative value
included, omits the field, and an omitted field keeps its conservative value.

## 5. Declared counts

<!-- generated:counts -->
| Quantity | Count |
|---|---|
| cells | 74 |
| untimed probes | 74 |
| timed children | 370 |
| accepted results | 444 |
| raw windows | 1850 |
| timing progress records | 2220 |
| nominal target-window seconds | 462.5 |
| coding codec leaves | 7 |
| measured leaves | 3 |
<!-- /generated:counts -->

Each cell is one probe and five timed children, in the block order of extent
§6: per field and stratum a probe block, then five timed blocks. The session
budget and the child limits are those of the extent protocol; a
session that exhausts its budget is resumed under the same campaign
identifier.

The queue estimate is the nominal target-window time plus the active time of
the rehearsal session of §8, which holds every untimed step of every child
and one dispatch per window. It is an estimate, not a measurement.

<!-- generated:estimate -->
| Term | Value |
|---|---|
| nominal target-window seconds | 462.5 |
| active seconds of the rehearsal session (one run, unlocked host) | 445 |
| session budget seconds | 10800 |
| estimated minutes, rounded up to a multiple of 5 | 20 |
<!-- /generated:estimate -->

## 6. Imported owners

[`campaign-declaration.json`](campaign-declaration.json) imports the core
owner the baked core constants pin and the one committed calibrated algebra
owner, each by envelope path and SHA-256 beside the committed complete
envelope whose section wrapper it must equal. Preparation, composition,
publication and validation treat both imports as seam §6 treats the imported
algebra owner. The complete envelope holds three sections.

## 7. Declaration, launcher and destinations

[`campaign-declaration.json`](campaign-declaration.json), written by
`make-campaign-declaration.py`, is the one committed declaration of this
campaign. `run-campaign.sh` is its queue launcher: it starts a run through
`dev/scripts/tuning-extent-campaign.sh 0a77ecfc`, or resumes the one existing
stage of the issue under that stage's run identifier, and leaves its queue
line open while the campaign is incomplete. Its one line of the
[benchmark-window queue](../../1a379447-zen3-cpu-performance/bench-window/queue.tsv)
carries the estimate of §5 behind the window's load guard.

Repository destinations are derived from the run identifier as in seam §7,
with the coding-owner envelope at
`crates/gf2-coding/data/tuning-profiles/<run-id>.json` in place of a core-owner
envelope. Publication writes no core-owner and no algebra-owner destination.

## 8. Premeasurement checks

The untimed `--capability-report`, `--list-grid` and `--self-check` of the
Rust 1.95 release producer are committed under [`producer/`](producer/identity.json)
by `record-producer-reports.py`, with the digests of the producer source and
executable. `validate-tuning-extent-campaign.py --declaration-check 0a77ecfc`
accepts the declaration. Each generator of this directory has a `--check`
mode that writes nothing.

`rehearse-campaign.sh` runs the queue launcher end to end, untimed, over a
throwaway stage in a throwaway clone of the declared tree: the campaign
launcher, driver, coding producer, composer and validator build, stage,
prepare, run every cell, compose, validate and publish into the clone. Two
stand-ins, committed in the clone only, replace what a working session may
not do. [`rehearsal/untimed-windows.patch`](rehearsal/untimed-windows.patch)
makes a timed child dispatch each window once and report constant samples
without reading a clock, and
[`rehearsal/private-lock.sh`](rehearsal/private-lock.sh) holds a lock file of
the rehearsal in place of the host mutex. The rehearsed producer therefore has
another digest than the campaign's, its samples are no measurements, and its
decisions select nothing. A second launcher call resumes the complete stage
under its run identifier. [`rehearsal/record.json`](rehearsal/record.json)
holds the rehearsed revision, the stand-in digests, the journal's event
counts, the launcher's exit statuses, the validator's verdicts on the stage
and on its publication, and the selectors the published owner states.

## 9. What the campaign does not decide

The published owner states the swept values. The protocol family of purpose
`selector-calibration` that the story plan names reconfirms the profile on
holdout inputs with fill, transposition and workspace costs inside the call;
this campaign reads no result of it.
