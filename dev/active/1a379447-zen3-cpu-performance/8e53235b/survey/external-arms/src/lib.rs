//! External BCH encoders of the `4e732b56` survey behind the runner's arm
//! contract (jit:1358d6ab).
//!
//! An [`Engine`] holds one batch of a case's messages in its library's own
//! representation and encodes the whole batch per timed call. Every
//! preparation first maps the engine's codewords into gf2's user coordinates
//! and requires them to equal [`bch_reference`]'s bit for bit, so no arm
//! measures an encoder that writes another code.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr};
use std::hint::black_box;
use std::io;
use std::time::Duration;

use consumer_profile_gf2_side::{bch_messages, bch_reference, per_call_ns, BchReference, Case};
use gf2_coding::bch::encode::SystematicLayout;
use gf2_core::BitVec;
use tuning_campaign_support::arm::{ArmRequest, ArmResult, ARM_RESULT_SCHEMA};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::CacheState;
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::execution_windows_configured;
use tuning_campaign_support::transport;

/// Environment naming the engine an arm runs.
pub const ENGINE_VAR: &str = "GF2_BCH_EXTERNAL";

extern "C" {
    fn gf2_aff3ct_bch_new(
        m: c_int,
        prim: u32,
        t: c_int,
        k: c_int,
        n: c_int,
        inter: c_int,
    ) -> *mut c_void;
    fn gf2_aff3ct_bch_frames_per_wave(handle: *mut c_void) -> c_int;
    fn gf2_aff3ct_bch_encode(
        handle: *mut c_void,
        u: *const c_int,
        x: *mut c_int,
        k: c_int,
        n: c_int,
        waves: c_int,
    );
    fn gf2_aff3ct_bch_free(handle: *mut c_void);
    fn gf2_aff3ct_version() -> *const c_char;
    fn gf2_aff3ct_instructions() -> *const c_char;

    fn bch_init(m: c_int, t: c_int, prim_poly: c_uint, swap_bits: bool) -> *mut BchControl;
    fn bch_free(bch: *mut BchControl);
    fn bch_encode(bch: *mut BchControl, data: *const u8, len: c_uint, ecc: *mut u8);
}

/// The leading public fields of bchlib's `struct bch_control`.
#[repr(C)]
struct BchControl {
    m: c_uint,
    n: c_uint,
    t: c_uint,
    ecc_bits: c_uint,
    ecc_bytes: c_uint,
}

/// The external encoders an arm can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    /// AFF3CT `Encoder_BCH<int>`, one frame per call of its LFSR.
    Aff3ctLfsrScalar,
    /// AFF3CT `Encoder_BCH_inter<int>`, one SIMD wave of frames per call.
    Aff3ctLfsrSimdInter,
    /// bchlib `bch_encode`, the Linux kernel's table-driven remainder.
    BchlibTableRemainder,
}

impl EngineKind {
    /// Every engine, in the order the verification record lists them.
    pub const ALL: [Self; 3] = [
        Self::Aff3ctLfsrScalar,
        Self::Aff3ctLfsrSimdInter,
        Self::BchlibTableRemainder,
    ];

    /// Decodes the spelling of [`ENGINE_VAR`].
    ///
    /// # Errors
    ///
    /// Returns the unknown spelling.
    pub fn parse(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == value)
            .ok_or_else(|| format!("unknown engine {value:?}"))
    }

    /// The spelling [`ENGINE_VAR`] carries.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aff3ctLfsrScalar => "aff3ct-lfsr-scalar",
            Self::Aff3ctLfsrSimdInter => "aff3ct-lfsr-simd-inter",
            Self::BchlibTableRemainder => "bchlib-table-remainder",
        }
    }
}

/// Polynomial degree that user coordinate `u` of `layout` carries
/// (`SystematicLayout`'s contract).
fn degree(layout: SystematicLayout, n: usize, k: usize, u: usize) -> Result<usize, String> {
    match layout {
        SystematicLayout::MessageParityAscending => Ok((u + n - k) % n),
        SystematicLayout::MessageParityDescending => Ok(n - 1 - u),
        other => Err(format!("no coordinate map for layout {other:?}")),
    }
}

enum Native {
    /// One `int` per bit, degree `j` of a frame at index `j`; messages hold
    /// degrees `n - k` upward.
    Aff3ct {
        handle: *mut c_void,
        frames_per_wave: usize,
        messages: Vec<c_int>,
        codewords: Vec<c_int>,
    },
    /// Packed bytes, most significant bit first: stream bit `j` of a frame,
    /// message then parity, is degree `n - 1 - j`.
    Bchlib {
        control: *mut BchControl,
        message_bytes: usize,
        parity_bytes: usize,
        messages: Vec<u8>,
        parity: Vec<u8>,
    },
}

/// Why [`Engine::prepare`] yields no engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The engine does not serve the case: a parameter its library rejects, a
    /// batch that is no whole number of waves, a message that is no whole
    /// number of bytes, or a case that declares no BCH code.
    Declined(String),
    /// A codeword differs from gf2's; the message names the first.
    Differs(String),
}

impl From<String> for Refusal {
    fn from(reason: String) -> Self {
        Self::Declined(reason)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declined(reason) | Self::Differs(reason) => formatter.write_str(reason),
        }
    }
}

/// A prepared batch: native buffers filled, codewords held to the reference.
pub struct Engine {
    kind: EngineKind,
    native: Native,
    reference: BchReference,
    /// Per-call costs measured outside the timed call.
    pub conversion: ConversionCosts,
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: each pointer came from its library's constructor, is non-null
        // by `Engine::prepare` and is released once, here.
        unsafe {
            match self.native {
                Native::Aff3ct { handle, .. } => gf2_aff3ct_bch_free(handle),
                Native::Bchlib { control, .. } => bch_free(control),
            }
        }
    }
}

fn int(value: usize) -> Result<c_int, String> {
    c_int::try_from(value).map_err(|_| format!("{value} exceeds a C int"))
}

impl Engine {
    /// Builds `kind` for the case's code, fills its native message batch and
    /// requires its codewords to equal the reference's.
    ///
    /// # Errors
    ///
    /// Returns the [`Refusal`].
    pub fn prepare(kind: EngineKind, case: &Case) -> Result<Self, Refusal> {
        let reference = bch_reference(case)?;
        let (n, k) = (reference.n, reference.k);
        let batch = reference.messages.len();
        let mut setup = Ok(None);
        let setup_ns = per_call_ns(1, || setup = Self::native(kind, &reference));
        let native = setup?.ok_or_else(|| format!("{} rejects this code", kind.name()))?;
        let mut engine = Self {
            kind,
            native,
            reference,
            conversion: ConversionCosts {
                setup_ns,
                pack_ns: 0,
                unpack_ns: 0,
                batch_fill_ns: 0,
                dispatch_ns: 0,
            },
        };
        if let Native::Aff3ct {
            frames_per_wave, ..
        } = engine.native
        {
            if batch % frames_per_wave != 0 {
                return Err(Refusal::Declined(format!(
                    "a batch of {batch} is no whole number of {frames_per_wave}-frame waves"
                )));
            }
        }
        let mut packed = Ok(());
        engine.conversion.pack_ns = per_call_ns(4, || packed = engine.pack());
        packed?;
        engine.encode();
        let mut codewords = Ok(Vec::new());
        engine.conversion.unpack_ns = per_call_ns(4, || codewords = engine.codewords());
        engine.conversion.batch_fill_ns = per_call_ns(4, || {
            black_box(bch_messages(k, case.seed, batch));
        });
        if let Some(index) = codewords?
            .iter()
            .zip(&engine.reference.codewords)
            .position(|(external, reference)| external != reference)
        {
            return Err(Refusal::Differs(format!(
                "{} codeword {index} of the ({n}, {k}) code differs from gf2's",
                kind.name()
            )));
        }
        Ok(engine)
    }

    fn native(kind: EngineKind, reference: &BchReference) -> Result<Option<Native>, String> {
        let (n, k, batch) = (reference.n, reference.k, reference.messages.len());
        let prim = u32::try_from(reference.modulus).map_err(|_| "the modulus exceeds 32 bits")?;
        Ok(match kind {
            EngineKind::Aff3ctLfsrScalar | EngineKind::Aff3ctLfsrSimdInter => {
                let inter = c_int::from(kind == EngineKind::Aff3ctLfsrSimdInter);
                // SAFETY: plain integers in; the shim catches every C++ exception
                // and reports a rejected construction as null.
                let handle = unsafe {
                    gf2_aff3ct_bch_new(
                        int(reference.degree)?,
                        prim,
                        int(reference.t)?,
                        int(k)?,
                        int(n)?,
                        inter,
                    )
                };
                if handle.is_null() {
                    return Ok(None);
                }
                // SAFETY: `handle` is a live non-null shim handle.
                let frames_per_wave = unsafe { gf2_aff3ct_bch_frames_per_wave(handle) } as usize;
                Some(Native::Aff3ct {
                    handle,
                    frames_per_wave,
                    messages: vec![0; batch * k],
                    codewords: vec![0; batch * n],
                })
            }
            EngineKind::BchlibTableRemainder => {
                if k % 8 != 0 {
                    return Err(format!("bch_encode takes whole bytes; k = {k} is not"));
                }
                // SAFETY: plain integers in; bchlib returns null on rejected
                // parameters.
                let control =
                    unsafe { bch_init(int(reference.degree)?, int(reference.t)?, prim, false) };
                if control.is_null() {
                    return Ok(None);
                }
                // SAFETY: `control` is a live non-null control structure whose
                // leading fields `BchControl` mirrors.
                let (ecc_bits, ecc_bytes, mother) = unsafe {
                    (
                        (*control).ecc_bits as usize,
                        (*control).ecc_bytes as usize,
                        (*control).n as usize,
                    )
                };
                if ecc_bits != n - k || n > mother {
                    // SAFETY: released once; the engine never holds it.
                    unsafe { bch_free(control) };
                    return Ok(None);
                }
                Some(Native::Bchlib {
                    control,
                    message_bytes: k / 8,
                    parity_bytes: ecc_bytes,
                    messages: vec![0; batch * k / 8],
                    parity: vec![0; batch * ecc_bytes],
                })
            }
        })
    }

    fn pack(&mut self) -> Result<(), String> {
        let BchReference {
            n,
            k,
            layout,
            ref messages,
            ..
        } = self.reference;
        match &mut self.native {
            Native::Aff3ct {
                messages: native, ..
            } => {
                for (frame, message) in native.chunks_exact_mut(k).zip(messages) {
                    for u in 0..k {
                        frame[degree(layout, n, k, u)? - (n - k)] = c_int::from(message.get(u));
                    }
                }
            }
            Native::Bchlib {
                message_bytes,
                messages: native,
                ..
            } => {
                native.fill(0);
                for (frame, message) in native.chunks_exact_mut(*message_bytes).zip(messages) {
                    for u in 0..k {
                        if message.get(u) {
                            let bit = n - 1 - degree(layout, n, k, u)?;
                            frame[bit / 8] |= 0x80 >> (bit % 8);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Encodes the whole batch once: the timed call.
    pub fn encode(&mut self) {
        let BchReference { n, k, .. } = self.reference;
        match &mut self.native {
            Native::Aff3ct {
                handle,
                frames_per_wave,
                messages,
                codewords,
            } => {
                let waves = messages.len() / k / *frames_per_wave;
                // SAFETY: `messages` holds `waves * frames_per_wave` frames of
                // `k` ints and `codewords` as many of `n`, the extents one
                // shim call reads and writes; the lengths fit a C int by
                // `prepare`.
                unsafe {
                    gf2_aff3ct_bch_encode(
                        *handle,
                        black_box(messages.as_ptr()),
                        codewords.as_mut_ptr(),
                        k as c_int,
                        n as c_int,
                        waves as c_int,
                    );
                }
            }
            Native::Bchlib {
                control,
                message_bytes,
                parity_bytes,
                messages,
                parity,
            } => {
                // bch_encode accumulates into its parity buffer.
                parity.fill(0);
                for (message, ecc) in messages
                    .chunks_exact(*message_bytes)
                    .zip(parity.chunks_exact_mut(*parity_bytes))
                {
                    // SAFETY: `message` holds `message_bytes` bytes, within the
                    // control's message capacity by `native`, and `ecc` the
                    // control's `ecc_bytes`.
                    unsafe {
                        bch_encode(
                            *control,
                            black_box(message.as_ptr()),
                            *message_bytes as c_uint,
                            ecc.as_mut_ptr(),
                        );
                    }
                }
            }
        }
    }

    /// The batch's codewords in gf2's user coordinates.
    fn codewords(&self) -> Result<Vec<BitVec>, String> {
        let BchReference {
            n,
            k,
            layout,
            ref messages,
            ..
        } = self.reference;
        let mut out = Vec::with_capacity(messages.len());
        for frame in 0..messages.len() {
            let mut codeword = BitVec::zeros(n);
            for u in 0..n {
                let d = degree(layout, n, k, u)?;
                let bit = match &self.native {
                    Native::Aff3ct { codewords, .. } => codewords[frame * n + d] != 0,
                    Native::Bchlib {
                        message_bytes,
                        parity_bytes,
                        messages,
                        parity,
                        ..
                    } => {
                        let j = n - 1 - d;
                        let (bytes, j) = if j < k {
                            (&messages[frame * message_bytes..], j)
                        } else {
                            (&parity[frame * parity_bytes..], j - k)
                        };
                        bytes[j / 8] & (0x80 >> (j % 8)) != 0
                    }
                };
                codeword.set(u, bit);
            }
            out.push(codeword);
        }
        Ok(out)
    }

    /// The route and library identity the arm observed, for the receipt.
    pub fn selected_path(&self) -> String {
        let (n, k, batch) = (
            self.reference.n,
            self.reference.k,
            self.reference.messages.len(),
        );
        let library = match self.native {
            Native::Aff3ct {
                frames_per_wave, ..
            } => {
                // SAFETY: the shim returns static NUL-terminated strings.
                let (version, instructions) = unsafe {
                    (
                        CStr::from_ptr(gf2_aff3ct_version()),
                        CStr::from_ptr(gf2_aff3ct_instructions()),
                    )
                };
                format!(
                    "AFF3CT {} at {} (library sha256 {}), {} frames per wave, MIPP {}",
                    version.to_string_lossy(),
                    env!("BCH_EXTERNAL_AFF3CT_REVISION"),
                    env!("BCH_EXTERNAL_AFF3CT_LIBRARY_SHA256"),
                    frames_per_wave,
                    instructions.to_string_lossy(),
                )
            }
            Native::Bchlib { .. } => {
                format!("bchlib bch.c at {}", env!("BCH_EXTERNAL_BCHLIB_REVISION"))
            }
        };
        format!(
            "{}/({n}, {k}) batch {batch}, codewords equal gf2's; {library}",
            self.kind.name()
        )
    }
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("bch-external-arm: {}", message.as_ref());
    std::process::exit(2);
}

/// Serves one runner request on stdin with the engine [`ENGINE_VAR`] names.
pub fn serve() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: ArmRequest =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fail(error),
        };
    let case: Case = serde_json::from_value(request.case.clone())
        .unwrap_or_else(|error| fail(format!("case does not decode: {error}")));
    let kind = std::env::var(ENGINE_VAR)
        .map_err(|_| format!("{ENGINE_VAR} is absent"))
        .and_then(|value| EngineKind::parse(&value))
        .unwrap_or_else(|error| fail(error));
    // The one bank is cache-resident after preparation's encode; a cold or
    // streaming cell would need banks this arm does not hold.
    if request.cache_state != CacheState::Warm {
        fail("the external arm serves warm cells only");
    }
    let mut engine =
        Engine::prepare(kind, &case).unwrap_or_else(|refusal| fail(refusal.to_string()));
    let mut body = |_bank: usize| engine.encode();
    let windows = if request.windows == 0 {
        body(0);
        Vec::new()
    } else {
        execution_windows_configured(
            0,
            u64::from(request.windows),
            Duration::from_millis(u64::from(request.window_target_ms)),
            &mut body,
            |_| Ok(()),
        )
        .unwrap_or_else(|error| fail(format!("timing failed: {error}")))
        .iter()
        .map(|sample| WindowRecord {
            calls: sample.calls,
            elapsed_ns: sample.elapsed_ns,
        })
        .collect()
    };
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows,
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed: CpuAffinity::observe()
            .map(|affinity| affinity.cpus().to_vec())
            .unwrap_or_default(),
        selected_path: Some(engine.selected_path()),
        conversion: Some(engine.conversion),
        quality: None,
        calibrated: None,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn dvb_short(batch: u64) -> Case {
        Case {
            workload: "dvb-bch-encode".to_owned(),
            size: BTreeMap::from([("n".to_owned(), 7200), ("batch".to_owned(), batch)]),
            seed: 7,
        }
    }

    #[test]
    fn one_flipped_codeword_bit_is_seen() {
        for kind in [
            EngineKind::Aff3ctLfsrScalar,
            EngineKind::BchlibTableRemainder,
        ] {
            let mut engine = Engine::prepare(kind, &dvb_short(2)).expect("the engine serves");
            match &mut engine.native {
                Native::Aff3ct { codewords, .. } => codewords[7200 + 3] ^= 1,
                Native::Bchlib { parity, .. } => parity[0] ^= 0x80,
            }
            let codewords = engine.codewords().expect("the layout maps");
            let differing: Vec<bool> = codewords
                .iter()
                .zip(&engine.reference.codewords)
                .map(|(external, reference)| external != reference)
                .collect();
            let expected = match kind {
                EngineKind::BchlibTableRemainder => [true, false],
                _ => [false, true],
            };
            assert_eq!(differing, expected, "{}", kind.name());
        }
    }

    #[test]
    fn an_engine_declines_what_its_library_cannot_encode() {
        assert!(matches!(
            Engine::prepare(EngineKind::Aff3ctLfsrSimdInter, &dvb_short(1)),
            Err(Refusal::Declined(_))
        ));
    }
}
