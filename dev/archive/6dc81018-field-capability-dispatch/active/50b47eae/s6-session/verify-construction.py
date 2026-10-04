#!/usr/bin/env python3
"""Verify the realized construction of the v4 C2 translation-only ensemble.

Per-arm mode:   verify-construction.py <ledger.tsv> <arm>
Cross-arm mode: verify-construction.py --cross <ledger-ref.tsv> <ledger-cand.tsv>

Per-arm checks are v2 SA4 read through v3 SB3 restricted to axis E, plus v4
C2's balance properties. Cross-arm checks are v4 C3's distributional-equality
precondition. Both run before any timed window.
"""
import sys, collections

def load(path):
    rows = []
    with open(path) as fh:
        header = fh.readline().rstrip("\n").split("\t")
        for line in fh:
            rows.append(dict(zip(header, line.rstrip("\n").split("\t"))))
    return rows

fails = []
def check(name, ok, detail=""):
    print(f"[{'ok' if ok else 'FAIL'}] {name}{(': ' + detail) if detail else ''}")
    if not ok:
        fails.append(name)

def arm_facts(rows):
    js = [int(r["member_j"]) for r in rows]
    vaddr = {int(r["member_j"]): int(r["text_vaddr"], 16) for r in rows}
    E = {int(r["member_j"]): int(r["E"]) for r in rows}
    poff = {j: vaddr[j] % 4096 for j in js}
    return js, vaddr, E, poff

def verify_arm(path, arm):
    rows = load(path)
    print(f"== arm {arm}: {len(rows)} ledger rows ==")
    failed = [r for r in rows if r["sha256"] == "FAILED"]
    check("no member failed to build twice", not failed,
          "" if not failed else f"{[r['member_j'] for r in failed]}")
    check("128 members", len(rows) == 128, f"{len(rows)}")
    js, vaddr, E, poff = arm_facts(rows)
    check("member indices 0..127 exactly once", sorted(js) == list(range(128)))
    check("E levels 0..127 exactly once", sorted(E.values()) == list(range(128)))
    shas = [r["sha256"] for r in rows]
    check("128 distinct binaries", len(set(shas)) == 128, f"{len(set(shas))} distinct")
    # page-offset translation law (v3 B3.1, single level of G)
    residues = set((vaddr[j] - 32 * E[j]) % 4096 for j in js)
    P = residues.pop() if len(residues) == 1 else None
    check("page offset is (P + 32*E) mod 4096 at every member", P is not None
          and all(poff[j] == (P + 32 * E[j]) % 4096 for j in js), f"P={P}")
    check("128 distinct page offsets", len(set(poff.values())) == 128,
          f"{len(set(poff.values()))} distinct")
    planes = collections.Counter(vaddr[j] - 32 * E[j] for j in js)
    plane = planes.most_common(1)[0][0]
    displaced = [(j, (vaddr[j] - 32 * E[j] - plane) // 4096) for j in js
                 if vaddr[j] - 32 * E[j] != plane]
    print(f"   whole-page displacements (member, pages): {displaced if displaced else 'none'}")
    even = [j for j in js if j % 2 == 0]
    odd = [j for j in js if j % 2 == 1]
    check("halves hold 64 members each", len(even) == 64 and len(odd) == 64)
    off_e = collections.Counter(vaddr[j] % 64 for j in even)
    off_o = collections.Counter(vaddr[j] % 64 for j in odd)
    check("halves hold 32 members at each of the 2 realized cache-line offsets",
          off_e == off_o and set(off_e.values()) == {32},
          f"even={dict(sorted(off_e.items()))} odd={dict(sorted(off_o.items()))}")
    ep_e = collections.Counter(E[j] % 2 for j in even)
    ep_o = collections.Counter(E[j] % 2 for j in odd)
    check("halves hold 32 members at each parity of E", ep_e == ep_o == {0: 32, 1: 32},
          f"even={dict(ep_e)} odd={dict(ep_o)}")
    set_e = collections.Counter((vaddr[j] >> 6) % 64 for j in even)
    set_o = collections.Counter((vaddr[j] >> 6) % 64 for j in odd)
    check("each half covers all 64 L1i sets exactly once",
          len(set_e) == 64 and set(set_e.values()) == {1}
          and len(set_o) == 64 and set(set_o.values()) == {1})
    check("the two halves' (address>>6) mod 64 multisets are equal", set_e == set_o)
    sizes = [int(r["bytes"]) for r in rows]
    print(f"   binary sizes {min(sizes)}..{max(sizes)} bytes")
    bt = [int(r["build_s"]) for r in rows]
    print(f"   build wall-clock {min(bt)}..{max(bt)} s, total {sum(bt)} s")
    retries = [r["member_j"] for r in rows if r["attempts"] != "1"]
    print(f"   members needing a retry: {retries if retries else 'none'}")

def verify_cross(pref, pcand):
    rows_r, rows_c = load(pref), load(pcand)
    print("== cross-arm (v4 C3) ==")
    _, vr, _, pr = arm_facts(rows_r)
    _, vc, _, pc = arm_facts(rows_c)
    Pr = min(pr.values()) % 32
    Pc = min(pc.values()) % 32
    check("P_ref == P_cand (mod 32)", Pr == Pc, f"ref%32={Pr} cand%32={Pc}")
    check("page-offset sets equal", set(pr.values()) == set(pc.values()))
    offs_r = collections.Counter(v % 64 for v in vr.values())
    offs_c = collections.Counter(v % 64 for v in vc.values())
    check("cache-line offset multisets equal", offs_r == offs_c,
          f"ref={dict(sorted(offs_r.items()))} cand={dict(sorted(offs_c.items()))}")
    sets_r = collections.Counter((v >> 6) % 64 for v in vr.values())
    sets_c = collections.Counter((v >> 6) % 64 for v in vc.values())
    check("L1i set-index multisets equal", sets_r == sets_c)

if sys.argv[1] == "--cross":
    verify_cross(sys.argv[2], sys.argv[3])
else:
    verify_arm(sys.argv[1], sys.argv[2])
print(f"RESULT: {'PASS' if not fails else 'FAIL ' + str(fails)}")
sys.exit(0 if not fails else 1)
