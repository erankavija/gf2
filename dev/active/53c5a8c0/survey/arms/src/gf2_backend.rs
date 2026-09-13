//! The gf2 side of every cell (jit:53c5a8c0).
//!
//! Each operation calls a production entry point a consumer reaches today. The
//! crossover cases carry two gf2 paths, selected by `GF2_CROSSOVER_PATH`, so
//! one executable serves both arms of a crossover cell and a measured ratio
//! attributes to the entry point rather than to two builds:
//!
//! * [`CrossoverPath::Element`] — the per-element path: the single carry-less
//!   multiply of the default `gf2m::detect` bundle, `FieldVec::dot_product`,
//!   and `Gf2mElement` multiplication one pair at a time.
//! * [`CrossoverPath::Batch`] — the batched path: the raw-batch kernel of the
//!   same bundle, `FieldVec::simd_dot_product`, and `gf2m::batch::batch_mul`.
//!
//! The long-product and wide-field cases have one gf2 path each: the public
//! `clmul_wide`, the owned product that routes through the crate's capability
//! dispatch, and `Gf2mWide::mul_ref`, which composes that dispatch with
//! `BarrettReducerWide`. Nothing in `crates/` changes; these arms only observe
//! which path the current code selects at run time.

use gf2_core::field::FieldVec;
use gf2_core::gf2m::batch::batch_mul;
use gf2_core::gf2m::wide::clmul_wide;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_kernels_simd::gf2m::{ClmulBatchFn, ClmulFn};

use crate::wide_field::{reduction_probe_ns, Wide256, Wide571, WideReducer};
use crate::{
    field_element_word, probe_ns, Backend, Bank, Case, Conversion, FIELD_DEGREE, FIELD_POLY,
};

/// Word counts the long-product path is instantiated for.
///
/// `clmul_wide` is const-generic in the operand and product word counts, so an
/// arm can only reach the lengths it names. Each entry pairs a width with its
/// doubled product width; the list carries every cell size plus the 1, 63, 64,
/// 65 and 127/128 word boundaries the validator exercises.
///
/// Each arm calls a separate `#[inline(never)]` monomorphisation rather than
/// inlining every width into one body. A single body would carry the largest
/// width's product array in its stack frame, and a frame that size makes every
/// call — the four-word one included — pay a stack probe that has nothing to
/// do with the product being timed. One call instruction per product is the
/// cost that replaces it, and the external arm pays the same shape through its
/// own library call.
macro_rules! for_each_length {
    ($words:expr, $body:ident, $($argument:expr),*) => {
        match $words {
            1 => $body::<1, 2>($($argument),*),
            2 => $body::<2, 4>($($argument),*),
            3 => $body::<3, 6>($($argument),*),
            4 => $body::<4, 8>($($argument),*),
            5 => $body::<5, 10>($($argument),*),
            8 => $body::<8, 16>($($argument),*),
            9 => $body::<9, 18>($($argument),*),
            16 => $body::<16, 32>($($argument),*),
            32 => $body::<32, 64>($($argument),*),
            63 => $body::<63, 126>($($argument),*),
            64 => $body::<64, 128>($($argument),*),
            65 => $body::<65, 130>($($argument),*),
            127 => $body::<127, 254>($($argument),*),
            128 => $body::<128, 256>($($argument),*),
            256 => $body::<256, 512>($($argument),*),
            other => panic!("the gf2 arm is not instantiated for {other}-word operands"),
        }
    };
}

/// The public owned product at one width, discarded through `black_box`.
#[inline(never)]
fn owned_product<const N: usize, const M: usize>(a: &[u64], b: &[u64]) {
    let a: &[u64; N] = a.try_into().expect("operand length matches the arm");
    let b: &[u64; N] = b.try_into().expect("operand length matches the arm");
    std::hint::black_box(clmul_wide::<N, M>(a, b));
}

/// The public owned product at one width, written into `out`.
#[inline(never)]
fn owned_product_into<const N: usize, const M: usize>(a: &[u64], b: &[u64], out: &mut [u64]) {
    let a: &[u64; N] = a.try_into().expect("operand length matches the arm");
    let b: &[u64; N] = b.try_into().expect("operand length matches the arm");
    out[..M].copy_from_slice(&clmul_wide::<N, M>(a, b));
}

/// Runs the public owned long product for any instantiated word count,
/// discarding the result.
///
/// `clmul_wide` overwrites a fresh `2 * words`-word product, which is the
/// operation an external long-product library performs. Its sibling
/// `clmul_wide_slice` XOR-accumulates instead, so timing that form against an
/// overwriting library would charge gf2 for a destination clear and an
/// accumulation the comparator never performs.
pub fn long_product(words: usize, a: &[u64], b: &[u64]) {
    for_each_length!(words, owned_product, a, b)
}

/// The same owned long product, written into `out` for the validator.
///
/// # Panics
///
/// Panics if `out` is shorter than `2 * words`.
pub fn long_product_into(words: usize, a: &[u64], b: &[u64], out: &mut [u64]) {
    for_each_length!(words, owned_product_into, a, b, out)
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

/// Which of the two gf2 paths a crossover cell's arm exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossoverPath {
    /// One element or one word product per call.
    Element,
    /// The batched kernel over the whole vector.
    Batch,
}

impl CrossoverPath {
    /// Reads the path from `GF2_CROSSOVER_PATH`.
    ///
    /// # Panics
    ///
    /// Panics when the variable is absent or names neither path: a crossover
    /// arm that guessed its own path would attribute a ratio to the wrong
    /// entry point.
    pub fn from_environment() -> Self {
        match std::env::var("GF2_CROSSOVER_PATH").as_deref() {
            Ok("element") => CrossoverPath::Element,
            Ok("batch") => CrossoverPath::Batch,
            other => panic!("GF2_CROSSOVER_PATH must be `element` or `batch`, got {other:?}"),
        }
    }

    /// The tag this path reports inside `selected_path`.
    pub fn as_str(self) -> &'static str {
        match self {
            CrossoverPath::Element => "element",
            CrossoverPath::Batch => "batch",
        }
    }
}

/// The two GF(2^m) vectors a whole-consumer dot-product call multiplies.
type DotVectors = (FieldVec<Gf2mElement>, FieldVec<Gf2mElement>);

/// Packs `count` raw fixture word pairs into the two field vectors a dot-product
/// consumer holds.
fn pack_dot_vectors(field: &Gf2mField, count: usize, bank: &Bank) -> DotVectors {
    let left: Vec<Gf2mElement> = (0..count)
        .map(|index| field.element(field_element_word(bank.a[index])))
        .collect();
    let right: Vec<Gf2mElement> = (0..count)
        .map(|index| field.element(field_element_word(bank.b[index])))
        .collect();
    (FieldVec::from(left), FieldVec::from(right))
}

/// gf2 arm state: the detected bundle, the field of the whole-consumer cells,
/// and the scratch each case needs.
pub struct Gf2Backend {
    path: Option<CrossoverPath>,
    batch: Option<ClmulBatchFn>,
    clmul: Option<ClmulFn>,
    field: Gf2mField,
    products: Vec<u128>,
    packed_a: Vec<u64>,
    packed_b: Vec<u64>,
    reducer: Option<WideReducer>,
    selected: String,
}

impl Gf2Backend {
    /// The raw XOR accumulator `FieldVec::simd_dot_product` hands its single
    /// reduction, rebuilt from the same per-element products.
    ///
    /// gf2-core exposes neither that accumulator nor the batch kernel, reducer
    /// and carry-less multiply the dot product reads from its elements, so the
    /// harness rebuilds it: the operands masked to field elements, multiplied
    /// by the raw-batch kernel of the bundle `simd_dot_product` itself reads,
    /// then XOR-accumulated.
    pub fn dot_accumulator(&self, count: usize, bank: &Bank) -> u128 {
        let batch = self.batch.expect("the host resolved a batch kernel");
        let left: Vec<u64> = bank.a[..count].iter().copied().map(field_element_word).collect();
        let right: Vec<u64> = bank.b[..count].iter().copied().map(field_element_word).collect();
        let mut products = vec![0u128; count];
        batch(&left, &right, &mut products);
        products.iter().fold(0u128, |acc, product| acc ^ product)
    }

    /// The raw-batch lane identity this instance resolved.
    pub fn selected(&self) -> &str {
        &self.selected
    }
}

impl Backend for Gf2Backend {
    fn selected_path(&self) -> String {
        self.selected.clone()
    }

    fn create(case: &Case) -> Self {
        let gf2m = gf2_kernels_simd::gf2m::detect();
        let batch = gf2m.as_ref().and_then(|fns| fns.clmul_batch_fn);
        let clmul = gf2m.as_ref().and_then(|fns| fns.clmul_fn);
        let path = match case {
            Case::RawBatch { .. } | Case::FieldDot { .. } | Case::FieldBatchMul { .. } => {
                Some(CrossoverPath::from_environment())
            }
            Case::PolyMul { .. } | Case::WideFieldMul { .. } => None,
        };
        let selected = match case {
            Case::RawBatch { .. } => match path {
                Some(CrossoverPath::Batch) => batch_path(gf2m.as_ref()),
                _ => match gf2m.as_ref().and_then(|fns| fns.clmul_fn) {
                    Some(_) => "clmul_single:bundle-clmul".to_owned(),
                    None => "clmul_single:unavailable".to_owned(),
                },
            },
            Case::FieldDot { .. } => match path {
                Some(CrossoverPath::Batch) => {
                    format!("FieldVec::simd_dot_product({})", batch_path(gf2m.as_ref()))
                }
                _ => "FieldVec::dot_product".to_owned(),
            },
            Case::FieldBatchMul { .. } => match path {
                Some(CrossoverPath::Batch) => {
                    format!("gf2m::batch::batch_mul({})", batch_path(gf2m.as_ref()))
                }
                _ => "Gf2mElement::mul".to_owned(),
            },
            Case::PolyMul { words, .. } => {
                format!("clmul_wide:{}", wide_lane(*words))
            }
            Case::WideFieldMul { words, .. } => {
                format!("Gf2mWide::mul_ref:{}", wide_lane(*words))
            }
        };
        let count = case.operand_words();
        let products = match case {
            Case::RawBatch { count, .. } => vec![0u128; *count],
            _ => Vec::new(),
        };
        let (packed_a, packed_b) = match case {
            Case::FieldBatchMul { count, .. } => (vec![0u64; *count], vec![0u64; *count]),
            _ => (Vec::new(), Vec::new()),
        };
        let reducer = match case {
            Case::WideFieldMul { words, .. } => WideReducer::for_words(*words),
            _ => None,
        };
        let _ = count;
        Self {
            path,
            batch,
            clmul,
            field: Gf2mField::new(FIELD_DEGREE, FIELD_POLY),
            products,
            packed_a,
            packed_b,
            reducer,
            selected,
        }
    }

    fn run(&mut self, case: &Case, bank: &mut Bank) {
        match case {
            Case::RawBatch { count, .. } => {
                let count = *count;
                match self.path {
                    Some(CrossoverPath::Batch) => {
                        let batch = self.batch.expect("the host resolved a batch kernel");
                        batch(&bank.a[..count], &bank.b[..count], &mut self.products[..count]);
                    }
                    _ => {
                        let clmul = self.clmul.expect("the host resolved a single clmul");
                        for index in 0..count {
                            self.products[index] = clmul(bank.a[index], bank.b[index]);
                        }
                    }
                }
                std::hint::black_box(&self.products);
            }
            Case::FieldDot { count, .. } => {
                // Whole consumer: packing, the product-and-accumulate stage,
                // the reduction and the extraction of the field value are all
                // inside this call, and both paths pay the identical packing.
                let (left, right) = pack_dot_vectors(&self.field, *count, bank);
                bank.out[0] = match self.path {
                    Some(CrossoverPath::Batch) => left.simd_dot_product(&right).value(),
                    _ => left.dot_product(&right).value(),
                };
            }
            Case::FieldBatchMul { count, .. } => {
                let count = *count;
                for index in 0..count {
                    self.packed_a[index] = field_element_word(bank.a[index]);
                    self.packed_b[index] = field_element_word(bank.b[index]);
                }
                match self.path {
                    Some(CrossoverPath::Batch) => batch_mul(
                        &self.field,
                        &self.packed_a[..count],
                        &self.packed_b[..count],
                        &mut bank.out[..count],
                    ),
                    _ => {
                        for index in 0..count {
                            let left = self.field.element(self.packed_a[index]);
                            let right = self.field.element(self.packed_b[index]);
                            bank.out[index] = (&left * &right).value();
                        }
                    }
                }
            }
            Case::PolyMul { words, .. } => long_product(*words, &bank.a, &bank.b),
            Case::WideFieldMul { words, .. } => wide_field_product(*words, bank),
        }
    }

    fn conversion(&mut self, case: &Case, bank: &mut Bank) -> Conversion {
        let dispatch_ns = probe_ns(|| {
            std::hint::black_box(gf2_kernels_simd::gf2m_wide::detect_wide());
            std::hint::black_box(gf2_kernels_simd::gf2m::detect());
        });
        match case {
            Case::RawBatch { count, .. } => {
                let count = *count;
                let mut products = Vec::new();
                let setup_ns = probe_ns(|| products = vec![0u128; count]);
                std::hint::black_box(&products);
                // One call first, so the timed extraction moves products the
                // path wrote into a destination already touched.
                self.run(case, bank);
                let unpack_ns = probe_ns(|| {
                    for index in 0..count {
                        bank.out[2 * index] = self.products[index] as u64;
                        bank.out[2 * index + 1] = (self.products[index] >> 64) as u64;
                    }
                });
                Conversion {
                    setup_ns,
                    unpack_ns,
                    dispatch_ns,
                    ..Conversion::default()
                }
            }
            Case::FieldDot { count, .. } => {
                let setup_ns =
                    probe_ns(|| {
                        std::hint::black_box(Gf2mField::new(FIELD_DEGREE, FIELD_POLY));
                    });
                let mut vectors = None;
                let pack_ns = probe_ns(|| vectors = Some(pack_dot_vectors(&self.field, *count, bank)));
                let vectors = vectors.expect("the packing probe built both vectors");
                let batch_fill_ns = probe_ns(|| self.run(case, bank));
                let accumulator = self.dot_accumulator(*count, bank);
                let clmul = self.clmul.expect("the host resolved a single clmul");
                let reducer = gf2_core::gf2m::barrett::BarrettReducer::new(
                    u128::from(FIELD_POLY),
                    FIELD_DEGREE as u32,
                );
                let unpack_ns = crate::amortised_probe_ns(|| {
                    std::hint::black_box(
                        reducer.reduce_with_clmul(std::hint::black_box(accumulator), clmul),
                    );
                });
                std::hint::black_box(&vectors);
                Conversion {
                    setup_ns,
                    pack_ns,
                    unpack_ns,
                    batch_fill_ns,
                    dispatch_ns,
                }
            }
            Case::FieldBatchMul { count, .. } => {
                let count = *count;
                let setup_ns =
                    probe_ns(|| {
                        std::hint::black_box(Gf2mField::new(FIELD_DEGREE, FIELD_POLY));
                    });
                let pack_ns = probe_ns(|| {
                    for index in 0..count {
                        self.packed_a[index] = field_element_word(bank.a[index]);
                        self.packed_b[index] = field_element_word(bank.b[index]);
                    }
                });
                let batch_fill_ns = probe_ns(|| self.run(case, bank));
                Conversion {
                    setup_ns,
                    pack_ns,
                    batch_fill_ns,
                    dispatch_ns,
                    ..Conversion::default()
                }
            }
            Case::PolyMul { words, .. } => Conversion {
                unpack_ns: reduction_probe_ns(*words, bank, |field| self.run(case, field)),
                dispatch_ns,
                ..Conversion::default()
            },
            Case::WideFieldMul { words, .. } => {
                let words = *words;
                let reducer = self
                    .reducer
                    .as_ref()
                    .expect("a wide-field case resolved its reducer");
                let mut packed = crate::Bank {
                    a: reducer.element(&bank.a),
                    b: reducer.element(&bank.b),
                    out: vec![0; 2 * words],
                };
                let pack_ns = probe_ns(|| {
                    std::hint::black_box(reducer.element(&bank.a));
                });
                long_product_into(words, &packed.a, &packed.b, &mut packed.out);
                let unpack_ns = reducer.probe_ns(&packed.out);
                let batch_fill_ns = probe_ns(|| self.run(case, bank));
                Conversion {
                    pack_ns,
                    unpack_ns,
                    batch_fill_ns,
                    dispatch_ns,
                    ..Conversion::default()
                }
            }
        }
    }
}

/// Runtime-observed lane of the wide kernel that serves `words`-word operands,
/// or the portable name when the width has no kernel.
pub fn wide_lane(words: usize) -> &'static str {
    match words {
        4 => gf2_kernels_simd::gf2m_wide::detect()
            .map(|fns| fns.name)
            .unwrap_or("portable-scalar"),
        9 => gf2_kernels_simd::gf2m_wide::detect_571()
            .map(|fns| fns.name)
            .unwrap_or("portable-scalar"),
        _ => "portable-scalar",
    }
}

/// The whole-consumer wide field product at the two kernel-covered widths.
///
/// Operands are masked to field elements and multiplied with
/// `Gf2mWide::mul_ref`, which is the dispatch plus `BarrettReducerWide`; the
/// reduced element's words are written into the destination.
///
/// # Panics
///
/// Panics for a width with no wide field configured here.
pub fn wide_field_product(words: usize, bank: &mut Bank) {
    match words {
        4 => {
            let left = Wide256::from_words(mask_words::<4>(&bank.a, 256));
            let right = Wide256::from_words(mask_words::<4>(&bank.b, 256));
            let product = left.mul_ref(&right);
            bank.out[..4].copy_from_slice(product.words());
        }
        9 => {
            let left = Wide571::from_words(mask_words::<9>(&bank.a, 571));
            let right = Wide571::from_words(mask_words::<9>(&bank.b, 571));
            let product = left.mul_ref(&right);
            bank.out[..9].copy_from_slice(product.words());
        }
        other => panic!("no wide field is configured for {other}-word elements"),
    }
}

/// The low `degree` bits of `words`, as an `N`-word field element.
fn mask_words<const N: usize>(words: &[u64], degree: usize) -> [u64; N] {
    let mut out = [0u64; N];
    for (index, slot) in out.iter_mut().enumerate() {
        let low = 64 * index;
        *slot = if low + 64 <= degree {
            words[index]
        } else if low >= degree {
            0
        } else {
            words[index] & ((1u64 << (degree - low)) - 1)
        };
    }
    out
}
