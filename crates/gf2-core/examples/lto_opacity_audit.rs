//! Pins the `Fp65537Fns` and `ClmulWide256Fns` dispatch tables to concrete
//! instantiations inside `#[inline(never)] #[no_mangle]` wrappers, so that
//! `cargo asm --example lto_opacity_audit --features simd <wrapper>` dumps
//! their call sites.

#[cfg(feature = "simd")]
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
#[cfg(feature = "simd")]
use gf2_core::gfp::{Fp, SimdVecOps};

/// `x^256 + x^10 + x^5 + x^2 + 1`, the polynomial of `Gf2m256Config` in
/// `benches/gf2m_wide_mul.rs`.
#[cfg(feature = "simd")]
pub struct AuditCfg;

#[cfg(feature = "simd")]
impl Gf2mWideConfig<4> for AuditCfg {
    const M: usize = 256;
    // x^10 + x^5 + x^2 + 1 = 0x425; leading x^256 bit is implicit.
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
}

/// Call site of the `Fp65537Fns` dispatch behind `try_simd_mul_vec` on
/// `Fp<65537>`.
#[cfg(feature = "simd")]
#[inline(never)]
#[no_mangle]
pub fn lto_opacity_callsite_fp65537(a: &[Fp<65537>], b: &[Fp<65537>]) -> bool {
    <Fp<65537> as SimdVecOps>::try_simd_mul_vec(a, b).is_some()
}

/// Call site of the `ClmulWide256Fns` dispatch behind
/// `Gf2mWide<4, AuditCfg>::mul_ref`.
#[cfg(feature = "simd")]
#[inline(never)]
#[no_mangle]
pub fn lto_opacity_callsite_gf2m_wide256(
    a: &Gf2mWide<4, AuditCfg>,
    b: &Gf2mWide<4, AuditCfg>,
) -> Gf2mWide<4, AuditCfg> {
    a.mul_ref(b)
}

fn main() {
    #[cfg(feature = "simd")]
    {
        let a: Vec<Fp<65537>> = (0..32_u32).map(|x| Fp::<65537>::new(x as u64)).collect();
        let b: Vec<Fp<65537>> = (0..32_u32)
            .map(|x| Fp::<65537>::new((x + 1) as u64))
            .collect();
        let ok = lto_opacity_callsite_fp65537(&a, &b);
        println!("fp65537 callsite ran: simd_taken = {ok}");

        let x = Gf2mWide::<4, AuditCfg>::from_u64(0x1234);
        let y = Gf2mWide::<4, AuditCfg>::from_u64(0xbeef);
        let z = lto_opacity_callsite_gf2m_wide256(&x, &y);
        println!(
            "gf2m_wide256 callsite ran: z.words()[0] = {:#x}",
            z.words()[0]
        );
    }
    #[cfg(not(feature = "simd"))]
    {
        println!("simd feature disabled; nothing to audit");
    }
}
