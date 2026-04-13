# Epic d4851c3d — project-lead session handoff (v2)

**Epic:** `d4851c3d` — Implement QAM modulation with soft-decision demapping
**Session end:** 2026-04-14
**Outgoing project lead:** agent:project-lead (Claude Opus 4.6)
**Progress snapshot:** `dev/active/d4851c3d-progress.json` (structured) + this doc (strategic)

## Where we are

**Waves 1–6 executed.** 11 of 33 leaf/story issues closed, 2 stories/tasks deferred for structural gate reasons (`24144d1a` and `bf865220`), 1 task rescoped with user approval (`c007875b`). The epic DAG now has waves 7–11 still to dispatch.

### Closed cleanly (waves 1–6)

| ID | Title | Rework | Commit |
|---|---|---|---|
| `c87c5043` | Constellation + labeling data model | 1 | `c72fbab` |
| `d36ae697` | Batched map/demap traits | 1 | `6a2009b` |
| `3e3fe377` | Modem builders + validation | 1 | `61a3dbb` |
| `625f5e1b` | Scalar `GrayQamMapper` *(rescoped)* | 3 | `437471d` |
| `b2c9c0f0` | Arbitrary-constellation mapper | 2 | `516fb53` |
| `abf03b13` | Reference soft demapper | 4 | `99689b5` |
| `ee556fbf` | AWGN link adapter | 1 | `5ee1f36` |
| `c007875b` | Per-bit-channel analysis metadata *(rescoped)* | 2 | `f50b3a7` |
| `0aac93c6` | Reference-model tests | 1 | `9e73d7a` |
| `db1dda70` | Fast Gray-QAM soft demapper | **5** | `715b5c2` |

### Deferred — structural (repo-holistic) code-review block

| ID | Title | State | Unblock path |
|---|---|---|---|
| `24144d1a` | Design the general modem core API (story) | `in_progress_deferred` | Clear after `5fd315c0` deletes legacy `channel.rs` / `modulation.rs` / `fading.rs` |
| `bf865220` | Integrate modem composition into `SimulationRunner` | `in_progress_deferred` | Same — reviewer wants `BpskAwgnChannel` removed and `QpskRicianChannelModel` / `bin/sim_runner.rs` migrated. That is 5fd315c0's scope. |

Both cleared every success criterion and non-structural gate. The auto code-review script is repo-holistic and will not pass until the duplicated legacy modem surface is gone — exactly `5fd315c0`'s deliverable.

Stories `92186a40` and `46ffe45a` will hit the same wall when their code-review gates run; plan to gate them after `5fd315c0`.

## Hard-won lessons (extensions of v1)

1. **Rework costs compound with issue complexity.** `db1dda70` took **5 rounds**. Each round found a new valid issue (zero-gain NaN, production-path dup, I-axis validation hole, Q-axis mirror, per-label mapping hole, BPSK branch hole, stale docs). Budget multiple rework passes for anything touching the modem hot path or with many hidden invariants.

2. **Auto code-review escalates scope over time.** On a single issue the reviewer tightened from "the MSB-first helper is duplicated" to "the adapter has wrong Eb/N0 scaling" to "the map/noise/demap pipeline is duplicated" to "validation is duplicated" to "BPSK constructor assumptions unvalidated." Each finding was real. Implication: dispatch with a generous prompt listing known pitfalls (label extraction, Eb/N0, noise conventions, validation, geometry) and prefer small batches.

3. **Structural (repo-wide) SSOT blocks are predictable.** `24144d1a` was deferred in v1 for exactly the pattern that hit `bf865220` in v2: the reviewer flags legacy `channel.rs`/`modulation.rs`/`fading.rs` as live duplication of the new modem surface. Any task whose scope *claims* to migrate those surfaces will fail code-review until `5fd315c0` lands. Pre-flag these tasks in the dispatch plan and retry the gate after Wave 9.

4. **Separate commits per JIT issue.** User preference established 2026-04-14 mid-Wave-6 rework. All commits after that point have a single `jit:<short-id>` scope. Earlier multi-issue commits exist in history (e.g. `fix(jit:abf03b13,ee556fbf)`) — leave them alone, just don't make new ones.

5. **Agent dispatch works well for Wave 6-style parallel tasks** when file ownership is disjoint (new file + one `mod.rs` edit + tests-only + simulation.rs were fine). Agents still need explicit guidance on: shared `bit_at_msb_first` / `unpack_label_msb_first` helpers, shared `validate_demap_input` / `subset_log_map_llr` in `modem/demapper.rs`, and the repo-wide SSOT expectation.

6. **Gate calls can time out.** `jit gate pass <id> code-review` has a 10-min internal timeout; on busy runs it returns TIMEOUT but the gate still completes. Use `jit gate check-all` afterwards to see the real verdict.

## Shared helpers landed this session (consume, don't re-derive)

| Helper | Module | Purpose |
|---|---|---|
| `bit_at_msb_first(label, bit_idx, m)` | `modem::bit_pack` (crate-private) | Single-bit MSB-first label extraction |
| `unpack_label_msb_first(label, m)` | `modem::bit_pack` (`#[doc(hidden)] pub`) | Label → `Vec<bool>` for tests |
| `pack_label_msb_first(&[bool])` | `modem::bit_pack` (crate-private) | Pack bits → `u16` label |
| `check_batch_lengths(name, m, ..)` | `modem::bit_pack` (crate-private) | Batch mapper length preconditions |
| `validate_demap_input(name, view, input, out_len)` | `modem::demapper` (`pub(crate)`) | Demapper input validation |
| `subset_log_map_llr(distances, label_fn, n, m, b, method)` | `modem::demapper` (`pub(crate)`) | Min-shifted log-sum-exp reduction |
| `run_awgn_modem_pipeline(...)` | `modem::awgn_link` (private) | Map/noise/demap AWGN pipeline |
| `brute_force_log_map_llr(points, labels, m, y_i, y_q, h_i, h_q, n0, b)` | `modem::test_oracle` (`#[doc(hidden)] pub`) | Test-only brute-force oracle |
| `ModemChannelAdapter<M, D>` | `modem::awgn_link` | `ChannelModel`-shaped modem path |
| `ChannelModel::batch_alignment() -> usize` | `simulation` (trait default = 1) | Alignment-aware simulation batching |

## Dependency DAG (waves 7–11, current wave = 7)

```
Wave 7  c5cee991 ── SIMD Gray-QAM batch kernels         (dep: db1dda70 ✓)
        51334873* ── Arbitrary-constellation story      (dep: 0aac93c6 ✓, 24144d1a†)
        a23646dd ── Rician fading adapter               (dep: bf865220 deferred)
        0cafa5f5 ── BPSK compat surface                 (dep: bf865220 deferred)

Wave 8  52112411* ── Gray-QAM fast-path story           (dep: 24144d1a†, c5cee991)
        a9ccb8ae ── Per-bit LLR distribution tools      (dep: c007875b ✓, 51334873)
        b3bb774a ── QPSK replacement                    (dep: db1dda70 ✓, a23646dd)
        92186a40* ── Simulation/channel refactor story  (dep: a23646dd, 51334873)
        71c19c32 ── GPU demapper prototype              (dep: c5cee991, bf865220 deferred)

Wave 9  9c37ec8c ── GPU crossover doc                   (dep: 71c19c32)
        0f7a6cd9 ── Per-bit MI/GMI estimators           (dep: a9ccb8ae)
        5fd315c0 ── Delete duplicated modem impls       (dep: 0cafa5f5, b3bb774a)
          ↑ unblocks 24144d1a, 92186a40, 46ffe45a, bf865220 code-review
        80f218ca ── Analysis integration                (dep: c007875b ✓, 92186a40)

Wave 10 19069bc1* ── GPU story                          (dep: 9c37ec8c, 92186a40)
        f80407f8 ── Modem docs + examples               (dep: 5fd315c0)
        1663515c ── Generic vs fast-path benches        (dep: 5fd315c0, c5cee991)
        dafb938a ── Regression + property tests         (dep: 5fd315c0, 0aac93c6 ✓)
        46ffe45a* ── Legacy surface migration story     (dep: 5fd315c0, 52112411, 92186a40)
        448491d5 ── Zero-overhead bench                 (dep: c5cee991, 0f7a6cd9, 80f218ca)

Wave 11 e2c0f65a* ── Bit-channel analysis story         (dep: 448491d5, 52112411)
        0884289e* ── Ergonomics/benchmarks story        (dep: 46ffe45a, f80407f8, 1663515c, dafb938a)
```

**Notice:** Wave 7 has two tasks (`a23646dd`, `0cafa5f5`) whose only declared dep is `bf865220`, which is deferred. Two options: (a) treat `bf865220`'s functional completion as "done enough for dependents" and dispatch the dependents now, since the structural block is independent; (b) wait for `5fd315c0` in Wave 9 to land, which will retroactively clear `bf865220`. **Recommendation: (a).** The dependents consume `ModemChannelAdapter` and `run_uncoded_ber_with_channel`, both of which are shipped and functional. Either dispatch them now with a note that their code-review gate will also structurally block until Wave 9, or dispatch them after `5fd315c0` clears.

## Resume checklist for the next session

1. `git status` (expect clean). `git log --oneline -15` (sanity; `a89847a` should be HEAD).
2. `cargo test --workspace --all-features --release` (expect ≥ 2710 passed).
3. `cat dev/active/d4851c3d-progress.json | jq '.waves[] | {wave_number, issues: [.issues[] | {short_id, status}]}'` to confirm wave state; current_wave should be 7.
4. Decide on the `bf865220`-dep question (see "Notice" above). If dispatching Wave 7 now, flag `a23646dd` and `0cafa5f5` as structurally-blocked-at-code-review.
5. Claim and dispatch Wave 7 with parallel guidance. `c5cee991` touches `gf2-kernels-simd` so it may need more careful review than the other tasks.
6. Continue through Wave 11. Remember to:
   - retry gate on `bf865220`, `24144d1a`, `92186a40`, `46ffe45a` after `5fd315c0` lands.
   - separate commits per JIT issue (user preference from 2026-04-14).
7. Final step: mark epic `d4851c3d` done, write completion report per `.claude/skills/project-lead/references/completion-report-template.md`.

## Reference: state of `in_progress_deferred` issues

- `24144d1a` (story): gates tdd-reminder/cargo-ci/doc-review green; code-review blocks on legacy surface duplication.
- `bf865220` (task): gates tdd-reminder/cargo-ci/doc-review green; code-review blocks on `BpskAwgnChannel` + `QpskRicianChannelModel` + `bin/sim_runner.rs` still being present/live. Closed on commit `1c8352a` for the issue-scoped work.
