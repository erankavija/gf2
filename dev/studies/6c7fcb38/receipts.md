# $\mathbb{F}_7$ preregistered receipt campaign — committed receipts

Campaign run `20260814T230032Z-2085453`, field $q = 7$, grid execution id
`7002`. Every figure below is derived from the raw artifacts committed beside
this file; [`analysis.py`](analysis.py) in this directory regenerates all of
them from those artifacts and prints them under the section headings used here.

```sh
python3 dev/studies/6c7fcb38/analysis.py
```

| Artifact | Role |
| --- | --- |
| [`…-q7-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q7-grid.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q7-grid.log) | End-to-end Ryser timing grid, 75 cells |
| [`…-q7-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q7-gray-update.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q7-gray-update.log) | Dependency-chained Gray-update isolate, five orders |
| [`…-q7-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q7-horizontal-product.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q7-horizontal-product.log) | Horizontal-product isolate and zero/nonzero branch frequencies, five orders |
| [`../047b62ed/…-shared-equivalence.csv`](../047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv) / [`.log`](../047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.log) | Backend-equivalence gate, all fields in one global run |
| [`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt) | Revision, toolchain, binary hashes, host inventory, exact commands |
| [`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt) | Per-step status and exit codes |
| [`hip-resource-usage-20260814T172506Z-1610002/`](hip-resource-usage-20260814T172506Z-1610002/) | Compiler kernel-resource receipt and per-kernel logs |
| [`analysis.py`](analysis.py) | Derivation of every table here from the artifacts above |

Each CSV carries a `#` preamble that is the authoritative source for the facts
it embeds. Where this document quotes a provenance fact, the citation names the
file the fact comes from rather than restating it as an independent claim.

## 1. Provenance and reproduction (REQ-02, REQ-15)

Measurement revision `292320d5e7d1fefd6b94262d0960e40c8ea35ba1`; Rust toolchain
`1.95.0`, `rustc 1.95.0 (59807616e 2026-04-14)`; device compiler `hipcc`
reporting HIP `7.2.53211-9999` and AMD clang `22.0.0git`; ROCm runtime `7.2.4`
as recorded by the harness preamble. Host: AMD Ryzen 9 5900X 12-Core Processor,
24 logical CPUs, AVX2 present and AVX-512F absent, governor `powersave`; GPU
`card0`, AMD Radeon RX 6950 XT, `gfx1030`, unique id `0x8cd14d6d8a3c8a73`;
kernel `7.1.6-arch1-1`. Harness binary SHA-256
`edb03650bbbfb47064a3a2b5e021ac055394812a4362c577fdad07cdbd997bce`; the
$\mathbb{F}_7$ prototype executable `wave-gf7-device-evidence` SHA-256
`97c132c4af31697738ca0ea4ca47040bf8dedada1d5b9ce5283e9c240278c0ce`. Sources:
[`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt)
and the `#` preamble of each CSV.

Four binaries carry the measurement: the harness itself and the three prototype
device executables. All four are built and hash-pinned by the runner's
`prepare` step, which writes the binary manifest and the compiler resource
receipt in one pass from the same `git rev-parse HEAD`
([`dev/scripts/permanent-campaign-runner.sh:345`](../../scripts/permanent-campaign-runner.sh),
`:363`), so the receipt's `source_revision`
`f9224650a780ea8ed98bcdff6e662cdb0d7b94f4` is the revision the binaries were
built at, with `tracked_worktree_dirty=false` at that moment
([`hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](hip-resource-usage-20260814T172506Z-1610002/receipt.txt)).
The measure run executes at HEAD `292320d5`, one commit ahead: that commit
touches tracker state, two gate review prompts, and a package manifest, and no
file under `crates/` or `dev/research/`. The binaries are not rebuilt across it,
and the run does not take that on trust: `verify_manifest` re-hashes every
manifest binary and aborts on any mismatch
(`permanent-campaign-runner.sh:153-171`), and it runs three times per campaign,
before queueing for the mutex (`:647`), inside `run_campaign` (`:602`), and
again in the lock-held child (`:624`). The provenance file records the outcome
as matching `binary_hash` and `binary_manifest_hash` pairs for all four
binaries.

The three commands that produced this field's three CSVs are recorded verbatim
under `exact_commands_executed:` in
[`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt):107
and repeated in the `# invocation:` line of each CSV and the `# command:` line
of each log. The equivalence step of §3 is one global invocation shared by all
three fields, so its command is recorded in the $\mathbb{F}_3$ study's
provenance file beside the CSV it wrote, and in that CSV's own `# invocation:`
line. This field's own run summary carries all four steps that touch it — the
shared equivalence step and the three $q = 7$ steps — and each reports
`status=completed exit=0`
([`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt)).

The campaign runs under
[`dev/scripts/permanent-campaign-runner.sh`](../../scripts/permanent-campaign-runner.sh),
which holds the repository's canonical benchmark mutex `/tmp/gf2-ccx1.lock`
through
[`dev/scripts/ccx1-bench-flock.sh --full-host`](../../scripts/ccx1-bench-flock.sh):18,
`:27` for the whole internal pipeline rather than once per step
(`permanent-campaign-runner.sh:604-607`); `--full-host` deliberately omits
`taskset` because the grid's rayon cells are named on the full processor while
still sharing the one lock domain (`ccx1-bench-flock.sh:13-15`). `measure`
refuses a non-pristine tracked worktree twice: once before queueing for the
mutex and again inside the lock-held child before any step runs
(`permanent-campaign-runner.sh:646`, `:623`), so a commit or a rebuilt binary
landing during the lock wait cannot reach the campaign unchecked.

**This field's grid carries `--skip-machine-warmup`, and that is a property of
the pipeline rather than of the cell.** The 90 s full-rayon machine warm-up
belongs to the first grid executed under the held lock, which is the $q = 3$
grid; every later grid under the same lock passes the flag and inherits the
warmed host (`permanent-campaign-runner.sh:564-580`). This field's grid is the
third under that one lock — the CSV timestamps run `2026-08-14T23:00:34Z` for
the shared equivalence step, then `2026-08-15T01:31:59Z`, `01:43:38Z`, and
`01:46:10Z` for this field's grid, Gray-update, and horizontal-product steps —
and the lock is never released between them. The $q = 7$ grid preamble records
the inheritance as `machine warmup: skipped by --skip-machine-warmup; caller
must preserve a prior locked warmup`. Every cell still performs its own untimed
warm-up of at least 3 s before its first timed repetition, which the preamble's
`protocol:` line states and `WARMUP_SECONDS`
(`dev/research/permanent-sampling-feas/src/protocol.rs:101`) fixes.

The `tracked_worktree_dirty: true` line in
[`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt)
is the state at the moment the provenance block is written, which is mid-run
and after the runner has already emitted its own outputs under `dev/studies/`
(`permanent-campaign-runner.sh:179-187`). The pristine condition that gates the
run is the runner's own double refusal cited above, not that line. The
`ambient_rustc_command` block in the same file records `rustc: command not
found` with `command_exit_status: 127`: the scheduling environment has no
ambient `rustc` on `PATH`, which is why the CSV preambles carry `rustc:
unavailable` and `cargo: unavailable`. The toolchain actually used is pinned by
`rust_toolchain`, `build_rustc`, and the binary SHA-256 in the same file, and
the binary hash covers harness, path dependencies, toolchain, and compiled
feature set together.

Reproduction requires the exact commands as recorded, including `--only q=7`,
`--execution-id 7002`, and `--skip-machine-warmup`. A cell's first stream index
is `execution_id * 22 500 000 + order_index * 100 000 + 1`
(`dev/research/permanent-sampling-feas/src/main.rs:371-391`, `:54`), and
`order_index` is the cell's position after the spec list is shuffled with
`SEED_ROOT` and stably sorted by ascending $n$ (`main.rs:593-598`). The shuffle
runs over the filtered list (`:576`), so the same execution id with a different
`--only` filter addresses different matrices. Execution `7002` therefore owns
`157545000001..=157567500000`, which is the block the grid preamble records and
which `analysis.py` section 1 recomputes.

The stable sort by ascending $n$ exists for a reason this field is the extreme
case of, and the harness says so in place: without the guarantee that a cell
runs after a smaller $n$ on its own $(q, \text{backend}, M)$ chain, a cell with
no projection reference falls back to probing, "*and at q=7, n=28 on the GPU a
single-matrix probe costs 42 minutes*" (`main.rs:586`). Every censored cell in
§5 projects from a measured reference rather than from a probe because of that
ordering.

## 2. Candidate roster and what executes (REQ-01, REQ-13)

The grid enumerates the full planned candidate set from the prototype registry
rather than a pre-filtered one; every `Backend::ALL` entry appears at every
order, and the single GPU entry expands to both configured batch sizes
(`dev/research/permanent-sampling-feas/src/main.rs:307-334`, `:50`). The file
holds 75 cells: 15 per order, of which 25 are `measured`, 4 are `censored`, and
46 are `unsupported`. Every non-`measured` cell carries its reason in `note`.

| Path | Class | Grid outcome | Reason recorded in `note` |
| --- | --- | --- | --- |
| `gpu_hip` (`permanent_bipedal7_kernel`) | current GPU path | `measured` at $M \in \{256, 1024\}$ and $n \in \{12, 16, 20\}$; `censored` in all four cells at $n \in \{24, 28\}$ | see §5 |
| `f7-lookup-table-control` (`wave_gf7_lookup_table_kernel<W>`) | planned $\mathbb{F}_7$ prototype, lane-owns-interval | `measured` at every order | — |
| `f7-three-plane-permanent` (`prepare_three_plane_columns` then `wave_gf7_three_plane_kernel`) | planned $\mathbb{F}_7$ prototype, lane-owns-interval, bit-sliced | `measured` at every order | — |
| `f7-three-plane-accumulator` | planned $\mathbb{F}_7$ prototype | `unsupported` at every order | `f7-three-plane-accumulator has no device batch evaluator: hip/f7_three_plane_equivalence.hip holds a single-thread three-plane accumulator conformance probe, not a full-permanent batch kernel; the permanent-shaped use of this arithmetic is the f7-three-plane-permanent candidate` |
| `cpu_scalar` (`permanent_bipedal7`) | in-tree CPU | `measured` at $n \in \{12, 16\}$; `unsupported` at $n \in \{20, 24, 28\}$ | `permanent_bipedal7 asserts n <= Packed7::LANES = 16; n = <n>` |
| `cpu_rayon_batch_scalar` (`permanent_bipedal7` per matrix across rayon workers) | in-tree CPU | `measured` at $n \in \{12, 16\}$; `unsupported` at $n \in \{20, 24, 28\}$ | same lane bound |
| `cpu_ryser_generic` (`permanent_ryser`) | in-tree CPU | `measured` at every order | — |
| `cpu_avx2` | in-tree CPU | `unsupported` | `gf2-algebra exposes no AVX2 permanent path for F_7` |
| `cpu_rayon_batch_avx2` | in-tree CPU | `unsupported` | `gf2-algebra exposes no AVX2 permanent path for F_7` |
| `cpu_rayon_intra_matrix` | in-tree CPU | `unsupported` | `gf2-algebra exposes no rayon permanent path for F_7` |
| `wave-gf3` | planned $\mathbb{F}_3$ prototype | `unsupported` | `wave-gf3 evaluates F_3 permanents, not F_7` |
| `fold-gf3` | planned $\mathbb{F}_3$ prototype | `unsupported` | `fold-gf3 evaluates F_3 permanents, not F_7` |
| `f5-byte-control` | planned $\mathbb{F}_5$ prototype | `unsupported` | `f5-byte-control evaluates F_5 permanents, not F_7` |
| `f5-three-plane` | planned $\mathbb{F}_5$ prototype | `unsupported` | `f5-three-plane evaluates F_5 permanents, not F_7` |

The kernel each CPU row forces is read from the harness rather than assumed:
`cpu_scalar` and `cpu_rayon_batch_scalar` both call
`gf2_algebra::permanent::bipedal7::permanent_bipedal7` over a `Packed7Matrix`,
the second across rayon workers one matrix each
(`dev/research/permanent-sampling-feas/src/backend.rs:413-414`, `:435-437`);
`cpu_ryser_generic` calls the field-agnostic `permanent_ryser` over unpacked
`Fp<7>` elements (`:461`); `gpu_hip` calls
`gf2_algebra::gpu::permanent_batch_bipedal7` (`:476`).

**This field's packed CPU kernel stops at $n = 16$, and that is what makes its
CPU baseline change character mid-grid.** `permanent_bipedal7` asserts
`n <= Packed7::LANES = 16`, and the harness records the refusal rather than
silently substituting another kernel
(`dev/research/permanent-sampling-feas/src/backend.rs:258`, `:277`). So at
$n \in \{12, 16\}$ the best applicable in-tree CPU path is batch rayon over the
packed kernel, and at $n \in \{20, 24, 28\}$ it is the field-agnostic
`cpu_ryser_generic`, which is the only in-tree CPU path that runs there at all.
§4.2 identifies it per order from this run's own data rather than assuming it.

**Both bit-sliced arithmetic candidates are planned, and only one of them
executes as a permanent path.** `f7-three-plane-permanent` reaches
`wave_gf7_three_plane_kernel` and is `measured` in all five of its grid cells.
`f7-three-plane-accumulator` is `unsupported` in all five, and in every
equivalence row, with the structural reason quoted in the table above. This is
REQ-13's subject for this field and it is not vacuous: the candidate's HIP
source compiles clean for `gfx1030` — the receipt of §7 carries
`f7_three_plane_equivalence` at 40 `TotalSGPRs` and 2 `VGPRs`
(`f7_three_plane_equivalence.hip.resource.log:1`) — so the falsification is
neither a compile failure nor a resource exhaustion but a structural one: what
that translation unit holds is a single-thread arithmetic conformance probe and
not a full-permanent batch kernel, and the permanent-shaped use of the same
three-plane arithmetic is the `f7-three-plane-permanent` candidate that *is*
measured. Its exclusion from the timing comparison cites exactly that, in the
harness's own words, and no timing is attributed to it in §4. It does appear in
the horizontal-product isolate of §6.2, because that isolate times a circuit
rather than a permanent path.

A registered candidate's grid cell evaluates that candidate's own device kernel
over the same batch object the built-in backends receive
(`dev/research/permanent-sampling-feas/src/backend.rs:505-531`), reaching the
kernel through the candidate's prebuilt HIP executable, which stays resident
for the cell so a batch pays the pipe transfer and the executable's own
per-batch device allocation rather than a process start
(`dev/research/permanent_wave_gpu/src/device_batch.rs:9-13`). The launch
geometry is one block per matrix and `active_lanes_for_order(n)` lanes per
block, which is 32 at every order this campaign measures
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31`,
`:16`), and each lane owns a balanced Gray interval (`:33-40`).

Three in-tree CPU paths are excluded for a different reason, and the record
keeps it distinct from a device falsification. `cpu_avx2`,
`cpu_rayon_batch_avx2`, and `cpu_rayon_intra_matrix` are `unsupported` at every
order because `gf2-algebra` implements no AVX2 and no rayon intra-matrix
permanent path for this field
(`dev/research/permanent-sampling-feas/src/backend.rs:286`, `:299`); the
feasibility study records the same absence, that "*F_5 and F_7 have only scalar
kernels*"
([`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md):374).
That is a library-capability absence in the host tree, verified by the same
reason string appearing in this run's equivalence file (§3). It is not a
compile, correctness, or resource falsification on `gfx1030`, and it is not
presented as one. §16 records that reading.

## 3. Equivalence gate (REQ-11)

The equivalence step is the first step of the pipeline
([`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt);
`timestamp_utc: 2026-08-14T23:00:34Z` in the equivalence CSV against
`2026-08-15T01:31:59Z` in the grid CSV), so it precedes every timing cell of the
same run, on the same host and the same binary hash.

The harness `equivalence` subcommand is global: one invocation covers all three
fields and takes no field filter, and `--execution-id` is parsed by `grid`
alone. The run summary therefore records `execution_id=fixed-streams` for this
step, which is what the step does — it draws from fixed stream addresses of the
form $(\text{seed\_root}, q, n, \texttt{equivalence}, 0)$ recorded in its own
preamble, rather than from a reserved per-execution index block. One global run
gates all three fields; this document reads its $q = 7$ rows.

Every backend at a given $(q, n)$ is compared against the *same* matrices: the
batch is built once and reused across the whole backend loop, registered
prototype candidates included
(`dev/research/permanent-sampling-feas/src/equivalence.rs:139`, `:186-192`), and
the preamble states the same contract on its `candidates:` line.

**The reference kernel changes at $n = 20$, and the file says so before any
verdict is read.** Its `check:` preamble line states that the scalar
single-word kernel is the reference wherever it exists, "*at q=7, n>16 it does
not (permanent_bipedal7 asserts n <= Packed7::LANES = 16), so the generic
permanent_ryser is the reference there*". So this field's oracle is
`cpu_scalar` at $n \le 16$ and `cpu_ryser_generic` above it, and the number of
paths under comparison falls from five to three at the same step, because the
two packed CPU paths leave the comparison rather than joining it.

| $n$ | reference | backends compared against it | `matrices` | `mismatches` | `zeros_reference` = `zeros_backend` | `status` |
| ---: | --- | --- | ---: | ---: | ---: | --- |
| 8 | `cpu_scalar` | `cpu_rayon_batch_scalar`, `gpu_hip`, `cpu_ryser_generic`, `f7-lookup-table-control`, `f7-three-plane-permanent` | 512 | 0 | 64 | `identical` |
| 12 | `cpu_scalar` | same five | 512 | 0 | 65 | `identical` |
| 16 | `cpu_scalar` | same five | 512 | 0 | 82 | `identical` |
| 20 | `cpu_ryser_generic` | `gpu_hip`, `f7-lookup-table-control`, `f7-three-plane-permanent` | 512 | 0 | 63 | `identical` |
| 24 | `cpu_ryser_generic` | same three | 32 | 0 | 5 | `identical` |
| 28 | `cpu_ryser_generic` | same three | 4 | 0 | 0 | `identical` |

All 24 $q = 7$ comparison cells report `mismatches = 0` and
`status = identical`, including both executing prototypes at every order the
grid times, and the log closes with `all backends agree per matrix`. The whole
file carries 102 `identical` rows across the three fields. The 54 remaining
$q = 7$ rows report `matrices = 0` with the reasons of §2 — three in-tree CPU
paths absent for this field, two packed CPU paths past their lane bound above
$n = 16$, four out-of-field prototypes, and the accumulator's structural
absence — and are not equivalence evidence in either direction.

The equivalence order set covers every grid order and adds $n = 8$ below them,
so no timing in §4 rests on equivalence confirmed only at a smaller order. The
counts fall to 32 and 4 at $n = 24$ and $n = 28$ because the harness sizes each
cell against a 240 s budget from committed per-matrix costs
(`dev/research/permanent-sampling-feas/src/main.rs:64-77`); the preamble's
`sample counts:` line records that only the $q = 5$, $n = 28$ cell halves below
its ceiling, so this field keeps its per-order ceiling at every order. A
four-matrix cell is a much weaker gate than a 512-matrix one, and it is the gate
this field's largest order actually has.

## 4. End-to-end Ryser timing grid (REQ-01, REQ-02, REQ-05, REQ-06)

### 4.1 Recorded fields

REQ-02 asks for a specific field set. Each is a named column of
[`…-q7-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q7-grid.csv), or a
line of the provenance file for the facts that are constant across the run.

| REQ-02 item | Where recorded |
| --- | --- |
| matrix order | `n` |
| batch size | `batch_size` |
| seed | `seed_root` and `seed_index_first` (with `timed_index_first` for the timed draw) |
| purpose tag | `timed_purpose` (`grid_timed`; warm-up uses `grid_warmup` and the sizing probe `grid_probe`, disjoint seed-address domains — `dev/research/permanent-sampling-feas/src/sampler.rs:120-125`, `:141-145`) |
| kernel-only throughput | `matrices` / `kernel_device_s`, tabulated in §4.3 |
| end-to-end throughput | `composite_matrices_per_s` (generate + evaluate + reduce + store) and `eval_matrices_per_s` |
| launch duration | `host_submission_s` and `device_submission_to_kernel_s` |
| git revision | `source_revision` in the provenance file |
| CPU model | `# cpu:` preamble line, `lscpu` block in the provenance file |
| GPU model | `# gpu:` preamble line, `rocm-smi` block in the provenance file |
| ROCm version | `# rocm:` preamble line (`7.2.4`); `hipcc --version` in the provenance file |
| compiler version | `rust_toolchain`, `build_rustc`, `rocm_hipcc_version_command`, `amd_clang_version_command` in the provenance file |
| censoring / failure observation | `outcome`, `note`, `phase_timing_note`, `projected_matrices_per_s`, `projection_reference_n` |

Supporting columns present on every row: `reps`, `matrices`, `zeros`,
`total_s`, `gen_s`, `eval_s`, `reduce_s`, `store_s`, `probe_matrix_s`,
`rep_min_s`, `rep_max_s`, `rep_sd_s`, `threads`, `pinned_core`, `cpu_mhz_mean`,
`cpu_temp_c`, `gpu_temp_c`, `order_index`.

One artifact-format note, because it affects any reader who parses the file:
the grid writer emits `phase_timing_note` and `note` unquoted, and the
out-of-field rows carry a literal comma inside both (`… evaluates F_5
permanents, not F_7`), so those lines hold more fields than the 38-column
header names, and `note` is not the only column past the split. `analysis.py`
splits them on the `seed_root` prefix rather than on field count. The defect is
tracked as bug `3ea21d74`. The equivalence CSV carries the same literal commas,
but its free text is its last column, so joining the surplus fields back onto
`status` recovers it. The two isolate CSVs use semicolons inside their free text
and need no handling at all.

### 4.2 Composite throughput and the best path per order

Composite matrices per second, the rate the study's envelope is derived from.
Full grid in the CSV; `analysis.py` section 4.2 prints every one of the 25
measured cells.

| $n$ | best overall | rate | best in-tree CPU path | rate | best prototype | rate | best GPU config | rate | prototype / CPU | GPU / CPU |
| ---: | --- | ---: | --- | ---: | --- | ---: | --- | ---: | ---: | ---: |
| 12 | `f7-lookup-table-control` | 630 625.0668 | `cpu_rayon_batch_scalar` | 71 761.1277 | `f7-lookup-table-control` | 630 625.0668 | `gpu_hip` $M{=}1024$ | 20 966.9089 | 8.7878 | 0.2922 |
| 16 | `f7-three-plane-permanent` | 301 514.5246 | `cpu_rayon_batch_scalar` | 3 703.3679 | `f7-three-plane-permanent` | 301 514.5246 | `gpu_hip` $M{=}1024$ | 1 109.4474 | 81.4163 | 0.2996 |
| 20 | `f7-three-plane-permanent` | 8 397.8509 | `cpu_ryser_generic` | 15.3900 | `f7-three-plane-permanent` | 8 397.8509 | `gpu_hip` $M{=}1024$ | 57.9362 | 545.6693 | 3.7645 |
| 24 | `f7-three-plane-permanent` | 265.5329 | `cpu_ryser_generic` | 0.7979 | `f7-three-plane-permanent` | 265.5329 | all four cells censored | — | 332.7897 | — |
| 28 | `f7-three-plane-permanent` | 3.0407 | `cpu_ryser_generic` | 0.0427 | `f7-three-plane-permanent` | 3.0407 | all four cells censored | — | 71.2108 | — |

**The prototype-over-CPU column is not one quantity across the five rows, and
the reason is §2's lane bound rather than anything about the device.** At
$n \in \{12, 16\}$ the denominator is batch rayon over the packed
`permanent_bipedal7` kernel; at $n \in \{20, 24, 28\}$ it is single-thread
`cpu_ryser_generic`, because no packed CPU kernel runs there and no rayon
permanent path exists for this field at all (§2). The 545.6693 at $n = 20$ is
therefore a ratio against a single-threaded generic driver, and the 81.4163 at
$n = 16$ a ratio against 24 rayon workers on a packed kernel. Both are the ratio
REQ-14 and REQ-19 ask for, against the *best applicable* in-tree CPU path, and
the discontinuity between them belongs to the CPU side of the fraction.

**The shipped GPU path beats the CPU at exactly one order, and it is the same
change of denominator that puts it there.** Its ratio is 0.2922 and 0.2996 at
$n = 12$ and $n = 16$, and 3.7645 at $n = 20$. Nothing about the kernel changes
at that step; what changes is that batch rayon leaves the comparison.

**The bit-sliced path leads at four of five orders, and loses the fifth to the
lookup control.** `f7-three-plane-permanent` over `f7-lookup-table-control` is
0.4524, 2.0661, 20.5474, 168.1439, and 66.1022 at $n = 12$ through $28$. §4.5
records that the three largest of those figures are measured at batches the same
calibration sized 1.4, 9, and 3 times apart, and that this run does not separate
the circuit effect from the batch effect.

**The two corpus regimes.** A matrix is fully determined by
$(\text{seed\_root}, q, n, \text{purpose}, \text{stream\_index})$
(`dev/research/permanent-sampling-feas/src/sampler.rs:232-244`) and the draw is
backend-independent — "*Both forms draw the same entries from the same stream,
so a cell's sample does not depend on which backend measures it*"
(`dev/research/permanent-sampling-feas/src/protocol.rs:362-363`). Timing cells
draw from that one sampler on preregistered, structurally disjoint per-cell
stream addresses: each cell owns a reserved block of 100 000 stream indices
keyed on its `order_index` (`main.rs:54`, `:371-391`, `:593-598`) — "*Every
backend cell draws from its own reserved purpose/index address range, so pooling
across backends pools independent samples*" (`main.rs:1060-1062`).
The equivalence comparison of §3 runs on one literally identical matrix corpus,
built once per $(q, n)$ and reused for every path. Both regimes are what REQ-01
asks for, and each is used where it belongs.

### 4.3 Device phase separation (REQ-05, REQ-06)

`kernel_device_s` is a device-event kernel-only total. `h2d_device_s`,
`d2h_device_s`, and `device_submission_to_kernel_s` are separate device-event
totals; `host_submission_s` is a host-clock total. No column subtracts a host
timestamp from a device one, and CPU rows leave all five empty with
`phase_timing_note = event timing unavailable: backend is not GPU/HIP` rather
than substituting an evaluator wall clock
(`dev/research/permanent-sampling-feas/src/protocol.rs:1173`). The four
not-attempted `gpu_hip` cells of §5 leave the same five columns empty with
`phase_timing_note = event timing unavailable: cell did not run`
(`protocol.rs:119`).

Totals per cell, in seconds. `residual` is
`eval_s − kernel_device_s − h2d_device_s − d2h_device_s − device_submission_to_kernel_s`,
the host-side allocation, serialisation, stream wait, and free that the
dispatcher performs around each call.

| $n$ | path | $M$ | outcome | `eval_s` | `kernel_device_s` | `h2d_device_s` | `d2h_device_s` | `host_submission_s` | `device_submission_to_kernel_s` | residual | kernel / eval |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `f7-lookup-table-control` | 5 833 | measured | 2.009350 | 0.285233 | 0.020867 | 0.009474 | 0.514207 | 0.002322 | 1.691454 | 0.1420 |
| 12 | `f7-three-plane-permanent` | 46 | measured | 3.592782 | 1.199132 | 0.278535 | 0.497640 | 2.942379 | 0.145221 | 1.472254 | 0.3338 |
| 12 | `gpu_hip` | 256 | measured | 4.968501 | 4.721292 | 0.002405 | 0.001778 | 0.000629 | 0.000912 | 0.242114 | 0.9502 |
| 12 | `gpu_hip` | 1 024 | measured | 4.943577 | 4.762272 | 0.001873 | 0.000983 | 0.000349 | 0.000496 | 0.177953 | 0.9633 |
| 16 | `f7-lookup-table-control` | 695 | measured | 3.950914 | 3.166891 | 0.017674 | 0.016989 | 3.353777 | 0.004796 | 0.744564 | 0.8016 |
| 16 | `f7-three-plane-permanent` | 1 109 | measured | 2.858362 | 1.359174 | 0.027209 | 0.020655 | 1.649552 | 0.006034 | 1.445290 | 0.4755 |
| 16 | `gpu_hip` | 256 | measured | 5.242316 | 5.223382 | 0.000160 | 0.000105 | 0.000046 | 0.000054 | 0.018615 | 0.9964 |
| 16 | `gpu_hip` | 1 024 | measured | 5.529770 | 5.511976 | 0.000136 | 0.000058 | 0.000030 | 0.000028 | 0.017572 | 0.9968 |
| 20 | `f7-lookup-table-control` | 29 | measured | 5.033315 | 5.024030 | 0.000693 | 0.001241 | 5.030627 | 0.000347 | 0.007004 | 0.9982 |
| 20 | `f7-three-plane-permanent` | 41 | measured | 4.903451 | 4.779097 | 0.012121 | 0.017148 | 4.848719 | 0.004960 | 0.090125 | 0.9746 |
| 20 | `gpu_hip` | 256 | measured | 45.652425 | 45.639768 | 0.000083 | 0.000048 | 0.000038 | 0.000025 | 0.012501 | 0.9997 |
| 20 | `gpu_hip` | 1 024 | measured | 88.362480 | 88.342801 | 0.000166 | 0.000050 | 0.000042 | 0.000025 | 0.019438 | 0.9998 |
| 24 | `f7-lookup-table-control` | 2 | measured | 6.332094 | 6.330204 | 0.000049 | 0.000118 | 6.332071 | 0.000025 | 0.001698 | 0.9997 |
| 24 | `f7-three-plane-permanent` | 18 | measured | 5.012180 | 5.002998 | 0.000702 | 0.001301 | 5.009652 | 0.000359 | 0.006820 | 0.9982 |
| 24 | `gpu_hip` | 256 | censored | — | — | — | — | — | — | — | — |
| 24 | `gpu_hip` | 1 024 | censored | — | — | — | — | — | — | — | — |
| 28 | `f7-lookup-table-control` | 1 | measured | 108.616118 | 108.614027 | 0.000056 | 0.000152 | 108.616101 | 0.000025 | 0.001858 | 1.0000 |
| 28 | `f7-three-plane-permanent` | 3 | measured | 5.919579 | 5.917556 | 0.000058 | 0.000134 | 5.919528 | 0.000029 | 0.001802 | 0.9997 |
| 28 | `gpu_hip` | 256 | censored | — | — | — | — | — | — | — | — |
| 28 | `gpu_hip` | 1 024 | censored | — | — | — | — | — | — | — | — |

All four censored cells were never attempted and have no spans to retain. No
$q = 7$ cell in this grid is censored after executing, so unlike the
$\mathbb{F}_5$ grid this field has no cell that keeps diagnostic spans while
losing its rates.

**`host_submission_s` measures two different things, and the ratio to `eval_s`
says which.** On `gpu_hip` rows it is the instrumented dispatcher's own
asynchronous submission span (`backend.rs:613`), and it is $0.0000$–$0.0001$ of
`eval_s`. On prototype rows it is a host clock wrapped around the candidate's
whole synchronous batch evaluation (`backend.rs:516-520`), and it is
$0.2559$–$1.0000$ of `eval_s`, reaching $0.9888$–$1.0000$ from $n = 20$ up,
where the device work dominates. A prototype row's `host_submission_s` is
therefore not a launch overhead and is not read as one here. The dual semantics
are tracked as bug `79c5ace7`. The device-clock
`device_submission_to_kernel_s` is the launch measure that is comparable across
all four device paths, and §4.4 uses it.

Kernel-only throughput against end-to-end throughput. Kernel-only is
`matrices` / `kernel_device_s`, so it charges the device kernel span alone;
`eval_matrices_per_s` charges the whole dispatch; `composite_matrices_per_s`
charges generation, evaluation, reduction, and store. A censored cell carries
no throughput value in any of the three.

| $n$ | path | $M$ | outcome | `matrices` | kernel-only matrices/s | eval-only matrices/s | composite matrices/s | kernel-only / composite |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 12 | `f7-lookup-table-control` | 5 833 | measured | 2 910 667 | 10 204 524.0207 | 1 448 561.5566 | 630 625.0668 | 16.1816 |
| 12 | `f7-three-plane-permanent` | 46 | measured | 1 370 570 | 1 142 968.4138 | 381 478.7901 | 285 266.5608 | 4.0067 |
| 12 | `gpu_hip` | 256 | measured | 48 128 | 10 193.8198 | 9 686.6232 | 9 606.8305 | 1.0611 |
| 12 | `gpu_hip` | 1 024 | measured | 105 472 | 22 147.4120 | 21 335.1589 | 20 966.9089 | 1.0563 |
| 16 | `f7-lookup-table-control` | 695 | measured | 713 765 | 225 383.5070 | 180 658.2152 | 145 932.9173 | 1.5444 |
| 16 | `f7-three-plane-permanent` | 1 109 | measured | 1 438 373 | 1 058 269.9492 | 503 215.8091 | 301 514.5246 | 3.5098 |
| 16 | `gpu_hip` | 256 | measured | 2 816 | 539.1143 | 537.1672 | 536.7621 | 1.0044 |
| 16 | `gpu_hip` | 1 024 | measured | 6 144 | 1 114.6638 | 1 111.0770 | 1 109.4474 | 1.0047 |
| 20 | `f7-lookup-table-control` | 29 | measured | 2 059 | 409.8304 | 409.0744 | 408.7068 | 1.0027 |
| 20 | `f7-three-plane-permanent` | 41 | measured | 41 943 | 8 776.3442 | 8 553.7720 | 8 397.8509 | 1.0451 |
| 20 | `gpu_hip` | 256 | measured | 1 280 | 28.0457 | 28.0379 | 28.0363 | 1.0003 |
| 20 | `gpu_hip` | 1 024 | measured | 5 120 | 57.9561 | 57.9431 | 57.9362 | 1.0003 |
| 24 | `f7-lookup-table-control` | 2 | measured | 10 | 1.5797 | 1.5793 | 1.5792 | 1.0003 |
| 24 | `f7-three-plane-permanent` | 18 | measured | 1 332 | 266.2404 | 265.7526 | 265.5329 | 1.0027 |
| 24 | `gpu_hip` | 256 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 24 | `gpu_hip` | 1 024 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 28 | `f7-lookup-table-control` | 1 | measured | 5 | 0.0460 | 0.0460 | 0.0460 | 1.0008 |
| 28 | `f7-three-plane-permanent` | 3 | measured | 18 | 3.0418 | 3.0408 | 3.0407 | 1.0004 |
| 28 | `gpu_hip` | 256 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 28 | `gpu_hip` | 1 024 | censored | 0 | withheld | `NaN` | `NaN` | — |

The kernel-only-to-composite ratio is the size of everything the device kernel
does not pay for. It is 16.1816× at $n = 12$ for `f7-lookup-table-control` and
1.0004× at $n = 28$ for the three-plane path: sampling, packing, reduction, and
the dispatcher's surrounding work cost 93.8 % of the achievable rate at the
smallest order on the fastest cell in this campaign, and 0.04 % at the largest.
On the shipped path the ratio never exceeds 1.0611×, because that kernel is slow
enough that its own span dominates the dispatch even at $n = 12$.

Per-launch cost, dividing each total by that cell's `reps`. Each timed
repetition opens one sampler at one stream index and evaluates one batch
(`dev/research/permanent-sampling-feas/src/protocol.rs:765-781`), and one batch
evaluation is one dispatch — `gridDim.x = M` blocks of one thread for the
shipped kernel
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip:350-351`,
`:363-366`), `gridDim.x = M` blocks of 32 lanes for the prototypes
(`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:677`, `:682`,
`:717`, `:721`) — so `reps` is the launch count. The three-plane row's kernel
column charges two launches per repetition rather than one, for the reason §12
gives.

| $n$ | path | $M$ | `reps` | host submission µs/launch | device submission→kernel µs/launch | kernel ms/launch | H2D µs/launch | D2H µs/launch |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `f7-lookup-table-control` | 5 833 | 499 | 1 030.4749 | 4.6533 | 0.5716 | 41.8176 | 18.9860 |
| 12 | `f7-three-plane-permanent` | 46 | 29 795 | 98.7541 | 4.8740 | 0.0402 | 9.3484 | 16.7021 |
| 12 | `gpu_hip` | 256 | 188 | 3.3457 | 4.8511 | 25.1133 | 12.7926 | 9.4574 |
| 12 | `gpu_hip` | 1 024 | 103 | 3.3883 | 4.8155 | 46.2357 | 18.1845 | 9.5437 |
| 16 | `f7-lookup-table-control` | 695 | 1 027 | 3 265.6056 | 4.6699 | 3.0836 | 17.2093 | 16.5424 |
| 16 | `f7-three-plane-permanent` | 1 109 | 1 297 | 1 271.8211 | 4.6523 | 1.0479 | 20.9784 | 15.9252 |
| 16 | `gpu_hip` | 256 | 11 | 4.1818 | 4.9091 | 474.8529 | 14.5455 | 9.5455 |
| 16 | `gpu_hip` | 1 024 | 6 | 5.0000 | 4.6667 | 918.6627 | 22.6667 | 9.6667 |
| 20 | `f7-lookup-table-control` | 29 | 71 | 70 853.9014 | 4.8873 | 70.7610 | 9.7606 | 17.4789 |
| 20 | `f7-three-plane-permanent` | 41 | 1 023 | 4 739.7058 | 4.8485 | 4.6716 | 11.8485 | 16.7625 |
| 20 | `gpu_hip` | 256 | 5 | 7.6000 | 5.0000 | 9 127.9536 | 16.6000 | 9.6000 |
| 20 | `gpu_hip` | 1 024 | 5 | 8.4000 | 5.0000 | 17 668.5602 | 33.2000 | 10.0000 |
| 24 | `f7-lookup-table-control` | 2 | 5 | 1 266 414.2000 | 5.0000 | 1 266.0408 | 9.8000 | 23.6000 |
| 24 | `f7-three-plane-permanent` | 18 | 74 | 67 698.0000 | 4.8514 | 67.6081 | 9.4865 | 17.5811 |
| 28 | `f7-lookup-table-control` | 1 | 5 | 21 723 220.2000 | 5.0000 | 21 722.8054 | 11.2000 | 30.4000 |
| 28 | `f7-three-plane-permanent` | 3 | 6 | 986 588.0000 | 4.8333 | 986.2593 | 9.6667 | 22.3333 |

What the separation shows. Device launch overhead is nearly constant across
every path, order, and batch size: `device_submission_to_kernel_s` is
4.65–5.00 µs per launch over kernel spans that range from 0.040 ms to 21.7 s,
nearly six orders of magnitude. Device-to-host copy is flat at 9.46–30.40 µs per
launch because each launch returns at most $M$ result words. Host-to-device copy
is 9.3–41.8 µs per launch and tracks $M$ and $n$. What does move is the
host-side residual: it is 84.2 % of `eval_s` at $n = 12$ on
`f7-lookup-table-control` and 41.0 % on the three-plane path at the same order,
is 18.8 % and 50.6 % at $n = 16$, 0.1 % and 1.8 % at $n = 20$, and is
under 0.2 % from $n = 24$ up; on the shipped path it is 4.9 % and 3.6 % at
$n = 12$ and under 0.4 % from $n = 16$ up. The dispatcher's per-call allocation,
serialisation, and free — not transfer and not launch — is what costs the
prototypes their smallest orders, and it is attributable from these columns
without a second run.

### 4.4 Best operating points (REQ-14)

The best operating point of a device path is taken to be the measured cell
whose composite throughput ratio against the best applicable in-tree CPU path
at that order is largest. Among the prototypes, the one whose best operating
point carries the highest such ratio is the best-performing prototype.

| Path | Best operating point | rate | best CPU path at that order | rate | ratio | device launch |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| `f7-three-plane-permanent` (best prototype) | $n = 20$, $M = 41$ | 8 397.8509 | `cpu_ryser_generic` | 15.3900 | **545.6693** | 4.8485 µs/launch |
| `f7-lookup-table-control` | $n = 12$, $M = 5833$ | 630 625.0668 | `cpu_rayon_batch_scalar` | 71 761.1277 | 8.7878 | 4.6533 µs/launch |
| `gpu_hip` (shipped) | $n = 20$, $M = 1024$ | 57.9362 | `cpu_ryser_generic` | 15.3900 | 3.7645 | 5.0000 µs/launch |

At `f7-three-plane-permanent`'s best operating point the launch duration is
`device_submission_to_kernel_s` 0.004960 s over 1 023 launches, or **4.8485 µs
per launch** on the device clock. The host-clock column at the same cell reads
`host_submission_s` 4.848719 s over 1 023 launches, or 4 739.7058 µs per launch,
which is the synchronous submit-and-complete span described in §4.3 rather than
a launch overhead. The shipped path's own figure at its best operating point is
5.0000 µs per launch on the device clock and 8.4000 µs on the host clock, both
single-digit microseconds because that path's host column is an asynchronous
submission.

**The best-performing prototype by ratio and the best by absolute rate are
different paths at different orders**, and both readings are printed by
`analysis.py` section 4.4. By highest absolute throughput the best prototype
cell in this campaign is `f7-lookup-table-control` at $n = 12$, $M = 5833$,
630 625.0668 matrices/s, which is 8.7878 times batch rayon at the same order.
By ratio against the best applicable CPU path it is `f7-three-plane-permanent`
at $n = 20$. REQ-14 asks for the best-performing prototype at its best operating
point, and this receipt reads that as the ratio criterion, so the headline is
545.6693× at 4.8485 µs per launch; the absolute-rate reading is stated here
rather than left for a reader to discover.

### 4.5 What the prototypes' batch sizes do to the comparison

The grid fixes `gpu_hip` at $M \in \{256, 1024\}$ and lets every other cell size
itself: a cell with no requested batch size runs a one-matrix probe and takes
$M = \lceil 2\,\text{s} / \texttt{probe\_matrix\_s} \rceil$, clamped to
$[\,\text{floor},\, 65\,536\,]$
(`dev/research/permanent-sampling-feas/src/protocol.rs:725-737`, and
`GPU_BATCHES` at `dev/research/permanent-sampling-feas/src/main.rs:50`). The
floor is one matrix for a single-threaded backend and
`MATRICES_PER_WORKER * rayon::current_num_threads()` for a multithreaded one,
which is $4 \times 24 = 96$ on this host (`protocol.rs:114`, `:731-737`).
`analysis.py` section 4.5 reconstructs $\lceil 2\,\text{s} / \texttt{probe} \rceil$
from each committed probe. It reproduces every calibrated cell's batch exactly
at $n \ge 16$; at $n = 12$ four cells differ by 2 to 16 matrices, and the cause
is the committed probe's printed precision rather than the rule — the CSV writes
`probe_matrix_s` to six decimals, and at $n = 12$ one printed value covers a
range of true probes. The script therefore checks each of those four against the
interval that rounding implies, and every one falls inside it. The rayon floor
never binds at this field, because both rayon cells run at $n \in \{12, 16\}$
with batches of 12 033 and 622.

A fixed-batch cell probes only when it has no projection reference
(`protocol.rs:687`), so `gpu_hip` carries a `probe_matrix_s` of 0.060482 at
$n = 12$, the smallest order, and `NaN` at every order above it. That is why
its probe column is empty where a reader might expect a figure.

For the prototypes the calibration produces batches of 1 to 5 833 matrices, none
of which matches the control's 256 or 1024.

Because a prototype block owns one matrix and 32 lanes, the batch size *is* the
device parallelism of the cell:

| $n$ | `f7-lookup-table-control` $M$ | active lanes | dynamic shared B/block | `f7-three-plane-permanent` $M$ | active lanes | dynamic shared B/block | `gpu_hip` $M$ | active lanes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 5 833 | 186 656 | 96 | 46 | 1 472 | 288 | 256 / 1024 | 256 / 1024 |
| 16 | 695 | 22 240 | 128 | 1 109 | 35 488 | 384 | 256 / 1024 | 256 / 1024 |
| 20 | 29 | 928 | 320 | 41 | 1 312 | 480 | 256 / 1024 | 256 / 1024 |
| 24 | 2 | 64 | 384 | 18 | 576 | 576 | 256 / 1024 | 256 / 1024 |
| 28 | 1 | 32 | 448 | 3 | 96 | 672 | 256 / 1024 | 256 / 1024 |

**The lookup control's collapse between $n = 16$ and $n = 20$ is where the
calibration hurts most, and the mechanism is in the committed columns.** Its
batch falls from 695 to 29, because `probe_matrix_s` rises from 0.002880 s to
0.070611 s, a 24.5× step against a 20× step in Ryser work. The achieved
per-matrix `eval_s` rises from 0.000005535 s to 0.002444543 s over the same
step, a 442× jump. The batch collapse accounts for most of it: the cell drops
from 22 240 active lanes to 928, and the probe-to-achieved amortisation ratio
falls from 520.3 to 28.9. A single-matrix probe is a latency and a batch is a
throughput, and this cell is where the gap between them closes far enough that
the calibration under-sizes the batch by a factor the measurement never
recovers. The consequence is stated rather than smoothed over: the 20.5×, 168×,
and 66× three-plane-over-lookup-control figures at $n = 20$, 24, and 28 are the
joint effect of the two circuits and of cells that run 1 312 against 928 lanes,
576 against 64, and 96 against 32.

The same mechanism runs the other way at $n = 12$, which is the one order where
the lookup control wins: there its probe is 0.000343 s against the three-plane
path's 0.044326 s, so the calibration gives it 5 833 matrices against 46, and
186 656 active lanes against 1 472. The 0.4524 ratio at that order is
therefore not evidence that the lookup circuit is the faster one, and §17
records it as an open question rather than a finding.

The probe is a single-matrix latency and the preamble forbids deriving a
batched rate from it; the ratio between the two is nevertheless informative
about who is being under-sized. `probe_matrix_s` divided by achieved
`eval_s`/matrix is 1.0–1.2 for the two single-thread CPU paths, 12.0–12.9 for
batch rayon, and 1.0–16 909 for the prototypes, falling with $n$ on both. The
prototypes' batches are calibrated from a latency that overstates their
per-matrix cost by up to four orders of magnitude at the small orders, which is
why their measured cells sit far from the batch sizes the same 2 s target gives
the CPU paths.

## 5. Censoring (REQ-12)

Four cells in the $q = 7$ grid are censored, all of one kind. Not one of them
carries a measured throughput value: `composite_matrices_per_s` and
`eval_matrices_per_s` are `NaN` on all four, and §4.3 withholds the kernel-only
rate for the same reason.

**All four are `gpu_hip` cells that were not attempted.** The censoring rule is
applied before the cell runs: a cell is censored when its projected rate implies
a repetition longer than the 120 s per-cell cap, which for a fixed-batch cell is
$M / \hat{R}$ (`protocol.rs:668-673`).

| cell | `order_index` | `projected_matrices_per_s` | `projection_reference_n` | implied repetition | `reps` | `matrices` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| $n = 24$, $M = 256$ | 46 | 1.460222 | 20 | 175 s | 0 | 0 |
| $n = 24$, $M = 1024$ | 56 | 3.017510 | 20 | 339 s | 0 | 0 |
| $n = 28$, $M = 1024$ | 64 | 0.161652 | 20 | 6 335 s | 0 | 0 |
| $n = 28$, $M = 256$ | 67 | 0.078226 | 20 | 3 273 s | 0 | 0 |

Each states its censoring reason verbatim in `note`, of the form `not
attempted: the measured rate at n=20 projects to 1.4602 matrices/s at n=24
under Ryser's n*2^n work model, so one repetition would take 175 s against the
120 s cap. … The cell carries no measured rate`. The rate each is projected
from is its own chain's measured $n = 20$ cell: 28.036300 matrices/s at
$M = 256$ and 57.936200 at $M = 1024$, rescaled through $n \cdot 2^n$
($28.036300 \times 20 \cdot 2^{20} / (24 \cdot 2^{24}) = 1.460224$ and
$57.936200 \times 20 \cdot 2^{20} / (28 \cdot 2^{28}) = 0.161652$, both matching
the CSV to seven significant figures).

**No $q = 7$ cell is censored after executing**, and every measured cell in this
grid satisfies the harness's own stopping rule: the fewest repetitions any
measured cell runs is 5, and the longest measured cell closes at 117.147505 s
against the 120 s cap (`analysis.py` section 11.1 checks both).

The projection is an estimate and is labelled as one in the CSV preamble. This
run's own $q = 7$ chains measure the direction and size of its bias where both
ends of a step are measured:

| path | step | projection | measured | error |
| --- | ---: | ---: | ---: | ---: |
| `gpu_hip` $M{=}256$ | $12 \rightarrow 16$ | 450.3202 | 536.7621 | $-16.1\%$ |
| `gpu_hip` $M{=}256$ | $16 \rightarrow 20$ | 26.8381 | 28.0363 | $-4.3\%$ |
| `gpu_hip` $M{=}1024$ | $12 \rightarrow 16$ | 982.8239 | 1 109.4474 | $-11.4\%$ |
| `gpu_hip` $M{=}1024$ | $16 \rightarrow 20$ | 55.4724 | 57.9362 | $-4.3\%$ |
| `f7-lookup-table-control` | $12 \rightarrow 16$ | 29 560.5500 | 145 932.9173 | $-79.7\%$ |
| `f7-lookup-table-control` | $16 \rightarrow 20$ | 7 296.6459 | 408.7068 | $+1685.3\%$ |
| `f7-lookup-table-control` | $20 \rightarrow 24$ | 21.2868 | 1.5792 | $+1247.9\%$ |
| `f7-lookup-table-control` | $24 \rightarrow 28$ | 0.0846 | 0.0460 | $+83.9\%$ |
| `f7-three-plane-permanent` | $12 \rightarrow 16$ | 13 371.8700 | 301 514.5246 | $-95.6\%$ |
| `f7-three-plane-permanent` | $16 \rightarrow 20$ | 15 075.7262 | 8 397.8509 | $+79.5\%$ |
| `f7-three-plane-permanent` | $20 \rightarrow 24$ | 437.3881 | 265.5329 | $+64.7\%$ |
| `f7-three-plane-permanent` | $24 \rightarrow 28$ | 14.2250 | 3.0407 | $+367.8\%$ |
| `cpu_scalar` | $12 \rightarrow 16$ | 300.2028 | 311.0805 | $-3.5\%$ |
| `cpu_rayon_batch_scalar` | $12 \rightarrow 16$ | 3 363.8029 | 3 703.3679 | $-9.2\%$ |
| `cpu_ryser_generic` | $12 \rightarrow 16$ | 341.8848 | 313.3910 | $+9.1\%$ |
| `cpu_ryser_generic` | $16 \rightarrow 20$ | 15.6696 | 15.3900 | $+1.8\%$ |
| `cpu_ryser_generic` | $20 \rightarrow 24$ | 0.8016 | 0.7979 | $+0.5\%$ |
| `cpu_ryser_generic` | $24 \rightarrow 28$ | 0.0427 | 0.0427 | $+0.1\%$ |

**Every censored cell in this field sits on a chain whose projection this run
measures as conservative, and none sits on a probe-calibrated one.** All four
are `gpu_hip` at a fixed batch, so their chains are the two `gpu_hip` chains,
and every measured step on both runs low, by $-16.1\%$ to $-4.3\%$, with the
magnitude shrinking as $n$ grows — which is what the launch-amortisation
mechanism the preamble names predicts. Their true rates are therefore expected
to be somewhat higher than the projections above, and no ordering is asserted
from them.

**Both prototype chains land high, and neither owns a censored cell.** The
lookup control's projection is 1685.3 % and 1247.9 % high at the two middle
steps and the three-plane path's is 367.8 % high at its last, so a censored cell
on either chain would carry a projection with no reason to be conservative. This
field has no such cell: both prototypes are measured at all five orders. That is
the difference between this field and $\mathbb{F}_5$, where the byte control's
$n = 28$ cell was censored on exactly such a chain
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §5), and it is why §17
lists no censored-projection caveat for $\mathbb{F}_7$.

No $q = 7$ cell reports a build failure, a correctness failure, or a device
resource exhaustion. The 46 non-`measured`, non-`censored` cells are the
out-of-field prototype rows, the accumulator's structural absence, the three
in-tree CPU paths this field lacks, and the six packed-CPU cells above the
sixteen-lane bound (§2).

## 6. Component isolation (REQ-03)

REQ-03 asks that the dependency-chained Gray update, the horizontal product,
and the end-to-end Ryser loop each be isolated for every representation of this
field that executes; that a representation whose component admits no distinct
isolate — a fused or branchless circuit — be recorded with that structural
reason in place of a duration; and that a censored isolate carry its censoring
reason. The end-to-end Ryser loop is §4, covering all five executing backends at
the orders each runs. Both component isolates run at all five orders in this
campaign (`# orders: 12, 16, 20, 24, 28` in each isolate preamble; each order is
an independent row per candidate with its own seed address). Each subsection
below gives the durations it measures, then the reason carried by every row that
has none, and names which of the two kinds that reason is.

### 6.1 Dependency-chained Gray update

Shape: one accumulator reads its immediately preceding add/subtract result — a
latency chain, not independent-operation throughput. Net duration is
`(Σ update spans − Σ same-geometry compiler-barrier spans) / (steps × reps)`,
with CPU rows using paired host spans and HIP rows paired device-event kernel
spans (preamble of
[`…-q7-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q7-gray-update.csv)).

**This field has two Gray-update circuits, and they are measured at different
sets of orders**, because the packed host representation carries the same
sixteen-lane bound as the packed CPU permanent kernel. Seven rows are
`measured`; none is censored.

| $n$ | `cpu_scalar` `net_per_operation_s` | per row | `gpu_hip` `net_per_operation_s` | per row | device / host |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 2.457e-09 | 2.0475e-10 | 2.082557e-06 | 1.735464e-07 | 847.6 |
| 16 | 2.445e-09 | 1.5281e-10 | 2.773900e-06 | 1.733688e-07 | 1 134.5 |
| 20 | unsupported | — | 3.465913e-06 | 1.732957e-07 | — |
| 24 | unsupported | — | 4.157866e-06 | 1.732444e-07 | — |
| 28 | unsupported | — | 4.848253e-06 | 1.731519e-07 | — |

`cpu_scalar` is `measured` at $n \in \{12, 16\}$ over 1 000 001 steps and 1 448
and 1 439 repetitions, with `duration_basis = host_clock_update_chain`, and
`unsupported` above with the reason `packed F_7 Gray-update representation has
16 lanes; n exceeds 16`, which the isolate raises for exactly this backend,
field, and order combination
(`dev/research/permanent-sampling-feas/src/gray_update.rs:260-262`). `gpu_hip`
is `measured` at every order over the same step count and 5 repetitions, with
`duration_basis = device_event_kernel`.

**The two circuits have different shapes in $n$, and the measurement separates
them cleanly.** The host figure is flat in $n$ to within 0.49 % across the two
orders it has; the device figure grows by a factor of 2.33 across the full range
and is flat to within 0.23 % once divided by $n$. That is exactly what the two
sources predict. The host circuit is the packed `Packed7` add/subtract, one
bit-plane operation per step regardless of order
(`dev/research/permanent-sampling-feas/src/gray_update.rs:310-316`). The device
circuit for $q \ne 3$ walks the column-sum array one byte per row per step —
`for (int row = 0; row < n; ++row)` inside the step loop
(`crates/gf2-kernels-hip/hip/permanent/gray_update_micro.hip:63-69`) — and its
paired barrier baseline traverses the same geometry, `const int inner = q == 3
? 1 : n` (`:84`), so the subtraction is same-shape at every order. The device's
per-row cost is therefore the invariant quantity, and it is 1.7315–1.7355 ×
10⁻⁷ s across all five orders.

The comparison this permits is a per-operation latency comparison of two
single-chain circuits under the same paired-barrier subtraction, and on that
basis the host's packed update is 847.6× cheaper per Gray step at $n = 12$ and
1 134.5× cheaper at $n = 16$. It is not a throughput claim about either path,
and it does not extend above $n = 16$, where the host circuit does not run.

**All three $\mathbb{F}_7$ prototypes carry a structural reason instead of a
measurement**, which is the fused-circuit case: `unsupported:
f7-three-plane-permanent's Gray update runs fused inside its full-permanent
kernel and no device source isolates it; timing the shipped gpu_hip update under
this name would report another backend's circuit`, and the same string for
`f7-lookup-table-control` and `f7-three-plane-accumulator`
(`dev/research/permanent-sampling-feas/src/gray_update.rs:249-255`). The four
remaining CPU rows carry `unsupported: <backend> has no isolated
dependency-chained Gray-update evaluator`, under the rule the isolate's own
preamble states — "*A row reports a span only from a circuit distinct to the
backend it names*" — so they are the same structural case:
`cpu_rayon_batch_scalar` shares `cpu_scalar`'s packed host circuit and has none
of its own to isolate, and the three paths this field lacks have no evaluator at
all. The four out-of-field prototypes carry a by-field reason.

So this field has two Gray-update circuits across its representations, a packed
host one bounded at sixteen lanes and a byte device one that is not, and the
isolate measures both at every order each is defined at. Every other row's
absence is recorded as the structural fact that the row's backend has no
distinct circuit to time, or that its representation does not reach the order.

### 6.2 Horizontal product

Shape: each timed sample is one unconditioned row-sum vector. The zero branch
times the representation's early product exit; the nonzero branch times zero
detection plus the complete representation-specific reduction. Timing is paired
device-event kernel spans only, excluding allocation, upload, download,
submission, sampling, grouping, and host policy.

This field has two horizontal-product circuits under four backend names.
`gpu_hip` and `f7-lookup-table-control` both select
`HorizontalProductCircuit::F7Lookup`, and `f7-three-plane-accumulator` and
`f7-three-plane-permanent` both select `HorizontalProductCircuit::F7ThreePlane`
(`dev/research/permanent-sampling-feas/src/horizontal_product.rs:488`,
`:503-508`), which enter `horizontal_product_micro_kernel` and dispatch to
`f7_lookup_product` and `f7_three_plane_product`
(`crates/gf2-kernels-hip/hip/permanent/horizontal_product_micro.hip:152-156`).
Every $q = 7$ row draws from the same fixed observation address
(`timed_purpose = horizontal_product_timed`, `timed_index_first = 0`), so each
pair is one circuit timed twice rather than two circuits, and the agreement
inside a pair is a replicate rather than a comparison. The `F7Lookup` pair
agrees to within 17.90 % on the zero branch (largest gap at $n = 20$) and
2.99 % on the nonzero branch (largest gap at $n = 28$); those are the largest
disagreements, each a rounding of a maximum over the five orders. The
`F7ThreePlane` pair carries durations on the same branch at one order only,
$n = 24$ on the zero branch, where they differ by a factor of 2.0000 at
$2 \times 10^{-12}$ against $1 \times 10^{-12}$ — two figures at the CSV's last
printed digit, which supports no comparison and is not read as one.

Net per-operation durations, in seconds. `analysis.py` section 6.2 prints every
span and baseline behind them.

| $n$ | `gpu_hip` zero fast | `gpu_hip` nonzero slow | `f7-lookup-table-control` zero fast | `f7-lookup-table-control` nonzero slow | `f7-three-plane-accumulator` zero fast | `f7-three-plane-accumulator` nonzero slow | `f7-three-plane-permanent` zero fast | `f7-three-plane-permanent` nonzero slow |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 2.42e-10 | 9.25e-10 | 2.37e-10 | 9.35e-10 | absent | absent | absent | absent |
| 16 | 3.71e-10 | 2.399e-09 | 3.74e-10 | 2.406e-09 | absent | absent | absent | absent |
| 20 | 3.03e-10 | 6.119e-09 | 2.57e-10 | 6.010e-09 | 2e-12 | absent | absent | absent |
| 24 | 4.37e-10 | 1.4132e-08 | 4.32e-10 | 1.4005e-08 | 2e-12 | 6.00e-10 | 1e-12 | absent |
| 28 | 4.77e-10 | 3.0892e-08 | 4.72e-10 | 2.9996e-08 | 3e-12 | absent | absent | 9.6e-11 |

**The lookup circuit yields a duration on both branches at every order; the
three-plane circuit yields one on six of its twenty branch cells and carries a
censoring reason on the other fourteen.** Every absence above is a censored
branch, and each carries its reason verbatim: `zero fast timing unavailable: raw
device span minus its same-geometry baseline was nonpositive; so no false
positive rate is reported`, and the analogous sentence for the nonzero branch.
The three-plane reduction is cheap enough on this device that its branch
frequently runs faster than the same-geometry barrier baseline it is measured
against; the harness reports no rate rather than a clamped or negative one. At
$n \in \{12, 16\}$ that hits both branches of both three-plane rows, so those
four cells' `outcome` is `censored` and their `note` carries both censoring
reasons.

Where the two circuits do both yield a nonzero-branch duration — $n = 24$ on
the accumulator row and $n = 28$ on the permanent row — the three-plane figure
is 6.00 × 10⁻¹⁰ s against the lookup circuit's 1.4005 × 10⁻⁸ s, and
9.6 × 10⁻¹¹ s against 2.9996 × 10⁻⁸ s. Those are 23.3× and 312× apart, on two
single cells whose partner branches are censored, so this receipt reports them
as the two figures they are and asserts no ordering of the circuits from them.
§17 records that as a limit.

Every CPU row is `unavailable: <backend> has no distinct device-event
horizontal-product isolate; no generic or host-clock replacement was used`, and
the four out-of-field prototypes carry a by-field reason naming the circuit they
would time instead, of the form `unsupported: f5-three-plane uses the
f5-three-plane-c4 horizontal-product circuit over F_5; not F_7`.

So of the two $\mathbb{F}_7$ horizontal-product representations, the lookup
circuit yields isolated durations on both branches at every order under both
names that select it, and the three-plane circuit yields four zero-branch and
two nonzero-branch durations across its two names and five orders, with the
other fourteen of its twenty branch cells censored and each carrying its reason.
No absence is left without a reason of the kind the criterion names.

## 7. Kernel resources against design-predicted budgets (REQ-07, REQ-09)

Source: the compiler kernel-resource receipt
[`hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](hip-resource-usage-20260814T172506Z-1610002/receipt.txt)
at `source_revision f9224650a780ea8ed98bcdff6e662cdb0d7b94f4`, architecture
`gfx1030`, flag `-Rpass-analysis=kernel-resource-usage`, every entry
`exit_status: 0` with source, object, and log SHA-256. The per-kernel logs carry
the remarks quoted below verbatim; `analysis.py` section 7 reparses them.

The device reports registers in two fields: `TotalSGPRs` are scalar registers
shared by a wave, `VGPRs` are vector registers held per lane, so `VGPRs` is the
per-thread register figure and `TotalSGPRs` the per-wave one. `ScratchSize
[bytes/lane]` is private scratch memory per thread. `LDS Size [bytes/block]` is
the compiler's *static* LDS field and does not include memory requested
dynamically at launch. The `log` column cites the first line of that kernel's
remark block within the named log in the receipt directory.

| Kernel | Path | `TotalSGPRs` | `VGPRs` | scratch B/lane | SGPR spill | VGPR spill | static LDS B/block | occupancy waves/SIMD | log |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `permanent_bipedal7_kernel` | `gpu_hip` end-to-end | 107 | 128 | 4000 | 4 | 6 | 0 | 8 | `permanent_bipedal7.hip.resource.log:1` |
| `wave_gf7_lookup_table_kernel<1>` | `f7-lookup-table-control`, $n \le 16$ | 19 | 31 | 0 | 0 | 0 | 0 | 16 | `wave_gf7_equivalence.hip.resource.log:23` |
| `wave_gf7_lookup_table_kernel<2>` | `f7-lookup-table-control`, $n > 16$ | 26 | 41 | 24 | 0 | 0 | 0 | 16 | `wave_gf7_equivalence.hip.resource.log:34` |
| `prepare_three_plane_columns` | `f7-three-plane-permanent`, bit-plane staging | 16 | 17 | 0 | 0 | 0 | 0 | 16 | `wave_gf7_equivalence.hip.resource.log:1` |
| `wave_gf7_three_plane_kernel` | `f7-three-plane-permanent`, Gray walk | 16 | 34 | 0 | 0 | 0 | 0 | 16 | `wave_gf7_equivalence.hip.resource.log:12` |
| `gray_update_micro_kernel` | Gray-update isolate | 86 | 5 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:1` |
| `gray_update_compiler_barrier_baseline_kernel` | its paired baseline | 13 | 2 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:12` |
| `horizontal_product_micro_kernel` | horizontal-product isolate | 26 | 3 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:24` |
| `horizontal_product_compiler_barrier_baseline_kernel` | its paired baseline | 14 | 2 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:35` |

The `permanent_bipedal7` log carries one further kernel that no timing in this
campaign exercises, `permanent_bipedal7_lut_checksum_kernel` at 15 `TotalSGPRs`
and 3 `VGPRs` with zero scratch, zero spills, zero static LDS, and occupancy 16
(`permanent_bipedal7.hip.resource.log:12`).

The two horizontal-product kernels appear in the `permanent_bipedal7.hip` log
because `horizontal_product_micro.hip` has no translation unit of its own: it is
included textually so the $\mathbb{F}_7$ lookup circuit reads that unit's
`__constant__` table, and compiling it alone fails on that undeclared symbol.
The receipt records it as `translation_unit_role: included-fragment` against the
`permanent_bipedal7.hip` log. So the log holding this field's shipped kernel is
also the log holding both isolate kernels this field's §6.2 measures.

### 7.1 Predicted budgets and what the device reports

**Lane-owns-interval prototypes.** The only committed design statement of a
resource budget for these kernels is a per-block shared-memory allocation, in
[`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md):
"*`LDS Size` is the compiler's static-LDS field. It does not include the
explicit dynamic shared column tables requested at launch: $24n$ bytes for the
three-plane kernel and $8n\lceil n/16 \rceil$ bytes for the lookup control.*"
(`:493-495`). The launches confirm both formulas: `shared_bytes` is
`3 * n * sizeof(std::uint64_t)` for the three-plane Gray walk
(`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:683`), backing
`extern __shared__ std::uint64_t staged_columns[]` at `:176`, and
`n * sizeof(std::uint64_t)` at $n \le 16$ with
`n * kMaxControlWords * sizeof(std::uint64_t)` above (`:717-718`, `:721-723`),
which is $8n\lceil n/16 \rceil$ for both instantiations because
$\lceil n/16 \rceil$ is 1 through $n = 16$ and `kMaxControlWords` is 2 for the
orders above it that the control accepts.

| Kernel | predicted per-lane registers | measured per-lane (`VGPRs`) | measured per-wave (`TotalSGPRs`) | predicted per-block shared | measured static LDS |
| --- | --- | ---: | ---: | --- | --- |
| `wave_gf7_lookup_table_kernel<1>` | none committed | 31 | 19 | $8n\lceil n/16 \rceil$ B | 0 B (static field only) |
| `wave_gf7_lookup_table_kernel<2>` | none committed | 41 | 26 | $8n\lceil n/16 \rceil$ B | 0 B (static field only) |
| `wave_gf7_three_plane_kernel` | none committed | 34 | 16 | $24n$ B | 0 B (static field only) |
| `prepare_three_plane_columns` | none committed | 17 | 16 | 0 B/block, exact (`wave_gf7_equivalence.hip:677`) | 0 B |

The shared-memory prediction for the three timing kernels is **not confirmed and
not refuted by this campaign**, and the committed design says so first:
`-Rpass-analysis=kernel-resource-usage` reports static LDS only, and all three
tables are requested dynamically at launch, so a 0 in that column is silence
rather than evidence of absence. No occupancy conclusion in §8 rests on treating
it as a zero. §4.5 carries the launch-time figure per cell, 96–448 bytes per
block for the lookup control and 288–672 for the three-plane path, from the
source formulas rather than from any measurement.

The staging kernel is the one prototype kernel whose shared-memory picture *is*
complete: `prepare_three_plane_columns` is launched with `0` dynamic shared
bytes (`wave_gf7_equivalence.hip:677`) and declares no `__shared__` of its own,
so 0 bytes per block is exact for it and the measurement confirms it.

**No committed design states a per-lane register budget for any
$\mathbb{F}_7$ kernel.** The nine-32-bit-register lower bound in the same README
is stated in its "## F_3 wave-cooperative evidence boundary" section, of
`WaveGf3` and `FoldGf3`: "*Their stable source-level state model is two packed
`u64` words plus `u64` Gray cursor/end bounds and one `u32` partial sum: a 9 x
32-bit-register lower bound.*" (`:172-174`), and the same paragraph names the
compiler resource output "*rather than this lower bound*" as the authoritative
allocation evidence (`:176-178`). Neither covers this field, whose committed
table is $24n$ and $8n\lceil n/16 \rceil$. That absence is recorded here as an
absence and nowhere filled with a back-derived number; §7.2 states it per
kernel.

**One committed design claim about registers is qualitative rather than
numeric, and this field's measurement confirms it.** The lane-owns-interval
design states that "*Each lane owns its complete packed row-sum accumulator in
registers for the whole of its interval*"
([`dev/active/0de41c82/plan.md`](../../active/0de41c82/plan.md):38). All four
$\mathbb{F}_7$ prototype kernel instantiations report zero spills of either
kind, and three of the four report 0 scratch bytes per lane, so on this device
and at this architecture nothing of their accumulator state is spilled to
memory and the claim holds as stated. The control mapping under the same
compiler and flags reports 4 000 scratch bytes per lane and ten spills, so the
contrast is measured on both sides rather than argued. This is a qualitative
residency claim and not a per-lane register budget, and §7.2 does not count it
as one.

**The one nonzero scratch figure among the prototypes is a committed
observation rather than a divergence.** `wave_gf7_lookup_table_kernel<2>`
reports 24 scratch bytes per lane with zero spills, and the committed README
records the same figure from an independent capture together with the reading it
is given: "*The 24-byte lookup-`<2>` scratch result is preserved as compiler
evidence, not interpreted as a performance conclusion.*" (`:496-497`). This
campaign reproduces it and gives it the same reading.

**A committed prior measurement of all four prototype kernels reproduces
exactly, and is reported as replication rather than as prediction.** The
README's F_7 section carries a resource table from a separate clean capture at
revision `a29c5e8a71a2795d5e1e97f10f698c4123b9275d` (`:434-436`, `:486-491`):
`prepare_three_plane_columns` at 16 SGPRs, 17 VGPRs, 0 B/lane scratch, 0/0
spills, 0 B/block static LDS, and 16 waves/SIMD; `wave_gf7_three_plane_kernel`
at 16, 34, 0, 0/0, 0, 16; `wave_gf7_lookup_table_kernel<1>` at 19, 31, 0, 0/0,
0, 16; and `<2>` at 26, 41, 24, 0/0, 0, 16. Every one of those 24 figures
matches this campaign's receipt. Two independent hipcc captures of the same
source at the same architecture agree field for field. This is a reproducibility
check on the receipt, not a prediction meeting a measurement, and it is not
counted as prediction coverage in §7.2.

**Shipped GPU path (`permanent_bipedal7_kernel`).** Its predicted per-block
shared-memory allocation is exact and available: the launch passes
`sharedMemBytes = 0`
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip:366`) and the
translation unit contains no `__shared__` declaration, so 0 bytes/block is the
complete shared-memory picture for this kernel and the measurement confirms it.
No committed design document states a predicted per-lane register budget for it.

Its measured resource profile is this field's central quantitative finding, and
it has an exact cause in the source. **128 VGPRs per lane, 4000 scratch bytes
per lane, 4 SGPR spills, 6 VGPR spills, and occupancy 8** — half the wave-slot
ceiling every other kernel in this field's set reaches. The private allocation
comes from `uint8_t columns[63][63]` and `uint8_t col_sum[63]`
(`permanent_bipedal7.hip:132`, `:148`), indexed at runtime by `flip` inside the
Gray walk, so they cannot be register-allocated. Their declared sizes are 3 969
and 63 bytes, 4 032 in total, against the 4 000 bytes/lane the compiler reports;
the two do not match exactly and no committed source or receipt explains the
32-byte difference, so the measured figure is quoted as measured and no
arithmetic identity is asserted for it.

A cross-check in the same receipt directory shows the figure is structural
rather than incidental: `permanent_bipedal5_kernel`, a different translation
unit with the same two array declarations, reports identical figures on all
eight fields — 107 SGPRs, 128 VGPRs, 4000 scratch bytes, 4 and 6 spills, 0
static LDS, occupancy 8 (`permanent_bipedal5.hip.resource.log:1`). The shipped
byte-arithmetic mapping costs the same resources in both fields that use it.

Nothing in this campaign contradicts a numeric register or shared-memory
prediction that a committed design states for this field, because the committed
statements are the $24n$ and $8n\lceil n/16 \rceil$ shared tables, which the
compiler's static field cannot observe, and the exact 0 B/block for the six
kernels whose launches request none, which it confirms. The absence is a fact
about the committed record rather than an absence of searching: the study's own
investigation states that no register, spill, or occupancy measurement of any
permanent kernel existed in the tree before this work
([`dev/active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):410-411),
and the only committed resource figures for this field's kernels are the prior
measurement replicated above. That is the state of the record, and §7.2 gives it
per kernel rather than leaving it to be inferred. The contradictions this run
*does* produce are of other committed statements and are recorded in §9 and §17
with the statements they contradict.

### 7.2 Prediction coverage, stated per kernel

Which of the two predicted quantities a committed design actually states, for
each of the nine kernels this campaign measures. Where one exists, §7.1 pairs it
with the measurement above; where none exists, the row says so and no figure is
supplied in its place.

| Kernel | committed per-lane register prediction | committed per-block shared-memory prediction |
| --- | --- | --- |
| `permanent_bipedal7_kernel` | none committed | 0 B/block, exact (`permanent_bipedal7.hip:366`, no `__shared__`) |
| `wave_gf7_lookup_table_kernel<1>` | none committed | $8n\lceil n/16 \rceil$ B/block (`README.md:493-495`) |
| `wave_gf7_lookup_table_kernel<2>` | none committed | $8n\lceil n/16 \rceil$ B/block (`README.md:493-495`) |
| `prepare_three_plane_columns` | none committed | 0 B/block, exact (`wave_gf7_equivalence.hip:677`, no `__shared__` in this kernel) |
| `wave_gf7_three_plane_kernel` | none committed | $24n$ B/block (`README.md:493-495`) |
| `gray_update_micro_kernel` | none committed | 0 B/block, exact (`gray_update_micro.hip:112`, no `__shared__`) |
| `gray_update_compiler_barrier_baseline_kernel` | none committed | 0 B/block, exact (`gray_update_micro.hip:126`, no `__shared__`) |
| `horizontal_product_micro_kernel` | none committed | 0 B/block, exact (`horizontal_product_micro.hip:196`, no `__shared__`) |
| `horizontal_product_compiler_barrier_baseline_kernel` | none committed | 0 B/block, exact (`horizontal_product_micro.hip:208-209`, no `__shared__`) |

So every measured kernel has a committed per-block shared-memory prediction, and
the measurement confirms it for the six whose launches request none.
**Not one of the nine has a committed per-lane register prediction.** That is
recorded as the state of the committed record rather than as a measurement
result, and no post-hoc budget is derived for any of them from the numbers this
receipt measured — a figure back-derived from the measurement could not
afterwards be diverged from it. This field's prediction-beside-measurement
pairing is therefore one-sided on registers throughout, and §17 records it as
the limit it is.

One of the nine kernels reports nonzero spill counts, the shipped
`permanent_bipedal7_kernel` at 4 SGPR and 6 VGPR spills, and no committed design
predicted the spill behaviour of any kernel in this set in either direction.

## 8. What limits occupancy (REQ-08)

**The resource that limits occupancy differs by kernel in this field, and in
every case it is the per-lane vector-register count or the architectural
wave-slot ceiling — never scalar registers, private scratch, static LDS, or
spills.** Occupancy is a count of waves per SIMD; a kernel whose per-lane
vector-register demand is small enough sits at the hardware maximum, and one
whose demand is large enough is held below it. One of this field's nine measured
kernels is held below it.

Per-thread usage is quoted as `VGPRs` / `TotalSGPRs` / scratch bytes per lane /
static LDS bytes per block / SGPR + VGPR spills, all from the receipt of §7.

| Kernel | measured per-thread usage | occupancy waves/SIMD | resource that limits occupancy | log |
| --- | --- | ---: | --- | --- |
| `permanent_bipedal7_kernel` | 128 / 107 / 4000 / 0 / 4+6 | 8 | per-lane vector registers | `permanent_bipedal7.hip.resource.log:1` |
| `wave_gf7_lookup_table_kernel<1>` | 31 / 19 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:23` |
| `wave_gf7_lookup_table_kernel<2>` | 41 / 26 / 24 / 0 / 0+0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:34` |
| `prepare_three_plane_columns` | 17 / 16 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:1` |
| `wave_gf7_three_plane_kernel` | 34 / 16 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:12` |
| `gray_update_micro_kernel` | 5 / 86 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `gray_update_micro.hip.resource.log:1` |
| `gray_update_compiler_barrier_baseline_kernel` | 2 / 13 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `gray_update_micro.hip.resource.log:12` |
| `horizontal_product_micro_kernel` | 3 / 26 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `permanent_bipedal7.hip.resource.log:24` |
| `horizontal_product_compiler_barrier_baseline_kernel` | 2 / 14 / 0 / 0 / 0+0 | 16 | wave-slot ceiling | `permanent_bipedal7.hip.resource.log:35` |

**The evidence this derivation rests on is the whole receipt, and it is
checkable.** The compiler computes the occupancy field from the per-thread
quantities it reports, so the question is which of them the field responds to.
Across the 26 kernel entries in the receipt directory, `VGPRs` alone accounts
for every reported value: a per-SIMD budget of 1 024 vector registers allocated
in units of 16, capped at 16 waves/SIMD, reproduces all 26 entries with no
exception. `analysis.py` section 8 computes
$\min\left(16,\ \left\lfloor 1024 / \left(16 \lceil \texttt{VGPRs}/16 \rceil\right)\right\rfloor\right)$
for every entry, prints it beside the reported occupancy, and asserts the two
agree; the assertion holds at 26 of 26, and the script fails if a future receipt
breaks it.

The other quantities are shown not to bind by counterexample in the same
receipt: `gray_update_micro_kernel` carries the highest scalar-register count of
any kernel that reaches the ceiling, 86 `TotalSGPRs`, and still reports 16;
`permanent_bipedal3_kernel` carries 1040 scratch bytes per lane and still
reports 16 (`permanent_bipedal3.hip.resource.log:1`); and
`wave_gf7_lookup_table_kernel<2>` carries this field's only nonzero prototype
scratch at 24 bytes per lane and also reports 16. Neither scalar registers nor
private scratch costs a wave slot anywhere in the receipt, and no kernel in it
reports nonzero static LDS.

Two controls bracket the field. The **upper control** is
`permanent_wave_gpu_probe`, an empty kernel with 0 `TotalSGPRs`, 0 `VGPRs`, 0
scratch, and 0 LDS (`probe.hip.resource.log:1`), which reports 16. A kernel that
consumes no registers at all cannot be register-limited, so 16 is the saturation
value of that field on `gfx1030` and therefore the architectural ceiling this
receipt names. The **lower control** is `permanent_bipedal7_kernel` itself at
128 `VGPRs` reporting 8 — this field's own shipped path, not a borrowed one. The
field does fall when per-lane register demand is heavy, so a report of 16 is a
measured absence of register pressure rather than an unresponsive constant.

**The named limiter holds at each measured order** $n \in \{12, 16, 20, 24, 28\}$
for every row of the table. The evidence is that $n$ reaches these kernels as a
runtime argument rather than a template parameter, so no per-order variation in
resource usage is possible: the mangled names are
`_Z25permanent_bipedal7_kernelPKhiiPy`,
`_ZN12_GLOBAL__N_127prepare_three_plane_columnsEPKhiiPm`, and
`_ZN12_GLOBAL__N_127wave_gf7_three_plane_kernelEPKmiiPj`, none of which carries
a template parameter, and the compiler emits one resource block per kernel
rather than one per order. The lookup control is the one exception and it is
resolved rather than assumed: it *is* templated, on a word count and not on $n$,
so the compiler emits two blocks — `<1>` and `<2>` — and the harness selects
between them by order at launch, `<1>` at $n \le 16$ and `<2>` above
(`wave_gf7_equivalence.hip:716-724`). Both report occupancy 16, so the named
limiter is the same at every measured order for that path too, and the table
carries each instantiation on its own row with the orders it serves.

**What the shipped path pays for its byte arithmetic, in wave slots.** The
control mapping holds a $63 \times 63$ byte column table plus a 63-byte
accumulator in per-thread private memory (§7.1), and the compiler's response is
128 VGPRs with 4 000 scratch bytes and ten spills. That costs it half the wave
slots of the SIMD: 8 waves against the ceiling's 16. The lane-owns-interval
prototypes move the column table into per-block shared memory and hold either
three packed `u64` planes or $\lceil n/16 \rceil$ nibble words per lane, and the
compiler's response is 17 to 41 VGPRs with zero spills, worth the full 16 waves
each. So on this field the mapping change buys twice the occupancy of the
control and leaves every prototype kernel at the ceiling — a stronger result
than $\mathbb{F}_5$'s, where both prototypes sat at 12
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §8), and the difference is
that this field's prototypes hold far less per-lane state.

Occupancy and device-wide resident work are separate quantities, and the table
above answers only the first. What bounds each path's *device parallelism* at
each measured order is its launch geometry and its batch size, fixed by the
kernel's own contract and by the cell's calibration rather than by its resource
usage. The shipped kernel launches `gridDim.x = M` blocks of `dim3 block(1, 1,
1)`, one matrix per block, with only thread 0 doing work
(`permanent_bipedal7.hip:350-351`, `:363-366`), so its resident wave count
cannot exceed $M$ and each resident wave carries one active lane: 256 or 1024
single-lane waves for the whole device. The prototypes launch `gridDim.x = M`
blocks of `active_lanes_for_order(n)` lanes, which is 32 at every order this
campaign measures (`wave_ryser_mapping.h:29-31`), giving the active-lane counts
tabulated in §4.5: 32 to 186 656 lanes depending on the cell's calibrated batch.
The lane-owns-interval mapping supplies 32 lanes per matrix where the control
supplies one, and the batch calibration is what decides how many matrices are in
flight.

One resource this derivation cannot rule on is dynamic LDS, for the reason in
§7.1: the compiler's static field is silent about the prototypes' $24n$-byte and
$8n\lceil n/16 \rceil$-byte launch-time tables, so no claim is made that shared
memory does or does not bound the prototype mapping's occupancy. That scopes
three rows of the table. For the two lookup instantiations and the three-plane
Gray-walk kernel the named limiter is the binding one among the resources this
receipt can observe, and the launch-time shared allocation is outside that set;
for the other six kernels the compiler's static LDS field is the complete
shared-memory picture, because each launch passes `sharedMemBytes = 0`
(`permanent_bipedal7.hip:366`, `wave_gf7_equivalence.hip:677`,
`gray_update_micro.hip:112`, `:126`, `horizontal_product_micro.hip:196`,
`:208-209`) and none of those translation units declares `__shared__` for those
kernels, so nothing is left unobserved.

## 9. Zero fast path and its exact marginal expectation (REQ-10)

The horizontal-product isolate carries the observed branch frequencies. They
derive from one fixed unconditioned host observation batch per order, addressed
by $(\text{seed\_root}, q, n, \texttt{horizontal\_product\_timed},
\text{seed\_index})$; device timing resamples under the canonical warm-up and
repetition policy and does not alter these counts (preamble of
[`…-q7-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q7-horizontal-product.csv)).
Every row at one order carries the same counts, which `analysis.py` section 9
verifies across all fourteen rows per order, so each order's frequency is one
observation rather than fourteen.

For $q = 7$, sample count $N = 4096$ at each order. Intervals are two-sided
Wilson score intervals at nominal 95 % coverage with $z = 1.959963984540054$,
computed in `analysis.py`; the Wilson form is used because it stays inside
$[0, 1]$ and remains defined at zero successes.

| $n$ | branch | observed | frequency | exact expectation | Wilson 95 % | expectation inside interval |
| ---: | --- | ---: | ---: | ---: | --- | --- |
| 12 | zero fast | 3469 / 4096 | 0.846923828 | $1 - (6/7)^{12} = 0.842732666089$ | [0.835572480, 0.857625056] | yes |
| 12 | nonzero slow | 627 / 4096 | 0.153076172 | $(6/7)^{12} = 0.157267333911$ | [0.142374944, 0.164427520] | yes |
| 16 | zero fast | 3770 / 4096 | 0.920410156 | $1 - (6/7)^{16} = 0.915111010100$ | [0.911722043, 0.928310440] | yes |
| 16 | nonzero slow | 326 / 4096 | 0.079589844 | $(6/7)^{16} = 0.084888989900$ | [0.071689560, 0.088277957] | yes |
| 20 | zero fast | 3914 / 4096 | 0.955566406 | $1 - (6/7)^{20} = 0.954179037522$ | [0.948817719, 0.961461383] | yes |
| 20 | nonzero slow | 182 / 4096 | 0.044433594 | $(6/7)^{20} = 0.045820962478$ | [0.038538617, 0.051182281] | yes |
| 24 | zero fast | 3992 / 4096 | 0.974609375 | $1 - (6/7)^{24} = 0.975266985684$ | [0.969328961, 0.979000392] | yes |
| 24 | nonzero slow | 104 / 4096 | 0.025390625 | $(6/7)^{24} = 0.024733014316$ | [0.020999608, 0.030671039] | yes |
| 28 | zero fast | 4041 / 4096 | 0.986572266 | $1 - (6/7)^{28} = 0.986649734880$ | [0.982563839, 0.989668878] | yes |
| 28 | nonzero slow | 55 / 4096 | 0.013427734 | $(6/7)^{28} = 0.013350265120$ | [0.010331122, 0.017436161] | yes |

**The two expectations are complements.** $1 - ((q-1)/q)^n$ and $((q-1)/q)^n$
sum to exactly 1, which the computed values reproduce to 15 decimal places at
every order, and the two observed frequencies likewise sum to exactly 1 because
the branches partition the 4096 samples. The CSV states the same relation in its
`note` column on every valid row and carries both expectations in its own
`zero_fast_expected_frequency` and `nonzero_slow_expected_frequency` columns,
which agree with the values above to all twelve printed digits. Every
expectation falls inside its interval, so the 4096-sample observation is
consistent with the exact marginal at every measured order.

The same file carries a second, much larger observation of the same branches:
the device timing loop classifies every one of its own timed operations into
the two branches, and those counts are `zero_fast_timed_operations` and
`nonzero_slow_timed_operations`. They are drawn from the same purpose stream as
the observation batch, so they are not an independent replicate and are not
pooled with it; the four backends' rows at one order likewise resample the same
addresses and are not four replicates. They are reported because they bound the
same quantity three orders of magnitude more tightly. `gpu_hip` rows shown;
`analysis.py` section 9 prints all twenty.

| $n$ | zero-fast ops | nonzero-slow ops | total | slow frequency | exact expectation | Wilson 95 % | expectation inside interval | $z$ |
| ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: |
| 12 | 6 604 625 | 1 231 023 | 7 835 648 | 0.157105449 | 0.157267334 | [0.156850821, 0.157360414] | yes | $-1.24$ |
| 16 | 6 960 274 | 645 998 | 7 606 272 | 0.084929648 | 0.084888990 | [0.084731741, 0.085127973] | yes | $+0.40$ |
| 20 | 6 980 733 | 334 723 | 7 315 456 | 0.045755589 | 0.045820962 | [0.045604409, 0.045907247] | yes | $-0.85$ |
| 24 | 6 889 668 | 175 932 | 7 065 600 | 0.024899796 | 0.024733014 | [0.024785161, 0.025014948] | **no** | $+2.85$ |
| 28 | 6 708 476 | 90 884 | 6 799 360 | 0.013366552 | 0.013350265 | [0.013280509, 0.013453146] | yes | $+0.37$ |

Each total equals `reps × samples_per_rep` exactly, which `analysis.py` asserts.

**At $n = 24$ the exact marginal falls outside the interval, and that is
recorded rather than smoothed over.** The observation counts 175 932
nonzero-slow operations against an expectation of 174 753.6, a $+2.85\sigma$
excess, and the Wilson lower bound 0.024785161 sits 0.0000521 above the exact
expectation 0.024733014. All four backends reproduce it because they resample
the same addresses: $z = +2.85$, $+2.83$, $+2.75$, and $+2.74$. This is one
order of the five, under a nominal 95 % procedure applied without multiplicity
adjustment, so a single miss is within what the procedure produces by
construction; the four other orders sit at $|z| \le 1.24$ and the 4096-sample
observation at $n = 24$ covers comfortably. It is recorded because it is the one
cell in this campaign where an observed branch frequency and its exact marginal
are not consistent at the stated coverage, and because a reader recomputing the
intervals would find it. The $\mathbb{F}_5$ campaign recorded the same kind of
miss at its own $n = 28$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §9); across the two fields
that is two orders out of ten under a nominal 95 % procedure.

$\mathbb{F}_7$ has the lowest zero fast-path share of the three fields at every
order, because $1 - ((q-1)/q)^n$ decreases in $q$ at fixed $n$:

| $q$ | zero fast at $n = 12$ | zero fast at $n = 28$ |
| ---: | ---: | ---: |
| 3 | 0.992292653 | 0.999988266 |
| 5 | 0.931280523 | 0.998065719 |
| 7 | 0.842732666 | 0.986649735 |

This run's data follows that ordering, and it is what decides which branch this
field's circuits can time. The slow branch is reached by 627 of 4096 samples at
$n = 12$ and still by 55 at $n = 28$, so this field's slow branch carries 90 884
to 1 231 023 timed operations per cell and yields a duration at every order on
both lookup-circuit rows (§6.2) — the most slow-branch traffic of the three
fields. The cost of the complete reduction is the quantity this field's isolate
exposes best, and the branch that is hardest to time here is the fast one on the
three-plane circuit, which is cheap enough to disappear into its own barrier
baseline.

### 9.1 Permanent-zero fraction observed in passing

Distinct from the branch frequencies above, and reported because the `zeros`
column of the grid records it: the fraction of sampled matrices whose permanent
is $0 \bmod 7$. These are by-products of the timing protocol with no
preregistered $N$; the counts are whatever each cell's repetition policy
required. Pooling across cells at one order pools independent samples, because
each cell draws from its own reserved index block (§4.2).

| $n$ | zeros | matrices | fraction | Wilson 95 % |
| ---: | ---: | ---: | ---: | --- |
| 12 | 703 061 | 4 919 297 | 0.142919 | [0.142610, 0.143229] |
| 16 | 312 951 | 2 185 988 | 0.143162 | [0.142699, 0.143627] |
| 20 | 7 417 | 50 557 | 0.146706 | [0.143648, 0.149817] |
| 24 | 185 | 1 352 | 0.136834 | [0.119541, 0.156185] |
| 28 | 3 | 28 | 0.107143 | [0.037118, 0.271959] |

The $n = 28$ row pools 28 matrices, because the only measured cells at that
order run 5, 5, and 18 matrices; its interval spans a factor of 7.3 and it
supports nothing on its own.

## 10. Execution mapping: control and lane-owns-interval (REQ-04)

The control mapping is `permanent_bipedal7_kernel`, which maps one matrix to
one block of one working thread
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip:350-351`,
`:363-366`), which is the current one-thread-per-matrix execution mapping. It
is attempted at all five orders and `measured` at $n \in \{12, 16, 20\}$ at both
batch sizes $\{256, 1024\}$, with the throughput, phase, and resource figures of
§4 and §7; its four cells at $n \in \{24, 28\}$ are censored (§5).

The lane-owns-interval mapping is `wave_gf7_lookup_table_kernel<W>` and the
`prepare_three_plane_columns`/`wave_gf7_three_plane_kernel` pair, each launching
one block per matrix with `active_lanes_for_order(n)` lanes and a dynamic shared
column table (`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:677`,
`:682-684`, `:717-724`), each lane owning a balanced Gray interval
(`:184-186`, `dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:33-40`).
Both specializations are equivalence-confirmed against the CPU oracle at every
grid order in the same run (§3), and both are `measured` in all five of their
grid cells.

Each mapping's batch size is carried in its own column beside the throughput it
produced. The control's are the grid's two fixed sizes; the prototypes' are what
each cell's own probe calibrated.

| $n$ | control $M$ | control rate | `f7-lookup-table-control` $M$ | rate | `f7-three-plane-permanent` $M$ | rate | three-plane / control |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 1024 | 20 966.9089 | 5 833 | 630 625.0668 | 46 | 285 266.5608 | 13.6056 |
| 16 | 1024 | 1 109.4474 | 695 | 145 932.9173 | 1 109 | 301 514.5246 | 271.7700 |
| 20 | 1024 | 57.9362 | 29 | 408.7068 | 41 | 8 397.8509 | 144.9500 |
| 24 | censored | — | 2 | 1.5792 | 18 | 265.5329 | — |
| 28 | censored | — | 1 | 0.0460 | 3 | 3.0407 | — |

The control column is `gpu_hip` at its better configuration per order, which is
$M = 1024$ at all three orders where it is measured.

**The two mappings are compared at three matching orders, not five, and the
reason is on the record.** Both mappings carry a measured cell at
$n \in \{12, 16, 20\}$. At $n = 24$ and $n = 28$ the control has no measured
cell in either configuration — all four are censored before running (§5) — so no
ratio between the mappings is stated at those two orders and none is derived
from the control's projections. The lane-owns-interval mapping is measured at
all five, and the order axis it establishes on its own runs from 12 to 28; the
axis on which the two mappings are compared runs from 12 to 20.

The grid fixes the control at $M \in \{256, 1024\}$ and calibrates every
prototype cell from its own probe, producing $M \in \{1, 2, 3, 18, 29, 41, 46,
695, 1109, 5833\}$; §4.5 quantifies what that difference is worth, in active
lanes per repetition and in the probe cost that sets each batch. One prototype
cell in this campaign happens to run at 1 109 matrices, 8.3 % above the
control's 1 024, and it is the three-plane cell at $n = 16$: there the two
mappings run 35 488 active lanes against 1 024, which is the mapping's own
32-lanes-per-matrix factor and not a batch difference, and the measured ratio is
271.7700. Every other pair differs in device parallelism as well as in mapping —
at $n = 20$ the three-plane cell runs 1 312 active lanes against the control's
1 024, and at $n = 12$ it runs 1 472 against 1 024 while the lookup cell runs
186 656 — and the size of the mapping's own contribution at those orders is not
separable from the batch. The ordering on the order axis is what these ratios
support.

The comparison that this campaign does support at matched geometry is the
resource comparison of §7: control at 107 SGPR / 128 VGPR / 4000 scratch bytes
per lane / 0 shared bytes per block / 10 spills, against prototypes at 16–26
SGPR / 17–41 VGPR / 0–24 scratch / $24n$ or $8n\lceil n/16 \rceil$ dynamic
shared bytes per block / 0 spills. That is a real difference in where the column
table lives, measured on both sides, and it is where the study's occupancy
hypothesis stands after this run for this field: **the control mapping is
register-limited and spilling at half the wave-slot ceiling, every
lane-owns-interval kernel sits at the ceiling, and neither is limited by
anything else the compiler reports** (§8). The prediction that shared-memory
pressure grows with matrix order remains untested, because the compiler's static
field cannot see the dynamic allocation and no prototype cell in this run is
large enough for that pressure to bind.

## 11. This campaign against the figures it has to confirm or overturn

The comparison target is the prior grid committed as
[`dev/studies/b488f02c/throughput-2026-08-07.csv`](../b488f02c/throughput-2026-08-07.csv),
which is the artifact
[`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.4 renders its table from. Every prior rate quoted below is read from that CSV
by `analysis.py`, not from the rendered table, so no figure of the prior run is
maintained in two places. That grid runs on the same CPU, the same GPU, the same
ROCm 7.2.4, and the same `powersave` governor, under the same harness protocol
(its own preamble); the two differ in host kernel (`7.1.3-arch1-3` there against
`7.1.6-arch1-1` here) and in Rust toolchain (`1.97.0` there against the
campaign's manifest-pinned `1.95.0`). `analysis.py` section 11 prints the full
15-pair comparison.

**Batch rayon is confirmed roughly $3\times$ ahead of the current GPU path at
every shared order, and the scope of "every shared order" is the finding.** The
prior run measures `cpu_rayon_batch_scalar` over the better `gpu_hip`
configuration at 3.3597× and 3.3755× at $n = 12$ and $n = 16$; this campaign
measures 3.4226× and 3.3380× on the same pairs. The direction and the magnitude
reproduce. What the prior grid does not contain is a third shared order: at
$q = 7$ there is no rayon cell above $n = 16$ in either run, because no packed
CPU kernel runs there (§2), so the two orders above are the whole of the shared
axis. **At $n = 20$, where the best applicable CPU path is the generic Ryser
driver instead, the shipped GPU path leads it by 3.7645×** — the reading that
the CPU owns this field holds only where a packed CPU kernel exists to own it
with.

What does not survive at all is the reading that the CPU owns this field
end to end: `f7-three-plane-permanent` leads every measured cell at four of the
five orders and `f7-lookup-table-control` at the fifth, by 8.7878× over the best
CPU path at $n = 12$ rising to 545.6693× at $n = 20$, and both are measured at
the two orders where the shipped GPU path is censored.

**The four censored GPU cells at $n \in \{24, 28\}$ are confirmed, cell for
cell.** The prior run censors `gpu_hip` at $M \in \{256, 1024\}$ and
$n \in \{24, 28\}$ with projections 1.457513, 3.017410, 0.078081, and 0.161647;
this run censors exactly the same four cells with projections 1.460222,
3.017510, 0.078226, and 0.161652, each within 0.2 % of the prior figure and each
projected from the same reference order $n = 20$. The censoring policy that
keeps an unattempted cell from being read as a measured one behaves identically
across the two runs.

**Run-to-run agreement is close, and closer than $\mathbb{F}_5$'s.** The prior
CSV carries 15 measured $q = 7$ cells and this run measures all 15, so the
comparison is over 15 backend/order pairs. The median absolute disagreement is
0.23 % and the largest disagreement of any pair is 4.09 %, at $n = 12$,
`gpu_hip` $M = 256$. Grouped by the kernel each path forces:

| kernel the path forces | pairs | negative | range | mean |
| --- | ---: | ---: | --- | ---: |
| packed `permanent_bipedal7` (`cpu_scalar`, `cpu_rayon_batch_scalar`) | 4 | 4 | $[-1.41\%, -0.91\%]$ | $-1.10\%$ |
| generic `permanent_ryser` (`cpu_ryser_generic`) | 5 | 3 | $[-0.38\%, +0.23\%]$ | $-0.05\%$ |
| shipped `gpu_hip` | 6 | 2 | $[-4.09\%, +0.19\%]$ | $-1.10\%$ |

**The packed-kernel shift the $\mathbb{F}_5$ campaign recorded appears here too,
in the same direction and an order of magnitude smaller.** All four
`permanent_bipedal7` pairs are slower in this run, by 0.91 % to 1.41 %, against
the $\mathbb{F}_5$ campaign's nine pairs at 1.52 % to 9.10 %
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §11). Four same-sign draws
is a $p = 0.125$ outcome under a two-sided sign test with no directional prior,
so this field's four pairs do not establish the shift on their own; they are
consistent with it, and the field where it is established is the other one. The
consequence for this field's figures is small either way: the packed CPU
baseline is at most 1.41 % slower here than in the prior run, and it is the
denominator of only the two ratios at $n \in \{12, 16\}$, which would be 8.7878
and 81.4163 against this run's baseline and 8.6915 and 80.5951 against the prior
run's.

### 11.1 Prior cells published outside the stopping rule, and this field's exposure

Reading the prior rates from the CSV rather than from the rendered table makes
the two comparable, so the comparison was made, and it surfaces cells the prior
artifact publishes as measured in a state its own preamble's protocol censors.
Tracked as open bug `4fdd781a`.

The rule, as the harness implements it, has to be applied exactly to say which
cells those are. A cell stops once both minimums are met or the cap is reached,
and it is censored only when the cap arrives first
(`dev/research/permanent-sampling-feas/src/protocol.rs:178-186`, `:190-191`). So
the 120 s cap bounds when a further repetition may *start*, not a cell's total:
a cell that met both minimums may legitimately close above 120 s because the
repetition already in flight finished.

| $q$, $n$, path | prior `outcome` | prior `reps` | prior `total_s` | prior published rate | reading |
| --- | --- | ---: | ---: | ---: | --- |
| 5, 28, `cpu_rayon_batch_scalar` | `measured` | 1 | 148.177477 | 0.6479 | cap reached before both minimums — the rule censors this cell |
| 3, 28, `gpu_hip` $M{=}1024$ | `measured` | 3 | 159.439482 | 19.2675 | cap reached before both minimums — the rule censors this cell |
| 3, 28, `gpu_hip` $M{=}256$ | `measured` | 5 | 150.019069 | 8.5322 | both minimums met before the cap; the last repetition ran long — conformant |

**Not one of the three is a $q = 7$ cell**, and the check finds no $q = 7$
violation in the prior file at all, so nothing in §11's comparison rests on a
prior figure its own stopping rule forbids. The same check applied to this run's
own $q = 7$ grid finds zero measured cells outside the rule: the fewest
repetitions any measured cell runs is 5 and the longest closes at 117.147505 s
(§5). `analysis.py` section 11.1 enumerates all of it.

The third row is recorded because a check that flags it as a violation would be
wrong, and because bug `4fdd781a`'s description names two cells while a
total-only reading of the cap would name three. The distinction is the harness's
own, and this receipt states it so the correction that bug tracks is applied to
the two cells that need it.

## 12. Bit-plane input preparation cost (REQ-16)

REQ-16 asks for measured bit-plane input preparation cost, reported separately
from the Gray walk, with the host-side portion distinguished from any
device-side portion, for every bit-sliced path that executes. One bit-sliced
path executes as a permanent path in this field, `f7-three-plane-permanent`
(§14 fixes which paths are bit-sliced and which are not).

**The host-side portion is zero, and that is a source fact rather than a
measurement.** The harness serialises the batch to canonical matrix *bytes* and
streams those to the candidate's executable
(`dev/research/permanent-sampling-feas/src/backend.rs:505-513`, via
`serialise_permanent_packed7`), and the frame the executable reads carries the
same bytes into a device buffer
(`dev/research/permanent_wave_gpu/hip/wave_batch_stream.h:196-203`). The
byte-to-plane transpose then runs on the device, as its own kernel,
`prepare_three_plane_columns`
(`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:134-163`). No
host-side plane preparation exists on the measured path, so there is no host
portion to measure rather than an unmeasured one. The grid's `gen_s` column is
sampler matrix generation and packing, not plane preparation.

**The device-side portion is measured, but not separately from the Gray walk,
and no committed column separates them.** The three-plane batch launch enqueues
the preparation kernel and then the Gray-walk kernel back to back inside one
call (`wave_gf7_equivalence.hip:677`, `:682`, both inside
`ThreePlaneBatchLaunch::operator()` at `:670-686`), and the harness's event
bracketing records `kKernelStart` before that call and `kKernelEnd` after it
(`wave_batch_stream.h:204-207`). The header states the consequence outright:
"*The kernel span brackets that whole enqueue, so a candidate whose mapping
needs a staged preparation launch reports both of its launches.*"
(`wave_batch_stream.h:183-184`). So `kernel_device_s` on every
`f7-three-plane-permanent` row is the sum of preparation and Gray walk, and the
`DeviceSpans` structure the executable returns carries only `h2d`, `kernel`,
`d2h`, and `submission_to_kernel`
(`dev/research/permanent_wave_gpu/src/device_batch.rs:66-78`) — there is no
preparation field to read.

The combined figure, per launch, with the two device-side transfer columns
beside it for scale:

| $n$ | $M$ | `reps` | `kernel_device_s` (preparation + Gray walk) | combined s/launch | `h2d_device_s` | host-side preparation |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 12 | 46 | 29 795 | 1.199132 | 0.0000402 | 0.278535 | none on this path |
| 16 | 1 109 | 1 297 | 1.359174 | 0.0010479 | 0.027209 | none on this path |
| 20 | 41 | 1 023 | 4.779097 | 0.0046716 | 0.012121 | none on this path |
| 24 | 18 | 74 | 5.002998 | 0.0676081 | 0.000702 | none on this path |
| 28 | 3 | 6 | 5.917556 | 0.9862593 | 0.000058 | none on this path |

**No committed artifact of this run times the preparation kernel on its own,
and none of the harness's seven measurement modes produces one.** The modes are
`equivalence`, `grid`, `sustained`, `gray-update`, `horizontal-product`,
`envelope`, and `zerofrac`
(`dev/research/permanent-sampling-feas/src/usage.txt`); none is a preparation
isolate, and the `gray-update` mode records `f7-three-plane-permanent` as
`unsupported` for the fused-circuit reason of §6.1 rather than timing its
staging. The one preparation-specific route in the tree is a correctness stage,
not a timing one: `--stage three-plane-preparation`
(`dev/research/permanent_wave_gpu/src/bin/wave_gf7_device_evidence.rs:140`,
`:174`, `:189`) copies every prepared `b0`, `b1`, and `b2` plane back and
compares it against the canonical row-major bytes
(`wave_gf7_equivalence.hip:397-419`), and emits a verdict rather than a
duration. The committed README records that route's clean run and describes it
in exactly those terms — it "*does not compute Ryser, and succeeded only after
copying every prepared `b0`, `b1`, and `b2` column plane back from the device
and comparing them with the canonical row-major matrix bytes*"
([`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md):460-463).

What this campaign therefore establishes for REQ-16 is the host/device
attribution, from source, and the combined device span, from measurement. **The
separation of the preparation cost from the Gray-walk cost is not established,
and no figure is invented for it.** What the receipt can bound is the sum: the
preparation kernel's contribution is at most the whole combined span above, and
the compiler receipt of §7 records the staging kernel at 16 `TotalSGPRs`, 17
`VGPRs`, zero scratch, zero spills, and occupancy 16, against the Gray-walk
kernel's 16, 34, zero, zero, and 16. Separating the two costs needs either a
second event pair inside `ThreePlaneBatchLaunch` or a preparation-only timing
mode, and both are harness changes rather than readings of this run. §17 records
it as the gap it is.

## 13. Achieved occupancy (REQ-17)

REQ-17 asks for achieved occupancy for each measured $\mathbb{F}_7$ kernel from
observed runtime evidence — profiler counters taken during the measured run —
beside the occupancy that kernel's stated register and shared-memory budget
predicts. It states that compiler resource-usage output may accompany that
figure but never stands in for it, and that where the profiler is unavailable on
the campaign host the receipt records achieved occupancy as not measured, with
the reason.

**Achieved occupancy is not measured for any kernel in this campaign, and the
reason is not that the profiler was unavailable.** The three facts, each
checkable:

1. **The runner invokes no profiler.** A search of the whole 30 313-byte
   [`dev/scripts/permanent-campaign-runner.sh`](../../scripts/permanent-campaign-runner.sh)
   for `rocprof`, `profil`, `occupanc`, and `counter` returns no match. Every
   invocation of the measure pipeline is a plain harness call, recorded verbatim
   in the `# invocation:` line of each CSV and under `exact_commands_executed:`
   in the provenance file (§1), and none is wrapped in a profiler.
2. **No committed artifact of this run carries profiler counters.**
   `analysis.py` section 13 scans all ten committed artifacts of run
   `20260814T230032Z-2085453` — the four CSVs, the four logs, the provenance
   file, and the run summary — for the same tokens plus `MeanOccupancyPerCU`,
   `GRBM`, and `SQ_WAVES`, reports zero hits in every file, and asserts the
   total is zero, so a future run that does carry counters fails the script
   rather than passing silently under this section's text.
3. **A profiler is present on the campaign host.** `/opt/rocm/bin/rocprof`,
   `/opt/rocm/bin/rocprofv2`, and `/opt/rocm/bin/rocprofv3` are installed and on
   `PATH`; `rocprof-compute` and `omniperf` are not. No profiler was executed in
   the course of writing these receipts, so what is asserted here is presence on
   `PATH`, not a demonstration that one opens the device.

So the criterion's escape clause does not apply on its stated terms. Recording
"not measured because the profiler is unavailable on the campaign host" would be
a false statement about this host. What the record supports is narrower and is
stated as such: **achieved occupancy is not measured, because the campaign
pipeline collected no profiler counters, on a host where a profiler is
installed.** §16 reads REQ-17 as unmet on that basis rather than as satisfied by
the escape clause, and §17 carries it as the campaign's largest gap.

**The only occupancy figures anywhere in this receipt set are compiler
predictions, and they are not offered as a stand-in.** The `Occupancy
[waves/SIMD]` field of `-Rpass-analysis=kernel-resource-usage` is emitted at
compile time from the per-thread resource counts in the same remark block; §8
shows that it is reproduced exactly, for all 26 kernel entries in the receipt,
by a function of `VGPRs` alone. A quantity that a compile-time register count
determines completely is a prediction of occupancy, not an observation of it,
and the receipt it comes from is dated `20260814T172506Z` — before the measured
run began at `23:00:34Z` — at a different revision
(`f9224650a780ea8ed98bcdff6e662cdb0d7b94f4`) from the run's HEAD. It is reported
in §7 and §8 as what it is, and no row of this document presents it as achieved
occupancy.

The second half of the criterion is available and is recorded here for the
kernel-by-kernel pairing it asks for, so that a later profiled run has the
predicted column to sit beside. The budget-predicted occupancy is the compiler
figure of §7, and the per-lane register and per-block shared-memory budgets
those predictions would rest on are the ones §7.2 enumerates — of which none is
a committed per-lane register budget, so for every $\mathbb{F}_7$ kernel the
prediction side of REQ-17's pairing rests on the compiler's own register
measurement rather than on a design's stated budget.

| Kernel | budget-predicted occupancy (compiler, waves/SIMD) | achieved occupancy (profiler counters) |
| --- | ---: | --- |
| `permanent_bipedal7_kernel` | 8 | not measured — no profiler counters collected during the run |
| `wave_gf7_lookup_table_kernel<1>` | 16 | not measured — same reason |
| `wave_gf7_lookup_table_kernel<2>` | 16 | not measured — same reason |
| `prepare_three_plane_columns` | 16 | not measured — same reason |
| `wave_gf7_three_plane_kernel` | 16 | not measured — same reason |
| `gray_update_micro_kernel` | 16 | not measured — same reason |
| `gray_update_compiler_barrier_baseline_kernel` | 16 | not measured — same reason |
| `horizontal_product_micro_kernel` | 16 | not measured — same reason |
| `horizontal_product_compiler_barrier_baseline_kernel` | 16 | not measured — same reason |

Closing this needs a profiled re-measurement on the campaign host, which is
device work and a change to the runner, not a reading of the artifacts this
campaign committed.

## 14. Exact operation above the sixteen-lane limit (REQ-18)

REQ-18 asks that every bit-sliced path that executes be measured at $n = 16$,
$n = 20$, and $n = 24$, demonstrating exact operation above the current
sixteen-lane limit at measurement scale.

**Which $\mathbb{F}_7$ paths are bit-sliced is settled from source rather than
from name.** `f7-three-plane-permanent` is bit-sliced: its columns are three
`u64` bit planes `b0`, `b1`, `b2`
(`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:75-80`), staged by
the byte-to-plane transpose of §12 and walked by
`wave_gf7_three_plane_kernel`. `f7-three-plane-accumulator` uses the same
bit-sliced arithmetic but does not execute as a permanent path at any order
(§2), so it has no grid cell to measure at any order. The other two device paths
are not bit-sliced: the shipped `gpu_hip` kernel is direct byte arithmetic,
"*each GF(7) element is a uint8_t in {0,1,...,6}*"
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip:14-17`), and
`f7-lookup-table-control` packs $\lceil n/16 \rceil$ nibble words per column and
reads the canonical `Packed7` tables
(`dev/research/permanent_wave_gpu/src/wave_gf7.rs:27-30`).

So REQ-18 binds one path, and that path is measured at all five orders with an
`identical` equivalence verdict at each:

| $n$ | grid outcome | $M$ | composite matrices/s | equivalence reference | matrices | mismatches | status |
| ---: | --- | ---: | ---: | --- | ---: | ---: | --- |
| 12 | `measured` | 46 | 285 266.5608 | `cpu_scalar` | 512 | 0 | `identical` |
| **16** | `measured` | 1 109 | 301 514.5246 | `cpu_scalar` | 512 | 0 | `identical` |
| **20** | `measured` | 41 | 8 397.8509 | `cpu_ryser_generic` | 512 | 0 | `identical` |
| **24** | `measured` | 18 | 265.5329 | `cpu_ryser_generic` | 32 | 0 | `identical` |
| 28 | `measured` | 3 | 3.0407 | `cpu_ryser_generic` | 4 | 0 | `identical` |

**The limit this demonstrates operation above is a real bound in the tree, and
the same run records it refusing the packed path at the same orders.** At
$n \in \{20, 24, 28\}$ the equivalence file carries `cpu_scalar` as
`unsupported: permanent_bipedal7 asserts n <= Packed7::LANES = 16; n = <n>`,
which is why the oracle switches to the generic Ryser driver there (§3). The
bit-sliced path runs at those orders and agrees with that oracle per matrix, so
the demonstration is against an independent kernel rather than against the one
whose limit is in question.

**Measurement scale, stated exactly rather than implied.** At the three orders
the criterion names, the correctness gate compares 512, 512, and 32 matrices,
and the timing cells evaluate 1 438 373, 41 943, and 1 332 matrices over 1 297,
1 023, and 74 launches. The $n = 24$ correctness gate is 32 matrices against 512
at the two smaller orders, and §17 records that as the weaker gate it is. The
$n = 28$ row is outside the criterion and is included because the path is
measured there too, on a four-matrix gate.

## 15. The declared operating point (REQ-19)

REQ-19 is aspirational and asks a specific question: at the declared
scientifically relevant operating point for this field, does the best-performing
prototype exceed the best applicable in-tree CPU throughput by at least
$1.5\times$, with its launch duration inside the documented safe bound.

**The declared operating point for this field is $n = 20$, and the campaign
protocol says so twice.** It fixes the frozen cell universe as "*every integer
$n$ from $4$ through the processor-feasible frontier measured in the envelope
receipt: $n=28$ for $q=3$, $n=24$ for $q=5$, and $n=20$ for $q=7$*"
([`dev/simulation_results/permanent-zero-fraction/protocol.md`](../../simulation_results/permanent-zero-fraction/protocol.md):51-54),
and its backend-selection section fixes exactly four premeasurement
configurations, of which this field's is "*$(7,20)$ on the accelerator at
$M=1024$*" (`:253-256`).

At that point:

| quantity | value |
| --- | ---: |
| best-performing prototype | `f7-three-plane-permanent`, $M = 41$ |
| its composite throughput | 8 397.8509 matrices/s |
| best applicable in-tree CPU path | `cpu_ryser_generic`, $M = 31$ |
| its composite throughput | 15.3900 matrices/s |
| **measured factor** | **545.6693×** |
| device launch duration | 4.8485 µs/launch (`device_submission_to_kernel_s` 0.004960 s over 1 023 launches) |
| kernel span per launch | 0.004672 s |
| per-launch work $M \cdot 2^n$ | $4.299 \times 10^7$ |

**The $1.5\times$ target is exceeded by a factor of 364 at the declared
operating point**, and the target is met at every other order this campaign
measures: the prototype-over-CPU factor is 8.7878, 81.4163, 545.6693, 332.7897,
and 71.2108 at $n = 12$ through $28$. The magnitude at $n = 20$ has to be read
with §4.2's caveat rather than alone: the denominator there is a single-threaded
generic Ryser driver, because this field has no packed CPU kernel and no rayon
permanent path at that order. The same protocol section names the accelerator
at $M = 1024$ as this field's premeasurement configuration, and this run
measures that exact cell at 57.9362 matrices/s — 3.7645× the same CPU
denominator, and 144.9500× slower than the prototype cell.

**The safe launch-duration bound is not yet a committed figure of this study,
and this receipt does not invent one.** Deriving a watchdog-safe per-launch work
bound per field from this study's own measurements is the deliverable of a
separate issue that has not run; the only committed numeric bound is the
archived prior calibration in
[`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`](../../archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md)
§2.5, which places a hang boundary at "*≈190–200 s*" per launch and reports
bounded sub-batches holding every launch at "*≈10–117 s*". That calibration's
work budget, $\text{sub\_batch} \cdot 2^n \le 1.3 \times 10^9$, is stated for
$q = 5$; no $q = 7$ work budget is committed, so this receipt applies only the
per-launch span boundary to this field and does not transfer the work budget
across fields. The campaign plan directs that the archived figure be treated
"*as a prior*" rather than as an established device property
([`dev/active/0de41c82/plan.md`](../../active/0de41c82/plan.md):13), because the
one observed hang's attribution to a watchdog timeout was explicitly retracted
([`../b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md):264-268).
It is cited here on those terms.

Measured against that prior, this campaign's launches sit inside it with margin,
and `analysis.py` section 15 tabulates every device row:

- The declared operating point's launch runs a kernel span of 0.004672 s,
  **0.00002 of the archived 190 s boundary**, carrying $4.299 \times 10^7$ units
  of $M \cdot 2^n$ work.
- The longest kernel span per launch anywhere in this campaign is 21.7228 s
  (`f7-lookup-table-control`, $n = 28$, $M = 1$), 0.1143 of the boundary, inside
  the "≈10–117 s" band the archived calibration reports for its bounded
  sub-batches.
- The largest per-launch work anywhere in this campaign is
  $1.074 \times 10^9$ (`gpu_hip`, $n = 20$, $M = 1024$), whose kernel span is
  17.6686 s, 0.0930 of the boundary.
- No cell in this run failed with a device fault; every step reports
  `status=completed exit=0`.

That last point is a one-run observation on this host and not a bound. This
campaign supplies 16 device-backed cells' worth of evidence toward the bound the
study still owes.

**Verdict: the throughput half of REQ-19 is met with a very large margin at the
declared operating point; the launch-duration half is met against the only
committed bound, which is an archived prior rather than a bound this study has
yet derived.**

## 16. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §2, §3, §4.2 | **Satisfied.** The current GPU path at both configured batch sizes, both planned $\mathbb{F}_7$ prototype paths that execute, and all three in-tree CPU paths this field has are compared over $\mathbb{F}_7$ at five orders, with the best applicable CPU path identified per order from this run's own data. Every timing cell draws from one identical sampler on preregistered, structurally disjoint per-cell stream addresses (`main.rs:54`, `:371-391`, `:593-598`); the equivalence comparison runs on one literally identical matrix corpus per $(q, n)$ (`equivalence.rs:139`, `:186-192`). |
| REQ-02 | §1, §4.1 | **Satisfied.** Every listed item maps to a named CSV column or provenance line, tabulated in §4.1. |
| REQ-03 | §4, §6 | **Satisfied.** The end-to-end Ryser loop is isolated for all five executing backends at every order each runs. Gray update, at all five orders: durations for both of this field's circuits, the packed host one (`cpu_scalar`, at the two orders its sixteen-lane representation reaches) and the byte device one (`gpu_hip`, at all five), with no censored row; all three registered $\mathbb{F}_7$ prototypes recorded with the structural reason that their update runs fused inside their full-permanent kernel with no device source isolating it; the remaining CPU backends recorded as having no distinct evaluator, and `cpu_scalar` above $n = 16$ recorded with its representation's lane bound. Horizontal product, at all five orders: durations on both branches for the F7Lookup circuit under both names that select it, durations on six of twenty branch cells for the F7ThreePlane circuit with each of the other fourteen carrying its censoring reason verbatim, and every CPU and out-of-field row recorded with its structural reason. No absence is left without a reason of the kind the criterion names. |
| REQ-04 | §10, §4.5 | **Satisfied, with its coverage stated exactly.** Both mappings are measured at the three matching orders $n \in \{12, 16, 20\}$; at $n \in \{24, 28\}$ the control has no measured cell because all four of its cells are censored (§5), and §10 states that rather than deriving a ratio from a projection. The control's fixed $M \in \{256, 1024\}$ and each prototype cell's probe-calibrated $M \in \{1, 2, 3, 18, 29, 41, 46, 695, 1109, 5833\}$ are recorded beside their throughputs in the §10 mapping table and in every throughput table of §4. §4.5 quantifies the confound: the batch size *is* the prototype's device parallelism, tabulated as 32–186 656 active lanes against the control's 256 or 1024, with the probe cost that sets each batch, the reconstruction of every batch from that probe, and the printed-precision bound on the four cells the reconstruction does not hit exactly. §10 names the one cell pair where the batches nearly match, the $n = 16$ three-plane cell at $M = 1109$ against the control's 1024, as the closest this run comes to separating the mapping effect from the batch effect. The mappings are therefore comparable on the order axis over the three orders both occupy. |
| REQ-05 | §4.3 | **Satisfied.** `kernel_device_s` is its own device-event column on all four device paths; allocation, copy, and host serialisation are outside it, and the residual is derivable per cell. §12 states the one qualification, that on the bit-sliced path this column brackets two kernel launches rather than one. |
| REQ-06 | §4.3 | **Satisfied.** `h2d_device_s`, `d2h_device_s`, `host_submission_s`, and `device_submission_to_kernel_s` are four separate columns, and per-launch costs follow from them and `reps` without a second run. §4.3 states, with the `host_submission_s`/`eval_s` ratios as evidence, that the host-clock column measures an asynchronous submission on `gpu_hip` rows and a synchronous submit-and-complete on prototype rows, so that it is not misread as a prototype launch overhead. |
| REQ-07 | §7, §7.1, §7.2 | **Satisfied.** Registers per thread, private scratch per thread, static LDS per block, and both spill counts are reported for all nine measured kernels in §7's table, each with its resource-log line. Predictions are paired with those measurements wherever a committed design states one: the three prototype timing kernels against the README's $24n$ and $8n\lceil n/16 \rceil$ per-block shared tables, and every remaining kernel against the exact 0 bytes/block its own launch and translation unit state. §7.2 gives the coverage per kernel and per quantity. No committed design states a per-lane register budget for any of the nine, which is recorded as such with the scope of the F_3-only statement that might be mistaken for one, and no post-hoc figure is supplied — a figure back-derived from the measurement could not afterwards be diverged from it. The README's committed prior resource table for the four prototype kernels is reported separately in §7.1 as an exact 24-figure replication and explicitly not as prediction coverage, and its own reading of the 24-byte lookup-`<2>` scratch result is quoted and preserved. The one blind spot is recorded rather than closed by assumption: the compiler's static-LDS field cannot observe the launch-time tables, so that prediction is neither confirmed nor refuted here and no occupancy conclusion in §8 rests on reading its 0 as an absence. |
| REQ-08 | §8 | **Satisfied.** §8's table names the occupancy-limiting resource for each of the nine measured $\mathbb{F}_7$ kernels: per-lane vector registers for the shipped control, which reports 8 waves/SIMD, and the `gfx1030` architectural wave-slot ceiling of 16 for the other eight. Each row carries its per-thread usage, its occupancy, and its resource-log line. The derivation rests on stated, checkable evidence: `VGPRs` alone reproduces all 26 occupancy entries in the receipt under a 1 024-register, 16-unit allocation capped at 16, which `analysis.py` section 8 asserts; scalar registers and private scratch are shown not to bind by counterexample in the same receipt (86 SGPRs at occupancy 16, 1040 and 24 scratch bytes at occupancy 16); the empty `permanent_wave_gpu_probe` at zero usage reporting 16 establishes the ceiling; and this field's own `permanent_bipedal7_kernel` at 128 `VGPRs` reporting 8 shows the field falls under pressure rather than being constant. The naming holds at each measured order $n \in \{12, 16, 20, 24, 28\}$, with the evidence stated: $n$ is a runtime argument and not a template parameter on the shipped kernel or either three-plane kernel, and the one templated path is templated on word count rather than on $n$, emits two instantiations, and reports 16 on both. The dynamic-LDS blind spot is stated rather than assumed away, and §8 scopes it to the three rows it actually qualifies. |
| REQ-09 | §7.1, §7.2, §9, §17 | **Satisfied.** No measurement in this campaign contradicts a numeric register or shared-memory prediction that a committed design states for this field, and §7.1 states why rather than leaving it implicit: the only committed budget statements are the $24n$ and $8n\lceil n/16 \rceil$ dynamic shared tables, which the compiler's static field cannot observe, and the exact 0 B/block for the six kernels whose launches request none, which it confirms. No prediction is silently restated: the F_3-scoped nine-register lower bound is quoted with its scope and not applied here, the README's prior F_7 resource table is reported as a replication with all 24 figures matching, the README's own non-interpretation of the 24-byte lookup-`<2>` scratch figure is preserved, and the one qualitative committed claim about register residency is quoted and confirmed against zero spills on all four prototype instantiations. The contradictions this run does produce are of other committed statements and are carried with them — the projection-accuracy statement against this file's own prototype chains (§5), the prior artifact's sub-minimum published cells (§11.1), and the $n = 24$ branch-frequency interval (§9) — collected in §17. |
| REQ-10 | §9 | **Satisfied.** Both frequencies, both exact expectations $1 - (6/7)^n$ and $(6/7)^n$, their complement relation, the sample count 4096, and Wilson 95 % intervals are reported at all five measured orders, with the interval method fixed deterministically at $z = 1.959963984540054$. A second, larger branch observation from the same file's timed-operation counts is reported beside it with its own intervals, its $z$ statistics, and its dependence on the same purpose stream stated, including the one order where the exact marginal falls outside the interval. |
| REQ-11 | §3 | **Satisfied.** The CPU oracle is `cpu_scalar` at $n \le 16$ and `cpu_ryser_generic` above it — the switch is the equivalence file's own, stated in its preamble and forced by the packed kernel's sixteen-lane assertion — and every executing path is re-confirmed identical against the applicable oracle on the campaign host, in the same run, before any timing cell, at every order the grid times and at $n = 8$ below them. The $n = 24$ and $n = 28$ cells compare 32 and 4 matrices against 512 at the smaller orders; §3 states this rather than presenting it as an equal gate. |
| REQ-12 | §5 | **Satisfied.** All four censored cells state their censoring reason and the rate they were projected from, and carry `NaN` for both throughput columns; §4.3 withholds their kernel-only rates for the same reason. The projection's measured bias is given per chain, and §5 states which chain each censored cell sits on: all four are on `gpu_hip` chains whose every measured step runs low, so their projections are conservative, and neither prototype chain — both of which land high — owns a censored cell. |
| REQ-13 | §2 | **Satisfied, and not vacuous for this field.** `f7-three-plane-accumulator` is a planned $\mathbb{F}_7$ candidate that cannot execute as a permanent path on the target device, and it is named in the record with its falsification, quoted verbatim: its HIP translation unit holds a single-thread three-plane accumulator conformance probe and not a full-permanent batch kernel. §2 states that this is a structural falsification rather than a compile or resource one, and cites the compiler receipt entry that shows the same source compiling clean for `gfx1030`, so the exclusion is not mistaken for a build failure. Its exclusion from the timing comparison cites that evidence, and no timing is attributed to it in §4; it appears in §6.2 only because that isolate times a circuit rather than a path. The four out-of-field prototypes carry their by-field reasons. Separately and distinctly, three in-tree CPU paths cannot execute for this field at all and two more stop at $n = 16$; they are named with the library-capability absence and the lane bound that exclude them, quoted verbatim, confirmed independently in the equivalence file, and §2 states that these are host-tree capability limits rather than device falsifications. |
| REQ-14 | §4.4 | **Satisfied.** The best-performing prototype by ratio against the best applicable in-tree CPU path is `f7-three-plane-permanent`; its best operating point is $n = 20$, $M = 41$, where it measures 8 397.8509 matrices/s against `cpu_ryser_generic` at 15.3900, a ratio of **545.6693×**, with a launch duration of **4.8485 µs per launch** on the device clock (`device_submission_to_kernel_s` 0.004960 s over 1 023 launches). §4.4 also states the alternative reading — the highest absolute prototype rate in the campaign is `f7-lookup-table-control` at $n = 12$, 630 625.0668 matrices/s and 8.7878× the best CPU path, at 4.6533 µs per launch — rather than leaving the choice of reading implicit. §4.5 records that this operating point's batch is not matched to the control's. |
| REQ-15 | §1 | **Satisfied.** Three exact commands for this field plus the shared equivalence command are committed with their revision, toolchain, and binary hashes; `analysis.py` regenerates every table here from the committed artifacts with one command; the run executes on the prepared benchmark host under the repository's full-host benchmark mutex, with a pristine-worktree refusal and a binary-hash verification enforced both before and after the lock is acquired, and with the machine warm-up held by the first grid under the same lock. |
| REQ-16 | §12 | **Partially met, and the shortfall is named.** The host-side portion is established from source and is zero: the harness streams canonical matrix bytes and the byte-to-plane transpose runs on the device as its own kernel, so there is no host-side preparation to measure. The device-side portion is measured, but not separately from the Gray walk: both launches sit inside one event-bracketed span by the harness's own documented design, so `kernel_device_s` is their sum and no committed column separates them. §12 gives the combined per-launch figure at all five orders, shows that no harness mode and no committed artifact of this run times the preparation kernel alone, and identifies the only preparation-specific route in the tree as a correctness stage that emits no duration. No separated figure is invented. **The criterion's "separately from the Gray walk" is not satisfied by this run's evidence.** |
| REQ-17 | §13 | **Not met.** Achieved occupancy is not measured for any $\mathbb{F}_7$ kernel: the runner invokes no profiler, and `analysis.py` section 13 asserts that no committed artifact of this run carries profiler counters. The criterion's escape clause permits recording achieved occupancy as not measured only where the profiler is unavailable on the campaign host, and that condition does not hold — `rocprof`, `rocprofv2`, and `rocprofv3` are installed and on `PATH`. §13 therefore records the true reason rather than the permitted one, states it as unmet, and reports the compiler occupancy figure explicitly as a compile-time prediction that this receipt does not offer as a stand-in. Closing this needs a profiled re-measurement. |
| REQ-18 | §14 | **Satisfied.** The one bit-sliced path that executes as a permanent path, `f7-three-plane-permanent`, is `measured` at $n = 16$, $n = 20$, and $n = 24$ — and at $n = 12$ and $n = 28$ besides — and is `identical` with zero mismatches against the campaign oracle at every one of those orders, on 512, 512, and 32 matrices at the three the criterion names. §14 settles which paths are bit-sliced from source, records that the second bit-sliced candidate does not execute as a permanent path at any order and so has no cell to measure, and states the measurement scale at each order rather than implying it. The sixteen-lane limit the demonstration is above is the same one this run records refusing the packed CPU path at those orders, which is why the oracle is the independent generic Ryser driver there. |
| REQ-19 | §15 | **Aspirational; throughput met, bound met against a prior.** At the declared operating point $n = 20$, `f7-three-plane-permanent` measures 8 397.8509 matrices/s against `cpu_ryser_generic` at 15.3900, a factor of **545.6693×** against a $1.5\times$ target, with a launch duration of 4.8485 µs on the device clock and a kernel span of 0.004672 s per launch. §15 states the caveat that the denominator at this order is a single-threaded generic driver because this field has no packed CPU kernel there. No safe launch-duration bound derived by this study is committed yet; measured against the only committed figure — the archived ≈190–200 s per-launch calibration, cited as a prior rather than as an established device property, and with its $q = 5$ work budget deliberately not transferred to this field — this operating point sits at 0.00002 of the span boundary. |

## 17. What this campaign does not establish

Collected so a reader does not have to reassemble it from the sections above.

1. **Achieved occupancy is not measured for any kernel.** REQ-17's runtime
   evidence does not exist in this run, the escape clause's precondition does not
   hold on this host, and §13 records that rather than either fabricating a
   figure or claiming an exemption. This is the campaign's largest gap and the
   only criterion it reports as unmet.
2. **Bit-plane preparation is not separated from the Gray walk.** The two
   launches share one event span by design, so §12 can attribute the cost to the
   device and bound it by the combined span but cannot divide it. Separating them
   needs a harness change, not a re-reading.
3. **The two mappings are compared at three orders, not five.** All four control
   cells at $n \in \{24, 28\}$ are censored before running, so §10's mapping
   ratios exist only at $n \in \{12, 16, 20\}$ and nothing here settles the
   ordering between the two mappings at this field's two largest orders.
4. **Almost no prototype cell is measured near the control's batch sizes.**
   Every prototype cell sizes itself from a one-matrix probe, so the mapping
   comparison mixes the mapping effect with a device-parallelism effect that
   ranges over a factor of 5 833 across the prototype cells. The one near-match
   is the $n = 16$ three-plane cell at $M = 1109$ against the control's 1 024;
   everywhere else, separating the two needs a run with the prototype batch
   sizes pinned to the control's.
5. **The $n = 12$ ordering between the two prototypes is a batch artefact as
   much as a circuit result.** The lookup control leads the bit-sliced path
   0.4524 to 1 at that order while running 5 833 matrices against 46, and
   186 656 active lanes against 1 472 (§4.5). Nothing in this run isolates how
   much of that reversal is the circuit, and it is the one order where the
   bit-sliced path does not lead.
6. **The $n = 20$ headline ratio is not decomposed.**
   `f7-three-plane-permanent` at 545.6693× the best CPU path runs 1 312 lanes
   against the shipped path's 1 024 and is compared against a single-threaded
   generic driver rather than a packed or parallel one. Both the mapping term
   and the choice of denominator are large, and no cell in this run separates
   them.
7. **The two horizontal-product circuits are not ordered.** The F7ThreePlane
   circuit yields a nonzero-branch duration at exactly two cells, and its partner
   branches there are censored (§6.2). The 23.3× and 312× gaps against the
   F7Lookup circuit at those cells are reported as the two figures they are, and
   no ordering of the circuits is asserted from them.
8. **The $24n$-byte and $8n\lceil n/16 \rceil$-byte shared column tables are
   unmeasured.** The compiler reports static LDS only, so this field's committed
   shared-memory prediction is neither confirmed nor refuted here, and no
   prototype cell in this run is large enough for shared-memory pressure to bind.
9. **No per-lane register budget is predicted for any of the nine measured
   kernels.** The prediction-beside-measurement pairing is one-sided on registers
   throughout this field (§7.2). Their measured figures stand on their own and
   there is nothing to diverge from, so this campaign cannot test a register
   hypothesis for any kernel it measures.
10. **The 32-byte gap between declared and reported scratch is unexplained.**
    `permanent_bipedal7_kernel` declares 4 032 bytes of runtime-indexed private
    arrays and the compiler reports 4 000 bytes/lane. No committed source or
    receipt accounts for the difference, and none is asserted here.
11. **The $n = 28$ equivalence cell compares four matrices** and the $n = 24$
    cell 32, against 512 at the four smallest orders, so this field's two largest
    orders rest on much weaker correctness gates than the rest — including the
    $n = 24$ gate that REQ-18's demonstration depends on.
12. **The Gray-update host circuit is not measured above $n = 16$.** The packed
    $\mathbb{F}_7$ representation has sixteen lanes, so §6.1's host-versus-device
    latency comparison exists at two orders and nothing here extends it.
13. **The safe launch-duration bound is not this study's own.** §15 measures
    against an archived prior whose watchdog attribution was retracted and whose
    work budget is stated for a different field; deriving the study's own bound
    is a separate deliverable and this campaign contributes 16 device-backed
    cells toward it rather than closing it.
14. **The permanent-zero fraction at $n = 28$ pools 28 matrices.** Its interval
    spans a factor of 7.3 and it supports nothing on its own (§9.1).

Three observations in this run contradict a statement outside its own numbers
and are recorded rather than restated, per `@/inv/falsification-preserved`:

- **The grid preamble's projection-accuracy statement fails on both prototype
  chains.** The preamble scopes its "*lands LOW at every step*" claim to the
  $q = 3$ GPU chain and calls the step to other fields an extrapolation. On this
  field's own GPU chains the claim holds ($-16.1\%$ to $-4.3\%$), which covers
  all four censored cells. On the two prototype chains it fails badly, landing
  high by up to 1685.3 % on the lookup control and 367.8 % on the three-plane
  path. Neither chain owns a censored cell in this field, so no published
  projection here inherits that failure — but the failure is the second and third
  instance of the pattern the $\mathbb{F}_3$ campaign predicted and the
  $\mathbb{F}_5$ campaign first observed with a censored cell attached.
- **The prior grid publishes two rates its own stated stopping rule forbids** —
  $q = 5$, $n = 28$ at one repetition and $q = 3$, $n = 28$ at three, both
  against a five-repetition minimum reached only after the 120 s cap (§11.1).
  Neither is a $q = 7$ cell, and this run's own $q = 7$ grid has no cell outside
  the rule. §11.1 additionally separates a third prior cell that a total-only
  reading of the cap would wrongly flag, so that bug `4fdd781a`'s correction is
  applied to the two cells that need it and not to the one that does not.
- **The exact marginal branch expectation falls outside the Wilson interval of
  the $n = 24$ timed-operation observation**, at $z = +2.85$, on all four
  backends that resample the same addresses (§9). It is one order of five under a
  nominal 95 % procedure with no multiplicity adjustment, and the four other
  orders sit at $|z| \le 1.24$.
