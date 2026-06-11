# 152388f4 — DVB-T2 BICM AWGN campaign runner

Design doc for the campaign binary that drives the DVB-T2 BICM AWGN
sweeps for epic 2928ccce.

## Goal

A CLI binary at `crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs`
*[since moved: the binary now lives at
`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` on the `gf2-sim`
pipeline, and the legacy `gf2-coding` binary file was deleted per the
user-approved `bbf6b6ee` amendment 2026-06-11b]*
that runs the full AWGN simulation for one `(rate, modulation)`
configuration and produces a CSV curve, a JSON tracing log, and a
calibration smoke sweep. Six invocations (3 rates × 2 modulations)
reproduce every curve required by the epic.

Plotting is emitted-data-only: the binary writes CSV; a separate
script in `dev/benchmarks/dvb_t2_awgn/` produces the PNG overlay vs
the ETSI TR 102 831 reference.

## Decisions captured from breakdown interview

| Topic | Choice |
|-------|--------|
| Plotting | Emit data only; external script generates PNG overlay |
| GPU | Optional `--gpu` flag dispatching to `gf2-coding/--features hip` |
| Calibration | `--calibrate` with configurable `--calibrate-frames` and `--calibrate-bracket` |
| Reference TOML | Per-MODCOD table with `(es_n0_db, fer)` point list |

## CLI surface

```
dvb_t2_awgn_campaign \
  --rate {1/2|2/3|3/4} \
  --modulation {16qam|64qam} \
  --esn0-range <start>:<stop>:<step>  \
  --target-errors <N>              [default: 100] \
  --max-frames <N>                 [default: 10_000_000] \
  --seed <u64>                     [default: 0xC0DEF00D] \
  --output-dir <path>              [required] \
  --resume                         [default: false] \
  --gpu                            [default: false; requires hip feature] \
  --calibrate                      [default: false] \
  --calibrate-frames <N>           [default: 1000; used with --calibrate] \
  --calibrate-bracket <a:b:c>      [default: predicted bracket per MODCOD]
```

`--esn0-range` is mutually exclusive with `--calibrate`; calibration
uses its own bracket derived from the ETSI TR 102 831 reference for
the selected `(rate, modulation)`.

## Output layout

Production run (`<output-dir>/`):

```
curve_<rate>_<mod>.csv     # columns: es_n0_db, fer, ber, frames,
                           #          errors, mean_iters, wall_seconds
tracing.jsonl              # structured tracing log (one record/event)
README.md                  # invocation, seed, host info, wall-clock
checkpoints/               # per-SNR JSON files (fd73e8a8 format)
  config_hash.txt
  snr_0000.json
  ...
```

Calibration run (`--calibrate`):

```
calibration_<rate>_<mod>.csv   # 3-point sweep at --calibrate-frames
calibration.jsonl              # tracing log
```

## Plotting

Out-of-binary; the runner emits CSV only.

Script `dev/benchmarks/dvb_t2_awgn/plot.py` (or `.sh` invoking
gnuplot) consumes `curve_<rate>_<mod>.csv` plus the reference TOML
and produces `curve_<rate>_<mod>.png`. The script is checked in
alongside the binary; CI does not invoke it.

Rationale: avoids pulling a Rust plotting backend (`plotters`,
`charming`) into the workspace dep tree. Operators rerun the script
after each campaign or to re-style the overlay.

## ETSI TR 102 831 reference TOML

`crates/gf2-coding/data/dvb_t2_tr102831_reference.toml`:

```toml
# DVB-T2 AWGN reference points from ETSI TR 102 831 v1.2.1 (2012-10)
# Hand-extracted from the implementation-guidelines AWGN curves.

[normal_r1_2_16qam]
source = "TR 102 831 Fig 10, p.55"
# (es_n0_db, fer) point pairs along the reference curve
points = [
  [4.5, 1.0e-1],
  [5.0, 1.0e-2],
  [5.5, 1.0e-3],
  [6.0, 1.0e-4],
]

[normal_r1_2_64qam]
source = "TR 102 831 Fig 12, p.57"
points = [ ... ]

# ... (one section per in-scope MODCOD)
```

Section naming: `normal_r<num>_<den>_<modulation>`. Plot script
looks up the section matching the curve being overlaid.

## --gpu dispatch

When `--gpu` is set:

1. Verify the binary was compiled with `--features hip`. If not,
   error with a clear message naming the feature.
2. Construct the chain using a GPU-aware BP decoder path (provided
   by the HIP prototype epic 806eb14e once landed). For each frame
   batch, dispatch BP to GPU; AWGN noise + QAM demap remain on CPU
   (the demap GPU prototype exists but is not yet integrated as the
   default path).
3. On any GPU runtime failure (device unavailable, OOM), fall back
   to CPU with a `tracing::warn!` event recording the fallback. The
   campaign does not abort.

Until 806eb14e lands, `--gpu` may panic with `unimplemented!` or
emit a clear "GPU path not yet integrated" error. The CLI surface
is reserved now so the runner doesn't need a breaking change later.

## --calibrate semantics

Sweeps `--calibrate-frames` frames per SNR over a 3-point bracket
(low / center / high) around the reference FER=10⁻⁴ point for the
selected `(rate, modulation)`. Bracket defaults are derived from
the reference TOML; user can override with `--calibrate-bracket
a:b:c` (three explicit Es/N0 values).

Pass criterion (informational, not enforced by the binary): center
FER lies within `[1e-3, 1e-5]`. Operator decides whether to launch
production. Output stays under
`dev/benchmarks/dvb_t2_awgn/calibration/`.

## Out of scope

- Interactive UI / progress bar (per epic non-goal).
- Rust-side plotting backend.
- New AWGN channel model (16-QAM/64-QAM extends existing
  `BpskAwgnChannel` per existing modem framework).
- Production GPU integration (waits on 806eb14e LDPC BP prototype).
- Code refactoring beyond plugging 16-QAM / 64-QAM into the existing
  mapper / demapper.

## Open questions for implementer

- Does the existing `BpskAwgnChannel` need a `QamAwgnChannel`
  sibling, or does the modem framework already abstract symbol
  generation? -> verify before claiming.
- Is `dev/benchmarks/dvb_t2_awgn/plot.py` expected to be invoked by
  CI or only by operators? -> operators only (per epic non-goal of
  "no interactive UI" and CI-time budget).
- Should `--gpu` and `--calibrate` compose? -> yes, with the same
  CPU-fallback semantics.
