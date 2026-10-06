//! Stage decomposition of the GF(2^m) dot product (jit:53c5a8c0).
//!
//! `FieldVec::simd_dot_product` performs packing, per-chunk extraction, the
//! raw-batch products, the XOR accumulation, one Barrett reduction and the
//! construction of the returned element in one monolithic body, and the batch
//! kernel, reducer and carry-less multiply it reads from its elements are
//! crate-private. No production seam exposes a stage boundary, so this module
//! reconstructs each stage from the same inputs and the same kernels the
//! consumer reaches, and times them separately. A reconstructed stage is
//! evidence about that stage's cost, not an instrumentation of the consumer:
//! the consumer's own timing is [`Stage::Consumer`], measured by the same
//! probe, and the stage figures are reported beside it rather than summed into
//! it.
//!
//! Every stage is amortised over [`crate::AMORTISED_PROBE_REPEATS`]
//! repetitions inside one process, and the diagnostic runs the whole set in
//! repeated fresh processes so its summary can resample across processes.

use gf2_core::field::FieldVec;
use gf2_core::gf2m::barrett::BarrettReducer;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2_kernels_simd::gf2m::{ClmulBatchFn, ClmulFn};
use serde::Serialize;

use crate::{field_element_word, Bank, FIELD_DEGREE, FIELD_POLY};

/// One reconstructed stage of the dot product, or one whole consumer call.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Building the two `FieldVec<Gf2mElement>` operands from raw words.
    Pack,
    /// Extracting the element values of both operands into flat `u64` buffers,
    /// the per-chunk loop the consumer runs before each batch call.
    Extract,
    /// The raw-batch carry-less products over the extracted buffers.
    Products,
    /// The XOR fold of the 128-bit products into one accumulator.
    Accumulate,
    /// The single Barrett reduction of that accumulator.
    Reduce,
    /// The whole `FieldVec::simd_dot_product` call on packed operands.
    Consumer,
    /// The whole `FieldVec::dot_product` call on the same packed operands.
    ConsumerScalar,
}

impl Stage {
    /// Every stage the diagnostic measures, in pipeline order.
    pub const ALL: [Stage; 7] = [
        Stage::Pack,
        Stage::Extract,
        Stage::Products,
        Stage::Accumulate,
        Stage::Reduce,
        Stage::Consumer,
        Stage::ConsumerScalar,
    ];

    /// The kebab-case identifier this stage carries in the diagnostic record.
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Pack => "pack",
            Stage::Extract => "extract",
            Stage::Products => "products",
            Stage::Accumulate => "accumulate",
            Stage::Reduce => "reduce",
            Stage::Consumer => "consumer",
            Stage::ConsumerScalar => "consumer-scalar",
        }
    }
}

/// One measured stage observation.
#[derive(Clone, Debug, Serialize)]
pub struct StageSample {
    pub stage: &'static str,
    pub elements: usize,
    pub repeats: u64,
    pub total_ns: u64,
}

/// The fixtures every stage of one vector length shares.
pub struct StageFixture {
    field: Gf2mField,
    left: FieldVec<Gf2mElement>,
    right: FieldVec<Gf2mElement>,
    values_a: Vec<u64>,
    values_b: Vec<u64>,
    products: Vec<u128>,
    accumulator: u128,
    batch: ClmulBatchFn,
    clmul: ClmulFn,
    reducer: BarrettReducer,
    elements: usize,
}

impl StageFixture {
    /// Builds every stage input for a `count`-element dot product on `bank`.
    ///
    /// # Panics
    ///
    /// Panics when the host publishes no carry-less batch kernel, because a
    /// stage decomposition of the dispatched consumer would then describe a
    /// path the consumer does not take.
    pub fn new(count: usize, bank: &Bank) -> Self {
        let field = Gf2mField::new(FIELD_DEGREE, FIELD_POLY);
        let bundle = gf2_kernels_simd::gf2m::detect()
            .expect("the stage diagnostic needs the host's carry-less bundle");
        let batch = bundle
            .clmul_batch_fn
            .expect("the host's bundle publishes a batch kernel");
        let clmul = bundle
            .clmul_fn
            .expect("the host's bundle publishes a single carry-less multiply");
        let values_a: Vec<u64> = bank.a[..count]
            .iter()
            .copied()
            .map(field_element_word)
            .collect();
        let values_b: Vec<u64> = bank.b[..count]
            .iter()
            .copied()
            .map(field_element_word)
            .collect();
        let left = FieldVec::from(
            values_a
                .iter()
                .map(|word| field.element(*word))
                .collect::<Vec<Gf2mElement>>(),
        );
        let right = FieldVec::from(
            values_b
                .iter()
                .map(|word| field.element(*word))
                .collect::<Vec<Gf2mElement>>(),
        );
        let mut products = vec![0u128; count];
        batch(&values_a, &values_b, &mut products);
        let accumulator = products.iter().fold(0u128, |acc, product| acc ^ product);
        let reducer = BarrettReducer::new(u128::from(FIELD_POLY), FIELD_DEGREE as u32);
        Self {
            field,
            left,
            right,
            values_a,
            values_b,
            products,
            accumulator,
            batch,
            clmul,
            reducer,
            elements: count,
        }
    }

    /// The reduced dot product the consumer returns, for the validator.
    pub fn consumer_value(&self) -> u64 {
        self.left.simd_dot_product(&self.right).value()
    }

    /// The reduction of the reconstructed accumulator, which must equal
    /// [`Self::consumer_value`].
    pub fn reconstructed_value(&self) -> u64 {
        self.reducer.reduce_with_clmul(self.accumulator, self.clmul)
    }

    /// The raw accumulator the reduction stage receives.
    pub fn accumulator(&self) -> u128 {
        self.accumulator
    }

    /// Times one stage, amortised over [`crate::AMORTISED_PROBE_REPEATS`].
    pub fn measure(&mut self, stage: Stage) -> StageSample {
        let count = self.elements;
        let total_ns = match stage {
            Stage::Pack => {
                let field = &self.field;
                let values_a = &self.values_a;
                let values_b = &self.values_b;
                crate::amortised_probe_total_ns(|| {
                    let left: Vec<Gf2mElement> =
                        values_a.iter().map(|word| field.element(*word)).collect();
                    let right: Vec<Gf2mElement> =
                        values_b.iter().map(|word| field.element(*word)).collect();
                    std::hint::black_box((FieldVec::from(left), FieldVec::from(right)));
                })
            }
            Stage::Extract => {
                let left = &self.left;
                let right = &self.right;
                let values_a = &mut self.values_a;
                let values_b = &mut self.values_b;
                crate::amortised_probe_total_ns(|| {
                    for (index, (a, b)) in left.iter().zip(right.iter()).enumerate() {
                        values_a[index] = a.value();
                        values_b[index] = b.value();
                    }
                    std::hint::black_box((&values_a, &values_b));
                })
            }
            Stage::Products => {
                let batch = self.batch;
                let values_a = &self.values_a;
                let values_b = &self.values_b;
                let products = &mut self.products;
                crate::amortised_probe_total_ns(|| {
                    batch(values_a, values_b, products);
                    std::hint::black_box(&products);
                })
            }
            Stage::Accumulate => {
                let products = &self.products;
                crate::amortised_probe_total_ns(|| {
                    let mut accumulator = 0u128;
                    for product in products.iter() {
                        accumulator ^= *product;
                    }
                    std::hint::black_box(accumulator);
                })
            }
            Stage::Reduce => {
                let reducer = &self.reducer;
                let clmul = self.clmul;
                let accumulator = self.accumulator;
                crate::amortised_probe_total_ns(|| {
                    std::hint::black_box(
                        reducer.reduce_with_clmul(std::hint::black_box(accumulator), clmul),
                    );
                })
            }
            Stage::Consumer => {
                let left = &self.left;
                let right = &self.right;
                crate::amortised_probe_total_ns(|| {
                    std::hint::black_box(left.simd_dot_product(right));
                })
            }
            Stage::ConsumerScalar => {
                let left = &self.left;
                let right = &self.right;
                crate::amortised_probe_total_ns(|| {
                    std::hint::black_box(left.dot_product(right));
                })
            }
        };
        StageSample {
            stage: stage.as_str(),
            elements: count,
            repeats: crate::AMORTISED_PROBE_REPEATS,
            total_ns,
        }
    }
}
