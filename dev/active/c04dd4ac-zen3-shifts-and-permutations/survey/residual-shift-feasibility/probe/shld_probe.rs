//! Asks the toolchain directly whether it exposes a double-precision shift
//! intrinsic under BMI2. Compiled only by `capture-asm.sh`, which records
//! rustc's own answer; it is not part of the prototype crate.

#[cfg(target_arch = "x86_64")]
pub unsafe fn probe(hi: u64, lo: u64, b: i32) -> (u64, u64) {
    use core::arch::x86_64::{_shld_u64, _shrd_u64};
    (_shld_u64(hi, lo, b), _shrd_u64(hi, lo, b))
}
