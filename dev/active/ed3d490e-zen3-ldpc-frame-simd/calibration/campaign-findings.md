# Decoder calibration campaign: findings (jit:20af7e6e)

> **Diátaxis Type:** Research

The campaign `gf2-0a77ecfc-20261013t011147z-815998` is measured: it executes
[`campaign-declaration.json`](campaign-declaration.json) under
[`premeasurement-protocol.md`](premeasurement-protocol.md) in one session of
the benchmark window, completes and is published. Its receipt is
[`gf2-0a77ecfc-20261013t011147z-815998.md`](../../../benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998.md)
with the [evidence index](../../../benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998-evidence.md);
the published owner is
`crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json`
and the complete envelope
`dev/reference_data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json`.
No cell of the declaration waits for a window.

The receipt and its evidence index cite raw results, run-time records and the
execution journal under `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/`,
a git-ignored path. Those files are committed unaltered as the one archive
`dev/bench_results/0a77ecfc/gf2-0a77ecfc-20261013t011147z-815998-evidence-archive.tar.zst`,
whose members keep the cited names. From the repository root:

```sh
zstd -dc dev/bench_results/0a77ecfc/gf2-0a77ecfc-20261013t011147z-815998-evidence-archive.tar.zst | tar -x
sha256sum -c dev/benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998.sha256
sha256sum -c .agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/SHA256SUMS
```

## F-01: the journal accounts for every declared unit

[`campaign-verification.json`](campaign-verification.json), written by
`verify-campaign.py`, reads the execution journal of the extracted archive
under the digest the committed checksum manifest pins. Every declared unit starts, validates,
checkpoints and completes once with a clean child exit, every cell holds its
probe and its declared timed children, every timed child reports its declared
windows, and the journal ends in a terminal `complete` record; the record's
`journal` block holds the declared counts and the event counts.

The independent validator's verdicts on the stage and on the publication are
the two lines of
[`gf2-0a77ecfc-20261013t011147z-815998-revalidation.log`](../../../benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998-revalidation.log).

## F-02: the owner's measurement block names the declared producer

The `owner_envelope` block of the verification record holds the harness path,
the behaviour token and the producer executable digest of the published
owner. They equal the producer, behaviour token and executable of
[`producer/identity.json`](producer/identity.json) and the producer reports
beside it, and the harness is a behaviour source and a build input of the
producing manifest the campaign record pins by digest. Those bytes are
committed as
[`inputs/producing/…/producing-build-inputs.json`](inputs/producing/dev/active/ed3d490e-zen3-ldpc-frame-simd/calibration/producing-build-inputs.json);
the record's `producing_manifest` entry holds the path and the digest. The
live [`producing-build-inputs.json`](producing-build-inputs.json) enumerates
the current tree and is no input of this campaign.

## F-03: two lane bounds are selected and stated

The receipt's section *Argmin and threshold decisions* records
`decode.lane_min_frames` and `decode.lane_max_edges` with reason
`selected-non-default` and `emitted` true, and the owner states both at the
`selected` value of each decision. The decision of `decode.lane_max_edges`
records `grid_boundary_limited` true: its selected schedule class is an end of
the declared grid, and the campaign measures no candidate beyond it.

The scores of each decision are the producer's own statistic over the timed
children of each candidate, with every execution in the receipt; they are
descriptive and carry no interval.

## F-04: the omitted selectors keep their conservative values

The receipt records `decode.granules_per_task` with reason `measured-default`
and `emitted` false: the decision selects the conservative candidate, and
under protocol §4 the owner omits the field. The encode family is outside the
campaign (protocol §1), and the owner states none of its selectors.

`crates/gf2-coding/tests/tuning_profile_committed.rs` decodes the published
owner through the owner codec and requires every selector the owner omits to
resolve to its conservative value.

## What the campaign does not decide

- The measured choices hold for the host, grid and protocol the receipt's
  section *Limitations* names; the owner's measurement block records the host
  facts.
- The `selector-calibration` family of protocol §9 reconfirms the profile on
  holdout inputs; this record reads no result of it.
- `gf2-coding` bakes no profile: a process resolves the stated values only
  after it installs the owner.
