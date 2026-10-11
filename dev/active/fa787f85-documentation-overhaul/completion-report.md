# Epic Complete: Overhaul documentation for research adoption (fa787f85)

**Started:** 2026-10-01
**Completed:** 2026-10-08
**Assignee:** agent:jit-execution-lead

## Summary

The repository carries a researcher-facing documentation surface under
`docs/` with a concise root README, one operational guide (`AGENTS.md`), an
invariant-grounded review gate and a mechanical link gate. Development
artifacts of terminal epics live in marker-backed archives, artifacts of open
epics under their epic directories, and the migration manifest records the
disposition of each pre-overhaul artifact.

## Metrics

| Metric | Value |
|---|---|
| Children completed | `progress.json` `waves`: every issue `done` except two `rejected` |
| Waves executed | 16 |
| Rework cycles | sum of `progress.json` `rework_counts` |
| Escalations | entries of `progress.json` `escalations` |
| Sub-agent dispatches | not counted across the thirteen sessions; session 13 dispatched ten |
| Issues created during execution | entries of `progress.json` `created_during_execution` |

## Success Criteria

`satisfies:fa787f85/REQ-nn` labels in the tracker name the carriers; the list
below gives the issues that close each criterion.

- [x] REQ-01 documentation invariants — `@/issue/3f29e945`, `@/issue/9b2886a7`, `@/issue/871b401d`, `@/issue/fca578b1`
- [x] REQ-02 `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md` — `@/issue/a82d2347`
- [x] REQ-03 root README — `@/issue/fdb998ec`
- [x] REQ-04 `docs/` layout, entry pages, two tutorials — `@/issue/b6fdb815`, `@/issue/cdba4e71`, `@/issue/e3ff339a`, `@/issue/b16d013a` and the page units
- [x] REQ-05 freshly authored pages — the page units, `@/issue/fca578b1`
- [x] REQ-06 performance evidence page — `@/issue/01c20b61`, `@/issue/1fca3739`
- [x] REQ-07 Rustdoc example audit — `@/issue/a0a29512`, `@/issue/153297cf`, `@/issue/2596b143`
- [x] REQ-08 roadmaps — `@/issue/e85b8edf`, `@/issue/4bdcd65a`, `@/issue/60652fe4`, `@/issue/875914b3`
- [x] REQ-09 decks — `@/issue/fb209a81`, `@/issue/d0b1545e`
- [x] REQ-10 container archives — `@/issue/83f9ce69`, `@/issue/f29a9225`, `@/issue/3759f995`, `@/issue/42037c91`, `@/issue/1ca94ec2`, `@/issue/f902240f`; decision DEC-02
- [x] REQ-11 active layout — the regroup and move units from `@/issue/4f161183` to `@/issue/4979271f`; decisions DEC-01 and DEC-02
- [x] REQ-12 legacy mirror — `@/issue/103a792a`, `@/issue/ec655592`, `@/issue/eb2a833d`
- [x] REQ-13 manifest and checker — `@/issue/a24b2af7`, `@/issue/96cea1b9`, `@/issue/a0ec34f2`, `@/issue/af84619e`
- [x] REQ-14 `dev/` top level — `@/issue/ec655592`, `@/issue/b2bb1065`, `@/issue/0c5aa3b7`, `@/issue/b959ffe8`; decision DEC-04
- [x] REQ-15 documentation policy — `@/issue/ec7d2aec`, `@/issue/d6ebc4ec`
- [x] REQ-16 gates — `@/issue/3f29e945`, `@/issue/d6ebc4ec`
- [x] REQ-17 current-state permanent text — `@/issue/ffc35b8c`, `@/issue/12907582`, `@/issue/fca578b1`, `@/issue/c423c7aa`; decision DEC-05
- [x] REQ-18 inherited tasks — `@/issue/41e1d47d`
- [x] REQ-19 links — `@/issue/c852d9b2` and the re-archive units; decision DEC-03
- [x] REQ-20 planning brief — `@/issue/41e1d47d`, `@/issue/c423c7aa`

## Wave Execution Log

- **Waves 1 to 4:** the contract story: Rustdoc and doctest CI steps, documentation invariants, the review prompt, the `docs-mechanical` gate.
- **Wave 5:** `AGENTS.md` lane, roadmap mapping, inherited-task triage, manifest schema and checker.
- **Waves 6 and 7:** inventories, permanent pages, crate READMEs, guide moves, policy widening.
- **Waves 8 to 10:** the source-comment sweep story and its censuses.
- **Wave 11:** container re-archives, regroup of flat entries, legacy moves.
- **Wave 12:** decoupling and moves of code-read entries, `dev/plans` removal, root README, `dev/index.md`, the permanent-surface audit.
- **Waves 13 to 16:** leftover directories, policy narrowing, manifest completion, a criteria read with three remediation units, link verification, checker retirement.

## Key Decisions

- Records name a commit that holds every file their commands read, and an output is committed only when a clean checkout of that commit reproduces it.
- Multi-issue merges and tracker state commits carry the epic id; unit ids tag only the unit's own content.
- A read of the tree against the twenty criteria ran before the epic gates (session 13); its findings closed as units or owner decisions, and the six epic gates passed on their first run.
- Defects outside the epic's scope are tracked under `@/issue/b4b4b9ee`.

## Escalations

`progress.json` `escalations` holds each question with its ruling. The rulings
that changed scope are the decisions DEC-01 to DEC-05 of the epic and the DEC
entries of the units. One question is open: the 72-character subject of
commit `62cd9a3eb` on unpushed main.

## Issues Discovered During Execution

`progress.json` `created_during_execution` lists each issue with its reason.
Session 13 filed `@/issue/4979271f`, `@/issue/d0b1545e`, `@/issue/c423c7aa`
in the epic and `@/issue/bdf8e2e5`, `@/issue/720f5f74`, `@/issue/727677b4`,
`@/issue/aed7e749` under `@/issue/b4b4b9ee`.

## Holistic Quality Notes

- Review failures clustered in records, not in the work: undefined symbols in commands, volatile counts, narration, a verified commit without its outputs. A record with fenced runnable commands and an establishes/does-not-establish table passed on the first run.
- Three `c852d9b2` review-gate runs overlapped the benchmark window of 2026-10-07T23:00:04Z to 23:03:35Z, which timed jobs of `@/issue/bfc2ceab` and `@/issue/63bad95d`; no build ran in the overlap.
- The epic directory stays under `dev/active/`; its container archive preview lists moves with pending deletions and needs a pre-flight before execution.
