# Epic d4851c3d — project-lead session handoff (v3)

**Epic:** `d4851c3d` — Implement QAM modulation with soft-decision demapping
**Session end:** 2026-04-14
**Outgoing project lead:** agent:project-lead (Claude Opus 4.6)
**Progress snapshot:** `dev/active/d4851c3d-progress.json` (structured) + this doc (strategic)

## Where we are

**Waves 1–7 executed.** 14 of 33 leaf/story issues closed, 3 stories deferred for structural gate reasons (`24144d1a`, `51334873`, to be followed by `92186a40`, `46ffe45a`), 1 task rescoped with user approval (`c007875b`). The epic DAG now has waves 8–11 still to dispatch.

### Closed cleanly in Wave 7

| ID | Title | Rework | Commit |
|---|---|---|---|
| `c5cee991` | SIMD-ready Gray-QAM batch kernels | 2 | `5c70f8d` |
| `0cafa5f5` | BPSK framework-backed compat surface | 0 | `cb8c0c7` |
| `a23646dd` | Rician fading modem adapter | 3 | `eee9344` |

### Deferred — structural (repo-holistic) code-review block

| ID | Title | State | Unblock path |
|---|---|---|---|
| `24144d1a` | Design the general modem core API (story) | `in_progress_deferred` | After `5fd315c0` deletes legacy `channel.rs` / `modulation.rs` / `fading.rs` |
| `51334873` | Arbitrary-constellation mapping + reference demapping (story) | `deferred_to_wave_9` | Same — all leaf SCs covered by done tasks, story code-review waits on `5fd315c0` |

Both stories' own success criteria are covered by completed leaf tasks. The auto code-review script is repo-holistic and will not pass until duplicated legacy modem surface is gone — exactly `5fd315c0`'s deliverable. Expect `92186a40` and `46ffe45a` to hit the same wall when their code-review gates run.

## Hard-won lessons (extensions of v2)

1. **Calibration changes are the most hidden source of bugs.** `a23646dd` took **3 rounds** on the same class of issue: (a) first flag was Es=1 vs Es=2 symbol-energy mismatch (3 dB); (b) second flag was the local variable named `sigma_squared` being semantically N0 in this file — my first fix misinterpreted the variable, producing 2x-too-quiet noise; (c) third flag was SSOT — the corrected formula duplicated one in `modem/awgn_link.rs`. **Implication:** when touching any `sigma^2`/`N0`/`Eb/N0` code, verify the variable *semantics* by tracing downstream sampling (what std does `Normal::new(0, _)` get?) and re-verify after the fix, AND look for other copies of the same formula across the codebase before landing.

2. **Extract shared helpers preemptively for Eb/N0 / N0 / σ² math.** `modem::awgn_link::{unit_energy_sigma_sq_from_eb_n0_db, unit_energy_n0_from_eb_n0_db}` are now the SSOT for noise-scale computation. Future tasks that touch noise scaling should *call* these and not redefine the formula. If a new helper becomes necessary, add it next to the existing ones.

3. **New public items require `# Arguments`, `# Examples` (tested), `# Panics`, `# Complexity` — per `CLAUDE.md`.** The c5cee991 reviewer flagged missing sections on 8 public items even though the prose was there. Budget 15–20 minutes per new pub item for the full doc set.

4. **AVX2 safe wrappers need slice-length asserts before unsafe pointer arithmetic.** The `get_unchecked_mut` + `.add()` pattern is UB-reachable from safe public API without length checks. Add `assert_eq!` on every derived length in the safe wrapper, and add `#[should_panic]` regression tests. This will be the pattern for every future SIMD kernel in `gf2-kernels-simd`.

5. **`gf2-kernels-simd` is now non-optional for `gf2-coding`.** Done as part of c5cee991's SSOT fix. The `simd` feature now only propagates to `gf2-core/simd`. Downstream consumers always see the kernel crate's scalar functions. The cfg for `x86_64` still lives inside the kernel crate itself.

6. **The shared `ChannelModel::batch_alignment()` is load-bearing.** Introduced in `bf865220`, used by `QpskRicianChannelModel::batch_alignment() -> 2` and by `SimulationRunner::run_uncoded_ber_with_channel`. Any new higher-order modulation `ChannelModel` impl must override it with `bits_per_symbol`.

## Shared helpers landed this session (extend the v2 list)

| Helper | Module | Purpose |
|---|---|---|
| `GrayPamDistanceFnsF32/F64` | `gf2_kernels_simd::modem` | Function-pointer bundle for the Gray-PAM squared-distance hot loop |
| `detect_f32()` / `detect_f64()` | `gf2_kernels_simd::modem` | Runtime-detect the best-available kernel bundle (AVX2 or scalar) |
| `scalar_pam_sq_distances_f32/f64` | `gf2_kernels_simd::modem` | Scalar backend; also the oracle for AVX2 parity tests |
| `unit_energy_sigma_sq_from_eb_n0_db(m, rate, eb_n0_db)` | `gf2_coding::modem::awgn_link` | Canonical Eb/N0 → per-component AWGN σ² for unit-energy symbols |
| `unit_energy_n0_from_eb_n0_db(m, rate, eb_n0_db)` | `gf2_coding::modem::awgn_link` | Canonical Eb/N0 → N0 (= 2σ²) |

## Dependency DAG (waves 8–11, current wave = 8)

```
Wave 8  52112411* ── Gray-QAM fast-path story           (dep: 24144d1a†, c5cee991 ✓)
        a9ccb8ae ── Per-bit LLR distribution tools      (dep: c007875b ✓, 51334873†)
        b3bb774a ── QPSK replacement                    (dep: db1dda70 ✓, a23646dd ✓)
        92186a40* ── Simulation/channel refactor story  (dep: a23646dd ✓, 51334873†)
        71c19c32 ── GPU demapper prototype              (dep: c5cee991 ✓, bf865220 ✓)

Wave 9  9c37ec8c ── GPU crossover doc                   (dep: 71c19c32)
        0f7a6cd9 ── Per-bit MI/GMI estimators           (dep: a9ccb8ae)
        5fd315c0 ── Delete duplicated modem impls       (dep: 0cafa5f5 ✓, b3bb774a)
          ↑ unblocks 24144d1a, 51334873, 92186a40, 46ffe45a code-review
        80f218ca ── Analysis integration                (dep: c007875b ✓, 92186a40)

Wave 10 19069bc1* ── GPU story                          (dep: 9c37ec8c, 92186a40)
        f80407f8 ── Modem docs + examples               (dep: 5fd315c0)
        1663515c ── Generic vs fast-path benches        (dep: 5fd315c0, c5cee991 ✓)
        dafb938a ── Regression + property tests         (dep: 5fd315c0, 0aac93c6 ✓)
        46ffe45a* ── Legacy surface migration story     (dep: 5fd315c0, 52112411, 92186a40)
        448491d5 ── Zero-overhead bench                 (dep: c5cee991 ✓, 0f7a6cd9, 80f218ca)

Wave 11 e2c0f65a* ── Bit-channel analysis story         (dep: 448491d5, 52112411)
        0884289e* ── Ergonomics/benchmarks story        (dep: 46ffe45a, f80407f8, 1663515c, dafb938a)
```

**Stars (*) mark stories.** All stories in this epic close as a formality after their leaf deps complete; several will defer on code-review until `5fd315c0` lands.

## Wave 8 dispatch plan

| Issue | Risk | File ownership | Dispatch batch |
|---|---|---|---|
| `a9ccb8ae` | Low-medium — new analysis module | `modem/analysis.rs` (new) + `modem/mod.rs` | A (alone or first) |
| `b3bb774a` | Medium — replaces placeholder QPSK path with framework | `simulation.rs` + possibly `channel.rs` | B (alone; may touch files 0cafa5f5/a23646dd also modified — coordinate) |
| `71c19c32` | High — GPU prototype, research quality not production | `gf2-kernels-hip` likely + new scaffolding | A (in parallel with a9ccb8ae — disjoint files) |
| `52112411` | Story — formality close | none | Last (after leafs) |
| `92186a40` | Story — will defer on code-review | none | Last (likely defer) |

Suggested dispatch order:
1. Dispatch `a9ccb8ae` + `71c19c32` in parallel (disjoint files: new analysis module vs new HIP scaffolding).
2. After both land, dispatch `b3bb774a` alone (touches `simulation.rs` which is shared across the epic).
3. Close the two stories (`52112411` likely clean, `92186a40` will defer).

## Resume checklist for the next session

1. `git status` (expect clean). `git log --oneline -15` (sanity; `0833146` should be HEAD).
2. `cargo test --workspace --all-features --release` (expect ≥ 2741 passed).
3. `cat dev/active/d4851c3d-progress.json | jq '.waves[] | {wave_number, issues: [.issues[] | {short_id, status}]}'` to confirm wave state; `current_wave` should be 8.
4. Claim and dispatch Wave 8 per the plan above. Budget 2–3 rework rounds per leaf task.
5. Continue through Wave 11. Remember to:
   - retry gate on `24144d1a`, `51334873`, `92186a40`, `46ffe45a` after `5fd315c0` lands.
   - separate commits per JIT issue (user preference from 2026-04-14).
   - call `unit_energy_{sigma_sq,n0}_from_eb_n0_db` from any new code that needs Eb/N0 → noise conversion.
   - never pipe shell output through `awk` (triggers permission prompts).
6. Final step: mark epic `d4851c3d` done, write completion report per `.claude/skills/project-lead/references/completion-report-template.md`.

## Reference: state of `in_progress_deferred` / `deferred_to_wave_9` issues

- `24144d1a` (story): gates tdd-reminder/cargo-ci/doc-review green; code-review blocks on legacy surface duplication.
- `51334873` (story): all leaf SCs covered (b2c9c0f0, abf03b13, 3e3fe377 done); code-review will block on legacy duplication when run.
- Both clear when `5fd315c0` lands in Wave 9.
