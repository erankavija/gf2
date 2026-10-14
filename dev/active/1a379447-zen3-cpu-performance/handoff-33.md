# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 37

**Date:** 2026-10-14
**Session number:** 37
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-32.md` in this directory

## Current state

- Epic: `1a379447` — state: backlog in the tracker (derived; work is in progress)
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 131 done, 3 in_progress, 36 backlog/ready, 26 rejected. Direct children: 27 of 31 done; open `d11a45d4`, `ec4e71ca` (in progress), `d93a1c7e`, `1362381c`; `9eb6f13c` and `17e08faa` sit under `d93a1c7e` in the reduced graph.
- Active claims (`agent:worker`): `9eb6f13c` (rework 1, branch `worktree-agent-9eb6f13c`), `d11a45d4` (branch `worktree-agent-d11a45d4`), `ec4e71ca` (branch `worktree-agent-ec4e71ca`). Each worker was told to commit, stop and send its report.
- Open escalations: none.
- Progress file: `progress.json` in this directory

## What just happened

- Done, all gates on main: `cba35553`, `4252085a`, `63bad95d` (`f8ed59d3e`, two reworks), `20af7e6e` (`8d606de6a`, four reworks), `817fc47d` (`19aedabfb`), `815703a1` (`81734321a`, two reworks). Verdicts under `reviews/`.
- `63bad95d`: launchers, freezers, the shared `dev/scripts/tuning-extent-campaign.sh`, the dense and logical harness guards, the campaign driver and validator lost their Git-status conditions (invoker ruling); the core campaign archive is committed under `dev/bench_results/63bad95d/`; profile arms use the shared SplitMix64 and `BuildIdentity::compiled`.
- `20af7e6e`: decoder campaign archive committed; `calibration/producer-rebuild/record.json` reproduces the producer digest under declared flags.
- `817fc47d`: three selectors moved under the tuning sections, five structural; fixed `fp_small_batch_dot` (wrapping `u32` drain sum, primes 131 to 251).
- `815703a1`: remaining Git-state conditions removed; `resume_equivalent` compares no source pointer; protocol snapshots of `dbd8787d` and `a83583e0` committed.
- `9eb6f13c` merged at `b6e363c40` (option B: RNG stated beside the owner); cargo-ci passes; code-review failed on the algebra fixture pin. Rework 1 in `agent-9eb6f13c`, tip `3ab3ebf82` at last check.
- Filed in the epic: `815703a1`, `9eb6f13c`, `17e08faa`, `d93a1c7e`, `d11a45d4`, `ec4e71ca`.
- `d11a45d4` REQ-01 amended (`52fba47d7`) by the invoker.
- Reclaimed `agent-63bad95d-int`, `-fam`, `-m4ri`, `-pm4ri`, `agent-817fc47d`.

## What to do next

- [ ] Read the reports of `w-815703a1` (9eb6f13c rework 1), `w-d11a45d4` and `w-ec4e71ca`, or the branch tips if a report is missing. Run `./scripts/cargo-ci.sh` in each worktree yourself with nothing else running there, then merge and gate: `9eb6f13c` (cargo-ci, code-review), `d11a45d4` and `ec4e71ca` (cargo-ci, code-review).
- [ ] `17e08faa` (runtime records defined once), then `d93a1c7e` (resume on unchanged executables); both change `tuning-campaign-support`; serialize.
- [ ] After `d93a1c7e` merges: in `agent-ed3d490e-run` run `survey/record-gf2-arms-build.py`, `record-intra-quality.py record`, `record-peak-memory.py record`, re-register the records; then dispatch `5d590bac`.
- [ ] Story `ed3d490e` remaining leaves up to `6230e0b6`; then `1362381c`; then epic completion (Section 10).
- [ ] Remove the `e430f035` exception comments in the root `Cargo.toml`.
- [ ] Reclaim `agent-0a77ecfc`, `agent-815703a1`, `agent-63bad95d-run` once nothing reads them (the real `0a77ecfc` stage lives at `/tmp/gf2-0a77ecfc-20261013t011147z-815998`).

## Traps — do not repeat these

Traps of `handoff-32.md` and earlier remain in force.

- **Do NOT wait for a worker's cargo-ci report once its run exceeds the 600 s foreground limit.** Six workers went idle without reporting after backgrounding cargo-ci. Run `./scripts/cargo-ci.sh` in the worker's worktree yourself; tell workers to stop before cargo-ci.
- **Do NOT run CI in a worktree a worker is still editing.** The lead's run at `e2e7e3900` (9eb6f13c) failed on a stale ledger and 15 leaked fixture directories, both from the worker's concurrent edits and tests. Wait for the worker's explicit stop.
- **Do NOT write a criterion that a byte-for-byte regeneration holds when the cited records can move** (handoff-28, hit again in `d11a45d4` REQ-01). Write "the committed output equals the generator's output".
- **Do NOT fix one instance of an identity condition and leave its siblings** (handoff-16, repeated four times this session: Git status in the driver, revision in a struct equality, a tree-derived digest inside `resume_equivalent`). Order the exhaustive trace of every Git-derived value to every comparison it reaches at the first finding.
- **Do NOT let a verifier compare a collected campaign's pin with a live closure.** `family-producing-closures` regenerates live manifests on every source change; collected campaigns pin snapshots under `inputs/producing/` (`20af7e6e` rework 1).
- **Do NOT leave raw campaign evidence only in `.agents/campaign-evidence/`.** Research-review blocks on `measurement-contract.md:114`; commit the archive as a `.tar.zst` under `dev/bench_results/<issue>/` (`63bad95d`, `20af7e6e`).
- **Do NOT let a library embed the workspace `Cargo.lock`.** Every arm compiling the library then reads it at build; embed it in the consumer (`9eb6f13c`).
- **Do NOT add a dependency to `tuning-campaign-support` casually.** 33 standalone crates with committed lock files depend on it by path.
- **Do NOT name a variable `path` in a zsh loop.** It is tied to `PATH`; every command after it fails with "command not found".
- **`jit doc add` refuses binaries and paths deeper than its budget;** use `--skip-scan` for Markdown that links a deep snapshot, and cite archives from a linked document.
- **Restart sccache from the home directory before every lead CI run.** A server started under a worker's deleted temp directory failed builds with "No such file or directory".

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`; story: `jit issue show ed3d490e`
- Verdicts of this session: `reviews/4252085a-r4.md`, `reviews/63bad95d-r2.md`, `reviews/20af7e6e-r4.md`, `reviews/817fc47d-r1.md`, `reviews/815703a1-r2.md`
- Rulings: `progress.json` `escalations` (session 37 entries, including the interview of 2026-10-14)
- Campaign archives: `dev/bench_results/63bad95d/*-evidence-archive.tar.zst`, `dev/bench_results/0a77ecfc/*-evidence-archive.tar.zst`
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`; state `.agents/bench-window/`
