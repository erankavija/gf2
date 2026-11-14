//! The external arms of the BCH cutover comparison: bchlib and AFF3CT on W1,
//! M4RI and AFF3CT on W2, the baselines the epic's survey selected, each in
//! every configuration its build offers so the comparison can take the
//! fastest. Every configuration writes gf2's descending layout for the
//! harness's digest and converts outside the timed call, except the AFF3CT W2
//! arm. AFF3CT 4.7.0 has no generator-matrix entry point, so
//! `experiments/encode-compare/external/cpp/aff3ct_bch.cpp` (`generator`)
//! composes it: inside the timed call it allocates a zeroed packed matrix,
//! encodes the unit messages in waves and packs each codeword into rows. The
//! packed matrix is the W2 output contract gf2's timed `generator_matrix` meets.

use super::{from_msb_first, BchlibBuild, BCHLIB_BUILDS};
use encode_compare::cutover::harness::{self, digest, Arm, Cell, Config, Prepared, Workload};
use std::ffi::c_void;

extern "C" {
    fn gf2_aff3ct_bch_new_i8(k: i32, n: i32, m: i32, t: i32, prim: u32, inter: i32) -> *mut c_void;
    fn gf2_aff3ct_bch_new_i32(k: i32, n: i32, m: i32, t: i32, prim: u32, inter: i32)
        -> *mut c_void;
    fn gf2_aff3ct_bch_frames(handle: *mut c_void) -> i32;
    fn gf2_aff3ct_bch_encode(handle: *mut c_void, u: *const c_void, x: *mut c_void, waves: i32);
    fn gf2_aff3ct_bch_generator(handle: *mut c_void, rows: *mut u64, words_per_row: i32);
    fn gf2_aff3ct_bch_free(handle: *mut c_void);
}

fn bit(words: &[u64], index: usize) -> bool {
    words[index / 64] >> (index % 64) & 1 == 1
}

/// `words` holding `bits` bits, packed most significant bit first, bchlib's
/// data layout.
fn msb_first(words: &[u64], bits: usize) -> Vec<u8> {
    let mut out = vec![0u8; bits.div_ceil(8)];
    for i in (0..bits).filter(|&i| bit(words, i)) {
        out[i / 8] |= 0x80 >> (i % 8);
    }
    out
}

struct Bchlib;

struct BchlibConfig {
    build: BchlibBuild,
    handle: *mut c_void,
    inputs: Vec<Vec<u8>>,
    ecc: Vec<u8>,
    ecc_bytes: usize,
    k: usize,
    n: usize,
}

impl Config for BchlibConfig {
    fn label(&self) -> String {
        self.build.label.to_owned()
    }

    fn call(&mut self) {
        for (data, ecc) in self.inputs.iter().zip(self.ecc.chunks_mut(self.ecc_bytes)) {
            // SAFETY: `data` holds the message bytes and `ecc` the control's
            // ecc_bytes, the lengths `bch_encode` reads and writes.
            unsafe {
                (self.build.encode)(
                    self.handle,
                    data.as_ptr(),
                    data.len() as u32,
                    ecc.as_mut_ptr(),
                )
            }
        }
    }

    fn output_digest(&mut self) -> u64 {
        let r = self.n - self.k;
        let codewords: Vec<gf2_core::BitVec> = self
            .inputs
            .iter()
            .zip(self.ecc.chunks(self.ecc_bytes))
            .map(|(data, ecc)| {
                let mut out = gf2_core::BitVec::zeros(self.n);
                out.copy_bits(0, &from_msb_first(data, self.k), 0, self.k);
                out.copy_bits(self.k, &from_msb_first(ecc, r), 0, r);
                out
            })
            .collect();
        digest(
            codewords.len(),
            self.n,
            codewords.iter().map(gf2_core::BitVec::words),
        )
    }
}

impl Drop for BchlibConfig {
    fn drop(&mut self) {
        // SAFETY: `handle` came from this build's `new` and is freed once.
        unsafe { (self.build.free)(self.handle) }
    }
}

impl Arm for Bchlib {
    fn role(&self) -> &'static str {
        "external"
    }

    fn name(&self) -> String {
        "bchlib".to_owned()
    }

    fn configs<'a>(&'a self, cell: &Cell, messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>> {
        let row = cell.row;
        if cell.workload != Workload::W1 {
            return Vec::new();
        }
        let (m, t) = (row.m as i32, row.t as i32);
        BCHLIB_BUILDS
            .iter()
            .filter(|build| build.constant.is_none_or(|fixed| fixed == (m, t)))
            .map(|&build| -> Prepared<'a> {
                let skip = |reason: &str| Err((build.label.to_owned(), reason.to_owned()));
                if !row.k.is_multiple_of(8) {
                    return skip("bch_encode takes whole message bytes");
                }
                // SAFETY: null signals rejection; a non-null handle is owned
                // by the configuration.
                let handle = unsafe { (build.new)(m, t, row.prim as u32) };
                if handle.is_null() {
                    return skip("bch_init rejects the field or radius");
                }
                // SAFETY: `handle` is a live control of this build.
                let ecc_bits = unsafe { (build.ecc_bits)(handle) } as usize;
                let ecc_bytes = ecc_bits.div_ceil(8);
                let config = BchlibConfig {
                    build,
                    handle,
                    inputs: messages.iter().map(|w| msb_first(w, row.k)).collect(),
                    ecc: vec![0; ecc_bytes * messages.len()],
                    ecc_bytes,
                    k: row.k,
                    n: row.n,
                };
                if ecc_bits != row.n - row.k {
                    return skip("bchlib derives another generator degree");
                }
                Ok(Box::new(config))
            })
            .collect()
    }
}

/// One AFF3CT bit type: its element buffers and constructor.
#[derive(Clone, Copy)]
enum Precision {
    I8,
    I32,
}

struct Aff3ct;

struct Aff3ctConfig {
    handle: *mut c_void,
    label: String,
    precision: Precision,
    frames: usize,
    batch: usize,
    k: usize,
    n: usize,
    workload: Workload,
    u8s: Vec<i8>,
    x8s: Vec<i8>,
    u32s: Vec<i32>,
    x32s: Vec<i32>,
    matrix: Vec<u64>,
}

impl Aff3ctConfig {
    fn waves(&self) -> usize {
        self.batch.div_ceil(self.frames)
    }

    fn entry(&self, frame: usize, index: usize) -> bool {
        match self.precision {
            Precision::I8 => self.x8s[frame * self.n + index] != 0,
            Precision::I32 => self.x32s[frame * self.n + index] != 0,
        }
    }
}

impl Config for Aff3ctConfig {
    fn label(&self) -> String {
        self.label.clone()
    }

    fn call(&mut self) {
        match self.workload {
            Workload::W1 => {
                let (u, x) = match self.precision {
                    Precision::I8 => (
                        self.u8s.as_ptr() as *const c_void,
                        self.x8s.as_mut_ptr() as *mut c_void,
                    ),
                    Precision::I32 => (
                        self.u32s.as_ptr() as *const c_void,
                        self.x32s.as_mut_ptr() as *mut c_void,
                    ),
                };
                // SAFETY: `u` and `x` hold `waves * frames` frames of k and n
                // elements of the handle's bit type.
                unsafe { gf2_aff3ct_bch_encode(self.handle, u, x, self.waves() as i32) }
            }
            Workload::W2 => {
                let words = self.n.div_ceil(64);
                self.matrix = Vec::new();
                self.matrix = vec![0u64; self.k * words];
                // SAFETY: `matrix` holds k rows of `words` zeroed words.
                unsafe {
                    gf2_aff3ct_bch_generator(self.handle, self.matrix.as_mut_ptr(), words as i32)
                }
            }
        }
    }

    fn output_digest(&mut self) -> u64 {
        let words = self.n.div_ceil(64);
        match self.workload {
            Workload::W1 => {
                // gf2 codeword bit c is AFF3CT entry n - 1 - c.
                let codewords: Vec<Vec<u64>> = (0..self.batch)
                    .map(|frame| {
                        let mut out = vec![0u64; words];
                        for c in (0..self.n).filter(|&c| self.entry(frame, self.n - 1 - c)) {
                            out[c / 64] |= 1 << (c % 64);
                        }
                        out
                    })
                    .collect();
                digest(self.batch, self.n, codewords.iter().map(Vec::as_slice))
            }
            Workload::W2 => digest(self.k, self.n, self.matrix.chunks(words)),
        }
    }
}

impl Drop for Aff3ctConfig {
    fn drop(&mut self) {
        // SAFETY: `handle` came from a constructor and is freed once.
        unsafe { gf2_aff3ct_bch_free(self.handle) }
    }
}

impl Arm for Aff3ct {
    fn role(&self) -> &'static str {
        "external"
    }

    fn name(&self) -> String {
        "AFF3CT 4.7.0".to_owned()
    }

    fn configs<'a>(&'a self, cell: &Cell, messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>> {
        let row = cell.row;
        let (k, n) = (row.k, row.n);
        let mut out = Vec::new();
        for (precision, bits) in [(Precision::I8, "int8"), (Precision::I32, "int32")] {
            for (inter, encoder) in [(0, "Encoder_BCH"), (1, "Encoder_BCH_inter")] {
                let label = format!("{encoder} {bits}");
                let args = (
                    k as i32,
                    n as i32,
                    row.m as i32,
                    row.t as i32,
                    row.prim as u32,
                );
                // SAFETY: null signals rejection; a non-null handle is owned
                // by the configuration.
                let handle = unsafe {
                    match precision {
                        Precision::I8 => {
                            gf2_aff3ct_bch_new_i8(args.0, args.1, args.2, args.3, args.4, inter)
                        }
                        Precision::I32 => {
                            gf2_aff3ct_bch_new_i32(args.0, args.1, args.2, args.3, args.4, inter)
                        }
                    }
                };
                if handle.is_null() {
                    out.push(Err((label, "AFF3CT rejects the code".to_owned())));
                    continue;
                }
                // SAFETY: `handle` is live.
                let frames = unsafe { gf2_aff3ct_bch_frames(handle) } as usize;
                let batch = match cell.workload {
                    Workload::W1 => messages.len(),
                    Workload::W2 => 0,
                };
                let elements = batch.div_ceil(frames) * frames;
                // gf2 message bit i is AFF3CT entry k - 1 - i; padding frames
                // of a partial wave stay zero.
                let mut u = vec![0i32; elements * k];
                for (frame, words) in messages.iter().enumerate() {
                    for i in (0..k).filter(|&i| bit(words, i)) {
                        u[frame * k + k - 1 - i] = 1;
                    }
                }
                let mut config = Aff3ctConfig {
                    handle,
                    label,
                    precision,
                    frames,
                    batch,
                    k,
                    n,
                    workload: cell.workload,
                    u8s: Vec::new(),
                    x8s: Vec::new(),
                    u32s: Vec::new(),
                    x32s: Vec::new(),
                    matrix: Vec::new(),
                };
                match precision {
                    Precision::I8 => {
                        config.u8s = u.iter().map(|&v| v as i8).collect();
                        config.x8s = vec![0; elements * n];
                    }
                    Precision::I32 => {
                        config.u32s = u;
                        config.x32s = vec![0; elements * n];
                    }
                }
                out.push(Ok(Box::new(config) as Box<dyn Config>));
            }
        }
        out
    }
}

struct M4ri {
    /// Generator coefficients per contract row, from gf2's code.
    generators: Vec<Vec<u8>>,
}

#[cfg(gf2_m4ri)]
mod m4ri {
    use super::*;

    extern "C" {
        fn gf2_m4ri_generator(k: i32, n: i32, g: *const u8, deg: i32, method: i32) -> *mut c_void;
        fn gf2_m4ri_row(matrix: *mut c_void, row: i32) -> *const u64;
        fn gf2_m4ri_free(matrix: *mut c_void);
    }

    pub(super) struct M4riConfig<'a> {
        pub(super) method: i32,
        pub(super) generator: &'a [u8],
        pub(super) k: usize,
        pub(super) n: usize,
        pub(super) matrix: *mut c_void,
    }

    impl M4riConfig<'_> {
        fn release(&mut self) {
            if !self.matrix.is_null() {
                // SAFETY: `matrix` came from `gf2_m4ri_generator` and is
                // freed once.
                unsafe { gf2_m4ri_free(self.matrix) };
                self.matrix = std::ptr::null_mut();
            }
        }
    }

    impl Config for M4riConfig<'_> {
        fn label(&self) -> String {
            match self.method {
                0 => "mzd_echelonize_m4ri".to_owned(),
                _ => "mzd_echelonize_pluq".to_owned(),
            }
        }

        fn call(&mut self) {
            self.release();
            // SAFETY: `generator` holds deg + 1 coefficients.
            self.matrix = unsafe {
                gf2_m4ri_generator(
                    self.k as i32,
                    self.n as i32,
                    self.generator.as_ptr(),
                    self.generator.len() as i32 - 1,
                    self.method,
                )
            };
        }

        fn output_digest(&mut self) -> u64 {
            let words = self.n.div_ceil(64);
            let rows: Vec<&[u64]> = (0..self.k)
                .map(|row| {
                    // SAFETY: a k x n matrix row holds `words` words and
                    // lives until the next call.
                    unsafe {
                        std::slice::from_raw_parts(gf2_m4ri_row(self.matrix, row as i32), words)
                    }
                })
                .collect();
            digest(self.k, self.n, rows)
        }
    }

    impl Drop for M4riConfig<'_> {
        fn drop(&mut self) {
            self.release();
        }
    }
}

impl Arm for M4ri {
    fn role(&self) -> &'static str {
        "external"
    }

    fn name(&self) -> String {
        "M4RI".to_owned()
    }

    fn configs<'a>(&'a self, cell: &Cell, _messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>> {
        if cell.workload != Workload::W2 {
            return Vec::new();
        }
        let index = harness::ROWS
            .iter()
            .position(|row| row.name == cell.row.name)
            .expect("a contract row");
        #[cfg(gf2_m4ri)]
        {
            (0..2)
                .map(|method| -> Prepared<'a> {
                    Ok(Box::new(m4ri::M4riConfig {
                        method,
                        generator: &self.generators[index],
                        k: cell.row.k,
                        n: cell.row.n,
                        matrix: std::ptr::null_mut(),
                    }))
                })
                .collect()
        }
        #[cfg(not(gf2_m4ri))]
        {
            let _ = &self.generators[index];
            vec![Err((
                "M4RI".to_owned(),
                "built without GF2_M4RI_ROOT".to_owned(),
            ))]
        }
    }
}

/// `encode-compare cutover round <n>`: one round of every external arm.
pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    let round = match (args.get(2).map(String::as_str), args.get(3)) {
        (Some("round"), Some(round)) => round.parse().expect("a round number"),
        _ => {
            eprintln!("usage: encode-compare cutover round <n>");
            std::process::exit(2);
        }
    };
    let m4ri = M4ri {
        generators: harness::ROWS
            .iter()
            .map(encode_compare::cutover::current::generator_coefficients)
            .collect(),
    };
    for arm in [&Bchlib as &dyn Arm, &Aff3ct, &m4ri] {
        harness::run_round(arm, round);
    }
}
