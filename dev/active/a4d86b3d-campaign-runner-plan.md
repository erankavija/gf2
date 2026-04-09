# Plan: Configurable Simulation Campaign Runner (a4d86b3d)

## Context

Simulation campaigns are hardcoded in example binaries. Each binary runs a fixed set of curves with hardcoded parameters. There's no way to run a single curve, change parameters, or compose campaigns without editing Rust source. This blocks efficient iteration on the GRAND epic's simulation work.

## Approach

A TOML-based campaign config parsed by a binary at `crates/gf2-coding/src/bin/sim_runner.rs`. Each curve is independently runnable via `--curve` filter. Parallel SNR sweep via `--parallel`. Resume from existing CSV.

## Dependencies to add

In `crates/gf2-coding/Cargo.toml` (regular, not feature-gated):
- `serde = { version = "1", features = ["derive"] }`
- `toml = "0.8"`

## Campaign config format

File: `dev/campaigns/phase1_fig3.toml` (example)

```toml
[campaign]
name = "phase1_fig3"
output_dir = "dev/simulation_results"

[[curve]]
name = "fig3_ebch_product"
type = "product"
component = "ebch_16_11"
turbo = { max_iterations = 20, alpha = 0.5, list_size = 4, max_queries = 1000000 }
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000

[[curve]]
name = "fig3_ldpc_nms"
type = "ldpc"
base_graph = 2
n = 256
k = 121
algorithm = "nms"
scale = 0.75
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000

[[curve]]
name = "fig3_ldpc_sp"
type = "ldpc"
base_graph = 2
n = 256
k = 121
algorithm = "sum_product"
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000
```

For GLDPC:
```toml
[[curve]]
name = "fig7_gldpc"
type = "gldpc"
variant = "lentmaier_1024"
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000
```

## Binary: `src/bin/sim_runner.rs`

```
USAGE:
    sim_runner <campaign.toml> [OPTIONS]

OPTIONS:
    --curve <name>     Run only the named curve (can repeat)
    --parallel         Use parallel SNR sweep (rayon)
    --seed <u64>       Override RNG seed (default: 42)
    --dry-run          Print what would run without executing
```

### Implementation structure

```rust
// Config types (serde deserialize)
#[derive(Deserialize)]
struct CampaignConfig {
    campaign: CampaignMeta,
    curve: Vec<CurveConfig>,
}

#[derive(Deserialize)]
struct CampaignMeta {
    name: String,
    output_dir: String,
}

#[derive(Deserialize)]
struct CurveConfig {
    name: String,
    #[serde(rename = "type")]
    curve_type: CurveType,  // "ldpc", "product", "gldpc"
    // LDPC fields (optional)
    base_graph: Option<u8>,
    n: Option<usize>,
    k: Option<usize>,
    algorithm: Option<String>,  // "nms", "min_sum", "sum_product", "offset_ms"
    scale: Option<f32>,
    // Product fields (optional)
    component: Option<String>,  // "ebch_16_11", "drm_32_21", etc.
    turbo: Option<TurboConfig>,
    // GLDPC fields (optional)
    variant: Option<String>,    // "lentmaier_1024"
    // Common
    snr: SnrRange,
    min_errors: usize,
    max_frames: usize,
}

#[derive(Deserialize)]
struct SnrRange {
    start: f64,
    stop: f64,
    step: f64,
}

#[derive(Deserialize)]
struct TurboConfig {
    max_iterations: usize,
    alpha: f32,
    list_size: usize,
    max_queries: usize,
}
```

### Curve execution

A `run_curve(curve: &CurveConfig, output_dir: &str, parallel: bool, seed: u64)` function that:
1. Builds the `SimulationConfig` from `curve.snr`, `curve.min_errors`, `curve.max_frames`, seed, output_path
2. Matches on `curve.curve_type`:
   - `"ldpc"` → build `Nr5gRateMatchedCode` + `Nr5gRateMatchedDecoder`, use `run_coded_iterative_parallel` (if parallel) or `run_coded_iterative`
   - `"product"` → build `ProductCode` + `TurboDecoder`, use `run_with_decoder` (always sequential per-SNR, product decode is single-threaded)
   - `"gldpc"` → build `QcGldpcCode` + `GldpcDecoder`, use `run_coded_iterative_parallel` or `run_coded_iterative`
3. Prints completion summary

### Component registry

Map string names to constructors:
- `"ebch_16_11"` → `ExtendedBchCode::ebch_16_11()`
- `"ebch_16_7"` → `ExtendedBchCode::ebch_16_7()`
- `"ebch_32_26"` → `ExtendedBchCode::ebch_32_26()`
- `"ebch_64_57"` → `ExtendedBchCode::ebch_64_57()`
- `"drm_32_21"` → `DrmCode::drm_32_21()`
- `"lentmaier_1024"` → `QcGldpcCode::lentmaier_1024()`

Algorithm registry:
- `"nms"` → `DecoderAlgorithm::NormalizedMinSum(scale)` (default scale 0.75)
- `"min_sum"` → `DecoderAlgorithm::MinSum`
- `"sum_product"` → `DecoderAlgorithm::SumProduct`
- `"offset_ms"` → `DecoderAlgorithm::OffsetMinSum(beta)`

## Campaign config files to create

```
dev/campaigns/
  phase1_fig3.toml    — Fig 3: eBCH product + LDPC NMS + LDPC SP (256,121)
  phase1_fig1.toml    — Fig 1: dRM product + LDPC NMS + LDPC SP (1024,441)
  phase3_fig7.toml    — Fig 7: GLDPC + LDPC NMS (1024,646)
```

## Files to create/modify

| File | Action |
|------|--------|
| `crates/gf2-coding/Cargo.toml` | Add serde, toml deps |
| `crates/gf2-coding/src/bin/sim_runner.rs` | New binary — config parsing + curve execution |
| `dev/campaigns/phase1_fig3.toml` | Campaign config |
| `dev/campaigns/phase1_fig1.toml` | Campaign config |
| `dev/campaigns/phase3_fig7.toml` | Campaign config |

Existing examples (`grand_phase1_sims.rs`, `grand_phase3_sim.rs`) are left as-is for now — they can be deprecated once the campaign runner is validated.

## Verification

```bash
# Build
cargo build -p gf2-coding --release --all-features

# Dry run — verify parsing
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase1_fig3.toml --dry-run

# Run single curve
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase1_fig3.toml --curve fig3_ldpc_nms --parallel

# Run all curves in a campaign
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase1_fig3.toml --parallel

# Tests
cargo test --workspace --all-features --release  # < 60s
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```
