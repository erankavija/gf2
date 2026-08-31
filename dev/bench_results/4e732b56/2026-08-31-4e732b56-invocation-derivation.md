# Invocation derivation record: external-baseline survey (jit:4e732b56)

Written 2026-09-01, post hoc, by the rework-2 session closing the lead's
evidence-integrity re-review. `run-survey.sh` emitted no `# command:` /
`# environment:` invocation headers into its own log/perf-stat output before
the revision that added header emission to `run_one` and the perf-stat block
(committed together with this record). Every stage run under that earlier
revision therefore has no runtime-recorded invocation in its log; the rows
below derive each one from `run-survey.sh`'s argument construction at that
earlier revision plus the stage's own log content and covering host manifest,
so the receipt can cite this record instead of carrying hand-written headers
inside committed tool output (`@/inv/runtime-observed-provenance`).

Two cells (`small-aff3ct` W2, `small-m4ri` W2) were re-run directly by the
rework-1 session on 2026-08-31 to satisfy the five-millisecond repetition
rule (research-review F3). Those invocations were executed by that session,
not derived — recorded here as `session-recorded` at the timestamp the
session reported, since `run-survey.sh` still did not emit headers itself at
that point (the direct `ccx1-bench-flock.sh` invocation, not `run_one`, was
used for the targeted re-run).

Basis values:

- `reconstructed` — derived from `run-survey.sh`'s argument construction at
  the pre-header-emission revision, plus the stage's log content and host
  manifest. Not run-time-recorded.
- `session-recorded` — executed directly by an agent session (not through
  `run-survey.sh`'s header-emitting path) at the stated timestamp; recorded
  here from the session's own account rather than a tool-emitted header.

| Stage | Invocation | Basis | Host manifest |
|---|---|---|---|
| `small-aff3ct` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `small-aff3ct` | `GF2_SURVEY_CODES=B1,B2,B3 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w2` | session-recorded (2026-08-31T23:17:01+03:00) | `2026-08-31-4e732b56-small-host.txt` |
| `small-bchlib` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `small-gf2` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/.agents/ext/survey-target/release/survey-gf2-side all` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `small-itpp` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `small-m4ri` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `small-m4ri` | `GF2_SURVEY_CODES=B1,B2,B3 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all` | session-recorded (2026-08-31T23:17:35+03:00) | `2026-08-31-4e732b56-small-host.txt` |
| `t2n-aff3ct` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |
| `t2n-bchlib` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |
| `t2n-gf2` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/.agents/ext/survey-target/release/survey-gf2-side all` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |
| `t2n-itpp` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |
| `t2n-m4ri` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/bench_results/4e732b56/generators.txt all` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |
| `small-aff3ct-perf` | `GF2_SURVEY_CODES=B1,B2,B3,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | reconstructed | `2026-08-31-4e732b56-small-host.txt` |
| `t2n-bchlib-perf` | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | reconstructed | `2026-08-31-4e732b56-t2n-host.txt` |

All invocation strings are unchanged from the receipt's prior "Per-stage
provenance" table; only their storage location and labeled basis are new.
`make-receipt.py` cites this record by path for any stage whose log carries
no native `# command:` header.
