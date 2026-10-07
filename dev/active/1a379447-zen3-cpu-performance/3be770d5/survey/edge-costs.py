#!/usr/bin/env python3
"""Derive the per-iteration structural work of both flooding decoders (jit:3be770d5).

Reads recorded MacKay AList files and counts, for one flooding iteration, the
edge visits, adjacency-position searches, reduction inputs, allocations and
syndrome work that each implementation's loop structure implies on that exact
graph. The counts are exact consequences of the source loops cited below and
the graph; they are derived work, not timings.

gf2 (`crates/gf2-coding/src/ldpc/core.rs`, `crates/gf2-coding/src/llr.rs`):
- `check_node_update_normalized_minsum`: for every check c and output position
  p it gathers the d_c - 1 other variable-to-check messages, locating each with
  `find_check_position`, a linear scan of the variable's check list that
  compares idx + 1 entries; then one `boxplus_normalized_minsum_n` call per
  edge, which with the default `simd` feature collects its inputs into a fresh
  `Vec<f32>` before the kernel call.
- `variable_node_update`: `check_to_var_message` scans the check's variable
  list for the variable (idx + 1 comparisons), twice per edge.
- `decode_to_codeword` with early termination: `hard_decode` pushes n bits
  into a new `BitVec`, and `is_valid_codeword` computes the CSR syndrome into
  a second new `BitVec` of m bits.

AFF3CT (`Decoder_LDPC_BP_flooding.hxx` of the pinned v4.7.0 source):
- `_decode_single_ite`: two passes over each check's inputs through the
  precomputed `transpose` edge index (min/second-min/sign in the update rule),
  so 2 E indexed reads and E indexed writes.
- `_initialize_var_to_chk`: variable-major sums, 2 E sequential reads and E
  writes.
- `_compute_post` plus `check_syndrome_soft` on the soft posterior: E reads and
  n writes, then at most E reads for the syndrome (it stops at the first
  unsatisfied check).

Both sides use the ascending adjacency order of the AList; gf2's CSR and CSC
are sorted (`SpBitMatrix::from_coo`, `transpose`).

Usage: edge-costs.py CODE=PATH [CODE=PATH ...] > structural-costs.json
"""

import hashlib
import json
import sys
from collections import Counter


def read_alist(path):
    with open(path, encoding="ascii") as handle:
        tokens = [int(token) for token in handle.read().split()]
    position = 0

    def take(count):
        nonlocal position
        values = tokens[position:position + count]
        position += count
        return values

    n, m = take(2)
    max_column, max_row = take(2)
    column_weights = take(n)
    row_weights = take(m)
    columns = []
    for degree in column_weights:
        entries = take(max_column)
        columns.append(sorted(entry - 1 for entry in entries[:degree]))
    rows = []
    for degree in row_weights:
        entries = take(max_row)
        rows.append(sorted(entry - 1 for entry in entries[:degree]))
    if position != len(tokens):
        raise SystemExit(f"{path}: trailing AList values")
    return n, m, columns, rows


def costs(path):
    n, m, columns, rows = read_alist(path)
    edges = sum(len(row) for row in rows)
    if edges != sum(len(column) for column in columns):
        raise SystemExit(f"{path}: row and column sections disagree")
    # Position of check c in variable v's check list, and of v in c's list.
    check_index = [{check: index for index, check in enumerate(column)} for column in columns]
    variable_index = [{var: index for index, var in enumerate(row)} for row in rows]
    cn_gathered = 0
    cn_search = 0
    for check, row in enumerate(rows):
        degree = len(row)
        scans = sum(check_index[var][check] + 1 for var in row)
        cn_gathered += degree * (degree - 1)
        # Every output position scans for each of the other d_c - 1 inputs.
        cn_search += (degree - 1) * scans
    vn_search = 2 * sum(
        variable_index[check][var] + 1
        for var, column in enumerate(columns)
        for check in column
    )
    check_degrees = Counter(len(row) for row in rows)
    variable_degrees = Counter(len(column) for column in columns)
    return {
        "alist_sha256": hashlib.sha256(open(path, "rb").read()).hexdigest(),
        "n": n,
        "m": m,
        "edges": edges,
        "check_degree_histogram": sorted(check_degrees.items()),
        "variable_degree_histogram": sorted(variable_degrees.items()),
        "gf2_per_iteration": {
            "check_inputs_gathered": cn_gathered,
            "check_position_search_comparisons": cn_search,
            "min_sum_calls": edges,
            "min_sum_input_vec_allocations": edges,
            "variable_edge_reads": 2 * edges,
            "variable_position_search_comparisons": vn_search,
            "syndrome_bits_pushed": n + m,
            "syndrome_bitvec_allocations": 2,
            "syndrome_edge_bit_reads": edges,
        },
        "aff3ct_per_iteration": {
            "check_indexed_reads": 2 * edges,
            "check_indexed_writes": edges,
            "variable_sequential_reads": 2 * edges,
            "variable_writes": edges,
            "posterior_reads": edges,
            "syndrome_edge_reads_at_most": edges,
            "allocations": 0,
        },
    }


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    report = {"schema": "ldpc-structural-costs-v1", "codes": {}}
    for argument in sys.argv[1:]:
        code, _, path = argument.partition("=")
        report["codes"][code] = costs(path)
    json.dump(report, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
