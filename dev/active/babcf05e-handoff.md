# Handoff — gf2-core PPC-spiral performance sweep (babcf05e) — session 1

**Date:** 2026-04-26
**Session number:** 1
**Prior handoffs:** none (first session of this epic)
**Closely related epic:** `bb85c68a` (FieldMatrix linear algebra) — at session-6 95% complete in its own handoff chain (`dev/active/bb85c68a-handoff*.md`); the uncompetitiveness profile from `bb85c68a` task `a9ab0a4f` is what motivated *this* epic (see `dev/bench_results/2026-04-26-uncompetitiveness-profile.md`).

## Current state

- Epic: `babcf05e` — state: **in_progress**, claimed by `agent:project-lead`
- Wave in progress: **wave 2** (S0 measurement infrastructure) — 2 of 4 sub-tasks DONE; 2 remain
- Children summary (epic's transitive task tree): **2 done** (`c7791a20`, `2a0ffb18`, `c3a9a4cb` — actually 3 done counting c7791a20 from wave 1), **0 in_progress**, **14 backlog** (Tier A–E tasks), **1 ready** (4f845881 + b2ecd2ff in wave 2; 1d230525 follow-up bug also ready)
- Active claims: none (all wave-1+2a+2b workers released their claims after closure)
- Open escalations: none
- Progress file: `dev/active/babcf05e-progress.json`

### Issue map at handoff

| Issue | Wave | State | Notes |
|---|---|---|---|
| `c7791a20` | 1 | done | profile.release block; user-approved Option-1 amendment after [hard] 1.3× criterion was empirically falsified to 0.91× |
| `2a0ffb18` | 2a | done | Proptest scaffolding; 1 rework cycle (R0 missed `[[test]] required-features = ["simd"]` in Cargo.toml + used `format!{:?}` instead of `T: PartialEq`) |
| `c3a9a4cb` | 2b | done | ASM convention; 1 rework cycle (R0 had `-C target-cpu=""` bug in fallback + docstring/code mismatch). New JIT gate `asm-artefact-present` REGISTERED |
| `4f845881` | 2c | ready | PPC compare harness + criterion-1.5x gate registration. NOT YET DISPATCHED |
| `b2ecd2ff` | 2d | ready | Pin criterion baselines for all targeted kernels. Heavy bench-run task. NOT YET DISPATCHED |
| `166b2691` (S0 story) | 2 | backlog | Story-level closure pending all 4 sub-tasks done |
| **Tier A–E tasks** | 3–7 | backlog | Eleven leaf tasks plus three Tier-A/E parent stories (d76f6931, 211102c6, 2c866544, 01afbd6d, afc80980); all blocked on S0 completion |
| `1d230525` | follow-up | ready | NEW bug created during c3a9a4cb R1: regen-asm.sh fallback `--out-dir` duplicate flag. NOT BLOCKING; opportunistic cleanup |

## What just happened

**Session-1 actions, dense bullet form:**

- Claimed epic `babcf05e` for `agent:project-lead`. Surveyed scaffolding: 6 stories + 16 child tasks already wired by user. B1 (`1c1c4242`) and C4 (`3168d114`) found to exist transitively via B3→B1, C5→C4 — initial misread; no new tasks needed. DAG gaps: only `babcf05e → 166b2691` was missing as a direct dep, but JIT confirmed it's transitively reachable, so no edges added.
- Read `bb85c68a-handoff-6.md` for cross-epic context and trap inheritance. Preserved trap rules (parallel agent isolation, AI-reviewer narrowing-interpretations, lead-only gate transitions).
- **Wave 1 (commit `1a2f6f9` = c7791a20):** Worker added `[profile.release] { lto = "thin", codegen-units = 1 }` to workspace Cargo.toml. Empirically falsified the ≥1.3× SIMD-matmul hypothesis: measured 0.91× regression at n=1024/4096 (medians of 4 runs; thin AND fat LTO both fail). User approved **Option 1 (amend criterion)**. Description amended via `jit issue update`; cargo-ci PASS, code-review PASS, closed.
- **Wave 1 user directive saved to memory** as `feedback_measurements_not_guesses.md`: *"Our work shall be based on measurements. Not guesses."* Plus `feedback_jit_doc_links.md`: use `jit doc add` not inline `dev/...` path references in descriptions.
- **Wave 2a (commits `25a03ca` + `1a7d1b2` = 2a0ffb18):** Worker delivered shared proptest helper `crates/gf2-core/tests/simd_equiv/mod.rs` (assert_simd_matches_scalar, WORD_BOUNDARY_LENGTHS = `[0,1,63,64,65,127,128,129,255,256,257]`, unaligned_slice) + driver `tests/simd_equiv_demo.rs`. R0 review FAILed on (1) test imported `gf2_core::kernels::simd::maybe_simd` unconditionally but module is `#[cfg(feature = "simd")]`-gated (no `[[test]] required-features = ["simd"]`); (2) `format!("{:?}", _)` for state equality. R1 fixed both, gates PASS, closed.
- **Wave 2b (commits `a087951` + `3ccc023` = c3a9a4cb):** Worker delivered `dev/scripts/regen-asm.sh`, `scripts/asm-artefact-present.sh` (gate runner), `crates/gf2-kernels-simd/README.md`, sample artefact `crates/gf2-kernels-simd/src/x86/asm/avx2_xor.asm.txt` containing `vpxor`/`vxorps` mnemonics. Lead registered new gate via `jit gate define asm-artefact-present --mode auto --stage postcheck --checker-command ./scripts/asm-artefact-present.sh`. R0 review FAILed on (1) `regen-asm.sh` fallback passed `-C target-cpu=""` when `TARGET_CPU` empty; (2) docstring claimed default "native" but code/README defaulted to unset. R1 fixed both, gates PASS, closed. Out-of-scope finding (fallback `--out-dir` duplicate flag) tracked as new bug `1d230525`.

## What to do next

In priority order:

- [ ] **Dispatch sub-wave 2c (`4f845881`):** PPC compare harness + criterion-1.5x gate. Adds `dev/benchmarks/ppc-compare.sh <kernel-id>` (reads ppc-baselines.json from b2ecd2ff, runs `cargo bench --baseline ppc-v0-2026-04-25`, computes geomean speedup, exits 0 if ≥1.5×). Registers JIT gate `criterion-1.5x` (auto, postcheck, `--checker-command ./scripts/criterion-1.5x.sh` or similar). Pattern mirrors `c3a9a4cb`'s gate registration: worker delivers script + sample, lead runs `jit gate define`. **Note: 4f845881 doesn't strictly require b2ecd2ff to land first** — it just needs to know the baseline naming convention. The compare harness can be exercised with a dummy baseline name; b2ecd2ff actually populates `dev/benchmarks/ppc-baselines.json` with real entries.
- [ ] **Dispatch sub-wave 2d (`b2ecd2ff`):** Pin criterion baselines. This is the heavy task — runs `cargo bench -p gf2-core --bench <kernel-bench> -- --save-baseline ppc-v0-2026-04-26` for every Tier-A–D bench (matrix_vector, matmul, fp_specialized, fieldvec_dot_product, gf2m_mul_strategies, gf2m_wide_mul, m4rm_components, m4rm_profile, sparse), commits `dev/benchmarks/ppc-baselines.json` mapping kernels to baseline name + commit hash. Will take 15–30 min wall-clock for the bench saves. Expect cache-warm rebuild after the LTO profile change.
- [ ] **Close story `166b2691`** when all 4 sub-tasks (`c7791a20` ✓, `2a0ffb18` ✓, `c3a9a4cb` ✓, `4f845881`, `b2ecd2ff`) are done. Run cargo-ci + code-review at the story level, transition to done.
- [ ] **Plan Wave 3 (Tier A — Dispatch routing):** 4 tasks (`c69d2055`, `5223bb04`, `8e4b189c`, `cad241e6`). Dispatch in parallel via worktrees if file-conflict map allows (see traps). **CRITICAL: `c69d2055`'s description embeds the now-falsified ThinLTO inlining claim** — lead must amend it before dispatch (see traps).
- [ ] **Schedule bug `1d230525`** (regen-asm.sh fallback `--out-dir` duplicate) into a future wave or post-Wave-2 cleanup. NOT blocking.

## Traps — do not repeat these

**Carried forward from `bb85c68a-handoff-6.md` (still binding):**

- All bb85c68a session-6 traps remain in force. The most relevant for this epic:
  - **Lead runs all `jit gate pass/fail` and state transitions; workers commit code + return only.** Per memory `feedback_agents_no_gate_runs`. Workers also do NOT run `jit gate define`.
  - **`jit gate pass` MCP call has a 10-min tool-level timeout; long ai-review runs need `Bash(jit gate pass <issue> code-review, run_in_background=true, timeout=900000)`.**
  - **Serialize wave dispatches by default. CLAUDE.md forbids parallel `cargo` commands.** For independent file scopes use worktrees with separate `CARGO_TARGET_DIR`.
  - **AI reviewer may run its own local benches/tests to verify hard claims.** Worker self-reported numbers can be inverted under different settings.
  - **Reviewer "narrowing interpretations" pattern is real** — each rework cycle on a tricky issue tends to surface progressively narrower readings. When the 5th+ cycle's findings are increasingly cosmetic, apply the user-directive lens rather than chase every finding.
  - **`jit_gate_pass` on cargo-ci returns "Passed" even on fmt failure.** ALWAYS check `jit gate check-all` after a passing-message to confirm recorded gate status.

**New from session 1:**

- **`c69d2055`'s description embeds the FALSIFIED ThinLTO inlining claim.** Quote: *"After the workspace `[profile.release]` block lands (task `c7791a20`), `LogicalFns` *call sites* will inline through ThinLTO, but the per-call `OnceLock::get()` traffic and the runtime branch over `select_backend_for_size` remain."* Wave-1 measurement (commits `1a2f6f9`, `1a7d1b2`) showed `LogicalFns` is opaque to LTO regardless of mode. Before dispatching c69d2055 in Wave 3, the lead MUST amend its description to reflect measured evidence: the dispatch hoist itself IS the lever, not LTO. The c69d2055 dispatch prompt must explicitly tell the worker to re-evaluate description claims against measured baselines (the post-Wave-2 ppc-v0 baseline is the correct reference). Source: c7791a20 review Tier 2.5 sweep.
- **CLAUDE.md "Success-criterion maturity markers" matter — `[hard]` perf criteria backed by hypothesis (not measurement) are amendable in-loop.** User directive 2026-04-26 (now in `feedback_measurements_not_guesses.md`): *"We do need to get some competitive numbers, but our work shall be based on measurements. Not guesses."* When a `[hard]` criterion is falsified by data, escalate per `escalation-policy.md` entry 4 (Option 1 = amend description with measured-evidence note + cite user-approved escalation; Option 2 = revert + re-sequence; user picks). Do NOT auto-rework against a falsified hypothesis.
- **JIT doc references go via `jit doc add`, not inline path text in descriptions.** All older PPC-spiral issues (Apr-25 scaffolding) embed `Spec doc: dev/plans/gf2_core_ppc_spiral.md` inline in their descriptions. Going forward: every issue I dispatch gets `jit doc add` for relevant docs; existing inline-only references can be cleaned up opportunistically when each issue is dispatched. Memory: `feedback_jit_doc_links.md`.
- **The `simd` Cargo feature gates `gf2_core::kernels::simd`; integration tests using it MUST declare `[[test]] required-features = ["simd"]` in `crates/gf2-core/Cargo.toml`** (mirror the pattern at `Cargo.toml:145–174` for SIMD-gated benches). Inner-attribute `#![cfg(feature = "simd")]` inside the test file is the wrong pattern for this project. Source: 2a0ffb18 R0 finding.
- **`cargo-show-asm` is now installed on the dev host** (installed during c3a9a4cb R0 via `cargo install cargo-show-asm --locked`). The `dev/scripts/regen-asm.sh` primary path uses `cargo asm`. `RUSTFLAGS="-C target-cpu=native"` causes `#[target_feature(enable = "avx2")]` wrappers to inline away — the script defaults `TARGET_CPU=""` so per-symbol extraction works. README documents `TARGET_CPU=x86-64-v3` as the portable-baseline knob. Source: c3a9a4cb worker report.
- **LLVM emits `vxorps` (not `vpxor`) for `_mm256_xor_si256(a,b)` against arbitrary memory operands** due to the 1-byte-shorter VEX encoding. Both are operationally equivalent on Zen 3. The committed sample `crates/gf2-kernels-simd/src/x86/asm/avx2_xor.asm.txt` bundles the all-ones XOR pattern (`avx2_not_into`) so the literal `vpxor` mnemonic is visible. Future Tier-B kernel reviewers should accept either mnemonic. Source: c3a9a4cb worker report.
- **Tier-2.5 sweep should always grep `.jit/issues/*.json` for stale future-tense claims about the just-landed work.** Caught the c69d2055 stale-narrative on c7791a20 close. Run on every pre-close review.
- **Workers correctly declining out-of-scope rework is the desired behavior.** c3a9a4cb R1 worker found a real bug (`--out-dir` dup in fallback) but didn't fix it because the rework brief explicitly limited scope to the two reviewer-cited findings. Lead created follow-up bug `1d230525` instead. Pattern: rework prompts MUST say "Fix only the listed issues." Workers MUST flag out-of-scope discoveries in their final report.

## Open questions needing user input

None.

## Reference artefacts

- Epic: `jit issue show babcf05e`
- Progress file: `dev/active/babcf05e-progress.json` (updated this session; reflects Wave 1 + 2a + 2b done, 2c/2d remaining)
- Spec doc: `dev/plans/gf2_core_ppc_spiral.md` (attached on epic + every dispatched task)
- Motivation doc: `dev/bench_results/2026-04-26-uncompetitiveness-profile.md` (attached on c7791a20)
- Wave-1 measurement: `dev/bench_results/2026-04-26-profile-release-delta.md` (attached on c7791a20)
- Cross-epic handoff chain: `dev/active/bb85c68a-handoff*.md` (sessions 1–6 of the closely related FieldMatrix epic; trap inheritance source)
- Key new memories saved this session:
  - `~/.claude/projects/.../memory/feedback_measurements_not_guesses.md`
  - `~/.claude/projects/.../memory/feedback_jit_doc_links.md`
- Key recent commits:
  - `3ccc023` — c3a9a4cb R1: guard fallback target-cpu + align docstring
  - `a087951` — c3a9a4cb R0: asm convention + gate runner
  - `7d1b246` — sub-wave 2a close
  - `1a7d1b2` — 2a0ffb18 R1: feature-gate simd_equiv_demo + PartialEq
  - `25a03ca` — 2a0ffb18 R0: proptest helper
  - `72e437b` — b2ecd2ff state transition (backlog → ready after c7791a20 done)
  - `68f7c9f` — c7791a20 close + 2a0ffb18 claim
  - `1a2f6f9` — c7791a20: profile.release block (with empirical-falsification amendment)
  - `3d07769` — initial epic claim + wave-1 dispatch
- New JIT gate registered: `asm-artefact-present` (auto, postcheck, `./scripts/asm-artefact-present.sh`). Future SIMD-source-touching tasks (Tier B kernels) should add this gate to their gate-set via `jit gate add <id> asm-artefact-present` at dispatch time.
- New JIT bug created: `1d230525` (regen-asm.sh fallback `--out-dir` duplicate); state ready, not blocking.
