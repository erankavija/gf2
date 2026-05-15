#!/usr/bin/env python3
"""Post-processing fixups for the gf2-algebra Aeneas extraction.

The bipedal3 V1 proofs (see `dev/plans/d2_lean_bipedal3_sketch.md` and
JIT issue f05ffbe1) target the inherent `Bipedal3::{add,sub,mul,neg}_inherent`
methods. These are pure bitwise on `Std.U64` and do not reach into the
`gf2_core::gfp::Fp` or `FiniteField` machinery at runtime — but Charon still
extracts those trait impls transitively because they appear as type-level
bounds on `PackedField<Fp<3>>`.

The transitively-extracted `gf2_core` bodies have two problems Aeneas cannot
resolve in a partial extraction:

1. The `FiniteField` impl for `Fp<P>` uses `*.default` references that refer
   recursively to the impl itself (`WINOGRAD_THRESHOLD.default (… P)`),
   yielding `impl_def: could not resolve recursive fields`.

2. The per-trait `add/sub/mul/...` Fp impls reference body-defs
   (`gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp.add`) that are opaque
   in our extraction, surfacing as `Unknown constant`.

Neither of these gf2_core defs is exercised by the bipedal3 proofs.
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


def fixup_funs(path: str) -> None:
    with open(path) as f:
        text = f.read()

    # ------------------------------------------------------------------
    # 1) Replace the recursive `impl_def gf2_core.gfp.Fp.Insts.
    #    Gf2_coreFieldTraitsFiniteFieldU64U128 (P : Std.U64) : ... := { … }`
    #    block with a single axiom of the same signature.
    # ------------------------------------------------------------------
    impl_def_re = re.compile(
        r"@\[reducible, rust_trait_impl\s+\"gf2_core::field::traits::FiniteField<gf2_core::gfp::Fp<@P>, u64, u128>\"\]\s*\n"
        r"impl_def gf2_core\.gfp\.Fp\.Insts\.Gf2_coreFieldTraitsFiniteFieldU64U128 \(P :\s*\n"
        r"  Std\.U64\) : gf2_core\.field\.traits\.FiniteField \(gf2_core\.gfp\.Fp P\) Std\.U64\s*\n"
        r"  Std\.U128 := \{[\s\S]*?\n\}\n",
        re.MULTILINE,
    )
    # `axiom` declarations cannot carry `@[reducible]`. Keep only the
    # rust_trait_impl marker so the Aeneas trait-resolver still recognises
    # this as the FiniteField<u64, u128> impl on Fp<@P>.
    replacement = (
        "@[rust_trait_impl\n"
        "  \"gf2_core::field::traits::FiniteField<gf2_core::gfp::Fp<@P>, u64, u128>\"]\n"
        "axiom gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128 (P :\n"
        "  Std.U64) : gf2_core.field.traits.FiniteField (gf2_core.gfp.Fp P) Std.U64\n"
        "  Std.U128\n"
    )
    new_text, n = impl_def_re.subn(replacement, text)
    if n != 1:
        raise SystemExit(
            f"fix-aeneas-gf2algebra: expected exactly one FiniteField impl_def, got {n}"
        )
    text = new_text

    # ------------------------------------------------------------------
    # 2) Replace each broken `def gf2_core.gfp.Fp.Insts.CoreOps{Arith,…}* (P)`
    #    trait-impl wrapper (which references opaque .add / .sub / .mul /
    #    .neg / .div / .add_assign / etc bodies) with an axiom. These
    #    defs sit at lines 70..230 in the extraction; their bodies refer
    #    to `gf2_core.gfp.Fp.Insts.CoreOpsArith…FpFp.add` etc, which are
    #    not extracted as bodies. Axiomatising them eliminates the
    #    `Unknown constant` errors without losing anything the bipedal3
    #    proofs need (they never project these instances).
    # ------------------------------------------------------------------
    def_re = re.compile(
        r"@\[reducible, rust_trait_impl\s+\"([^\"]+)\"\]\s*\n"
        r"def (gf2_core\.gfp\.Fp\.Insts\.[A-Za-z0-9_]+) \(P : Std\.U64\) :\s*\n"
        r"((?:[^=]+))\s*:= \{[\s\S]*?\n\}\n",
        re.MULTILINE,
    )

    def replace_def(m: "re.Match[str]") -> str:
        marker = m.group(1)
        name = m.group(2)
        sig = m.group(3).rstrip()
        # axioms cannot be `@[reducible]`; keep only `rust_trait_impl`.
        return (
            f"@[rust_trait_impl \"{marker}\"]\n"
            f"axiom {name} (P : Std.U64) :\n{sig}\n"
        )

    text = def_re.sub(replace_def, text)

    # ------------------------------------------------------------------
    # 3) Remove the no-arg axiom-equivalent `def gf2_core.gfp.Fp.Insts.
    #    CoreCloneClone.clone` style body defs — these are referenced only
    #    by the trait impls above (now axioms), so they are unreachable.
    #    We leave them in place; they typically compile fine since their
    #    bodies are short. If they fail, this block can be extended.
    # ------------------------------------------------------------------

    with open(path, "w") as f:
        f.write(text)


if __name__ == "__main__":
    for p in sys.argv[1:]:
        fixup_funs(p)
