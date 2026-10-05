# Family producing closures in the CI contract

`dev/scripts/check-family-producing-closures.py` runs the `--check` of every
live generator named `make-*-producing-inputs.py` whose source offers `--check`.
The shared locator `repository_files.tracked_files` finds the generators, so a
later family needs no CI edit and the CI script names no development-artifact
path. `scripts/cargo-ci.sh` runs the checker and its self-test beside the shared
closure steps. The check compiles nothing and writes nothing in the repository;
children run with `-B`.

## Evidence

The self-test stages three fixture families in a temporary git repository: one
current, one with a source added to its enumerated directory, one with a source
removed. The check accepts the current family and reports exactly the other two,
each as `closure is not the closure of this tree`. A tree with no checkable
generator is rejected.

On the committed tree `python3 dev/scripts/check-family-producing-closures.py`
exits 0 for the dense and logical families of story 2037941f.

## Unchecked generators

The generators of the 1d4fd63d (external) and c04dd4ac (shift) families offer no
`--check` and write their output when run, so the check skips them.
