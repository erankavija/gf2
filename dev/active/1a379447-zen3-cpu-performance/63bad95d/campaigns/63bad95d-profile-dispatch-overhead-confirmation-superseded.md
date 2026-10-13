# Superseded: the `63bad95d-profile-dispatch-overhead` confirmation frozen from pilot trial 1

[`63bad95d-profile-dispatch-overhead-confirmation.json`](63bad95d-profile-dispatch-overhead-confirmation.json)
is superseded and no campaign measures it. The addendum, its
[derivation record](63bad95d-profile-dispatch-overhead-confirmation-derivation.txt),
the [resolution record](63bad95d-profile-dispatch-overhead-pilot-resolution.txt)
and the
[arm smoke record](../survey/profile-dispatch-overhead-confirmation-smoke.json)
keep their bytes. The
[family ledger](../../../../bench_results/63bad95d/63bad95d-profile-dispatch-overhead-ledger.jsonl)
holds no reservation for it, so the family's one confirmatory attempt is
unspent. `run-profile-families.sh` refuses to smoke or measure an addendum
that has this record beside it, and refuses to freeze over a committed
addendum.

## Reason

The addendum's resolution comes from the whole first pilot,
[`r1-pilot`](../../../../bench_results/63bad95d/63bad95d-profile-dispatch-overhead/r1-pilot/receipt.json),
and one cell sets it: the widest row of the resolution record is
`and-popcount-7w`. In that cell's raw pairs (`cells[].pairs` of the receipt,
indices 8 to 11) the per-call time of one or both arms leaves the level that
indices 0 to 7 hold for both arms, and the calibrated call counts of those
executions fall with it. The cells measured before and after it hold their
level throughout. The pilot's
[launcher log](../../../../bench_results/63bad95d/63bad95d-profile-dispatch-overhead/r1-pilot/launcher.log)
records the host load at start and end (`load_avg_start`, `load_avg_end`).
A disturbance that reaches both arms of one cell for part of its pairs is
read here as host contention; the receipt does not identify its cause.

The margins the derivation record states follow from that one row. With
them the frozen decision rule retains the baked threshold unless a cell
regresses by more than the stated margin, which is wider than the effect the
pilot addendum declares worthwhile.

## Replacement

The frozen pilot addendum
[`63bad95d-profile-dispatch-overhead-pilot.json`](63bad95d-profile-dispatch-overhead-pilot.json)
declares its pilot trials per cell in `search_budget.max_pilot_trials_per_cell`,
and the ledger holds one pilot reservation. The second trial is stage `pilot2`
of the launcher: campaign `63bad95d-r2-profile-dispatch-overhead-pilot`, the
same addendum bytes, its own seed, receipt directory `r2-pilot` beside
`r1-pilot`. `freeze 63bad95d-profile-dispatch-overhead pilot2` derives
`63bad95d-profile-dispatch-overhead-r2-confirmation.json` and
`63bad95d-profile-dispatch-overhead-r2-confirmation-derivation.txt`, and
stage `confirmation2` measures that addendum. The freezer refuses the second
trial's receipt
([refusal record](63bad95d-profile-dispatch-overhead-r2-freeze-refusal.txt)),
so neither file exists. Cells, arms, sizes, the holdout
declaration, the margin rule and the decision rule are those of the pilot
addendum and the plan record.

The first pilot's receipt stays committed as the family's first exploratory
trial (ledger sequence 0).
