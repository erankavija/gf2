# Failed attempt: outside the benchmark window

Launched interactively on 2026-09-28 at 13:07 UTC from main `5e2deb712`.
Build, staging, self-check, list-grid and capability-report steps completed.
`run-session` then exited 3 before `LockHeld`: `ccx1-bench-flock.sh` refuses the
host mutex outside a benchmark window (invoker policy 2026-09-13). No timed
child ran and no unit was accepted; the journal ends in terminal `failed`.

Preserved here: the complete `execution.log`, the build source records, and
SHA-256 digests of the larger stage manifests, which stay in the `/tmp` stage.
The campaign is re-run as a fresh identity from the overnight benchmark queue.
