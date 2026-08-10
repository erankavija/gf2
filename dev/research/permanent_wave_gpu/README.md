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
fixtures for each supported field, plus $\\mathbb{F}_7$ orders 16, 20, and 24.
The report retains every `MeasurementPath::ALL` entry.  A candidate that is not
yet executable receives an explicit unavailable reason instead of disappearing
from correctness evidence.

For a fixed nonempty subset, the zero fast path and all-nonzero slow path are
reported per $(q,n)$ cell beside the exact complementary expectations
$1 - ((q-1)/q)^n$ and $((q-1)/q)^n$, respectively.  The observations are a
fixed-subset diagnostic for the deterministic corpus, not a performance number.

On a ROCm host, compile every HIP source under `hip/` (including the no-op
probe before candidate kernels exist) with:

```sh
cargo build --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release --features hip
```

The candidate list is deliberately complete from the first commit and is
authoritatively defined by `MeasurementPath::ALL`. Add a candidate
implementation only by replacing its stub in the owning candidate module and
adding that candidate's HIP and test files. Do not add a second registry or
edit `src/lib.rs` or `src/paths.rs`; the measurement driver always enumerates
`MeasurementPath::ALL` and dispatches through that one registry.
