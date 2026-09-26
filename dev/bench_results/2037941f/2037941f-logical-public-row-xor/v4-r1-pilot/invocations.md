# Invocation record: 2037941f-logical-public-row-xor

> **Diátaxis Type:** Reference

Window argv comes from the committed window-log excerpt. Inner argv is
reconstructed from the pinned measured launcher and retained run inputs;
the historical run did not retain a process-exec trace. Paths and redirects
below are the operands the pinned scripts compute from those inputs.

## Input identities

| Input | SHA-256 |
|---|---|
| `dev/bench_results/2037941f/logical-window-jobs.log` | `0bff20661b7445601c6cb36b80ef639881fdfb41fadfeb22954fa7fabaa14cca` |
| `dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/receipt.json` | `488bfff72b7432c0c8dc9a9e0116ac41ed216d14ca5978b921256aff38422865` |
| `dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/launcher.log` | `992f49835d4f558c4381fce54f3d24144e8f296a950e8486d61089eac0d16158` |
| `dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/execution.log` | `ad01eefa116220385bfb5dd7b3f6675c30b87c1b815bbd410196249719bbbf25` |

## Observed window dispatch

`dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-public-row-xor --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-public-row-xor.json --run-id v4-r1`

Source lines: `2026-09-19T01:03:04Z job start issue=18a87159 key=5aa311352bba2c72 est_min=25 worktree=.agents/worktrees/agent-b64dc9c4 load=1.35 0.77 0.47 command=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh window --family 2037941f-logical-public-row-xor --addendum dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/logical-public-row-xor.json --run-id v4-r1`; `2026-09-19T01:05:09Z job exit issue=18a87159 key=5aa311352bba2c72 rc=0`.

## Reconstructed runner dispatch

Measured source: `d1425cb17226d0683659656f5f7dca83ab841ea9:dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-logical-harness.sh` (SHA-256 `0c3226f7cc6ddfd290ff26ac8884cde6772e3a6a6ebe0499ab587fd72a01f2e7`).

The launcher log records 9 runner sessions, ending with exit 0.

```sh
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner run /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor.plan.json
/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/release/benchmark-ab-runner finalize /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/target/bb769456-campaigns/v4-r1-2037941f-logical-public-row-xor /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-b64dc9c4/dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot
```

The runner's child arm argv, environment and executable digest are in
`dev/bench_results/2037941f/2037941f-logical-public-row-xor/v4-r1-pilot/receipt.json` and its append-only `execution.log`.
