# Shift and permutation investigation

> **Diátaxis Type:** Reference (planning investigation)

## Scope and classifications

This read-only sweep covers source, tests, benchmarks, examples, docs, and linked
evidence; it rebuilt no comparator and ran no timing. **Observed** is proven by
current source/artifact; **hypothesis** requires predeclared measurement;
**inapplicable** means the named operation is not a gf2 production operation;
**withdrawn** cannot support adoption.

| Premise / criterion | Classification | Evidence and consequence |
|---|---|---|
| LE indexing and clean tails | Observed | `crates/gf2-core/src/bitvec.rs:39-46`; shifts remask at `:475-523`, `:536-585`. |
| Word shifts dispatch; residual shifts loop | Observed | Dispatch is only `k % 64 == 0` (`bitvec.rs:488-520`, `:549-583`); AVX2 moves whole words (`crates/gf2-kernels-simd/src/x86/avx2.rs:457-550`). |
| Residual AVX2 lane-crossing/BMI2 wins | Hypothesis | Both forms compile and emit their nominated instructions at 1.95 ([feasibility record](shift-feasibility-record.md)), and the residual branch costs more than the word-aligned control at consequential sizes ([shift profile](shift-profile.md) §Result); neither form has a measured win. BMI2 is separately detected but unused (`crates/gf2-core/src/kernels/x86.rs:39-43`). |
| QC rotation is a frame primitive | Inapplicable | gf2 emits construction-time sparse edges `(i + shift) % Z` (`crates/gf2-coding/src/ldpc/core.rs:456-545`), with no per-frame rotation. |
| DVB and NR have comparable arms | Mixed observed | xdsopl DVB mapping is bit-exact but old warm evidence is withdrawn; repaired cells are exploratory only. AFF3CT depuncture is confirmed and gf2 wins (`dev/active/eda07788/findings.md:10-43,100-108`). |
| REQ-01 | Delivery contract | Require frozen protocol/addendum, paired release before/after receipt, correctness/adoption rule, and preserved negatives (`dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:7-46`). |
| REQ-07 | Partly observed / selection-gated | Profile separately. QC is inapplicable; DVB needs a fresh eligible campaign; NR depuncture is measured no-win; shifts lack materiality evidence. |
| REQ-08 | Satisfied prerequisite | The planning-time 1.95 compile and assembly record covers both nominated residual-shift forms ([feasibility record](shift-feasibility-record.md)), and the invoker selects the BMI2-gated form for implementation. The existing target-feature scopes and bundle establish the integration shape (`avx2.rs:459-460,707-745`; `crates/gf2-core/src/lib.rs:101-121`). |
| REQ-09 | Hard semantic contract | Cover zero-fill (not wrap), 0/1/7/8/63/64/65, overlength, lanes, incomplete tails and aliasing. Random reference checks exist (`crates/gf2-core/tests/property_tests.rs:76-96,223-249`); committed analysis validates 148 zero-fill cases (`dev/active/eda07788/survey/analysis-output-v3.txt:14-15`). |
| REQ-10 | Mapping-gated | Only xdsopl PCTITL and AFF3CT depuncture map. Rebuild xdsopl arms for a new eligible DVB campaign; AFF3CT is not a shift/rotation comparator. |
| REQ-11 | Delivery contract | Report kernel latency/throughput, owning whole-consumer cost, and every conversion/memory pass. Core owns zero-fill; coding owns interleave/rate matching. |

## Exhaustive consumer sweep

| Family | Production consumer / representation | Tests, benchmarks, docs, adapters | Disposition |
|---|---|---|---|
| Residual and word zero-fill shifts | Public in-place `BitVec`; no downstream production call. Word shifts use `LogicalFns`; residual uses safe `u64` carries. | `crates/gf2-core/benches/shifts.rs:1-205`; units/Kani `bitvec.rs:2037-2094,2704-2866`; property tests above; `examples/bitvec_basics.rs:122-134`. | Smallest measurable family: the residual L/R scalar path against the word-aligned control for materiality, then the selected BMI2 candidate against that scalar path for confirmation. |
| QC/circulant | `to_edges` feeds construction and NR `from_quasi_cyclic`, not frame rotation. | `crates/gf2-coding/tests/qc_ldpc_tests.rs:76-`; `examples/qc_ldpc_demo.rs:38-78`; sparse emitter `examples/bench_sparse_csv_emitter.rs:936-970`. | Preserve unmatched/inapplicable; no leaf without a new frame consumer. |
| Generic puncturing | `Punctured` derives coordinate maps/generator, not packed bit movement (`crates/gf2-coding/src/transform/mod.rs:1347-1380`; `coordinate_map.rs:256-280`). | BCH conformance `tests/bch_conformance.rs:301-323`. | Separate algebraic construction family. |
| NR selection/de-rate match | Private encoding gathers `transmitted_cols`; `prepare_llrs` allocates zeroed full LLRs, scatters and fills (`nr_5g/mod.rs:901-938,1172-1203`). Sim/CPU/GPU consume rate-matched code (`crates/gf2-sim/src/stages/nr_5g.rs:50-120`; `benches/nr_5g_realtime.rs:289-318`). | AFF3CT selection agrees but gf2 exposes no corresponding public operation, so no harness timing (`findings.md:351-365`). Depuncture adapter zeroes/converts inside timing and is not the gap (`:367-395,432-445`). | Preserve confirmed no-win and not-material native-control. Rebuild AFF3CT only after changing `prepare_llrs`. |
| DVB-T2 bit interleave | Packed `BitVec` scatter using precomputed maps (`crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs:473-601`); coding harness `dvb_t2_bicm_harness.rs:360-394`; framewise sim stages `crates/gf2-sim/src/stages/mod.rs:332-437`. | ETSI TP07a/BICM tests and `examples/dvb_t2_bicm_chain.rs:97-206`. xdsopl adapter unpacks, allocates/copies its overwritten input, permutes, then packs (`findings.md:125-`). Repair is `3e59cb9a`. | Rebuild both frozen gf2/xdsopl arms. Measure packed scatter vs PCTITL, including unpack/copy/pack/cache. Old warm evidence is withdrawn; repaired cells cannot confirm. |
| Other interleaving | NR modulation interleave is `Vec<bool>`/`Vec<Llr>` (`crates/gf2-coding/src/ldpc/nr_5g/interleaver.rs:1-240`), consumed by sim stages (`gf2-sim/src/stages/nr_5g.rs:215-255`); generic fading has another interleaver. | Source/test/doc only; no external mapping. | Separate, unmatched family; do not merge with DVB. |

## Prior art, feasibility, and fit

Pinned survey: xdsopl/LDPC (DVB PCTITL), AFF3CT v4.7.0 (puncture/depuncture,
fast QC encoder), and source-only srsRAN (`findings.md:83-108`). srsRAN rotates
circularly, semantically unlike zero-fill (`findings.md:479-491`). AFF3CT generic
user interleaving/non-equivalent NR configurations are unmeasured; srsRAN
derate matching is unavailable and non-bit-exact (`findings.md:114-120,447-455,521-523`).

Intrinsic code belongs only in `gf2-kernels-simd`; core retains safe canonical
dispatch/fallback through `LogicalFns` (`crates/gf2-kernels-simd/src/lib.rs:130-143`;
`crates/gf2-core/src/lib.rs:101-121`). The 1.95 compile, assembly and
runtime-gated scalar-fallback evidence the contract demands before breakdown
(`measurement-contract.md:108-113`) is the
[feasibility record](shift-feasibility-record.md) for both nominated
residual-shift forms.

No lasting architectural choice needs the invoker: retain zero-fill BitVec
semantics and keep QC construction, DVB permutation, and NR LLR mapping separate.
