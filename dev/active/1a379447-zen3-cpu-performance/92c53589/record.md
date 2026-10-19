# Categories of the 07ca8585 re-sampled profile (jit:92c53589)

## Executable identity and spans

The series `dev/bench_results/07ca8585/v4-r1-resampled-profile` pins its
`ldpc-profile` executable by digest on the identity line of `host.txt` and in
the `resolution` of `attribution-chains.json`. The `profile_resample` entry of
`dev/bench_results/07ca8585/preparation/kernel-build-identity.json` records
that executable as the `after` generation of the `3be770d5` harness, and
`generations.after` of `preparation/build-identity.json` beside it records that
generation's source revision, `20a3c7b195e25b70a18ba7a8d9d571a1d9a30335`, with
the same `ldpc-alloc-census` digest. The cached inline chains agree with that
revision: every `core.rs` and `min_sum.rs` location a chain assigns to a named
function lies inside that function at `20a3c7b1`. The kernel build's revision
`3f7b3dce` differs from it in `crates/gf2-coding/src/ldpc/` by doc comments
that move the `core.rs` functions, and most of those locations fall outside
their named function there.

`UPDATE_RULES` in
`dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py`
holds the spans of `20a3c7b1`: the layout of `BATCH_RULES` (`min_sum.rs` and the
`core.rs` update, variable, syndrome, reset and decode-loop functions with the
decode loop's call sites) at that revision's lines, and the harness paths of
that revision. `RULES_BY_EXECUTABLE` selects a series' rules by the executable
digest its resolution records, or for a resolution without one, by the build
identity digest the summarizer verified; the self-test categorizes one chain of
one executable name under two digests and stops on a digest without spans.

## Regenerated summary

`dev/bench_results/07ca8585/v4-r1-resampled-profile/own-source-spans/` holds
`profile.md`, `profile-summary.json` and `attribution.tsv` categorized with
`UPDATE_RULES` from the committed chains;
[summarize-own-spans.sh](summarize-own-spans.sh) writes them and reproduces
them byte for byte. They supersede the files of the same names at the series
root, whose categories come from the spans of the `3be770d5` executable and
whose bytes stand.

`dev/active/1a379447-zen3-cpu-performance/859bc8cb/reproduce-summaries.sh
91584ca79` prints `identical` for every other committed series and `different`
for this one.

## Conclusions that cite the profile's shares

| Statement | Verdict | Evidence |
|---|---|---|
| [07ca8585 findings](../07ca8585/findings.md), "The residual gap": the gf2 record cases carry no `edge-position-search`, `min-sum-input-vec`, `min-sum-dispatch` or `min-sum-reduction` category | Contradicted for `min-sum-reduction`, which every gf2 record case carries; confirmed for the other three | Category tables of `own-source-spans/profile.md` |
| Same paragraph: `check-node-loop` is the dominant named decoder work that remains | Contradicted for `gf2-dvb-l24`, where `variable-node-update` is the largest category with a Wilson interval disjoint from `check-node-loop`'s; at the other three gf2 cases `check-node-loop` is the largest single category, below half the samples. `check-node-loop` and `min-sum-reduction` together, the whole check-node update, hold the majority in every gf2 case | Category tables of `own-source-spans/profile.md` |
| Same paragraph: the sampled categories of the levers the issue spends disappear | Contradicted for the shared reduction (`min-sum-reduction`) and the syndrome and termination work (`syndrome-termination`), both present in every gf2 record case; confirmed for `edge-position-search`, `min-sum-input-vec` and `min-sum-dispatch`. The `allocator` samples at twenty-four workers are present in both summaries and are unchanged by the categorization | Category tables of `own-source-spans/profile.md` and of the series root's `profile.md` |
| [ed3d490e plan](../../ed3d490e-zen3-ldpc-frame-simd/plan.md): the re-sampled profile leaves the shared check-node loop as the dominant named decoder work | Contradicted as the second row states | Category tables of `own-source-spans/profile.md` |

`dev/bench_results/07ca8585/tables.md` cites no share of the profile. Each
contradicted statement keeps its text and carries a pointer to this record.
