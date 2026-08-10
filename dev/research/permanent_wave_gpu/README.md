# Wave-parallel permanent HIP prototype

This standalone research crate is the implementation home for the planned
wave-parallel permanent candidates over $\mathbb{F}_3$, $\mathbb{F}_5$, and
$\mathbb{F}_7$. It intentionally remains outside the default workspace so
candidate experiments and their rejected paths stay independently buildable.

Run the host-only registry tests with:

```sh
cargo test --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release
```

## Deterministic correctness corpus

The host-only test suite also constructs the shared fixture corpus with root
seed `0x6D0F_F83C_CAFE_2026`.  Reproduce its deterministic byte-level matrix
data and CPU-reference checks with:

```sh
cargo test --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release \
    --test fixtures_oracle
```

The corpus is addressed through the feasibility study's existing
`MeasurementPurpose::Equivalence` stream domain; it does not introduce another
randomness-purpose registry.  It includes structural empty, singleton, Gray
partition, add/subtract, partial-word, zero-product, and nonzero exponent-class
fixtures for each supported field, plus $\mathbb{F}_7$ orders 16, 20, and 24.
The report retains every `MeasurementPath::ALL` entry.  A candidate that is not
yet executable receives an explicit unavailable reason instead of disappearing
from correctness evidence.

For a fixed nonempty subset, the zero fast path and all-nonzero slow path are
reported per $(q,n)$ cell beside the exact complementary expectations
$1 - ((q-1)/q)^n$ and $((q-1)/q)^n$, respectively.  The observations are a
fixed-subset diagnostic for the deterministic corpus, not a performance number.

On a ROCm host, the opt-in HIP feature compiles every source under `hip/` and
links the prebuilt F_7 arithmetic-equivalence executable. Run it with:

```sh
cargo test --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release \
    --features hip --test hip_f7_three_plane
```

The executable launches one device thread and exhaustively checks all 49
ordered F_7 Mersenne add/subtract pairs and all $7^3$ three-lane active
zero-mask/$C_6$ product triples. It is an arithmetic conformance check, not a
permanent kernel: the host fixture corpus remains the evidence for permanent
equality at orders 16, 20, and 24, and `cc162697` owns permanent-shaped device
paths.

### Device arithmetic receipt — 2026-08-10

This non-performance conformance run was made from clean implementation commit
`df5a51f6` (`fix(jit:7b998fb9): add device arithmetic conformance`) on an AMD
Radeon RX 6950 XT (`gfx1030`, UUID `GPU-8cd14d6d8a3c8a73`) with ROCm HIP
`7.2.53211-9999` and AMD clang `22.0.0git`. The command was:

```sh
cargo +1.95.0 test --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --test hip_f7_three_plane -- --nocapture
```

It passed `device_f7_three_plane_arithmetic_is_exact`: release build 2.56 s,
device test 0.06 s, command wall time 2.64 s. The result confirms device
arithmetic only; it neither measures performance nor claims a device permanent
implementation.

The candidate list is deliberately complete from the first commit and is
authoritatively defined by `MeasurementPath::ALL`. Add a candidate
implementation only by replacing its stub in the owning candidate module and
adding that candidate's HIP and test files. Do not add a second registry or
edit `src/lib.rs` or `src/paths.rs`; the measurement driver always enumerates
`MeasurementPath::ALL` and dispatches through that one registry.

## F_3 wave-cooperative evidence boundary

`WaveGf3` is the first executable intra-matrix candidate. It partitions the
full sequential Gray range among at most 32 lanes, initializes every lane from
the canonical subset at that lane's interval start, and combines only scalar
partials in lane order. The host fixture/oracle path checks every tractable
committed F_3 fixture through order 16. Larger F_3 corpus cells, including the
order-63 partial-word fixture, remain explicit `Unsupported` rows whose reason
names the exponential full-permanent cost; they are not a capability claim or
silently omitted from the canonical report.

Its stable source-level state model is two packed `u64` words plus `u64` Gray
cursor/end bounds and one `u32` partial sum: a 9 x 32-bit-register lower
bound. The current subset and horizontal-product value are loop temporaries;
the committed compiler resource output, rather than this lower bound, is the
authoritative allocation evidence.

This is a preserved falsification of literal full-permanent execution at
order 63: `2^63` Ryser terms are not presented as a passing test. Instead the
host and device checks retain exact interval union/disjointness and
interval-start Gray-subset assertions at that order, plus active-tail-mask
product probes. The device-touching Rust test is feature-gated and ignored with
its required ROCm device reason. The separately invoked self-checking HIP
executable provides the actual device evidence, and its resource receipt is
recorded beside `hip/wave_gf3_equivalence.hip` after the checked gfx1030 build.

For actual element-wise device evidence, use the opt-in driver rather than an
ignored Cargo test. It streams every committed F_3 fixture through the bound,
with its fixture ID, canonical row-major bytes, and independently computed
`permanent_ryser` value, to the prebuilt HIP executable:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf3-device-evidence
```

### F_3 device/resource pre-commit capture — 2026-08-10

This capture is deliberately **not** a clean-revision receipt. It records the
device evidence obtained before the implementation commit so the raw result is
preserved rather than retroactively represented as clean. The final receipt
must repeat the two commands below from the committed, clean implementation
revision before making a clean-provenance claim.

- Base revision: `a816d3b713835339a3f43307d5447dca5c7e9699`.
- Source state: dirty implementation worktree; binary patch SHA-256
  `e5f0a54da2e6bacc69f9db6fed9d23816f47d496424e4c1d2897eae017d9102a`.
- Host: AMD Ryzen 9 5900X; AMD Radeon RX 6950 XT (`gfx1030`, UUID
  `GPU-8cd14d6d8a3c8a73`).
- Toolchain: ROCm HIP `7.2.53211-9999`; AMD clang `22.0.0git`.

The resource command was:

```sh
/opt/rocm/bin/hipcc --offload-arch=gfx1030 -O3 \
    -Rpass-analysis=kernel-resource-usage \
    -c hip/wave_gf3_equivalence.hip \
    -o /tmp/de2522d6-wave_gf3_equivalence.o \
    2> /tmp/de2522d6-wave_gf3_resource.log
```

Its byte-faithful stderr is
[`hip/wave_gf3_resource_usage-initial.log`](hip/wave_gf3_resource_usage-initial.log), SHA-256
`c538c2739cd8a50735c7c7041198c4c4df0bf526c13624d5579a5ac574861a49`.
For `wave_gf3_kernel`, the report gives 22 SGPRs, 22 VGPRs, zero scratch
bytes/lane, zero SGPR/VGPR spills, and occupancy 16 waves/SIMD. It gives zero
LDS bytes/block because it reports static LDS only; this is preserved compiler
output, not evidence that the kernel lacks its launch-time `16n`-byte dynamic
column table.

The directly executed prebuilt self-check was:

```sh
dev/research/permanent_wave_gpu/target/release/build/permanent-wave-gpu-8352961633a25476/out/wave_gf3_equivalence
```

It exited 0 with no diagnostics. Its SHA-256 was
`38c0cc9a9bfa48f2a9256d9426df7f6db37ab51cfbe9caaf5911661620e85a80`.
The executable checks independent CPU-permutation references for the small
structural cases and the device n=63 active-mask product probe; it does not
claim an infeasible n=63 full permanent.

### F_3 device/resource clean-revision receipt — 2026-08-10

This final confirmation was made from clean implementation commit
`18b022d6c6e13b6ae41759ce79a5ed420bd1d03f`
(`feat(jit:de2522d6): add F3 wave Ryser prototype`):
`git status --porcelain=v1 --untracked-files=all` was empty immediately before
the resource capture and direct device execution. The host was the AMD Ryzen 9
5900X / AMD Radeon RX 6950 XT (`gfx1030`, UUID
`GPU-8cd14d6d8a3c8a73`) above, using HIP `7.2.53211-9999` and AMD clang
`22.0.0git`.

The clean compiler command repeated the pre-commit command with output
`/tmp/de2522d6-wave_gf3_equivalence-clean.o` and stderr
`/tmp/de2522d6-wave_gf3_resource-clean.log`. Its 2,165-byte stderr was
byte-identical to the preserved initial raw log
([`hip/wave_gf3_resource_usage-initial.log`](hip/wave_gf3_resource_usage-initial.log);
`cmp` exit 0; SHA-256
`c538c2739cd8a50735c7c7041198c4c4df0bf526c13624d5579a5ac574861a49`),
so no resource difference was observed. It again reports 22 SGPRs, 22 VGPRs,
zero scratch and spills, occupancy 16 waves/SIMD, and zero static LDS; the
dynamic-LDS reporting limitation remains preserved rather than inferred away.

From `dev/research/permanent_wave_gpu`, the clean prebuilt executable command
was:

```sh
target/release/build/permanent-wave-gpu-8352961633a25476/out/wave_gf3_equivalence
```

It exited 0 with no diagnostics; its SHA-256 was
`38c0cc9a9bfa48f2a9256d9426df7f6db37ab51cfbe9caaf5911661620e85a80`.
The clean commit also passed both formatting checks, the default release test
suite, the focused no-default-feature registry test, default and HIP-feature
release Clippy with `-D warnings`, and compilation (without execution) of the
feature-gated ignored HIP test.

### F_3 device/resource review-rework capture — 2026-08-10

This capture is deliberately **not** a clean-revision receipt. It records the
review rework while the source tree was dirty relative to `8a788b03` (the
prior landed F_3 implementation), so the clean commit must repeat both commands
before it makes a clean-provenance claim. The host and toolchain were the
gfx1030 RX 6950 XT / HIP 7.2.53211 / AMD clang 22.0.0git named above.

The direct opt-in evidence command was:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf3-device-evidence
```

It exited 0 in 3.7 seconds and reported equality for all 12 streamed canonical
F_3 fixtures through order 16. In its `--fixture-stdin` mode, this also runs
the n=63 interval/start-subset mapping probe and active-mask product probe
after the fixture comparisons.

The resource command repeated the command above with output
`/tmp/de2522d6-wave_gf3_equivalence-rework.o` and stderr
`/tmp/de2522d6-wave_gf3_resource-rework.log`. Its 4,400-byte stderr is now the
byte-faithful canonical
[`hip/wave_gf3_resource_usage.log`](hip/wave_gf3_resource_usage.log), SHA-256
`4e052d08dab816ea569f9b0d2251d887cdc1d8db523300608c59a57c0a47ad8c`.
`wave_gf3_kernel` remains 22 SGPRs, 22 VGPRs, zero scratch and spills,
occupancy 16 waves/SIMD, and zero reported static LDS. The existing active-mask
probe remains 7 SGPRs and 2 VGPRs. The new n=63 mapping probe reports 7 SGPRs
and 8 VGPRs; the n=4 direction probe reports 9 SGPRs and 9 VGPRs. Both new
probes also report zero scratch and spills, occupancy 16, and zero static LDS.
The initial 2,165-byte capture remains preserved under its explicit
`-initial` filename rather than being overwritten.

### F_3 device/resource clean-rework receipt — 2026-08-10

This confirmation was made from clean reviewed revision
`22201e296b07ad1af20a738cefb56c13378f532d`; `git status --porcelain=v1
--untracked-files=all` was empty immediately before the checks. The host was
an AMD Ryzen 9 5900X with an AMD Radeon RX 6950 XT (`gfx1030`, UUID
`GPU-8cd14d6d8a3c8a73`), using ROCm HIP `7.2.53211-9999` and AMD clang
`22.0.0git`.

Both repository and standalone Rust 1.95 formatting checks passed in 1.3 s
each. The default release suite passed 16 unit tests, 2 fixture-oracle tests,
and the registry test (3.49 s build, 3.94 s test time). The focused
no-default registry test passed (2.17 s build; its reusable mapping warnings
remain the documented registry-only boundary). The host-only evidence
selection/protocol test passed (9.43 s HIP-feature build), as did default and
HIP-feature all-target Clippy with `-D warnings` (0.21 s and 6.93 s), and the
feature-gated ignored HIP test's compile-only command (2.27 s).

The direct non-test device command was:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf3-device-evidence
```

It exited 0 in 2.8 seconds, reporting element-wise equality for all 12
canonical F_3 fixtures through order 16. The executable's fixture-input mode
then ran the n=63 interval/start-subset mapping and active-mask product probes;
this command is direct device evidence, not `cargo test -- --ignored`.

The clean resource command was:

```sh
/opt/rocm/bin/hipcc --offload-arch=gfx1030 -O3 \
    -Rpass-analysis=kernel-resource-usage \
    -c hip/wave_gf3_equivalence.hip \
    -o /tmp/de2522d6-wave_gf3_equivalence-clean-rework.o \
    2> /tmp/de2522d6-wave_gf3_resource-clean-rework.log
```

It exited 0 in 4.1 seconds. `cmp` confirmed its stderr was byte-identical to
the 4,400-byte canonical
[`hip/wave_gf3_resource_usage.log`](hip/wave_gf3_resource_usage.log), with
SHA-256 `4e052d08dab816ea569f9b0d2251d887cdc1d8db523300608c59a57c0a47ad8c`.
The historical 2,165-byte initial capture remains
[`hip/wave_gf3_resource_usage-initial.log`](hip/wave_gf3_resource_usage-initial.log),
SHA-256 `c538c2739cd8a50735c7c7041198c4c4df0bf526c13624d5579a5ac574861a49`.

The default `fixture-oracle` feature retains the corpus and oracle surface,
which imports the harness's canonical sampler. When the harness imports this
crate's registry, it disables that feature and enables its own default
`prototype-registry` adapter instead. This mutually exclusive feature boundary
avoids a Cargo dependency cycle while keeping `MeasurementPath::ALL` available
in both directions as the sole candidate list.
