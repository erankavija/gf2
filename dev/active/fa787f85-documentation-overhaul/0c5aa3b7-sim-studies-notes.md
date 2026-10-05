# Loose Markdown in simulation_results and studies (0c5aa3b7)

Scope: every tracked `*.md` under `dev/simulation_results/` and `dev/studies/`
at `412b3cd49`. Owner epic b8206228 is open, so moved files sit under its
`dev/active/` directory.

## Classification

| File | Class | Evidence |
| --- | --- | --- |
| `dev/simulation_results/osd-ebch-128-64/README.md`, `schema1/README.md` | consumed | decided in `f902240f-osd-archive.md:69`; the dataset sits beside `crates/gf2-sim/tests/osd_campaign_protocol.rs:784` |
| `dev/simulation_results/permanent-zero-fraction/protocol.md` | consumed, digest-pinned | `crates/gf2-sim/src/permanent_rare_event/artifact.rs:73`, `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:1173`, `crates/gf2-sim/tests/permanent_campaign_bin.rs:206`; SHA-256 at `dev/active/02b8137c/validation-journal/run-state.json:111` |
| `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/freeze.md` | digest-pinned | SHA-256 at `dev/active/02b8137c/validation-journal/frozen-artifacts-start.json:10` |
| `dev/studies/b488f02c/literature-search-2026-08-08.md` | consumed, digest-pinned | `include_bytes!` at `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:178`; digest constant at `crates/gf2-sim/src/bin/permanent_campaign.rs:44` |
| `dev/studies/b488f02c/feasibility-study.md` | digest-pinned | `dev/active/b8206228-permanent-statistics/receipt-inventory.md:16` |
| `dev/studies/047b62ed/receipts.md` | digest-pinned | `dev/active/b8206228-permanent-statistics/receipt-inventory.md:26` |
| `dev/studies/91605d4d/receipts.md` | digest-pinned | `dev/active/b8206228-permanent-statistics/receipt-inventory.md:35` |
| `dev/studies/6c7fcb38/receipts.md` | digest-pinned | `dev/active/b8206228-permanent-statistics/receipt-inventory.md:44` |
| `dev/studies/a9284086/receipt.md` | consumed | `dev/studies/a9284086/analysis.py:1092` reads it |
| `dev/studies/0dffa759/findings.md` | digest-pinned citation target | cited by path in `dev/active/b8206228-permanent-statistics/backend-selection-draft.md:24`, whose SHA-256 sits at `dev/active/b8206228-permanent-statistics/manifest-draft/manifest.json:1634` |
| `dev/simulation_results/permanent-zero-fraction/README.md` | moves | `dev/active/b8206228-permanent-statistics/simulation_results/permanent-zero-fraction/README.md` |
| `dev/studies/0dffa759/req08-amendment-draft.md` | moves | `dev/active/b8206228-permanent-statistics/studies/0dffa759/req08-amendment-draft.md` |
| `dev/studies/0dffa759/rng-provenance-addendum.md` | moves | `dev/active/b8206228-permanent-statistics/studies/0dffa759/rng-provenance-addendum.md` |

No worker branch of `git worktree list` carries commits touching either
directory beyond the base commit.

## Repointed

| File | Change |
| --- | --- |
| moved README | link targets to `protocol.md`, the q3 target CSV and two benchmark files |
| moved addendum, moved amendment draft | link targets and their matching link labels |
| `dev/studies/0dffa759/findings.md` | seven link targets to the two moved files |
| `dev/active/b8206228-permanent-statistics/handoff.md:62` | dataset README path |
| `dev/active/b8206228-permanent-statistics/{dispatch-7a816262-phase1,handoff-7,handoff-18,handoff-20}.md`, `dev/active/b8206228-permanent-statistics/progress.json`, `dev/active/b8206228-permanent-statistics/breakdown.json`, `dev/active/b8206228-permanent-statistics/freeze-feasibility-draft.md`, `dev/active/b8206228-permanent-statistics/generate_manifest_draft.py` | citations of the moved files' paths |
| `crates/gf2-sim/src/bin/permanent_dataset.rs`, `crates/gf2-sim/src/permanent_campaign/provenance.rs` | path citation replaced by "the dataset README"; line counts unchanged |

## Retained citations

Files that keep their bytes and cite a moved note by its earlier path.

| File | Reason |
| --- | --- |
| `dev/studies/b488f02c/feasibility-study.md:1036,1095,1506` | SHA-256 at `dev/active/b8206228-permanent-statistics/receipt-inventory.md:16` |
| `dev/bench_results/*/inputs/producing/` | snapshot copies of the two gf2-sim sources |

## Verification

| Command | Establishes |
| --- | --- |
| `python3 contrib/gates/docs-mechanical.py` | gate passes |
| `python3 dev/active/fa787f85-documentation-overhaul/migration/check.py` | rows of this unit report no finding |
| `./scripts/cargo-budget.sh cargo fmt --all -- --check` | formatting of the two Rust edits |
| `git diff 412b3cd49 --name-only --diff-filter=AMR -- '*.md' \| python3 -c "import sys,re,os;[print('BROKEN',f,u) for f in sys.stdin.read().split() for u in re.findall(r'\]\(([^)\s#]+)',open(f,encoding='utf-8').read()) if not re.match(r'\w+:',u) and not os.path.exists(os.path.normpath(u[1:] if u[0]=='/' else os.path.join(os.path.dirname(f),u)))]"` | no output means no unresolved local inline link in the touched Markdown files |

Tracker references are relinked by `target/0c5aa3b7-relink.sh`. The `5758790d` reference to the dataset README stays pinned at `20a3fb30` under its earlier path.
