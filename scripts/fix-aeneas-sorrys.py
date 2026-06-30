#!/usr/bin/env python3
"""Replace Aeneas-generated 'sorry' function bodies with known-good implementations.

Aeneas (5220259c) cannot translate certain gfpn function bodies from the LLBC
produced by Charon (487f0320). The error is "Assertion failed: new value doesn't
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

    # Wide variant (Aeneas >= 5fc8fdf2 emits the Wide suffix)
    "gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt.call_once": """\
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

    # Wide variant (Aeneas >= 5fc8fdf2 emits the Wide suffix)
    "gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv": """\
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
    (gfpn.cubic.FiniteFieldCubicExtClause0_Clause0_Clause0_CharacteristicCubicExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldCubicExt
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

    # Wide variants (Aeneas >= 5fc8fdf2 emits the Wide suffix)
    "gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt.call_once": """\
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

    "gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv": """\
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
    (gfpn.quadratic.FiniteFieldQuadraticExtClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.inv.closure.Insts.CoreOpsFunctionFnOnceTupleClause0_BaseFieldQuadraticExt
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

OPAQUE_DEFS = {
    # Iterator/debug/batch-specialized support code pulled in by the narrowed
    # start set but outside the field-arithmetic proof obligations.
    "core.iter.adapters.zip.Zip.Insts.CoreIterTraitsIteratorIteratorPair",
    "core.slice.iter.IterMut.Insts.CoreIterTraitsIteratorIteratorMutAT",
    "gfp.specialized.PrimeShape.Insts.CoreFmtDebug.fmt",
    "gfp.specialized.batch_dot_mersenne31_loop.body",
    "gfp.specialized.batch_dot_mersenne31",
    "gfp.specialized.GoldilocksFp.Insts.CoreOpsArithAddAssignShared0GoldilocksFp",
    "gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.cardinality_log2_hint",
    "gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.characteristic",
    "gfp.specialized.GoldilocksFp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.cardinality_log2_hint",
    "gfp.specialized.GoldilocksFp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128.characteristic",
    "gfpn.cubic.CubicExtWide.Insts.CoreCmpEq",
    # Aeneas 5220259c + rustc nightly-2026-06-01: the derived `Eq` impl now
    # emits an `assert_fields_are_eq` projection on the unapplied
    # `ExtConfig → core.cmp.Eq` function for the parametrised gfpn types, which
    # does not type-check. The Display `fmt` impl desugars `?` to
    # `core.result.Result.Insts.CoreOpsTry_traitTry.branch`, a constant the
    # current Aeneas Lean Std backend does not provide. The `ConstField for
    # CubicExt` dictionary's `FiniteFieldInst` field fails trait-impl
    # resolution (`Could not find: trait_impl_id`). None are arithmetic proof
    # targets — the proofs use the separate `.order`/`.zero`/`.one` sub-defs —
    # so opaque the offending instance/fmt dictionaries.
    "gfpn.cubic.CubicExt.Insts.CoreCmpEq",
    "gfpn.cubic.CubicExt.Insts.CoreFmtDisplay.fmt",
    "gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide",
    "gfpn.quadratic.QuadraticExt.Insts.CoreCmpEq",
    "gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsConstFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide",
    "Shared0CubicExt.Insts.CoreOpsArithNegCubicExt",
    "gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide.cardinality_log2_hint",
    "gfpn.cubic.CubicExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicCubicExtWide.characteristic",
    "gfpn.quadratic.QuadraticExtWide.Insts.CoreCmpEq",
    "Shared0QuadraticExt.Insts.CoreOpsArithNegQuadraticExt",
    "gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.cardinality_log2_hint",
    "gfpn.quadratic.QuadraticExt.Insts.Gf2_coreFieldTraitsFiniteFieldClause0_Clause0_Clause0_CharacteristicQuadraticExtWide.characteristic",
}


def _def_name_at(lines: list[str], i: int) -> str | None:
    stripped = lines[i].strip()
    if stripped.startswith("def "):
        return stripped[4:].split()[0].split("(")[0].split("{")[0]
    if stripped == "def" and i + 1 < len(lines):
        return lines[i + 1].strip().split()[0].split("(")[0].split("{")[0]
    return None


def opaque_unsupported_defs(lines: list[str]) -> tuple[list[str], int]:
    patched = 0
    i = 0
    out: list[str] = []
    while i < len(lines):
        name = _def_name_at(lines, i)
        if name not in OPAQUE_DEFS:
            out.append(lines[i])
            i += 1
            continue

        block: list[str] = []
        while i < len(lines):
            if i != 0 and (lines[i].startswith("/-- ") or lines[i].startswith("end ")):
                break
            block.append(lines[i])
            i += 1

        body_idx = next((idx for idx, line in enumerate(block) if ":=" in line), None)
        if body_idx is None:
            out.extend(block)
            continue

        prefix = block[body_idx].split(":=", 1)[0]
        out.extend(block[:body_idx])
        out.append(prefix + ":= by\n")
        out.append("  sorry\n")
        patched += 1

    return out, patched


def patch_trait_default_fields(lines: list[str]) -> tuple[list[str], int]:
    text = "".join(lines)
    patched = 0
    replacements = [
        (
            r"WINOGRAD_THRESHOLD := field\.traits\.FiniteField\.WINOGRAD_THRESHOLD\.default\n"
            r"    \(?[^\n]+\n(?:    [^\n]+\n)?",
            "WINOGRAD_THRESHOLD := ok 32#usize\n",
        ),
        (
            r"TRI_BASE_THRESHOLD := field\.traits\.FiniteField\.TRI_BASE_THRESHOLD\.default\n"
            r"    \(?[^\n]+\n(?:    [^\n]+\n)?",
            # Mirrors the Rust SSOT in crates/gf2-core/src/field/traits.rs.
            # Updated to 8 by jit:73ec5da3 R3 after the empirical sweep.
            "TRI_BASE_THRESHOLD := ok 8#usize\n",
        ),
        (
            r"theorem_4_operand_bound :=\n"
            r"    [^\n]+\.theorem_4_operand_bound\n"
            r"(?:    [^\n]+\n)?",
            "theorem_4_operand_bound := ok 0#u128\n",
        ),
    ]
    for pattern, replacement in replacements:
        text, count = re.subn(pattern, replacement, text)
        patched += count
    return text.splitlines(keepends=True), patched


def patch_sorrys(funs_path: str) -> None:
    with open(funs_path, "r") as f:
        lines = f.readlines()

    lines, opaque_patched = opaque_unsupported_defs(lines)
    lines, field_patched = patch_trait_default_fields(lines)

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
        print(
            f"Patched {patched} sorry(s), opaqued {opaque_patched} unsupported def(s), "
            f"patched {field_patched} trait default field(s), "
            f"{remaining} remain (expected for opaque gfpn functions)"
        )
    else:
        print(
            f"Patched {patched} sorry(s), opaqued {opaque_patched} unsupported def(s), "
            f"patched {field_patched} trait default field(s), none remain"
        )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <Funs.lean>", file=sys.stderr)
        sys.exit(1)
    patch_sorrys(sys.argv[1])
