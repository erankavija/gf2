# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 2

**Date:** 2026-09-01T08:50+03:00
**Session number:** 2
**Prior handoffs:** handoff.md (session 1)

## Current state

- Epic `ae03bcd0` claimed/assigned to agent:jit-execution-lead; container blocked until subtree terminal, as designed.
- **Wave 1 CLOSED.** All four issues done. `4e732b56` (survey) passed doc-review + research-review after three rework rounds plus lead-direct closures; merged at `bcad5063`. Its worktree `agent-4e732b56` is deliberately KEPT (untracked built baselines: AFF3CT/bchlib/M4RI/IT++ for wave-11+ perf work). Wave-1 worktrees ad6778e8/3243bc1f/8f51d6cf reclaimed to the cache pool.
- **Wave 2:** `7a3a6738` done (design at dev/active/ae03bcd0-general-bch/bch-api-design.md, linked, doc-review passed). `c3d5cea5` merged (`aa0aecad`) with cargo-ci gate passed, but code-review R1 FAILED (F1: BinaryPrimeExt accepts reducible runtime moduli without typed error; F2: no certificate-consuming constructor, so validation is not reusable) and doc-review R1 FAILED (module rustdoc promises the unexposed certificate path — same root cause as F2). Rework attempt 1 (of max 2) dispatched to the LIVE Opus teammate `worker-c3d5cea5` via SendMessage with instructions to `git merge main` first and build on 19fe9394's `prove_irreducible`.
- **Wave 3 (owner-approved early dispatch, R-13):** `19fe9394` merged (`b9311d2b`), cargo-ci gate passed, code-review gate is next in the queue. `fc2fa0b6` (branch 97535714) and `ebdbd228` (branch 1fcc1702) worker-complete, awaiting merge+gates. `0c4d84cf` (branch bec5a0be) worker-complete; its base includes ebdbd228's tip, so merge it AFTER ebdbd228. `997f0ab9` work done lead-direct (follow-ups `1642af1c` nonbinary decoding, `1a8f6acd` derived-code decoding, `b4d7a25d` GPU encoding, all orphan enhancements outside the epic path); needs claim + repo-validate gate + done now that 7a3a6738 is done.
- Waves 4-15 unchanged in progress.json.
- Peer coordination: codex lead (epic 6dc81018) is merging their 389aa4de work in a clean window I granted at main `cc081f10`; my queue resumes on their completion message. Owner policy in force: ONLY critical serialization — locks arbitrate, no courtesy holds, no calendar embargoes (progress.json coordination notes).
- The 389aa4de calibration measurement itself was executed BY ME as a facilitation (protocol had two defects I diagnosed; both corrected with peer approval; attempt 3 succeeded; full artifact report + honest host-idle attestation delivered; stages under /tmp/gf2-389aa4de-*). That thread is complete on my side.

## What to do next

- [ ] On codex's window-completion message: resume the merge queue. Order: `jit gate evaluate 19fe9394 code-review` (record+commit); merge `worktree-agent-fc2fa0b6` → merged-tree cargo-ci.sh → its cargo-ci + code-review gates; merge `worktree-agent-ebdbd228` → CI → cargo-ci + code-review; merge `worktree-agent-0c4d84cf` → CI → cargo-ci + code-review + doc-review. One branch at a time, lock + announce, gate-record commit between. Rebase/merge main into a branch first if the peer's 389 merge landed conflicts (expected none: disjoint paths).
- [ ] `worker-c3d5cea5` rework report → verify (3-row resolution table: code-review F1, F2, doc-review F1) → merge its branch delta → re-run code-review + doc-review gates → done.
- [ ] `997f0ab9`: claim (dep met), evaluate repo-validate, state done.
- [ ] After all wave-2/3 issues done: reclaim worktrees agent-ebdbd228/19fe9394/fc2fa0b6/0c4d84cf (NOT agent-4e732b56), leak-check main, advance current_wave to 4, dispatch wave 4 (33812b86, 6aac5c44, 992c6c6e easy→Luna xhigh; 769c3144 quotient-ext-runtime hard→Sol xhigh per alternation, last hard native was Opus on 0c4d84cf). Bake into 769c3144/fa85c403 prompts: R-10 note (axiom harness now enforces every law), R-04 (conway-registry tower rule), and the extension-design module map fences.
- [ ] Standing wave-11 note: `88ca7d2f` (pin baseline receipt) must land before cutover-benches (wave 12) merges; survey worktree baselines support fd9d5416/d1b4f85e/2b6968d3.
- [ ] Carry-forwards already recorded: genmatrix-perf must REPLACE the basis-vector algorithm (survey §8.1); avx2-batch-kernels checks generated code for the interleaved-decay question; perf-receipts re-establishes absolutes on the kept governor; R-08 footprint additions for later gf2-coding waves.

## Traps — do not repeat these

- **`make-receipt.py` prints to stdout.** Running it without `> receipt.md` silently leaves the committed receipt stale while the terminal shows fresh output; cost one doc-review round. Always redirect, then verify the rendered hash table against the files programmatically.
- **jit doc references pin content hashes.** Every time a linked artifact changes, the reference (and its discovered assets) goes stale and doc-review fails on it. Refresh (`jit doc remove` + `jit doc add`) EVERY linked doc as the LAST step after content is final, then run doc-review. Also: `jit doc remove` can fail silently when piped — check `jit doc list` afterward.
- **Never `git add .jit` without checking porcelain first.** Gate evaluations (yours AND the peer lead's) write .jit outside lock windows; a broad add sweeps their unstaged records (incident 692f0653). Under the lock: verify `git status --porcelain .jit` clean before your own jit ops, commit immediately after, never `commit -am`.
- **Measure only from committed revisions.** Research-review rejects receipts whose recorded revision does not contain the measured source (R3-F1 forced a full re-measure). Commit harness edits BEFORE measuring; record `git rev-parse HEAD` of the tree the binaries were built from.
- **Benchmark cells must match the contract's declared cache state.** The W2 fresh-alloc violation (result buffers preallocated) invalidated the like-for-like ratio and forced another re-measure. Read the contract's cache-state section before touching any harness.
- **`jit issue claim` on a dep-blocked issue refuses and the error is easy to swallow** (`tail -1` ate it; workers ran unclaimed for hours). Use `jit issue assign` for blocked issues and verify assignee afterward.
- **codex exec sandbox has no network** (kodo receipt recorded a DNS failure as evidence). Route network-needing work to native agents.
- **Shell chains through wrappers do not propagate failure reliably.** `set -eu` in a Monitor zsh chain printed false success sentinels past a failed cargo-budget/jq step (fake SPAN-OK, empty artifacts). Guard EVERY step with explicit `|| { echo FAIL-<step>; exit 1; }` and verify artifact non-emptiness before declaring success. Also `cmd | tail -N && next` masks cmd's exit code — the pipe's status is tail's.
- **The peer's documented calibration protocol had two defects** (cargo bench rejects `--release`; `jq -rs` writes raw strings that `jq -r .` cannot re-read). Both corrected with explicit peer approval, disclosed for their receipt. When executing another lead's protocol verbatim, report defects and get approval — never silently deviate.
- **`pgrep -f` matches your own wrapper shell** — liveness probes and kill loops using it false-positive/spin (the harness now blocks such loops). Probe rollout-file freshness (`ls -t` + mtime) instead.
- **Forum listeners were killed twice by something external** (owner says not them; cause unknown). Relaunch on death, check `mailbox/agent_claude/new/` for queued messages after any listener gap, and never route recv output to /dev/null.
- Unresolved from session 1 and still in force: codex startup deadlock probing (rollout freshness within ~3 min), doc-review root-absolute-link rule, `jit issue claim` reassignment (release first).

## Open questions needing invoker input

None pending. (The c3d5cea5 rework is within the cap; everything else is queue work.)

## Reference artefacts

- progress.json (current, this directory); plan.md; breakdown.json; bch-api-design.md; extension-design.md.
- Survey deliverables: dev/active/4e732b56/{findings.md,workload-selection.md}, dev/bench_results/4e732b56/ (receipt + evidence receipts).
- Live teammate: `worker-c3d5cea5` (Opus, rework in flight); its worktree .agents/worktrees/agent-c3d5cea5.
- Peer: forum bus ~/Projects/forum-poc/forum.sh, FORUM_DIR=/tmp/jit-forum, agent:claude ↔ agent:codex, lock /tmp/gf2-main-leads.lock.
- 389aa4de facilitation stages: /tmp/gf2-389aa4de-20260901-{035703-2726638,035952-2735653,040229-2742533} (fail, fail, success).
