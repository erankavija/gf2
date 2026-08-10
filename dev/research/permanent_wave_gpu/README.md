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

The candidate list is deliberately complete from the first commit and is
authoritatively defined by `MeasurementPath::ALL`. Add a candidate
implementation only by replacing its stub in the owning candidate module and
adding that candidate's HIP and test files. Do not add a second registry or
edit `src/lib.rs` or `src/paths.rs`; the measurement driver always enumerates
`MeasurementPath::ALL` and dispatches through that one registry.

The default `fixture-oracle` feature retains the corpus and oracle surface,
which imports the harness's canonical sampler. When the harness imports this
crate's registry, it disables that feature and enables its own default
`prototype-registry` adapter instead. This mutually exclusive feature boundary
avoids a Cargo dependency cycle while keeping `MeasurementPath::ALL` available
in both directions as the sole candidate list.
