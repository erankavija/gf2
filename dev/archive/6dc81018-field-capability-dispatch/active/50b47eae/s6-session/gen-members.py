#!/usr/bin/env python3
"""Emit the v4 C2 enumeration for one arm.

usage: gen-members.py <ordinary-.text-vaddr-hex> <out-members.tsv>
Prints phi to stdout. The TSV has columns member_j, E.
"""
import sys

P = int(sys.argv[1], 16)
phi = 1 if (P % 64) >= 32 else 0
h0 = [e for e in range(128) if bin((e + phi) % 128).count("1") % 2 == 0]
h1 = [e for e in range(128) if bin((e + phi) % 128).count("1") % 2 == 1]
assert len(h0) == 64 and len(h1) == 64
E = {}
for i in range(64):
    E[2 * i] = h0[i]
    E[2 * i + 1] = h1[i]
assert sorted(E.values()) == list(range(128))
with open(sys.argv[2], "w") as fh:
    fh.write("member_j\tE\n")
    for j in range(128):
        fh.write(f"{j}\t{E[j]}\n")
print(phi)
