# $\mathbb{F}_3$ preregistered receipt campaign — committed receipts

Campaign run `20260814T230032Z-2085453`, field $q = 3$, grid execution id
`3002`. Every figure below is derived from the raw artifacts committed beside
this file; [`analysis.py`](analysis.py) in this directory regenerates all of
them from those artifacts and prints them under the section headings used here.

```sh
python3 dev/studies/047b62ed/analysis.py
```

| Artifact | Role |
| --- | --- |
| [`…-q3-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q3-grid.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q3-grid.log) | End-to-end Ryser timing grid, 75 cells |
| [`…-q3-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q3-gray-update.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q3-gray-update.log) | Dependency-chained Gray-update isolate, five orders |
| [`…-q3-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q3-horizontal-product.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-q3-horizontal-product.log) | Horizontal-product isolate and zero/nonzero branch frequencies, five orders |
| [`…-shared-equivalence.csv`](permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv) / [`.log`](permanent-campaign-20260814T230032Z-2085453-shared-equivalence.log) | Backend-equivalence gate, all fields in one global run |
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
`edb03650bbbfb47064a3a2b5e021ac055394812a4362c577fdad07cdbd997bce`. Sources:
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

The four commands that produced the four CSVs are recorded verbatim under
`exact_commands_executed:` in
[`…provenance.txt`](permanent-campaign-20260814T230032Z-2085453.provenance.txt)
and repeated in the `# invocation:` line of each CSV and the `# command:` line
of each log. All four steps report `status=completed exit=0` in
[`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt).

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
landing during the lock wait cannot reach the campaign unchecked. The grid
preamble additionally records a 90 s full-rayon machine warm-up before the
first timed cell.

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

Reproduction requires the exact commands as recorded, including `--only q=3`
and `--execution-id 3002`. A cell's first stream index is
`execution_id * 22 500 000 + order_index * 100 000 + 1`
(`dev/research/permanent-sampling-feas/src/main.rs:371-391`, `:54`, `:81`), and
`order_index` is the cell's position after the spec list is shuffled with
`SEED_ROOT` and stably sorted by ascending $n$
(`dev/research/permanent-sampling-feas/src/main.rs:593-598`). The shuffle runs
over the filtered list (`:576`), so the same execution id with a different
`--only` filter addresses different matrices. Execution `3002` therefore owns
`67545000001..=67567500000`, which is the block the grid preamble records.

## 2. Candidate roster and what executes (REQ-01, REQ-13)

The grid enumerates the full planned candidate set from the prototype registry
rather than a pre-filtered one; every `Backend::ALL` entry appears at every
order, and the single GPU entry expands to both configured batch sizes
(`dev/research/permanent-sampling-feas/src/main.rs:305-334`, `:50`). The file
holds 75 cells: 15 per order, of which 49 are `measured`, 1 is `censored`, and
25 are `unsupported`. Every non-`measured` cell carries its reason in `note`.

| Path | Class | Grid outcome | Reason recorded in `note` |
| --- | --- | --- | --- |
| `gpu_hip` (`permanent_bipedal3_kernel`) | current GPU path | `measured` at $M \in \{256, 1024\}$ and every order, except $n = 28$, $M = 1024$ | see §5 |
| `wave-gf3` (`wave_gf3_kernel<kHalving>`) | planned $\mathbb{F}_3$ prototype, lane-owns-interval | `measured` at every order | — |
| `fold-gf3` (`wave_gf3_kernel<kZeroMaskSignPopcount>`) | planned $\mathbb{F}_3$ prototype, lane-owns-interval | `measured` at every order | — |
| `cpu_scalar` (`permanent_bipedal3_singleword`) | in-tree CPU | `measured` | — |
| `cpu_avx2` (`permanent_bipedal3_singleword_simd`) | in-tree CPU | `measured` | — |
| `cpu_rayon_batch_scalar` | in-tree CPU | `measured` | — |
| `cpu_rayon_batch_avx2` | in-tree CPU | `measured` | — |
| `cpu_rayon_intra_matrix` | in-tree CPU | `measured` | — |
| `cpu_ryser_generic` | in-tree CPU | `measured` | — |
| `f5-byte-control` | planned $\mathbb{F}_5$ prototype | `unsupported` | `f5-byte-control evaluates F_5 permanents, not F_3` |
| `f5-three-plane` | planned $\mathbb{F}_5$ prototype | `unsupported` | `f5-three-plane evaluates F_5 permanents, not F_3` |
| `f7-lookup-table-control` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-lookup-table-control evaluates F_7 permanents, not F_3` |
| `f7-three-plane-permanent` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-three-plane-permanent evaluates F_7 permanents, not F_3` |
| `f7-three-plane-accumulator` | planned $\mathbb{F}_7$ prototype | `unsupported` | `f7-three-plane-accumulator has no device batch evaluator: hip/f7_three_plane_equivalence.hip holds a single-thread three-plane accumulator conformance probe, not a full-permanent batch kernel; the permanent-shaped use of this arithmetic is the f7-three-plane-permanent candidate` |

The kernel each CPU row forces is recorded in
[`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.2: `permanent_bipedal3` dispatches internally, so the grid never calls it and
instead calls `permanent_bipedal3_singleword` for `cpu_scalar` and
`permanent_bipedal3_singleword_simd` for `cpu_avx2` with an explicit selection.

**Both planned $\mathbb{F}_3$ prototypes execute on the target device and are
measured.** A registered candidate's grid cell evaluates that candidate's own
device kernel over the same batch object the built-in backends receive
(`dev/research/permanent-sampling-feas/src/backend.rs:505-531`), reaching
`wave_gf3_kernel<FoldKind>` through the candidate's prebuilt HIP executable,
which stays resident for the cell so a batch pays the pipe transfer and the
executable's own per-batch device allocation rather than a process start
(`dev/research/permanent_wave_gpu/src/device_batch.rs:9-13`, `:318-331`). The
launch geometry is one block per matrix, `active_lanes_for_order(n)` lanes per
block, and $2n$ packed words of dynamic shared memory
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:597-606`), with
`active_lanes_for_order(n) = 32` for $n \ge 5$
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31`, `:16`).

The five excluded candidates are planned for $\mathbb{F}_5$ and $\mathbb{F}_7$,
not for this field. Four are rejected by field, and the fifth by the structural
absence of a full-permanent batch kernel, in the words quoted in the table
above. **No planned $\mathbb{F}_3$ candidate fails to execute on this device**,
so REQ-13's compile, correctness, or resource falsification has no
$\mathbb{F}_3$ subject in this run, and none is invented; §12 records that
reading.

## 3. Equivalence gate (REQ-11)

The equivalence step is the first step of the pipeline
([`…run-summary.txt`](permanent-campaign-20260814T230032Z-2085453.run-summary.txt);
`timestamp_utc: 2026-08-14T23:00:34Z` in the equivalence CSV against
`2026-08-15T00:35:22Z` in the grid CSV), so it precedes every timing cell of the
same run, on the same host and the same binary hash.

The harness `equivalence` subcommand is global: one invocation covers all three
fields and takes no field filter, and `--execution-id` is parsed by `grid`
alone. The run summary therefore records `execution_id=fixed-streams` for this
step, which is what the step does — it draws from fixed stream addresses of the
form $(\text{seed\_root}, q, n, \texttt{equivalence}, 0)$ recorded in its own
preamble, rather than from a reserved per-execution index block. One global run
gates all three fields; this document reads its $q = 3$ rows.

Protocol: the per-order matrix counts of the preamble's `matrices_per_cell:`
line at $n \in \{8, 12, 16, 20, 24, 28\}$, with the scalar single-word kernel as
the reference. Every backend at a given $(q, n)$ is compared against the *same*
matrices: the batch is built once and reused across the whole backend loop,
registered prototype candidates included
(`dev/research/permanent-sampling-feas/src/equivalence.rs:139`, `:186-192`), and
the preamble states the same contract on its `candidates:` line.

| $n$ | Backends compared against `cpu_scalar` | `matrices` | `mismatches` | `zeros_reference` = `zeros_backend` | `status` |
| ---: | --- | ---: | ---: | ---: | --- |
| 8 | `cpu_avx2`, `cpu_rayon_batch_scalar`, `cpu_rayon_batch_avx2`, `cpu_rayon_intra_matrix`, `gpu_hip`, `cpu_ryser_generic`, `wave-gf3`, `fold-gf3` | 512 | 0 | 169 | `identical` |
| 12 | same eight | 512 | 0 | 165 | `identical` |
| 16 | same eight | 512 | 0 | 157 | `identical` |
| 20 | same eight | 512 | 0 | 162 | `identical` |
| 24 | same eight | 32 | 0 | 13 | `identical` |
| 28 | same eight | 4 | 0 | 0 | `identical` |

All 48 $q = 3$ comparison cells report `mismatches = 0` and
`status = identical`, including both prototypes at every order the grid times,
and the log closes with `all backends agree per matrix`. The whole file carries
102 `identical` rows across the three fields. The five out-of-field prototype
rows report `matrices = 0` with the reasons of §2 and are not equivalence
evidence in either direction.

The equivalence order set covers every grid order, so no timing in §4 rests on
equivalence confirmed only at a smaller order. The counts fall to 32 and 4 at
$n = 24$ and $n = 28$ because the harness sizes each cell against a 240 s
budget from committed per-matrix costs
(`dev/research/permanent-sampling-feas/src/main.rs:64-77`); a four-matrix cell
is a weaker gate than a 512-matrix one, and it is the gate those two orders
actually have.

## 4. End-to-end Ryser timing grid (REQ-01, REQ-02, REQ-05, REQ-06)

### 4.1 Recorded fields

REQ-02 asks for a specific field set. Each is a named column of
[`…-q3-grid.csv`](permanent-campaign-20260814T230032Z-2085453-q3-grid.csv), or a
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
permanents, not F_3`), so those 25 lines hold 40 fields against a 38-column
header, and `note` is not the only column past the split. `analysis.py` splits
them on the `seed_root` prefix rather than on field count. The equivalence CSV
carries the same literal commas on 90 of its rows, but its free text is its last
column, so joining the surplus fields back onto `status` recovers it. The two
isolate CSVs use semicolons inside their free text and need no handling at all.

### 4.2 Composite throughput and the best path per order

Composite matrices per second, the rate the study's envelope is derived from.
Full grid in the CSV; `analysis.py` section 4.2 prints every one of the 49
measured cells.

| $n$ | best overall | rate | best in-tree CPU path | rate | best prototype | rate | best GPU config | rate | prototype / CPU | GPU / CPU |
| ---: | --- | ---: | --- | ---: | --- | ---: | --- | ---: | ---: | ---: |
| 12 | `cpu_rayon_batch_scalar` | 301 317.4209 | `cpu_rayon_batch_scalar` | 301 317.4209 | `fold-gf3` | 228 042.8264 | `gpu_hip` $M{=}1024$ | 214 467.4416 | 0.7568 | 0.7118 |
| 16 | `fold-gf3` | 82 098.9857 | `cpu_rayon_batch_scalar` | 37 066.3694 | `fold-gf3` | 82 098.9857 | `gpu_hip` $M{=}1024$ | 57 627.3613 | 2.2149 | 1.5547 |
| 20 | `fold-gf3` | 69 485.2981 | `cpu_rayon_intra_matrix` | 2 966.4439 | `fold-gf3` | 69 485.2981 | `gpu_hip` $M{=}1024$ | 4 848.4654 | 23.4238 | 1.6344 |
| 24 | `fold-gf3` | 461.3922 | `cpu_rayon_intra_matrix` | 296.4992 | `fold-gf3` | 461.3922 | `gpu_hip` $M{=}1024$ | 312.1116 | 1.5561 | 1.0527 |
| 28 | `cpu_rayon_intra_matrix` | 19.4629 | `cpu_rayon_intra_matrix` | 19.4629 | `fold-gf3` | 3.9630 | `gpu_hip` $M{=}256$ | 8.5448 | 0.2036 | 0.4390 |

The best applicable in-tree CPU path is identified from this run's own data
rather than assumed: batch rayon over the scalar kernel leads the CPU field at
$n \in \{12, 16\}$, and intra-matrix rayon leads it at $n \in \{20, 24, 28\}$.
At $n = 28$ the best GPU configuration is $M = 256$ only because $M = 1024$ is
censored (§5); no rate is substituted for it.

The zero-mask/sign-popcount fold leads the halving control at every order:
`fold-gf3` over `wave-gf3` is 1.0762, 1.1508, 8.0810, 1.9053, and 2.2161 at
$n = 12, 16, 20, 24, 28$. §4.5 records that the $n = 20$ figure is measured at
a batch ten times larger than the control's, and that this run does not separate
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
(`dev/research/permanent-sampling-feas/src/backend.rs:535-536`, `:562-565`,
`dev/research/permanent-sampling-feas/src/protocol.rs:1173`).

Totals per cell, in seconds. `residual` is
`eval_s − kernel_device_s − h2d_device_s − d2h_device_s − device_submission_to_kernel_s`,
the host-side allocation, serialisation, stream wait, and free that the
dispatcher performs around each call.

| $n$ | path | $M$ | outcome | `eval_s` | `kernel_device_s` | `h2d_device_s` | `d2h_device_s` | `host_submission_s` | `device_submission_to_kernel_s` | residual | kernel / eval |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `fold-gf3` | 45 | measured | 2.932008 | 0.535738 | 0.225974 | 0.393908 | 2.185116 | 0.115777 | 1.660611 | 0.1827 |
| 12 | `wave-gf3` | 44 | measured | 3.075496 | 0.827824 | 0.215519 | 0.372656 | 2.374522 | 0.110491 | 1.549006 | 0.2692 |
| 12 | `gpu_hip` | 256 | measured | 3.934052 | 1.118576 | 0.029483 | 0.021332 | 0.007463 | 0.011116 | 2.753545 | 0.2843 |
| 12 | `gpu_hip` | 1024 | measured | 3.188752 | 1.336099 | 0.037500 | 0.009290 | 0.003330 | 0.009950 | 1.795913 | 0.4190 |
| 16 | `fold-gf3` | 44 | measured | 3.805396 | 1.816565 | 0.088651 | 0.149007 | 3.331228 | 0.044009 | 1.707164 | 0.4774 |
| 16 | `wave-gf3` | 45 | measured | 3.965927 | 2.260464 | 0.075938 | 0.128676 | 3.553061 | 0.037850 | 1.462999 | 0.5700 |
| 16 | `gpu_hip` | 256 | measured | 4.620362 | 3.888919 | 0.007660 | 0.004904 | 0.001720 | 0.002547 | 0.716332 | 0.8417 |
| 16 | `gpu_hip` | 1024 | measured | 4.205773 | 3.544462 | 0.005620 | 0.002572 | 0.000888 | 0.001284 | 0.651835 | 0.8428 |
| 20 | `fold-gf3` | 448 | measured | 3.596342 | 2.856751 | 0.013068 | 0.012267 | 2.991072 | 0.003511 | 0.710745 | 0.7943 |
| 20 | `wave-gf3` | 41 | measured | 4.814202 | 4.592984 | 0.012617 | 0.017558 | 4.736818 | 0.005074 | 0.185969 | 0.9540 |
| 20 | `gpu_hip` | 256 | measured | 5.041555 | 4.955491 | 0.000682 | 0.000404 | 0.000144 | 0.000204 | 0.084774 | 0.9829 |
| 20 | `gpu_hip` | 1024 | measured | 4.976589 | 4.894003 | 0.000620 | 0.000225 | 0.000090 | 0.000111 | 0.081630 | 0.9834 |
| 24 | `fold-gf3` | 22 | measured | 4.993827 | 4.969235 | 0.001040 | 0.001803 | 4.987881 | 0.000512 | 0.021237 | 0.9951 |
| 24 | `wave-gf3` | 17 | measured | 5.047464 | 5.037361 | 0.000688 | 0.001326 | 5.044296 | 0.000350 | 0.007739 | 0.9980 |
| 24 | `gpu_hip` | 256 | measured | 9.326022 | 9.312776 | 0.000096 | 0.000049 | 0.000032 | 0.000024 | 0.013077 | 0.9986 |
| 24 | `gpu_hip` | 1024 | measured | 16.377584 | 16.352739 | 0.000212 | 0.000050 | 0.000033 | 0.000024 | 0.024559 | 0.9985 |
| 28 | `fold-gf3` | 3 | measured | 5.298792 | 5.296738 | 0.000068 | 0.000155 | 5.298714 | 0.000034 | 0.001797 | 0.9996 |
| 28 | `wave-gf3` | 2 | measured | 5.591736 | 5.589927 | 0.000050 | 0.000112 | 5.591694 | 0.000024 | 0.001623 | 0.9997 |
| 28 | `gpu_hip` | 256 | measured | 149.789722 | 149.773193 | 0.000109 | 0.000049 | 0.000042 | 0.000024 | 0.016347 | 0.9999 |
| 28 | `gpu_hip` | 1024 | censored | 159.251788 | 159.232563 | 0.000163 | 0.000031 | 0.000024 | 0.000015 | 0.019016 | 0.9999 |

**`host_submission_s` measures two different things, and the ratio to `eval_s`
says which.** On `gpu_hip` rows it is the instrumented dispatcher's own
asynchronous submission span (`backend.rs:613`), and it is $0.0000$–$0.0019$ of
`eval_s`. On prototype rows it is a host clock wrapped around the candidate's
whole synchronous batch evaluation (`backend.rs:516-520`), and it is
$0.7453$–$1.0000$ of `eval_s`, reaching $0.9988$–$0.9994$ at $n = 24$ and
$1.0000$ at $n = 28$, where the device work dominates. A prototype row's
`host_submission_s` is therefore not a launch overhead and is not read as one
here. The
device-clock `device_submission_to_kernel_s` is the launch measure that is
comparable across all four device paths, and §4.4 uses it.

Kernel-only throughput against end-to-end throughput. Kernel-only is
`matrices` / `kernel_device_s`, so it charges the device kernel span alone;
`eval_matrices_per_s` charges the whole dispatch; `composite_matrices_per_s`
charges generation, evaluation, reduction, and store. The censored cell carries
no throughput value in any of the three.

| $n$ | path | $M$ | outcome | `matrices` | kernel-only matrices/s | eval-only matrices/s | composite matrices/s | kernel-only / composite |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 12 | `fold-gf3` | 45 | measured | 1 075 590 | 2 007 679.1267 | 366 844.1462 | 228 042.8264 | 8.8040 |
| 12 | `wave-gf3` | 44 | measured | 1 003 332 | 1 212 011.2488 | 326 234.2279 | 211 897.8391 | 5.7198 |
| 12 | `gpu_hip` | 256 | measured | 586 240 | 524 094.9207 | 149 016.8549 | 120 552.7194 | 4.3474 |
| 12 | `gpu_hip` | 1024 | measured | 1 022 976 | 765 643.8632 | 320 807.6053 | 214 467.4416 | 3.5700 |
| 16 | `fold-gf3` | 44 | measured | 399 344 | 219 834.6880 | 104 941.5131 | 82 098.9857 | 2.6777 |
| 16 | `wave-gf3` | 45 | measured | 348 300 | 154 083.4094 | 87 823.0996 | 71 337.8740 | 2.1599 |
| 16 | `gpu_hip` | 256 | measured | 134 400 | 34 559.7324 | 29 088.6294 | 27 069.1338 | 1.2767 |
| 16 | `gpu_hip` | 1024 | measured | 283 648 | 80 025.6851 | 67 442.5426 | 57 627.3613 | 1.3887 |
| 20 | `fold-gf3` | 448 | measured | 338 688 | 118 557.0601 | 94 175.6887 | 69 485.2981 | 1.7062 |
| 20 | `wave-gf3` | 41 | measured | 42 845 | 9 328.3582 | 8 899.7089 | 8 598.6487 | 1.0849 |
| 20 | `gpu_hip` | 256 | measured | 10 752 | 2 169.7144 | 2 132.6753 | 2 115.4934 | 1.0256 |
| 20 | `gpu_hip` | 1024 | measured | 24 576 | 5 021.6561 | 4 938.3227 | 4 848.4654 | 1.0357 |
| 24 | `fold-gf3` | 22 | measured | 2 310 | 464.8603 | 462.5711 | 461.3922 | 1.0075 |
| 24 | `wave-gf3` | 17 | measured | 1 224 | 242.9844 | 242.4980 | 242.1683 | 1.0034 |
| 24 | `gpu_hip` | 256 | measured | 1 280 | 137.4456 | 137.2504 | 137.1512 | 1.0021 |
| 24 | `gpu_hip` | 1024 | measured | 5 120 | 313.0974 | 312.6224 | 312.1116 | 1.0032 |
| 28 | `fold-gf3` | 3 | measured | 21 | 3.9647 | 3.9632 | 3.9630 | 1.0004 |
| 28 | `wave-gf3` | 2 | measured | 10 | 1.7889 | 1.7884 | 1.7883 | 1.0004 |
| 28 | `gpu_hip` | 256 | measured | 1 280 | 8.5463 | 8.5453 | 8.5448 | 1.0002 |
| 28 | `gpu_hip` | 1024 | censored | 3 072 | withheld | `NaN` | `NaN` | — |

The kernel-only-to-composite ratio is the size of everything the device kernel
does not pay for. It is 8.8040× at $n = 12$ for `fold-gf3` and 1.0002× at
$n = 28$ for `gpu_hip` at $M = 256$: sampling, packing, reduction, and the
dispatcher's surrounding work cost 88.6 % of the achievable rate at the
smallest order on the fastest path, and 0.02 % at the largest.

Per-launch cost, dividing each total by that cell's `reps`. Each timed
repetition opens one sampler at one stream index and evaluates one batch
(`dev/research/permanent-sampling-feas/src/protocol.rs:765-781`), and one batch
evaluation is one dispatch — `gridDim.x = M` blocks of one thread for the
shipped kernel
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:334-335`,
`:347-351`), `gridDim.x = M` blocks of 32 lanes for the prototypes
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:597-606`) — so
`reps` is the launch count.

| $n$ | path | $M$ | `reps` | host submission µs/launch | device submission→kernel µs/launch | kernel ms/launch | H2D µs/launch | D2H µs/launch |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | `fold-gf3` | 45 | 23 902 | 91.4198 | 4.8438 | 0.0224 | 9.4542 | 16.4801 |
| 12 | `wave-gf3` | 44 | 22 803 | 104.1320 | 4.8455 | 0.0363 | 9.4513 | 16.3424 |
| 12 | `gpu_hip` | 256 | 2 290 | 3.2590 | 4.8541 | 0.4885 | 12.8747 | 9.3153 |
| 12 | `gpu_hip` | 1024 | 999 | 3.3333 | 9.9600 | 1.3374 | 37.5375 | 9.2993 |
| 16 | `fold-gf3` | 44 | 9 076 | 367.0370 | 4.8489 | 0.2002 | 9.7676 | 16.4177 |
| 16 | `wave-gf3` | 45 | 7 740 | 459.0518 | 4.8902 | 0.2920 | 9.8111 | 16.6248 |
| 16 | `gpu_hip` | 256 | 525 | 3.2762 | 4.8514 | 7.4075 | 14.5905 | 9.3410 |
| 16 | `gpu_hip` | 1024 | 277 | 3.2058 | 4.6354 | 12.7959 | 20.2888 | 9.2852 |
| 20 | `fold-gf3` | 448 | 756 | 3 956.4444 | 4.6442 | 3.7788 | 17.2857 | 16.2262 |
| 20 | `wave-gf3` | 41 | 1 045 | 4 532.8402 | 4.8555 | 4.3952 | 12.0737 | 16.8019 |
| 20 | `gpu_hip` | 256 | 42 | 3.4286 | 4.8571 | 117.9879 | 16.2381 | 9.6190 |
| 20 | `gpu_hip` | 1024 | 24 | 3.7500 | 4.6250 | 203.9168 | 25.8333 | 9.3750 |
| 24 | `fold-gf3` | 22 | 105 | 47 503.6286 | 4.8762 | 47.3260 | 9.9048 | 17.1714 |
| 24 | `wave-gf3` | 17 | 72 | 70 059.6667 | 4.8611 | 69.9633 | 9.5556 | 18.4167 |
| 24 | `gpu_hip` | 256 | 5 | 6.4000 | 4.8000 | 1 862.5552 | 19.2000 | 9.8000 |
| 24 | `gpu_hip` | 1024 | 5 | 6.6000 | 4.8000 | 3 270.5478 | 42.4000 | 10.0000 |
| 28 | `fold-gf3` | 3 | 7 | 756 959.1429 | 4.8571 | 756.6769 | 9.7143 | 22.1429 |
| 28 | `wave-gf3` | 2 | 5 | 1 118 338.8000 | 4.8000 | 1 117.9854 | 10.0000 | 22.4000 |
| 28 | `gpu_hip` | 256 | 5 | 8.4000 | 4.8000 | 29 954.6386 | 21.8000 | 9.8000 |
| 28 | `gpu_hip` | 1024 | 3 | 8.0000 | 5.0000 | 53 077.5210 | 54.3333 | 10.3333 |

What the separation shows. Device launch overhead is nearly constant across
every path, order, and batch size: `device_submission_to_kernel_s` is
4.62–9.96 µs per launch over kernel spans that range from 0.022 ms to 53.1 s,
six orders of magnitude. Device-to-host copy is flat at
9.3–22.4 µs per launch because each launch returns at most $M$ result words.
Host-to-device copy is 9.4–54.3 µs per launch and tracks $M$ and $n$. What does
move is the host-side residual: it is 70.0 % of `eval_s` at $n = 12$,
$M = 256$ on the shipped path and 56.3 % on the same path's $M = 1024$ cell,
falls to 1.6 % at $n = 20$, and is under 0.2 % from $n = 24$ up. The
dispatcher's per-call allocation, serialisation, and free — not transfer and
not launch — is what costs the shipped GPU path the $n = 12$ cell, and it is
attributable from these columns without a second run.

### 4.4 Best operating points (REQ-14)

The best operating point of a device path is taken to be the measured cell
whose composite throughput ratio against the best applicable in-tree CPU path
at that order is largest. Among the prototypes, the one whose best operating
point carries the highest such ratio is the best-performing prototype.

| Path | Best operating point | rate | best CPU path at that order | rate | ratio | device launch |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| `fold-gf3` (best prototype) | $n = 20$, $M = 448$ | 69 485.2981 | `cpu_rayon_intra_matrix` | 2 966.4439 | **23.4238** | 4.6442 µs/launch |
| `gpu_hip` (shipped) | $n = 20$, $M = 1024$ | 4 848.4654 | `cpu_rayon_intra_matrix` | 2 966.4439 | 1.6344 | 4.6250 µs/launch |

At `fold-gf3`'s best operating point the launch duration is
`device_submission_to_kernel_s` 0.003511 s over 756 launches, or **4.6442 µs
per launch** on the device clock. The host-clock column at the same cell reads
`host_submission_s` 2.991072 s over 756 launches, or 3 956.4444 µs per launch,
which is the synchronous submit-and-complete span described in §4.3 rather than
a launch overhead. The shipped path's own figure at its best operating point is
4.6250 µs per launch on the device clock and 3.7500 µs on the host clock, both
single-digit microseconds because that path's host column is an asynchronous
submission.

By highest absolute throughput rather than ratio, the best prototype cell is
$n = 12$, $M = 45$ at 228 042.8264 matrices/s, which is 0.7568 of
`cpu_rayon_batch_scalar` at the same order; both readings are printed by
`analysis.py` section 4.4.

### 4.5 What the prototypes' batch sizes do to the comparison

The grid fixes `gpu_hip` at $M \in \{256, 1024\}$ and lets every other cell size
itself: a cell with no requested batch size runs a one-matrix probe and takes
$M = \lceil 2\,\text{s} / \texttt{probe\_matrix\_s} \rceil$, clamped to
$[\,\text{floor},\, 65\,536\,]$
(`dev/research/permanent-sampling-feas/src/protocol.rs:724-737`, and
`GPU_BATCHES` at `dev/research/permanent-sampling-feas/src/main.rs:50`). For the
prototypes that produces batches of 2 to 448 matrices, none of which matches the
control's 256 or 1024.

Because a prototype block owns one matrix and 32 lanes, the batch size *is* the
device parallelism of the cell:

| $n$ | `fold-gf3` $M$ | active lanes | `wave-gf3` $M$ | active lanes | `gpu_hip` $M$ | active lanes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 45 | 1 440 | 44 | 1 408 | 256 / 1024 | 256 / 1024 |
| 16 | 44 | 1 408 | 45 | 1 440 | 256 / 1024 | 256 / 1024 |
| 20 | 448 | 14 336 | 41 | 1 312 | 256 / 1024 | 256 / 1024 |
| 24 | 22 | 704 | 17 | 544 | 256 / 1024 | 256 / 1024 |
| 28 | 3 | 96 | 2 | 64 | 256 / 1024 | 256 / 1024 |

`fold-gf3` at $n = 20$ is the one prototype cell that fills the device an order
of magnitude better than its neighbours, and it is also the cell that carries
the campaign's largest ratio. The cause is in the calibration input rather than
in a probe fault: `probe_matrix_s` is 0.004474 s for `fold-gf3` at $n = 20$
against 0.048872 s for `wave-gf3` at the same order, a 10.9× difference that
the batched measurement reproduces at 10.6× in per-matrix `eval_s`
(1.0618 × 10⁻⁵ s against 1.1236 × 10⁻⁴ s). The fold really is about ten times
cheaper per matrix at that order, and the 2 s target therefore buys it ten
times the batch.

The consequence for the comparison is stated rather than smoothed over: the
$n = 20$ ratio of 23.4238 is the joint effect of the fold circuit and a cell
that runs 14 336 lanes against the shipped path's 1 024, and this campaign
measures no prototype cell at $M \in \{256, 1024\}$ that would separate them.
Every cell's batch size is carried beside its throughput here and in §10, and
§13 keeps the undecomposed ratio on the list of what this run leaves open.

The probe is a single-matrix latency and the preamble forbids deriving a
batched rate from it; the ratio between the two is nevertheless informative
about who is being under-sized. `probe_matrix_s` divided by achieved
`eval_s`/matrix is 1.0–1.3 for the three single-thread CPU paths, 1.0–3.0 for
intra-matrix rayon, 10.7–19.6 for the two batch-rayon paths, and 2.1–16 612 for
the prototypes, falling monotonically with $n$ on both prototypes. The
prototypes' batches are calibrated from a latency that overstates their
per-matrix cost by three to four orders of magnitude at the small orders, which
is why their measured cells sit far below the batch sizes the same 2 s target
gives the CPU paths.

## 5. Censoring (REQ-12)

One cell in the $q = 3$ grid is censored.

- Cell: $q = 3$, $n = 28$, `gpu_hip`, `batch_size` 1024, `order_index` 64.
- `outcome`: `censored`.
- `composite_matrices_per_s`: `NaN`. `eval_matrices_per_s`: `NaN`. No measured
  throughput value is carried, and §4.3 withholds the kernel-only rate for the
  same reason.
- Censoring reason, verbatim from the `note` column: `unavailable: 120 s cap
  ended timing before both minimums (5 repetitions and 5 s); no derived rate is
  reported`. The cell completed 3 repetitions of 53.02–53.20 s each in 159.27 s
  of total time; the protocol requires at least 5 repetitions as well as at
  least 5 s of timed work (`protocol.rs:101-107`, `:178-191`), and the 120 s cap
  censors rather than truncates, so the third repetition finished and the fourth
  was not started.
- Projection: `projected_matrices_per_s` 16.720265, from
  `projection_reference_n` 24. The rate it is projected from is that cell's own
  chain, `gpu_hip` at $M = 1024$, $n = 24$, measured at 312.1116 matrices/s,
  rescaled through Ryser's $n \cdot 2^n$ work model
  ($312.1116 \times 24 \cdot 2^{24} / (28 \cdot 2^{28}) = 16.720264$, matching
  the CSV's 16.720265 to seven significant figures).

The projection is an estimate and is labelled as one in the CSV preamble. This
run's own $q = 3$ device chains measure the direction and size of its bias
where both ends of a step are measured:

| path | step | projection | measured | error |
| --- | --- | ---: | ---: | ---: |
| `gpu_hip` $M{=}256$ | $12 \rightarrow 16$ | 5 650.9087 | 27 069.1338 | $-79.1\%$ |
| `gpu_hip` $M{=}256$ | $16 \rightarrow 20$ | 1 353.4567 | 2 115.4934 | $-36.0\%$ |
| `gpu_hip` $M{=}256$ | $20 \rightarrow 24$ | 110.1819 | 137.1512 | $-19.7\%$ |
| `gpu_hip` $M{=}256$ | $24 \rightarrow 28$ | 7.3474 | 8.5448 | $-14.0\%$ |
| `gpu_hip` $M{=}1024$ | $12 \rightarrow 16$ | 10 053.1613 | 57 627.3613 | $-82.6\%$ |
| `gpu_hip` $M{=}1024$ | $16 \rightarrow 20$ | 2 881.3681 | 4 848.4654 | $-40.6\%$ |
| `gpu_hip` $M{=}1024$ | $20 \rightarrow 24$ | 252.5242 | 312.1116 | $-19.1\%$ |
| `wave-gf3` | $12 \rightarrow 16$ | 9 932.7112 | 71 337.8740 | $-86.1\%$ |
| `wave-gf3` | $16 \rightarrow 20$ | 3 566.8937 | 8 598.6487 | $-58.5\%$ |
| `wave-gf3` | $20 \rightarrow 24$ | 447.8463 | 242.1683 | $+84.9\%$ |
| `wave-gf3` | $24 \rightarrow 28$ | 12.9733 | 1.7883 | $+625.5\%$ |
| `fold-gf3` | $12 \rightarrow 16$ | 10 689.5075 | 82 098.9857 | $-87.0\%$ |
| `fold-gf3` | $16 \rightarrow 20$ | 4 104.9493 | 69 485.2981 | $-94.1\%$ |
| `fold-gf3` | $20 \rightarrow 24$ | 3 619.0259 | 461.3922 | $+684.4\%$ |
| `fold-gf3` | $24 \rightarrow 28$ | 24.7174 | 3.9630 | $+523.7\%$ |

On the `gpu_hip` chains every step runs low and the magnitude shrinks
monotonically with $n$, which is what the launch-amortisation mechanism the
preamble names predicts, and it is the scope the preamble claims: "*on the q=3
GPU chain, where the projection can be checked against a measurement, it lands
LOW at every step*". **On the prototype chains that pattern does not hold**: the
projection lands low at the two small steps and high by 85 % to 684 % at the two
large ones. The mechanism is §4.5's — a prototype chain's batch size, and with
it its device parallelism, changes by up to a factor of 20 between adjacent
orders, so a work-model rescale that assumes a fixed configuration has no reason
to hold along it. This is recorded rather than reconciled; the preamble's claim is
about the GPU chain and this run does not contradict it there.

The step that governs the censored cell is $24 \rightarrow 28$ on the same
`gpu_hip` family, measured on the $M = 256$ chain of this same file at
$-14.0\%$. Carrying that factor across gives roughly 19.4 matrices/s as the
cell's expected true rate. That figure is an extrapolation from a neighbouring
batch size, not a measurement; the censored cell carries no throughput here and
none is asserted for it. It is stated because it is the quantity that decides
the $n = 28$ ordering, and §11 records what follows.

No $q = 3$ cell reports a build failure, a correctness failure, or a device
resource exhaustion. The 25 non-`measured`, non-`censored` cells are the
out-of-field prototype rows of §2.

## 6. Component isolation (REQ-03)

REQ-03 asks that the dependency-chained Gray update, the horizontal product,
and the end-to-end Ryser loop each be isolated for every representation of this
field that executes; that a representation whose component admits no distinct
isolate — a fused or branchless circuit — be recorded with that structural
reason in place of a duration; and that a censored isolate carry its censoring
reason. The end-to-end Ryser loop is §4, covering all nine executing paths at
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
[`…-q3-gray-update.csv`](permanent-campaign-20260814T230032Z-2085453-q3-gray-update.csv)).

| $n$ | `cpu_scalar` `net_per_operation_s` | `gpu_hip` `update_s` | `gpu_hip` baseline | `gpu_hip` net |
| ---: | ---: | ---: | ---: | --- |
| 12 | 3.27e-10 | 2.237653733 | 2.702512101 | absent |
| 16 | 3.36e-10 | 2.237917272 | 2.702815432 | absent |
| 20 | 3.34e-10 | 2.238167417 | 2.703112754 | absent |
| 24 | 3.32e-10 | 2.238409869 | 2.703418835 | absent |
| 28 | 3.37e-10 | 2.242713809 | 2.708629457 | absent |

`cpu_scalar` is `measured` at every order over 1 000 001 steps and 4275–4305
repetitions, with `duration_basis = host_clock_update_chain`. Its net
per-operation duration is flat in $n$ to within 3.1 %, which is the signature the
shape predicts: the Gray update is constant work per step and the matrix order
does not enter it.

`gpu_hip` is `censored` at every order with the reason, verbatim at $n = 12$:
`unavailable: paired update span 2.237653733 s minus compiler-barrier baseline
2.702512101 s is nonpositive; no net per-operation duration is reported`. The
Bipedal3 two-plane add/subtract is cheap enough on this device that the chain
runs *faster* than the same-geometry barrier baseline it is measured against, by
0.4649–0.4659 s over 68 × 1 000 001 operations at every order. The harness
reports no rate rather than a clamped or negative one, and no false positive
rate is substituted.

That reason is a censoring reason: the row executed, its paired spans are
retained as diagnostics, and only the derived rate is withheld.

**The two $\mathbb{F}_3$ prototypes carry a structural reason instead of a
measurement**, which is the fused-circuit case: `unsupported: fold-gf3 applies
the same packed Bipedal3 add/subtract the shipped gpu_hip Gray-update circuit
times; and its own update runs fused inside the full-permanent wave kernel; no
circuit distinct to this candidate exists to isolate`, and the same string for
`wave-gf3` (`dev/research/permanent-sampling-feas/src/gray_update.rs:231-248`).
The five remaining CPU rows carry `unsupported: <backend> has no isolated
dependency-chained Gray-update evaluator`, under the rule the isolate's own
preamble states — "*A row reports a span only from a circuit distinct to the
backend it names*" — so they are the same structural case: they share
`cpu_scalar`'s packed host add/subtract and have no circuit of their own to
isolate.

So this field has one Gray-update circuit across all its representations. The
isolate measures it on the host at every order, attempts it on the device at
every order and censors it there with the reason above, and records every other
row's absence as the structural fact that the row's backend has no distinct
circuit to time.

### 6.2 Horizontal product

Shape: each timed sample is one unconditioned row-sum vector. The zero branch
times the representation's early product exit; the nonzero branch times zero
detection plus the complete representation-specific reduction. Timing is paired
device-event kernel spans only, excluding allocation, upload, download,
submission, sampling, grouping, and host policy.

`fold-gf3` is `measured` at every order. Its circuit is
`HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount`
(`dev/research/permanent-sampling-feas/src/horizontal_product.rs:496-498`),
which enters `horizontal_product_micro_kernel` and dispatches to
`bipedal3_zero_mask_sign_popcount_product`
(`crates/gf2-kernels-hip/hip/permanent/horizontal_product_micro.hip:141-144`).

| $n$ | `reps` | `zero_fast_s` | zero baseline | `zero_fast_net_per_operation_s` | zero ops | `nonzero_slow_s` | nonzero baseline | nonzero net | nonzero ops |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| 12 | 1 781 | 0.016941578 | 0.016739697 | 2.8e-11 | 7 238 834 | 0.015472542 | 0.015715163 | absent | 56 142 |
| 16 | 1 717 | 0.016108213 | 0.015967134 | 2.0e-11 | 7 022 093 | 0.014665668 | 0.014972596 | absent | 10 739 |
| 20 | 1 828 | 0.017283377 | 0.017089773 | 2.6e-11 | 7 485 247 | 0.011296911 | 0.011543860 | absent | 2 241 |
| 24 | 2 177 | 0.020431881 | 0.020232145 | 2.2e-11 | 8 916 463 | 0.004011606 | 0.004106485 | absent | 529 |
| 28 | 2 224 | 0.021166517 | 0.020910727 | 2.8e-11 | 9 109 408 | 0.000838733 | 0.000858527 | absent | 96 |

The nonzero-slow branch is censored at every order, and carries its censoring
reason: `nonzero slow timing unavailable: raw device span minus its
same-geometry baseline was nonpositive; so no false positive rate is reported`.
Only the zero-fast branch carries a duration, and it is flat at
2.0–2.8 × 10⁻¹¹ s per operation across the five orders, which the shape predicts
for an early exit whose expected work does not grow with $n$.

`gpu_hip` and `wave-gf3` both select `Bipedal3Halving`
(`horizontal_product.rs:486`, `:494`) and are `unavailable` with the reason
`gpu_hip`/`wave-gf3` `uses the bipedal3-halving reduction; whose zero result is
observed only after its complete reduction; emitting separate branch timings
would invent a different circuit`. The halving reduction has no observable
branch boundary — "*The halving control only discovers zero after its full
reduction and must not be assigned synthetic branch timings*"
(`crates/gf2-kernels-hip/src/permanent/mod.rs:1383-1388`) — so the row returns
before any launch rather than reporting a branch split the circuit does not
have. Every CPU row is `unavailable: <backend> has no distinct device-event
horizontal-product isolate; no generic or host-clock replacement was used`.

So of the two $\mathbb{F}_3$ horizontal-product representations, the
zero-mask/sign-popcount fold yields an isolated duration on its zero branch at
every order and a censoring reason on its nonzero branch, and the
bipedal3-halving representation yields none because it is branchless at this
boundary — the structural case, recorded as the reason rather than worked
around.

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
| `permanent_bipedal3_kernel` | `gpu_hip` end-to-end | 27 | 19 | 1040 | 0 | 0 | 0 | 16 | `permanent_bipedal3.hip.resource.log:1` |
| `wave_gf3_kernel<FoldKindE0>` | `wave-gf3` end-to-end | 22 | 22 | 0 | 0 | 0 | 0 | 16 | `wave_gf3_equivalence.hip.resource.log:23` |
| `wave_gf3_kernel<FoldKindE1>` | `fold-gf3` end-to-end | 22 | 25 | 0 | 0 | 0 | 0 | 16 | `wave_gf3_equivalence.hip.resource.log:34` |
| `gray_update_micro_kernel` | Gray-update isolate | 86 | 5 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:1` |
| `gray_update_compiler_barrier_baseline_kernel` | its paired baseline | 13 | 2 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:12` |
| `horizontal_product_micro_kernel` | horizontal-product isolate | 26 | 3 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:24` |
| `horizontal_product_compiler_barrier_baseline_kernel` | its paired baseline | 14 | 2 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:35` |
| `permanent_wave_gpu_probe` | empty control kernel | 0 | 0 | 0 | 0 | 0 | 0 | 16 | `probe.hip.resource.log:1` |

The same logs carry four probe kernels of the wave prototype that no timing in
this campaign exercises: `n63_mapping_probe` at 7 SGPRs and 8 VGPRs
(`wave_gf3_equivalence.hip.resource.log:1`), `n4_direction_probe` at 9 and 9
(`:12`), and both `active_mask_product_probe` specializations at 7 and 2
(`:43`, `:54`), each with zero scratch, zero spills, zero static LDS, and
occupancy 16.

`FoldKindE0` is the halving control and `FoldKindE1` the zero-mask/sign-popcount
candidate, in source declaration order
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:157`). The two
horizontal-product kernels appear in the `permanent_bipedal7.hip` log because
`horizontal_product_micro.hip` has no translation unit of its own: it is
included textually so the $\mathbb{F}_7$ lookup circuit reads that unit's
`__constant__` table, and compiling it alone fails on that undeclared symbol.
The receipt records it as `translation_unit_role: included-fragment` against the
`permanent_bipedal7.hip` log.

### 7.1 Predicted budgets and what the device reports

**Lane-owns-interval prototypes (`wave-gf3`, `fold-gf3`).** The design predicts
a per-lane register budget and a per-block shared-memory allocation, both stated
in
[`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md):
the state model is "*two packed `u64` words plus `u64` Gray cursor/end bounds
and one `u32` partial sum: a 9 x 32-bit-register lower bound*" (`:172-174`), and
"*The launch still reserves the source-declared dynamic column table of $16n$
bytes per block, which this static-LDS report does not include*" (`:422-423`).
The launch confirms the second: `2 * n * sizeof(std::uint64_t)` shared bytes for
both specializations
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:597-606`,
`:353-361`), backing `extern __shared__ std::uint64_t staged_columns[]` at
`:171`.

| Kernel | predicted per-lane registers | measured per-lane (`VGPRs`) | measured per-wave (`TotalSGPRs`) | predicted per-block shared | measured static LDS |
| --- | --- | ---: | ---: | --- | --- |
| `wave_gf3_kernel<FoldKindE0>` | $\ge 9 \times$ 32-bit | 22 | 22 | $16n$ B | 0 B (static field only) |
| `wave_gf3_kernel<FoldKindE1>` | $\ge 9 \times$ 32-bit | 25 | 22 | $16n$ B | 0 B (static field only) |

The register prediction is a per-lane lower bound and the per-lane measurement
respects it, at 2.44× and 2.78× the bound. The README records the falsification
this produces, and this receipt quotes it rather than paraphrasing: *"The
resource comparison falsifies a zero-allocation interpretation of the nine
32-bit-unit source-level mapping model: neither fold adds persistent mapping
state, but the candidate's local zero-mask/popcount work costs three additional
VGPRs."* (`:425-428`). That three-VGPR gap reproduces in this campaign's own
receipt, at 22 VGPRs for the control against 25 for the candidate.

The shared-memory prediction is **not confirmed and not refuted by this
campaign**. `-Rpass-analysis=kernel-resource-usage` reports static LDS only, and
the $16n$-byte table is requested dynamically at launch, so a 0 in that column
is silence rather than evidence of absence. No occupancy conclusion in §8 rests
on treating it as a zero.

**Shipped GPU path (`permanent_bipedal3_kernel`).** No committed design document
states a predicted per-lane register budget for this kernel; §12 records that
absence rather than filling it with a back-derived number. Its predicted
per-block shared-memory allocation is exact and available: the launch passes
`sharedMemBytes = 0`
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:350`) and the
translation unit contains no `__shared__` declaration, so 0 bytes/block is the
complete shared-memory picture for this kernel and the measurement confirms it.

Its measured **1040 scratch bytes per lane** is the campaign's other
quantitative finding on resources, and it has an exact cause in the source:
`uint64_t col_m[64]` and `uint64_t col_s[64]`
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:215-216`) are
1024 bytes of per-thread column table, indexed at runtime by `flip = ctz(k)`
inside the Gray walk (`:272-280`), so they cannot be register-allocated. This
does not contradict the study's characterisation of the kernel as updating its
$(\text{sum}_m, \text{sum}_s)$ accumulator in $O(1)$ work per Gray step
([`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.3) — that claim is about the accumulator, which the measurement leaves intact
at 19 VGPRs with zero spills. It does locate, in measured bytes, the allocation
the lane-owns-interval design proposes to relocate: the shipped mapping holds
the column table in per-thread private memory backed by device memory, and the
prototype mapping holds a $16n$-byte column table in per-block shared memory. At
$n = 28$ those are 1024 bytes per lane against 448 bytes per block.

**Gray-update isolate.** `gray_update_micro_kernel` reports 86 `TotalSGPRs`
against 5 `VGPRs` — the most scalar-register-heavy kernel in this field's set.
No design document predicts a budget for it. Its paired barrier baseline reports
13 and 2. That the baseline is far cheaper in registers while measuring *longer*
on the device (§6.1) is recorded as the observation it is, and is why that
isolate reports no net rate.

No kernel measured over this field reports a nonzero SGPR or VGPR spill count,
and no measurement in this campaign contradicts a numeric register or
shared-memory prediction that a committed design states. The one prediction that
a measurement does contradict — the zero-allocation reading of the nine-unit
model — is carried above with its contradiction, in the words of the artifact
that recorded it.

## 8. What limits occupancy (REQ-08)

The compiler reports `Occupancy [waves/SIMD]: 16` for every kernel this campaign
measures over $\mathbb{F}_3$, at every order, and the resource figures do not
vary with $n$. Two controls in the same receipt fix what that number means.

The upper control is `permanent_wave_gpu_probe`, an empty kernel with 0
`TotalSGPRs`, 0 `VGPRs`, 0 scratch, and 0 LDS (`probe.hip.resource.log:1`),
which also reports 16. A kernel that consumes no registers at all cannot be
register-limited, so 16 is the saturation value of that field on `gfx1030`.

The lower control is `permanent_bipedal7_kernel` in the same receipt
(`permanent_bipedal7.hip.resource.log:1`): 107 `TotalSGPRs`, 128 `VGPRs`,
4000 scratch bytes per lane, 4 SGPR spills, 6 VGPR spills, and **occupancy 8**.
The field does fall when per-thread usage is heavy, so a report of 16 is a
measured absence of register pressure rather than an unresponsive constant. That
kernel evaluates $\mathbb{F}_7$ permanents and no cell of this campaign runs it;
it is cited as the control that makes the $\mathbb{F}_3$ readings interpretable.

Derived from the measured per-thread resource usage, therefore: **no measured
$\mathbb{F}_3$ kernel is limited by registers, private scratch, or static LDS,
at any measured order.** The supporting facts are that each reports occupancy at
the ceiling that the empty probe establishes, each reports zero SGPR and VGPR
spills, none reports a static LDS allocation, and the heavy $\mathbb{F}_7$
kernel in the same receipt shows the occupancy field falling to 8 when register
use, scratch, and spills all rise.

The statement holds at every measured order without a per-order table because
$n$ reaches these kernels as a runtime argument rather than a template
parameter: the mangled names are `_Z25permanent_bipedal3_kernelPKhiiPy` and
`_ZN12_GLOBAL__N_115wave_gf3_kernelILNS_8FoldKindE0EEEvPKhiiPj`, whose only
template parameter is `FoldKind`, and the compiler emits one resource block per
kernel rather than one per order.

What does bound each path's device parallelism at each measured order is its
launch geometry and its batch size, which are fixed by the kernel's own contract
and by the cell's calibration rather than by its resource usage. The shipped
kernel launches `gridDim.x = M` blocks of `dim3 block(1, 1, 1)`, one matrix per
block, with only thread 0 doing work
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:174-176`, `:334-335`,
`:347-351`), so its resident wave count cannot exceed $M$ and each resident wave
carries one active lane: 256 or 1024 single-lane waves for the whole device. The
prototypes launch `gridDim.x = M` blocks of `active_lanes_for_order(n)` lanes,
which is 32 at every order this campaign measures and $2^n$ below $n = 5$
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31`), giving the
active-lane counts tabulated in §4.5: 64 to 14 336 lanes depending on the cell's
calibrated batch. The lane-owns-interval mapping supplies 32 lanes per matrix
where the control supplies one, and the batch calibration is what decides how
many matrices are in flight.

One resource this derivation cannot rule on is dynamic LDS, for the reason in
§7.1: the compiler's static field is silent about the prototypes' $16n$-byte
launch-time table, so no claim is made that shared memory does or does not bound
the prototype mapping's occupancy.

## 9. Zero fast path and its exact marginal expectation (REQ-10)

The horizontal-product isolate carries the observed branch frequencies. They
derive from one fixed unconditioned host observation batch per order, addressed
by $(\text{seed\_root}, q, n, \texttt{horizontal\_product\_timed},
\text{seed\_index})$; device timing resamples under the canonical warm-up and
repetition policy and does not alter these counts (preamble of
[`…-q3-horizontal-product.csv`](permanent-campaign-20260814T230032Z-2085453-q3-horizontal-product.csv)).
Every row at one order carries the same counts, so each order's frequency is one
observation rather than fourteen.

For $q = 3$, sample count $N = 4096$ at each order. Intervals are two-sided
Wilson score intervals at nominal 95 % coverage with $z = 1.959963984540054$,
computed in `analysis.py`; the Wilson form is used because it stays inside
$[0, 1]$ and remains defined at zero successes, which two of these ten cells
have.

| $n$ | branch | observed | frequency | exact expectation | Wilson 95 % | expectation inside interval |
| ---: | --- | ---: | ---: | ---: | --- | --- |
| 12 | zero fast | 4064 / 4096 | 0.992187500 | $1 - (2/3)^{12} = 0.992292653371$ | [0.988992173, 0.994460490] | yes |
| 12 | nonzero slow | 32 / 4096 | 0.007812500 | $(2/3)^{12} = 0.007707346629$ | [0.005539510, 0.011007827] | yes |
| 16 | zero fast | 4092 / 4096 | 0.999023438 | $1 - (2/3)^{16} = 0.998477561160$ | [0.997491557, 0.999620170] | yes |
| 16 | nonzero slow | 4 / 4096 | 0.000976562 | $(2/3)^{16} = 0.001522438840$ | [0.000379830, 0.002508443] | yes |
| 20 | zero fast | 4094 / 4096 | 0.999511719 | $1 - (2/3)^{20} = 0.999699271340$ | [0.998221290, 0.999866085] | yes |
| 20 | nonzero slow | 2 / 4096 | 0.000488281 | $(2/3)^{20} = 0.000300728660$ | [0.000133915, 0.001778710] | yes |
| 24 | zero fast | 4096 / 4096 | 1.000000000 | $1 - (2/3)^{24} = 0.999940596808$ | [0.999063023, 1.000000000] | yes |
| 24 | nonzero slow | 0 / 4096 | 0.000000000 | $(2/3)^{24} = 0.000059403192$ | [0.000000000, 0.000936977] | yes |
| 28 | zero fast | 4096 / 4096 | 1.000000000 | $1 - (2/3)^{28} = 0.999988266036$ | [0.999063023, 1.000000000] | yes |
| 28 | nonzero slow | 0 / 4096 | 0.000000000 | $(2/3)^{28} = 0.000011733964$ | [0.000000000, 0.000936977] | yes |

**The two expectations are complements.** $1 - ((q-1)/q)^n$ and $((q-1)/q)^n$
sum to exactly 1, which the computed values reproduce to 15 decimal places at
every order, and the two observed frequencies likewise sum to exactly 1 because
the branches partition the 4096 samples. The CSV states the same relation in its
`note` column on the measured row and carries both expectations in its own
`zero_fast_expected_frequency` and `nonzero_slow_expected_frequency` columns,
which agree with the values above to all twelve printed digits. Every
expectation falls inside its interval, so the observation is consistent with the
exact marginal at every measured order.

At $n = 24$ and $n = 28$ the 4096-sample batch contains no nonzero-slow sample
at all, and its interval is correspondingly wide. The same file carries a second,
much larger observation of the same branches: the device timing loop classifies
every one of its own timed operations into the two branches, and those counts
are `zero_fast_timed_operations` and `nonzero_slow_timed_operations`. They are
drawn from the same purpose stream as the observation batch, so they are not an
independent replicate and are not pooled with it; they are reported because they
bound the same quantity three orders of magnitude more tightly.

| $n$ | zero-fast ops | nonzero-slow ops | total | slow frequency | exact expectation | Wilson 95 % |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 12 | 7 238 834 | 56 142 | 7 294 976 | 0.007695981 | 0.007707347 | [0.007632825, 0.007759656] |
| 16 | 7 022 093 | 10 739 | 7 032 832 | 0.001526981 | 0.001522439 | [0.001498394, 0.001556113] |
| 20 | 7 485 247 | 2 241 | 7 487 488 | 0.000299299 | 0.000300729 | [0.000287163, 0.000311948] |
| 24 | 8 916 463 | 529 | 8 916 992 | 0.000059325 | 0.000059403 | [0.000054480, 0.000064600] |
| 28 | 9 109 408 | 96 | 9 109 504 | 0.000010538 | 0.000011734 | [0.000008631, 0.000012868] |

The exact expectation falls inside the interval at all five orders on this
second observation too, and the intervals are three orders of magnitude
narrower: at $n = 28$, 96 nonzero-slow operations out of 9 109 504 bracket the
frequency in $[8.631, 12.868] \times 10^{-6}$ against an expectation of
$1.1734 \times 10^{-5}$. Each total equals `reps × samples_per_rep` exactly,
which `analysis.py` asserts.

$\mathbb{F}_3$ has the largest zero fast-path share of the three fields at every
order, because $1 - ((q-1)/q)^n$ decreases in $q$ at fixed $n$:

| $q$ | zero fast at $n = 12$ | zero fast at $n = 28$ |
| ---: | ---: | ---: |
| 3 | 0.992292653 | 0.999988266 |
| 5 | 0.931280523 | 0.998065719 |
| 7 | 0.842732666 | 0.986649735 |

This run's data follows that ordering, and it is what decides which branch this
field's fold comparison can time: the slow branch is reached by 32 of 4096
samples at $n = 12$ and by none at $n = 24$ and $n = 28$, which is why its
device timing has too few operations to survive the barrier subtraction (§6.2)
while the fast branch, at 7.2–9.1 million timed operations, is the one that
carries a duration. The cost of the early return is the quantity this field's
isolate actually exposes.

### 9.1 Permanent-zero fraction observed in passing

Distinct from the branch frequencies above, and reported because the `zeros`
column of the grid records it: the fraction of sampled matrices whose permanent
is $0 \bmod 3$. These are by-products of the timing protocol with no
preregistered $N$; the counts are whatever each cell's repetition policy
required. Pooling across cells at one order pools independent samples, because
each cell draws from its own reserved index block (§4.2).

| $n$ | zeros | matrices | fraction | Wilson 95 % |
| ---: | ---: | ---: | ---: | --- |
| 12 | 2 227 919 | 6 676 136 | 0.333714 | [0.333356, 0.334072] |
| 16 | 510 881 | 1 531 565 | 0.333568 | [0.332822, 0.334315] |
| 20 | 152 302 | 456 236 | 0.333823 | [0.332456, 0.335193] |
| 24 | 4 834 | 14 353 | 0.336794 | [0.329106, 0.344568] |
| 28 | 955 | 2 771 | 0.344641 | [0.327172, 0.362540] |

## 10. Execution mapping: control and lane-owns-interval (REQ-04)

The control mapping is measured across the whole grid. `permanent_bipedal3_kernel`
maps one matrix to one block of one working thread
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:174-176`,
`:334-335`), which is the current one-thread-per-matrix execution mapping, at
orders $\{12, 16, 20, 24, 28\}$ and batch sizes $\{256, 1024\}$, with the
throughput, phase, and resource figures of §4 and §7.

The lane-owns-interval mapping is measured across the whole grid as well.
`wave_gf3_kernel<FoldKind>` launches one block per matrix with
`active_lanes_for_order(n)` lanes and a $2n$-word dynamic shared column table
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:597-606`), each
lane owning a balanced Gray interval
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:33-40`). Both
specializations are equivalence-confirmed against the CPU oracle at every grid
order in the same run (§3) and are `measured` in all ten of their grid cells.

Each mapping's batch size is carried in its own column beside the throughput it
produced. The control's are the grid's two fixed sizes; the prototypes' are what
each cell's own probe calibrated.

| $n$ | control $M$ | control rate | `wave-gf3` $M$ | `wave-gf3` rate | `fold-gf3` $M$ | `fold-gf3` rate | fold / control |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 1024 | 214 467.4416 | 44 | 211 897.8391 | 45 | 228 042.8264 | 1.0633 |
| 16 | 1024 | 57 627.3613 | 45 | 71 337.8740 | 44 | 82 098.9857 | 1.4247 |
| 20 | 1024 | 4 848.4654 | 41 | 8 598.6487 | 448 | 69 485.2981 | 14.3314 |
| 24 | 1024 | 312.1116 | 17 | 242.1683 | 22 | 461.3922 | 1.4783 |
| 28 | 256 | 8.5448 | 2 | 1.7883 | 3 | 3.9630 | 0.4638 |

The control column is `gpu_hip` at its better configuration per order, which is
$M = 1024$ everywhere except $n = 28$, where $M = 1024$ is censored (§5) and
$M = 256$ is the only measured control cell.

**The two mappings are compared at matching orders, and each cell's batch size
is carried beside its throughput because the two differ in it.** All five orders
carry both mappings. The grid fixes the control at $M \in \{256, 1024\}$ and
calibrates every prototype cell from its own probe, producing $M \in \{2, 3, 17,
22, 41, 44, 45, 448\}$; §4.5 quantifies what that difference is worth, in active
lanes per repetition and in the probe cost that sets it. No prototype cell in
this campaign runs at 256 or at 1024, so the fold/control ratios above compare
cells that differ in device parallelism as well as in mapping, and the $n = 20$
column is where that difference is largest. The ordering on the order axis is
what these ratios support; the size of the mapping's own contribution at any one
order is not separable from them.

The comparison that this campaign does support at matched geometry is the
resource comparison of §7: control at 27 SGPR / 19 VGPR / 1040 scratch bytes per
lane / 0 shared bytes per block, against prototypes at 22 SGPR / 22–25 VGPR /
0 scratch / $16n$ dynamic shared bytes per block. That is a real difference in
where the column table lives, measured on both sides, and it is where the
study's occupancy hypothesis stands after this run: neither mapping is
register-limited or spill-limited on `gfx1030`, and the prediction that
shared-memory pressure grows with matrix order is untested because the compiler's
static field cannot see the dynamic allocation and no prototype cell in this run
is large enough for that pressure to bind.

## 11. This campaign against the figures it has to confirm or overturn

The comparison target is
[`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.4. Its grid runs on the same CPU, the same GPU, the same ROCm 7.2.4, and the
same `powersave` governor, under the same harness protocol
([`dev/studies/b488f02c/throughput-2026-08-07.csv`](../b488f02c/throughput-2026-08-07.csv)
preamble); the two differ in host kernel (`7.1.3-arch1-3` there against
`7.1.6-arch1-1` here) and in Rust toolchain (`1.97.0` there against the
campaign's manifest-pinned `1.95.0`). `analysis.py` section 11 prints the full
34-pair comparison.

**The crossover shape of the shipped paths is confirmed, and a prototype
overturns its middle.** The study reports that batch rayon takes $n = 12$, the
GPU at $M = 1024$ takes $n = 16$ through $n = 24$, and intra-matrix rayon takes
$n = 28$. Restricted to the paths the study measured, this campaign reproduces
every one of those: at $n = 12$ `cpu_rayon_batch_scalar` leads at 301 317.4209
against the GPU's 214 467.4416; at $n \in \{16, 20, 24\}$ `gpu_hip` at $M = 1024$ leads
every CPU path; at $n = 28$ `cpu_rayon_intra_matrix` leads every measured cell.
Over the full candidate set the middle of that range belongs to `fold-gf3`,
which leads every path at $n \in \{16, 20, 24\}$ by 1.4247×, 14.3314×, and
1.4783× over the best GPU configuration.

**The decay of the shipped GPU's margin is confirmed at $n = 20$ and $n = 24$
and unresolved at $n = 28$.** The study's margins over intra-matrix rayon are
1.63× at $n = 20$, 1.05× at $n = 24$, and 0.984× at $n = 28$. This campaign
measures **1.6344×** at $n = 20$ and **1.0527×** at $n = 24$, confirming the
first two within 1 %. It cannot confirm or overturn 0.984×: that figure is the
$M = 1024$ cell, which is censored here (§5), and this run's only measured
$n = 28$ GPU cell is $M = 256$ at 8.5448 matrices/s, 0.4390× the best CPU path.
The $-14.0\%$ projection bias measured on this file's own $24 \rightarrow 28$
`gpu_hip` chain puts the censored cell's expected rate near 19.4 against
intra-matrix rayon's 19.4629, which is the same too-close-to-call territory the
study describes; it is an extrapolation and settles nothing. **No ordering is
asserted at $n = 28$ between those two.** What this run does settle at $n = 28$
is that neither prototype is competitive there: `fold-gf3` at 3.9630 and
`wave-gf3` at 1.7883 are 0.2036× and 0.0919× the best CPU path, in cells of 3
and 2 matrices.

**The $28.65\times \rightarrow 0.46\times$ restatement is confirmed.** The
2026-05-15 receipt's headline is a 28.65× GPU win at $n = 24$ against "CPU
SIMD", which the study reproduces as 29.3× and restates as 0.46× once the
baseline is the best CPU path. This campaign measures `gpu_hip` at $M = 256$
over `cpu_avx2` as **28.66×** at $n = 24$ and **28.58×** at $n = 28$, and the
same configuration over the best applicable in-tree CPU path as **0.4626×** at
$n = 24$ and **0.4390×** at $n = 28$. The headline ratio and its restatement
both reproduce, so the divergence remains entirely in the choice of CPU
baseline.

**Run-to-run agreement.** Across the 34 backend/order pairs both runs measure,
the median absolute disagreement is 2.08 % and every CPU pair agrees within
7.59 %. From $n = 20$ upward every pair agrees within 4.58 % and every GPU pair
within 0.96 %, including $n = 28$, $M = 256$ at 0.15 % and $n = 24$,
$M = 1024$ at 0.55 %. The single large disagreement is $n = 12$, $M = 256$ at
$-44.77\%$ (218 275 against 120 552.7194), with $n = 12$, $M = 1024$ at
$-13.40\%$. The phase columns locate it: at $n = 12$, $M = 256$ the kernel is
28.4 % of `eval_s` and the host-side residual is 70.0 % (§4.3), so that cell
measures the dispatcher's per-call host work far more than it measures the
device, and it is the least reproducible cell in the grid. This is recorded as a
limit on the smallest order rather than smoothed over; it does not touch the
crossover, which is decided at $n \ge 16$ where agreement is within 10.4 %.

**The projection bias is confirmed in direction and, at the two steps the study
publishes, in magnitude — on the GPU chain only.** The study measures the
$q = 3$ GPU projection landing low by 14–18 % at $20 \rightarrow 24$ and
$24 \rightarrow 28$; this campaign measures $-19.7\%$ and $-14.0\%$ at
$M = 256$ and $-19.1\%$ at $20 \rightarrow 24$, $M = 1024$. It also extends the
chain to two steps the study does not publish, where the bias is far larger:
$-79.1\%$ and $-82.6\%$ at $12 \rightarrow 16$, and $-36.0\%$ and $-40.6\%$ at
$16 \rightarrow 20$. The bias shrinks monotonically with $n$ on both GPU chains.
On the two prototype chains it does not shrink and it changes sign (§5), so the
"lands low at every step" statement is a property of a fixed-batch chain and not
of the work model.

## 12. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §2, §3, §4.2 | **Satisfied.** The current GPU path at both configured batch sizes, both planned $\mathbb{F}_3$ prototype paths (each `measured` in all ten of its grid cells), and all six in-tree CPU paths are compared over $\mathbb{F}_3$ at five orders, with the best applicable CPU path identified per order from this run's own data. Every timing cell draws from one identical sampler on preregistered, structurally disjoint per-cell stream addresses (`main.rs:54`, `:371-391`, `:593-598`); the equivalence comparison runs on one literally identical matrix corpus per $(q, n)$ (`equivalence.rs:139`, `:186-192`). |
| REQ-02 | §1, §4.1 | **Satisfied.** Every listed item maps to a named CSV column or provenance line, tabulated in §4.1. |
| REQ-03 | §4, §6 | **Satisfied.** The end-to-end Ryser loop is isolated for all nine executing paths at all five orders. Gray update, at all five orders: a duration for the host circuit (`cpu_scalar`); the shipped device circuit censored with its censoring reason verbatim, the nonpositive barrier subtraction; both prototypes recorded with the structural reason that their update is the same packed Bipedal3 add/subtract, fused inside the full-permanent wave kernel, so no candidate-distinct circuit exists to isolate; the remaining CPU backends recorded as having no distinct evaluator, under the preamble's rule that a row reports a span only from a circuit distinct to the backend it names. Horizontal product, at all five orders: a zero-branch duration for the zero-mask/sign-popcount representation, its nonzero branch censored with its censoring reason at every order, and the bipedal3-halving representation recorded with the structural reason that its zero result is observable only after the complete reduction, so it is branchless at this boundary and no synthetic split is emitted. No absence is left without a reason of the kind the criterion names. |
| REQ-04 | §10, §4.5 | **Satisfied.** Both mappings are measured at all five matching orders. The control's fixed $M \in \{256, 1024\}$ and each prototype cell's probe-calibrated $M \in \{2, 3, 17, 22, 41, 44, 45, 448\}$ are recorded beside their throughputs in the §10 mapping table and in every throughput table of §4. §4.5 quantifies the confound: the batch size *is* the prototype's device parallelism, tabulated as 64–14 336 active lanes against the control's 256 or 1024, with the probe cost that sets each batch and the probe-to-achieved ratio per path. The mappings are therefore comparable on the order axis, which is what §10's ordering rests on. |
| REQ-05 | §4.3 | **Satisfied.** `kernel_device_s` is its own device-event column on all four device paths; allocation, copy, and host serialisation are outside it, and the residual is derivable per cell. |
| REQ-06 | §4.3 | **Satisfied.** `h2d_device_s`, `d2h_device_s`, `host_submission_s`, and `device_submission_to_kernel_s` are four separate columns, and per-launch costs follow from them and `reps` without a second run. §4.3 states, with the `host_submission_s`/`eval_s` ratios as evidence, that the host-clock column measures an asynchronous submission on `gpu_hip` rows and a synchronous submit-and-complete on prototype rows, so that it is not misread as a prototype launch overhead. |
| REQ-07 | §7 | **Satisfied with two recorded gaps.** Registers per thread, scratch per thread, static LDS per block, and both spill counts are reported for all eight kernels beside their design predictions where a prediction exists. The gaps: the compiler's static-LDS field cannot observe the prototypes' $16n$-byte dynamic table, and no committed design states a per-lane register budget for `permanent_bipedal3_kernel`, `gray_update_micro_kernel`, or `horizontal_product_micro_kernel`. |
| REQ-08 | §8 | **Satisfied.** The limiting resource is named per kernel and shown to be constant in $n$, derived from the measured per-thread usage with two controls in the same receipt: the empty probe kernel fixes 16 waves/SIMD as the field's ceiling, and `permanent_bipedal7_kernel` at 128 VGPRs and occupancy 8 shows the field responds to pressure. The dynamic-LDS blind spot is stated rather than assumed away. |
| REQ-09 | §7.1 | **Satisfied.** The one prediction a measurement contradicts — the zero-allocation reading of the nine 32-bit-unit mapping model — is carried with its contradiction in the recording artifact's own words, and the 1040-byte scratch measurement is reported against the design narrative it qualifies. No prediction is silently restated. |
| REQ-10 | §9 | **Satisfied.** Both frequencies, both exact expectations, their complement relation, the sample count 4096, and Wilson 95 % intervals are reported at all five measured orders, with the interval method fixed deterministically. A second, larger branch observation from the same file's timed-operation counts is reported beside it with its own intervals and its dependence on the same purpose stream stated. |
| REQ-11 | §3 | **Satisfied.** `cpu_scalar` is the oracle and the other eight executing paths, both prototypes included, are re-confirmed identical against it on the campaign host, in the same run, before any timing cell, at every order the grid times. The $n = 24$ and $n = 28$ cells compare 32 and 4 matrices against 512 at the smaller orders, which is stated rather than presented as an equal gate. |
| REQ-12 | §5 | **Satisfied.** The one censored cell states its reason, its projection, and the measured rate the projection is scaled from, and carries `NaN` for both throughput columns; §4.3 withholds its kernel-only rate for the same reason. |
| REQ-13 | §2 | **Satisfied; vacuous for this field, and the reason matters.** No planned $\mathbb{F}_3$ candidate fails to execute on the target device: both compile clean for `gfx1030`, both appear in the resource receipt, both are equivalence-identical at every grid order, and both are `measured` in every grid cell. There is therefore no compile, correctness, or resource falsification to cite, and none is invented. The five candidates excluded from this field's record are planned for $\mathbb{F}_5$ and $\mathbb{F}_7$; four are rejected by field and the fifth by the structural absence of a full-permanent batch kernel, each quoted verbatim in §2. |
| REQ-14 | §4.4 | **Satisfied.** The best-performing prototype is `fold-gf3`; its best operating point is $n = 20$, $M = 448$, where it measures 69 485.2981 matrices/s against `cpu_rayon_intra_matrix` at 2 966.4439, a ratio of **23.4238×**, with a launch duration of **4.6442 µs per launch** on the device clock (`device_submission_to_kernel_s` 0.003511 s over 756 launches). §4.5 records that this operating point's batch is not matched to the control's. |
| REQ-15 | §1 | **Satisfied.** Four exact commands are committed with their revision, toolchain, and binary hashes; `analysis.py` regenerates every table here from the committed artifacts with one command; the run executes on the prepared benchmark host under the repository's full-host benchmark mutex, with a pristine-worktree refusal and a binary-hash verification enforced both before and after the lock is acquired. |

## 13. What this campaign does not establish

Collected so a reader does not have to reassemble it from the sections above.

1. **No prototype cell is measured at the control's batch sizes.** Every
   prototype cell sizes itself from a one-matrix probe, so the mapping
   comparison of §10 mixes the mapping effect with a device-parallelism effect
   that ranges over a factor of 224 across the prototype cells. Separating the
   two needs a run with the prototype batch sizes pinned to the control's.
2. **The $n = 20$ headline ratio is not decomposed.** `fold-gf3` at 23.4238×
   the best CPU path runs 14 336 lanes against the shipped path's 1 024. No cell
   in this run isolates how much of that ratio is the fold circuit and how much
   is the batch.
3. **The $16n$-byte shared column table is unmeasured.** The compiler reports
   static LDS only, so the study's central shared-memory prediction is neither
   confirmed nor refuted here, and no prototype cell in this run is large enough
   for shared-memory pressure to bind.
4. **No per-lane register budget is predicted for the shipped kernel.** REQ-07's
   prediction-beside-measurement pairing is one-sided for
   `permanent_bipedal3_kernel`, `gray_update_micro_kernel`, and
   `horizontal_product_micro_kernel`.
5. **The $n = 28$, $M = 1024$ ordering is open.** The cell is censored; the
   study's 0.984× figure for it is neither confirmed nor overturned.
6. **This field's Gray update has no device duration.** The device circuit is
   attempted at all five orders and censored at all five for a nonpositive
   barrier subtraction, so the only Gray-update duration in the record is the
   host one.
7. **The halving representation's horizontal product has no branch timing**, by
   construction rather than by omission, so the fold's zero-branch duration has
   no same-field counterpart to be compared against.
8. **The $n = 24$ and $n = 28$ equivalence cells are small.** Thirty-two and
   four matrices respectively, against 512 at the lower orders, so the two
   largest orders' timings rest on a weaker correctness gate than the rest.

Two observations in this run contradict a statement outside its own numbers and
are recorded rather than restated, per `@/inv/falsification-preserved`:

- The zero-allocation reading of the wave prototypes' nine 32-bit-unit mapping
  model, contradicted by the three-VGPR gap between the two folds (§7.1), in the
  recording artifact's own words.
- The grid preamble's projection-accuracy statement, which holds on the two
  `gpu_hip` chains it scopes itself to and fails on both prototype chains, where
  the projection lands high by 85 % and 684 % at $20 \rightarrow 24$ and by
  626 % and 524 % at $24 \rightarrow 28$ (§5). A censored prototype cell's
  projection would therefore not be conservative; this run has no such cell, and
  the observation is recorded before one appears.
