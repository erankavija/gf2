//! A/B arm for the shipped GF(2^8) dense product (jit:4c1e441f).
//!
//! Measures `field::matrix::gemm` over GF(2^8) modulo the cell's declared
//! polynomial, at a square dimension, either on operands the consumer already
//! holds or from byte region to byte region. The baseline position holds every
//! call on the route the library takes without the cached table and the
//! candidate position lets the table lane run, so a pair isolates the lane and
//! nothing else.
//!
//! The executable links a counting global allocator so each cell reports what
//! one call allocates. The counters are armed for one untimed probe call and
//! disarmed for every timed window, where they cost one relaxed load per
//! allocation on both arms of a pair.

use byte_field_arm_common::alloc::CountingAllocator;
use byte_field_arm_common::{probe_ns, Case, Conversion, Metric, ObservedWorkload, Operation};
use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use gf256_gemm_arm::{observed_lane, Lane, Representation, SquareProduct};
use std::hint::black_box;
use tuning_campaign_support::protocol::CacheState;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() -> ! {
    byte_field_arm_common::run_observed(|request, case| {
        if case.operation != Operation::Matmul {
            return Err(format!(
                "this family measures field::matrix::gemm; the cell declares {:?}",
                case.operation
            ));
        }
        if case.workers != 1 {
            return Err(format!(
                "field::matrix::gemm is single-threaded; the cell declares {} workers",
                case.workers
            ));
        }
        if request.cache_state != CacheState::Warm {
            return Err(format!(
                "this arm probes its allocation count with untimed calls, which only a warm cell \
                 admits; the cell declares {:?}",
                request.cache_state
            ));
        }
        Lane::from_env()?.apply();
        match Representation::from_env()? {
            Representation::Element => {
                let setup_ns = probe_ns(|| {
                    black_box(RuntimeGf256::new());
                });
                let field = RuntimeGf256::new();
                check_polynomial(&field, case)?;
                Ok(build_product(field, case, setup_ns))
            }
            Representation::Wide => {
                check_polynomial(&WideGf256, case)?;
                // A compile-time field has no runtime context to build.
                Ok(build_product(WideGf256, case, 0))
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

/// The square product through `field::matrix::gemm`, at the metric the cell
/// declares.
///
/// The allocation count is probed on a second untimed call, so the one table
/// build a process pays and the operand packing both precede it and the count
/// is the steady-state per-call count the cell reports.
fn build_product<B: ByteField + 'static>(
    field: B,
    case: &Case,
    setup_ns: u64,
) -> ObservedWorkload<'static> {
    let mut product = SquareProduct::new(&field, case.n, case.seed);
    let conversion = Conversion {
        setup_ns,
        pack_ns: probe_ns(|| {
            workload::pack_into_matrix(&field, &product.a_bytes, &mut product.a);
            workload::pack_into_matrix(&field, &product.b_bytes, &mut product.b);
        }),
        unpack_ns: probe_ns(|| {
            workload::unpack_matrix::<B>(&product.a, &mut product.out_bytes)
        }),
        // The cached table is built inside the first call that reaches it, so
        // the arm has no separate preparation step to time.
        batch_fill_ns: 0,
        dispatch_ns: 0,
    };
    let mut body: Box<dyn FnMut(usize)> = match case.metric {
        Metric::KernelIsolated => Box::new(move |_bank| {
            black_box(product.kernel_isolated());
        }),
        Metric::WholeConsumer => Box::new(move |_bank| {
            product.whole_consumer(&field);
            black_box(&product.out_bytes);
        }),
    };
    body(0);
    let allocated = byte_field_arm_common::alloc::allocations_during(|| body(0));
    let poly = case.poly;
    let without_table = B::gemm_route();
    ObservedWorkload {
        conversion,
        body,
        observe: Box::new(move || {
            format!(
                "gf2-core/field::matrix::gemm<{}>/poly=0x{poly:X}/{}/route-without-table={}/\
                 allocs-per-call={}/alloc-bytes-per-call={}",
                B::NAME,
                observed_lane(),
                without_table,
                allocated.calls,
                allocated.bytes
            )
        }),
    }
}
