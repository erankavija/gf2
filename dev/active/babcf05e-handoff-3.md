# Handoff — gf2-core PPC-spiral performance sweep (babcf05e) — session 3

**Date:** 2026-04-27
**Session number:** 3
**Prior handoffs:** `dev/active/babcf05e-handoff.md` (s1), `dev/active/babcf05e-handoff-2.md` (s2)

## Current state

- Epic: `babcf05e` — state: **in_progress**, claimed by `agent:project-lead`
- Wave 2 (S0) **DONE** — story `166b2691` closed at `b04659d` after `bd00d76a` retrospective asm audit landed (commit `3f124da` close, `9b44963` R4 fmt + LTO conclusion alignment)
- Wave 3 (Tier-A) **prepped, not dispatched** — all 13 Tier-A/B/C/D tasks now carry `ppc-kernel:<id>` labels and the `criterion-1.5x` gate (commit `e1a8f0e`)
- Active claims: none
- Open escalations: none
- Progress file: `dev/active/babcf05e-progress.json` (needs current_wave bump on next session entry — see below)

### Issue map at handoff

| Issue | Wave | State | Notes |
|---|---|---|---|
| `c7791a20` | 1 | done | session 1 |
| `2a0ffb18` | 2a | done | s1 |
| `c3a9a4cb` | 2b | done | s1 |
| `1d230525` | S0 | done | s2 R1 |
| `941d1528` | S0 | done | s2 (cargo-ci stub guard) |
| `4f845881` | 2c | done | s2 R2 |
| `b2ecd2ff` | 2d | done | s2 R1 |
| `bd00d76a` | S0 | done | **s3 R4** — retrospective asm audit; LTO opacity now empirically confirmed for all 5 dispatch tables via `examples/lto_opacity_audit.rs` harness; bug `e555c46d` filed for sse4.1 register-spill defect |
| `166b2691` (S0 story) | 2 | done | **s3** — closed after bd00d76a; 2/2 gates passed |
| `c7e91dfd` | infra | done | **s3** — MSRV 1.80 → 1.95 bump (rustup re-installed, 24 clippy 1.95 lints fixed, 30+ docs updated/banner-archived) |
| `c69d2055` | 3 (A2) | ready | label `ppc-kernel:A2` + `criterion-1.5x` gate added s3. **Description still embeds the falsified ThinLTO inlining claim** (per c7791a20 measurement: LogicalFns is opaque to LTO regardless of mode). User chose "dispatch as-is, accept reviewer interpretation" rather than amend. |
| `5223bb04` | 3 (A2) | ready | s3 prep done |
| `8e4b189c` | 3 (A1) | ready | s3 prep done |
| `cad241e6` | 3 (A3) | ready | s3 prep done; carries `asm-artefact-present` gate (already had); requires Lean4 build (`./scripts/verify-lean.sh`) |
| `1c1c4242` | 4 (B1) | ready | s3 prep done |
| `54a0e75c` | 4 (B2) | ready | s3 prep done |
| `19bc3199` | 4 (B3) | backlog | depends on `1c1c4242` (B1) |
| `ec286cee` | 5 (C1) | ready | s3 prep done |
| `7c954fb5` | 5 (C2) | ready | s3 prep done |
| `86c09a51` | 5 (C3) | ready | s3 prep done |
| `3168d114` | 5 (C4) | ready | s3 prep done |
| `33d3f5b7` | 5 (C5) | backlog | depends on `3168d114` (C4) |
| `f1a896f0` | 6 (D1) | ready | s3 prep done |
| `cbf576d1` | 6 (D2) | ready | s3 prep done |
| `1f899836` | 7 (E) | backlog | rayon multithread, no kernel label |
| `10ba5d08` | 7 (E) | backlog | rayon multithread, no kernel label |
| `e555c46d` | follow-up bug | ready | s3 NEW — sse4.1 missing on pclmulqdq wrappers, register spill via out-of-line `_mm_extract_epi64`. Tier-A/B-worthy actionable finding from the asm audit. |

## What just happened (session 3)

**Closed:**
- `bd00d76a` — retrospective asm audit. 5 module asm artefacts + audit report + bug `e555c46d`. **Built dedicated `examples/lto_opacity_audit.rs` harness** (s3 expansion) to dump direct asm for `Fp65537Fns` and `ClmulWide256Fns` callsites which had no public lib symbol — turning all 5 dispatch tables into empirically-confirmed (was 3 confirmed + 2 by structural identity). Required 4 review cycles; reviewer narrowing-interpretations pattern observed (see Traps).
- `166b2691` (S0 story) — all 4 hard criteria verified in tree, 2/2 gates passed.
- `c7e91dfd` — **NEW infra issue this session** — MSRV bumped 1.80 → 1.95 across 4 Cargo.tomls + CLAUDE.md. Reinstalled rustup officially (was running on a stub that silently no-op'd). Fixed 24 clippy 1.95 lints (manual `is_multiple_of`, explicit `into_iter`, `sort_by` → `sort_by_key+Reverse`). 30+ doc references updated; archival/handoff docs got historical-record banners. Wired as dependency of `166b2691` on user request.
- `e555c46d` — **NEW follow-up bug** filed from asm audit findings. Sse4.1 missing on `pclmulqdq` wrappers; `_mm_extract_epi64` spills out-of-line in 4 functions. Single root-cause fix.

**Wave 3-6 prep (commit `e1a8f0e`):**
- 13 `ppc-kernel:<id>` labels applied (A1=8e4b189c, A2=c69d2055+5223bb04, A3=cad241e6, B1-B3, C1-C5, D1-D2)
- 13 `criterion-1.5x` gates added to the same issues
- The gate consumes the label via `scripts/criterion-1.5x.sh` to look up `bench_target` + `size` leaves in `dev/benchmarks/ppc-baselines.json`

**Pre-dispatch criterion audit of Wave-3 Tier-A:** No blockers. All file paths exist (one minor: `c69d2055` mentions `alg/inverse.rs` as one of 3 e.g. alternatives — file does not exist but `alg/m4rm.rs::build_gray_table_flat` and `alg/rref.rs` do, so any of the 3 satisfies the criterion). Baselines are pinned: A1 `matrix_vector` ✓, A2 `matmul` ✓, A3 `fp_specialized + fieldvec_dot_product` ✓.

## What's next

### Immediate (Wave 3 dispatch — Tier A, 4 tasks)

**Conflict matrix:**
- `5223bb04` and `8e4b189c` both touch `crates/gf2-core/src/matrix.rs` → **MUST serialize** (or use worktrees)
- `c69d2055` primary edits in `crates/gf2-core/src/alg/m4rm.rs`, secondary possible in `matrix.rs` → potential conflict with the matrix.rs pair
- `cad241e6` touches `crates/gf2-core/src/gfp/simd_ops.rs` + `crates/gf2-kernels-simd/...` → no conflict with the others

**Recommended sub-wave plan:**
- **3a:** dispatch `cad241e6` + `c69d2055` in parallel (different files, low conflict). Worktree mode optional.
- **3b:** after 3a closes, dispatch `8e4b189c` (matvec).
- **3c:** after 3b closes, dispatch `5223bb04` (mul non-M4RM path).

Alternative: serialize all 4. Simpler, slower.

### After Wave 3
- Wave 4 (Tier B): `1c1c4242` (B1 transpose) → unblocks `19bc3199` (B3)
- Wave 5 (Tier C): `ec286cee`, `7c954fb5`, `86c09a51`, `3168d114` parallel; `33d3f5b7` after C4
- Wave 6 (Tier D): `f1a896f0`, `cbf576d1` parallel
- Wave 7 (Tier E): rayon multithread, gated on Wave 4/5 cache-miss-rate evidence

### Pending (deferred from session 3)
- **Task #6 (Option A asm-artefact-present output improvement)** — user requested better gate output: list of covered kernels + artefact location, concise. Non-blocking. Implementation is a small edit to `scripts/asm-artefact-present.sh` to print the per-module asm-txt file paths it verified.
- **Bug `e555c46d`** — sse4.1 register-spill on pclmulqdq wrappers. Ready to dispatch independently.

## Traps — do not repeat these

Inherited traps from prior handoffs (still in force unless resolved here):
- Mock-the-database trap (s1) — *not relevant to this epic, kept for context*
- Constant-amendment trap (s2) — saved as `feedback_pre_dispatch_criterion_audit.md`. Applied this session: criterion audit run before dispatch on Wave 3.
- Cargo stub silent-pass trap (s2) — fixed by `941d1528`'s `ensure_real_cargo()` guard. **Inherited risk:** if rustup is reinstalled or PATH manipulated, re-verify by checking a `cargo-ci` run records ~7s real time, not <500ms.

**New traps this session:**

1. **Reviewer narrowing-interpretations pattern.** During `bd00d76a`'s 5 review cycles + `c7e91dfd`'s 5 review cycles, the code-reviewer kept escalating findings in narrower scope each round (e.g., R3 PASS-with-note → R4 still flagging the note as primary fail). User's diagnostic: "Is the reviewer complaining about historical dev documents?" — yes. Workaround: write the audit report once with all evidence inline, then if a reviewer cycle returns a "note" rather than a hard fail, **ask the user whether to amend the doc or accept the verdict** rather than running another rework loop. The user's verdict overrides the reviewer.

2. **`alg/inverse.rs` does not exist.** `c69d2055`'s description names it as one of 3 e.g. alternatives for the "additional caller" criterion. The other 2 (`alg/rref.rs`, `alg/m4rm.rs::build_gray_table_flat`) exist, so the criterion is satisfiable, but a literal worker may try to find/edit `alg/inverse.rs` and fail. **Mitigation in dispatch prompt:** explicitly point the worker at `alg/m4rm.rs::build_gray_table_flat` (which has the clearest hot-loop pattern of the 3) as the preferred caller.

3. **`c69d2055` description embeds a falsified ThinLTO inlining claim.** Per c7791a20 measurement (commit `1a2f6f9`), `LogicalFns` is opaque to LTO regardless of mode. User chose "dispatch as-is, accept reviewer interpretation" — so do NOT amend the description, but **do not let the worker rely on the falsified premise** when planning. The hoist-out-of-hot-path approach the task ultimately mandates is correct independently of why; it works because it eliminates the per-call OnceLock probe + branch, not because LTO would have inlined anything.

4. **fmt diff on long expression chains.** `examples/lto_opacity_audit.rs` failed R3 cargo-ci on a single fmt diff (one-line `Vec<Fp<65537>>` map+collect needed wrapping). Always run `cargo fmt --all` before committing freshly-written examples or test files; rustfmt's threshold is ~100 chars and is easy to miss.

5. **Gate-pass MCP timeout at 600s.** `mcp__jit__jit_gate_pass` for `code-review` on `166b2691` timed out once at 10 min, then succeeded on retry. The reviewer occasionally takes 8+ minutes. Retry is the correct response — the gate runner is idempotent.

6. **Worker MUST NOT run shell scripts directly.** User clarified: "You should never have to run the shell scripts themselves." Always go through `mcp__jit__jit_gate_pass`; never `./scripts/ai-review.sh` or `./scripts/cargo-ci.sh` from the lead's bash.

## Hardware / environment / MSRV

- **MSRV: 1.95** (bumped this session via `c7e91dfd`). Was 1.80 in s1/s2.
- Host: AMD Ryzen 9 5900X (Zen 3). Has AVX2, PCLMUL, VPCLMULQDQ, VAES, SHA-NI; **no AVX-512**.
- `~/.cargo/bin/cargo` now points to a real cargo 1.95.0 (rustup 1.29.0 reinstalled this session).
- All gate runs from `b402f6c` forward record real ~7s `cargo-ci` durations.

## Branch state at handoff

- Branch: `main`
- Recent commits (this session):
  - `3f124da` chore(jit:bd00d76a): close — retrospective asm audit done
  - `9b44963` fix(jit:bd00d76a): R4 — fmt + reconcile LTO conclusion
  - `b04659d` chore(jit:166b2691): close — S0 measurement infrastructure done
  - `e1a8f0e` chore(jit:babcf05e): wave 3-6 prep — ppc-kernel labels + criterion-1.5x gates
- Working tree on handoff write: `scripts/code-review-prompt.md` carries an unstaged reorganization (3 rules moved up + 2 new doc bullets) — **review and either commit or revert before next session**. It was modified earlier in s3 outside the bd00d76a/166b2691 close scope.
- `cargo fmt --all -- --check`: clean (verified at handoff write time on the committed state)
- `cargo nextest run --workspace --all-features --release --profile ci`: not run on handoff commit; gate history shows `cargo-ci` PASS at `3f124da` and earlier in session.

## Continuation pointers for session 4

1. **Resume by reading `babcf05e-progress.json`** then this handoff. Update the progress file's `current_wave` to 3 and rewrite the wave-2 entry to reflect closure.
2. **Decide on the unstaged `scripts/code-review-prompt.md` edit** — commit, revert, or split into a separate JIT issue (it's a reviewer-policy change, arguably needs its own issue).
3. **Dispatch sub-wave 3a:** `cad241e6` + `c69d2055`. Use the dispatch prompt template at `.claude/skills/jit-parallel/references/agent-prompt-template.md`. Include the trap-2 mitigation (point c69d2055 worker at `m4rm.rs::build_gray_table_flat` as preferred secondary caller).
4. After 3a closes, dispatch 3b (`8e4b189c`), then 3c (`5223bb04`).
5. **Optional:** improve `scripts/asm-artefact-present.sh` output per Option A (Task #6 from session 3 plan).
6. **Optional:** dispatch `e555c46d` (the sse4.1 follow-up bug) — independent of Wave 3, single root-cause fix.

## Rework cycles this session (telemetry)

| Issue | Initial | Rework #1 | Rework #2 | Rework #3 | Rework #4 | Final |
|---|---|---|---|---|---|---|
| `bd00d76a` | (multi-commit R0) | a45333a+692fd07 | 36e5888 | 5cee4d5 (LTO harness) | 9b44963 (fmt+reconcile) | DONE |
| `c7e91dfd` | (R0 MSRV bump) | (R1 clippy 1.95) | (R2 doc drift) | (R3 historical banners) | (R4 audit doc deferred-items) | DONE |
| `166b2691` | (gate-only) | — | — | — | — | DONE |

Total commits in s3: ~15 across the 3 closures plus prep. Each issue's final commit is in the branch state list above.
