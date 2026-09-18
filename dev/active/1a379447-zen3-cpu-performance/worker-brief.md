# Worker brief for epic 1a379447 measurement and optimization tasks

Every worker dispatched on a child of epic 1a379447 (Zen 3 CPU performance) follows this brief in addition to its issue text. It states the conventions the epic's reviewers enforce; each line traces to a gate failure or an invoker ruling recorded in the epic's progress file.

## Contract and protocol

- Read `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md`, `dev/active/f547c394/protocol.md` (protocol version 4, with `amendment-v4.md`) and `dev/active/f547c394/addendum.schema.json` before freezing anything. Every numeric setting comes from the frozen addendum and the protocol's shared settings; a launcher adds none.
- Freeze before trial: the family addendum is committed before the campaign launches; a confirmation addendum is derived from the committed pilot receipt with `dev/active/c7113c5a/survey/freeze-confirmation.py` (the canonical freezer: pilot pinned by path and SHA-256, resolution = the pilot's widest relative bootstrap half-width at the confirmation's corrected alpha, margins strictly above 1 + resolution, derivation record beside the addendum). Extend the freezer at its source when a family needs more; never write a private variant. Pass `--family-description` so the confirmation's family prose describes the confirmatory stage: a copied pilot sentence ("this addendum is the pilot: every cell is exploratory") inside a frozen confirmation is a research-review finding.
- P-20 admits at most six confirmatory cells on a family's first attempt (`10000 * alpha_c / 2 >= 20`); recompute from `dev/tools/tuning-campaign-support/src/receipt.rs` and `trial_ledger.rs` for the family's ledger rather than trusting the number. A family whose ledger history admits no confirmatory cell records that as a preserved outcome with the arithmetic and runs no non-confirmatory "confirmation".
- The family ledger is append-only. Crashed, interrupted and failed confirmations spend their reservation. The one exception is the voided-attempt rule (protocol.md, ledger section): an attempt the executor aborts for a procedural defect in its own freeze or launch, before reading any result, is voided with its stage preserved beside the published outcomes and a committed attempt record; its reservation does not enter the chain. Never edit a ledger by hand for any other reason.
- Smoke every arm before queuing a timed campaign through a non-timed harness smoke that drives the arms with the runner's own `transport` encoder, framing and child environment, pins its request mirror against the runner's request in a test, and records zero timing windows (invoker ruling, 2026-09-18). Code reading does not establish the wire contract. A smoke that drives `benchmark-ab-runner run` executes timed pilot windows and is a timed run: code-review blocks it outside the benchmark window. Precedents: `logical-campaign smoke` under `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/harness` and `qc-arm-smoke` under `dev/active/f63a2464/survey/arms`.
- Provenance freeze (invoker ruling, 2026-09-17): protocol v4, the receipt schema and the addendum schema are final for this epic. A provenance gap found in review is reported to the lead as a follow-up outside the epic; only a gap that an epic REQ names becomes epic work. No new protocol, schema or checker versions.
- A campaign is verified from its own execution log, never from a job's exit code: `cell-start` = `cell-complete` = the addendum's cell count, every cell at its declared pairs with status `measured`, a terminal `complete` record, and the acceptance summary's verdict.

## Timed runs

- Invoker policy (2026-09-13): the host is never locked during a working session. No worker launches a timed campaign, profile session or any run that takes `dev/scripts/ccx1-bench-flock.sh`; the wrapper refuses to run outside a benchmark window and `scripts/cargo-budget.sh` takes no lock there. Every measurement is a queue line in `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv` (`issue<TAB>repo-relative worktree<TAB>est_minutes<TAB>command`) committed on the worker's branch; the launcher must export `~/.cargo/bin` on PATH itself (the window unit has no login shell) and must resume an interrupted campaign under its own identity rather than start a new one. The worker's report lists its queue lines and states which cells are measured and which wait for the window.
- Everything untimed (correctness validation, builds, tests, cargo-ci, asm, allocation counters, plan checks, smoke through the runner on a throwaway plan) runs in-session, unlocked.
- Rerun nothing that is committed. Superseded runs stay committed under their own directories.

## Evidence and prose

- Single-source prose: no measured or derived figure is copied into a report. A quantitative statement is a conclusion plus a pointer to its authoritative location (a generated table row, a receipt's acceptance summary, a ledger line). Finding counts included: "accepted (finding count on the tables' Source line)", never "accepted with zero findings".
- Every table is written by a committed generator from committed evidence and reproduces byte for byte on re-run (`git diff --exit-code` after two consecutive runs).
- Every stochastic number shown carries its sample count and interval or an explicit descriptive label; `1/(1 - share)` is an Amdahl ceiling and is labelled an estimate.
- Falsification-preserved: outcomes the evaluator records as `fail`, `not-material`, `regressed` or `not-confirmatory` are reported exactly as recorded; withdrawn or contradicted cells keep their bytes and gain a recorded contradiction.
- Code claims go into a `survey/source-evidence.json` ledger (project, commit, path, line, the verbatim line, why), not into prose line numbers.
- A measurement tool or harness emits only what it observes at run time, derives from its own record, cites by identity (commit id, path) or declares as a labelled plan constant. No typed prior figure, file count, host condition or wrapper assertion in tool source.
- Comments and rustdoc carry only what the code leaves non-obvious (AGENTS.md, correctness and test policy): the contract, an invariant, the reason behind a choice, a pointer to evidence. Shortest form wins; no commit or task history in source; a module orientation is one short paragraph. Verbose comments are a blocking code-review finding.
- Rustdoc states the mechanism actually used: name the cargo feature that enables a kernel path (`simd` is not a default feature of gf2-core) as well as the runtime capability.
- A test-only process-global override (a dispatch or lane forcing flag) is serialized with a mutex held for the whole toggle-execute-observe-restore section, following `crates/gf2-core/tests/prime_route_dispatch.rs`.
- Present tense in permanent artifacts: no previously/now narration, no dated history, no `WAITING` markers; superseded artifacts are named as superseded by structure.
- Every `cites:` label on the issue appears as `[Key]` in the report text; every external number cites a registry key.
- No deferred/TODO/follow-up marker without the owning issue named; a defect found outside the issue's scope is reported to the lead for filing.

## Repository mechanics

- Commit scopes `jit:<short-id>`, conventional subjects, first line under 72 characters; commit the evidence first, before any prose.
- Nested `Cargo.lock` files under a receipt's `inputs/` are git-ignored: `git add -f` them, then run `python3 dev/scripts/check-receipt-input-snapshots.py`.
- `./scripts/cargo-ci.sh` from the worktree root before delivery; exit 75 / QUEUED OUT is host contention (re-run); "can't find crate" is a stale seeded cache (`cargo clean -p <crate> --profile ci-test`, or remove `target/debug` and `target/ci-test`).
- Merge `main` into the branch only after the timed runs, so a confirmation measures the tree its pilot measured; the merge must touch no evidence file.
- Never write `.jit/`; never run state-mutating jit commands from a worktree (the tracker refuses them). The lead links artifacts: deliver a `path<TAB>doc_type<TAB>label` table in the scratchpad path the dispatch names.
- Never remove a directory without looking inside it; external comparator sources live under the primary checkout's `.agents/ext/<issue>` and are shared.
- Final report under 3500 characters; longer reports are dropped whole. Report problems as problems; a contract that cannot be met is stated, not softened.
