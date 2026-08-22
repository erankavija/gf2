#!/usr/bin/env python3
"""Rewrite the gf2-core source tree into the post-seam shape, for probe S2.

This is probe material for JIT issue 7d7c647c, not production code. It applies
the mechanical part of the planned cutover so Charon and Aeneas can be run
against the resulting tree and the extracted surface compared with the baseline
(probe S1):

* the four tuning constants leave the `FiniteField` trait declaration
  (`crates/gf2-core/src/field/traits.rs`), together with their rustdoc;
* the `Fp<P>` `PLE_PANEL_COLS` override leaves the impl
  (`crates/gf2-core/src/gfp/mod.rs`);
* every non-test read site substitutes a module-level constant of the same
  value for the `F::`-qualified associated-constant read, which is the shape
  the seam's resolved read takes at the same position;
* the `PlePanelLane` tag and its `#[doc(hidden)]` hook join `FiniteField`, with
  the `Fp<P>` override carrying `#[cfg(not(verify_lean))]` — the shape the
  sixteen existing accelerator hooks already take.

`charon cargo` builds the library target only, so `#[cfg(test)]` readers need no
rewrite; the assertions in those modules are listed in the design's cutover
tasks instead.

Usage, from the repository root:

    python3 dev/active/7d7c647c/probes/make-seam-tree.py apply
    python3 dev/active/7d7c647c/probes/make-seam-tree.py revert

`revert` restores the files from the `.probe-orig` copies `apply` writes.
"""

from __future__ import annotations

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
CORE = ROOT / "crates" / "gf2-core" / "src"

TRAITS = CORE / "field" / "traits.rs"
GFP = CORE / "gfp" / "mod.rs"
WINOGRAD = CORE / "field" / "winograd.rs"
TRIANGULAR = CORE / "field" / "triangular.rs"
PLE = CORE / "field" / "ple.rs"
FIELD_MOD = CORE / "field" / "mod.rs"

TOUCHED = [TRAITS, GFP, WINOGRAD, TRIANGULAR, PLE, FIELD_MOD]

CONSTS = ["WINOGRAD_THRESHOLD", "TRI_BASE_THRESHOLD", "PLE_BASE_COLS", "PLE_PANEL_COLS"]

# Module-level stand-ins the read sites bind to after the constants leave the
# trait. Values are today's defaults, so the probe changes the extraction
# surface and nothing else.
STANDIN = {
    WINOGRAD: "pub(crate) const WINOGRAD_THRESHOLD: usize = 128;\n",
    TRIANGULAR: "pub(crate) const TRI_BASE_THRESHOLD: usize = 8;\n",
    PLE: (
        "pub(crate) const PLE_BASE_COLS: usize = 1;\n"
        "pub(crate) const PLE_PANEL_COLS: usize = 1;\n"
    ),
}


def strip_item_with_doc(text: str, pattern: str) -> str:
    """Delete the item `pattern` matches, plus the rustdoc block above it."""
    m = re.search(pattern, text, re.MULTILINE)
    if m is None:
        raise SystemExit(f"pattern not found: {pattern}")
    start, end = m.span()
    lines_before = text[:start].split("\n")
    cut = len(lines_before) - 1
    while cut > 0 and lines_before[cut - 1].lstrip().startswith("///"):
        cut -= 1
    new_start = len("\n".join(lines_before[:cut])) + (1 if cut else 0)
    return text[:new_start] + text[end:].lstrip("\n")


def apply() -> None:
    for path in TOUCHED:
        backup = path.with_suffix(path.suffix + ".probe-orig")
        if not backup.exists():
            shutil.copy2(path, backup)

    text = TRAITS.read_text()
    for name in CONSTS:
        text = strip_item_with_doc(text, rf"^    const {name}: usize = [^;]*;\n")
    TRAITS.write_text(text)

    text = GFP.read_text()
    text = strip_item_with_doc(
        text, r"^    const PLE_PANEL_COLS: usize = \{\n(?:.*\n)*?    \};\n"
    )
    GFP.write_text(text)

    for path, standin in STANDIN.items():
        text = path.read_text()
        for name in CONSTS:
            text = text.replace(f"F::{name}", name)
            text = text.replace(f"Self::{name}", name)
        # Insert the stand-ins after the module's leading `//!` header.
        lines = text.split("\n")
        i = 0
        while i < len(lines) and (
            lines[i].startswith("//!") or lines[i].strip() == "" and i == 0
        ):
            i += 1
        lines.insert(i, "\n" + standin)
        path.write_text("\n".join(lines))

    _add_lane_hook()
    print("applied; revert with: make-seam-tree.py revert")


LANE_TAG = """
/// Lane class of the panel-base PLE kernel a carrier registers. The width the
/// driver uses for each class comes from the tuning profile, so no numeric
/// tuning value reaches the trait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlePanelLane {
    /// Canonical-byte AVX2 panel kernel.
    Byte,
    /// u16-lane AVX2 panel kernel.
    U16,
}
"""

LANE_HOOK = """
    /// Lane class of this carrier's panel-base PLE kernel, or `None` when it
    /// registers none. Replaces the boolean availability probe.
    #[doc(hidden)]
    #[inline]
    fn simd_ple_panel_lane() -> Option<PlePanelLane> {
        None
    }
"""

FP_OVERRIDE = """
    #[cfg(not(verify_lean))]
    #[inline]
    fn simd_ple_panel_lane() -> Option<crate::field::PlePanelLane> {
        if !simd_ops::fp_ple_panel_base_available::<P>() {
            None
        } else if P <= 251 {
            Some(crate::field::PlePanelLane::Byte)
        } else {
            Some(crate::field::PlePanelLane::U16)
        }
    }
"""


def _add_lane_hook() -> None:
    text = TRAITS.read_text()
    anchor = "pub trait FiniteField"
    idx = text.index(anchor)
    line_start = text.rindex("\n", 0, idx) + 1
    text = text[:line_start] + LANE_TAG + "\n" + text[line_start:]
    marker = "    fn has_simd_ple_panel_base() -> bool {"
    hook_at = text.index(marker)
    doc_at = text.rindex("    /// Non-allocating availability probe", 0, hook_at)
    text = text[:doc_at] + LANE_HOOK + "\n" + text[doc_at:]
    TRAITS.write_text(text)

    text = FIELD_MOD.read_text()
    text = text.replace(
        "pub use traits::{ConstField, FiniteField, FiniteFieldExt};",
        "pub use traits::{ConstField, FiniteField, FiniteFieldExt, PlePanelLane};",
    )
    FIELD_MOD.write_text(text)

    text = GFP.read_text()
    marker = "    fn has_simd_ple_panel_base() -> bool {"
    at = text.index(marker)
    doc_at = text.rindex("    /// Non-allocating availability probe", 0, at)
    text = text[:doc_at] + FP_OVERRIDE + "\n" + text[doc_at:]
    GFP.write_text(text)


def revert() -> None:
    for path in TOUCHED:
        backup = path.with_suffix(path.suffix + ".probe-orig")
        if backup.exists():
            shutil.move(str(backup), str(path))
    print("reverted")


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in {"apply", "revert"}:
        raise SystemExit(__doc__)
    (apply if sys.argv[1] == "apply" else revert)()
