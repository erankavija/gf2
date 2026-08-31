"""Regression tests for the gf2-algebra Aeneas postprocessor."""

from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import unittest


SCRIPT = Path(__file__).parents[1] / "fix-aeneas-gf2algebra.py"
SPEC = spec_from_file_location("fix_aeneas_gf2algebra", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


PREFIX = "gf2_core.gfp.Fp.Insts."
OPAQUE_WRAPPERS = (
    "CoreOpsArithAddFpFp",
    "CoreOpsArithSubFpFp",
    "CoreOpsArithMulFpFp",
    "CoreOpsArithDivFpFp",
    "CoreOpsArithAddAssignFp",
    "CoreOpsArithAddAssignShared0Fp",
    "CoreOpsArithAddShared0FpFp",
    "CoreOpsArithSubShared0FpFp",
    "CoreOpsArithMulShared0FpFp",
    "CoreOpsArithDivShared0FpFp",
)
TRANSPARENT_WRAPPER = "CoreOpsArithNegFp"


FINITE_FIELD_DICTIONARY = """\
@[reducible, rust_trait_impl
  "gf2_core::field::traits::FiniteField<gf2_core::gfp::Fp<@P>, u64, u128>"]
def gf2_core.gfp.Fp.Insts.Gf2_coreFieldTraitsFiniteFieldU64U128 (P :
  Std.U64) : gf2_core.field.traits.FiniteField (gf2_core.gfp.Fp P) Std.U64
  Std.U128 := {
  characteristic := gf2_core.gfp.Fp.Insts.characteristic
}
"""


def wrapper(name: str) -> str:
    return f"""\
@[reducible, rust_trait_impl "fixture::{name}"]
def {PREFIX}{name} (P : Std.U64) :
  core.ops.arith.Add (gf2_core.gfp.Fp P) (gf2_core.gfp.Fp P)
  (gf2_core.gfp.Fp P) := {{
  add := {PREFIX}{name}.add
}}
"""


def valid_source() -> str:
    wrappers = "\n".join(wrapper(name) for name in OPAQUE_WRAPPERS)
    return f"{FINITE_FIELD_DICTIONARY}\n{wrappers}\n{wrapper(TRANSPARENT_WRAPPER)}"


class OpaqueFpWrapperTests(unittest.TestCase):
    def test_exact_allowlist_is_axiomatized(self) -> None:
        actual, replaced = MODULE.axiomatize_opaque_fp_wrappers(valid_source())

        self.assertEqual(replaced, 10)
        for name in OPAQUE_WRAPPERS:
            self.assertIn(f"axiom {PREFIX}{name}", actual)
            self.assertNotIn(f"def {PREFIX}{name}", actual)

    def test_finite_field_and_known_transparent_wrapper_survive(self) -> None:
        actual, _ = MODULE.axiomatize_opaque_fp_wrappers(valid_source())

        finite_field = f"{PREFIX}Gf2_coreFieldTraitsFiniteFieldU64U128"
        self.assertIn(f"def {finite_field}", actual)
        self.assertNotIn(f"axiom {finite_field}", actual)
        self.assertIn(f"def {PREFIX}{TRANSPARENT_WRAPPER}", actual)
        self.assertNotIn(f"axiom {PREFIX}{TRANSPARENT_WRAPPER}", actual)

    def test_missing_allowlisted_wrapper_fails(self) -> None:
        source = valid_source().replace(wrapper(OPAQUE_WRAPPERS[-1]), "", 1)

        with self.assertRaisesRegex(SystemExit, "missing.*CoreOpsArithDivShared0FpFp"):
            MODULE.axiomatize_opaque_fp_wrappers(source)

    def test_duplicate_allowlisted_wrapper_fails(self) -> None:
        source = valid_source() + wrapper(OPAQUE_WRAPPERS[0])

        with self.assertRaisesRegex(SystemExit, "duplicate.*CoreOpsArithAddFpFp"):
            MODULE.axiomatize_opaque_fp_wrappers(source)

    def test_unknown_core_ops_wrapper_fails(self) -> None:
        source = valid_source() + wrapper("CoreOpsArithPowFpFp")

        with self.assertRaisesRegex(SystemExit, "unexpected.*CoreOpsArithPowFpFp"):
            MODULE.axiomatize_opaque_fp_wrappers(source)


if __name__ == "__main__":
    unittest.main()
