//! Dynamic attribution harness for the DVB-T2 bit-interleaver consumers
//! (jit:9fb40c83).
//!
//! The protocol receipt decides no adoption; this binary explains the current
//! routes. `verify` is non-timed correctness evidence. `session` produces five
//! descriptive windows and an exact allocation census for every declared path.
//! `counters` repeats one path so `perf` can attribute hardware events and hot
//! instructions in a scheduled benchmark window.

use gf2_coding::dvb_t2_bicm_harness::BicmAwgnChannel;
use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2BitInterleaver;
use gf2_coding::modem::DemapMethod;
use gf2_core::BitVec;
use gf2_sim::batch::BitPackedBatch;
use gf2_sim::stages::BitInterleave;
use gf2_sim::Stage;
use serde::Serialize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use survey_gf2_side::{
    bitvec_from_words, frame_bits, modcod_for_name, pack_bits, seeded_word_banks, unpack_words,
    xdsopl_forward_mut, MODCODS,
};

/// Allocation counter used only by this development harness.
///
/// # Safety
///
/// Every operation forwards the exact pointer and layout contract to
/// [`System`]. Relaxed atomic increments observe allocation count and requested
/// bytes without changing allocation or deallocation semantics.
struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: the global allocator receives a valid allocation layout and
        // forwards it unchanged to `System`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer/layout pair is the one supplied to this allocator
        // and is forwarded unchanged to `System`.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        // SAFETY: the pointer/layout pair and requested new size are forwarded
        // unchanged to `System`.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Boundary {
    Direct,
    SimStage,
    XdsoplPacked,
    BicmChannel,
}

#[derive(Clone, Copy, Serialize)]
struct Case {
    id: &'static str,
    modcod: &'static str,
    boundary: Boundary,
}

fn ladder() -> Vec<Case> {
    let mut cases = Vec::new();
    for modcod in MODCODS {
        for (boundary, tag) in [
            (Boundary::Direct, "direct"),
            (Boundary::SimStage, "sim-stage"),
            (Boundary::XdsoplPacked, "xdsopl-packed"),
            (Boundary::BicmChannel, "bicm-channel"),
        ] {
            cases.push(Case {
                id: Box::leak(format!("{modcod}-{tag}").into_boxed_str()),
                modcod,
                boundary,
            });
        }
    }
    cases
}

#[derive(Serialize)]
struct Record {
    #[serde(flatten)]
    case: Case,
    selected_path: String,
    ns_per_call: f64,
    windows_ns_per_call: Vec<f64>,
    calls_per_window: u64,
    allocations_per_call: u64,
    allocated_bytes_per_call: u64,
    setup_ns: u64,
}

#[derive(Serialize)]
struct CounterRecord {
    case: String,
    selected_path: String,
    calls: u64,
}

struct Prepared {
    selected_path: String,
    setup_ns: u64,
    body: Box<dyn FnMut()>,
}

const WINDOWS: usize = 5;
const WINDOW_TARGET: Duration = Duration::from_millis(40);

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let cases = ladder();
    match arguments.get(1).map(String::as_str) {
        Some("ladder") => println!(
            "{}",
            serde_json::to_string_pretty(&cases).expect("ladder encodes")
        ),
        Some("verify") => verify(),
        Some("session") => {
            require_benchmark_window();
            let output = arguments.get(2).expect("session needs an output path");
            let records: Vec<_> = cases.into_iter().map(measure).collect();
            std::fs::write(
                output,
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&records).expect("records encode")
                ),
            )
            .expect("session output writes");
            eprintln!("{} cases -> {output}", records.len());
        }
        Some("counters") => {
            require_benchmark_window();
            let wanted = arguments.get(2).expect("counters needs a case identifier");
            let case = cases
                .into_iter()
                .find(|case| case.id == wanted)
                .unwrap_or_else(|| panic!("no case named {wanted}"));
            let Prepared {
                selected_path,
                mut body,
                ..
            } = prepare(case);
            body();
            let started = Instant::now();
            let mut calls = 0_u64;
            while started.elapsed() < Duration::from_secs(1) {
                body();
                calls += 1;
            }
            println!(
                "{}",
                serde_json::to_string(&CounterRecord {
                    case: case.id.into(),
                    selected_path,
                    calls,
                })
                .expect("counter record encodes")
            );
        }
        _ => {
            eprintln!(
                "usage: dvb-profile ladder | verify | session <output.json> | counters <case-id>"
            );
            std::process::exit(2);
        }
    }
}

fn require_benchmark_window() {
    if std::env::var("GF2_BENCH_WINDOW").as_deref() != Ok("1")
        || std::env::var("GF2_BENCH").as_deref() != Ok("1")
    {
        eprintln!("timed DVB profiling requires GF2_BENCH_WINDOW=1 and GF2_BENCH=1");
        std::process::exit(2);
    }
}

fn verify() {
    for (index, modcod) in MODCODS.iter().enumerate() {
        let bits = frame_bits(modcod);
        let words = seeded_word_banks(0x9fb4_0000 + index as u64, 2, bits);
        let interleaver = Arc::new(DvbT2BitInterleaver::new(modcod_for_name(modcod)));
        let stage = BitInterleave::new(interleaver.clone());
        let channel = BicmAwgnChannel::new(
            DvbT2BitInterleaver::new(modcod_for_name(modcod)),
            modcod_for_name(modcod).modulation.bits_per_cell(),
            DemapMethod::MaxLog,
        );
        for bank in &words {
            let input = bitvec_from_words(bank, bits);
            let direct = interleaver.interleave(&input);
            let staged = stage
                .process(&BitPackedBatch::new(vec![input.clone()]), &mut ())
                .expect("simulation stage succeeds");
            assert_eq!(staged.frames, vec![direct.clone()]);

            let unpacked = unpack_words(bank, bits);
            let mut mutable = unpacked.clone();
            let mut output = vec![0_i32; bits];
            xdsopl_forward_mut(modcod, &mut mutable, &mut output);
            assert_eq!(pack_bits(&output), direct.words());

            let first = channel.transmit_and_demodulate_with_noise(&input, 0.0, 1.0, || 0.0);
            let second = channel.transmit_and_demodulate_with_noise(&input, 0.0, 1.0, || 0.0);
            assert_eq!(first, second, "{modcod}: BICM path is not deterministic");
            assert_eq!(first.len(), bits);
        }
        println!(
            "PASS {modcod}: direct, simulation-stage, xdsopl packed adapter and deterministic BICM consumer agree on declared boundaries"
        );
    }
}

fn measure(case: Case) -> Record {
    let Prepared {
        selected_path,
        setup_ns,
        mut body,
    } = prepare(case);
    body();
    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = ALLOCATED_BYTES.load(Ordering::Relaxed);
    body();
    let allocations_per_call = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let allocated_bytes_per_call = ALLOCATED_BYTES.load(Ordering::Relaxed) - bytes_before;

    let single = time(&mut body, 1).max(1);
    let calls_per_window = ((WINDOW_TARGET.as_nanos() as u64) / single).max(1);
    let mut windows = Vec::with_capacity(WINDOWS);
    for _ in 0..WINDOWS {
        let elapsed = time(&mut body, calls_per_window);
        windows.push(elapsed as f64 / calls_per_window as f64);
    }
    let mut ordered = windows.clone();
    ordered.sort_by(f64::total_cmp);
    Record {
        case,
        selected_path,
        ns_per_call: ordered[ordered.len() / 2],
        windows_ns_per_call: windows,
        calls_per_window,
        allocations_per_call,
        allocated_bytes_per_call,
        setup_ns,
    }
}

fn time(body: &mut Box<dyn FnMut()>, calls: u64) -> u64 {
    let started = Instant::now();
    for _ in 0..calls {
        body();
    }
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn prepare(case: Case) -> Prepared {
    let bits = frame_bits(case.modcod);
    let words = seeded_word_banks(0x9fb4_1000 + bits as u64, 1, bits)
        .pop()
        .expect("one fixture bank");
    let input = bitvec_from_words(&words, bits);
    let started = Instant::now();
    match case.boundary {
        Boundary::Direct => {
            let interleaver = DvbT2BitInterleaver::new(modcod_for_name(case.modcod));
            let setup_ns = elapsed_ns(started);
            Prepared {
                selected_path: format!(
                    "gf2-coding/DvbT2BitInterleaver::interleave/scalar-bit-scatter/{}",
                    case.modcod
                ),
                setup_ns,
                body: Box::new(move || {
                    black_box(interleaver.interleave(&input));
                }),
            }
        }
        Boundary::SimStage => {
            let interleaver = Arc::new(DvbT2BitInterleaver::new(modcod_for_name(case.modcod)));
            let stage = BitInterleave::new(interleaver);
            let batch = BitPackedBatch::new(vec![input]);
            let setup_ns = elapsed_ns(started);
            Prepared {
                selected_path: format!(
                    "gf2-sim/BitInterleave::process/DvbT2BitInterleaver::interleave/scalar-bit-scatter/{}",
                    case.modcod
                ),
                setup_ns,
                body: Box::new(move || {
                    black_box(stage.process(&batch, &mut ()).expect("stage succeeds"));
                }),
            }
        }
        Boundary::XdsoplPacked => {
            let modcod = case.modcod.to_owned();
            let setup_ns = elapsed_ns(started);
            Prepared {
                selected_path: format!(
                    "profile-adapter/packed-unpack-copy/xdsopl-PCTITL/pack/{}",
                    case.modcod
                ),
                setup_ns,
                body: Box::new(move || {
                    let unpacked = unpack_words(input.words(), bits);
                    let mut mutable = unpacked.clone();
                    let mut output = vec![0_i32; bits];
                    xdsopl_forward_mut(&modcod, &mut mutable, &mut output);
                    black_box(BitVec::from_words(pack_bits(&output), bits));
                }),
            }
        }
        Boundary::BicmChannel => {
            let modulation = modcod_for_name(case.modcod).modulation;
            let channel = BicmAwgnChannel::new(
                DvbT2BitInterleaver::new(modcod_for_name(case.modcod)),
                modulation.bits_per_cell(),
                DemapMethod::MaxLog,
            );
            let setup_ns = elapsed_ns(started);
            Prepared {
                selected_path: format!(
                    "gf2-coding/BicmAwgnChannel::transmit_and_demodulate_with_noise/max-log/{}",
                    case.modcod
                ),
                setup_ns,
                body: Box::new(move || {
                    black_box(channel.transmit_and_demodulate_with_noise(&input, 0.0, 1.0, || 0.0));
                }),
            }
        }
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
