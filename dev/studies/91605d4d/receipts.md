# $\mathbb{F}_5$ preregistered receipt campaign — committed receipts

Campaign run `20260814T230032Z-2085453`, field $q = 5$, grid execution id
`5002`. Every figure below is derived from the raw artifacts committed beside
this file; [`analysis.py`](analysis.py) in this directory regenerates all of
them from those artifacts and prints them under the section headings used here.

```sh
python3 dev/studies/91605d4d/analysis.py
```

| Artifact | Role |
| --- | --- |
| [`…-q5-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q5-grid.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q5-grid.log) | End-to-end Ryser timing grid, 75 cells |
| [`…-q5-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q5-gray-update.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q5-gray-update.log) | Dependency-chained Gray-update isolate, five orders |
| [`…-q5-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q5-horizontal-product.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q5-horizontal-product.log) | Horizontal-product isolate and zero/nonzero branch frequencies, five orders |
| [`../047b62ed/…-shared-equivalence.csv`](../047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv) / [`.log`](../047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.log) | Backend-equivalence gate, all fields in one global run |
| [`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt) | Revision, toolchain, binary hashes, host inventory, exact commands |
| [`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt) | Per-step status and exit codes |
| [`../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/`](../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/) | Compiler kernel-resource receipt and per-kernel logs |
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
$\mathbb{F}_5$ prototype executable `f5-wave-device-evidence` SHA-256
`bee168a131de7ca9f5a916363dee9e827e0136ac556d4153790073b9ce75e088`. Sources:
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
([`../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt)).
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
[`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt)
and repeated in the `# invocation:` line of each CSV and the `# command:` line
of each log. The equivalence step of §3 is one global invocation shared by all
three fields, so its command is recorded in the $\mathbb{F}_3$ study's
provenance file beside the CSV it wrote, and in that CSV's own `# invocation:`
line. This field's own run summary carries all four steps that touch it — the
shared equivalence step and the three $q = 5$ steps — and each reports
`status=completed exit=0`
([`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt)).

The campaign runs under
[`dev/scripts/permanent-campaign-runner.sh`](../../scripts/permanent-campaign-runner.sh),
which holds the repository's canonical benchmark mutex `/tmp/gf2-ccx1.lock`
through
[`dev/scripts/ccx1-bench-flock.sh --full-host`](../../scripts/ccx1-bench-flock.sh)
for the whole internal pipeline rather than once per step
(`permanent-campaign-runner.sh:604-607`); `--full-host` deliberately omits
`taskset` because the grid's rayon cells are named on the full processor while
still sharing the one lock domain (`ccx1-bench-flock.sh:13-15`, `:27`).
`measure` refuses a non-pristine tracked worktree twice: once before queueing
for the mutex and again inside the lock-held child before any step runs
(`permanent-campaign-runner.sh:646`, `:623`), so a commit or a rebuilt binary
landing during the lock wait cannot reach the campaign unchecked.

**This field's grid carries `--skip-machine-warmup`, and that is a property of
the pipeline rather than of the cell.** The 90 s full-rayon machine warm-up
belongs to the first grid executed under the held lock, which is the $q = 3$
grid; every later grid under the same lock passes the flag and inherits the
warmed host (`permanent-campaign-runner.sh:567-577`). The $q = 5$ grid preamble
records this as `machine warmup: skipped by --skip-machine-warmup; caller must
preserve a prior locked warmup`, and the lock is never released between the two
grids. Every cell still performs its own untimed warm-up of at least 3 s before
its first timed repetition, which the preamble's `protocol:` line states.

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

Reproduction requires the exact commands as recorded, including `--only q=5`,
`--execution-id 5002`, and `--skip-machine-warmup`. A cell's first stream index
is `execution_id * 22 500 000 + order_index * 100 000 + 1`
(`dev/research/permanent-sampling-feas/src/main.rs:371-391`, `:54`, `:81`), and
`order_index` is the cell's position after the spec list is shuffled with
`SEED_ROOT` and stably sorted by ascending $n$
(`dev/research/permanent-sampling-feas/src/main.rs:593-598`). The shuffle runs
over the filtered list (`:576`), so the same execution id with a different
`--only` filter addresses different matrices. Execution `5002` therefore owns
`112545000001..=112567500000`, which is the block the grid preamble records and
which `analysis.py` section 1 recomputes.

## 2. Candidate roster and what executes (REQ-01, REQ-13)

The grid enumerates the full planned candidate set from the prototype registry
rather than a pre-filtered one; every `Backend::ALL` entry appears at every
order, and the single GPU entry expands to both configured batch sizes
(`dev/research/permanent-sampling-feas/src/main.rs:305-334`, `:50`). The file
holds 75 cells: 15 per order, of which 29 are `measured`, 6 are `censored`, and
40 are `unsupported`. Every non-`measured` cell carries its reason in `note`.

| Path | Class | Grid outcome | Reason recorded in `note` |
| --- | --- | --- | --- |
| `gpu_hip` (`permanent_bipedal5_kernel`) | current GPU path | `measured` at $M \in \{256, 1024\}$ and $n \in \{12, 16, 20\}$; `censored` in all four cells at $n \in \{24, 28\}$ | see §5 |
| `f5-byte-control` (`f5_byte_control_kernel`) | planned $\mathbb{F}_5$ prototype, lane-owns-interval | `measured` at $n \in \{12, 16, 20, 24\}$; `censored` at $n = 28$ | see §5 |
| `f5-three-plane` (`f5_three_plane_kernel`) | planned $\mathbb{F}_5$ prototype, lane-owns-interval | `measured` at every order | — |
| `cpu_scalar` (`permanent_bipedal5`) | in-tree CPU | `measured` at every order | — |
| `cpu_rayon_batch_scalar` (`permanent_bipedal5` per matrix across rayon workers) | in-tree CPU | `measured` at $n \in \{12, 16, 20, 24\}$; `censored` at $n = 28$ | see §5 |
| `cpu_ryser_generic` (`permanent_ryser`) | in-tree CPU | `measured` at every order | — |
| `cpu_avx2` | in-tree CPU | `unsupported` | `gf2-algebra exposes no AVX2 permanent path for F_5` |
| `cpu_rayon_batch_avx2` | in-tree CPU | `unsupported` | `gf2-algebra exposes no AVX2 permanent path for F_5` |
| `cpu_rayon_intra_matrix` | in-tree CPU | `unsupported` | `gf2-algebra exposes no rayon permanent path for F_5` |
| `wave-gf3` | planned $\mathbb{F}_3$ prototype | `unsupported` | `wave-gf3 evaluates F_3 permanents, not F_5` |
| `fold-gf3` | planned $\mathbb{F}_3$ prototype | `unsupported` | `fold-gf3 evaluates F_3 permanents, not F_5` |
| `f7-lookup-table-control` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-lookup-table-control evaluates F_7 permanents, not F_5` |
| `f7-three-plane-permanent` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-three-plane-permanent evaluates F_7 permanents, not F_5` |
| `f7-three-plane-accumulator` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-three-plane-accumulator has no device batch evaluator: hip/f7_three_plane_equivalence.hip holds a single-thread three-plane accumulator conformance probe, not a full-permanent batch kernel; the permanent-shaped use of this arithmetic is the f7-three-plane-permanent candidate` |

The kernel each CPU row forces is read from the harness rather than assumed:
`cpu_scalar` and `cpu_rayon_batch_scalar` both call
`gf2_algebra::permanent::bipedal5::permanent_bipedal5` over a `Packed5Matrix`,
the second across rayon workers one matrix each
(`dev/research/permanent-sampling-feas/src/backend.rs:410-411`, `:431-433`);
`cpu_ryser_generic` calls the field-agnostic `permanent_ryser` over unpacked
`Fp<5>` elements (`:458`); `gpu_hip` calls
`gf2_algebra::gpu::permanent_batch_bipedal5` (`:471`). So this field's CPU
baseline is already bit-sliced on the update step: `permanent_bipedal5` holds
its column sums in one packed `Packed5` `u64`-triple and updates them in $O(1)$
bit-plane work per Gray step, then folds the first $n$ lanes into a scalar with
a serial lane-by-lane `fold_mul_first_n`
(`crates/gf2-algebra/src/permanent/bipedal5.rs:103-107`, `:135-142`).

**Both planned $\mathbb{F}_5$ prototypes execute on the target device and are
measured.** A registered candidate's grid cell evaluates that candidate's own
device kernel over the same batch object the built-in backends receive
(`dev/research/permanent-sampling-feas/src/backend.rs:505-531`), reaching
`f5_byte_control_kernel` and `f5_three_plane_kernel` through the candidate's
prebuilt HIP executable, which stays resident for the cell so a batch pays the
pipe transfer and the executable's own per-batch device allocation rather than
a process start
(`dev/research/permanent_wave_gpu/src/device_batch.rs:9-13`, `:318-331`). The
launch geometry is one block per matrix and `active_lanes_for_order(n)` lanes
per block, with $n^2$ bytes of dynamic shared memory for the byte control and
$24n$ bytes for the three-plane path
(`dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip:727-737`), and
`active_lanes_for_order(n) = 32` for $n \ge 5$
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31`, `:16`). Each
lane owns a balanced Gray interval
(`f5_wave_equivalence.hip:241-243`, `wave_ryser_mapping.h:33-40`).

**No planned $\mathbb{F}_5$ candidate fails to execute on the target device**,
so REQ-13's compile, correctness, or resource falsification has no
$\mathbb{F}_5$ device subject in this run, and none is invented. Five
candidates are excluded by field or by the structural absence of a
full-permanent batch kernel, in the words quoted in the table above.

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
reason string appearing in this run's equivalence file (§3), and it is what
their exclusion from the timing comparison cites. It is not a compile,
correctness, or resource falsification on `gfx1030`, and it is not presented as
one. §13 records that reading.

## 3. Equivalence gate (REQ-11)

The equivalence step is the first step of the pipeline
([`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt);
`timestamp_utc: 2026-08-14T23:00:34Z` in the equivalence CSV against
`2026-08-15T00:59:54Z` in the grid CSV), so it precedes every timing cell of the
same run, on the same host and the same binary hash.

The harness `equivalence` subcommand is global: one invocation covers all three
fields and takes no field filter, and `--execution-id` is parsed by `grid`
alone. The run summary therefore records `execution_id=fixed-streams` for this
step, which is what the step does — it draws from fixed stream addresses of the
form $(\text{seed\_root}, q, n, \texttt{equivalence}, 0)$ recorded in its own
preamble, rather than from a reserved per-execution index block. One global run
gates all three fields; this document reads its $q = 5$ rows.

Protocol: the per-order matrix counts of the preamble's `matrices_per_cell:`
line at $n \in \{8, 12, 16, 20, 24, 28\}$, with the scalar single-word kernel as
the reference. Every backend at a given $(q, n)$ is compared against the *same*
matrices: the batch is built once and reused across the whole backend loop,
registered prototype candidates included
(`dev/research/permanent-sampling-feas/src/equivalence.rs:139`, `:186-192`), and
the preamble states the same contract on its `candidates:` line.

| $n$ | Backends compared against `cpu_scalar` | `matrices` | `mismatches` | `zeros_reference` = `zeros_backend` | `status` |
| ---: | --- | ---: | ---: | ---: | --- |
| 8 | `cpu_rayon_batch_scalar`, `gpu_hip`, `cpu_ryser_generic`, `f5-byte-control`, `f5-three-plane` | 512 | 0 | 105 | `identical` |
| 12 | same five | 512 | 0 | 95 | `identical` |
| 16 | same five | 512 | 0 | 110 | `identical` |
| 20 | same five | 512 | 0 | 109 | `identical` |
| 24 | same five | 32 | 0 | 6 | `identical` |
| 28 | same five | 2 | 0 | 0 | `identical` |

All 30 $q = 5$ comparison cells report `mismatches = 0` and
`status = identical`, including both prototypes at every order the grid times,
and the log closes with `all backends agree per matrix`. The whole file carries
102 `identical` rows across the three fields. The 48 remaining $q = 5$ rows
report `matrices = 0` with the reasons of §2 — three in-tree CPU paths absent
for this field and five out-of-field prototypes — and are not equivalence
evidence in either direction.

The equivalence order set covers every grid order and adds $n = 8$ below them,
so no timing in §4 rests on equivalence confirmed only at a smaller order. The
counts fall to 32 and 2 at $n = 24$ and $n = 28$ because the harness sizes each
cell against a 240 s budget from committed per-matrix costs
(`dev/research/permanent-sampling-feas/src/main.rs:64-77`). The $n = 28$ count
is an explicit per-field override, recorded in the preamble's
`matrices_per_cell:` line as `n=28: 4 (q=5: 2)` and derived in the same
preamble's `sample counts:` line from this field's own committed probe costs:
22.054209 s, 22.269200 s, and 24.030546 s per matrix for the three measured CPU
backends, a three-backend sum of 68.354 s per matrix, so four matrices project
to 273 s and two to 137 s against the 240 s budget. A two-matrix cell is a
much weaker gate than a 512-matrix one, and it is the gate this field's largest
order actually has.

## 4. End-to-end Ryser timing grid (REQ-01, REQ-02, REQ-05, REQ-06)

### 4.1 Recorded fields

REQ-02 asks for a specific field set. Each is a named column of
[`…-q5-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q5-grid.csv), or a
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
out-of-field rows carry a literal comma inside both (`… evaluates F_7
permanents, not F_5`), so those lines hold 40 fields against a 38-column
header, and `note` is not the only column past the split. `analysis.py` splits
them on the `seed_root` prefix rather than on field count. The equivalence CSV
carries the same literal commas, but its free text is its last column, so
joining the surplus fields back onto `status` recovers it. The two isolate CSVs
use semicolons inside their free text and need no handling at all.

### 4.2 Composite throughput and the best path per order

Composite matrices per second, the rate the study's envelope is derived from.
Full grid in the CSV; `analysis.py` section 4.2 prints every one of the 29
measured cells.

| $n$ | best overall | rate | best in-tree CPU path | rate | best prototype | rate | best GPU config | rate | prototype / CPU | GPU / CPU |
| ---: | --- | ---: | --- | ---: | --- | ---: | --- | ---: | ---: | ---: |
| 12 | `f5-three-plane` | 207 146.4078 | `cpu_rayon_batch_scalar` | 66 412.1210 | `f5-three-plane` | 207 146.4078 | `gpu_hip` $M{=}1024$ | 22 663.0579 | 3.1191 | 0.3412 |
| 16 | `f5-three-plane` | 66 125.0000 | `cpu_rayon_batch_scalar` | 3 816.8266 | `f5-three-plane` | 66 125.0000 | `gpu_hip` $M{=}1024$ | 1 224.3304 | 17.3246 | 0.3208 |
| 20 | `f5-three-plane` | 7 415.9777 | `cpu_rayon_batch_scalar` | 197.5673 | `f5-three-plane` | 7 415.9777 | `gpu_hip` $M{=}1024$ | 63.9389 | 37.5365 | 0.3236 |
| 24 | `f5-three-plane` | 211.9399 | `cpu_rayon_batch_scalar` | 10.7539 | `f5-three-plane` | 211.9399 | all four cells censored | — | 19.7082 | — |
| 28 | `f5-three-plane` | 1.5777 | `cpu_scalar` | 0.0454 | `f5-three-plane` | 1.5777 | all four cells censored | — | 34.7511 | — |

The best applicable in-tree CPU path is identified from this run's own data
rather than assumed: batch rayon over the packed `permanent_bipedal5` kernel
leads the CPU field at $n \in \{12, 16, 20, 24\}$, and at $n = 28$ that cell is
censored (§5) so the fastest measured CPU cell is single-thread `cpu_scalar` at
0.0454 matrices/s. The censored rayon cell's projection is 0.5761 matrices/s,
which would lead the CPU field at $n = 28$ if it held; it carries no measured
rate, none is substituted for it here, and §5 records what follows for the
$n = 28$ ratio.

The three-plane accumulator leads the byte control at every order both are
measured: `f5-three-plane` over `f5-byte-control` is 4.8768, 12.5899, 431.1439,
and 1358.5891 at $n = 12, 16, 20, 24$. §4.5 records that the two large figures
are measured at batches the same calibration sized 7 and 17 times apart, and
that this run does not separate the circuit effect from the batch effect.

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
(`dev/research/permanent-sampling-feas/src/backend.rs:535-536`, `:562-565`,
`dev/research/permanent-sampling-feas/src/protocol.rs:1173`). The four
not-attempted `gpu_hip` cells of §5 leave the same five columns empty with
`phase_timing_note = event timing unavailable: cell did not run`.

Totals per cell, in seconds. `residual` is
`eval_s − kernel_device_s − h2d_device_s − d2h_device_s − device_submission_to_kernel_s`,
the host-side allocation, serialisation, stream wait, and free that the
dispatcher performs around each call.

| $n$ | path | $M$ | outcome | `eval_s` | `kernel_device_s` | `h2d_device_s` | `d2h_device_s` | `host_submission_s` | `device_submission_to_kernel_s` | residual | kernel / eval |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `f5-byte-control` | 45 | measured | 4.499778 | 4.131049 | 0.043852 | 0.077333 | 4.414607 | 0.022564 | 0.224980 | 0.9181 |
| 12 | `f5-three-plane` | 44 | measured | 2.693805 | 0.798447 | 0.206822 | 0.357133 | 2.299472 | 0.106469 | 1.224934 | 0.2964 |
| 12 | `gpu_hip` | 256 | measured | 4.894007 | 4.615749 | 0.002743 | 0.002009 | 0.000704 | 0.001032 | 0.272474 | 0.9431 |
| 12 | `gpu_hip` | 1024 | measured | 4.755494 | 4.575751 | 0.001881 | 0.001032 | 0.000365 | 0.000521 | 0.176309 | 0.9622 |
| 16 | `f5-byte-control` | 102 | measured | 4.906493 | 4.849877 | 0.003233 | 0.004463 | 4.887972 | 0.001233 | 0.047687 | 0.9885 |
| 16 | `f5-three-plane` | 45 | measured | 3.789008 | 2.481452 | 0.069480 | 0.118152 | 3.560107 | 0.034506 | 1.085418 | 0.6549 |
| 16 | `gpu_hip` | 256 | measured | 5.386661 | 5.364382 | 0.000192 | 0.000126 | 0.000057 | 0.000063 | 0.021898 | 0.9959 |
| 16 | `gpu_hip` | 1024 | measured | 4.998413 | 4.980839 | 0.000140 | 0.000058 | 0.000036 | 0.000029 | 0.017347 | 0.9965 |
| 20 | `f5-byte-control` | 6 | measured | 5.231824 | 5.229181 | 0.000139 | 0.000305 | 5.231718 | 0.000073 | 0.002126 | 0.9995 |
| 20 | `f5-three-plane` | 41 | measured | 4.801438 | 4.636133 | 0.010875 | 0.015115 | 4.760673 | 0.004379 | 0.134936 | 0.9656 |
| 20 | `gpu_hip` | 256 | measured | 39.354818 | 39.342386 | 0.000085 | 0.000049 | 0.000042 | 0.000025 | 0.012273 | 0.9997 |
| 20 | `gpu_hip` | 1024 | measured | 80.052056 | 80.033958 | 0.000168 | 0.000049 | 0.000047 | 0.000025 | 0.017856 | 0.9998 |
| 24 | `f5-byte-control` | 1 | measured | 32.060008 | 32.058006 | 0.000053 | 0.000139 | 32.059995 | 0.000024 | 0.001786 | 0.9999 |
| 24 | `f5-three-plane` | 17 | measured | 5.045892 | 5.038064 | 0.000600 | 0.001099 | 5.044189 | 0.000306 | 0.005823 | 0.9984 |
| 24 | `gpu_hip` | 256 | censored | — | — | — | — | — | — | — | — |
| 24 | `gpu_hip` | 1024 | censored | — | — | — | — | — | — | — | — |
| 28 | `f5-byte-control` | 1 | censored | 230.907585 | 230.905844 | 0.000026 | 0.000063 | 230.907579 | 0.000010 | 0.001642 | 1.0000 |
| 28 | `f5-three-plane` | 2 | measured | 6.338337 | 6.336480 | 0.000049 | 0.000117 | 6.338311 | 0.000024 | 0.001667 | 0.9997 |
| 28 | `gpu_hip` | 256 | censored | — | — | — | — | — | — | — | — |
| 28 | `gpu_hip` | 1024 | censored | — | — | — | — | — | — | — | — |

The `f5-byte-control` row at $n = 28$ is censored and retains its phase columns
as diagnostics: it executed two repetitions before the cap ended it, so its
device spans exist while its derived rates do not. The four `gpu_hip` cells
were never attempted and have no spans to retain.

**`host_submission_s` measures two different things, and the ratio to `eval_s`
says which.** On `gpu_hip` rows it is the instrumented dispatcher's own
asynchronous submission span (`backend.rs:613`), and it is $0.0000$–$0.0001$ of
`eval_s`. On prototype rows it is a host clock wrapped around the candidate's
whole synchronous batch evaluation (`backend.rs:516-520`), and it is
$0.8536$–$1.0000$ of `eval_s`, reaching $0.9997$–$1.0000$ from $n = 24$ up,
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
| 12 | `f5-byte-control` | 45 | measured | 209 025 | 50 598.5284 | 46 452.2893 | 42 476.2230 | 1.1912 |
| 12 | `f5-three-plane` | 44 | measured | 959 508 | 1 201 717.8347 | 356 190.5771 | 207 146.4078 | 5.8013 |
| 12 | `gpu_hip` | 256 | measured | 54 272 | 11 758.0050 | 11 089.4819 | 10 853.5891 | 1.0833 |
| 12 | `gpu_hip` | 1024 | measured | 112 640 | 24 616.7241 | 23 686.2863 | 22 663.0579 | 1.0862 |
| 16 | `f5-byte-control` | 102 | measured | 26 214 | 5 405.0855 | 5 342.7168 | 5 252.2160 | 1.0291 |
| 16 | `f5-three-plane` | 45 | measured | 320 310 | 129 081.6828 | 84 536.6346 | 66 125.0000 | 1.9521 |
| 16 | `gpu_hip` | 256 | measured | 3 328 | 620.3883 | 617.8224 | 616.5929 | 1.0062 |
| 16 | `gpu_hip` | 1024 | measured | 6 144 | 1 233.5271 | 1 229.1902 | 1 224.3304 | 1.0075 |
| 20 | `f5-byte-control` | 6 | measured | 90 | 17.2111 | 17.2024 | 17.2007 | 1.0006 |
| 20 | `f5-three-plane` | 41 | measured | 36 941 | 7 968.0630 | 7 693.7365 | 7 415.9777 | 1.0744 |
| 20 | `gpu_hip` | 256 | measured | 1 280 | 32.5349 | 32.5246 | 32.5194 | 1.0005 |
| 20 | `gpu_hip` | 1024 | measured | 5 120 | 63.9728 | 63.9584 | 63.9389 | 1.0005 |
| 24 | `f5-byte-control` | 1 | measured | 5 | 0.1560 | 0.1560 | 0.1560 | 0.9998 |
| 24 | `f5-three-plane` | 17 | measured | 1 071 | 212.5817 | 212.2519 | 211.9399 | 1.0030 |
| 24 | `gpu_hip` | 256 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 24 | `gpu_hip` | 1024 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 28 | `f5-byte-control` | 1 | censored | 2 | withheld | `NaN` | `NaN` | — |
| 28 | `f5-three-plane` | 2 | measured | 10 | 1.5782 | 1.5777 | 1.5777 | 1.0003 |
| 28 | `gpu_hip` | 256 | censored | 0 | withheld | `NaN` | `NaN` | — |
| 28 | `gpu_hip` | 1024 | censored | 0 | withheld | `NaN` | `NaN` | — |

The kernel-only-to-composite ratio is the size of everything the device kernel
does not pay for. It is 5.8013× at $n = 12$ for `f5-three-plane` and 1.0003× at
$n = 28$ for the same path: sampling, packing, reduction, and the dispatcher's
surrounding work cost 82.8 % of the achievable rate at the smallest order on
the fastest path, and 0.03 % at the largest. On the shipped path the ratio never
exceeds 1.0862×, because that kernel is slow enough that its own span dominates
the dispatch even at $n = 12$.

Per-launch cost, dividing each total by that cell's `reps`. Each timed
repetition opens one sampler at one stream index and evaluates one batch
(`dev/research/permanent-sampling-feas/src/protocol.rs:765-781`), and one batch
evaluation is one dispatch — `gridDim.x = M` blocks of one thread for the
shipped kernel
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal5.hip:253-254`,
`:266-270`), `gridDim.x = M` blocks of 32 lanes for the prototypes
(`dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip:727-737`) — so
`reps` is the launch count.

| $n$ | path | $M$ | `reps` | host submission µs/launch | device submission→kernel µs/launch | kernel ms/launch | H2D µs/launch | D2H µs/launch |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `f5-byte-control` | 45 | 4 645 | 950.3998 | 4.8577 | 0.8894 | 9.4407 | 16.6487 |
| 12 | `f5-three-plane` | 44 | 21 807 | 105.4465 | 4.8823 | 0.0366 | 9.4842 | 16.3770 |
| 12 | `gpu_hip` | 256 | 212 | 3.3208 | 4.8679 | 21.7724 | 12.9387 | 9.4764 |
| 12 | `gpu_hip` | 1024 | 110 | 3.3182 | 4.7364 | 41.5977 | 17.1000 | 9.3818 |
| 16 | `f5-byte-control` | 102 | 257 | 19 019.3463 | 4.7977 | 18.8711 | 12.5798 | 17.3658 |
| 16 | `f5-three-plane` | 45 | 7 118 | 500.1555 | 4.8477 | 0.3486 | 9.7612 | 16.5990 |
| 16 | `gpu_hip` | 256 | 13 | 4.3846 | 4.8462 | 412.6448 | 14.7692 | 9.6923 |
| 16 | `gpu_hip` | 1024 | 6 | 6.0000 | 4.8333 | 830.1398 | 23.3333 | 9.6667 |
| 20 | `f5-byte-control` | 6 | 15 | 348 781.2000 | 4.8667 | 348.6121 | 9.2667 | 20.3333 |
| 20 | `f5-three-plane` | 41 | 901 | 5 283.7658 | 4.8602 | 5.1455 | 12.0699 | 16.7758 |
| 20 | `gpu_hip` | 256 | 5 | 8.4000 | 5.0000 | 7 868.4772 | 17.0000 | 9.8000 |
| 20 | `gpu_hip` | 1024 | 5 | 9.4000 | 5.0000 | 16 006.7916 | 33.6000 | 9.8000 |
| 24 | `f5-byte-control` | 1 | 5 | 6 411 999.0000 | 4.8000 | 6 411.6012 | 10.6000 | 27.8000 |
| 24 | `f5-three-plane` | 17 | 63 | 80 066.4921 | 4.8571 | 79.9693 | 9.5238 | 17.4444 |
| 28 | `f5-byte-control` | 1 | 2 | 115 453 789.5000 | 5.0000 | 115 452.9220 | 13.0000 | 31.5000 |
| 28 | `f5-three-plane` | 2 | 5 | 1 267 662.2000 | 4.8000 | 1 267.2960 | 9.8000 | 23.4000 |

What the separation shows. Device launch overhead is nearly constant across
every path, order, and batch size: `device_submission_to_kernel_s` is
4.74–5.00 µs per launch over kernel spans that range from 0.037 ms to 115.5 s,
six orders of magnitude. Device-to-host copy is flat at 9.4–31.5 µs per launch
because each launch returns at most $M$ result words. Host-to-device copy is
9.3–33.6 µs per launch and tracks $M$ and $n$. What does move is the host-side
residual: it is 45.5 % of `eval_s` at $n = 12$ on `f5-three-plane` and 28.6 % on
the same path at $n = 16$, falls to 2.8 % at $n = 20$, and is under 0.2 % from
$n = 24$ up; on the shipped path it is 5.6 % and 3.7 % at $n = 12$ and under
0.5 % from $n = 16$ up. The dispatcher's per-call allocation, serialisation, and
free — not transfer and not launch — is what costs the prototypes their two
smallest orders, and it is attributable from these columns without a second run.

### 4.4 Best operating points (REQ-14)

The best operating point of a device path is taken to be the measured cell
whose composite throughput ratio against the best applicable in-tree CPU path
at that order is largest. Among the prototypes, the one whose best operating
point carries the highest such ratio is the best-performing prototype.

| Path | Best operating point | rate | best CPU path at that order | rate | ratio | device launch |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| `f5-three-plane` (best prototype) | $n = 20$, $M = 41$ | 7 415.9777 | `cpu_rayon_batch_scalar` | 197.5673 | **37.5365** | 4.8602 µs/launch |
| `gpu_hip` (shipped) | $n = 12$, $M = 1024$ | 22 663.0579 | `cpu_rayon_batch_scalar` | 66 412.1210 | 0.3412 | 4.7364 µs/launch |

At `f5-three-plane`'s best operating point the launch duration is
`device_submission_to_kernel_s` 0.004379 s over 901 launches, or **4.8602 µs
per launch** on the device clock. The host-clock column at the same cell reads
`host_submission_s` 4.760673 s over 901 launches, or 5 283.7658 µs per launch,
which is the synchronous submit-and-complete span described in §4.3 rather than
a launch overhead. The shipped path's own figure at its best operating point is
4.7364 µs per launch on the device clock and 3.3182 µs on the host clock, both
single-digit microseconds because that path's host column is an asynchronous
submission.

**The shipped path has no operating point where it beats the CPU.** Its ratio
against the best applicable in-tree CPU path is 0.3412, 0.3208, and 0.3236 at
$n = 12, 16, 20$, and it has no measured cell at $n \in \{24, 28\}$. Its best
ratio and its best absolute rate are the same cell.

By highest absolute throughput rather than ratio, the best prototype cell is
$n = 12$, $M = 44$ at 207 146.4078 matrices/s, which is 3.1191 times
`cpu_rayon_batch_scalar` at the same order; both readings are printed by
`analysis.py` section 4.4.

### 4.5 What the prototypes' batch sizes do to the comparison

The grid fixes `gpu_hip` at $M \in \{256, 1024\}$ and lets every other cell size
itself: a cell with no requested batch size runs a one-matrix probe and takes
$M = \lceil 2\,\text{s} / \texttt{probe\_matrix\_s} \rceil$, clamped to
$[\,\text{floor},\, 65\,536\,]$
(`dev/research/permanent-sampling-feas/src/protocol.rs:724-737`, and
`GPU_BATCHES` at `dev/research/permanent-sampling-feas/src/main.rs:50`). The
floor is one matrix for a single-threaded backend and
`MATRICES_PER_WORKER * rayon::current_num_threads()` for a multithreaded one,
which is $4 \times 24 = 96$ on this host (`protocol.rs:114`, `:732-737`).
`analysis.py` section 4.5 reconstructs $\lceil 2\,\text{s} / \texttt{probe} \rceil$
from each committed probe: it reproduces every prototype cell's batch exactly,
and the two `cpu_rayon_batch_scalar` cells where it does not are the two the
floor holds at 96 against targets of 31 and 2.

For the prototypes the calibration produces batches of 1 to 102 matrices, none
of which matches the control's 256 or 1024.

Because a prototype block owns one matrix and 32 lanes, the batch size *is* the
device parallelism of the cell:

| $n$ | `f5-byte-control` $M$ | active lanes | shared B/block | `f5-three-plane` $M$ | active lanes | shared B/block | `gpu_hip` $M$ | active lanes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 45 | 1 440 | 144 | 44 | 1 408 | 288 | 256 / 1024 | 256 / 1024 |
| 16 | 102 | 3 264 | 256 | 45 | 1 440 | 384 | 256 / 1024 | 256 / 1024 |
| 20 | 6 | 192 | 400 | 41 | 1 312 | 480 | 256 / 1024 | 256 / 1024 |
| 24 | 1 | 32 | 576 | 17 | 544 | 576 | 256 / 1024 | 256 / 1024 |
| 28 | 1 | 32 | 784 | 2 | 64 | 672 | 256 / 1024 | 256 / 1024 |

**The byte control's $n = 20$ cell is where the calibration hurts most, and the
mechanism is in the committed columns.** Its batch falls from 102 at $n = 16$ to
6 at $n = 20$, because `probe_matrix_s` rises from 0.019749 s to 0.393233 s, a
19.9× step against a 20× step in Ryser work — the probe scales as the work model
predicts. The achieved per-matrix `eval_s` rises from 0.000187 s to 0.058131 s
over the same step, a 311× jump. The batch collapse accounts for almost all of
it: the cell drops from 3 264 active lanes to 192, and the probe-to-achieved
amortisation ratio falls from 105.5 to 6.8. A single-matrix probe is a latency
and a batch is a throughput, and this cell is where the gap between them
closes far enough that the calibration under-sizes the batch by a factor the
measurement never recovers. The consequence is stated rather than smoothed
over: the 431× and 1359× three-plane-over-byte-control figures at $n = 20$ and
$n = 24$ are the joint effect of the two circuits and of cells that run 1 312
against 192 lanes and 544 against 32.

The probe is a single-matrix latency and the preamble forbids deriving a
batched rate from it; the ratio between the two is nevertheless informative
about who is being under-sized. `probe_matrix_s` divided by achieved
`eval_s`/matrix is 1.0–1.3 for the two single-thread CPU paths, 12.7–13.5 for
batch rayon, and 1.0–16 235 for the prototypes, falling monotonically with $n$
on both. The prototypes' batches are calibrated from a latency that overstates
their per-matrix cost by up to four orders of magnitude at the small orders,
which is why their measured cells sit far below the batch sizes the same 2 s
target gives the CPU paths.

## 5. Censoring (REQ-12)

Six cells in the $q = 5$ grid are censored, in two kinds. Not one of them
carries a measured throughput value: `composite_matrices_per_s` and
`eval_matrices_per_s` are `NaN` on all six, and §4.3 withholds the kernel-only
rate for the same reason.

**Four `gpu_hip` cells are not attempted.** The censoring rule is applied
before the cell runs: a cell is censored when its projected rate implies a
repetition longer than the 120 s per-cell cap, which for a fixed-batch cell is
$M / \hat{R}$.

| cell | `order_index` | `projected_matrices_per_s` | `projection_reference_n` | implied repetition | `reps` | `matrices` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| $n = 24$, $M = 256$ | 46 | 1.693719 | 20 | 151 s | 0 | 0 |
| $n = 24$, $M = 1024$ | 56 | 3.330152 | 20 | 307 s | 0 | 0 |
| $n = 28$, $M = 256$ | 67 | 0.090735 | 20 | 2 821 s | 0 | 0 |
| $n = 28$, $M = 1024$ | 64 | 0.178401 | 20 | 5 740 s | 0 | 0 |

Each states its censoring reason verbatim in `note`, of the form `not
attempted: the measured rate at n=20 projects to 1.6937 matrices/s at n=24
under Ryser's n*2^n work model, so one repetition would take 151 s against the
120 s cap. … The cell carries no measured rate`. The rate each is projected
from is its own chain's measured $n = 20$ cell: 32.519400 matrices/s at
$M = 256$ and 63.938900 at $M = 1024$, rescaled through $n \cdot 2^n$
($32.519400 \times 20 \cdot 2^{20} / (24 \cdot 2^{24}) = 1.693719$ and
$63.938900 \times 20 \cdot 2^{20} / (28 \cdot 2^{28}) = 0.178401$, both matching
the CSV to seven significant figures).

**Two cells are censored after executing**, by the cap ending timing before
both protocol minimums were met. They retain their diagnostic spans and lose
only their derived rates.

| cell | `reps` | per-repetition wall clock | `projected_matrices_per_s` | `projection_reference_n` |
| --- | ---: | ---: | ---: | ---: |
| $n = 28$, `cpu_rayon_batch_scalar`, $M = 96$ | 1 | 163.812679 s | 0.576099 | 24 |
| $n = 28$, `f5-byte-control`, $M = 1$ | 2 | 115.354091 s and 115.553543 s | 0.008355 | 24 |

Both carry the reason verbatim: `unavailable: 120 s cap ended timing before both
minimums (5 repetitions and 5 s); no derived rate is reported`. The protocol
requires at least 5 repetitions as well as at least 5 s of timed work
(`protocol.rs:103-107`, `:178-191`), and the cap censors rather than truncates,
so in each case the repetition in flight finished and the next was not started.
The rayon cell's batch is held at the floor of 96 by §4.5's rule rather than
chosen: at roughly 1.7 s per matrix a 96-matrix repetition cannot fit the cap at
this order, so the floor is the mechanism of this censoring.

The projection is an estimate and is labelled as one in the CSV preamble. This
run's own $q = 5$ chains measure the direction and size of its bias where both
ends of a step are measured:

| path | step | projection | measured | error |
| --- | ---: | ---: | ---: | ---: |
| `gpu_hip` $M{=}256$ | $12 \rightarrow 16$ | 508.7620 | 616.5929 | $-17.5\%$ |
| `gpu_hip` $M{=}256$ | $16 \rightarrow 20$ | 30.8296 | 32.5194 | $-5.2\%$ |
| `gpu_hip` $M{=}1024$ | $12 \rightarrow 16$ | 1 062.3308 | 1 224.3304 | $-13.2\%$ |
| `gpu_hip` $M{=}1024$ | $16 \rightarrow 20$ | 61.2165 | 63.9389 | $-4.3\%$ |
| `f5-byte-control` | $12 \rightarrow 16$ | 1 991.0730 | 5 252.2160 | $-62.1\%$ |
| `f5-byte-control` | $16 \rightarrow 20$ | 262.6108 | 17.2007 | $+1426.7\%$ |
| `f5-byte-control` | $20 \rightarrow 24$ | 0.8959 | 0.1560 | $+474.3\%$ |
| `f5-three-plane` | $12 \rightarrow 16$ | 9 709.9879 | 66 125.0000 | $-85.3\%$ |
| `f5-three-plane` | $16 \rightarrow 20$ | 3 306.2500 | 7 415.9777 | $-55.4\%$ |
| `f5-three-plane` | $20 \rightarrow 24$ | 386.2488 | 211.9399 | $+82.2\%$ |
| `f5-three-plane` | $24 \rightarrow 28$ | 11.3539 | 1.5777 | $+619.7\%$ |
| `cpu_scalar` | $12 \rightarrow 16$ | 293.1625 | 307.4492 | $-4.6\%$ |
| `cpu_scalar` | $16 \rightarrow 20$ | 15.3725 | 15.5794 | $-1.3\%$ |
| `cpu_scalar` | $20 \rightarrow 24$ | 0.8114 | 0.8259 | $-1.8\%$ |
| `cpu_scalar` | $24 \rightarrow 28$ | 0.0442 | 0.0454 | $-2.5\%$ |
| `cpu_rayon_batch_scalar` | $12 \rightarrow 16$ | 3 113.0682 | 3 816.8266 | $-18.4\%$ |
| `cpu_rayon_batch_scalar` | $16 \rightarrow 20$ | 190.8413 | 197.5673 | $-3.4\%$ |
| `cpu_rayon_batch_scalar` | $20 \rightarrow 24$ | 10.2900 | 10.7539 | $-4.3\%$ |
| `cpu_ryser_generic` | $12 \rightarrow 16$ | 321.1320 | 304.5116 | $+5.5\%$ |
| `cpu_ryser_generic` | $16 \rightarrow 20$ | 15.2256 | 14.9872 | $+1.6\%$ |
| `cpu_ryser_generic` | $20 \rightarrow 24$ | 0.7806 | 0.7771 | $+0.4\%$ |
| `cpu_ryser_generic` | $24 \rightarrow 28$ | 0.0416 | 0.0418 | $-0.4\%$ |

**The four not-attempted `gpu_hip` cells sit on a chain where the projection
runs low, and the two executed censorings do not both.** On both `gpu_hip`
chains every measured step runs low and the magnitude shrinks with $n$, which
is what the launch-amortisation mechanism the preamble names predicts. The
`cpu_rayon_batch_scalar` chain likewise runs low at every step, so its censored
$n = 28$ projection of 0.5761 is conservative in the same direction; §11
records that the prior run published 0.6479 for that cell, 12.5 % above this
projection, which is within the same chain's measured bias band.

**The `f5-byte-control` chain is the exception, and its censored cell inherits
it.** That chain's projection lands high by 474.3 % at $20 \rightarrow 24$, so
the $n = 28$ projection of 0.008355, scaled from the same chain one step
further, has no reason to be conservative and is more likely an overestimate.
§14 records this as the falsification it is: the $\mathbb{F}_3$ campaign wrote
down that a censored prototype cell's projection would not be conservative and
noted that no such cell existed yet
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §13); this campaign has
one, on exactly the kind of chain that campaign named.

No $q = 5$ cell reports a build failure, a correctness failure, or a device
resource exhaustion. The 40 non-`measured`, non-`censored` cells are the
out-of-field prototype rows and the three in-tree CPU paths this field lacks
(§2).

## 6. Component isolation (REQ-03)

REQ-03 asks that the dependency-chained Gray update, the horizontal product,
and the end-to-end Ryser loop each be isolated for every representation of this
field that executes; that a representation whose component admits no distinct
isolate — a fused or branchless circuit — be recorded with that structural
reason in place of a duration; and that a censored isolate carry its censoring
reason. The end-to-end Ryser loop is §4, covering all six executing backends at
all five orders. Both component isolates run at all five orders in this campaign
(`# orders: 12, 16, 20, 24, 28` in each isolate preamble; each order is an
independent row per candidate with its own seed address). Each subsection below
gives the durations it measures, then the reason carried by every row that has
none, and names which of the two kinds that reason is.

### 6.1 Dependency-chained Gray update

Shape: one accumulator reads its immediately preceding add/subtract result — a
latency chain, not independent-operation throughput. Net duration is
`(Σ update spans − Σ same-geometry compiler-barrier spans) / (steps × reps)`,
with CPU rows using paired host spans and HIP rows paired device-event kernel
spans (preamble of
[`…-q5-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q5-gray-update.csv)).

**Both this field's circuits are measured at every order, and no row is
censored.** All 10 measured rows carry a duration.

| $n$ | `cpu_scalar` `net_per_operation_s` | per row | `gpu_hip` `net_per_operation_s` | per row | device / host |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 2.247e-09 | 1.8725e-10 | 2.086705e-06 | 1.738921e-07 | 928.7 |
| 16 | 2.252e-09 | 1.4075e-10 | 2.779060e-06 | 1.736912e-07 | 1 234.0 |
| 20 | 2.252e-09 | 1.1260e-10 | 3.465471e-06 | 1.732736e-07 | 1 538.8 |
| 24 | 2.240e-09 | 9.3333e-11 | 4.156456e-06 | 1.731857e-07 | 1 855.6 |
| 28 | 2.245e-09 | 8.0179e-11 | 4.847539e-06 | 1.731264e-07 | 2 159.3 |

`cpu_scalar` is `measured` at every order over 1 000 001 steps and 1 149–1 153
repetitions, with `duration_basis = host_clock_update_chain`. `gpu_hip` is
`measured` at every order over the same step count and 5 repetitions, with
`duration_basis = device_event_kernel`.

**The two circuits have different shapes in $n$, and the measurement separates
them cleanly.** The host figure is flat in $n$ to within 0.54 %; the device
figure grows by a factor of 2.32 across the same range and is flat to within
0.44 % once divided by $n$. That is exactly what the two sources predict. The
host circuit is the packed `Packed5` add/subtract, one bit-plane operation per
step regardless of order
(`dev/research/permanent-sampling-feas/src/gray_update.rs:305`,
`crates/gf2-algebra/src/permanent/bipedal5.rs:100-107`). The device circuit for
$q \ne 3$ walks the column-sum array one byte per row per step —
`for (int row = 0; row < n; ++row)` inside the step loop
(`crates/gf2-kernels-hip/hip/permanent/gray_update_micro.hip:63-69`) — and its
paired barrier baseline traverses the same geometry, `const int inner = q == 3
? 1 : n` (`:84`), so the subtraction is same-shape at every order. The device's
per-row cost is therefore the invariant quantity, and it is 1.7313–1.7389 ×
10⁻⁷ s across all five orders.

The comparison this permits is a per-operation latency comparison of two
single-chain circuits under the same paired-barrier subtraction, and on that
basis the host's packed update is 928.7× cheaper per Gray step at $n = 12$ and
2 159.3× cheaper at $n = 28$. It is not a throughput claim about either path.

**The two $\mathbb{F}_5$ prototypes carry a structural reason instead of a
measurement**, which is the fused-circuit case: `unsupported: f5-three-plane's
Gray update runs fused inside its full-permanent kernel and no device source
isolates it; timing the shipped gpu_hip update under this name would report
another backend's circuit`, and the same string for `f5-byte-control`
(`dev/research/permanent-sampling-feas/src/gray_update.rs:231-248`). The four
remaining CPU rows carry `unsupported: <backend> has no isolated
dependency-chained Gray-update evaluator`, under the rule the isolate's own
preamble states — "*A row reports a span only from a circuit distinct to the
backend it names*" — so they are the same structural case: `cpu_rayon_batch_scalar`
shares `cpu_scalar`'s packed host circuit and has none of its own to isolate,
and the three paths this field lacks have no evaluator at all.

So this field has two Gray-update circuits across its representations, a packed
host one and a byte device one, and the isolate measures both at every order.
Every other row's absence is recorded as the structural fact that the row's
backend has no distinct circuit to time.

### 6.2 Horizontal product

Shape: each timed sample is one unconditioned row-sum vector. The zero branch
times the representation's early product exit; the nonzero branch times zero
detection plus the complete representation-specific reduction. Timing is paired
device-event kernel spans only, excluding allocation, upload, download,
submission, sampling, grouping, and host policy.

This field has two horizontal-product circuits under three backend names.
`gpu_hip` and `f5-byte-control` both select
`HorizontalProductCircuit::F5Byte` and `f5-three-plane` selects
`HorizontalProductCircuit::F5ThreePlane`
(`dev/research/permanent-sampling-feas/src/horizontal_product.rs:487`, `:499`,
`:500-502`), which enter `horizontal_product_micro_kernel` and dispatch to
`f5_byte_product` and `f5_three_plane_product`
(`crates/gf2-kernels-hip/hip/permanent/horizontal_product_micro.hip:146-150`).
Every $q = 5$ row draws from the same fixed observation address
(`timed_purpose = horizontal_product_timed`, `timed_index_first = 0`), so the
`gpu_hip` and `f5-byte-control` rows are the same circuit timed twice rather
than two circuits, and their agreement is a replicate rather than a comparison.
They agree to within 9.30 % on the zero branch (largest gap at $n = 20$) and
8.67 % on the nonzero branch (largest gap at $n = 12$); those are the largest
disagreements, each a rounding of a maximum over the five orders.

Net per-operation durations, in seconds. `analysis.py` section 6.2 prints every
span and baseline behind them.

| $n$ | `gpu_hip` zero fast | `gpu_hip` nonzero slow | `f5-byte-control` zero fast | `f5-byte-control` nonzero slow | `f5-three-plane` zero fast | `f5-three-plane` nonzero slow | byte / three-plane, nonzero |
| ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| 12 | 1.17e-10 | 7.27e-10 | 1.15e-10 | 7.96e-10 | absent | absent | — |
| 16 | 1.21e-10 | 2.314e-09 | 1.22e-10 | 2.248e-09 | absent | 1.095e-09 | 2.0530 |
| 20 | 1.17e-10 | 8.839e-09 | 1.29e-10 | 8.946e-09 | absent | 2.634e-09 | 3.3964 |
| 24 | 1.81e-10 | 2.6280e-08 | 1.79e-10 | 2.5567e-08 | absent | 6.742e-09 | 3.7922 |
| 28 | 2.02e-10 | 7.4582e-08 | 2.01e-10 | 7.4665e-08 | absent | 1.4667e-08 | 5.0907 |

**The three-plane reduction is the cheaper nonzero-branch circuit at every
order both are timed, and its advantage grows with $n$**: 2.05× at $n = 16$
rising to 5.09× at $n = 28$. That is the shape the two circuits predict — the
byte product walks $n$ bytes serially while the three-plane product reduces
three packed planes — and it is the one place in this campaign where the two
$\mathbb{F}_5$ arithmetic circuits are compared at matched geometry, since
every isolate row launches `gridDim.x = sample_count` blocks of one thread with
`sharedMemBytes = 0` (`horizontal_product_micro.hip:196`).

`f5-three-plane`'s zero-fast branch is censored at every order, and carries its
censoring reason: `zero fast timing unavailable: raw device span minus its
same-geometry baseline was nonpositive; so no false positive rate is reported`.
Its early product exit is cheap enough on this device that the branch runs
faster than the same-geometry barrier baseline it is measured against, by
1.81 × 10⁻⁵ s to 5.93 × 10⁻⁵ s over 6.9–7.5 million operations. The harness
reports no rate rather than a clamped or negative one.

At $n = 12$ the same nonpositive subtraction hits `f5-three-plane`'s nonzero
branch as well, so that row's `outcome` is `censored` rather than `measured`
and its `note` carries both censoring reasons. That is the only cell in this
isolate where a representation yields no duration on either branch.

Every CPU row is `unavailable: <backend> has no distinct device-event
horizontal-product isolate; no generic or host-clock replacement was used`, and
the five out-of-field prototypes carry a by-field reason naming the circuit they
would time instead, of the form `unsupported: fold-gf3 uses the
bipedal3-zero-mask-sign-popcount horizontal-product circuit over F_3; not F_5`.

So of the two $\mathbb{F}_5$ horizontal-product representations, the byte
circuit yields isolated durations on both branches at every order, and the
three-plane circuit yields a nonzero-branch duration at four orders with its
zero branch censored at all five and both branches censored at $n = 12$. No
absence is left without a reason of the kind the criterion names.

## 7. Kernel resources against design-predicted budgets (REQ-07, REQ-09)

Source: the compiler kernel-resource receipt
[`../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt)
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
| `permanent_bipedal5_kernel` | `gpu_hip` end-to-end | 107 | 128 | 4000 | 4 | 6 | 0 | 8 | `permanent_bipedal5.hip.resource.log:1` |
| `f5_byte_control_kernel` | `f5-byte-control` end-to-end | 78 | 77 | 0 | 0 | 0 | 0 | 12 | `f5_wave_equivalence.hip.resource.log:1` |
| `f5_three_plane_kernel` | `f5-three-plane` end-to-end | 20 | 66 | 0 | 0 | 0 | 0 | 12 | `f5_wave_equivalence.hip.resource.log:12` |
| `gray_update_micro_kernel` | Gray-update isolate | 86 | 5 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:1` |
| `gray_update_compiler_barrier_baseline_kernel` | its paired baseline | 13 | 2 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:12` |
| `horizontal_product_micro_kernel` | horizontal-product isolate | 26 | 3 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:24` |
| `horizontal_product_compiler_barrier_baseline_kernel` | its paired baseline | 14 | 2 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:35` |

The same log carries four probe kernels of the $\mathbb{F}_5$ prototype that no
timing in this campaign exercises: `n63_mapping_probe` at 7 SGPRs and 8 VGPRs
(`f5_wave_equivalence.hip.resource.log:23`), `n4_direction_probe` at 9 and 9
(`:34`), `f5_active_mask_probe` at 9 and 3 (`:45`), and `f5_field_and_c4_probe`
at 30 and 3 (`:56`), each with zero scratch, zero spills, zero static LDS, and
occupancy 16.

The two horizontal-product kernels appear in the `permanent_bipedal7.hip` log
because `horizontal_product_micro.hip` has no translation unit of its own: it is
included textually so the $\mathbb{F}_7$ lookup circuit reads that unit's
`__constant__` table, and compiling it alone fails on that undeclared symbol.
The receipt records it as `translation_unit_role: included-fragment` against the
`permanent_bipedal7.hip` log.

### 7.1 Predicted budgets and what the device reports

**Lane-owns-interval prototypes (`f5-byte-control`, `f5-three-plane`).** The
only committed design statement of a resource budget for these two kernels is a
per-block shared-memory allocation, in
[`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md):
"*The compiler's `LDS Size` is static LDS only. It reports zero for both
kernels; this must not be read as an absence of the explicit dynamic shared
column tables requested at launch: $n^2$ bytes for byte control and $24n$ bytes
for the three-plane path.*" (`:154-157`). The launch confirms the sizes:
`shared_bytes` is `n * n` for the byte control and
`3 * n * sizeof(std::uint64_t)` for the three-plane path
(`dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip:727-731`),
backing `extern __shared__ std::uint8_t staged_matrix[]` at `:233` and
`extern __shared__ std::uint64_t staged_columns[]` at `:296`.

| Kernel | predicted per-lane registers | measured per-lane (`VGPRs`) | measured per-wave (`TotalSGPRs`) | predicted per-block shared | measured static LDS |
| --- | --- | ---: | ---: | --- | --- |
| `f5_byte_control_kernel` | none committed | 77 | 78 | $n^2$ B | 0 B (static field only) |
| `f5_three_plane_kernel` | none committed | 66 | 20 | $24n$ B | 0 B (static field only) |

The shared-memory prediction is **not confirmed and not refuted by this
campaign**, and the committed design says so first:
`-Rpass-analysis=kernel-resource-usage` reports static LDS only, and both tables
are requested dynamically at launch, so a 0 in that column is silence rather
than evidence of absence. No occupancy conclusion in §8 rests on treating it as
a zero. §4.5 carries the launch-time figure per cell, 144–784 bytes per block
for the byte control and 288–672 for the three-plane path, from the source
formulas rather than from any measurement.

**No committed design states a per-lane register budget for either
$\mathbb{F}_5$ kernel.** The nine-32-bit-register lower bound in the same README
is stated in its "## F_3 wave-cooperative evidence boundary" section, of
`WaveGf3` and `FoldGf3`: "*Their stable source-level state model is two packed
`u64` words plus `u64` Gray cursor/end bounds and one `u32` partial sum: a 9 x
32-bit-register lower bound.*" (`:160`, `:172-174`), and the $16n$-byte shared
table it pairs with is likewise F_3's (`:422-423`). Neither covers this field,
whose committed tables are $n^2$ and $24n$. That absence is recorded here as an
absence and nowhere filled with a back-derived number; §7.2 states it per
kernel.

**One committed design claim about registers is qualitative rather than
numeric, and this field's measurement confirms it.** The lane-owns-interval
design states that "*Each lane owns its complete packed row-sum accumulator in
registers for the whole of its interval*"
([`dev/active/0de41c82/plan.md`](../../active/0de41c82/plan.md):38). Both
$\mathbb{F}_5$ prototypes report 0 scratch bytes per lane and zero spills of
either kind, so on this device and at this architecture nothing of their
accumulator state is spilled to memory and the claim holds as stated. The
control mapping under the same compiler and flags reports 4 000 scratch bytes
per lane and ten spills, so the contrast is measured on both sides rather than
argued. This is a qualitative residency claim and not a per-lane register
budget, and §7.2 does not count it as one.

**A committed prior measurement of the same two kernels reproduces exactly, and
is reported as replication rather than as prediction.** The README's F_5
section carries a resource table from a separate clean capture at commit
`9b78666c2a75d8217d8191db323c801cb0e0c95e` (`:104-108`, `:149-152`): byte
control at 78 SGPRs, 77 VGPRs, 0 B/lane scratch, 0/0 spills, 0 B/block static
LDS, and 12 waves/SIMD; three plane at 20, 66, 0, 0/0, 0, and 12. Every one of
those twelve figures matches this campaign's receipt. Two independent hipcc
captures of the same source at the same architecture agree field for field.
This is a reproducibility check on the receipt, not a prediction meeting a
measurement, and it is not counted as prediction coverage in §7.2.

**Shipped GPU path (`permanent_bipedal5_kernel`).** Its predicted per-block
shared-memory allocation is exact and available: the launch passes
`sharedMemBytes = 0`
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal5.hip:269`) and the
translation unit contains no `__shared__` declaration, so 0 bytes/block is the
complete shared-memory picture for this kernel and the measurement confirms it.
No committed design document states a predicted per-lane register budget for it.

Its measured resource profile is this field's central quantitative finding, and
it has an exact cause in the source. **128 VGPRs per lane, 4000 scratch bytes
per lane, 4 SGPR spills, 6 VGPR spills, and occupancy 8** — half the wave-slot
ceiling every other kernel in this field's set reaches. The private allocation
comes from `uint8_t columns[63][63]` and `uint8_t col_sum[63]`
(`permanent_bipedal5.hip:132`, `:143`), indexed at runtime by `flip` inside the
Gray walk (`:181-190`), so they cannot be register-allocated. Their declared
sizes are 3 969 and 63 bytes, 4 032 in total, against the 4 000 bytes/lane the
compiler reports; the two do not match exactly and no committed source or
receipt explains the 32-byte difference, so the measured figure is quoted as
measured and no arithmetic identity is asserted for it.

A cross-check in the same receipt shows the figure is structural rather than
incidental: `permanent_bipedal7_kernel`, a different translation unit with the
same two array declarations (`permanent_bipedal7.hip:132`, `:148`), reports
identical figures on all eight fields — 107 SGPRs, 128 VGPRs, 4000 scratch
bytes, 4 and 6 spills, 0 static LDS, occupancy 8. The shipped byte-arithmetic
mapping costs the same resources in both fields that use it.

Nothing in this campaign contradicts a numeric register or shared-memory
prediction that a committed design states for this field, because the two
committed statements are the $n^2$/$24n$ shared tables, which the compiler's
static field cannot observe, and the exact 0 B/block for the five kernels whose
launches request none, which it confirms. The absence is a fact about the
committed record rather than an absence of searching: the study's own
investigation states that no register, spill, or occupancy measurement of any
permanent kernel existed in the tree before this work
([`dev/active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):410-411),
and the only committed resource figures for this field's kernels are the prior
measurement replicated above. That is the state of the record, and §7.2 gives
it per kernel rather than leaving it to be inferred. The contradictions this run
*does* produce are of other committed statements and are recorded in §5 and §14
with the statements they contradict.

### 7.2 Prediction coverage, stated per kernel

Which of the two predicted quantities a committed design actually states, for
each of the seven kernels this campaign measures. Where one exists, §7.1 pairs
it with the measurement above; where none exists, the row says so and no figure
is supplied in its place.

| Kernel | committed per-lane register prediction | committed per-block shared-memory prediction |
| --- | --- | --- |
| `permanent_bipedal5_kernel` | none committed | 0 B/block, exact (`permanent_bipedal5.hip:269`, no `__shared__`) |
| `f5_byte_control_kernel` | none committed | $n^2$ B/block (`README.md:154-157`) |
| `f5_three_plane_kernel` | none committed | $24n$ B/block (`README.md:154-157`) |
| `gray_update_micro_kernel` | none committed | 0 B/block, exact (`gray_update_micro.hip:112`, no `__shared__`) |
| `gray_update_compiler_barrier_baseline_kernel` | none committed | 0 B/block, exact (`gray_update_micro.hip:126`, no `__shared__`) |
| `horizontal_product_micro_kernel` | none committed | 0 B/block, exact (`horizontal_product_micro.hip:196`, no `__shared__`) |
| `horizontal_product_compiler_barrier_baseline_kernel` | none committed | 0 B/block, exact (`horizontal_product_micro.hip:208-209`, no `__shared__`) |

So every measured kernel has a committed per-block shared-memory prediction, and
the measurement confirms it for the five whose launches request none.
**Not one of the seven has a committed per-lane register prediction.** That is
recorded as the state of the committed record rather than as a measurement
result, and no post-hoc budget is derived for any of them from the numbers this
receipt measured — a figure back-derived from the measurement could not
afterwards be diverged from it. This field's prediction-beside-measurement
pairing is therefore one-sided on registers throughout, which is a stronger
absence than $\mathbb{F}_3$'s and is what §14 records as the limit it is.

Two of the seven kernels report nonzero spill counts, both of them the same
shipped `permanent_bipedal5_kernel` figures (4 SGPR and 6 VGPR spills), and no
committed design predicted the spill behaviour of any kernel in this set in
either direction.

## 8. What limits occupancy (REQ-08)

**The resource that limits occupancy differs by kernel in this field, and in
every case it is the per-lane vector-register count or the architectural
wave-slot ceiling — never scalar registers, private scratch, static LDS, or
spills.** Occupancy is a count of waves per SIMD; a kernel whose per-lane
vector-register demand is small enough sits at the hardware maximum, and one
whose demand is large enough is held below it. Three of this field's seven
measured kernels are held below it.

Per-thread usage is quoted as `VGPRs` / `TotalSGPRs` / scratch bytes per lane /
static LDS bytes per block / SGPR + VGPR spills, all from the receipt of §7.

| Kernel | measured per-thread usage | occupancy waves/SIMD | resource that limits occupancy | log |
| --- | --- | ---: | --- | --- |
| `permanent_bipedal5_kernel` | 128 / 107 / 4000 / 0 / 4+6 | 8 | per-lane vector registers | `permanent_bipedal5.hip.resource.log:1` |
| `f5_byte_control_kernel` | 77 / 78 / 0 / 0 / 0+0 | 12 | per-lane vector registers | `f5_wave_equivalence.hip.resource.log:1` |
| `f5_three_plane_kernel` | 66 / 20 / 0 / 0 / 0+0 | 12 | per-lane vector registers | `f5_wave_equivalence.hip.resource.log:12` |
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
receipt: `gray_update_micro_kernel` carries the highest scalar-register count in
this field's set at 86 `TotalSGPRs` and still reports 16;
`permanent_bipedal3_kernel` carries 1040 scratch bytes per lane and still
reports 16 (`permanent_bipedal3.hip.resource.log:1`). Neither scalar registers
nor private scratch costs a wave slot anywhere in the receipt, and no kernel in
it reports nonzero static LDS except `wave_gf7_lookup_table_kernel<2>` at 24
bytes, which also reports 16.

Two controls bracket the field. The **upper control** is
`permanent_wave_gpu_probe`, an empty kernel with 0 `TotalSGPRs`, 0 `VGPRs`, 0
scratch, and 0 LDS (`probe.hip.resource.log:1`), which reports 16. A kernel that
consumes no registers at all cannot be register-limited, so 16 is the saturation
value of that field on `gfx1030` and therefore the architectural ceiling this
receipt names. The **lower control** is `permanent_bipedal5_kernel` itself at
128 `VGPRs` reporting 8 — this field's own shipped path, not a borrowed one —
and the same figures reappear on `permanent_bipedal7_kernel`
(`permanent_bipedal7.hip.resource.log:1`). The field does fall when per-lane
register demand is heavy, so a report of 16 is a measured absence of register
pressure rather than an unresponsive constant.

**The named limiter holds at each measured order** $n \in \{12, 16, 20, 24, 28\}$
for every row of the table. The evidence is that $n$ reaches these kernels as a
runtime argument rather than a template parameter, so no per-order variation in
resource usage is possible: the mangled names are
`_Z25permanent_bipedal5_kernelPKhiiPy`,
`_ZN12_GLOBAL__N_122f5_byte_control_kernelEPKhiiPj`, and
`_ZN12_GLOBAL__N_121f5_three_plane_kernelEPKhiiPj`, none of which carries a
template parameter, and the compiler emits one resource block per kernel rather
than one per order. The per-thread figures above are therefore the figures at
every measured order, and the resource they name as limiting is the same at
every measured order.

**What the shipped path pays for its byte arithmetic, in wave slots.** The
control mapping holds a $63 \times 63$ byte column table plus a 63-byte
accumulator in per-thread private memory (§7.1), and the compiler's response is
128 VGPRs with 4 000 scratch bytes and ten spills. That costs it half the wave
slots of the SIMD: 8 waves against the ceiling's 16. The lane-owns-interval
prototypes move the column table into per-block shared memory and hold only a
63-byte per-lane row-sum array or a three-word packed accumulator, and the
compiler's response is 77 and 66 VGPRs with zero scratch and zero spills, worth
12 waves each. So the mapping change buys 1.5× the occupancy of the control and
still leaves both prototypes a quarter below the ceiling, with per-lane vector
registers the resource that costs them the remaining four slots in both cases.

Occupancy and device-wide resident work are separate quantities, and the table
above answers only the first. What bounds each path's *device parallelism* at
each measured order is its launch geometry and its batch size, fixed by the
kernel's own contract and by the cell's calibration rather than by its resource
usage. The shipped kernel launches `gridDim.x = M` blocks of `dim3 block(1, 1,
1)`, one matrix per block, with only thread 0 doing work
(`permanent_bipedal5.hip:253-254`, `:266-270`), so its resident wave count
cannot exceed $M$ and each resident wave carries one active lane: 256 or 1024
single-lane waves for the whole device. The prototypes launch `gridDim.x = M`
blocks of `active_lanes_for_order(n)` lanes, which is 32 at every order this
campaign measures (`wave_ryser_mapping.h:29-31`), giving the active-lane counts
tabulated in §4.5: 32 to 3 264 lanes depending on the cell's calibrated batch.
The lane-owns-interval mapping supplies 32 lanes per matrix where the control
supplies one, and the batch calibration is what decides how many matrices are
in flight.

One resource this derivation cannot rule on is dynamic LDS, for the reason in
§7.1: the compiler's static field is silent about the prototypes' $n^2$-byte
and $24n$-byte launch-time tables, so no claim is made that shared memory does
or does not bound the prototype mapping's occupancy. That scopes two rows of
the table. For `f5_byte_control_kernel` and `f5_three_plane_kernel` the named
limiter is the binding one among the resources this receipt can observe, and
the launch-time shared allocation is outside that set; for the other five
kernels the compiler's static LDS field is the complete shared-memory picture,
because each launch passes `sharedMemBytes = 0`
(`permanent_bipedal5.hip:269`, `gray_update_micro.hip:112`, `:126`,
`horizontal_product_micro.hip:196`, `:208-209`) and none of those three
translation units contains a `__shared__` declaration, so nothing is left
unobserved.

## 9. Zero fast path and its exact marginal expectation (REQ-10)

The horizontal-product isolate carries the observed branch frequencies. They
derive from one fixed unconditioned host observation batch per order, addressed
by $(\text{seed\_root}, q, n, \texttt{horizontal\_product\_timed},
\text{seed\_index})$; device timing resamples under the canonical warm-up and
repetition policy and does not alter these counts (preamble of
[`…-q5-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q5-horizontal-product.csv)).
Every row at one order carries the same counts, which `analysis.py` section 9
verifies across all fourteen rows per order, so each order's frequency is one
observation rather than fourteen.

For $q = 5$, sample count $N = 4096$ at each order. Intervals are two-sided
Wilson score intervals at nominal 95 % coverage with $z = 1.959963984540054$,
computed in `analysis.py`; the Wilson form is used because it stays inside
$[0, 1]$ and remains defined at zero successes.

| $n$ | branch | observed | frequency | exact expectation | Wilson 95 % | expectation inside interval |
| ---: | --- | ---: | ---: | ---: | --- | --- |
| 12 | zero fast | 3813 / 4096 | 0.930908203 | $1 - (4/5)^{12} = 0.931280523264$ | [0.922730930, 0.938277974] | yes |
| 12 | nonzero slow | 283 / 4096 | 0.069091797 | $(4/5)^{12} = 0.068719476736$ | [0.061722026, 0.077269070] | yes |
| 16 | zero fast | 3983 / 4096 | 0.972412109 | $1 - (4/5)^{16} = 0.971852502329$ | [0.966936376, 0.977002564] | yes |
| 16 | nonzero slow | 113 / 4096 | 0.027587891 | $(4/5)^{16} = 0.028147497671$ | [0.022997436, 0.033063624] | yes |
| 20 | zero fast | 4041 / 4096 | 0.986572266 | $1 - (4/5)^{20} = 0.988470784954$ | [0.982563839, 0.989668878] | yes |
| 20 | nonzero slow | 55 / 4096 | 0.013427734 | $(4/5)^{20} = 0.011529215046$ | [0.010331122, 0.017436161] | yes |
| 24 | zero fast | 4081 / 4096 | 0.996337891 | $1 - (4/5)^{24} = 0.995277633517$ | [0.993966259, 0.997779408] | yes |
| 24 | nonzero slow | 15 / 4096 | 0.003662109 | $(4/5)^{24} = 0.004722366483$ | [0.002220592, 0.006033741] | yes |
| 28 | zero fast | 4085 / 4096 | 0.997314453 | $1 - (4/5)^{28} = 0.998065718689$ | [0.995197218, 0.998499744] | yes |
| 28 | nonzero slow | 11 / 4096 | 0.002685547 | $(4/5)^{28} = 0.001934281311$ | [0.001500256, 0.004802782] | yes |

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
pooled with it; the three backends' rows at one order likewise resample the
same addresses and are not three replicates. They are reported because they
bound the same quantity three orders of magnitude more tightly. `gpu_hip` rows
shown; `analysis.py` section 9 prints all fifteen.

| $n$ | zero-fast ops | nonzero-slow ops | total | slow frequency | exact expectation | Wilson 95 % | expectation inside interval | $z$ |
| ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | ---: |
| 12 | 7 575 358 | 559 298 | 8 134 656 | 0.068754966 | 0.068719477 | [0.068581285, 0.068929055] | yes | $+0.40$ |
| 16 | 7 658 522 | 222 182 | 7 880 704 | 0.028193166 | 0.028147498 | [0.028077831, 0.028308962] | yes | $+0.78$ |
| 20 | 7 486 299 | 87 205 | 7 573 504 | 0.011514485 | 0.011529215 | [0.011438751, 0.011590715] | yes | $-0.38$ |
| 24 | 7 358 211 | 35 069 | 7 393 280 | 0.004743362 | 0.004722366 | [0.004694091, 0.004793146] | yes | $+0.83$ |
| 28 | 7 141 616 | 14 096 | 7 155 712 | 0.001969895 | 0.001934281 | [0.001937674, 0.002002651] | **no** | $+2.17$ |

Each total equals `reps × samples_per_rep` exactly, which `analysis.py` asserts.

**At $n = 28$ the exact marginal falls outside the interval, and that is
recorded rather than smoothed over.** The observation counts 14 096
nonzero-slow operations against an expectation of 13 841.2, a $+2.17\sigma$
excess, and the Wilson lower bound 0.001937674 sits 0.0000034 above the exact
expectation 0.001934281. All three backends reproduce it because they resample
the same addresses: $z = +2.17$, $+2.17$, and $+2.15$. This is one order of the
five, under a nominal 95 % procedure applied without multiplicity adjustment, so
a single miss is within what the procedure produces by construction; the four
smaller orders sit at $|z| \le 0.83$ and the 4096-sample observation at $n = 28$
covers comfortably. It is recorded because it is the one cell in this campaign
where an observed branch frequency and its exact marginal are not consistent at
the stated coverage, and because a reader recomputing the intervals would find
it.

$\mathbb{F}_5$ sits between the other two fields in zero fast-path share at
every order, because $1 - ((q-1)/q)^n$ decreases in $q$ at fixed $n$:

| $q$ | zero fast at $n = 12$ | zero fast at $n = 28$ |
| ---: | ---: | ---: |
| 3 | 0.992292653 | 0.999988266 |
| 5 | 0.931280523 | 0.998065719 |
| 7 | 0.842732666 | 0.986649735 |

This run's data follows that ordering, and it is what decides which branch this
field's circuits can time. The slow branch is reached by 283 of 4096 samples at
$n = 12$ and still by 11 at $n = 28$, so unlike $\mathbb{F}_3$ — where the slow
branch has too few timed operations to survive the barrier subtraction at the
large orders — this field's slow branch carries 14 096 to 559 298 timed
operations per cell and yields a duration at every order on both byte-circuit
rows (§6.2). The cost of the complete reduction is the quantity this field's
isolate actually exposes, and it is the quantity on which the three-plane
circuit beats the byte one.

### 9.1 Permanent-zero fraction observed in passing

Distinct from the branch frequencies above, and reported because the `zeros`
column of the grid records it: the fraction of sampled matrices whose permanent
is $0 \bmod 5$. These are by-products of the timing protocol with no
preregistered $N$; the counts are whatever each cell's repetition policy
required. Pooling across cells at one order pools independent samples, because
each cell draws from its own reserved index block (§4.2).

| $n$ | zeros | matrices | fraction | Wilson 95 % |
| ---: | ---: | ---: | ---: | --- |
| 12 | 351 715 | 1 760 184 | 0.199817 | [0.199227, 0.200409] |
| 16 | 76 090 | 381 680 | 0.199355 | [0.198091, 0.200626] |
| 20 | 8 965 | 44 792 | 0.200147 | [0.196468, 0.203878] |
| 24 | 303 | 1 576 | 0.192259 | [0.173561, 0.212454] |
| 28 | 6 | 20 | 0.300000 | [0.145477, 0.518973] |

The $n = 28$ row pools 20 matrices, because the only measured cells at that
order run 5, 5, and 10 matrices; its interval spans a factor of 3.6 and it
supports nothing on its own.

## 10. Execution mapping: control and lane-owns-interval (REQ-04)

The control mapping is `permanent_bipedal5_kernel`, which maps one matrix to
one block of one working thread
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal5.hip:250-254`,
`:266-270`), which is the current one-thread-per-matrix execution mapping. It
is attempted at all five orders and `measured` at $n \in \{12, 16, 20\}$ at both
batch sizes $\{256, 1024\}$, with the throughput, phase, and resource figures of
§4 and §7; its four cells at $n \in \{24, 28\}$ are censored (§5).

The lane-owns-interval mapping is `f5_byte_control_kernel` and
`f5_three_plane_kernel`, each launching one block per matrix with
`active_lanes_for_order(n)` lanes and a dynamic shared column table
(`dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip:727-737`), each
lane owning a balanced Gray interval (`:241-243`,
`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:33-40`). Both
specializations are equivalence-confirmed against the CPU oracle at every grid
order in the same run (§3). `f5-three-plane` is `measured` in all five of its
grid cells and `f5-byte-control` in four of five.

Each mapping's batch size is carried in its own column beside the throughput it
produced. The control's are the grid's two fixed sizes; the prototypes' are what
each cell's own probe calibrated.

| $n$ | control $M$ | control rate | `f5-byte-control` $M$ | rate | `f5-three-plane` $M$ | rate | three-plane / control |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 1024 | 22 663.0579 | 45 | 42 476.2230 | 44 | 207 146.4078 | 9.1403 |
| 16 | 1024 | 1 224.3304 | 102 | 5 252.2160 | 45 | 66 125.0000 | 54.0091 |
| 20 | 1024 | 63.9389 | 6 | 17.2007 | 41 | 7 415.9777 | 115.9854 |
| 24 | censored | — | 1 | 0.1560 | 17 | 211.9399 | — |
| 28 | censored | — | censored | — | 2 | 1.5777 | — |

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
prototype cell from its own probe, producing $M \in \{1, 2, 6, 17, 41, 44, 45,
102\}$; §4.5 quantifies what that difference is worth, in active lanes per
repetition and in the probe cost that sets each batch. No prototype cell in this
campaign runs at 256 or at 1024, so the ratios above compare cells that differ
in device parallelism as well as in mapping — at $n = 20$ the three-plane cell
runs 1 312 active lanes against the control's 1 024, and at $n = 16$ it runs
1 440 against 1 024 — and the size of the mapping's own contribution at any one
order is not separable from them. The ordering on the order axis is what these
ratios support.

The comparison that this campaign does support at matched geometry is the
resource comparison of §7: control at 107 SGPR / 128 VGPR / 4000 scratch bytes
per lane / 0 shared bytes per block / 10 spills, against prototypes at 20–78
SGPR / 66–77 VGPR / 0 scratch / $n^2$ or $24n$ dynamic shared bytes per block /
0 spills. That is a real difference in where the column table lives, measured on
both sides, and it is where the study's occupancy hypothesis stands after this
run for this field: **the control mapping is register-limited and spilling at
half the wave-slot ceiling, the lane-owns-interval mapping is register-limited
at three quarters of it, and neither is limited by anything else the compiler
reports** (§8). The prediction that shared-memory pressure grows with matrix
order remains untested, because the compiler's static field cannot see the
dynamic allocation and no prototype cell in this run is large enough for that
pressure to bind.

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
20-pair comparison.

**Batch rayon is confirmed roughly $3\times$ ahead of the current GPU path at
every shared order, and a prototype overturns the conclusion that follows from
it.** The prior run measures `cpu_rayon_batch_scalar` over the better `gpu_hip`
configuration at 3.1160×, 3.4020×, and 3.3435× at $n = 12, 16, 20$, the three
orders where both are measured; this campaign measures 2.9304×, 3.1175×, and
3.0899× on the same pairs. The direction, the magnitude, and the fact that it
holds at every shared order all reproduce. What does not survive is the reading
that the CPU therefore owns this field: `f5-three-plane` leads every measured
cell at every one of the five orders, by 3.1191× over the best CPU path at
$n = 12$ rising to 37.5365× at $n = 20$, and it is measured at the two orders
where the shipped GPU path is censored.

**The four censored GPU cells at $n \in \{24, 28\}$ are confirmed, cell for
cell.** The prior run censors `gpu_hip` at $M \in \{256, 1024\}$ and
$n \in \{24, 28\}$ with projections 1.686983, 3.329136, 0.090374, and 0.178347;
this run censors exactly the same four cells with projections 1.693719,
3.330152, 0.090735, and 0.178401, each within 0.4 % of the prior figure and each
projected from the same reference order $n = 20$. The censoring policy that
keeps an unattempted cell from being read as a measured one behaves identically
across the two runs.

**This run's own $q = 5$ GPU chain confirms the prior study's measured
projection bias.** The study measures the $q = 5$ GPU projection landing low by
13.2 % and 4.7 % at $12 \rightarrow 16$ and 11.1 % and 4.3 % at
$16 \rightarrow 20$ (feasibility-study.md:499-502). This campaign measures
$-17.5\%$ and $-5.2\%$ at $M = 256$ and $-13.2\%$ and $-4.3\%$ at $M = 1024$ —
the $M = 1024$ pair matching to the printed precision, and the $M = 256$ pair
larger by 4.3 and 0.5 percentage points. The direction, the shrinking with $n$,
and the size at $M = 1024$ all reproduce.

**Run-to-run agreement, and one systematic shift inside it.** The prior CSV
carries 21 measured $q = 5$ cells and this run measures 20 of them — every one
except $n = 28$, `cpu_rayon_batch_scalar`, censored here (§5) — so the
comparison is over 20 backend/order pairs. The median absolute disagreement is
2.36 % and the largest disagreement of any pair is 9.10 %, at $n = 24$,
`cpu_rayon_batch_scalar`. Inside that, the sign pattern is not uniform:

| kernel the path forces | pairs | negative | range | mean |
| --- | ---: | ---: | --- | ---: |
| packed `permanent_bipedal5` (`cpu_scalar`, `cpu_rayon_batch_scalar`) | 9 | 9 | $[-9.10\%, -1.52\%]$ | $-6.34\%$ |
| generic `permanent_ryser` (`cpu_ryser_generic`) | 5 | 3 | $[-0.79\%, +0.24\%]$ | $-0.38\%$ |
| shipped `gpu_hip` | 6 | 3 | $[-5.12\%, +0.40\%]$ | $-1.19\%$ |

**Every one of the nine pairs on the packed `permanent_bipedal5` path is slower
in this run**, by 1.52 % to 9.10 %, while the generic Ryser path moves by under
0.8 % in either direction at all five orders and the GPU path splits three and
three. Nine same-sign draws is a $p = 0.004$ outcome under a two-sided sign test
with no directional prior, so it is a systematic shift rather than noise. This
receipt does not attribute it: the two runs differ in host kernel and in Rust
toolchain, the packed kernel is the one whose codegen a toolchain change would
move and the generic one is not, but nothing in either artifact isolates the
cause and no experiment here separates them. What it does record is the
consequence, because it is load-bearing: the best applicable in-tree CPU
baseline is 7.55 % to 9.10 % slower here than in the prior run at
$n \in \{16, 20, 24\}$, and every prototype-over-CPU ratio in §4.2 is larger by
that factor than it would be against the prior run's baseline. At $n = 20$ the
ratio is 37.5365 against this run's baseline and would be 34.7008 against the
prior run's.

### 11.1 One prior cell published as measured below the protocol's own minimum

Reading the prior rates from the CSV rather than from the rendered table makes
the two comparable, so the comparison was made. It surfaces one cell that the
prior artifact publishes in a state its own preamble's protocol forbids.

| $q$, $n$, path | prior `outcome` | prior `reps` | prior `total_s` | prior published rate | this run |
| --- | --- | ---: | ---: | ---: | --- |
| 5, 28, `cpu_rayon_batch_scalar` | `measured` | 1 | 148.177477 | 0.6479 | `censored`, `reps` 1, 163.812679 s |

The prior CSV's `protocol:` preamble line states `warmup >= 3 s, then >= 5 reps
and >= 5 s timed, cap 120 s per cell`, and this cell carries one repetition of
148 s. It is one of exactly two cells in the whole prior file that are
`measured` with fewer than five repetitions — the other is $q = 3$, $n = 28$,
`gpu_hip` at $M = 1024$, three repetitions, published at 19.2675 — and both are
cells this campaign's runs censor. `analysis.py` section 11 enumerates both from
the prior CSV directly.

This matters to two figures here rather than being a curiosity. It is why the
prior study's table shows a best-CPU rate at $q = 5$, $n = 28$ where this run
shows none, and therefore why the best applicable in-tree CPU path at that order
is `cpu_scalar` at 0.0454 here against batch rayon at 0.6479 there — a 14.3×
difference in the denominator of the $n = 28$ ratio. It is recorded because a
receipt that publishes a rate its own stated stopping rule would have censored
is the kind of drift this campaign's figures are supposed to make visible, per
`@/inv/falsification-preserved`.

The prior figure is nevertheless usable as one check, stated with its
provenance: 0.6479 from one repetition sits 12.5 % above this run's projection
of 0.5761 for the same cell, and that chain's own measured projection bias in
this run is $-3.4\%$ to $-18.4\%$ (§5). The direction and rough size agree. No
ordering at $n = 28$ between batch rayon and any other path is asserted from
either number.

## 12. The declared operating point (REQ-16)

REQ-16 is aspirational and asks a specific question: at the declared
scientifically relevant operating point for this field, does the best-performing
prototype exceed the best applicable in-tree CPU throughput by at least
$1.5\times$, with its launch duration inside the documented safe bound.

**The declared operating point for this field is $n = 24$, and two committed
documents say so independently.** The campaign protocol fixes the frozen cell
universe as "*every integer $n$ from $4$ through the processor-feasible frontier
measured in the envelope receipt: $n=28$ for $q=3$, $n=24$ for $q=5$, and
$n=20$ for $q=7$*"
([`dev/simulation_results/permanent-zero-fraction/protocol.md`](../../simulation_results/permanent-zero-fraction/protocol.md):51-54),
and the same document's backend-selection section fixes exactly four
premeasurement configurations, of which this field's is "*$(5,24)$ on batch
rayon*" (`:253-255`). The feasibility study reaches $n = 24$ for this field
independently, as the feasible frontier at a standard error of $10^{-3}$ under
the 12 h per-cell budget
([`../b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md):780-782).

At that point:

| quantity | value |
| --- | ---: |
| best-performing prototype | `f5-three-plane`, $M = 17$ |
| its composite throughput | 211.9399 matrices/s |
| best applicable in-tree CPU path | `cpu_rayon_batch_scalar`, $M = 96$ |
| its composite throughput | 10.7539 matrices/s |
| **measured factor** | **19.7082×** |
| device launch duration | 4.8571 µs/launch (`device_submission_to_kernel_s` 0.000306 s over 63 launches) |
| kernel span per launch | 0.0800 s |
| per-launch work $M \cdot 2^n$ | $2.852 \times 10^8$ |

**The $1.5\times$ target is exceeded by a factor of 13.1 at the declared
operating point**, and the same holds at every other order this campaign
measures: the prototype-over-CPU factor is 3.1191, 17.3246, 37.5365, 19.7082,
and 34.7511 at $n = 12$ through $28$. The shipped GPU path meets the target at
no order, at 0.3412, 0.3208, and 0.3236 where it is measured.

**The safe launch-duration bound is not yet a committed figure of this study,
and this receipt does not invent one.** Deriving a watchdog-safe per-launch work
bound per field from this study's own measurements is the deliverable of a
separate issue that has not run; the only committed numeric bound is the
archived prior calibration in
[`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`](../../archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md)
§2.5, which places a hang boundary at "*≈190–200 s*" per launch, reports bounded
sub-batches holding every launch at "*≈10–117 s*", and sets a $q = 5$ work
budget of $\text{sub\_batch} \cdot 2^n \le 1.3 \times 10^9$. The campaign plan
directs that this figure be treated "*as a prior*" rather than as an established
device property
([`dev/active/0de41c82/plan.md`](../../active/0de41c82/plan.md):13), because the
one observed hang's attribution to a watchdog timeout was explicitly retracted
([`../b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md):264-268).
It is cited here on those terms.

Measured against that prior, this campaign's launches sit inside it with margin,
and `analysis.py` section 12 tabulates every device row:

- The declared operating point's launch carries $2.852 \times 10^8$ units of
  $M \cdot 2^n$ work, **0.2194 of the archived $q = 5$ budget**, in a kernel
  span of 0.0800 s, **0.0004 of the archived 190 s boundary**.
- The largest per-launch work anywhere in this campaign is $1.074 \times 10^9$
  (`gpu_hip`, $n = 20$, $M = 1024$), 0.8260 of the budget.
- The longest kernel span per launch anywhere is 115.4529 s (`f5-byte-control`,
  $n = 28$, $M = 1$), 0.6076 of the boundary, inside the "≈10–117 s" band the
  archived calibration reports for its bounded sub-batches.
- No cell in this run failed with a device fault; every step reports
  `status=completed exit=0`.

That last point is a one-run observation on this host and not a bound. This
campaign supplies 16 device-backed cells' worth of evidence toward the bound the
study still owes, in the field and order region where the archived calibration
observed its original hang. That hang was a single 2048-matrix $\mathbb{F}_5$
$n = 20$ launch at "≈200 s+"; its $M \cdot 2^n$ of $2.147 \times 10^9$ is 1.65
times the archived $1.3 \times 10^9$ budget and exactly twice this campaign's
largest launch, which is the same $(q, n)$ cell at $M = 1024$ and completed
without fault in 16.0068 s.

**Verdict: the throughput half of REQ-16 is met with a large margin at the
declared operating point; the launch-duration half is met against the only
committed bound, which is an archived prior rather than a bound this study has
yet derived.**

## 13. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §2, §3, §4.2 | **Satisfied.** The current GPU path at both configured batch sizes, both planned $\mathbb{F}_5$ prototype paths, and all three in-tree CPU paths this field has are compared over $\mathbb{F}_5$ at five orders, with the best applicable CPU path identified per order from this run's own data. Every timing cell draws from one identical sampler on preregistered, structurally disjoint per-cell stream addresses (`main.rs:54`, `:371-391`, `:593-598`); the equivalence comparison runs on one literally identical matrix corpus per $(q, n)$ (`equivalence.rs:139`, `:186-192`). |
| REQ-02 | §1, §4.1 | **Satisfied.** Every listed item maps to a named CSV column or provenance line, tabulated in §4.1. |
| REQ-03 | §4, §6 | **Satisfied.** The end-to-end Ryser loop is isolated for all six executing backends at all five orders. Gray update, at all five orders: durations for both of this field's circuits, the packed host one (`cpu_scalar`) and the byte device one (`gpu_hip`), with no censored row; both prototypes recorded with the structural reason that their update runs fused inside their full-permanent kernel with no device source isolating it; the remaining CPU backends recorded as having no distinct evaluator, under the preamble's rule that a row reports a span only from a circuit distinct to the backend it names. Horizontal product, at all five orders: durations on both branches for the F5Byte circuit under both names that select it, a nonzero-branch duration for the F5ThreePlane circuit at four orders, its zero branch censored with its censoring reason at all five and both branches censored at $n = 12$, and every CPU and out-of-field row recorded with its structural reason. No absence is left without a reason of the kind the criterion names. |
| REQ-04 | §10, §4.5 | **Satisfied, with its coverage stated exactly.** Both mappings are measured at the three matching orders $n \in \{12, 16, 20\}$; at $n \in \{24, 28\}$ the control has no measured cell because all four of its cells are censored (§5), and §10 states that rather than deriving a ratio from a projection. The control's fixed $M \in \{256, 1024\}$ and each prototype cell's probe-calibrated $M \in \{1, 2, 6, 17, 41, 44, 45, 102\}$ are recorded beside their throughputs in the §10 mapping table and in every throughput table of §4. §4.5 quantifies the confound: the batch size *is* the prototype's device parallelism, tabulated as 32–3 264 active lanes against the control's 256 or 1024, with the probe cost that sets each batch, the reconstruction of every batch from that probe, and the rayon floor that overrides it. The mappings are therefore comparable on the order axis over the three orders both occupy. |
| REQ-05 | §4.3 | **Satisfied.** `kernel_device_s` is its own device-event column on all four device paths; allocation, copy, and host serialisation are outside it, and the residual is derivable per cell. |
| REQ-06 | §4.3 | **Satisfied.** `h2d_device_s`, `d2h_device_s`, `host_submission_s`, and `device_submission_to_kernel_s` are four separate columns, and per-launch costs follow from them and `reps` without a second run. §4.3 states, with the `host_submission_s`/`eval_s` ratios as evidence, that the host-clock column measures an asynchronous submission on `gpu_hip` rows and a synchronous submit-and-complete on prototype rows, so that it is not misread as a prototype launch overhead. |
| REQ-07 | §7, §7.1, §7.2 | **Satisfied.** Registers per thread, private scratch per thread, static LDS per block, and both spill counts are reported for all seven measured kernels in §7's table, each with its resource-log line. Predictions are paired with those measurements wherever a committed design states one: the two prototype kernels against the README's $n^2$ and $24n$ per-block shared tables, and every non-prototype kernel against the exact 0 bytes/block its own launch and translation unit state. §7.2 gives the coverage per kernel and per quantity. No committed design states a per-lane register budget for any of the seven, which is recorded as such with the scope of the F_3-only statement that might be mistaken for one, and no post-hoc figure is supplied — a figure back-derived from the measurement could not afterwards be diverged from it. The README's committed prior resource table for the two prototype kernels is reported separately in §7.1 as an exact replication and explicitly not as prediction coverage. The one blind spot is recorded rather than closed by assumption: the compiler's static-LDS field cannot observe the launch-time tables, so that prediction is neither confirmed nor refuted here and no occupancy conclusion in §8 rests on reading its 0 as an absence. |
| REQ-08 | §8 | **Satisfied.** §8's table names the occupancy-limiting resource for each of the seven measured $\mathbb{F}_5$ kernels: per-lane vector registers for the three full-permanent kernels, which report 8, 12, and 12 waves/SIMD, and the `gfx1030` architectural wave-slot ceiling of 16 for the four isolate kernels. Each row carries its per-thread usage, its occupancy, and its resource-log line. The derivation rests on stated, checkable evidence: `VGPRs` alone reproduces all 26 occupancy entries in the receipt under a 1 024-register, 16-unit allocation capped at 16, which `analysis.py` section 8 asserts; scalar registers and private scratch are shown not to bind by counterexample in the same receipt (86 SGPRs at occupancy 16, 1040 scratch bytes at occupancy 16); the empty `permanent_wave_gpu_probe` at zero usage reporting 16 establishes the ceiling; and this field's own `permanent_bipedal5_kernel` at 128 `VGPRs` reporting 8 shows the field falls under pressure rather than being constant. The naming holds at each measured order $n \in \{12, 16, 20, 24, 28\}$, with the evidence stated: $n$ is a runtime argument and not a template parameter on any of the three, so the compiler emits one resource block per kernel and no per-order variation is possible. The dynamic-LDS blind spot is stated rather than assumed away, and §8 scopes it to the two prototype rows it actually qualifies. |
| REQ-09 | §7.1, §7.2, §5, §14 | **Satisfied.** No measurement in this campaign contradicts a numeric register or shared-memory prediction that a committed design states for this field, and §7.1 states why rather than leaving it implicit: the only committed budget statements are the $n^2$ and $24n$ dynamic shared tables, which the compiler's static field cannot observe, and the exact 0 B/block for the five kernels whose launches request none, which it confirms. No prediction is silently restated: the F_3-scoped nine-register lower bound is quoted with its scope and not applied here, the README's prior F_5 resource table is reported as a replication with all twelve figures matching, and the one qualitative committed claim about register residency is quoted and confirmed against zero scratch and zero spills on both prototypes. The contradictions this run does produce are of other committed statements and are carried with them — the projection-accuracy statement against this file's own prototype chains (§5), the prior artifact's sub-minimum published cell (§11.1), the packed-kernel run-to-run shift (§11), and the $n = 28$ branch-frequency interval (§9) — collected in §14. |
| REQ-10 | §9 | **Satisfied.** Both frequencies, both exact expectations $1 - (4/5)^n$ and $(4/5)^n$, their complement relation, the sample count 4096, and Wilson 95 % intervals are reported at all five measured orders, with the interval method fixed deterministically at $z = 1.959963984540054$. A second, larger branch observation from the same file's timed-operation counts is reported beside it with its own intervals, its $z$ statistics, and its dependence on the same purpose stream stated, including the one order where the exact marginal falls outside the interval. |
| REQ-11 | §3 | **Satisfied.** `cpu_scalar` is the oracle and the other five executing paths, both prototypes included, are re-confirmed identical against it on the campaign host, in the same run, before any timing cell, at every order the grid times and at $n = 8$ below them. The $n = 24$ and $n = 28$ cells compare 32 and 2 matrices against 512 at the smaller orders, with the $n = 28$ figure an explicit per-field override; §3 states this rather than presenting it as an equal gate. |
| REQ-12 | §5 | **Satisfied.** All six censored cells state their censoring reason and the rate they were projected from, and carry `NaN` for both throughput columns; §4.3 withholds their kernel-only rates for the same reason. The two censoring kinds — not attempted before running, and capped after running — are distinguished, and the projection's measured bias is given per chain so that each censored cell's projection can be read with the bias of the chain it came from, including the one chain where it is not conservative. |
| REQ-13 | §2 | **Satisfied; vacuous for this field's device candidates, and the reason matters.** No planned $\mathbb{F}_5$ candidate fails to execute on the target device: both prototypes compile clean for `gfx1030`, both appear in the resource receipt, both are equivalence-identical at every grid order, and both are `measured` in every grid cell they were attempted in. There is therefore no compile, correctness, or resource falsification to cite, and none is invented. The five candidates excluded by field or by the structural absence of a full-permanent batch kernel carry their reasons verbatim in §2. Separately and distinctly, three in-tree CPU paths cannot execute for this field at all; they are named with the library-capability absence that excludes them, quoted verbatim, confirmed independently in the equivalence file and in the feasibility study, and §2 states that this is a host-tree capability absence rather than a device falsification. |
| REQ-14 | §4.4 | **Satisfied.** The best-performing prototype is `f5-three-plane`; its best operating point is $n = 20$, $M = 41$, where it measures 7 415.9777 matrices/s against `cpu_rayon_batch_scalar` at 197.5673, a ratio of **37.5365×**, with a launch duration of **4.8602 µs per launch** on the device clock (`device_submission_to_kernel_s` 0.004379 s over 901 launches). §4.5 records that this operating point's batch is not matched to the control's. |
| REQ-15 | §1 | **Satisfied.** Three exact commands for this field plus the shared equivalence command are committed with their revision, toolchain, and binary hashes; `analysis.py` regenerates every table here from the committed artifacts with one command; the run executes on the prepared benchmark host under the repository's full-host benchmark mutex, with a pristine-worktree refusal and a binary-hash verification enforced both before and after the lock is acquired, and with the machine warm-up held by the first grid under the same lock. |
| REQ-16 | §12 | **Aspirational; throughput met, bound met against a prior.** At the declared operating point $n = 24$, `f5-three-plane` measures 211.9399 matrices/s against `cpu_rayon_batch_scalar` at 10.7539, a factor of **19.7082×** against a $1.5\times$ target, with a launch duration of 4.8571 µs on the device clock and a kernel span of 0.0800 s per launch. No safe launch-duration bound derived by this study is committed yet; measured against the only committed figure — the archived ≈190–200 s per-launch calibration and its $1.3 \times 10^9$ $q = 5$ work budget, cited as a prior rather than as an established device property — this operating point sits at 0.0004 of the span boundary and 0.2194 of the work budget. |

## 14. What this campaign does not establish

Collected so a reader does not have to reassemble it from the sections above.

1. **The two mappings are compared at three orders, not five.** All four
   control cells at $n \in \{24, 28\}$ are censored before running, so §10's
   mapping ratios exist only at $n \in \{12, 16, 20\}$ and nothing here settles
   the ordering between the two mappings at this field's two largest orders.
2. **No prototype cell is measured at the control's batch sizes.** Every
   prototype cell sizes itself from a one-matrix probe, so the mapping
   comparison mixes the mapping effect with a device-parallelism effect that
   ranges over a factor of 102 across the prototype cells. Separating the two
   needs a run with the prototype batch sizes pinned to the control's.
3. **The $n = 20$ headline ratio is not decomposed.** `f5-three-plane` at
   37.5365× the best CPU path runs 1 312 lanes against the shipped path's 1 024.
   No cell in this run isolates how much of that ratio is the three-plane
   circuit and how much is the batch, and the byte control's collapse to 6
   matrices at the same order (§4.5) shows how large the batch term can get.
4. **The $n^2$-byte and $24n$-byte shared column tables are unmeasured.** The
   compiler reports static LDS only, so this field's committed shared-memory
   prediction is neither confirmed nor refuted here, and no prototype cell in
   this run is large enough for shared-memory pressure to bind.
5. **No per-lane register budget is predicted for any of the seven measured
   kernels.** The prediction-beside-measurement pairing is one-sided on
   registers throughout this field (§7.2). Their measured figures stand on
   their own and there is nothing to diverge from, so this campaign cannot test
   a register hypothesis for any kernel it measures — which is a weaker position
   than $\mathbb{F}_3$'s, where two of seven had one.
6. **The 32-byte gap between declared and reported scratch is unexplained.**
   `permanent_bipedal5_kernel` declares 4 032 bytes of runtime-indexed private
   arrays and the compiler reports 4 000 bytes/lane. No committed source or
   receipt accounts for the difference, and none is asserted here.
7. **The $n = 28$ equivalence cell compares two matrices.** Two against 512 at
   the four smallest orders, so this field's largest order rests on a much
   weaker correctness gate than the rest, and $n = 24$'s 32-matrix cell is
   weaker than the rest too.
8. **The $n = 28$ ordering against batch rayon is open.** That cell is censored
   here and published in the prior run from a single repetition its own protocol
   would censor (§11.1), so no ordering between `f5-three-plane` and batch rayon
   at $n = 28$ is asserted from either artifact. The 34.7511× figure in §4.2 is
   against `cpu_scalar`, the fastest *measured* CPU cell at that order, and it
   would be 2.7386× against the censored rayon cell's projection if that
   projection held.
9. **The safe launch-duration bound is not this study's own.** §12 measures
   against an archived prior whose watchdog attribution was retracted; deriving
   the study's own bound is a separate deliverable and this campaign contributes
   16 device-backed cells toward it rather than closing it.
10. **The permanent-zero fraction at $n = 28$ pools 20 matrices.** Its interval
    spans a factor of 3.6 and supports nothing on its own (§9.1).

Four observations in this run contradict a statement outside its own numbers and
are recorded rather than restated, per `@/inv/falsification-preserved`:

- **The grid preamble's projection-accuracy statement fails on a chain that owns
  a censored cell.** The preamble scopes its "*lands LOW at every step*" claim to
  the $q = 3$ GPU chain and calls the step to other fields an extrapolation. On
  this field's own GPU chains the claim holds ($-17.5\%$ to $-4.3\%$), which
  covers the four censored `gpu_hip` cells. On the `f5-byte-control` chain it
  fails, landing high by 474.3 % at $20 \rightarrow 24$, and that chain owns the
  censored $n = 28$ cell whose projection is therefore not conservative (§5).
  The $\mathbb{F}_3$ campaign recorded this possibility before any such cell
  existed; this campaign has one.
- **The prior grid publishes a $q = 5$, $n = 28$ rate its own stated stopping
  rule forbids**, one repetition of 148 s against a five-repetition minimum and
  a 120 s cap, and that figure is the best-CPU baseline the prior study's table
  shows at that order (§11.1). This run censors the same cell.
- **The packed `permanent_bipedal5` path is uniformly slower than in the prior
  run** — all nine comparable pairs negative, mean $-6.34\%$ — while the generic
  Ryser path on the same host moves by under 0.8 % in either direction. The
  cause is not isolated by either artifact and is not asserted; the effect on
  this campaign's ratios is stated (§11).
- **The exact marginal branch expectation falls outside the Wilson interval of
  the $n = 28$ timed-operation observation**, at $z = +2.17$, on all three
  backends that resample the same addresses (§9). It is one order of five under
  a nominal 95 % procedure with no multiplicity adjustment, and the four other
  orders sit at $|z| \le 0.83$.
