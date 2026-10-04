#!/usr/bin/env python3
"""Verify the realized construction of one arm's ensemble from its build ledger.

Checks the properties layout-attribution-verdict-v2 §A4 states, read through
amendment v3 §B3 (owner decision DEC-J, 2026-08-22): the translation law on
page offsets mod 4096 with one constant P per level of G, distinct placements
as (page offset, G) pairs with 128 distinct page offsets within each level of
G, and every whole-page displacement recorded by member. All other checks read
as v2 wrote them. The v2-as-written text of this script is preserved beside it
as verify-construction-v2.py.
"""
import sys, collections

path = sys.argv[1]
arm = sys.argv[2]

rows = []
with open(path) as fh:
    header = fh.readline().rstrip("\n").split("\t")
    for line in fh:
        f = line.rstrip("\n").split("\t")
        rows.append(dict(zip(header, f)))

print(f"== arm {arm}: {len(rows)} ledger rows ==")
fails = []
def check(name, ok, detail=""):
    print(f"[{'ok' if ok else 'FAIL'}] {name}{(': ' + detail) if detail else ''}")
    if not ok:
        fails.append(name)

failed_builds = [r for r in rows if r["sha256"] == "FAILED"]
check("no member failed to build twice", not failed_builds,
      "" if not failed_builds else f"{[r['member_j'] for r in failed_builds]}")

check("256 members", len(rows) == 256, f"{len(rows)}")
js = [int(r["member_j"]) for r in rows]
check("member indices 0..255 exactly once", sorted(js) == list(range(256)))

# enumeration formulas
bad = [j for j in js if int(dict((r["member_j"], r) for r in rows)[str(j)]["E"]) != j // 2
       or int(dict((r["member_j"], r) for r in rows)[str(j)]["G"]) != ((j // 4) + j) % 2]
check("E(j)=floor(j/2) and G(j)=(floor(j/4)+j) mod 2 at every member", not bad, str(bad[:5]))

shas = [r["sha256"] for r in rows]
check("256 distinct binaries", len(set(shas)) == 256, f"{len(set(shas))} distinct")

vaddr = {int(r["member_j"]): int(r["text_vaddr"], 16) for r in rows}
E = {int(r["member_j"]): int(r["E"]) for r in rows}
G = {int(r["member_j"]): int(r["G"]) for r in rows}
poff = {j: vaddr[j] % 4096 for j in js}

# v3 B3.2: 128 distinct page offsets within each level of G, and 256 distinct
# (page offset, G) placements
per_g_ok = True
for g in (0, 1):
    n = len(set(poff[j] for j in js if G[j] == g))
    if n != 128:
        per_g_ok = False
    print(f"   G={g}: {n} distinct page offsets")
check("128 distinct .text page offsets within each level of G", per_g_ok)
placements = set((poff[j], G[j]) for j in js)
check("256 distinct (page offset, G) placements", len(placements) == 256,
      f"{len(placements)} distinct")

# v3 B3.1: page-offset translation law — one constant P per level of G with
# page_offset = (P(G) + 32*E) mod 4096 at every member; whole-page
# displacements recorded by member
P = {}
law_ok = True
for g in (0, 1):
    members = [j for j in js if G[j] == g]
    residues = set((vaddr[j] - 32 * E[j]) % 4096 for j in members)
    if len(residues) != 1:
        law_ok = False
        P[g] = None
    else:
        P[g] = residues.pop()
    for j in members:
        if P[g] is not None and poff[j] != (P[g] + 32 * E[j]) % 4096:
            law_ok = False
check("page offset is (P(G) + 32*E) mod 4096 at every member", law_ok,
      f"P(0)={P[0]} P(1)={P[1]}")
planes = {g: collections.Counter((vaddr[j] - 32 * E[j]) for j in js if G[j] == g)
          for g in (0, 1)}
displaced = []
for g in (0, 1):
    plane = planes[g].most_common(1)[0][0]
    for j in js:
        if G[j] == g and vaddr[j] - 32 * E[j] != plane:
            displaced.append((j, (vaddr[j] - 32 * E[j] - plane) // 4096))
print(f"   whole-page displacements (member, pages): {displaced if displaced else 'none'}")

even = [j for j in js if j % 2 == 0]
odd = [j for j in js if j % 2 == 1]
check("halves hold 128 members each", len(even) == 128 and len(odd) == 128)

# cache-line offsets
off_even = collections.Counter(vaddr[j] % 64 for j in even)
off_odd = collections.Counter(vaddr[j] % 64 for j in odd)
distinct_offsets = sorted(set(list(off_even) + list(off_odd)))
per_half = 128 // len(distinct_offsets)
check(f"each half holds exactly {per_half} members at each of the {len(distinct_offsets)} realized cache-line offsets",
      off_even == off_odd and set(off_even.values()) == {per_half} and set(off_odd.values()) == {per_half},
      f"offsets={distinct_offsets} even={dict(sorted(off_even.items()))} odd={dict(sorted(off_odd.items()))}")

# L1i set index multisets
set_even = collections.Counter((vaddr[j] >> 6) % 64 for j in even)
set_odd = collections.Counter((vaddr[j] >> 6) % 64 for j in odd)
check("the two multisets of (address>>6) mod 64 are equal", set_even == set_odd)
check("each half covers all 64 L1i sets exactly twice",
      set(set_even.values()) == {2} and len(set_even) == 64,
      f"distinct sets even={len(set_even)} counts={sorted(set(set_even.values()))}")

# axis balance per half
e_even = collections.Counter(E[j] for j in even)
e_odd = collections.Counter(E[j] for j in odd)
check("one member per half at each of the 128 levels of E",
      len(e_even) == 128 and set(e_even.values()) == {1} and e_even == e_odd)
g_even = collections.Counter(G[j] for j in even)
g_odd = collections.Counter(G[j] for j in odd)
check("64 members per half at each of the 2 levels of G",
      g_even == {0: 64, 1: 64} and g_odd == {0: 64, 1: 64}, f"even={dict(g_even)} odd={dict(g_odd)}")

sizes = [int(r["bytes"]) for r in rows]
print(f"   binary sizes {min(sizes)}..{max(sizes)} bytes (+{100*(max(sizes)-min(sizes))/min(sizes):.2f} %)")
bt = [int(r["build_s"]) for r in rows]
print(f"   build wall-clock {min(bt)}..{max(bt)} s, total {sum(bt)} s")
retries = [r["member_j"] for r in rows if r["attempts"] != "1"]
print(f"   members needing a retry: {retries if retries else 'none'}")
print(f"RESULT: {'PASS' if not fails else 'FAIL ' + str(fails)}")
sys.exit(0 if not fails else 1)
