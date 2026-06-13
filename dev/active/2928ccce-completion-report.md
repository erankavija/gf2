# Epic completion report — DVB-T2 AWGN simulation campaign (2928ccce)

**Closed:** 2026-06-13. **Lead:** agent:project-lead (session 2).

## Outcome

The terminal child `e4849f07` (production FER campaign) — the only
remaining open work from session 1 — was executed end to end on the GPU
and closed. All six (rate × modulation) curves are produced, plotted,
and committed; the epic's success criteria are met (with one
user-approved, spec-grounded amendment to the gap criterion).

## Metrics

- Children: 9 done (8 from session 1 + `548a8563`), 1 rejected
  (`a7b1bb21`, obsolete), terminal child `e4849f07` closed this session.
- Session-2 waves: 1 (execution wave — the production campaign).
- Escalations: 1 (gap criterion, see below). Rework cycles: 0.
- GPU campaign wall-clock: main sweep ~5.5 h + sub-10⁻⁴ extensions
  ~10.9 h (host load avg ~28 throttled the extensions).

## Success-criteria mapping (epic 2928ccce)

| Criterion | Delivered by | Status |
|-----------|--------------|--------|
| 6 curves within gap of ETSI TS 102 831 @FER=10⁻⁴ | `e4849f07` campaign | MET (amended: ≤0.5 dB 16-QAM, ≤0.65 dB 64-QAM) |
| TP04→TP07a bit-exact | `4cdaf1c5` (+`548a8563`) | MET (session 1) |
| ≥100 errors bracketing 10⁻⁴ / ≥10⁶ frames deepest | `e4849f07` | MET (1.2M frames at each deepest point) |
| Artefacts committed under dev/benchmarks/dvb_t2_awgn/ | `e4849f07` | MET (CSV+PNG+tracing+README+PLAN+CLOSURE) |
| SIGINT/kill resume | `fd73e8a8`, `152388f4` | MET (session 1) |
| Structured tracing to JSON, jq-tailable | `fd73e8a8` | MET (tracing_*.jsonl) |
| Single CLI invocation per curve | `152388f4`/`bbf6b6ee` | MET |

## Measured gaps (FER = 10⁻⁴)

| MODCOD | ETSI | sim crossing | gap | result |
|--------|:----:|:------------:|:---:|:------:|
| 1/2 16-QAM | 6.0 | 6.167 | 0.167 | ≤0.5 ✓ (≤0.3 aspirational ✓) |
| 2/3 16-QAM | 8.9 | 9.017 | 0.117 | ≤0.5 ✓ (≤0.3 aspirational ✓) |
| 3/4 16-QAM | 10.0 | 10.224 | 0.224 | ≤0.5 ✓ (≤0.3 aspirational ✓) |
| 1/2 64-QAM | 9.9 | 10.506 | 0.606 | ≤0.65 ✓ |
| 2/3 64-QAM | 13.5 | 14.085 | 0.585 | ≤0.65 ✓ |
| 3/4 64-QAM | 15.1 | 15.614 | 0.514 | ≤0.65 ✓ |

## Key autonomous decisions

- Used the migrated `gf2-sim` hybrid CPU+GPU pipeline (`--features hip`,
  `--gpu`) with SumProduct + ExactLogMap, per the calibration
  recommendation and the now-done `gf2-sim` epic.
- Anchored sweep brackets on the regression FER-at-anchor + measured
  cliff slope; retuned to 0.05 dB steps and trimmed the deep tail after
  observing the waterfall is a sharp cliff (~1 decade / 0.045 dB),
  avoiding wasted max-frames runs.
- Refreshed stale PLAN.md/README.md (legacy `SimulationRunner` / CPU
  framing → gf2-sim GPU pipeline), fixed `plot.py` "TR"→"TS" label.

## Escalation log

1. **Gap criterion vs genie-aided reference.** The 64-QAM curves missed
   the [hard] ≤0.5 dB target (0.51–0.61 dB). Root cause: ETSI TS 102 831
   Table 44 assumes "Genie-Aided" demapping, which the spec (§14.2)
   states is "optimistic ... for high-order constellations." In-scope
   single-pass ExactLogMap BICM is below the genie bound. User approved
   "Amend with spec rationale" — kept ≤0.5 dB for 16-QAM, set ≤0.65 dB
   for 64-QAM, recorded observed numbers + rationale on both issues.
   Documented in `dev/benchmarks/dvb_t2_awgn/CLOSURE.md`.

## Holistic quality notes

- All six curves bracket FER=10⁻⁴ with measured data (≥100 errors at the
  above-cliff point; ≥1.2×10⁶ frames at the deepest plotted point).
- 16-QAM curves also meet the aspirational ≤0.3 dB target.
- The GPU path is byte-identical to CPU on the contractual columns
  (frames/errors/fer) per the `gf2-sim` regression receipts.
