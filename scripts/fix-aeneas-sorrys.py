#!/usr/bin/env python3
"""Replace Aeneas-generated 'sorry' function bodies with known-good implementations.

Aeneas (1180be60) cannot translate certain gfpn function bodies from the LLBC
produced by Charon (419f53b6+). The error is "Assertion failed: new value doesn't
have the same type as its destination" for trait impl ops (Add, Sub, Neg, Mul) on
QuadraticExt and CubicExt. This script restores the correct bodies, which are
semantically equivalent to the Rust source and were previously generated correctly
by older Charon/Aeneas versions.

Usage: python3 fix-aeneas-sorrys.py <Funs.lean>
"""

import re
import sys

# Known-good function bodies, keyed by the function name that appears after "def ".
# Each value is the body text that replaces "  := do\n  sorry".
PATCHES = {
    "gfpn.ext_config.ExtConfig.mul_by_non_residue.default": """\
  := do
  let t ← ExtConfigInst.NON_RESIDUE
  ExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
    x t""",

    "gfpn.cubic.CubicExt.norm": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c2
  let t2 ← ext_configExtConfigInst.mul_by_non_residue t1
  let s0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t t2
  let t3 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c2 self.c2
  let t4 ← ext_configExtConfigInst.mul_by_non_residue t3
  let t5 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c1
  let s1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t4 t5
  let t6 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c1
  let t7 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c2
  let s2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t6 t7
  let t8 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 s0
  let t9 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c2 s1
  let t10 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 s2
  let t11 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      t9 t10
  let t12 ← ext_configExtConfigInst.mul_by_non_residue t11
  ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
    t8 t12""",

    "gfpn.cubic.CubicExt.Insts.CoreOpsArithAddCubicExtCubicExt.add": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c0 rhs.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c1 rhs.c1
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c2 rhs.c2
  gfpn.cubic.CubicExt.new ext_configExtConfigInst t t1 t2""",

    "gfpn.cubic.CubicExt.Insts.CoreOpsArithSubCubicExtCubicExt.sub": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      self.c0 rhs.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      self.c1 rhs.c1
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      self.c2 rhs.c2
  gfpn.cubic.CubicExt.new ext_configExtConfigInst t t1 t2""",

    "gfpn.cubic.CubicExt.Insts.CoreOpsArithNegCubicExt.neg": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c1
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c2
  gfpn.cubic.CubicExt.new ext_configExtConfigInst t t1 t2""",

    "gfpn.cubic.CubicExt.Insts.CoreOpsArithMulCubicExtCubicExt.mul": """\
  := do
  let v0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 rhs.c0
  let v1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 rhs.c1
  let v2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c2 rhs.c2
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c1 self.c2
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      rhs.c1 rhs.c2
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t t1
  let t3 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t2 v1
  let x ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t3 v2
  let t4 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c0 self.c1
  let t5 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      rhs.c0 rhs.c1
  let t6 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t4 t5
  let t7 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t6 v0
  let y ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t7 v1
  let t8 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c0 self.c2
  let t9 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      rhs.c0 rhs.c2
  let t10 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t8 t9
  let t11 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t10 v0
  let t12 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      t11 v1
  let z ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t12 v2
  let t13 ← ext_configExtConfigInst.mul_by_non_residue x
  let c0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      v0 t13
  let t14 ← ext_configExtConfigInst.mul_by_non_residue v2
  let c1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      y t14
  gfpn.cubic.CubicExt.new ext_configExtConfigInst c0 c1 z""",

    "gfpn.quadratic.QuadraticExt.conjugate": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst self.c0 t""",

    "gfpn.quadratic.QuadraticExt.norm": """\
  := do
  let t0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c1
  let t ← ext_configExtConfigInst.mul_by_non_residue t1
  ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
    t0 t""",

    "gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithAddQuadraticExtQuadraticExt.add": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c0 rhs.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c1 rhs.c1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst t t1""",

    "gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithSubQuadraticExtQuadraticExt.sub": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      self.c0 rhs.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      self.c1 rhs.c1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst t t1""",

    "gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithNegQuadraticExt.neg": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      self.c1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst t t1""",

    "gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt.call_once": """\
  := do
  let (t, t1, t2) := c
  let t3 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t tupled_args
  let t4 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t1 tupled_args
  let t5 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t2 tupled_args
  gfpn.cubic.CubicExt.new ext_configExtConfigInst t3 t4 t5""",

    "gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExt.inv": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c2
  let t2 ← ext_configExtConfigInst.mul_by_non_residue t1
  let s0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t t2
  let t3 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c2 self.c2
  let t4 ← ext_configExtConfigInst.mul_by_non_residue t3
  let t5 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c1
  let s1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t4 t5
  let t6 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c1
  let t7 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c2
  let s2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t6 t7
  let t8 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 s0
  let t9 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c2 s1
  let t10 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 s2
  let t11 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      t9 t10
  let t12 ← ext_configExtConfigInst.mul_by_non_residue t11
  let norm ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      t8 t12
  let o ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.inv norm
  core.option.Option.map
    (gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt
    ext_configExtConfigInst) o (s0, s1, s2)""",

    "gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once": """\
  := do
  let t ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      c.c0 tupled_args
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      c.c1 tupled_args
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithNegInst.neg
      t1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst t t2""",

    "gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv": """\
  := do
  let t0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 self.c0
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 self.c1
  let t ← ext_configExtConfigInst.mul_by_non_residue t1
  let norm ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t0 t
  let o ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.inv norm
  core.option.Option.map
    (gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExt.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt
    ext_configExtConfigInst) o self""",

    "gfpn.quadratic.QuadraticExt.Insts.CoreOpsArithMulQuadraticExtQuadraticExt.mul": """\
  := do
  let v0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c0 rhs.c0
  let v1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      self.c1 rhs.c1
  let t ← ext_configExtConfigInst.mul_by_non_residue v1
  let c0 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      v0 t
  let t1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      self.c0 self.c1
  let t2 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithAddInst.add
      rhs.c0 rhs.c1
  let t3 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithMulInst.mul
      t1 t2
  let t4 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t3 v0
  let c1 ←
    ext_configExtConfigInst.fieldtraitsConstFieldInst.FiniteFieldInst.coreopsarithSubInst.sub
      t4 v1
  gfpn.quadratic.QuadraticExt.new ext_configExtConfigInst c0 c1""",
}


def patch_sorrys(funs_path: str) -> None:
    with open(funs_path, "r") as f:
        lines = f.readlines()

    patched = 0
    i = 0
    while i < len(lines):
        # Look for "  sorry\n" preceded by "  := do\n"
        if lines[i].rstrip() == "  sorry" and i >= 1 and lines[i - 1].rstrip() == "  := do":
            # Walk backwards to find the function name (line containing "def")
            func_name = None
            for j in range(i - 2, max(i - 20, -1), -1):
                # The function name is the first non-whitespace token after "def"
                line = lines[j]
                if line.startswith("def "):
                    func_name = line[4:].strip().split()[0].split("(")[0].split("{")[0]
                    break
                elif line.startswith("def\n"):
                    # Name is on the next line
                    func_name = lines[j + 1].strip().split()[0].split("(")[0].split("{")[0]
                    break

            if func_name and func_name in PATCHES:
                # Replace "  := do\n  sorry" with the patched body
                body_lines = PATCHES[func_name].split("\n")
                # Replace lines[i-1] and lines[i] with the body
                lines[i - 1 : i + 1] = [line + "\n" for line in body_lines]
                patched += 1
                # Adjust index for inserted lines
                i += len(body_lines) - 2
        i += 1

    with open(funs_path, "w") as f:
        f.writelines(lines)

    remaining = sum(1 for line in lines if line.rstrip() == "  sorry")
    if remaining > 0:
        print(f"Patched {patched} sorry(s), {remaining} remain (expected for opaque gfpn functions)")
    else:
        print(f"Patched {patched} sorry(s), none remain")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <Funs.lean>", file=sys.stderr)
        sys.exit(1)
    patch_sorrys(sys.argv[1])
