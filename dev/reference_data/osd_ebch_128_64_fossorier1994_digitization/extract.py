#!/usr/bin/env python3
"""Reproduce the Figure 4.14 pixel-to-data calibration receipt.

The marker centers in ``PIXEL_READS`` are manual measurements.  This script
pins their source, renders the same page at the recorded resolution, checks
that each measurement window contains figure ink, applies the committed axis
calibration, and emits the complete machine-readable receipt as JSON.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import BinaryIO, NamedTuple


EXPECTED_SHA256 = "9e867a44f2a7d54047396383db5e6bc4fb0cb083c83faea2a9c8649a6f3e5eeb"
EXPECTED_BYTES = 4_640_976
DPI = 300

# Printed dissertation page 60 is PDF page 80: the scan has twenty title and
# front-matter pages before the dissertation's Arabic page numbering begins.
PDF_PAGE = 80
PRINTED_PAGE = 60
EXPECTED_RENDER_SIZE = (2583, 3324)
MARKER_WINDOW_SIZE = 23
DARK_PIXEL_THRESHOLD = 128

X_ANCHORS = tuple(
    {"x_px": 502.5 + 178.0 * (eb_n0_db - 1), "eb_n0_db": float(eb_n0_db)}
    for eb_n0_db in range(1, 12)
)
Y_ANCHORS = tuple(
    {"y_px": y_px, "log10_ber": float(-decade)}
    for decade, y_px in enumerate(
        (759, 898, 1039, 1180, 1320, 1460, 1600, 1742, 1883, 2024, 2165)
    )
)


class PixelRead(NamedTuple):
    decoder: str
    csv_eb_n0_db: float
    csv_field: str
    x_px: int
    y_px: int
    read_basis: str = "marker_center"


# Coordinates use the top-left of the rendered PDF page as (0, 0).  Primary
# reads map to ``value_log10`` in the CSV; cross-check reads map to
# ``figure_crosscheck_log10`` on table-derived CSV rows.
PIXEL_READS = (
    PixelRead("OSD_order0", 1.55, "value_log10", 600, 872),
    PixelRead("OSD_order0", 2.22, "value_log10", 720, 890),
    PixelRead("OSD_order0", 3.01, "value_log10", 860, 923),
    PixelRead("OSD_order0", 3.47, "value_log10", 942, 949),
    PixelRead("OSD_order0", 3.98, "value_log10", 1033, 984),
    PixelRead("OSD_order0", 4.56, "value_log10", 1136, 1023),
    PixelRead("OSD_order0", 5.23, "value_log10", 1255, 1086),
    PixelRead("OSD_order0", 6.02, "value_log10", 1396, 1170),
    PixelRead("OSD_order0", 6.48, "value_log10", 1478, 1228),
    PixelRead("OSD_order0", 6.99, "value_log10", 1569, 1300),
    PixelRead("OSD_order0", 7.57, "value_log10", 1672, 1389),
    PixelRead("OSD_order1", 1.55, "value_log10", 600, 926),
    PixelRead("OSD_order1", 2.22, "value_log10", 720, 973),
    PixelRead("OSD_order1", 3.01, "value_log10", 860, 1040),
    PixelRead("OSD_order1", 3.47, "value_log10", 942, 1097),
    PixelRead("OSD_order1", 3.98, "value_log10", 1033, 1164),
    PixelRead("OSD_order1", 4.56, "value_log10", 1136, 1258),
    PixelRead("OSD_order1", 5.23, "value_log10", 1255, 1377),
    PixelRead("OSD_order2", 1.55, "value_log10", 600, 992),
    PixelRead("OSD_order2", 2.22, "figure_crosscheck_log10", 720, 1070),
    PixelRead("OSD_order2", 3.01, "figure_crosscheck_log10", 860, 1186),
    PixelRead("OSD_order2", 3.47, "figure_crosscheck_log10", 942, 1271),
    PixelRead("OSD_order2", 3.98, "figure_crosscheck_log10", 1033, 1391),
    PixelRead("OSD_order2", 4.56, "figure_crosscheck_log10", 1136, 1529),
    PixelRead("OSD_order3", 1.55, "value_log10", 600, 1045),
    PixelRead("OSD_order3", 2.22, "figure_crosscheck_log10", 720, 1150),
    PixelRead("OSD_order3", 3.01, "figure_crosscheck_log10", 860, 1337),
    PixelRead("OSD_order3", 3.47, "figure_crosscheck_log10", 942, 1452),
    PixelRead(
        "OSD_order4",
        1.55,
        "value_log10",
        600,
        1062,
        "curve_at_abscissa_order4_marker_glyph_illegible",
    ),
    PixelRead("OSD_order4", 2.22, "figure_crosscheck_log10", 720, 1188),
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_pnm_token(source: BinaryIO) -> bytes:
    token = bytearray()
    while True:
        byte = source.read(1)
        if not byte:
            raise ValueError("unexpected end of PGM header")
        if byte == b"#":
            source.readline()
        elif not byte.isspace():
            token.extend(byte)
            break
    while True:
        byte = source.read(1)
        if not byte or byte.isspace():
            return bytes(token)
        token.extend(byte)


def read_pgm(path: Path) -> tuple[int, int, bytes]:
    with path.open("rb") as image:
        magic = read_pnm_token(image)
        width = int(read_pnm_token(image))
        height = int(read_pnm_token(image))
        maximum = int(read_pnm_token(image))
        pixels = image.read()
    if magic != b"P5" or maximum != 255:
        raise ValueError(f"unsupported PGM encoding: magic={magic!r}, max={maximum}")
    if len(pixels) != width * height:
        raise ValueError(
            f"truncated PGM: expected {width * height} pixels, got {len(pixels)}"
        )
    return width, height, pixels


def render_page(pdf: Path, output_prefix: Path) -> Path:
    renderer = shutil.which("pdftoppm")
    if renderer is None:
        raise RuntimeError("pdftoppm is required but was not found on PATH")
    command = [
        renderer,
        "-f",
        str(PDF_PAGE),
        "-l",
        str(PDF_PAGE),
        "-r",
        str(DPI),
        "-singlefile",
        "-gray",
        str(pdf),
        str(output_prefix),
    ]
    subprocess.run(command, check=True, capture_output=True, text=True)
    return output_prefix.with_suffix(".pgm")


def eb_n0_from_x(x_px: int) -> float:
    return 1.0 + (x_px - 502.5) / 178.0


def log10_ber_from_y(y_px: int) -> float:
    for upper, lower in zip(Y_ANCHORS, Y_ANCHORS[1:]):
        if upper["y_px"] <= y_px <= lower["y_px"]:
            fraction = (y_px - upper["y_px"]) / (lower["y_px"] - upper["y_px"])
            return upper["log10_ber"] - fraction
    raise ValueError(f"pixel y={y_px} is outside the calibrated BER axis")


def dark_pixel_count(
    pixels: bytes, width: int, height: int, x_px: int, y_px: int
) -> int:
    radius = MARKER_WINDOW_SIZE // 2
    if not (radius <= x_px < width - radius and radius <= y_px < height - radius):
        raise ValueError(f"marker window centered at ({x_px}, {y_px}) leaves the image")
    return sum(
        pixels[y * width + x] < DARK_PIXEL_THRESHOLD
        for y in range(y_px - radius, y_px + radius + 1)
        for x in range(x_px - radius, x_px + radius + 1)
    )


def make_receipt(pdf: Path) -> dict[str, object]:
    actual_sha256 = sha256(pdf)
    if actual_sha256 != EXPECTED_SHA256:
        raise ValueError(
            "source PDF SHA-256 mismatch: "
            f"expected {EXPECTED_SHA256}, got {actual_sha256}"
        )
    actual_bytes = pdf.stat().st_size
    if actual_bytes != EXPECTED_BYTES:
        raise ValueError(
            f"source PDF size mismatch: expected {EXPECTED_BYTES}, got {actual_bytes}"
        )

    with tempfile.TemporaryDirectory(prefix="fossorier1994-digitization-") as temporary:
        rendered = render_page(pdf, Path(temporary) / "page-80")
        width, height, pixels = read_pgm(rendered)
    if (width, height) != EXPECTED_RENDER_SIZE:
        raise ValueError(
            "render size mismatch at 300 DPI: "
            f"expected {EXPECTED_RENDER_SIZE}, got {(width, height)}"
        )

    points = []
    for read in PIXEL_READS:
        ink_count = dark_pixel_count(pixels, width, height, read.x_px, read.y_px)
        if ink_count == 0:
            raise ValueError(
                f"no dark pixels in marker window for {read.decoder} "
                f"at Eb/N0={read.csv_eb_n0_db}"
            )
        points.append(
            {
                "id": f"{read.decoder}@{read.csv_eb_n0_db:.2f}:{read.csv_field}",
                "decoder": read.decoder,
                "csv_eb_n0_db": read.csv_eb_n0_db,
                "csv_field": read.csv_field,
                "pixel": {"x": read.x_px, "y": read.y_px},
                "read_basis": read.read_basis,
                "derived_eb_n0_db": round(eb_n0_from_x(read.x_px), 4),
                "derived_value_log10": round(log10_ber_from_y(read.y_px), 4),
                "dark_pixel_count_23x23": ink_count,
            }
        )

    return {
        "schema_version": 1,
        "receipt_date": "2026-08-25",
        "source": {
            "work_citekey": "Fossorier1994",
            "scholarspace_item": "46246020-a80c-43a9-912e-68f3774c3f3e",
            "scholarspace_bitstream": "3a655537-c031-436f-ac2d-61d2516aa046",
            "filename": "uhm_phd_9519442_r.pdf",
            "sha256": actual_sha256,
            "bytes": actual_bytes,
        },
        "render": {
            "pdf_page_one_based": PDF_PAGE,
            "printed_dissertation_page": PRINTED_PAGE,
            "dpi": DPI,
            "pixel_width": width,
            "pixel_height": height,
            "coordinate_origin": "top_left",
            "command": (
                "pdftoppm -f 80 -l 80 -r 300 -singlefile -gray "
                "<pdf> <temporary-output-prefix>"
            ),
        },
        "calibration": {
            "eb_n0_db": {
                "method": "linear_between_vertical_snr_gridlines",
                "anchors": X_ANCHORS,
                "pixels_per_db": 178.0,
            },
            "log10_ber": {
                "method": "piecewise_linear_between_horizontal_decade_gridlines",
                "anchors": Y_ANCHORS,
            },
            "digitization_uncertainty_log10": 0.1,
            "marker_window_px": [MARKER_WINDOW_SIZE, MARKER_WINDOW_SIZE],
            "dark_pixel_threshold": DARK_PIXEL_THRESHOLD,
        },
        "points": points,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pdf", type=Path, help="path to uhm_phd_9519442_r.pdf")
    args = parser.parse_args()
    try:
        receipt = make_receipt(args.pdf)
    except (OSError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(f"extract.py: {error}", file=sys.stderr)
        return 1
    json.dump(receipt, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
