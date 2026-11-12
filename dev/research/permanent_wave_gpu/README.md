# Wave-parallel permanent HIP prototype

This standalone research crate is the implementation home for the planned
wave-parallel permanent candidates over $\mathbb{F}_3$, $\mathbb{F}_5$, and
$\mathbb{F}_7$. It intentionally remains outside the default workspace so
candidate experiments and their rejected paths stay independently buildable.

## Architecture scope and compile evidence

The claimed architecture scope is exactly `gfx1030`, the target the device
drivers below run on. HIP builds accept an explicit comma-separated
`PERMANENT_WAVE_GPU_OFFLOAD_ARCHES` value and default to `gfx1030`; the value
is passed to every prototype kernel source and executable build. This
compile-time selection does not claim runtime support, portability, or
performance on an architecture without device evidence.

Produce the versioned compile receipt from a fully clean tracked and
untracked worktree on a ROCm host with:

```sh
bash dev/research/permanent_wave_gpu/scripts/compile_arch_evidence.sh
```

The script captures revision and clean-worktree provenance before creating
outputs, uses an isolated target directory, and writes the receipt and the
verbose Cargo, build and toolchain log under `target/permanent_wave_gpu/` at
the repository root only after every attempt finishes. The default receipt
claims only `gfx1030`; set `PERMANENT_WAVE_GPU_EVIDENCE_ARCHES` to record an
explicit extra attempt. The script always includes `gfx1030`, and preserves
any extra architecture's exact outcome and diagnostic without promoting it
into the claimed set.

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

The corpus uses the existing `StreamPurpose::Validation` stream domain;
it does not introduce another
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

The candidate list is deliberately complete from the first commit and is
authoritatively defined by `MeasurementPath::ALL`. Add a candidate
implementation only by replacing its stub in the owning candidate module and
adding that candidate's HIP and test files. Do not add a second registry or
edit `src/lib.rs` or `src/paths.rs`; the measurement driver always enumerates
`MeasurementPath::ALL` and dispatches through that one registry.

## Compiler resource capture

The gfx1030 report of `-Rpass-analysis=kernel-resource-usage` is the
authoritative allocation evidence for every kernel: SGPRs, VGPRs, private
scratch, SGPR/VGPR spills, static LDS and occupancy. Capture it for one source
from this directory with:

```sh
mkdir -p target
/opt/rocm/bin/hipcc --offload-arch=gfx1030 -O3 \
    -Rpass-analysis=kernel-resource-usage \
    -c hip/wave_gf3_equivalence.hip \
    -o target/wave_gf3_equivalence.o \
    2> target/wave_gf3_resource.log
```

and likewise for `hip/f5_wave_equivalence.hip` and
`hip/wave_gf7_equivalence.hip`. The report names each kernel instantiation
separately. Its `LDS Size` field is static LDS only: it excludes the dynamic
shared column tables each launch requests, which are $16n$ bytes for the F_3
folds, $n^2$ bytes for the F_5 byte control, $24n$ bytes for the F_5 and F_7
three-plane kernels, and $8n\lceil n/16 \rceil$ bytes for the F_7 lookup
control. A resource report is allocation evidence, not a timing or throughput
claim.

## F_5 wave device evidence

The device-evidence driver is a normal binary, not an ignored Cargo test:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin f5-wave-device-evidence
```

It streams every committed F_5 fixture admitted by the exact host bound
`n <= 16`, with canonical row-major bytes and an independently calculated
`gf2_algebra::permanent::permanent_ryser` value, and requires both the
byte-control and canonical Packed5 three-plane kernels to match it
element-wise. It then runs the exact n=63 interval mapping, active-mask,
exhaustive ordered F_5 add/subtract, and three-lane C4 product probes. The
n=63 corpus rows remain explicit `Unsupported` full permanents rather than an
infeasible `2^63` device walk.

## F_3 wave-cooperative evidence boundary

`WaveGf3` is the executable intra-matrix control and `FoldGf3` is its
zero-mask/sign-popcount counterpart. They partition the full sequential Gray
range among at most 32 lanes, initialize every lane from the canonical subset
at that lane's interval start, and combine only scalar partials in lane order.
The host fixture/oracle path checks every tractable committed F_3 fixture
through order 16. Larger F_3 corpus cells, including the order-63
partial-word fixture, remain explicit `Unsupported` rows whose reason names
the exponential full-permanent cost; they are not a capability claim or
silently omitted from the canonical report.

Their stable source-level state model is two packed `u64` words plus `u64`
Gray cursor/end bounds and one `u32` partial sum: a 9 x 32-bit-register lower
bound. Neither fold adds persistent mapping state: the halving control uses
only local fold temporaries, and the zero-mask candidate additionally has
local active/zero-mask and popcount temporaries. The compiler resource output
for both separately instantiated kernels, rather than this lower bound, is the
authoritative allocation evidence.

This is a preserved falsification of literal full-permanent execution at
order 63: `2^63` Ryser terms are not presented as a passing test. Instead the
host and device checks retain exact interval union/disjointness and
interval-start Gray-subset assertions at that order, plus active-tail-mask
product probes. The device-touching Rust test is feature-gated and ignored with
its required ROCm device reason. The separately invoked self-checking HIP
executable provides the actual device evidence.

For actual element-wise device evidence, use the opt-in driver rather than an
ignored Cargo test. It streams every committed F_3 fixture through the bound,
with its fixture ID, canonical row-major bytes, and independently computed
`permanent_ryser` value, to the prebuilt HIP executable. The driver selects
only `WaveGf3` or `FoldGf3` through the existing registry entries:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf3-device-evidence -- --path wave-gf3
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf3-device-evidence -- --path fold-gf3
```

In fixture-input mode a successful exit also requires the independent
order-63 interval/start-subset mapping and active-mask product probes; it does
not claim a full order-63 permanent.

The resource report falsifies a zero-allocation reading of the nine-register
source model: neither fold adds persistent mapping state, but the zero-mask
candidate's local mask and popcount work allocates more VGPRs than the halving
control, with no scratch or spills in either. Any performance conclusion is a
separate measurement.

The default `fixture-oracle` feature retains the corpus and oracle surface,
which imports the `permanent-statistics` campaign's canonical sampler. Without
that feature the crate builds only its candidate registry, with
`MeasurementPath::ALL` as the sole candidate list.

## F_7 wave permanent device evidence

The direct, non-test evidence commands are:

```sh
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf7-device-evidence -- \
    --path f7-lookup-table-control
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf7-device-evidence -- \
    --path f7-three-plane-permanent
cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
    --release --features hip --bin wave-gf7-device-evidence -- \
    --path f7-three-plane-permanent --stage three-plane-preparation
```

The lookup control and the three-plane permanent each compare against the
host driver's independent generic `permanent_ryser` value for every committed
F_7 fixture at orders 16, 20, and 24. The preparation stage uses its separate
stream, does not compute Ryser, and succeeds only after copying every prepared
`b0`, `b1`, and `b2` column plane back from the device and comparing it with
the canonical row-major matrix bytes.
