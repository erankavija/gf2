//! The gf2 side of every cell (jit:c7113c5a).
//!
//! Each operation calls the production entry point a consumer reaches today:
//! the dispatched fixed-size wide kernels of `gf2_kernels_simd::gf2m_wide` for
//! 4-limb and 9-limb products (the kernels `Gf2mWide::mul_ref` reaches through
//! gf2-core's cached detection), `gf2_core::gf2m::wide::clmul_wide_slice` for
//! every other length and for the public-API path, the raw-batch lane
//! `gf2_kernels_simd::gf2m::detect` publishes for independent 64x64 products,
//! and `FieldVec::simd_dot_product` for the whole-consumer GF(2^8) dot product.
//! Nothing in `crates/` changes; this arm only observes which path the current
//! code selects at run time.

use gf2_core::field::FieldVec;
use gf2_core::gf2m::barrett::{clmul, BarrettReducer};
use gf2_core::gf2m::wide::clmul_wide_slice;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_kernels_simd::gf2m::ClmulBatchFn;
use gf2_kernels_simd::gf2m_wide::{ClmulWide256Fn, ClmulWide571Fn};

use crate::wide_field::reduction_probe_ns;
use crate::{
    amortised_probe_ns, probe_ns, Backend, Bank, Case, Conversion, DOT_FIELD_DEGREE, DOT_FIELD_POLY,
};

/// Word counts the schoolbook long-product path is instantiated for.
///
/// `clmul_wide_slice` is const-generic in the operand word count, so an arm can
/// only reach the lengths it names. The list carries every cell size plus the
/// 1, 63, 64, 65 and 127/128 word boundaries the validator exercises.
macro_rules! for_each_length {
    ($words:expr, $body:ident) => {
        match $words {
            1 => $body!(1),
            2 => $body!(2),
            3 => $body!(3),
            4 => $body!(4),
            5 => $body!(5),
            8 => $body!(8),
            9 => $body!(9),
            16 => $body!(16),
            63 => $body!(63),
            64 => $body!(64),
            65 => $body!(65),
            127 => $body!(127),
            128 => $body!(128),
            256 => $body!(256),
            1024 => $body!(1024),
            2048 => $body!(2048),
            other => panic!("gf2 arm is not instantiated for {other}-word operands"),
        }
    };
}

/// Runs the schoolbook long product for any instantiated word count.
pub fn schoolbook(words: usize, a: &[u64], b: &[u64], out: &mut [u64]) {
    out.fill(0);
    macro_rules! call {
        ($n:literal) => {{
            let a: &[u64; $n] = a
                .try_into()
                .expect("operand length matches the dispatch arm");
            let b: &[u64; $n] = b
                .try_into()
                .expect("operand length matches the dispatch arm");
            clmul_wide_slice::<$n>(a, b, out)
        }};
    }
    for_each_length!(words, call)
}

/// The raw-batch lane `gf2_kernels_simd::gf2m::detect` published on this host.
///
/// The lane tag is the one the detection itself returns beside the function
/// pointer, so the arm reports the selected lane rather than re-deriving it
/// from CPU flags.
pub fn batch_path(fns: Option<&gf2_kernels_simd::gf2m::Gf2mFns>) -> String {
    match fns.and_then(|fns| fns.clmul_batch_path) {
        Some(lane) => format!("clmul_batch:{lane}"),
        None => "clmul_batch:unavailable".to_owned(),
    }
}

/// Which long-product path this arm exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolyPath {
    /// Dispatched wide kernel where one exists, schoolbook otherwise. This is
    /// what a `Gf2mWide` field consumer reaches today.
    Dispatched,
    /// `clmul_wide_slice` unconditionally: the public long-product API.
    Schoolbook,
}

impl PolyPath {
    /// Reads the path from `GF2_POLY_PATH`; absent or `dispatched` selects the
    /// dispatched path.
    pub fn from_environment() -> Self {
        match std::env::var("GF2_POLY_PATH").as_deref() {
            Ok("schoolbook") => PolyPath::Schoolbook,
            Ok("dispatched") | Err(_) => PolyPath::Dispatched,
            Ok(other) => panic!("GF2_POLY_PATH={other:?} is not dispatched or schoolbook"),
        }
    }
}

/// The two GF(2^8) vectors the whole-consumer dot-product cell multiplies.
type DotVectors = (FieldVec<Gf2mElement>, FieldVec<Gf2mElement>);

/// gf2 arm state: the dispatched kernels this host resolved, plus the
/// whole-consumer field vectors when the case needs them.
pub struct Gf2Backend {
    path: PolyPath,
    wide256: Option<(ClmulWide256Fn, &'static str)>,
    wide571: Option<(ClmulWide571Fn, &'static str)>,
    batch: Option<ClmulBatchFn>,
    batch_name: String,
    products: Vec<u128>,
    dot: Option<DotVectors>,
    selected: String,
}

impl Gf2Backend {
    fn build_dot_vectors(case: &Case, bank: &Bank) -> DotVectors {
        let Case::Gf2mDot { count, .. } = case else {
            unreachable!("dot vectors are built for the dot case only")
        };
        let field = Gf2mField::new(DOT_FIELD_DEGREE, DOT_FIELD_POLY);
        let mask = (1u64 << DOT_FIELD_DEGREE) - 1;
        let left: Vec<Gf2mElement> = (0..*count)
            .map(|index| field.element(bank.a[index] & mask))
            .collect();
        let right: Vec<Gf2mElement> = (0..*count)
            .map(|index| field.element(bank.b[index] & mask))
            .collect();
        (FieldVec::from(left), FieldVec::from(right))
    }
}

impl Backend for Gf2Backend {
    fn selected_path(&self) -> String {
        self.selected.clone()
    }

    fn create(case: &Case) -> Self {
        let path = PolyPath::from_environment();
        let wide = gf2_kernels_simd::gf2m_wide::detect_wide();
        let gf2m = gf2_kernels_simd::gf2m::detect();
        let batch = gf2m.as_ref().and_then(|fns| fns.clmul_batch_fn);
        let batch_name = batch_path(gf2m.as_ref());
        let selected = match case {
            Case::PolyMul { words, .. } => match (path, words, wide.as_ref()) {
                (PolyPath::Dispatched, 4, Some(fns)) => {
                    format!("wide256:{}", fns.wide256.name)
                }
                (PolyPath::Dispatched, 9, Some(fns)) => {
                    format!("wide571:{}", fns.wide571.name)
                }
                _ => "clmul_wide_slice:schoolbook".to_owned(),
            },
            Case::ClmulBatch { .. } => batch_name.clone(),
            Case::Gf2mDot { .. } => "FieldVec::simd_dot_product".to_owned(),
        };
        let products = match case {
            Case::ClmulBatch { count, .. } => vec![0u128; *count],
            _ => Vec::new(),
        };
        Self {
            path,
            wide256: wide
                .as_ref()
                .map(|fns| (fns.wide256.clmul, fns.wide256.name)),
            wide571: wide
                .as_ref()
                .map(|fns| (fns.wide571.clmul, fns.wide571.name)),
            batch,
            batch_name,
            products,
            dot: None,
            selected,
        }
    }

    fn run(&mut self, case: &Case, bank: &mut Bank) {
        match case {
            Case::PolyMul { words, .. } => match (self.path, *words, self.wide256, self.wide571) {
                (PolyPath::Dispatched, 4, Some((kernel, _)), _) => {
                    let a: &[u64; 4] = (&bank.a[..]).try_into().expect("4-word operand");
                    let b: &[u64; 4] = (&bank.b[..]).try_into().expect("4-word operand");
                    let out: &mut [u64; 8] =
                        (&mut bank.out[..]).try_into().expect("8-word product");
                    kernel(a, b, out);
                }
                (PolyPath::Dispatched, 9, _, Some((kernel, _))) => {
                    let a: &[u64; 9] = (&bank.a[..]).try_into().expect("9-word operand");
                    let b: &[u64; 9] = (&bank.b[..]).try_into().expect("9-word operand");
                    let out: &mut [u64; 18] =
                        (&mut bank.out[..]).try_into().expect("18-word product");
                    kernel(a, b, out);
                }
                _ => schoolbook(*words, &bank.a, &bank.b, &mut bank.out),
            },
            Case::ClmulBatch { count, .. } => {
                let batch = self.batch.expect("the host resolved a batch clmul kernel");
                batch(&bank.a[..*count], &bank.b[..*count], &mut self.products);
                for (index, product) in self.products.iter().enumerate() {
                    bank.out[2 * index] = *product as u64;
                    bank.out[2 * index + 1] = (*product >> 64) as u64;
                }
            }
            Case::Gf2mDot { .. } => {
                let (left, right) = self
                    .dot
                    .get_or_insert_with(|| Self::build_dot_vectors(case, bank));
                bank.out[0] = left.simd_dot_product(right).value();
            }
        }
    }

    fn conversion(&mut self, case: &Case, bank: &mut Bank) -> Conversion {
        let dispatch_ns = probe_ns(|| {
            std::hint::black_box(gf2_kernels_simd::gf2m_wide::detect_wide());
            std::hint::black_box(gf2_kernels_simd::gf2m::detect());
        });
        match case {
            Case::PolyMul { words, .. } => Conversion {
                setup_ns: probe_ns(|| bank.out.fill(0)),
                unpack_ns: reduction_probe_ns(*words, bank, |field| self.run(case, field)),
                dispatch_ns,
                ..Conversion::default()
            },
            Case::ClmulBatch { count, .. } => {
                let count = *count;
                let unpack_ns = probe_ns(|| {
                    for index in 0..count {
                        bank.out[2 * index] = self.products[index] as u64;
                        bank.out[2 * index + 1] = (self.products[index] >> 64) as u64;
                    }
                });
                Conversion {
                    setup_ns: probe_ns(|| {
                        std::hint::black_box(&self.products);
                    }),
                    unpack_ns,
                    dispatch_ns,
                    ..Conversion::default()
                }
            }
            Case::Gf2mDot { .. } => {
                let mut vectors = None;
                let pack_ns = probe_ns(|| vectors = Some(Self::build_dot_vectors(case, bank)));
                let vectors = vectors.expect("the packing probe built both vectors");
                let mut reduced = None;
                let batch_fill_ns =
                    probe_ns(|| reduced = Some(vectors.0.simd_dot_product(&vectors.1)));
                let reduced = reduced.expect("the accumulation probe produced an element");
                // The reduction stage on its own, with the identical reducer and
                // clmul the gf2x arm composes, so the two arms' figures subtract
                // the same quantity from their measured calls.
                let reducer =
                    BarrettReducer::new(u128::from(DOT_FIELD_POLY), DOT_FIELD_DEGREE as u32);
                let accumulator = u128::from(reduced.value());
                let unpack_ns = amortised_probe_ns(|| {
                    std::hint::black_box(
                        reducer.reduce_with_clmul(std::hint::black_box(accumulator), clmul),
                    );
                });
                Conversion {
                    setup_ns: probe_ns(|| {
                        std::hint::black_box(Gf2mField::new(DOT_FIELD_DEGREE, DOT_FIELD_POLY));
                    }),
                    pack_ns,
                    unpack_ns,
                    batch_fill_ns,
                    dispatch_ns,
                }
            }
        }
    }
}

impl Gf2Backend {
    /// The batch-kernel identity this instance resolved, for the validator.
    pub fn batch_name(&self) -> &str {
        &self.batch_name
    }
}
