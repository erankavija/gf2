# Where the matvec SIMD lane spends its instructions at ragged strides

> **Diátaxis Type:** Explanation

Issue `@/issue/7dbe1393` asks where the SIMD lane of the allocated
`BitMatrix::matvec` spends its time, relative to the scalar lane, at strides
of 9, 10, 62, 63 and 66 words. This page reads the untimed evidence, the
emitted assembly of the measured executable and the exact execution count of
every instruction of the three lanes, and then the profile session's
counters. It carries no figure. Every count is in
[the generated tables](asm-attribution-tables.md) or the profile summary, and
a statement below names the section and column it reads. Code claims are claims of the
[source ledger](survey/source-evidence.json), cited by identifier in
backticks.

## Records

- [`survey/asm/simd/`](survey/asm/simd/index.txt) is the release assembly of
  the harness arm at Rust 1.95, written by the dense story's
  `disassemble-dense.sh --out`; its index names the executable by SHA-256.
- [`survey/lane-instruction-counts.json`](survey/lane-instruction-counts.json),
  written by
  [`survey/record-lane-instruction-counts.py`](survey/record-lane-instruction-counts.py),
  holds the execution count of each instruction of the lane symbols for one
  call of the product per lane and stride. The arm answers the
  validation-position request of a frozen cell with one untimed call, and
  Valgrind's Callgrind counts it. No timing window opens.
- [`survey/make-asm-attribution.py`](survey/make-asm-attribution.py) joins the
  two and refuses records of different executables.
- [`avx2.asm.txt`](../../../../crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt)
  is the kernel crate's own artefact. It lists each kernel as the crate
  compiles it alone; the arm links the kernels under thin LTO, and the
  assembly record above is the code the campaign times.

An instruction count is no latency: instructions differ in cost, and the
counts say nothing of cache traffic or branch prediction. The profile session
measures those.

## What each lane executes per row

**The row body is the same on both kernel lanes.** The SIMD lane and the
parity-kernel lane run one row loop: an indirect call through the bundle, a
test of the result and the output append (`matvec-simd-kernel-call`,
`matvec-parity-kernel-call`). The *Row body* column of *Instructions per row*
is equal on the two lanes at each stride, and the *Symbols* section shows the
two lane bodies at the same instruction count within one. The scalar lane has
no call: its row body holds the tail words, the reduction and the word count
inline, which is why its *Row body* column is larger and its *Kernel
straight-line* column is empty.

**Full vectors.** The scalar lane's word loop is compiled to 128-bit SSE2: the
*Loops* section names one loop per lane row with `movdqu`, `pand` and `pxor`,
taking four words a trip (`scalar-accumulators`). The SIMD lane's kernel
spends its vector loop on a population count: each vector passes two
`vpshufb` nibble lookups and one `vpsadbw` byte sum, as the marker columns
count per row. The product needs one parity per row, so that count is work
the scalar lane does not do. Per four words the SIMD lane's loop executes
about as many instructions as the scalar lane's, which the *Instructions in
the loop* column shows against each loop's words per trip.

**The partial-vector tail.** The SIMD lane's kernel counts each word past its
last whole vector on its own (`popcnt-tail-word-count`). The kernel enables
`avx2` and not `popcnt` (`popcnt-kernel-signature`), so each such count is the
portable shift-and-multiply sequence: the loop the *Loops* section lists with
`imul` runs once per tail word, and the `imul` column of the SIMD lane equals
the stride's words past a whole vector. The linked kernel also carries two
vectorised forms of that tail loop that no stride below four tail words
enters (the two loops with zero trips at every stride), and reaching the
scalar form costs straight-line instructions: the *Kernel straight-line*
column of the SIMD lane is larger at every ragged stride than at the aligned
control. The scalar lane handles its tail in three unrolled word steps inside
its row body.

**Reduction to parity.** The scalar lane folds its accumulators to one word
and counts that word once per row (`scalar-word-count`); its `imul` column is
constant across strides and holds that count's multiply and the row offset's.
The SIMD lane sums its per-vector counts, adds the tail counts and keeps the
low bit.

**Call and setup.** Both kernel lanes pay one indirect call, one bundle-entry
jump and the kernel's setup and `vzeroupper` per row; the `call` column counts
the call. The scalar lane pays none of them.

**Per stride.** At each of the strides 9, 10, 62, 63 and 66 the *Total* column
puts the SIMD lane above the scalar lane, and at the 32-word control below
it. The lane-threshold rows of `4337c02e` record the same order in time
([tables](../4337c02e/tables.md)): the scalar lane ahead at the five ragged
strides and behind at 32. Between 9 and 10 words, and between 62 and 63, the
SIMD lane's total rises by one tail-word loop trip while the scalar lane's
rises by a few instructions.

## What the profile session records

The [profile summary](../../../bench_results/7dbe1393/ragged-tail-profile/profile-summary.md)
holds hardware counters and sampled symbols for the three lanes at the five
ragged strides and the control, each as a median of full process passes with
its order-statistic interval; its opening paragraph states the pass count and
the interval's coverage. The session's
[execution log](../../../bench_results/7dbe1393/ragged-tail-profile/execution.log)
holds a `start` and a `done` line for every case of
[`survey/profile-cases.txt`](survey/profile-cases.txt) and a terminal
`complete` line, and the summary's generator, the dense story's
`summarize-dense-profile.py`, reproduces the committed summary byte for byte
from the retained passes. The rows are descriptive: the session carries no
paired design and no decision.

**Counters.** In the *cycles/call* column the SIMD lane's interval lies above
the scalar lane's at each of the strides 9, 10, 62, 63 and 66, and the two
intervals overlap at the 32-word control. The *instructions/call* column has
the order of the untimed counts: the SIMD lane above the scalar lane at the
five ragged strides and below it at the control. The *L1 misses/call*
column differs little between the lanes of one stride, so the gap at the
ragged strides is executed work and not first-level cache traffic.

**Where the SIMD lane's cycles fall.** In *Sampled cycle attribution* the
kernel `avx2_and_popcnt` holds the largest share of the SIMD lane at every
stride; the lane body `matvec_simd` and the bundle entry `and_popcnt_fn` hold
the rest of the retained rows. The scalar lane's samples fall in
`matvec_scalar` alone. The time the SIMD lane spends beyond the scalar lane
is therefore in the kernel the instruction attribution examines, and the
per-row call and entry are a smaller, stride-independent part.

**The candidate.** The parity-kernel lane's *instructions/call* is below both
other lanes at every stride, and its *cycles/call* median is below the SIMD
lane's at every stride and at or below the scalar lane's; the interval
columns state where those orders are resolved.

**Host condition.** The window runner read the host's load averages at the
session's start; its start line is in
[`survey/bench-window-jobs.txt`](survey/bench-window-jobs.txt), the first job
of the record. The load it states is that of a host running other work, so
the counter intervals include that interference.

## The candidate the evidence supports

The gap has two parts the kernel owns: a population count where a parity
suffices, on every vector, and a per-word portable count on the tail. A kernel
change addresses both; the per-row call is outside the kernel and stays equal
in both arms of the family.

`avx2_and_parity` XOR-folds the row's intersection in 256-bit accumulators,
folds the one to three tail words through one masked load of each operand
(`parity-masked-tail`), and takes one count of the folded word
(`parity-fold`), which the compiler lowers to the parity flag. In the tables
the parity-kernel lane executes no `vpshufb`, no `vpsadbw` and no `imul`, two
`vpmaskmovq` at a ragged stride and none at the control, and one `setnp` per
row; its *Total* column is below both other lanes at every stride of the
family, the control included.

Whether that order holds in time is the question of the
[frozen addendum](matvec-ragged-tail-addendum.md); the
[measurement plan](measurement-plan.md) states how it is measured and what
the family records.
