# Wave-parallel permanent HIP prototype

This standalone research crate is the implementation home for the planned
wave-parallel permanent candidates over $\mathbb{F}_3$, $\mathbb{F}_5$, and
$\mathbb{F}_7$. It intentionally remains outside the default workspace so
candidate experiments and their rejected paths stay independently buildable.

Run the host-only registry tests with:

```sh
cargo test --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release
```

On a ROCm host, compile every HIP source under `hip/` (including the no-op
probe before candidate kernels exist) with:

```sh
cargo build --manifest-path dev/research/permanent_wave_gpu/Cargo.toml --release --features hip
```

The candidate list is deliberately complete from the first commit: two F_3
paths, the F_5 byte-control and three-plane paths, the standalone F_7
three-plane accumulator, and the F_7 lookup-table-control and permanent-shaped
three-plane paths. Add a candidate implementation only by replacing its stub
in the owning candidate module and adding that candidate's HIP and test files.
Do not add a second registry or edit `src/lib.rs` or `src/paths.rs`; the
measurement driver always enumerates `MeasurementPath::ALL` and dispatches
through that one registry.
