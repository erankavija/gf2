"""Regression tests for the gf2-algebra Aeneas postprocessor."""

from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import unittest


SCRIPT = Path(__file__).parents[1] / "fix-aeneas-gf2algebra.py"
SPEC = spec_from_file_location("fix_aeneas_gf2algebra", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class OpaqueFpWrapperTests(unittest.TestCase):
    def test_plain_finite_field_dictionary_survives(self) -> None:
        source = """\
@[reducible, rust_trait_impl
  "gf2_core::field::traits::FiniteField<gf2_core::gfp::Fp<@P>, u64, u128>"]
def gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128 (P :
  Std.U64) : gf2_core.field.traits.FiniteField (gf2_core.gfp.Fp P) Std.U64
  Std.U128 := {
  characteristic := gf2_core.gfp.Fp.Insts.characteristic
}

@[reducible, rust_trait_impl
  "core::ops::arith::Add<gf2_core::gfp::Fp<@P>, gf2_core::gfp::Fp<@P>, gf2_core::gfp::Fp<@P>>"]
def gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp (P : Std.U64) :
  core.ops.arith.Add (gf2_core.gfp.Fp P) (gf2_core.gfp.Fp P)
  (gf2_core.gfp.Fp P) := {
  add := gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp.add
}
"""

        actual, replaced = MODULE.axiomatize_opaque_fp_wrappers(source)

        self.assertEqual(replaced, 1)
        self.assertIn(
            "def gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128",
            actual,
        )
        self.assertNotIn(
            "axiom gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128",
            actual,
        )
        self.assertIn(
            "axiom gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp", actual
        )


if __name__ == "__main__":
    unittest.main()
