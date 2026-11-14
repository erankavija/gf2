# Investigation: tracker repository (6ba9be17)

Facts the plan rests on, read from the tree at `main` 6892ecd54 and from the
jit 1.0.0 reference documentation in the jit source: the storage format,
storage record, configuration and worktree references. Each finding has a
stable identifier the manifest cites.

## Claim classification

| Id | Claim | Class | Evidence |
|---|---|---|---|
| F-01 | The tracker is 12.97 MB of the tracked tree in 1397 files, 1368 of them issue records | valid-and-open | `git ls-files .jit`; limit `max_tree_bytes = 34040000` in [tracked-budget.toml](../../../scripts/tracked-budget.toml) |
| F-02 | jit selects a separate data root through `JIT_DATA_DIR`, and the code checkout through `JIT_WORKTREE_ROOT` or `[worktree].root`; the environment variable outranks the key | valid-and-open | jit storage format reference, section "Data and Worktree Roots" |
| F-03 | A gate checker runs from the code worktree root, and its run record carries that checkout's `commit` and `branch` | valid-and-open | jit storage format reference, same section; storage record reference, gate run fields |
| F-04 | With neither setting, the code root is the checkout that contains the data root, so a gate started from a linked worktree with only `JIT_DATA_DIR` set runs in the primary checkout | valid-and-open | jit storage format reference, same section; steward's scratch experiment |
| F-05 | jit discovery walks up from the invocation directory and stops at a git boundary, so a linked worktree without its own store finds none | valid-and-open, read from source only | jit storage discovery module, test `test_discover_from_stops_at_git_boundary` |
| F-06 | Claim leases live in the git common directory of the repository that holds the data root; a lease names `agent_id` and `worktree_id` | valid-and-open | jit storage format reference, section "The `.git/jit/` Control Plane" |
| F-07 | A data root inside a checkout holds one worktree identity, so checkouts sharing a nested store differ only by `JIT_AGENT_ID` | valid-and-open | jit storage format reference, section "Worktree Identity"; steward's scratch experiment |
| F-08 | The issue record stores a gate's `status` and `run_id`; the commit is in the gate-run record only | valid-and-open | any issue record with a passed gate; jit storage record reference |
| F-09 | Gate-run records are ignored by git and hold 46 MB in the primary checkout; the kani gate writes compressed logs under `.jit/gate-artifacts/` | valid-and-open | [.gitignore](../../../.gitignore) lines 62 to 70; [cargo-kani-gate.sh](../../../scripts/cargo-kani-gate.sh) line 8 |
| F-10 | jit commits nothing to git and has no event-log compaction; `jit worktree exec` runs a command under the store's write lock | valid-and-open | jit worktree reference, section "jit worktree exec" |
| F-11 | The reference checker reads invariant identifiers from `.jit/invariants.toml` and skips the check when the file is absent | valid-and-open | [check-references.py](../../../scripts/check-references.py) lines 118 and 790 to 794 |
| F-12 | The reference checker treats `.jit/` as a repository root, scans issue records as tracked files, and passes any fault whose target git ignores | valid-and-open | [check-references.py](../../../scripts/check-references.py) lines 83, 834, 845 and 879 to 886 |
| F-13 | `@/issue/` references are resolved only by the docs-mechanical gate, which calls jit | valid-and-open | [docs-mechanical.py](../../../contrib/gates/docs-mechanical.py) lines 202 to 222 |
| F-14 | The docs-mechanical gate reads `.jit/config.toml` under the code root and copies the code repository's unignored `.jit` files to render projections | valid-and-open | [docs-mechanical.py](../../../contrib/gates/docs-mechanical.py) lines 188, 229 and 274 |
| F-15 | The research-review gate reads `.jit/references.toml` relative to its working directory | valid-and-open | [research-review.sh](../../../contrib/gates/research-review.sh) line 24 |
| F-16 | The gate scripts and review prompts under `contrib/gates/` are projected from `packages/sim-research/assets/live/`, and the JIT workflow section of `AGENTS.md` from that package's guidance region | valid-and-open | [manifest.toml](../../../packages/sim-research/manifest.toml) lines 857 to 893 |
| F-17 | CI on GitHub runs the reference and budget checkers and never calls jit | valid-and-open | [ci.yml](../../../.github/workflows/ci.yml) lines 24 to 30 |
| F-18 | The invariant projection in `AGENTS.md` lists every invariant identifier between the `jit:invariants` markers | valid-and-open | [AGENTS.md](../../../AGENTS.md) lines 131 to 162; `[projection.invariants]` in the tracker configuration |
| F-19 | The repository has 67 worktrees, each with its own copy of the store; this repository allows tracker writes from linked checkouts | valid-and-open | `git worktree list`; `[worktree] write_policy = "allow"` in the tracker configuration |
| F-20 | The history rewrite requires every live branch merged or abandoned and no active session, and checks `jit validate` on the rewritten tree | valid-and-open | [history-rewrite-runbook.md](../b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md), Preconditions and step 7 |
| F-21 | The worker dispatch script creates linked worktrees and prints a prompt header; it sets no jit environment | valid-and-open | [dispatch-worker-worktree.sh](../../../.agents/skills/jit-execution-lead/scripts/dispatch-worker-worktree.sh) lines 100 and 133 to 153 |
| F-22 | The pre-commit hook and `scripts/cargo-ci.sh` run the budget and reference checkers | valid-and-open | [pre-commit](../../../.githooks/pre-commit); [cargo-ci.sh](../../../scripts/cargo-ci.sh) lines 279 to 283 |
| F-23 | jit passes its own environment to a gate checker and removes only an unset `JIT_CONTEXT_FILE`, so `JIT_DATA_DIR` and `JIT_WORKTREE_ROOT` reach the checker | valid-and-open, read from source only | jit gate execution module, where the checker command is built |
| F-24 | A run that reused a verdict launched no checker: its `commit` is unset and its `origin` names the run that executed the checker | valid-and-open | jit storage record reference, gate run fields `commit` and `origin` |
| F-25 | The vendored standards scanner and fixer take the project root as argument, exit 2 unless `.jit` exists under it, read `.jit/config.toml` and the document directories from it and run jit there; neither reads `JIT_DATA_DIR` | valid-and-open | [standards-scan.sh](../../../.agents/skills/jit-project-lead/scripts/standards-scan.sh) lines 48 to 63, 86 and 116; [standards-fix.sh](../../../.agents/skills/jit-project-lead/scripts/standards-fix.sh) lines 56 to 90 |
| F-26 | The rewrite's path filter considers only paths under the bench results directory and `dev/active/`, and keeps every other path; its pin mapper rewrites `.jit/issues/*.json` of the checkout it runs in; the runbook validates and reseats without a separate tracker | valid-and-open | [rewrite_paths.py](../b8206228-permanent-statistics/45ecf975/rewrite_paths.py) line 4; [map_pins.py](../b8206228-permanent-statistics/45ecf975/map_pins.py) line 14; [history-rewrite-runbook.md](../b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md) steps 5 to 10 |
| F-27 | jit resolves an unpinned document link in the code checkout's working tree and a pinned one at its commit in the code checkout's git objects, which linked worktrees share; the reference checker resolves both against the current checkout's files | valid-and-open, read from source only | jit document reference module, `DocumentReferenceRequests::for_reference`; jit storage format reference, section "Data and Worktree Roots"; [check-references.py](../../../scripts/check-references.py) lines 384 to 387 |
| F-28 | A gate pass that clears an issue's last unpassed gate moves it to done and promotes its dependents | valid-and-open | `jit gate evaluate --help` |
| F-29 | A gate definition declares its inputs as roots and exclusions, reported by jit's machine-readable gate output | valid-and-open | `jit gate show cargo-ci --json`, field `inputs` |

## Not verified

- F-04, F-05 and F-07 were not re-run in a scratch repository by this
  investigation: its session could run git only inside its own worktree. They
  rest on the jit documentation, the jit source and the steward's experiment.
  The rehearsal in the plan re-checks all three before the cutover.
- Whether `jit validate` reads the 46 MB of gate-run records within the
  rehearsal's time bound on a store shared by all worktrees.
- How `[worktree] root` resolves for a store that is its own git repository
  nested in the checkout; the rehearsal checks it.
- F-27 for a nested independent store: that a pinned link resolves from the
  code repository's objects rests on the jit source and its test of pinned
  reads in the selected code subdirectory; the rehearsal checks it from a
  second checkout.
- The contents of the interim budget exclusion that lead gf2-e9 implements;
  it was not on `main` at the commit read.

## Consumers of tracker paths

`git grep -l -F '.jit/' -- ':!.jit'` lists them. By family: the checkers and
their configuration under `scripts/`; the gate checkers, prompts and their
package sources (F-14 to F-16); `AGENTS.md` and `dev/index.md`; the vendored
jit skills under `.agents/skills/`, which name the data root generically; the
one-shot `dev/scripts/migrate-link-labels.py`; and dated documents under
`dev/active/`.

## Invariants touched

- `@/inv/resolvable-references`: a tracker path in tracked text resolves only
  with the tracker mounted. The statement needs an owner-approved amendment.
- `@/inv/canonical-cutover`: the cutover removes the union-merge attribute,
  the tracker exceptions of both checkers and the interim budget exclusion.
- `@/inv/committed-evidence-budget`: gate-run records stay uncommitted in the
  tracker repository, as they are today.
- `@/inv/no-dev-path-coupling`: every script resolves the store through the
  data root, never through a path relative to the caller.
