# Host-calibration receipt for the tuning-profile selector families

This is the calibration run of issue `5ecc9bf8`: one execution of the explicit
host-calibration action `crates/gf2-core/benches/tuning_calibration.rs` on the
project benchmark host, under the repository lock wrapper, emitting a
`TuningProfile` document that the loader of issue `f35daec0` accepts.

The action measures both arms of each selector crossover across a size grid
straddling the conservative default, selects the smallest grid point at which
the asymptotic arm beats the other by more than the measured noise band, and
keeps the conservative default on a tie, on a non-monotone crossover, or where
no grid point offers both arms. The rule and the provenance field set are those
of `dev/active/220cab0b/design.md` §2.7.

The run changes no selector and installs no profile. No calibrated profile is
committed under `crates/gf2-core/data/tuning-profiles/`, by owner decision, so
the crate's selection behaviour is identical before and after this receipt: the
emitted document records what the host measures, and nothing reads it.

## Result

The sweep measures 45 grid points over five fields, 25 timed windows per arm per
grid point. Three fields cross over monotonically and take a measured value, one
keeps its default as a calibration outcome, and one is not calibrated at all and
is therefore absent from the emitted document. Every grid point where both arms
exist passes an equivalence probe before either arm is timed; a failed probe
aborts the run, and this run exits 0 with all five sweeps reported.

| Field | Default | Emitted | Selecting grid point | Margin | Noise band |
|---|---:|---:|---|---:|---:|
| `bit_backend.simd_min_words` | 8 | **4** | 4 buffer words | 0.2306 | 0.0033 |
| `polynomial.karatsuba_min_degree` | 32 | *absent* | — | — | — |
| `polynomial.karatsuba_max_out_len` | 128 | 128 (default kept) | — | — | — |
| `polynomial.div_rem_fast_min_len` | 2048 | **1024** | 1024 divisor coefficients | 0.2003 | 0.0011 |
| `polynomial.subproduct_min_len` | 4096 | **512** | 512 coefficients = points | 0.1846 | 0.0011 |

### Kept default against absent field

The two fields that do not take a measured value make different claims, and the
emitted document distinguishes them.

`karatsuba_max_out_len` **keeps its default as a calibration outcome**. Both arms
were measured at all nine grid points and the crossover is non-monotone, so
§2.7 rule 3 applies. The document states `128`, because the sweep covers the
field and concluded that the default stands (F1).

`karatsuba_min_degree` is **not calibrated at all**: no grid point offers both
arms, so no comparison exists to conclude anything from. Design §5 condition 5
governs that case — "Until the sweep covers it, a committed profile omits the
field and inherits the default; a profile that carries an uncalibrated value is
a `@/inv/benchmark-backed-performance` defect" — so the emitted document has no
`karatsuba_min_degree` key. An absent field is a supported state of the schema:
`from_json` resolves it to the conservative default, so the loaded profile still
selects 32 and behaves identically, while the document stops asserting a value
nothing measured. Issue `389aa4de` owns closing the gap (F2).

### Contradictions

Of the three fields whose value moves, `div_rem_fast_min_len` and
`subproduct_min_len` each also contradict their constant's own tuning note, and
records F4 and F5 carry those with their numbers. `simd_min_words` moves without
a falsification attached: the heuristic at
`crates/gf2-core/src/kernels/backend.rs` states the rule the constant
implements, not a measured claim, so a lower measured crossover is a calibration
result rather than a contradiction.

Four of the five in-source defaults are therefore contradicted or moved by this
host's measurements. Only `karatsuba_min_degree` is neither confirmed nor
contradicted, because its two arms cannot be compared at all. This receipt
changes none of the constants: calibration emits a profile, it never edits a
default.

## Reproducible protocol and provenance

Every value below is observed by the harness during this run, and every one of
them appears in the emitted profile's `provenance` object.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/tuning_calibration.rs`; schema `tuning-calibration-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | `dev/active/220cab0b/design.md` §2.7 sweep and selection rule, executed by the harness |
| Source revision | `e202c08089210f98842558bf1336143e6b6962d1`; `source_dirty=true` — see Falsification record F6 |
| Bench binary SHA-256 | `ac4c7731ca499a668459e701007e57bfcfafbadaa4d7dbc2f2344dba31224350` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0 bench` |
| Features | `tuning-profile`, `simd` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| Runtime CPU features | `avx2`, `fma`, `pclmulqdq`, `sse4.1`, `vpclmulqdq`; the detected logical SIMD backend reports `avx2` |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`, observed by the harness on one of its own inherited descriptors; CPUs 6--11, observed as `Cpus_allowed_list: 6-11` |
| Niceness | The wrapper's best-effort `nice -n -5` is denied to this non-root user; `nice` reports `cannot set niceness: Permission denied` and the child runs at niceness 0. The lock and the affinity remain in force |
| Timed work | Five executions of five repetitions per arm per grid point, `--target-ms 250`; 523.9 s of timed windows |
| Measured duration | 2026-08-19 22:17:10--22:25:54 UTC (8 min 44 s), the wrapper's whole child lifetime |
| Emitted profile | `/tmp/gf2-5ecc9bf8-calibration-e202c080.json`, SHA-256 `f0c3100ddd23fe3bbaadb5bf3b7fde51af78fc6e9929bb76886b1b032a165635`; 1086 bytes |

The lock is observed rather than asserted. `dev/scripts/ccx1-bench-flock.sh`
takes `flock -x` without `-o`, so the locked descriptor survives the exec chain
and is inherited by every descendant including the bench binary; the harness
matches its own open descriptors against the kernel's lock table by device and
inode and refuses to collect any host fact when no match exists. The wrapper
acquired the mutex without blocking: the shell recorded the invocation at
22:17:10 UTC and the harness stamped `measured_at` at the same second.

The bench binary is built before the timed run, so the run reports
`Finished bench profile in 0.04s` and no compilation happens under the lock.

## Commands

```sh
OUT=/tmp/gf2-5ecc9bf8-calibration-e202c080.json
test ! -e "$OUT"
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh \
  cargo +1.95.0 bench -p gf2-core --features tuning-profile,simd \
  --bench tuning_calibration -- \
  --executions 5 --repetitions 5 --target-ms 250 \
  --out "$OUT" \
  --lock-wrapper dev/scripts/ccx1-bench-flock.sh \
  --receipt dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md
```

The design's §2.7 invocation names `--features tuning-profile` alone. This run
adds `simd`, because `SelectedBackend::Simd` exists only under that feature:
without it the bit-backend family has one reachable arm, `simd_min_words` is
inert on the resulting build, and the sweep would report the field as having no
comparable grid point. The feature list is recorded above and the emitted
`cpu_features` records what the host then detected at run time.

## Statistics

An arm's rate at a grid point is the median of its 25 timed windows in
nanoseconds per call. Its **spread** is the interquartile range of those windows
divided by that median. The **noise band** at a grid point is the larger of the
two arms' spreads, and the **margin** is the asymptotic arm's relative
improvement,

$$
\text{margin} = \frac{t_{\text{conservative}} - t_{\text{asymptotic}}}{t_{\text{conservative}}},
$$

so the asymptotic arm wins at a grid point exactly when
$\text{margin} > \text{band}$. Each execution recalibrates its own call count
against the 250 ms target and then records five windows; the two arms alternate
at execution granularity so a drift across a grid point is spread over both.
Each timed window is a monomorphic loop over one entry point, with the arm
selection outside it.

## `bit_backend.simd_min_words` — selected 4

Arms: `gf2_core::kernels::ScalarBackend` against the detected
`gf2_core::kernels::simd::SimdBackend`, both called through `Backend::xor` on
eight cache-line-aligned fixture banks.

| Buffer words | scalar ns | spread | simd ns | spread | margin | band | Wins |
|---:|---:|---:|---:|---:|---:|---:|---|
| 1 | 2.631 | 0.0023 | 2.886 | 0.0039 | −0.0968 | 0.0039 | scalar |
| 2 | 2.855 | 0.0018 | 3.124 | 0.0035 | −0.0942 | 0.0035 | scalar |
| **4** | 3.510 | 0.0020 | 2.701 | 0.0033 | **0.2306** | 0.0033 | **simd** |
| 7 | 4.179 | 0.0022 | 3.570 | 0.0036 | 0.1458 | 0.0036 | simd |
| 8 | 4.744 | 0.0040 | 3.377 | 0.0032 | 0.2883 | 0.0040 | simd |
| 9 | 4.930 | 0.0041 | 4.021 | 0.0026 | 0.1845 | 0.0041 | simd |
| 16 | 7.735 | 0.0193 | 4.203 | 0.0026 | 0.4566 | 0.0193 | simd |
| 32 | 11.170 | 0.0023 | 5.229 | 0.0055 | 0.5319 | 0.0055 | simd |
| 64 | 19.916 | 0.0022 | 7.099 | 0.0031 | 0.6435 | 0.0031 | simd |

The crossover is monotone and interior: the SIMD arm loses at 1 and 2 words and
wins at every grid point from 4 upward. The grid carries no point at 3 words, so
the measurement bounds the true crossover to $(2, 4]$ and the selected value is
the smallest measured winning point. The 16-word cell carries the widest spread
in this family at 1.93 %, well inside its 45.7 % margin.

## `polynomial.karatsuba_min_degree` — absent from the emitted document

| Operand degree | schoolbook ns | spread | karatsuba ns | spread | Comparison |
|---:|---:|---:|---:|---:|---|
| 4 | 37.191 | 0.0028 | — | — | none |
| 8 | 112.703 | 0.0053 | — | — | none |
| 16 | 362.345 | 0.0018 | — | — | none |
| 31 | 1248.166 | 0.0022 | — | — | none |
| 32 | — | — | 1135.357 | 0.0027 | none |
| 33 | — | — | 1176.103 | 0.0019 | none |
| 64 | — | — | 3702.905 | 0.0009 | none |
| 128 | — | — | 11780.431 | 0.0036 | none |
| 256 | — | — | 36340.125 | 0.0017 | none |

Every grid point reaches exactly one arm, so no grid point yields a comparison
and the field is uncalibrated. The emitted document omits it; F2 states why and
names the issue that owns closing the gap.

The dispatcher curve is still informative and is recorded: `FieldPoly::mul`
costs 1248.166 ns at degree 31 on the schoolbook arm and 1135.357 ns at degree
32 on the Karatsuba arm — 9.0 % **less** for one more coefficient in each
operand. A monotone cost curve would rise across that step. The step down is
consistent with the crossover lying below 32, but it is not a two-arm
measurement and this receipt claims no crossover from it.

## `polynomial.karatsuba_max_out_len` — default 128 kept, non-monotone

Arms: `FieldPoly::mul` (the schoolbook/Karatsuba dispatch, which `mul_fast`
selects at or below the threshold) against `FieldPoly::mul_ntt`. Product length
$\text{out\_len} = 2n - 1$ for equal-length operands, so every grid point is odd.

| Product length | NTT length $N$ | karatsuba ns | spread | mul_ntt ns | spread | margin | band | Wins |
|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 15 | 16 | 85.926 | 0.0013 | 1150.360 | 0.0188 | −12.3878 | 0.0188 | karatsuba |
| 31 | 32 | 323.803 | 0.0011 | 1790.653 | 0.0043 | −4.5301 | 0.0043 | karatsuba |
| 63 | 64 | 1258.619 | 0.0018 | 3078.752 | 0.0013 | −1.4461 | 0.0018 | karatsuba |
| 127 | 128 | 3804.011 | 0.0017 | 5845.125 | 0.0006 | −0.5366 | 0.0017 | karatsuba |
| 129 | 256 | 3788.505 | 0.0027 | 11829.852 | 0.0008 | −2.1226 | 0.0027 | karatsuba |
| 191 | 256 | 7037.831 | 0.0054 | 11826.315 | 0.0011 | −0.6804 | 0.0054 | karatsuba |
| 255 | 256 | 11888.795 | 0.0012 | 11823.014 | 0.0012 | 0.0055 | 0.0012 | **mul_ntt** |
| 383 | 512 | 21701.632 | 0.0599 | 24863.835 | 0.0006 | −0.1457 | 0.0599 | karatsuba |
| 511 | 512 | 36558.665 | 0.0007 | 24861.794 | 0.0006 | 0.3199 | 0.0007 | **mul_ntt** |

The asymptotic arm wins at 255, loses at 383, and wins again at 511, so §2.7
rule 3 applies and the conservative default stands. Records F1 and F3 carry the
structure behind it.

The 383 cell's Karatsuba arm is the one elevated dispersion in the whole sweep
at 5.99 %, an order of magnitude above every other cell. The verdict there does
not turn on it: the margin is −0.1457, so the asymptotic arm loses at that point
whatever the band, and the non-monotonicity is established by the 255 and 511
cells whose spreads are 0.12 % and 0.07 %.

## `polynomial.div_rem_fast_min_len` — selected 1024

Arms: `FieldPoly::div_rem` against `FieldPoly::div_rem_fast`, on a dividend of
twice the divisor's coefficient count. The grid point is the divisor length, the
smaller of the two lengths `div_rem_auto` gates on.

| Divisor coefficients | div_rem ns | spread | div_rem_fast ns | spread | margin | band | Wins |
|---:|---:|---:|---:|---:|---:|---:|---|
| 64 | 9 000 | 0.0014 | 27 006 | 0.0015 | −2.0008 | 0.0015 | div_rem |
| 128 | 26 400 | 0.0031 | 90 620 | 0.0009 | −2.4326 | 0.0031 | div_rem |
| 256 | 90 532 | 0.0011 | 209 244 | 0.0011 | −1.3113 | 0.0011 | div_rem |
| 512 | 333 734 | 0.0007 | 466 234 | 0.0010 | −0.3970 | 0.0010 | div_rem |
| **1024** | 1 281 163 | 0.0011 | 1 024 520 | 0.0011 | **0.2003** | 0.0011 | **div_rem_fast** |
| 2047 | 5 360 678 | 0.0017 | 1 481 365 | 0.0017 | 0.7237 | 0.0017 | div_rem_fast |
| 2048 | 5 573 680 | 0.0016 | 2 306 161 | 0.0012 | 0.5862 | 0.0016 | div_rem_fast |
| 2049 | 5 124 891 | 0.0018 | 2 968 828 | 0.0015 | 0.4207 | 0.0018 | div_rem_fast |
| 4096 | 22 126 784 | 0.0018 | 5 082 611 | 0.0014 | 0.7703 | 0.0018 | div_rem_fast |

Monotone and interior: the fast arm loses at every grid point up to 512 and wins
at every grid point from 1024 upward. Record F4 compares this with the
constant's own tuning note.

## `polynomial.subproduct_min_len` — selected 512

Arms: `FieldPoly::eval_batch` (naive per-point Horner) against
`batch_evaluate_subproduct_auto`, the free function that runs the subproduct
tree unconditionally with `div_rem_auto`-backed reductions. Coefficient count
equals point count at every grid point, matching the two-sided gate.

| Coefficients = points | eval_batch ns | spread | subproduct_auto ns | spread | margin | band | Wins |
|---:|---:|---:|---:|---:|---:|---:|---|
| 128 | 53 831 | 0.0007 | 92 775 | 0.0017 | −0.7235 | 0.0017 | eval_batch |
| 256 | 219 080 | 0.0007 | 250 294 | 0.0057 | −0.1425 | 0.0057 | eval_batch |
| **512** | 883 634 | 0.0004 | 720 546 | 0.0011 | **0.1846** | 0.0011 | **subproduct_auto** |
| 1024 | 3 549 267 | 0.0003 | 2 224 051 | 0.0028 | 0.3734 | 0.0028 | subproduct_auto |
| 2048 | 14 232 168 | 0.0007 | 7 377 995 | 0.0068 | 0.4816 | 0.0068 | subproduct_auto |
| 4095 | 57 473 806 | 0.0006 | 18 771 198 | 0.0079 | 0.6734 | 0.0079 | subproduct_auto |
| 4096 | 57 389 256 | 0.0005 | 18 693 149 | 0.0072 | 0.6743 | 0.0072 | subproduct_auto |
| 4097 | 57 413 851 | 0.0007 | 19 340 035 | 0.0023 | 0.6631 | 0.0023 | subproduct_auto |
| 8192 | 229 566 394 | 0.0006 | 46 706 579 | 0.0075 | 0.7965 | 0.0075 | subproduct_auto |

Monotone and interior: the subproduct arm loses at 128 and 256 and wins from 512
upward. Record F5 compares this with the constant's tuning note.

## Cross-check against the committed pre-cutover baseline

The polynomial cells this sweep shares an entry point with agree with
`2026-08-19-pre-cutover-baseline.md` to within 1 %, which anchors the protocol
against an independently committed receipt from a different binary:

| Cell | Baseline ns/call | This run ns/call | Difference |
|---|---:|---:|---:|
| `mul_fast` at out_len 127, Karatsuba arm | 3 813.539 | 3 804.011 | 0.25 % |
| `mul_fast` at out_len 129, NTT arm | 11 910.678 | 11 829.852 | 0.68 % |
| `mul_fast` at out_len 255, NTT arm | 11 919.874 | 11 823.014 | 0.81 % |
| `mul` at operand length 33, Karatsuba arm | 1 140.815 | 1 135.357 | 0.48 % |

The bit-backend cells are not comparable in the same way: the baseline times
`gf2_core::kernels::ops::xor_inplace`, which resolves the backend on every call,
while this sweep times each backend directly, so the two differ by the
resolution the sweep deliberately excludes from the arm comparison.

## Emitted profile

Written to a unique absent `/tmp` path, then read back and reparsed through
`TuningProfile::from_json`; the parsed value compares equal to the one the sweep
built, and only then is it reported as an artifact. The document below is the
file byte-for-byte.

```json
{"schema_version":1,"profile_id":"gf2-5ecc9bf8-calibration-e202c080","provenance":{"kind":"calibrated","measured_at":"2026-08-19T22:17:10Z","source_revision":"e202c08089210f98842558bf1336143e6b6962d1","source_dirty":true,"harness":"crates/gf2-core/benches/tuning_calibration.rs","harness_schema":"tuning-calibration-v1","binary_sha256":"ac4c7731ca499a668459e701007e57bfcfafbadaa4d7dbc2f2344dba31224350","toolchain":"rustc 1.95.0 (59807616e 2026-04-14)","host":"fraktaali","cpu_model":"AMD Ryzen 9 5900X 12-Core Processor","cpu_features":["avx2","fma","pclmulqdq","sse4.1","vpclmulqdq"],"os_kernel":"Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux","governor":"powersave","lock_wrapper":"dev/scripts/ccx1-bench-flock.sh","lock_file":"/tmp/gf2-ccx1.lock","cpu_affinity":"6-11","executions":5,"repetitions":5,"target_ms":250,"receipt":"dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md"},"selectors":{"bit_backend":{"simd_min_words":4},"polynomial":{"karatsuba_max_out_len":128,"div_rem_fast_min_len":1024,"subproduct_min_len":512}}}
```

`selectors.polynomial` states three of its four fields. `karatsuba_min_degree`
is absent, which is what the schema's optional fields are for: `from_json`
resolves the absent key to the conservative default, so the loaded profile
selects 32 exactly as the compiled-in constant does. The round trip the harness
performs proves the loader accepts the document — one it rejected would abort
the run.

The remaining keys are in the order `TuningProfile::to_json` writes them, so an
emitted document has the same shape as the committed
`crates/gf2-core/data/tuning-profiles/conservative.json` and differs from it
only in content. The harness deletes the omitted key from the serialized text
and then checks the edited text against the same deletion applied to the parsed
value, so the ordering survives without the edit being taken on trust.

## Falsification record

### F1 — the Karatsuba/NTT crossover is not expressible as one threshold

`polynomial.karatsuba_max_out_len` gates `out_len <= t` to the Karatsuba arm, so
it presumes a single product length above which the NTT arm is always the
cheaper one. This sweep measures no such length.

The NTT arm's cost is a step function of the transform length
$N = 2^{\lceil \log_2 \text{out\_len} \rceil}$, not of the product length. It is
flat inside each power-of-two band and doubles at each band boundary:

| NTT band | Product lengths measured | mul_ntt ns | Spread within band |
|---|---|---:|---:|
| $N = 256$ | 129, 191, 255 | 11 829.9, 11 826.3, 11 823.0 | 0.06 % |
| $N = 512$ | 383, 511 | 24 863.8, 24 861.8 | 0.01 % |

The Karatsuba arm's cost rises smoothly across the same range: 3 788.5 at 129,
7 037.8 at 191, 11 888.8 at 255, 21 701.6 at 383, 36 558.7 at 511. A flat arm
crossing a rising one therefore crosses **once per band**: the NTT arm wins near
the top of a band (255, 511) and loses near the bottom (129, 383). The optimal
threshold is periodic in $\log_2 \text{out\_len}$, and no scalar $t$ with the
comparison `out_len <= t` can select the cheaper arm at both 255 and 383.

This contradicts the schema's premise for this field rather than any single
measurement, so §2.7 rule 3 keeps the default and the finding is recorded here
instead of being resolved by picking one of the two crossings.

### F2 — one field's two arms have no separate entry point

`polynomial.karatsuba_min_degree` gates the schoolbook/Karatsuba dispatch, whose
two implementations are the private `mul_schoolbook_impl` at
`crates/gf2-core/src/field/poly.rs:2484` and `mul_karatsuba_raw` at `:2531`.
Neither has a public entry point, and the module states the omission
deliberately at `crates/gf2-core/src/field/poly.rs:72`: "There is no standalone
`pub fn mul_karatsuba`: the schoolbook ⇄ Karatsuba crossover is internal". The
one public entry, `FieldPoly::mul`, resolves the arm itself from the compiled-in
`KARATSUBA_THRESHOLD`.

A benchmark target links `gf2-core` as an external crate and can name only its
public API, so no grid point offers both arms and §2.7 rule 1 cannot be executed
for this field. Installing a profile does not help at this revision: no
`poly.rs` read site consults `tuning::active()` yet, and `tuning::install`
resolves once per process in any case.

The field is therefore **uncalibrated, not measured-and-confirmed**, and the
emitted document omits it rather than stating 32 — the encoding design §5
condition 5 requires, so that no reader mistakes an inherited value for a
measured one. The dispatcher curve in the section above bounds nothing about the
crossover.

**Tracked owner: issue `389aa4de`**, "Extend the calibration sweep to
karatsuba_min_degree after the polynomial cutover", which depends on the
polynomial cutover `697fc55b`. Once that cutover has `mul_impl` read
`tuning::active()`, one child process installing a minimal threshold takes the
Karatsuba arm and another installing a maximal one takes the schoolbook arm, so
both arms come off the same public `FieldPoly::mul` on one grid with no new
public API. The gap is tracked rather than deferred.

### F3 — the `NTT_THRESHOLD` step recorded by `e8fe47f5` reproduces exactly

`2026-08-19-procedure-verification.md` §Falsification records that
`NTT_THRESHOLD = 128` appears mis-tuned: the `mul_fast` dispatcher takes the
`FieldPoly::mul` arm at 3 793 ns for `out_len = 127` and the NTT arm at
12 057 ns for `out_len = 129`, a 3.18× step upward on crossing into the arm the
threshold selects for being cheaper, with the arms converging near
`out_len = 255`.

This sweep reproduces every part of it from a different binary:

| Quantity | `e8fe47f5` record | This run |
|---|---:|---:|
| Karatsuba arm at out_len 127 | 3 793 ns | 3 804.011 ns |
| NTT arm at out_len 129 | 12 057 ns | 11 829.852 ns |
| Dispatcher step 127 → 129 | 3.18× | 3.11× |
| Arms at out_len 255 | converge | 11 888.8 vs 11 823.0, margin 0.0055 |

At out_len 129 itself the two arms stand 3.12× apart — 11 829.852 ns for the NTT
arm the threshold selects against 3 788.505 ns for the Karatsuba arm it rejects.

The mechanism is F1's: `NTT_THRESHOLD = 128` puts the switch at the *bottom* of
the $N = 256$ band, where the NTT arm pays a full doubling of transform length
for one extra output coefficient and the Karatsuba arm is at its cheapest within
the band. The step is a consequence of the threshold's placement, not noise.

Neither the constant at `crates/gf2-core/src/field/poly.rs:2711` nor its rustdoc
is changed by this issue, and the rustdoc's claim that below the threshold "the
caller is better served by the existing schoolbook / Karatsuba dispatch" is not
restated here as if it held: this receipt measures the Karatsuba arm as the
better-served one at 129, 191 and 383 as well, all of them above the threshold.

### F4 — `DIV_REM_THRESHOLD`'s pinned value excludes the cell its own tuning note cites

The rustdoc at `crates/gf2-core/src/field/poly.rs:2909` derives the value from
two cells: schoolbook still wins at $n = 1024$, $m = 512$ (1.07 ms against
1.12 ms fast), and the fast path "wins decisively" at $n = 2048$, $m = 1024$
(4.25 ms against 2.49 ms), "so the threshold is pinned at 2048, the smallest
power of two above the measured crossover".

This sweep measures the same two operand shapes and confirms the crossover sits
between them:

| Cell | Tuning note | This run |
|---|---|---|
| $n = 1024$, $m = 512$ | 1.07 ms vs 1.12 ms — schoolbook wins | 333.7 µs vs 466.2 µs — schoolbook wins |
| $n = 2048$, $m = 1024$ | 4.25 ms vs 2.49 ms — fast wins | 1.281 ms vs 1.025 ms — fast wins |

The contradiction is in the pinning, not the crossover. `div_rem_auto` gates on
`self.len < t || divisor.len < t`, so **both** lengths must reach $t$. With
$t = 2048$ the cell the note cites as the fast path's decisive win —
$n = 2048$, $m = 1024$ — has $m < t$ and is routed to schoolbook. The threshold
as pinned therefore excludes the measurement that justifies it. Expressed in the
gate's own units, the measured crossover is at a divisor length of 1024, which
is the value this run selects.

The absolute figures are also stale: both arms are 2.4× to 3.3× faster than the
note records, and the fast arm's advantage at $n = 2048$, $m = 1024$ is 1.25×
here against the note's 1.71×.

The constant and its rustdoc are unchanged by this issue.

### F5 — `SUBPRODUCT_THRESHOLD`'s tuning note is stale by 2.9× on the subproduct arm

The rustdoc at `crates/gf2-core/src/field/poly.rs:2192` records the measured
crossover for the `batch_evaluate_auto` dispatcher at $n = k = 4096$ on the
reference Zen 3 host, "54.29 ms `subproduct_auto` vs 60.89 ms naive, a 0.89×
win".

At that same cell this run measures 18.69 ms for the subproduct arm against
57.39 ms naive — a 0.326× win. The naive arm agrees with the note to 5.7 %; the
subproduct arm is 2.90× faster than recorded. The crossover is correspondingly
three octaves lower: the subproduct arm already wins at $n = k = 512$ by 1.23×,
and loses only at 256 and below.

The measurement is anchored by the committed baseline, which records
`polynomial/batch_evaluate_auto/coeffs=4096/points=4096` at 19.245 ms on the
subproduct arm and `coeffs=4095/points=4095` at 56.938 ms on the naive arm —
2.9 % and 0.9 % from this run's figures for the same work. The note's 54.29 ms is
the outlier, not this run.

The constant and its rustdoc are unchanged by this issue.

### F6 — the emitted profile is not committable from this run

`Provenance::Calibrated` carries `source_dirty: true`, because the calibration
harness that produced the numbers is itself uncommitted at the revision the
profile names, `e202c08089210f98842558bf1336143e6b6962d1`. The design requires
`source_dirty: false` of a committed profile: a dirty revision does not
reproduce the binary that produced the measurements.

This does not weaken the receipt — the binary is identified by its SHA-256 and
every figure above comes from that binary — but it does bound what the emitted
document may be used for. No profile is committed under
`crates/gf2-core/data/tuning-profiles/` from this run, which is also the owner
decision for this issue independently of the dirty tree. The harness prints the
condition rather than emitting silently.

## What this receipt does not claim

- It records one host. The library performs no host matching and auto-selects no
  profile, so nothing here changes any build's behaviour.
- The three measured-value fields are measured against the arms' own entry
  points, not against the dispatchers that will read them after the cutover. A
  dispatcher adds the same profile read to both arms, so the comparison is
  unaffected, but the absolute rates here are lower than a dispatcher's.
- `simd_min_words` is measured on `Backend::xor` alone. The production path at
  `crates/gf2-core/src/kernels/ops.rs` resolves the backend per call, which adds
  a constant to both arms and so shrinks the relative margin at the smallest
  buffers below the 23.1 % measured at 4 words.
- The grids are logarithmic. Each selected value is the smallest **grid point**
  at which the asymptotic arm wins, so it bounds the true crossover from above,
  and no claim is made about sizes between grid points.
- Nothing is claimed about `karatsuba_min_degree`'s crossover. The emitted
  document's silence on that field is the claim: it is unmeasured.
- No number here is a non-regression verdict. The pinned set of
  `selector-non-regression-plan-v1.md` and its receipts own that comparison.

## Format-2 supersession (2026-08-26)

Every figure and the emitted JSON quoted above remain the evidence produced by
this run. The quoted emitted file is 1,086 bytes with SHA-256
`f0c3100ddd23fe3bbaadb5bf3b7fde51af78fc6e9929bb76886b1b032a165635`.
Later history committed the same document with a terminating newline: those
exact 1,087 bytes have SHA-256
`674eea65379d1c814cd54584ad1ea4517fc3f2adbef3d5229d58593e9aad63bb`
and are preserved at
`dev/archive/3fa7c9d0/tuning-profiles/gf2-5ecc9bf8-calibration-e202c080-v1.json`.
They remain format-1 historical evidence and are never relabeled format 2.

The active `conservative.json` artifacts are inherited format-2 owner and
complete envelopes. They make no claim to this receipt's calibration, even
where a conservative value happens to equal a value recorded here. A future
`389aa4de` receipt may supersede this run with measured format-2 core-section
evidence; until then, baked measurements cite the archived bytes and digest
above. This section changes no prior measurement, conclusion, or quoted byte.

## Amendment — issue `389aa4de` (2026-09-01)

This section is appended and changes no line or figure above it.

F2 records `polynomial.karatsuba_min_degree` as uncalibrated at this receipt's
revision. That condition is discharged. The polynomial cutover makes
`FieldPoly::mul` profile-steerable, and the calibration harness reaches both
arms in verified fresh child processes. The schoolbook child installs the top
of the admissible range; the Karatsuba child installs the grid point itself,
so `mul_karatsuba_raw` performs the one split over schoolbook base cases that a
threshold at that point produces. This DEC-B16 forcing replaces the earlier
minimal-against-maximal description.

[`2026-09-01-389aa4de.md`](2026-09-01-389aa4de.md) records the five-field
format-2 sweep. It selects `karatsuba_min_degree = 31`, so the field is
sweepable and no longer omitted. The codec-derived result is five measured and
32 omitted fields from the 37-field core inventory; 33 is the historical
four-field omission count, not the current count. DEC-G's baked
`simd_min_words` value remains 4 and is re-pinned to that measured format-2
core section without changing the boundary.

The paragraph above §Result saying no calibrated profile was committed was
true at this receipt's revision. Later history first committed and then
archived this run's exact format-1 bytes. Issue `389aa4de` cuts the last live
reader and citation over to
`crates/gf2-core/data/tuning-profiles/gf2-389aa4de-20260901-040229-2742533.json`;
after that cutover the uncited archived file is deleted. The historical path,
1,087-byte digest
`674eea65379d1c814cd54584ad1ea4517fc3f2adbef3d5229d58593e9aad63bb`,
dirty source revision `e202c08089210f98842558bf1336143e6b6962d1`, the emitted
document quoted above, and every measured figure remain the exact record of
this run. No format-1 reader, compatibility alias, or dual token remains.
