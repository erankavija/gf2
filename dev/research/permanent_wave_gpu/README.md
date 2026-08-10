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

This is a preserved falsification of literal full-permanent execution at
order 63: `2^63` Ryser terms are not presented as a passing test. Instead the
host and device checks retain exact interval union/disjointness and
interval-start Gray-subset assertions at that order, plus active-tail-mask
product probes. The device-touching Rust test is feature-gated and ignored with
its required ROCm device reason. The separately invoked self-checking HIP
executable provides the actual device evidence, and its resource receipt is
recorded beside `hip/wave_gf3_equivalence.hip` after the checked gfx1030 build.

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
[`hip/wave_gf3_resource_usage.log`](hip/wave_gf3_resource_usage.log), SHA-256
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

The default `fixture-oracle` feature retains the corpus and oracle surface,
which imports the harness's canonical sampler. When the harness imports this
crate's registry, it disables that feature and enables its own default
`prototype-registry` adapter instead. This mutually exclusive feature boundary
avoids a Cargo dependency cycle while keeping `MeasurementPath::ALL` available
in both directions as the sole candidate list.
