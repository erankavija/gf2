# Handoff — gf2-core PPC-spiral performance sweep (babcf05e) — session 2

**Date:** 2026-04-27
**Session number:** 2
**Prior handoffs:** `dev/active/babcf05e-handoff.md` (session 1, 2026-04-26)

## Current state

- Epic: `babcf05e` — state: **in_progress**, claimed by `agent:project-lead`
- Wave in progress: **wave 2** (S0 measurement infrastructure) — **5 of 6 sub-tasks DONE**, only `bd00d76a` (retrospective asm audit) remains
- Children summary: **8 done** (`c7791a20`, `2a0ffb18`, `c3a9a4cb`, `4f845881`, `b2ecd2ff`, `1d230525`, `941d1528` NEW), **0 in_progress**, **14 backlog** (Tier A–E parents and tasks), **1 ready** (`bd00d76a`)
- Active claims: none (b2ecd2ff worker released; lead released after close)
- Open escalations: none
- Progress file: `dev/active/babcf05e-progress.json` (rewritten this session)
- New JIT issue created and rejected as duplicate: `986ddc44` (criterion-1.5x wrapper — folded into 4f845881 R1)

### Issue map at handoff

| Issue | Wave | State | Notes |
|---|---|---|---|
| `c7791a20` | 1 | done | session 1 |
| `2a0ffb18` | 2a | done | session 1 |
| `c3a9a4cb` | 2b | done | session 1 |
| `1d230525` | S0 | done | session 2 (R1) — regen-asm fallback hardened against pipefail+SIGPIPE + empty-match |
| `941d1528` | S0 | done | session 2 NEW — cargo-ci stub guard. CRITICAL: every cargo-ci PASS before commit `b402f6c` was a 102 ms stub no-op |
| `4f845881` | 2c | done | session 2 (R0+R1 + user-approved SC2 amendment); criterion-1.5x gate now wired via scripts/criterion-1.5x.sh wrapper + ppc-kernel:<id> label convention |
| `b2ecd2ff` | 2d | done | session 2 (R0+R1 + two user-approved amendments A1+B1); 11/13 kernels pinned to ppc-v0-2026-04-27 @ 52eaac8 |
| `986ddc44` | S0 | **rejected** | session 2 — duplicate of 4f845881 R1 |
| `bd00d76a` | S0 | ready | session 2 deferred. Heavy task — 26 entry points across 5 modules under crates/gf2-kernels-simd/src/x86/. Last S0 sub-task before story closure |
| `166b2691` (S0 story) | 2 | backlog | depends on `bd00d76a` (and 986ddc44 — already rejected). Close once bd00d76a done |
| **Tier A–E tasks** | 3–7 | backlog | All need a `ppc-kernel:<id>` label before criterion-1.5x can fire. Wave-3 prep covers labelling + amending c69d2055 description |

## What just happened

**Session-2 actions, dense bullet form:**

- **1d230525 (regen-asm fallback bug, R0+R1):** R0 (`eec992c`) fixed the literal `--out-dir` dup + 2 latent bugs (RETURN trap unbound `$tmp` under `set -u`; grep needle used unmangled symbol against Itanium-mangled `.s`). Code-review failed R0 on (a) `cargo rustc … 2>&1 | head -5` SIGPIPE under `pipefail`, (b) silent no-match success in fallback grep loop. R1 (`35b2d62`) buffered cargo output to a file + added `found` flag. R1 PASS both gates.
- **941d1528 (cargo-ci stub false-pass — DISCOVERED + FIXED MID-SESSION):** during 1d230525's review, I noticed every prior `cargo-ci` PASS recorded sub-second durations (102 ms typical) — the dev host has `~/.cargo/bin/cargo` symlinked to `~/.cargo/bin/rustup`, which is a 3-line stub script that prints `INTERCEPTED CARGO ARGS` and exits 0. So `scripts/cargo-ci.sh`'s four cargo invocations all silently exited 0 → script reported `✓ check / ✓ test / ✓ clippy / ✓ fmt: ok` → JIT recorded PASS. **Every cargo-ci PASS in session 1 was a false positive.** Genuine verification has been workers manually bypassing PATH. User confirmed: "It is critical that cargo-ci gate runs the actual cargo. It is completely useless otherwise." Fix (`b402f6c`): `ensure_real_cargo()` guard at top of `scripts/cargo-ci.sh` detects the stub and prepends `~/.rustup/toolchains/stable-*/bin` to PATH; if no real cargo can be resolved, exit 2 instead of silent rubber-stamp. All gate runs from `b402f6c` forward record real ~7s pipeline durations.
- **4f845881 (PPC compare harness + criterion-1.5x gate, R0+R1+R2):** R0 (`d610f93`) delivered `dev/benchmarks/ppc-compare.sh`, `dev/benchmarks/ppc-baselines.json` schema + 13-kernel skeleton, and `dev/benchmarks/ppc-compare.test.sh` 7/7 tests. Code-review failed R0 on (a) the gate I registered with `--checker-command "./dev/benchmarks/ppc-compare.sh"` had no `--pass-context` and no kernel-id resolution → non-invokable; (b) literal SC2 wording 'same-as-baseline = exit-0' is mathematically impossible under the script's `>=1.5x` threshold. R1 (`fc687ed`): wrapper script `scripts/criterion-1.5x.sh` reads `JIT_CONTEXT_FILE` and extracts `ppc-kernel:<id>` label → forwards to `ppc-compare.sh`; T8 slower-run test added (8/8); README + manifest comment document the label convention. Lead post-merge: `jit gate remove criterion-1.5x` then `jit gate define ... --pass-context --checker-command "./scripts/criterion-1.5x.sh"`. R1 still failed code-review on the literal SC2 contradiction. **User approved amendment Option A** (2026-04-27): SC2 wording `same-as-baseline (exit-0)` → `>=1.5x faster (exit-0) AND same-as-baseline OR slower (exit-1)`. R2 PASS both gates on commit `fc687ed`.
- **986ddc44 created+rejected:** I filed a follow-up bug for the wiring wrapper after R0, then realized the reviewer's R0 finding was the same issue, so folded it into 4f845881 R1 and rejected 986ddc44 as duplicate (per `feedback_everyones_responsibility.md`).
- **b2ecd2ff (pin criterion baselines, R0+R1+two amendments):** Initial agent dispatch returned partial progress (only saved `dense_matvec/64`) — agent kept yielding mid-bench. Took over as lead operational work. Ran 8 benches sequentially with bypassed PATH: `matrix_vector`, `matmul`, `fp_specialized`, `fieldvec_dot_product`, `gf2m_mul_strategies`, `gf2m_wide_mul`, `m4rm_components`, `m4rm_profile`. R0 (`149423f`) wrote a manifest with bench paths derived from actual criterion output. Code-review failed R0 on: (a) literal `ppc-v0-2026-04-25` baseline name vs my `ppc-v0-2026-04-27`; (b) `[hard]` "all baselines exist" contradicts the spec doc's documented exceptions (B1 transpose has no bench yet etc). **User approved amendments A1+B1** (2026-04-27): A1 update issue text to match measurement day; B1 amend "all baselines exist" → "all baselines exist for kernels whose bench exists at HEAD." R1 (`ca527db`) caught two bench files I missed: `crates/gf2-core/benches/soa_batch.rs` (QuadraticExt SoA — pinned C4) and `crates/gf2-core/benches/sparse.rs` (more specific path for D1). Repointed C4 → `soa_batch_mul_fq2_fp65537/batch_soa` and D1 → `sparse_matvec/density_0.05`. **11/13 Tier A–D kernels now pinned**; 2 stay legitimately TBD (B1 transpose, C5 cubic SoA, D2 Cuthill-McKee — bench will be added by their kernel-implementation tasks per spec doc). R2 PASS both gates (393s code-review, 6.7s cargo-ci).

## What to do next

In priority order:

- [ ] **Dispatch `bd00d76a` (retrospective asm audit).** Heavy task: 26 SIMD entry points across 5 modules under `crates/gf2-kernels-simd/src/x86/`. Worker delivers per-module `*.asm.txt` artefacts via `dev/scripts/regen-asm.sh`, an audit summary at `dev/bench_results/2026-04-27-asm-audit.md`, and follow-up bug tickets for any "suspicious" findings. Includes an LTO-opacity check across all 5 dispatch tables (`LogicalFns`, `Fp65537Fns`, `MersenneFns`, `Gf2mWideFns`, `ClmulFns`) to verify whether the c7791a20 finding (LTO-opacity) extends beyond `LogicalFns`. Expect ~30 min wall-clock for all asm regen + audit. **Worker dispatch tip from this session:** workers struggle with long-running cargo invocations because their per-turn budget runs out; consider chunking the audit into per-module sub-tasks OR doing the asm regen as lead operational work and dispatching only the audit-doc writing.
- [ ] **Close story `166b2691`** when `bd00d76a` is done. All 6 sub-tasks will then be done (`c7791a20`, `2a0ffb18`, `c3a9a4cb`, `4f845881`, `b2ecd2ff`, `bd00d76a`) plus 2 follow-ups (`1d230525`, `941d1528`) plus 1 rejected (`986ddc44`). Run cargo-ci + code-review at the story level.
- [ ] **Wave 3 prep (Tier A — Dispatch routing).** Before dispatching ANY Tier-A worker:
  - **Amend `c69d2055`'s description** to remove the falsified ThinLTO inlining claim (per session-1 traps + this session's c7791a20 amendment). New text: "Wave-1 measurement (c7791a20, commit `1a2f6f9`, see `dev/bench_results/2026-04-26-profile-release-delta.md`) empirically established that `LogicalFns` is opaque to LTO regardless of mode. The dispatch hoist itself is the lever: by binding the resolved `fn` to a local `let xor = ...`, every inner-loop call becomes a static call site that LLVM CAN specialise (the per-call `OnceLock::get()` probe disappears, runtime branch over `select_backend_for_size` is hoisted)." This requires user approval per the description-amendment policy — escalate ONCE alongside the Wave-3 dispatch with bundled `ppc-kernel:` label additions.
  - **Add `ppc-kernel:<id>` labels** to all Tier-A/B/C/D issues. Mapping (from the manifest at HEAD):
    - `8e4b189c` → `ppc-kernel:A1` (matvec popcnt)
    - `5223bb04` → `ppc-kernel:A2` (BitMatrix::mul row-XOR)
    - `cad241e6` → `ppc-kernel:A3` (Generic Fp Solinas)
    - `c69d2055` → `ppc-kernel:A2` (xor_inplace hoist — A2 is the underlying kernel; or possibly its own ID if the user prefers separate tracking)
    - `1c1c4242` → `ppc-kernel:B1`
    - `54a0e75c` → `ppc-kernel:B2`
    - `19bc3199` → `ppc-kernel:B3`
    - `ec286cee` → `ppc-kernel:C1`, `7c954fb5` → `ppc-kernel:C2`, `86c09a51` → `ppc-kernel:C3`, `3168d114` → `ppc-kernel:C4`, `33d3f5b7` → `ppc-kernel:C5`
    - `f1a896f0` → `ppc-kernel:D1`, `cbf576d1` → `ppc-kernel:D2`
  - **Add `criterion-1.5x` gate** to each Tier-A/B/C/D task (via `jit gate add <id> criterion-1.5x`). The gate will exit 3 (TBD) for kernels whose baseline is still placeholder (B1, C5, D2) until the kernel-implementation task adds the bench.
- [ ] **Wave 3 dispatch.** 4 Tier-A tasks: `c69d2055` (after description amendment), `5223bb04`, `8e4b189c`, `cad241e6`. They edit different file scopes — parallel-safe via worktrees with separate `CARGO_TARGET_DIR`. Each needs the per-issue `ppc-kernel:` label + `criterion-1.5x` gate added BEFORE worker dispatch.

## Traps — do not repeat these

**Carried forward (still binding):**

- All session-1 traps remain binding (see `dev/active/babcf05e-handoff.md` lines 53–74). The most relevant for session 3:
  - **Lead runs all `jit gate pass/fail` and state transitions.** Workers commit code + return only.
  - **Workers DO NOT run `jit gate define`.**
  - **Serialize wave dispatches by default.** CLAUDE.md forbids parallel `cargo` commands against shared `target/`.
  - **AI reviewer may run its own local benches** to verify hard claims.
  - **Reviewer "narrowing interpretations" pattern** — apply user-directive lens after 2nd cycle of cosmetic findings.
  - **`jit gate pass` CLI prints "Passed gate ..."** even when the underlying checker FAILED; ALWAYS check `mcp__jit__jit_gate_check-all` to confirm the actual `status` field.
  - **`c69d2055`'s description embeds the FALSIFIED ThinLTO inlining claim** — must be amended (with user approval) before Wave 3 dispatch.
  - **CLAUDE.md success-criterion markers + `feedback_measurements_not_guesses.md`:** `[hard]` perf criteria backed by hypothesis are amendable in-loop ONLY with explicit user approval per escalation policy.

**New from session 2:**

- **`~/.cargo/bin/cargo` is a stub on this dev host.** Symlink `~/.cargo/bin/cargo -> ~/.cargo/bin/rustup` where rustup is a 3-line bash stub: `echo "INTERCEPTED CARGO ARGS: $*" >&2; exit 0`. **All cargo invocations from the default PATH silently no-op.** scripts/cargo-ci.sh now has `ensure_real_cargo()` guard that detects this and falls back to `~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`. Workers running cargo for their own verification MUST set `PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"` (and `CARGO_HOME=/tmp/cargo_real` if cargo subcommands are needed). Source: `941d1528` discovered + fixed in session 2.
- **`jit gate pass <issue> code-review` itself runs the AI reviewer** via `scripts/ai-review.sh`, which is configured with `--pass-context` to pull issue context into the AI prompt. It is NOT self-approval. Do not add weird env prefixes (`PATH=... CARGO_HOME=...`) to `jit gate pass` invocations — the dev host will reject them as suspicious. User feedback 2026-04-27: "Why do we need to have some strange path additions in the command? jit gate pass should just work." Keep `jit gate pass` clean; environment manipulation belongs inside the gate runner script.
- **Mid-flight `[hard]` criterion amendments break flow.** This session landed three amendments (4f845881 SC2, b2ecd2ff A1, b2ecd2ff B1). User feedback 2026-04-27: "I'm a bit dissatisfied on the amount of amendments that we need to do constantly. That is somewhat breaking the flow." **Fix going forward:** `feedback_pre_dispatch_criterion_audit.md` — before dispatching ANY worker on an issue, read every `[hard]` criterion bullet and check for stale dates, hardcoded numbers, "every X" claims contradicted by spec, and references to bench/file/symbol that may not exist. Bundle all suspect criteria into ONE escalation question at dispatch time, not at review time.
- **Worker agents struggle with long-running cargo benches.** The b2ecd2ff dispatch agent kept yielding control mid-bench (turn budget exhaustion) and reported partial progress like "Still compiling. Let me wait for the monitor signal." Each turn was ~30 seconds, not enough to compile + run + save a single bench. Took over as lead operational work — used Bash with `run_in_background=true`, then `TaskOutput` to wait for completion. Future heavy-bench tasks: dispatch as lead operational work, not via Agent, OR explicitly tell the worker to use `run_in_background=true` + `TaskOutput` pattern with 600000 ms timeouts.
- **Bench inventory survey is required before pinning.** I missed `crates/gf2-core/benches/soa_batch.rs` (covers C4) and the more specific `sparse_matvec/density_*` path in `crates/gf2-core/benches/sparse.rs` (covers D1) by relying on the spec doc's bench-name list rather than `ls crates/gf2-core/benches/*.rs`. R1 caught both. **Lesson:** when pinning baselines or wiring a kernel-id gate, `ls` + `grep -l 'criterion_main!'` across the whole bench directory; do not rely on the spec doc's enumeration.
- **The criterion path is `<top-level-group>/<param>/<baseline>`, NOT `<bench-file>/<param>/<baseline>`.** Criterion stores under the `benchmark_group(name)` name, not the bench file name. `cargo bench --bench matrix_vector` produces output under `target/criterion/dense_matvec/`, `target/criterion/dense_matvec_transpose/`, etc. The `bench_target` field in `dev/benchmarks/ppc-baselines.json` is the criterion top-level group name (which can be slash-separated for nested groups like `gf2m_mul_crossover/pclmulqdq_barrett`), NOT the bench file name. Source: b2ecd2ff session-2 lessons.
- **`cargo bench --bench <name> -- --save-baseline X` with no other args runs ALL benches in the file.** The `--bench <name>` arg is the cargo target name (= bench file name), not a filter. To save baselines for only some groups, you'd need pattern-filtering args after `--`. For pinning, running the whole bench is fine.

## Open questions needing user input

- **Wave 3 dispatch**: amending `c69d2055`'s description (falsified ThinLTO claim) requires user approval per the description-amendment policy. To minimize amendment-flow churn (per `feedback_pre_dispatch_criterion_audit.md`), bundle this with the `ppc-kernel:` label additions in a single escalation at the start of session 3. Suggested ask:

  > Approving Wave 3 prep:
  > 1. Amend c69d2055's Background paragraph to remove the falsified ThinLTO claim (replace with measured-evidence language; the [hard] success criteria are unchanged).
  > 2. Add `ppc-kernel:<id>` labels to the 13 Tier A–D issues per the mapping in the progress file.
  > 3. Add `criterion-1.5x` gate to each Tier A–D issue (the gate will exit 3 (TBD) for B1/C5/D2 until those tasks add their benches).
  > Approve all three?

## Reference artefacts

- Epic: `jit issue show babcf05e`
- Progress file: `dev/active/babcf05e-progress.json` (rewritten this session)
- Spec doc: `dev/plans/gf2_core_ppc_spiral.md`
- Cross-epic handoff chain: `dev/active/bb85c68a-handoff*.md`
- New memories saved this session:
  - `~/.claude/projects/-home-vkaskivuo-Projects-gf2/memory/feedback_pre_dispatch_criterion_audit.md` (audit `[hard]` criteria BEFORE dispatch, not at review time)
- Key recent commits (chronological):
  - `b753ab1` claim 1d230525 + sub-wave-2-bug planning
  - `eec992c` 1d230525 R0 — fix --out-dir dup + 2 latent bugs
  - `b402f6c` **941d1528** R0 — cargo-ci stub guard (ensures real cargo in pipeline)
  - `35b2d62` 1d230525 R1 — pipefail SIGPIPE fix + found flag
  - `ddaae25` close 1d230525 + 941d1528
  - `04b6491` claim 4f845881
  - `d610f93` 4f845881 R0 — ppc-compare harness + manifest schema + 7/7 tests
  - `fc687ed` 4f845881 R1 — criterion-1.5x wrapper + ppc-kernel:<id> label convention + T8 (8/8)
  - `d9a603c` close 4f845881 + reject 986ddc44
  - `52eaac8` claim b2ecd2ff
  - `149423f` b2ecd2ff R0 — pin 9 baselines (saved manually after agent struggled with bench durations)
  - `ca527db` b2ecd2ff R1 — pin C4 (soa_batch) + D1 (sparse) — reviewer caught these
  - `345bec1` close b2ecd2ff (3 commits in 1 amendment cycle: R0 + R1 + R2 description amendment)
- New JIT gate registered: `criterion-1.5x` (auto, postcheck, `--pass-context`, `./scripts/criterion-1.5x.sh`) — invokable per-issue once a `ppc-kernel:<id>` label is added.
- Updated tooling artefacts:
  - `scripts/cargo-ci.sh` — `ensure_real_cargo()` guard against `~/.cargo/bin/cargo` stub
  - `scripts/criterion-1.5x.sh` — JIT-context wrapper around `dev/benchmarks/ppc-compare.sh`
  - `dev/benchmarks/ppc-compare.sh` — geomean speedup harness (8/8 tests)
  - `dev/benchmarks/criterion-1.5x.test.sh` — wrapper unit tests (6/6)
  - `dev/benchmarks/ppc-baselines.json` — 11/13 kernels pinned to ppc-v0-2026-04-27 @ 52eaac8
- Critical baseline data: 11 of 13 Tier A–D kernels have real `target/criterion/<group>/<size>/ppc-v0-2026-04-27/` saved at HEAD=ca527db. Compare harness end-to-end-tested on A1 (`speedup 1.000x at 512 and 1024 → FAIL` as expected for V0 self-comparison).
