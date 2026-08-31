#!/usr/bin/env python3
"""Post-processing fixups for the gf2-algebra Aeneas extraction.

The bipedal3 V1 proofs (see `dev/plans/a0c0a45f/d2_lean_bipedal3_sketch.md` and
JIT issue f05ffbe1) target the inherent `Bipedal3::{add,sub,mul,neg}_inherent`
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


OPAQUE_FP_WRAPPER_RE = re.compile(
    r"@\[reducible, rust_trait_impl\s+\"([^\"]+)\"\]\s*\n"
    r"def (gf2_core\.gfp\.Fp\.Insts\.CoreOps[A-Za-z0-9_]+) \(P : Std\.U64\) :\s*\n"
    r"((?:[^=]+))\s*:= \{[\s\S]*?\n\}\n",
    re.MULTILINE,
)


def axiomatize_opaque_fp_wrappers(text: str) -> tuple[str, int]:
    """Axiomatize only Fp operator wrappers whose bodies are opaque."""

    def replace_def(m: "re.Match[str]") -> str:
        marker = m.group(1)
        name = m.group(2)
        sig = m.group(3).rstrip()
        # Axioms cannot be `@[reducible]`; keep only `rust_trait_impl`.
        return (
            f"@[rust_trait_impl \"{marker}\"]\n"
            f"axiom {name} (P : Std.U64) :\n{sig}\n"
        )

    return OPAQUE_FP_WRAPPER_RE.subn(replace_def, text)


def fixup_funs(path: str) -> None:
    with open(path) as f:
        text = f.read()

    # ------------------------------------------------------------------
    # 1) Replace each broken `def gf2_core.gfp.Fp.Insts.CoreOps{Arith,…}* (P)`
    #    trait-impl wrapper (which references opaque .add / .sub / .mul /
    #    .neg / .div / .add_assign / etc bodies) with an axiom. These
    #    defs sit at lines 70..230 in the extraction; their bodies refer
    #    to `gf2_core.gfp.Fp.Insts.CoreOpsArith…FpFp.add` etc, which are
    #    not extracted as bodies. Axiomatising them eliminates the
    #    `Unknown constant` errors without losing anything the bipedal3
    #    proofs need (they never project these instances).
    #
    #    The post-seam FiniteField dictionary is an ordinary, non-recursive
    #    `def` and remains intact. Restricting the pattern to `CoreOps*`
    #    prevents the workaround from swallowing that generated dictionary.
    # ------------------------------------------------------------------
    text, _ = axiomatize_opaque_fp_wrappers(text)

    # ------------------------------------------------------------------
    # 2) Axiomatise the malformed `Zip` iterator-adapter trait-impl
    #    instance.
    #
    #    The D5 packed5 extraction (`--features f5`, JIT 30e98ef1) pulls
    #    in `Packed5Vec::all_zero`, whose `self.b0.iter().zip(self.b1
    #    .iter()).zip(self.b2.iter()).all(...)` chain makes Charon emit a
    #    `core::iter::adapters::zip::Zip` `Iterator` instance. Charon
    #    generates that record with fields `next / zip / map / enumerate
    #    / collect / all`, but the Aeneas Lean stdlib `Iterator`
    #    structure (`backends/lean/Aeneas/Std/Core/Iter.lean:51-58`) has
    #    exactly four fields: `next / step_by / enumerate / take`. The
    #    generated record is therefore rejected by Lean
    #    (`zip is not a field of structure …Iterator`, `Fields missing:
    #    step_by, take`), and its `…Pair.{next,zip,map,collect,all}`
    #    helper bodies are never even defined (undefined references).
    #
    #    `Packed5Vec` is explicitly out of D5 scope
    #    (`dev/plans/30e98ef1/d5_lean_packed5_sketch.md` §7); the proofs target
    #    only the `Packed5::{add,sub,mul,neg}_inherent` element ops and
    #    never project this `Zip` instance (it is referenced nowhere
    #    outside its own malformed body). Replacing the whole `def …
    #    := { … }` block with an `axiom` of the same declared signature
    #    eliminates the build error without losing anything the proofs
    #    use — the identical R5/R6 unreachable-artefact-axiomatisation
    #    pattern as the gf2_core `Fp` trait-impls above.
    # ------------------------------------------------------------------
    zip_inst_re = re.compile(
        r"@\[reducible, rust_trait_impl\s*\n"
        r'\s*"core::iter::traits::iterator::Iterator<core::iter::adapters::zip::Zip<@A, @B>, \(@Clause0_Item, @Clause1_Item\)>"\]\s*\n'
        r"def (core\.iter\.adapters\.zip\.Zip\.Insts\.CoreIterTraitsIteratorIteratorPair) "
        r"(\{A :\s*\n"
        r"  Type\} \{B : Type\} \{Clause0_Item : Type\} \{Clause1_Item : Type\}\s*\n"
        r"  \(traitsiteratorIteratorInst : core\.iter\.traits\.iterator\.Iterator A\s*\n"
        r"  Clause0_Item\) \(traitsiteratorIteratorInst1 :\s*\n"
        r"  core\.iter\.traits\.iterator\.Iterator B Clause1_Item\)) :\s*\n"
        r"  (core\.iter\.traits\.iterator\.Iterator \(core\.iter\.adapters\.zip\.Zip A B\)\s*\n"
        r"  \(Clause0_Item × Clause1_Item\)) := \{[\s\S]*?\n\}\n",
        re.MULTILINE,
    )

    def replace_zip_inst(m: "re.Match[str]") -> str:
        name = m.group(1)
        binders = m.group(2)
        sig = m.group(3)
        return (
            "@[rust_trait_impl\n"
            '  "core::iter::traits::iterator::Iterator<core::iter::adapters::zip::Zip<@A, @B>, (@Clause0_Item, @Clause1_Item)>"]\n'
            f"axiom {name} {binders} :\n  {sig}\n"
        )

    text, nz = zip_inst_re.subn(replace_zip_inst, text)
    if nz != 1:
        raise SystemExit(
            f"fix-aeneas-gf2algebra: expected exactly one Zip Iterator instance, got {nz}"
        )

    # ------------------------------------------------------------------
    # 2b) Axiomatise the malformed `slice::iter::IterMut` Iterator
    #     trait-impl instance.
    #
    #     The D6 packed7 extraction (`--features f5,f7`, JIT 30e98ef1)
    #     pulls in `Packed7Vec`'s `add_assign`/`sub_assign`/`mul_assign`/
    #     `neg_assign`, which iterate `self.words.iter_mut()`. That makes
    #     Charon emit a `core::slice::iter::IterMut` `Iterator` trait
    #     instance. Identical malformation to the `Zip` case above:
    #     Charon generates the record with fields `next / zip / map /
    #     enumerate / collect / all`, but the Aeneas Lean stdlib
    #     `Iterator` structure (`backends/lean/Aeneas/Std/Core/Iter.lean`)
    #     has exactly four fields `next / step_by / enumerate / take`, so
    #     Lean rejects the record (`zip is not a field of structure …
    #     Iterator`, `Fields missing: step_by, take`), and the
    #     `…IterMut.Insts.CoreIterTraitsIteratorIteratorMutAT.{zip,map,
    #     collect,all}` helper bodies are never even defined.
    #
    #     `Packed7Vec` (like `Packed5Vec`) is explicitly out of D5/D6
    #     scope (`dev/plans/30e98ef1/d6_lean_packed7_sketch.md` §7); the proofs
    #     target only the `Packed7::{add,sub,mul,neg}_inherent` element
    #     ops and never project this `IterMut` instance (it is referenced
    #     nowhere outside its own malformed body / the out-of-scope
    #     `Packed{5,7}Vec` ops). Replacing the whole `def … := { … }`
    #     block with an `axiom` of the same declared signature eliminates
    #     the build error without losing anything the proofs use — the
    #     identical R5/R6 unreachable-artefact-axiomatisation pattern as
    #     the gf2_core `Fp` trait-impls and the `Zip` instance above.
    #     This is the D6 §7 anticipated, out-of-scope-but-necessary
    #     post-process extension (the f7 `Packed7Vec` analogue of the
    #     D5 Packed5Vec `Zip` artefact).
    # ------------------------------------------------------------------
    itermut_inst_re = re.compile(
        r"@\[reducible, rust_trait_impl\s*\n"
        r'\s*"core::iter::traits::iterator::Iterator<core::slice::iter::IterMut<\'a, @T>, &\'a mut @T>"\]\s*\n'
        r"def (core\.slice\.iter\.IterMut\.Insts\.CoreIterTraitsIteratorIteratorMutAT) "
        r"(\(T :\s*\n"
        r"  Type\)) : (core\.iter\.traits\.iterator\.Iterator \(core\.slice\.iter\.IterMut T\) T) := \{[\s\S]*?\n\}\n",
        re.MULTILINE,
    )

    def replace_itermut_inst(m: "re.Match[str]") -> str:
        name = m.group(1)
        binders = m.group(2)
        sig = m.group(3)
        return (
            "@[rust_trait_impl\n"
            "  \"core::iter::traits::iterator::Iterator<core::slice::iter::IterMut<'a, @T>, &'a mut @T>\"]\n"
            f"axiom {name} {binders} :\n  {sig}\n"
        )

    text, nim = itermut_inst_re.subn(replace_itermut_inst, text)
    if nim != 1:
        raise SystemExit(
            f"fix-aeneas-gf2algebra: expected exactly one IterMut Iterator instance, got {nim}"
        )

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
