//! LTO-opacity audit harness for the dispatch tables that have no
//! standalone callers in the lib at HEAD.
//!
//! Background: `Fp65537Fns` and `ClmulWide256Fns` are reached only through
//! generic public callers (`Fp<P>` SoA ops parameterised over `P`,
//! `Gf2mWide<N, Cfg>` parameterised over `N` and `Cfg`) which monomorphise
//! and inline at every concrete instantiation, leaving no top-level lib
//! symbol whose asm can be dumped via `cargo asm -p gf2-core --lib …`.
//!
//! This example pins each dispatch table to a concrete instantiation
//! inside a `#[no_mangle] pub extern "C"` wrapper. Building the example
//! and running
//!
//! ```bash
//! cargo asm -p gf2-core --example lto_opacity_audit \
//!     --features simd lto_opacity_callsite_fp65537 --simplify
//! cargo asm -p gf2-core --example lto_opacity_audit \
//!     --features simd lto_opacity_callsite_gf2m_wide256 --simplify
//! ```
//!
//! produces direct asm for the call sites the audit refers to.
//! Used by `dev/bench_results/2026-04-27-asm-audit.md`'s LTO-opacity
//! section to provide empirical evidence (matching the three
//! already-confirmed dispatch tables) rather than a structural-identity
//! inference.
//!
//! The wrappers are `#[inline(never)] #[no_mangle] pub fn` so they survive
//! monomorphisation as their own codegen unit and keep a stable symbol
//! name (Rust ABI; `extern "C"` is unnecessary since the wrappers are
//! never called across an FFI boundary).

#[cfg(feature = "simd")]
use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
#[cfg(feature = "simd")]
use gf2_core::gfp::{Fp, SimdVecOps};

/// Concrete `Gf2mWide<4>` configuration used by the wide-clmul callsite
/// wrapper. Polynomial: x^256 + x^10 + x^5 + x^2 + 1 — same irreducible
/// used by `Gf2m256Config` in `crates/gf2-core/benches/gf2m_wide_mul.rs`,
/// so the wrapper exercises the exact code path the production benches
/// target.
#[cfg(feature = "simd")]
pub struct AuditCfg;

#[cfg(feature = "simd")]
impl Gf2mWideConfig<4> for AuditCfg {
    const M: usize = 256;
    // x^10 + x^5 + x^2 + 1 = 0x425; leading x^256 bit is implicit.
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
}

/// `Fp65537Fns` call site: pins the `try_simd_mul_vec` dispatch on
/// `Fp<65537>` so the indirect call through
/// `OnceLock<Option<Fp65537Fns>>` → `batch_mul_fn` is observable in
/// asm. The wrapper is `#[no_mangle] pub extern "C"` to defeat
/// inlining into the example's `main`.
#[cfg(feature = "simd")]
#[inline(never)]
#[no_mangle]
pub fn lto_opacity_callsite_fp65537(a: &[Fp<65537>], b: &[Fp<65537>]) -> bool {
    <Fp<65537> as SimdVecOps>::try_simd_mul_vec(a, b).is_some()
}

/// `ClmulWide256Fns` call site: pins `Gf2mWide<4, AuditCfg>::mul_ref` at
/// a single instantiation so the dispatch through
/// `OnceLock<Option<ClmulWide256Fns>>` → `clmul` is observable in asm.
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
