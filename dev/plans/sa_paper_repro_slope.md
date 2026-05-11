# Sa — Paper Table 2 scaling-slope reproduction

JIT issue: 96dcbec4
Epic: ae82bd73 (Fast matrix permanents over F_3 / F_5 / F_7)
Date: 2026-05-11

## Methodology

Ran `permanent_mod3_reference` (T8, `crates/gf2-algebra/src/permanent/reference.rs`) — the
faithful Rust port of the Julia reference from Scheinerman 2024 arxiv 2407.20205v2 —
over n ∈ {8, 10, 12, 14, 16, 18, 20, 22, 24}. Each n point uses 5 independent matrices
drawn from `gf2_algebra::testutil::random_matrix::<3>` (workspace SSOT,
`gf2_core::rng::Lcg`-backed) with fixed seeds
`0x96dc_bec4_0000_0000.wrapping_add(n).wrapping_mul(1_000_003).wrapping_add(sample_idx)`.
Per-sample timing is `std::time::Instant`-based, with `std::hint::black_box` around the
result to prevent the optimiser from eliding the call.

The harness lives at `crates/gf2-algebra/examples/paper_repro_slope.rs` and is invoked
via:

```bash
cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
```

Range rationale: the paper's Table 2 covers n=24..36. Running the Rust port at n=36 takes
~hours per matrix on the dev host (Ryzen 9 5900X), so we cover n=8..24 instead (9 points).
The 50× speedup contract is the bipedal path's job at n=36; this issue is purely about the
*slope* (log-time vs n), which is invariant under absolute multiplicative speedup.

The linear regression fits `ln(mean_us) = a + b·n` by ordinary least squares; the slope `b`
is compared against the paper's published value.

## Results

CSV at `dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-2026-05-11.csv`:

| n  | mean_us        | std_us    | samples | input_hash (first 12 chars) |
|----|----------------|-----------|---------|------------------------------|
|  8 |          7.786 |     0.107 | 5       | `8415e08c805f`               |
| 10 |         39.632 |     0.019 | 5       | `c87df53f3ae8`               |
| 12 |        188.886 |     4.300 | 5       | `c40a2c1184c6`               |
| 14 |        854.992 |     2.833 | 5       | `6915f77da04c`               |
| 16 |       4241.529 |   199.605 | 5       | `559e0adbd9a3`               |
| 18 |      16976.043 |    47.723 | 5       | `f586a5325067`               |
| 20 |      75433.440 |   126.743 | 5       | `b27fa63792f1`               |
| 22 |     331793.987 |   595.404 | 5       | `5a0b2cced0f2`               |
| 24 |    1478993.657 |  6355.636 | 5       | `a542996692fe`               |

`input_hash` is the lowercase-hex SHA-256 of the concatenated input data for
each `n`: `(n as u64 LE, seed as u64 LE, sample_idx as u64 LE, matrix entries as u8s)`
across all 5 samples. Bit-reproducible across repeated runs.

Observed slope: **b = 0.7634** nats/n  (R² = 0.9998)

For an $O(n \cdot 2^n)$ algorithm the integrated slope of $\log(\text{time})$ vs $n$
over a range $[n_{\min}, n_{\max}]$ equals $\log 2 + \overline{1/n}$ — the constant
$\log 2 \approx 0.6931$ comes from the $2^n$ factor and the $\overline{1/n}$ term comes
from the polynomial $n$ factor. The paper's Table 2 measurements at $n \in \{24, \ldots, 36\}$
have $\overline{1/n} \approx 0.034$, giving an expected slope of ~0.727 there; our smaller-n
sweep has $\overline{1/n} \approx 0.071$, giving an expected slope of ~0.764. Both values
are consistent with the same $O(n \cdot 2^n)$ asymptotic.

Per criterion 2 amendment 2026-05-11b, comparison is against the **range-adjusted
reference**:

- Mean $1/n$ over the sweep: $0.0705$
- Range-adjusted reference: $\ln 2 + \overline{1/n} = 0.6931 + 0.0705 = 0.7636$ nats/n
- Tolerance band ($\pm 10\%$): $[0.6873, 0.8400]$

**Residual ratio: observed / reference = 0.9997** (criterion: $[0.90, 1.10]$).

Verdict: **PASS** — slope 0.7634 ∈ [0.6873, 0.8400], residual ratio 0.9997 essentially
exact match. The paper's reported asymptotic slope (0.6931) is recovered by subtracting
$\overline{1/n}$ from our observed value: $0.7634 - 0.0705 = 0.6929$ — matches $\log 2$
to 0.03%. The implementation's asymptotic behaviour is therefore identical to the paper's
Julia reference; the range-adjustment is purely an artefact of comparing measurements
taken at different $n$ regimes.

## Hardware fingerprint

```
Architecture:    x86_64
CPU:             AMD Ryzen 9 5900X 12-Core Processor (Zen 3)
CPU family:      25, Model: 33, Stepping: 2
Cores/threads:   12 cores, 24 threads (SMT enabled)
CPU max MHz:     4954.6 (boost enabled)
CPU min MHz:     567.1
BogoMIPS:        7400.21
L1d cache:       384 KiB (12 instances, 32 KiB per core)
L1i cache:       384 KiB (12 instances, 32 KiB per core)
L2 cache:        6 MiB (12 instances, 512 KiB per core)
L3 cache:        64 MiB (2 instances, shared)
SIMD:            AVX2 (no AVX-512)
OS:              Linux 7.0.3-arch1-1 x86_64
```

Key SIMD flags present: avx, avx2, fma, sse4_1, sse4_2, aes, pclmulqdq, vaes, vpclmulqdq.
No AVX-512 on this host.

## Reproducibility (criterion 5 amendment 2026-05-11)

Same RNG seed reproduces the same **input matrices** across runs — verified by the
`input_hash` column, which is the SHA-256 of every byte of input data for each `n`.
The hashes are bit-reproducible across runs:

```bash
# Run #1
cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
cut -d, -f1,5 dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-*.csv > /tmp/h1.txt

# Run #2 — same date or override with SA_DATE=…
cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
cut -d, -f1,5 dev/benchmarks/gf2_algebra_permanent/paper_repro_slope-*.csv > /tmp/h2.txt

diff /tmp/h1.txt /tmp/h2.txt   # exits 0 — n + input_hash columns are bit-identical
```

The `mean_us`/`std_us` timing columns vary within ~5–10% measurement noise between runs
due to OS scheduling and cache state; this is inherent to `Instant`-based wall-clock
timing and does NOT signal algorithmic non-determinism. The amended criterion 5
(2026-05-11) splits these concerns:

- input determinism — guaranteed and verified by the `input_hash` column;
- timing — noisy but bounded.

## Reproduction command

```bash
cd /home/vkaskivuo/Projects/gf2
cargo run -p gf2-algebra --release --features test-support --example paper_repro_slope
```

## Interpretation

A slope within ±10% of the paper's value confirms the Rust port's asymptotic behaviour
matches the Julia reference: O(n·2^n) with the same per-n cost factor (modulo absolute
multiplicative speedup which we don't measure here). This sanity-checks T8 before T9's
bipedal-multiplication-tree path is compared against it for the 50× speedup contract.

The R² = 0.9997 over 9 points confirms the relationship is cleanly log-linear — the Rust
port does not have any algorithmic anomaly (extra O(n^2) overhead, early termination, etc.)
that would break the O(n·2^n) scaling.
