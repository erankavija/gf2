# Constellation and Labeling Data Model — Implementation Plan

**Issue:** `c87c5043` — Define constellation and labeling data model
**Parent story:** `24144d1a` — Design the general modem core API
**Epic:** `d4851c3d` — Implement QAM modulation with soft-decision demapping
**Epic design doc:** `dev/active/d4851c3d-modem-framework-design.md`
**Date:** 2026-04-13

## 1. Role of this task in the epic

This is the foundation task of the modem framework. Every downstream story consumes its output:

| Downstream consumer | What it needs from this task |
|---|---|
| `d36ae697` Batched map/demap traits | `ModemSpec<S>`, `ModemView<'a, S>`, `DemapMethod`, `SymbolPoint<S>`, `LabelWord`, `Normalization` |
| `3e3fe377` Modem builders and validation | Builder seed entry points + the sealed invariants this task establishes |
| `51334873` Arbitrary-constellation reference path | `ModemView` slice accessors; exact log-MAP path reads points and labels directly |
| `52112411` Gray square-QAM fast path | `ModemSpec::gray_square_qam(order)` presets, `BitChannelSemantics::{IAxisPam, QAxisPam}` |
| `e2c0f65a` Bit-channel analysis | `BitChannelId`, `BitChannelSemantics`, `DemapMethod` recording in outputs |
| `92186a40` Simulation/channel refactor | `ModemSpec::bpsk()` preset for BPSK/AWGN migration; QPSK preset for Rician migration |
| `46ffe45a` Legacy modem surface deletion | Same |
| `003e4088` DVB-T2 FEC integration | Gray-QAM preset for 16/64/256-QAM with DVB-T2-compatible bit ordering |

Because every branch of the DAG reads from this task, **get the invariants and type names right now**. A breaking change here forces renames in eight downstream issues.

## 2. Design decisions (locked)

Decided in the 2026-04-13 interview. Rationale summarized inline; see `dev/active/d4851c3d-modem-framework-design.md` for the epic-level design context.

| # | Decision | Rationale |
|---|---|---|
| D1 | `ModemSpec` is generic over a sealed `ModemScalar` trait. | Flexibility between f32 (SIMD/GPU default) and f64 (current reference path) without forcing a choice now. |
| D2 | `ModemScalar` is **sealed**, implemented only for `f32` and `f64`. | Users can't implement it externally. Unsealing later is an additive, semver-minor change if fixed-point demand emerges. Keeps specialization clean. |
| D3 | `pub type DefaultScalar = f32`. Presets default to `ModemSpec<f32>`. | Matches `Llr(f32)`, maximizes SIMD lane density, halves lookup-table memory. Explicit f64 available via generic builders when needed. |
| D4 | Constellation points are stored **post-normalized**; the scale factor and original `Normalization` request are retained on the spec. | Zero per-symbol multiply in the hot path. Analysis/export can recover the unit-grid geometry via the stored scale. |
| D5 | Canonical LLR ordering is **MSB-first within a symbol, symbol-major across symbols**. Bit `k` of the label word with MSB index 0 corresponds to LLR position `k` within each symbol. | Matches the existing `QpskModulator::symbols_to_llrs` layout; simplifies DVB-T2 BICM wiring where bit-to-cell mapping is defined MSB-first. |
| D6 | Label storage is `u16` per symbol. | Admits up to 16-bit research constellations (64K-QAM) at negligible cost. Every preset uses ≤8 bits; spec records the actual `bits_per_symbol: u8`. |
| D7 | Construction is sealed. Public `ModemSpec<S>` has private fields; public `ModemView<'a, S>` exposes **both contiguous slices and per-item accessors**. | Backends that want SIMD/GPU-friendly arrays get slices; analysis and examples get ergonomic point-at-a-time access without reshape. |
| D8 | Invalid construction **panics with descriptive messages**. No `Result`/error enum on builders in this task. | Matches existing crate style (`AwgnChannel::from_variance` panics). Builder callers are library code with static-known inputs; user-supplied custom constellations get the same panic behavior with an explicit message. |
| D9 | `DemapMethod` (`ExactLogMap`, `MaxLog`) is defined here, alongside `ModemCapabilities { supports_exact_log_map: bool, supports_max_log: bool }`. | Builders can reject method/spec mismatches (e.g., a preset declares it doesn't yet support exact log-MAP) at construction rather than at the trait layer. Trait task `d36ae697` consumes these types; it doesn't redefine them. |
| D10 | `BitChannelSemantics` ships on day one with day-one presets: `Opaque(u8)` for arbitrary constellations, `SingleAxisPam(u8)` for BPSK, `IAxisPam(u8)`/`QAxisPam(u8)` for Gray square-QAM. | Analysis story `e2c0f65a` consumes it without retrofitting the data model. The `Opaque` fallback costs nothing semantically; the labelled variants unlock richer analysis later. |

## 3. Module layout

All new code lives under `crates/gf2-coding/src/modem/`. This task creates only the types/definition files; trait files and backends follow in later tasks.

```text
crates/gf2-coding/src/
  modem/
    mod.rs           # public re-exports; keeps the module surface curated
    scalar.rs        # sealed ModemScalar trait, DefaultScalar alias
    types.rs         # SymbolPoint, LabelWord, BitChannelId, BitChannelSemantics,
                     # Normalization, DemapMethod, ModemCapabilities
    spec.rs          # ModemSpec<S> (sealed) + internal invariant helpers
    view.rs          # ModemView<'a, S> with slice + per-item accessors
    presets.rs       # construction seeds for BPSK and Gray square-QAM
                     # (Full builders land in task 3e3fe377; this file provides
                     # the preset-side entry points that the builder task
                     # will wire up.)
```

`mod.rs` re-exports from `lib.rs` as:

```rust
pub mod modem;
// lib.rs additionally re-exports selected items at the crate root for
// backward-compatible import paths (e.g., pub use modem::DefaultScalar;).
```

## 4. Type surface

This is the complete public surface owed by `c87c5043`. Downstream tasks build on these exact names and shapes.

### 4.1 `modem/scalar.rs`

```rust
mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// Scalar used for constellation I/Q coordinates and demapper math.
/// Sealed: implemented only for f32 and f64.
pub trait ModemScalar: sealed::Sealed + Copy + PartialOrd + core::fmt::Debug + 'static {
    fn zero() -> Self;
    fn one() -> Self;
    fn two() -> Self;
    fn from_f64(v: f64) -> Self;
    fn to_f32(self) -> f32; // for producing Llr values
    fn sqrt(self) -> Self;
    fn abs(self) -> Self;
    fn mul_add(self, a: Self, b: Self) -> Self;
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn min(self, other: Self) -> Self;
    fn max(self, other: Self) -> Self;
}

impl ModemScalar for f32 { /* trivial */ }
impl ModemScalar for f64 { /* trivial */ }

/// Default scalar for presets and most downstream code.
pub type DefaultScalar = f32;
```

### 4.2 `modem/types.rs`

```rust
/// An I/Q constellation point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolPoint<S: ModemScalar> {
    pub i: S,
    pub q: S,
}

impl<S: ModemScalar> SymbolPoint<S> {
    pub fn new(i: S, q: S) -> Self;
    pub fn energy(self) -> S; // i*i + q*q
}

/// Bit label for a single symbol. Bit k corresponds to LLR position k within
/// the symbol under the MSB-first intra-symbol ordering (decision D5).
/// `width` is the number of meaningful MSBs used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelWord {
    pub bits: u16,
    pub width: u8,
}

impl LabelWord {
    /// Panics if `width > 16` or `bits >> width != 0`.
    pub fn new(bits: u16, width: u8) -> Self;
    pub fn bit(self, k: u8) -> bool; // k = 0 is MSB; panics if k >= width
}

/// Identifier for a bit position within a symbol. k = 0 is the MSB
/// under the canonical ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitChannelId {
    pub bit_index: u8,
}

/// Semantic role of a bit position. Set by presets; Opaque for arbitrary
/// constellations. Analysis and documentation consume this but hot demap
/// loops never read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitChannelSemantics {
    Opaque(u8),          // arbitrary constellation, index only
    SingleAxisPam(u8),   // BPSK preset
    IAxisPam(u8),        // Gray square-QAM, in-phase axis PAM bit
    QAxisPam(u8),        // Gray square-QAM, quadrature axis PAM bit
}

/// Normalization contract. Points are stored post-normalized (D4);
/// the retained variant records what the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Normalization<S: ModemScalar> {
    UnitAverageSymbolEnergy,
    ExplicitEs(S),
}

/// Selectable demapper semantics. Locked in this task (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemapMethod {
    ExactLogMap,
    MaxLog,
}

/// Which demap methods a given modem spec currently supports. Builders
/// populate this; the trait layer (d36ae697) reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModemCapabilities {
    pub supports_exact_log_map: bool,
    pub supports_max_log: bool,
}
```

### 4.3 `modem/spec.rs`

```rust
/// Sealed, validated modem description. All construction goes through
/// builders / presets; fields are private so invariants cannot be broken
/// after construction.
#[derive(Debug, Clone)]
pub struct ModemSpec<S: ModemScalar> {
    // points stored post-normalized (D4); indexed by label bits (as integer)
    points: Vec<SymbolPoint<S>>,
    labels: Vec<LabelWord>,                // same length as points
    bits_per_symbol: u8,                   // >= 1, <= 16
    bit_channels: Vec<BitChannelSemantics>, // length bits_per_symbol
    normalization: Normalization<S>,
    normalization_scale: S,                // factor applied to raw grid
    capabilities: ModemCapabilities,
}

impl<S: ModemScalar> ModemSpec<S> {
    /// Borrowed view of the spec. Backends and analysis consume this.
    pub fn view(&self) -> ModemView<'_, S>;

    pub fn bits_per_symbol(&self) -> u8;
    pub fn num_symbols(&self) -> usize; // == 1 << bits_per_symbol
    pub fn normalization(&self) -> Normalization<S>;
    pub fn normalization_scale(&self) -> S;
    pub fn capabilities(&self) -> ModemCapabilities;
}
```

### 4.4 `modem/view.rs`

```rust
/// Borrowed read-only view over a ModemSpec. Provides both contiguous
/// slices (for SIMD/GPU backends) and per-item accessors (for analysis
/// and examples). Decision D7.
#[derive(Debug, Clone, Copy)]
pub struct ModemView<'a, S: ModemScalar> {
    points: &'a [SymbolPoint<S>],
    labels: &'a [LabelWord],
    bit_channels: &'a [BitChannelSemantics],
    bits_per_symbol: u8,
    normalization: Normalization<S>,
    normalization_scale: S,
    capabilities: ModemCapabilities,
}

impl<'a, S: ModemScalar> ModemView<'a, S> {
    // Contiguous slices
    pub fn points(&self) -> &'a [SymbolPoint<S>];
    pub fn labels(&self) -> &'a [LabelWord];
    pub fn bit_channels(&self) -> &'a [BitChannelSemantics];

    // Per-item accessors; panic on out-of-range
    pub fn point(&self, idx: usize) -> SymbolPoint<S>;
    pub fn label(&self, idx: usize) -> LabelWord;
    pub fn bit_channel(&self, bit_idx: u8) -> BitChannelSemantics;
    pub fn bit_channel_id(&self, bit_idx: u8) -> BitChannelId;

    pub fn num_symbols(&self) -> usize;
    pub fn bits_per_symbol(&self) -> u8;
    pub fn normalization(&self) -> Normalization<S>;
    pub fn normalization_scale(&self) -> S;
    pub fn capabilities(&self) -> ModemCapabilities;
}
```

### 4.5 `modem/presets.rs` (seeds only)

This file ships preset constructors whose full implementation overlaps with task `3e3fe377`. In this task we ship:

- Signatures + doc comments
- Working `ModemSpec::bpsk()` preset (trivial; 2 points, 1 bit)
- Working `ModemSpec::gray_square_qam(order: usize)` preset for `order ∈ {2, 4, 16, 64, 256}`
- Tests that validate invariants for every shipped preset

Task `3e3fe377` will add the general `custom_constellation` builder and richer validation; preset work is already done by the end of `c87c5043`.

```rust
impl ModemSpec<DefaultScalar> {
    /// BPSK: ±1 on the I axis, bit 0 → +1, bit 1 → -1, unit energy.
    pub fn bpsk() -> Self;

    /// Gray-coded square QAM. `order` must be one of 2, 4, 16, 64, 256
    /// (BPSK, QPSK, 16-QAM, 64-QAM, 256-QAM). `order = 2` is equivalent
    /// to `bpsk()`. Unit average symbol energy. Panics otherwise.
    pub fn gray_square_qam(order: usize) -> Self;
}

// Generic variants for f64 research workflows
impl<S: ModemScalar> ModemSpec<S> {
    pub fn bpsk_with_scalar() -> Self;
    pub fn gray_square_qam_with_scalar(order: usize) -> Self;
}
```

Bit-to-symbol mapping for Gray square QAM (locked):

- `m = log2(order)` total bits; `m/2` I-axis bits + `m/2` Q-axis bits for even `m`.
- For `m = 1` (BPSK): single `SingleAxisPam(0)` bit.
- For `m = 2` (QPSK): `[IAxisPam(0), QAxisPam(0)]` — bit 0 (MSB) is I, bit 1 is Q. Matches current QPSK layout.
- For `m ≥ 4`: first `m/2` MSBs are the I-axis Gray-PAM label (top-down by PAM significance); remaining `m/2` bits are Q-axis Gray-PAM label. This matches DVB-T2 EN 302 755 Table 14 bit-to-cell mapping.
- I-axis and Q-axis PAM bit k = 0 is the most significant PAM bit (coarsest level).

Normalization: unit average symbol energy. For square `M = 2^m` QAM over the symmetric PAM grid `{±1, ±3, ..., ±(√M − 1)}` on each axis, the unnormalized average energy is `2 · (M − 1) / 3`, so the scale factor is `sqrt(3 / (2 · (M − 1)))`. Store this scale, multiply points through it at construction.

## 5. Invariants enforced by construction

All invariants panic with a descriptive message on violation (D8). The sealed spec (D7) means these are established once at construction and trusted everywhere downstream — backends and analysis never need to re-verify.

1. `bits_per_symbol ∈ [1, 16]`.
2. `points.len() == labels.len() == 1 << bits_per_symbol`.
3. `bit_channels.len() == bits_per_symbol`.
4. Every `LabelWord` has `width == bits_per_symbol` and `bits < (1 << bits_per_symbol)`.
5. Labels are a **bijection** over `{0, ..., 2^bits_per_symbol - 1}`: no duplicates, no missing labels.
6. Points are stored post-normalized: under `UnitAverageSymbolEnergy`, `(1/N) · Σ (i² + q²) ≈ 1` within a tight tolerance (`1e-5` for f32, `1e-10` for f64).
7. `normalization_scale > 0`.
8. `capabilities.supports_exact_log_map || capabilities.supports_max_log` is true.
9. For presets declared as Gray-coded square QAM, adjacent I-axis PAM levels differ in exactly one I-label bit, and likewise for Q. This is tested at preset construction but checked via a dedicated `cfg(test)` helper, not at runtime in release builds.

## 6. Test matrix

Tests live in `#[cfg(test)] mod tests` alongside each module file, plus a `crates/gf2-coding/tests/modem_data_model.rs` integration file.

### 6.1 Type-level tests

- `LabelWord::new` panics on `width > 16`; panics when `bits >> width != 0`; `bit(k)` returns MSB-first.
- `BitChannelSemantics` variants construct and compare.
- `SymbolPoint::energy` over f32 and f64 matches `i² + q²`.

### 6.2 Preset tests

For each preset in `{BPSK, QPSK, 16-QAM, 64-QAM, 256-QAM}`:

- `num_symbols() == 1 << bits_per_symbol()`.
- Labels form a bijection over `0..2^m`.
- Post-normalization: empirical average symbol energy equals 1.0 within tolerance.
- For Gray square QAM, adjacent I-axis points (sorted by I coordinate, same Q) differ in exactly one I-label bit; likewise for Q-axis. Validates the Gray structure.
- For QPSK: exact point locations match the current `QpskModulator` layout after scaling (regression safety for the legacy surface migration).
- `capabilities.supports_exact_log_map == true` and `capabilities.supports_max_log == true` for every preset shipped here.

### 6.3 Invariant panic tests

Each invariant listed in §5 gets a dedicated `#[should_panic(expected = "...")]` test that constructs an intentionally broken spec via a crate-internal `unsafe`-free helper (e.g., a `cfg(test)` raw constructor) and verifies the panic message.

### 6.4 View tests

- `ModemView` slice accessors and per-item accessors return the same data.
- `bit_channel_id(k)` matches `BitChannelId { bit_index: k }` for all k in `0..bits_per_symbol`.
- View is `Copy` and usable across backend boundaries.

### 6.5 Property tests (proptest)

- For random valid `(bits_per_symbol, shuffle)` pairs, constructing a custom-label permutation of `0..2^m` produces a valid spec. (Uses a crate-internal constructor stub; the full public builder lands in task `3e3fe377`.)
- Duplicated or missing labels in random candidate spec panic with the expected message.

## 7. Out of scope for this task

- Batched mapper/demapper traits → task `d36ae697`.
- General `custom_constellation(...)` public builder → task `3e3fe377` (this task ships only presets).
- Any actual demapper math (exact log-MAP or max-log) → story `51334873` and `52112411`.
- Rewiring `channel.rs`, `modulation.rs`, `fading.rs` → story `92186a40` / `46ffe45a`.
- Bit-channel analysis collectors and MI/GMI estimators → story `e2c0f65a`.

What _is_ in scope: every type, every invariant, every preset shipping `BitChannelSemantics`, and the test matrix above.

## 8. Implementation sequence

1. Create `modem/` module, wire into `crates/gf2-coding/src/lib.rs`.
2. Add `scalar.rs` — sealed trait + f32/f64 impls + `DefaultScalar` alias.
3. Add `types.rs` — all value types listed in §4.2.
4. Add `spec.rs` — sealed `ModemSpec<S>` with private fields and public accessors.
5. Add `view.rs` — `ModemView<'a, S>` with slices + per-item accessors.
6. Add `presets.rs` — BPSK and Gray square-QAM (2/4/16/64/256) presets with `BitChannelSemantics` populated.
7. Add unit tests per-file; add integration test file.
8. Run `cargo test -p gf2-coding --release`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
9. Run `cargo doc --no-deps` to verify doc examples compile.

## 9. Risks and open items

- **Precision of preset normalization in f32:** unit-energy invariant tolerance is relaxed to `1e-5` for f32; tighter for f64. If this proves insufficient for downstream demapper tests, fall back to computing the scale in f64 and converting at the end. Preset tables are small (≤256 points), so this is safe.
- **DVB-T2 bit-to-cell ordering drift:** §4.5 fixes the preset ordering to match DVB-T2 EN 302 755 Table 14. Downstream DVB-T2 integration (`003e4088`) must validate this against a known TX vector. If the validation fails, the fix stays local to `presets.rs` without spec-level change.
- **Sealed spec vs external research extensions:** users who want a custom constellation today can only get it when `3e3fe377` lands. This is acceptable — c87c5043 exists to freeze the types, not to ship the general builder.
- **Future fixed-point LLR path:** sealing `ModemScalar` to f32/f64 is not the barrier; the `Llr(f32)` type elsewhere in the crate is. Revisit as part of any future fixed-point epic, not here.

## 10. Gate exit criteria

Per `jit_issue_show c87c5043`, required gates: `tdd-reminder`, `cargo-ci`, `code-review`, `doc-review`.

- `cargo-ci`: `cargo test -p gf2-coding --release` green, `cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
- `tdd-reminder`: tests precede implementation commits per crate convention.
- `doc-review`: every public item in §4 has a doc comment with an `# Examples` block that `cargo test --doc` exercises.
- `code-review`: sealed trait / private fields / panic-only error surface reviewed by a second agent before marking done.
