#![allow(clippy::many_single_char_names)]
use crate::LogicalFns;
use core::arch::x86_64::*;

const M4RM_TILE_ROWS: usize = 8;
const M4RM_TILE_WORDS: usize = 4;

#[inline(always)]
unsafe fn loadu(ptr: *const u8) -> __m256i {
    _mm256_loadu_si256(ptr as *const __m256i)
}

#[inline(always)]
unsafe fn storeu(ptr: *mut u8, v: __m256i) {
    _mm256_storeu_si256(ptr as *mut __m256i, v)
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_xor_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4; // 4 u64 per 256-bit vector
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_xor_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) ^= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_m4rm_gray_xor16(acc: &mut [[u64; 8]; 2], src: &[u64]) {
    debug_assert!(src.len() >= 16);

    // SAFETY: callers pass a fixed 16-word accumulator and at least 16 source
    // words. The loop performs four unaligned 32-byte loads/stores inside those
    // bounds and never aliases `src` mutably.
    let acc_ptr = acc.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < 4 {
        let off = (i * 32) as isize;
        let a = loadu(acc_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        storeu(acc_ptr.offset(off), _mm256_xor_si256(a, b));
        i += 1;
    }
}

/// AVX2 full Gray-code table build for stride_words == 4 (256-bit rows).
///
/// One YMM register holds the running accumulator; each Gray step is a single
/// load + XOR + store. The Gray-walk control (curr_gray, flipped bit) is scalar
/// and matrix-data-independent, so it stays out of the SIMD critical path.
#[target_feature(enable = "avx2")]
unsafe fn avx2_m4rm_gray_build4(
    buffer: &mut [u64],
    panel: &[u64],
    stride_words: usize,
    table_size: usize,
    valid_rows: usize,
) {
    debug_assert_eq!(stride_words, 4);
    debug_assert!(buffer.len() >= table_size * stride_words);
    debug_assert!(panel.len() >= valid_rows * stride_words);

    // SAFETY: the wrapper asserts `buffer.len() >= table_size * stride_words`
    // and `panel.len() >= valid_rows * stride_words` with `stride_words == 4`
    // (32 bytes/row). Every store targets `curr_gray * 32` bytes with
    // `curr_gray < table_size`, and every panel load reads `bit_pos * 32` bytes
    // guarded by `bit_pos < valid_rows`; all unaligned 32-byte accesses stay
    // within those bounds and never alias `panel` mutably.
    let buf = buffer.as_mut_ptr() as *mut u8;
    let pan = panel.as_ptr() as *const u8;

    // Entry 0 is the zero vector.
    let zero = _mm256_setzero_si256();
    storeu(buf, zero);

    let mut acc = zero;
    let mut prev_gray = 0usize;
    let mut i = 1usize;
    while i < table_size {
        let curr_gray = i ^ (i >> 1);
        let bit_pos = (prev_gray ^ curr_gray).trailing_zeros() as usize;
        if bit_pos < valid_rows {
            let row = loadu(pan.add(bit_pos * 32));
            acc = _mm256_xor_si256(acc, row);
        }
        storeu(buf.add(curr_gray * 32), acc);
        prev_gray = curr_gray;
        i += 1;
    }
}

/// AVX2 full Gray-code table build for stride_words == 8 (512-bit rows).
///
/// Two YMM accumulators cover the 8-word row; identical Gray-walk control flow
/// to the stride-4 builder.
#[target_feature(enable = "avx2")]
unsafe fn avx2_m4rm_gray_build8(
    buffer: &mut [u64],
    panel: &[u64],
    stride_words: usize,
    table_size: usize,
    valid_rows: usize,
) {
    debug_assert_eq!(stride_words, 8);
    debug_assert!(buffer.len() >= table_size * stride_words);
    debug_assert!(panel.len() >= valid_rows * stride_words);

    // SAFETY: the wrapper asserts `buffer.len() >= table_size * stride_words`
    // and `panel.len() >= valid_rows * stride_words` with `stride_words == 8`
    // (64 bytes/row). Each store pair targets `curr_gray * 64` and `+ 32` bytes
    // with `curr_gray < table_size`, and each panel load pair reads `bit_pos *
    // 64` and `+ 32` bytes guarded by `bit_pos < valid_rows`; all unaligned
    // 32-byte accesses stay within those bounds and never alias `panel` mutably.
    let buf = buffer.as_mut_ptr() as *mut u8;
    let pan = panel.as_ptr() as *const u8;

    let zero = _mm256_setzero_si256();
    storeu(buf, zero);
    storeu(buf.add(32), zero);

    let mut acc0 = zero;
    let mut acc1 = zero;
    let mut prev_gray = 0usize;
    let mut i = 1usize;
    while i < table_size {
        let curr_gray = i ^ (i >> 1);
        let bit_pos = (prev_gray ^ curr_gray).trailing_zeros() as usize;
        if bit_pos < valid_rows {
            let base = bit_pos * 64;
            acc0 = _mm256_xor_si256(acc0, loadu(pan.add(base)));
            acc1 = _mm256_xor_si256(acc1, loadu(pan.add(base + 32)));
        }
        let dst = buf.add(curr_gray * 64);
        storeu(dst, acc0);
        storeu(dst.add(32), acc1);
        prev_gray = curr_gray;
        i += 1;
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_m4rm_tile8x4(
    c_block: &mut [u64],
    stride_words: usize,
    word_start: usize,
    table_buffer: &[u64],
    idx: &[usize; M4RM_TILE_ROWS],
) {
    debug_assert!(c_block.len() >= 8 * stride_words);
    debug_assert!(word_start + 4 <= stride_words);

    let c = c_block.as_mut_ptr();
    let table = table_buffer.as_ptr();
    let load_c = |row: usize| loadu(c.add(row * stride_words + word_start).cast::<u8>());
    let load_t = |row: usize| loadu(table.add(idx[row] * stride_words + word_start).cast::<u8>());

    let acc0 = _mm256_xor_si256(load_c(0), load_t(0));
    let acc1 = _mm256_xor_si256(load_c(1), load_t(1));
    let acc2 = _mm256_xor_si256(load_c(2), load_t(2));
    let acc3 = _mm256_xor_si256(load_c(3), load_t(3));
    let acc4 = _mm256_xor_si256(load_c(4), load_t(4));
    let acc5 = _mm256_xor_si256(load_c(5), load_t(5));
    let acc6 = _mm256_xor_si256(load_c(6), load_t(6));
    let acc7 = _mm256_xor_si256(load_c(7), load_t(7));

    storeu(c.add(word_start).cast::<u8>(), acc0);
    storeu(c.add(stride_words + word_start).cast::<u8>(), acc1);
    storeu(c.add(2 * stride_words + word_start).cast::<u8>(), acc2);
    storeu(c.add(3 * stride_words + word_start).cast::<u8>(), acc3);
    storeu(c.add(4 * stride_words + word_start).cast::<u8>(), acc4);
    storeu(c.add(5 * stride_words + word_start).cast::<u8>(), acc5);
    storeu(c.add(6 * stride_words + word_start).cast::<u8>(), acc6);
    storeu(c.add(7 * stride_words + word_start).cast::<u8>(), acc7);
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_m4rm_tile8xn(
    c_block: &mut [u64],
    stride_words: usize,
    table_buffer: &[u64],
    idx: &[usize; M4RM_TILE_ROWS],
) {
    let mut word_start = 0usize;
    while word_start + M4RM_TILE_WORDS <= stride_words {
        avx2_m4rm_tile8x4(c_block, stride_words, word_start, table_buffer, idx);
        word_start += M4RM_TILE_WORDS;
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_and_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4;
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_and_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) &= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_or_into(dst: &mut [u64], src: &[u64]) {
    let len = dst.len().min(src.len());
    let nvec = len / 4;
    let dst_ptr = dst.as_mut_ptr() as *mut u8;
    let src_ptr = src.as_ptr() as *const u8;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(dst_ptr.offset(off));
        let b = loadu(src_ptr.offset(off));
        let r = _mm256_or_si256(a, b);
        storeu(dst_ptr.offset(off), r);
        i += 1;
    }
    for j in (nvec * 4)..len {
        *dst.get_unchecked_mut(j) |= *src.get_unchecked(j);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_not_into(buf: &mut [u64]) {
    let len = buf.len();
    let nvec = len / 4;
    let ptr = buf.as_mut_ptr() as *mut u8;
    let ones = _mm256_set1_epi64x(-1);
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let a = loadu(ptr.offset(off));
        let r = _mm256_xor_si256(a, ones);
        storeu(ptr.offset(off), r);
        i += 1;
    }
    // Tail: avoid aliasing &/&mut; use raw pointer arithmetic
    let p = buf.as_mut_ptr();
    for j in (nvec * 4)..len {
        let v = *p.add(j);
        *p.add(j) = !v;
    }
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {
    if buf.is_empty() {
        return 0;
    }
    // Byte-wise popcount via nibble LUT + vpshufb, then widen-sum with vpsadbw.
    let ptr = buf.as_ptr() as *const u8;
    let nbytes = buf.len() * 8;

    let lut = _mm256_setr_epi8(
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3,
        3, 4,
    );
    let mask0f = _mm256_set1_epi8(0x0f);
    let mut acc = _mm256_setzero_si256();

    let nvec = nbytes / 32;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let v = loadu(ptr.offset(off));
        let lo = _mm256_and_si256(v, mask0f);
        let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask0f);
        let pc_lo = _mm256_shuffle_epi8(lut, lo);
        let pc_hi = _mm256_shuffle_epi8(lut, hi);
        let pc = _mm256_add_epi8(pc_lo, pc_hi);
        // Sum bytes to 64-bit lanes
        acc = _mm256_add_epi64(acc, _mm256_sad_epu8(pc, _mm256_setzero_si256()));
        i += 1;
    }

    // Horizontal add acc's four 64-bit lanes
    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256(acc, 1);
    let acc128 = _mm_add_epi64(acc_lo, acc_hi);
    let mut total = _mm_cvtsi128_si64(acc128) as u64;
    // Avoid _mm_extract_epi64 (SSE4.1); instead shift right by 8 bytes and read low 64
    let acc128_hi = _mm_srli_si128(acc128, 8);
    total += _mm_cvtsi128_si64(acc128_hi) as u64;

    // Tail bytes
    let rem = nbytes & 31;
    if rem != 0 {
        let tail_ptr = ptr.add(nvec * 32);
        for k in 0..rem {
            total += (*tail_ptr.add(k)).count_ones() as u64;
        }
    }

    total
}

#[target_feature(enable = "avx2")]
unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {
    let len = lhs.len().min(rhs.len());
    if len == 0 {
        return 0;
    }

    let lhs_ptr = lhs.as_ptr() as *const u8;
    let rhs_ptr = rhs.as_ptr() as *const u8;
    let nbytes = len * 8;

    let lut = _mm256_setr_epi8(
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3,
        3, 4,
    );
    let mask0f = _mm256_set1_epi8(0x0f);
    let zero = _mm256_setzero_si256();
    let mut acc = zero;

    let nvec = nbytes / 32;
    let mut i = 0usize;
    while i < nvec {
        let off = (i * 32) as isize;
        let lhs_v = loadu(lhs_ptr.offset(off));
        let rhs_v = loadu(rhs_ptr.offset(off));
        let v = _mm256_and_si256(lhs_v, rhs_v);
        let lo = _mm256_and_si256(v, mask0f);
        let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask0f);
        let pc_lo = _mm256_shuffle_epi8(lut, lo);
        let pc_hi = _mm256_shuffle_epi8(lut, hi);
        let pc = _mm256_add_epi8(pc_lo, pc_hi);
        acc = _mm256_add_epi64(acc, _mm256_sad_epu8(pc, zero));
        i += 1;
    }

    let acc_lo = _mm256_castsi256_si128(acc);
    let acc_hi = _mm256_extracti128_si256(acc, 1);
    let acc128 = _mm_add_epi64(acc_lo, acc_hi);
    let mut total = _mm_cvtsi128_si64(acc128) as u64;
    let acc128_hi = _mm_srli_si128(acc128, 8);
    total += _mm_cvtsi128_si64(acc128_hi) as u64;

    for j in (nvec * 4)..len {
        total += (*lhs.get_unchecked(j) & *rhs.get_unchecked(j)).count_ones() as u64;
    }

    total
}

/// Finds the index of the first set bit using AVX2.
///
/// Strategy: Compare each vector against zero, extract movemask,
/// find first non-zero mask, then find trailing zeros within that mask.
#[target_feature(enable = "avx2")]
unsafe fn avx2_find_first_one(buf: &[u64]) -> Option<usize> {
    if buf.is_empty() {
        return None;
    }

    let ptr = buf.as_ptr() as *const u8;
    let nvec = buf.len() / 4; // 4 u64 per 256-bit vector
    let zero = _mm256_setzero_si256();

    // Process full vectors
    for i in 0..nvec {
        let off = (i * 32) as isize;
        let v = loadu(ptr.offset(off));
        // Compare for equality with zero
        let cmp = _mm256_cmpeq_epi64(v, zero);
        let mask = _mm256_movemask_epi8(cmp) as u32;

        // If mask != 0xFFFFFFFF, then at least one u64 is non-zero
        if mask != 0xFFFFFFFF {
            // Check each of the 4 u64s in this vector
            let words = &buf[i * 4..(i * 4 + 4)];
            for (j, &word) in words.iter().enumerate() {
                if word != 0 {
                    let bit_in_word = word.trailing_zeros() as usize;
                    return Some((i * 4 + j) * 64 + bit_in_word);
                }
            }
        }
    }

    // Process tail
    #[allow(clippy::needless_range_loop)]
    for i in (nvec * 4)..buf.len() {
        if buf[i] != 0 {
            let bit_in_word = buf[i].trailing_zeros() as usize;
            return Some(i * 64 + bit_in_word);
        }
    }

    None
}

/// Finds the index of the first clear bit using AVX2.
///
/// Strategy: Similar to find_first_one but inverts the logic.
#[target_feature(enable = "avx2")]
unsafe fn avx2_find_first_zero(buf: &[u64]) -> Option<usize> {
    if buf.is_empty() {
        return None;
    }

    let ptr = buf.as_ptr() as *const u8;
    let nvec = buf.len() / 4;
    let ones = _mm256_set1_epi64x(-1);

    // Process full vectors
    for i in 0..nvec {
        let off = (i * 32) as isize;
        let v = loadu(ptr.offset(off));
        // Compare for equality with all ones
        let cmp = _mm256_cmpeq_epi64(v, ones);
        let mask = _mm256_movemask_epi8(cmp) as u32;

        // If mask != 0xFFFFFFFF, then at least one u64 is not all ones
        if mask != 0xFFFFFFFF {
            // Check each of the 4 u64s in this vector
            let words = &buf[i * 4..(i * 4 + 4)];
            for (j, &word) in words.iter().enumerate() {
                if word != !0u64 {
                    let bit_in_word = (!word).trailing_zeros() as usize;
                    return Some((i * 4 + j) * 64 + bit_in_word);
                }
            }
        }
    }

    // Process tail
    #[allow(clippy::needless_range_loop)]
    for i in (nvec * 4)..buf.len() {
        if buf[i] != !0u64 {
            let bit_in_word = (!buf[i]).trailing_zeros() as usize;
            return Some(i * 64 + bit_in_word);
        }
    }

    None
}

/// Word-aligned left shift: shifts entire u64 words left by `word_shift` positions.
/// Words shifted out on the left are lost; zeros fill from the right.
#[target_feature(enable = "avx2")]
unsafe fn avx2_shift_left_words(buf: &mut [u64], word_shift: usize) {
    if word_shift == 0 || buf.is_empty() {
        return;
    }

    if word_shift >= buf.len() {
        buf.fill(0);
        return;
    }

    let len = buf.len();
    let ptr = buf.as_mut_ptr() as *mut u8;
    let zero = _mm256_setzero_si256();

    // Process in reverse with vectors to avoid overwrites
    let num_to_move = len - word_shift;
    let nvec = num_to_move / 4;
    let vec_words = nvec * 4;

    // Handle remaining words with scalar FIRST (in reverse to avoid overwrites)
    for i in (vec_words..num_to_move).rev() {
        buf[i + word_shift] = buf[i];
    }

    // Copy full vectors from source to destination (in reverse)
    for i in (0..nvec).rev() {
        let src_idx = i * 4;
        let dst_idx = src_idx + word_shift;
        let src_off = (src_idx * 8) as isize;
        let dst_off = (dst_idx * 8) as isize;
        let v = loadu(ptr.offset(src_off));
        storeu(ptr.offset(dst_off), v);
    }

    // Zero fill lower words with vectors where possible
    let zero_nvec = word_shift / 4;
    for i in 0..zero_nvec {
        storeu(ptr.add(i * 4 * 8), zero);
    }

    // Zero fill remaining lower words
    let scalar_start = zero_nvec * 4;
    let scalar_count = word_shift.saturating_sub(scalar_start);
    buf.iter_mut()
        .skip(scalar_start)
        .take(scalar_count)
        .for_each(|x| *x = 0);
}

/// Word-aligned right shift: shifts entire u64 words right by `word_shift` positions.
/// Words shifted out on the right are lost; zeros fill from the left.
#[target_feature(enable = "avx2")]
unsafe fn avx2_shift_right_words(buf: &mut [u64], word_shift: usize) {
    if word_shift == 0 || buf.is_empty() {
        return;
    }

    if word_shift >= buf.len() {
        buf.fill(0);
        return;
    }

    let len = buf.len();
    let ptr = buf.as_mut_ptr() as *mut u8;
    let zero = _mm256_setzero_si256();

    // Process forward with vectors (no overwrite concern)
    let num_to_move = len - word_shift;
    let nvec = num_to_move / 4;

    // Copy full vectors from source to destination
    for i in 0..nvec {
        let src_idx = i * 4 + word_shift;
        let dst_idx = i * 4;
        let src_off = (src_idx * 8) as isize;
        let dst_off = (dst_idx * 8) as isize;
        let v = loadu(ptr.offset(src_off));
        storeu(ptr.offset(dst_off), v);
    }

    // Handle remaining words with scalar
    let vec_words = nvec * 4;
    for i in vec_words..num_to_move {
        buf[i] = buf[i + word_shift];
    }

    // Zero fill upper words with vectors where possible
    let zero_start = len - word_shift;
    let zero_nvec = word_shift / 4;
    for i in 0..zero_nvec {
        let idx = zero_start + i * 4;
        if idx + 4 <= len {
            storeu(ptr.add(idx * 8), zero);
        }
    }

    // Zero fill remaining upper words
    let vec_zero_end = zero_start + zero_nvec * 4;
    buf.iter_mut()
        .take(len)
        .skip(vec_zero_end)
        .for_each(|x| *x = 0);
}

pub(crate) fn fns() -> LogicalFns {
    // Provide safe wrappers that call into the unsafe AVX2 fns.
    fn and_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() {
            return;
        }
        unsafe { avx2_and_into(dst, src) }
    }
    fn or_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() {
            return;
        }
        unsafe { avx2_or_into(dst, src) }
    }
    fn xor_fn(dst: &mut [u64], src: &[u64]) {
        if dst.is_empty() {
            return;
        }
        unsafe { avx2_xor_into(dst, src) }
    }
    fn m4rm_gray_xor16_fn(acc: &mut [[u64; 8]; 2], src: &[u64]) {
        assert!(
            src.len() >= 16,
            "m4rm_gray_xor16: src must contain at least 16 words"
        );
        unsafe { avx2_m4rm_gray_xor16(acc, src) }
    }
    fn m4rm_gray_build4_fn(
        buffer: &mut [u64],
        panel: &[u64],
        stride_words: usize,
        table_size: usize,
        valid_rows: usize,
    ) {
        assert_eq!(stride_words, 4, "m4rm_gray_build4: stride_words must be 4");
        assert!(
            buffer.len() >= table_size * stride_words,
            "m4rm_gray_build4: buffer too small"
        );
        assert!(
            panel.len() >= valid_rows * stride_words,
            "m4rm_gray_build4: panel too small"
        );
        unsafe { avx2_m4rm_gray_build4(buffer, panel, stride_words, table_size, valid_rows) }
    }
    fn m4rm_gray_build8_fn(
        buffer: &mut [u64],
        panel: &[u64],
        stride_words: usize,
        table_size: usize,
        valid_rows: usize,
    ) {
        assert_eq!(stride_words, 8, "m4rm_gray_build8: stride_words must be 8");
        assert!(
            buffer.len() >= table_size * stride_words,
            "m4rm_gray_build8: buffer too small"
        );
        assert!(
            panel.len() >= valid_rows * stride_words,
            "m4rm_gray_build8: panel too small"
        );
        unsafe { avx2_m4rm_gray_build8(buffer, panel, stride_words, table_size, valid_rows) }
    }
    fn m4rm_tile8x4_fn(
        c_block: &mut [u64],
        stride_words: usize,
        word_start: usize,
        table_buffer: &[u64],
        idx: &[usize; M4RM_TILE_ROWS],
    ) {
        assert!(
            m4rm_c_block_covers_rows(c_block.len(), stride_words),
            "m4rm_tile8x4: c_block must contain 8 rows of stride_words words"
        );
        assert!(
            word_start
                .checked_add(M4RM_TILE_WORDS)
                .is_some_and(|end| end <= stride_words),
            "m4rm_tile8x4: word_start + 4 must be within stride_words"
        );
        assert!(
            m4rm_table_rows_cover(
                table_buffer.len(),
                stride_words,
                word_start,
                M4RM_TILE_WORDS,
                idx,
            ),
            "m4rm_tile8x4: table_buffer must cover all indexed 8x4 table rows"
        );
        unsafe { avx2_m4rm_tile8x4(c_block, stride_words, word_start, table_buffer, idx) }
    }
    fn m4rm_tile8xn_fn(
        c_block: &mut [u64],
        stride_words: usize,
        table_buffer: &[u64],
        idx: &[usize; M4RM_TILE_ROWS],
    ) {
        assert!(
            m4rm_c_block_covers_rows(c_block.len(), stride_words),
            "m4rm_tile8xn: c_block must contain 8 rows of stride_words words"
        );

        let full_words = stride_words / M4RM_TILE_WORDS * M4RM_TILE_WORDS;
        if full_words == 0 {
            return;
        }
        assert!(
            m4rm_table_rows_cover(table_buffer.len(), stride_words, 0, full_words, idx),
            "m4rm_tile8xn: table_buffer must cover all indexed 8xN table rows"
        );
        unsafe { avx2_m4rm_tile8xn(c_block, stride_words, table_buffer, idx) }
    }
    fn not_fn(dst: &mut [u64]) {
        if dst.is_empty() {
            return;
        }
        unsafe { avx2_not_into(dst) }
    }
    fn popcnt_fn(src: &[u64]) -> u64 {
        unsafe { avx2_popcnt(src) }
    }
    fn and_popcnt_fn(lhs: &[u64], rhs: &[u64]) -> u64 {
        unsafe { avx2_and_popcnt(lhs, rhs) }
    }
    fn find_first_one_fn(src: &[u64]) -> Option<usize> {
        unsafe { avx2_find_first_one(src) }
    }
    fn find_first_zero_fn(src: &[u64]) -> Option<usize> {
        unsafe { avx2_find_first_zero(src) }
    }
    fn shift_left_words_fn(buf: &mut [u64], word_shift: usize) {
        if buf.is_empty() {
            return;
        }
        unsafe { avx2_shift_left_words(buf, word_shift) }
    }
    fn shift_right_words_fn(buf: &mut [u64], word_shift: usize) {
        if buf.is_empty() {
            return;
        }
        unsafe { avx2_shift_right_words(buf, word_shift) }
    }
    LogicalFns {
        and_fn,
        or_fn,
        xor_fn,
        m4rm_gray_xor16_fn,
        m4rm_gray_build4_fn,
        m4rm_gray_build8_fn,
        m4rm_tile8x4_fn,
        m4rm_tile8xn_fn,
        not_fn,
        popcnt_fn,
        and_popcnt_fn,
        find_first_one_fn,
        find_first_zero_fn,
        shift_left_words_fn,
        shift_right_words_fn,
    }
}

#[inline]
fn m4rm_c_block_covers_rows(c_block_len: usize, stride_words: usize) -> bool {
    stride_words
        .checked_mul(M4RM_TILE_ROWS)
        .is_some_and(|required| c_block_len >= required)
}

#[inline]
fn m4rm_table_rows_cover(
    table_len: usize,
    stride_words: usize,
    word_start: usize,
    word_count: usize,
    idx: &[usize; M4RM_TILE_ROWS],
) -> bool {
    if word_count == 0 {
        return true;
    }
    match word_start.checked_add(word_count) {
        Some(end) if end <= stride_words => {}
        _ => return false,
    }

    idx.iter().all(|&entry| {
        entry
            .checked_mul(stride_words)
            .and_then(|base| base.checked_add(word_start))
            .and_then(|start| start.checked_add(word_count))
            .is_some_and(|end| end <= table_len)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m4rm_table_rows_cover_accepts_valid_tile() {
        let idx = [0, 1, 2, 3, 4, 5, 6, 7];
        assert!(m4rm_table_rows_cover(8 * 16, 16, 12, 4, &idx));
        assert!(m4rm_table_rows_cover(8 * 16, 16, 0, 16, &idx));
    }

    #[test]
    fn test_m4rm_table_rows_cover_rejects_short_buffer() {
        let idx = [0, 1, 2, 3, 4, 5, 6, 7];
        assert!(!m4rm_table_rows_cover(8 * 16 - 1, 16, 12, 4, &idx));
    }

    #[test]
    fn test_m4rm_table_rows_cover_rejects_overflowing_index() {
        let idx = [usize::MAX, 1, 2, 3, 4, 5, 6, 7];
        assert!(!m4rm_table_rows_cover(8 * 16, 16, 0, 4, &idx));
    }

    #[test]
    #[should_panic(expected = "table_buffer must cover")]
    fn test_m4rm_tile8x4_wrapper_panics_on_invalid_table() {
        let fns = fns();
        let mut c_block = vec![0u64; 8 * 16];
        let table = vec![0u64; 16];
        let idx = [0, 1, 2, 3, 4, 5, 6, 7];
        (fns.m4rm_tile8x4_fn)(&mut c_block, 16, 0, &table, &idx);
    }

    #[test]
    #[should_panic(expected = "src must contain at least 16 words")]
    fn test_m4rm_gray_xor16_wrapper_panics_on_short_src() {
        let fns = fns();
        let mut acc = [[0u64; 8]; 2];
        let src = [0u64; 15];
        (fns.m4rm_gray_xor16_fn)(&mut acc, &src);
    }

    /// Scalar reference Gray-code table build: entry `g` = XOR of panel rows
    /// whose bit is set in the binary index `g` (only `valid_rows` rows count).
    fn scalar_gray_build(
        panel: &[u64],
        stride: usize,
        table_size: usize,
        valid_rows: usize,
    ) -> Vec<u64> {
        let mut buf = vec![0u64; table_size * stride];
        for g in 0..table_size {
            let off = g * stride;
            for bit in 0..valid_rows {
                if (g & (1 << bit)) != 0 {
                    for w in 0..stride {
                        buf[off + w] ^= panel[bit * stride + w];
                    }
                }
            }
        }
        buf
    }

    fn pseudo_panel(rows: usize, stride: usize, seed: u64) -> Vec<u64> {
        let mut s = seed | 1;
        let mut out = vec![0u64; rows * stride];
        for v in out.iter_mut() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            *v = s;
        }
        out
    }

    #[test]
    fn test_m4rm_gray_build4_matches_scalar() {
        let fns = fns();
        let stride = 4;
        for valid_rows in 1..=8usize {
            let table_size = 1usize << valid_rows;
            let panel = pseudo_panel(valid_rows, stride, 0xA53C_9E11 ^ valid_rows as u64);
            let expected = scalar_gray_build(&panel, stride, table_size, valid_rows);
            let mut got = vec![0u64; table_size * stride];
            (fns.m4rm_gray_build4_fn)(&mut got, &panel, stride, table_size, valid_rows);
            assert_eq!(got, expected, "stride4 build mismatch at k={valid_rows}");
        }
    }

    #[test]
    fn test_m4rm_gray_build8_matches_scalar() {
        let fns = fns();
        let stride = 8;
        for valid_rows in 1..=8usize {
            let table_size = 1usize << valid_rows;
            let panel = pseudo_panel(valid_rows, stride, 0x71B2_44DD ^ valid_rows as u64);
            let expected = scalar_gray_build(&panel, stride, table_size, valid_rows);
            let mut got = vec![0u64; table_size * stride];
            (fns.m4rm_gray_build8_fn)(&mut got, &panel, stride, table_size, valid_rows);
            assert_eq!(got, expected, "stride8 build mismatch at k={valid_rows}");
        }
    }

    #[test]
    #[should_panic(expected = "m4rm_gray_build4: stride_words must be 4")]
    fn test_m4rm_gray_build4_rejects_wrong_stride() {
        let fns = fns();
        let mut buf = vec![0u64; 8];
        let panel = vec![0u64; 8];
        (fns.m4rm_gray_build4_fn)(&mut buf, &panel, 8, 2, 1);
    }
}
