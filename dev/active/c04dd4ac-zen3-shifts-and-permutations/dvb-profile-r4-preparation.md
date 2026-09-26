# DVB-T2 profile interval preparation

> **Diátaxis Type:** Research note

The accepted protocol-v4 paired receipt at
[`v4-r2-pilot`](../../bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-pilot/receipt.json)
continues to decide materiality. The fresh dynamic profile writes to
`dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r3-dynamic-profile/`.
Its nine repetitions each collect the declared hardware counters, hot symbols,
and instruction samples. The attribution renderer uses the fresh perf outputs
beside the accepted paired and across-session cost evidence. The scheduled run
is pending; this note makes no interval or closure claim for it.

The exact queue line is in
[`bench-window/queue.tsv`](../1a379447-zen3-cpu-performance/bench-window/queue.tsv).
The launcher prints and appends the authoritative profile log, checks its
runtime producing-input snapshot on resume, and preserves an interrupted
repetition before retrying it. The snapshot covers declared producing behavior,
the actual executable and its build record. The existing provenance renderer
checks that snapshot against the measured tree and records the top-level and
inner invocation.

After the scheduled run, generate the provenance record and attribution tables
from the committed fresh profile. Review all nine perf statuses and the
interval tables before replacing the report's single-session characterization.
A nonzero perf status stays in the profile as an unavailable outcome.

## Cumulative review resolution

Before editing, the cumulative audit read the supplied failed gate records,
ran `jit doc list 9fb40c83`, scanned every linked path for deferred-item
markers, and searched the issue's active and benchmark roots for each finding's
smoke, provenance, single-session and `rep-01` terms. The linked-document
deferred-marker scan found no matches. The remaining single-session statements
refer to the preserved v4-r2 profile; the fresh profile and tables remain
pending the scheduled run.

| Round | Finding | Resolution at this tree |
|---|---|---|
| R1 code F1 | Smoke invoked timed pilot executions outside the window and lock. | `survey/smoke-dvb-arms.sh` invokes shared `benchmark-ab-runner smoke`; its arm path is untimed. |
| R1 code F2 | Narrative comments preserved task history. | `survey/smoke-dvb-arms.sh` carries one short contract and usage note. |
| R1 research TIER1-F1 | ETSI citation label had no inline key. | `dvb-interleave-profile.md` cites `[Etsi2015]`. |
| R1 research TIER1-F2 | xdsopl citation label had no inline key. | `dvb-interleave-profile.md` cites `[Xdsopl2026]`. |
| R2 code F1 | Private smoke duplicated runner dispatch and wire contract. | `survey/smoke-dvb-arms.sh` calls the shared runner's smoke command; the private driver is absent. |
| R2 code F2 | Long smoke comments repeated across files. | The shared launcher and renderer use concise contracts; the per-finding scan found no private smoke driver. |
| R2 research F1 | Dynamic profile lacked invocation and source/build/RNG closure. | `v4-r2-dynamic-profile-provenance.md` pins the old profile; the fresh profile's runtime snapshot and `survey/make-profile-provenance.py` bind its own closure and argv. |
| R2 research F2 | BICM scatter shares lacked intervals. | `dvb-interleave-profile-tables.md`, “Scatter share of the BICM channel across sessions,” gives nine-session intervals. |
| R2 research F3 | Counter, symbol and instruction tables came from one session. | `survey/run-profile.sh` collects all three in every fresh repetition; `survey/make-dvb-tables.py` computes nine-session intervals. **Pending scheduled evidence.** |
| R3 research F3 | Retained-session counters and top symbols still lacked intervals. | Fresh profile and renderer prepared as above. **Pending scheduled evidence and report update.** |
