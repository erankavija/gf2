# 18858de1 root resolution

`run-window.sh` and `follow-window.sh` of the benchmark-window runner resolve
the repository root with `git rev-parse --show-toplevel` from their own
directory. `--print-root` prints that root and exits before the window opens:
it precedes the `GF2_BENCH_WINDOW` export, the state directory and the lock.

Source: `456e24fe3c6df031b7b5840b9e931e78eedbe5e5`.

## Command

Run from the repository root:

```
dev/active/fa787f85-documentation-overhaul/18858de1-root-resolution.sh
```

The [script](18858de1-root-resolution.sh) clones the invoking checkout's HEAD
into the git-ignored `target/` directory, invokes each runner script of the
clone with `--print-root` from working directory `/`, fails unless the printed
root equals the clone path, and removes the clone on exit.

## Raw output

[18858de1-root-resolution.txt](18858de1-root-resolution.txt) holds the output.
Paths are relative to the invoking checkout's root, and each `$` line is the
invocation with `<clone>` standing for the clone path on the first line. Each
script prints the clone path; the last line reports whether the invocation
left window state in the clone.
