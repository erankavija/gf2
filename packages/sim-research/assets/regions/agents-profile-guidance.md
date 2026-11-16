## gf2 Engineering Invariants

<!-- jit:invariants:begin -->
_No invariants declared._
<!-- jit:invariants:end -->

## JIT workflow

- `.jit/` is the checkout of the tracker repository; the code repository ignores it. A fresh clone mounts it with `scripts/tracker-bootstrap <path-or-url>`. On a host with a mounted tracker, its home is `git -C .jit remote get-url origin`; a fresh clone on that host mounts it with `scripts/tracker-bootstrap "$(git -C <primary-checkout>/.jit remote get-url origin)"`.
- Run jit from a linked worktree through `scripts/tracker-env jit ...`. The entry point sets `JIT_DATA_DIR` to the primary checkout's `.jit/`, where such a worktree finds the store's files, and `JIT_WORKTREE_ROOT` to the worktree. A gate run there records the caller's `JIT_AGENT_ID`, otherwise the holder of the issue's active lease, otherwise `agent:<worktree name>`.
- jit limits each agent's indefinite leases by `[coordination] max_indefinite_leases_per_agent`, read from `.jit/config.toml`, which leaves it unset, so jit's default of two applies; `jit config show` prints the effective value. A lead holding more issues takes finite leases and extends each with `jit claim renew` before it expires; an indefinite lease needs a `jit claim heartbeat` within jit's claim stale threshold.
- Skill scripts that require `.jit` under their root run against the primary checkout.
- Read `.jit/reference/content-standards.md` before authoring issues or planning documents.
- Derive hierarchy, templates, gates, namespaces, and documentation paths from repository configuration.
- A label means what its `[namespaces.<ns>]` declaration in `.jit/config.toml` says it means; read that declaration before judging what a label on an issue claims.
- Use `jit issue status`, `jit query available`, and the dependency graph to select and sequence work.
- Link a document that exists only on a branch with `jit doc add` pinned to a branch commit (`--commit`), and re-link it unpinned when it lands.
- A branch lands only after `scripts/check-landing <issue> <tip>` passes.
- A code-producing issue carries the `landed` gate through the `code-task` preset; the lead evaluates it after the merged-tree gate.
- A lead runs `scripts/tracker-snapshot` after each landing, and the steward at the end of a session. `scripts/tracker-snapshot --push` also pushes the store to its home; without `--push` it commits locally, and `git -C .jit push origin main` sends the commits.
- Run the configured gates before completing an issue; a passing review placeholder is advisory evidence only.
