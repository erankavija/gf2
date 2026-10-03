# Exception `harness-path-dependencies`

`harness-path-dependencies` is a named exception to
`@/inv/no-dev-path-coupling`.

## Scope

The exception covers Cargo `path` dependencies in survey harness manifests: a
tracked `Cargo.toml` of the active-development tree, outside `inputs/`
snapshot directories, that declares a dependency through a path relative to a
parent of its own directory. Each such path counts directory levels to a
workspace crate, to `tuning-campaign-support`, or to another harness crate.

This command, run from the directory that holds this record, lists every
manifest in scope:

```
git grep -l --full-name -E 'path *= *"\.\./' -- ':(glob)../**/Cargo.toml' ':(exclude,glob)../**/inputs/**'
```

Every other way a harness crate locates a development artifact stays under the
invariant.

## Rule for a move

A move of a harness crate, or of a directory one of its path dependencies
names, rewrites the path text in each affected manifest to the new depth and
rebuilds the crate:

```
./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path <manifest>
```

Snapshot copies under `inputs/` keep their bytes.

## Convergence condition

The exception ends when each harness crate builds from its location and from a
copy one directory level deeper with no manifest change, while the root
lockfile and every tracked harness lockfile resolve under `--locked`. Issue
58a4bb51 tracks it.

## Evaluated forms

`7bac1303-harness-path-trials.sh` evaluates three depth-independent forms on
Rust 1.95 in a scratch copy of `HEAD` and writes
`7bac1303-harness-path-trials.txt`, which states its toolchain and source
revision:

```
dev/active/fa787f85-documentation-overhaul/7bac1303-harness-path-trials.sh
```

Every resolution in the trials is this command, run from the root of the
scratch copy:

```
./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --offline [--locked] [--config .cargo/harness-paths.toml] --manifest-path <manifest>
```

Each result line of the output gives the manifest, the exit status, the
patches Cargo reports as unused, and the first error line. Resolution is
offline, so a manifest whose dependencies are absent from the host registry
cache fails in every section; the `# control` section gives the status of each
tracked lockfile in the unchanged repository, and its tally line is the
reference for the tallies below.

### Form 1: `[patch]` table in the repository `.cargo/config.toml`

A harness manifest declares each dependency by name and version requirement,
and the repository configuration maps each name to a path relative to the
repository root. The `# form 1` section shows the table and the `--locked`
resolution of every tracked lockfile with the harness manifests unchanged.

Rejected: Cargo reports each patched package absent from a dependency graph as
unused, records it as a `[[patch.unused]]` entry in that graph's lockfile, and
under `--locked` fails with `cannot update the lock file`. The `form 1:` tally
line and the per-manifest `unused=[...]` lists show the extent, the root
manifest included; the lockfile diff of the root manifest follows the tally.
Every entry added to the table changes those lockfiles again.

### Form 2: the same table passed with `--config`

The table lives in `.cargo/harness-paths.toml`, which Cargo reads only when a
command names it, and the harness manifests declare each dependency as
`version = "*"`. The `# form 2` section shows that the root manifest resolves
under `--locked` with no flag, that a harness manifest resolves with the flag
(`with-flag` lines; each failure there is a registry download refused
offline), and that it fails without the flag (`without-flag` lines).

Rejected: every cargo invocation on a harness manifest has to pass the flag,
and scripts that issue those invocations are digest-pinned. The section lists
each tracked script of the active tree whose text matches
`cargo[^#]*\<(build|run|test)\>`, its SHA-256, and the number of tracked JSON
files that hold that digest (`pinned_in`); a script with a nonzero count
changes its pinned identity when it gains the flag. The `with-flag` lines also
carry `unused=[...]` lists: harness lockfiles gain `[[patch.unused]]` entries
under this form as under form 1.

### Form 3: harness workspace at a fixed location

One workspace manifest at the root of the active-development tree declares the
dependencies under `[workspace.dependencies]`, and a harness manifest declares
each as `workspace = true`. The `# form 3` section lists the `[workspace]`
table of every harness manifest and runs three resolutions on one member.

Rejected on three constraints the section shows:

- A harness manifest that keeps its own `[workspace]` table is its own
  workspace root, and inheritance fails with
  `` `workspace.dependencies` was not defined ``.
- A member without that table resolves into the lockfile of the shared
  workspace, so the per-crate lockfiles stop selecting its dependency
  versions.
- A copy of the member one directory level deeper fails with
  `current package believes it's in a workspace when it's not`, because
  membership is a path in the workspace manifest.
