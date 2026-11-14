#!/usr/bin/env python3
"""Post-processing fixups for the gf2-algebra Aeneas extraction.

The bipedal3 V1 proofs (@/issue/f05ffbe1) target the inherent `Bipedal3::{add,sub,mul,neg}_inherent`
methods. These are pure bitwise on `Std.U64` and do not reach into the
`gf2_core::gfp::Fp` or `FiniteField` machinery at runtime — but Charon still
extracts those trait impls transitively because they appear as type-level
bounds on `PackedField<Fp<3>>`.

The transitively-extracted per-trait `add/sub/mul/...` Fp impls reference body
definitions such as `gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp.add` that are
opaque in this extraction, surfacing as `Unknown constant`.

These gf2_core defs are not exercised by the bipedal3 proofs.
The bipedal3 inherent / trait arithmetic operates purely on `Std.U64`
words; the only thing the proofs care about is the body shape of the four
`Insts.Gf2_algebraPackedPackedFieldFp3U64U128.{add,sub,mul,neg}` defs and
the four `Bipedal3.{add,sub,mul,neg}_inherent` wrappers.

This script rewrites the broken / partial gf2_core defs as axioms in
`Funs.lean`. The proofs never elaborate them.

Run after `aeneas` and `fix-aeneas-dupes.py`:

    python3 scripts/fix-aeneas-gf2algebra.py proofs/Gf2Algebra/Funs.lean
"""

import re
import sys
from collections import Counter


OPAQUE_FP_WRAPPER_NAMES = (
    "gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithSubFpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithMulFpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithDivFpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithAddAssignFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithAddAssignShared0Fp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithAddShared0FpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithSubShared0FpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithMulShared0FpFp",
    "gf2_core.gfp.Fp.Insts.CoreOpsArithDivShared0FpFp",
)
OPAQUE_FP_WRAPPER_SET = frozenset(OPAQUE_FP_WRAPPER_NAMES)
TRANSPARENT_FP_WRAPPER_SET = frozenset(
    {"gf2_core.gfp.Fp.Insts.CoreOpsArithNegFp"}
)
EXPECTED_FP_WRAPPER_SET = OPAQUE_FP_WRAPPER_SET | TRANSPARENT_FP_WRAPPER_SET

FP_CORE_OPS_DEF_RE = re.compile(
    r"^def (?P<name>gf2_core\.gfp\.Fp\.Insts\.CoreOps[A-Za-z0-9_]+)(?=\s)",
    re.MULTILINE,
)
FP_CORE_OPS_WRAPPER_RE = re.compile(
    r"@\[reducible, rust_trait_impl\s+\"(?P<marker>[^\"]+)\"\]\s*\n"
    r"def (?P<name>gf2_core\.gfp\.Fp\.Insts\.CoreOps[A-Za-z0-9_]+) "
    r"\(P : Std\.U64\)\s*:\s*"
    r"(?P<sig>[^=]+?)\s*:= \{[\s\S]*?\n\}\n",
    re.MULTILINE,
)


def axiomatize_opaque_fp_wrappers(text: str) -> tuple[str, int]:
    """Axiomatize the exact known set of opaque Fp operator wrappers."""

    declared = [m.group("name") for m in FP_CORE_OPS_DEF_RE.finditer(text)]
    counts = Counter(declared)
    missing = sorted(EXPECTED_FP_WRAPPER_SET - counts.keys())
    duplicate = sorted(name for name, count in counts.items() if count != 1)
    unexpected = sorted(counts.keys() - EXPECTED_FP_WRAPPER_SET)
    if missing or duplicate or unexpected:
        details = []
        if missing:
            details.append("missing: " + ", ".join(missing))
        if duplicate:
            details.append("duplicate: " + ", ".join(duplicate))
        if unexpected:
            details.append("unexpected: " + ", ".join(unexpected))
        raise SystemExit(
            "fix-aeneas-gf2algebra: Fp CoreOps wrapper set changed ("
            + "; ".join(details)
            + ")"
        )

    shaped = [
        m.group("name") for m in FP_CORE_OPS_WRAPPER_RE.finditer(text)
    ]
    shape_counts = Counter(shaped)
    malformed = sorted(
        name
        for name in EXPECTED_FP_WRAPPER_SET
        if shape_counts.get(name) != 1
    )
    if malformed:
        raise SystemExit(
            "fix-aeneas-gf2algebra: unsupported Fp CoreOps wrapper shape: "
            + ", ".join(malformed)
        )

    def replace_def(m: "re.Match[str]") -> str:
        nonlocal replaced
        marker = m.group("marker")
        name = m.group("name")
        if name in TRANSPARENT_FP_WRAPPER_SET:
            return m.group(0)
        replaced += 1
        sig = m.group("sig").rstrip()
        # Axioms cannot be `@[reducible]`; keep only `rust_trait_impl`.
        return (
            f"@[rust_trait_impl \"{marker}\"]\n"
            f"axiom {name} (P : Std.U64) :\n  {sig}\n"
        )

    replaced = 0
    rewritten = FP_CORE_OPS_WRAPPER_RE.sub(replace_def, text)
    if replaced != len(OPAQUE_FP_WRAPPER_NAMES):
        raise SystemExit(
            "fix-aeneas-gf2algebra: expected to axiomatize exactly "
            f"{len(OPAQUE_FP_WRAPPER_NAMES)} Fp CoreOps wrappers, got {replaced}"
        )
    return rewritten, replaced


# Map and Zip can use the upstream Iterator model's defaults once their
# dictionaries contain only `next`.
ITERATOR_NAMES = (
    "core.iter.adapters.map.Map.Insts.CoreIterTraitsIteratorIterator",
    "core.iter.adapters.zip.Zip.Insts.CoreIterTraitsIteratorIteratorPair",
)


def normalize_iterator_wrappers(text: str) -> str:
    for name in ITERATOR_NAMES:
        pattern = re.compile(
            r'@\[reducible, rust_trait_impl(?P<marker>[^\]]+)\]\s*\n'
            r'impl_def ' + re.escape(name) +
            r'(?P<signature>\s[\s\S]*?)\s*:= \{\n'
            r'(?P<next>  next :=[\s\S]*?)(?=  zip :=)'
            r'[\s\S]*?\n\}\n'
        )

        def replace(m: "re.Match[str]") -> str:
            return (f'@[reducible, rust_trait_impl{m["marker"]}]\n'
                    f'def {name}{m["signature"]} := {{\n{m["next"]}}}\n')

        text, count = pattern.subn(replace, text)
        if count != 1:
            raise SystemExit(
                f"fix-aeneas-gf2algebra: expected one {name} dictionary, got {count}"
            )
    return text


def fixup_funs(path: str) -> None:
    with open(path) as f:
        text = f.read()
    text, _ = axiomatize_opaque_fp_wrappers(text)
    text = normalize_iterator_wrappers(text)
    with open(path, "w") as f:
        f.write(text)


if __name__ == "__main__":
    for p in sys.argv[1:]:
        fixup_funs(p)
