#!/usr/bin/env python3
"""Regenerates kernel assembly listings from the symbols each one records.

Each listing is rewritten by `regen-asm.sh` with the symbol selectors of its
own `; symbol:` dividers and the `RUSTFLAGS` of its banner, so a listing is the
record of how to regenerate it. A listing without a symbol divider is left
as it is. The toolchain is the one the environment selects. `regen-asm.sh`
merges cargo's status output into the listing; a listing holding a status line
other than one whole `Compiling` or `Finished` line is regenerated, so
listing bodies differ only where the emitted assembly does.

Usage: regen-asm-listings.py [LISTING...]

Without arguments, every `*.asm.txt` of the `gf2-kernels-simd` package that
`regen-asm.sh` wrote is regenerated.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

import asm_listing
import repository_files

PACKAGE = "gf2-kernels-simd"
BANNER = "; gf2-core PPC-spiral asm artefact\n"
# Cargo indents its status lines with spaces; assembly lines start with a tab,
# a label or a symbol name.
STATUS = re.compile(r"^ .*$", re.M)
WHOLE_STATUS = re.compile(r" +(Compiling \S+ v\S+ \(.*\)|Finished .* target\(s\) in [0-9.]+s)")
REGENERATOR = b"#!/usr/bin/env bash\n# Regenerates per-symbol asm artefacts for SIMD review.\n"


def main() -> int:
    root = repository_files.repository_root(Path.cwd())
    listings = sys.argv[1:] or sorted(
        path
        for path in repository_files.tracked_files(root, "*.asm.txt")
        if path.startswith(repository_files.package_directory(root, PACKAGE) + "/")
        and (root / path).read_text().startswith(BANNER)
    )
    regenerate = root / repository_files.document(root, "regen-asm.sh", REGENERATOR)
    for listing in listings:
        artefact = (root / listing).read_text()
        selectors = asm_listing.selectors(artefact)
        if not selectors:
            print(f"{listing}: no symbol divider; left as it is", file=sys.stderr)
            continue
        environment = os.environ | {"EXTRA_RUSTFLAGS": asm_listing.rustflags(artefact)}
        while True:
            subprocess.run(
                [str(regenerate), PACKAGE, selectors[0], listing, *selectors[1:]],
                cwd=root,
                check=True,
                env=environment,
            )
            status = STATUS.findall((root / listing).read_text())
            if all(WHOLE_STATUS.fullmatch(line) for line in status):
                break
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
