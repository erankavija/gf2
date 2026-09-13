# Timing the batch `cat-file` change

Commit `76812a4e6a0dd6ecf499dc98e8d3a3b052c50a31` replaces one `git cat-file
blob` process per committed-blob read with a single `git cat-file --batch`
process serving every read, in `dev/scripts/check-receipt-input-snapshots.py`
and in `dev/active/a203a23c/restore-receipt-inputs.py` alike. Its message
claims a wall time for each whole tool in each form. `measure.py` measures that
quantity and `timing.json` records it: the claim of record for those four
figures is `timing.json`, and this file states the sampling plan the record
follows.

## What is measured

The measured quantity is the wall time of a whole tool as a process, from start
to exit:

```
python3 -B <tool source of the form's revision> <the tool's arguments>
```

The per-file form is the source committed at `76812a4e`'s parent,
`a6427615adf607caca5e4adb3be917a20f18891f`; the batch form is the source
committed at `76812a4e` itself. Each form's source comes out of the object
database with `git show <revision>:<path>` into a scratch directory outside
this repository, so running a form never rewrites this repository's checkout.

Four series cover both tools in both forms:

- `checker_per_file`, `checker_batch` run
  `check-receipt-input-snapshots.py --revision 76812a4e...` from this
  repository's root. The checker reads committed objects only and writes
  nothing, so the working tree cannot reach either the verdict or the time.
- `restoration_per_file`, `restoration_batch` run
  `restore-receipt-inputs.py 8cfc015d...` from the root of a scratch clone.
  The whole process is timed, including the check, the digest index over every
  file the base revision tracks, the pinned-file read that assembles the pins,
  and the copy-and-stage tail that writes the restored files into the working
  tree and stages them.

## The scratch clone

The restoration generator copies files into its working tree and stages them,
so it runs against a `git clone --shared --no-checkout` of this repository
placed in a session scratch directory and detached at
`8cfc015de59f8e3accc51bca46d13d94b0bee267`. That is the revision the tool was
written against: the `base_revision` of
`dev/active/a203a23c/restored-inputs.json`, and the revision whose committed
receipts pin snapshot files the revision itself does not carry. Sharing the
object store means the clone reads this repository's own objects; the clone's
working tree and index absorb every write.

`measure.py` runs `git reset --hard` and `git clean -fdx` inside the clone
before each repetition and once after the last, so each repetition performs the
whole restoration and nothing the tool writes survives the series. The form's
two source files are placed into the clone after each reset, outside the timed
region, because `restore-receipt-inputs.py` imports the checker from its own
working tree: a form is both files of its revision together.

This repository is never written. The clone is never committed.

## Sampling plan

`measure.py` carries this plan as its `SAMPLING_PLAN` constant and `timing.json`
records it beside the samples.

- Repetitions: 5 per series, fixed before the run.
- Statistic of record: the minimum and the median of each series. Every
  repetition is reported alongside them.
- Stopping: none. Each series runs its five repetitions whatever they show, no
  repetition is dropped as an outlier, and the result is not inspected to
  decide whether to sample further.
- Series order, fixed: `checker_per_file`, `checker_batch`,
  `restoration_per_file`, `restoration_batch`. A series' repetitions run
  consecutively; forms are compared across series, not repetition by
  repetition.
- Warm-up: none. No repetition is discarded as a warm-up; the fixed series
  order gives each series the same preceding sequence of reads on every run.

## Provenance

`timing.json` records the source revision of each form, the workload revision,
the restoration base revision, the host (CPU model, core count, kernel,
frequency governors, SMT), the toolchain (`python3` and `git` versions), the
invocation, what the scratch clone holds, and the scheduling the run achieves
rather than the scheduling the wrapper requests: `ccx1-bench-flock.sh` asks for
`nice -n -5`, an unprivileged user gets `nice: cannot set niceness`, and the
`observed_niceness` field carries the niceness that applies. The host's load
average at the start of the run is recorded with it.

The `rng` field states that no random number generator takes part: the harness
draws no random numbers and fixes the workload, the arguments and the series
order as constants, and neither tool imports a random number generator in
either form. The checker's only randomness-bearing import, `tempfile`, is
reached from its `--self-test` path alone, which these invocations do not take.
Every repetition runs the same work, so the spread within a series is host
timing noise.

## Running it

```
./dev/scripts/ccx1-bench-flock.sh --full-host \
  python3 -B dev/active/a203a23c/timing/measure.py \
  --scratch <scratch-directory> --repetitions 5 \
  > dev/active/a203a23c/timing/timing.json
```

`--scratch` names any directory outside this repository; it holds the clone and
the extracted tool sources, and `timing.json` records its filesystem rather
than its path. `ccx1-bench-flock.sh --full-host` holds this host's benchmark
mutex exclusively without pinning to a core subset, so no other benchmark or
build on the host overlaps the run.
