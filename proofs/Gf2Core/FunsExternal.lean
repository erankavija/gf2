-- External models for the selected gf2-core extraction.
-- Wrapping negation and U128 addition are concrete; other externals are trusted.
module
public import Aeneas
public import Gf2Core.Types
@[expose] public section
open Aeneas Aeneas.Std Result ControlFlow Error
set_option linter.dupNamespace false
set_option linter.hashCommand false
set_option linter.unusedVariables false
set_option linter.style.whitespace false
set_option linter.style.setOption false
set_option linter.style.longLine false
set_option linter.internalConstructors false

/- You can set the `maxHeartbeats` value with the `-max-heartbeats` CLI option -/
set_option maxHeartbeats 1000000

/- You can set the `maxRecDepth` value with the `-max-recdepth` CLI option -/
set_option maxRecDepth 2048
open gf2_core

/-- [core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::field]:
    Source: '/rustc/library/core/src/fmt/builders.rs', lines 156:4-156:76
    Name pattern: [core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::field]
    Visibility: public -/
@[rust_fun
  "core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::field"]
axiom core.fmt.builders.DebugStruct.field
  :
  core.fmt.builders.DebugStruct → Str → Dyn (fun _dyn => core.fmt.Debug
    _dyn) → Result (core.fmt.builders.DebugStruct ×
    (core.fmt.builders.DebugStruct → core.fmt.builders.DebugStruct) ×
    (core.fmt.builders.DebugStruct → core.fmt.builders.DebugStruct →
    (core.fmt.builders.DebugStruct × core.fmt.builders.DebugStruct)))

/-- [core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::finish]:
    Source: '/rustc/library/core/src/fmt/builders.rs', lines 268:4-268:43
    Name pattern: [core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::finish]
    Visibility: public -/
@[rust_fun
  "core::fmt::builders::{core::fmt::builders::DebugStruct<'a, 'b>}::finish"]
axiom core.fmt.builders.DebugStruct.finish
  :
  core.fmt.builders.DebugStruct → Result ((core.result.Result Unit
    core.fmt.Error) × core.fmt.builders.DebugStruct ×
    (core.fmt.builders.DebugStruct → core.fmt.builders.DebugStruct))

/-- [core::fmt::{core::fmt::Formatter<'a>}::debug_struct]:
    Source: '/rustc/library/core/src/fmt/mod.rs', lines 2448:4-2448:76
    Name pattern: [core::fmt::{core::fmt::Formatter<'a>}::debug_struct]
    Visibility: public -/
@[rust_fun "core::fmt::{core::fmt::Formatter<'a>}::debug_struct"]
axiom core.fmt.Formatter.debug_struct
  :
  core.fmt.Formatter → Str → Result (core.fmt.builders.DebugStruct ×
    (core.fmt.builders.DebugStruct → core.fmt.Formatter))

/-- [core::hash::impls::{impl core::hash::Hash for u64}::hash]:
    Source: '/rustc/library/core/src/hash/mod.rs', lines 813:16-813:56; this code is generated from a macro invocation at: '/rustc/library/core/src/hash/mod.rs', lines 831:4-844:5
    Name pattern: [core::hash::impls::{core::hash::Hash<u64>}::hash]
    Visibility: public -/
@[rust_fun "core::hash::impls::{core::hash::Hash<u64>}::hash"]
axiom U64.Insts.CoreHashHash.hash
  {H : Type _} (HasherInst : core.hash.Hasher H) : Std.U64 → H → Result H

/-- [core::num::{u64}::wrapping_neg]:
    Source: '/rustc/library/core/src/num/uint_macros.rs', lines 2890:8-2890:47; this code is generated from a macro invocation at: '/rustc/library/core/src/num/mod.rs', lines 1324:4-1348:5
    Name pattern: [core::num::{u64}::wrapping_neg]
    Visibility: public -/
@[rust_fun "core::num::{u64}::wrapping_neg"]
def core.num.U64.wrapping_neg (x : Std.U64) : Result Std.U64 :=
  ok (UScalar.wrapping_sub (⟨0#64⟩ : Std.U64) x)

/-- [core::num::{u64}::trailing_zeros]:
    Source: '/rustc/library/core/src/num/uint_macros.rs', lines 177:8-177:48; this code is generated from a macro invocation at: '/rustc/library/core/src/num/mod.rs', lines 1324:4-1348:5
    Name pattern: [core::num::{u64}::trailing_zeros]
    Visibility: public -/
@[rust_fun "core::num::{u64}::trailing_zeros"]
axiom core.num.U64.trailing_zeros : Std.U64 → Result Std.U32

/-- [core::num::{u128}::saturating_mul]:
    Source: '/rustc/library/core/src/num/uint_macros.rs', lines 2614:8-2614:60; this code is generated from a macro invocation at: '/rustc/library/core/src/num/mod.rs', lines 1356:4-1382:5
    Name pattern: [core::num::{u128}::saturating_mul]
    Visibility: public -/
@[rust_fun "core::num::{u128}::saturating_mul"]
axiom core.num.U128.saturating_mul : Std.U128 → Std.U128 → Result Std.U128

/-- [core::ops::arith::{impl core::ops::arith::Add<u128, u128> for u128}::add]:
    Source: '/rustc/library/core/src/ops/arith.rs', lines 104:12-104:41; this code is generated from a macro invocation at: '/rustc/library/core/src/ops/arith.rs', lines 113:0-113:82
    Name pattern: [core::ops::arith::{core::ops::arith::Add<u128, u128, u128>}::add]
    Visibility: public -/
@[rust_fun "core::ops::arith::{core::ops::arith::Add<u128, u128, u128>}::add"]
def U128.Insts.CoreOpsArithAddU128U128.add (x y : Std.U128) : Result Std.U128 :=
  UScalar.add x y

/-- [core::ops::arith::{impl core::ops::arith::AddAssign<u128> for u128}::add_assign]:
    Source: '/rustc/library/core/src/ops/arith.rs', lines 790:12-790:47; this code is generated from a macro invocation at: '/rustc/library/core/src/ops/arith.rs', lines 799:0-799:89
    Name pattern: [core::ops::arith::{core::ops::arith::AddAssign<u128, u128>}::add_assign]
    Visibility: public -/
@[rust_fun
  "core::ops::arith::{core::ops::arith::AddAssign<u128, u128>}::add_assign"]
def U128.Insts.CoreOpsArithAddAssignU128.add_assign (x y : Std.U128) : Result Std.U128 :=
  UScalar.add x y

/-- [core::option::{core::option::Option<T>}::and_then]:
    Source: '/rustc/library/core/src/option.rs', lines 1539:4-1541:61
    Name pattern: [core::option::{core::option::Option<@T>}::and_then]
    Visibility: public -/
@[rust_fun "core::option::{core::option::Option<@T>}::and_then"]
axiom core.option.Option.and_then
  {T : Type _} {U : Type _} {F : Type _} (opsfunctionFnOnceFTupleTOptionInst :
  core.ops.function.FnOnce F T (Option U)) :
  Option T → F → Result (Option U)

/-- [gf2_core::field::traits::FiniteField::zero_hint]:
    Source: 'crates/gf2-core/src/field/traits.rs', lines 79:4-81:5
    Visibility: public -/
@[trait_default]
axiom field.traits.FiniteField.zero_hint.default
  {Self : Type _} {Clause0_Characteristic : Type _} {Clause0_Wide : Type _}
  (FiniteFieldInst : field.traits.FiniteField Self Clause0_Characteristic
  Clause0_Wide) :
  Result (Option Self)

/-- [gf2_core::field::traits::FiniteField::cardinality_log2_hint]:
    Source: 'crates/gf2-core/src/field/traits.rs', lines 89:4-91:5
    Visibility: public -/
@[trait_default]
axiom field.traits.FiniteField.cardinality_log2_hint.default
  {Self : Type _} {Clause0_Characteristic : Type _} {Clause0_Wide : Type _}
  (FiniteFieldInst : field.traits.FiniteField Self Clause0_Characteristic
  Clause0_Wide) :
  Result (Option Std.U32)

/-- [gf2_core::field::traits::FiniteField::mul_product_sum_wide]:
    Source: 'crates/gf2-core/src/field/traits.rs', lines 121:4-123:5
    Visibility: public -/
@[trait_default]
axiom field.traits.FiniteField.mul_product_sum_wide.default
  {Self : Type _} {Clause0_Characteristic : Type _} {Clause0_Wide : Type _}
  (FiniteFieldInst : field.traits.FiniteField Self Clause0_Characteristic
  Clause0_Wide) :
  Self → Self → Result Clause0_Wide

/-- [gf2_core::field::traits::FiniteField::reduce_product_sum_wide]:
    Source: 'crates/gf2-core/src/field/traits.rs', lines 140:4-142:5
    Visibility: public -/
@[trait_default]
axiom field.traits.FiniteField.reduce_product_sum_wide.default
  {Self : Type _} {Clause0_Characteristic : Type _} {Clause0_Wide : Type _}
  (FiniteFieldInst : field.traits.FiniteField Self Clause0_Characteristic
  Clause0_Wide) :
  Clause0_Wide → Result Self

/-- [gf2_core::field::traits::FiniteField::theorem_4_operand_bound]:
    Source: 'crates/gf2-core/src/field/traits.rs', lines 385:4-387:5
    Visibility: public -/
@[trait_default]
def field.traits.FiniteField.theorem_4_operand_bound.default
  {Self : Type _} {Clause0_Characteristic : Type _} {Clause0_Wide : Type _}
  (FiniteFieldInst : field.traits.FiniteField Self Clause0_Characteristic
  Clause0_Wide) :
  Result Std.U128 := ok 340282366920938463463374607431768211455#u128

/-- [gf2_core::gfp::specialized::batch_mul_mersenne31]:
    Source: 'crates/gf2-core/src/gfp/specialized.rs', lines 270:0-285:1
    Visibility: public -/
axiom gfp.specialized.batch_mul_mersenne31
  :
  Slice Std.U32 → Slice Std.U32 → Slice Std.U32 → Result (Slice Std.U32)

/-- [gf2_core::gfp::specialized::batch_mul_add_mersenne31]:
    Source: 'crates/gf2-core/src/gfp/specialized.rs', lines 296:0-318:1
    Visibility: public -/
axiom gfp.specialized.batch_mul_add_mersenne31
  :
  Slice Std.U32 → Slice Std.U32 → Slice Std.U32 → Result (Slice Std.U32)

/-- [gf2_core::gfp::specialized::batch_dot_mersenne31]:
    Source: 'crates/gf2-core/src/gfp/specialized.rs', lines 329:0-349:1
    Visibility: public -/
axiom gfp.specialized.batch_dot_mersenne31
  : Slice Std.U32 → Slice Std.U32 → Result Std.U32

/-- [gf2_core::gfpn::cubic::{impl core::fmt::Debug for gf2_core::gfpn::cubic::CubicExtWide<W>}::fmt]:
    Source: 'crates/gf2-core/src/gfpn/cubic.rs', lines 93:4-99:5
    Visibility: public -/
axiom gfpn.cubic.CubicExtWide.Insts.CoreFmtDebug.fmt
  {W : Type _} (corefmtDebugInst : core.fmt.Debug W) :
  gfpn.cubic.CubicExtWide W → core.fmt.Formatter → Result
    ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter)

/-- [gf2_core::gfpn::quadratic::{impl core::fmt::Debug for gf2_core::gfpn::quadratic::QuadraticExtWide<W>}::fmt]:
    Source: 'crates/gf2-core/src/gfpn/quadratic.rs', lines 83:4-88:5
    Visibility: public -/
axiom gfpn.quadratic.QuadraticExtWide.Insts.CoreFmtDebug.fmt
  {W : Type _} (corefmtDebugInst : core.fmt.Debug W) :
  gfpn.quadratic.QuadraticExtWide W → core.fmt.Formatter → Result
    ((core.result.Result Unit core.fmt.Error) × core.fmt.Formatter)
