# Handoff — Overhaul documentation for research adoption (fa787f85) — session 4

**Date:** 2026-10-04
**Session number:** 4
**Prior handoffs:** `handoff.md` (session 1), `handoff-2.md` (session 3)

## Current state

- Epic: `fa787f85`, state backlog, assigned `agent:jit-execution-lead`.
- Wave in progress: wave 8 of 16 (the source-comment sweep under story `ffc35b8c`). Waves 5 to 7 are closed except `3fd3db5e`.
- Children summary for the subtree: 72 done, 5 in_progress, 18 ready, 34 backlog, 3 rejected.
- Active claims (`agent:worker`, no worker running): `3aef9e33`, `67b5d1d8`, `3da13f39`, `290b8716`, `554c2935`.
- Open escalations: none. Every question raised this session has an owner ruling in `progress.json` `escalations`.
- Worktrees of this epic: none. Main is clean at the handoff commit.
- Progress file: `progress.json` in this directory.

## What just happened

- Owner interview on the ten session-3 questions; rulings applied to criteria (ec7d2aec, b16d013a, ee50e270, fdb998ec, 7bac1303, fb209a81) and recorded as DEC entries. 1adfe2e9 and c84ac71a rejected (absorbed, obsolete).
- Closed with all gates passed: ee50e270, 12907582, 5a25717c, ec7d2aec, b16d013a, e3ff339a, 7bac1303, eb2a833d, 18858de1, 01c20b61, fb209a81, 629e0ff0, 62f0d0e6, 62ce5927, b0b1faf5, 67048b47.
- 7bac1303 became a named exception (`harness-path-dependencies`) after cargo 1.95 trials showed the `[patch]` form breaks `--locked` resolution; moves rewrite harness paths (f79bcba3, 198aafa3, 7039d680 amended). b0b1faf5 got the exception `evidence-directory-paths`.
- The archive of epic 6dc81018 (another session) broke `cargo-ci` through a duplicated campaign declaration; 62ce5927 fixed the lookups. It also left the migration checker failing; 96cea1b9 is filed.
- 67048b47 linked 1,202 owned artifacts through a generated script.
- Sweep: eleven units written in two waves, then reworked to a stricter standard after the first reviews. Closed: f46046f0, 9f47bb94, ee2b0c0c, 66858db0, bfb37a21, 7bfad039, 57436474. `git diff --shortstat` of each unit's commits gives the line counts.
- 554c2935 registered citation keys for the works cited in source and a prose-to-key map (`554c2935-citation-map.md`); it stays open until the sweep removes the pointers that have no key.
- Filed under tech-debt epic b4b4b9ee: the defects and planned-work items found by workers; `progress.json` `created_during_execution` lists them. High priority: 1ffa72fc (safe SIMD wrappers with debug-only shape checks), 6b5f4f3c (generic GF(2^m) kernel reduction above degree 33).
- Owner approved rewording the unpushed tip merge commit to the epic scope (now `c7707f4d4`).

## What to do next

- [ ] Re-gate 290b8716 (`code-review`, `doc-review`). Its only finding was the mis-tagged merge, now reworded; no rework is needed.
- [ ] One fix lane (one worker, one worktree, one commit per issue, merge commit tagged `jit:fa787f85`) for the last findings, each rework attempt 2 of 2:
  - 3aef9e33: test comments that narrate adjacent setup and assertions (`packed/bipedal3.rs:588`, `:605`, `:1280`, `:1585`, `packed/scalar.rs:262`, `:629`). Sweep every test module in scope.
  - 67b5d1d8: the same class at `gf2m/field.rs:2008` to `:2041`.
  - 3da13f39: `gfpn/batch.rs:8-11`, `:235-239`, `:592-596` say every non-`Fp<P>` field takes the scalar combine and call `SimdKaratsubaHook` crate-internal; the trait is public and re-exported. State the dispatch contract once, at the hook, and link it from the other two sites.
- [ ] Dispatch sweep wave C: 4997a3ed, 9dda3958, cafb463c, 8a9db61a, f22ec590, 93aed46a. Generate each brief from `ffc35b8c-sweep-brief-template.md` (placeholders `{ID}`, `{SHA}`, `{TITLE}`, `{SCOPE}`, `{RG_PATHS}`, `{CRATE}`, `{DOC_NOTE}`). Per-unit notes to put in `{DOC_NOTE}`:
  - 4997a3ed: `primitive_polys.rs` attributes 0x11D to an "IEEE AES standard"; delete the wrong attribution. The crate root doc in `lib.rs` loses "High-Performance" and its accessor-only example.
  - 9dda3958: remove the "Shoup §12.4" pointer in `charpoly.rs`; `field/ple.rs` belongs to f2064e90 (wrong arXiv id 1703.02438, correct key `DumasPernetSultan2017`).
  - cafb463c: delete the Condo et al. citation in `grand/`, cite `Yuan2025`; Solomon et al. is `Solomon2020`; the MMIX constant in `gldpc/mod.rs` cites `Knuth1997`.
- [ ] Then wave D (8f451167, f2064e90, aa0558c1, 72768e37, c6aa0e84, 13dfe1a5) and wave E (eba21481, 50201002). Remaining unkeyed citations to route: MMIX constant in `ldpc/dvb_t2/bit_interleaver.rs`, `tests/dvb_t2_bicm_chain.rs`, `gf2-core/src/rng.rs`; unnamed paper in `fading.rs` and `examples/ldpc_bler_check.rs`; "NASA/CCSDS" example cites `Ccsds2017`.
- [ ] After the sweep: regenerate `554c2935-citation-map.txt`, confirm its "citations without a registry entry" section is empty, gate 554c2935.
- [ ] Ready non-sweep tasks: 96cea1b9 (manifest reconciliation; blocks the migration checker), 37099e5b, f0dd63f1. 3fd3db5e waits on fd9d5416 (still in progress).
- [ ] Before the `ffc35b8c` story gate, reconcile `surfaced_pitfalls` (dvb_t2_matrices.rs module doc, the lost `library-first-generality` exception citation in `bch/dvb_t2/mod.rs`, `ldpc/dvb_t2/table_interpretation.md`).

## Traps — do not repeat these

Traps of `handoff.md` and `handoff-2.md` remain in force. New this session:

- **Do NOT brief a sweep unit with less than the review standard.** The first brief forbade removing Rustdoc examples and let workers reformat banners; all five units failed review (`teaching-rustdoc-examples`, `non-obvious-comments`). Use `ffc35b8c-sweep-brief-template.md`; reviewers judge the whole scope, pre-existing text included.
- **Do NOT tag a multi-issue lane merge with one of its issues.** Reviewers attribute every commit whose subject carries `jit:<id>`; 290b8716 failed on a registry edit that belonged to 554c2935. Tag lane merges `jit:fa787f85`.
- **Do NOT `git add .jit` without reading `git status --short .jit`.** Commit ab6b6b09b swept in another session's archive of 6dc81018.
- **Do NOT trust the worktree script's exit silently.** It stops at the first branch collision after creating earlier worktrees; three workers were sent to missing worktrees. Delete merged `worktree-agent-*` branches first and check `git worktree list` before dispatch.
- **Do NOT gate after an sccache server started inside a CI run.** `scripts/cargo-ci.sh` exports a scratch `TMPDIR` that the server keeps after deletion (e250b5bb). Run `sccache --stop-server; sccache --start-server` from a normal shell before a gate batch, and give workers `CARGO_CI_NO_SCCACHE=1`.
- **Do NOT resume a sweep worker for one small fix.** A resume replays 300k to 500k tokens of context. Batch every follow-up into one message, or give small findings from several units to one fresh fix-lane worker.
- **Do NOT force `cargo-ci` per issue on one tree.** One forced run followed by `jit gate evaluate-many cargo-ci <ids>` reuses the result; five forced runs cost half an hour.
- **Do NOT read "Agent produced no output" as a review verdict.** It is a reviewer-service timeout; re-run the gate.
- **Do NOT name a measured commit that lacks the measuring script, or compare a fresh run with files checked out at that commit.** 62f0d0e6 failed twice on this; the reproduction compares a run in a scratch worktree with the files of the record's commit.
- **Do NOT leave short issue ids bare in a commit body.** A reviewer could not resolve them; write `<statement> -> <id> (jit issue show <id>)`.
- **Do NOT expect a container archive to leave checks green.** It copies documents that non-terminal issues also own; identity lookups then see duplicates. Run `cargo-ci` and `migration/check.py` after any archive lands.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show fa787f85`; tree: `jit graph tree fa787f85`
- Plan: `plan.md`; brief: `fa787f85-planning-brief.md` (this directory)
- Progress: `progress.json` (escalations, surfaced_pitfalls, notes, traps)
- Sweep worker brief: `ffc35b8c-sweep-brief-template.md`; pre-sweep census: `62f0d0e6-comment-census.md`
- Citation registry: `.jit/references.toml`; map: `554c2935-citation-map.md`
- Exception records: `7bac1303-harness-path-exception.md`, `b0b1faf5-output-baseline.md`
- Host note: the DVB-T2 vector default is `$DVB_TEST_VECTORS_PATH`, else `~/dvb_test_vectors`; on this host the full tree is `/data/specs/dvb/t2/streams`, so the slow-tier vector tests need the variable set.
