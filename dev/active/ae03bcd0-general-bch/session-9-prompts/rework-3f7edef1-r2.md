# Rework Required — 3f7edef1 (attempt 2 of 2)

You are dispatched to JIT issue 3f7edef1 (External oracle and standards-vector agreement). Your worktree is at:
  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-3f7edef1
on branch worktree-agent-3f7edef1. Rework attempt 1 was merged to main at 93f08af2 and gated: cargo-ci PASSED (on rerun; the first run hit a transient GPU smoke-test abort in gf2-sim under host load, unrelated), code-review R2 FAILED (F1, escalated — see below), doc-review R2 FAILED (F1, closed lead-direct on main at e899f8c8), research-review R2 FAILED (F1–F5, below). **First command:** `git merge main` (main is at 388a9456 or later; expect no conflicts).

Hard rules (worktree-dispatch-protocol):
- Run every shell command from your worktree root; use only paths relative to it in tool calls (reading the lead-provided files under `/tmp/claude-1000/.../scratchpad/` by absolute path is allowed; reading the ETSI streams through the tests' own `test_vectors_path()` resolver is fine).
- Never `git checkout`, `git switch`, `git worktree add/remove`, `git stash`, `git reset --hard`. Commit on your branch only; do not push.
- Never write `.jit/`, never `jit doc add`, never change issue state.
- Do the work yourself in this session: no delegation to codex, `codex exec`, or sub-agents, whatever your global CLAUDE.md says.
- Keep build caches inside the worktree, never on tmpfs.

## Required pre-commit audit (paste raw output at the top of your report)

```bash
jit gate status 3f7edef1 code-review --all; jit gate status 3f7edef1 doc-review --all; jit gate status 3f7edef1 research-review --all
python3 -c 'import json,glob
for path in glob.glob(".jit/gate-runs/*/result.json"):
    r=json.load(open(path))
    if r.get("status")=="failed" and r.get("issue_id","").startswith("3f7edef1"):
        for f in (r.get("findings") or {}).get("findings",[]):
            print(r["gate_key"], r.get("completed_at","")[:16], "[{}] {}: {} ({}:{})".format(f.get("severity"),f.get("id"),f.get("summary"),f.get("file"),f.get("line")))'
jit doc list 3f7edef1
grep -inE '\b(deferred|todo|future work|open question|not (yet )?implemented|follow-?up|out of scope|punt(ed)?)\b' dev/active/ae03bcd0-general-bch/oracle-provenance.md dev/active/ae03bcd0-general-bch/oracle-receipt.md dev/active/ae03bcd0-general-bch/oracle/*.{sh,py,g} crates/gf2-coding/tests/bch_oracle_agreement.rs
```
(The gate-runs directory exists only in the main checkout; if the scan is empty in your worktree, the findings below are the verbatim record.)

Resolve: `jit item show @/issue/3f7edef1/requirement/REQ-01 @/issue/3f7edef1/requirement/REQ-02 @/invariant/claims-trace-to-artifacts @/invariant/runtime-observed-provenance @/invariant/standards-vector-conformance @/invariant/present-tense-prose @/invariant/single-source-prose @/invariant/behavioral-evidence-validity`.

## Review verdict — research-review R2 (2026-09-02 17:02Z), five findings

**F1 [high, blocking] — ESCALATED, do not work on it.** "B4 is still not an agreement result from the required GUAVA `BCHCode` oracle. … Amendment 2 is not a binding `## Decisions` item and conflicts with REQ-01." Code-review R2 raised the same point. The lead has escalated the B4 rule to the owner (the issue's own text must carry it); leave the B4 fixture shape, Amendment 2, and the suite's B4 handling exactly as they are. Your resolution-table row for F1 says "escalated to the owner; unchanged this round".

**F2 [medium, blocking]** — "The new 50 GiB B4 attempt is prose-only. Its wall clock, processor time, RSS, swap use, and 'largest heap the host can back' conclusion appear nowhere outside `oracle-provenance.md`; the committed receipt covers only the 4 GiB fixture run." (`oracle-provenance.md:326`.) **Closure:** commit the lead's run artifacts as a record the provenance cites. They are at `/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/scratchpad/b4-attempt-50g/` — `command.sh` (the exact invocation), `gap.out` (`built=false`, `cpu_ms=273885`), `gap.err` (GAP's memory-limit error), `rss.log` (VmRSS/VmHWM sampled every 5 s; peak VmHWM 54 428 680 KiB), `timeline.txt` (start/end UTC, free memory before), `scope.txt` (the systemd scope summary: 9 min 14 s CPU over 9 min 17 s wall, 52G memory peak, 15.6G swap peak). Copy them verbatim into `dev/active/ae03bcd0-general-bch/oracle/attempts/2026-09-02-b4-heap-50g/` and rewrite the provenance's B4 attempt table so every number it states is read from those files (cite each with a relative link; state the host, the GAP identity being the same binary the receipt hashes, and that the attempt reads no repository source). Drop any figure the artifacts do not contain (the "48.6 GiB available at launch" is in `timeline.txt`'s `free-before` line; the earlier 40 GiB probe has no artifact — state it only if you can cite one, else remove it).

**F3 [high, blocking]** — "The published claim that all 808 DVB-T2 blocks agreed bit-for-bit has no committed execution receipt. The fixture runner never runs the standards-vector test." (`oracle-provenance.md:403`.) **Closure:** `run.sh` gains a final stage that runs the standards-vector case through the budget wrapper — `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --test bch_oracle_agreement --cargo-profile ci-test --profile ci -E 'test(the_etsi_dvb_t2_streams_encode_to_their_verified_codewords)' --no-capture` (or `--success-output immediate`) — and the receipt records that stage's exact invocation, wall clock, nextest's summary line, and the test's own printed facts. Make the test print, as observed facts, the resolved stream directory, the SHA-256 of the two stream files it read, the frame and block counts, and the number of blocks that agreed, so the receipt carries them verbatim from the run. When the streams are absent, the test prints that and returns, and the receipt records "streams absent, comparison not run" rather than a number. The provenance's "Standards vectors" section then cites the receipt for the counts and hashes instead of stating them itself (`@/invariant/claims-trace-to-artifacts`). Regenerate on this host with the streams present so the committed receipt carries the 808/808 run.

**F4 [low, advisory; no-argue applies]** — "Amendment 1 calls the message counts, 4096 threshold, and DVB payload count 'predeclared,' but commit history places the amendment after the fixtures and results." (`plan.md:135`.) **Closure:** honest wording. The amendment *fixes* the sampling rule the suites use from its date forward; it does not claim to have preceded the first fixtures. Rewrite Amendment 1's lead sentence and every "predeclare(s/d)" that refers to it in `plan.md`, `test_support.rs`, `bch_oracle_agreement.rs`, and the provenance (`rg -n "predeclar" dev/active/ae03bcd0-general-bch crates/gf2-coding/src/test_support.rs crates/gf2-coding/tests/bch_oracle_agreement.rs`) to "fixes"/"records" as appropriate. Do not touch the epic-level protocol paragraphs above the amendments.

**F5 [medium, advisory; no-argue applies]** — "`attemptPeak` is sampled before the row's second `GeneratorPolCode` attempt, so B4's second allocation can first appear in N1's recorded high-water mark. The per-row memory attribution is therefore unreliable." (`gap_oracle.g:495`.) **Closure:** sample the peak after **both** attempts of a row (record `bchcode_attempt_peak_rss_kib` after the `BCHCode` attempt and a second member, e.g. `wrapper_attempts_peak_rss_kib`, after the `GeneratorPolCode` attempt, or one peak after both — pick one shape and document it), and rewrite the receipt's and provenance's sentence about row-by-row attribution so it claims only what a monotone process-wide high-water mark supports (the row's attempts did not exceed the mark; a rise between rows bounds their cost from below). Update the suite's identity/shape assertions for the new member.

**doc-review R2 F1** — closed lead-direct (e899f8c8: the example's documented command uses the budget wrapper). Keep it; sweep for any other bare `cargo` command in prose you touch.

**code-review R2 F1** — same subject as research F1; escalated; unchanged this round.

## Sweeps (paste raw results)

1. The audit block above: one resolution-table row per finding across all rounds and gates (code-review R1 F1–F3, R2 F1; doc-review R1 F1, R2 F1; research-review R1 F1–F5, R2 F1–F5).
2. Deferred-item grep (audit block) over every touched file, including the new attempts directory.
3. `rg -n "predeclar|808|largest heap|48\.6|40 GiB|25\.7" dev/active/ae03bcd0-general-bch crates/gf2-coding/tests/bch_oracle_agreement.rs crates/gf2-coding/src/test_support.rs` — every remaining match is either read from a committed artifact the sentence cites or is removed.
4. `grep -nE '\]\(/|`/dev' dev/active/ae03bcd0-general-bch/oracle-provenance.md dev/active/ae03bcd0-general-bch/oracle-receipt.md` (no root-absolute links); present tense; no dates in prose beyond receipt fields, artifact directory names, and amendment headings.

## Verification (paste outputs)

- Regenerate from a clean committed tree: `dev/active/ae03bcd0-general-bch/oracle/run.sh` (with the ETSI streams at the default path), then commit the regenerated fixtures and receipt; the receipt's recorded revision must contain every generating source including the new stage.
- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --test bch_oracle_agreement --cargo-profile ci-test --profile ci` with wall times.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table.
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Resolution table (required)

`| # | Gate | Round | Finding (verbatim) | Resolution (file:line or SHA at HEAD) |` — one row per finding across all rounds and gates plus every sweep match.

## Commits

`feat|test|docs|fix|chore(jit:3f7edef1): …`, subjects under 72 chars; stage explicitly; never `commit -am`.

## Report

Reply to the lead with SendMessage (to: "team-lead") in messages under 2500 characters each; the first carries: branch tip SHA, the resolution table, the receipt's new standards-vector stage line, the cargo-ci step table, and anything not closed with the reason. Finish with the same content as your final response.
