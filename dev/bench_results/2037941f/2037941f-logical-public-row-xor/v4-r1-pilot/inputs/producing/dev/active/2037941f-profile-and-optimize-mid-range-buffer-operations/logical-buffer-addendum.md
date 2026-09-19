# Logical-buffer measurement addendum

> **Diátaxis Type:** Reference
>
> **Addendum identity:** `2037941f-logical-buffer-v1`
>
> **Owning protocol issue:** `3ea122df`
>
> **Frozen UTC:** `2026-09-15T03:56:39Z`

This addendum freezes the logical-buffer questions, cells, inputs, statistical
rules, and adoption limits for story `2037941f`. It specializes the
[Zen 3 measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and `zen3-benchmark-protocol` version 4 in
[`dev/active/f547c394/protocol.md`](../f547c394/protocol.md). A campaign JSON
addendum transcribes one family below into the version-4 schema; it may supply
content pins and a pilot-derived numeric resolution, but it may not change a
cell, margin, limit, or decision rule.

No sample in this document is a timing result. Exploratory and confirmatory
measurements begin only after the harness, executable identities, and applicable
schema-valid campaign addendum are committed.

## Operations and cost boundaries

The campaign uses four distinct questions. They never share a timing cell.

| Question | Baseline operation | Compared operation | Cost boundary |
|---|---|---|---|
| `isolated-xor` | A call to public `gf2_core::kernels::ops::xor_inplace` for every logical operation | A current resolved function or one bounded unroll candidate | In-place XOR only; dispatch is inside the public arm and resolution is outside the resolved arm. Allocation, fixture generation, reset, validation, and output observation stay outside timing. The resolved arm is exploratory attribution only, never a confirmatory candidate. |
| `public-row-xor` | One public `BitMatrix::row_xor(dst, src)` | The same route with a selected kernel candidate; no private row-layout shortcut | Row lookup, split, dispatch, and in-place XOR are inside timing. Matrix construction, fixture fill, validation, and reset stay outside timing. Results are reported in nanoseconds per public row XOR. |
| `nr-bg2-construction` | `QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, target_n, target_k)` | At most one consumer-specific resolver-hoist candidate inside `compute_mother_encoding` | The entire public constructor is timed: lifting selection, sparse mother-code creation, dense conversion, allocation, RREF row operations, column maps, and returned object. No prepared matrix or private RREF-only shortcut is comparable. |
| `isal-base-gap` | A fresh gf2 destination, a copy of source zero, and public `xor_inplace` for source one | ISA-L `xor_gen_base` with two sources, its fresh destination, and its `void *` array | Both arms include destination arrangement, the gf2 copy, ISA-L pointer-array construction, required alignment, conversion if any, dispatch, and output observation. Fixture generation and validation stay outside timing. |

The ISA-L specification is
[`isal-comparator.md`](isal-comparator.md). The external source is ISA-L tag
`v2.32.1`, commit `7c3479e0a9dac17f448603ec1ad64c7c625f530c`,
and the qualified scalar symbol is always labelled `xor_gen_base/scalar`.
The public dispatched `xor_gen` arm is **unavailable** in this addendum because
the qualifying host has no NASM-built multibinary executable or observed
dispatch route. Its unavailable row is retained with that reason and zero
samples. A later NASM installation does not silently enable it; admitting
`xor_gen` requires a newly qualified comparator and a versioned amendment.

All operations use two sources. Multi-source XOR is outside this addendum.
Source and destination never alias. ISA-L cells use only the aligned layout;
an unaligned ISA-L call is outside its qualified interface and is recorded as
inapplicable rather than attempted.

## Selected production route

The primary whole-consumer route is
`QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 49)`. The public constructor
selects BG2 lifting factor $Z=9$, builds a $378 \times 468$ parity-check matrix,
and converts it to dense rows whose stride is

$$
\left\lceil \frac{468}{64} \right\rceil = 8\ \text{words}.
$$

`compute_mother_encoding` repeatedly calls public `BitMatrix::row_xor` in its
right-to-left RREF loops. The call therefore resolves the logical backend for
each row operation. This is the only dispatch-hoist consumer admitted by this
addendum: a repeated, unhoisted, eight-word production loop including all
constructor work. The other documented NR targets below are non-regression
controls for the same source-level route.

| Cell suffix | Public constructor arguments `(BG, n, k)` | $Z$ | Dense dimensions | Stride words | Objective |
|---|---:|---:|---:|---:|---|
| `bg2-256-49-z9-8w` | `(2, 256, 49)` | 9 | $378 \times 468$ | 8 | improvement, primary |
| `bg2-256-121-z22-18w` | `(2, 256, 121)` | 22 | $924 \times 1144$ | 18 | non-regression |
| `bg2-625-225-z30-25w` | `(2, 625, 225)` | 30 | $1260 \times 1560$ | 25 | non-regression |
| `bg1-1024-640-z30-32w` | `(1, 1024, 640)` | 30 | $1380 \times 2040$ | 32 | non-regression |
| `bg2-1024-441-z56-46w` | `(2, 1024, 441)` | 56 | $2352 \times 2912$ | 46 | non-regression |

The harness verifies every stated $Z$, dimension, and stride from the returned
public objects before enabling timing. A mismatch makes the cell unavailable;
it never substitutes another target.

## Frozen sizes and cell matrices

The anchor word counts are exactly $W \in \{8,9,63,64,65\}$. The neighboring
counts $W=7$ and $W=66$ bound the conservative eight-word cutover and the
story's upper edge. The anchor cells, not the neighbors, form confirmatory
multiple-comparison families. Neighbor, offset, partial-tail, and streaming
cells remain exploratory until a later final-integration addendum names a
smaller holdout matrix before seeing its samples.

### Isolated XOR

The normative primary Cartesian product is:

- word count: `8`, `9`, `63`, `64`, or `65`;
- layout: `a64`, with each source and destination beginning at address
  $0 \pmod {64}$;
- cache state: `warm`;
- metric/scaling/core: `kernel-isolated`, `single-core-latency`, `single-core`;
- cell identifier: `xor-{W}w-a64-warm`.

The exact exploratory matrix adds:

- `xor-7w-a64-warm` and `xor-66w-a64-warm`;
- `xor-{W}w-o8-warm` for every anchor, where both non-overlapping views begin
  eight bytes after separate 64-byte-aligned slab bases;
- `xor-{W}w-a64-streaming` for every anchor.

Each operation consumes exactly $W$ words. The public and resolved arms see
the same fixture bytes and alignment. The pre-candidate baseline uses a
byte-identical public identity arm to retain the current absolute median.
Resolved-function attribution is limited to $W=8$ and $W=9$ exploratory rows;
it does not remeasure the preserved 64-word hoist question. A selected unroll
candidate replaces only the identity compared arm and remains behind the same
public dispatch in its candidate build.

### Public row XOR

The normative primary Cartesian product is:

- word count: `8`, `9`, `63`, `64`, or `65`;
- matrix shape: 64 rows and exactly $64W$ columns;
- row-pair cycle: `(dst, src)` is `(1,0)`, `(0,1)`, `(17,16)`, `(16,17)`,
  `(33,32)`, `(32,33)`, `(49,48)`, then `(48,49)`;
- cache state: `warm`;
- metric/scaling/core: `kernel-isolated`, `single-core-latency`, `single-core`;
- cell identifier: `row-xor-{W}w-full-warm`.

The exact exploratory matrix adds:

- `row-xor-7w-full-warm` and `row-xor-66w-full-warm`;
- `row-xor-{W}w-tail63-warm` for every anchor, with
  $64(W-1)+63$ logical columns and one zero padding bit;
- `row-xor-{W}w-full-streaming` for every anchor.

One reported operation is one public `row_xor`, even though a child cycles
through eight directed row pairs. The arm reports the allocation base alignment
and the source and destination addresses modulo 64 for every pair. The alignment
policy is the unmodified production `BitMatrix` allocation plus the declared
row indices; the harness may observe it but may not copy rows into an aligned
private representation.

The pre-candidate row campaign uses a byte-identical public identity arm. The
pre-candidate NR campaign constructs the same public code in both arms. These
identity comparisons retain current absolute medians and size resolution; they
make no selection claim. Candidate campaigns replace only the compared build.

### NR construction and ISA-L

The five NR rows in the selected-route table are the complete confirmatory
family. Their cache state is `warm`, their metric is `whole-consumer`, and
their scaling/core is `single-core-latency`/`single-core`. The exploratory
matrix repeats the primary eight-word row once with `cold` first-use semantics.
Cell identifiers are `nr-construct-{suffix}-{cache}`.

The ISA-L scalar-gap primary family uses the five anchor word counts, `a64`,
`warm`, `kernel-isolated`, and `single-core-latency`; its identifiers are
`isal-base-gap-{W}w-a64-warm`. Exploratory rows add $W=7$, $W=66$, and the
five anchor `streaming` cells. Every receipt also carries the unavailable
`isal-dispatched-xor-gen` row with the qualified NASM-absence reason and zero
samples. That declared unavailable row is retained with its reason and zero
samples; it spends no comparison in a confirmatory reservation (Amendment 1).

The 6-physical-core, 12-physical-core, and 24-logical-CPU arms are inapplicable
to these serial APIs. Receipts retain each inapplicable arm and its reason;
they do not fabricate worker counts or duplicate single-core samples.

## Input identities and correctness

The campaign seed is `0x2037_941f_3ea1_22df`. Cells are ordered first by the
question order in this document, then by the order of each listed axis. Cell
ordinal zero uses the campaign seed; ordinal $i$ uses the $i$-th subsequent
SplitMix64 output as its workload seed. Each additional fixture bank consumes
the next SplitMix64 output from that cell's stream. The implementation is the
canonical `tuning_campaign_support::abtest::SplitMix64`; substituting another
generator or hashing cell-name text is not permitted.

For XOR and row cells, SplitMix64 fills words in canonical row-major order.
Source and destination streams are independent consecutive outputs. The
partial final word is masked immediately after generation and after every
operation. Repeating an in-place operation toggles the destination; validation
tracks the call parity rather than resetting data inside a timed window.

NR cells have no random matrix input: the `(BG, n, k)` triple and the committed
3GPP tables are their input identity. The workload seed still fixes pair order
and bootstrap draws. The public returned object's parameters and a stable
digest of its sparse parity-check structure are observed outside timing and
must match between arms.

Before timed execution, every arm passes an untimed deterministic smoke over
logical bit lengths $0$, $1$, $63$, $64$, and $65$, all seven word counts,
both `a64` and `o8` layouts where supported, full and `tail63` matrices, the
four bidirectional row-pair groups, and resume after each cell boundary. The oracle
checks every output word, source immutability, canonical LSB-first bit indexing,
logical length, and zero tail padding. ISA-L additionally checks its fresh
destination, 32-byte alignment, pointer-array order, and byte/word result.
Smoke execution emits no timing samples and cannot serve as a pilot.

## Cache, warmup, and sampling

All builds finish before timing and use release mode. Measurements run on the
prepared Ryzen 9 5900X under the repository's full-host benchmark-window lock.
The runner records the observed CPU IDs, topology, affinity, SMT, governors,
clocks, capabilities, toolchain, executable digests, and selected routes.
The gf2 baseline and candidate are `conservative-portable` Rust 1.95 release
builds with the workspace's ordinary all-feature configuration and no
`target-cpu=native` override. The ISA-L arm is the separately pinned `external`
build with the compiler flags in its comparator specification. A tuned or
native gf2 build is a different arm and requires a versioned amendment.

Cache policies are exact:

- `warm`: initialize the fixture, then execute exactly one untimed pass of the
  measured operation over that cell's complete working set before calibration.
- `streaming`: create eight fixture banks of at least 8 MiB each, rounded up to
  an integral number of complete source/destination tuples or matrices. Touch
  every initialized byte outside timing, do not execute the measured operation
  as warmup, and rotate banks once per operation. The reported working set is
  at least 64 MiB. Rotation is the claim; no cache-level eviction is claimed.
- `cold`: use a fresh child and fresh constructor inputs, execute no measured
  operation before the first window, and fix `cold_calls` to one. It is a
  first-use series, not a hardware cache-miss-latency claim.

Every exploratory cell runs exactly 24 paired executions. Every confirmatory
cell runs exactly 24 fresh paired executions. Each pair launches adjacent
fresh baseline and compared-arm children in the protocol's seed-determined,
two-pair-counterbalanced order. Each warm or streaming execution uses five
windows targeted at 100 ms after calibration; a cold execution uses five
one-call windows. No adaptive sample extension, early significance stop, or
reuse of exploratory pairs is allowed. The child timeout is 120 seconds.

The outlier policy removes nothing. A window at least twice its execution
median is flagged; a flagged fraction above 0.10 makes the cell unstable and
non-qualifying. The attempt and search budgets remain spent.

## Estimator, confidence, and multiple comparisons

The resampling unit is one whole paired execution. For baseline medians $b_i$
and compared-arm medians $c_i$, the estimator is

$$
\hat{s}=\frac{\operatorname{median}_i b_i}
               {\operatorname{median}_i c_i},
$$

so $\hat{s}>1$ favors the compared arm. The interval is the protocol-v4
nearest-rank percentile bootstrap of 10,000 whole-pair resamples, using its
xoshiro256** stream seeded through SplitMix64. The family error rate is 0.05.
Attempt $t$ spends $\alpha_t=0.05/[t(t+1)]$ and Bonferroni uses
$\alpha_c=\alpha_t/m$, where the append-only ledger supplies $t$ and the
cumulative reserved comparison count $m$.

Each canonical question owns a distinct ledger and family identity. Candidate
selection families set `holdout.required` to false; final integration freezes
its own holdout family before observing after-change samples.

| Family identity | Canonical ledger | Confirmatory cells | Maximum $m$ in its first confirmation |
|---|---|---:|---:|
| `2037941f-logical-isolated-xor` | `dev/bench_results/2037941f/logical-isolated-xor-ledger.jsonl` | five anchor isolated cells | 5 |
| `2037941f-logical-public-row-xor` | `dev/bench_results/2037941f/logical-public-row-xor-ledger.jsonl` | five anchor row cells | 5 |
| `2037941f-logical-nr-construction` | `dev/bench_results/2037941f/logical-nr-construction-ledger.jsonl` | five selected-route/control cells | 5 |
| `2037941f-logical-isal-base-gap` | `dev/bench_results/2037941f/logical-isal-base-gap-ledger.jsonl` | five scalar-gap cells | 5 |

Candidate pilots within one question remain in that question's ledger and
spend zero comparisons. Candidate selection occurs only among those pilots;
the selected identity receives the family's single confirmatory reservation.
At $t=1$ and $m=5$ for all four families, $\alpha_c=0.005$, leaving 25
expected bootstrap draws in each tail; the ISA-L family reaches this $m$ and
$\alpha_c$ by amendment (Amendment 1). A second confirmation in the
same family is forbidden both by the one-attempt rule and because sequential
error spending can violate the protocol's tail-support check. Final integration
uses a separately frozen `final-integration` family because it asks the new
production-route question, not a renamed retry of candidate selection.

The decision interval $[\ell,u]$ follows protocol version 4: `improved` when
$\ell$ reaches the worthwhile or material-gap threshold, `not-worse` when
$\ell \ge 1/\theta_{\mathrm{eq}}`, `regressed` when
$u < 1/\theta_{\mathrm{eq}}$, and `inconclusive` otherwise. Negative,
not-material, unstable, inconclusive, unavailable, and inapplicable rows stay
in the receipt.

## Effect, resolution, and complexity rules

The pilot-derived resolution $r$ is the largest relative bootstrap half-width
over the five primary cells at their corrected $\alpha_c$. It is rounded
upward to the next 0.001 and pinned with the pilot receipt and digest in the
confirmatory JSON addendum. A second independent deterministic bootstrap must
also fit $r$, as protocol version 4 requires. If $r$ exceeds the family ceiling
below, the family records `resolution-insufficient` and runs no confirmation.
Margins are fixed here and strictly exceed one plus every admitted $r$.

| Family | Worthwhile speedup | Equivalence margin | Resolution ceiling | Complexity budget and rationale |
|---|---:|---:|---:|---|
| isolated XOR | 1.05 | 1.03 | 0.020 | At most one new private target-feature XOR function with its explicit safety contract, 80 added nonblank production lines, one existing unsafe-kernel file, and no allocation, dependency, public API, or second dispatcher. Five percent is the smallest isolated gain worth carrying into consumers; isolated evidence never authorizes adoption by itself. |
| public row XOR | 1.03 | 1.02 | 0.015 | Shares the isolated candidate's code budget and permits no row-layout shortcut or extra production lines. Three percent in the public row operation is the smallest consumer-visible signal worth confirming. |
| NR construction | 1.02 | 1.015 | 0.010 | One resolver-hoist candidate, at most 32 added nonblank production lines in the existing NR construction module, no unsafe code, dependency, allocation, public API, semantic type, or alternate matrix abstraction. Two percent of the whole constructor is worthwhile because maintenance is one existing resolver plus one fixed-width loop binding. |
| ISA-L scalar gap | not an adoption objective | 1.05 | 0.030 | `material_gap_threshold = 1.10`; zero production lines and zero unsafe kernels. A ten-percent external gap warrants attribution, not selection. The scalar result cannot stand in for dispatched ISA-L. |

A logical-kernel adoption requires both of these results under predeclared
dispatch bands: every isolated anchor in a selected band is `improved`, and
every matching public-row anchor is `improved`; anchors immediately outside
the band are `not-worse`. The allowed selection is the `{8,9}` band, the
`{63,64,65}` band, or both bands. The candidate changes the kernel body, not
the per-call public dispatch contract. A kernel that fails a public-row
condition remains an isolated result.

The NR resolver-hoist candidate is independently adoptable only when the
eight-word primary constructor is `improved` and all four other NR constructors
are `not-worse`. Its isolated eight-word resolved-function result is attribution,
not a substitute for whole-constructor benefit. Combined production adoption
may add at most 112 nonblank source lines and touch only the existing logical
kernel file plus the NR construction module; it inherits every stricter family
limit.

## Search and stopping rules

The frozen search budget is:

- at most three isolated unroll identities, selected from factors 2, 4, and 8;
- at most one NR consumer-specific resolver-hoist identity;
- at most four exploratory pilot trials per cell across the identity baseline
  or resolved attribution and all candidate identities;
- exactly one confirmatory attempt for the selected identity and protocol
  version;
- no production change selected solely from ISA-L or isolated throughput.

Profiles may eliminate candidates before a pilot and preserve the reason. They
may not introduce another factor, a generic threshold edit, a new borrowed-word
view, a coding-private kernel, or an unlisted consumer. Search stops when each
budgeted identity has one completed pilot, when a profile rules out the
remaining identities, or when the per-cell pilot cap is reached, whichever
comes first. An unstable or inconclusive confirmation is terminal for that
identity and protocol version. Sampling never expands to chase a margin.

## Preserved no-win and scope exclusions

The confirmed 64-word generic dispatch-hoist no-win remains authoritative:
the predecessor profile records the hoist as not material on 64-word rows
([`dev/active/04b85d10/findings.md:100-110`](../04b85d10/findings.md#L100-L110),
tables § `logical-v3-confirmation`, row `logical-row-xor-dispatch-64w-1core` in
[`dev/bench_results/04b85d10/tables.md`](../../bench_results/04b85d10/tables.md)).
This addendum therefore forbids a candidate that changes public `xor_inplace`,
public `BitMatrix::row_xor`, or the global size threshold merely to hoist
dispatch for a 64-word call. It also excludes already-hoisted M4RM, RREF,
Gauss inversion, Strassen, and dense-multiply loops.

The only admitted hoist is the source-demonstrated NR construction contract:
many unhoisted public row calls at the selected eight-word stride inside one
whole constructor. If its whole-consumer cell does not meet the frozen rule,
the current route remains selected and the no-win is retained. A materially
different future consumer needs a new versioned addendum; it cannot reuse these
samples or reopen the generic 64-word result.

`BitSlice` remains a bit-access view. This addendum creates no borrowed-word
API and names no zero-copy production candidate. The downstream private
zero-copy selection may close as no-candidate; any proposed cell not already
listed here requires a versioned protocol amendment before pilot work, and any
public/shared representation change requires a separately authorized
container.

## Amendments

### Amendment 1 (2026-09-16, issue `b9302771`) — ISA-L family reservation count

This addendum froze the ISA-L scalar-gap family at $m=6$ and a corrected
$\alpha_c=0.05\,/\,(1\cdot2\cdot6)=0.0041\overline{6}$: five scalar-gap
confirmatory cells plus the unavailable `isal-dispatched-xor-gen` row, which
this addendum said spent a comparison although it produces no pair.
`tuning_campaign_support::trial_ledger::reserve` counts only non-exploratory
cells and reserves five, so the frozen $m=6$ could never be reserved and the
family's confirmation (`65c0e13d`) could not start. The invoker ruled on
2026-09-16 that this closed addendum is amended to $m=5$ and
$\alpha_c=0.005$ rather than changing the protocol or the ledger mechanism.
The unavailable row stays in every receipt with its reason and zero samples;
it spends no comparison. No other family, cell, seed, margin, or budget
changes.

## Receipt and stopping evidence

Every campaign snapshots this document, the measurement contract, protocol,
schema, campaign JSON, producing-source closure, executable, toolchain, and
inputs by SHA-256 before opening its append-only log. The log path is printed
before the first bounded run, and completed cells resume without repetition.
Receipts retain exact commands, raw pairs and windows, seeds, cache claims,
observed routes and alignment, fixture-bank sizes, output validation, assembly,
profiles, host facts, statistical decisions, and every negative or unavailable
row.

A current pre-change baseline and a fresh after build are mandatory for any
production adoption. Navigation commits and unrelated worktree changes are
informational; only producing content identities decide validity. No finite
search is reported as an absolute optimum.
