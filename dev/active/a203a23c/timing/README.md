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
database with `git show <revision>:<path>` into a git-ignored scratch
directory, so running a form never rewrites this repository's checkout.

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
placed in a git-ignored directory under `target/` and detached at
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

Placing the clone under `target/` puts it on the filesystem the repository sits
on, which is the point of the placement rather than a convenience: the
restoration generator's real working tree is this repository's own, so a clone
on a different filesystem would fold a filesystem difference into the wall
times of the series that writes. `timing.json` records the mount source, the
filesystem type and the device id of both the repository and the scratch, and
that the two share a device, under `scratch.filesystem`. `measure.py` refuses a
scratch inside the repository that git does not ignore.

Nothing this repository tracks is written. The clone is never committed.

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

## Result

`timing.json` carries the samples and the statistics of record. A series' five
wall times are at `series.<name>.repetitions_s`, its minimum at
`series.<name>.min_s` and its median at `series.<name>.median_s`, for each of
`checker_per_file`, `checker_batch`, `restoration_per_file` and
`restoration_batch`. Every series also records the process it runs, its working
directory, its workload, the source revision of its form, and the exit code of
every repetition at `series.<name>.exit_codes`, all of which are 0. The two
restoration series agree on the work performed: the counts of restored and
unrestored pinned files their last repetition prints are at
`series.<name>.final_stdout`.

`published_claim.figures.<name>` sets each figure of commit `76812a4e`'s
message beside the median this record measures for it, as `published_s`,
`measured_median_s`, their ratio, and `confirmed_within_tolerance`, which holds
when the median lies within 10 % of the published figure. Three of the four
figures are confirmed: `checker_per_file`, `checker_batch` and
`restoration_per_file`. The batch restoration figure is not.

### Superseded observation

Commit `76812a4e`'s message states that "the repository check falls from 23.1 s
to 2.6 s and the restoration from 60.2 s to 10.1 s". The 10.1 s of that
sentence rests on an observation that was never committed, and the whole-process
median at `series.restoration_batch.median_s` falls well below it; their ratio
is at `published_claim.figures.restoration_batch.median_over_published`. The
10.1 s figure is preserved here and at
`published_claim.figures.restoration_batch.published_s` as that superseded
uncommitted observation. `timing.json` is the claim of record: the batch form
of the restoration generator is faster than the commit message reports, and the
direction and the order of magnitude of the reduction the commit claims hold.

The gap is not an artefact of where the scratch clone sits. The superseded
tmpfs run reaches the same verdict on the same series, at
`timing-tmpfs-superseded.json`'s `series.restoration_batch.median_s` and
`published_claim.figures.restoration_batch.confirmed_within_tolerance`, and its
median differs from the on-disk median by well under the distance either one
stands from 10.1 s.

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
  --scratch target/a203a23c-scratch --repetitions 5 \
  > dev/active/a203a23c/timing/timing.json
```

`--scratch` holds the clone and the extracted tool sources; `target/` is
git-ignored and on the repository's own filesystem, and `timing.json` records
that filesystem rather than the path. `ccx1-bench-flock.sh --full-host` holds
this host's benchmark mutex exclusively without pinning to a core subset, so no
other benchmark or build on the host overlaps the run.

`timing-tmpfs-superseded.json` is an earlier run of the same harness under the
same sampling plan whose scratch clone sat on a tmpfs instead, so its
restoration series wrote to memory while the tool's real working tree writes to
disk. It is superseded for that reason and kept as the record it is;
`timing.json` is the claim of record.
