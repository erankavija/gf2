#!/usr/bin/env python3
"""Check whether the xdsopl/LDPC DVB-T2 table builds the identical H (jit:c077a88b).

The gf2 DVB-T2 rate-1/2 parity-check matrix is exported to MacKay AList by the
workspace `export_alist` binary. xdsopl/LDPC carries the same ETSI EN 302 755
table as a C++ header of accumulator positions. This script rebuilds H from the
xdsopl header with the standard's IRA accumulation rule and compares the column
adjacency sets with the AList. Equal sets mean both decoders operate on the
bit-identical code and an xdsopl arm needs no code adapter; any difference is
reported with the first mismatching column.

Usage:
  verify-xdsopl-code.py --header <xdsopl>/dvb_t2_tables.hh --table DVB_T2_TABLE_A1 \
      --alist <exported.alist> --output <report.json>
"""

import argparse
import json
import re
import sys


def parse_table(path, name):
    """Extracts M, N, K, DEG, LEN and POS of one xdsopl table struct."""
    text = open(path, encoding="utf-8").read()
    start = text.index("struct %s" % name)
    end = text.index("\n};", start)
    body = text[start:end]

    def scalar(field):
        return int(re.search(r"static const int %s = (\d+);" % field, body).group(1))

    def array(field):
        block = re.search(
            r"static constexpr int %s\[\] = \{(.*?)\};" % field, body, re.S
        ).group(1)
        values = [int(v) for v in re.findall(r"-?\d+", block)]
        while values and values[-1] == 0:
            values.pop()
        return values

    return {
        "M": scalar("M"),
        "N": scalar("N"),
        "K": scalar("K"),
        "DEG": array("DEG"),
        "LEN": array("LEN"),
        "POS": array("POS"),
    }


def build_columns(table):
    """Column adjacency sets of H under the ETSI IRA accumulation rule."""
    m, n, k = table["M"], table["N"], table["K"]
    r = n - k
    q = r // m
    rows = []
    cursor = 0
    for degree, length in zip(table["DEG"], table["LEN"]):
        for _ in range(length):
            rows.append(table["POS"][cursor : cursor + degree])
            cursor += degree
    if cursor != len(table["POS"]):
        raise SystemExit("POS holds %d entries, groups consume %d" % (len(table["POS"]), cursor))
    columns = [set() for _ in range(n)]
    for bit in range(k):
        group = rows[bit // m]
        offset = bit % m
        for position in group:
            columns[bit].add((position + offset * q) % r)
    # Staircase parity part. The xdsopl flooding decoder forms check i from
    # parity LLRs i-1 and i (`flooding_decoder.hh` `check_node_update`), so
    # parity column p lies in checks p and p+1, which is the orientation the
    # exported gf2 matrix uses.
    for parity in range(r):
        columns[k + parity].add(parity)
        if parity + 1 < r:
            columns[k + parity].add(parity + 1)
    return columns


def read_alist(path):
    """Column adjacency sets of an AList file, converted to 0-based rows."""
    with open(path, encoding="utf-8") as handle:
        tokens = handle.read().split("\n")
    n, m = (int(v) for v in tokens[0].split())
    columns = []
    for line in tokens[4 : 4 + n]:
        columns.append({int(v) - 1 for v in line.split() if int(v) != 0})
    return n, m, columns


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--header", required=True)
    parser.add_argument("--table", required=True)
    parser.add_argument("--alist", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    table = parse_table(args.header, args.table)
    built = build_columns(table)
    n, m, reference = read_alist(args.alist)

    report = {
        "schema": "ldpc-survey-code-identity-v1",
        "table": args.table,
        "header": args.header,
        "alist": args.alist,
        "table_dimensions": {"N": table["N"], "K": table["K"], "M": table["M"]},
        "alist_dimensions": {"N": n, "M": m},
        "identical": False,
        "first_difference": None,
        "nonzeros_table": sum(len(c) for c in built),
        "nonzeros_alist": sum(len(c) for c in reference),
    }
    if table["N"] != n or table["N"] - table["K"] != m:
        report["first_difference"] = "dimensions differ"
    else:
        difference = next(
            (i for i in range(n) if built[i] != reference[i]),
            None,
        )
        if difference is None:
            report["identical"] = True
        else:
            report["first_difference"] = {
                "column": difference,
                "table_rows": sorted(built[difference]),
                "alist_rows": sorted(reference[difference]),
            }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(json.dumps(report if report["identical"] else report, indent=2))
    return 0 if report["identical"] else 1


if __name__ == "__main__":
    sys.exit(main())
