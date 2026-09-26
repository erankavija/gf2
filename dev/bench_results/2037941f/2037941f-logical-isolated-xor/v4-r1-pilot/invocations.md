# Invocation record: 2037941f-logical-isolated-xor

> **Diátaxis Type:** Reference

Window argv comes from the committed window-log excerpt. Inner argv is
reconstructed from the pinned measured launcher and retained run inputs;
the historical run did not retain a process-exec trace. Paths and redirects
below are the operands the pinned scripts compute from those inputs.

## Input identities

| Input | SHA-256 |
|---|---|
| `dev/bench_results/2037941f/logical-window-jobs.log` | `0bff20661b7445601c6cb36b80ef639881fdfb41fadfeb22954fa7fabaa14cca` |
| `dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/receipt.json` | `1994686cbee1c972e4e573dd7bd57228218672dd8c4b973ed6e89256bc11015c` |
| `dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/launcher.log` | `0dd758191818d5ecc366b28d237adcdf69adac91cee1e30714883743c1a38a4a` |
| `dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/execution.log` | `c40cbe6cb19694997223acb9960afe23f7fbd8b505770f3521b3e892e7690940` |

## Observed window dispatch

`dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-isolated-xor --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-isolated-xor.json --run-id v4-r1`

Source lines: `2026-09-19T01:00:28Z job start issue=18a87159 key=0b69167d1136a398 est_min=20 worktree=.agents/worktrees/agent-b64dc9c4 load=0.42 0.30 0.28 command=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-isolated-xor --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-isolated-xor.json --run-id v4-r1`; `2026-09-19T01:03:04Z job exit issue=18a87159 key=0b69167d1136a398 rc=0`.

## Reconstructed runner dispatch

Measured source: `d1425cb17226d0683659656f5f7dca83ab841ea9:dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh` (SHA-256 `0c3226f7cc6ddfd290ff26ac8884cde6772e3a6a6ebe0499ab587fd72a01f2e7`).

The launcher log records 9 runner sessions, ending with exit 0.

```sh
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor.plan.json
/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner finalize /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-isolated-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot
```

The runner's child arm argv, environment and executable digest are in
`dev/bench_results/2037941f/2037941f-logical-isolated-xor/v4-r1-pilot/receipt.json` and its append-only `execution.log`.
