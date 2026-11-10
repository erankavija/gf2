//! `encode-compare nr` times gf2's NR rate-matched encoder against srsRAN's
//! and AFF3CT's on the cells of [`encode_compare::nr::NR_CELLS`] and prints one
//! table row per cell, worker count and arm; `ENCODE_COMPARE_WORKERS` (default
//! `1,12`) lists the worker counts. `encode-compare bch` times gf2's DVB-T2
//! BCH encoder against bchlib's on [`encode_compare::bch::BCH_CELLS`] at one
//! worker. Before timing, every arm must reproduce gf2's codewords for all
//! the cell's messages; an arm that does not, or whose library derives
//! another base graph or lifting size, is reported and not timed.

use encode_compare::bch::{BchCell, Gf2BchArm, BCH_CELLS};
use encode_compare::nr::{Gf2NrArm, NrCell, NR_CELLS};
use encode_compare::{frames_per_second, host_description, messages, Arm, MESSAGES};
use gf2_coding::traits::block::BlockCode;
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use std::ffi::{c_char, c_void, CString};
use std::sync::Arc;

extern "C" {
    fn gf2_bchlib_new(m: i32, t: i32, prim_poly: u32) -> *mut c_void;
    fn gf2_bchlib_free(handle: *mut c_void);
    fn gf2_bchlib_ecc_bits(handle: *mut c_void) -> u32;
    fn gf2_bchlib_encode(handle: *mut c_void, data: *const u8, len: u32, ecc: *mut u8);
    fn gf2_srsran_nr_avx2() -> i32;
    fn gf2_srsran_nr_k_ldpc(bg: i32, z: i32) -> i32;
    fn gf2_srsran_nr_new() -> *mut c_void;
    fn gf2_srsran_nr_free(handle: *mut c_void);
    fn gf2_srsran_nr_encode(
        handle: *mut c_void,
        bg: i32,
        z: i32,
        k: i32,
        n: i32,
        input: *const u8,
        output: *mut u8,
    );
    fn gf2_aff3ct_nr_derive(k: i32, n: i32, bg: *mut i32, z: *mut i32) -> i32;
    fn gf2_aff3ct_nr_new_i8(k: i32, n: i32, conf: *const c_char) -> *mut c_void;
    fn gf2_aff3ct_nr_new_i32(k: i32, n: i32, conf: *const c_char) -> *mut c_void;
    fn gf2_aff3ct_nr_free_i8(handle: *mut c_void);
    fn gf2_aff3ct_nr_free_i32(handle: *mut c_void);
    fn gf2_aff3ct_nr_encode_i8(
        handle: *mut c_void,
        message: *const i8,
        transmitted: *mut i8,
    ) -> i32;
    fn gf2_aff3ct_nr_encode_i32(
        handle: *mut c_void,
        message: *const i32,
        transmitted: *mut i32,
    ) -> i32;
}

/// `bits` packed most significant bit first into `bytes` bytes, srsRAN's
/// `bit_buffer` layout.
fn msb_first(bits: &BitVec, bytes: usize) -> Vec<u8> {
    let mut out = vec![0u8; bytes];
    for i in 0..bits.len() {
        if bits.get(i) {
            out[i / 8] |= 0x80 >> (i % 8);
        }
    }
    out
}

fn from_msb_first(bytes: &[u8], len: usize) -> BitVec {
    let mut out = BitVec::zeros(len);
    for i in 0..len {
        out.set(i, bytes[i / 8] & (0x80 >> (i % 8)) != 0);
    }
    out
}

struct Cell {
    spec: NrCell,
    z: usize,
    messages: Arc<Vec<BitVec>>,
    expected: Vec<BitVec>,
}

struct SrsranArm {
    handle: *mut c_void,
    cell: NrCell,
    z: i32,
    inputs: Arc<Vec<Vec<u8>>>,
    output: Vec<u8>,
}

impl SrsranArm {
    fn new(cell: &Cell, inputs: Arc<Vec<Vec<u8>>>) -> Self {
        // SAFETY: the constructor takes no arguments and returns an owned handle.
        let handle = unsafe { gf2_srsran_nr_new() };
        Self {
            handle,
            cell: cell.spec,
            z: cell.z as i32,
            inputs,
            output: vec![0; cell.spec.n.div_ceil(8)],
        }
    }

    fn output(&self) -> BitVec {
        from_msb_first(&self.output, self.cell.n)
    }
}

impl Arm for SrsranArm {
    fn encode(&mut self, index: usize) {
        let c = self.cell;
        // SAFETY: `inputs[index]` holds k_ldpc bits and `output` n bits, the
        // lengths the shim reads and writes for (bg, z, k, n).
        unsafe {
            gf2_srsran_nr_encode(
                self.handle,
                i32::from(c.base_graph),
                self.z,
                c.k as i32,
                c.n as i32,
                self.inputs[index].as_ptr(),
                self.output.as_mut_ptr(),
            );
        }
    }
}

impl Drop for SrsranArm {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `gf2_srsran_nr_new` and is freed once.
        unsafe { gf2_srsran_nr_free(self.handle) }
    }
}

/// AFF3CT at one bit type, one value per bit.
struct Aff3ctArm<B: Copy + Default + 'static> {
    handle: *mut c_void,
    inputs: Arc<Vec<Vec<B>>>,
    output: Vec<B>,
    encode: unsafe extern "C" fn(*mut c_void, *const B, *mut B) -> i32,
    free: unsafe extern "C" fn(*mut c_void),
}

impl<B: Copy + Default + PartialEq + From<bool> + 'static> Aff3ctArm<B> {
    fn output(&self) -> BitVec {
        let mut out = BitVec::zeros(self.output.len());
        for (i, v) in self.output.iter().enumerate() {
            out.set(i, *v != B::default());
        }
        out
    }
}

impl<B: Copy + Default + 'static> Arm for Aff3ctArm<B> {
    fn encode(&mut self, index: usize) {
        // SAFETY: `inputs[index]` holds K values and `output` N, the lengths
        // the handle was built for.
        let status = unsafe {
            (self.encode)(
                self.handle,
                self.inputs[index].as_ptr(),
                self.output.as_mut_ptr(),
            )
        };
        debug_assert_eq!(status, 0);
    }
}

impl<B: Copy + Default + 'static> Drop for Aff3ctArm<B> {
    fn drop(&mut self) {
        // SAFETY: `handle` came from the matching constructor and is freed once.
        unsafe { (self.free)(self.handle) }
    }
}

fn workers() -> Vec<usize> {
    std::env::var("ENCODE_COMPARE_WORKERS")
        .unwrap_or_else(|_| "1,12".to_owned())
        .split(',')
        .map(|w| {
            w.trim()
                .parse()
                .expect("ENCODE_COMPARE_WORKERS lists worker counts")
        })
        .collect()
}

/// Checks `make`'s arm against the cell's expected codewords.
fn reproduces<A: Arm>(cell: &Cell, mut arm: A, output: impl Fn(&A) -> BitVec) -> bool {
    (0..cell.messages.len()).all(|i| {
        arm.encode(i);
        output(&arm) == cell.expected[i]
    })
}

fn row(cell: &Cell, workers: usize, arm: &str, fps: f64, gf2_fps: Option<f64>) {
    let ratio = gf2_fps.map_or("-".to_owned(), |g| format!("{:.2}", g / fps));
    println!(
        "| BG{} n={} k={} Z={} | {workers} | {arm} | {fps:.0} | {:.1} | {ratio} |",
        cell.spec.base_graph,
        cell.spec.n,
        cell.spec.k,
        cell.z,
        fps * cell.spec.k as f64 / 1e6
    );
}

fn nr() {
    let aff3ct_root =
        std::env::var("GF2_AFF3CT_ROOT").expect("GF2_AFF3CT_ROOT names the staged AFF3CT tree");
    let conf = CString::new(format!("{aff3ct_root}/conf")).expect("a path without NUL");
    // SAFETY: a CPU-feature query.
    let srsran_backend = if unsafe { gf2_srsran_nr_avx2() } == 1 {
        "avx2"
    } else {
        "generic"
    };
    println!("# host: {}", host_description());
    println!(
        "# srsRAN encoder backend: {srsran_backend}; AFF3CT Encoder_LDPC_QC_fast + Puncturer_5G"
    );
    println!("# ratio: gf2 frames/s over the arm's; above 1 means gf2 is faster");
    println!("| cell | workers | arm | frames/s | info Mb/s | gf2 ratio |");
    println!("|---|---|---|---|---|---|");

    for spec in NR_CELLS {
        let code = Arc::new(spec.code());
        let z = code.params().lifting_factor;
        let messages = Arc::new(messages(spec.k, MESSAGES, spec.seed()));
        let expected = messages.iter().map(|m| code.encode(m)).collect();
        let cell = Cell {
            spec,
            z,
            messages: messages.clone(),
            expected,
        };
        let bg = i32::from(spec.base_graph);

        // SAFETY: a table lookup for a valid (bg, z).
        let k_ldpc = unsafe { gf2_srsran_nr_k_ldpc(bg, z as i32) } as usize;
        let srsran_inputs = Arc::new(
            messages
                .iter()
                .map(|m| msb_first(m, k_ldpc.div_ceil(8)))
                .collect::<Vec<_>>(),
        );
        let srsran_ok = k_ldpc == code.params().full_k
            && reproduces(
                &cell,
                SrsranArm::new(&cell, srsran_inputs.clone()),
                SrsranArm::output,
            );

        let (mut a_bg, mut a_z) = (0, 0);
        // SAFETY: writes two integers.
        let derived =
            unsafe { gf2_aff3ct_nr_derive(spec.k as i32, spec.n as i32, &mut a_bg, &mut a_z) } == 0;
        let aff3ct_matches = derived && a_bg == bg && a_z as usize == z;
        let i8_inputs = Arc::new(
            messages
                .iter()
                .map(|m| {
                    (0..m.len())
                        .map(|i| i8::from(m.get(i)))
                        .collect::<Vec<i8>>()
                })
                .collect::<Vec<_>>(),
        );
        let i32_inputs = Arc::new(
            messages
                .iter()
                .map(|m| {
                    (0..m.len())
                        .map(|i| i32::from(m.get(i)))
                        .collect::<Vec<i32>>()
                })
                .collect::<Vec<_>>(),
        );
        let new_i8 = || -> Option<Aff3ctArm<i8>> {
            // SAFETY: `conf` outlives the call; null means AFF3CT rejected the pair.
            let handle =
                unsafe { gf2_aff3ct_nr_new_i8(spec.k as i32, spec.n as i32, conf.as_ptr()) };
            (!handle.is_null()).then(|| Aff3ctArm {
                handle,
                inputs: i8_inputs.clone(),
                output: vec![0; spec.n],
                encode: gf2_aff3ct_nr_encode_i8,
                free: gf2_aff3ct_nr_free_i8,
            })
        };
        let new_i32 = || -> Option<Aff3ctArm<i32>> {
            // SAFETY: as for the int8 arm.
            let handle =
                unsafe { gf2_aff3ct_nr_new_i32(spec.k as i32, spec.n as i32, conf.as_ptr()) };
            (!handle.is_null()).then(|| Aff3ctArm {
                handle,
                inputs: i32_inputs.clone(),
                output: vec![0; spec.n],
                encode: gf2_aff3ct_nr_encode_i32,
                free: gf2_aff3ct_nr_free_i32,
            })
        };
        let i8_ok =
            aff3ct_matches && new_i8().is_some_and(|arm| reproduces(&cell, arm, Aff3ctArm::output));
        let i32_ok = aff3ct_matches
            && new_i32().is_some_and(|arm| reproduces(&cell, arm, Aff3ctArm::output));
        if !srsran_ok {
            println!(
                "# BG{bg} n={} k={}: srsRAN does not reproduce gf2's codewords; not timed",
                spec.n, spec.k
            );
        }
        if !aff3ct_matches {
            println!(
                "# BG{bg} n={} k={}: AFF3CT derives BG{a_bg} Z={a_z} (gf2 BG{bg} Z={z}); not timed",
                spec.n, spec.k
            );
        } else if !(i8_ok && i32_ok) {
            println!(
                "# BG{bg} n={} k={}: AFF3CT int8 {i8_ok}, int32 {i32_ok} reproduce gf2",
                spec.n, spec.k
            );
        }

        for &w in &workers() {
            let gf2 = frames_per_second(w, MESSAGES, || {
                Box::new(Gf2NrArm::new(code.clone(), messages.clone()))
            });
            row(&cell, w, "gf2", gf2, None);
            if srsran_ok {
                let fps = frames_per_second(w, MESSAGES, || {
                    Box::new(SrsranArm::new(&cell, srsran_inputs.clone()))
                });
                row(
                    &cell,
                    w,
                    &format!("srsRAN {srsran_backend}"),
                    fps,
                    Some(gf2),
                );
            }
            if i8_ok {
                let fps =
                    frames_per_second(w, MESSAGES, || Box::new(new_i8().expect("validated above")));
                row(&cell, w, "AFF3CT int8", fps, Some(gf2));
            }
            if i32_ok {
                let fps = frames_per_second(w, MESSAGES, || {
                    Box::new(new_i32().expect("validated above"))
                });
                row(&cell, w, "AFF3CT int32", fps, Some(gf2));
            }
        }
    }
}

/// bchlib's encoder for one code, over messages packed most significant bit
/// first, the byte order its polynomial convention reads.
struct BchlibArm {
    handle: *mut c_void,
    inputs: Arc<Vec<Vec<u8>>>,
    ecc: Vec<u8>,
}

impl BchlibArm {
    /// `None` when bchlib rejects the field or the correction radius.
    fn new(m: i32, t: i32, prim_poly: u32, inputs: Arc<Vec<Vec<u8>>>) -> Option<Self> {
        // SAFETY: null signals rejection; a non-null handle is owned here.
        let handle = unsafe { gf2_bchlib_new(m, t, prim_poly) };
        if handle.is_null() {
            return None;
        }
        // SAFETY: `handle` is a live bchlib control.
        let ecc_bits = unsafe { gf2_bchlib_ecc_bits(handle) } as usize;
        Some(Self {
            handle,
            inputs,
            ecc: vec![0; ecc_bits.div_ceil(8)],
        })
    }

    /// The data bits then the parity bits, both most significant bit first.
    fn codeword(&self, index: usize, k: usize, r: usize) -> BitVec {
        let mut bits = from_msb_first(&self.inputs[index], k);
        let parity = from_msb_first(&self.ecc, r);
        let mut out = BitVec::zeros(k + r);
        out.copy_bits(0, &bits, 0, k);
        out.copy_bits(k, &parity, 0, r);
        bits.clear();
        out
    }
}

impl Arm for BchlibArm {
    fn encode(&mut self, index: usize) {
        let data = &self.inputs[index];
        // SAFETY: `data` holds the message bytes and `ecc` the control's
        // ecc_bytes, the lengths `bch_encode` reads and writes.
        unsafe {
            gf2_bchlib_encode(
                self.handle,
                data.as_ptr(),
                data.len() as u32,
                self.ecc.as_mut_ptr(),
            )
        }
    }
}

impl Drop for BchlibArm {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `gf2_bchlib_new` and is freed once.
        unsafe { gf2_bchlib_free(self.handle) }
    }
}

/// The field degree, correction radius and primitive polynomial of a
/// DVB-T2 rate-1/2 outer code (`@/citation/Etsi2015` Tables 6a and 6b).
fn bch_parameters(cell: &BchCell) -> (i32, i32, u32) {
    match cell.frame {
        gf2_coding::bch::dvb_t2::FrameSize::Short => (14, 12, 0b100000000101011),
        gf2_coding::bch::dvb_t2::FrameSize::Normal => (16, 12, 0b10000000000101101),
    }
}

fn bch() {
    println!("# host: {}", host_description());
    println!("# bchlib bch_encode (four-table remainder); gf2 DvbT2BchCode::encode_into");
    println!("# ratio: gf2 codewords/s over the arm's; above 1 means gf2 is faster");
    println!("| cell | batch | arm | codewords/s | info Mb/s | gf2 ratio |");
    println!("|---|---|---|---|---|---|");
    for cell in BCH_CELLS {
        let code = Arc::new(cell.code());
        let (n, k) = (BlockCode::n(code.as_ref()), BlockCode::k(code.as_ref()));
        let r = n - k;
        let messages = Arc::new(messages(k, cell.batch, cell.seed()));
        let expected: Vec<BitVec> = messages
            .iter()
            .map(|m| {
                gf2_coding::traits::block::BlockEncoder::encode(code.as_ref(), m)
                    .expect("a message of length k")
            })
            .collect();
        let inputs = Arc::new(
            messages
                .iter()
                .map(|m| msb_first(m, k.div_ceil(8)))
                .collect::<Vec<_>>(),
        );
        let (m, t, prim) = bch_parameters(&cell);
        let name = format!("{:?} n={n} k={k}", cell.frame);
        let row = |arm: &str, fps: f64, gf2: Option<f64>| {
            let ratio = gf2.map_or("-".to_owned(), |g| format!("{:.2}", g / fps));
            println!(
                "| {name} | {} | {arm} | {fps:.0} | {:.1} | {ratio} |",
                cell.batch,
                fps * k as f64 / 1e6
            );
        };
        let bchlib_ok = k % 8 == 0
            && BchlibArm::new(m, t, prim, inputs.clone()).is_some_and(|mut arm| {
                (0..messages.len()).all(|i| {
                    arm.encode(i);
                    arm.codeword(i, k, r) == expected[i]
                })
            });
        if !bchlib_ok {
            println!("# {name}: bchlib does not reproduce gf2's codewords; not timed");
        }
        let gf2 = frames_per_second(1, cell.batch, || {
            Box::new(Gf2BchArm::new(code.clone(), messages.clone()))
        });
        row("gf2", gf2, None);
        if bchlib_ok {
            let fps = frames_per_second(1, cell.batch, || {
                Box::new(BchlibArm::new(m, t, prim, inputs.clone()).expect("validated above"))
            });
            row("bchlib", fps, Some(gf2));
        }
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("nr") => nr(),
        Some("bch") => bch(),
        _ => {
            eprintln!("usage: encode-compare nr|bch");
            std::process::exit(2);
        }
    }
}
