# Fossorier 1994 and Yue2022 digitization receipts

This directory makes the manual axis-calibrated reads behind
[`../osd_ebch_128_64_fossorier1994.csv`](../osd_ebch_128_64_fossorier1994.csv)
auditable. It covers every primary Figure 4.14 ordinate and every Figure 4.14
cross-check attached to a printed-table row. The separate
[`yue2022_fig1_receipt.json`](yue2022_fig1_receipt.json) pins the six manual
by-eye reads used for the qualitative Yue2022 cross-check.

## Reproduce

Download the open-access dissertation and run:

```console
curl -fL -o /tmp/uhm_phd_9519442_r.pdf \
  https://scholarspace.manoa.hawaii.edu/bitstreams/3a655537-c031-436f-ac2d-61d2516aa046/download
python3 extract.py /tmp/uhm_phd_9519442_r.pdf > /tmp/calibration.json
diff -u calibration.json /tmp/calibration.json

curl -fL -o /tmp/2206.09572v2.pdf https://arxiv.org/pdf/2206.09572v2
python3 extract.py --receipt yue2022 /tmp/2206.09572v2.pdf \
  > /tmp/yue2022_fig1_receipt.json
diff -u yue2022_fig1_receipt.json /tmp/yue2022_fig1_receipt.json
```

The script requires `pdftoppm`. It aborts unless the PDF SHA-256 is
`9e867a44f2a7d54047396383db5e6bc4fb0cb083c83faea2a9c8649a6f3e5eeb`
and the file size is 4,640,976 bytes. It renders PDF page 80, which is printed
dissertation page 60, at 300 DPI. The JSON output records the render geometry,
axis anchors, every manual pixel coordinate, the calibrated values, and a dark
pixel count for each 23×23 inspection window.

The Yue2022 mode aborts unless the arXiv PDF SHA-256 is
`869b8cc048b5d210f00db6ea8f69c2b368f5245e2bacc2b5aa0c14e91d5cf8ed`
and its size is 11,239,658 bytes. Its receipt records PDF page 3, Figure 1's
left panel, the six BLER reads, their rough factor-1.3 by-eye precision, and the
near-ML comparison object. It does not promote those reads into the CSV.

## What the receipt proves

The receipt binds the coordinates to the pinned source bytes and the stated
axis calibration. Re-running it verifies that each coordinate falls on figure
ink in the pinned 300-DPI render and deterministically recomputes its
$E_b/N_0$ and $\log_{10}(\mathrm{BER})$ values. The CSV reconciliation is
within its declared conservative ±0.1-decade figure-read bound; the exact
per-point deltas are documented in the parent provenance document. It also
recalculates every ordinate over the full integer Cartesian marker-center
neighborhood from −2 to +2 pixels in both axes. The maximum observed one-sided
change is 0.0144 decade (0.0288 decade across the full minimum-to-maximum
range), within the declared conservative ±0.1-decade bound.

## Limits

The marker centers are human measurements, not outputs of an image classifier.
Line thickness, scan blur, and overlap between markers, plotted curves, and
dotted gridlines make the center ambiguous; figure reads therefore carry a
declared conservative ±0.1 bound in $\log_{10}(\mathrm{BER})$, not a measured
statistical interval. The Order-4 marker glyph is
illegible in the legend and at its first plotted point, so that point is
explicitly recorded as a curve read at the known abscissa, without identifying
or guessing the glyph. The script does not reproduce the printed numerals in
Tables 4.7–4.9; those are exact transcriptions, while the receipt covers their
nine independent figure cross-checks. Yue2022's reads remain manual by-eye
measurements; their receipt makes the source, precision, and comparison inputs
auditable but does not turn them into axis-calibrated measurements.
