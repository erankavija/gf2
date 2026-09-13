#!/usr/bin/env python3
"""Derive the per-iteration structural work of both update paths (jit:07ca8585).

Exact counts derived from each recorded graph and the loop structure of the two
generations, not timings. The `before` counts are read from the predecessor's
committed derivation rather than recomputed, so the two documents cannot
disagree; the `after` counts follow from the loop structure this issue
introduces and the same graph dimensions.

The `after` loop structure, per flooding iteration:

  check update      each check reads its `d_c` incoming messages twice, once to
                    reduce and once to write, and writes `d_c` outgoing
                    messages: `2 E` reads and `E` writes over the whole graph,
                    from `m` reduction calls, with no neighbour search and no
                    allocation.
  variable update   each variable walks its slots twice, once to sum the belief
                    and once to write its outgoing messages, resolving each
                    slot through one index load: `2 E` message reads, `2 E`
                    index loads, `E` writes, no neighbour search.
  syndrome          `n` hard decisions into a reused buffer, then at most `E`
                    edge reads, stopping at the first unsatisfied check, with
                    no allocation.

Usage (from the worktree root):
  edge-costs.py --before PATH --output PATH
"""

import argparse
import json
import pathlib


def after_counts(code):
    edges = code["edges"]
    return {
        "check_inputs_read": 2 * edges,
        "check_outputs_written": edges,
        "check_position_search_comparisons": 0,
        "min_sum_reduction_calls": code["m"],
        "min_sum_input_vec_allocations": 0,
        "variable_edge_reads": 2 * edges,
        "variable_index_loads": 2 * edges,
        "variable_position_search_comparisons": 0,
        "hard_decision_writes": code["n"],
        "syndrome_edge_bit_reads_at_most": edges,
        "syndrome_bitvec_allocations": 0,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()

    before = json.loads(args.before.read_text(encoding="utf-8"))
    codes = {}
    for key, code in before["codes"].items():
        codes[key] = {
            "alist_sha256": code["alist_sha256"],
            "n": code["n"],
            "m": code["m"],
            "edges": code["edges"],
            "check_degree_histogram": code["check_degree_histogram"],
            "variable_degree_histogram": code["variable_degree_histogram"],
            "before_per_iteration": code["gf2_per_iteration"],
            "after_per_iteration": after_counts(code),
            "aff3ct_per_iteration": code["aff3ct_per_iteration"],
        }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(
            {
                "schema": "ldpc-update-structural-costs-v1",
                "before_source": {
                    "path": str(args.before),
                    "generator": "dev/active/3be770d5/survey/edge-costs.py",
                },
                "codes": codes,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(args.output)


if __name__ == "__main__":
    main()
