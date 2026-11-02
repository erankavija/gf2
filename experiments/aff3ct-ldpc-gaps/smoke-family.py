#!/usr/bin/env python3
"""Run a materialized family plan through the shared staged untimed lifecycle."""

from pathlib import Path
import runpy
import sys

if __name__ == "__main__":
    sys.argv.append("--untimed")
    runpy.run_path(str(Path(__file__).with_name("run-family-campaign.py")), run_name="__main__")
