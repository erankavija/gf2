//! A/B arm for the shipped GF(2^8) axpy path (jit:ad2a6a58).
//!
//! Measures `FieldVec::axpy` on operands already in the consumer's
//! representation, over GF(2^8) modulo the cell's declared polynomial. The
//! baseline position holds every call on the scalar element lane through the
//! shipped lane switch and the candidate position lets the accelerated lane
//! run, so a pair isolates the lane and nothing else.

use byte_field_arm_common::{
    banks, probe_ns, Case, Conversion, Metric, ObservedWorkload, OperandStream, Operation,
    SplitMix64,
};
use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use gf2_core::field::FieldVec;
use gf256_axpy_arm::{observed_lane, Lane, Representation};
use std::hint::black_box;

fn main() -> ! {
    byte_field_arm_common::run_observed(|request, case| {
        if case.operation != Operation::Axpy {
            return Err(format!(
                "this family measures FieldVec::axpy; the cell declares {:?}",
                case.operation
            ));
        }
        if case.metric != Metric::KernelIsolated {
            return Err(
                "the shipped path writes in place and offers no byte region, so every cell of \
                 this family is kernel-isolated"
                    .to_owned(),
            );
        }
        if case.workers != 1 {
            return Err(format!(
                "FieldVec::axpy is single-threaded; the cell declares {} workers",
                case.workers
            ));
        }
        Lane::from_env()?.apply();
        let bank_count = banks(request.cache_state);
        match Representation::from_env()? {
            Representation::Element => {
                let setup_ns = probe_ns(|| {
                    black_box(RuntimeGf256::new());
                });
                let field = RuntimeGf256::new();
                check_polynomial(&field, case)?;
                Ok(build_axpy(field, case, bank_count, setup_ns))
            }
            Representation::Wide => {
                check_polynomial(&WideGf256, case)?;
                // A compile-time field has no runtime context to build.
                Ok(build_axpy(WideGf256, case, bank_count, 0))
            }
        }
    })
}

fn check_polynomial<B: ByteField>(field: &B, case: &Case) -> Result<(), String> {
    if field.polynomial() == case.poly {
        Ok(())
    } else {
        Err(format!(
            "{} works modulo 0x{:X}; the cell declares 0x{:X}",
            B::NAME,
            field.polynomial(),
            case.poly
        ))
    }
}

/// Fixed-coefficient region multiply-accumulate through `FieldVec::axpy`.
///
/// Preparation reaches no GF(2^8) multiplication, so a `cold` cell's first
/// dispatch — and, on the candidate lane, the one table build a process pays
/// — falls inside its first timed window.
fn build_axpy<B: ByteField + 'static>(
    field: B,
    case: &Case,
    bank_count: usize,
    setup_ns: u64,
) -> ObservedWorkload<'static> {
    let mut rng = SplitMix64::new(case.seed);
    let mut source_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    let mut target_bytes: Vec<Vec<u8>> = Vec::with_capacity(bank_count);
    for _ in 0..bank_count {
        let mut source = vec![0u8; case.bytes];
        let mut target = vec![0u8; case.bytes];
        rng.fill(&mut source);
        rng.fill(&mut target);
        source_bytes.push(source);
        target_bytes.push(target);
    }
    let coefficient = field.element(rng.coefficient());
    let mut sources: Vec<FieldVec<B::Elem>> = source_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut targets: Vec<FieldVec<B::Elem>> = target_bytes
        .iter()
        .map(|bytes| workload::pack_vec(&field, bytes))
        .collect();
    let mut scratch = vec![0u8; case.bytes];
    let conversion = Conversion {
        setup_ns,
        // Reported to say what a consumer starting from bytes would pay; no
        // cell of this family includes it, because the shipped hook reads and
        // writes the representation the caller already holds.
        pack_ns: probe_ns(|| {
            workload::pack_into_vec(&field, &source_bytes[0], &mut sources[0]);
            workload::pack_into_vec(&field, &target_bytes[0], &mut targets[0]);
        }),
        unpack_ns: probe_ns(|| workload::unpack_vec::<B>(&targets[0], &mut scratch)),
        // The cached table is built inside the first call that reaches it, so
        // the arm has no separate preparation step to time; the `cold` cells
        // are where that build is measured.
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    let poly = case.poly;
    ObservedWorkload {
        conversion,
        body: Box::new(move |bank| {
            let index = bank % bank_count;
            workload::axpy(
                &mut targets[index],
                &coefficient,
                black_box(&sources[index]),
            );
            black_box(&targets[index]);
        }),
        observe: Box::new(move || {
            format!(
                "gf2-core/FieldVec<{}>::axpy/poly=0x{poly:X}/{}",
                B::NAME,
                observed_lane()
            )
        }),
    }
}
