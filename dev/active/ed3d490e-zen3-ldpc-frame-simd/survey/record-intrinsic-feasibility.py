#!/usr/bin/env python3
"""Record the MSRV intrinsic feasibility of the inter-frame lane forms (jit:ed3d490e).

Runs the canonical feasibility recorder over this directory's probe crate, so
the record has that recorder's schema and observes only what its commands
report. The recorder is named on the command line.

Usage (from the worktree root):
  record-intrinsic-feasibility.py RECORDER OUT_DIR
"""

import importlib.util
import pathlib
import sys


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    spec = importlib.util.spec_from_file_location("feasibility_recorder", sys.argv[1])
    recorder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(recorder)
    here = pathlib.Path(__file__).resolve().parent
    recorder.PROBE = here.relative_to(recorder.ROOT) / "intrinsics-probe"
    recorder.TARGET = pathlib.Path("target/ldpc-frame-lanes-intrinsics-probe")
    sys.argv = [sys.argv[1], sys.argv[2]]
    recorder.main()


if __name__ == "__main__":
    main()
