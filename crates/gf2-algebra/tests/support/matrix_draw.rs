use gf2_core::gfp::Fp;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

const RNG_BUFFER_BYTES: usize = 64;

pub(super) struct MatrixDraw {
    rng: ChaCha20Rng,
    bytes: [u8; RNG_BUFFER_BYTES],
    cursor: usize,
}

impl MatrixDraw {
    pub(super) fn addressed(
        root: u64,
        field_order: u64,
        dimension: usize,
        purpose_tag: u8,
        stream: u64,
    ) -> Self {
        let final_word = (u64::from(purpose_tag) << 56) | stream;
        let words = [root, field_order, dimension as u64, final_word];
        let mut seed = [0; 32];
        for (index, word) in words.into_iter().enumerate() {
            let offset = index * 8;
            seed[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
        }

        Self {
            rng: ChaCha20Rng::from_seed(seed),
            bytes: [0; RNG_BUFFER_BYTES],
            cursor: RNG_BUFFER_BYTES,
        }
    }

    pub(super) fn fill_matrix<const Q: u64>(&mut self, entries: &mut [Fp<Q>]) {
        for entry in entries {
            *entry = self.next_entry();
        }
    }

    fn next_entry<const Q: u64>(&mut self) -> Fp<Q> {
        let bound = 256 - 256 % Q;
        loop {
            let byte = self.next_raw_byte();
            if u64::from(byte) < bound {
                return Fp::new(u64::from(byte) % Q);
            }
        }
    }

    fn next_raw_byte(&mut self) -> u8 {
        if self.cursor == self.bytes.len() {
            self.rng.fill_bytes(&mut self.bytes);
            self.cursor = 0;
        }
        let byte = self.bytes[self.cursor];
        self.cursor += 1;
        byte
    }
}
