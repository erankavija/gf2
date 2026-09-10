//! Repeated measurement of the dot-product cell's separated GF(2^8) reduction
//! (jit:c7113c5a).
//!
//! The native confirmation receipt's dot-product `unpack` values time Barrett
//! reduction of the dot product's reduced output, which `reduce_with_clmul`
//! returns at its early exit. This diagnostic times the reduction of the raw
//! XOR-accumulated product that the dot product's single reduction receives,
//! on the operands the cell's single timed worker uses, and repeats the
//! superseded probe beside it:
//!
//! - `consumer`: [`dot_reducer`] with the PCLMULQDQ carry-less multiply of
//!   gf2's default GF(2^m) bundle, the reduction `FieldVec::simd_dot_product`
//!   performs and the gf2 arm's call contains.
//! - `composed`: the same reducer with the scalar `barrett::clmul`, the
//!   reduction the gf2x arm's call composes.
//! - `superseded`: the receipt's probe, the scalar reduction of the reduced
//!   dot product.
//!
//! Before timing it checks that gf2's batch kernel and gf2x build the same
//! accumulator, and that reducing it gives both arms' dot products.
//!
//! Usage: `dot-reduction-probe <case-json> <process-index> <repetitions>`
//!
//! Writes one JSON line: the fixture, the selected paths, the host observation
//! and, per repetition and probe, the total nanoseconds of
//! `AMORTISED_PROBE_REPEATS` reductions. Repetition `r` runs the probes in an
//! order rotated by `r`.

use serde::Serialize;
use std::collections::BTreeMap;

use gf2_core::gf2m::barrett::clmul;
use poly_baseline_arms::gf2_backend::{dot_accumulator, Gf2Backend};
use poly_baseline_arms::{
    banks, dot_reducer, dot_reduction_total_ns, Backend, Case, AMORTISED_PROBE_REPEATS,
    DOT_FIELD_DEGREE,
};
use tuning_campaign_support::host::{CpuAffinity, HostObservation};

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

use gf2x_backend::Gf2xBackend;

/// The worker a single-core cell times, whose only bank a warm cell reuses.
const TIMED_WORKER: usize = 0;

/// One probe: its name, the value it reduces and the carry-less multiply.
type Probe = (&'static str, u128, fn(u64, u64) -> u128);

#[derive(Serialize)]
struct Fixture {
    case: Case,
    generator: &'static str,
    worker: usize,
    bank: usize,
    accumulator: String,
    accumulator_degree: u32,
    reduced: u64,
}

#[derive(Serialize)]
struct Record {
    schema: &'static str,
    issue: &'static str,
    process: u32,
    repetitions: u32,
    reductions_per_value: u64,
    fixture: Fixture,
    probes: BTreeMap<&'static str, &'static str>,
    gf2_batch_path: String,
    gf2x_selected_path: String,
    cpus_observed: Vec<u32>,
    host: HostObservation,
    total_ns: BTreeMap<&'static str, Vec<u64>>,
}

fn main() {
    let usage = "usage: dot-reduction-probe <case-json> <process-index> <repetitions>";
    let mut args = std::env::args().skip(1);
    let case: Case = serde_json::from_str(&args.next().expect(usage)).expect("the case decodes");
    let process: u32 = args.next().expect(usage).parse().expect(usage);
    let repetitions: u32 = args.next().expect(usage).parse().expect(usage);
    let Case::Gf2mDot { count, .. } = case else {
        panic!("the diagnostic measures a gf2m-dot case");
    };

    let mut bank = banks(&case, TIMED_WORKER, 1).pop().expect("one bank");
    let mut gf2 = Gf2Backend::create(&case);
    let mut gf2x = Gf2xBackend::create(&case);
    let fns = gf2_kernels_simd::gf2m::detect().expect("the host has gf2's PCLMULQDQ bundle");
    let batch = fns.clmul_batch_fn.expect("the bundle has a batch kernel");
    let hardware = fns
        .clmul_fn
        .expect("the bundle has a single carry-less multiply");

    let accumulator = dot_accumulator(batch, count, &bank);
    assert_eq!(
        gf2x.dot_accumulator(count, &bank),
        accumulator,
        "gf2's batch kernel and gf2x build different accumulators"
    );
    gf2.run(&case, &mut bank);
    let reduced = bank.out[0];
    gf2x.run(&case, &mut bank);
    assert_eq!(bank.out[0], reduced, "the two arms' dot products differ");
    assert_eq!(
        dot_reducer().reduce_with_clmul(accumulator, hardware),
        reduced,
        "the rebuilt accumulator does not reduce to simd_dot_product's result"
    );
    let accumulator_degree = accumulator.ilog2();
    assert!(
        accumulator_degree >= DOT_FIELD_DEGREE as u32,
        "the timed operands' accumulator is already reduced"
    );

    let probes: [Probe; 3] = [
        ("consumer", accumulator, hardware),
        ("composed", accumulator, clmul),
        ("superseded", u128::from(reduced), clmul),
    ];
    let mut total_ns: BTreeMap<&'static str, Vec<u64>> = probes
        .iter()
        .map(|(name, ..)| (*name, Vec::with_capacity(repetitions as usize)))
        .collect();
    for repetition in 0..repetitions as usize {
        for offset in 0..probes.len() {
            let (name, input, multiply) = probes[(repetition + offset) % probes.len()];
            total_ns
                .get_mut(name)
                .expect("every probe has a series")
                .push(dot_reduction_total_ns(input, multiply));
        }
    }

    let record = Record {
        schema: "c7113c5a-dot-reduction-diagnostic-v1",
        issue: "c7113c5a",
        process,
        repetitions,
        reductions_per_value: AMORTISED_PROBE_REPEATS,
        fixture: Fixture {
            case,
            generator: "tuning_campaign_support::abtest::SplitMix64 expanded by \
                        poly_baseline_arms::banks (survey/gf2-side/src/lib.rs)",
            worker: TIMED_WORKER,
            bank: 0,
            accumulator: format!("{accumulator:#x}"),
            accumulator_degree,
            reduced,
        },
        probes: BTreeMap::from([
            (
                "consumer",
                "raw accumulator; dot_reducer with gf2_kernels_simd::gf2m::detect().clmul_fn \
                 (PCLMULQDQ), as FieldVec::simd_dot_product",
            ),
            (
                "composed",
                "raw accumulator; dot_reducer with gf2_core::gf2m::barrett::clmul (scalar), \
                 as the gf2x arm",
            ),
            (
                "superseded",
                "reduced dot product; dot_reducer with gf2_core::gf2m::barrett::clmul, \
                 as both arms' unpack probe in the v3 receipts",
            ),
        ]),
        gf2_batch_path: gf2.batch_name().to_owned(),
        gf2x_selected_path: gf2x.selected_path(),
        cpus_observed: CpuAffinity::observe()
            .map(|affinity| affinity.cpus().to_vec())
            .unwrap_or_default(),
        host: HostObservation::observe().expect("the host observation succeeds"),
        total_ns,
    };
    println!(
        "{}",
        serde_json::to_string(&record).expect("the record serialises")
    );
}
