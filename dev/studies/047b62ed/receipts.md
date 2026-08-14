# $\mathbb{F}_3$ preregistered receipt campaign — committed receipts

Campaign run `20260813T230032Z-1321576`, field $q = 3$, grid execution id
`3002`. Every figure below is derived from the raw artifacts committed beside
this file; `analysis.py` in this directory regenerates all of them from those
CSVs and prints them under the section headings used here.

| Artifact | Role |
| --- | --- |
| [`…-q3-grid.csv`](permanent-campaign-20260813T230032Z-1321576-q3-grid.csv) / [`.log`](permanent-campaign-20260813T230032Z-1321576-q3-grid.log) | End-to-end Ryser timing grid, 75 cells |
| [`…-q3-gray-update.csv`](permanent-campaign-20260813T230032Z-1321576-q3-gray-update.csv) / [`.log`](permanent-campaign-20260813T230032Z-1321576-q3-gray-update.log) | Dependency-chained Gray-update isolate |
| [`…-q3-horizontal-product.csv`](permanent-campaign-20260813T230032Z-1321576-q3-horizontal-product.csv) / [`.log`](permanent-campaign-20260813T230032Z-1321576-q3-horizontal-product.log) | Horizontal-product isolate and zero/nonzero branch frequencies |
| [`…-shared-equivalence.csv`](permanent-campaign-20260813T230032Z-1321576-shared-equivalence.csv) / [`.log`](permanent-campaign-20260813T230032Z-1321576-shared-equivalence.log) | Backend-equivalence gate, all fields in one global run |
| [`…provenance.txt`](permanent-campaign-20260813T230032Z-1321576.provenance.txt) | Revision, toolchain, binary hashes, host inventory, exact commands |
| [`…run-summary.txt`](permanent-campaign-20260813T230032Z-1321576.run-summary.txt) | Per-step status and exit codes |
| [`../6c7fcb38/hip-resource-usage-20260813T193800Z-793245/`](../6c7fcb38/hip-resource-usage-20260813T193800Z-793245/) | Compiler kernel-resource receipt and per-kernel logs |
| [`analysis.py`](analysis.py) | Derivation of every table here from the CSVs above |

Each CSV carries a `#` preamble that is the authoritative source for the facts
it embeds. Where this document quotes a provenance fact, the citation names the
file the fact comes from rather than restating it as an independent claim.

## 1. Provenance and reproduction (REQ-02, REQ-15)

Source revision `afe4cafec88683df812fece597584318123ebdd0`; Rust toolchain
`1.95.0`, `rustc 1.95.0 (59807616e 2026-04-14)`; device compiler
`hipcc` reporting HIP `7.2.53211-9999` and AMD clang `22.0.0git`; ROCm runtime
`7.2.4` as recorded by the harness preamble. Host: AMD Ryzen 9 5900X 12-Core
Processor, 24 logical CPUs, AVX2 present and AVX-512F absent, governor
`powersave`; GPU `card0`, AMD Radeon RX 6950 XT, `gfx1030`, unique id
`0x8cd14d6d8a3c8a73`; kernel `7.1.6-arch1-1`. Harness binary SHA-256
`cb05d168b9c9f8ac28ad3d93fceafac8d27e3e0f48441c15a3fb6614ef0690e7`. Sources:
[`…provenance.txt`](permanent-campaign-20260813T230032Z-1321576.provenance.txt)
and the `#` preamble of each CSV.

The four commands that produced the four CSVs are recorded verbatim under
`exact_commands_executed:` in
[`…provenance.txt`](permanent-campaign-20260813T230032Z-1321576.provenance.txt)
and repeated in the `# invocation:` line of each CSV and the `# command:` line
of each log. All four steps report `status=completed exit=0` in
[`…run-summary.txt`](permanent-campaign-20260813T230032Z-1321576.run-summary.txt).

The campaign runs under
[`dev/scripts/permanent-campaign-runner.sh`](../../scripts/permanent-campaign-runner.sh),
scheduled as the one-shot `systemd-run --user --on-calendar='2026-08-14 02:00'`
unit documented in that script's header. It holds the repository's canonical
benchmark mutex `/tmp/gf2-ccx1.lock` through
[`dev/scripts/ccx1-bench-flock.sh --full-host`](../../scripts/ccx1-bench-flock.sh)
for the whole internal pipeline rather than once per step
(`permanent-campaign-runner.sh:586-594`), and `--full-host` deliberately omits
`taskset` because the grid's rayon cells are named on the full processor while
still sharing the one lock domain (`ccx1-bench-flock.sh:11-15`). `measure`
refuses a non-pristine tracked worktree twice: once before queueing for the
mutex and again inside the lock-held child before any step runs
(`permanent-campaign-runner.sh:628-635`, `:603-616`), so a commit or a rebuilt
binary landing during the lock wait cannot reach the campaign unchecked. The
grid preamble additionally records a 90 s full-rayon machine warm-up before the
first timed cell.

The `tracked_worktree_dirty: true` line in
[`…provenance.txt`](permanent-campaign-20260813T230032Z-1321576.provenance.txt)
is the state at the moment the provenance block is written, which is mid-run and
after the runner has already emitted its own outputs under `dev/studies/`. The
pristine condition that gates the run is the runner's own double refusal cited
above, not that line. The `ambient_rustc_command` block in the same file records
`rustc: command not found` with `command_exit_status: 127`: the systemd-timer
environment has no ambient `rustc` on `PATH`, which is why the CSV preambles
carry `rustc: unavailable` and `cargo: unavailable`. The toolchain actually used
is pinned by `rust_toolchain`, `build_rustc`, and the binary SHA-256 in the same
file, and the binary hash covers harness, path dependencies, toolchain, and
compiled feature set together.

Reproduction requires the exact commands as recorded, including `--only q=3`
and `--execution-id 3002`. A cell's seed index is
`execution_id * 22 500 000 + order_index * 100 000 + 1`
(`dev/research/permanent-sampling-feas/src/main.rs:310-330`, `:54`), and
`order_index` is the cell's position after the spec list is shuffled with
`SEED_ROOT` and stably sorted by ascending `n`
(`dev/research/permanent-sampling-feas/src/main.rs:476-481`). The shuffle runs
over the filtered list, so the same execution id with a different `--only`
filter addresses different matrices.

## 2. Candidate roster and what executes (REQ-01, REQ-13)

The grid enumerates the full planned candidate set from the prototype registry
rather than a pre-filtered one; every `Backend::ALL` entry appears at every
order (`dev/research/permanent-sampling-feas/src/main.rs:250-261`). The file
holds 75 cells: 15 per order, of which 39 are `measured`, 1 is `censored`, and
35 are `unsupported`. Every non-`measured` cell carries its reason in `note`.

| Path | Class | Grid outcome at every order | Reason recorded in `note` |
| --- | --- | --- | --- |
| `gpu_hip` (`permanent_bipedal3_kernel`) | current GPU path | `measured` at $M \in \{256, 1024\}$, except $n = 28$, $M = 1024$ | see §5 |
| `cpu_scalar` (`permanent_bipedal3_singleword`) | in-tree CPU | `measured` | — |
| `cpu_avx2` (`permanent_bipedal3_singleword_simd`) | in-tree CPU | `measured` | — |
| `cpu_rayon_batch_scalar` | in-tree CPU | `measured` | — |
| `cpu_rayon_batch_avx2` | in-tree CPU | `measured` | — |
| `cpu_rayon_intra_matrix` | in-tree CPU | `measured` | — |
| `cpu_ryser_generic` | in-tree CPU | `measured` | — |
| `wave-gf3` (`wave_gf3_kernel<kHalving>`) | planned $\mathbb{F}_3$ prototype | `unsupported` | `prototype candidate wave-gf3 has no harness batch evaluator yet` |
| `fold-gf3` (`wave_gf3_kernel<kZeroMaskSignPopcount>`) | planned $\mathbb{F}_3$ prototype | `unsupported` | `prototype candidate fold-gf3 has no harness batch evaluator yet` |
| `f5-byte-control`, `f5-three-plane` | planned $\mathbb{F}_5$ prototypes | `unsupported` | same string; also out of field |
| `f7-lookup-table-control`, `f7-three-plane-accumulator`, `f7-three-plane-permanent` | planned $\mathbb{F}_7$ prototypes | `unsupported` | same string; also out of field |

The kernel each CPU row forces is recorded in
[`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.2: `permanent_bipedal3` dispatches internally, so the grid never calls it and
instead calls `permanent_bipedal3_singleword` for `cpu_scalar` and
`permanent_bipedal3_singleword_simd` for `cpu_avx2` with an explicit selection.

The exclusion reason is identical for all seven prototypes and is a harness
integration state, not a device falsification. It is emitted by the
`Backend::Prototype(path)` arm of `support`
(`dev/research/permanent-sampling-feas/src/backend.rs:315-319`), reached from
`grid` through `run_cell` (`dev/research/permanent-sampling-feas/src/protocol.rs:627-630`)
and from `equivalence` through its own support match
(`dev/research/permanent-sampling-feas/src/equivalence.rs:174-177`); it fires
because every prototype's `dispatch()` returns `Ok(())` without launching a
kernel (`dev/research/permanent_wave_gpu/src/paths.rs:82-85`,
`dev/research/permanent_wave_gpu/src/wave.rs:127-129`).

**This is the campaign's central negative result, and it is not a REQ-13
exclusion.** REQ-13 covers a candidate that *cannot* execute on the target
device and asks for its compile, correctness, or resource falsification. The
two $\mathbb{F}_3$ prototypes do execute on this device: they compile clean for
`gfx1030` and their kernels appear in the compiler resource receipt (§7), and
the paired-fold clean device receipt in
[`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md)
records both `wave-gf3` and `fold-gf3` matching all 12 canonical $\mathbb{F}_3$
fixtures through order 16 on this same host and GPU. That receipt is a separate
run at revision `682ef5787f5a3792872fb94441bf66e5c38c866c`, not part of this
campaign, and it is correctness evidence rather than timing. Their absence from this
campaign's timing comparison is that the harness has no batch evaluator wired to
them, so the run has no falsification to cite and none is invented. §12 records
the consequence for REQ-01, REQ-04, and REQ-14.

The three $\mathbb{F}_7$ and two $\mathbb{F}_5$ prototypes carry a second,
field-level exclusion visible in the horizontal-product isolate, where each is
rejected by circuit rather than by harness state — for example
`unsupported: f5-three-plane uses the f5-three-plane-c4 horizontal-product
circuit over F_5; not F_3`
([`…-q3-horizontal-product.csv`](permanent-campaign-20260813T230032Z-1321576-q3-horizontal-product.csv),
`note` column). They are out of this field's scope and their exclusion from the
$\mathbb{F}_3$ timing comparison cites that circuit mismatch.

## 3. Equivalence gate (REQ-11)

The equivalence step is the first step of the pipeline
([`…run-summary.txt`](permanent-campaign-20260813T230032Z-1321576.run-summary.txt),
`timestamp_utc: 2026-08-13T23:00:34Z` in the equivalence CSV against
`23:03:28Z` in the grid CSV), so it precedes every timing cell of the same run
on the same host and the same binary hash.

The harness `equivalence` subcommand is global: it takes no field filter, and
`--execution-id` is parsed by `grid` alone. The run summary therefore records
`execution_id=fixed-streams` for this step, which is truthful — the step draws
from fixed stream addresses of the form `(seed_root, purpose, index)` recorded
in its own preamble, not from a reserved per-execution index block
(`permanent-campaign-runner.sh:61-69`). One global run gates all three fields;
this document reads its $q = 3$ rows.

Protocol: 512 matrices per cell at `seed_root 0xb488f02c00000001`, purpose
`equivalence`, index 0, at $n \in \{8, 12, 16, 20\}$, with the scalar
single-word kernel as the reference. Every backend at a given $(q, n)$ is
compared against the *same* 512 matrices: the batch is built once and reused
across the whole backend loop
(`dev/research/permanent-sampling-feas/src/equivalence.rs:139`, `:157`).

| $n$ | Backends compared against `cpu_scalar` | `matrices` | `mismatches` | `zeros_reference` = `zeros_backend` | `status` |
| ---: | --- | ---: | ---: | ---: | --- |
| 8 | `cpu_avx2`, `cpu_rayon_batch_scalar`, `cpu_rayon_batch_avx2`, `cpu_rayon_intra_matrix`, `gpu_hip`, `cpu_ryser_generic` | 512 | 0 | 169 | `identical` |
| 12 | same six | 512 | 0 | 165 | `identical` |
| 16 | same six | 512 | 0 | 157 | `identical` |
| 20 | same six | 512 | 0 | 162 | `identical` |

All 24 $q = 3$ comparison cells report `mismatches = 0` and
`status = identical`, and the log closes with `all backends agree per matrix`.
The seven prototype rows report `matrices = 0` with the harness-state reason of
§2 and are not equivalence evidence in either direction.

Two limits on this gate, stated rather than inferred away. First, its order set
stops at $n = 20$ (`# sizes:` line of the equivalence preamble) while the grid
measures $n \in \{12, 16, 20, 24, 28\}$, so the $n = 24$ and $n = 28$ timings
rest on backend-level equivalence confirmed at smaller orders, not on an
order-matched cell. Second, no prototype path is equivalence-confirmed by this
run, which is the same harness gap as §2.

## 4. End-to-end Ryser timing grid (REQ-01, REQ-02, REQ-05, REQ-06)

### 4.1 Recorded fields

REQ-02 asks for a specific field set. Each is a named column of
[`…-q3-grid.csv`](permanent-campaign-20260813T230032Z-1321576-q3-grid.csv), or a
line of the provenance file for the facts that are constant across the run.

| REQ-02 item | Where recorded |
| --- | --- |
| matrix order | `n` |
| batch size | `batch_size` |
| seed | `seed_root` and `seed_index_first` (with `timed_index_first` for the timed draw) |
| purpose tag | `timed_purpose` (`grid_timed`; warm-up uses `grid_warmup` and the sizing probe `grid_probe`, disjoint tag domains — `dev/research/permanent-sampling-feas/src/sampler.rs:208`) |
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

### 4.2 Composite throughput and the best path per order

Composite matrices per second, the rate the study's envelope is derived from.
Full grid in the CSV; `analysis.py` section 4 prints every cell.

| $n$ | best path overall | rate | best in-tree CPU path | rate | best GPU config | rate | GPU / best CPU |
| ---: | --- | ---: | --- | ---: | --- | ---: | ---: |
| 12 | `cpu_rayon_batch_scalar` | 303 392.3924 | `cpu_rayon_batch_scalar` | 303 392.3924 | `gpu_hip` $M{=}1024$ | 217 172.3805 | 0.7158 |
| 16 | `gpu_hip` $M{=}1024$ | 58 228.0963 | `cpu_rayon_batch_scalar` | 37 003.5993 | `gpu_hip` $M{=}1024$ | 58 228.0963 | 1.5736 |
| 20 | `gpu_hip` $M{=}1024$ | 4 867.0949 | `cpu_rayon_intra_matrix` | 2 928.4149 | `gpu_hip` $M{=}1024$ | 4 867.0949 | 1.6620 |
| 24 | `gpu_hip` $M{=}1024$ | 312.6832 | `cpu_rayon_intra_matrix` | 291.8001 | `gpu_hip` $M{=}1024$ | 312.6832 | 1.0716 |
| 28 | `cpu_rayon_intra_matrix` | 19.1403 | `cpu_rayon_intra_matrix` | 19.1403 | `gpu_hip` $M{=}256$ | 8.5515 | 0.4468 |

The best applicable in-tree CPU path is identified from this run's own data
rather than assumed: batch rayon over the scalar kernel leads the CPU field at
$n \in \{12, 16\}$, and intra-matrix rayon leads it at $n \in \{20, 24, 28\}$.
At $n = 28$ the best GPU configuration is $M = 256$ only because $M = 1024$ is
censored (§5); no rate is substituted for it.

**One identical matrix corpus per comparison holds for the equivalence
comparison and does not hold for the timing comparison.** A matrix is fully
determined by `(seed_root, q, n, purpose, stream_index)`
(`dev/research/permanent-sampling-feas/src/sampler.rs:232`, `:295`) and the
draw is backend-independent — "*Both forms draw the same entries from the same
stream, so a cell's sample does not depend on which backend measures it*"
(`dev/research/permanent-sampling-feas/src/protocol.rs:362-363`) — so every path in
the grid samples the same distribution through the same exact-rejection sampler
at the same $(q, n)$. But each grid cell reserves its own disjoint block of
100 000 stream indices keyed on its shuffled `order_index`
(`dev/research/permanent-sampling-feas/src/main.rs:54`, `:310-330`, `:476-481`),
so two backends at one $(q, n)$ consume different index ranges and hence
different matrices; the harness relies on exactly this to call cross-backend
zero counts independent samples
(`dev/research/permanent-sampling-feas/src/main.rs:938-945`). The identical-corpus
condition is met by the equivalence run of §3, where one 512-matrix batch is
built once and reused for every backend. §12 records this against REQ-01.

### 4.3 Device phase separation (REQ-05, REQ-06)

`kernel_device_s` is a device-event kernel-only total. `h2d_device_s`,
`d2h_device_s`, and `device_submission_to_kernel_s` are separate device-event
totals; `host_submission_s` is a host-clock total of the submission wrapper
call. No column subtracts a host timestamp from a device one, and CPU rows leave
all five empty with `phase_timing_note = event timing unavailable: backend is
not GPU/HIP` rather than substituting an evaluator wall clock
(`dev/research/permanent-sampling-feas/src/protocol.rs:20-28`,
`dev/research/permanent-sampling-feas/src/backend.rs:30-48`).

Totals per cell, in seconds. `residual` is
`eval_s − kernel_device_s − h2d_device_s − d2h_device_s − device_submission_to_kernel_s`,
the host-side allocation, serialization, stream wait, and free that the
dispatcher performs around each call.

| $n$ | $M$ | outcome | `eval_s` | `kernel_device_s` | `h2d_device_s` | `d2h_device_s` | `host_submission_s` | `device_submission_to_kernel_s` | residual | kernel / eval |
| ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 256 | measured | 3.945126 | 1.126798 | 0.029711 | 0.021944 | 0.007433 | 0.011191 | 2.755482 | 0.2856 |
| 12 | 1024 | measured | 3.190467 | 1.329111 | 0.037525 | 0.009245 | 0.003347 | 0.009775 | 1.804811 | 0.4166 |
| 16 | 256 | measured | 4.627724 | 3.909794 | 0.007655 | 0.005037 | 0.001714 | 0.002563 | 0.702675 | 0.8449 |
| 16 | 1024 | measured | 4.217821 | 3.574263 | 0.005743 | 0.002607 | 0.000898 | 0.001299 | 0.633909 | 0.8474 |
| 20 | 256 | measured | 5.026229 | 4.945205 | 0.000688 | 0.000411 | 0.000144 | 0.000204 | 0.079721 | 0.9839 |
| 20 | 1024 | measured | 4.959187 | 4.881862 | 0.000621 | 0.000226 | 0.000087 | 0.000111 | 0.076367 | 0.9844 |
| 24 | 256 | measured | 9.325287 | 9.311242 | 0.000096 | 0.000049 | 0.000024 | 0.000024 | 0.013876 | 0.9985 |
| 24 | 1024 | measured | 16.348135 | 16.324992 | 0.000211 | 0.000050 | 0.000034 | 0.000024 | 0.022858 | 0.9986 |
| 28 | 256 | measured | 149.673203 | 149.657658 | 0.000109 | 0.000049 | 0.000043 | 0.000024 | 0.015363 | 0.9999 |
| 28 | 1024 | censored | 158.285782 | 158.266688 | 0.000161 | 0.000030 | 0.000025 | 0.000014 | 0.018889 | 0.9999 |

Kernel-only throughput against end-to-end throughput. Kernel-only is
`matrices` / `kernel_device_s`, so it charges the device kernel span alone;
`eval_matrices_per_s` charges the whole dispatch; `composite_matrices_per_s`
charges generation, evaluation, reduction, and store. The censored cell carries
no throughput value in any of the three.

| $n$ | $M$ | outcome | `matrices` | kernel-only matrices/s | eval-only matrices/s | composite matrices/s | kernel-only / composite |
| ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 12 | 256 | measured | 590 848 | 524 360.1781 | 149 766.5682 | 121 487.1337 | 4.3162 |
| 12 | 1024 | measured | 1 035 264 | 778 914.6279 | 324 486.6204 | 217 172.3805 | 3.5866 |
| 16 | 256 | measured | 135 424 | 34 637.1190 | 29 263.6317 | 27 277.6466 | 1.2698 |
| 16 | 1024 | measured | 286 720 | 80 217.9358 | 67 978.2305 | 58 228.0963 | 1.3776 |
| 20 | 256 | measured | 10 752 | 2 174.2274 | 2 139.1782 | 2 122.2775 | 1.0245 |
| 20 | 1024 | measured | 24 576 | 5 034.1448 | 4 955.6507 | 4 867.0949 | 1.0343 |
| 24 | 256 | measured | 1 280 | 137.4682 | 137.2612 | 137.1639 | 1.0022 |
| 24 | 1024 | measured | 5 120 | 313.6296 | 313.1856 | 312.6832 | 1.0030 |
| 28 | 256 | measured | 1 280 | 8.5529 | 8.5520 | 8.5515 | 1.0002 |
| 28 | 1024 | censored | 3 072 | withheld | `NaN` | `NaN` | — |

The kernel-only-to-composite ratio is the size of everything the device kernel
does not pay for. It is 4.32× at $n = 12$, $M = 256$ and 1.0002× at $n = 28$,
$M = 256$: the shipped dispatcher's surrounding work costs three quarters of the
achievable rate at the smallest order and is invisible at the largest.

Per-launch cost, dividing each total by that cell's `reps`. Each timed
repetition opens one sampler at one stream index and evaluates one batch
(`dev/research/permanent-sampling-feas/src/protocol.rs:764-772`), and one batch
evaluation is one dispatch of `gridDim.x = M` blocks
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:335`, `:347-351`),
so `reps` is the launch count.

| $n$ | $M$ | `reps` | host submission µs/launch | device submission→kernel µs/launch | kernel ms/launch | H2D µs/launch | D2H µs/launch |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 | 256 | 2308 | 3.2205 | 4.8488 | 0.4882 | 12.8731 | 9.5078 |
| 12 | 1024 | 1011 | 3.3106 | 9.6686 | 1.3146 | 37.1167 | 9.1444 |
| 16 | 256 | 529 | 3.2401 | 4.8450 | 7.3909 | 14.4707 | 9.5217 |
| 16 | 1024 | 280 | 3.2071 | 4.6393 | 12.7652 | 20.5107 | 9.3107 |
| 20 | 256 | 42 | 3.4286 | 4.8571 | 117.7430 | 16.3810 | 9.7857 |
| 20 | 1024 | 24 | 3.6250 | 4.6250 | 203.4109 | 25.8750 | 9.4167 |
| 24 | 256 | 5 | 4.8000 | 4.8000 | 1862.2484 | 19.2000 | 9.8000 |
| 24 | 1024 | 5 | 6.8000 | 4.8000 | 3264.9984 | 42.2000 | 10.0000 |
| 28 | 256 | 5 | 8.6000 | 4.8000 | 29931.5316 | 21.8000 | 9.8000 |
| 28 | 1024 | 3 | 8.3333 | 4.6667 | 52755.5627 | 53.6667 | 10.0000 |

What the separation shows. Launch cost is nearly constant in $n$ and $M$: host
submission stays between 3.2 and 8.6 µs and device submission-to-kernel between
4.6 and 9.7 µs per launch across a kernel span that grows by five orders of
magnitude, from 0.49 ms to 52.8 s. Device-to-host copy is flat at 9.1–10.0 µs
per launch because each launch returns $M$ eight-byte permanents at most. What
does move is the host-side residual: it is 70 % of `eval_s` at $n = 12$,
$M = 256$ and 15 % at $M = 1024$, falls to 1.6 % at $n = 20$, and is under
0.2 % from $n = 24$ up. The dispatcher's per-call allocation, serialization,
and free — not transfer and not launch — is what costs the GPU the $n = 12$
cell, and it is attributable from these columns without a second run.

### 4.4 Best operating point of the shipped GPU path (REQ-14, in part)

Of the measured configurations, `gpu_hip` at $n = 20$, $M = 1024$ has the
highest ratio against the best applicable in-tree CPU path: 4 867.0949 against
`cpu_rayon_intra_matrix` at 2 928.4149, a factor of **1.6620**. At that point
the launch duration is `host_submission_s` 0.000087 s over 24 launches, or
**3.625 µs per launch** on the host clock, and `device_submission_to_kernel_s`
0.000111 s over 24 launches, or **4.625 µs per launch** on the device clock.
This is the shipped path's figure. REQ-14 asks for the best-performing
*prototype*; §12 records that no prototype carries a throughput in this run.

## 5. Censoring (REQ-12)

One cell in the $q = 3$ grid is censored.

- Cell: $q = 3$, $n = 28$, `gpu_hip`, `batch_size` 1024, `order_index` 64.
- `outcome`: `censored`.
- `composite_matrices_per_s`: `NaN`. `eval_matrices_per_s`: `NaN`. No measured
  throughput value is carried.
- Censoring reason, verbatim from the `note` column: `unavailable: 120 s cap
  ended timing before both minimums (5 repetitions and 5 s); no derived rate is
  reported`. The cell completed 3 repetitions of 52.72–52.81 s each in 158.31 s
  of total time; the protocol requires at least 5 repetitions as well as at
  least 5 s of timed work, and the 120 s cap censors rather than truncates, so
  the third repetition finished and the fourth was not started.
- Projection: `projected_matrices_per_s` 16.750887, from
  `projection_reference_n` 24. The rate it is projected from is that cell's own
  chain, `gpu_hip` at $M = 1024$, $n = 24$, measured at 312.6832 matrices/s,
  rescaled through Ryser's $n \cdot 2^n$ work model
  ($312.6832 \times 24 \cdot 2^{24} / (28 \cdot 2^{28}) = 16.750886$, matching
  the CSV to seven significant figures).

The projection is an estimate and is labelled as one in the CSV preamble. This
run's own $q = 3$ GPU chain measures the direction and size of its bias where
both ends of a step are measured:

| $M$ | step | projection | measured | error |
| ---: | --- | ---: | ---: | ---: |
| 256 | $12 \rightarrow 16$ | 5 694.7094 | 27 277.6466 | $-79.1\%$ |
| 256 | $16 \rightarrow 20$ | 1 363.8823 | 2 122.2775 | $-35.7\%$ |
| 256 | $20 \rightarrow 24$ | 110.5353 | 137.1639 | $-19.4\%$ |
| 256 | $24 \rightarrow 28$ | 7.3481 | 8.5515 | $-14.1\%$ |
| 1024 | $12 \rightarrow 16$ | 10 179.9553 | 58 228.0963 | $-82.5\%$ |
| 1024 | $16 \rightarrow 20$ | 2 911.4048 | 4 867.0949 | $-40.2\%$ |
| 1024 | $20 \rightarrow 24$ | 253.4945 | 312.6832 | $-18.9\%$ |

Every step runs low and the magnitude shrinks monotonically with $n$, which is
what the launch-amortisation mechanism the preamble names predicts. The step
that governs the censored cell is $24 \rightarrow 28$, measured on the $M = 256$
chain of this same file at $-14.1\%$. Carrying that factor across gives roughly
19.5 matrices/s as the cell's expected true rate. That figure is an
extrapolation from a neighbouring batch size, not a measurement; the censored
cell carries no throughput here and none is asserted for it. It is stated
because it is the quantity that decides the $n = 28$ ordering, and §11 records
what follows.

No $q = 3$ cell reports a build failure, a correctness failure, or a device
resource exhaustion. The 35 non-`measured`, non-`censored` cells are the
`unsupported` prototype rows of §2.

## 6. Component isolation (REQ-03)

REQ-03 asks that the dependency-chained Gray update, the horizontal product,
and the end-to-end Ryser loop each be isolated for every representation of this
field that executes. The end-to-end Ryser loop is §4. The two isolates run at
one order each: both subcommands take a single `--n` defaulting to 12 and loop
over $q$ and backend only
(`dev/research/permanent-sampling-feas/src/main.rs:98-106`, `:132-136`,
`:165-173`, `:199-207`), and the committed commands pass no `--n` override
(`…provenance.txt`, `exact_commands_executed:`). Both isolates therefore
describe $n = 12$.

### 6.1 Dependency-chained Gray update

Shape: one accumulator reads its immediately preceding add/subtract result — a
latency chain, not independent-operation throughput. Net duration is
`(Σ update spans − Σ same-geometry compiler-barrier spans) / (steps × reps)`,
with CPU rows using paired host spans and HIP rows paired device-event kernel
spans (preamble of
[`…-q3-gray-update.csv`](permanent-campaign-20260813T230032Z-1321576-q3-gray-update.csv)).

| backend | outcome | `steps` | `reps` | `update_s` | `compiler_barrier_baseline_s` | `net_per_operation_s` | `duration_basis` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `cpu_scalar` | measured | 1 000 001 | 4355 | 3.177416956 | 1.815668271 | 3.13e-10 | `host_clock_update_chain` |
| `gpu_hip` | censored | 1 000 001 | 68 | 2.237469662 | 2.702262588 | absent | `device_event_kernel` |
| `wave-gf3` | censored | 1 000 001 | 68 | 2.237814635 | 2.702688985 | absent | `device_event_kernel` |
| `fold-gf3` | censored | 1 000 001 | 68 | 2.237874021 | 2.702727775 | absent | `device_event_kernel` |
| `cpu_avx2`, `cpu_rayon_batch_scalar`, `cpu_rayon_batch_avx2`, `cpu_rayon_intra_matrix`, `cpu_ryser_generic` | unsupported | 1 000 001 | 0 | — | — | absent | `unavailable` |

Censoring reason for the three device rows, verbatim: `unavailable: paired
update span … minus compiler-barrier baseline … is nonpositive; no net
per-operation duration is reported`. The Bipedal3 two-plane add/subtract is
cheap enough on this device that the chain runs *faster* than the
same-geometry barrier baseline it is measured against, by roughly 0.465 s over
68 × 1 000 001 operations. The harness reports no rate rather than a clamped or
negative one, and no false positive rate is substituted. The five CPU rows carry
`unsupported: <backend> has no isolated dependency-chained Gray-update
evaluator`, so only one CPU representation is isolated at all.

**The three device rows measure one kernel, not three.** The `wave-gf3` and
`fold-gf3` arms of the Gray-update dispatcher call the same `gpu_repetition`
helper as `Backend::Gpu`
(`dev/research/permanent-sampling-feas/src/gray_update.rs:310-315`), whose
device entry point selects its field from `q` alone and never from the backend
(`dev/research/permanent-sampling-feas/src/gray_update.rs:419-425`); the launch
is `gray_update_micro_kernel`
(`crates/gf2-kernels-hip/hip/permanent/gray_update_micro.hip:44`) in every case.
The three spans agree to within 0.02 %, which is the expected signature of one
kernel measured three times. These rows are not evidence about the wave
prototypes' Gray update, and this receipt does not present them as such.

### 6.2 Horizontal product

Shape: each timed sample is one unconditioned row-sum vector. The zero branch
times the representation's early product exit; the nonzero branch times zero
detection plus the complete representation-specific reduction. Timing is paired
device-event kernel spans only, excluding allocation, upload, download,
submission, sampling, grouping, and host policy.

| backend | outcome | `reps` | `zero_fast_s` | baseline | `zero_fast_net_per_operation_s` | `zero_fast_timed_operations` | `nonzero_slow_s` | baseline | `nonzero_slow_net_per_operation_s` | `nonzero_slow_timed_operations` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| `fold-gf3` | measured | 1794 | 0.016821713 | 0.016629419 | 2.6e-11 | 7 291 683 | 0.015356805 | 0.015587296 | absent | 56 541 |

`fold-gf3` is the one measured row. Its circuit is
`HorizontalProductCircuit::Bipedal3ZeroMaskSignPopcount`
(`dev/research/permanent-sampling-feas/src/horizontal_product.rs:496`), which
enters `horizontal_product_micro_kernel` and dispatches to
`bipedal3_zero_mask_sign_popcount_product`
(`crates/gf2-kernels-hip/hip/permanent/horizontal_product_micro.hip:141-143`).
This is a genuinely distinct circuit from the shipped one, and it is the
zero-mask/sign-popcount representation the `fold-gf3` candidate is named for —
it is not, however, the prototype kernel
`wave_gf3_kernel<FoldKind::kZeroMaskSignPopcount>`, which the harness never
launches (`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:353` is
its only launch site, inside the prototype crate's own executable).

Its nonzero-slow branch is censored: `nonzero slow timing unavailable: raw
device span minus its same-geometry baseline was nonpositive; so no false
positive rate is reported`. Only the zero-fast branch carries a duration.

`wave-gf3` and `gpu_hip` both select `Bipedal3Halving`
(`dev/research/permanent-sampling-feas/src/horizontal_product.rs:486`, `:493`)
and are `unsupported` with the reason `gpu_hip`/`wave-gf3` `uses the
bipedal3-halving reduction, whose zero result is observed only after its
complete reduction; emitting separate branch timings would invent a different
circuit`. The halving reduction has no observable branch boundary
(`crates/gf2-kernels-hip/src/permanent/mod.rs:1387-1388`), so the row returns
before any launch rather than reporting a branch split the circuit does not
have. Every CPU row is `unsupported: <backend> has no distinct device-event
horizontal-product isolate; no generic or host-clock replacement was used`.

So of the $\mathbb{F}_3$ representations, exactly one — the zero-mask/sign-popcount
fold — yields an isolated horizontal-product duration, and only on its zero
branch. The bipedal3-halving representation yields none by construction, and
that construction is recorded as the reason rather than worked around.

## 7. Kernel resources against design-predicted budgets (REQ-07, REQ-09)

Source: the compiler kernel-resource receipt
[`../6c7fcb38/hip-resource-usage-20260813T193800Z-793245/receipt.txt`](../6c7fcb38/hip-resource-usage-20260813T193800Z-793245/receipt.txt)
at `source_revision ed51847ad5364fa15d5fc27b81a04b993deb8ad3`, architecture
`gfx1030`, flag `-Rpass-analysis=kernel-resource-usage`, every entry
`exit_status: 0` with source, object, and log SHA-256. The per-kernel logs
carry the remarks quoted below verbatim.

The device reports registers in two files: `TotalSGPRs` are scalar registers
shared by a wave, `VGPRs` are vector registers held per lane, so `VGPRs` is the
per-thread register figure and `TotalSGPRs` the per-wave one. `ScratchSize
[bytes/lane]` is private scratch memory per thread. `LDS Size [bytes/block]` is
the compiler's *static* LDS field and does not include memory requested
dynamically at launch. The `log` column cites the first line of that kernel's
remark block within the named log in the receipt directory.

| Kernel | Path | `TotalSGPRs` | `VGPRs` | scratch B/lane | SGPR spill | VGPR spill | static LDS B/block | occupancy waves/SIMD | log |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `permanent_bipedal3_kernel` | `gpu_hip` end-to-end | 27 | 19 | 1040 | 0 | 0 | 0 | 16 | `permanent_bipedal3.hip.resource.log:1` |
| `wave_gf3_kernel<FoldKindE0>` | `wave-gf3` prototype | 22 | 22 | 0 | 0 | 0 | 0 | 16 | `wave_gf3_equivalence.hip.resource.log:23` |
| `wave_gf3_kernel<FoldKindE1>` | `fold-gf3` prototype | 22 | 25 | 0 | 0 | 0 | 0 | 16 | `wave_gf3_equivalence.hip.resource.log:34` |
| `gray_update_micro_kernel` | Gray-update isolate | 86 | 5 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:1` |
| `gray_update_compiler_barrier_baseline_kernel` | its paired baseline | 13 | 2 | 0 | 0 | 0 | 0 | 16 | `gray_update_micro.hip.resource.log:12` |
| `horizontal_product_micro_kernel` | horizontal-product isolate | 26 | 3 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:24` |
| `horizontal_product_compiler_barrier_baseline_kernel` | its paired baseline | 14 | 2 | 0 | 0 | 0 | 0 | 16 | `permanent_bipedal7.hip.resource.log:35` |
| `permanent_wave_gpu_probe` | empty control kernel | 0 | 0 | 0 | 0 | 0 | 0 | 16 | `probe.hip.resource.log:1` |

The same log also carries four probe kernels of the wave prototype that no
timing in this campaign exercises: `n63_mapping_probe` at 7 SGPRs and 8 VGPRs
(`wave_gf3_equivalence.hip.resource.log:1`), `n4_direction_probe` at 9 and 9
(`:12`), and both `active_mask_product_probe` specializations at 7 and 2
(`:43`, `:54`), each with zero scratch, zero spills, zero static LDS, and
occupancy 16.

`FoldKindE0` is the halving control and `FoldKindE1` the zero-mask/sign-popcount
candidate, in source declaration order
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:67`, `:315-316`).
The two horizontal-product kernels appear in the `permanent_bipedal7.hip` log
because `horizontal_product_micro.hip` has no translation unit of its own: it is
included textually so the $\mathbb{F}_7$ lookup circuit reads that unit's
`__constant__ d_MUL_LUT`, and compiling it alone fails on that undeclared
symbol. The receipt records it as `translation_unit_role: included-fragment`
against the `permanent_bipedal7.hip` log, and the runner enumerates the
fragment relation explicitly (`permanent-campaign-runner.sh:50-59`).

### 7.1 Predicted budgets and what the device reports

**Lane-owns-interval prototypes (`wave-gf3`, `fold-gf3`).** The design predicts
a per-lane register budget and a per-block shared-memory allocation, both
stated in
[`dev/research/permanent_wave_gpu/README.md`](../../research/permanent_wave_gpu/README.md):
the state model is "two packed `u64` words plus `u64` Gray cursor/end bounds and
one `u32` partial sum: a 9 x 32-bit-register lower bound", and "the launch still
reserves the source-declared dynamic column table of $16n$ bytes per block,
which this static-LDS report does not include". The launch confirms the second:
`2 * n * sizeof(std::uint64_t)` shared bytes for both specializations
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:353-361`), backing
`extern __shared__ std::uint64_t staged_columns[]` at `:171`.

| Kernel | predicted per-lane registers | measured per-lane (`VGPRs`) | measured per-wave (`TotalSGPRs`) | predicted per-block shared | measured static LDS |
| --- | --- | ---: | ---: | --- | --- |
| `wave_gf3_kernel<FoldKindE0>` | $\ge 9 \times$ 32-bit | 22 | 22 | $16n$ B | 0 B (static field only) |
| `wave_gf3_kernel<FoldKindE1>` | $\ge 9 \times$ 32-bit | 25 | 22 | $16n$ B | 0 B (static field only) |

The register prediction is a per-lane lower bound and the per-lane measurement
respects it, at 2.44× and 2.78× the bound. The README records the falsification
this produces, and this receipt quotes it rather than paraphrasing: *"The resource
comparison falsifies a zero-allocation interpretation of the nine 32-bit-unit
source-level mapping model: neither fold adds persistent mapping state, but the
candidate's local zero-mask/popcount work costs three additional VGPRs."* The
three-VGPR gap between the two specializations reproduces exactly in this
campaign's receipt (22 against 25).

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
`(sum_m, sum_s)` accumulator in $O(1)$ work per Gray step
([`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.3) — that claim is about the accumulator, which the measurement leaves
intact at 19 VGPRs with zero spills. It does locate, in measured bytes, the
allocation the lane-owns-interval design proposes to relocate: the shipped
mapping holds the column table in per-thread private memory backed by device
memory, and the prototype mapping holds a $16n$-byte column table in per-block
shared memory. At $n = 28$ those are 1024 bytes per lane against 448 bytes per
block.

**Gray-update isolate.** `gray_update_micro_kernel` reports 86 `TotalSGPRs`
against 5 `VGPRs` — the most scalar-register-heavy kernel in the receipt and the
only one above 30. No design document predicts a budget for it. Its paired
barrier baseline reports 13 and 2. That the baseline is far cheaper in registers
while measuring *longer* on the device (§6.1) is recorded as the observation it
is, and is why that isolate reports no net rate.

No kernel in this receipt reports a nonzero SGPR or VGPR spill count, and no
measurement in this campaign contradicts a numeric register or shared-memory
prediction that a committed design states. The one prediction that a
measurement does contradict — the zero-allocation reading of the nine-unit model
— is carried above with its contradiction, in the words of the artifact that
recorded it.

## 8. What limits occupancy (REQ-08)

The compiler reports `Occupancy [waves/SIMD]: 16` for every kernel in the
receipt. The evidence that 16 is the architectural ceiling on `gfx1030` and not
a register-derived figure is in the receipt itself:
`permanent_wave_gpu_probe`, an empty kernel with 0 `TotalSGPRs`, 0 `VGPRs`,
0 scratch, and 0 LDS (`probe.hip.resource.log:1`), reports the same 16. A kernel
that consumes no registers at all cannot be register-limited, so 16 is the
saturation value of that field. Every $\mathbb{F}_3$ kernel measured here
reaches it.

Derived from the measured per-thread resource usage, therefore: **no measured
$\mathbb{F}_3$ kernel is limited by registers, private scratch, or static LDS,
at any measured order.** The supporting facts are that each reports occupancy at
the ceiling, each reports zero SGPR and VGPR spills, and none reports a static
LDS allocation.

The resource figures do not vary with $n$, so the same statement holds at every
measured order without a per-order table. The evidence is that $n$ reaches these
kernels as a runtime argument, not a template parameter: the mangled names are
`_Z25permanent_bipedal3_kernelPKhiiPy` and
`_ZN12_GLOBAL__N_115wave_gf3_kernelILNS_8FoldKindE0EEEvPKhiiPj`, whose only
template parameter is `FoldKind`, and the compiler emits one resource block per
kernel rather than one per order.

What does bound the shipped path's device parallelism at each measured order is
its launch geometry, which is fixed by the kernel's own contract rather than by
its resource usage: `gridDim.x = M` blocks, `dim3 block(1, 1, 1)`, one matrix per
block, and only thread 0 doing work
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:160-175`, `:335`,
`:347-351`). The resident wave count therefore cannot exceed $M$, and each
resident wave carries one active lane. At the measured batch sizes that is 256
or 1024 single-lane waves for the whole device, against a per-SIMD capacity of
16 waves that no measured kernel's register footprint reduces. The
lane-owns-interval mapping is the alternative this study poses:
`active_lanes_for_order(n)` lanes per matrix, which is 32 at every order this
campaign measures and $2^n$ below $n = 5$
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:16`, `:29-31`), with
a balanced Gray interval per lane (`:33-40`). This campaign measures the
control's geometry and does not measure the prototype's, per §2.

One resource this derivation cannot rule on is dynamic LDS, for the reason in
§7.1: the compiler's static field is silent about the prototypes' $16n$-byte
launch-time table, so no claim is made that shared memory does or does not bound
the prototype mapping's occupancy.

## 9. Zero fast path and its exact marginal expectation (REQ-10)

The horizontal-product isolate carries the observed branch frequencies. They
derive from one fixed unconditioned host observation batch addressed by
`(seed_root, q, n, horizontal_product_timed, seed_index)`; device timing, where
available, resamples under the canonical warm-up and repetition policy and does
not alter these counts (preamble of
[`…-q3-horizontal-product.csv`](permanent-campaign-20260813T230032Z-1321576-q3-horizontal-product.csv)).
The same 4096-sample batch backs every row of the file, so the frequencies are
one observation, not fourteen.

For $q = 3$, $n = 12$, sample count $N = 4096$. Intervals are two-sided Wilson
score intervals at nominal 95 % coverage, computed in `analysis.py`.

| quantity | observed | frequency | exact expectation | Wilson 95 % | expectation inside interval |
| --- | ---: | ---: | ---: | --- | --- |
| zero fast path | 4064 / 4096 | 0.992187500 | $1 - (2/3)^{12} = 0.992292653$ | [0.988992173, 0.994460490] | yes |
| nonzero slow path | 32 / 4096 | 0.007812500 | $(2/3)^{12} = 0.007707347$ | [0.005539510, 0.011007827] | yes |

The two expectations are complements: $1 - ((q-1)/q)^n$ and $((q-1)/q)^n$ sum to
exactly 1, which the computed values reproduce to 15 decimal places, and the two
observed frequencies likewise sum to exactly 1 because the branches partition
the 4096 samples. The CSV states the complement relation in its own `note`
column on the measured row. Both expectations fall inside their intervals, so
the observation is consistent with the exact marginal at this order.

$n = 12$ is the only order at which the zero fast path is observed. The
horizontal-product subcommand fixes one order per invocation
(`dev/research/permanent-sampling-feas/src/main.rs:98-106`, `:132-136`) and the
committed command passes no `--n`, so the file carries $n = 12$ alone. §12
records this against REQ-10's "at each measured order".

### 9.1 A contradiction of the premise this campaign was scoped on

The issue's background states that $\mathbb{F}_3$ has the smallest zero
fast-path share of the three fields, and concludes that its fold comparison is
the one least likely to be decided by an early return. The exact marginal the
same background cites gives the opposite ordering. $1 - ((q-1)/q)^n$ is
decreasing in $q$ at fixed $n$, so $\mathbb{F}_3$ has the **largest** zero
fast-path share of the three fields at every order:

| $q$ | zero fast at $n = 12$ | zero fast at $n = 28$ |
| ---: | ---: | ---: |
| 3 | 0.992292653 | 0.999988266 |
| 5 | 0.931280523 | 0.998065719 |
| 7 | 0.842732666 | 0.986649735 |

This run's own data follows the corrected ordering rather than the premise. At
$n = 12$ the nonzero slow branch is reached by 32 of 4096 samples, which is why
its device timing has too few operations to survive the barrier subtraction
(§6.2) while the fast branch, at 7 291 683 timed operations, does. So the
$\mathbb{F}_3$ fold comparison is the one *most* likely to be decided by the
early return, and in this campaign the slow branch is in fact the one that
yields no timing. Recorded per `@/inv/falsification-preserved`; the premise is
contradicted here rather than restated.

### 9.2 Permanent-zero fraction observed in passing

Distinct from the branch frequencies above, and reported because the `zeros`
column of the grid records it: the fraction of sampled matrices whose permanent
is $0 \bmod 3$. These are by-products of the timing protocol with no
preregistered $N$; the counts are whatever each cell's repetition policy
required. Pooling across backends at one order pools independent samples,
because each cell draws from its own reserved index block (§4.2).

| $n$ | zeros | matrices | fraction | Wilson 95 % |
| ---: | ---: | ---: | ---: | --- |
| 12 | 1 557 580 | 4 662 564 | 0.334061 | [0.333633, 0.334489] |
| 16 | 265 345 | 794 859 | 0.333827 | [0.332791, 0.334864] |
| 20 | 26 331 | 79 285 | 0.332106 | [0.328836, 0.335392] |
| 24 | 3 576 | 10 604 | 0.337231 | [0.328293, 0.346287] |
| 28 | 941 | 2 740 | 0.343431 | [0.325881, 0.361419] |

## 10. Execution mapping: control and lane-owns-interval (REQ-04)

The control mapping is measured across the whole grid. `permanent_bipedal3_kernel`
maps one matrix to one block of one working thread
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:160-175`), which
is the current one-thread-per-matrix execution mapping, at orders
$\{12, 16, 20, 24, 28\}$ and batch sizes $\{256, 1024\}$, with the throughput,
phase, and resource figures of §4 and §7.

The lane-owns-interval mapping is implemented and executes on this device —
`wave_gf3_kernel<FoldKind>` launches `cases.size()` blocks of
`active_lanes_for_order(n)` lanes with a `2n`-word dynamic shared column table
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:353-361`), each
lane owning a balanced Gray interval
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:33-40`), and both
specializations compile clean and match all 12 canonical fixtures through order
16 per the README receipt. **This campaign measures no timing for it at any
order or batch size**, for the harness reason of §2. The comparison REQ-04 asks
for — the two mappings at matching orders and batch sizes — has one side only.

The comparison this campaign does support at matched geometry is the resource
comparison of §7: control 27 SGPR / 19 VGPR / 1040 scratch bytes per lane / 0
shared bytes per block, against prototypes at 22 SGPR / 22–25 VGPR / 0 scratch /
$16n$ dynamic shared bytes per block. That is a real difference in where the
column table lives, measured on both sides, and it is where the study's
occupancy hypothesis stands after this run: neither mapping is register-limited
or spill-limited on `gfx1030`, and the prediction that shared-memory pressure
grows with matrix order is untested because the compiler's static field cannot
see the dynamic allocation and no prototype timing exists to expose it.

## 11. This campaign against the figures it has to confirm or overturn

The comparison target is
[`dev/studies/b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md)
§4.4. Both runs use the same protocol, host, and GPU; `analysis.py` prints the
full 34-pair comparison.

**The crossover shape is confirmed.** The study reports that batch rayon takes
$n = 12$, the GPU at $M = 1024$ takes $n = 16$ through $n = 24$, and
intra-matrix rayon takes $n = 28$. This campaign reproduces all four: at
$n = 12$ `cpu_rayon_batch_scalar` leads at 303 392.3924 against the GPU's
217 172.3805; at $n \in \{16, 20, 24\}$ `gpu_hip` at $M = 1024$ leads; at
$n = 28$ `cpu_rayon_intra_matrix` leads every measured cell.

**The decay of the GPU's margin is confirmed at $n = 20$ and $n = 24$ and
unresolved at $n = 28$.** The study's margins over intra-matrix rayon are 1.63×
at $n = 20$, 1.05× at $n = 24$, and 0.984× at $n = 28$. This campaign measures
**1.6620×** at $n = 20$ and **1.0716×** at $n = 24$, confirming the first two
within 2 %. It cannot confirm or overturn 0.984×: that figure is the $M = 1024$
cell, which is censored here (§5), and this run's only measured $n = 28$ GPU
cell is $M = 256$ at 8.5515 matrices/s, 0.4468× the best CPU path. The
$-14.1\%$ projection bias measured on this file's own $24 \rightarrow 28$ chain
puts the censored cell's expected rate near 19.5 against intra-matrix rayon's
19.1403, which is the same too-close-to-call territory the study describes; it
is an extrapolation and settles nothing. **No ordering is asserted at
$n = 28$.**

**The $28.65\times \rightarrow 0.46\times$ restatement is confirmed.** The
2026-05-15 receipt's headline is a 28.65× GPU win at $n = 24$ against "CPU
SIMD", which the study reproduces as 29.3× and restates as 0.46× once the
baseline is the best CPU path. This campaign measures `gpu_hip` at $M = 256$
over `cpu_avx2` as **28.58×** at $n = 24$ and **28.56×** at $n = 28$, and the
same configuration over the best applicable in-tree CPU path as **0.4701×** at
$n = 24$ and **0.4468×** at $n = 28$. The headline ratio and its restatement
both reproduce, so the divergence remains entirely in the choice of CPU
baseline.

**Run-to-run agreement.** Across the 34 backend/order pairs both runs measure,
the median absolute disagreement is 2.24 %. From $n = 20$ upward every pair
agrees within 4.61 % and every GPU pair within 0.74 % — including $n = 28$,
$M = 256$ at 0.23 % and $n = 24$, $M = 1024$ at 0.74 %. The single large
disagreement is $n = 12$, $M = 256$ at $-44.34\%$ (218 275 against
121 487.1337), with $n = 12$, $M = 1024$ at $-12.31\%$ and
`cpu_rayon_batch_scalar` at $n = 12$ at $+8.33\%$. The phase columns locate it:
at $n = 12$, $M = 256$ the kernel is 28.6 % of `eval_s` and the host-side
residual is 70 % (§4.3), so that cell measures the dispatcher's per-call host
work far more than it measures the device, and it is the least reproducible cell
in the grid. This is recorded as a limit on the smallest order rather than
smoothed over; it does not touch the crossover, which is decided at
$n \ge 16$ where agreement is within 5 %.

**The projection bias is confirmed in direction and, at the two steps the study
publishes, in magnitude.** The study measures the $q = 3$ GPU projection landing
low by 14–18 % at $20 \rightarrow 24$ and $24 \rightarrow 28$; this campaign
measures $-19.4\%$ and $-14.1\%$ at $M = 256$ and $-18.9\%$ at
$20 \rightarrow 24$, $M = 1024$. It also extends the chain to two steps the
study does not publish, where the bias is far larger: $-79.1\%$ and $-82.5\%$ at
$12 \rightarrow 16$, and $-35.7\%$ and $-40.2\%$ at $16 \rightarrow 20$. The
bias shrinks monotonically with $n$ on both chains. A projection taken from a
small-$n$ reference understates a large-$n$ rate by far more than the study's
published range suggests, which is a limit on the work model at small $n$ and
not a correction to the two steps the study reports.

## 12. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §2, §4 | **Satisfied in part.** The current GPU path and all six in-tree CPU paths are compared over $\mathbb{F}_3$. No planned prototype path is in the timing comparison, because none has a harness batch evaluator; both $\mathbb{F}_3$ prototypes do execute on the target device, so this is not the "that executes" carve-out. The identical-corpus condition holds for the equivalence comparison (§3) and not for the timing grid, whose cells draw disjoint index blocks from one sampler (§4.2). |
| REQ-02 | §1, §4.1 | Satisfied. Every listed item maps to a named CSV column or provenance line. |
| REQ-03 | §4, §6 | **Satisfied in part.** The end-to-end Ryser loop is isolated for every executing path at five orders. The Gray-update and horizontal-product isolates exist but run at $n = 12$ only, cover one CPU representation and one distinct device circuit between them, and four of their five device/prototype rows are censored for a nonpositive barrier subtraction. |
| REQ-04 | §10 | **Not satisfiable from this run.** The control mapping is measured across the grid; the lane-owns-interval mapping has no timing at any order or batch size. The two mappings are compared on measured kernel resources only. |
| REQ-05 | §4.3 | Satisfied. `kernel_device_s` is its own device-event column; allocation, copy, and host serialization are outside it, and the residual is derivable per cell. |
| REQ-06 | §4.3 | Satisfied. `h2d_device_s`, `d2h_device_s`, `host_submission_s`, and `device_submission_to_kernel_s` are four separate columns; per-launch costs follow from them and `reps` without a second run. |
| REQ-07 | §7 | **Satisfied with caveats.** Registers per thread, scratch per thread, static LDS per block, and both spill counts are reported for all eight kernels beside their design predictions where a prediction exists. Two gaps: the compiler's static-LDS field cannot observe the prototypes' $16n$-byte dynamic table, and no committed design states a per-lane register budget for `permanent_bipedal3_kernel`, `gray_update_micro_kernel`, or `horizontal_product_micro_kernel`. |
| REQ-08 | §8 | Satisfied. The limiting resource is named per kernel and shown to be constant in $n$, derived from the measured usage with the empty probe kernel as the control for the occupancy ceiling; the dynamic-LDS blind spot is stated rather than assumed away. |
| REQ-09 | §7.1 | Satisfied. The one prediction a measurement contradicts — the zero-allocation reading of the nine 32-bit-unit mapping model — is carried with its contradiction in the recording artifact's own words, and the 1040-byte scratch measurement is reported against the design narrative it qualifies. No prediction is silently restated. |
| REQ-10 | §9, §9.1 | **Satisfied at the one order observed.** Both frequencies, both exact expectations, their complement relation, sample count 4096, and Wilson 95 % intervals are reported at $n = 12$. No other order carries a zero-fast-path observation, because the subcommand fixes one order per invocation and the committed command passes no `--n`. §9.1 records that the same exact marginal contradicts the issue background's field ordering. |
| REQ-11 | §3 | **Satisfied with a caveat.** All six executing paths are re-confirmed identical against the CPU oracle on the campaign host, in the same run, before any timing. The equivalence order set stops at $n = 20$, so $n = 24$ and $n = 28$ timings have no order-matched equivalence cell. |
| REQ-12 | §5 | Satisfied. The one censored cell states its reason, its projection, and the measured rate the projection is scaled from, and carries `NaN` for both throughput columns. |
| REQ-13 | §2 | **Vacuously satisfied, and the reason matters.** No planned $\mathbb{F}_3$ candidate fails to execute on the target device: both compile clean for `gfx1030`, both appear in the resource receipt, and both match all 12 canonical fixtures on this host. There is therefore no compile, correctness, or resource falsification to cite, and none is invented. The five out-of-field prototypes are excluded by circuit, quoted in §2. |
| REQ-14 | §4.4, §2 | **Not satisfiable from this run.** No prototype carries an end-to-end throughput, so no prototype ratio against the best CPU path exists. The equivalent figure for the shipped GPU path is reported: 1.6620× at $n = 20$, $M = 1024$, with 3.625 µs host and 4.625 µs device launch duration per launch. |
| REQ-15 | §1 | Satisfied. Four exact commands are committed with their revision, toolchain, and binary hashes; the run executes on the prepared benchmark host under the repository's full-host benchmark mutex, with a pristine-worktree refusal enforced both before and after the lock is acquired. |

## 13. What this campaign does not establish

Collected so a reader does not have to reassemble it from the sections above.

1. **No prototype timing exists over $\mathbb{F}_3$.** `wave-gf3` and
   `fold-gf3` are `unsupported` in the grid and in the equivalence run, for the
   single reason `prototype candidate <name> has no harness batch evaluator
   yet`. This is what blocks REQ-01's prototype arm, REQ-04, and REQ-14.
2. **Two of the three Gray-update device rows are aliases.** The `wave-gf3` and
   `fold-gf3` rows of the Gray-update isolate launch the same
   `gray_update_micro_kernel` as `gpu_hip`, per
   `dev/research/permanent-sampling-feas/src/gray_update.rs:310-315` and
   `:419-425`. They are not prototype measurements and this receipt does not
   read them as such. All three are censored regardless.
3. **The $16n$-byte shared column table is unmeasured.** The compiler reports
   static LDS only, so the study's central shared-memory prediction is neither
   confirmed nor refuted here.
4. **No per-lane register budget is predicted for the shipped kernel.** REQ-07's
   prediction-beside-measurement pairing is one-sided for
   `permanent_bipedal3_kernel`, `gray_update_micro_kernel`, and
   `horizontal_product_micro_kernel`.
5. **The $n = 28$, $M = 1024$ ordering is open.** The cell is censored; the
   study's 0.984× figure for it is neither confirmed nor overturned.
6. **The two isolates cover one order.** Both run at $n = 12$, so REQ-03's
   component isolation and REQ-10's frequency comparison describe that order
   alone.
7. **The timing grid does not share a corpus across backends.** Each cell draws
   its own disjoint 100 000-index block from the same sampler at the same
   $(q, n)$, so the comparison is between independent draws from one
   distribution rather than between paths run on identical matrices.

Two claims outside this campaign's own numbers are contradicted by its data and
are recorded rather than restated, per `@/inv/falsification-preserved`:

- The zero-allocation reading of the wave prototypes' nine 32-bit-unit mapping
  model, contradicted by the three-VGPR gap between the two folds (§7.1).
- The premise that $\mathbb{F}_3$ has the smallest zero fast-path share of the
  three fields, contradicted by the exact marginal that premise cites (§9.1).
  $\mathbb{F}_3$ has the largest such share at every order, and this run's slow
  branch is the one that yields no timing.
