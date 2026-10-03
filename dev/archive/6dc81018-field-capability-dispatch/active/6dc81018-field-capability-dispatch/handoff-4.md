# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 8

**Date:** 2026-08-22T16:50Z
**Session number:** 8
**Prior handoffs:** `handoff.md` (session 4), `handoff-2.md` (session 6), `handoff-3.md` (session 7)

## Current state

- Epic: `6dc81018` — state: `in_progress`, claimed by `agent:jit-execution-lead`
- Wave: 18 of 18 — session 6 (the post-fix measured session) RUNNING DETACHED on this host
- Children: all `done` except `50b47eae` (`in_progress`, claimed `agent:worker`). `fc976a80` closed this session (DEC-M); `eb9b324c` created, worked, reviewed and closed this session (DEC-L → fix → gates → done).
- Active claims: `50b47eae` (agent:worker, from session 4), `6dc81018` (agent:jit-execution-lead)
- Open escalations: none awaiting input. DEC-L (#25 in `progress.json`), DEC-M (#26) and DEC-N (#27) all resolved this session.
- Progress file: `progress.json` beside this file (waves 17–18 appended)

**A detached session-6 pipeline is running.** Do not start builds, benches, or heavy CI on this host until it finishes, and NOTHING may commit on main after THIS handoff's commit until the driver reaches its terminal line (the timed-phase guard aborts on HEAD or porcelain movement; the candidate arm must also build and run at one revision — this handoff's commit is intended to be that revision):

- Driver PID 2036725 (PPID 1, own SID), log `/tmp/gf2-ens6/logs/continue-driver.log` (1 line at launch), terminal line `DRIVER: COMPLETE` or `DRIVER: ABORTED <reason>`.
- Pipeline scripts are byte-equivalent to the committed `dev/active/50b47eae/s5-session/` texts modulo `ens5→ens6` / `s5→s6` / `-5→-6` path swaps, verified by round-trip diff at launch.
- Phases: ref build (~30 min from 16:39:44Z) → cand build (~30 min) → per-arm + cross-arm verification (v4 C2/C3) → idle-wait + preflight → timed phase, 256 executions in one lock session (~47 min) → copy durables → committed `--compare` and `--layout-audit`. Expected complete ~18:35Z.
- Outputs land untracked at `dev/benchmarks/tuning_profiles/2026-08-22-ensemble-{reference,candidate}-arm-6.csv`, `2026-08-22-member-provenance-{ref,cand}-6.tsv`, and `dev/active/50b47eae/s6-session/`.

## What just happened (session 8, in order)

- Session-5 driver read `DRIVER: COMPLETE`. Audit PASS on all preconditions; comparison FAIL at exactly two cells: `polynomial/mul_fast/len=32` 1.058269 and `len=64` 1.050254 (set geomean 1.001087). Receipt-4's excursion cell read 1.000593 — `fc976a80`'s construction-artifact diagnosis confirmed by the arbitration.
- **Receipt-5 committed** (`2026-08-22-post-cutover-receipt-5.md`, commit `1d7a1f58`) with CSVs, TSVs, `s5-session/` record; all durables `jit doc add`-linked to `50b47eae`. §7.1 extraction correctly used the ledger row with E=0 (execution 2, φ=1 arm).
- **DEC-L** (escalation, owner): track rework, diagnose first → task `eb9b324c` created (gates cargo-ci/code-review/doc-review; DAG: `50b47eae → eb9b324c → fc976a80`).
- **Brainstorm with codex** over the user's forum-poc mailbox bus (user-directed): converged two-site diagnosis, committed as `dev/active/eb9b324c/diagnosis-brainstorm.md`. Key: excursion is 2-periodic in E (VA bit 5); candidate/reference E-parity maps to OPPOSITE absolute phases (φ differs); `not_inplace/words=1` is ~17 % bit5-sensitive in BOTH revisions (pre-existing, v4-balanced).
- **fc976a80 closed** after DEC-M: doc-review F1 found REQ-01's premise (a code/codegen cause) falsified by the diagnosis itself; owner amended REQ-01 to admit the construction-property branch; doc-review re-run PASS; cargo-ci and code-review PASS. Commit `4612d3f3`.
- **eb9b324c dispatched** (opus worker), returned: Site-A fix `dd3eee59` (one `mul_dispatch` inner dispatcher on a resolved threshold; single `tuning::active()` per call; selectors delegate to `*_resolved` helpers), E2 perf (whole-suite counters null by construction; symbol-level rules out instruction-execution cost — 7–13 cycles/call vs ≈420-cycle term), 16-member pilot: 0.997219/1.000586 at the two failing cells at matched page offsets, parity split 1.0800 → 0.9994. Honest disclosures: sensitivity relocates to `mul/len=16,32` (inside τ_cell pooled at pilot scale, outside at one phase); Karatsuba relief is placement-borne (Site B untouched, byte-identical function, only its address moved).
- Review round 1: code-review FAIL (F1 blocking dangling `@/inv/canonical-abstraction` → `convention-convergence`; F2 rustdoc opening; F3 phase-ratio labeling). Rework `619a9666` fixed all three; all gates re-run PASS at that commit; **eb9b324c closed** (`9cdfb11d`).
- **DEC-N** (owner): session 6 approved + this handoff. Pipeline authored under `/tmp/gf2-ens6/`, launched detached 16:39:44Z (driver pid 2036725 per its own log line).

## What to do next

- [ ] Read the LAST line of `/tmp/gf2-ens6/logs/continue-driver.log` (match only lines past the launch baseline of 1 line).
- [ ] `DRIVER: ABORTED` before any timed window: diagnose from the named log. A **cross-arm** verification failure (v4 C3) goes to the OWNER — fact of the revision pair, no in-session remedy. Other pre-timed failures: fix and relaunch `bash /tmp/gf2-ens6/continue-driver.sh` detached (logs dir already exists). Aborted mid-timed-phase: partial record stands as taken — preserve, do not re-run, escalate to owner.
- [ ] `DRIVER: COMPLETE`: read `dev/active/50b47eae/s6-session/comparison.txt` and `layout-audit.txt`.
- [ ] Author `dev/benchmarks/tuning_profiles/2026-08-22-post-cutover-receipt-6.md` in receipt-5's register. Cover: v4/DEC-K governing text + DEC-N dispatch; K=128 E-only; per-arm φ (`phi-{ref,cand}.txt` — the FIXED candidate's base moved to `0x1e840`, so read the realized φ from the ledger, do not assume session 5's); cross-arm verification output; revision ruling — candidate binaries build AND run at this handoff's commit (verify `head=` in `build-cand.log` equals the run-time `git_revision` and state it; if anything committed in between, apply the escalation-#18 empty-build-input-diff treatment); §7.1 extraction — select the candidate ordinary build by the ledger row with E=0, execution = that row's j+1; run `--self-check`/`--list-cells` from the staged ordinary members (s5 deviation: post-hoc is acceptable, record it); read `mul/len=16` and `mul/len=32` with receipt-5's care (pilot flagged relocated phase sensitivity there), and the Karatsuba cells.
- [ ] Commit CSVs, TSVs, receipt, session record; `jit doc add 50b47eae` each durable.
- [ ] Verdict PASS + audit PASS → `50b47eae` REQ-01 met, REQ-02 met (receipts 1–5 preserved; fc976a80 + eb9b324c tracked and closed). Evaluate 50b47eae gates (Tier 1.5 prior findings: F1 "no passing receipt" — answered by receipt-6 — and F2 rework chain complete: fc976a80 AND eb9b324c both closed). Close `50b47eae`.
- [ ] Then Section 10: reconcile `surfaced_pitfalls` — the REQ-05 entry ("open": no wave child delivers the proof-obligation factoring; 1ac74567's sketch + 34d85cb9's spike + DEC-A narrowing may or may not satisfy it — re-read the epic's REQ-05 as amended by DEC-A and either map it to those children's artifacts or create a remediation task / escalate) and the 34d85cb9 advisory entry ("accepted residue", confirm the owner decision covers it). Then `jit gate evaluate-all 6dc81018` (repo-validate, doc-review, holistic-review), completion report per `references/completion-report-template.md`, close epic, archive progress + handoffs per documentation config, `jit doc add` the completion report at its final path.
- [ ] Verdict FAIL routing: audit-precondition failure → v1 §6.6 as written (precision/half-split at this K → owner per v4 C4; coverage/decorrelation → owner, tracked axes amendment). Attributable comparison FAIL → owner (all excursion decisions are theirs after DEC-L). Receipts preserved either way.
- [ ] After the epic closes: reclaim `control-0c072d73` LAST (only after the final receipt is committed); remove `/tmp/gf2-ens5` (KEEP until then — its staged binaries are receipt-5/eb9b324c evidence, hash-pinned in ledgers) and `/tmp/gf2-ens6` only after everything durable is committed.

## Traps — do not repeat these

- **Do NOT wait on gate re-runs by grepping `jit gate status-all` for "last run" lines** — they match the PREVIOUS run at the old commit and the loop exits immediately. Count `Commit: <new-sha>` lines instead (this session's wait loop did exactly this wrong once).
- **Do NOT let a worker use `git stash` while lead-owned uncommitted state (.jit events, progress.json) is in the tree.** The eb9b324c worker stashed/popped the lead's in-flight gate events to build a HEAD comparison; nothing was lost, but the failure mode is silent. Put "no stash; use a scratch copy" in dispatch prompts when the lead holds dirty state.
- **Do NOT treat whole-suite perf counters as evidence for any per-cell claim under this harness.** With a fixed `--target-ms` budget per cell, a slower cell simply makes fewer calls; run totals barely move (measured: 50.20 G vs 49.69 G cycles across a 7 % per-cell swing). They cannot support OR refute a mechanism. Symbol-level sampling only, and symbols are shared across cells — state the tier of every claim.
- **Do NOT read E-parity as an absolute fetch-block phase.** φ differs per arm (ref 0, cand 1 in session 5), so "E even" is bit5=0 in one arm and bit5=1 in the other. Label every phase ratio explicitly as `bit5=0 over bit5=1` (code-review F3 was exactly this ambiguity).
- **Do NOT trust `pgrep -f` to find a freshly launched detached driver** — it can match the launching shell wrapper. Read the driver's own `start pid=` log line and verify detachment with `ps -o pid,ppid,sid` (PPID 1, own SID).
- **Do NOT `cd` into a subdirectory inside a compound command and keep using repo-relative paths** — the second half of this session's script-verification command broke exactly so. Re-anchor absolute paths (standing trap from handoff-2, re-confirmed).
- The nonexistent-invariant citation class: `@/inv/canonical-abstraction` does not exist; the registry id is `convention-convergence` and the repo's citation form is `@/inv/convention-convergence`. Resolve every `@/…` address with `jit item show` before committing prose that cites it (code-review F1).
- All traps from `handoff-3.md` remain in force, notably: append-only driver logs (match only lines past the launch baseline); select ordinary builds by ledger E=0, never by execution index; never compare arms whose realized placement distributions differ (v4 C3 guards it); sub-32-byte placement control is unrealizable on this workspace; create the logs dir before a detached nohup launch; no commits on main during a timed phase; benchmark-mode work never goes to codex; closed-issue records under `dev/active/` are amended additively only; no bare `===` echo separators in zsh; process-liveness polling strings never inside Bash heredocs.

## Open questions needing invoker input

None. DEC-L, DEC-M and DEC-N settled this session's decision points; session 6's verdict routing is predeclared (What to do next). The next likely decision points: (a) an attributable FAIL in session 6 → owner; (b) the REQ-05 pitfall reconciliation at Section 10 if the sketch+spike artifacts are judged not to satisfy the criterion as amended by DEC-A → owner.

## Reference artefacts

- Epic: `jit issue show 6dc81018`; open child: `50b47eae`
- Session-6 pipeline: `/tmp/gf2-ens6/` (driver PID 2036725); after completion also `dev/active/50b47eae/s6-session/`
- Governing chain: plan v1 → verdict v1 → v2 (DEC-I) → v3 (DEC-J) → v4 (DEC-K), all under `dev/benchmarks/tuning_profiles/`; DEC-L/M/N in `progress.json` escalations #25–#27
- Receipts (preserved): pre-cutover baseline; post-cutover 1–3 (FAIL, unattributable), 4 (FAIL, attributable, construction — fc976a80), 5 (FAIL, attributable, code — fixed by eb9b324c)
- Fix + diagnosis: `dev/active/eb9b324c/` (findings.md, diagnosis-brainstorm.md, perf/, pilot/); commits `dd3eee59`, `619a9666`
- fc976a80 record: `dev/active/fc976a80/findings.md` (REQ-01 as amended by DEC-M)
- Session-5 record: `dev/active/50b47eae/s5-session/`; staged binaries `/tmp/gf2-ens5/` (KEEP until epic close)
- Progress: `progress.json` beside this file
- Out-of-epic follow-ons under `86b9c719`: `a6636671`, `4dd5372a`, `99c92597`, `389aa4de`, `76665001`
