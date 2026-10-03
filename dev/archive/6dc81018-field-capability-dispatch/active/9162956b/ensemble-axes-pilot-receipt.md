# Verification receipt: the coverage-repairing ensemble axes

This records the build-time study and the three timed pilots behind the amended
ensemble axes of issue `9162956b`, predeclared in
[`layout-attribution-verdict-v2.md`](/dev/benchmarks/tuning_profiles/layout-attribution-verdict-v2.md).
Every figure that document states about the amended axes comes from the runs
below.

**This receipt establishes no verdict.** It builds and measures the *reference*
revision only. v1 §6.5's audit needs both arms, so it is run on nothing here,
and no verdict ratio and no decorrelation reading exists. What this receipt
decides is which axes the ensemble carries.

The three standing receipts and the two standing plans stand as taken. None is
modified, superseded, re-run or adjusted here, and
[`layout-attribution-verdict-v1.md`](/dev/benchmarks/tuning_profiles/layout-attribution-verdict-v1.md)
loses no line to it.

## Provenance

| Item | Value |
|---|---|
| Revision built and measured | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the reference revision of v1 §4 |
| Checkout | `.agents/worktrees/control-0c072d73`, detached, reused read-only; `git status --porcelain --untracked-files=all` empty at the start and the end of both timed sessions |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, invoked as `cargo +1.95.0` |
| Target | `-p gf2-core --features simd --bench selector_non_regression` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64` |
| Timed protocol | `dev/scripts/ccx1-bench-flock.sh`, `GF2_BENCH=1`, CPUs 6–11, governor `powersave`, `--target-ms 250`, one execution of one repetition per build, thirty-four pinned cells, schema token `selector-non-regression-v1` |
| Pilot 1 protocol | [`ensemble-axes-pilot-protocol.md`](ensemble-axes-pilot-protocol.md), SHA-256 `f2d4359c515d5c2f277f952ae4d59e92f1ece274418d89564a5392d34b8a7fe0`, written 2026-08-21T09:51Z, before the pilot's first timed window at 10:30Z |
| Pilot 2 protocol | [`ensemble-axes-pilot-2-protocol.md`](ensemble-axes-pilot-2-protocol.md), SHA-256 `5ed1aadd37c2fbbfbbe0dab76902df0016abacc94ee61edfb8fc1e4acbc43c7b`, written 2026-08-21T11:02Z, before the second pilot's first timed window at 12:05Z |
| Pilot 3 protocol | [`ensemble-axes-pilot-3-protocol.md`](ensemble-axes-pilot-3-protocol.md), SHA-256 `86747559b4f1c924e4e7dc7e087f423e43158916ea26a23ab64f02c75b9156cf`, written 2026-08-21T13:04:35Z, before the third pilot's first timed window at 13:04:58Z |

**Raw files.** Each group writes one CSV, appended across its executions, to an
absolute `/tmp` path checked absent beforehand, then copied byte-for-byte into
this directory. Every row carries `source_dirty=false` and revision
`0c072d73ca65cf50af98b8c4b61ed876f8218df6`.

| File | SHA-256 |
|---|---|
| [`pilot-1-v1.csv`](pilot-1-v1.csv) | `a270034944479a5f3bc7ea0710c8b623d9583ff8b632d05830cbd41382c194ca` |
| [`pilot-1-sh.csv`](pilot-1-sh.csv) | `5fb4aeaa3a3dbb77a2a5eec1a945df7b2128dbe7169bb1c215c9feae412f0889` |
| [`pilot-1-v2.csv`](pilot-1-v2.csv) | `b9dd7cfbad1a1a226c71f3f58ed8c0efa0009490f74bec5acd57582616c72d01` |
| [`pilot-2-ens.csv`](pilot-2-ens.csv) | `b3e2107411f2c42fdef475e57c4dcd623e1d3a7042cb8f2e56bcda623a2e5238` |
| [`pilot-2-ctl.csv`](pilot-2-ctl.csv) | `f6eb11e5261f2c3b9836c7ce927fbf8f25688c200c09ea3cf9954310465cc96a` |
| [`pilot-2-fix.csv`](pilot-2-fix.csv) | `8e4d43019bf06a68cc829b400878c62f7014544462ae69b839e31dec4ac82428` |
| [`pilot-3-ens.csv`](pilot-3-ens.csv) | `fd1ea024ecc1bf225133000d162d315a97c012a8d9447cb9d86b7d958a1da7f5` |
| [`pilot-3-ctl.csv`](pilot-3-ctl.csv) | `f99364f3ba0e56a3eaf98e4255f4439c073101e2ab8d9635e27d4efe59e39fb7` |
| [`pilot-3-fix.csv`](pilot-3-fix.csv) | `70271ab3145ae79c21790a627b0d1767382040c5ab06c22fbd562d090c968972` |
| [`pilot-3-member-provenance.tsv`](pilot-3-member-provenance.tsv) | `4dc5c75ceedf955238ed782ac3eefe7e351c5cef85ae6e5e89714e2fb5ccf1fb` |

**The build pipeline reproduces the committed evidence.** The ordinary build of
the reference revision hashes to
`c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, which is the
reference arm's member 0 in receipt 3 and the control build of both earlier
sessions. All thirty-two members of pilot 1's Group V1 are bit-identical to the
same members of receipt 3's committed reference arm. Nothing in the pipeline
below changes what a member of v1's ensemble is.

**The target directory does not enter the binary.** The same ordinary build
reproduces the same SHA-256 from a target directory outside the checkout, so the
pilots build into a scratch directory on `/tmp` and delete it between members.
No build wrote to either checkout's `target/`, and the run consumed no disk
below its 10 GiB floor.

## 1. Estimators, validated before use

Both statistics the pilots report are computed outside the bench target, so both
are validated against the committed evidence first. Applied to
[`2026-08-20-ensemble-reference-arm.csv`](/dev/benchmarks/tuning_profiles/2026-08-20-ensemble-reference-arm.csv):

- `s_R`, the sample standard deviation over builds of the logarithm of a build's
  pooled rate, reproduces receipt 3's audit `reference_log_sd` column at all
  thirty-four cells to the six decimals the audit prints — 0.047448 at
  `bit_backend/or_inplace/words=1`, 0.038850 at
  `bit_backend/xor_inplace/words=1`, and thirty-two more.
- The half-split null, the ratio of the pooled rate over even-`j` members to the
  pooled rate over odd-`j` members, reproduces the audit's `null_ratio` column
  at all thirty-four cells, with no mismatch above 5·10⁻⁶.

## 2. What separates the natural pair

`σ̂` — the whole empirical content of v1 §6.4's coverage floor — is defined from
the natural pair of v1 §8: the committed baseline build, made in the main
checkout at `/home/vkaskivuo/Projects/gf2`, and receipt 2's control rebuild of
the same revision, made in `.agents/worktrees/control-0c072d73`. Their SHA-256
are `acbfd9c49325b4063b7a419114e05bacd6b98299bbde45fc89016f0d162db0fe` and
`c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`.

**The bench target embeds its manifest directory in `.rodata`.** `strings` finds
`/home/vkaskivuo/Projects/gf2/.agents/worktrees/control-0c072d73/crates/gf2-core`
in the control build. The two checkouts' manifest directories are 79 and 44
characters, so `.rodata` ends 35 bytes earlier in the baseline build and
everything after it sits at a lower address.

**The difference is a translation.** Copying the reference revision's sources to
a directory whose root path has the main checkout's length (28 characters) and
building there gives a binary in which every one of the 736 common text symbols
moves, all by the same −32 bytes, changing every symbol's address modulo 64 and
modulo 4096, with no symbol moved relative to any other. Copying to roots of
other lengths shifts by other constants, in 32-byte steps: two roots whose
lengths fall inside the same step — `/tmp/gp/a` and `/tmp/gp/bbb` — produce
different SHA-256 and an identical placement, moving none of the 736 symbols
against each other. Path *length* translates the image; path *content* changes
symbol hashes and moves no code.

**v1 §3.1's axes never translate the image.** They pad inside it. §4 below
measures the consequence directly.

## 3. The build-time screen

Every candidate is built by putting it in `RUSTFLAGS` and reading the produced
binary from Cargo's own JSON, as
[`ensemble-axis-verification.md`](/dev/active/972e2b88/ensemble-axis-verification.md)
does. Placement movement is `nm --defined-only` restricted to `t` and `T`
symbols, matched across builds after normalising the symbol-name hash, and
compared address by address. The ordinary build is 726,808 bytes with `.text` at
`0x1cbc0`.

| Candidate | `RUSTFLAGS` | Bytes | `.text` | Symbols moved | Movement |
|---|---|---:|---|---:|---|
| ordinary | (none) | 726,808 | `0x1cbc0` | — | — |
| build-id, default length | `-C link-arg=-Wl,--build-id=0x<20 bytes>` | 726,808 | `0x1cbc0` | **0/736** | none |
| build-id +32 | `…=0x<52 bytes>` | 726,840 | `0x1cbe0` | 736/736 | one constant, +32 |
| build-id +96 | `…=0x<116 bytes>` | 726,904 | `0x1cc20` | 736/736 | one constant, +96 |
| build-id +224 | `…=0x<244 bytes>` | 727,032 | `0x1cca0` | 736/736 | one constant, +224 |
| build-id +2016 | `…=0x<2036 bytes>` | 728,824 | `0x1d3a0` | 736/736 | one constant, +2016 |
| build-id +4064 | `…=0x<4084 bytes>` | 730,872 | `0x1dba0` | 736/736 | one constant, +4064 |
| `-C metadata=lv2-1` | disambiguator nonce | 726,816 | `0x1cbc0` | 121/736 | a block of 111 symbols by +240 |
| `-C link-dead-code` | dead code retained | 794,472 | `0x1f5d0` | 709/709 | 120 distinct deltas |
| `-x86-branches-within-32B-boundaries` | v1 §6.6's axis D | 743,480 | `0x1cbc0` | 731/736 | 481 distinct deltas |
| A = 7, B = 3, C = 3, D = 1 | v1's most perturbing member | 852,888 | `0x1cc80` | 736/736 | 709 distinct deltas |

**The build-id axis is a pure translation.** At payload length `20 + 32·E` the
`.text` section starts `32·E` bytes further along, verified at
`E` = 1, 3, 7, 63 and 127, and the extracted `.text` and `.rodata` **bytes are
byte-identical to the ordinary build's** at every level. The axis changes where
the program is loaded and provably nothing else. A build-id of the default
length changes the binary's SHA-256 and moves no symbol, exactly as v1 §6.6
found for `--sort-section=name`; it is the payload's *length* that is the axis,
not its value.

**Why the alignment axes quantise it away.** `-C llvm-args=-align-all-functions`
raises `.text`'s own section alignment: 16 at the ordinary build, 32 under
`-x86-branches-within-32B-boundaries`, and **128** under
`-align-all-functions=7`. A 32-byte translation request is then rounded to a
multiple of the section alignment — the most perturbing member asked for +4064
and received +4096, a whole page, which changes no address modulo 4096 at all.
`-C link-dead-code` adds symbols rather than aligning them, leaves `.text`
aligned to 16, and a translation combined with it lands exactly where it is
asked to: `0x1f5d0`, `0x1f5f0` and `0x1f670` for `E` = 0, 1 and 5.

## 4. Pilot 1

### 4.1 What it tested and how it read

The protocol fixed three groups, their member lists, their slot order, and a
decision rule with a 95 % lower confidence bound, before its first timed window.
The axis set under test was v1 §6.6's K = 256 rung with a translation axis
`E(j) = ⌊j/2⌋` added.

- **Group V1**, thirty-two members of v1 §3.1's K = 128 enumeration — the
  control.
- **Group SH**, thirty-two members varying the translation axis alone.
- **Group V2**, sixty-four members of the axis set under test — the decision
  group.

### 4.2 The builds

128 members, 126 distinct binaries — member 0 is shared by all three groups —
built in 1,814 s, 14 s to 21 s each. All thirty-two Group V1 binaries are
bit-identical to receipt 3's committed reference arm at the same members.

Two builds hit a transient `rustc` internal compiler error, at Group V2 members
`j` = 37 and `j` = 181. Both are the panic `active query job entry` in
`rustc_query_impl` while building the `criterion` dependency, and both succeeded
on an immediate retry with the identical flags, so neither is a property of the
member. The driver carries a single retry from that point; it is recorded under
Deviations.

### 4.3 The placement the three groups reach

| Group | Members | Symbols changing address mod 64 | mod 4096 | Pure translations | Distinct `.text` page offsets |
|---|---:|---:|---:|---:|---:|
| V1 | 32 | 83.3 % | 99.4 % | 0 | **6** |
| SH | 32 | 51.6 % | 100 % | 31 of 31 | **32** |
| V2 | 64 | 81.2 % | 99.7 % | 0 | 54 |

v1's ensemble moves symbols but barely moves the image: its thirty-two members
occupy six page offsets, all inside a 208-byte window of the 4,096-byte page
(2992, 3008, 3024, 3040, 3072 and 3200). The translation axis alone gives one
distinct page offset per member.

### 4.4 The measurement

The timed session ran 128 executions in 1,389 s under the lock, 10 s to 11 s
apart, mean 10.85 s, at a one-minute load average of 0.23 at the start, with
`HEAD` and an empty porcelain status recorded at the start and the end.

| Cell | Floor `σ̂`/2 | Group V1 | Group SH | Group V2 |
|---|---:|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.059077 | 0.049436 | **0.084487** | 0.049124 |
| `bit_backend/xor_inplace/words=1` | 0.049556 | 0.030728 | **0.071955** | 0.042778 |

**The decision rule reads REJECT.** Group V2's `s_R` is below the floor at both
cells, so neither the point estimate nor its lower bound clears it. The
calibration guard holds: Group V1 stands at 1.042 and 0.791 of receipt 3's
committed `s_R` at the two cells, inside the predeclared `[0.65, 1.55]`.

**Group SH clears both floors, and clears every other cell's floor too.** Its
`s_R` is at or above `σ̂`/2 at all thirty-four pinned cells, with no failure, and
its root mean square over the pinned set is 0.035739 against `σ̂`'s 0.033878.
Group V1 fails at the same two cells receipt 3 failed at; Group V2 fails at
exactly those two.

### 4.5 Why the combination loses the axis

Within Group V2, split on the level of A:

| Subgroup | Members | `s_R` at `or_inplace/words=1` | `s_R` at `xor_inplace/words=1` |
|---|---:|---:|---:|
| A = 0 | 8 | 0.064526 | 0.055292 |
| A = 1–3 | 24 | 0.046343 | 0.040748 |
| A = 4–7 | 32 | 0.039968 | 0.041951 |

The translation's effect decays as the function-alignment level rises, which is
the section-alignment quantisation §3 measures at build time. An ensemble that
aligns every function to a large boundary cannot also sample where the image
sits inside a cache line, and at these two cells that is what a natural rebuild
varies.

## 5. Pilot 2

### 5.1 What it tested

The protocol fixed the enumeration, the groups, the slot order and a six-gate
decision rule before its first timed window. The enumeration was the translation
axis E with a second axis G, `-C link-dead-code`, at two levels:

    E(j) = ⌊j / 2⌋        G(j) = (E(j) + j) mod 2        K = 256

v1's alignment axes are not in it. Three groups ran interleaved: the
**complete** 256-member reference ensemble, a thirty-two-member control reusing
pilot 1's Group V1 binaries, and eight executions of one fixed binary as a drift
probe.

### 5.2 The builds

256 members, **256 distinct binaries at 256 distinct `.text` page offsets**, so
no member repeats another's placement. Binary sizes span 726,808 to 798,536
bytes; the build phase took 3,554 s of compiler time; no member failed to build
and no retry was needed. `--self-check` prints `self-check PASS` with
`simd_min_words=8` from the most perturbing member, E = 127 with G = 1, and from
the dead-code-only member.

### 5.3 The measurement

296 executions in 3,241 s under the lock, at a one-minute load average of 0.29
at the start, with `HEAD` and an empty porcelain status at the start and at the
end.

| Gate | Requirement | Measured | Reading |
|---|---|---|---|
| (a) Coverage, per cell | `s_R(c) ≥ σ̂(c)/2` at all thirty-four | no failure; 0.069028 against 0.059077 at `or_inplace/words=1` and 0.069178 against 0.049556 at `xor_inplace/words=1` | **ok** |
| (b) Coverage, whole set | RMS `s_R` ≥ 0.033878 | 0.045339 | **ok** |
| (c) Half-split null | inside ±5 % per cell, ±2 % on the geometric mean | fails at four cells, widest 0.876003 at `or_inplace/words=1`; geometric mean 0.991160 | **FAIL** |
| (d) Precision, projected | margin ≥ 3 at every cell and at the set | narrowest 5.702, set margin 16.183; scaled reading 6.073 | **ok** |
| (e) Drift | Group FIX ≤ ¼ of `s_R` at the two cells | 0.000937 and 0.001584 against 0.017257 and 0.017295 | **ok** |
| (f) Calibration | Group CTL within `[0.65, 1.55]` of pilot 1's Group V1 | 0.108536 and 0.074355, ratios 2.195 and 2.420 | **FAIL** |

**Pilot 2 reads REJECT**, and it stands as taken.

### 5.4 Why the calibration guard failed

Group CTL ran pilot 1's Group V1 binaries, at the same execution indices, under
the same protocol. Their measurements agree binary for binary at every execution
but two. At `bit_backend/or_inplace/words=1`, execution 24 recorded 6.534
ns/call against pilot 1's 3.739 for the same binary — a single process-level
excursion of +75 % — and pilot 1's own execution 1 recorded 4.052 against this
session's 3.527. At `bit_backend/xor_inplace/words=1` execution 24 recorded
4.416 against 3.070 and every other execution agrees. Group FIX carries one of
its own: its seventh execution stands above the other seven at several `simd`
cells, 5.850 against 3.744 at `bit_backend/xor_inplace/words=8`.

One such execution moves a thirty-two-sample standard deviation by a factor of
two, which is the whole of the guard's failure: thirty of thirty-two executions
at one cell and thirty-one of thirty-two at the other agree within 5 %. These
are the excursions plan §7 records and receipt 3 observes; the ensemble group,
at 256 members, carries none — its ratio of largest to smallest rate at
`or_inplace/words=1` is 1.193, against Group V1's 1.183 over the same kind of
spread.

### 5.5 Why the half-split null failed, from build-time facts

Every member's realized `.text` address is `base(G) + 32·E` exactly, with
`base(0) = 0x1cbc0` and `base(1) = 0x1f5d0`. A member's offset inside a 64-byte
cache line is therefore `16·G + 32·(E mod 2)` and takes four values, and the
measured rate at `bit_backend/or_inplace/words=1` takes four matching clusters —
its 256 members sit near 3.52, 3.73, 3.95 and 4.16 ns/call.

Under `G(j) = (E(j) + j) mod 2` the two halves of the member-index parity split
occupy **disjoint** sets of those offsets: the even half holds 64 members at
offset 0 and 64 at 48, the odd half 64 at 16 and 64 at 32. Each axis was
marginally balanced — one member per level of E and sixty-four per level of G in
each half — and the *joint* distribution was not. That is exactly the
"difference between two unlike halves" v1 §3.2 exists to prevent, and it is
computable from the build log with no timed measurement.

`G(j) = (⌊j/4⌋ + j) mod 2` repairs it. Over the same 256 members and the same
realized addresses, each half then holds exactly 32 members at each of the four
cache-line offsets, the two multisets of `(address >> 6) mod 64` are equal with
all sixty-four L1i sets covered twice per half, each of the 128 levels of E
appears once per half, and each level of G sixty-four times. The 256 members
stay distinct and their 256 page offsets stay distinct.

## 6. Pilot 3

### 6.1 What it tested

Pilot 2's rejection was diagnosed from build-time facts, and pilot 3's protocol
fixed the corrected enumeration and the same six gates, with the two guards
restated robustly, before its first timed window. The enumeration is

    E(j) = ⌊j / 2⌋        G(j) = (⌊j / 4⌋ + j) mod 2        K = 256

with axis E the build-id translation and axis G `-C link-dead-code`, each level
0 omitting its option, so member 0 is the ordinary build. The 256 binaries are
pilot 2's, unchanged and unrebuilt: the set of `(E, G)` members is the same set,
and the correction relabels which member index — and so which half — each binary
carries. Their `RUSTFLAGS`, sizes, SHA-256, `.text` addresses and page offsets
are [`pilot-3-member-provenance.tsv`](pilot-3-member-provenance.tsv), SHA-256
`4dc5c75ceedf955238ed782ac3eefe7e351c5cef85ae6e5e89714e2fb5ccf1fb`.

The construction was verified over all 256 members from that provenance, before
the session: 256 distinct members at 256 distinct page offsets; each half of the
parity split holding exactly 32 members at each of the four cache-line offsets;
the two multisets of `(address >> 6) mod 64` equal, covering all sixty-four L1i
sets twice per half; one member per half at each of the 128 levels of E and
sixty-four at each of the two levels of G.

### 6.2 The measurement

296 executions in 3,239 s under the lock, no failed run, at a one-minute load
average of 0.08 at the start, with `HEAD` at `0c072d73` and an empty porcelain
status at the start and the end.

| Gate | Requirement | Measured | Reading |
|---|---|---|---|
| (a) Coverage, per cell | `s_R(c) ≥ σ̂(c)/2` at all thirty-four | no failure; tightest ratio 1.17 | **ok** |
| (b) Coverage, whole set | RMS `s_R` ≥ 0.033878 | 0.046937 | **ok** |
| (c) Half-split null | inside ±5 % per cell, ±2 % on the geometric mean | no failure; widest 1.022538; geometric mean 1.000845 | **ok** |
| (d) Precision, projected | margin ≥ 3 at every cell and at the set | narrowest 4.849, set margin 15.931; scaled reading 3.922 | **ok** |
| (e) Drift | `MADN(FIX)` ≤ ¼ of `s_R` at the two cells | 0.001100 and 0.000357 against 0.017271 and 0.017459 | **ok** |
| (f) Calibration | ≥ 30 of 32 Group CTL executions within 5 % of pilot 1's Group V1 | 31 of 32 and 32 of 32; the one disagreement is pilot 1's own execution 1, 4.052 against this session's 3.514 | **ok** |

**Pilot 3 reads ADOPT.**

At the two cells receipt 3's audit failed:

| Cell | Floor `σ̂`/2 | `s_R` | Fraction of the floor reached |
|---|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.059077 | 0.069084 | **116.9 %** |
| `bit_backend/xor_inplace/words=1` | 0.049556 | 0.069835 | **140.9 %** |

The correction is visible against pilot 2 at the cell that diagnosed it: the
half-split null at `bit_backend/or_inplace/words=1` moves from 0.876003 to
0.999998 over the same 256 binaries measured under the same protocol, with only
the assignment of members to halves changed.

### 6.3 Every cell

`s_R` over the 256 builds, the half-split null between the parity halves, and
the projected precision margin `ln(1.05) / se` at K = 256 with `se` computed
from this `s_R` and receipt 3's committed candidate arm.

| Cell | `σ̂`/2 | `s_R` | ratio | half-split null | projected margin |
|---|---:|---:|---:|---:|---:|
| `bit_backend/and_inplace/words=1` | 0.020476 | 0.050074 | 2.45 | 1.008218 | 12.16 |
| `bit_backend/and_inplace/words=8` | 0.016662 | 0.066494 | 3.99 | 0.995146 | 6.32 |
| `bit_backend/not_inplace/words=1` | 0.000111 | 0.096327 | 867.81 | 0.996855 | 7.36 |
| `bit_backend/not_inplace/words=8` | 0.003363 | 0.085508 | 25.43 | 0.992733 | 6.62 |
| `bit_backend/or_inplace/words=1` | 0.059077 | 0.069084 | 1.17 | 0.999998 | 9.64 |
| `bit_backend/or_inplace/words=8` | 0.013455 | 0.048489 | 3.60 | 0.993965 | 9.25 |
| `bit_backend/popcount/words=1` | 0.016466 | 0.027844 | 1.69 | 0.999474 | 15.11 |
| `bit_backend/popcount/words=8` | 0.000221 | 0.005379 | 24.34 | 0.998945 | 12.83 |
| `bit_backend/xor_inplace/words=1` | 0.049556 | 0.069835 | 1.41 | 0.999570 | 9.22 |
| `bit_backend/xor_inplace/words=16` | 0.003720 | 0.107369 | 28.87 | 1.022538 | 4.85 |
| `bit_backend/xor_inplace/words=4` | 0.018958 | 0.050456 | 2.66 | 0.998638 | 8.62 |
| `bit_backend/xor_inplace/words=64` | 0.002449 | 0.064184 | 26.21 | 1.007109 | 9.98 |
| `bit_backend/xor_inplace/words=7` | 0.007889 | 0.071533 | 9.07 | 1.011410 | 9.07 |
| `bit_backend/xor_inplace/words=8` | 0.003905 | 0.049248 | 12.61 | 0.996064 | 6.23 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | 0.000517 | 0.001530 | 2.96 | 1.000082 | 225.80 |
| `polynomial/batch_evaluate/coeffs=4095/points=4095` | 0.001293 | 0.002033 | 1.57 | 0.999961 | 190.59 |
| `polynomial/batch_evaluate/coeffs=4096/points=4096` | 0.013567 | 0.019823 | 1.46 | 1.000605 | 26.18 |
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | 0.000217 | 0.001695 | 7.83 | 0.999941 | 241.45 |
| `polynomial/batch_evaluate_auto/coeffs=4095/points=4095` | 0.001224 | 0.002879 | 2.35 | 0.999980 | 136.16 |
| `polynomial/batch_evaluate_auto/coeffs=4096/points=4096` | 0.006403 | 0.017041 | 2.66 | 0.999749 | 29.23 |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 0.041960 | 0.049994 | 1.19 | 1.001004 | 11.26 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2047` | 0.001385 | 0.026652 | 19.24 | 0.998467 | 19.80 |
| `polynomial/div_rem_auto/dividend=4096/divisor=2048` | 0.003791 | 0.006671 | 1.76 | 1.000427 | 90.28 |
| `polynomial/div_rem_auto/dividend=8192/divisor=4096` | 0.001843 | 0.004005 | 2.17 | 1.000004 | 131.50 |
| `polynomial/mul/len=16` | 0.006326 | 0.032351 | 5.11 | 1.003559 | 14.73 |
| `polynomial/mul/len=256` | 0.004661 | 0.036647 | 7.86 | 1.000011 | 14.44 |
| `polynomial/mul/len=32` | 0.006410 | 0.035765 | 5.58 | 1.003201 | 12.31 |
| `polynomial/mul/len=33` | 0.000865 | 0.022163 | 25.62 | 0.999423 | 22.24 |
| `polynomial/mul/len=64` | 0.004387 | 0.040890 | 9.32 | 1.001231 | 13.14 |
| `polynomial/mul_fast/len=128` | 0.004202 | 0.006639 | 1.58 | 1.000031 | 72.66 |
| `polynomial/mul_fast/len=32` | 0.000938 | 0.040048 | 42.67 | 1.000024 | 13.29 |
| `polynomial/mul_fast/len=512` | 0.004221 | 0.007197 | 1.70 | 1.000050 | 76.52 |
| `polynomial/mul_fast/len=64` | 0.001924 | 0.036079 | 18.75 | 1.000887 | 14.31 |
| `polynomial/mul_fast/len=65` | 0.004564 | 0.006417 | 1.41 | 0.999882 | 73.73 |

Coverage clears its floor at every cell, the half-split null stays inside ±2.3 %
at every cell, and the narrowest projected precision margin is 4.85.

## 7. What the pilots decided

- **The ensemble's axes become the translation axis E and the dead-code axis G,
  at K = 256.** v1 §3.1's A, B and C and v1 §6.6's D are replaced, not extended:
  pilot 1 measures them suppressing the translation, and §3 measures why.
- **Coverage is repaired at the two cells that failed it**, and at no cost to
  the other thirty-two: every cell clears `σ̂`/2, and the set-level comparison
  clears by 39 %.
- **The half-split null, precision and drift readings hold** under the corrected
  assignment.
- **Decorrelation is not decided here.** It needs two arms.


## Deviations

Every departure from the procedure as written, however small.

- **The wrapper's `nice -n -5` is denied**, as plan §6 anticipates, and the
  invoking session's own niceness is 5, so the child runs at niceness 5. This
  matches receipt 3's session. The lock and the CPU 6–11 affinity are in force
  and the host carries no competing work during either timed phase.
- **Two transient compiler crashes.** §4.2 records them; the build driver was
  given a single-retry rule after the first, and the second was retried by that
  rule. Both retries produced a binary under the identical flags.
- **The pilots build into a scratch target directory on `/tmp`, pruned between
  members.** The Provenance section records the check that the target directory
  does not enter the binary: the ordinary build reproduces the committed
  `c7be7a87…` from it.
- **Pilot 1's Group SH and Group V2 subgroup readings are recorded, not
  predeclared as decisions.** The protocol states that Group SH enters no branch
  of its rule, and §4.5's split by A level is a diagnosis written after the
  measurement. Neither is read as a decision under pilot 1's rule; the decision
  under that rule is REJECT, and the axis set they point to is decided by the
  separately predeclared pilot 2.
- **Three pilots ran, each under its own protocol, and each stands as taken.**
  Pilots 1 and 2 read REJECT and are recorded here with their numbers, their
  failing gates and their diagnoses. No pilot was re-read under another pilot's
  rule and no rule was relaxed: pilot 2 tests an axis set pilot 1's measurement
  pointed to, and pilot 3 tests an enumeration whose correction pilot 2's
  *build-time* facts forced. Pilot 3's decision statistic was not computed from
  pilot 2's measurements before pilot 3's protocol was fixed.
- **Pilot 3 re-measures pilot 2's binaries rather than rebuilding them.** The
  set of 256 `(E, G)` members is identical under the two enumerations; what
  changes is which member index, and so which half of the parity split, each
  binary carries. The 256 executions are a fresh, independent measurement.
- **Pilot 2's and pilot 3's guard groups read robustly and their ensemble group
  does not.** §5.4 records the process-level excursions the guards are robust
  to. No execution of any ensemble group is dropped, trimmed or robustified.

## What this receipt does not establish

- **It measures no candidate revision.** No verdict ratio, and no measurement of
  v1 §6.4's decorrelation precondition, which needs two arms. The measured
  session's own audit decides both.
- **Its precision reading is a projection.** It combines the measured reference
  dispersion with receipt 3's committed candidate arm, which was built under
  different axes. The measured session's audit decides precision.
- **It samples one host, one toolchain and one revision.** Every figure is from
  `fraktaali` under `rustc 1.95.0` at revision `0c072d73`.
- **Symbol movement is counted, not weighted.** As `972e2b88`'s receipt states,
  a moved symbol may sit at an address whose cache-line offset is unchanged; the
  count bounds nothing about the size of a layout effect. What bounds it here is
  the timed measurement.
