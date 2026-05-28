# Handoff — DVB-T2 AWGN simulation campaign (2928ccce) — session 1 (final)

**Date:** 2026-05-28
**Session number:** 1
**Prior handoffs:** none (this supersedes the in-flight versions)

## Current state

- Epic: `2928ccce` — state: `in_progress` (cannot close — see below)
- All implementation/validation waves COMPLETE. Only the user-side
  production campaign remains.
- Children summary:
  - **Done (8)**: `cef2e631`, `a18956a4`, `87a2402f`, `fd73e8a8`,
    `003e4088`, `4cdaf1c5`, `152388f4`, plus created-in-flight
    `548a8563` (parity-interleaver fix).
  - **Rejected (1)**: `a7b1bb21` (§6.1.4 demux — obsolete; see below).
  - **Ready for user (1)**: `e4849f07` (production FER runs).
- Progress file: `dev/active/2928ccce-progress.json` (current).

## Why the epic is not closed

`e4849f07` (the terminal child) requires six production FER curves
(≥10⁶ frames each, multi-day wall-clock) plus PNG overlays and a
≤0.5 dB gap to the ETSI reference. Per the agreed Wave-5 scope (user
chose "lead delivers calibration smoke, user runs prod" at session
start), the lead delivered the calibration smoke + PLAN.md + README.md;
the production runs and final closure are user-side. The epic closes
once `e4849f07`'s artefacts land.

## What was delivered this session

- **Infrastructure + codec (Wave 1)**: 11 LDPC table source files +
  roundtrip tests (cef2e631); BCH+LDPC concat codec with `&self` API
  via OnceCell/Mutex (a18956a4); DVB-T2 §6.1.3 bit interleaver
  (87a2402f, then completed by 548a8563); resumable observable
  `SimulationRunner` with tracing + per-SNR checkpoints + subprocess
  SIGINT resume (fd73e8a8).
- **Integration (Wave 2)**: documented BICM chain composition +
  example + integration test (003e4088).
- **Validation (Wave 3)**: TP06→TP07a bit-exact against ETSI vectors
  for VV020/VV009/VV014 (4cdaf1c5 + 548a8563).
- **Runner (Wave 4)**: `dvb_t2_awgn_campaign` binary (SSOT-clean,
  delegates to SimulationRunner), `plot.py`, ETSI TS 102 831 Table 44
  reference TOML, resume integration test (152388f4).
- **Calibration (Wave 5, partial)**: 6-config calibration smoke +
  PLAN.md + README.md under `dev/benchmarks/dvb_t2_awgn/`.

## What to do next (user / next-session lead)

1. [ ] **Tune the decoder/demapper to close the ~1-1.5 dB gap** before
   production (see README.md "Key finding"). The chain decodes
   correctly but the default plain-`MinSum` LDPC + `MaxLog` demap
   waterfalls ~1-1.5 dB worse than the ETSI QEF threshold. Switch to
   `NormalizedMinSum(~0.75)` or `SumProduct` decoding + `ExactLogMap`
   demapping (both allowed by the epic's "tuning parameters" clause).
   Re-run the r1/2 16-QAM calibration to confirm the knee moves toward
   ~6.0-6.5 dB.
2. [ ] Re-centre the per-config `--esn0-range` brackets in PLAN.md on
   the tuned waterfall knees.
3. [ ] Run the six production sweeps per PLAN.md (≥100 frame errors at
   the FER=10⁻⁴ bracket). Each is resumable via `--resume`.
4. [ ] Plot each with `plot.py`; record the achieved gap per curve.
5. [ ] If any curve's gap > 0.5 dB after tuning, escalate per the
   amendment/escalation policy before closing (the 0.5 dB is the
   epic's [hard] target; "measurements not guesses" — a data-backed
   amendment may be warranted).
6. [ ] Commit the six CSV/PNG/closure-note artefacts under
   `dev/benchmarks/dvb_t2_awgn/`, then close `e4849f07` and `2928ccce`.

## Traps — do not repeat these

- **Shell cwd persists across Bash calls.** Never `cd` into a worktree
  then `git merge` in a later call — it lands on the wrong branch. Use
  `git -C /home/vkaskivuo/Projects/gf2 ...`. (Cost a recovery this
  session.)
- **`check-leak-into-main.sh` honours cwd** — run it from the main repo
  root, not a worktree.
- **Do not trust worker self-reports verbatim.** Bugs that passed
  worker self-tests this session: Rate3_5-vs-Rate3_4 scope typo;
  row-major-vs-column-major permutation (self-roundtrip masked it);
  QAM-bypass test helper; and the BIG one — the bit interleaver
  shipped (87a2402f, all gates green) MISSING the entire §6.1.3
  parity-interleaving sub-stage. Self-consistent roundtrip tests do
  NOT catch spec-divergence. Always add a test against EXTERNAL
  reference data (ETSI vectors) when correctness is spec-defined.
- **`jit gate pass <id> code-review` via the MCP times out at 600 s**;
  the AI reviewer often runs longer. Run it via the CLI in a
  background Bash task instead: `jit gate pass <id> code-review --by
  agent:project-lead` with `run_in_background: true`. (Per user
  instruction this session.) Do NOT invoke `./scripts/ai-review.sh`
  directly.
- **`pgrep -f <pat>` self-matches** — never use it in a wait loop.
- **ETSI vectors moved** to `/data/specs/dvb/t2/streams/` (old
  `/data/specs/dvb/streams/` is gone). Docs at
  `/data/specs/dvb/t2/documentation/`. TS 102 831 (impl guidelines,
  AWGN C/N tables) at
  `/data/specs/etsi/deliver/etsi_ts/102800_102899/102831/01.02.01_60/`.
- **TP07a = §6.1.3 output** (parity interleave + column twist), NOT
  §6.1.4. TP07 (no `a`) = §6.1.4 cell-word-demux output (not
  implemented; not needed for this epic). VV001-CR35 is QPSK Rate3/5
  (earlier workers mislabeled it 256-QAM). Verify modulation from
  TP08 cells/block: 32400=QPSK, 16200=16-QAM, 10800=64-QAM,
  8100=256-QAM (Normal 64800-bit frame).
- **wall_seconds + ber are non-deterministic across processes**
  (wall-clock; AVX2 f32 non-associativity in LDPC min-sum). The resume
  byte-identity criterion was amended (user-approved) to deterministic
  columns only (es_n0_db, fer, frames, errors, mean_iters).
- **`run_coded_iterative_parallel` uses StdRng** (no within-SNR
  heartbeat resume); per-SNR-boundary resume only.
- **Reviewer is contract enforcer** — fix structurally or escalate;
  don't argue. Several criteria were amended this session WITH user
  approval (87a2402f TP07a, fd73e8a8 heartbeat scoping, 003e4088 TP07a
  removal, 152388f4 resume byte-identity). Never amend without
  approval.

## Open questions needing user input

- **Decoder/demapper tuning vs gap criterion** (the one live decision):
  the calibration shows ~1-1.5 dB gap with the default MinSum+MaxLog.
  Tune before production (recommended) or accept + amend the 0.5 dB
  criterion with data. The lead will surface this in the session
  report.

## Reference artefacts

- Epic: `jit issue show 2928ccce`
- Campaign artefacts: `dev/benchmarks/dvb_t2_awgn/{PLAN.md,README.md,smoke/,plot.py}`
- Reference TOML: `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml`
- Binary: `crates/gf2-coding/src/bin/dvb_t2_awgn_campaign.rs`
- ETSI vectors: `/data/specs/dvb/t2/streams/`; TS 102 831:
  `/data/specs/etsi/deliver/etsi_ts/102800_102899/102831/01.02.01_60/ts_102831v010201p.txt`
- Progress file: `dev/active/2928ccce-progress.json`
