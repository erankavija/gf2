// png.rs — minimal pure-Rust RGB PNG encoder (no external dependencies).
//
// Writes a valid 24-bit RGB PNG using zlib stored blocks (DEFLATE type 0 —
// uncompressed).  This produces a larger file than a compressed PNG but is
// entirely correct and requires zero external dependencies or unsafe code.
//
// Reference: PNG specification ISO/IEC 15948:2004, RFC 1951 (DEFLATE).

use std::io::Write;

// ---------------------------------------------------------------------------
// CRC-32 (PNG chunk integrity)
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, entry) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xedb8_8320 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
            }
            *entry = c;
        }
        t
    });
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

// ---------------------------------------------------------------------------
// Adler-32 (zlib checksum)
// ---------------------------------------------------------------------------

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

// ---------------------------------------------------------------------------
// PNG chunk writer
// ---------------------------------------------------------------------------

fn write_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    // Length (big-endian u32)
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    // Type
    out.extend_from_slice(tag);
    // Data
    out.extend_from_slice(data);
    // CRC over type + data
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(tag);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

// ---------------------------------------------------------------------------
// DEFLATE stored blocks (type 0 — uncompressed)
// ---------------------------------------------------------------------------

/// Wrap `raw_data` in a valid DEFLATE stream using stored blocks.
/// Max block payload per RFC 1951: 65535 bytes.
fn deflate_stored(raw_data: &[u8]) -> Vec<u8> {
    const MAX_BLOCK: usize = 65535;
    let mut out = Vec::new();

    let mut offset = 0;
    while offset < raw_data.len() || raw_data.is_empty() {
        let end = (offset + MAX_BLOCK).min(raw_data.len());
        let block = &raw_data[offset..end];
        let is_last = end == raw_data.len();
        let bfinal: u8 = if is_last { 1 } else { 0 };
        // BFINAL | BTYPE=00 (stored)
        out.push(bfinal);
        let len = block.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(block);
        offset = end;
        if raw_data.is_empty() {
            break;
        }
    }
    out
}

/// Wrap DEFLATE data in a zlib container (RFC 1950).
fn zlib_wrap(raw_data: &[u8]) -> Vec<u8> {
    let checksum = adler32(raw_data);
    let deflated = deflate_stored(raw_data);
    let mut out = Vec::new();
    // CMF: CM=8 (deflate), CINFO=7 (window 32K) -> 0x78
    // FLG: chosen so (CMF*256+FLG) % 31 == 0 -> 0x01
    out.push(0x78);
    out.push(0x01);
    out.extend_from_slice(&deflated);
    out.extend_from_slice(&checksum.to_be_bytes());
    out
}

// ---------------------------------------------------------------------------
// PNG encoder
// ---------------------------------------------------------------------------

/// Encode an RGB pixel buffer as a PNG file, writing bytes into `out`.
///
/// # Arguments
///
/// * `pixels` — flat RGB byte slice, row-major, length == width * height * 3.
/// * `width`, `height` — image dimensions in pixels.
/// * `out` — output byte buffer.
///
/// # Examples
///
/// ```
/// use perm_uniformity::png::encode_png;
/// let mut out = Vec::new();
/// // One white pixel.
/// encode_png(&[255, 255, 255], 1, 1, &mut out);
/// assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
/// ```
///
/// # Panics
///
/// Panics if `pixels.len() != width * height * 3`.
///
/// # Complexity
///
/// `O(width * height)` (one pass to filter scanlines, one stored-deflate pass).
pub fn encode_png(pixels: &[u8], width: usize, height: usize, out: &mut Vec<u8>) {
    assert_eq!(
        pixels.len(),
        width * height * 3,
        "pixel buffer size mismatch"
    );

    // PNG signature
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // colour type: RGB
    ihdr.push(0); // compression method: deflate
    ihdr.push(0); // filter method: adaptive
    ihdr.push(0); // interlace: none
    write_chunk(out, b"IHDR", &ihdr);

    // Build filter-prepended scanlines (filter type 0 = None for every row).
    let mut scanlines: Vec<u8> = Vec::with_capacity(height * (1 + width * 3));
    for row in 0..height {
        scanlines.push(0); // filter byte: None
        let row_start = row * width * 3;
        scanlines.extend_from_slice(&pixels[row_start..row_start + width * 3]);
    }

    // IDAT (zlib-wrapped deflate-stored scanlines)
    let idat_data = zlib_wrap(&scanlines);
    write_chunk(out, b"IDAT", &idat_data);

    // IEND
    write_chunk(out, b"IEND", b"");
}

/// Write an RGB pixel buffer as a PNG file.
///
/// Returns `Ok(())` on success, or an IO error.
///
/// # Arguments
///
/// * `path` — destination file path.
/// * `pixels` — flat RGB byte slice, row-major, length == width * height * 3.
/// * `width`, `height` — image dimensions in pixels.
///
/// # Examples
///
/// ```
/// use perm_uniformity::png::write_png_file;
/// let mut p = std::env::temp_dir();
/// p.push("perm_uniformity_doctest_1x1.png");
/// let path = p.to_str().unwrap();
/// write_png_file(path, &[0, 0, 0], 1, 1).unwrap();
/// assert!(std::path::Path::new(path).exists());
/// std::fs::remove_file(path).ok();
/// ```
///
/// # Panics
///
/// Panics if `pixels.len() != width * height * 3` (propagated from
/// [`encode_png`]). Filesystem failures are returned as `Err`, not panics.
///
/// # Complexity
///
/// `O(width * height)` plus a single file write.
pub fn write_png_file(
    path: &str,
    pixels: &[u8],
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    encode_png(pixels, width, height, &mut buf);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_signature() {
        let pixels = vec![255u8; 4 * 4 * 3]; // 4x4 white image
        let mut out = Vec::new();
        encode_png(&pixels, 4, 4, &mut out);
        // PNG signature: 137 80 78 71 13 10 26 10
        assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n", "PNG signature mismatch");
    }

    #[test]
    fn test_png_ihdr_chunk() {
        let pixels = vec![0u8; 10 * 5 * 3]; // 10x5 black image
        let mut out = Vec::new();
        encode_png(&pixels, 10, 5, &mut out);
        // After 8-byte signature: IHDR chunk
        // [4-byte length][4-byte "IHDR"][13-byte data][4-byte CRC]
        let ihdr_len = u32::from_be_bytes(out[8..12].try_into().unwrap());
        assert_eq!(ihdr_len, 13, "IHDR data length must be 13");
        assert_eq!(&out[12..16], b"IHDR", "chunk type must be IHDR");
        let w = u32::from_be_bytes(out[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(out[20..24].try_into().unwrap());
        assert_eq!(w, 10, "IHDR width mismatch");
        assert_eq!(h, 5, "IHDR height mismatch");
        assert_eq!(out[24], 8, "bit depth must be 8");
        assert_eq!(out[25], 2, "colour type must be 2 (RGB)");
    }

    #[test]
    fn test_png_iend_chunk() {
        let pixels = vec![128u8; 2 * 2 * 3];
        let mut out = Vec::new();
        encode_png(&pixels, 2, 2, &mut out);
        // Last 12 bytes: 4-byte zero length + b"IEND" + 4-byte CRC
        let tail = &out[out.len() - 12..];
        assert_eq!(&tail[..4], &[0, 0, 0, 0], "IEND length must be 0");
        assert_eq!(&tail[4..8], b"IEND", "last chunk must be IEND");
    }

    #[test]
    fn test_png_roundtrip_to_file() {
        // Write to a temp file and check the file is at minimum the right size.
        let pixels = vec![200u8; 8 * 8 * 3];
        let path = "/tmp/perm_uniformity_test_png_roundtrip.png";
        write_png_file(path, &pixels, 8, 8).expect("write_png_file failed");
        let data = std::fs::read(path).expect("read back failed");
        assert!(data.len() > 8 + 25 + 12, "PNG too small"); // sig + IHDR + IEND
        assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_file(path);
    }
}
