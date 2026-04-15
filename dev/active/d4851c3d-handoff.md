# Epic d4851c3d — project-lead session handoff (v4)

**Epic:** `d4851c3d` — Implement QAM modulation with soft-decision demapping
**Session end:** 2026-04-15
**Outgoing project lead:** agent:project-lead (Claude Opus 4.6)
**Progress snapshot:** `dev/active/d4851c3d-progress.json` (structured) + this doc (strategic)

## Where we are

**Waves 1–11 landed; only guardrail leaf + final story + epic close remain.**
Session started at 63 done / 0 leaves in flight. Ended at **81 done**, with
448491d5 (zero-overhead guardrail) in its retry loop and e2c0f65a still to
claim after it closes.

### Closed this session (18 issues)

| ID | Title | Rework | Notes |
|---|---|---|---|
| `71c19c32` | GPU demapper prototype | 2 | `6ab9d6a` — capability narrowing |
| `b3bb774a` | QPSK framework migration | 0 | `46fd683` |
| `5fd315c0` | **Delete duplicated modem implementations** | 1 | `d8663d5` — deleted `modulation.rs`, shrunk `channel.rs` 832→200 LoC. Unblocked 5 deferred stories. |
| `24144d1a` | Modem core API (story) | 0 | closed after legacy deletion |
| `52112411` | Gray-QAM fast-path (story) | 2 | added `ModemSpec::preferred_*` factories + CPU benches |
| `51334873` | Arb-constellation (story) | 2 | `from_spec` preserves caller metadata; LCG SSOT lift to `gf2-core` |
| `9aa0f8b7` | **Workspace LCG consolidation** (new) | 1 | `d8258e6` — promoted `gf2_core::test_rng` → `gf2_core::rng`, migrated 5 sites |
| `a9ccb8ae` | Per-bit LLR statistics | 1 | `67e1b08` + `6aadc83` MI docs/tests |
| `dafb938a` | Modem regression + property tests | 1 | Box-Muller helper extracted |
| `9c37ec8c` | GPU crossover decision | 3 | keep-experimental; scalar full-demapper bench added; doc attached via `jit doc add` |
| `f80407f8` | Modem docs + examples | 2 | 3 examples under `crates/gf2-coding/examples/` + `preferred_*` blanket impls for `Box<dyn Batch…>` |
| `1663515c` | Generic-vs-fast benches | 1 | `modem_generic_vs_fast.rs`; bench helpers in shared `bench_support.rs` |
| `0f7a6cd9` | MI/GMI estimators | 0 | GmiMethod + histogram MI, docs corrected |
| `46ffe45a` | Legacy migration (story) | 0 | formality close after 5fd315c0 |
| `92186a40` | Sim/channel refactor (story) | 0 | formality close |
| `80f218ca` | Bit-channel analysis integration | 2 | `AnalysisCapture` + alignment panic + multi-SNR aggregation test |
| `0884289e` | Ergonomics/examples/benches (story) | 1 | Gray-QAM validator SSOT refactor |
| `19069bc1` | GPU story (story) | 1 | CLAUDE.md updated: `gf2-kernels-hip` recognized as second unsafe host |

### In flight / remaining

| ID | State | Blocking |
|---|---|---|
| `448491d5` | `in_progress`, code-review retry fired on commit `e53154f` | Added modem-backed QAM bench group + MSB-first bit-position regression test in the latest fix. Resume: `jit gate check-all 448491d5 --json` when the async run completes. |
| `e2c0f65a` | `ready`, blocked on 448491d5 | Bit-channel analysis story; formality close after 448491d5 lands. Deps: `448491d5` + `52112411` ✓ |
| `d4851c3d` | **Epic**, blocked on e2c0f65a | Run `jit gate check-all d4851c3d` after e2c0f65a closes; complete per `.claude/skills/project-lead` Section 10. |

**No deferred issues remain.** Every structural defer from the v3 handoff was cleared this session.

## Hard-won lessons (extensions of v3)

1. **Use the CLI, fire gates async.** `jit gate pass <id> <key>` via Bash `run_in_background: true` is a must; the AI code-review script takes 4–7 min per call and blocks the foreground tool otherwise. Multiple concurrent background gates work fine; be disciplined about one CLI call per Bash tool use (user called this out explicitly — no `for` loops).
2. **The code-review reviewer iterates on small findings.** Expect 1–3 rework rounds per issue even when the fundamental implementation is correct. Findings are typically SSOT violations (duplicated helpers), doc/impl mismatches (especially # Panics sections), or missing-test-of-what-the-docs-claim. Address each one inline, commit, re-fire — don't dispatch a whole worktree agent for a 10-line fix.
3. **`preferred_*` factories need blanket impls for `Box<dyn Trait>`.** `ModemSpec::preferred_mapper()` returns `Box<dyn BatchMapper<S> + Send + Sync>`. For callers to pass it into `ModemChannelAdapter::new(mapper, …)` (which takes generic `M: BatchMapper<S>`), the crate now provides `impl<S, T: BatchMapper<S> + ?Sized> BatchMapper<S> for Box<T>` (same for `BatchSoftDemapper`). Any future trait with similar generic consumers should ship the blanket impl at the same time.
4. **Workspace LCG has two acceptable unsafe hosts.** `CLAUDE.md` now says unsafe lives in `gf2-kernels-simd` AND `gf2-kernels-hip` (FFI requires it; HIP crate is opt-in and excluded from default workspace). The previous text was written before the HIP crate existed. If a third accelerator lands (cuBLAS? Vulkan?), update CLAUDE.md *with* the commit that introduces it.
5. **`FastGrayQamDemapper::new_with_scalar_kernel`** is the escape hatch for host-dispatch benchmarks. Default `new()` still auto-detects AVX2; the scalar constructor exists purely so benches can measure the scalar full-demapper baseline on AVX2 hosts (for the 9c37ec8c crossover report).
6. **Decision docs must be linked via `jit doc add`.** The agent can write the markdown file, but only the project lead (or the user) runs `jit doc add <issue> <path> --doc-type decision`. The sentinel HTML comment trick (`<!-- jit: link this document to issue X -->`) is worth keeping as a reminder but is not a substitute for the CLI call.

## Shared helpers landed this session (extend the v3 list)

| Helper | Module | Purpose |
|---|---|---|
| `gf2_core::rng::Lcg` | `gf2-core` | **Workspace SSOT** deterministic LCG. Previously `gf2_coding::modem::test_oracle::Lcg`; promoted so `gf2-kernels-simd` can consume it without a dep cycle. Closed-range `[-1, 1]` / `[lo, hi]` (endpoints reachable). |
| `modem::test_oracle::{bit_stream, permutation, label_stream}` | `gf2-coding` | Free functions taking a seed; previously `Lcg::bit_stream(seed, n)` etc. |
| `ModemSpec::preferred_mapper()` / `preferred_soft_demapper()` | `gf2-coding::modem` | Routes Gray-QAM presets to fast path, falls back to reference path otherwise. Returns `Box<dyn Batch…>`. |
| `ModemSpec::is_gray_square_qam_preset()` | `gf2-coding::modem` | Non-panicking probe; both it and `assert_valid_gray_square_qam_spec` delegate to `check_gray_square_qam_spec` (Result-returning SSOT). |
| `GrayQamMapper::from_spec(spec)` | `gf2-coding::modem` | Preserves caller's validated spec (vs. `from_preset_order` which rebuilds canonical preset). |
| `FastGrayQamDemapper::new_with_scalar_kernel(spec)` | `gf2-coding::modem` | Benchmark-only: pins the scalar PAM distance kernel. |
| `AnalysisCapture` | `gf2-coding::modem` | Opt-in handle wrapping `&mut PerBitLlrStats`, passed to `SimulationRunner::run_uncoded_ber_with_analysis`. |
| `impl BatchMapper<S> for Box<dyn BatchMapper<S> + ?Sized>` | `gf2-coding::modem::mapper` | Blanket impl so `preferred_*` output is directly usable. Same for `BatchSoftDemapper`. |
| `crates/gf2-coding/benches/bench_support.rs` | bench shim | Shared `deterministic_bits` / `deterministic_rx`; included via `#[path] mod bench_support;` |
| `crates/gf2-kernels-hip/tests/gpu_bench_support.rs` | bench shim | Shared `spec_for_order` / `gen_batch` for HIP test + bench |
| `info_theory::{shannon_capacity, shannon_limit}` | `gf2-coding` | Moved out of the deleted `channel.rs` into their own module. |

## New dev artefacts

- `dev/active/9c37ec8c-gpu-crossover-decision.md` — **keep-experimental** decision, measured AVX2 + scalar full-demapper crossover tables, attached via `jit doc add` as `doc_type=decision`.

## Resume checklist for the next session

1. `git status` (expect clean). `git log --oneline -8`; HEAD should be on a 448491d5 fix commit.
2. `jit status` — target state: "83 done, 1 in progress (d4851c3d), 0 epic leaves open".
3. `jit gate check-all 448491d5 --json` — if the async retry hasn't returned yet, re-fire with `jit gate pass 448491d5 code-review --by agent:project-lead` in the background.
4. Once 448491d5 passes: `jit gate pass 448491d5 doc-review --by agent:project-lead && jit issue update 448491d5 --state done`.
5. Claim **e2c0f65a** (final story, formality close):
   - `jit gate pass e2c0f65a tdd-reminder --by agent:project-lead`
   - `jit issue claim e2c0f65a agent:claude`
   - `jit gate pass e2c0f65a cargo-ci --by agent:project-lead` (background)
   - `jit gate pass e2c0f65a code-review --by agent:project-lead` (background)
6. After e2c0f65a lands: **epic close**.
   - `jit gate check-all d4851c3d --json` — run every gate in order.
   - Produce completion report per `.claude/skills/project-lead/references/completion-report-template.md`.
   - `jit issue update d4851c3d --state done`.
7. Archive `dev/active/d4851c3d-*` artifacts per project-lead Section 10.

## Commit footprint

HEAD is `e53154f` (448491d5 fix). 247 session commits + ~30 merges. `cargo test --workspace --all-features --release` passes (2770+ tests). No unsafe code outside the two accelerator crates. All workspace LCG duplicates consolidated onto `gf2_core::rng::Lcg`.

## Reference: wave DAG state (complete)

```
Wave 8  52112411 ✓  a9ccb8ae ✓  b3bb774a ✓  92186a40 ✓  71c19c32 ✓
Wave 9  9c37ec8c ✓  0f7a6cd9 ✓  5fd315c0 ✓  80f218ca ✓  9aa0f8b7 ✓ (new)
Wave 10 19069bc1 ✓  f80407f8 ✓  1663515c ✓  dafb938a ✓  46ffe45a ✓  448491d5 🟡
Wave 11 e2c0f65a ⏳  0884289e ✓
```

Legend: ✓ done, 🟡 gates retrying, ⏳ ready to claim.
