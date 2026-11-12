"""Tabulates the rounds `cutover.sh` collects, one markdown row per cell and arm.

Usage: summarize.py <rounds.tsv>

A configuration's rate is work bits per second in each round it was timed;
a row reports the median and the min..max over rounds. The current revision
is the reference: the pre-cutover row's verdict is `slower` when every current
round is below every pre-cutover round, `faster` when every one is above, and
`within spread` otherwise. An external library's row is its fastest
configuration by median, with the same verdict read from gf2's side. A
configuration whose output digest differs from the current revision's on the
cell is reported and left out; the script then exits 1.
"""

import statistics
import sys
from collections import OrderedDict

FIELDS = (
    "round role arm config workload row batch work calls seconds digest status"
).split()


def load(path):
    configs = OrderedDict()
    for line in open(path):
        record = dict(zip(FIELDS, line.rstrip("\n").split("\t")))
        cell = (record["workload"], record["row"], int(record["batch"]))
        key = (cell, record["role"], record["arm"], record["config"])
        entry = configs.setdefault(
            key, {"rates": [], "digests": set(), "statuses": set()}
        )
        if record["status"] == "timed":
            work, calls = float(record["work"]), int(record["calls"])
            entry["rates"].append(work * calls / float(record["seconds"]) / 1e6)
            entry["digests"].add(record["digest"])
        else:
            entry["statuses"].add(record["status"])
    return configs


def verdict(current, other):
    if max(current) < min(other):
        return "gf2 slower"
    if min(current) > max(other):
        return "gf2 faster"
    return "within spread"


def main(path):
    configs = load(path)
    cells = OrderedDict()
    for (cell, role, arm, config), entry in configs.items():
        cells.setdefault(cell, []).append((role, arm, config, entry))
    differs = False
    print("| cell | arm | configuration | median Mb/s | min..max Mb/s | gf2 ratio | verdict |")
    print("|---|---|---|---|---|---|---|")
    for (workload, row, batch), entries in cells.items():
        name = f"{workload} {row}" + (f" B={batch}" if workload == "W1" else "")
        current = [e for e in entries if e[0] == "current" and e[3]["rates"]]
        reference = current[0][3] if current else None
        best = OrderedDict()
        for role, arm, config, entry in entries:
            if not entry["rates"]:
                status = ", ".join(sorted(entry["statuses"]))
                print(f"| {name} | {arm} | {config} | - | - | - | {status} |")
                continue
            if reference is not None and entry["digests"] != reference["digests"]:
                differs = True
                print(f"| {name} | {arm} | {config} | - | - | - | output differs |")
                continue
            median = statistics.median(entry["rates"])
            if role != "external":
                best[(role, arm)] = (config, entry, median)
            elif (role, arm) not in best or median > best[(role, arm)][2]:
                best[(role, arm)] = (config, entry, median)
        for (role, arm), (config, entry, median) in best.items():
            rates = entry["rates"]
            spread = f"{min(rates):.1f}..{max(rates):.1f}"
            if role == "current" or reference is None:
                ratio, judged = "-", "-"
            else:
                ratio = f"{statistics.median(reference['rates']) / median:.2f}"
                judged = verdict(reference["rates"], rates)
            label = config + (" (fastest)" if role == "external" else "")
            print(
                f"| {name} | {arm} | {label} | {median:.1f} | {spread} | {ratio} | {judged} |"
            )
    return 1 if differs else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
