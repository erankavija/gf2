"""Rewrite the gf2-core source tree into the intermediate hoist shape, probe S4.

Probe material for JIT issue 7d7c647c, not production code. It applies task U2
of the design's breakdown: the three uniform defaults become module-level
constants in their own selector modules and the trait defaults name them
instead of restating a literal. The trait still declares all four constants and
every read site still reads them, so the probe answers one question — whether a
trait default whose body names a crate-private constant changes what the pinned
Charon and Aeneas pair emit.

Usage, from the repository root:

    python3 dev/active/7d7c647c/probes/make-hoist-tree.py apply
    python3 dev/active/7d7c647c/probes/make-hoist-tree.py revert
"""
import shutil, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[4]
CORE = ROOT / "crates/gf2-core/src"
T = CORE / "field/traits.rs"
W = CORE / "field/winograd.rs"
TR = CORE / "field/triangular.rs"
P = CORE / "field/ple.rs"
FILES = [T, W, TR, P]

def apply():
    for f in FILES:
        b = f.with_suffix(f.suffix + ".probe-orig")
        if not b.exists():
            shutil.copy2(f, b)
    for f, decl in [
        (W, "\npub(crate) const WINOGRAD_MIN_DIM_DEFAULT: usize = 128;\n"),
        (TR, "\npub(crate) const TRI_BASE_MAX_DIM_DEFAULT: usize = 8;\n"),
        (P, "\npub(crate) const PLE_SCALAR_BASE_MAX_COLS_DEFAULT: usize = 1;\n"),
    ]:
        t = f.read_text().split("\n")
        i = 0
        while i < len(t) and t[i].startswith("//!"):
            i += 1
        t.insert(i, decl)
        f.write_text("\n".join(t))
    t = T.read_text()
    for old, new in [
        ("const WINOGRAD_THRESHOLD: usize = 128;",
         "const WINOGRAD_THRESHOLD: usize = crate::field::winograd::WINOGRAD_MIN_DIM_DEFAULT;"),
        ("const TRI_BASE_THRESHOLD: usize = 8;",
         "const TRI_BASE_THRESHOLD: usize = crate::field::triangular::TRI_BASE_MAX_DIM_DEFAULT;"),
        ("const PLE_BASE_COLS: usize = 1;",
         "const PLE_BASE_COLS: usize = crate::field::ple::PLE_SCALAR_BASE_MAX_COLS_DEFAULT;"),
    ]:
        assert old in t, old
        t = t.replace(old, new)
    T.write_text(t)
    print("applied")

def revert():
    for f in FILES:
        b = f.with_suffix(f.suffix + ".probe-orig")
        if b.exists():
            shutil.move(str(b), str(f))
    print("reverted")

if len(sys.argv) != 2 or sys.argv[1] not in {"apply", "revert"}:
    raise SystemExit(__doc__)
(apply if sys.argv[1] == "apply" else revert)()
