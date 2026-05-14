# gf2-algebra public API freeze for W6 Lean verification

**Issue:** `8c902184` — Freeze gf2-algebra public API surface for verification
**Epic:** `ae82bd73` — Fast matrix permanents over F_3 / F_5 / F_7
**Drafted:** 2026-05-14 (session 8)
**Status:** Awaiting user sign-off (see `## Approval` section)

## Why this freeze

The W6 Lean verification work (`f05ffbe1`, `0606186a`, `30e98ef1`) compiles
Rust → LLBC → Lean via Charon/Aeneas against the `gf2-algebra` public surface.
Per CLAUDE.md's "Verification work" section, proof code is brittle to API
drift: a Charon regeneration after a signature change can silently invalidate
hours of Lean proof work. This document records an explicit freeze checkpoint:
the public API of the symbols listed below is locked against breaking changes
for the duration of W6, and post-freeze breaking changes require explicit user
approval AND a corresponding re-verification of impacted V1 / V2 / V3 proofs.

This is a documentation-only checkpoint. It does not change any code. The
implementation has already been locked by the W4 sub-4b close commits
(`f1867c9a` and earlier).

## Locked symbols

All symbols below are at their state in `main @ f1867c9a`. Each entry names
the canonical public path; aliases re-exported via `pub use` from
`gf2_algebra::packed` and `gf2_algebra::permanent` are also frozen at the
re-export.

### `gf2_algebra::packed::*`

| Symbol | Defined in | Notes |
|---|---|---|
| `PackedField<F>` (trait) | `packed/mod.rs:88` | The trait surface itself is frozen; new methods would break impl-completeness for downstream Lean proofs. |
| `PackedFieldVec<F>` (trait) | `packed/mod.rs:374` | Same |
| `Bipedal3` (struct) | `packed/bipedal3.rs:85` | F_3 packed type, the core W3 deliverable. |
| `Bipedal3Vec` (struct) | `packed/bipedal3.rs:1416` | |
| `Bipedal3Matrix` (struct) | `packed/bipedal3.rs:2784` | |
| `ScalarPackedFp3` (struct) | `packed/scalar.rs:70` | Reference oracle (F_3 only). |
| `ScalarPackedFp3Vec` (struct) | `packed/scalar.rs:199` | |
| `Packed5` (struct) | `packed/packed5.rs:208` | F_5 packed type (R1 Candidate D); feature-gated `f5`. |
| `Packed5Vec` (struct) | `packed/packed5.rs:695` | |
| `Packed5Matrix` (struct) | `packed/packed5.rs:1306` | |
| `Packed7` (struct) | `packed/packed7.rs:211` | F_7 packed type (R2 Candidate A); feature-gated `f7`. |
| `Packed7Vec` (struct) | `packed/packed7.rs:773` | |
| `Packed7Matrix` (struct) | `packed/packed7.rs:1252` | |
| `Packed7::LANES` (const) | `packed/packed7.rs:216` | `= 16`. |
| inherent methods on each of the above | various | All `pub fn` methods directly on the struct (e.g. `Bipedal3::add`, `Packed5::sub`, `Packed7::fold_mul_first_n`). |

### `gf2_algebra::permanent::*`

| Symbol | Defined in | Notes |
|---|---|---|
| `permanent_ryser<F>` | `permanent/ryser.rs:89` | Generic Ryser; W6-V2 (`0606186a`) is bounded `n ≤ 63`. |
| `permanent_mod3_reference` | `permanent/reference.rs:90` | Paper-faithful F_3 reference (T8). |
| `permanent_bipedal3` | `permanent/bipedal3.rs:158` | F_3 dispatcher (single-word + multi-word + SIMD path). |
| `permanent_bipedal3_singleword` | `permanent/bipedal3.rs:242` | F_3 single-word inner (V1, `f05ffbe1`). |
| `permanent_bipedal3_singleword_simd` | `permanent/bipedal3.rs:382` | SIMD dispatch path; not a verification target. |
| `permanent_bipedal3_multiword` | `permanent/bipedal3_multiword.rs:150` | F_3 multi-word; not in initial V1. |
| `permanent_bipedal3_parallel` | `permanent/parallel_bipedal3.rs:102` | Rayon path; not in initial V1. |
| `permanent_bipedal3_parallel_with_chunk` | `permanent/parallel_bipedal3.rs:148` | Same. |
| `permanent_bipedal5` | `permanent/bipedal5.rs:108` | F_5 dispatcher; feature-gated `f5`. |
| `permanent_bipedal5_singleword` | `permanent/bipedal5.rs:173` | V3 (`30e98ef1`) target for F_5. |
| `permanent_bipedal7` | `permanent/bipedal7.rs:110` | F_7 dispatcher; feature-gated `f7`. |
| `permanent_bipedal7_singleword` | `permanent/bipedal7.rs:163` | V3 (`30e98ef1`) target for F_7. |
| `CHUNK_SUBSETS` (const) | `permanent/parallel_bipedal3.rs:49` | Rayon tuning constant; included for completeness. |
| `N_MAX_MULTIWORD`, `L1D_BYTES`, `MAX_MATRIX_BYTES_FOR_L1`, `matrix_bytes_for_n` | `permanent/bipedal3_multiword.rs:64+` | Multi-word constants/helpers; included for completeness. |

### `gf2_algebra::gray::*`

| Symbol | Defined in | Notes |
|---|---|---|
| `gray_code_iter` | `gray.rs` | Used by every `permanent_bipedal*` impl; signature consumed by V1 + V2 + V3. |

### GPU dispatch surface

**Not yet frozen.** The W5 GPU dispatcher (`2fbbdfa5`) has not landed at the
time of this freeze. When it lands, the public surface of
`gf2_algebra::gpu::*` should be added to this freeze in a follow-up
amendment (or, alternatively, the W5 work itself can be deferred past W6 —
no W6 proof currently consumes GPU code).

## W6 consumers — what each issue proves against the frozen surface

| W6 issue | Targets the frozen symbols | Proof sketch |
|---|---|---|
| `f05ffbe1` — Lean proof, bipedal F_3 correctness per D2 sketch | `permanent::bipedal3::permanent_bipedal3_singleword` (via `Bipedal3::{add,sub,mul,neg}` underlying op formulas) | Approved sketch: `a0c0a45f` |
| `0606186a` — Lean proof, Ryser bounded n ≤ 63 per D3 sketch | `permanent::ryser::permanent_ryser` generic over `FiniteField`; bounded `n ≤ 63` | Approved sketch: `4aaa6e4d` |
| `30e98ef1` — Lean proof, F_5 / F_7 packed correctness (aspirational) | `permanent::bipedal5::permanent_bipedal5_singleword` and `permanent::bipedal7::permanent_bipedal7_singleword`; underlying `Packed5::{add,sub,mul,neg}` and `Packed7::{add,sub,mul,neg}` | New sketch required (R1 Candidate D 3-plane proof shape; R2 Candidate A LUT proof shape). If sketch proves intractable, criterion is amended per the `[aspirational]` marker. |

## Change-control protocol (post-freeze)

Once this checkpoint is approved:

1. **Breaking changes** to any symbol in the lists above require explicit
   user approval before merge.
2. **Additive changes** (new public symbols that do not change existing
   signatures) are allowed without explicit approval, but should be noted
   in the next session handoff so the next-session lead is aware.
3. **If a W6 proof consumes a changed symbol**, the corresponding W6 issue
   (`f05ffbe1`, `0606186a`, `30e98ef1`) must be re-verified against the new
   API before the breaking change merges. The proof's `lake build` must
   succeed with no `sorry` against the post-change API.
4. **Re-verification cost** is borne by the issue that introduces the
   breaking change, not by the W6 issue.

## Approval

Approved by: vesa.kaskivuo@iki.fi
Date: 2026-05-14
Mechanism: AskUserQuestion confirmation, session 8 of epic `ae82bd73`,
option "Approve as-is" of the four-option freeze sign-off menu.

The freeze is in effect from this commit onward. W6 Lean issues
(`f05ffbe1`, `0606186a`, `30e98ef1`) may now dispatch against the
locked surface above.

## Notes

- This document is a **documentation-only freeze checkpoint**, not a CI
  enforcement. The enforcement is "lead reads this doc before approving a
  breaking change to a frozen symbol". No automated checker exists.
- The freeze can be lifted at the end of W6 (when all three Lean issues
  are `done`), at which point `8c902184` is closed and this doc is moved
  to `dev/archive/features/`.
- See JIT issue `8c902184` for the canonical statement of the freeze
  criteria; this doc is the supporting evidence.
