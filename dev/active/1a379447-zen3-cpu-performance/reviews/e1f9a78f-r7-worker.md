# Dense harness round 7 worker resolution

> **Diátaxis Type:** Reference

The `window` launcher captures argv before dispatch and prints a shell-quoted command in `launcher.log`. The same command encoding supplies build and smoke records. The focused test runs the launcher parser and `launcher.log` writer in a disposable fixture with an empty value, spaces, shell punctuation, and a newline. The fixture exits before any runner command. No timed run was launched.

## Cumulative resolution

Paths in this table are relative to `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/`.

| Round | Source | Finding (verbatim) | Closure at this tree |
|---|---|---|---|
| R1 | review at `8f1ace4e` F1 | Timed M4RI samples do not record or verify the dynamically loaded libm4ri.so identity. | `survey/dense-harness/src/external.rs:72,85,102`; loaded object path and digest are checked |
| R2 | review at `7a235715` F1 | Allocated-matvec output retention clears and drops outputs inside a timed window when its bounded capacity is reached, violating the frozen release-outside-window cost boundary. | `survey/dense-harness/src/routes.rs:199,208,221,226`; admission and release stay outside windows |
| R2 | review at `7a235715` F2 | Attributable commit 9ab18326 uses the disallowed wip conventional-commit subject. | Historical-ref ancestry below; cited SHA is outside `main` |
| R3 | review at `db58b44a` F1 | The prepared-host retention-feasibility claim is unsupported by a protocol-specific receipt and is not identified as an estimate. | `dense-parity-harness.md:174-180`; calibration is admitted or refused at run time |
| R4 | review at `461ba3e2` F1 | Padded sizing creates streaming banks below the frozen 8 MiB minimum for 9w, 63w, and 65w. | `survey/dense-harness/src/fixture.rs:86-92,120-160`; streaming banks use ceiling division |
| R5 | review at `f7cf2189` F1 | Four non-exempt tagged merge commits use disallowed merge(...) subjects. | Historical-ref ancestry below; cited SHAs are outside `main` |
| R5 | review at `f7cf2189` F2 | Twelve non-exempt tagged commits have subjects of 72 or more characters. | Historical-ref ancestry below; cited SHAs are outside `main` |
| R6 | review at `becc0650` F1 | The timed launcher logs an empty post-parse argument list, so receipt launcher.log does not retain the exact invocation. | `survey/run-dense-harness.sh:25-32,340`; argv preserved and shell-quoted; `survey/test-dense-launcher-argv.py:55` |
| R6 | review at `becc0650` F2 | Four tagged commits use the disallowed merge(...) subject type instead of the required chore(jit:<id>): merge ... form. | Historical-ref ancestry below; cited SHAs are outside `main` |
| R6 | review at `becc0650` F3 | Fourteen tagged commit subjects exceed the repository's 71-character limit. | Historical-ref ancestry below; cited SHAs are outside `main` |
| R6 | review at `becc0650` F4 | Commit 9ab18326 uses the disallowed wip(...) commit type. | Historical-ref ancestry below; cited SHA is outside `main` |

## Historical-ref ancestry

At the audit point, `main` is `822f3a53533dddece894dec628abb904ee0bda30`. `git merge-base --is-ancestor <sha> main` returns false for each exact cited SHA: `7ddc05ed`, `d772848e`, `84f07a70`, `8f1ace4e`, `bd8b600b`, `838275b0`, `b2661831`, `5a53a00`, `5c111d32`, `9f33c449`, `e30b2d9a`, `361a5987`, `9ab18326`, `5f164f96`, `c02cc0ec`. `git log main --grep=jit:e1f9a78f` returns 55 issue-scoped main ancestors; all 55 match the allowed type and 71-character subject limit. The ancestry command was run for each full SHA resolved with `git rev-parse <short>^{commit}`. Subjects were checked with `git show -s --format=%s <sha>`; all 55 main subjects matched `^(feat|fix|docs|test|refactor|perf|chore)\(jit:e1f9a78f\): .+` and had length at most 71.

## Prior behavior and scan

Loaded M4RI object identity, output release outside windows, ceiling-sized streaming banks, and run-time retention admission remain at the cited paths above. The shared staged smoke record remains `survey/dense-runner-smoke.txt:12,23,35` with zero timing samples. The pre-edit linked-doc marker scan found no matches. Per-finding `rg -n` sweeps of the launcher, harness sources, and harness prose found no additional unfixed instance of these findings.

## Focused checks

The launcher test fails in all three cases against base commit `93d3d72f` and passes at this tree. `python3 -B survey/test-dense-launcher-argv.py`, `bash -n survey/run-dense-harness.sh`, `python3 -B survey/make-dense-producing-inputs.py --check`, and `git diff --check` pass. The full repository gates remain with the lead.
