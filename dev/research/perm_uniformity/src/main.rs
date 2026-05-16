// perm-uniformity: empirical Monte Carlo comparison of perm(A) vs det(A)
// distributions over GF(q) for q in {3, 5, 7}.
//
// JIT issue 8e4e19a0.
//
// Methodology:
//   For each (q, n) cell, draw N random n x n matrices over F_q and record
//   the output of perm(A) and det(A) separately.  Compute the total
//   variation distance (TVD) of each distribution from the uniform
//   distribution on F_q.  Bootstrap 1000 resamples to obtain 95% CI.
//
//   Criterion 6 (D1 fix): the difference bootstrap CI on
//   (TVD_perm - TVD_det) is used.  Resampling the two independent streams
//   separately gives the 95th-percentile of the difference distribution;
//   PASS when that quantile is < 0.
//
// Output: dev/benchmarks/perm_uniformity/results-2026-05-15.csv
//
// Determinism: uses gf2_core::rng::Lcg with a fixed seed per cell.
// Same seed -> bit-identical CSV (statistical columns) across runs.
// Timing columns (mean_us_perm, mean_us_det) vary by wall-clock.
//
// Usage:
//   cargo run --manifest-path dev/research/perm_uniformity/Cargo.toml \
//       --release
//   # Optional: override output dir via OUTPUT_DIR env var.
//   OUTPUT_DIR=/tmp cargo run --manifest-path ... --release

use std::fs;
use std::io::Write;
use std::time::Instant;

use gf2_algebra::packed::{Bipedal3Matrix, Packed5Matrix, Packed7Matrix};
use gf2_algebra::permanent::permanent_bipedal3_parallel;
use gf2_algebra::permanent::{permanent_bipedal3, permanent_bipedal5, permanent_bipedal7};
use gf2_core::field::inverse::det;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::gfp::Fp;

use perm_uniformity::harness::{run_cell, CellResult};

// ---------------------------------------------------------------------------
// Deterministic seed embedded in the CSV header.
// ---------------------------------------------------------------------------

/// Master seed for the sweep.  Each cell derives its seed from this value
/// plus a deterministic salt so the streams are independent.
const SEED: u64 = 0xc0_ffee_0000_0001_u64;

// ---------------------------------------------------------------------------
// Sweep grid
// ---------------------------------------------------------------------------

/// Per-cell specification: which prime, which size, how many samples.
///
/// N is chosen so the expected TVD CI half-width is <= 0.01 and the cell
/// completes within a practical wall-clock budget.
struct CellSpec {
    q: u64,
    n: usize,
    n_samples: usize,
    /// Use the parallel bipedal3 kernel for this cell (F_3 only, n >= 24).
    use_parallel: bool,
}

/// Full sweep grid as specified in JIT issue 8e4e19a0.
fn sweep_grid() -> Vec<CellSpec> {
    let mut cells = Vec::new();

    // F_3: n in {6, 8, 10, 12, 16, 20, 24, 28, 32}
    for &n in &[6usize, 8, 10, 12, 16, 20, 24, 28, 32] {
        let (n_samples, use_parallel) = match n {
            6 | 8 | 10 => (500_000, false),
            12 => (50_000, false),
            16 => (10_000, false),
            20 => (2_000, false),
            24 => (500, true),
            28 => (200, true),
            _ => (50, true), // n=32
        };
        cells.push(CellSpec {
            q: 3,
            n,
            n_samples,
            use_parallel,
        });
    }

    // F_5: n in {6, 8, 10, 12, 14}
    for &n in &[6usize, 8, 10, 12, 14] {
        cells.push(CellSpec {
            q: 5,
            n,
            n_samples: 50_000,
            use_parallel: false,
        });
    }

    // F_7: n in {6, 8, 10, 12, 14} (F_7 caps at n <= 16 = LANES)
    for &n in &[6usize, 8, 10, 12, 14] {
        cells.push(CellSpec {
            q: 7,
            n,
            n_samples: 50_000,
            use_parallel: false,
        });
    }

    cells
}

// ---------------------------------------------------------------------------
// Deterministic per-cell seed
// ---------------------------------------------------------------------------

/// Deterministic per-cell seed derived from (q, n, which).
fn cell_seed(q: u64, n: usize, which: u64) -> u64 {
    SEED.wrapping_add(q.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add((n as u64).wrapping_mul(0x6c62_272e_07bb_0142))
        .wrapping_add(which.wrapping_mul(0x1234_5678_9abc_def0))
}

// ---------------------------------------------------------------------------
// Per-prime cell runners (using shared run_cell harness)
// ---------------------------------------------------------------------------

/// Run a cell for q=3.
fn run_cell_f3(n: usize, n_samples: usize, use_parallel: bool) -> CellResult {
    let q = 3u64;
    run_cell(
        q,
        n,
        n_samples,
        cell_seed(q, n, 0), // perm stream seed
        cell_seed(q, n, 1), // det stream seed
        cell_seed(q, n, 2), // bootstrap perm seed
        cell_seed(q, n, 3), // bootstrap det seed
        cell_seed(q, n, 4), // bootstrap diff seed
        |rng, size| {
            let flat: Vec<Fp<3>> = (0..size * size)
                .map(|_| Fp::<3>::new(rng.next_u64() % 3))
                .collect();
            let mat = Bipedal3Matrix::from_row_major(&flat, size, size);
            let p = if use_parallel {
                permanent_bipedal3_parallel(&mat)
            } else {
                permanent_bipedal3(&mat)
            };
            p.value() as u8
        },
        |rng, size| {
            let mut mat = FieldMatrix::<Fp<3>>::zeros(size, size);
            for r in 0..size {
                for c in 0..size {
                    mat.set(r, c, Fp::<3>::new(rng.next_u64() % 3));
                }
            }
            det(&mat).value() as u8
        },
    )
}

/// Run a cell for q=5.
fn run_cell_f5(n: usize, n_samples: usize) -> CellResult {
    let q = 5u64;
    run_cell(
        q,
        n,
        n_samples,
        cell_seed(q, n, 0),
        cell_seed(q, n, 1),
        cell_seed(q, n, 2),
        cell_seed(q, n, 3),
        cell_seed(q, n, 4),
        |rng, size| {
            let flat: Vec<Fp<5>> = (0..size * size)
                .map(|_| Fp::<5>::new(rng.next_u64() % 5))
                .collect();
            let mat = Packed5Matrix::from_row_major(&flat, size, size);
            permanent_bipedal5(&mat).value() as u8
        },
        |rng, size| {
            let mut mat = FieldMatrix::<Fp<5>>::zeros(size, size);
            for r in 0..size {
                for c in 0..size {
                    mat.set(r, c, Fp::<5>::new(rng.next_u64() % 5));
                }
            }
            det(&mat).value() as u8
        },
    )
}

/// Run a cell for q=7.
fn run_cell_f7(n: usize, n_samples: usize) -> CellResult {
    let q = 7u64;
    run_cell(
        q,
        n,
        n_samples,
        cell_seed(q, n, 0),
        cell_seed(q, n, 1),
        cell_seed(q, n, 2),
        cell_seed(q, n, 3),
        cell_seed(q, n, 4),
        |rng, size| {
            let flat: Vec<Fp<7>> = (0..size * size)
                .map(|_| Fp::<7>::new(rng.next_u64() % 7))
                .collect();
            let mat = Packed7Matrix::from_row_major(&flat, size, size);
            permanent_bipedal7(&mat).value() as u8
        },
        |rng, size| {
            let mut mat = FieldMatrix::<Fp<7>>::zeros(size, size);
            for r in 0..size {
                for c in 0..size {
                    mat.set(r, c, Fp::<7>::new(rng.next_u64() % 7));
                }
            }
            det(&mat).value() as u8
        },
    )
}

// ---------------------------------------------------------------------------
// PNG plot — faceted by q, CI bands, log-scale y (D2 + D3 fix)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Embedded 5×7 bitmap font (pure-Rust, no unsafe, no external crates)
// ---------------------------------------------------------------------------
//
// Each glyph is 5 columns × 7 rows, stored as 7 u8 values.  Each u8 encodes
// one row: bit 4 (MSB of the 5-bit field) is the leftmost pixel.  Bit 0 is
// the rightmost pixel.  A set bit means "draw this pixel".
//
// Character coverage: digits 0-9, uppercase A-Z, lowercase a-z,
// plus space, '-', '^', '.', '(', ')', '=', '1' already in digits.

/// Return the 5×7 glyph for `ch`, or a fallback (rectangle) if not found.
/// Each element of the returned array is a 5-bit row mask (bits 4..0 = cols 0..4).
fn glyph(ch: char) -> [u8; 7] {
    match ch {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x04, 0x04, 0x04],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x04, 0x0A, 0x11, 0x11, 0x1F, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x0A, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        'a' => [0x00, 0x00, 0x0E, 0x01, 0x0F, 0x11, 0x0F],
        'b' => [0x10, 0x10, 0x1E, 0x11, 0x11, 0x11, 0x1E],
        'c' => [0x00, 0x00, 0x0E, 0x10, 0x10, 0x11, 0x0E],
        'd' => [0x01, 0x01, 0x0F, 0x11, 0x11, 0x11, 0x0F],
        'e' => [0x00, 0x00, 0x0E, 0x11, 0x1F, 0x10, 0x0E],
        'f' => [0x06, 0x09, 0x08, 0x1C, 0x08, 0x08, 0x08],
        'g' => [0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'h' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
        'i' => [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x0E],
        'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0C],
        'k' => [0x10, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'l' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'm' => [0x00, 0x00, 0x1A, 0x15, 0x15, 0x11, 0x11],
        'n' => [0x00, 0x00, 0x16, 0x19, 0x11, 0x11, 0x11],
        'o' => [0x00, 0x00, 0x0E, 0x11, 0x11, 0x11, 0x0E],
        'p' => [0x00, 0x00, 0x1E, 0x11, 0x11, 0x1E, 0x10],
        'q' => [0x00, 0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01],
        'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        's' => [0x00, 0x00, 0x0E, 0x10, 0x0E, 0x01, 0x1E],
        't' => [0x08, 0x08, 0x1C, 0x08, 0x08, 0x09, 0x06],
        'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0D],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'w' => [0x00, 0x00, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'x' => [0x00, 0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11],
        'y' => [0x00, 0x11, 0x11, 0x0F, 0x01, 0x11, 0x0E],
        'z' => [0x00, 0x00, 0x1F, 0x02, 0x04, 0x08, 0x1F],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '^' => [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F], // unknown: filled rectangle
    }
}

/// Width of one glyph in pixels (5 columns + 1 spacing column = 6 px/char).
const GLYPH_W: usize = 5;
/// Height of one glyph in pixels.
const GLYPH_H: usize = 7;
/// Horizontal advance per character (glyph + 1 px gap).
const GLYPH_ADV: usize = GLYPH_W + 1;

/// Mutable canvas wrapping a flat RGB pixel buffer.
struct Canvas {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Canvas {
            pixels: vec![255u8; width * height * 3],
            width,
            height,
        }
    }

    /// Set pixel at (x, y) if in bounds.
    fn set_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8)) {
        if x < self.width && y < self.height {
            let base = (y * self.width + x) * 3;
            self.pixels[base] = rgb.0;
            self.pixels[base + 1] = rgb.1;
            self.pixels[base + 2] = rgb.2;
        }
    }

    /// Alpha-blend a pixel at (x, y): 70% existing + 30% new colour.
    fn blend_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8)) {
        if x < self.width && y < self.height {
            let base = (y * self.width + x) * 3;
            self.pixels[base] = ((self.pixels[base] as u16 * 7 + rgb.0 as u16 * 3) / 10) as u8;
            self.pixels[base + 1] =
                ((self.pixels[base + 1] as u16 * 7 + rgb.1 as u16 * 3) / 10) as u8;
            self.pixels[base + 2] =
                ((self.pixels[base + 2] as u16 * 7 + rgb.2 as u16 * 3) / 10) as u8;
        }
    }

    /// Draw a filled disk centred at (cx, cy) with given radius.
    fn draw_disk(&mut self, cx: usize, cy: usize, radius: usize, rgb: (u8, u8, u8)) {
        let r2 = (radius as i64) * (radius as i64);
        for dy in -(radius as i64)..=(radius as i64) {
            for dx in -(radius as i64)..=(radius as i64) {
                if dx * dx + dy * dy <= r2 {
                    let px = (cx as i64 + dx) as usize;
                    let py = (cy as i64 + dy) as usize;
                    self.set_pixel(px, py, rgb);
                }
            }
        }
    }

    /// Draw a line from (x0,y0) to (x1,y1) using Bresenham-style interpolation.
    fn draw_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, rgb: (u8, u8, u8)) {
        let steps = ((x1 as i64 - x0 as i64).abs() + (y1 as i64 - y0 as i64).abs() + 1) as usize;
        if steps == 0 {
            return;
        }
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let lx = (x0 as f64 + t * (x1 as f64 - x0 as f64)) as usize;
            let ly = (y0 as f64 + t * (y1 as f64 - y0 as f64)) as usize;
            self.set_pixel(lx, ly, rgb);
        }
    }

    /// Draw a semi-transparent vertical CI ribbon from y_lo to y_hi at column x.
    fn draw_ci_ribbon(
        &mut self,
        x: usize,
        y_lo: usize,
        y_hi: usize,
        width: usize,
        rgb: (u8, u8, u8),
    ) {
        let (lo, hi) = (y_lo.min(y_hi), y_lo.max(y_hi));
        for bx in x.saturating_sub(width / 2)..=(x + width / 2) {
            for y in lo..=hi {
                self.blend_pixel(bx, y, rgb);
            }
        }
    }

    /// Blit a single glyph at pixel (x, y) — top-left of the glyph cell.
    /// `scale` = 1 means each logical pixel becomes 1×1 screen pixel.
    fn draw_glyph(&mut self, x: usize, y: usize, ch: char, rgb: (u8, u8, u8), scale: usize) {
        let rows = glyph(ch);
        for (row_idx, &row_mask) in rows.iter().enumerate() {
            for col in 0..GLYPH_W {
                let bit = (row_mask >> (GLYPH_W - 1 - col)) & 1;
                if bit != 0 {
                    let px0 = x + col * scale;
                    let py0 = y + row_idx * scale;
                    for dy in 0..scale {
                        for dx in 0..scale {
                            self.set_pixel(px0 + dx, py0 + dy, rgb);
                        }
                    }
                }
            }
        }
    }

    /// Draw a left-to-right text string starting at pixel (x, y) — top-left of
    /// the first character.  `scale` enlarges each pixel (1 = normal, 2 = 2×).
    /// Returns the x coordinate immediately after the last character.
    fn draw_text(&mut self, x: usize, y: usize, s: &str, rgb: (u8, u8, u8), scale: usize) -> usize {
        let mut cx = x;
        for ch in s.chars() {
            self.draw_glyph(cx, y, ch, rgb, scale);
            cx += GLYPH_ADV * scale;
        }
        cx
    }

    /// Return the pixel width of a string rendered at `scale`.
    fn text_width(s: &str, scale: usize) -> usize {
        let chars = s.chars().count();
        if chars == 0 {
            return 0;
        }
        // All chars advance GLYPH_ADV*scale, but the last char does not need
        // the trailing gap.  Keep it simple: advance * chars - 1 * scale gap.
        (chars * GLYPH_ADV - 1) * scale
    }

    /// Draw text centred horizontally around pixel x_centre.
    fn draw_text_centred(
        &mut self,
        x_centre: usize,
        y: usize,
        s: &str,
        rgb: (u8, u8, u8),
        scale: usize,
    ) {
        let w = Self::text_width(s, scale);
        let x0 = x_centre.saturating_sub(w / 2);
        self.draw_text(x0, y, s, rgb, scale);
    }

    /// Draw text right-aligned: the rightmost pixel of the last glyph sits at
    /// `x_right`.
    fn draw_text_right(
        &mut self,
        x_right: usize,
        y: usize,
        s: &str,
        rgb: (u8, u8, u8),
        scale: usize,
    ) {
        let w = Self::text_width(s, scale);
        let x0 = x_right.saturating_sub(w);
        self.draw_text(x0, y, s, rgb, scale);
    }

    /// Draw text rotated 90° counter-clockwise (bottom-to-top) at position
    /// (x, y_bottom): the bottom of the first character is at y_bottom, text
    /// reads upwards.  Used for the y-axis title.
    fn draw_text_vertical(
        &mut self,
        x: usize,
        y_bottom: usize,
        s: &str,
        rgb: (u8, u8, u8),
        scale: usize,
    ) {
        // We draw each glyph rotated: column → row, row → -column.
        // Origin: top-left of the upward-going text starts at (x, y_bottom - text_height).
        // text_height = chars * GLYPH_ADV * scale.
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        for (ci, &ch) in chars.iter().enumerate() {
            // The ci-th character occupies vertical slots [ci*GLYPH_ADV .. ci*GLYPH_ADV + GLYPH_H]
            // going upward from y_bottom.
            let rows = glyph(ch);
            for (row_idx, &row_mask) in rows.iter().enumerate() {
                for col in 0..GLYPH_W {
                    let bit = (row_mask >> (GLYPH_W - 1 - col)) & 1;
                    if bit != 0 {
                        // In normal coords: glyph pixel at (col, row_idx).
                        // Rotated 90° CCW: new_x = row_idx, new_y = (GLYPH_W-1 - col).
                        // Character spacing: character ci shifts y upward by ci*GLYPH_ADV.
                        let rotated_x = row_idx; // was row → now column
                        let rotated_y = (n - 1 - ci) * GLYPH_ADV + (GLYPH_W - 1 - col);
                        let px0 = x + rotated_x * scale;
                        let py0 = y_bottom.saturating_sub((rotated_y + 1) * scale);
                        for dy in 0..scale {
                            for dx in 0..scale {
                                self.set_pixel(px0 + dx, py0 + dy, rgb);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Write a plot faceted by q (3 panels side-by-side), with CI bands (filled
/// ribbon between ci_lo and ci_hi), log-scale y axis.
///
/// Labels added (publication-grade):
///   - Per-panel title "q=3" / "q=5" / "q=7" centred above each panel.
///   - Y-axis tick labels "10^0" .. "10^-4" left of the y-axis (right-aligned
///     to the axis tick mark).
///   - Y-axis title "TVD (log10)" drawn vertically on the leftmost panel only.
///   - X-axis tick labels showing the actual n value below each data point.
///   - X-axis title "n" centred below each panel.
///   - Per-panel legend: coloured "perm" and "det" labels inside the plot area.
///
/// If `convert` (ImageMagick) is available the PPM is promoted to PNG and the
/// PPM is removed.  Otherwise, a pure-Rust minimal PNG is written directly
/// (no external dependencies, no unsafe code) — D3 fix.
fn write_plot(results: &[CellResult], path: &str) {
    let qs = [3u64, 5, 7];
    let n_panels = qs.len(); // 3

    // Overall canvas: 3 panels side by side.
    // Increased margins to accommodate axis labels and titles.
    let panel_w = 400usize;
    let panel_h = 420usize;
    let margin_left = 80usize; // room for y-tick labels + y-title
    let margin_right = 20usize;
    let margin_top = 50usize; // room for panel title
    let margin_bottom = 65usize; // room for x-tick labels + x-title
    let panel_gap = 20usize;

    let total_w = n_panels * panel_w + (n_panels - 1) * panel_gap;
    let total_h = panel_h;
    let mut canvas = Canvas::new(total_w, total_h);

    // Colour palette (perm, det) per q index.
    let colours_perm: [(u8, u8, u8); 3] = [(220, 50, 47), (38, 139, 210), (42, 161, 152)];
    let colours_det: [(u8, u8, u8); 3] = [(203, 75, 22), (108, 113, 196), (133, 153, 0)];

    let y_min_log = -4.0f64;
    let y_max_log = 0.0f64;

    // Shared text colours.
    let label_rgb = (40u8, 40u8, 40u8);
    let axis_rgb = (80u8, 80u8, 80u8);
    let grid_rgb = (220u8, 220u8, 220u8);

    for (qi, &q) in qs.iter().enumerate() {
        let cell_q: Vec<&CellResult> = results.iter().filter(|r| r.q == q).collect();
        if cell_q.is_empty() {
            continue;
        }

        // Panel origin (top-left corner of the panel rectangle).
        let panel_origin_x = qi * (panel_w + panel_gap);

        // Plot area within the panel.
        let plot_x0 = panel_origin_x + margin_left;
        let plot_y0 = margin_top;
        let plot_w = panel_w - margin_left - margin_right;
        let plot_h = panel_h - margin_top - margin_bottom;

        let n_values: Vec<usize> = cell_q.iter().map(|r| r.n).collect();
        let n_min = *n_values.iter().min().unwrap_or(&6);
        let n_max = *n_values.iter().max().unwrap_or(&14);

        let to_px_x = |n: usize| -> usize {
            let frac = if n_max == n_min {
                0.5
            } else {
                (n - n_min) as f64 / (n_max - n_min) as f64
            };
            plot_x0 + (frac * (plot_w as f64 - 1.0)) as usize
        };

        let to_px_y = |tvd: f64| -> usize {
            let log_tvd = if tvd <= 0.0 {
                y_min_log
            } else {
                tvd.log10().clamp(y_min_log, y_max_log)
            };
            // y=0 at top, so larger tvd (less negative log) maps to smaller py.
            let frac = (log_tvd - y_max_log) / (y_min_log - y_max_log);
            plot_y0 + (frac * (plot_h as f64 - 1.0)) as usize
        };

        // Draw axis lines (dark grey).
        for x in plot_x0..=(plot_x0 + plot_w) {
            canvas.set_pixel(x, plot_y0 + plot_h, axis_rgb);
        }
        for y in plot_y0..=(plot_y0 + plot_h) {
            canvas.set_pixel(plot_x0, y, axis_rgb);
        }

        // Draw horizontal grid lines at each log10 tick.
        for log_tick in [-4i32, -3, -2, -1, 0] {
            let y_tick = to_px_y(10f64.powi(log_tick));
            for x in plot_x0..=(plot_x0 + plot_w) {
                canvas.set_pixel(x, y_tick, grid_rgb);
            }

            // Y-axis tick mark (3 px left of axis line).
            for dx in 1usize..=4 {
                canvas.set_pixel(plot_x0.saturating_sub(dx), y_tick, axis_rgb);
            }

            // Y-axis tick label, e.g. "10^0", "10^-1", ..., "10^-4".
            // Right-align to 5 px left of the tick mark.
            let tick_label = match log_tick {
                0 => "10^0",
                -1 => "10^-1",
                -2 => "10^-2",
                -3 => "10^-3",
                -4 => "10^-4",
                _ => "",
            };
            // Vertically centre the 7-px-tall glyph on the tick row.
            let label_y = y_tick.saturating_sub(GLYPH_H / 2);
            let label_x_right = plot_x0.saturating_sub(6); // 6 px gap from axis
            canvas.draw_text_right(label_x_right, label_y, tick_label, label_rgb, 1);
        }

        // Y-axis title "TVD (log10)" — drawn vertically on the leftmost panel only.
        if qi == 0 {
            // The vertical text is drawn bottom-up; anchor y is the bottom of
            // the plot area.  x is 2 px from the left edge of the canvas.
            let vtitle = "TVD (log10)";
            // Height of the vertical text (rotated): n_chars * GLYPH_ADV * scale px.
            let vtitle_height = vtitle.chars().count() * GLYPH_ADV;
            // Centre the text vertically in the plot area.
            let vert_centre_y = plot_y0 + plot_h / 2;
            let y_bottom = vert_centre_y + vtitle_height / 2;
            canvas.draw_text_vertical(2, y_bottom, vtitle, label_rgb, 1);
        }

        // Panel title "q=3" / "q=5" / "q=7" centred above the plot area.
        let title = match q {
            3 => "q=3",
            5 => "q=5",
            7 => "q=7",
            _ => "q=?",
        };
        let title_y = plot_y0.saturating_sub(GLYPH_H + 6); // 6 px above plot top
        let panel_centre_x = plot_x0 + plot_w / 2;
        canvas.draw_text_centred(panel_centre_x, title_y, title, label_rgb, 2);

        let col_perm = colours_perm[qi];
        let col_det = colours_det[qi];

        // First pass: draw CI bands (semi-transparent ribbon) for perm and det.
        for cell in &cell_q {
            let px = to_px_x(cell.n);

            // Perm CI ribbon (width 7 = 3 pixels on each side + centre).
            canvas.draw_ci_ribbon(
                px,
                to_px_y(cell.tvd_perm_ci_lo.max(1e-4)),
                to_px_y(cell.tvd_perm_ci_hi.max(1e-4)),
                7,
                col_perm,
            );

            // Det CI ribbon.
            canvas.draw_ci_ribbon(
                px,
                to_px_y(cell.tvd_det_ci_lo.max(1e-4)),
                to_px_y(cell.tvd_det_ci_hi.max(1e-4)),
                7,
                col_det,
            );
        }

        // Second pass: draw lines connecting point estimates, then dots on top.
        let mut prev_perm: Option<(usize, usize)> = None;
        let mut prev_det: Option<(usize, usize)> = None;

        for cell in &cell_q {
            let px = to_px_x(cell.n);
            let py_p = to_px_y(cell.tvd_perm.max(1e-4));
            let py_d = to_px_y(cell.tvd_det.max(1e-4));

            if let Some((ppx, ppy)) = prev_perm {
                canvas.draw_line(ppx, ppy, px, py_p, col_perm);
            }
            if let Some((ppx, ppy)) = prev_det {
                canvas.draw_line(ppx, ppy, px, py_d, col_det);
            }
            prev_perm = Some((px, py_p));
            prev_det = Some((px, py_d));

            // Point dots on top.
            canvas.draw_disk(px, py_p, 4, col_perm);
            canvas.draw_disk(px, py_d, 4, col_det);
        }

        // X-axis tick labels: show n value below each data point.
        // Use scale=1 glyphs, centred under the point.  Tick mark 3 px below axis.
        let x_tick_y = plot_y0 + plot_h + 2; // tick line below axis
        let x_label_y = x_tick_y + 5; // label below tick
        for &n_val in &n_values {
            let px = to_px_x(n_val);
            // Short tick mark.
            for dy in 0..4usize {
                canvas.set_pixel(px, plot_y0 + plot_h + dy, axis_rgb);
            }
            let label = n_val.to_string();
            canvas.draw_text_centred(px, x_label_y, &label, label_rgb, 1);
        }

        // X-axis title "n" centred under the panel.
        let x_title_y = x_label_y + GLYPH_H + 6;
        canvas.draw_text_centred(panel_centre_x, x_title_y, "n", label_rgb, 2);

        // Legend — drawn inside the plot area near the top-right corner.
        // Two rows: one for "perm", one for "det".
        let legend_swatch_w = 18usize; // width of coloured swatch
        let legend_text_x_off = legend_swatch_w + 4;
        let legend_row_h = GLYPH_H + 4;
        // Position: 5 px from the right edge of the plot, 10 px from the top.
        let legend_x_right = plot_x0 + plot_w - 5;
        let legend_x_swatch = legend_x_right
            .saturating_sub(Canvas::text_width("perm", 1) + legend_text_x_off + legend_swatch_w);
        let legend_y0 = plot_y0 + 10;

        for (row, (label_str, col)) in [("perm", col_perm), ("det", col_det)].iter().enumerate() {
            let ly = legend_y0 + row * legend_row_h;
            // Filled swatch (a short line at mid-height).
            let swatch_mid_y = ly + GLYPH_H / 2;
            for dx in 0..legend_swatch_w {
                canvas.set_pixel(legend_x_swatch + dx, swatch_mid_y, *col);
                if swatch_mid_y > 0 {
                    canvas.set_pixel(legend_x_swatch + dx, swatch_mid_y - 1, *col);
                }
                canvas.set_pixel(legend_x_swatch + dx, swatch_mid_y + 1, *col);
            }
            // Small dot on top of swatch (matches the data-point dots).
            canvas.draw_disk(legend_x_swatch + legend_swatch_w / 2, swatch_mid_y, 3, *col);
            // Text label.
            canvas.draw_text(
                legend_x_swatch + legend_text_x_off,
                ly,
                label_str,
                label_rgb,
                1,
            );
        }
    }

    // Always use the pure-Rust PNG encoder for byte-identical output across
    // runs (criterion 3 / D3).  The ImageMagick path is intentionally skipped:
    // ImageMagick's palette quantiser and compression are non-deterministic
    // across invocations, which breaks sha256 reproducibility.
    match perm_uniformity::png::write_png_file(path, &canvas.pixels, total_w, total_h) {
        Ok(()) => eprintln!("  plot written to {path} (pure-Rust PNG encoder)"),
        Err(e) => eprintln!("  ERROR writing PNG: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let output_dir = std::env::var("OUTPUT_DIR")
        .unwrap_or_else(|_| "dev/benchmarks/perm_uniformity".to_string());
    fs::create_dir_all(&output_dir).expect("cannot create output dir");

    let csv_path = format!("{output_dir}/results-2026-05-15.csv");
    let plot_path = format!("{output_dir}/tvd_vs_n.png");

    println!("perm-uniformity sweep (JIT 8e4e19a0)");
    println!("  seed = {SEED:#018x}");
    println!("  output = {csv_path}");
    println!();

    let grid = sweep_grid();
    let total_cells = grid.len();
    let mut results: Vec<CellResult> = Vec::with_capacity(total_cells);

    let t_sweep_start = Instant::now();

    for (cell_idx, spec) in grid.iter().enumerate() {
        let t_cell = Instant::now();
        println!(
            "[{}/{}] q={} n={} N={} parallel={}",
            cell_idx + 1,
            total_cells,
            spec.q,
            spec.n,
            spec.n_samples,
            spec.use_parallel
        );

        let result = match spec.q {
            3 => run_cell_f3(spec.n, spec.n_samples, spec.use_parallel),
            5 => run_cell_f5(spec.n, spec.n_samples),
            7 => run_cell_f7(spec.n, spec.n_samples),
            _ => unreachable!(),
        };

        let elapsed = t_cell.elapsed().as_secs_f64();
        println!(
            "        tvd_perm={:.6} [{:.6},{:.6}]  tvd_det={:.6} [{:.6},{:.6}]  diff_q95={:.6}  {:.1}s",
            result.tvd_perm,
            result.tvd_perm_ci_lo,
            result.tvd_perm_ci_hi,
            result.tvd_det,
            result.tvd_det_ci_lo,
            result.tvd_det_ci_hi,
            result.diff_q95,
            elapsed
        );

        results.push(result);
    }

    let total_elapsed = t_sweep_start.elapsed().as_secs_f64();
    println!();
    println!(
        "Sweep complete in {:.1}s ({:.1} min)",
        total_elapsed,
        total_elapsed / 60.0
    );

    // Write CSV
    let mut f = fs::File::create(&csv_path).expect("cannot create CSV");
    writeln!(
        f,
        "# perm-uniformity sweep  seed={SEED:#018x}  date=2026-05-15  jit=8e4e19a0"
    )
    .unwrap();
    writeln!(
        f,
        "q,n,samples,tvd_perm,tvd_perm_ci_lo,tvd_perm_ci_hi,tvd_det,tvd_det_ci_lo,tvd_det_ci_hi,mean_us_perm,mean_us_det"
    )
    .unwrap();
    for r in &results {
        writeln!(
            f,
            "{},{},{},{:.8},{:.8},{:.8},{:.8},{:.8},{:.8},{:.4},{:.4}",
            r.q,
            r.n,
            r.n_samples,
            r.tvd_perm,
            r.tvd_perm_ci_lo,
            r.tvd_perm_ci_hi,
            r.tvd_det,
            r.tvd_det_ci_lo,
            r.tvd_det_ci_hi,
            r.mean_us_perm,
            r.mean_us_det
        )
        .unwrap();
    }
    println!("CSV written to {csv_path}");

    // Write plot (faceted by q, CI bands — D2 fix).
    write_plot(&results, &plot_path);

    // Criterion 5: monotonicity for q=3.
    println!();
    println!("--- Criterion 5: TVD_perm monotone non-increasing for q=3 ---");
    let f3_cells: Vec<&CellResult> = results.iter().filter(|r| r.q == 3).collect();
    let mut mono_ok = true;
    for i in 1..f3_cells.len() {
        let prev = f3_cells[i - 1];
        let curr = f3_cells[i];
        if curr.tvd_perm_ci_lo > prev.tvd_perm_ci_hi + 1e-6 {
            println!(
                "  WARN: n={} CI_lo={:.6} > prev CI_hi={:.6} (n={}) -- CIs do not overlap",
                curr.n, curr.tvd_perm_ci_lo, prev.tvd_perm_ci_hi, prev.n
            );
            mono_ok = false;
        } else {
            println!(
                "  OK   n={} TVD_perm={:.6} CI=[{:.6},{:.6}] (prev CI_hi={:.6}, CIs overlap)",
                curr.n,
                curr.tvd_perm,
                curr.tvd_perm_ci_lo,
                curr.tvd_perm_ci_hi,
                prev.tvd_perm_ci_hi
            );
        }
    }
    if mono_ok {
        println!("  PASS: TVD_perm is monotone non-increasing (within CI overlap) for q=3");
    } else {
        println!("  FAIL: monotonicity violated");
    }

    // Criterion 6: TVD_perm <= TVD_det at 95% confidence — CORRECTED STATISTIC (D1 fix).
    //
    // Uses the paired bootstrap CI of the difference (TVD_perm - TVD_det):
    //   - Resample both streams independently 1000 times.
    //   - Check the 95th-percentile of the bootstrap difference distribution is < 0.
    //   - PASS when diff_q95 < 0 (i.e., even the 95th-percentile outcome is negative).
    //
    // This is stronger than the previous check (tvd_perm_ci_hi < tvd_det) which used
    // the perm CI in isolation without accounting for sampling uncertainty in TVD_det.
    println!();
    println!("--- Criterion 6: TVD_perm <= TVD_det at 95% confidence (diff bootstrap) ---");
    println!("  Statistic: 95th-pctile of bootstrap(TVD_perm - TVD_det).  PASS when diff_q95 < 0.");
    let mut ineq_ok = true;
    for r in results.iter().filter(|r| r.n >= 8) {
        let noise_floor = ((r.q as f64) / (2.0 * r.n_samples as f64)).sqrt();
        let noise_dominated = noise_floor > r.tvd_det * 0.5;
        if noise_dominated {
            println!(
                "  NOISE q={} n={} N={}: noise_floor={:.4} > TVD_det/2={:.4} -- N too small to confirm",
                r.q, r.n, r.n_samples, noise_floor, r.tvd_det * 0.5
            );
        } else if r.diff_q95 < 0.0 {
            println!(
                "  OK   q={} n={}: TVD_perm={:.6}  TVD_det={:.6}  diff_q95={:.6} < 0  (PASS)",
                r.q, r.n, r.tvd_perm, r.tvd_det, r.diff_q95
            );
        } else {
            println!(
                "  FAIL q={} n={}: TVD_perm={:.6}  TVD_det={:.6}  diff_q95={:.6} >= 0  (FAIL)",
                r.q, r.n, r.tvd_perm, r.tvd_det, r.diff_q95
            );
            ineq_ok = false;
        }
    }
    if ineq_ok {
        println!("  PASS: criterion 6 holds for all non-noise cells (diff_q95 < 0)");
    } else {
        println!("  FAIL: criterion 6 violated at one or more non-noise cells");
    }

    // Aspirational criterion 10: exponential decay fit for F_3.
    println!();
    println!("--- Criterion 10 (aspirational): exponential decay fit for F_3 ---");
    fit_exponential_f3(&f3_cells);

    println!();
    println!("Done. Wall-clock: {:.1}s", total_elapsed);
}

/// Fit TVD_perm(n) ~ c * beta^{-n} for F_3 by linear regression on log(TVD) vs n.
fn fit_exponential_f3(cells: &[&CellResult]) {
    let usable: Vec<(&CellResult, f64)> = cells
        .iter()
        .filter_map(|c| {
            if c.tvd_perm > 1e-6 {
                Some((*c, c.tvd_perm.ln()))
            } else {
                None
            }
        })
        .collect();

    if usable.len() < 3 {
        println!("  Insufficient data points for fit (need at least 3 with TVD > 1e-6)");
        return;
    }

    let n_pts = usable.len() as f64;
    let sum_n: f64 = usable.iter().map(|(c, _)| c.n as f64).sum();
    let sum_y: f64 = usable.iter().map(|(_, y)| y).sum();
    let sum_n2: f64 = usable.iter().map(|(c, _)| (c.n as f64).powi(2)).sum();
    let sum_ny: f64 = usable.iter().map(|(c, y)| c.n as f64 * y).sum();

    let denom = n_pts * sum_n2 - sum_n * sum_n;
    if denom.abs() < 1e-12 {
        println!("  Degenerate fit (all n equal?)");
        return;
    }

    let slope = (n_pts * sum_ny - sum_n * sum_y) / denom;
    let intercept = (sum_y - slope * sum_n) / n_pts;

    let beta = (-slope).exp();
    let c = intercept.exp();

    println!("  Fit: TVD_perm(n) ~ {c:.4e} * {beta:.4}^{{-n}}");
    println!("  beta = {beta:.4}, c = {c:.4e}");
    println!(
        "  (HKS Theorem 1.2 predicts exponential decay rate; compare beta to predicted value)"
    );
}
