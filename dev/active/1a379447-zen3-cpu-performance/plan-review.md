# Zen 3 epic plan review resolution

## Scope and evidence

On 2026-09-06 Claude reviewed epic `1a379447` at `bd186992` through the local
`forum-poc` exchange. Codex challenged the findings and both agreed on the
resolutions below; the user authorized applying them. This records a planning
review, not a passed implementation gate or a benchmark result.

The source-confirmed CLMUL capability predicate and LDPC allocation/traversal
costs motivate experiments. Claims of a tenfold decoder gain, universally faster
layered modes, fixed iteration reductions, guaranteed SOTA losses and subjective
win probabilities are not accepted performance evidence. Relative SOTA results
remain unknown until quality-compatible same-host measurements exist.

## Accepted decisions

- Require a bounded quantized, layered and QC-aware LDPC exploration/decision
  checkpoint. Candidate shipping is conditional on reviewed numerical contracts
  and measured throughput, latency and BER/FER. A supported no-qualifying-candidate
  decision is valid; skipping the exploration is not.
- Separate shared protocol/statistical support from family surveys and consumer
  profiles. An unrelated comparator build must not block LDPC or the bounded
  CLMUL predicate repair.
- Survey additional LDPC candidates identified by `@/ref/Srsran2026`,
  `@/ref/Xdsopl2026` and `@/ref/OpenAirInterface2026`. Verify pinned build capabilities,
  compatibility and licenses before admitting an arm. None is designated the
  winner in advance. Closed-source implementations are context, not open-source arms.
- Make byte-field work a normal-priority consumer feasibility study of
  `FieldVec::axpy` and batched matrix operations. Include representation conversion;
  use `@/ref/Mfourrie2026` for matching matrix cells. Defer a byte-region API or
  storage redesign until consumer evidence justifies it.
- Use the [shared measurement contract](measurement-contract.md), an immutable
  executable protocol and frozen cell addenda. Replace copied rules and the inherited
  15% threshold with justified family-specific worthwhile-effect and complexity
  criteria. Keep profiling-only work free of production-adoption obligations.
- Assign decoder-specific selector calibration to the decoder story through the
  canonical tuning mechanism. Keep the final scorecard dependent on both calibration
  tracks. Require completed BCH evidence for integrated reporting, not unrelated
  generic transpose feasibility. Use a fresh current-code before measurement.
- Keep a bounded before/after benchmark, fallback checks and Rust 1.95 assembly in
  the CLMUL predicate repair. Separate exhaustive crossover/polynomial comparison
  work; do not force gf2x into an incompatible raw independent-product comparison.

## Feasibility conclusion

Missed acceleration and avoidable decoder work are concrete optimization targets.
Competing with mature decoders may require algorithm, precision and layout changes,
not just wider Boolean operations. Population count, region arithmetic and streaming
buffers may offer limited kernel-only headroom; consumer fusion and conversion costs
must decide priority. These are planning hypotheses, not measured ceilings.

The agreed changes make the measurement/profiling work actionable. Nontrivial
implementation stories still require their stated reviewed designs, MSRV probes
and worker-sized decomposition before implementation fans out. No production
optimization or benchmark campaign is performed by this plan revision.
