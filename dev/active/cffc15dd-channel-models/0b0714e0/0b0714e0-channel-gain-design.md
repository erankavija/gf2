# 0b0714e0: carry per-symbol channel gains to the soft demapper

Design proposal for JIT issue `0b0714e0`. Scope: the `gf2-sim` pipeline data
model; perfect receiver CSI. No code is changed by this document.

## 1. Current state (evidence)

| Fact | Location |
|---|---|
| `SymbolBatch` holds only `pub i`, `pub q: Vec<Vec<f32>>`; `new` asserts paired lengths; derives `PartialEq, Default, Clone` | `crates/gf2-sim/src/batch.rs:96-153` |
| Rayleigh draws `h = draw_cn01(rng)` per symbol, uses it in `r = h x + n`, and drops it | `crates/gf2-sim/src/channels/rayleigh.rs:175-196` |
| Rician builds `h_r = los_mag + scatter*v_r`, `h_i = scatter*v_i`, uses and drops it | `crates/gf2-sim/src/channels/rician.rs:219-249` |
| Word budget: `draw_standard_normal` = 4 words, `draw_cn01` = 8 words; fading = 16 words/symbol, AWGN = 8; `debug_assert!(drawn <= FRAME_STRIDE - 256)` | `channels/mod.rs:30-81`, `rayleigh.rs:19-28,197-202`, `rician.rs:30-34,249-254`, `awgn.rs:23,243-248` |
| Per-frame seek: `apply_for_frame` → `ctx.reseek_to_frame` → `apply` | `rayleigh.rs:146-154` (same shape in rician/awgn) |
| Channel stages are `Stage<SymbolBatch, SymbolBatch>`, `process` = `input.clone()` + `apply` | `rayleigh.rs:206-230`, `rician.rs:262-281`, `awgn.rs:255-275` |
| `GrayQamDemapCore::demap_frame` passes `gain_i: None, gain_q: None` and a constant `noise_var` vector | `crates/gf2-sim/src/stages/mod.rs:206-222`; frame loop `demap_batch` `:225-233` |
| The core is shared by `GrayQamDemap` (DVB-T2), `NrGrayQamDemap`, and hip `CpuGrayQamDemapper` | `stages/mod.rs:124-131`; `stages/nr_5g.rs:380-410`; `gpu/demap.rs:161-166` |
| `DemapInput { rx_i, rx_q, gain_i: Option<&[S]>, gain_q: Option<&[S]>, noise_var, method }`; `None` = unit gain | `crates/gf2-coding/src/modem/demapper.rs:71-91` |
| `FastGrayQamDemapper`: pre-rotates `z = conj(h) y`, `g = |h|^2`, `n0_eq = n0 g`, zero-gain guard `inv_n0_eq = 0`; `None` branch substitutes `(1.0, 0.0)` | `crates/gf2-coding/src/modem/fast_gray_qam_demapper.rs:12-28, 355-400`; half-specified gains panic `:834-845` |
| SIMD distance kernel takes `g = |h|^2` and `inv_n0_eq` per symbol; zero-gain contract | `crates/gf2-kernels-simd/src/modem.rs:37, 51-62, 153-163` |
| GPU demap is AWGN-only ("constant per-symbol `noise_var`, AWGN (no channel gains)") | `crates/gf2-sim/src/gpu/demap.rs:365-372`, stage `:493`, `demap_batch` `:389`, `demap_batch_on_stream` `:470`; executor route `executor/topology.rs:645-680` |
| `GpuAwgn::process` clones its input then mutates I/Q | `crates/gf2-sim/src/gpu/awgn.rs:551-580` |
| Graph edges are type-checked by `TypeId` (`output_type == edge.element_type == input_type`) | `crates/gf2-sim/src/stage.rs:203-207, 355-360`; `executor/topology.rs:917-928`; `graph/mod.rs:332` |
| Fan-in concat / split-half know four canonical batch types, rebuilding `SymbolBatch` from `i`, `q` only | `executor/topology.rs:751-779, 786-818` |
| Presets materialise only `Channel::Awgn` and couple demapper `noise_var` via `es_n0_db_to_n0` | `presets/nr_5g.rs:302-366`, `presets/dvb_t2.rs:192` |
| Tests pinning `SymbolBatch` shape/edges: `preset_vs_graph` finds the channel as the unique `SymbolBatch -> SymbolBatch` stage and drives a fixed batch into the `SymbolBatch -> LlrBatch` demapper | `tests/preset_vs_graph.rs:117-123, 313-366` |
| Other `SymbolBatch` constructors/consumers | `tests/{checkpoint_compat,channels_determinism,channels_smoke,gpu_byte_identity,gpu_demap_byte_identity,type_mismatch,dag_topology,dvb_t2_chain_via_graph,dvb_t2_stage_roundtrip}.rs`, `src/bin/{checkpoint_sweep,gpu_awgn_throughput,gpu_demap_throughput}.rs`, `examples/dvb_t2_graph_api.rs` |
| No struct-literal construction of `SymbolBatch` exists; all sites use `SymbolBatch::new` | `rg 'SymbolBatch \{'` |
| A parallel fading model exists in the coding layer (`RicianChannel`, f64 `Complex`, `generate_frame_gains`) | `crates/gf2-coding/src/fading.rs:46, 272, 413` |

Key numerical fact for REQ-04: `FastGrayQamDemapper` maps `None` gains to
`(1.0, 0.0)` (`fast_gray_qam_demapper.rs:367-370`), so explicit unit gains and
`None` produce bit-identical LLRs (`z_i = y_i`, `z_q = y_q - 0*y_i = y_q`,
`g = 1`, `n0_eq = n0`). This makes "gains absent" and "unit gains" safely
interchangeable at a fan-in boundary.

## 2. Designs

### A. Optional gain lanes on `SymbolBatch` (recommended)

```rust
pub struct ChannelGains { pub i: Vec<Vec<f32>>, pub q: Vec<Vec<f32>> } // SoA, per frame
pub struct SymbolBatch { pub i: .., pub q: .., pub gains: Option<ChannelGains> }
impl SymbolBatch {
    pub fn new(i, q) -> Self                       // gains: None (unchanged signature)
    pub fn with_gains(i, q, gains: ChannelGains) -> Self // asserts shape == (i, q)
}
```

- Fading `apply` sets `batch.gains = Some(..)` from the exact `f32` `h_r, h_i`
  already used in `r = h x + n`. RNG draws and order are untouched (REQ-05).
- `GrayQamDemapCore::demap_frame(rx_i, rx_q, gains: Option<(&[f32], &[f32])>)`
  forwards to `DemapInput::gain_i/gain_q` (REQ-02). Stage signatures stay
  `Stage<SymbolBatch, LlrBatch>`.
- AWGN on a faded batch: `clone()` preserves gains automatically (CPU and
  `GpuAwgn`). Fading on an already-faded batch: return a `StageError`
  (cascaded fading changes the effective noise law; out of scope).

### B. Distinct `FadedSymbolBatch` with typed edges

```rust
pub struct FadedSymbolBatch { pub symbols: SymbolBatch, pub gains: ChannelGains }
impl Stage<SymbolBatch, FadedSymbolBatch> for Rayleigh / Rician
impl Stage<FadedSymbolBatch, LlrBatch> for coherent demap stages
```

- Build-time `TypeMismatch` prevents feeding a faded batch to an AWGN-only
  demapper (including the GPU demapper).
- A stage type implementing `Stage` for two input types makes `erase::<I, O, S>`
  (`stage.rs:501`) ambiguous at call sites, so the coherent demappers become
  separate stage types (DVB-T2 and NR each), or a generic wrapper.
- AWGN-after-fading needs a third impl `Stage<FadedSymbolBatch, FadedSymbolBatch>`.

### C. Side-channel in scratch

Fading stages write gains into `ChannelScratch`; the demapper reads them.
Scratch is per-stage and type-erased per stage (`stage.rs:55-60, 394-396`), so
this requires a new executor-owned shared slot keyed by frame. It breaks under
fan-in concat and split-half (`topology.rs:751-818`), under worker-count
changes and checkpoint/resume (gains live outside the batch that is split,
merged, and replayed), and it is a bypass around the batch abstraction, which
the **convention-convergence** invariant classifies as a defect. Rejected.

## 3. Consequences

| Concern | A: optional lanes | B: `FadedSymbolBatch` | C: scratch |
|---|---|---|---|
| Graph type safety | Edge types unchanged; "gains present but ignored" is a runtime concern, confined to the GPU demapper (handled below) | Strongest: miswiring is a `BuildError::TypeMismatch` | None; hidden coupling |
| AWGN byte-identity (REQ-04) | AWGN never sets gains → `None` → identical `DemapInput`; derived `PartialEq` equal for `None == None` | Unchanged (AWGN types untouched) | Unchanged |
| GPU path | `GpuAwgn` propagates via `clone`. `GpuGrayQamDemapper` / `execute_gpu_demap` route a batch with gains to the CPU fallback (`accelerator-safe-fallback`: unsupported capability selects the tested fallback, which is the shared core) | GPU demap never sees faded batches (type-level); GPU AWGN needs a faded impl or fading chains stay CPU-only | Unsupported |
| Memory / SoA | +2 `f32` lanes per symbol only on fading chains; same per-frame SoA as `i`/`q`; zero cost for AWGN | Same as A plus a wrapper | Same bytes, outside the batch |
| Deterministic seeking | Gains are a pure function of the per-frame seeked RNG, carried in the batch through split/concat/resume (`deterministic-seeded-execution`) | Same | Broken across split/merge/resume |
| Presets / `Channel` enum | Unchanged shape; a later `Channel::Rayleigh` variant materialises another `SymbolBatch -> SymbolBatch` stage | Preset must pick demap stage type per channel variant | n/a |
| Change footprint | Small (list below) | Large: new batch type, new canonical case in concat/split, new coherent demap stages ×2, new AWGN impl, `preset_vs_graph` heuristic, `checkpoint_sweep`/`checkpoint_compat` call sites | Executor-wide |

## 4. Consumers that change under design A (complete)

Production:
1. `batch.rs` — add `ChannelGains`, `SymbolBatch::gains`, `with_gains`, shape assertions; update type and module docs (`batch.rs:1-23, 80-102`).
2. `channels/rayleigh.rs` — `apply` writes gains; reject already-faded input in `process` (`:175-230`); module docs `:1-36`.
3. `channels/rician.rs` — same (`:219-281`); module docs.
4. `channels/mod.rs` — module doc (`:1-15`) states fading stages attach gains.
5. `channels/awgn.rs` — no code change (clone preserves gains); doc note that gains pass through.
6. `stages/mod.rs` — `GrayQamDemapCore::demap_frame`/`demap_batch` forward gains (`:206-233`); core doc "under AWGN" (`:124-131`) and `GrayQamDemap` doc "AWGN-shaped" (`:508-515`) become "coherent, AWGN when gains are absent".
7. `stages/nr_5g.rs` — doc only (`:330-336`); behaviour inherited from the core.
8. `gpu/demap.rs` — `CpuGrayQamDemapper` inherits core behaviour (`:161-166`); `GpuGrayQamDemapper::process`/`demap_batch`/`demap_batch_on_stream` delegate to `cpu_fallback` when `gains.is_some()` (`:389, 470, 493`); doc `:365-372`.
9. `executor/topology.rs` — `execute_gpu_demap` gains check (`:645-680`); `concat_batches` concatenates gains, materialising unit gains `(1, 0)` for parts with `None` when any part has gains (bit-identical per §1) (`:773-777`); `split_half` splits gains (`:809-814`).
10. `gpu/awgn.rs` — no code change (`clone` at `:577-580`); confirm in a test.

Tests (new, per issue criteria):
- REQ-03: Rayleigh (and Rician) stage → `GrayQamDemap`/`NrGrayQamDemap` LLRs equal direct `FastGrayQamDemapper::demap_llrs` with the batch's `rx` and `gains`, bit for bit (`to_bits`), fixed seed.
- REQ-05: word-position delta after `apply` equals `16 * num_symbols` (unchanged), and `channels_determinism` cross-worker identity extended to compare gains.
- REQ-04: existing `gpu_byte_identity`, `gpu_demap_byte_identity`, `preset_vs_graph`, `checkpoint_compat`, `dvb_t2_chain_via_graph` pass unmodified.
- `concat`/`split_half` round-trip with gains and with mixed gains/`None` parts.
- hip-gated: GPU demap with a gain-carrying batch returns the CPU-fallback LLRs (`backend-behavioral-equivalence`).

Existing call sites unaffected (use `SymbolBatch::new`, read only `i`/`q`): the
test, bin, and example files listed in §1.

## 5. Invariant notes

- **library-first-generality**: coherent demapping already lives in the library
  (`DemapInput`, `FastGrayQamDemapper`); this change is the pipeline data model,
  which belongs in `gf2-sim::batch`. The fading draw itself stays in
  `gf2-sim::channels` because it is bound to the §3 ChaCha20 word-budget
  contract. `ChannelGains` should be shaped so the demap core passes slices
  straight into `DemapInput` without copying.
- **convention-convergence**: the gain representation follows `DemapInput`'s
  existing split-lane `(gain_i, gain_q)` convention and the batch's SoA
  convention; no parallel complex type is introduced. Pre-existing divergence,
  to report as a follow-up issue rather than fix here: `gf2_coding::fading`
  has its own f64 `Complex` and `RicianChannel::generate_frame_gains`
  (`fading.rs:46, 272, 413`), a second Rician model next to
  `gf2_sim::channels::Rician`.
- **deterministic-seeded-execution** / **accelerator-safe-fallback**: covered in §3.
- **present-tense-prose**: doc updates in §4 describe the resulting behaviour.

## 6. Recommendation

Design A. It satisfies REQ-01..05 with the smallest footprint, keeps every
graph edge type and preset shape unchanged, keeps AWGN bit-identical by
construction (`None` path untouched, and `None` ≡ unit gain bit-for-bit for
fan-in), and carries gains inside the unit that the executor splits, merges,
and replays. Its one type-safety gap, a gain-unaware consumer, is the GPU
demapper only, closed by routing gain-carrying batches to the shared CPU
fallback. Choose B only if more gain-unaware `SymbolBatch` consumers (e.g.
symbol-level interleavers or a second GPU symbol stage) are planned soon, since
each would need the same runtime guard under A.
