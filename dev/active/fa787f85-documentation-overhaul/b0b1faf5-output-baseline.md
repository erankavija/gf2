# b0b1faf5 output baseline

Outputs of the four survey scripts that read the shared producing manifest,
before and after they locate it through `repository_files.py`: the manifest
beside the live protocol documents, byte-identical copies counted as one. Raw
captures are committed beside this record.

Baseline source: `edf3249f05fe5d1024cd31e6a23ea9c1100b595a`, whose four scripts
are those of its parent `5e300cc92a79dbc7a2b054a181af443169b6bfa8`, the tree
the baseline run copied. After source:
`18d31dec9769b78625393130c08b698a79c32af2`.

## Command

Run from the repository root at each source state. `R` is this record's
directory:

```
R=$(dirname "$(git ls-files ':(glob)**/b0b1faf5-output-baseline.md')")
CARGO_CI_NO_SCCACHE=1 "$R/b0b1faf5-outputs.sh" <capture-directory> 5e300cc92a79dbc7a2b054a181af443169b6bfa8
```

The header of `b0b1faf5-outputs.sh` states the scratch copy, the fixture
arguments, the committed inputs and the values a capture replaces. Each capture
gives the command line of one script, its exit status, stdout, stderr and the
SHA-256 of every file the run writes.

## Raw outputs

| State | Captures |
| --- | --- |
| Baseline | `b0b1faf5-before/` |
| After | `b0b1faf5-after/` |

## Equality

The captures are identical; this command prints nothing:

```
diff -r "$R/b0b1faf5-before" "$R/b0b1faf5-after"
```

## Exception `evidence-directory-paths`

`evidence-directory-paths` is a named exception to
`@/inv/no-dev-path-coupling`.

### Scope

The exception covers literal `dev/bench_results/` paths in the survey scripts
that locate the shared producing manifest through `repository_files.py`. Each
such path names an evidence directory, a source or input archive in one, or a
campaign launcher; none of these declares an identity in its bytes, and the
scripts write the paths into the build identities and producing manifests they
record.

This command, run from the repository root, lists every literal in scope:

```
git grep -n 'dev/bench_results' -- $(git grep -l 'repository_files\.py' -- ':(glob)**/survey/*.py' ':(exclude,glob)**/inputs/**')
```

Every other way these scripts locate a development artifact stays under the
invariant: the shared producing manifest, a harness crate and the script's own
directory resolve at runtime.

### Rule for a move

A move of an evidence directory, archive or launcher a literal names rewrites
the literal in each script the command lists and re-runs the comparison of
this record: the captures of `b0b1faf5-outputs.sh` before and after the
rewrite differ only in the moved path. Snapshot copies under `inputs/` keep
their bytes.

### Convergence condition

The exception ends when the listing command prints nothing while each script
still reproduces its capture. Issue 10f23a84 tracks it.
