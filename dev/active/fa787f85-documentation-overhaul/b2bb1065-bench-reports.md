# Top-level reports of dev/bench_results (b2bb1065)

Move commit: `edbbdd4d9`. `S` is
`dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md`; `A` is
`dev/archive/97bf0879-gf2-core-sota-performance`.

## Markdown files at the top level

Scope: `git ls-files ':(glob)dev/bench_results/*.md'` at the parent of the
move commit.

| File | Owner epic, tracker state | Code consumer | Placement | Manifest row |
|---|---|---|---|---|
| `S` | 97bf0879, `archived` | none | `git mv` to `A/bench_results/`, marker `A/.jit-container` | `jit-container-archive`, that destination, `complete` |
| `dev/bench_results/e0251af3-rank-event-rates.md` | b8206228, `backlog` | `crates/gf2-algebra/tests/rank_event_rates.rs:19` (`RECEIPT_FILE`), written at `:292` | in place | `retained-operational`, `complete` |
| `dev/bench_results` (directory row) | none | listed in the row | in place | `retained-operational`, `complete` |

The rank-event file is the receipt an ignored test regenerates at that path,
so the relocation protocol keeps it where its consumer writes it. Its tracker
reference on e0251af3 names the same path.

Consumer scan, run per basename over `scripts/`, `dev/scripts/`, `dev/tools/`,
`crates/`, `contrib/`, `.github/`, `.config/` and every `*.toml` except the
manifest: `git grep -nF <basename>`. The only hit is the `RECEIPT_FILE` line.
A search of the tracked tree outside `.jit/` for the SHA-256 and the git blob
id of `S` finds no occurrence.

The other top-level files carry the extensions `csv`, `txt` and `sh`. Each
`txt` file opens with a tool capture header or raw tool output.

## Tracker references to `S`

| Issue, document index | Pinned commit |
|---|---|
| 7a106fe4 0, 97bf0879 7, cc5de315 1 | `11be30f` |
| cc5de315 2 | `4cde8a8` |

The file has a different blob at each pinned commit and at the move commit,
and the archive path exists at neither pinned commit. The references keep
their path and pin. `jit doc check-links --scope issue:<id> --json`, run in
the worktree at the move commit for each of the three issues, reports
`"valid": true` with 0 errors and 0 warnings. The check does not show how a
further `jit archive container 97bf0879 --execute` treats the placed file.

## Citations of `S`

Scan: `git grep -nF "$S" -- . ':!.jit'`.

| File | Treatment | Ground |
|---|---|---|
| `perf-evidence-catalog.md:126` | repointed to `A/bench_results/` | live catalog |
| `dev/bench_results/3eeb57f6/2026-05-28-b0fa00af-sota-scorecard-final.md`, `dev/bench_results/b117c95e-gf2-core-perf/2026-05-08-2cfc4372-sota-scorecard.md` | kept | receipt files; the SHA-256 of each occurs in another tracked file |
| Files under `dev/archive/026fc832-gf2-core-sota-stretch/` and `A/` | kept | archived artifacts; each occurrence is backticked path text, none is a link target |
| `migration/manifest.toml` | kept | `path` and `inbound` fields |
| `83f9ce69-rearchive.md:103`, `83f9ce69-preexec-plan.json`, `investigation.md:50`, `handoff-8.md:36`, `progress.json:1900` | kept | records of the state at their own commits |

No Rust source, README, `AGENTS.md` or page under `docs/` cites `S`.

## Verification

| Command | Result | Does not establish |
|---|---|---|
| `sed 's/103a792a/b2bb1065/g' dev/active/fa787f85-documentation-overhaul/103a792a-link-scan.py \| python3 -` | 0 unresolved local links in the Markdown files the commits tagged `jit:b2bb1065` touch | anchors, reference-style links, backticked paths |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | no finding on the three rows above; the output differs from the output on main `412b3cd49` only in the two completeness lines | exit status 0: the run exits 1 on `pending source is missing` findings for rows of other units, identical on main |
| `python3 dev/scripts/check-receipt-input-snapshots.py`, `python3 dev/scripts/check-addendum-schema-versions.py` | exit 0, output identical to main `412b3cd49` | the remaining steps of `scripts/cargo-ci.sh` |
| `python3 contrib/gates/docs-mechanical.py` | PASS | – |
