# Determinant companion cost at campaign sizes

This receipt measures `FieldMatrix::det` beside one existing processor
permanent candidate at every requested $(q,n)$ cell. It is a kernel-cost
calibration for the determinant companion, not the composite
draw-pack-evaluate-count backend-selection receipt. The later campaign freeze
combines this marginal cost with its separately measured selected backend.

## Verdict

The determinant companion's measured marginal addition fits the twelve-hour
per-cell ceiling at every measured cell. At the protocol's fixed sample counts,
the largest addition is $0.014288$ h (about $51.4$ s) at $(q,n)=(3,20)$;
there are **no determinant-marginal failures**. Every determinant addition also
fits inside the feasibility study's $15\%$ operational reserve of $1.8$ h.

The reference permanent plus determinant total is a different verdict. These
single-matrix processor candidates exceed twelve hours at
$(3,20)$, $(3,28)$, and $(5,24)$. That does not make the determinant
unaffordable: the determinant contributes only $0.014288$, $0.000328$, and
$0.000188$ h respectively. It says those permanent candidates are not the
campaign's eventual throughput selections at those cells.

## Protocol and provenance

| Item | Recorded value |
|---|---|
| Canonical raw receipt | `dev/benchmarks/permanent_campaign/determinant-cost.csv` |
| Raw receipt SHA-256 | `4f192f4c834e0c552a10dd790f1f80b3e685e8628da0d28fa8e710763dae8325` |
| Schema | `determinant-companion-v2` |
| Source revision | `67c093025374e18de3ce0b9583bcce07bfbb4d4e` |
| Tracked measurement source dirty | `false` |
| Benchmark binary SHA-256 | `638aa2731ccec44dc7af03080b8f607b57806f8b381479e03674915c6b3b53e4` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`; Cargo bench release profile |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor; x86-64; 12 cores / 24 threads |
| Kernel | Linux `7.1.6-arch1-1`, x86-64 GNU/Linux |
| Frequency policy | `powersave` governor; boost enabled; no fixed-frequency claim |
| Isolation | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`; pinned to logical CPUs 6–11 |
| Niceness | Wrapper's best-effort `nice -n -5` was denied; measurements ran at ordinary niceness |
| Canonical measurement window | 2026-08-09 18:04:17–18:06:58 UTC |
| Seed root | `0x8cb4_def5_0000_0000` |
| Cell seed | root XOR $(q \ll 48)$ XOR $(n \ll 32)$; fixture $i$ adds $i$, for $i=0,\ldots,31$ |
| Fixtures | 32 deterministic row-major matrices per cell; determinant and permanent representations come from identical entries; the shared recorded-window start offset is `(execution - 1) * 5 + (repetition - 1)` modulo 32 |
| Timed boundary | Evaluation only; generation and conversion to `FieldMatrix` / packed forms are excluded |
| Repetitions | Five fresh processes; five raw repetitions per operation per cell; 25 repetitions per operation per cell |
| Window calibration | 250 ms target per repetition; a call slower than the target runs once |
| Raw evidence | 550 rows: $5$ executions $\times 11$ cells $\times 2$ operations $\times 5$ repetitions |

At the one-call frontier cells, the 25 permanent repetitions cover fixture
starts $0,\ldots,24$ exactly. The determinant row with the same execution,
repetition, field, and size records the identical start.

The canonical command, run from the worktree root, was:

```sh
test ! -e dev/benchmarks/permanent_campaign/determinant-cost.csv
./dev/scripts/ccx1-bench-flock.sh bash -lc 'for e in 1 2 3 4 5; do cargo +1.95.0 bench -p gf2-algebra --bench determinant_companion --features test-support -- --execution "$e" --repetitions 5 --target-ms 250 --output dev/benchmarks/permanent_campaign/determinant-cost.csv --append; done'
```

The harness resolves relative output paths from the workspace root even though
Cargo launches the bench with the package directory as its working directory.
The five-process canonical run took about $178.9$ s wall clock including
per-process calibration and Cargo launch overhead.
Commit `566b6ffe` followed the measurement and changes only rustfmt whitespace;
the raw receipt and binary hash remain pinned to measured revision `67c09302`.

## Candidate naming and scope

The permanent column uses a supported campaign candidate, not an assertion that
the candidate will win the later backend freeze:

- $\mathbb F_3$: public `permanent_bipedal3`, whose current single-matrix
  dispatch is the scalar kernel. The direct single-matrix AVX2, four-matrix
  batch, intra-matrix Rayon, and accelerator paths are separate APIs and are
  not measured here.
- $\mathbb F_5$: public `permanent_bipedal5`, the packed scalar path.
- $\mathbb F_7$, $n\leq16$: public `permanent_bipedal7`, the packed scalar
  path.
- $\mathbb F_7$, $n=20$: `permanent_ryser::<Fp<7>>`, the existing generic CPU
  candidate, because the packed $\mathbb F_7$ path has a 16-lane limit.

This interpretation follows the reviewed decomposition: determinant
calibration and contested-backend remeasurement are separate leaves, and the
campaign freeze depends on both. A composite backend-selection claim would
require draw, pack, evaluate, and count throughput and is outside this
receipt.

## Pooled wall time and measured ratio

For operation $x$, the reported time is formed from pooled raw totals,

$$
t_x=\frac{\sum_r T_{x,r}}{\sum_r M_{x,r}},
\qquad
R_{\det/\operatorname{per}}=\frac{t_{\det}}{t_{\operatorname{per}}}.
$$

No per-repetition reciprocal and no average of ratios enters the result. The
complexity proxy is $n^2/2^n$; the last column is measured ratio divided by
that proxy.

| $q$ | $n$ | determinant ($\mu$s/matrix) | permanent ($\mu$s/matrix) | measured $R_{\det/\mathrm{per}}$ | $n^2/2^n$ | measured / proxy |
|---:|---:|---:|---:|---:|---:|---:|
| 3 | 4 | 0.189867 | 0.113133 | 1.67826707 | 1 | 1.678 |
| 3 | 12 | 1.014044 | 18.444512 | 0.0549781071 | 0.03515625 | 1.564 |
| 3 | 20 | 2.571821 | 4,555.352831 | $5.6457110\times10^{-4}$ | $3.8146973\times10^{-4}$ | 1.480 |
| 3 | 28 | 5.313695 | 1,170,147.961480 | $4.5410452\times10^{-6}$ | $2.9206276\times10^{-6}$ | 1.555 |
| 5 | 4 | 0.193106 | 0.316038 | 0.611020916 | 1 | 0.611 |
| 5 | 12 | 1.087119 | 158.898627 | 0.00684158703 | 0.03515625 | 0.195 |
| 5 | 20 | 2.767362 | 66,320.728720 | $4.1726954\times10^{-5}$ | $3.8146973\times10^{-4}$ | 0.109 |
| 5 | 24 | 4.213811 | 1,266,584.681200 | $3.3269080\times10^{-6}$ | $3.4332275\times10^{-5}$ | 0.097 |
| 7 | 4 | 0.187834 | 0.335077 | 0.560569574 | 1 | 0.561 |
| 7 | 12 | 1.154036 | 164.677078 | 0.00700787185 | 0.03515625 | 0.199 |
| 7 | 20 | 2.893068 | 68,711.101533 | $4.2104813\times10^{-5}$ | $3.8146973\times10^{-4}$ | 0.110 |

### Complexity-proxy contradiction

The measurements preserve the order-level conclusion—determinant share falls
rapidly with $n$—but contradict use of $n^2/2^n$ as a field-independent
quantitative ratio. Over $\mathbb F_3$ the measured ratio is $1.48$–$1.68$
times the proxy. At the larger $\mathbb F_5$ and $\mathbb F_7$ cells the proxy
instead overstates determinant share by about $5.0$–$10.3$ times. At $n=4$
the proxy is one, while the determinant is slower than the $\mathbb F_3$
permanent and faster than the $\mathbb F_5$ and $\mathbb F_7$ permanents.
These are constant-factor/backend effects, not a contradiction of the
$O(n^3)$ versus $\Theta(n 2^n)$ complexity classes.

## Dispersion

“Within” is the range, across the five executions, of the sample coefficient
of variation of that execution's five repetitions. “Across” is the sample
coefficient of variation of the five per-execution pooled times. Slow cells
with one call per repetition still have five independently timed repetitions
per process.

| $q$ | $n$ | det within CV | det across CV | permanent within CV | permanent across CV |
|---:|---:|---:|---:|---:|---:|
| 3 | 4 | 0.274–0.751% | 3.635% | 0.120–1.508% | 0.818% |
| 3 | 12 | 0.430–1.561% | 1.217% | 0.399–1.617% | 0.730% |
| 3 | 20 | 0.235–1.364% | 3.284% | 0.440–0.679% | 0.334% |
| 3 | 28 | 0.283–1.326% | 1.069% | 0.196–0.642% | 0.299% |
| 5 | 4 | 0.284–0.901% | 0.414% | 0.360–0.775% | 0.515% |
| 5 | 12 | 0.148–0.775% | 0.375% | 0.354–1.070% | 0.888% |
| 5 | 20 | 0.203–1.032% | 0.988% | 0.596–1.562% | 0.512% |
| 5 | 24 | 0.282–0.905% | 0.453% | 0.186–0.679% | 0.265% |
| 7 | 4 | 0.176–1.110% | 0.252% | 0.423–1.210% | 0.509% |
| 7 | 12 | 0.535–0.787% | 0.479% | 0.271–0.971% | 0.449% |
| 7 | 20 | 0.259–1.192% | 0.685% | 0.456–1.337% | 0.560% |

### Preserved non-canonical first-run contradiction

An initial five-process run at source revision `2f0a2eaf` wrote to a
package-relative staging path because the harness had not yet anchored relative
outputs to the workspace root. It was structurally valid but was not selected
or published as the canonical cohort. Its raw SHA-256 was
`6e2ba2d8a256f6e542a417c19a1b4112346fe2f97b429c167ac77a6d820222cd`.
After the canonical run validated, that worker-owned staging file was moved
recoverably to `/tmp/determinant-cost-misplaced-2f0a2eaf.csv` rather than
silently treated as canonical evidence.

An intermediate version-1 cohort at source revision `4f173f3a` was also
structurally valid (raw SHA-256
`eb1c12181a8095444a7c2ee064a0a6fd9d54f071ead51567880dc35ec765c330`),
but read-only review found that its one-call frontier repetitions always
started at fixture 0. It is preserved at
`/tmp/determinant-cost-canonical-pre-offset-4f173f3a.csv` and excluded from the
canonical pool. Version 2 fixes that defect and records the paired fixture
start in every raw row.

The initial package-relative run exposed a real dispersion warning. At
$(q,n)=(5,12)$, determinant
per-execution pooled times were $1.304$, $1.088$, $1.061$, $1.059$, and
$1.062\ \mu$s/matrix, giving $9.540\%$ across-execution CV despite only
$0.190$–$1.552\%$ within-execution CV. The canonical rerun measured $1.086$,
$1.094$, $1.085$, $1.088$, and $1.083\ \mu$s/matrix, or $0.375\%$
across CV.
The first execution's transient did not reproduce, so it is preserved as a
contradiction rather than pooled into or erased from the canonical cohort.

More broadly, canonical-versus-initial pooled permanent times differed by up
to $6.694\%$ at $(5,12)$ and $5.723\%$ at $(5,24)$, wider than either
cohort's within-process dispersion. The powersave governor and unfixed boost
state are therefore material limitations. The orders-of-magnitude marginal
budget verdict is robust to those differences; this receipt must not be used
to rank close permanent backends.

## Twelve-hour budget projection

The fixed $N$ values come from the frozen campaign protocol. “Det addition” is
$Nt_{\det}$ and answers whether adding the companion fits. “Reference total”
is $N(t_{\operatorname{per}}+t_{\det})$ for the candidate measured here; it is
reported to prevent the candidate from being mistaken for the later selected
backend.

| $q$ | $n$ | fixed $N$ | det addition (h) | det addition fits 12 h | reference total (h) | reference total fits 12 h |
|---:|---:|---:|---:|:---:|---:|:---:|
| 3 | 4 | 20,000,000 | 0.001055 | yes | 0.001683 | yes |
| 3 | 12 | 20,000,000 | 0.005634 | yes | 0.108103 | yes |
| 3 | 20 | 20,000,000 | 0.014288 | yes | 25.321804 | **no** |
| 3 | 28 | 222,223 | 0.000328 | yes | 72.231936 | **no** |
| 5 | 4 | 16,000,000 | 0.000858 | yes | 0.002263 | yes |
| 5 | 12 | 16,000,000 | 0.004832 | yes | 0.711048 | yes |
| 5 | 20 | 160,000 | 0.000123 | yes | 2.947711 | yes |
| 5 | 24 | 160,000 | 0.000187 | yes | 56.292840 | **no** |
| 7 | 4 | 12,244,898 | 0.000639 | yes | 0.001779 | yes |
| 7 | 12 | 12,244,898 | 0.003925 | yes | 0.564051 | yes |
| 7 | 20 | 122,449 | 0.000098 | yes | 2.337211 | yes |

Thus no cell excludes the determinant companion on measured marginal cost.
The three reference-total failures instead require the separately receipted
parallel or accelerator selection before the root manifest freezes.
