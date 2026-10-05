# 7dbcde0c BCH entries move record

Base commit `412b3cd49`; runner root resolution in `2191bcaba`, moves and
citation repoints in `3720a3304`. Paths below are relative to `dev/active/`.

## Entries

| Entry | Tracker state at the move | Destination | Self-path literal edit |
|---|---|---|---|
| d1b4f85e | done | `ae03bcd0-general-bch/d1b4f85e/` | `run.sh:25`, usage line |
| 591a1c5e | done | `ae03bcd0-general-bch/591a1c5e/` | none: no script |
| fd9d5416 | in_progress (DEC-01) | `ae03bcd0-general-bch/fd9d5416/` | none: `render_receipt.py` and its test hold no literal of their directory |

`jit doc dir ae03bcd0 dev/active` prints `dev/active/ae03bcd0-general-bch`.
`git diff -M --stat 412b3cd49 3720a3304 -- '*render_receipt.py'` reports two
renames with zero changed lines. `2191bcaba` replaces the three-level parent
path of `d1b4f85e/run.sh:28` with `git -C <script dir> rev-parse --show-toplevel`;
both expressions print the same directory at `412b3cd49`. The runner has not
run at either commit.

## Renderer output (REQ-04)

The repeat run in [3fd3db5e-baseline.md](3fd3db5e-baseline.md) contradicts
REQ-04 as written: the test results are identical, and the synthetic receipt
differs from the baseline capture in two lines that carry three paths the
renderer observes at run time. The same record states the command whose `diff`
is empty with the directory above each entry removed from those paths. The
committed fd9d5416 receipt keeps its bytes and differs from a render at
`3720a3304` in the corresponding two lines.

## Checks

| Check | Command | Result | Establishes |
|---|---|---|---|
| Unmerged work | `git cherry main <branch>` and `git diff --stat main...<branch> -- <the epic directory and the three entries>` for each branch and the detached head of `git worktree list` | no diff line | no worker branch changes these paths |
| Uncommitted work | `git -C <worktree> status --short -- <the same paths>` for each worktree | empty | no worktree holds an uncommitted edit of these paths when the move starts; an edit begun afterwards is outside it |
| Digest pins | `git grep -nF -e <SHA-256> -e <blob id> -- . ':!.jit'` of each file of the two tables below at `412b3cd49` | hits for `3fd3db5e-before.txt` and `host.txt` only | no committed digest pins a repointed file |
| Link scan | `sed 's/jit:103a792a/jit:7dbcde0c/' dev/active/fa787f85-documentation-overhaul/103a792a-link-scan.py \| python3 -` | `unresolved local links: 0` | each inline link in the Markdown files of the `jit:7dbcde0c` commits resolves; anchors are outside it |
| Mechanical check | `python3 contrib/gates/docs-mechanical.py` | PASS, 0 findings | links and anchors of `dev/active/**/*.md` |
| Manifest | `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding names a moved row; six rows complete; exit 1 on six `pending source is missing` findings for rows under 3be770d5, 4c1e441f, ad2a6a58 and f63a2464, equally at `412b3cd49` | source absent and destination present for each moved row |

## Repointed files outside the moved entries

| File | Edit |
|---|---|
| `ae03bcd0-general-bch/handoff-8.md`, `progress.json` | code spans and one JSON string naming the three entries |
| `4e732b56/workload-selection.md:312` | code span naming the runner |
| `fa787f85-documentation-overhaul/handoff-6.md:58`, `wave11-worker-brief.md:58` | code span naming the fd9d5416 directory |

Inside the entries, `fd9d5416/receipt-protocol.md` takes the destination path in
its code spans and one more parent level in each relative link that leaves the
epic directory.

## Files that keep the flat path

| File | Kind |
|---|---|
| `dev/bench_results/fd9d5416/2026-10-02-fd9d5416-bch-performance-receipt.md`, `2026-10-02-d1b4f85e-w1w2/host.txt` | receipt and run directory |
| `fa787f85-documentation-overhaul/3fd3db5e-before.txt` | capture pinned by SHA-256 in the baseline record |
| `fa787f85-documentation-overhaul/67048b47-linked-pairs.txt`, `7bac1303-harness-path-trials.txt` | committed script output |
| `fa787f85-documentation-overhaul/breakdown.json` | copies of issue descriptions and their touched-path lists |
| `fa787f85-documentation-overhaul/dev-path-code-consumers.md:49,176` | consumer inventory rows citing line numbers of the scanned state |
| `1a379447-zen3-cpu-performance/bench-window/queue.tsv:43` | bench-window queue line; its command runs in the worktree `agent-d1b4f85e-run`, a detached head that holds the flat path |
