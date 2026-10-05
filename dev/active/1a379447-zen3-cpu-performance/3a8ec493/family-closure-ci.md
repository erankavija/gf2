# Family producing closures in the CI contract

`check-family-producing-closures.py` runs the `--check` of every
live generator named `make-*-producing-inputs.py` whose source offers `--check`.
The shared locator `repository_files.tracked_files` finds the generators, so a
later family needs no CI edit. `scripts/cargo-ci.sh` finds `repository_files.py`
by name through git, asks it for the checker's location, and runs the checker
and its self-test with `python3 -B`; the CI script names no development-artifact
path. The check compiles nothing and writes nothing in the repository: the steps
and the generators they spawn run with `-B`, so no bytecode directory appears.

## Evidence

The self-test stages three fixture families in a temporary git repository: one
current, one with a source added to its enumerated directory, one with a source
removed. The check accepts the current family and reports exactly the other two,
each as `closure is not the closure of this tree`. A tree with no checkable
generator is rejected.

From a tree with no bytecode directory, `git status --porcelain --ignored` is
identical before and after both steps. On the committed tree the checker exits 0 for the dense and logical families of story 2037941f.

## Unchecked generators

The generators of the 1d4fd63d (external) and c04dd4ac (shift) families offer no
`--check` and write their output when run, so the check skips them.
