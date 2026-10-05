#!/usr/bin/env python3
"""Write the assembly of the check prototypes (jit:bfc2ceab).

Runs the shared `regen-asm.sh` from the `check-prototype` crate under Rust
1.95 for every prototype function `review.py` names, and writes
`check-prototype.asm.txt` beside this script. The listing's banner carries the
commit and time of the run; `make-instruction-counts.py` reads the
instruction text alone.

Usage: regen-prototype-listing.py, with no arguments
"""

import os
import subprocess

import review
from locate import HERE, ROOT, repository_files

REGENERATOR = b"#!/usr/bin/env bash\n# Regenerates per-symbol asm artefacts for SIMD review.\n"
LISTING = "check-prototype.asm.txt"
TOOLCHAIN = "1.95.0"


def prototype_functions():
    """Every prototype function the review names, in review order, once."""
    names = []
    for interface in review.INTERFACES:
        names += interface["prototype"]
        for _, current, checked in interface.get("prototype_extra", []):
            names += [current, checked]
    return list(dict.fromkeys(names))


def main():
    regenerate = ROOT / repository_files.document(ROOT, "regen-asm.sh", REGENERATOR)
    symbols = [f"{review.PROTOTYPE_CRATE}::{name}" for name in prototype_functions()]
    subprocess.run(
        [str(regenerate), review.PROTOTYPE, symbols[0], f"../{LISTING}", *symbols[1:]],
        cwd=HERE / "check-prototype",
        check=True,
        env=os.environ | {"RUSTUP_TOOLCHAIN": TOOLCHAIN},
    )
    print(f"{(HERE / LISTING).relative_to(ROOT)}: {len(symbols)} symbols")


if __name__ == "__main__":
    main()
