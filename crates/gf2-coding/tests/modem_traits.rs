//! Integration test: ensure the modem batched-trait surface is publicly
//! reachable from the crate root.
//!
//! This intentionally exercises only the public types and trait shapes;
//! correctness of any specific demapper math lives with the backend
//! implementations in tasks `51334873` (reference path) and `52112411`
//! (Gray-QAM fast path).

use gf2_coding::llr::Llr;
use gf2_coding::modem::{
    BatchHardDemapper, BatchMapper, BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec,
    ModemView,
};

struct StubModem {
    spec: ModemSpec<f32>,
}

impl BatchMapper<f32> for StubModem {
    fn spec(&self) -> ModemView<'_, f32> {
        self.spec.view()
    }

    fn map_bits(&self, bits: &[bool], out_i: &mut [f32], out_q: &mut [f32]) {
        let bps = self.spec.bits_per_symbol() as usize;
        assert_eq!(bits.len() % bps, 0);
        let n = bits.len() / bps;
        assert_eq!(out_i.len(), n);
        assert_eq!(out_q.len(), n);
        out_i.fill(0.0);
        out_q.fill(0.0);
    }
}

impl BatchSoftDemapper<f32> for StubModem {
    fn spec(&self) -> ModemView<'_, f32> {
        self.spec.view()
    }

    fn demap_llrs(&self, input: DemapInput<'_, f32>, out_llrs: &mut [Llr]) {
        let n = input.rx_i.len();
        let m = self.spec.bits_per_symbol() as usize;
        assert_eq!(input.rx_q.len(), n);
        assert_eq!(input.noise_var.len(), n);
        assert_eq!(out_llrs.len(), n * m);
        for slot in out_llrs.iter_mut() {
            *slot = Llr::new(0.0);
        }
    }
}

impl BatchHardDemapper<f32> for StubModem {
    fn spec(&self) -> ModemView<'_, f32> {
        self.spec.view()
    }

    fn demap_bits(&self, input: DemapInput<'_, f32>, out_bits: &mut [bool]) {
        let n = input.rx_i.len();
        let m = self.spec.bits_per_symbol() as usize;
        assert_eq!(input.rx_q.len(), n);
        assert_eq!(input.noise_var.len(), n);
        assert_eq!(out_bits.len(), n * m);
        out_bits.fill(false);
    }
}

#[test]
fn test_batched_trait_surface_is_public() {
    let modem = StubModem {
        spec: ModemSpec::gray_square_qam(4),
    };

    // BatchMapper path.
    let bps = BatchMapper::<f32>::spec(&modem).bits_per_symbol() as usize;
    assert_eq!(bps, 2);
    let bits = vec![false; bps * 3];
    let mut out_i = vec![1.0_f32; 3];
    let mut out_q = vec![1.0_f32; 3];
    modem.map_bits(&bits, &mut out_i, &mut out_q);
    assert!(out_i.iter().all(|x| *x == 0.0));
    assert!(out_q.iter().all(|x| *x == 0.0));

    // DemapInput construction over the public surface.
    let rx_i = [0.1_f32, -0.2, 0.3];
    let rx_q = [-0.1_f32, 0.2, -0.3];
    let noise_var = [0.05_f32; 3];
    let input = DemapInput::<f32> {
        rx_i: &rx_i,
        rx_q: &rx_q,
        gain_i: None,
        gain_q: None,
        noise_var: &noise_var,
        method: DemapMethod::MaxLog,
    };

    // Soft demap path.
    let mut out_llrs = vec![Llr::new(1.0); rx_i.len() * bps];
    BatchSoftDemapper::demap_llrs(&modem, input, &mut out_llrs);
    assert!(out_llrs.iter().all(|l| l.value() == 0.0));

    // Hard demap path; reuse the same DemapInput (it's Copy).
    let mut out_bits = vec![true; rx_i.len() * bps];
    BatchHardDemapper::demap_bits(&modem, input, &mut out_bits);
    assert!(out_bits.iter().all(|b| !*b));
}
