# Summarizer equivalence on the committed profile series (jit:859bc8cb)

`reproduce-summaries.sh` compares the profile summarizer
`dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py`
as committed on `main` with the working-tree version. Its series are the
committed directories carrying an `attribution-chains.json`, enumerated with
`git ls-files`. Each series is copied into a scratch repository per summarizer,
summarized there, and every file of the two output trees is compared with
`cmp`; the file sets must also match. The pinned profiling executable is absent
from the host, so both summarizers reuse the committed `attribution-chains.json`.

```
dev/active/1a379447-zen3-cpu-performance/859bc8cb/reproduce-summaries.sh
```

The script prints one `series<TAB>verdict` line per series and exits non-zero
when any verdict is `different`. Its observed output on this host:

```
dev/bench_results/07ca8585/v4-r1-resampled-profile	identical
dev/bench_results/3be770d5/v3-r1-steady-profile	identical
dev/bench_results/3be770d5/v4-r1-steady-profile	identical
```

The comparison is between the two summarizers on the same inputs and host. A
byte-for-byte regeneration of the committed summaries is not part of it.
