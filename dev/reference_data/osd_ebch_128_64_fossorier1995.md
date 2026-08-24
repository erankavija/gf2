# eBCH $(128,64)$ ordered-statistics decoding reference curve

> **Diátaxis Type:** Reference

Provenance for
[`osd_ebch_128_64_fossorier1995.csv`](/dev/reference_data/osd_ebch_128_64_fossorier1995.csv),
a digitization of the published ordered-statistics decoding (OSD) error-rate
curves of the extended BCH $(128,64,22)$ code on the additive white Gaussian
noise (AWGN) channel with BPSK signalling.

The dataset carries five series, one per reprocessing order $l \in \{0,1,2,3,4\}$.
The order-2 series is the reproduction target; the order-1 series is the
published control described under
[Order-1 control status](#order-1-control-status).

**The published metric is bit error rate, not block error rate.** The epic's
reproduction target is stated as BLER; the source publishes BER, and this
dataset stores BER unconverted. Read
[Channel, modulation, metric, and abscissa](#channel-modulation-metric-and-abscissa)
before comparing anything against these values.

## Source pin

The reproduction target work is `@/citation/Fossorier1995` — Fossorier, Lin,
"Soft-Decision Decoding of Linear Block Codes Based on Ordered Statistics",
IEEE Trans. Inf. Theory 41(5):1379–1396, 1995, doi:`10.1109/18.412683`.

**The page, figure, and series identifiers inside `Fossorier1995` are not
verified.** That article is closed access: Unpaywall reports
`is_oa: false`, `oa_status: "closed"`, `has_repository_copy: false` and an
empty `oa_locations` list for `10.1109/18.412683` (queried 2026-08-24), and the
IEEE Xplore article-PDF endpoints answer with authentication redirects. No open
copy was located. Its figure numbering, page numbering, and series legends
therefore remain unknown, and this document asserts none.

The digitized values come from the author's antecedent dissertation, which is
open access and reports the same algorithm, code, channel, and reprocessing
orders:

| Field | Value |
| --- | --- |
| Work | Fossorier, Marc P.C., *Decoding of linear block codes based on ordered statistics*, Ph.D. dissertation, University of Hawai‘i at Mānoa, December 1994 (UMI order number 9519442) |
| Repository | ScholarSpace, item `46246020-a80c-43a9-912e-68f3774c3f3e`, bitstream `3a655537-c031-436f-ac2d-61d2516aa046` (`uhm_phd_9519442_r.pdf`) |
| Retrieved | 2026-08-24 |
| Bytes / pages | 4 640 976 / 202 |
| SHA-256 | `9e867a44f2a7d54047396383db5e6bc4fb0cb083c83faea2a9c8649a6f3e5eeb` |
| Digitized locations | Figure 4.14, p. 60 (all five series); Table 4.7, p. 71 (order 2); Table 4.8, p. 71 (order 3); Table 4.9, p. 71 (order 4) |

Figure 4.14 carries the caption "Error performances for the (128,64,22)
extended BCH code" and a `Simulations` legend listing Orders 0 through 4. Four
of the five marker glyphs are legible in the scan — `x: Order 0`, `+: Order 1`,
`o: Order 2`, `*: Order 3` — while the Order 4 glyph is obscured by a dotted
gridline and is recorded as illegible rather than guessed. The figure also
carries an uncoded-BPSK reference, a soft-decision union bound, and theoretical
order-$l$ curves. Tables 4.7–4.9 are captioned "Order-$l$ simulation results
for (128,64,22) extended BCH code (\*: union bound)".

This dissertation resolves in no key of the citation registry. Adding one is
outside this artifact's scope and is recorded under
[Gaps](#gaps-and-contradictions).

## Code construction identity

What the source fixes:

- Code parameters $(N,K,d_H) = (128,64,22)$, named "extended BCH", i.e. the
  primitive BCH code of length $127$ and dimension $64$ extended by an overall
  parity bit.
- Rate $R = K/N = 1/2$, used directly in the abscissa convention below.
- The systematic convention the decoder applies: the generator matrix $G$ is
  column-permuted by the reliability order $\lambda_1$, a second permutation
  $\lambda_2$ moves columns that are dependent on their predecessors, and the
  result is reduced by Gaussian elimination to a systematic $G_1$ with the
  matching parity check $H_1 = [P^{T} \mid I_{N-K}]$ (§2.2.1, pp. 10–13). The
  systematic form is therefore derived per received block; the source stores
  and pins no fixed systematic generator.
- One construction-dependent structural statistic that a reproduction can
  check: $N_{\text{ave}}(1) = 33.00$ for `eB (128,64,22)`, the average number
  of columns depending on one dimension, averaged over systematic forms
  (Table 4.2, p. 40), against the binomial reference $(N-K)/2 + 1 = 33.00$.

What the source does **not** fix: the primitive polynomial of
$\mathrm{GF}(2^7)$, the generator polynomial of the length-127 base code, the
field element ordering, and the coordinate order of the extension bit. No
statement anywhere in the dissertation names them. A campaign choosing a
primitive polynomial therefore makes a campaign decision that carries no
warrant from this source and must be recorded as such.

Analytical note, not a source claim: primitive BCH codes of a fixed length and
designed distance built over different representations of $\mathrm{GF}(2^7)$
are equivalent under a coordinate permutation, and reliability-ordered decoding
is covariant with that permutation, so these curves cannot discriminate the
choice.

## Channel, modulation, metric, and abscissa

| Aspect | Pinned value |
| --- | --- |
| Channel | AWGN, $r_i = x_i + w_i$, with $w_i$ independent zero-mean Gaussian of variance $N_0/2$ (§2.1, pp. 9–10) |
| Modulation | BPSK, $x_i = (-1)^{c_i} \in \{\pm 1\}$; energy per transmitted bit normalized to unity, so $E_s = 1$ (§2.1, p. 9; §4.2.3, p. 49) |
| Metric | **Bit error rate.** The plotted and tabulated quantity is $P_e$, named verbatim as a BER: "At the BER $P_e = 10^{-6}$ …" (p. 64), "near optimum performance … for BER $P_e \le 10^{-6}$" (p. 63) |
| Abscissa | $E_b/N_0$ in dB, rate-adjusted with $R = 1/2$ |
| Decision rule | Minimum squared Euclidean distance to the received sequence |

The figure and table axes are labelled only "SNR (in dB)". Two facts fix the
normalization as $E_b/N_0$:

1. The published coding gains are quoted against uncoded BPSK, which is a gain
   at equal $E_b/N_0$. At BER $10^{-6}$ the source reports 5.6 dB for order 2,
   6.5 dB for order 3, and 7.0 dB for order 4 (p. 64). Uncoded BPSK reaches
   BER $10^{-6}$ at $E_b/N_0 = 10.53$ dB. Reading the plotted abscissa as
   $E_b/N_0$, the three series cross $10^{-6}$ at $4.78$, $4.07$, and $3.62$ dB
   — linear interpolations in $\log_{10} P_e$ between the bracketing tabulated
   points — giving 5.74, 6.46, and 6.91 dB, each within 0.15 dB of the printed
   value. Reading it as $E_s/N_0$ instead shifts every coded curve $10\log_{10}
   (1/R) = 3.01$ dB to the right on an $E_b/N_0$ axis, cutting the three gains
   to 2.73, 3.45, and 3.90 dB and contradicting all three printed numbers.
2. The six tabulated abscissas $\{2.22, 3.01, 3.47, 3.98, 4.56, 5.23\}$ dB
   reproduce $10\log_{10}(2/N_0)$ exactly, to every printed digit, for
   $N_0 \in \{1.2, 1.0, 0.9, 0.8, 0.7, 0.6\}$ with $E_s = 1$ and $R = 1/2$.

**The metric is BER, not BLER.** The dataset stores what the source publishes
and applies no conversion. A BLER comparison against this dataset requires the
campaign to supply its own bit-error accounting; the ratio between the two
metrics is not published here and is not inferable from these values.

## Reprocessing list order and tie policy

The candidate enumeration order is by ascending Hamming weight over the $K$
most-reliable independent (MRI) positions. Order-$l$ reprocessing is defined as
(§4.1.1, p. 30):

> For $1 \le i \le l$, make all possible changes of $i$ of the $K$ MRI bits of
> $a$. For each change, reconstruct the corresponding codeword … Compute the
> squared Euclidean distance … and record the codeword $a^{*}$ for which
> $x^{*}$ is closest to $z$. When all the $\sum_{i=0}^{l} \binom{K}{i}$
> possible codewords have been tested, order-$l$ reprocessing of $a$ is
> completed …

Consequences that bind a reproduction:

- Phase 0 is the re-encoding of the unaltered MRI hard decisions; phase $i$
  enumerates every weight-$i$ pattern over the MRI positions; phases run in
  increasing $i$. The order-$l$ list is the union of phases $0$ through $l$,
  of size $\sum_{i=0}^{l}\binom{K}{i}$.
- Within a phase the source imposes no enumeration order, and none is
  observable: the decision is the minimizer of squared Euclidean distance over
  the whole list, not a first-acceptance rule.
- **Reliability tie policy: none is specified.** The ordering is defined by
  $|y_1| > |y_2| > \dots > |y_N|$ with the explicit assumption that
  "$y_i = y_j$ with $i \ne j$ has zero probability of occurrence" (§2.2.1,
  p. 11). The source therefore states a measure-zero argument in place of a
  tie-breaking rule. A finite-precision reproduction must define its own rule
  and record it as an implementation decision, not as a source-matched one.
- **Distance tie policy: none is specified.** The text says only that the
  closest codeword is recorded.
- A resource test (§4.1.3, p. 34) prunes candidates that cannot improve the
  recorded minimum. Chapter 5 threshold rules and Chapter 6 fading-channel
  results are separate scopes and contribute nothing to this dataset.

## Digitization method and precision

Two ordinate sources appear in the CSV, distinguished by the
`ordinate_source` column.

`printed_table` — the value is the printed numeral of Table 4.7, 4.8, or 4.9,
transcribed from a 300 dpi render of the scanned page and read visually. The
tables print $P_e$ as $10^{-x}$ with one decimal in the exponent, so the
transcription is exact and the source's own quantization is $\pm 0.05$ in
$\log_{10}$, about $\pm 12\%$ in linear value. The `value_kind` column carries
the table's `*` annotation: `union_bound` marks the entries the caption flags
as union-bound values rather than simulation, `simulation` marks the rest.

`axis_calibrated_pixel_read` — the value is read from Figure 4.14 by axis
calibration:

1. Render p. 60 at 300 dpi (`pdftoppm -r 300`).
2. Locate the plot frame and the dotted decade gridlines by dark-pixel row and
   column projections. The eleven horizontal gridlines land at rows
   759, 898, 1039, 1180, 1320, 1460, 1600, 1742, 1883, 2024, 2165 for
   $\log_{10} P_e = 0 \dots -10$; the vertical gridlines give
   $178.0$ px/dB with $E_b/N_0 = 1$ dB at column $502.5$.
3. Map marker rows to $\log_{10} P_e$ by piecewise-linear interpolation between
   adjacent decade gridlines, which absorbs the scanner's vertical drift.
4. Locate markers by scanning a $23 \times 23$ window along each series
   abscissa and scoring $\min(\text{rows hit}, \text{columns hit})$, which
   separates a marker glyph from a line crossing it, then confirm every
   retained marker visually at 4–6$\times$ zoom.

Precision of the pixel-read ordinates: $\pm 0.1$ in $\log_{10}$
(about $\pm 25\%$ in linear value). This figure is measured, not assumed. The
`figure_crosscheck_log10` column carries the pixel read for each row whose
ordinate also appears in a printed table; across the nine such rows eight agree
with the printed numeral within $0.12$ decades, and the ninth is the
contradiction recorded below.

Abscissas: rows whose abscissa appears in a printed table carry the printed
numeral. The remaining abscissas — 1.55, 6.02, 6.48, 6.99, 7.57 dB — are the
grid values $10\log_{10}(2/N_0)$ for $N_0 \in \{1.4, 0.5, 0.45, 0.4, 0.35\}$,
extending the grid the tabulated abscissas establish. Marker columns measured
from the scan agree with those grid values within 0.05 dB, the largest
deviation being 0.045 dB at 7.57 dB.

Sample sizes: the source states no per-point block count for the tabulated
series. It states 50 000 coded blocks for the order-3 and order-4 complexity
comparison at the highest simulated SNR, and warns in the same paragraph that
"the number of simulated blocks is far too small to obtain reliable information
at such an error performance" (p. 69). No confidence interval is published for
any point in this dataset.

## Series inventory

| `decoder` | Points | Ordinate source |
| --- | --- | --- |
| `OSD_order0` | 11 | Figure 4.14 |
| `OSD_order1` | 7 | Figure 4.14 |
| `OSD_order2` | 7 | 1 from Figure 4.14, 6 from Table 4.7 |
| `OSD_order3` | 7 | 1 from Figure 4.14, 6 from Table 4.8 |
| `OSD_order4` | 7 | 1 from Figure 4.14, 6 from Table 4.9 |

Every row carries `metric = BER`, `code_params = eBCH(128,64,22)`, the source
legend text in `legend_full`, and the exact page-level locator in
`source_locator`. The `value` column is $10^{\texttt{value\_log10}}$ and exists
so that the existing
[`compare_results.py`](/dev/reference_data/scripts/compare_results.py) helper
can consume the file; `value_log10` holds the published or measured quantity at
its own precision.

## Yue2022 cross-check

`@/citation/Yue2022` — Yue, Miloslavskaya, Shirvanimoghaddam, Vucetic, Li,
"Efficient Decoders for Short Block Length Codes in 6G URLLC",
arXiv:2206.09572v2 (22 Dec 2022), retrieved 2026-08-24, SHA-256
`869b8cc048b5d210f00db6ea8f69c2b368f5245e2bacc2b5aa0c14e91d5cf8ed`. Its
reference [6] is `Fossorier1995`.

The comparable object is Figure 1, left panel, "$(128,64)$ codes": the eBCH
simulation curve, its ML bound, and OSD markers, plotted as block error rate
against $E_s/N_0$ [dB] over $1.0$–$3.5$ dB. Read from that panel by eye, not by
axis calibration, and therefore accurate only to roughly a factor of $1.3$, the
eBCH OSD markers fall near $1.1\times10^{-1}$, $4.3\times10^{-2}$,
$9\times10^{-3}$, $1.3\times10^{-3}$, $1.4\times10^{-4}$ and $1.0\times10^{-5}$
at $1.0, 1.5, 2.0, 2.5, 3.0, 3.5$ dB. These reads support the qualitative
cross-check only; no Yue2022 value enters the CSV.

Material differences, recorded and not reconciled:

- **Metric.** Yue2022 plots block error rate. Fossorier's series are bit error
  rates. The two are not interconvertible without a bit-errors-per-block-error
  figure that neither source publishes.
- **Modulation and SNR bookkeeping.** Yue2022 uses "AWGN channel with QPSK
  modulation", energy per transmitted symbol $E_s = 1$, complex noise of
  variance $N_0$, and defines the abscissa as $10\log_{10}(E_s/N_0)$ (§III).
  Under that convention $E_b/N_0 = E_s/(2RN_0)$, which for $R = 1/2$ makes the
  plotted abscissa numerically equal to $E_b/N_0$ and therefore directly
  comparable with Fossorier's. The epic's reproduction target is BI-AWGN with
  BPSK; Yue2022's modulation differs from both that target and Fossorier's.
- **OSD order.** Yue2022 runs "the original OSD algorithm with the optimal
  decoding order $m = \lceil d_H/4 - 1 \rceil$", which is $5$ for
  $d_H = 22$ — not the order 2 of the reproduction target. Its eBCH curve is
  therefore a near-ML curve, comparable with Fossorier's order-4 series rather
  than with the order-2 target.
- **Construction.** Yue2022 pins no primitive polynomial either; it names the
  code only as the $(128,64)$ eBCH code with the remark that such codes have
  the highest minimum Hamming distances among state-of-the-art linear codes.
- **Sample size.** Yue2022 states its 1000-decoding-error stopping rule for
  Figures 3–4 only, and states no sample size for Figure 1.

Consistency observed at the shared abscissa: Fossorier reports optimum
performance at BER $10^{-6}$ for a 7.0 dB coding gain, i.e. $E_b/N_0 \approx
3.53$ dB (p. 64), and Yue2022's eBCH ML bound reaches BLER $\approx 10^{-5}$
near $3.4$–$3.5$ dB on its abscissa. Assuming — an assumption of this document,
not a published figure — between 4 and 12 information-bit errors per block
error, the former maps onto $4\times10^{-6}$–$1.2\times10^{-5}$, which brackets
the latter. This is an agreement of near-ML references only; it says nothing
about the order-2 target series and no value in the CSV is adjusted because of
it.

## Order-1 control status

A published order-1 series exists in the digitized figure: the `+: Order 1`
markers of Figure 4.14, stored as `OSD_order1` at seven abscissas from 1.55 to
5.23 dB. It is figure-digitized at $\pm 0.1$ decades; no table in the source
tabulates order 1 for this code, so it carries no printed numerals.

Constraints on how that series may be used:

- It is a bit error rate on the same axes as the order-2 series, and it is
  published, not inferred.
- It is not a BLER control, and it is not an authority for any campaign
  quantity that is not a BER at these abscissas under this decoder definition.
- A campaign order-1 control run under a different metric, a different tie
  policy, a different construction, or a different abscissa convention is a
  campaign result. It may be compared with `OSD_order1`, but any claim it makes
  about published order-1 behaviour is unsupported unless the campaign
  reproduces this metric and these conventions.

## Gaps and contradictions

- **Unverified article-level pin.** The exact page, figure number, series
  legends, and figure-level construction statements of `Fossorier1995` are
  unverified because the article is closed access. This document pins the
  1994 dissertation instead and asserts nothing about the article's internal
  numbering. Whether the article reproduces Figure 4.14 and Tables 4.7–4.9
  unchanged is unknown.
- **Registry gap.** The 1994 dissertation supplies every number in the CSV and
  resolves in no citation-registry key. `@/inv/external-claims-cited` is
  therefore not satisfied by this artifact alone; a registry entry for the
  dissertation is required, and adding it is outside this artifact's scope.
- **Metric mismatch with the reproduction target.** The target is BLER; the
  published series are BER. No conversion is applied.
- **Table-versus-figure contradiction at 4.56 dB, order 2.** Table 4.7 prints
  $P_e = 10^{-5.7}$; the corresponding marker in Figure 4.14 reads
  $10^{-5.49}$, a gap of $0.21$ decades against a read precision of $0.1$. The
  four other order-2 abscissas carrying a figure cross-read agree within $0.11$
  decades. The CSV keeps the printed table value in `value_log10` and the pixel
  read in `figure_crosscheck_log10`; neither is corrected against the other, and
  which one the source intends is unresolved.
- **Unresolved figure/text equation reference.** The theoretical curves in
  Figure 4.14 are labelled "Eq. 4.46", which defines the codeword error
  probability $P_s(i)$, while §4.3.1 (pp. 51–52) says the simulated results are
  compared against Equation 4.47, the bit error bound $P_b(i)$. The plotted
  simulation markers are BER by the p. 63–64 statements; the theoretical curves
  are not part of this dataset and their label is left as the source has it.
- **Unpublished uncertainty.** No sample count or confidence interval is
  published for any digitized point, and the source itself warns that its block
  counts are too small at the lowest error rates. Rows marked
  `value_kind = union_bound` are bound values, not measurements, and must not
  be treated as simulation evidence.
