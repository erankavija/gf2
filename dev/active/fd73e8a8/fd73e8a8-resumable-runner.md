# fd73e8a8 — Resumable observable SimulationRunner

Design doc for the SimulationRunner observability and resume work that
underpins the DVB-T2 AWGN campaign (epic 2928ccce).

## Goal

Add observability (structured tracing) and crash-safe resume to
`crates/gf2-coding/src/simulation.rs` so multi-hour campaign runs can
be launched unattended, tailed live with `jq`, and recover from
SIGINT / SIGTERM / kill-9 without recomputing finished work.

Public surface today is additive-only: existing `run_uncoded_ber`,
`run_coded`, `run_coded_iterative`, `run_coded_iterative_parallel`
keep their signatures; new behaviour is opt-in via new
`SimulationConfig` fields.

## Decisions captured from breakdown interview

| Topic | Choice |
|-------|--------|
| Checkpoint granularity | Per-SNR-point JSON file + within-SNR heartbeat |
| RNG | `rand_chacha::ChaCha20Rng` with `set_word_pos` resume seek |
| Signal handling | `ctrlc` (lighter than `signal-hook`, MSRV 1.95 stable) |
| Tracing target | JSON-lines via `tracing-subscriber` |

## Checkpoint format

Layout under `<checkpoint_dir>/`:

```
config_hash.txt         # blake3 of canonical-encoded SimulationConfig
snr_0000.json           # per-SNR-point state (one per index)
snr_0001.json
...
```

`snr_<i>.json` schema:

```json
{
  "snr_index": 7,
  "es_n0_db": 8.5,
  "frames_completed": 320000,
  "errors_accumulated": 47,
  "iter_stats": {
    "total_iters": 894231,
    "max_iters": 50,
    "early_term_count": 318943
  },
  "rng_word_pos": 18432000,
  "frames_target": 1000000,
  "errors_target": 100,
  "completed": false,
  "config_hash": "blake3:..."
}
```

`completed: true` means the point hit `frames_target` or
`errors_target`; resume skips it. `completed: false` means a
heartbeat checkpoint mid-point; resume continues from
`frames_completed` using the recorded `rng_word_pos`.

## RNG seek (deterministic resume)

Per-SNR-point RNG construction:

```rust
let seed = config.seed ^ (snr_index as u64).rotate_left(13);
let mut rng = ChaCha20Rng::seed_from_u64(seed);
rng.set_word_pos(checkpoint.rng_word_pos);  // 0 for fresh point
```

`rng_word_pos` is captured at every heartbeat as
`rng.get_word_pos()`. Word position is bit-exact reproducible across
`ChaCha20Rng` versions (documented in `rand_chacha`).

Integration test: spawn a campaign, send SIGINT mid-SNR-point,
restart, verify final CSV is byte-identical to a same-seed
uninterrupted reference run.

## Signal handling

```rust
let interrupted = Arc::new(AtomicBool::new(false));
let flag = interrupted.clone();
ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))?;
```

Inner simulation loop polls `interrupted` between frame batches; on
trip, flushes the current SNR checkpoint and exits with non-zero
status. `ctrlc` handles both SIGINT and SIGTERM on Unix.

## Tracing

Top-level span: campaign with `config_hash`, `run_uuid`, `seed`.

Per-SNR span: `es_n0_db`, `frames_target`, `errors_target`.

Events:

- `snr_completed` — terminal event per SNR, includes FER, BER,
  mean_iters, elapsed_seconds.
- `heartbeat` — opt-in via `heartbeat_every_frames`; emits
  frames_completed, errors_so_far, elapsed_seconds.

Subscriber: `tracing-subscriber` with JSON formatter when
`tracing_log_path` is `Some`. One JSON-lines record per event.

## SimulationConfig additions

```rust
pub struct SimulationConfig {
    // ... existing fields ...
    pub checkpoint_dir: Option<PathBuf>,
    pub tracing_log_path: Option<PathBuf>,
    pub heartbeat_every_frames: Option<usize>,
}
```

`heartbeat_every_frames = None` -> per-SNR coarse checkpointing only
(no within-SNR resume granularity). The hard criterion for
byte-identical resume in the issue only requires resume from the
*next unfinished SNR point*; within-SNR resume is an opt-in
enhancement and is required by the kill-mid-SNR integration test.

## Config-hash mismatch

On startup with `checkpoint_dir = Some(p)`:

1. Read `config_hash.txt` if present.
2. Recompute hash from current `SimulationConfig`.
3. If mismatch: abort with a clear error naming the differing field
   (best-effort diff). No partial recovery.
4. If matched: enumerate `snr_<i>.json` files, validate per-file
   hash, skip completed points, resume the first incomplete point.

## Out of scope

- Multi-host / distributed campaigns (single-machine only).
- Mid-frame resume (frame is the atomic unit).
- Live progress bar / TUI (per epic non-goal).
- Pluggable serialization (JSON only).

## Open questions for implementer

- Should `tracing_log_path` rotate on size? -> No, single JSONL file
  per campaign; rotation is the operator's concern.
- Atomic checkpoint write strategy: `write to tmp + rename` or
  `O_DSYNC`? -> `tmp + rename` is sufficient on POSIX.
- Should `set_word_pos` be exposed on `rand_chacha 0.9`? -> verify
  before claiming the issue (memory: `Do not guess APIs`).
