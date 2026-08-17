# Field-by-field go/no-go synthesis with production design

The accelerator study's findings document. It states a go or no-go verdict for
$\mathbb{F}_3$, $\mathbb{F}_5$, and $\mathbb{F}_7$, the production design behind
every go, the validity argument for the $\mathbb{F}_3$ Boolean representation,
and the decision on where a permanent-specialized representation lives.

**This document introduces no measurement.** Every quantity in it is read from a
committed receipt or from an in-tree source location, and is cited to the file
it comes from rather than restated as an independent claim. Where two committed
artifacts disagree, §10 records the disagreement rather than choosing silently.

| Evidence | Role |
| --- | --- |
| [`../047b62ed/receipts.md`](../047b62ed/receipts.md) | $\mathbb{F}_3$ preregistered receipt campaign |
| [`../91605d4d/receipts.md`](../91605d4d/receipts.md) | $\mathbb{F}_5$ preregistered receipt campaign |
| [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) | $\mathbb{F}_7$ preregistered receipt campaign, with its paired profiled evidence run |
| [`../a9284086/receipt.md`](../a9284086/receipt.md) | Device-wide runtime qualification: wave utilization, occupancy, launch-duration envelopes |
| [`../b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md) | Prior feasibility study and its gap table |
| [`../../research/permanent_wave_gpu/`](../../research/permanent_wave_gpu/README.md) | The executable prototype: shared mapping header, three per-field equivalence units |
| [`../../active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md), [`plan.md`](../../active/0de41c82/plan.md) | The study's own survey of this ground |
| [`req08-amendment-draft.md`](req08-amendment-draft.md) | Drafted amendment to the archived $\mathbb{F}_7$ encoding decision (§8) |

All three field campaigns and the runtime qualification measure one host and one
architecture: AMD Ryzen 9 5900X, AMD Radeon RX 6950 XT, `gfx1030`, ROCm 7.2.4
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §1). Nothing here claims
anything about another architecture
([`../a9284086/receipt.md`](../a9284086/receipt.md) §12.8).

## 1. The decision rule, and what each verdict has to clear

The study's own rule is committed before the measurements:

> A go decision requires exact equivalence, reproducible receipts, a safe launch
> duration, and an end-to-end crossover against the best applicable CPU path. An
> operator-only win is insufficient. A no-go decision retains the candidate,
> resource report, and falsifying measurements so the negative result remains
> reproducible.
> ([`../../active/0de41c82/bipedal-f5-f7-representation-study.md`](../../active/0de41c82/bipedal-f5-f7-representation-study.md):268-273)

Four conditions, and this document takes each literally.

**Exact equivalence** is the campaigns' shared gate: one global invocation run as
the first step of the pipeline, before any timing cell, comparing every executing
path against the field's CPU oracle on one literally identical matrix corpus per
$(q, n)$ ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §3).

**Reproducible receipts** are the campaigns' own provenance: revision, toolchain,
host inventory, and binary SHA-256 pinned and re-verified three times per
campaign, under the repository's canonical benchmark mutex, with `analysis.py`
regenerating every table from committed artifacts
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §1).

**A safe launch duration** is discharged against the bound this study documents.
The runtime qualification derives one per field from its own measurements and
states it in place: the *observed clean-completion envelope*, "*the largest
per-launch work and the longest per-launch span at which a launch of a retained
path completed without device fault, on this host, at the pinned binary*", each
figure carried by a named cell with its outcome and its launch count
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.2). Per field it is
per-launch work to $2.7488 \times 10^{11}$ and per-launch span to 53.077521 s at
$q = 3$; to $1.0737 \times 10^{9}$ and 115.452922 s at $q = 5$; and to
$1.0737 \times 10^{9}$ and 21.722805 s at $q = 7$ (§9.4). It is observed rather
than derived from a device model, and it is a *conservative lower bound* on the
safe region: no launch in the qualification was run to failure, so it locates no
upper boundary, and every launch inside it is inside a duration the evidence
observes completing cleanly. Deriving an upper boundary needs a launch run to
failure and remains an open deliverable (§12.1). This document calls that envelope the
documented safe launch-duration bound, and checks each verdict's declared
operating point against its own field's figures.

**An end-to-end crossover against the best applicable in-tree CPU path** is the
composite-throughput comparison of each campaign's §4.2, with the best CPU path
identified per order from that run's own data rather than assumed. "Composite"
charges generation, evaluation, reduction, and store — an operator-only or
kernel-only win does not count
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.2, §4.3).

The declared scientifically relevant operating point per field is fixed by the
campaign protocol, at the processor-feasible frontier of each field: $n = 28$
for $q = 3$, $n = 24$ for $q = 5$, and $n = 20$ for $q = 7$
([`../../simulation_results/permanent-zero-fraction/protocol.md`](../../simulation_results/permanent-zero-fraction/protocol.md):51-54).
The same protocol fixes four premeasurement configurations: $(3,28)$ on the
accelerator at $M = 1024$ and on intra-matrix rayon, $(5,24)$ on batch rayon,
and $(7,20)$ on the accelerator at $M = 1024$ (`protocol.md`:251-254).

## 2. The intra-matrix parallel Ryser decomposition the study exercised (REQ-03)

The executable prototype is
[`dev/research/permanent_wave_gpu/`](../../research/permanent_wave_gpu/README.md).
Its cross-field control mapping lives once, in
[`hip/wave_ryser_mapping.h`](../../research/permanent_wave_gpu/hip/wave_ryser_mapping.h),
and all six lane-owns-interval kernels of the three fields call it. Packed column
staging and packed field arithmetic stay in each candidate kernel; the header
owns only the mapping (`wave_ryser_mapping.h:1-6`).

One block owns one matrix. The block's active lane count is
`active_lanes_for_order(n)`, which is $2^n$ below $n = 5$ and 32 at and above it
(`wave_ryser_mapping.h:29-31`), so it is 32 at every order these campaigns
measure ([`../a9284086/receipt.md`](../a9284086/receipt.md) §3).

**The per-lane Gray interval.** `balanced_interval(total, lane, lanes)` gives
lane $i$ the contiguous half-open range starting at
$i\lfloor T/L \rfloor + \min(i,\, T \bmod L)$, of length $\lfloor T/L \rfloor$
plus one more when $i < T \bmod L$ (`wave_ryser_mapping.h:33-40`). Every walk
kernel calls it with $T = 2^n$ and $L =$ `blockDim.x`:
`wave_gf3_equivalence.hip:196-198`, `f5_wave_equivalence.hip:243`, `:312`,
`wave_gf7_equivalence.hip:185`, `:298`. The intervals are contiguous, disjoint,
and cover $[0, 2^n)$ by construction. At every measured order
$2^n \bmod 32 = 0$, so the remainder term is zero and all 32 lanes own exactly
$2^{n-5}$ indices — 128 at $n = 12$ up to 8 388 608 at $n = 28$, with no
tail-interval idleness ([`../a9284086/receipt.md`](../a9284086/receipt.md) §3,
where `analysis.py` section 3 asserts the divisibility).

**The per-lane accumulator initialization.** A lane does not walk from index 0.
It reconstructs the packed column-sum accumulator directly at its own interval
start, from the canonical Gray subset $g(k) = k \oplus (k \gg 1)$
(`wave_ryser_mapping.h:42-44`) of that start index: it iterates the $n$ columns
once and folds in every column present in `gray_subset(interval.start)`. That
prefix reconstruction is `wave_gf3_equivalence.hip:208-213` for $\mathbb{F}_3$
(via `add3`), `f5_wave_equivalence.hip:247-254` and `:316-322` for the two
$\mathbb{F}_5$ representations, and `wave_gf7_equivalence.hip:186`, `:190-197`
for the $\mathbb{F}_7$ three-plane path (via `add_three_plane`). Its cost is
$O(n)$ per lane once, against $2^{n-5}$ walk steps, so it is what makes the
partition free rather than a serial prefix.

**The lane-local row-product reduction.** At its interval start and after every
transition, the lane reduces its own packed column-sum accumulator to one scalar
field element — the Ryser row product — without touching another lane. Each
representation supplies its own circuit for this and nothing else about the
mapping changes: `fold_product<Fold>` for $\mathbb{F}_3$
(`wave_gf3_equivalence.hip:140-149`, dispatching to `fold_product_halving` at
`:103` or `fold_product_zero_mask_sign_popcount` at `:129`), `byte_product` at
`f5_wave_equivalence.hip:169` and `c4_product` at `:182` for the two
$\mathbb{F}_5$ representations, and `control_product` at
`wave_gf7_equivalence.hip:255` and `three_plane_product` at `:111` for the two
$\mathbb{F}_7$ representations. The product is signed by the parity of the
subset's popcount and accumulated into a per-lane scalar partial
(`wave_gf3_equivalence.hip:215-233`, `wave_gf7_equivalence.hip:201-216`).

**The cross-lane partial-sum reduction.** The only quantity that crosses a lane
boundary is the scalar partial. `reduce_partials_in_lane_order`
(`wave_ryser_mapping.h:67-78`) runs `lane_count` `__shfl` steps in which every
lane participates and lane 0 alone accumulates, reading source lanes in
increasing lane index. The fixed source order is what makes the reduction
schedule-independent, which the header states in place
(`wave_ryser_mapping.h:64-66`) and each kernel repeats at its call site
(`wave_gf3_equivalence.hip:236-239`). No packed state crosses lanes
(`wave_gf7_equivalence.hip:4-6`). Lane 0 then applies the one outer Ryser sign,
$(-1)^n$ in the field, via `apply_outer_ryser_sign`
(`wave_ryser_mapping.h:80-83`), and writes the result
(`wave_gf3_equivalence.hip:240-243`, `f5_wave_equivalence.hip:274-276`, `:341-343`,
`wave_gf7_equivalence.hip:220-223`, `:327-330`).

The one retained device path whose kernel is not a Gray walk is the
$\mathbb{F}_7$ bit-plane transpose `prepare_three_plane_columns`
(`wave_gf7_equivalence.hip:134-163`), which stages bytes into planes before the
walk and occupies $n$ of 32 lanes
([`../a9284086/receipt.md`](../a9284086/receipt.md) §3).

This mapping is what the wave-utilization figures rest on: the lane-owns-interval
mapping makes 32 of 32 lanes active, against 3.1250 % for the shipped
one-thread-per-matrix mapping, each exact by construction and cited to its
launch site ([`../a9284086/receipt.md`](../a9284086/receipt.md) §3).

## 3. $\mathbb{F}_3$ — no-go

**Verdict: no-go.** At the declared operating point $n = 28$ no device path
reaches the best applicable in-tree CPU path. The best measured device cell of
any kind at that order is the shipped `gpu_hip` at $M = 256$, 8.5448 matrices/s,
which is **0.4390×** `cpu_rayon_intra_matrix` at 19.4629 matrices/s; the best
prototype cell is `fold-gf3` at $M = 3$, 3.9630 matrices/s, **0.2036×** the same
baseline ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.2).

The equivalence and reproducibility conditions are met — all 48 $q = 3$
comparison cells report `mismatches = 0` and `status = identical`, both
prototypes included, at every order the grid times
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §3) — and so is the
safe-launch-duration condition at the declared operating point: no $q = 3$ cell
failed with a device fault, and the two $n = 28$ `gpu_hip` cells are themselves
what this field's documented safe launch-duration bound is read from — the
$M = 256$ cell's five completed launches at 29.954639 s of device kernel time
each, and the $M = 1024$ cell's three completed launches at 53.02–53.20 s, which
set the $q = 3$ observed clean-completion envelope at $2.7488 \times 10^{11}$
work units and 53.077521 s per launch
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.2, §9.3). The condition
that fails is the end-to-end crossover, at the order the campaign protocol
declares.

### 3.1 The measured crossover condition (REQ-02)

$\mathbb{F}_3$ is the one field whose measured ratios bracket a crossing in both
directions. Every crossing this document states is a *bracketing between two
adjacent measured orders*: the grid measures $n \in \{12, 16, 20, 24, 28\}$ and
nothing between, so a crossing is located no more finely than the measured pair
whose ratios straddle 1, and no crossing is placed anywhere the measured pairs do
not straddle it. Composite throughput ratios against the
best applicable in-tree CPU path at each order
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.2):

| $n$ | best in-tree CPU path | rate | best prototype / CPU | best shipped GPU / CPU |
| ---: | --- | ---: | ---: | ---: |
| 12 | `cpu_rayon_batch_scalar` | 301 317.4209 | 0.7568 | 0.7118 |
| 16 | `cpu_rayon_batch_scalar` | 37 066.3694 | 2.2149 | 1.5547 |
| 20 | `cpu_rayon_intra_matrix` | 2 966.4439 | 23.4238 | 1.6344 |
| 24 | `cpu_rayon_intra_matrix` | 296.4992 | 1.5561 | 1.0527 |
| 28 | `cpu_rayon_intra_matrix` | 19.4629 | 0.2036 | 0.4390 |

The prototype's ratio crosses 1 upward between the measured $n = 12$ and
$n = 16$ — 0.7568 to 2.2149 — and back downward between $n = 24$ and $n = 28$ —
1.5561 to 0.2036. The best measured shipped cell per order brackets crossings on
the same two pairs, 0.7118 to 1.5547 and 1.0527 to 0.4390, subject to the caveat
below that its $n = 28$ entry is a smaller batch than its $n = 24$ one. Each of
these is a bracketing between two adjacent measured orders and none is located
more finely, because the grid measures no order between them.

**The downward crossing of the better shipped configuration is not located at
all, and the campaign says so.** The $n = 28$, $M = 1024$ cell — which is one of
the protocol's own four premeasurement configurations — is censored: the 120 s
cap ended timing after three repetitions of 53.02–53.20 s, before the protocol's
five-repetition and five-second minimums, and the cell carries `NaN` for both
throughput columns ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §5).
Its projection is 16.720265 matrices/s from $n = 24$; the same file's own
measured $24 \rightarrow 28$ bias on the $M = 256$ `gpu_hip` chain is $-14.0\%$,
which carried across puts the cell near 19.4 against the CPU's 19.4629. That is
an extrapolation from a neighbouring batch size, not a measurement, and the
campaign asserts no ordering at $n = 28$ between those two
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §11, §13.5).

So the no-go rests on what is measured — every measured $n = 28$ device cell
loses to the CPU — and the one cell that could overturn it is censored and is
recorded as open rather than projected into a verdict.

### 3.2 What the field does establish, kept rather than discarded

The interior window is a real positive result and is preserved
(`@/inv/falsification-preserved`). `fold-gf3`, the zero-mask/sign-popcount fold,
leads every path in the campaign at $n \in \{16, 20, 24\}$, and its best
operating point is $n = 20$, $M = 448$ at 69 485.2981 matrices/s against
`cpu_rayon_intra_matrix` at 2 966.4439 — a ratio of 23.4238× at 4.6442 µs of
device launch overhead per launch
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.4).

That headline is confounded and the campaign quantifies the confound rather than
smoothing it: the cell runs 448 matrices and 14 336 active lanes against the
shipped path's 1 024, no prototype cell in the campaign runs at the control's
batch sizes, and nothing in the run separates the fold circuit from the batch
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.5, §13.1, §13.2). The
comparison the campaign does support at matched geometry is the resource one:
control at 19 `VGPRs` / 27 `TotalSGPRs` / 1 040 scratch bytes per lane against
the two prototype folds at 22 and 25 `VGPRs` / 22 `TotalSGPRs` / zero scratch,
every one of them at the architectural ceiling of 16 waves/SIMD
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §7, §8).

### 3.3 Why the $\mathbb{F}_3$ Boolean representation is valid for a permanent over $\mathbb{F}_3$ (REQ-04)

The representation is two Boolean planes per lane, and the question is whether
computing with Boolean words makes this a permanent over $\mathbb{F}_2$. It does
not, and five independent facts say so.

**The encoding carries three values, not two, and its validity rests on the
encoding's equivalence classes rather than on a canonical form.** Each
$\mathbb{F}_3$ lane is a pair of bits `(mag, sgn)`: $0 \mapsto (0,0)$,
$1 \mapsto (1,0)$, $2 \mapsto (1,1)$, with `sgn` a don't-care when `mag` is clear
(`crates/gf2-algebra/src/packed/bipedal3.rs:12-21`, `:53-65`). The fourth
codeword $(0,1)$ is a second encoding of zero, so the four codewords carry three
field values under three classes — $\{(0,0), (0,1)\}$, $\{(1,0)\}$, $\{(1,1)\}$ —
and a clear `mag` bit *is* field zero whatever `sgn` holds. Every decode in the
repository reads exactly that class and nothing finer: `lane` returns 0 on
`m == 0` before it consults the sign bit (`bipedal3.rs:727-728`), `all_zero` is
`self.mag == 0` (`:811-812`), and `Eq` compares `mag` and then masks the sign
difference with `mag`, so sign bits on zero lanes cannot separate two values
(`:127-134`). The device fold circuits read the same class: the
zero-mask/sign-popcount fold takes a clear magnitude bit as a zero lane
"*regardless of its don't-care sign bit*"
(`wave_gf3_equivalence.hip:126-128`, `:132-135`), and the halving control's
magnitude AND-chain returns 0 whenever any active lane's magnitude bit is clear,
sign plane unread (`:105-122`).

The operations respect those classes. Evaluating `add`, `sub`, `mul`, and `neg`
(`bipedal3.rs:581-593`, `:620-632`, `:680-685`, `:652-657`) on all sixteen
codeword pairs — the alternative zero included as an input — decodes to the
correct $\mathbb{F}_3$ result in every case, which is what makes the class the
carrier of the field value; that enumeration is computed here from those source
lines and is stated as a computation rather than as a measurement. The crate's
own doc comment additionally claims the alternative zero is "*never produced by
`add/sub/mul/neg` from canonical inputs*" (`:19-21`, `:62-65`). That narrower
claim is false of the implemented circuits — `add((1,1),(1,0))`,
`sub((1,0),(1,0))`, and `mul((0,0),(1,1))` each return $(0,1)$ — and the argument
here is deliberately independent of it (§10, F.13). Two bits per lane address
$\{0, 1, 2\}$; one bit per lane would address $\{0, 1\}$. The lane alphabet is
$\mathbb{F}_3$.

**The circuits are $\mathbb{F}_3$ arithmetic, not $\mathbb{F}_2$ arithmetic.**
`add` and `sub` are the bitwise formulas of [Scheinerman2024] Theorem 2.1,
"*Bipedal Representation Operations*", transliterated once
(`crates/gf2-algebra/src/packed/bipedal3.rs:5-7`; that doc comment names the
object "Theorem 2.1 / Algorithm 2", and the paper carries no object labelled
Algorithm 2 — Theorem 2.1 is where its add, subtract, multiply, and divide
formulas are stated). The device prototype uses the same two circuits, `add3`
and `sub3`
(`dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip:72-90`). They are
distinct operations: over $\mathbb{F}_2$ addition and subtraction coincide, and
here they do not — `add3` and `sub3` have different bodies and the Gray walk
selects between them on the transition direction
(`wave_gf3_equivalence.hip:223-229`). A representation on which $a + b$ and
$a - b$ differ is not carrying $\mathbb{F}_2$.

**Two algebraic facts specific to $q = 3$ are what make the encoding Boolean,
and neither transfers.** First, $3 = 2^2 - 1$, so reduction after addition is a
Mersenne-style fold rather than a division. Second, $|\mathbb{F}_3^*| = 2$, so
the multiplicative group is $\mathbb{Z}/2$ and the only distinction among
nonzero elements is a sign bit. The zero-mask/sign-popcount fold uses exactly
that and says so in place: "*A clear magnitude bit identifies a zero F_3 lane
regardless of its don't-care sign bit. Once every active lane is nonzero, the
sign population parity is the product in F_3^\**"
(`wave_gf3_equivalence.hip:126-128`), implemented as a zero mask over the active
lanes and a popcount parity of the sign plane (`:129-137`). The sign plane is
the group $\mathbb{F}_3^* \cong \mathbb{Z}/2$; it is not the field. Neither
$|\mathbb{F}_5^*| = 4$ nor $|\mathbb{F}_7^*| = 6$ admits the same reduction, and
the $\mathbb{F}_5$ and $\mathbb{F}_7$ prototypes correspondingly carry three
planes and a decode rather than two planes and a parity
(`f5_wave_equivalence.hip:182`, `wave_gf7_equivalence.hip:111`).

**The outer Ryser sign is nontrivial in $\mathbb{F}_3$ and is applied.** Ryser's
formula carries $(-1)^{n-|S|}$, which [Scheinerman2024] Equation (2) writes as
the pair of factors $(-1)^n$ and $(-1)^{|S|}$; over $\mathbb{F}_2$ that factor is
identically 1 and the permanent collapses onto the determinant. Over
$\mathbb{F}_3$, $-1 \ne 1$, and the kernel applies the sign twice over: per Gray
step by the parity of the subset popcount, with `fp3_sub` rather than an XOR
(`wave_gf3_equivalence.hip:92`, `:217-219`, `:231-233`), and once at the end via
`apply_outer_ryser_sign`, whose negation is `negate_scalar<3>` — $v \mapsto 3-v$
for $v \ne 0$ (`wave_ryser_mapping.h:59-62`, `:80-83`). A kernel computing an
$\mathbb{F}_2$ permanent would have nothing for those instructions to do.

**The measurements discriminate the field, not only the source.** Two committed
observations are functions of $q = 3$ specifically and both hold. The
horizontal-product branch frequencies match the exact marginal $1 - (2/3)^n$ and
$(2/3)^n$ at all five orders, inside two-sided Wilson 95 % intervals on 4 096
samples and again on the 7.3–9.1 million timed operations of the same file
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §9): the zero-fast
expectation is 0.992292653371 at $n = 12$ against an observed 0.992187500, and
0.999988266036 at $n = 28$.

That marginal is $1 - ((q-1)/q)^n$ evaluated at $q = 3$, and the corresponding
$q = 2$ value $1 - 2^{-n}$ is excluded by the committed intervals — with its
coverage stated exactly rather than claimed for every cell. On the 4 096-sample
observation it is excluded at $n = 12$, 16, and 20, where $1 - 2^{-n}$ is
0.999755859, 0.999984741, and 0.999999046 against upper Wilson bounds of
0.994460490, 0.999620170, and 0.999866085; at $n = 24$ and $n = 28$ that
observation cannot discriminate, because both branches read 4096/4096 and the
interval reaches 1. On the same file's timed-operation observation, three orders
of magnitude tighter, it is excluded at all five orders on the slow branch:
$2^{-n}$ is $2.441 \times 10^{-4}$ down to $3.725 \times 10^{-9}$ against
measured intervals of $[7.633, 7.760] \times 10^{-3}$ down to
$[8.631, 12.868] \times 10^{-6}$. Both comparisons are computed here from the
intervals and expectations that receipt prints.

Second, the fraction of sampled matrices whose permanent is zero is 0.333714
with Wilson interval [0.333356, 0.334072] at $n = 12$ and stays near $1/3$ at
every order ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §9.1) — the
$\mathbb{F}_3$ value.

**And the result is checked against an independent oracle.** The prototypes are
compared per matrix against `permanent_bipedal3_singleword`, on 512 matrices at
$n \in \{8, 12, 16, 20\}$, 32 at $n = 24$, and 4 at $n = 28$, with zero
mismatches and identical zero counts throughout
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §3). That oracle is an
$\mathbb{F}_3$ kernel whose element type is `Fp<3>` and which runs the
repository's shared field-law conformance suite
(`@/inv/finite-field-laws`). The equivalence gate would not close if the device
path were computing a permanent over $\mathbb{F}_2$.

## 4. $\mathbb{F}_5$ — go, on the lane-owns-interval three-plane prototype

**Verdict: go**, for `f5-three-plane` under the lane-owns-interval mapping, and
**no-go for the shipped `gpu_hip` path**, which has no operating point where it
beats the CPU. At the declared operating point $n = 24$, `f5-three-plane` at
$M = 17$ measures 211.9399 matrices/s against `cpu_rayon_batch_scalar` at
$M = 96$ measuring 10.7539 — a factor of **19.7082×**
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12). The equivalence gate
closes on all 30 $q = 5$ comparison cells with `mismatches = 0`, both prototypes
included, at every order the grid times and at $n = 8$ below them
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §3).

**The declared operating point is inside this field's documented safe
launch-duration bound.** It runs a kernel span of 0.0800 s per launch on
$2.852 \times 10^{8}$ units of $M \cdot 2^n$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12) against the $q = 5$
observed clean-completion envelope of 115.452922 s per launch and
$1.0737 \times 10^{9}$ work units
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4) — 0.00069 of the span
figure and 0.266 of the work figure, computed in §9 from those two sources.

### 4.1 The measured crossover condition (REQ-02)

**The prototype's ratio is above 1 at every measured order, so no measured pair
of adjacent orders brackets a crossing**: `f5-three-plane` leads the best
applicable in-tree CPU path at all five, from 3.1191× to 37.5365×. Where the
ratio crosses 1 below $n = 12$, or whether it does, is outside the grid and this
document locates no crossing for it
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.2):

| $n$ | best in-tree CPU path | rate | `f5-three-plane` / CPU | shipped `gpu_hip` / CPU |
| ---: | --- | ---: | ---: | ---: |
| 12 | `cpu_rayon_batch_scalar` | 66 412.1210 | 3.1191 | 0.3412 |
| 16 | `cpu_rayon_batch_scalar` | 3 816.8266 | 17.3246 | 0.3208 |
| 20 | `cpu_rayon_batch_scalar` | 197.5673 | 37.5365 | 0.3236 |
| 24 | `cpu_rayon_batch_scalar` | 10.7539 | 19.7082 | all four cells censored |
| 28 | `cpu_scalar` | 0.0454 | 34.7511 | all four cells censored |

**The shipped path stays below 1 everywhere it is measured, so it brackets no
crossing either.** Its ratio is 0.3412, 0.3208, and 0.3236 at the three orders it
is measured, and all four of its cells at $n \in \{24, 28\}$ are censored before
running because the projected repetition exceeds the 120 s cap
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.4, §5), so the two
largest orders carry no measured ratio for it at all.

Three qualifications travel with the prototype column, all from the campaign's
own record. The $n = 28$ denominator is `cpu_scalar` rather than batch rayon,
because the batch-rayon cell at that order is censored; the ratio against that
cell's projection would be 2.7386× if the projection held, and no ordering at
$n = 28$ is asserted from either number
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §14.8). Every prototype
cell sizes itself from a one-matrix probe, giving
$M \in \{1, 2, 6, 17, 41, 44, 45, 102\}$ against the control's 256 or 1024, so
no ratio separates the mapping effect from a device-parallelism effect spanning
a factor of 102 ([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.5,
§14.2). And the packed CPU baseline is uniformly slower in this run than in the
prior one, on all nine comparable pairs, mean $-6.34\%$ — so at $n = 20$ the
ratio is 37.5365 against this run's baseline and would be 34.7008 against the
prior run's ([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §11). The
verdict survives all three: the smallest prototype ratio anywhere in the field
is 3.1191×.

### 4.2 Two measured arithmetic representations (REQ-05)

Both planned $\mathbb{F}_5$ representations execute on the device, are
equivalence-identical, and are measured before either is selected
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §2, §3).

| | `f5-byte-control` | `f5-three-plane` |
| --- | --- | --- |
| lane state | byte column-sum array, $n + 20$ bytes source lower bound (`f5_wave_equivalence.hip:9-12`) | three packed `u64` planes, 11 32-bit units source lower bound (`f5_wave_equivalence.hip:13-15`) |
| row product | `byte_product`, serial over $n$ bytes (`f5_wave_equivalence.hip:169`) | `c4_product`, three-plane reduce (`f5_wave_equivalence.hip:182`) |
| dynamic shared per block | $n^2$ B, 144–784 B over the measured orders | $24n$ B, 288–672 B |
| `VGPRs` / `TotalSGPRs` / scratch B/lane | 77 / 78 / 0 | 66 / 20 / 0 |
| occupancy waves/SIMD | 12 | 12 |
| composite matrices/s, $n = 12 \ldots 28$ | 42 476.2230, 5 252.2160, 17.2007, 0.1560, censored | 207 146.4078, 66 125.0000, 7 415.9777, 211.9399, 1.5777 |

Resource figures from [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §7,
throughputs from §4.2 and §10, shared-memory formulas from §4.5.

The three-plane path leads the byte control at every order both are measured, by
4.8768, 12.5899, 431.1439, and 1358.5891 at $n = 12, 16, 20, 24$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.2). The two large
figures are batch-confounded — at $n = 20$ the byte control's batch collapses
from 102 to 6 matrices because its probe cost rises 19.9× and the calibration
under-sizes it, dropping it from 3 264 active lanes to 192
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.5).

**The one comparison of the two circuits at matched geometry is the
horizontal-product isolate**, where every row launches
`gridDim.x = sample_count` blocks of one thread with `sharedMemBytes = 0`
(`crates/gf2-kernels-hip/hip/permanent/horizontal_product_micro.hip:196`). On
the nonzero-slow branch — the complete reduction — the three-plane circuit is
cheaper at every order both are timed, and its advantage grows with $n$: 2.0530×
at $n = 16$, 3.3964× at $n = 20$, 3.7922× at $n = 24$, and 5.0907× at $n = 28$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §6.2). Its zero-fast
branch is censored at every order because the early exit runs faster than its
own same-geometry barrier baseline, and both its branches are censored at
$n = 12$; each censoring carries its reason verbatim
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §6.2).

**Selection: `f5-three-plane`.** It leads on end-to-end throughput at every
order, it is the only $\mathbb{F}_5$ path measured at all five orders, it costs
fewer per-lane vector registers (66 against 77) and far fewer scalar registers
(20 against 78), and it wins the one matched-geometry circuit comparison the
campaign supports. The byte control is retained in the record with its
measurements and its one censored cell rather than removed (§10).

## 5. $\mathbb{F}_7$ — go, on the bit-sliced three-plane prototype

**Verdict: go**, for `f7-three-plane-permanent` under the lane-owns-interval
mapping. At the declared operating point $n = 20$ it measures 8 397.8509
matrices/s at $M = 41$ against `cpu_ryser_generic` at 15.3900 — a factor of
**545.6693×** ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15). The
equivalence gate closes on all 24 $q = 7$ comparison cells with
`mismatches = 0`, both executing prototypes included, at every order the grid
times and at $n = 8$ below them
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §3).

**The declared operating point is inside this field's documented safe
launch-duration bound.** It runs a kernel span of 0.004672 s per launch on
$4.299 \times 10^{7}$ units of $M \cdot 2^n$
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15) against the $q = 7$
observed clean-completion envelope of 21.722805 s per launch and
$1.0737 \times 10^{9}$ work units
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4) — 0.000215 of the span
figure and 0.040 of the work figure, computed in §9 from those two sources.

### 5.1 The measured crossover condition (REQ-02)

**Both prototypes' ratios are above 1 at every measured order, so no measured
pair of adjacent orders brackets a crossing for either**; where the ratio crosses
1 below $n = 12$, or whether it does, is outside the grid and this document
locates no crossing for it. **The shipped path is the one $\mathbb{F}_7$ column
that does bracket a crossing**, upward between the measured $n = 16$ and
$n = 20$, which the campaign reports as beating the CPU "*at exactly one order*"
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2):

| $n$ | best in-tree CPU path | rate | best prototype / CPU | shipped `gpu_hip` / CPU |
| ---: | --- | ---: | ---: | ---: |
| 12 | `cpu_rayon_batch_scalar` | 71 761.1277 | 8.7878 | 0.2922 |
| 16 | `cpu_rayon_batch_scalar` | 3 703.3679 | 81.4163 | 0.2996 |
| 20 | `cpu_ryser_generic` | 15.3900 | 545.6693 | 3.7645 |
| 24 | `cpu_ryser_generic` | 0.7979 | 332.7897 | all four cells censored |
| 28 | `cpu_ryser_generic` | 0.0427 | 71.2108 | all four cells censored |

**That crossing is a property of the CPU side of the fraction.** The shipped
ratio moves from 0.2996 at $n = 16$ to 3.7645 at $n = 20$, and the receipt
states the mechanism outright: nothing about the
kernel changes at that step; what changes is that batch rayon leaves the
comparison, because `permanent_bipedal7` asserts
$n \le \texttt{Packed7::LANES} = 16$ and no rayon permanent path exists for this
field at all ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §2, §4.2).
The same change of denominator inflates the prototype column at $n \ge 20$: the
545.6693× is measured against a single-threaded generic Ryser driver, where the
81.4163× at $n = 16$ is measured against 24 rayon workers on a packed kernel
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2, §17.7). The
*ordering* is robust to that — the prototype leads at $n = 12$ and $n = 16$ too,
where a packed parallel CPU path exists — but the *magnitude* at $n \ge 20$ is a
statement about the missing CPU path as much as about the device. That absence
is itself a tracked gap: G8, "$q = 7$ CPU ceiling at $n = 16$", status
"structural limit"
([`../b488f02c/feasibility-study.md`](../b488f02c/feasibility-study.md):1040).

### 5.2 Two measured arithmetic representations (REQ-05)

Two of the three planned $\mathbb{F}_7$ representations execute as permanent
paths and are measured; the third is recorded with its falsification (§10, F.1).

| | `f7-lookup-table-control` | `f7-three-plane-permanent` |
| --- | --- | --- |
| lane state | $\lceil n/16 \rceil$ packed `u64` nibble words; 7 units at $n \le 16$, 9 above (`wave_gf7_equivalence.hip:14-17`) | three `u64` bit planes `b0`, `b1`, `b2`; 11 units (`wave_gf7_equivalence.hip:8-12`, `:75-79`) |
| arithmetic | the canonical two-nibble `Packed7` tables, uploaded from the public Rust arrays (`wave_gf7_equivalence.hip:25-29`) | Mersenne-fold plane add and subtract, no multiplication tables (`wave_gf7_equivalence.hip:82-109`, `:30`) |
| staging | in-kernel column pack | separate device kernel `prepare_three_plane_columns` (`wave_gf7_equivalence.hip:134-163`) |
| dynamic shared per block | $8n\lceil n/16 \rceil$ B, 96–448 B | $24n$ B, 288–672 B; 0 B for the staging kernel |
| `VGPRs` / `TotalSGPRs` / scratch B/lane | 31 / 19 / 0 (`<1>`), 41 / 26 / 24 (`<2>`) | 17 / 16 / 0 (staging), 34 / 16 / 0 (walk) |
| occupancy waves/SIMD | 16 on both instantiations | 16 on both kernels |
| composite matrices/s, $n = 12 \ldots 28$ | 630 625.0668, 145 932.9173, 408.7068, 1.5792, 0.0460 | 285 266.5608, 301 514.5246, 8 397.8509, 265.5329, 3.0407 |

Resource figures from [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §7,
throughputs from §4.2 and §10, shared-memory formulas from §4.5 and §7.1.

The bit-sliced path leads the lookup control at four of five orders — 2.0661,
20.5474, 168.1439, and 66.1022 at $n = 16, 20, 24, 28$ — and loses the fifth,
0.4524 at $n = 12$ ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2).
The campaign records the $n = 12$ reversal as a batch artefact as much as a
circuit result: the lookup control runs 5 833 matrices and 186 656 active lanes
there against the bit-sliced path's 46 and 1 472, and nothing in the run
isolates how much of the reversal is the circuit
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.5, §17.6). The three
largest of the four wins carry the same confound in the other direction
(§4.5, §17.5).

**The closest the campaign comes to matched geometry is $n = 16$**, where the
bit-sliced cell runs $M = 1109$ against the control mapping's 1 024 — 8.3 %
apart — and measures 271.7700× that mapping's rate
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §10). Against the lookup
control at the same order the ratio is 2.0661×, at batches 1 109 against 695.

**Bit-plane preparation is measured and is small.** The host-side portion is
zero by construction — the harness streams canonical matrix bytes and the
byte-to-plane transpose runs on the device as its own kernel — and the
device-side portion, resolved by the paired profiled run, is 8.4207 %, 0.7347 %,
0.0571 %, and 0.0045 % of the two-kernel pair at $n = 12, 16, 20, 24$. At the
declared operating point staging costs 0.0571 % of the device time the path
spends per launch ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §12).

**The bit-sliced path operates exactly above the sixteen-lane packed bound.** It
is `measured` and `identical` with zero mismatches at $n = 16$, 20, and 24, on
512, 512, and 32 matrices, against an oracle that is the independent generic
Ryser driver at the two orders where the packed CPU kernel refuses to run
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §14).

**Selection: `f7-three-plane-permanent`.** It leads end to end at four of five
orders including the declared operating point, it is measured at all five, it
costs the fewest per-lane vector registers of the walk kernels (34 against 41
for the lookup control's large instantiation) with zero scratch against that
instantiation's 24 bytes per lane, it needs no multiplication tables, and it
operates exactly above the sixteen-lane bound the public packed kernel stops at.
The lookup control is retained with its measurements, including the one order
where it wins.

## 6. Production design behind each go (REQ-06)

One design, instantiated per field. It applies to the two go verdicts,
$\mathbb{F}_5$ on `f5_three_plane_kernel` and $\mathbb{F}_7$ on the
`prepare_three_plane_columns` / `wave_gf7_three_plane_kernel` pair. It does not
apply to $\mathbb{F}_3$, whose verdict is no-go.

The design converges on the device infrastructure that already exists and is
public in [`crates/gf2-kernels-hip/src/host/`](../../../crates/gf2-kernels-hip/src/host/mod.rs),
rather than growing a parallel one. That module re-exports `DeviceBuffer` and
`PinnedHostBuffer` from `alloc`, `GfxTarget` from `arch`, `HipEvent` and
`HipEventSpan` from `events`, `LaunchDims` and `MAX_BLOCK_THREADS` from
`launch`, and `HipStream` and `HipStreamPool` from `streams`
(`crates/gf2-kernels-hip/src/host/mod.rs:32-36`).

**The gap this design closes is measured, not asserted.** The permanent
dispatcher bypasses that infrastructure today. Each call allocates two fresh
device buffers and frees them on return
(`crates/gf2-kernels-hip/src/permanent/mod.rs:2275`, `:2277` for $\mathbb{F}_3$;
`:2373`, `:2375` for $\mathbb{F}_5$; `:2477`, `:2479` for $\mathbb{F}_7$),
through the crate-private `DecoderDeviceBuffer`
(`crates/gf2-kernels-hip/src/lib.rs:279`) rather than the public
`DeviceBuffer`; it panics on every allocation and copy failure
(`permanent/mod.rs:2276`, `:2278`, `:2282`, `:2315`) and asserts on every device
return code (`:2295`, `:2302`, and the same pair per field at `:2391`, `:2398`,
`:2497`, `:2505`). The cost of the per-call host work is visible in the
receipts: the host-side residual around each dispatch is 70.0 % of `eval_s` at
$n = 12$, $M = 256$ on the shipped $\mathbb{F}_3$ path, and the campaign
attributes it to the dispatcher's per-call allocation, serialisation, and free
rather than to transfer or launch
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.3).

### 6.1 Input layout

The device kernel reads canonical row-major matrix bytes, one $n \times n$ byte
matrix per batch slot, exactly the serialisation the measured prototype consumes
(`dev/research/permanent_wave_gpu/hip/wave_batch_stream.h:196-203`). Packing to
the field representation happens on the device, not the host: for
$\mathbb{F}_7$ the byte-to-plane transpose is its own kernel
(`wave_gf7_equivalence.hip:134-163`), which is why the host-side preparation
cost is zero by construction and not merely unmeasured
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §12). Keeping the host
layout at canonical bytes preserves that property and keeps the public
`Packed5Matrix` and `Packed7Matrix` surfaces
(`crates/gf2-algebra/src/packed/packed5.rs:1420`,
`crates/gf2-algebra/src/packed/packed7.rs:1345`) as the caller-facing input
types.

Per-block shared memory is the read-only staged column table the launch
requests: $24n$ bytes for both three-plane walks, 288–672 B over the measured
orders. The largest request over every retained path and measured order is
784 B, and 32 such blocks — the device's own per-CU wave-slot cap — request
25 088 B against 65 536 B of LDS per compute unit, so shared memory does not
limit the occupancy of any retained path at any measured order
([`../a9284086/receipt.md`](../a9284086/receipt.md) §4).

### 6.2 Persistent device buffers and streams

Input and output buffers become `DeviceBuffer<u8>` and `DeviceBuffer<u32>` owned
by a per-device dispatcher context and sized once for the configured maximum
batch, replacing the per-call allocation cited above. `DeviceBuffer` already
provides the whole surface this needs: `new` and `new_with_fallback`
(`crates/gf2-kernels-hip/src/host/alloc.rs:202`, `:298`), `copy_from_host` and
`copy_to_host` (`:432`, `:479`), the pinned asynchronous pair
`copy_from_pinned_async` and `copy_to_pinned_async` (`:535`, `:593`), and RAII
release on drop (`:627`). Host staging buffers become `PinnedHostBuffer<T>`
(`alloc.rs:656`) so the asynchronous copies have page-locked memory to work
against.

Streams come from `HipStreamPool`
(`crates/gf2-kernels-hip/src/host/streams.rs:221`), constructed once per device
with `new(device_id, n)` (`:266`) and acquired per launch with `acquire` or
`acquire_idle` (`:352`, `:426`), with `synchronize_all` (`:463`) at the context's
own drain points. This replaces the dispatcher's current internal device-wide
`hipDeviceSynchronize` per call (`permanent/mod.rs:2300-2304`), which the
campaigns record as a per-call serialisation the shipped path pays at every
order ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.3).

Launch geometry is `LaunchDims`
(`crates/gf2-kernels-hip/src/host/launch.rs:43`). The lane-owns-interval mapping
needs the explicit constructor rather than the batch heuristic:
`LaunchDims::explicit(grid_x, block_x)` (`:124`) with `grid_x` the sub-batch
matrix count and `block_x` = `active_lanes_for_order(n)`, which is 32 at every
measured order (`wave_ryser_mapping.h:29-31`). That value is well inside
`MAX_BLOCK_THREADS` at 256 (`launch.rs:33`).

Device-side timing uses `HipEvent` and `HipEventSpan`
(`crates/gf2-kernels-hip/src/host/events.rs:22`, `:148`), which is the same
device-event instrumentation the campaigns' `kernel_device_s`, `h2d_device_s`,
`d2h_device_s`, and `device_submission_to_kernel_s` columns rest on
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.3).

### 6.3 Bounded-duration launches

Each evaluation of $M$ matrices is split into sub-batches, and each sub-batch is
one dispatch: one timed repetition evaluates one batch and one batch evaluation
is one dispatch
(`dev/research/permanent-sampling-feas/src/protocol.rs:765-781`).

**The bound the sub-batch is sized against is a per-launch device *span*
target, not a per-launch work budget, and the runtime qualification measures
why.** At the one value of $M \cdot 2^n$ that five committed cells share, the
measured per-launch spans run from 0.117988 s to 115.452922 s — a factor of
978.5 — so a budget on $M \cdot 2^n$ does not bound per-launch device time
across paths, and any use of the archived per-field work budgets outside the
single kernel they were calibrated on carries that error
([`../a9284086/receipt.md`](../a9284086/receipt.md) §8, §9.4). The design
therefore takes a configured per-launch span target, calibrates $M$ against it
from a measured span rather than from a single-matrix probe, and re-calibrates
when the observed span drifts from the target.

The span target must sit inside the documented safe launch-duration bound — the
observed clean-completion envelope of §1, per field, a conservative lower bound
on the safe region rather than a located boundary
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4): per-launch spans to
115.452922 s complete without fault at $q = 5$ and to 21.722805 s at $q = 7$.
The two go operating points sit far inside both — 0.0800 s per launch at
$q = 5$, $n = 24$ and 0.004672 s at $q = 7$, $n = 20$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15) — so the constraint the
target actually binds is device utilization, not safety. The nearest counter
readings are 0.3515 % of the device's 2 560 wave slots for
`f5_three_plane_kernel` on a 9-wave launch at $q = 5$, $n = 24$
([`../a9284086/receipt.md`](../a9284086/receipt.md) §5) and 0.5043 % for
`wave_gf7_three_plane_kernel` on a 13-wave launch at $q = 7$, $n = 20$
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13); both come from
profiled cells that re-calibrate their own batches and carry no timing
authority, so they bound utilization at those cells rather than at the timing
cells' batches. §12 records that this design cannot be validated at a wide grid
from the current evidence.

An explicit host cooldown between sub-batches is *not* carried into this design.
The archived calibration's 400 ms cooldown is derived from the watchdog
mechanism whose attribution is retracted
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.1), and no committed
measurement in this study supports or refutes it. It is a configuration knob
defaulting to zero, with the archived prior cited as the reason it exists.

### 6.4 Per-device initialization

A dispatcher context is created once per device and holds: the resolved
`GfxTarget` from `GfxTarget::detect_device(device_id)`
(`crates/gf2-kernels-hip/src/host/arch.rs:189`), the loaded kernel blob for that
target via `has_compiled_blob` and `load_blob` (`:310`, `:343`), the
`HipStreamPool` for the device, the persistent `DeviceBuffer` and
`PinnedHostBuffer` pairs, and the uploaded constant tables where the
representation needs them. For $\mathbb{F}_7$ the three-plane path needs none:
it has no multiplication tables (`wave_gf7_equivalence.hip:29`), which removes
the shipped path's per-process 3 × 64 KiB table upload
(`crates/gf2-kernels-hip/src/permanent/mod.rs:174-176`) from the initialization
sequence entirely.

Initialization is where the architecture check happens, once, rather than per
call. The committed architecture scope of this study is one target, `gfx1030`
([`../a9284086/receipt.md`](../a9284086/receipt.md) §12.8), so
`GfxTarget::detect_device` returning anything else is an unsupported capability
and takes the fallback path of §6.5 rather than reaching a launch.

### 6.5 Error propagation

The repository's accelerator contract is `@/inv/accelerator-safe-fallback`:
unsupported SIMD or GPU capabilities and recoverable accelerator resource
failures select a tested safe fallback; fatal kernel or driver failures remain
explicit. Three classes, three behaviours:

**Unsupported capability selects the CPU path.** `GfxTarget::detect_device`
returning a target with no compiled blob, or returning
`HipError::UnsupportedArch` (`crates/gf2-kernels-hip/src/host/arch.rs:219`,
`:229`), resolves the batch on the in-tree CPU kernel: `permanent_bipedal5` for
$\mathbb{F}_5$ (`crates/gf2-algebra/src/permanent/bipedal5.rs:108`) and
`permanent_ryser` over `Fp<7>` for $\mathbb{F}_7$ above the sixteen-lane bound,
which is the oracle the $\mathbb{F}_7$ equivalence gate already uses at those
orders ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §3).

**Recoverable resource failure selects the CPU path.** Allocation goes through
`DeviceBuffer::new_with_fallback`
(`crates/gf2-kernels-hip/src/host/alloc.rs:298`), which is built for exactly
this: it pre-flights the request against the target device's own total memory
and returns `HipError::OutOfMemory` rather than panicking, and its rustdoc
states the contract — "*paths surface [`HipError::OutOfMemory`] so the executor
can substitute a CPU fallback (design doc §8) — never a panic*"
(`alloc.rs:270-271`). The dispatcher maps that variant, and stream-acquisition
failures from `HipStreamPool::acquire_idle` (`streams.rs:426`), onto a fallback
selection with a `tracing::warn!`, which is the behaviour `HipError`'s own
documentation already describes for the pipeline executor.

**Fatal device failure stays explicit.** A nonzero return code from the kernel
launch or from a synchronize that is not an out-of-memory condition propagates
as `Err(HipError::Hip { .. })` to the caller. It does not silently retry and it
does not silently fall back, because a driver fault of unidentified cause is
exactly the condition this study's one recorded device fault leaves unexplained
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.3). What changes from
today is the mechanism, not the visibility: the `assert_eq!(rc, 0, ...)` sites
at `permanent/mod.rs:2295`, `:2302`, `:2391`, `:2398`, `:2497`, and `:2505`
become typed errors, so a caller can observe and record the fault instead of
losing the process.

This is a convergence onto the existing mechanism rather than a new one:
`new_with_fallback`, the `HipError::OutOfMemory` variant, and the
`UnsupportedArch` variant all exist and are documented for this use; what is
missing is a consumer. A read-only sweep of `gf2-kernels-hip` and `gf2-algebra`
finds no CPU fallback selection implemented anywhere in either crate today, and
the permanent API's own rustdoc documents panic on nonzero HIP returns
(`crates/gf2-algebra/src/gpu.rs:340-341`, `:438`, `:542-544`).

### 6.6 Behavioral-equivalence coverage

`@/inv/backend-behavioral-equivalence` and `@/inv/shared-test-contracts` require
the GPU path to run the same behavioral conformance suite as the scalar, SIMD,
and parallel paths. The coverage this design carries:

1. **The shared field-law suite** on the field arithmetic
   (`@/inv/finite-field-laws`), unchanged, since the internal three-plane state
   of §7 is an implementation of the same field.
2. **Per-matrix equality against the CPU oracle** on one identical corpus per
   $(q, n)$, which is the campaigns' existing gate shape: the batch is built
   once and reused across the whole backend loop
   (`dev/research/permanent-sampling-feas/src/equivalence.rs:139`, `:186-192`).
   The oracle is `permanent_bipedal5` for $\mathbb{F}_5$ at all orders and
   `permanent_bipedal7` for $\mathbb{F}_7$ at $n \le 16$ with `permanent_ryser`
   above it, which is the switch the $\mathbb{F}_7$ equivalence file already
   makes and states in its own preamble
   ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §3).
3. **Representation-boundary orders.** The binding boundaries for the permanent
   are the representation limits the receipts already exercise — $n = 16$, where
   `permanent_bipedal7` asserts and the oracle switches
   ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §2), and $n = 63$, the
   single-word ceiling both packed CPU kernels assert on
   (`crates/gf2-algebra/src/permanent/bipedal5.rs:117-123`) — plus $n < 5$,
   where `active_lanes_for_order` returns $2^n$ rather than 32
   (`wave_ryser_mapping.h:29-31`) and the lane partition changes shape.
4. **Determinism across the fallback boundary**
   (`@/inv/deterministic-seeded-execution`): a fixed seed and configuration
   produce identical results on the device path and on the CPU path it falls
   back to. The mapping already supplies the device half — the cross-lane
   reduction reads sources in fixed increasing lane order, so repeated launches
   have no schedule-dependent reduction order (`wave_ryser_mapping.h:64-66`) —
   and the sub-batching of §6.3 must be shown not to change which matrix is the
   $i$-th sample, the property the archived chunked path asserts with
   `validate_chunked_equals_unchunked`
   (`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`:239-243).
5. **The fallback paths are tested paths.** The invariant requires the fallback
   to be *tested*, so each fallback branch is exercised by a test that forces
   it: an unsupported-architecture branch and a recoverable-OOM branch, the
   latter reachable through `new_with_fallback`'s documented behaviour on an
   over-total-memory request (`alloc.rs:281-290`).

Machine-dependent throughput assertions are benchmark gates and not ordinary
tests (`@/inv/benchmark-backed-performance`); the equivalence coverage above is
the ordinary-tier obligation.

## 7. The representation boundary (REQ-07)

**Two distinct surfaces.** They are not the same object and this document names
them apart:

- **The public packed-field representation** for $\mathbb{F}_7$ is `Packed7`
  (`crates/gf2-algebra/src/packed/packed7.rs:211`): one `u64` holding
  `LANES = 16` elements as 3-bit values at 4-bit-aligned slots, with binary
  operations by $2^{16}$-entry lookup table (`packed7.rs:216`, and the archived
  decision that chose it, §8). It is the canonical abstraction:
  `Packed7Matrix` (`packed7.rs:1345`) is the caller-facing input type,
  `permanent_bipedal7` is written against it, the HIP shipped path uploads its
  exact table byte arrays
  (`crates/gf2-kernels-hip/src/permanent/mod.rs:174-176`), and
  `proofs/Gf2Algebra/Proofs/Packed7Correctness.lean` verifies it.
- **The permanent-specialized internal state** is the three-plane bit-sliced
  accumulator `ThreePlane { b0, b1, b2 }`
  (`dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip:75-79`), with
  Mersenne-fold add and subtract (`:82-109`) and the plane-reduce row product
  (`:111`). It exists only inside the permanent kernel, holds one lane's column
  sums for the duration of that lane's Gray interval, and never crosses a lane
  or a public API boundary.

**Choice: the second compliant resolution.** Keep `Packed7` as the canonical
public packed $\mathbb{F}_7$ representation and define the three-plane state as
an internal permanent-kernel representation with explicit scope, shared
behavioral tests, and a tracked convergence condition
([`../../active/0de41c82/bipedal-f5-f7-representation-study.md`](../../active/0de41c82/bipedal-f5-f7-representation-study.md):255-260,
option 2).

**Grounding in the committed evidence.** The other compliant resolution —
changing `Packed7` at its source — is conditioned on "*broader `PackedField`
evidence*" (same source, option 1), and this study's evidence is not broader in
the ways that matter. It is one architecture, `gfx1030`, on one host
([`../a9284086/receipt.md`](../a9284086/receipt.md) §12.8). It is device
evidence only: nothing here re-measures the CPU packed encoding, which is the
surface `Packed7` actually is, and the archived decision that chose the LUT
encoding was decided on a scalar CPU prototype (§8). Almost no prototype cell is
measured near the control's batch sizes, so the end-to-end ratios mix the
mapping effect with a device-parallelism effect spanning a factor of 5 833
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.5). And the campaign
declines to order the two $\mathbb{F}_7$ arithmetic circuits against each other:
the three-plane circuit yields a nonzero-branch duration at exactly two cells,
with the partner branches censored, so the 23.3× and 312× gaps against the
lookup circuit are reported as the two figures they are and no ordering is
asserted from them ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.8).
Changing a canonical public representation on evidence the producing receipts
decline to order it on would be a change the record does not support.

**Grounding in the repository's conventions.** `@/inv/convention-convergence`
forbids a local parallel variant "*unless it is a named, cited exception with a
tracked convergence condition*", which is why this resolution carries the
exception below as a deliverable rather than a note
([`../../active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):717-737).
The proof-obligation asymmetry decides the cost side: a new *public* packed
representation attracts a proof obligation by convention, and `AGENTS.md`
requires an approved sketch — lemma statements, strategy, exact production path
— before proof code; a permanent-internal, non-public kernel state does not
obviously attract one
([`../../active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):291-295).
Option 1 also reaches the SIMD mirror
(`crates/gf2-kernels-simd/src/bipedal/packed7.rs` and
`x86/bipedal_avx2_packed7.rs`), the HIP LUT upload path, and
`Packed7Correctness.lean` (`investigation.md`:730-737). The study's own plan
records the same conclusion in advance and leaves option 1 to a new bracket
([`../../active/0de41c82/plan.md`](../../active/0de41c82/plan.md):177).

**$\mathbb{F}_5$ needs no exception, and the asymmetry is itself evidence for
the choice.** `Packed5` is already a three-plane representation —
`Packed5 { b0, b1, b2 }` with a five-way decode-then-cross-product Boolean
circuit and no lookup table
(`crates/gf2-algebra/src/packed/packed5.rs:184-187`, `:208-212`) — because the
archived $\mathbb{F}_5$ decision chose exactly that candidate
([`../../active/0de41c82/investigation.md`](../../active/0de41c82/investigation.md):369-371).
So the $\mathbb{F}_5$ prototype's internal state and the public representation
agree in shape, and the $\mathbb{F}_5$ production design of §6 introduces no
second representation at all. The boundary question is $\mathbb{F}_7$-only, and
it is $\mathbb{F}_7$-only because the archived $\mathbb{F}_7$ decision went the
other way. That is the same decision §8 addresses.

### 7.1 The named exception and its tracked convergence condition

> **Exception `permanent-f7-internal-three-plane`.** The $\mathbb{F}_7$
> permanent kernel holds its per-lane column-sum accumulator in a three-plane
> bit-sliced state (`b0`, `b1`, `b2`) rather than in the canonical public
> `Packed7` representation (`crates/gf2-algebra/src/packed/packed7.rs:211`).
>
> **Scope.** The state exists inside the $\mathbb{F}_7$ permanent kernel only:
> per lane, for the duration of that lane's Gray interval. It is not a
> `PackedField` implementation, it appears in no public signature, it crosses no
> lane boundary (`wave_gf7_equivalence.hip:4-6`), and every value entering or
> leaving the kernel is canonical row-major $\mathbb{F}_7$ bytes or a scalar
> `Fp<7>`.
>
> **Why it exists.** The archived $\mathbb{F}_7$ encoding decision rejected the
> three-plane candidate under a Ryser workload model whose weighting this
> study's receipts contradict (§8), and the bit-sliced path is the measured
> winner on the permanent workload at four of five orders and at the declared
> operating point (§5). The evidence is device-only and single-architecture, so
> it does not yet support changing the public representation (§7).
>
> **Shared behavioral tests.** The internal state runs the same per-matrix
> equivalence gate against the same CPU oracle as every other backend, on one
> identical corpus per $(q, n)$ (§6.6), and its results are `identical` with
> zero mismatches at every order the grid times
> ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §3, §14).
>
> **Tracked convergence condition.** The exception is discharged, and the two
> representations converge on one form, when *either* of the following holds:
>
> 1. **Convergence upward.** A committed receipt orders the three-plane and
>    lookup $\mathbb{F}_7$ arithmetic circuits against each other at matched
>    launch geometry — the comparison
>    [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.8 declines to make
>    — *and* a CPU-side re-measurement on the `PackedField` surface reproduces
>    the ordering, on the workload the archived decision weighted (§8). Both
>    together are the "broader `PackedField` evidence" option 1 is conditioned
>    on. `Packed7` then changes at its source, under an approved Lean sketch,
>    and the internal state is removed.
> 2. **Convergence downward.** The receipts fail to reproduce the ordering, or
>    the $\mathbb{F}_7$ go verdict of §5 is withdrawn. The internal state is
>    removed and the permanent kernel returns to `Packed7`.
>
> Until one of those resolves, the exception is cited at the internal state's
> definition site and at the $\mathbb{F}_7$ dispatcher, and this document is the
> record it cites. The tracked issue carrying the condition is `d2fb76b9`
> (*Converge the F_7 permanent kernel's internal three-plane state with the
> public Packed7 representation*), filed under the quality and tech-debt
> umbrella when this synthesis landed.

## 8. The archived $\mathbb{F}_7$ encoding decision (REQ-08)

**Determination: the study's receipts support the three-plane $\mathbb{F}_7$
candidate under the permanent workload.** Under REQ-08 that means the archived
decision is amended at its source with the permanent-workload evidence and the
changed verdict.

**The archived decision, and what it rejected.**
[`dev/archive/ae82bd73-gf2-algebra-permanent/plans/f10152f6/r2_f7_encoding_decision.md`](../../archive/ae82bd73-gf2-algebra-permanent/plans/f10152f6/r2_f7_encoding_decision.md)
ratifies Candidate A — 3-bit values at 4-bit-aligned slots with $2^{16}$-entry
binary-op LUTs — as the $\mathbb{F}_7$ packed encoding (`:9-23`, `:220-228`).
Candidate D is the bit-sliced three-plane canonical $(b_0, b_1, b_2)$ encoding
with Mersenne-fold add and subtract (`:50`). D was rejected on a weighting: "*For
the Gray-code Ryser kernel, each Gray-code transition does 1 packed add/sub
(column-sum update) and ~`n−1` packed muls (row-product update). Per Gray-code
step at n=36, the mul:add ratio is 35:1 ≈ 97% mul, 3% add*" (`:145-148`). D is
9.5× faster on add and about 1.7× slower on mul, so under a 97 % mul weighting A
wins by 1.61× (`:164-184`). The document states that a re-bench "*is not a
re-decision authority for T19*" (`:196`, `:210-211`).

**Three strands of this study's evidence bear on it, and they are not equally
strong.**

*Strand 1 — the workload model's weighting is contradicted by measurement, and
this is the strongest strand.* The archived model charges the full row-product
reduction on every Gray step. The shipped and prototype $\mathbb{F}_7$ kernels
do not: the row product has an early zero exit, and the campaign measures how
often the complete reduction actually runs. The nonzero-slow branch — the
complete reduction — is reached with exact marginal frequency $(6/7)^n$:
0.157267 at $n = 12$, 0.084889 at $n = 16$, 0.045821 at $n = 20$, 0.024733 at
$n = 24$, and 0.013350 at $n = 28$. The receipt checks each against two
observations of the same branches, and the two do not agree at one order. On the
4 096-sample observation batch, all five expectations fall inside their two-sided
Wilson 95 % intervals. On the 6.8–7.8 million timed operations of the same file,
four of the five do and $n = 24$ does not: the interval there is
$[0.024785161, 0.025014948]$ against the exact 0.024733014, a $+2.85\sigma$
excess reproduced on all four backends that resample the same addresses. The
receipt records that miss rather than smoothing it, as the one cell of the
campaign where an observed branch frequency and its exact marginal are
inconsistent at the stated coverage, and states its reading: one order of five
under a nominal 95 % procedure with no multiplicity adjustment, with the other
four at $|z| \le 1.24$ and the 4 096-sample observation at $n = 24$ covering
comfortably ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §9, §17; §10,
F.10 here). So at $n = 20$ — an order both observations cover, at $z = -0.85$ on
the tighter one — the complete reduction runs on 4.58 % of Gray steps, not on
every one of them.
The archived model's premise — that the permanent workload is mul-dominated at
the ratio $n{-}1 : 1$ — is falsified for the kernels this study measures, and falsified by
a quantity with an exact closed form rather than by a timing.

*Strand 2 — the two circuits' measured costs, where they can be compared at
matched geometry.* On the horizontal-product isolate, which launches
`gridDim.x = sample_count` blocks of one thread with `sharedMemBytes = 0`, the
three-plane reduction measures $6.00 \times 10^{-10}$ s against the lookup
circuit's $1.4005 \times 10^{-8}$ s at $n = 24$, and $9.6 \times 10^{-11}$ s
against $2.9996 \times 10^{-8}$ s at $n = 28$ — 23.3× and 312× apart
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §6.2). **This strand is
explicitly not an ordering.** Those are two cells whose partner branches are
censored, and the campaign asserts no ordering of the circuits from them
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.8). It is carried here
because it is the only matched-geometry comparison in the record and because its
direction is the opposite of the archived model's mul prediction, not because it
settles anything.

*Strand 3 — end to end, on the permanent workload, on the device.* The
bit-sliced path leads the lookup control at $n = 16$, 20, 24, and 28 by 2.0661,
20.5474, 168.1439, and 66.1022, and loses at $n = 12$ by 0.4524
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2). The lookup control
here *is* the archived Candidate A: it "*retains the current canonical
two-nibble F_7 table layout*" and receives "*all three public canonical Rust
`Packed7` table byte arrays over stdin and uploads those exact bytes before a
control launch*" (`wave_gf7_equivalence.hip:25-29`). So the comparison is A
against D on the permanent workload, which is the comparison the archived §6
performs by model. Every one of those ratios is batch-confounded, in both
directions, and the campaign says so (§4.5, §17.5, §17.6).

**What the amendment must and must not claim.** The archived decision is a *CPU
packed-encoding* decision, benchmarked with a scalar Rust prototype on the same
Ryzen 9 5900X host (`r2_f7_encoding_decision.md:52-69`). This study measures
*device* kernels on `gfx1030` and re-measures nothing on the CPU. The verdict
that changes is therefore scoped: the workload model in `r2` §6 does not
describe the permanent workload these kernels run, and on that workload the
three-plane candidate is the measured winner on the device. Candidate A remains
the public `Packed7` encoding, because no CPU measurement in this study
displaces it and because §7 chooses the internal-state resolution. The amendment
records a falsified premise and a scoped changed verdict, not a re-decision of
the public encoding.

**Status: applied with owner approval (2026-08-17).** `dev/archive/` is
permanent repository content and amending it needs the owner's approval
([`../../active/0de41c82/plan.md`](../../active/0de41c82/plan.md):174); the
owner approved the drafted text verbatim and the note and pointer stubs of
[`req08-amendment-draft.md`](req08-amendment-draft.md) are inserted in the
archived file at the draft's stated insertion points. The amendment follows the
format the same archived directory already uses: the 2026-08-16 supersession
note above `r4_gpu_uniformity_resample.md` §2.5 preserves the original text
unchanged beneath it and cross-cites rather than rewriting
(`@/inv/falsification-preserved`).

**What the amendment changes is scoped as stated above**: the standing of the
§6 workload model and the scope of the archived verdict. `Packed7` remains
Candidate A, and §7's internal-state choice is consistent with that.

## 9. The 1.5× operating-point question (REQ-11)

**The throughput half is met at both declared operating points, with no
shortfall.**

| field | declared point | best prototype | its rate | best applicable in-tree CPU path | its rate | measured factor | receipt |
| --- | ---: | --- | ---: | --- | ---: | ---: | --- |
| $\mathbb{F}_5$ | $n = 24$ | `f5-three-plane`, $M = 17$ | 211.9399 | `cpu_rayon_batch_scalar`, $M = 96$ | 10.7539 | **19.7082×** | [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12 |
| $\mathbb{F}_7$ | $n = 20$ | `f7-three-plane-permanent`, $M = 41$ | 8 397.8509 | `cpu_ryser_generic`, $M = 31$ | 15.3900 | **545.6693×** | [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15 |

Both exceed 1.5×; there is no shortfall to record on either. The $\mathbb{F}_7$
magnitude carries §5.1's caveat: its denominator is a single-threaded generic
Ryser driver, because this field has no packed CPU kernel and no rayon permanent
path at $n = 20$.

**The inside-the-bound half is answered against the documented safe
launch-duration bound, which is the runtime qualification's observed
clean-completion envelope per field.** That is the per-field bound the study
derives and commits — "*the watchdog-safe per-launch bound this study supports is
the envelope of §9.2*" ([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4)
— and it is a *conservative lower* bound on the safe region, because "*No launch
in this study was run to failure, so none of these is an upper bound on where
faults begin, and no boundary is located*" (§9.4), which its limitations restate
as "*No watchdog boundary is located. §9's bound is an envelope of observed clean
completions, not an upper bound on safe operation*" (§12.1). Inside it means
inside a per-launch span and per-launch work at which launches of a retained path
are observed completing without device fault on this host; it does not mean below
a located fault threshold, because none is located. Two documented limits bear on
the question, and this document answers against both, naming each with its
status.

**(a) The documented safe launch-duration bound: this study's observed
clean-completion envelope per field**
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.2, §9.4):

| field | point's span per launch | field's observed span envelope | share | point's $M \cdot 2^n$ | field's observed work envelope | share |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| $\mathbb{F}_5$, $n = 24$ | 0.0800 s | 115.452922 s | 0.00069 | $2.852 \times 10^{8}$ | $1.0737 \times 10^{9}$ | 0.266 |
| $\mathbb{F}_7$, $n = 20$ | 0.004672 s | 21.722805 s | 0.000215 | $4.299 \times 10^{7}$ | $1.0737 \times 10^{9}$ | 0.040 |

Spans and works from [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12
and [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15; envelopes from
[`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4. The two share columns
are computed here from those two sources. Both operating points are inside the
envelope of clean completions their own field records, by three to four orders
of magnitude on the span axis.

**(b) The archived prior calibration**, cited as a prior and never as an
established device property, because the mechanism it names was retracted for
the one fault with a committed record
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.1). It places a hang
boundary at "*≈190–200 s*" per launch and sets per-field work budgets on
$\text{sub\_batch} \cdot 2^n$ of $4.0 \times 10^9$ at $q = 3$,
$1.3 \times 10^9$ at $q = 5$, and $3.5 \times 10^8$ at $q = 7$
(`dev/archive/ae82bd73-gf2-algebra-permanent/plans/b293af5a/r4_gpu_uniformity_resample.md`:225-227),
and it carries a dated supersession note stating that its budgets are not a
necessary condition for safe operation and its boundary is not a measured device
property (`r4_gpu_uniformity_resample.md`:176-215). Against it: the
$\mathbb{F}_5$ point is 0.0004 of the span boundary and 0.2194 of the $q = 5$
work budget ([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12); the
$\mathbb{F}_7$ point is 0.00002 of the span boundary and 0.1228 of the $q = 7$
work budget, both read from the $\mathbb{F}_7$ receipt, which applies the
archived $q = 7$ budget to its own field alongside that span boundary
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15, §16 REQ-19). §10
records the falsified premise that receipt preserves beside the application
(F.4).

**The statement REQ-11 asks for, made exactly.** For $\mathbb{F}_5$ and for
$\mathbb{F}_7$, the declared scientifically relevant operating point reaches at
least 1.5× the best applicable in-tree CPU throughput — at 19.7082× and
545.6693× — and does so inside the documented safe launch-duration bound, its own
field's observed clean-completion envelope, on both the span axis and the work
axis, by three to four orders of magnitude on the span axis. Each point is also
inside the archived prior's span boundary and work budget, the other documented
limit. The two go verdicts therefore clear all four conditions of §1's decision
rule.

**What that bound is, stated with its scope.** The envelope is observed rather
than derived, and it is a conservative lower bound: no launch in the evidence was
run to failure, so no upper boundary on safe operation is located, and this
document claims none. The longest span the study observes anywhere is
115.452922 s against the archived ≈190–200 s figure that nothing here probes
above, and the one recorded fault's cause and span remain unknown
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4, §12.1, §12.2). Locating
an upper boundary needs a launch run to failure, which no committed run performs;
it is an open deliverable and §12.1 carries it. What the verdicts rest on is the
bound the record documents, not the boundary it does not.

## 10. Falsification record (REQ-09)

Falsified hypotheses, rejected candidates, censored measurements, and
disagreements between committed artifacts, recorded with the evidence
contradicting them.

**F.1 — A planned $\mathbb{F}_7$ candidate does not execute as a permanent
path.** `f7-three-plane-accumulator` is `unsupported` in all five grid cells and
in every equivalence row. The falsification is structural rather than a compile
or resource failure: its HIP source compiles clean for `gfx1030` — the resource
receipt carries `f7_three_plane_equivalence` at 40 `TotalSGPRs` and 2 `VGPRs` —
but the translation unit holds a single-thread arithmetic conformance probe
rather than a full-permanent batch kernel
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §2). It is retained in the
record and appears in the horizontal-product isolate, because that isolate times
a circuit rather than a path.

**F.2 — The $\mathbb{F}_3$ crossover closes before the declared operating
point.** The prototype leads the CPU at $n \in \{16, 20, 24\}$ and loses at
$n = 28$ by a factor of 4.9. The interior win is preserved with its confound
rather than dropped (§3.2).

**F.3 — Censored cells carry no rate, and their projections are not uniformly
conservative.** One $q = 3$ cell, six $q = 5$ cells, and four $q = 7$ cells are
censored; each states its reason and carries `NaN` for both throughput columns
([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §5,
[`../91605d4d/receipts.md`](../91605d4d/receipts.md) §5,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §5). The grid preamble's
"*lands LOW at every step*" projection claim holds on the fixed-batch `gpu_hip`
chains and fails on the probe-calibrated prototype chains, where projections
land high by up to 1685.3 %. The $\mathbb{F}_3$ campaign predicted this before
any such cell existed; the $\mathbb{F}_5$ campaign has one, `f5-byte-control` at
$n = 28$, on exactly such a chain
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §5, §14).

**F.4 — The $\mathbb{F}_7$ receipt carries a falsified premise about the archived
$q = 7$ work budget beside its correction, and this synthesis is where the
disagreement surfaced.** The archived source commits per-field work budgets on
$\text{sub\_batch} \cdot 2^n$ — "*q=3: 4.0e9; q=5: 1.3e9; q=7: 3.5e8 — the F_7
LUT kernel is ≈5× slower so it gets a lower budget*"
(`r4_gpu_uniformity_resample.md`:225-227) — and the runtime qualification
tabulates the $q = 7$ figure of $3.5 \times 10^8$ and records it exceeded 3.07×
without fault ([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.1, §9.2,
§10.2). The $\mathbb{F}_7$ receipt's §15 holds the contradicting premise —
the budget "*is stated for $q = 5$; no $q = 7$ work budget is committed*" —
preserved verbatim under a dated **Correction (2026-08-17)** heading that states
it false and cites the archived line (`@/inv/falsification-preserved`). That §15
applies the archived $q = 7$ budget to its own field alongside the archived span
boundary, records the 3.07× exceedance with the runtime qualification's
citations, and scopes its inside-with-margin claim to the span boundary
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15). The defect is this
study's finding, filed as JIT issue `e1c96c9c` and discharged in the producing
receipt rather than here. No verdict in this document turns on the budget either
way: applying it only tightens §9's statement, since the $\mathbb{F}_7$ operating
point's $4.299 \times 10^7$ is 0.1228 of it.

**F.5 — The record carries two distinct device-fault observations, described
differently, and the retraction attaches to one of them.** The archived
calibration's own hang is "*a single 2048-matrix F_5 n=20 launch (≈200 s+)*"
(`r4_gpu_uniformity_resample.md`:231-232), which the $\mathbb{F}_5$ campaign
repeats ([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12). The fault
with a committed operational record is a different configuration: $q = 3$,
$n = 24$, `gpu_hip` at $M = 4096$, 2026-08-07
(`dev/studies/b488f02c/gpu-hang-2026-08-07.log`). The explicit retraction of the
watchdog attribution — "*nothing here attributes the hang to a watchdog
timeout*" — is in the second one's log, and the supersession note applies it to
the archived calibration's account
([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.1). The archived $q = 5$
hang has no separate committed artifact beyond the archived prose. Recorded
because §9's bound statement rests on which observation the retraction covers.

**F.6 — Prior published rates outside their own stopping rule.** Two cells of
the prior feasibility grid are published `measured` below the protocol's
five-repetition minimum — $q = 5$, $n = 28$ batch rayon at one repetition, and
$q = 3$, $n = 28$ `gpu_hip` $M = 1024$ at three — and both now carry a dagger
and footnote under bug `4fdd781a`
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §11.1). Both are cells
this study's runs censor. The first is why the $\mathbb{F}_5$ $n = 28$
denominator is `cpu_scalar` here (§4.1).

**F.7 — The lane-owns-interval mapping's stated resource budgets are not what
limits occupancy anywhere.** The mapping's central occupancy hypothesis is
falsified at every level the evidence reaches: applying the compiler's own model
to each committed per-lane register budget returns the architectural ceiling for
all seven kernels that have one; per-block shared memory leaves the LDS with
headroom above the per-CU wave-slot cap; where per-SIMD occupancy does fall
below the ceiling — four of eleven retained-path kernels — the limiter is the
compiler's realised per-lane vector-register allocation, which exceeds every
stated budget by 2.44× to 9.62×; and where *achieved* occupancy falls below the
per-SIMD ceiling, which is everywhere in the study, the limiting quantity is the
launch width the batch calibration supplies
([`../a9284086/receipt.md`](../a9284086/receipt.md) §6).

**F.8 — The runtime record shares the compiler's dynamic-LDS blind spot.**
`rocprofv3` reports `LDS_Block_Size` 0 for every dispatch, including launches
that request 144 to 576 bytes of dynamic shared memory. The one runtime reading
of a dynamic table anywhere in the study is a `rocprofv2` capability probe that
read `LDS_Per_Workgroup` 512 for `wave_gf7_three_plane_kernel` at $n = 20$ — the
design's 480-byte prediction at a 128-byte granularity — in four dispatches
before aborting. Tracked as JIT issue `023233c5`
([`../a9284086/receipt.md`](../a9284086/receipt.md) §4, §10.5).

**F.9 — Nine of twelve round-1 occupancy readings fail the counters' own
definitions**, by factors of 3.182 to $1.28 \times 10^4$, and are tabulated as
failures rather than repaired; the $\mathbb{F}_7$ three-plane pair has no
admissible reading at $n = 12$ in either round
([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §13, §17.2).

**F.10 — Two branch-frequency observations miss their exact marginal at the
stated coverage.** The $\mathbb{F}_5$ timed-operation observation at $n = 28$
sits at $z = +2.17$ and the $\mathbb{F}_7$ one at $n = 24$ at $z = +2.85$, both
outside their Wilson 95 % intervals, on all backends that resample the same
addresses. Two orders out of ten under a nominal 95 % procedure with no
multiplicity adjustment; the other eight sit at $|z| \le 1.24$
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §9,
[`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §9).

**F.11 — The packed CPU baseline is uniformly slower than in the prior run**, on
all nine comparable $\mathbb{F}_5$ pairs, mean $-6.34\%$, $p = 0.004$ under a
two-sided sign test, while the generic Ryser path on the same host moves by
under 0.8 %. Neither artifact isolates the cause. Its consequence for this
document is stated where it bites, in §4.1
([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §11).

**F.12 — Rejected candidates are retained with their measurements.**
`f5-byte-control` and `f7-lookup-table-control` are not selected (§4.2, §5.2)
and both remain in the record with their throughputs, their resource profiles,
their censored cells, and the one order where the lookup control wins.

**F.13 — The $\mathbb{F}_3$ packed encoding's own doc comment claims its
alternative zero is never produced, and the implemented circuits produce it.**
`bipedal3.rs` states that the codeword $(0,1)$ is "*never produced by
`add/sub/mul/neg` from canonical inputs*" (`:19-21`, `:62-65`). Evaluating the
committed formulas on canonical inputs contradicts that for three of the four
operations: `add` returns $(0,1)$ for $2 + 1$ (`:581-593`), `sub` for $1 - 1$
(`:620-632`), and `mul` for $0 \times 2$ and $2 \times 0$ (`:680-685`); only
`neg` never produces it (`:652-657`). Every one of those results decodes to the
correct field value, because the encoding's three classes are what carry the
field element and all four operations respect them, so nothing computed anywhere
in this record is wrong: the affected claim is the doc comment's, not a
measurement's. §3.3's validity argument is written not to depend on it. Recorded
rather than repaired here, because this document changes no code (§12.11); the
doc-comment defect is tracked as JIT issue `63d931a9`.

## 11. Criterion-by-criterion conformance

| REQ | Where addressed | Status |
| --- | --- | --- |
| REQ-01 | §3, §4, §5 | **Satisfied.** A verdict per field, each with a receipt citation: $\mathbb{F}_3$ no-go, on [`../047b62ed/receipts.md`](../047b62ed/receipts.md) §4.2 — no device path reaches the CPU at the declared operating point $n = 28$; $\mathbb{F}_5$ go on `f5-three-plane`, on [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §12 and §3; $\mathbb{F}_7$ go on `f7-three-plane-permanent`, on [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §15, §3, and §14. Each verdict is checked against the study's own four-part decision rule, quoted in §1, including the safe-launch-duration condition: each field's section checks its declared operating point's per-launch span and work against that field's documented safe launch-duration bound, the observed clean-completion envelope of [`../a9284086/receipt.md`](../a9284086/receipt.md) §9.2 and §9.4, and no field's declared point falls outside it. |
| REQ-02 | §3.1, §4.1, §5.1 | **Satisfied, with every crossing located only as a bracketing between two adjacent measured orders.** $\mathbb{F}_3$: the prototype's ratio brackets a crossing upward between $n = 12$ and $n = 16$ (0.7568 to 2.2149) and downward between $n = 24$ and $n = 28$ (1.5561 to 0.2036), and the shipped path's brackets the same two on the same two pairs (0.7118 to 1.5547, 1.0527 to 0.4390); the better shipped configuration's downward crossing is unlocated because the $n = 28$, $M = 1024$ cell is censored, and its projection is labelled an extrapolation. $\mathbb{F}_5$: the prototype's ratio is above 1 at all five measured orders and the shipped path's below 1 at the three it is measured, so neither column brackets a crossing and none is placed below $n = 12$. $\mathbb{F}_7$: neither prototype column brackets a crossing for the same reason; the shipped path is the one $\mathbb{F}_7$ column that does, upward between $n = 16$ and $n = 20$, which [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2 reports as beating the CPU at exactly one order, stated here with its mechanism — the CPU denominator changing at the packed kernel's sixteen-lane bound. Each statement names the best applicable in-tree CPU path per order, identified from the run's own data. |
| REQ-03 | §2 | **Satisfied.** The per-lane Gray interval (`wave_ryser_mapping.h:33-40`), the per-lane accumulator initialization by prefix reconstruction at $g(\text{interval.start})$ (`wave_ryser_mapping.h:42-44`, with the per-field loops cited), the lane-local row-product reduction (five circuits cited by field and representation), and the cross-lane partial-sum reduction in fixed increasing lane order (`wave_ryser_mapping.h:67-78`) are each specified with the source that implements them, and the executable prototype is linked. |
| REQ-04 | §3.3 | **Satisfied.** The two-bit `(mag, sgn)` encoding carries three values under three equivalence classes, a clear `mag` bit being field zero whatever `sgn` holds, and validity rests on those classes rather than on a canonical form: every decode in the crate and both device fold circuits read the class, and `add`, `sub`, `mul`, and `neg` map class to class on all sixteen codeword pairs, the alternative zero included as an input. The doc comment's narrower claim that the alternative zero is never produced is contradicted by three of those four operations and is recorded as such (§10, F.13) rather than relied on; `add` and `sub` are distinct operations, which excludes $\mathbb{F}_2$; the two algebraic facts that make the encoding Boolean are $q = 3$-specific — $3 = 2^2 - 1$ and $\mathbb{F}_3^* \cong \mathbb{Z}/2$, the latter used directly by the sign-popcount fold; the Ryser sign is nontrivial in $\mathbb{F}_3$ and is applied twice over, where over $\mathbb{F}_2$ it would vanish and the permanent would collapse onto the determinant; and two committed measurements are functions of $q = 3$ specifically and hold — the branch marginal $1 - (2/3)^n$ at all five orders and the permanent-zero fraction near $1/3$ — with the $q = 2$ alternative excluded by the committed intervals at three of five orders on the 4 096-sample observation and at all five on the timed-operation observation, coverage stated per cell rather than claimed uniformly. The result is checked per matrix against an `Fp<3>` oracle that runs the shared field-law suite. |
| REQ-05 | §4.2, §5.2 | **Satisfied.** $\mathbb{F}_5$: `f5-byte-control` and `f5-three-plane`, each with lane state, row-product circuit, shared-memory formula, measured `VGPRs`/`TotalSGPRs`/scratch, occupancy, and composite rate at five orders, cited to [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §4.2, §6.2, §7, §10, before the selection. $\mathbb{F}_7$: `f7-lookup-table-control` and `f7-three-plane-permanent`, the same fields, cited to [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §4.2, §7, §10, §12, before the selection; the third planned candidate is recorded with its structural falsification (§10, F.1). |
| REQ-06 | §6 | **Satisfied.** For both go verdicts: input layout (§6.1, canonical row-major bytes, device-side packing, shared-table sizes); persistent device buffers and streams named as the existing `DeviceBuffer`, `HipStreamPool`, and `LaunchDims` of `crates/gf2-kernels-hip/src/host/`, each cited to its defining line, with `PinnedHostBuffer`, `HipStream`, `HipEvent`, `HipEventSpan`, and `GfxTarget` beside them (§6.2); bounded-duration launches sized against a per-launch span target rather than a work budget, with the 978.5× measurement that rules out the work-budget form (§6.3); per-device initialization through `GfxTarget::detect_device` and blob loading, once per device (§6.4); error propagation selecting a tested CPU fallback for unsupported capabilities and recoverable resource failures through `DeviceBuffer::new_with_fallback` and the `UnsupportedArch` and `OutOfMemory` variants, while fatal device failures propagate as typed errors instead of the current asserts (§6.5); and behavioral-equivalence coverage on the shared field-law suite, the per-matrix oracle gate, the representation-boundary orders, determinism across the fallback boundary, and tests that force each fallback branch (§6.6). The gap each element closes is cited to the dispatcher line that currently bypasses it. |
| REQ-07 | §7, §7.1 | **Satisfied.** The public packed-field representation `Packed7` (`packed7.rs:211`, `:216`) and the permanent-specialized internal state `ThreePlane` (`wave_gf7_equivalence.hip:75-79`) are named as distinct surfaces with their scopes. The second compliant resolution is chosen — public representation unchanged, three-plane state internal — grounded in the committed evidence's single architecture, device-only scope, unresolved batch confound, and the campaign's own refusal to order the two circuits, and in `@/inv/convention-convergence` together with the proof-obligation asymmetry. §7.1 records the named exception `permanent-f7-internal-three-plane` with its scope, its shared behavioral tests, and a tracked convergence condition with two discharge routes. $\mathbb{F}_5$ needs no exception because `Packed5` is already three-plane (`packed5.rs:208-212`). |
| REQ-08 | §8 | **Satisfied.** The receipts support the three-plane $\mathbb{F}_7$ candidate under the permanent workload, on three strands of which the strongest is the falsified weighting: the archived model charges the complete row-product reduction on every Gray step, and the measured exact marginal $(6/7)^n$ puts it at 4.58 % of steps at $n = 20$. The amendment, drafted at [`req08-amendment-draft.md`](req08-amendment-draft.md), is **applied to the archived file with owner approval (2026-08-17)**: a dated additive note above its §1 and two pointer stubs, no existing sentence edited. `Packed7` remains Candidate A and §7's choice is consistent with that. |
| REQ-09 | §10 | **Satisfied.** Thirteen entries, each recorded with the evidence contradicting the statement it bears on rather than replacing it: the non-executing candidate, the $\mathbb{F}_3$ crossover closing before the operating point, the censoring record and the non-conservative projection chains, the archived $q = 7$ budget premise the $\mathbb{F}_7$ receipt preserves beside its correction (filed by this study as `e1c96c9c`), the two distinct fault observations, the prior sub-minimum published cells, the falsified occupancy hypothesis, the dynamic-LDS blind spot, the failed occupancy readings, the two branch-frequency interval misses, the packed-CPU run-to-run shift, the retained rejected candidates, and the $\mathbb{F}_3$ alternative-zero doc comment the implemented circuits contradict. |
| REQ-10 | throughout | **Satisfied.** Every quantitative claim carries the receipt file and section, or the in-tree source path and line, it resolves to. Where this document computes a figure or a fact the sources do not print — §9's envelope share columns, §3.3's $q = 2$ marginal against the committed Wilson intervals, and §3.3's enumeration of the four bipedal operations over all sixteen codeword pairs — it names its inputs with their sources and states that the result is computed here. |
| REQ-11 | §9 | **Satisfied.** Both factors are recorded with their receipts — 19.7082× at $\mathbb{F}_5$, $n = 24$ and 545.6693× at $\mathbb{F}_7$, $n = 20$ — and both exceed 1.5×, so there is no throughput shortfall to record. Both points are inside the documented safe launch-duration bound, which is the runtime qualification's per-field observed clean-completion envelope ([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.2, §9.4): 0.00069 and 0.000215 of their fields' span figures and 0.266 and 0.040 of their work figures. Both are also inside the archived prior's span boundary and work budget, the other documented limit, cited as a prior. The bound's scope is stated with it: it is observed rather than derived and is a conservative lower bound on the safe region, since no launch in the evidence was run to failure, so no upper boundary on safe operation is located and none is claimed; locating one is an open deliverable (§12.1). |

## 12. What this synthesis does not establish

Collected so a reader does not have to reassemble it.

1. **No upper boundary on safe launch duration is located, so the bound the
   verdicts are checked against is a conservative lower one.** §9's
   inside-the-bound statements are against the observed clean-completion
   envelope — a lower bound on the safe region — and against a prior whose
   mechanism is retracted. The longest span this study observes anywhere is
   115.452922 s, 0.61 of the archived ≈190–200 s figure, and nothing in the
   evidence probes above it
   ([`../a9284086/receipt.md`](../a9284086/receipt.md) §9.4, §12.1). Locating a
   boundary needs a launch run to failure, which no committed run performs; it
   is an open deliverable, and until it lands the safe region is known to
   include the envelope and its upper extent is unmeasured.
2. **The go verdicts are validated only at probe-calibrated batches, and the
   production design of §6.3 runs at wide grids the evidence does not cover.**
   Every prototype cell sizes itself from a one-matrix probe, giving launch
   widths of 1 to 1 614 waves against the device's 2 560 slots; every measured
   kernel holds most or all of the waves it launches resident, so nothing here
   approaches the per-SIMD residency ceiling, and no cell in the study tests the
   register-pressure prediction
   ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.3,
   [`../a9284086/receipt.md`](../a9284086/receipt.md) §12.4). A production
   dispatcher calibrating $M$ against a span target will run wider than anything
   measured, and its behaviour there is untested.
3. **No measurement separates the mapping from the batch.** Almost no prototype
   cell runs at the control's batch sizes; the one near-match is the
   $\mathbb{F}_7$ three-plane cell at $M = 1109$ against 1 024. Every
   prototype-over-control ratio in §3 through §5 mixes the two effects, over a
   factor of 224 at $\mathbb{F}_3$, 102 at $\mathbb{F}_5$, and 5 833 at
   $\mathbb{F}_7$ ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §13.1,
   [`../91605d4d/receipts.md`](../91605d4d/receipts.md) §14.2,
   [`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.5).
4. **The two $\mathbb{F}_7$ arithmetic circuits are not ordered**, which is why
   §7 chooses the internal-state resolution and why §8's strand 2 is carried as
   two figures rather than a finding
   ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.8).
5. **Nothing here re-measures any CPU packed encoding.** The $\mathbb{F}_7$
   amendment of §8 is scoped to the permanent workload on the device for exactly
   this reason, and the "broader `PackedField` evidence" that option 1 of §7
   requires does not exist in this study.
6. **The $\mathbb{F}_3$ $n = 28$, $M = 1024$ ordering is open.** That cell is one
   of the protocol's four premeasurement configurations and is censored here;
   the prior study's 0.984× figure for it is neither confirmed nor overturned,
   and this document asserts no ordering at that cell
   ([`../047b62ed/receipts.md`](../047b62ed/receipts.md) §11, §13.5).
7. **The $\mathbb{F}_5$ $n = 28$ ordering against batch rayon is open**, because
   that cell is censored here and published in the prior run from a single
   repetition its own protocol would censor
   ([`../91605d4d/receipts.md`](../91605d4d/receipts.md) §14.8).
8. **The largest orders rest on weak correctness gates.** The equivalence cells
   compare 4 matrices at $\mathbb{F}_3$ $n = 28$, 32 and 2 at $\mathbb{F}_5$
   $n = 24$ and $n = 28$, and 32 and 4 at $\mathbb{F}_7$ $n = 24$ and $n = 28$,
   against 512 at the smaller orders — including the $n = 24$ gate the
   $\mathbb{F}_7$ above-sixteen-lane demonstration depends on
   ([`../6c7fcb38/receipts.md`](../6c7fcb38/receipts.md) §17.12).
9. **The dynamic shared column tables are unmeasured** at every kernel but one,
   so the design's per-block shared-memory figures in §6.1 are launch formulas
   rather than observations
   ([`../a9284086/receipt.md`](../a9284086/receipt.md) §12.5).
10. **Architecture scope is one device.** Every figure is `gfx1030`, and the
    production design's per-device initialization in §6.4 has one target to
    resolve to ([`../a9284086/receipt.md`](../a9284086/receipt.md) §12.8).
11. **The production design of §6 is a design, not an implementation.** No code
    in `crates/` is changed by this document, nothing in it is compiled, and the
    behavioral-equivalence coverage of §6.6 is an obligation rather than a
    passing suite.

[Scheinerman2024]: https://arxiv.org/abs/2407.20205
