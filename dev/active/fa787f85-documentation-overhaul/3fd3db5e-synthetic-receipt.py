#!/usr/bin/env python3
"""Prints the receipt the BCH renderer test renders for its synthetic run.

Usage: 3fd3db5e-synthetic-receipt.py <test_render_receipt.py>

Builds the test module's default `Fixture` in a temporary directory, renders
it, and prints the receipt followed by the verdict flag. `<tmp>` replaces the
temporary directory, the only value that differs between runs.
"""
from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location("test_render_receipt", sys.argv[1])
test = importlib.util.module_from_spec(spec)
spec.loader.exec_module(test)

with tempfile.TemporaryDirectory() as tmp:
    text, ok = test.Fixture(Path(tmp)).render()
    for spelling in (str(Path(tmp).resolve()), tmp):
        text = text.replace(spelling, "<tmp>")
print(text)
print(f"ok: {ok}")
