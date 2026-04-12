# Epic 6efb756b Session Handover — 2026-04-10

## Status

Waves 1-3 complete (all 10 implementation issues done, gates passed). Wave 4 (simulation campaigns) in progress. Two infrastructure issues (incremental logging a2026f8c, campaign runner a4d86b3d) completed and gate-passed.

## Key Fixes This Session

1. **Per-i_LS shift tables** (f9d6d4c) — Root cause of LDPC BLER gap. BG1/BG2 now use correct 3GPP shift values per lifting set index.
2. **ORBGRAND probability accumulation** (afe270c) — Root cause of turbo decoder failure. P(C\L) now approaches 0 correctly.
3. **Sum-product boxplus clamping** (a93e8a9) — tanh saturation to 1.0 in f32 caused atanh(1)=Inf→NaN. Clamped to 1-ε.
4. **3GPP K_b Z selection** (0da5234) — Z=22 for (256,121) instead of Z=13, matching 3GPP spec and paper.
5. **GLDPC decoder config** (a3063f3) — list_size=4, even_code=true, max_queries=100K for eBCH(32,26).
6. **FILLER_LLR** — Changed from 20→6→15. Value 15.0 avoids f32 tanh saturation while providing strong prior.

## Verified Results

### Fig 3 LDPC Sum-Product (256,121) vs Paper BP — MATCH
| SNR | Ours | Paper | Ratio |
|-----|------|-------|-------|
| 0.0 | 0.833 | 0.881 | 0.95x |
| 1.0 | 0.368 | 0.329 | 1.12x |
| 2.0 | 0.034 | 0.045 | 0.76x |
| 3.0 | 0.00088 | 0.001 | 0.88x |

### Fig 3 Product Code vs LDPC — Product Outperforms (paper's headline result)
Product code BLER < LDPC NMS BLER at all SNR points 0-4 dB (verified in quick mode).

### Fig 1 dRM Product — VERIFIED with corrected (32,21,6) code
dRM product code outperforms LDPC BP at 1.75+ dB.
Authoritative results: `dev/simulation_results/fig1_drm_product.{csv,json}`
Campaign: `dev/campaigns/phase1_fig1.toml`

### Fig 3 eBCH Product — VERIFIED with SOGRAND (queries/bit)
eBCH product code outperforms the paper's LDPC BP from 1.5 dB onward and
our checked-in LDPC SP baseline from 2.0 dB onward.
Authoritative results: `dev/simulation_results/fig3_ebch_product.{csv,json}`
Campaign: `dev/campaigns/phase1_fig3.toml`

Full comparison: `dev/simulation_results/phase1_comparison_report.md`

## Next Session Plan

### Step 1: Quick sanity verification (high BLER region, ~10 min)
Run all campaigns with `--quick` or reduced min_errors (10) to verify:
- Fig 3: product, NMS, SP curves have correct waterfall shape (0-2 dB)
- Fig 1: product, NMS, SP curves at 0-1 dB
- Fig 7: GLDPC, LDPC NMS at 0-2 dB
- All results should align with paper at high BLER (>0.01)

```bash
# Quick verification for each campaign
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase1_fig3.toml --parallel
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase1_fig1.toml --parallel
cargo run -p gf2-coding --release --all-features --bin sim_runner -- dev/campaigns/phase3_fig7.toml --parallel
```

Temporarily reduce min_errors to 10 and max_frames to 1000 in campaign TOMLs for this step.

### Step 2: If Step 1 checks out, run with moderate statistics (~2-6 hours)
Restore min_errors=50-100, max_frames=50000. Let campaigns run.
With max_queries=100K for n=32 decoders, this should be feasible.

### Step 3: Commit results, run gate checks on 92086311 and 3ce1cd12

### Step 4: Wave 5 (Phase 2 + Phase 4) if time permits

## Open Issues

- **3GPP K_b Z selection changes test vectors** — regression vectors regenerated for new Z values
- **Natural column ordering** — decoder uses decode_to_codeword + positions 0..target_k, but encoder still uses RREF. Noiseless roundtrip passes, so alignment is correct for current Z values.
- **GLDPC performance** — With max_queries=100K (down from 1M), SOGRAND quality at deep waterfall may degrade. Need to verify.
- **Fig 1 dRM product very slow** — 0.03 fps at n=1024 even with 100K queries. May need further optimization or accept fewer statistics.

## Campaign Configs

```
dev/campaigns/phase1_fig3.toml  — 3 curves, (256,121)
dev/campaigns/phase1_fig1.toml  — 3 curves, (1024,441), max_queries=100K
dev/campaigns/phase3_fig7.toml  — 2 curves, (1024,646), max_frames=50K
```

## Commits Since Last Handover

See `git log --oneline edecd7e..HEAD` for full list (~25 commits).
