# Epic d4851c3d — project-lead session handoff

**Epic:** `d4851c3d` — Implement QAM modulation with soft-decision demapping
**Session end:** 2026-04-13
**Outgoing project lead:** agent:project-lead (Claude Opus 4.6)
**Progress snapshot:** `dev/active/d4851c3d-progress.json` (structured) + this doc (strategic)

## Where we are

5 of 11 waves executed (15 % of issues in the epic DAG), 7 of 33 leaf/story issues closed, 1 story deferred for structural reasons, 2 implementation issues awaiting final gate passes.

### Closed cleanly

| ID | Title | Rework rounds | Commit |
|---|---|---|---|
| `c87c5043` | Define constellation and labeling data model | 1 (bounds check on `ModemView::bit_channel_id`) | `c72fbab` |
| `d36ae697` | Define batched map and demap traits | 1 (stub duplication, test naming, module docstring) | `6a2009b` |
| `3e3fe377` | Add modem builders and validation | 1 (f64 precision loss in `compute_scale`; added `ModemScalar::to_f64`) | `61a3dbb` |
| `625f5e1b` | Gray-QAM mapper *(rescoped)* | 3 (production dup, test-side dup, proptest dup) | `c7206fe` / `516fb53` / `437471d` |
| `b2c9c0f0` | Implement arbitrary-constellation mapper | 2 (shared `bit_pack` refactor) | `c7206fe` / `516fb53` |

**Scope change applied** (with user approval): `625f5e1b` was redefined from "Add Gray square-QAM presets" (which `c87c5043` already shipped) to "Add a scalar `GrayQamMapper<S>` struct implementing `BatchMapper<S>`".

### Deferred — structural gate conflict

| ID | Title | Status |
|---|---|---|
| `24144d1a` | Design the general modem core API (story) | **`in_progress`** |

**Why deferred:** The story's own success criteria are met — types, builder, batch traits are shipped and the design doc identifies the migration tasks. Three of four gates pass (`tdd-reminder`, `cargo-ci`, `doc-review`). The fourth, `code-review`, runs `./scripts/ai-review.sh` which applies a **repo-holistic no-duplication rule**: it sees legacy `channel.rs`, `modulation.rs`, `fading.rs` coexisting with the new `modem/` surface and rejects the story until they are removed.

**Unblock path:** `24144d1a` will transition to `done` after issue `5fd315c0` ("Delete duplicated modem implementations") lands in Wave 9 under story `46ffe45a`. Attempting to close `24144d1a` earlier by manually passing `code-review` does not work — `jit gate pass <id> code-review` re-runs the automated checker, which reads the live tree.

The same structural issue will block stories `92186a40` (simulation + channel refactor) and `46ffe45a` (legacy surface migration) until `5fd315c0` completes.

### Committed but not yet closed

| ID | Title | Commit | Remaining |
|---|---|---|---|
| `abf03b13` | Implement reference soft demapper | `5ee1f36` | `cargo-ci` + `code-review` + `doc-review` gates; transition to `done` |
| `ee556fbf` | Refactor the AWGN link adapter | `5ee1f36` | same |

Both landed as new files (`crates/gf2-coding/src/modem/ref_demapper.rs`, `crates/gf2-coding/src/modem/awgn_link.rs`). Both agents hit transient API 500s after emitting code but before finishing their `mod.rs` wiring; project lead completed the wiring and fixed two minor issues (unnecessary `f64.to_f64()` in tests, `clippy::too_many_arguments` on the brute-force log-MAP oracle). `cargo test -p gf2-coding --release`, `fmt --check`, `clippy -D warnings`, and `cargo test --doc` are all green at HEAD (commit `5ee1f36`).

## Open scope question — `c007875b`

Wave 5 nominally includes `c007875b` "Define bit-channel metadata and demapper analysis modes", but its deliverables look like they're already shipped in `c87c5043`:
- `BitChannelId` — shipped
- `BitChannelSemantics` (Opaque / SingleAxisPam / IAxisPam / QAxisPam) — shipped
- `DemapMethod` (ExactLogMap / MaxLog) — shipped
- `ModemCapabilities` — shipped

Only SC3 ("Normalization and noise-parameter assumptions needed for analysis are documented in the API contract") may have genuine unfilled work — specifically the per-component AWGN noise variance convention used by `DemapInput::noise_var`, which `ee556fbf` touched but which isn't called out at the trait layer.

**Recommendation:** interview the user (same pattern as the `625f5e1b` rescope interview) with three options:
1. Close as duplicate of `c87c5043` (`jit issue reject` with `resolution:duplicate`).
2. Narrow scope to a documentation-only task: add a short "Noise and normalization contract" section to `crates/gf2-coding/src/modem/mod.rs` and to the `DemapInput` doc comment, then close.
3. Redefine as something substantive like "Extend `ModemCapabilities` with analysis hints" — less likely to be what the user wants, but worth offering.

`c007875b` is currently `backlog` because its dep (`24144d1a`) is not `done`. In practice it can be handled now since the types it names exist in-tree; the dep gate just needs adjusting, or the task can wait until Wave 9 closes `24144d1a`.

## Dependency DAG (waves 5–11)

Wave numbering comes from the persisted `d4851c3d-progress.json`. `*` marks a story container; `†` means this DAG shows the listed issue as a dep, not the other way around.

```
Wave 5  abf03b13 ── committed, gates pending
        ee556fbf ── committed, gates pending
        c007875b ── scope question (see above)

Wave 6  0aac93c6 ── Add reference-model tests           (dep: abf03b13)
        db1dda70 ── Fast Gray-QAM soft demapper         (dep: 625f5e1b, abf03b13)
        bf865220 ── SimulationRunner composition        (dep: ee556fbf)

Wave 7  c5cee991 ── SIMD Gray-QAM batch kernels         (dep: db1dda70)
        51334873* ── Arbitrary-constellation story      (dep: 0aac93c6, 24144d1a†)
        a23646dd ── Rician fading adapter               (dep: bf865220)
        0cafa5f5 ── BPSK compat surface                 (dep: bf865220)

Wave 8  52112411* ── Gray-QAM fast-path story           (dep: 24144d1a†, c5cee991)
        a9ccb8ae ── Per-bit LLR distribution tools      (dep: c007875b, 51334873)
        b3bb774a ── QPSK replacement                    (dep: db1dda70, a23646dd)
        92186a40* ── Simulation/channel refactor story  (dep: a23646dd, 51334873)
        71c19c32 ── GPU demapper prototype              (dep: c5cee991, bf865220)

Wave 9  9c37ec8c ── GPU crossover doc                   (dep: 71c19c32)
        0f7a6cd9 ── Per-bit MI/GMI estimators           (dep: a9ccb8ae)
        5fd315c0 ── Delete duplicated modem impls       (dep: 0cafa5f5, b3bb774a)
          ↑ this unblocks 24144d1a, 92186a40, 46ffe45a code-review gates
        80f218ca ── Analysis integration                (dep: c007875b, 92186a40)

Wave 10 19069bc1* ── GPU story                          (dep: 9c37ec8c, 92186a40)
        f80407f8 ── Modem docs + examples               (dep: 5fd315c0)
        1663515c ── Generic vs fast-path benches        (dep: 5fd315c0, c5cee991)
        dafb938a ── Regression + property tests         (dep: 5fd315c0, 0aac93c6)
        46ffe45a* ── Legacy surface migration story     (dep: 5fd315c0, 52112411, 92186a40)
        448491d5 ── Zero-overhead bench                 (dep: c5cee991, 0f7a6cd9, 80f218ca)

Wave 11 e2c0f65a* ── Bit-channel analysis story        (dep: 448491d5, 52112411)
        0884289e* ── Ergonomics/benchmarks story       (dep: 46ffe45a, f80407f8, 1663515c, dafb938a)

Epic d4851c3d itself: closes when {0884289e, 19069bc1, e2c0f65a} are done.
```

## Hard-won lessons for the next lead

1. **Auto code-review is strict.** Every rework-round this session (8 of them) flagged real issues: missing bounds checks, duplicated logic, precision bugs, stale docs. Budget **~10 min per issue for auto-review plus one rework cycle**, and inspect the feedback carefully — it has been accurate.

2. **Single-source-of-truth is repo-wide, not just within the issue's files.** The reviewer will call out duplication between the new code and legacy code, even when the legacy code is explicitly scheduled for removal in a different issue. Three stories (`24144d1a`, `92186a40`, `46ffe45a`) will not pass code-review until `5fd315c0` lands. Plan accordingly.

3. **Scope redundancy in the original issue tree.** Issues `625f5e1b` and likely `c007875b` turned out to be mostly duplicated by work already done in `c87c5043`. Future "Add X presets" / "Define X metadata" issues under this epic should be inspected for similar overlap before dispatch.

4. **Parallel dispatch needs conflict-free files.** Dispatching two implementation agents in one wave works when each agent owns distinct files. When both agents need to edit `modem/mod.rs`, instruct them to each touch only the lines they own (one `mod ...;` + one `pub use ...;` line, alphabetized). I observed this work cleanly for `625f5e1b` + `b2c9c0f0`.

5. **Agent API errors can orphan in-flight work.** Both Wave 5 agents (`abf03b13`, `ee556fbf`) hit API 500s after emitting files but before finishing their final `mod.rs` wiring. Always `git status` + `cargo check` after agent completion — the code may be fine but the module wiring may be missing.

6. **Persisted progress survives everything.** The `dev/active/d4851c3d-progress.json` wave plan plus this handoff should be enough for a fresh session to resume without needing any part of the prior conversation.

## Resume checklist for the next session

1. `git status` (expect clean), `git log --oneline -12` (sanity).
2. `cargo test -p gf2-coding --release` (expect green).
3. `cat dev/active/d4851c3d-progress.json | jq '.waves[] | {wave_number, issues: [.issues[] | {short_id, status}]}'` to confirm wave state.
4. Re-run `code-review` gate on `abf03b13` and `ee556fbf`: `jit gate pass <id> code-review`. Wait ~5–6 min. Inspect verdict; rework if needed; else transition to `done` and update progress.
5. Handle `c007875b` per the open-scope-question section above. Use `AskUserQuestion` with the three options.
6. Claim and dispatch Wave 6 issues. Follow parallel-dispatch conflict guidance from lesson #4.
7. When you reach `5fd315c0`, close it first, then retry gate passes on the deferred stories (`24144d1a`, `92186a40`, `46ffe45a`) — the structural duplication block should clear.
8. Continue through Wave 11. Final step: mark epic `d4851c3d` done, write a completion report at `dev/active/d4851c3d-completion.md` per `.claude/skills/project-lead/references/completion-report-template.md`.
