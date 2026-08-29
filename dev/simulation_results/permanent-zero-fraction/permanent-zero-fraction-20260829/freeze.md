# Permanent campaign freeze: `permanent-zero-fraction-20260829`

This record binds the campaign decisions that the strict `CampaignManifest` JSON schema intentionally does not carry. It lives at the root of the date-qualified campaign directory beside `manifest.json` and `checksums.sha256`, because these decisions apply to this immutable dataset identity rather than to a reusable protocol or a benchmark cohort. The directory is never reused for a changed cell universe, sample count, multiplicity allocation, stream construction, backend selection, or determinant plan.

## Frozen identity and evidence

- Campaign id: `permanent-zero-fraction-20260829`.
- Dataset directory: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`.
- Root seed: `0x7a81626200000001` (`8827444917669199873`).
- Freeze host: `fraktaali`, Linux `7.1.8-arch1-3`.
- Processor: AMD Ryzen 9 5900X 12-Core Processor; 12 physical cores and 24 logical threads.
- Accelerator: AMD Radeon RX 6950 XT (Navi 21, `gfx1030`); ROCm `7.2.4`, HIP `7.2.53211-9999`, and amdgpu `7.1.8-arch1-3`.
- Manifest content SHA-256: `c37305910037d5c0f0a41f51a6be3960a53d27ce66d63bda87c6ae45fb4b2952`.
- Protocol: `dev/simulation_results/permanent-zero-fraction/protocol.md`, SHA-256 `249f3de398cd234cdd9c1f1d352fc909394f3bacf13acda606d95da343693639`.
- Backend selection receipt: `dev/benchmarks/permanent_campaign/backend-selection-v1.md`, SHA-256 `fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe`.
- Determinant machine receipt: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv`, SHA-256 `d3779aa3817bbefc995b858527bf418b4d3931a8f5406281eb4f0b71a7b45e77`.
- Determinant rendered report: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md`, SHA-256 `839ab3f1f22a995cc32bc6a7102f4c6aad721b6eff5713d3976df36838d2334a`.

The manifest enumerates the complete 63-cell universe in lexicographic `(q,n)` order. Every cell's backend is the selected backend in the accepted 63-row receipt, and every `backend_receipt` binds that receipt by the path and digest above. The receipt applies protocol rules 1–4 and records the measured basis and any exclusion or single-eligible-arm basis for every decision; no historical draft is a manifest input.

## Pre-draw execution receipt

This freeze record is the committed pre-draw execution receipt that the
protocol's governing clause (`protocol.md:3-6`) requires:

> This document governs the first permanent-zero-fraction campaign whose
> committed pre-draw execution receipt records this repository-relative path,
> content SHA-256, and corresponding root-manifest identity.

The three bindings for this receipt are:

- Protocol path: `dev/simulation_results/permanent-zero-fraction/protocol.md`.
- Protocol content SHA-256: `249f3de398cd234cdd9c1f1d352fc909394f3bacf13acda606d95da343693639`.
- Root-manifest identity: campaign id `permanent-zero-fraction-20260829`; manifest content SHA-256 `c37305910037d5c0f0a41f51a6be3960a53d27ce66d63bda87c6ae45fb4b2952`.

REQ-02 resolves in both directions: `protocol.md:3-6` identifies its receipt
descriptively as the artifact recording exactly these three bindings. This
section closes the loop by explicitly claiming the pre-draw execution receipt
role and citing that clause. No protocol edit occurs: the protocol is frozen,
and a changed protocol would define a new campaign id.

## Cell universe, multiplicity, and error allocation

The frozen values cite the protocol's [cell universe and sample-size table](/dev/simulation_results/permanent-zero-fraction/protocol.md#frozen-cell-universe-and-sample-sizes), [empty extension family](/dev/simulation_results/permanent-zero-fraction/protocol.md#extension-family), and [global error budget](/dev/simulation_results/permanent-zero-fraction/protocol.md#global-error-budget-and-exact-decisions):

| q | n range | cells | N per cell | shards per cell | final shard matrices |
|---:|:---|---:|---:|---:|---:|
| 3 | 4–20 | 17 | 20,000,000 | 400 | 50,000 |
| 3 | 21–28 | 8 | 222,223 | 5 | 22,223 |
| 5 | 4–16 | 13 | 16,000,000 | 320 | 50,000 |
| 5 | 17–24 | 8 | 160,000 | 4 | 10,000 |
| 7 | 4–16 | 13 | 12,244,898 | 245 | 44,898 |
| 7 | 17–20 | 4 | 122,449 | 3 | 22,449 |

The extension family is `E = ∅`, so the multiplicity count is exactly `K = 63`. The single global family-wise false-alarm budget is `0.05`: the one-sided permanent-floor family receives `0.025`, and the two-sided determinant family receives `0.025`. Each family has 63 tests, so both use the exact per-cell level `0.025 / 63 = 1 / 2520` (approximately `3.96825e-4`). These values remain in this freeze record rather than being added as unknown fields to the `deny_unknown_fields` JSON schema.

Adding any cell after this freeze requires a new campaign id, an explicit restatement of the multiplicity adjustment and error allocation, and a new pre-draw freeze. This campaign id and its `K = 63` adjustment are not reused.

## Shards and stream construction

The shard size is 50,000 matrices. It bounds atomic restart work while keeping file and checkpoint counts tractable; applying `ceil(N / 50,000)` independently to each cell produces 14,229 shards. Each cell has contiguous `shard_id` values from zero through its shard count minus one.

The stream-purpose namespace is complete and uses the sampler's canonical tags:

| Purpose | Tag | Reserved use |
|---|---:|---|
| `validation` | 1 | sampler and estimator validation |
| `timing` | 2 | timing fixtures and measurements |
| `campaign-cells` | 3 | published campaign cells; every manifested shard uses this purpose |
| `rare-event` | 4 | rare-event estimation |

The sampler seed consists of four little-endian `u64` words: `(root_seed, q, n, (purpose_tag << 56) | stream_index)`. For the zero-based lexicographic cell ordinal and cell-local shard id, the manifest fixes `stream_index = (cell_ordinal << 32) | shard_id`.

Disjointness follows from that encoding. Distinct purpose tags change the high eight bits of the seed's fourth word, while every stream index is confined to its low 56 bits. Within one purpose, distinct cells change the `(q,n)` seed words and occupy distinct 24-bit ordinal slots in stream-index bits 32–55. Within one cell, distinct shards occupy distinct low-32-bit shard-id slots. Thus purposes, cells, and shards cannot alias by construction; no conclusion depends on inspecting the allocated values for collisions.

The largest allocated shard id is 399, below the scheduler's exclusive 1,000,000 limit and below `2^32`. The exact largest allocated stream index is 266,287,972,354 at ordinal 62, shard 2. The conservative envelope formed from the largest ordinal (62) and largest shard id anywhere (399) is `(62 << 32) | 399 = 266,287,972,751`, still far below the exclusive low-56-bit limit `2^56 = 72,057,594,037,927,936`. The larger envelope value is not itself allocated because the final lexicographic cell has three shards.

## Determinant companion and measured marginal cost

The protocol [requires the determinant companion on the identical draws for all 63 cells](/dev/simulation_results/permanent-zero-fraction/protocol.md#frozen-cell-universe-and-sample-sizes), and `determinant_companion` is `evaluate` in every manifest cell. Issue `ec22205e` closed the all-cell v5 cohort with five measured processes at every cell. The table below transcribes the directly measured per-process mean marginal cost and its process-level 95% interval from the v5 machine receipt and rendered report. The projected fixed-N seconds are labeled as derived projections. Every row is a direct measurement at that exact `(q,n)`; no representative-order interpolation is presented as measurement.

| q | n | measured processes | measured mean (µs/matrix) | measured mean 95% CI (µs/matrix) | derived fixed-N projection (s) |
|---:|---:|:---:|---:|:---|---:|
| 3 | 4 | 5 / 5 | 0.188771963 | [0.187656408, 0.189887517] | 3.775439 |
| 3 | 5 | 5 / 5 | 0.248388232 | [0.247464706, 0.249311758] | 4.967765 |
| 3 | 6 | 5 / 5 | 0.322133792 | [0.319954579, 0.324313005] | 6.442676 |
| 3 | 7 | 5 / 5 | 0.416626047 | [0.414689089, 0.418563005] | 8.332521 |
| 3 | 8 | 5 / 5 | 0.520280282 | [0.507004991, 0.533555572] | 10.405606 |
| 3 | 9 | 5 / 5 | 0.611189216 | [0.603017938, 0.619360494] | 12.223784 |
| 3 | 10 | 5 / 5 | 0.708042089 | [0.702253197, 0.713830981] | 14.160842 |
| 3 | 11 | 5 / 5 | 0.827347316 | [0.822061318, 0.832633314] | 16.546946 |
| 3 | 12 | 5 / 5 | 0.999193616 | [0.995193996, 1.003193235] | 19.983872 |
| 3 | 13 | 5 / 5 | 1.152826370 | [1.149815492, 1.155837248] | 23.056527 |
| 3 | 14 | 5 / 5 | 1.293213450 | [1.287868533, 1.298558367] | 25.864269 |
| 3 | 15 | 5 / 5 | 1.503575670 | [1.496594855, 1.510556486] | 30.071513 |
| 3 | 16 | 5 / 5 | 1.723417950 | [1.704710150, 1.742125749] | 34.468359 |
| 3 | 17 | 5 / 5 | 1.902791487 | [1.894584084, 1.910998889] | 38.055830 |
| 3 | 18 | 5 / 5 | 2.095799736 | [2.087716459, 2.103883013] | 41.915995 |
| 3 | 19 | 5 / 5 | 2.291271549 | [2.271507567, 2.311035532] | 45.825431 |
| 3 | 20 | 5 / 5 | 2.508433077 | [2.490702560, 2.526163595] | 50.168662 |
| 3 | 21 | 5 / 5 | 2.783976981 | [2.755523293, 2.812430669] | 0.618664 |
| 3 | 22 | 5 / 5 | 3.118457997 | [3.018944853, 3.217971141] | 0.692993 |
| 3 | 23 | 5 / 5 | 3.421759300 | [3.373191217, 3.470327382] | 0.760394 |
| 3 | 24 | 5 / 5 | 3.789503017 | [3.764627764, 3.814378270] | 0.842115 |
| 3 | 25 | 5 / 5 | 4.105494337 | [4.077468039, 4.133520634] | 0.912335 |
| 3 | 26 | 5 / 5 | 4.398673005 | [4.385284109, 4.412061900] | 0.977486 |
| 3 | 27 | 5 / 5 | 4.775198541 | [4.754847850, 4.795549232] | 1.061159 |
| 3 | 28 | 5 / 5 | 5.239784906 | [5.224030789, 5.255539023] | 1.164401 |
| 5 | 4 | 5 / 5 | 0.192382400 | [0.191264612, 0.193500188] | 3.078118 |
| 5 | 5 | 5 / 5 | 0.253620480 | [0.253027134, 0.254213826] | 4.057928 |
| 5 | 6 | 5 / 5 | 0.329916986 | [0.327613221, 0.332220752] | 5.278672 |
| 5 | 7 | 5 / 5 | 0.435550373 | [0.425893166, 0.445207581] | 6.968806 |
| 5 | 8 | 5 / 5 | 0.559227559 | [0.550219391, 0.568235728] | 8.947641 |
| 5 | 9 | 5 / 5 | 0.641153274 | [0.637710271, 0.644596277] | 10.258452 |
| 5 | 10 | 5 / 5 | 0.743133214 | [0.741302391, 0.744964037] | 11.890131 |
| 5 | 11 | 5 / 5 | 0.873114870 | [0.869310980, 0.876918759] | 13.969838 |
| 5 | 12 | 5 / 5 | 1.051958740 | [1.048808927, 1.055108554] | 16.831340 |
| 5 | 13 | 5 / 5 | 1.199647078 | [1.184969075, 1.214325082] | 19.194353 |
| 5 | 14 | 5 / 5 | 1.396177543 | [1.382401326, 1.409953760] | 22.338841 |
| 5 | 15 | 5 / 5 | 1.610723440 | [1.598172230, 1.623274651] | 25.771575 |
| 5 | 16 | 5 / 5 | 1.873473406 | [1.853023575, 1.893923238] | 29.975575 |
| 5 | 17 | 5 / 5 | 2.046567973 | [2.034566989, 2.058568957] | 0.327451 |
| 5 | 18 | 5 / 5 | 2.227464069 | [2.221047095, 2.233881044] | 0.356394 |
| 5 | 19 | 5 / 5 | 2.454804608 | [2.444154395, 2.465454820] | 0.392769 |
| 5 | 20 | 5 / 5 | 2.696678684 | [2.685143896, 2.708213473] | 0.431469 |
| 5 | 21 | 5 / 5 | 2.979211745 | [2.969291748, 2.989131741] | 0.476674 |
| 5 | 22 | 5 / 5 | 3.300123896 | [3.283373283, 3.316874510] | 0.528020 |
| 5 | 23 | 5 / 5 | 3.682067689 | [3.620989618, 3.743145760] | 0.589131 |
| 5 | 24 | 5 / 5 | 4.111986884 | [4.072186484, 4.151787283] | 0.657918 |
| 7 | 4 | 5 / 5 | 0.199340750 | [0.196907729, 0.201773771] | 2.440907 |
| 7 | 5 | 5 / 5 | 0.263471860 | [0.261604394, 0.265339325] | 3.226186 |
| 7 | 6 | 5 / 5 | 0.333545537 | [0.330873972, 0.336217102] | 4.084231 |
| 7 | 7 | 5 / 5 | 0.448066389 | [0.443869577, 0.452263201] | 5.486527 |
| 7 | 8 | 5 / 5 | 0.567772870 | [0.558297320, 0.577248421] | 6.952321 |
| 7 | 9 | 5 / 5 | 0.672874046 | [0.639217921, 0.706530172] | 8.239274 |
| 7 | 10 | 5 / 5 | 0.762395475 | [0.757400484, 0.767390466] | 9.335455 |
| 7 | 11 | 5 / 5 | 0.886855368 | [0.881389377, 0.892321359] | 10.859454 |
| 7 | 12 | 5 / 5 | 1.071465079 | [1.058675005, 1.084255152] | 13.119981 |
| 7 | 13 | 5 / 5 | 1.240305171 | [1.222011356, 1.258598987] | 15.187410 |
| 7 | 14 | 5 / 5 | 1.426994200 | [1.416383746, 1.437604653] | 17.473398 |
| 7 | 15 | 5 / 5 | 1.644262351 | [1.638041041, 1.650483660] | 20.133825 |
| 7 | 16 | 5 / 5 | 1.930309223 | [1.904779862, 1.955838584] | 23.636440 |
| 7 | 17 | 5 / 5 | 2.102966746 | [2.089132318, 2.116801175] | 0.257506 |
| 7 | 18 | 5 / 5 | 2.286437702 | [2.275229698, 2.297645707] | 0.279972 |
| 7 | 19 | 5 / 5 | 2.531467121 | [2.518928794, 2.544005449] | 0.309976 |
| 7 | 20 | 5 / 5 | 2.780574627 | [2.761175375, 2.799973879] | 0.340479 |

## Manifest integrity contract

`manifest.json` contains no self-hash field. `gf2_sim::permanent_campaign::provenance::manifest_content_hash` recomputes SHA-256 over the exact `manifest.json` bytes, while `checksums.sha256` records that digest in its `manifest.json` entry and does not hash itself. A reader reads the sidecar entry, recomputes the content hash, and compares the two; modification therefore produces a mismatch instead of silently changing the campaign identity.

This freeze record itself is not a `checksums.sha256` member: the campaign README's integrity set is deliberately closed over raw data, and a checksum file cannot close if it also covers a record that quotes its entries — which this file does. The record's tamper evidence is repository history: `freeze.md` is a committed, tracked file, and any modification after the freeze commit `57c9633f` appears as a tracked-file diff rather than passing silently. This disposition is recorded under issue `7a816262`.

## Pre-draw and validation record

Each transcript in this section is pinned to the revision at which it ran: the freeze-preparation validations ran at revision `396cf929`, the parent of the freeze commit `57c9633f`, and the post-commit confirmations ran at revision `67f5108f`. The freeze-preparation audit at `396cf929` reports no non-fixture campaign shard, field summary, pooled summary, or campaign checkpoint in the tree, and a history-wide path audit reports only the committed fixture shards under `fixtures/`. The freeze commit `57c9633f` therefore precedes every non-fixture shard commit in repository history, which supplies REQ-02's ordering evidence.

The HIP-enabled emitter is built only after the shared Cargo process check returns no process:

```text
$ pgrep -a cargo || true
(no output)
$ cargo +1.95.0 build -p gf2-sim --release --features hip --bin permanent_campaign
warning: gf2-kernels-hip@0.1.0: skip gfx940: hipcc --offload-arch=gfx940 on /home/vkaskivuo/Projects/gf2/crates/gf2-kernels-hip/kernels/gfx940/probe.cpp exited with exit status: 1
   Compiling gf2-kernels-hip v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-kernels-hip)
   Compiling gf2-algebra v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-algebra)
   Compiling gf2-coding v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-coding)
   Compiling gf2-sim v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-sim)
    Finished `release` profile [optimized] target(s) in 18.41s
```

The `gfx940` probe warning is an expected architecture skip; the linked emitter carries the host's HIP, HSA, and profiler runtime libraries. Its SHA-256 is the nonzero digest frozen in `manifest.json`.

The following command, run at revision `396cf929`, enters through the real `read_manifest` call in `permanent_campaign`, so exit status zero is the strict-schema parse evidence as well as the required provenance output:

```text
$ target/release/permanent_campaign --print-provenance --manifest dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829
{
  "git_revision": "396cf9295163e887519ada23532f7fa5634702c2",
  "binary_sha256": "2d6edcd940abe9340143c8b724a8274fff8eca1200a491ad13deec9e386eba58",
  "deps_source_revision": "6348b0c974b135d8b6d5923beb388d80fe7f4d10",
  "deps_source_dirty": false,
  "compiler_version": "rustc 1.95.0 (59807616e 2026-04-14)",
  "rng_algorithm": "cha_cha20",
  "rng_version": "rand_chacha 0.9.0 (ChaCha20Rng)",
  "invocation": [
    "target/release/permanent_campaign",
    "--print-provenance",
    "--manifest",
    "dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829"
  ],
  "accelerator_runtime": {
    "state": "present",
    "value": "ROCm 7.2.4; HIP 7.2.53211-9999; amdgpu 7.1.8-arch1-3"
  },
  "cpu_model": "AMD Ryzen 9 5900X 12-Core Processor",
  "cpu_physical_cores": 12,
  "cpu_logical_threads": 24,
  "gpu_model": {
    "state": "present",
    "value": "AMD Radeon RX 6950 XT (Navi 21, gfx1030)"
  }
}
$ echo $?
0
```

At revision `396cf929` the printed value also matches the manifest's stored provenance exactly:

```text
$ if diff -u <(jq -S '.provenance' dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json) <(target/release/permanent_campaign --print-provenance --manifest dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829 | jq -S '.') >/dev/null; then echo 'PASS: read_manifest accepted the strict schema and printed provenance exactly matches manifest provenance'; else echo 'FAIL: printed provenance differs'; exit 1; fi
PASS: read_manifest accepted the strict schema and printed provenance exactly matches manifest provenance
```

`observe_provenance` reports the live checkout's `git_revision`, so at any other revision the printed provenance differs from the manifest's stored provenance in exactly that field; the stored value pins `396cf929` as the manifest-creation revision. The recomputed diff at revision `67f5108f` confirms `git_revision` is the only differing field.

The emission check fails closed at every stage, as required. At revision `396cf929`, where the manifest bytes are not yet committed, it refuses on the committed-content comparison:

```text
$ target/release/permanent_dataset emission-check dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829
frozen campaign manifest dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json differs from its committed content
$ echo $?
1
```

At revision `67f5108f`, with the freeze commit in history, the same command advances past the committed-content comparison and refuses on the binary digest instead:

```text
$ target/release/permanent_dataset emission-check dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829
the frozen manifest names binary SHA-256 2d6edcd940abe9340143c8b724a8274fff8eca1200a491ad13deec9e386eba58, but the running executable is 86e887504731ddc7c94e4cb69b2e6f3773ed1d9cf9eca45658e1226c1ef820b8
$ echo $?
1
```

The advance from the changed-manifest refusal to the binary-digest refusal is positive evidence that the on-disk manifest bytes match their committed content. The remaining refusal is an inspection-tool defect, not a manifest defect: `permanent_dataset emission-check` calls `approve_emission`, which hashes the currently running `permanent_dataset` executable (`86e887504731ddc7c94e4cb69b2e6f3773ed1d9cf9eca45658e1226c1ef820b8`), while the manifest correctly pins the HIP-enabled `permanent_campaign` emitter (`2d6edcd940abe9340143c8b724a8274fff8eca1200a491ad13deec9e386eba58`). The writer applies the same guard to its own matching digest, so campaign execution is unaffected. No positive writer-only dry run exists; invoking the write path would violate the pre-draw constraint. The defect is tracked as issue `e1d45c20`, to be resolved or explicitly dispositioned before the first campaign draw.

The content-hash recomputation and sidecar comparison succeed over the final manifest bytes:

```text
$ cd dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829
$ sha256sum manifest.json
c37305910037d5c0f0a41f51a6be3960a53d27ce66d63bda87c6ae45fb4b2952  manifest.json
$ sha256sum -c checksums.sha256
manifest.json: OK
```

Every external digest asserted directly by this freeze record is recomputed from its named bytes:

```text
$ sha256sum dev/simulation_results/permanent-zero-fraction/protocol.md dev/benchmarks/permanent_campaign/backend-selection-v1.md dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md target/release/permanent_campaign target/release/permanent_dataset dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json
249f3de398cd234cdd9c1f1d352fc909394f3bacf13acda606d95da343693639  dev/simulation_results/permanent-zero-fraction/protocol.md
fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe  dev/benchmarks/permanent_campaign/backend-selection-v1.md
d3779aa3817bbefc995b858527bf418b4d3931a8f5406281eb4f0b71a7b45e77  dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv
839ab3f1f22a995cc32bc6a7102f4c6aad721b6eff5713d3976df36838d2334a  dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md
2d6edcd940abe9340143c8b724a8274fff8eca1200a491ad13deec9e386eba58  target/release/permanent_campaign
86e887504731ddc7c94e4cb69b2e6f3773ed1d9cf9eca45658e1226c1ef820b8  target/release/permanent_dataset
c37305910037d5c0f0a41f51a6be3960a53d27ce66d63bda87c6ae45fb4b2952  dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json
```

The committed v5 validator independently checks all direct determinant measurements and the rendered report:

```text
$ python3 dev/benchmarks/permanent_campaign/determinant_cost_v5.py validate --receipt dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv --report dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md
PASS: 63 cells, 315 process outcomes, receipt sha256 d3779aa3817bbefc995b858527bf418b4d3931a8f5406281eb4f0b71a7b45e77
```

The pre-draw worktree and history checks return:

```text
PASS: no non-fixture shard, checkpoint, field summary, or pooled summary exists in the worktree
PASS: repository history contains no non-fixture campaign shard or checkpoint path
```

No Rust source changes are part of this freeze, so `./scripts/cargo-ci.sh` is not run. Dataset `conform`, `verify`, and full-dataset checksum generation or verification are also not run because no campaign shard exists.
