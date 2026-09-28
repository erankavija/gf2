# Shift and permutation investigation

> **Diátaxis Type:** Reference (planning investigation)

## Scope and classifications

This source sweep covers tests, benchmarks, examples, docs, and linked
evidence. The sweep itself rebuilt no comparator and ran no timing; the linked
profiles and confirmations supply measured outcomes. **Observed** is proven by
current source/artifact; **hypothesis** requires predeclared measurement;
**inapplicable** means the named operation is not a gf2 production operation;
**withdrawn** cannot support adoption.

| Premise / criterion | Classification | Evidence and consequence |
|---|---|---|
| LE indexing and clean tails | Observed | `crates/gf2-core/src/bitvec.rs:39-46`; shifts remask at `:475-523`, `:536-585`. |
| Word and residual shifts dispatch | Observed | [`BitVec`](../../../crates/gf2-core/src/bitvec.rs) uses the logical kernel bundle for word-aligned shifts when `simd` and the host capability permit it. Residual offsets enter the [residual route](../../../crates/gf2-core/src/residual_shift.rs), which selects the BMI2 funnel behind the same cargo feature and observed `bmi2`, with the portable scalar funnel as fallback. |
| Residual AVX2 lane-crossing/BMI2 forms | Mixed observed and hypothesis | Both forms compile and emit their nominated instructions at Rust 1.95 ([feasibility record](shift-feasibility-record.md)). The [shift profile](shift-profile.md) finds material residual cost against a different word-aligned operation. The selected BMI2 route has a same-operation [accepted confirmation](../00dd43c3/confirmation-outcome.md) and is retained. AVX2 lane crossing remains feasible and unselected, with no measured candidate win. |
| QC rotation is a frame primitive | Inapplicable | gf2 emits construction-time sparse edges `(i + shift) % Z` (`crates/gf2-coding/src/ldpc/core.rs:456-545`), with no per-frame rotation. |
| DVB and NR have comparable arms | Observed with withdrawn history | The [DVB profile](dvb-interleave-profile.md) uses a rebuilt, operation-equivalent xdsopl arm in an accepted protocol-v4 exploratory receipt; its packed whole-consumer cells are no-win. Earlier warm evidence stays withdrawn. The [AFF3CT NR confirmation](../eda07788/findings.md) is accepted and records an AFF3CT no-win against gf2. |
| REQ-01 | Observed | The [publication tables](publication-tables.md) pin accepted receipt, protocol and addendum identities for the shift profile, same-operation BMI2 confirmation, DVB profile and retained NR no-win. The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md) governs adoption and preserves negative outcomes. |
| REQ-07 | Observed | Separate family dispositions are complete: the [shift profile](shift-profile.md) finds materiality, the [plan](plan.md) selects BMI2, and its [confirmation](../00dd43c3/confirmation-outcome.md) qualifies; [DVB](dvb-interleave-profile.md) is not material and nominates none; QC runtime rotation is inapplicable; accepted NR depuncture is a no-win. |
| REQ-08 | Observed | The [planning-time Rust 1.95 record](shift-feasibility-record.md) covers AVX2 and BMI2. The selected [BMI2 kernel route](../f8dd4dde/retention-rule.md) dispatches on observed capability with a scalar fallback; its [accepted confirmation](../00dd43c3/confirmation-outcome.md) retains it. AVX2 is feasible and unselected under the [reviewed plan](plan.md). |
| REQ-09 | Observed | The [shift profile](shift-profile.md), [feasibility oracle](shift-feasibility-record.md), and [route evidence](../f8dd4dde/retention-rule.md) cover zero-fill, offsets 0/1/7/8/63/64/65, overlength, lane crossings, incomplete tails and aliasing. The [DVB profile](dvb-interleave-profile.md) validates the ETSI order; QC wrap stays distinct. |
| REQ-10 | Observed | xdsopl `PCTITL` and AFF3CT depuncture are the operation-equivalent external arms. The [DVB profile](dvb-interleave-profile.md) pins rebuilt arms and unpack/copy/pack costs; the [NR findings](../eda07788/findings.md) preserve its accepted no-win. AFF3CT does not supply a shift/rotation comparator. |
| REQ-11 | Observed | The [publication tables](publication-tables.md) derive shift latency/throughput, DVB packed whole-stage and BICM costs, and adapter passes from accepted receipts and the repeated profile. `gf2-core` owns zero-fill dispatch; `gf2-coding` owns interleave and rate matching. |

## Exhaustive consumer sweep

| Family | Production consumer / representation | Tests, benchmarks, docs, adapters | Disposition |
|---|---|---|---|
| Residual and word zero-fill shifts | Public in-place `BitVec`; no downstream production call. Word shifts consult `LogicalFns` when available; the [residual route](../../../crates/gf2-core/src/residual_shift.rs) selects BMI2 under `simd` and observed `bmi2`, otherwise the safe scalar funnel. | `crates/gf2-core/benches/shifts.rs:1-205`; units/Kani and property tests cited above; [profile](shift-profile.md) and [confirmation](../00dd43c3/confirmation-outcome.md). | The exploratory scalar-versus-word control establishes material workload cost. Same-operation BMI2 confirmation qualifies and the frozen rule retains the route; AVX2 remains feasible and unselected. |
| QC/circulant | `to_edges` feeds construction and NR `from_quasi_cyclic`, not frame rotation. | `crates/gf2-coding/tests/qc_ldpc_tests.rs:76-`; `examples/qc_ldpc_demo.rs:38-78`; sparse emitter `examples/bench_sparse_csv_emitter.rs:936-970`. | Preserve unmatched/inapplicable; no leaf without a new frame consumer. |
| Generic puncturing | `Punctured` derives coordinate maps/generator, not packed bit movement (`crates/gf2-coding/src/transform/mod.rs:1347-1380`; `coordinate_map.rs:256-280`). | BCH conformance `tests/bch_conformance.rs:301-323`. | Separate algebraic construction family. |
| NR selection/de-rate match | Private encoding gathers `transmitted_cols`; `prepare_llrs` allocates zeroed full LLRs, scatters and fills (`nr_5g/mod.rs:901-938,1172-1203`). Sim/CPU/GPU consume rate-matched code (`crates/gf2-sim/src/stages/nr_5g.rs:50-120`; `benches/nr_5g_realtime.rs:289-318`). | AFF3CT selection agrees but gf2 exposes no corresponding public operation, so no harness timing ([prior findings](../eda07788/findings.md)). The depuncture adapter zeroes and converts inside each timed call. | The accepted AFF3CT comparison is a no-win and the native control is not material. `prepare_llrs` is unchanged, so its [confirmation](../../bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/acceptance-summary.md) remains authoritative. |
| DVB-T2 bit interleave | Packed `BitVec` scatter using precomputed maps (`crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs:473-601`); coding harness `dvb_t2_bicm_harness.rs:360-394`; framewise sim stages `crates/gf2-sim/src/stages/mod.rs:332-437`. | ETSI TP07a/BICM tests and `examples/dvb_t2_bicm_chain.rs:97-206`. The xdsopl adapter unpacks, copies overwritten input, allocates output, permutes and packs; the [DVB tables](dvb-interleave-profile-tables.md) account for each pass. | The [accepted protocol-v4 profile](dvb-interleave-profile.md) finds the packed scatter not material and nominates no form. Withdrawn warm evidence remains historical. |
| Other interleaving | NR modulation interleave is `Vec<bool>`/`Vec<Llr>` (`crates/gf2-coding/src/ldpc/nr_5g/interleaver.rs:1-240`), consumed by sim stages (`gf2-sim/src/stages/nr_5g.rs:215-255`); generic fading has another interleaver. | Source/test/doc only; no external mapping. | Separate, unmatched family; do not merge with DVB. |

## Prior art, feasibility, and fit

Pinned survey: xdsopl/LDPC (DVB PCTITL), AFF3CT v4.7.0 (puncture/depuncture,
fast QC encoder), and source-only srsRAN (`findings.md:83-108`). srsRAN rotates
circularly, semantically unlike zero-fill (`findings.md:479-491`). AFF3CT generic
user interleaving/non-equivalent NR configurations are unmeasured; srsRAN
derate matching is unavailable and non-bit-exact (`findings.md:114-120,447-455,521-523`).

CPU intrinsic code belongs in `gf2-kernels-simd`; `gf2-core` keeps safe
capability-gated dispatch through the word-shift `LogicalFns` bundle and the
[residual funnel route](../../../crates/gf2-core/src/residual_shift.rs). The
portable funnel remains the fallback. The [feasibility record](shift-feasibility-record.md)
provides the Rust 1.95 compile, assembly and fallback evidence for both nominated
forms; the [confirmation outcome](../00dd43c3/confirmation-outcome.md) decides
retention of the selected BMI2 route.

No lasting architectural choice needs the invoker: retain zero-fill BitVec
semantics and keep QC construction, DVB permutation, and NR LLR mapping separate.
