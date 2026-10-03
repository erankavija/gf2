#!/usr/bin/env python3
"""Map prose citations in Rust comments to citation-registry keys.

Usage: python3 554c2935-citation-map.py > 554c2935-citation-map.txt

Reads the tracked `.rs` files under `crates/` and `dev/tools/` of the
repository containing the current directory (`git rev-parse
--show-toplevel`, `git ls-files`). A comment line is a line whose first
non-blank characters are `//`, or the part of a code line after ` // `.

Section 1 prints `key | regex | files:lines` for every ROWS entry. Section 2,
"citations without a registry entry", prints the same for UNKEYED; a class
with no remaining line prints `-`. Section 3 prints every comment line that matches HEURISTIC, holds no
`@/citation/` address and matches no ROWS or UNKEYED entry, with the
REASONS label that explains it or `UNEXPLAINED`. Section 4 prints the label
counts. Every key in ROWS must exist in `.jit/references.toml`; the script
exits non-zero otherwise.
"""

import re
import subprocess
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path

# (key, phrase regex, path regex or None, veto regex or None). A row matches a
# comment line when the phrase matches, the path matches and the veto does not.
ROWS = [
    # Keys present before this mapping.
    ("AlbrechtBard2026", r"\bM4RI\b", None, None),
    ("Cassagne2019", r"\b(?:aff3ct|AFF3CT)\b", None, None),
    ("Ccsds2020", r"\[Ccsds2020\]", None, None),
    ("DvbVerification2010",
     r"VV001|ETSI (?:CSP|verification|vector|DVB-T2 (?:verified|test-vector))|real ETSI",
     None, None),
    ("Etsi2015", r"EN 302 755|ETSI(?: |-)(?:parameter|pinned|generator)|ETSI EN\b",
     None, None),
    ("Fossorier1994", r"Fossorier(?: 1994|'s)", None, None),
    ("Lentmaier2010", r"Lentmaier \(2010\)|\[Lentmaier2010\]", None, None),
    ("MaurerPontil2009", r"Maurer[- ](?:and )?Pontil", None, None),
    ("Mula2018", r"\\?\[Mula2018\\?\]", None, None),
    ("SageMath2026", r"\bSageMath\b", None, None),
    ("Scheinerman2024", r"Scheinerman", None, None),
    ("Scheinerman2024",
     r"\bpaper(?:'s)?\b|Theorem 2\.1|Table 2, Appendix B",
     r"gf2-algebra/|bipedal", r"Scheinerman"),
    ("Sionna2026", r"\bSionna\b", None, None),
    ("Steele2014", r"\[Steele2014\]|Steele, Lea and Flood", None, None),
    # 3GPP TS 38.212: clause 5.4.2.2 is validated against V16.4.0
    # (`crates/gf2-coding/data/ldpc/nr_5g/PROVENANCE.md`); every other
    # citation maps to the registered V15.0.0.
    ("ThreeGpp2020", r"5\.4\.2\.2", None, None),
    ("ThreeGpp2017", r"TS 38\.212|\b3GPP\b|Table 5\.3\.2-[123]", None,
     r"5\.4\.2\.2|TS 38\.214"),
    ("ThreeGpp2017", r"§5\.4\.2\.1", r"presets/nr_5g\.rs", None),
    # Keys added by this mapping.
    ("Ccsds2017", r"NASA/CCSDS K=7 standard", None, None),
    ("Nist2001", r"\bAES\b", None, r"IEEE AES standard|AES/crypto"),
    ("ThreeGpp2017a", r"TS 38\.214", None, None),
    ("Amd2020", r"AMD Zen 3 Software Optimization Guide|Software Optimization Guide",
     None, None),
    ("Bahl1974", r"Bahl, Cocke, Jelinek, Raviv|Minimizing Symbol Error Rate", None, None),
    ("Beuchat2010", r"Beuchat et al\.|ePrint 2010/354", None, None),
    ("Chase1972", r"Chase, D\. \(1972\)|measurement information\.\"", None, None),
    ("CoskunPfister2022", r"Coskun & Pfister|arxiv:2103\.16680", None, None),
    ("CrandallPomerance2005", r"Crandall & Pomerance|Numbers: A Computational Perspective",
     None, None),
    ("Devegili2006", r"Devegili|ePrint 2006/471", None, None),
    ("Duffy2019", r"Duffy, K\.R\., Li, J\.|Additive Noise Decoding\.\"", None, None),
    ("Duffy2022", r"Duffy–An–Médard(?: 2022)?", None, None),
    ("Duffy2022", r"slope heuristic from the paper|`IC` in the paper|paper default",
     r"grand/orbgrand\.rs", None),
    ("DumasGiorgiPernet2008", r"Dumas, Giorgi, Pernet|the FFLAS and FFPACK Packages",
     None, None),
    ("DumasPernet2012", r"Dumas[–-]Pernet(?![–-]Sultan)", None, None),
    ("DumasPernet2012", r"cited paper|^\W*2010, alg\. 2\.5", r"field/ple\.rs", None),
    ("DumasPernetSultan2017", r"Dumas-Pernet-Sultan|arXiv:1703\.02438", None, None),
    ("Etsi2012", r"T[RS] 102 831", None, None),
    ("FflasFfpack2021", r"\b(?:fflas|FFLAS)(?:-ffpack|-FFPACK)?\b", None,
     r"Dumas, Giorgi, Pernet|the FFLAS and FFPACK Packages|fflas_\w+\.(?:cpp|sh)"),
    ("Flint2026", r"\bFLINT\b", None, None),
    ("GentlemanSande1966", r"Gentleman & Sande|^\W*\(1966\)", r"field/ntt\.rs", None),
    ("GotoGeijn2008", r"Goto-vandeGeijn 2008|\bGoto/BLIS\b", None, None),
    ("HardyWright2008", r"Hardy & Wright", None, None),
    ("Higham2002", r"Higham ?§ ?14\.1", None, None),
    ("LidlNiederreiter1996", r"Lidl & Niederreiter", None, None),
    ("LinBox2025", r"\bLinBox\b", None, None),
    ("Lubeck2024", r"L[uü]beck|Luebeck", None, None),
    ("Machler2012", r"Machler \(?2012\)?", None, None),
    ("Massey1969", r"Massey \(1969\)", None, None),
    ("McEliece1996", r"McEliece \(1996\)|^\W*Inform\. Theory\.\*$", r"bcjr/mod\.rs", None),
    ("Menezes1997", r"Menezes et al\.", None, None),
    ("MollerGranlund2011", r"Granlund-Möller", None, None),
    ("Montgomery1987", r"Montgomery 1987", None, None),
    ("Nist2013", r"FIPS(?: PUB)? 186-4|NIST B-571", None, None),
    ("Nist2013", r"and from FIPS$", r"primitive_polys\.rs", None),
    ("Oeis2026", r"OEIS A011260", None, None),
    ("OpenBlas2022", r"\bOpenBLAS\b", None, None),
    ("Plonky2026", r"\bPlonky3\b", None, None),
    ("Proth1878", r"François Proth \(1878\)", None, None),
    ("Pyndiah1998", r"Pyndiah, R\.M\. \(1998\)|Pyndiah \(1998\)|^\W*codes\.\" \*IEEE Trans\. Commun\.\*",
     r"product/", None),
    ("Rabin1980", r"Rabin, M\. O\. \(1980\)|SIAM Journal on Computing|Rabin's", None, None),
    ("RandCore2025", r"rand_core(?:-0\.9\.3| 0\.9|'s)", None, None),
    ("RichardsonUrbanke2001", r"Richardson[-–]Urbanke", None, None),
    ("Seroussi1998", r"Seroussi|HPL-98-135", None, None),
    ("Shoup2025", r"\bNTL\b", None, None),
    ("Solomon2020", r"Solomon, A\., Duffy|using GRAND\.\" \*IEEE ISIT\*", None, None),
    ("Swan1962", r"Swan's theorem", None, None),
    ("TenBrink2001", r"ten Brink|parallel concatenated codes\"|IEEE Trans\. Commun\. 2001",
     r"modem/analysis\.rs", None),
    ("VanZee2015", r"BLIS 2015|\bGoto/BLIS\b|\bBLIS\b", None, None),
    ("Vigna2015", r"Vigna's (?:splitmix64\.c|xoroshiro reference)", None, None),
    ("Warren2012", r"Hacker's Delight", None, None),
    ("Warren2012", r"scalar Hacker's$", r"gf2-core/src/matrix\.rs", None),
    ("Wolf1978", r"Wolf \(1978\)|Using a Trellis\.\"", None, None),
    ("Yuan2025",
     r"Yuan[–, ]+Médard|SO-GRAND(?: paper| §V| \(§ V\))|Iterative Decoding to Outperform LDPCs",
     None, None),
    ("Yuan2025", r"\(the paper uses", r"src/crc\.rs", None),
    ("Yuan2025", r"\bpaper(?:'s)?\b|SO-GRAND (?:eq\.|§)",
     r"grand/(?:sogrand|orbgrand)\.rs|product/mod\.rs|bin/sim_runner\.rs|sogrand_crc_probe\.rs",
     r"slope heuristic from the paper|`IC` in the paper|paper default"),
    ("Zivkovic1994", r"Živković", None, None),
]

HEURISTIC = re.compile(
    r"\b[A-Z][a-zà-ÿ]+(?:[-–][A-Z][a-zà-ÿ]+)*,? \(?(?:1[89]|20)\d{2}\)?(?![-\d])"
    r"|et al\.|\be[Pp]rint\b|ar[Xx]iv|\bdoi\b|ISBN|\bIEEE (?:Trans|ISIT|AES)|\bACM\b|\bSIAM\b"
    r"|Trans\.|Proc\.|Math\. Comp|HPL-|OEIS|\bT[RS] \d{2,3}[ .]\d{3}|\bEN \d{3} \d{3}"
    r"|\bETSI\b|\b3GPP\b|\bCCSDS\b|\bFIPS\b|\bNIST\b|\bAES\b|\bMMIX\b"
    r"|[A-Z][a-zà-ÿ]+(?: (?:&|and) [A-Z][a-zà-ÿ]+)? ?§"
    r"|\[[A-Z][A-Za-z]+\d{4}[a-z]?\]|Hacker's Delight|Handbook|\bKnuth\b"
    r"|ptimization Guide|\bpaper\b|\bthesis\b|https?://"
    r"|\b(?:fflas|FFLAS|LinBox|NTL|FLINT|M4RI|OpenBLAS|BLIS|aff3ct|AFF3CT|Sionna"
    r"|Plonky3|SageMath|Magma|rand_core)\b"
)

# Citations without a registry entry: (why no entry exists, phrase regex, path
# regex or None). The comment sweep removes these source pointers.
UNKEYED = [
    ("no work with this title and venue exists",
     r"Condo, C\.|^\W*(?:\*IEEE Trans\. )?Commun\.\*$", r"grand/(?:mod|sogrand)\.rs"),
    ("constant not verified in the named work", r"\bKnuth\b|\bMMIX\b", None),
    ("no edition of the book has this section", r"Shoup §12\.4", None),
    ("no such standard, and the file holds no AES polynomial", r"IEEE AES standard", None),
    ("paper not named", r"\bpaper\b", r"gf2-coding/src/fading\.rs|ldpc_bler_check\.rs"),
]

# Labels for heuristic matches that are not citations; first match wins.
REASONS = [
    ("date or measurement stamp, not a publication year",
     r"\b(?:Amendment|Pinned|Measured|The|With|Zen \d|Since|Rust|Linux|rustc|GCC),? \(?20\d{2}"),
    ("section of an internal design or protocol document",
     r"(?:The|See|Design|Protocol|and) §"),
    ("the verb 'paper over'", r"\bpaper over\b"),
    ("names software without attributing a method or number", r"\bMagma\b"),
    ("CPU feature name", r"AES/crypto"),
]


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def comment_of(line: str) -> str | None:
    s = line.strip()
    if s.startswith("//"):
        return s
    i = line.find(" // ")
    return line[i + 1:] if i >= 0 else None


def main() -> int:
    root = Path(git("rev-parse", "--show-toplevel").strip())
    files = sorted(
        git(
            "-C", str(root), "ls-files", "-z", "crates/**/*.rs", "dev/tools/**/*.rs"
        ).split("\0")[:-1]
    )
    registry = tomllib.loads((root / ".jit/references.toml").read_text(encoding="utf-8"))
    known = {r["key"] for r in registry["references"]}
    missing = sorted({k for k, *_ in ROWS} - known)
    rows = [
        (k, re.compile(p), re.compile(f) if f else None, re.compile(v) if v else None)
        for k, p, f, v in ROWS
    ]
    open_rows = [
        (label, re.compile(p), re.compile(f) if f else None, None)
        for label, p, f in UNKEYED
    ]
    n_keyed = len(rows)
    rows += open_rows
    reasons = [(label, re.compile(p)) for label, p in REASONS]
    hits = [defaultdict(list) for _ in rows]
    residual = []
    for rel in files:
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        for n, line in enumerate(text.split("\n"), 1):
            c = comment_of(line)
            if c is None:
                continue
            matched = False
            for i, (_, pat, path, veto) in enumerate(rows):
                if path and not path.search(rel):
                    continue
                if pat.search(c) and not (veto and veto.search(c)):
                    hits[i][rel].append(n)
                    matched = True
            if matched or "@/citation/" in c or not HEURISTIC.search(c):
                continue
            label = next((l for l, p in reasons if p.search(c)), "UNEXPLAINED")
            residual.append((label, rel, n, c))

    for i, ((key, pat, path, veto), found) in enumerate(zip(rows, hits)):
        if i == 0:
            print("# 1. key | regex | files:lines")
        if i == n_keyed:
            print()
            print("# 2. citations without a registry entry: reason | regex | files:lines")
        scope = f" [path {path.pattern}]" if path else ""
        scope += f" [unless {veto.pattern}]" if veto else ""
        where = " ".join(
            f"{rel}:{','.join(map(str, ns))}" for rel, ns in sorted(found.items())
        )
        print(f"{key} | {pat.pattern}{scope} | {where or '-'}")
    print()
    print("# 3. heuristic matches outside both tables: label | file:line | comment")
    for label, rel, n, c in sorted(residual):
        print(f"{label} | {rel}:{n} | {c}")
    print()
    print("# 4. residual counts by label")
    for label, count in sorted(Counter(r[0] for r in residual).items()):
        print(f"{count:5d}  {label}")
    for key in missing:
        print(f"error: key {key} is absent from .jit/references.toml", file=sys.stderr)
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
