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

## F-05: the run records the build's profile and features, and no flag variable

The records of the collected run that bear on the producer's build are:

- `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/build/coding.jsonl` (SHA-256 `bff485837b4e0b118e7a00ee55d3634cc74f2dc672f7891af21a05c68ecf9897`
  in the archive's `SHA256SUMS`): Cargo's artifact messages, with the profile
  fields, the feature list and the freshness of every unit and the path of the
  producer executable;
- `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/build/build.log` (`28cbdb272ead80ce5e06c2f12f3427096e2a453671e39fbf89ebf3f8e3472762`):
  Cargo's non-verbose progress lines;
- [`staging-manifest.json`](../../../benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998-session/staging-manifest.json)
  and the two `build/source-*.json` records beside it: the staged executable
  digests and the source revision;
- `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/campaign.json` (`63edb0033c01fec941e8cf7b930a3cee1010375d56a7b9740e8d1efc676ed8fe`):
  the toolchain in `runtime`, the digest of every build input in
  `build_inputs`, and the environment of the measured children in `processes`;
- the owner's measurement block: the toolchain and the executable digest.

None of them holds the build's `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS` or a
compiler command line. The window state adds none: the job output
`.agents/bench-window/18a3cf103a202e97.out` of the primary checkout (host-local,
SHA-256 `48b31d4913672312dd7471ab1b9a669de22524e96817a2062ab19509a9b42e48`)
holds the launcher's and validator's lines, and the append-only
`.agents/bench-window/window.log` holds the job's start and exit lines with the
load at start. The launcher of that run passes its caller's environment to
Cargo, so the run's own records leave the effective flags unstated.

## F-06: a rebuild under declared flags reproduces the producer digest

[`producer-rebuild/record.json`](producer-rebuild/record.json), written by
`record-producer-rebuild.py`, is a reproduction made after the campaign, at
the record's `observed_utc`; it is no record of the campaign's own build. It
builds the producer at the campaign's source revision in a scratch tree
mounted at the recorded build path, with the environment cleared to the
record's `environment` block, which holds no flag variable, and with the
launcher's build command made verbose. The record's `reproduces` is true: the
rebuilt executable has the digest of the owner's measurement block and of
[`producer/identity.json`](producer/identity.json).

[`producer-rebuild/build.log`](producer-rebuild/build.log) holds every
compiler invocation of the dependency closure; the record's `build` block
holds the producer's full `rustc` command line and the codegen options of the
closure by invocation count, and its `toolchain` entry the compiler version.
Cargo's fingerprint of the producer unit states an empty `rustflags` list, and
its bytes equal those of the fingerprint the campaign's build left in the
producing checkout (`cargo_fingerprint`, a host-local observation).

The build flags of the measured executable are therefore the ones the record
declares: the committed Cargo profile and features, and no `RUSTFLAGS`.

## Closed identity set

| Item | Record |
|---|---|
| Executable digest | owner measurement block `binary_sha256`; `staging-manifest.json` of the session record; `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/campaign.json` `processes` |
| Build flags | `producer-rebuild/record.json` (F-06); `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/build/coding.jsonl` for profile and features (F-05) |
| Frozen configuration | `campaign-declaration.json` and `premeasurement-protocol.md` by the digests of the receipt's first section; `coding-list-grid.json` of the session record; `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/coding-manifest.json` for every case |
| Seed | `coding-list-grid.json` of the session record: seed root, derivation and every bank seed |
| RNG implementation and version | No run-time record names them; `@/issue/9eb6f13c` tracks the gap. The committed records pin the seed (row above), the `Cargo.lock` digest among `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/campaign.json` `build_inputs`, and, in `producer-rebuild/build.log`, the `rand` crates compiled into the reproduced executable |
| Toolchain version | owner measurement block `toolchain`; `.agents/campaign-evidence/gf2-0a77ecfc-20261013t011147z-815998/campaign.json` `runtime` |
| Run-time host facts | owner measurement block (`host`, `cpu_model`, `cpu_features`, `os_kernel`, `governor`); the `driver-diagnostic` records of the execution journal |

## What the campaign does not decide

- The measured choices hold for the host, grid and protocol the receipt's
  section *Limitations* names; the owner's measurement block records the host
  facts.
- The `selector-calibration` family of protocol §9 reconfirms the profile on
  holdout inputs; this record reads no result of it.
- `gf2-coding` bakes no profile: a process resolves the stated values only
  after it installs the owner.
