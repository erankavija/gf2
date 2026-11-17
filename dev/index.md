# gf2 development directory

`dev/` holds gf2's developer documentation, working documents and
development tooling. An experiment's code,
protocol and configuration live at
[`experiments/<name>/`](../experiments/). Permanent documentation is indexed at
[`docs/index.md`](../docs/index.md), and [`AGENTS.md`](../AGENTS.md) is the
engineering contract for all of them.

The [eBCH OSD](../experiments/osd-ebch/README.md) and
[DVB-T2 AWGN](../experiments/dvb-t2-awgn/README.md) campaign crates live under
`experiments/`.

The `[documentation]` table of [`.jit/config.toml`](../.jit/config.toml)
declares the development root, the issue-scoped area and the permanent paths.
`jit config get documentation` prints it.

## Tracker checkout

The workflow rules are in the JIT workflow section of
[`AGENTS.md`](../AGENTS.md). In short:

- `.jit/` is the checkout of the tracker repository, ignored by the code
  repository. A fresh clone mounts it with `scripts/tracker-bootstrap <path-or-url>`.
  On a host with a mounted tracker, its home is `git -C .jit remote get-url origin`; a fresh clone on that host mounts it with `scripts/tracker-bootstrap "$(git -C <primary-checkout>/.jit remote get-url origin)"`.
- A linked worktree runs jit as `scripts/tracker-env jit ...` and finds the
  store's files under the primary checkout's `.jit/`. A gate run there records
  the caller's `JIT_AGENT_ID`, otherwise the holder of the issue's active
  lease, otherwise `agent:<worktree name>`.
- jit limits each agent's indefinite leases by
  `[coordination] max_indefinite_leases_per_agent`, read from
  `.jit/config.toml`, which leaves it unset, so jit's default of two applies;
  `jit config show` prints the effective value. A lead
  holding more issues takes finite leases and extends each with
  `jit claim renew` before it expires; an indefinite lease needs a
  `jit claim heartbeat` within jit's claim stale threshold.
- Skill scripts that require `.jit` under their root run against the primary
  checkout.
- A document existing only on a branch is linked with `jit doc add --commit
  <branch-commit>` and re-linked unpinned when it lands.
- A branch lands by the [landing flow](#landing-flow). A code-producing issue
  carries the `landed` gate (`code-task` preset).
- A lead runs `scripts/tracker-snapshot` after each landing, the steward at the
  end of a session. With `--push` it also pushes the store to its home;
  without it, it commits locally and `git -C .jit push origin main` sends the
  commits. Each snapshot commits the pending changes of every session sharing
  the store, and its `Jit-Agent` trailer names the agent that took it, when
  `JIT_AGENT_ID` is set.

## Landing flow

One branch lands on `main` at a time, in these steps:

1. The issue's gates other than `landed` pass at the branch tip, evaluated
   through `scripts/tracker-env` in the branch's worktree.
2. `scripts/check-landing <issue> <tip>` passes.
3. The tip merges into `main` with `git merge --no-ff`, under a
   `chore(jit:<id>): merge ...` subject.
4. The merged-tree step: the required executed-command gates hold for the tree
   on `main` after the merge. The step is met without a run when `cargo-ci`'s
   verdict at the tip counts for the merge commit under the counting rule of
   `scripts/check-landing`: the gate's declared inputs are identical at both
   commits. A merge tree equal to the gated tip's tree, which a `--no-ff`
   merge of a tip that contains `main` gives, is the common case. Otherwise,
   when `main` moved between gating and merging, `cargo-ci` reruns at `main`
   before the next step. Running the whole landing check at the merge commit
   is not this step: it refuses the gates that declare no inputs.
5. `scripts/tracker-snapshot` records the merge-time store.
6. The `landed` gate passes; it judges that each gated commit is an ancestor
   of `main`.
7. `scripts/tracker-snapshot` records the `landed` verdict.

A store-sourced projection, such as the invariant list in `AGENTS.md`
rendered from `.jit/invariants.toml`, lands in this order: the tracked copy
changes on the branch; when the branch enters review, the store entry changes
and `scripts/tracker-snapshot` records it; the merge follows promptly. Between
the store edit and the merge, the documentation check and repository
validation in the primary checkout read `main`'s copy as stale, and nobody
repairs it there in that window, with `jit validate --fix` or otherwise.

## Epic documents: `active/`

`active/<epic-short-id>-<slug>/` holds the prose and code of one open epic, per
`@/inv/document-placement`, and `active/<issue-short-id>/` those of one open
issue outside an epic. The epic id resolves the directory, and `jit doc add`
links each document to its owning issue:

```bash
mkdir -p "$(jit doc dir <epic-id> dev/active)"
jit doc add <issue-id> <path>
```

`scripts/check-tracked-budget.py` refuses other file kinds there.

## Operational directories

Place material by the first column. The last column names code that reads or
writes the directory.

| Material | Directory | Readers and writers |
|---|---|---|
| `sim_runner` campaign definitions | `campaigns/` | `crates/gf2-coding/src/bin/sim_runner.rs`, `crates/gf2-coding/tests/grand_phase1_smoke.rs` |
| Reference curves, their comparison scripts, complete tuning profiles | `reference_data/` | `crates/gf2-coding/tests/ebch_128_64_reference.rs`, `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`, `dev/tools/tuning-profile-compose/src/main.rs`, `dev/scripts/gen_fig7_report.sh` |
| Standalone prototype crates | `research/` | `crates/gf2-kernels-simd/Cargo.toml` (dev-dependency), `.jit/gates.toml`, `experiments/permanent-benchmarks/scripts/permanent-repro.sh` |
| Benchmark lock wrapper, benchmark window and its queue, campaign launchers | `scripts/` | `scripts/cargo-ci.sh`, [`AGENTS.md`](../AGENTS.md) |
| Tuning-campaign crates | `tools/` | Root `Cargo.toml` (workspace member), dev-dependency of `crates/gf2-core` and `crates/gf2-algebra`, `scripts/cargo-ci.sh` step `tuning-composer` |

Harness output is written below the gitignored `target/` directory.
 