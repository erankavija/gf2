# Integrated LDPC comparator closing record

Closing record for @/issue/5f401907 under `comparator-family` in
[plan.md](plan.md), with AFF3CT's matched flooding f32 normalized min-sum
decoder [Cassagne2019]. Each of the four losing cells closes as a **preserved
negative or inconclusive experiment**. No comparator confirmation runs and no
cell has a confirmed result. All measured values here belong to host
**fraktaali, AMD Ryzen 9 5900X, 12 cores, 24 logical CPUs**; they support no
performance claim beyond this registered comparison.

## Evidence and stopping condition (REQ-01)

The family is `55c3e433-adopted-vs-aff3ct`, declared in
`experiments/aff3ct-ldpc-gaps/families/55c3e433-adopted-vs-aff3ct.json`.
Its baseline is adopted production `gf2-batch-tuned`; its candidate is
`aff3ct-flooding-nms-f32`. Both use the recorded AList graphs and LLRs,
normalization 0.75, cap 50, syndrome stopping and the declared steady-state
boundary. The receipts, acceptance summaries, quality inputs, freeze
derivation and window output are preserved outside the tracked tree.
Independently computed SHA-256 digests identify the receipts:

- Pilot 1, 2026-11-05: `1065c2cc8c486b90a0957f242d51ccfaa407f9b389a925e14419629430f6ccdb`.
- Pilot 2, 2026-11-06: `f47929f8491af5a63c629b4080c40656134a47da8fe0ba93200d9cfe6fdc71da`.
- Pilot 3, 2026-11-08: `1deea3f0a2845709d764c37c8c6cd11d3d4aff9d5b87f18091d970bdd692f03b`.

Each pilot's canonical evaluator label is `pilot`, verdict `accepted`,
`qualifies=false`, with zero findings, six sessions and twelve pairs per cell.
Accepted pilot evidence does not establish confirmation. On fraktaali, pilot
1's freeze refuses equivalence 1.1 because it does not exceed one plus
resolution 0.48; window-start load averages are 4.15, 7.06 and 7.37. Pilot 2's
freeze likewise refuses resolution 0.75; window-start loads are 0.75, 2.80 and
4.99. Its DVB-T2 `p12-b8` interval is [1.332798, 2.257641]. These refusals and
their measurements remain part of the result, without pooling or replacing
their samples.

Pilot 3 supplies the accepted freeze: its widest relative interval half-width
is 0.043622 on fraktaali, giving frozen resolution 0.05, equivalence 1.1 and
material gap 1.25. The preserved confirmation addendum and plan exist. The
confirmation stage refuses to start when its load guard observes one-minute
load 4.87 and five-minute load 2.83, each above limit 1.00. The window starts
at one-minute load 0.19; the pilot's own timed stage raises load before the
confirmation check. The owner declines another armed window. There is no
comparator confirmation receipt. REQ-01's owner-amended no-confirmation branch
therefore applies; its no-production-change branch does not.

## Cell results on fraktaali (REQ-02, REQ-03, REQ-04)

The generated table below is reproduced from pilot 3, the freeze's resolution
evidence, and retains pilots 1 and 2 as separate historical timing rows.
Every timing ratio is **gf2 time divided by AFF3CT time**, using the ratio of
medians and the canonical evaluator's paired-bootstrap interval. Every timing
estimate in the following cell results has twelve pairs and confidence 0.975
(97.5%). A ratio below one favors gf2. The evaluator's `improved` decision
favors its AFF3CT candidate; `regressed` favors gf2. All six cell outcomes are
`pilot`, with unresolved pilot measurement resolution, rather than confirmation
outcomes. The fixed margins are equivalence 1.1 and material gap 1.25.

- **DVB-T2 R1/2 `w1-b1`: preserved negative or inconclusive experiment.**
  Ratio 1.969487, interval [1.966276, 1.974566], evaluator `improved / pilot`.
  gf2 does not meet or exceed AFF3CT in this pilot; AFF3CT retains the advantage.
- **DVB-T2 R1/2 `p12-b8`: preserved negative or inconclusive experiment.**
  Ratio 1.810752, interval [1.789004, 1.837042], evaluator `improved / pilot`.
  gf2 does not meet or exceed AFF3CT in this pilot; AFF3CT retains the advantage.
- **NR BG1 Z=384 `w1-b1`: preserved negative or inconclusive experiment.**
  Ratio 0.191071, interval [0.190037, 0.192245], evaluator `regressed / pilot`.
  gf2 exceeds AFF3CT in this pilot, with no confirmed comparator result.
- **NR BG1 Z=384 `p12-b8`: preserved negative or inconclusive experiment.**
  Ratio 0.183584, interval [0.179041, 0.189497], evaluator `regressed / pilot`.
  gf2 exceeds AFF3CT in this pilot, with no confirmed comparator result.
- **DVB-T2 R1/2 `w1-b8`: lead preserved in the pilot.** Ratio 0.382207,
  interval [0.369997, 0.382810], evaluator `regressed / pilot`; gf2 is faster.
- **NR BG1 Z=384 `w1-b8`: lead preserved in the pilot.** Ratio 0.324140,
  interval [0.310001, 0.327441], evaluator `regressed / pilot`; gf2 is faster.

All six primary cell results cite pilot 3's receipt digest above. The losing
cells each have exactly one closure category; the favorable NR pilot evidence
does not promote either cell to confirmed status. The two one-worker
eight-frame leads are descriptive pilot results as well.

The quality records used by the fraktaali pilots give both arms **2 frame
errors / 128 frames** on DVB-T2, with 95% Wilson FER interval
[0.00429545, 0.05518098], and **1 / 128** on NR, with interval
[0.00138043, 0.04292626]. These are fixed-seed prepared quality samples shared
across worker and batch cases, not additional independent samples for each
timing pair. The generator checks them against every pair's recorded quality
and includes their SHA-256 digests. The preserved prepared inputs are
byte-identical to tracked history `545a3c814`.

## Table reproduction

From the repository root, set `GF2_LDPC_COMPARATOR_EVIDENCE` to the preserved
adopted-vs-aff3ct evidence directory containing run-1, run-2, run-3 and
prepared-3, then run:

```sh
evidence="${GF2_LDPC_COMPARATOR_EVIDENCE:?set the preserved comparator evidence directory}"
python3 experiments/aff3ct-ldpc-gaps/closing-table.py \
  --primary-pilot "$evidence/run-3/family-target/55c3e433-adopted-vs-aff3ct-pilot-3" \
  --historical-pilot "$evidence/run-1/family-target/55c3e433-adopted-vs-aff3ct-pilot" \
  --historical-pilot "$evidence/run-2/family-target/55c3e433-adopted-vs-aff3ct-pilot-2" \
  --quality-dir "$evidence/prepared-3/quality" \
  --output target/aff3ct-ldpc-gaps/closing-table.md
```

This command reads preserved evidence and writes only under target/. No timed
benchmark runs. The table, receipts and execution logs remain untracked.

## Production decision (REQ-05)

No quantized, layered, reassociated or AFF3CT-sign variant enters production.
The [numerical contract](../../../docs/reference/ldpc-decoder-numerical-contract.md)
remains the canonical contract. The
[check-variant nomination](check-variant-nomination.md) carries no check variant
into adoption. The exploratory QC go decision in
[qc-route-decision.md](qc-route-decision.md) is closed by the qualifying
confirmation of @/issue/3ee00fcf, as recorded in
[adoption-decision.md](adoption-decision.md): verdict `accepted`,
`qualifies=true`, receipt SHA-256
`87cfeaa2d11de5b8f03d5f5f6a8f83c0975b3dd2e256d25c692854c21c06a18a`.
That confirmation compares QC with pre-change production. It establishes
adoption and does not establish a confirmed AFF3CT comparison. The present
comparator closure preserves the DVB-T2 losses, the favorable NR pilot
measurements, both eight-frame pilot leads, both refused freezes and the
confirmation-guard refusal together.
