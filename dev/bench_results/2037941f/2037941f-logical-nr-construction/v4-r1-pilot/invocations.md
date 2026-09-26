# Invocation record: 2037941f-logical-nr-construction

> **Diátaxis Type:** Reference

Window argv comes from the committed window-log excerpt. Inner argv is
reconstructed from the pinned measured launcher and retained run inputs;
the historical run did not retain a process-exec trace. Paths and redirects
below are the operands the pinned scripts compute from those inputs.

## Input identities

| Input | SHA-256 |
|---|---|
| `dev/bench_results/2037941f/logical-window-jobs.log` | `0bff20661b7445601c6cb36b80ef639881fdfb41fadfeb22954fa7fabaa14cca` |
| `dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot/receipt.json` | `aaee7b11b6d003f1dadd478a9b33d0257257c8cfa945609da88d78994be99e42` |
| `dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot/launcher.log` | `61352e85d02f53befea429ed77ec806418847168d462698f1e38c09cab9fea17` |
| `dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot/execution.log` | `dbac35b7616fea7ee658a59e506c942d2cdaf8a7d30be55fb7c2827b81e5dcb2` |

## Observed window dispatch

`dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-nr-construction --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-nr-construction.json --run-id v4-r1`

Source lines: `2026-09-19T01:05:09Z job start issue=18a87159 key=37ee8e4e5485bbe6 est_min=20 worktree=.agents/worktrees/agent-b64dc9c4 load=1.20 0.92 0.56 command=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-nr-construction --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-nr-construction.json --run-id v4-r1`; `2026-09-19T01:05:46Z job exit issue=18a87159 key=37ee8e4e5485bbe6 rc=0`.

## Reconstructed runner dispatch

Measured source: `d1425cb17226d0683659656f5f7dca83ab841ea9:dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh` (SHA-256 `0c3226f7cc6ddfd290ff26ac8884cde6772e3a6a6ebe0499ab587fd72a01f2e7`).

The launcher log records 3 runner sessions, ending with exit 0.

```sh
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction.plan.json
/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner finalize /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-nr-construction /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot
```

The runner's child arm argv, environment and executable digest are in
`dev/bench_results/2037941f/2037941f-logical-nr-construction/v4-r1-pilot/receipt.json` and its append-only `execution.log`.
