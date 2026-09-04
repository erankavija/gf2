# Rework 3f7edef1 (attempt 1 of 2) — part 2, resumed from a colleague's draft

You are dispatched to JIT issue 3f7edef1 (External oracle and standards-vector agreement). Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-3f7edef1
on branch worktree-agent-3f7edef1.

Hard rules (worktree-dispatch-protocol):
- Run every shell command from your worktree root; use only paths relative to it in tool calls (reading the ETSI streams through the tests' own `test_vectors_path()` resolver is fine; the host symlink `~/dvb_test_vectors` resolves them).
- Never `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your branch only; do not push.
- Never write `.jit/`, never `jit doc add`, never change issue state. The lead links documents after merge.
- Do the work yourself in this session: no delegation to codex or sub-agents, whatever your global instructions say.
- Keep build caches inside the worktree, never on tmpfs.

## Situation

Round 1 of this issue was merged to main (fd8bc3d9) and failed three gates: code-review (3), doc-review (1, lead-owned), research-review (5). The rework has two parts. **Part 1 is committed** on your branch (197be2e0, 35be38a9, 7f32f4fd): code-review F1 closed for N2/N3 (gf2 constructed at GUAVA's root via `RootSelection::Explicit` and compared with GUAVA's unaided `BCHCode`), F2 closed (ETSI TP04→TP05, 808/808 blocks, 2.68 s fast tier), F3 verified. **Part 2 was drafted by the previous worker and cut off by a session limit.** The lead preserved that draft as commit `d6f03947 wip(jit:3f7edef1): preserve session-8 worker WIP after the session limit`. It touches `crates/gf2-coding/src/test_support.rs`, `crates/gf2-coding/tests/bch_oracle_agreement.rs`, `dev/active/ae03bcd0-general-bch/oracle-provenance.md`, `oracle/gap_oracle.g`, `oracle/run.sh`, `oracle/sage_oracle.py`, and `dev/active/ae03bcd0-general-bch/plan.md` (Amendment 1 of the `evidence-protocol` section).

Treat the draft as a colleague's: read it critically (`git show d6f03947`), keep what is right, fix what is not, finish it. First actions in order: (1) `git merge main` (main carries the `DvbVerification2010` citekey, commit b4c201ef, and other issues' merges; expect no conflicts in your footprint); (2) read the full original rework brief and the research-review findings below; (3) `jit item show @/citation/DvbVerification2010 @/citation/SageMath2026 @/citation/GapGroup2026 @/citation/Joyner2026 @/citation/Etsi2015` and the invariants named in the original brief.

The original brief and the research-review findings are in these two files (read both in full):
  /tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/scratchpad/s8/rework-3f7edef1-r1.md
  /tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/scratchpad/s8/rework-3f7edef1-r1-research.md

## What remains (part 2)

1. **research-review F2 (oracle identity).** The draft records `GAPInfo.KernelInfo` fields, the heap, package paths, and Sage's `sage.version.version` + `sys.executable`, and `run.sh` hashes the executables and `PackageInfo.g` files into an "Oracle identity" receipt section. Verify each field name exists on this host's GAP (`gap -q -c 'Print(GAPInfo.KernelInfo,"\n"); QUIT;'`) and Sage before relying on it; the fixtures' `oracle` object and the receipt must carry every member the suite's `SAGE_IDENTITY`/`GAP_IDENTITY` lists name.
2. **research-review F3 (bounded attempt, no static threshold).** The draft replaces `NATIVE_CODE_LIMIT` with a real `CALL_WITH_CATCH(BCHCode, …)` attempt per row under `gap -T`, recording `bchcode_built`, the heap, CPU ms, and peak RSS. Keep that shape. The B4 outcome the lead observed is in the **B4 section at the end of this file**; apply the branch it names.
3. **research-review F4.** The draft deletes the timing numbers from the provenance and states tier eligibility structurally. Keep that; sweep for any remaining wall-clock figure in the provenance (`rg -n "[0-9.]+ s\b|seconds" dev/active/ae03bcd0-general-bch/oracle-provenance.md`).
4. **research-review F5.** Amendment 1 in `plan.md` predeclares the sampling rule (messages per row, the 4096 threshold, three DVB payloads, exhaustive stream comparison). Keep it to sampling facts; the provenance and the code comments cite it. The lead refreshes the plan's tracker link after merge.
5. **Citation.** Cite `[DvbVerification2010]` next to the SHA-256 table in the provenance's "Standards vectors" section and list it in the Citations section (cite by key; do not restate registry prose). Check `.jit/references.toml` for its exact key.
6. **doc-review F1 (lead-owned).** Keep `oracle-provenance.md` and `oracle-receipt.md` at their paths; the receipt is self-contained with every table cell filled.
7. **Regenerate.** After every code and script change is committed, run `dev/active/ae03bcd0-general-bch/oracle/run.sh` from a clean committed tree (the receipt's recorded revision must contain the generating sources), then commit the regenerated fixtures (`crates/gf2-coding/tests/data/bch_oracle/*.json`) and receipt. The fixtures, scripts, receipt, and provenance regenerate byte-for-byte from `run.sh`; keep that property. GAP heap: use `ORACLE_GAP_HEAP` as the B4 section directs.
8. **Sweeps** (paste raw output): the pre-commit audit block of the original brief (all findings across the three gates, one resolution-table row each); the deferred-item grep over every touched file; `rg -n "unavailable|NATIVE_CODE_LIMIT|cannot build|not committed|host-gated|remain untouched|all the suite can wire|4.16dev" dev/active/ae03bcd0-general-bch crates/gf2-coding/tests/bch_oracle_agreement.rs crates/gf2-coding/src/test_support.rs` — every match describes the state after your work or is removed; `grep -nE '\]\(/|`/dev' dev/active/ae03bcd0-general-bch/oracle-provenance.md dev/active/ae03bcd0-general-bch/oracle-receipt.md` (no root-absolute links); present tense, no dates in prose beyond the receipt's recorded fields and the amendment headings the contract prescribes.

## Verification before you report (paste outputs)

- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --test bch_oracle_agreement --cargo-profile ci-test --profile ci` with every test's wall time (ETSI streams at the default path).
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table (copy `dev/tools/tuning-profile-compose/Cargo.lock` from the main checkout first if missing; if cargo reports a stale rlib as fresh, `find crates/gf2-coding/src -name '*.rs' -exec touch {} +`).
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Resolution table (required)

One row per finding across code-review R1 (F1–F3), doc-review R1 (F1), research-review R1 (F1–F5), plus every sweep match: `| # | Gate | Round | Finding (verbatim) | Resolution (file:line or SHA at HEAD) |`.

## Commits

Conventional subjects under 72 chars: `feat|test|docs|fix(jit:3f7edef1): …`. Stage explicitly; never `commit -am`. You may squash the wip commit into properly named commits (`git reset --soft` to a known SHA on your own branch is allowed; `--hard` is not).

## Report

Reply in messages under 2500 characters each; the first carries: branch tip SHA, the resolution table, the B4 row's recorded outcome, the test wall times, the cargo-ci step table, and anything not closed with the reason. Details in a second message.

## B4 — the lead's bounded GUAVA attempt (owner decision: retry idle, then amend)

Observed on this host (AMD Ryzen 9 5900X, 64196 MiB RAM, 32 GiB swap), otherwise idle (load 0.2), 2026-09-02 15:59:28Z–16:08:45Z:

```
systemd-run --user --scope -p MemoryMax=52G timeout 1200 \
  gap -q -A -T -o 50g -c 'LoadPackage("guava");; r:=CALL_WITH_CATCH(BCHCode,[65535,1,25,GF(2)]);; ...'
```

- Result: `CALL_WITH_CATCH` returned `false`; GAP printed `Error, reached the pre-set memory limit (change it with the -o command line option)`.
- Wall clock 9 min 17 s; CPU 9 min 14 s (`Runtime()` delta 273885 ms).
- Peak resident set 54 428 680 KiB (`VmHWM`, sampled every 5 s); the systemd scope reports 52 G memory peak and 15.6 G swap peak.
- The 50 GiB heap is the largest this host can back: 48.6 GiB was available at launch and the attempt already swapped 15.6 GiB. A 40 GiB heap was tried earlier the same day (reached 25.7 GiB resident after 160 s, still growing, stopped by the lead).

**Therefore the failure branch of the owner's decision applies.** Do all of the following:

1. In `dev/active/ae03bcd0-general-bch/plan.md`, under the `evidence-protocol` heading, add a second dated amendment subsection (or extend Amendment 1 with a clearly separated B4 clause, whichever reads cleaner) that predeclares: on corpus row B4 the GAP/GUAVA oracle result is GUAVA's own `BCHCode` generator derivation (`PrimitiveUnityRoot` and the cyclotomic-coset `MinimalPolynomial` loop, i.e. the derivation `BCHCode` itself performs before it materializes a code object) together with GUAVA's cyclic-code polynomial encoding map $c(x) = m(x)\,G(x)$, and that the run's own bounded `BCHCode` attempt and its observed outcome are recorded in the fixture and receipt. Every other row requires the full code object. Name this issue in the heading. Keep it to the protocol facts.
2. `run.sh`: keep `ORACLE_GAP_HEAP` (default 4g is fine as the run's bounded attempt: the receipt then records the attempt at that heap; do not default to 50g — the fixture generation must stay reproducible on ordinary hosts). Record the lead's larger-heap observation above in the provenance's B4 paragraph as a stated observation with its numbers (heap, wall, peak RSS, error text), attributed as a lead-run bounded attempt on the receipt's host, so the provenance carries both the run's own attempt and the largest one made.
3. The suite asserts exactly the B4 shape (`a_row_without_a_guava_code_object_records_the_derivation_and_the_attempt` in the draft is the right idea): `bchcode_built == false` is admitted only on rows the amendment names (B4), never generically; on B4 the fixture carries the derivation generator, the polynomial-map codewords, `bchcode_attempt_heap`, `bchcode_attempt_cpu_ms`, `bchcode_attempt_peak_rss_kib`; every other row has `bchcode_built == true` and the full code-object comparison. Add the row-name check so a future host that happens to build B4 or fails another row is a visible change rather than silent acceptance.
4. Regenerate fixtures and receipt from a clean committed tree as the brief says; the receipt's "GUAVA code-object attempts" table then shows B4 with `no` and its numbers.
