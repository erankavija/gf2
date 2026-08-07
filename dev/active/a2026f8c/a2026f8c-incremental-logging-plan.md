# Plan: Incremental Simulation Logging (a2026f8c)

## Context

Simulation campaigns run for hours (3.5+ hours observed for moderate-mode Phase 1/3). Currently results are written only after ALL SNR points complete, progress is ephemeral stderr, and a crash loses everything. This blocks practical iteration on the GRAND epic's Wave 4-5 simulations.

## Changes

### 1. Incremental CSV append (`simulation.rs`)

After each SNR point completes in `run_coded_iterative` (line ~941) and `run_coded_iterative_parallel` (line ~1073):
- If `config.output_path` is set, append the completed `SimulationResult` as a CSV row
- Write header only if file doesn't exist or is empty
- Use `OpenOptions::append(true).create(true)` for atomic append
- The final `write_to` call at the end overwrites with the complete file (clean, with header)

**Files:** `crates/gf2-coding/src/simulation.rs`
- Add `SimulationResult::write_csv_row_to(&self, path: &Path)` — appends one row
- Add `SimulationResults::write_csv_header_to(path: &Path)` — writes header if file empty/missing
- Modify `run_coded_iterative` inner loop: after `acc.into_result()`, call incremental write
- Same for `run_coded_iterative_parallel`

### 2. JSONL progress log (`simulation.rs`)

Alongside the CSV, write a `.progress.jsonl` file (derived from `output_path` by changing extension):
- Wall-clock based, adaptive: first entry after 10s, then every 60s
- Each entry: `{"timestamp": "ISO8601", "eb_n0_db": X, "frames": N, "frame_errors": M, "bler_estimate": Y, "elapsed_s": T}`
- Per-point completion entry: `{"type": "point_complete", "eb_n0_db": X, ...full result...}`
- Track wall time via `std::time::Instant` in `SnrAccumulator`

**Changes to `SnrAccumulator`:**
- Add `start_time: Instant` field
- Add `last_progress_time: Instant` field  
- Add method `should_write_progress() -> bool` — true if ≥60s since last (or ≥10s for first)
- Add method `write_progress_entry(path: &Path)` — appends JSONL line

**Changes to `report_progress()`:**
- Add wall-clock elapsed and ETA to stderr output
- ETA: after 2+ points complete, extrapolate from per-point durations weighted by frame count

### 3. Resumable runs (`simulation.rs`)

Before the SNR loop in `run_coded_iterative` / `run_coded_iterative_parallel`:
- If `config.output_path` exists and is non-empty, parse existing CSV rows
- For each SNR point in `config.eb_n0_range_db`, check if the CSV already has a row with matching `eb_n0_db` and `num_frame_errors >= config.min_errors`
- If so, use the existing result and skip simulation for that point
- Print `[X.X dB] RESUMED: using existing result (N errors, M frames)` to stderr

**New function:** `try_load_existing_results(path: &Path, min_errors: usize) -> HashMap<OrderedFloat<f64>, SimulationResult>`

### 4. Closure-based simulation runner (`simulation.rs`)

New method that accepts a decode closure instead of requiring `IterativeSoftDecoder`:

```rust
pub fn run_with_decoder<E, C, F>(
    encoder: &E,
    mut decode_fn: F,
    channel: &C,
    config: &SimulationConfig,
) -> SimulationResults
where
    E: BlockEncoder,
    C: ChannelModel,
    F: FnMut(&[Llr]) -> DecoderResult,
```

This replaces the custom `run_product_sim` loop in `grand_phase1_sims.rs`. The `TurboDecoder::decode()` wraps into a closure:

```rust
let mut turbo = TurboDecoder::new(component, turbo_config);
SimulationRunner::run_with_decoder(
    &product,
    |llrs| turbo.decode(llrs).into(),  // TurboDecoderResult -> DecoderResult
    &channel,
    &config,
)
```

**Requires:** `impl From<TurboDecoderResult> for DecoderResult` in `product/mod.rs`.

### 5. Update example runners

**`grand_phase1_sims.rs`:**
- Remove `run_product_sim` function entirely
- Use `SimulationRunner::run_with_decoder` for product code sims
- Remove local `count_bit_errors` (already done)
- Set `output_path` on all configs for incremental writes

**`grand_phase3_sim.rs`:**
- Set `output_path` on configs (already done for CSV, ensure JSONL path derived)

### 6. Stderr progress format

Per-point completion:
```
[3.0 dB] DONE: BLER=2.64e-3 (100 errors / 37892 frames) in 4m23s — ETA 12m for 2 remaining points
```

Intra-point (adaptive wall-clock):
```
[3.5 dB] progress: 8432/~100000 frames, 12 errors, ~47m remaining for this point
```

## Files to modify

| File | Changes |
|------|---------|
| `crates/gf2-coding/src/simulation.rs` | Incremental CSV, JSONL progress, resume, closure runner, ETA |
| `crates/gf2-coding/src/product/mod.rs` | `impl From<TurboDecoderResult> for DecoderResult` |
| `crates/gf2-coding/examples/grand_phase1_sims.rs` | Use `run_with_decoder`, set output_path |
| `crates/gf2-coding/examples/grand_phase3_sim.rs` | Ensure output_path set consistently |

## Testing

- `cargo test --workspace --all-features` — all existing tests pass (no behavioral change for completed sims)
- New unit test: `test_incremental_csv_append` — run small sim, verify CSV grows per point
- New unit test: `test_resume_skips_completed_points` — write partial CSV, run sim, verify skip
- New unit test: `test_jsonl_progress_written` — verify JSONL file created during sim
- New unit test: `test_run_with_decoder_matches_run_coded_iterative` — same config, same seed, same results
- Manual: run `--quick` mode, verify incremental CSV/JSONL written, kill mid-run, restart with same output_path, verify resume

## Verification

```bash
# Build and test
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check

# Manual verification: run quick sim, watch incremental output
cargo run -p gf2-coding --example grand_phase1_sims --release 2>&1 &
watch -n5 'wc -l dev/simulation_results/fig3_*.csv; cat dev/simulation_results/fig3_ebch_product_256_121.progress.jsonl | tail -3'

# Kill and resume test
kill %1
cargo run -p gf2-coding --example grand_phase1_sims --release 2>&1
# Should print "RESUMED" for completed points
```
