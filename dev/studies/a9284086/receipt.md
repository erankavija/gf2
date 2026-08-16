# Runtime qualification of wave utilization and launch duration — committed receipt

Device-wide runtime qualification of the retained GPU permanent paths of the
accelerator study, on the one measured architecture `gfx1030`. It consumes the
three field campaigns' committed evidence — timing run
`20260814T230032Z-2085453` for $\mathbb{F}_3$, $\mathbb{F}_5$, and
$\mathbb{F}_7$, and the compiler kernel-resource receipt those campaigns share —
and adds the runtime evidence no single field campaign carries: profiler
occupancy counters for the $\mathbb{F}_3$ and $\mathbb{F}_5$ device paths, in
the paired profiled evidence run committed beside this file.
[`analysis.py`](analysis.py) regenerates every figure below from those artifacts
and asserts the structural checks the prose rests on.

```sh
python3 dev/studies/a9284086/analysis.py
```

| Artifact | Role |
| --- | --- |
| [`profiled-20260816T002340Z/`](profiled-20260816T002340Z/provenance.txt) | This study's paired profiled evidence run: $\mathbb{F}_3$ and $\mathbb{F}_5$ occupancy counters and kernel traces, carrying no timing authority |
| [`profiled-run.sh`](profiled-run.sh) | The runner that produced it, executed verbatim |
| [`analysis.py`](analysis.py) | Derivation of every table here from the committed artifacts |
| [`../047b62ed/…-q3-grid.csv`](../047b62ed/permanent-campaign-20260814T230032Z-2085453-q3-grid.csv) | $\mathbb{F}_3$ timing grid: per-phase device columns and per-launch spans |
| [`../91605d4d/…-q5-grid.csv`](../91605d4d/permanent-campaign-20260814T230032Z-2085453-q5-grid.csv) | $\mathbb{F}_5$ timing grid, same columns |
| [`../6c7fcb38/…-q7-grid.csv`](../6c7fcb38/permanent-campaign-20260814T230032Z-2085453-q7-grid.csv) | $\mathbb{F}_7$ timing grid, same columns |
| [`../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt) | Compiler kernel-resource receipt for every kernel of every retained path |
| [`../6c7fcb38/profiled-20260815T181923Z/`](../6c7fcb38/profiled-20260815T181923Z/provenance.txt) | The committed $\mathbb{F}_7$ occupancy counters, cited rather than re-measured |
| [`../b488f02c/sustained-2026-08-07.csv`](../b488f02c/sustained-2026-08-07.csv) | Prior sustained receipt at the cell where the one recorded device fault occurred |
| [`../b488f02c/gpu-hang-2026-08-07.log`](../b488f02c/gpu-hang-2026-08-07.log) | That fault's only record, and its explicit retraction of the watchdog attribution |

Every figure this document takes from another study is cited to the file it
comes from rather than restated as an independent claim.

## 1. Provenance and reproduction (REQ-09)

Two bodies of evidence carry this qualification, and they are kept apart.

**The committed timing evidence** is campaign run `20260814T230032Z-2085453`,
whose three field grids supply every transfer, launch-overhead, and
kernel-duration figure here. Its revision, toolchain, host inventory, binary
hashes, and exact commands are recorded in each field study's own provenance
file, and each campaign's §1 states the benchmark-mutex discipline the run held
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §1,
[`../91605d4d/receipts.md`](../91605d4d/receipts.md) §1,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §1). Nothing in that run is
re-measured here.

**This study's own runtime evidence** is the paired profiled run
[`profiled-20260816T002340Z/`](profiled-20260816T002340Z/provenance.txt),
produced by [`profiled-run.sh`](profiled-run.sh) executed verbatim. It runs the
same hash-pinned harness binary as the timing run, SHA-256
`edb03650bbbfb47064a3a2b5e021ac055394812a4362c577fdad07cdbd997bce`, verified
against `target/permanent-campaign/manifest-v1.txt` before the first pass and
recorded in `run.log`; profiler `rocprofv3` 1.1.0 at `/opt/rocm/bin/rocprofv3`,
ROCm 7.2.4, device `gfx1030` (AMD Radeon RX 6950 XT), host AMD Ryzen 9 5900X
with 24 logical CPUs, kernel `7.1.8-arch1-3`; repository revision
`6f87b14a78e640fca9702bccfd6659d143fa19d0` at run start. Every pass ran under
[`dev/scripts/ccx1-bench-flock.sh --full-host`](../../scripts/ccx1-bench-flock.sh),
the repository's canonical benchmark mutex.

Thirty-four passes, one `COMMAND`/`EXIT` pair each in
[`run.log`](profiled-20260816T002340Z/run.log), all exit 0: seventeen cells,
each with one kernel-trace pass and one early-iteration counter pass. Every pass
wraps exactly one device-executing workload and every `-o` carries `%pid%`, the
two conditions the committed $\mathbb{F}_7$ run's provenance records as
necessary for per-process outputs to survive
([`../6c7fcb38/profiled-20260815T181923Z/provenance.txt`](../6c7fcb38/profiled-20260815T181923Z/provenance.txt),
`supersedes:`). `analysis.py` section 1 asserts that the pass directories and
the log's pass list agree exactly and that every exit code is zero, so a partial
run fails the script rather than passing silently under this text.

The grid passes reuse each field's committed `--execution-id` — `3002` for
$q = 3$ and `5002` for $q = 5$ — so the profiled cells draw the same
preregistered matrices as the timing run's cells at those orders.

**The seventeen cells are each field's three retained device paths at $n = 12$
and $n = 20$, the two orders the committed $\mathbb{F}_7$ counter evidence uses,
plus that field's declared operating point** — $n = 28$ for $q = 3$ and
$n = 24$ for $q = 5$, which the campaign protocol fixes as the
processor-feasible frontier of each field
([`dev/simulation_results/permanent-zero-fraction/protocol.md`](../../simulation_results/permanent-zero-fraction/protocol.md):51-54).
The $q = 5$, $n = 24$ shipped cells are censored before running in the timing
grid, so that cell dispatches nothing and is not a pass, which is why $q = 5$
contributes eight cells against $q = 3$'s nine.

**The two runs are deliberately separate, and this one carries no timing
authority.** Counter collection and kernel tracing perturb execution; the
preregistered timing evidence therefore carries no profiler, and no duration
from the profiled run is used as a timing result here. One profiled cell that
the timing grid records as `measured` comes out `censored` under tracing — the
$q = 3$, $n = 28$ shipped cell at $M = 256$ — which is that perturbation made
visible. Each profiled cell also re-calibrates its own batch from a
single-matrix probe under the profiler, so profiled batch sizes differ from the
timing batches and every table below names the batch it read.

Reproduction is the committed runner against the hash-pinned binary on the
prepared benchmark host:

```sh
dev/studies/a9284086/profiled-run.sh
```

## 2. The retained GPU permanent paths

Retention is the campaigns' own outcome: a path is retained when its candidate
executes as a full-permanent batch path on the target device, which each
campaign's §2 records per candidate. Nine paths are retained, three per field,
dispatching eleven kernels between them. One planned candidate is not retained
and is named with the falsification the $\mathbb{F}_7$ campaign recorded for it:
`f7-three-plane-accumulator` has no device batch evaluator, because its
translation unit holds a single-thread conformance probe rather than a
full-permanent batch kernel
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §2). The component-isolate
micro kernels of each campaign's §6 are circuits rather than permanent paths and
are outside this qualification's subject.

| $q$ | Path | Kernels dispatched | Lanes per block | Dynamic shared bytes per block |
| ---: | --- | --- | ---: | --- |
| 3 | `gpu_hip` | `permanent_bipedal3_kernel` | 1 | 0, exact |
| 3 | `wave-gf3` | `wave_gf3_kernel<kHalving>` | 32 | $16n$ |
| 3 | `fold-gf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 32 | $16n$ |
| 5 | `gpu_hip` | `permanent_bipedal5_kernel` | 1 | 0, exact |
| 5 | `f5-byte-control` | `f5_byte_control_kernel` | 32 | $n^2$ |
| 5 | `f5-three-plane` | `f5_three_plane_kernel` | 32 | $24n$ |
| 7 | `gpu_hip` | `permanent_bipedal7_kernel` | 1 | 0, exact |
| 7 | `f7-lookup-table-control` | `wave_gf7_lookup_table_kernel<1>` at $n \le 16$, `<2>` above | 32 | $8n\lceil n/16 \rceil$ |
| 7 | `f7-three-plane-permanent` | `prepare_three_plane_columns` then `wave_gf7_three_plane_kernel` | 32 | 0, exact, then $24n$ |

The three shipped paths launch `gridDim.x = M` blocks of `dim3 block(1, 1, 1)`
and return from every thread but thread 0
(`crates/gf2-kernels-hip/hip/permanent/permanent_bipedal3.hip:174`, `:334-335`,
`:350`; `permanent_bipedal5.hip:99`, `:253-254`, `:269`;
`permanent_bipedal7.hip:99`, `:350-351`, `:366`). The six lane-owns-interval
paths launch `gridDim.x = M` blocks of `active_lanes_for_order(n)` lanes, which
is 32 at every order these campaigns measure
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:29-31`), with the
dynamic shared column table the last column names
(`wave_gf3_equivalence.hip:597-606`, `f5_wave_equivalence.hip:727-737`,
`wave_gf7_equivalence.hip:676-684`, `:715-724`).

Per-block shared memory at each measured order, from those launch formulas:

| Path | $n = 12$ | $n = 16$ | $n = 20$ | $n = 24$ | $n = 28$ |
| --- | ---: | ---: | ---: | ---: | ---: |
| `wave-gf3`, `fold-gf3` | 192 B | 256 B | 320 B | 384 B | 448 B |
| `f5-byte-control` | 144 B | 256 B | 400 B | 576 B | 784 B |
| `f5-three-plane` | 288 B | 384 B | 480 B | 576 B | 672 B |
| `f7-lookup-table-control` | 96 B | 128 B | 320 B | 384 B | 448 B |
| `f7-three-plane-permanent` (walk) | 288 B | 384 B | 480 B | 576 B | 672 B |

## 3. Wave utilization of each execution mapping (REQ-01)

A wavefront on this device is 32 lanes, which the profiler's own per-pass agent
report states as `Wave_Front_Size` 32 and `analysis.py` section 1 checks
identical across every pass directory of the run. Wave utilization below is the
share of those 32 lanes performing work that contributes to the result. It is
derived from the launch geometry and the source loop bounds rather than
measured, because no per-lane device counter exists in this evidence; the
launch geometry half of the derivation is confirmed against the profiled
dispatches' own `Workgroup_Size`.

**The launch-geometry share is exact by construction and differs by a factor of
32 between the two mappings.**

| $q$ | Path | Mapping | Lanes the launch makes active | Share of a wavefront | Launch citation |
| ---: | --- | --- | ---: | ---: | --- |
| 3 | `gpu_hip` | one thread per matrix | 1 of 32 | **3.1250 %** | `permanent_bipedal3.hip:334-335`, with `if (threadIdx.x != 0) return;` at `:174` |
| 3 | `wave-gf3` | lane-owns-interval | 32 of 32 | **100.0000 %** | `wave_gf3_equivalence.hip:597-606` |
| 3 | `fold-gf3` | lane-owns-interval | 32 of 32 | **100.0000 %** | `wave_gf3_equivalence.hip:597-606` |
| 5 | `gpu_hip` | one thread per matrix | 1 of 32 | **3.1250 %** | `permanent_bipedal5.hip:253-254`, with `if (threadIdx.x != 0) return;` at `:99` |
| 5 | `f5-byte-control` | lane-owns-interval | 32 of 32 | **100.0000 %** | `f5_wave_equivalence.hip:727-737` |
| 5 | `f5-three-plane` | lane-owns-interval | 32 of 32 | **100.0000 %** | `f5_wave_equivalence.hip:727-737` |
| 7 | `gpu_hip` | one thread per matrix | 1 of 32 | **3.1250 %** | `permanent_bipedal7.hip:350-351`, with `if (threadIdx.x != 0) return;` at `:99` |
| 7 | `f7-lookup-table-control` | lane-owns-interval | 32 of 32 | **100.0000 %** | `wave_gf7_equivalence.hip:715-724` |
| 7 | `f7-three-plane-permanent` | lane-owns-interval | 32 of 32 | **100.0000 %** | `wave_gf7_equivalence.hip:676-684` |

Every lane-owns-interval row's 32 is `active_lanes_for_order(n)` at the orders
these campaigns measure (`wave_ryser_mapping.h:29-31`).

**The lane-owns-interval mapping has no tail-interval idleness at any measured
order, and that is exact rather than approximate.** `balanced_interval` gives
lane $i$ a contiguous range of $\lfloor T / L \rfloor$ indices plus one more
when $i$ is below $T \bmod L$
(`dev/research/permanent_wave_gpu/hip/wave_ryser_mapping.h:33-40`), and every
walk kernel calls it with $T = 2^n$ and $L = 32$
(`wave_gf3_equivalence.hip:196-198`, `f5_wave_equivalence.hip:241-243`,
`:310-312`, `wave_gf7_equivalence.hip:184-185`, `:297-298`). For every measured
order $2^n \bmod 32 = 0$, so the remainder is zero and all 32 lanes own exactly
$2^{n-5}$ indices: 128 at $n = 12$, 2 048 at $n = 16$, 32 768 at $n = 20$,
524 288 at $n = 24$, and 8 388 608 at $n = 28$. `analysis.py` section 3 asserts
the remainder is zero at every measured order, so an order that broke the
divisibility would fail the script.

**One kernel of the retained set is not a Gray walk, and its share is $n/32$.**
`prepare_three_plane_columns` is the $\mathbb{F}_7$ bit-plane transpose, and its
whole body is a strided loop over the matrix's $n$ columns
(`wave_gf7_equivalence.hip:144-145`), so at a 32-lane launch it occupies $n$ of
32 lanes: 37.5000 % at $n = 12$, 50.0000 % at $n = 16$, 62.5000 % at $n = 20$,
75.0000 % at $n = 24$, and 87.5000 % at $n = 28$.

**Refining the walk kernels' 100 % by the phases around the walk.** Each walk
kernel runs a strided global-to-shared staging loop, a per-lane prefix
reconstruction of the accumulator at its interval start, the Gray walk itself,
and a 32-step lane-order reduction. The staging loop covers $S$ items across 32
lanes, so it leaves $32\lceil S/32 \rceil - S$ lane-steps idle; the reduction
runs 32 `__shfl` steps in which each lane sources its own partial exactly once
and lane 0 alone accumulates
(`wave_ryser_mapping.h:67-78`). The table gives the resulting lane-step share
under two readings of the reduction: *strict* counts a reduction lane-step as
useful only where a lane sources its partial or lane 0 accumulates, *generous*
counts every lane's shuffle participation. This is a lane-step share and not a
time-weighted one, because no per-phase device timing exists in this evidence.

| Kernel | $S$ staged | $n = 12$ | $n = 16$ | $n = 20$ | $n = 24$ | $n = 28$ |
| --- | --- | --- | --- | --- | --- | --- |
| `wave_gf3_kernel<*>` | $n$ | 82.2977–99.6387 % | 98.5455–99.9762 % | 99.9075–99.9989 % | 99.9942–100.0000 % | 99.9996–100.0000 % |
| `f5_byte_control_kernel` | $n^2$ | 82.7684–99.7175 % | 98.5741–100.0000 % | 99.9071–99.9985 % | 99.9943–100.0000 % | 99.9996–100.0000 % |
| `f5_three_plane_kernel` | $n$ | 82.2977–99.6387 % | 98.5455–99.9762 % | 99.9075–99.9989 % | 99.9942–100.0000 % | 99.9996–100.0000 % |
| `wave_gf7_lookup_table_kernel<*>` | $n$ | 82.2977–99.6387 % | 98.5455–99.9762 % | 99.9075–99.9989 % | 99.9942–100.0000 % | 99.9996–100.0000 % |
| `wave_gf7_three_plane_kernel` | $3n$ | 82.2557–99.4971 % | 98.5462–99.9762 % | 99.9082–99.9996 % | 99.9941–99.9999 % | 99.9996–100.0000 % |

The two readings differ by at most 17.3 percentage points at $n = 12$ and by
under 0.1 at every order from $n = 20$ up, because the fixed 32-step reduction
is a fifth of a 128-step walk at $n = 12$ and a ten-thousandth of a 32 768-step
walk at $n = 20$. The staging loop costs $32\lceil S/32 \rceil - S$ idle
lane-steps once, at most 28 of them, which is 7 parts in $10^3$ of the walk's
lane-steps at $n = 12$ and under 3 parts in $10^4$ from $n = 16$ up.

**The shipped mapping's 3.1250 % holds for the whole kernel at every order and
batch**, because the block is one thread wide: there is no phase in which a
second lane of its wavefront does work.

## 4. Kernel resources and the occupancy-limiting resource (REQ-02)

Every figure in this section is a committed measurement of a field campaign,
cited to its file rather than re-measured. The source is the compiler
kernel-resource receipt
[`../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt`](../6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt)
at `source_revision f9224650a780ea8ed98bcdff6e662cdb0d7b94f4`, architecture
`gfx1030`, flag `-Rpass-analysis=kernel-resource-usage`, every entry
`exit_status: 0` with source, object, and log SHA-256; the three campaigns read
the same receipt in their §7 tables
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §7,
[`../91605d4d/receipts.md`](../91605d4d/receipts.md) §7,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §7). `VGPRs` is the
per-thread vector-register count, `TotalSGPRs` the per-wave scalar count,
`ScratchSize [bytes/lane]` private scratch per thread, and `LDS Size
[bytes/block]` the compiler's *static* LDS field. The `log` column cites the
first line of that kernel's remark block inside the named log of the receipt
directory.

| $q$ | Path | Kernel | `TotalSGPRs` | `VGPRs` | scratch B/lane | SGPR + VGPR spill | static LDS B/block | occupancy waves/SIMD | occupancy-limiting resource | log |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 3 | `gpu_hip` | `permanent_bipedal3_kernel` | 27 | 19 | 1040 | 0 + 0 | 0 | 16 | wave-slot ceiling | `permanent_bipedal3.hip.resource.log:1` |
| 3 | `wave-gf3` | `wave_gf3_kernel<kHalving>` | 22 | 22 | 0 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf3_equivalence.hip.resource.log:23` |
| 3 | `fold-gf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 22 | 25 | 0 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf3_equivalence.hip.resource.log:34` |
| 5 | `gpu_hip` | `permanent_bipedal5_kernel` | 107 | 128 | 4000 | 4 + 6 | 0 | 8 | per-lane vector registers | `permanent_bipedal5.hip.resource.log:1` |
| 5 | `f5-byte-control` | `f5_byte_control_kernel` | 78 | 77 | 0 | 0 + 0 | 0 | 12 | per-lane vector registers | `f5_wave_equivalence.hip.resource.log:1` |
| 5 | `f5-three-plane` | `f5_three_plane_kernel` | 20 | 66 | 0 | 0 + 0 | 0 | 12 | per-lane vector registers | `f5_wave_equivalence.hip.resource.log:12` |
| 7 | `gpu_hip` | `permanent_bipedal7_kernel` | 107 | 128 | 4000 | 4 + 6 | 0 | 8 | per-lane vector registers | `permanent_bipedal7.hip.resource.log:1` |
| 7 | `f7-lookup-table-control` | `wave_gf7_lookup_table_kernel<1>` | 19 | 31 | 0 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:23` |
| 7 | `f7-lookup-table-control` | `wave_gf7_lookup_table_kernel<2>` | 26 | 41 | 24 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:34` |
| 7 | `f7-three-plane-permanent` | `prepare_three_plane_columns` | 16 | 17 | 0 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:1` |
| 7 | `f7-three-plane-permanent` | `wave_gf7_three_plane_kernel` | 16 | 34 | 0 | 0 + 0 | 0 | 16 | wave-slot ceiling | `wave_gf7_equivalence.hip.resource.log:12` |

**The limiter column is derived from the receipt as a whole, and the derivation
is checkable.** The compiler emits its occupancy field from the per-thread
quantities in the same remark block, and across all 26 kernel entries in the
receipt directory `VGPRs` alone reproduces every reported value under a
1 024-register per-SIMD budget allocated in units of 16 and capped at 16
waves/SIMD. `analysis.py` section 4 computes
$\min\left(16,\ \left\lfloor 1024 / \left(16 \lceil \texttt{VGPRs}/16 \rceil\right)\right\rfloor\right)$
for every entry and asserts agreement; it holds at 26 of 26. Scalar registers
and private scratch are shown not to bind by counterexample in the same receipt
— `gray_update_micro_kernel` at 86 `TotalSGPRs` and `permanent_bipedal3_kernel`
at 1 040 scratch bytes per lane both report 16 — and no kernel in the receipt
reports nonzero static LDS. Four of the eleven retained-path kernels are held
below the ceiling, and all four by per-lane vector registers: the two
$\mathbb{F}_5$ prototypes at 12 waves/SIMD, and `permanent_bipedal5_kernel`
and `permanent_bipedal7_kernel` at 8, the two identical on all eight resource
fields because both translation units declare `uint8_t columns[63][63]` and
`uint8_t col_sum[63]`. The third shipped kernel, `permanent_bipedal3_kernel`,
reaches the ceiling at 19 `VGPRs` despite 1 040 scratch bytes per lane.

**Per-block shared memory is the one resource the compiler's field cannot see,
and the launch formulas of §2 supply it instead.** `-Rpass-analysis=kernel-resource-usage`
reports static LDS only, so a 0 in that column is silence about a dynamically
requested table rather than evidence of absence, which each campaign's §7.1
states first. The largest per-block request over every retained path and
measured order is 784 B, `f5-byte-control` at $n = 28$. One block of a
lane-owns-interval path is one wave, so the device's own `Max_Waves_Per_Cu` of
32 caps the blocks a compute unit may hold; 32 blocks at 784 B request 25 088 B
against the agent report's `Lds_Size_In_Kb` 64, or 65 536 B, per compute unit.
The LDS capacity would admit 83 such blocks. `analysis.py` section 4 asserts the
inequality, so a future path whose table grew past the LDS budget would fail the
script rather than pass under this text. **Per-block shared memory does not
limit the occupancy of any retained path at any measured order.**

**The runtime confirms the compiler's per-lane figures rather than replacing
them.** The per-dispatch resource fields of this study's counter CSVs report
`VGPR_Count` as the compiler's `VGPRs` rounded up to the runtime's allocation
granularity of eight, and reproduce the compiler's `Scratch_Size` exactly;
`analysis.py` section 5 asserts the rounding for every kernel it reads. The
committed $\mathbb{F}_7$ run records the same agreement for its own kernels
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13).

**`LDS_Block_Size` reads 0 at runtime as well, for launches that request a
dynamic table.** Every dispatch in this study's counter CSVs reports
`LDS_Block_Size` 0, including the lane-owns-interval kernels whose launches pass
the shared byte counts of §2, so the runtime record shares the compiler's blind
spot rather than closing it. That is recorded rather than smoothed over
(`@/inv/falsification-preserved`) and is already tracked: JIT issue `023233c5`
carries the confirmation pass, and cites the `rocprofv2` capability probe of the
$\mathbb{F}_7$ run, which read `LDS_Per_Workgroup` 512 for
`wave_gf7_three_plane_kernel` at $n = 20$ — the design's 480-byte prediction at
a 128-byte allocation granularity — in the four dispatches it captured before
aborting
([`../6c7fcb38/profiled-20260815T181923Z/rocprofv2-n20-threeplane/pmc_1/results_v2-n20-threeplane.csv`](../6c7fcb38/profiled-20260815T181923Z/rocprofv2-n20-threeplane/pmc_1/results_v2-n20-threeplane.csv)).
That is the one runtime observation of a dynamic shared table in the study's
evidence, and it confirms the launch formula for the kernel it covers.

## 5. Achieved occupancy from profiler counters (REQ-03)

Achieved occupancy here is observed runtime evidence and never a compiler
figure: it is `MeanOccupancyPerCU`, a per-dispatch counter collected by
`rocprofv3` while the workload executed, expressed as a share of the device's
own wave-slot count. The device reports `Cu_Count` 80 and `Max_Waves_Per_Cu` 32,
so it holds $80 \times 32 = 2\,560$ wave slots; `Simd_Count` 160 at
`Max_Waves_Per_Simd` 16 gives the same 2 560 from the other direction. Those
figures come from the profiler's own per-pass agent report, which `analysis.py`
section 1 checks is identical in all 34 pass directories.

**The profiler is available on this host, so the criterion's not-measured escape
does not arise, and it is not invoked.** Every one of the eleven kernels of the
nine retained paths has an admissible achieved-occupancy reading: six from this
study's run and five from the committed $\mathbb{F}_7$ run
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13). The committed
$\mathbb{F}_7$ run also records that `rocprofv2` and legacy `rocprof` abort this
workload under their interception on this host, with the failing commands and
their exit 101 retained, so `rocprofv3` is the one profiler here that both runs
the workload and produces counters for it.

**Admissibility is a stated physical test applied per dispatch, not a choice of
favourable values.** This run collects the $\mathbb{F}_7$ study's round-2
counter set from the start — `--pmc SQ_WAVES MeanOccupancyPerCU
--kernel-iteration-range [1-8]` — because that study established that
rocprofv3's full-pass derived occupancy counters divide by a `GRBM_GUI_ACTIVE`
window that drifts with cumulative dispatch count, exact on a pass's first
dispatches and degrading after. The admissibility policy is the one committed in
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13 and made authoritative
by the 2026-08-16 amendment in that run's `provenance.txt`: a reading counts when
`MeanOccupancyPerCU` lies inside $(0, 32]$ and puts no more waves resident than
the dispatch's own `Grid_Size`/`Workgroup_Size` launch geometry supplies, with
one percent of slack for the counter's rounding. `SQ_WAVES` is recorded per
dispatch and is not an admissibility test, because the same amendment records it
inflated relative to launch geometry under this collection mode.

**Every reading this run produced is admissible.** `analysis.py` section 5
applies the policy to all 135 counted dispatches and rejects none, so the table
below is the run's complete counter record rather than a surviving subset. That
is a difference from the committed $\mathbb{F}_7$ round-2 evidence, where two
$n = 20$ dispatches on the shipped kernel and both three-plane passes at
$n = 12$ fail the same policy and are recorded as failures there.

"Resident waves" is `MeanOccupancyPerCU` $\times$ `Cu_Count`; "residency" is that
count over the waves the dispatch launched; "width ceiling" is the highest
occupancy that launch width could reach, $\min(1, W / 2\,560)$; achieved
occupancy is the mean reading against the 2 560 slots. A width-1 group is the
single-matrix probe that opens each grid cell, except at $q = 5$, $n = 24$ on
the byte control, where the cell's own batch is one matrix.

| pass | kernel | launch width | dispatches | `MeanOccupancyPerCU` mean | min–max | resident waves | residency | width ceiling | achieved occupancy |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `pmc-q3-n12-gpuhip` | `permanent_bipedal3_kernel` | 1 | 1 | 0.008377 | 0.008377 | 0.67 | 0.6702 | 0.0391 % | **0.0262 %** |
| `pmc-q3-n12-gpuhip` | `permanent_bipedal3_kernel` | 256 | 7 | 2.939549 | 2.936072–2.944525 | 235.16 | 0.9175–0.9202 | 10.0000 % | **9.1861 %** |
| `pmc-q3-n12-wavegf3` | `wave_gf3_kernel<kHalving>` | 1 | 1 | 0.007894 | 0.007894 | 0.63 | 0.6315 | 0.0391 % | **0.0247 %** |
| `pmc-q3-n12-wavegf3` | `wave_gf3_kernel<kHalving>` | 13 | 7 | 0.104173 | 0.098551–0.125950 | 8.33 | 0.6065–0.7751 | 0.5078 % | **0.3255 %** |
| `pmc-q3-n12-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 1 | 1 | 0.009179 | 0.009179 | 0.73 | 0.7343 | 0.0391 % | **0.0287 %** |
| `pmc-q3-n12-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 12 | 7 | 0.105765 | 0.101232–0.108983 | 8.46 | 0.6749–0.7266 | 0.4688 % | **0.3305 %** |
| `pmc-q3-n20-gpuhip` | `permanent_bipedal3_kernel` | 1 | 1 | 0.012475 | 0.012475 | 1.00 | 0.9980 | 0.0391 % | **0.0390 %** |
| `pmc-q3-n20-gpuhip` | `permanent_bipedal3_kernel` | 256 | 7 | 3.035674 | 3.016255–3.141247 | 242.85 | 0.9426–0.9816 | 10.0000 % | **9.4865 %** |
| `pmc-q3-n20-wavegf3` | `wave_gf3_kernel<kHalving>` | 1 | 1 | 0.012471 | 0.012471 | 1.00 | 0.9976 | 0.0391 % | **0.0390 %** |
| `pmc-q3-n20-wavegf3` | `wave_gf3_kernel<kHalving>` | 13 | 7 | 0.162101 | 0.162063–0.162226 | 12.97 | 0.9973–0.9983 | 0.5078 % | **0.5066 %** |
| `pmc-q3-n20-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 1 | 1 | 0.012455 | 0.012455 | 1.00 | 0.9964 | 0.0391 % | **0.0389 %** |
| `pmc-q3-n20-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 12 | 7 | 0.149454 | 0.149414–0.149475 | 11.96 | 0.9961–0.9965 | 0.4688 % | **0.4670 %** |
| `pmc-q3-n28-gpuhip` | `permanent_bipedal3_kernel` | 1 | 1 | 0.012500 | 0.012500 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q3-n28-gpuhip` | `permanent_bipedal3_kernel` | 256 | 6 | 3.061032 | 3.019039–3.141912 | 244.88 | 0.9434–0.9818 | 10.0000 % | **9.5657 %** |
| `pmc-q3-n28-gpuhip` | `permanent_bipedal3_kernel` | 1024 | 1 | 11.269614 | 11.269614 | 901.57 | 0.8804 | 40.0000 % | **35.2175 %** |
| `pmc-q3-n28-wavegf3` | `wave_gf3_kernel<kHalving>` | 1 | 1 | 0.012500 | 0.012500 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q3-n28-wavegf3` | `wave_gf3_kernel<kHalving>` | 2 | 7 | 0.025000 | 0.025000–0.025000 | 2.00 | 1.0000 | 0.0781 % | **0.0781 %** |
| `pmc-q3-n28-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 1 | 1 | 0.012500 | 0.012500 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q3-n28-foldgf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | 3 | 7 | 0.037499 | 0.037499–0.037499 | 3.00 | 1.0000 | 0.1172 % | **0.1172 %** |
| `pmc-q5-n12-gpuhip` | `permanent_bipedal5_kernel` | 1 | 1 | 0.012234 | 0.012234 | 0.98 | 0.9787 | 0.0391 % | **0.0382 %** |
| `pmc-q5-n12-gpuhip` | `permanent_bipedal5_kernel` | 256 | 7 | 2.533167 | 2.512354–2.544635 | 202.65 | 0.7851–0.7952 | 10.0000 % | **7.9161 %** |
| `pmc-q5-n12-bytectl` | `f5_byte_control_kernel` | 1 | 1 | 0.012354 | 0.012354 | 0.99 | 0.9883 | 0.0391 % | **0.0386 %** |
| `pmc-q5-n12-bytectl` | `f5_byte_control_kernel` | 13 | 7 | 0.160044 | 0.159514–0.160366 | 12.80 | 0.9816–0.9869 | 0.5078 % | **0.5001 %** |
| `pmc-q5-n12-threeplane` | `f5_three_plane_kernel` | 1 | 1 | 0.009075 | 0.009075 | 0.73 | 0.7260 | 0.0391 % | **0.0284 %** |
| `pmc-q5-n12-threeplane` | `f5_three_plane_kernel` | 13 | 7 | 0.116376 | 0.111428–0.131696 | 9.31 | 0.6857–0.8104 | 0.5078 % | **0.3637 %** |
| `pmc-q5-n20-gpuhip` | `permanent_bipedal5_kernel` | 1 | 1 | 0.012499 | 0.012499 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q5-n20-gpuhip` | `permanent_bipedal5_kernel` | 256 | 6 | 2.683867 | 2.538471–2.963103 | 214.71 | 0.7933–0.9260 | 10.0000 % | **8.3871 %** |
| `pmc-q5-n20-gpuhip` | `permanent_bipedal5_kernel` | 1024 | 1 | 8.439886 | 8.439886 | 675.19 | 0.6594 | 40.0000 % | **26.3746 %** |
| `pmc-q5-n20-bytectl` | `f5_byte_control_kernel` | 1 | 1 | 0.012500 | 0.012500 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q5-n20-bytectl` | `f5_byte_control_kernel` | 4 | 7 | 0.049914 | 0.049826–0.049974 | 3.99 | 0.9965–0.9995 | 0.1562 % | **0.1560 %** |
| `pmc-q5-n20-threeplane` | `f5_three_plane_kernel` | 1 | 1 | 0.012475 | 0.012475 | 1.00 | 0.9980 | 0.0391 % | **0.0390 %** |
| `pmc-q5-n20-threeplane` | `f5_three_plane_kernel` | 12 | 7 | 0.149569 | 0.149512–0.149603 | 11.97 | 0.9967–0.9974 | 0.4688 % | **0.4674 %** |
| `pmc-q5-n24-bytectl` | `f5_byte_control_kernel` | 1 | 7 | 0.012500 | 0.012500–0.012500 | 1.00 | 1.0000 | 0.0391 % | **0.0391 %** |
| `pmc-q5-n24-threeplane` | `f5_three_plane_kernel` | 1 | 1 | 0.012498 | 0.012498 | 1.00 | 0.9999 | 0.0391 % | **0.0391 %** |
| `pmc-q5-n24-threeplane` | `f5_three_plane_kernel` | 9 | 7 | 0.112470 | 0.112456–0.112479 | 9.00 | 0.9996–0.9998 | 0.3516 % | **0.3515 %** |

**The readings agree with the launch geometry to the counter's last digits where
the launch is small enough to check by hand.** A single resident wave on 80
compute units is $1/80 = 0.012500$ waves per CU exactly, and the seventeen
width-1 groups read 0.007894 to 0.012500, which is 0.63 to 1.00 of that single
wave; the low end sits on the briefest dispatches, where the launch ramp is most
of the counter's window. At $q = 3$, $n = 28$ the halving control's two-wave
launch reads 0.025000 on all seven of its dispatches, which is $2/80$ exactly,
and the fold's three-wave launch reads 0.037499 on all seven against an exact
$3/80 = 0.037500$. `analysis.py` section 5 also asserts that each dispatch's
`VGPR_Count` is the compiler's `VGPRs` rounded up to the runtime's allocation
granularity of eight, and that its `Scratch_Size` reproduces the compiler's
figure, so the runtime and the receipt of §4 are checked against each other on
every kernel this run reads.

**Achieved occupancy per retained path, over both evidence bodies.** The
$\mathbb{F}_7$ column is the committed run's figures, cited to
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13 rather than re-measured.

| $q$ | Path | Kernel | Highest achieved occupancy | at launch width | per-SIMD ceiling as a share of the device | evidence |
| ---: | --- | --- | ---: | ---: | ---: | --- |
| 3 | `gpu_hip` | `permanent_bipedal3_kernel` | **35.2175 %** | 1 024 | 100 % | `pmc-q3-n28-gpuhip`, 1 dispatch |
| 3 | `wave-gf3` | `wave_gf3_kernel<kHalving>` | **0.5066 %** | 13 | 100 % | `pmc-q3-n20-wavegf3`, 7 dispatches |
| 3 | `fold-gf3` | `wave_gf3_kernel<kZeroMaskSignPopcount>` | **0.4670 %** | 12 | 100 % | `pmc-q3-n20-foldgf3`, 7 dispatches |
| 5 | `gpu_hip` | `permanent_bipedal5_kernel` | **26.3746 %** | 1 024 | 50 % | `pmc-q5-n20-gpuhip`, 1 dispatch |
| 5 | `f5-byte-control` | `f5_byte_control_kernel` | **0.5001 %** | 13 | 75 % | `pmc-q5-n12-bytectl`, 7 dispatches |
| 5 | `f5-three-plane` | `f5_three_plane_kernel` | **0.4674 %** | 12 | 75 % | `pmc-q5-n20-threeplane`, 7 dispatches |
| 7 | `gpu_hip` | `permanent_bipedal7_kernel` | **27.4731 %** | 1 024 | 50 % | committed, `pmc2-n20-gpuhip`, 1 dispatch |
| 7 | `f7-lookup-table-control` | `wave_gf7_lookup_table_kernel<1>` | **49.4160 %** | 1 614 | 100 % | committed, `pmc2-n12-lookup`, 7 dispatches |
| 7 | `f7-lookup-table-control` | `wave_gf7_lookup_table_kernel<2>` | **1.0855 %** | 28 | 100 % | committed, `pmc2-n20-lookup`, 7 dispatches |
| 7 | `f7-three-plane-permanent` | `prepare_three_plane_columns` | **0.1342 %** | 13 | 100 % | committed, `pmc2-n20-threeplane`, 7 dispatches |
| 7 | `f7-three-plane-permanent` | `wave_gf7_three_plane_kernel` | **0.5271 %** | 14 | 100 % | committed, `pmc2-n16-threeplane`, 7 dispatches |

**The compiler's occupancy figure never stands in for any of these.** The
`Occupancy [waves/SIMD]` field of §4 is emitted at compile time from the
per-thread resource counts in the same remark block, and §4 shows it is
reproduced exactly, for all 26 entries of the receipt, by a function of `VGPRs`
alone. A quantity a compile-time register count determines completely is a
prediction of occupancy and not an observation of it; it is reported in §4 as
what it is, and the achieved column above is filled only from counters.

## 6. What limits occupancy across the study's kernels (REQ-04)

The study's central occupancy hypothesis asks whether the per-lane register
budget and per-block shared-memory allocation of the lane-owns-interval mapping
is what limits occupancy. **Across the study's kernels the answer is no, at
every level the evidence can address, and a different resource limits at each
level.**

**Level 1 — the mapping's stated budgets do not limit occupancy anywhere.**
Seven of the eleven retained-path kernels have a committed per-lane register
budget, all of them source-level lower bounds on stable lane state: 9 32-bit
units for both $\mathbb{F}_3$ folds (`wave_gf3_equivalence.hip:10-12`),
$(n+20)/4$ units — 8 to 12 at the measured orders — for the $\mathbb{F}_5$ byte
control and 11 units for its three-plane path (`f5_wave_equivalence.hip:9-12`,
`:13-15`), and 7, 9, and 11 units for the two $\mathbb{F}_7$ lookup
instantiations and the $\mathbb{F}_7$ walk (`wave_gf7_equivalence.hip:14-17`,
`:8-12`). Applying the compiler's own occupancy model to each of those budgets
returns 16 waves/SIMD, the architectural ceiling, at every measured order;
`analysis.py` section 4 asserts it for all seven. Per-block shared memory does
not limit either: the largest request over every retained path and order is
784 B, and 32 such blocks — the device's own per-CU wave-slot cap — request
25 088 B against 65 536 B of LDS per compute unit, with the LDS budget admitting
83 blocks (§4). **If the mapping's kernels used the resources their design
states, every one of them would sit at the ceiling.**

**Level 2 — the compiler's realised allocation limits four of the eleven, and
the resource is per-lane vector registers.** Wherever a bound exists the measured
allocation exceeds it, by 2.44× to 9.62×, and on two of those seven kernels the
excess crosses the threshold at which a wave slot is lost. Two further kernels
fall below the ceiling with no committed budget to exceed:

| Kernel | stated budget | measured `VGPRs` | over the budget | occupancy waves/SIMD |
| --- | ---: | ---: | ---: | ---: |
| `f5_byte_control_kernel` | 8–12 units | 77 | 6.42–9.62× | 12 |
| `f5_three_plane_kernel` | 11 units | 66 | 6.00× | 12 |
| `permanent_bipedal5_kernel` | none committed | 128 | — | 8 |
| `permanent_bipedal7_kernel` | none committed | 128 | — | 8 |

The two $\mathbb{F}_5$ prototypes are the only lane-owns-interval kernels held
below the ceiling, at 12 waves/SIMD, and the resource that holds them there is
the compiler's per-lane vector-register allocation rather than the design's
budget or any shared-memory pressure — the same allocation that leaves the
$\mathbb{F}_3$ and $\mathbb{F}_7$ prototypes at 16. The two shipped
byte-arithmetic kernels sit at 8, halved by the $63 \times 63$ private column
table their mapping keeps per thread, which is the contrast the campaigns
measure on both sides (§4).

**Level 3 — at runtime nothing loses occupancy to any resource; the limiter is
grid width.** Across all 135 counted dispatches of this run, residency of the
waves each dispatch launched is 0.6065 to 1.0000, and 23 of the 35 launch-width
groups hold 0.94 or more of their launched waves resident (§5). A dispatch that
could not place a wave because registers or shared memory were exhausted would
show residency below 1 with slots left over; instead the two widest launches in
this run, 1 024 waves against 2 560 slots, read 35.2175 % and 26.3746 % against
the 40.0000 % that width allows. **What separates every achieved figure from its per-SIMD
ceiling is the number of waves the launch supplies, not the device's ability to
hold them.**

The grid width is set by the batch, and for the six lane-owns-interval paths the
batch is what each cell's single-matrix probe calibrates: 2 to 13 matrices in
this run's profiled cells, hence 2 to 13 waves against 2 560 slots, beside the
one-wave probe that opens each cell. The retained paths whose batch is fixed
rather than calibrated are the three shipped ones, at $M \in \{256, 1024\}$, and
they are the ones that reach tens of percent. The
committed $\mathbb{F}_7$ evidence carries the study's single wide prototype
launch, `wave_gf7_lookup_table_kernel<1>` at 1 614 waves reading 49.4160 %
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13), and it is the
highest achieved occupancy anywhere in the study.

**Verdict.** The per-lane register budget and per-block shared-memory allocation
that the lane-owns-interval mapping states is not what limits occupancy for any
retained path. Where a kernel's per-SIMD occupancy falls below the architectural
ceiling — the two $\mathbb{F}_5$ prototypes at 12 waves/SIMD and, on the control
mapping, the two shipped byte-arithmetic kernels at 8 — the limiting resource is
the compiler's realised per-lane vector-register allocation, measured in the
kernel-resource receipt of §4 and confirmed at runtime by the per-dispatch
`VGPR_Count` of §5. Where achieved occupancy falls below that per-SIMD ceiling,
which is everywhere in this study, the limiting quantity is the launch width the
batch calibration supplies, measured as the launch geometry and residency of §5.
Neither instruction scheduling nor the scattered shared-memory read of the
column table is reachable by this evidence: no counter in it separates them, and
none is named as the limiter here.

## 7. Transfer and launch overhead, separate from kernel time (REQ-05)

The three campaigns commit four device-event columns and one host-clock column
per cell, and this section reads them rather than re-measuring: `kernel_device_s`
is a device-event kernel-only total, `h2d_device_s` and `d2h_device_s` are
device-event transfer totals, `device_submission_to_kernel_s` is the device-event
total between the pre-submission marker and kernel start, and
`host_submission_s` is a host clock. No column subtracts a host timestamp from a
device one, and a CPU or unexecuted cell leaves all five empty with
`phase_timing_note` naming why rather than substituting a wall clock
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.3,
[`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.3,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.3). Each cell's `reps` is
its launch count, because one timed repetition evaluates one batch and one batch
evaluation is one dispatch
(`dev/research/permanent-sampling-feas/src/protocol.rs:765-781`); the one
exception is the $\mathbb{F}_7$ bit-sliced path, whose single event span brackets
the staging launch and the Gray-walk launch together
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §12).

Per launch, over the 52 device cells of the three grids that carry spans.

| $q$ | $n$ | Path | $M$ | outcome | `reps` | launch overhead µs | H2D µs | D2H µs |
| ---: | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: |
| 3 | 12 | `gpu_hip` | 256 | measured | 2 290 | 4.8541 | 12.87 | 9.32 |
| 3 | 12 | `gpu_hip` | 1 024 | measured | 999 | 9.9600 | 37.54 | 9.30 |
| 3 | 12 | `wave-gf3` | 44 | measured | 22 803 | 4.8455 | 9.45 | 16.34 |
| 3 | 12 | `fold-gf3` | 45 | measured | 23 902 | 4.8438 | 9.45 | 16.48 |
| 3 | 20 | `gpu_hip` | 256 | measured | 42 | 4.8571 | 16.24 | 9.62 |
| 3 | 20 | `gpu_hip` | 1 024 | measured | 24 | 4.6250 | 25.83 | 9.38 |
| 3 | 20 | `wave-gf3` | 41 | measured | 1 045 | 4.8555 | 12.07 | 16.80 |
| 3 | 20 | `fold-gf3` | 448 | measured | 756 | 4.6442 | 17.29 | 16.23 |
| 3 | 28 | `gpu_hip` | 256 | measured | 5 | 4.8000 | 21.80 | 9.80 |
| 3 | 28 | `gpu_hip` | 1 024 | censored | 3 | 5.0000 | 54.33 | 10.33 |
| 5 | 20 | `gpu_hip` | 256 | measured | 5 | 5.0000 | 17.00 | 9.80 |
| 5 | 20 | `gpu_hip` | 1 024 | measured | 5 | 5.0000 | 33.60 | 9.80 |
| 5 | 20 | `f5-byte-control` | 6 | measured | 15 | 4.8667 | 9.27 | 20.33 |
| 5 | 20 | `f5-three-plane` | 41 | measured | 901 | 4.8602 | 12.07 | 16.78 |
| 5 | 28 | `f5-byte-control` | 1 | censored | 2 | 5.0000 | 13.00 | 31.50 |
| 5 | 28 | `f5-three-plane` | 2 | measured | 5 | 4.8000 | 9.80 | 23.40 |
| 7 | 20 | `gpu_hip` | 256 | measured | 5 | 5.0000 | 16.60 | 9.60 |
| 7 | 20 | `gpu_hip` | 1 024 | measured | 5 | 5.0000 | 33.20 | 10.00 |
| 7 | 20 | `f7-lookup-table-control` | 29 | measured | 71 | 4.8873 | 9.76 | 17.48 |
| 7 | 20 | `f7-three-plane-permanent` | 41 | measured | 1 023 | 4.8485 | 11.85 | 16.76 |

`analysis.py` section 7 prints all 52 rows. The twenty above carry the minimum
and the maximum of each of the three per-launch columns, so the table spans the
full observed range of each rather than a window inside it.

**Launch overhead is a device-wide constant on this host.**
`device_submission_to_kernel_s` per launch is 4.6250–9.9600 µs over all 52
device cells, across three fields, nine paths, five orders, and kernel spans
from 0.022 ms to 115.5 s — six orders of magnitude of kernel time under a
2.15× spread of launch overhead. Every cell but one sits in 4.6250–5.0000 µs;
the single outlier is $q = 3$, $n = 12$, `gpu_hip` at $M = 1024$ with 9.9600 µs,
which is that path's smallest-order cell and the one cell whose H2D copy is also
the largest of its field at 37.54 µs per launch.

**Device-to-host transfer is flat and host-to-device tracks the batch.** D2H is
9.30–31.50 µs per launch because a launch returns at most $M$ result words; H2D
is 9.27–54.33 µs and rises with $M$ and $n$, since it carries $M n^2$ matrix
bytes. Both are one to four orders of magnitude below the kernel spans of §8 at
every order above $n = 12$.

**A prototype row's `host_submission_s` is not a launch overhead and is not read
as one.** On `gpu_hip` rows that column is the instrumented dispatcher's
asynchronous submission span, 3.2–9.4 µs per launch; on prototype rows it is a
host clock wrapped around the candidate's whole synchronous batch evaluation,
which reaches 115 453 789.5 µs per launch on the $\mathbb{F}_5$ byte control at
$n = 28$ — the kernel span itself, not an overhead. The dual semantics are the
campaigns' own finding and are tracked as bug `79c5ace7`. The device-clock
column above is the launch measure comparable across all nine paths.

## 8. Sustained-kernel duration against order and per-launch work (REQ-06)

Sustained-kernel duration per launch is `kernel_device_s` divided by that cell's
`reps`, a device-event span that excludes allocation, transfer, host
serialisation, and submission
(`dev/research/permanent-sampling-feas/src/usage.txt`, GRID TIMING COLUMNS).
Per-launch work is the batch times the Ryser index space, $M \cdot 2^n$, the
quantity the archived calibration of §9 budgets; the Ryser-weighted form
$M \cdot n \cdot 2^n$ is carried beside it because Ryser's cost per matrix is
$n 2^n$ rather than $2^n$.

Every measured cell of the shipped path, the one path with the same two batch
sizes at every order:

| $q$ | $n$ | $M$ | $M \cdot 2^n$ | s per launch | $q$ | $n$ | $M$ | $M \cdot 2^n$ | s per launch |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 12 | 256 | 1.0486e+06 | 0.000488 | 5 | 12 | 256 | 1.0486e+06 | 0.021772 |
| 3 | 12 | 1 024 | 4.1943e+06 | 0.001337 | 5 | 12 | 1 024 | 4.1943e+06 | 0.041598 |
| 3 | 16 | 256 | 1.6777e+07 | 0.007407 | 5 | 16 | 256 | 1.6777e+07 | 0.412645 |
| 3 | 16 | 1 024 | 6.7109e+07 | 0.012796 | 5 | 16 | 1 024 | 6.7109e+07 | 0.830140 |
| 3 | 20 | 256 | 2.6844e+08 | 0.117988 | 5 | 20 | 256 | 2.6844e+08 | 7.868477 |
| 3 | 20 | 1 024 | 1.0737e+09 | 0.203917 | 5 | 20 | 1 024 | 1.0737e+09 | 16.006792 |
| 3 | 24 | 256 | 4.2950e+09 | 1.862555 | 7 | 12 | 256 | 1.0486e+06 | 0.025113 |
| 3 | 24 | 1 024 | 1.7180e+10 | 3.270548 | 7 | 12 | 1 024 | 4.1943e+06 | 0.046236 |
| 3 | 28 | 256 | 6.8719e+10 | 29.954639 | 7 | 16 | 256 | 1.6777e+07 | 0.474853 |
| 3 | 28 | 1 024 | 2.7488e+11 | 53.077521 | 7 | 16 | 1 024 | 6.7109e+07 | 0.918663 |
| | | | | | 7 | 20 | 256 | 2.6844e+08 | 9.127954 |
| | | | | | 7 | 20 | 1 024 | 1.0737e+09 | 17.668560 |

Every measured cell of the six lane-owns-interval paths, at their own
probe-calibrated batches:

| $q$ | $n$ | Path | $M$ | $M \cdot 2^n$ | s per launch |
| ---: | ---: | --- | ---: | ---: | ---: |
| 3 | 12 | `wave-gf3` | 44 | 1.8022e+05 | 0.000036 |
| 3 | 12 | `fold-gf3` | 45 | 1.8432e+05 | 0.000022 |
| 3 | 16 | `wave-gf3` | 45 | 2.9491e+06 | 0.000292 |
| 3 | 16 | `fold-gf3` | 44 | 2.8836e+06 | 0.000200 |
| 3 | 20 | `wave-gf3` | 41 | 4.2992e+07 | 0.004395 |
| 3 | 20 | `fold-gf3` | 448 | 4.6976e+08 | 0.003779 |
| 3 | 24 | `wave-gf3` | 17 | 2.8521e+08 | 0.069963 |
| 3 | 24 | `fold-gf3` | 22 | 3.6910e+08 | 0.047326 |
| 3 | 28 | `wave-gf3` | 2 | 5.3687e+08 | 1.117985 |
| 3 | 28 | `fold-gf3` | 3 | 8.0531e+08 | 0.756677 |
| 5 | 12 | `f5-byte-control` | 45 | 1.8432e+05 | 0.000889 |
| 5 | 12 | `f5-three-plane` | 44 | 1.8022e+05 | 0.000037 |
| 5 | 16 | `f5-byte-control` | 102 | 6.6847e+06 | 0.018871 |
| 5 | 16 | `f5-three-plane` | 45 | 2.9491e+06 | 0.000349 |
| 5 | 20 | `f5-byte-control` | 6 | 6.2915e+06 | 0.348612 |
| 5 | 20 | `f5-three-plane` | 41 | 4.2992e+07 | 0.005146 |
| 5 | 24 | `f5-byte-control` | 1 | 1.6777e+07 | 6.411601 |
| 5 | 24 | `f5-three-plane` | 17 | 2.8521e+08 | 0.079969 |
| 5 | 28 | `f5-byte-control` | 1 | 2.6844e+08 | 115.452922 |
| 5 | 28 | `f5-three-plane` | 2 | 5.3687e+08 | 1.267296 |
| 7 | 12 | `f7-lookup-table-control` | 5 833 | 2.3892e+07 | 0.000572 |
| 7 | 12 | `f7-three-plane-permanent` | 46 | 1.8842e+05 | 0.000040 |
| 7 | 16 | `f7-lookup-table-control` | 695 | 4.5548e+07 | 0.003084 |
| 7 | 16 | `f7-three-plane-permanent` | 1 109 | 7.2679e+07 | 0.001048 |
| 7 | 20 | `f7-lookup-table-control` | 29 | 3.0409e+07 | 0.070761 |
| 7 | 20 | `f7-three-plane-permanent` | 41 | 4.2992e+07 | 0.004672 |
| 7 | 24 | `f7-lookup-table-control` | 2 | 3.3554e+07 | 1.266041 |
| 7 | 24 | `f7-three-plane-permanent` | 18 | 3.0199e+08 | 0.067608 |
| 7 | 28 | `f7-lookup-table-control` | 1 | 2.6844e+08 | 21.722805 |
| 7 | 28 | `f7-three-plane-permanent` | 3 | 8.0531e+08 | 0.986259 |

**Duration is a function of order and work per path, and the two axes do not
collapse into one.** Two readings make that concrete.

*At fixed order, quadrupling the batch does not quadruple the span.* The shipped
path is measured at $M = 256$ and $M = 1024$ at every order it runs, and the
span step is 1.7274–2.0343× for a 4.00× batch step at every order above
$n = 12$, and 2.7381× at $q = 3$, $n = 12$. A launch of 256 single-lane waves
occupies a tenth of the device's 2 560 wave slots (§5), so a fourfold batch
buys most of its extra work back in parallelism rather than spending it in
time.

*At fixed work, the span varies by three orders of magnitude across paths.*
Five measured cells carry exactly $M \cdot 2^n = 2.6844 \times 10^8$: the
shipped path at $n = 20$, $M = 256$ measures 0.117988 s at $q = 3$, 7.868477 s
at $q = 5$, and 9.127954 s at $q = 7$; the $\mathbb{F}_7$ lookup control at
$n = 28$, $M = 1$ measures 21.722805 s; and the $\mathbb{F}_5$ byte control at
the same order and batch measures 115.452922 s. The spread at one value of the
work axis is **978.5×**. `analysis.py` section 8 recomputes both readings.

The consequence for §9 is direct: a bound on $M \cdot 2^n$ is not a bound on
per-launch device time, and this study reports both axes rather than one.

## 9. Watchdog-safe per-launch work bound (REQ-07)

### 9.1 The prior, and the terms it is cited on

The only committed numeric bound before this study is an archived calibration:
[`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`](../../archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md)
§2.5 places a hang boundary at "*≈190–200 s*" per launch, reports bounded
sub-batches holding every launch at "*≈10–117 s*", sets per-field work budgets
on $\text{sub\_batch} \cdot 2^n$ of $4.0 \times 10^9$ at $q = 3$,
$1.3 \times 10^9$ at $q = 5$, and $3.5 \times 10^8$ at $q = 7$ with a 400 ms
host cooldown, and states that the $\mathbb{F}_3$ kernel "*never tripped it,
even on a 2300 s single launch at n=32*".

That calibration is cited here as a prior and never as an established device
property, because the mechanism it names was retracted for the one observation
that motivates it. The single recorded device fault,
[`../b488f02c/gpu-hang-2026-08-07.log`](../b488f02c/gpu-hang-2026-08-07.log),
records "*nothing here attributes the hang to a watchdog timeout: that is one
hypothesis among others (driver defect, memory pressure, a transient), and no
diagnostic was captured that would separate them*", and states that the file
"*supports NO claim in the study*". The campaign plan directs the same reading
([`../../active/0de41c82/plan.md`](../../active/0de41c82/plan.md):13), and the
study's investigation records the two documents as disagreeing and forbids
papering over it
([`../../active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):352-358).

### 9.2 What this study's own measurements establish, per field

The bound this study derives is an *observed clean-completion envelope*: the
largest per-launch work and the longest per-launch span at which a launch of a
retained path completed without device fault, on this host, at the pinned
binary. Each is a cell of a committed grid, named with its outcome and its
launch count. A cell the grid records as `censored` after executing retains its
device spans and its launches completed; the censoring is the harness's
stopping rule ending the cell, not a device failure
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.3).

| $q$ | Largest per-launch work observed | its span | Longest per-launch span observed | its work | archived work budget | observed work as a multiple of the budget |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | $2.7488 \times 10^{11}$ — $n = 28$, `gpu_hip`, $M = 1024$, `censored`, 3 launches | 53.077521 s | 53.077521 s, the same cell | $2.7488 \times 10^{11}$ | $4.0 \times 10^9$ | **68.72×** |
| 5 | $1.0737 \times 10^{9}$ — $n = 20$, `gpu_hip`, $M = 1024$, `measured`, 5 launches | 16.006792 s | 115.452922 s — $n = 28$, `f5-byte-control`, $M = 1$, `censored`, 2 launches | $2.6844 \times 10^{8}$ | $1.3 \times 10^9$ | 0.83× |
| 7 | $1.0737 \times 10^{9}$ — $n = 20$, `gpu_hip`, $M = 1024$, `measured`, 5 launches | 17.668560 s | 21.722805 s — $n = 28$, `f7-lookup-table-control`, $M = 1$, `measured`, 5 launches | $2.6844 \times 10^{8}$ | $3.5 \times 10^8$ | **3.07×** |

The $q = 3$ cell that carries both extremes is the one the $\mathbb{F}_3$
campaign describes launch by launch: it "*completed 3 repetitions of 53.02–53.20 s
each in 159.27 s of total time … the third repetition finished and the fourth
was not started*"
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §5), so its three launches
are three completions rather than an average over an interrupted cell.

Restricting to cells the grids record as `measured`, the largest $q = 3$ work is
$6.8719 \times 10^{10}$ at $n = 28$, $M = 256$, in a 29.954639 s span, 17.18×
the archived $q = 3$ budget; the $q = 5$ and $q = 7$ figures are unchanged
because their largest-work cells are already `measured`.

**No cell of any of the three campaigns failed with a device fault.** Every step
of the timing run reports `status=completed exit=0` in its run summary, and
every pass of this study's profiled run exits 0. Six committed device cells
carry per-launch work above their field's archived budget, and none of them
faulted:

| cell | work | over budget | span per launch |
| --- | ---: | ---: | ---: |
| $q = 3$, $n = 24$, `gpu_hip`, $M = 256$ | $4.2950 \times 10^{9}$ | 1.07× | 1.862555 s |
| $q = 3$, $n = 24$, `gpu_hip`, $M = 1024$ | $1.7180 \times 10^{10}$ | 4.29× | 3.270548 s |
| $q = 3$, $n = 28$, `gpu_hip`, $M = 256$ | $6.8719 \times 10^{10}$ | 17.18× | 29.954639 s |
| $q = 3$, $n = 28$, `gpu_hip`, $M = 1024$ | $2.7488 \times 10^{11}$ | 68.72× | 53.077521 s |
| $q = 7$, $n = 20$, `gpu_hip`, $M = 1024$ | $1.0737 \times 10^{9}$ | 3.07× | 17.668560 s |
| $q = 7$, $n = 28$, `f7-three-plane-permanent`, $M = 3$ | $8.0531 \times 10^{8}$ | 2.30× | 0.986259 s |

### 9.3 Reconciliation with the recorded fault

The one recorded device fault is a $q = 3$, $n = 24$, `gpu_hip` launch at
$M = 4096$, attempted once while probing the batch ceiling and never retried
([`../b488f02c/gpu-hang-2026-08-07.log`](../b488f02c/gpu-hang-2026-08-07.log);
the harness records the same event and lowers its own sustained probe to
$M = 2048$ because of it,
`dev/research/permanent-sampling-feas/src/main.rs:819-826`). Three of this
study's measurements bear on it directly.

**This study completed a launch of the same kernel at exactly that launch's
work, and one four times larger.** $4096 \cdot 2^{24}$ and $256 \cdot 2^{28}$
are both $6.8719 \times 10^{10}$, so the $q = 3$, $n = 28$, $M = 256$ cell
carries the faulted launch's $M \cdot 2^n$ exactly, and 1.17× its
$M \cdot n \cdot 2^n$; its five launches average 29.954639 s of device kernel
time each and all five completed. The $M = 1024$ cell at the same order carries
4.00× the faulted launch's $M \cdot 2^n$ and 4.67× its Ryser-weighted work, and
its three launches, at 53.02–53.20 s each, all completed.

**The faulted launch's span is not recorded anywhere, and this study bounds it
rather than asserting it.** At the faulted cell's own $(q, n)$ this study
measures 1.862555 s per launch at $M = 256$ and 3.270548 s at $M = 1024$. Under
strictly linear scaling in $M$ — an upper bound, since the measured $256
\rightarrow 1024$ step at that order costs 1.7559× rather than 4× (§8) — a
4 096-matrix launch runs **at most 13.0822 s**, which is 0.0689 of the archived
190 s boundary. The committed sustained receipt at the same cell agrees from a
different direction: it records 55 shards over 180.435 s at $M = 1024$ and 19
over 185.510 s at $M = 2048$, or 3.2806 s and 9.7637 s of wall time per launch
([`../b488f02c/sustained-2026-08-07.csv`](../b488f02c/sustained-2026-08-07.csv)),
which doubles to about 19.5 s at 4 096 matrices. Wall time per shard is itself
an upper bound on the device span, because it carries the host generation work
too, so both routes put the faulted launch an order of magnitude inside the
archived boundary.

**What that does and does not establish.** It establishes that the faulted
launch was, by every committed measurement of the same kernel at the same
order, an order of magnitude inside the archived per-launch span boundary, so a
watchdog timeout at ≈190–200 s does not account for it. That is independent
support for the retraction the fault's own record already carries. It does not
identify the cause: this study captured no diagnostic at a fault, because no
fault occurred in it.

### 9.4 The bound, stated

For each field, the watchdog-safe per-launch bound this study supports is the
envelope of §9.2, and it is a *lower* bound on the safe region rather than a
located boundary:

- $q = 3$: per-launch work to $2.7488 \times 10^{11}$ and per-launch span to
  53.077521 s complete without fault, on `permanent_bipedal3_kernel`.
- $q = 5$: per-launch work to $1.0737 \times 10^{9}$ and per-launch span to
  115.452922 s complete without fault, the work figure on
  `permanent_bipedal5_kernel` and the span figure on `f5_byte_control_kernel`.
- $q = 7$: per-launch work to $1.0737 \times 10^{9}$ and per-launch span to
  21.722805 s complete without fault, the work figure on
  `permanent_bipedal7_kernel` and the span figure on
  `wave_gf7_lookup_table_kernel<2>`.

No launch in this study was run to failure, so none of these is an upper bound
on where faults begin, and no boundary is located. The archived ≈190–200 s span
boundary is neither confirmed nor refuted here: the longest span this study
observes is 115.452922 s, 0.61 of it, and nothing in this evidence probes above
that.

**The per-field work budgets are the wrong shape for the job, and §8 measures
why.** A budget on $M \cdot 2^n$ would bound per-launch device time only if the
device time per unit of that work were roughly constant across the paths it is
applied to. At the one work value five cells share, spans run from 0.117988 s to
115.452922 s, a factor of 978.5. A per-launch *span* target is the quantity that
transfers across paths; a per-launch *work* budget is not, and any use of the
archived budgets outside the single kernel they were calibrated on carries that
error.

## 10. Measurements that contradict the record (REQ-08)

Recorded here with the statements they contradict, per
`@/inv/falsification-preserved`, rather than replacing them.

1. **The archived $q = 3$ work budget is exceeded by 68.72× without a fault.**
   The archived calibration bounds $\text{sub\_batch} \cdot 2^n$ at
   $4.0 \times 10^9$ for $q = 3$; the committed $\mathbb{F}_3$ grid carries four
   `gpu_hip` cells above it, the largest at $2.7488 \times 10^{11}$, all
   completing (§9.2). The archived budget stands in the record as the prior it
   is; it is not a necessary condition for safe operation of this kernel on this
   host.
2. **The archived $q = 7$ work budget is exceeded by 3.07× without a fault**, at
   $q = 7$, $n = 20$, `gpu_hip`, $M = 1024$, $1.0737 \times 10^{9}$ against
   $3.5 \times 10^8$, in a 17.668560 s span; and by 2.30× on
   `f7-three-plane-permanent` at $n = 28$ (§9.2).
3. **A per-launch work budget does not bound per-launch device time.** Five
   committed cells share $M \cdot 2^n = 2.6844 \times 10^{8}$ and their spans
   differ by 978.5× (§8). The archived calibration's premise — that a per-field
   budget on that quantity keeps every launch inside a span band — does not hold
   across the paths this study measures.
4. **The archived statement that the $\mathbb{F}_3$ kernel "never tripped it" is
   not the whole record.** The one device fault on record is a $q = 3$ launch on
   that same shipped kernel
   ([`../b488f02c/gpu-hang-2026-08-07.log`](../b488f02c/gpu-hang-2026-08-07.log)).
   The two are not strictly in conflict, because that fault's attribution to a
   watchdog was retracted and the archived sentence is about the watchdog; both
   are recorded here so the reader does not meet either alone.
5. **The runtime record shares the compiler's dynamic-LDS blind spot.**
   rocprofv3's per-dispatch `LDS_Block_Size` reads 0 for every kernel in this
   study's counter CSVs, including launches that request 144 to 576 bytes of
   dynamic shared memory at the orders it profiles (§2). The one runtime reading of a dynamic table in
   the study's evidence comes from the $\mathbb{F}_7$ run's `rocprofv2` probe.
   Tracked as JIT issue `023233c5`.

**Nothing here contradicts a committed register or shared-memory budget of the
lane-owns-interval mapping.** Seven of the eleven retained-path kernels have a
committed per-lane register budget, every one of them a source-level lower bound
on stable lane state that excludes loop temporaries and compiler allocation, and
every measured allocation is above its bound, by 2.44× to 9.62× (§4). The four
kernels with none are recorded as having none, verified by a sweep of every HIP
translation unit under the two device-source roots that `analysis.py` section 4
asserts. On the shared-memory side the compiler's static field observes nothing,
so it can neither confirm nor refute, and the one runtime reading of a dynamic
table confirms its launch formula at the allocator's granularity.

## 11. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §3 | **Satisfied.** Wave utilization is reported for each of the nine retained GPU permanent paths as the share of a 32-lane wavefront its execution mapping makes active: 3.1250 % for the three shipped one-thread-per-matrix paths and 100 % for the six lane-owns-interval paths, each exact by construction and cited to its launch site. The lane-owns-interval partition's tail-interval idleness is zero at every measured order because $2^n \bmod 32 = 0$, which `analysis.py` section 3 asserts, and the source's lanes-per-block is checked against the profiled dispatches' own `Workgroup_Size`. The phases around the Gray walk are decomposed into lane-steps and reported as a bracket under two readings of the reduction; `prepare_three_plane_columns`, whose whole body is a strided column loop, is reported separately at $n/32$. The derivation's basis — launch geometry and source loop bounds, not a per-lane counter — is stated. |
| REQ-02 | §4 | **Satisfied.** Registers per thread, private scratch memory per thread, shared memory per block, spill counts, and the occupancy-limiting resource are reported for all eleven kernels of the nine retained paths. Every compiler figure is cited to `dev/studies/6c7fcb38/hip-resource-usage-20260814T172506Z-1610002/receipt.txt` and its named per-kernel log, and none is re-measured; the three field campaigns read the same receipt in their own §7. Per-block shared memory is reported from the launch formulas because the compiler's field is static-LDS only, which the campaigns state and this receipt repeats rather than reading a 0 as an absence. The limiter naming is checked by `analysis.py` against all 26 receipt entries. |
| REQ-03 | §5 | **Satisfied.** Achieved occupancy comes from `MeanOccupancyPerCU`, a profiler counter collected per dispatch while the workload executed, for all eleven kernels: six from this study's paired profiled run, five cited to the committed $\mathbb{F}_7$ run. Each figure names its pass, launch width, and dispatch count, and is expressed against the device's own 2 560 wave slots from the profiler's agent report. Admissibility is the stated physical test the $\mathbb{F}_7$ study committed, applied per dispatch by `analysis.py`; all 135 counted dispatches pass it and none is discarded. Compiler resource-usage output never fills the achieved column: §4 shows the compiler's occupancy field is a function of `VGPRs` alone and §5 states that reading. The not-measured escape does not arise, because `rocprofv3` runs this workload on this host and returns counters. |
| REQ-04 | §6 | **Satisfied.** The verdict is stated across the study's kernels at three levels, each with the measurement behind it. The lane-owns-interval mapping's stated per-lane register budget implies the wave-slot ceiling for all seven kernels that have one, and its per-block shared-memory allocation leaves the device's LDS with headroom above the per-CU wave-slot cap, so neither is the limiter anywhere. Where per-SIMD occupancy falls below the ceiling — four of eleven kernels — the resource that limits it is the compiler's realised per-lane vector-register allocation, from the receipt of §4 and confirmed at runtime by `VGPR_Count` in §5. Where achieved occupancy falls below that ceiling, which is every cell in the study, the limiting quantity is the launch width the batch calibration supplies, measured as launch geometry and residency in §5. §6 states which candidate mechanisms this evidence cannot rule on. |
| REQ-05 | §7 | **Satisfied.** Host-to-device transfer, device-to-host transfer, and launch overhead are each a separate device-event column of the field campaigns' grids, reported per launch beside kernel time over all 52 device cells that carry spans, and cited to those columns rather than re-measured. `device_submission_to_kernel_s` is the launch measure used, and §7 states why a prototype row's `host_submission_s` is not one, citing bug `79c5ace7`. |
| REQ-06 | §8 | **Satisfied.** Sustained-kernel duration per launch is reported for every measured device cell of the three fields against both matrix order and per-launch work, in the two work forms $M \cdot 2^n$ and $M \cdot n \cdot 2^n$, all measured on the target device by the campaigns' device-event `kernel_device_s` column. Two readings show the two axes do not collapse: at fixed order a 4.00× batch step costs 1.73–2.03× the span, and at one fixed work value the span varies by 978.5× across paths. |
| REQ-07 | §9 | **Satisfied.** A per-field bound is derived from this study's own measurements and stated with the cell that supports it: per-launch work to $2.7488 \times 10^{11}$ and span to 53.077521 s at $q = 3$, to $1.0737 \times 10^{9}$ and 115.452922 s at $q = 5$, and to $1.0737 \times 10^{9}$ and 21.722805 s at $q = 7$, each naming its order, path, batch, outcome, and launch count. §9.4 states that these are lower bounds on the safe region rather than located boundaries, because no launch in this study was run to failure. The reconciliation with the archived calibration is explicit: six committed cells exceed their field's archived work budget without fault, one of them carrying exactly the recorded fault's $M \cdot 2^n$ and 1.17× its Ryser-weighted work; the faulted launch's own span is bounded above at 13.0822 s from this study's measurements at the same cell, an order of magnitude inside the archived boundary, which is independent support for the retraction that fault's record already carries. |
| REQ-08 | §10 | **Satisfied.** Five contradictions are recorded with the statements they contradict rather than replacing them: the archived $q = 3$ and $q = 7$ work budgets exceeded 68.72× and 3.07× without fault, the 978.5× span spread that falsifies a work budget's premise as a bound on device time, the archived "never tripped it" sentence against the one recorded $q = 3$ device fault, and rocprofv3's runtime `LDS_Block_Size` reading 0 for launches that request dynamic shared memory. §10 also states, and `analysis.py` checks, that nothing here contradicts a committed register or shared-memory budget. |
| REQ-09 | §1 | **Satisfied.** The new evidence is reproduced by one committed command, `dev/studies/a9284086/profiled-run.sh`, which verifies the binary against the manifest before its first pass and holds `dev/scripts/ccx1-bench-flock.sh --full-host` for every pass; `run.log` records all 34 commands verbatim with their exit codes, and `analysis.py` asserts the pass inventory and the zero exits. The consumed evidence is reproduced by its own campaigns' committed commands. `analysis.py` regenerates every table here from committed artifacts with one command. |

## 12. What this qualification does not establish

Collected so a reader does not have to reassemble it from the sections above.

1. **No watchdog boundary is located.** §9's bound is an envelope of observed
   clean completions, not an upper bound on safe operation. No launch in this
   study was run to failure, and the longest span it observes is 115.452922 s,
   0.61 of the archived ≈190–200 s figure, so nothing here tests above that.
2. **The recorded fault's cause remains unidentified.** This study bounds that
   launch's span from its own measurements and shows a watchdog timeout at the
   archived boundary does not account for it, which is what its own record
   already says. No diagnostic separating the remaining hypotheses exists,
   because no fault occurred in this study to capture one from.
3. **The faulting configuration is not re-run, and cannot be from the pinned
   binary.** The grid's GPU batch set is fixed at $\{256, 1024\}$
   (`dev/research/permanent-sampling-feas/src/main.rs:50`) and the `sustained`
   mode's run list is a hard-coded ten-row table whose largest GPU batch is
   2 048 (`main.rs:827-838`), so no launch above $M = 2048$ is reachable without
   rebuilding the binary and breaking the hash pin this evidence rests on.
4. **No $\mathbb{F}_3$ or $\mathbb{F}_5$ prototype cell launches a wide grid.**
   Their profiled launch widths are 2 to 13 waves against the device's 2 560
   slots, because each cell's batch is what its own single-matrix probe
   calibrates, so their achieved occupancy measures the calibrated batch rather
   than the mapping's ceiling. The study's one wide prototype launch is the
   $\mathbb{F}_7$ lookup control at 1 614 waves, committed elsewhere.
5. **Dynamic shared-memory allocation is unobserved at runtime for all but one
   kernel.** rocprofv3 reports `LDS_Block_Size` 0 for every dispatch in this
   run; the only runtime reading of a dynamic table in the study's evidence is
   the $\mathbb{F}_7$ walk kernel at $n = 20$ from a `rocprofv2` probe, and the
   confirmation pass is tracked as JIT issue `023233c5`.
6. **Wave utilization is derived rather than measured.** No counter in this
   evidence observes lane activity; §3's shares follow from the launch geometry
   and the source loop bounds, and its lane-step decomposition is not
   time-weighted because no per-phase device timing exists here.
7. **The profiled run carries no timing authority.** Its durations, batch sizes,
   and cell outcomes are the profiled workload's own record and are never used
   as throughput or timing results; §1 names the one cell whose outcome differs
   from the timing run under tracing.
8. **Architecture scope is one device.** Every figure here is `gfx1030` on the
   one measured host. The architecture-scope statement and its compile evidence
   are a separate deliverable, and nothing in this receipt claims anything about
   another architecture.
9. **Neither instruction scheduling nor the shared-memory access pattern is
   ruled on.** They are the study's other candidate explanations for occupancy
   pressure, and no counter in this evidence separates them; §6 names the
   limiters it can measure and stops there.
