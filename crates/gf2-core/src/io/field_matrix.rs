//! Canonical, identity-carrying serialization for [`FieldMatrix`].
//!
//! The format is deliberately independent of the legacy [`BitMatrix`] `.gf2`
//! format. It stores algebraic field identity and element representation, so
//! a byte stream is meaningful only when the caller supplies an equivalent
//! field witness while loading.
//!
//! # Format specification
//!
//! A file consists of a fixed header, an encoded [`FieldId`], a row-major
//! element payload, and a 32-byte BLAKE3 digest. All integers are unsigned
//! little-endian values. The digest is computed over every preceding byte in
//! the file: the fixed header, the field identity, and the payload. Thus a
//! change to either metadata or element bytes is detected before elements are
//! reconstructed.
//!
//! The fixed header is 44 bytes:
//!
//! | Offset | Size | Field |
//! | ---: | ---: | --- |
//! | 0 | 8 | Magic `GF2FMAT\0` |
//! | 8 | 2 | Container format version, currently [`FIELD_MATRIX_FORMAT_VERSION`] |
//! | 10 | 1 | [`FIELD_ID_ENCODING_VERSION`] used by the identity section |
//! | 11 | 1 | [`ELEMENT_REPR_VERSION`] used by the representation fields |
//! | 12 | 1 | Element representation tag, `1` for `PrimeCoordsLe` |
//! | 13 | 1 | Bytes per canonical prime coordinate |
//! | 14 | 2 | Reserved; must be zero |
//! | 16 | 8 | Row count |
//! | 24 | 8 | Column count |
//! | 32 | 4 | Field-identity section length |
//! | 36 | 8 | Payload length |
//!
//! The identity section is exactly [`FieldId::encode`] with the length in the
//! header. Its first byte repeats the field-identity encoding version. The
//! payload contains `rows * cols` elements in row-major order. Each element
//! contains `field_id.degree()` canonical coordinates, and every coordinate
//! occupies `coord_width` little-endian bytes. The representation is therefore
//! `ElementRepr::PrimeCoordsLe { coord_width }`, with `coord_width` equal to
//! [`FieldId::coordinate_width`]. The checksum follows the payload and is not
//! included in the payload length.
//!
//! Container, field-identity, and element-representation versions are
//! independent compatibility boundaries. A loader rejects an unknown version;
//! it never guesses how to reinterpret an older or newer stream. The legacy
//! `BitMatrix` format remains a separate format and is not accepted here.
//!
//! # Examples
//!
//! ```
//! use std::io::Cursor;
//! use gf2_core::field::matrix::FieldMatrix;
//! use gf2_core::gfp::Fp;
//! use gf2_core::io::field_matrix::{read_from, write_to};
//!
//! let matrix = FieldMatrix::<Fp<7>>::identity(2);
//! let mut bytes = Vec::new();
//! write_to(&matrix, &mut bytes)?;
//! let restored = read_from(Cursor::new(bytes), &Fp::<7>::new(0))?;
//! assert_eq!(restored, matrix);
//! # Ok::<(), gf2_core::io::IoError>(())
//! ```

use std::convert::TryFrom;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::field::extension::{
    ElementRepr, FieldError, FieldId, FieldIdentity, ELEMENT_REPR_VERSION,
    FIELD_ID_ENCODING_VERSION,
};
use crate::field::matrix::FieldMatrix;
use crate::io::{IoError, Result};

/// Magic bytes identifying the canonical `FieldMatrix` container.
pub const FIELD_MATRIX_MAGIC: &[u8; 8] = b"GF2FMAT\0";

/// Current version of the `FieldMatrix` container layout.
pub const FIELD_MATRIX_FORMAT_VERSION: u16 = 1;

/// Number of bytes in the fixed `FieldMatrix` header.
pub const FIELD_MATRIX_HEADER_SIZE: usize = 44;

const CHECKSUM_SIZE: usize = 32;
const ELEMENT_REPR_TAG_PRIME_COORDS_LE: u8 = 1;

/// Writes a `FieldMatrix` in the canonical binary format to `writer`.
///
/// This low-level function does not provide atomicity. Use
/// [`FieldMatrix::save_to_file`] for a destination path that must be replaced
/// atomically.
///
/// # Errors
///
/// Returns [`IoError::FieldIdentityMismatch`] if elements in one matrix carry
/// different identities, [`IoError::Field`] if an element violates its
/// coordinate contract, or [`IoError::Io`] when `writer` rejects bytes.
pub fn write_to<F: FieldIdentity, W: Write>(matrix: &FieldMatrix<F>, writer: &mut W) -> Result<()> {
    let bytes = encode(matrix)?;
    writer.write_all(&bytes)?;
    Ok(())
}

/// Reads a canonical `FieldMatrix` from `reader` using `expected_field` as the
/// reconstruction witness and identity contract.
///
/// # Errors
///
/// Returns a typed [`IoError`] for unsupported versions, malformed or
/// truncated input, identity or representation mismatches, inconsistent
/// dimensions, checksum failure, I/O failure, or rejected field coordinates.
pub fn read_from<F: FieldIdentity, R: Read>(
    mut reader: R,
    expected_field: &F,
) -> Result<FieldMatrix<F>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    decode(&bytes, expected_field)
}

impl<F: FieldIdentity> FieldMatrix<F> {
    /// Atomically replaces `path` with this matrix's canonical serialization.
    ///
    /// The complete byte stream is written to a PID-tagged sibling temporary
    /// file, synced, renamed into place, and followed by a directory sync. A
    /// failure before rename removes the temporary file and leaves the
    /// destination at its prior state. A directory-sync failure is reported
    /// after the complete replacement has already been renamed into place.
    ///
    /// # Errors
    ///
    /// Returns [`IoError::FieldIdentityMismatch`] for inconsistent element
    /// identities, [`IoError::Field`] for an invalid element coordinate, or
    /// [`IoError::Io`] for serialization and filesystem failures.
    ///
    /// # Examples
    ///
    /// ```
    /// # use gf2_core::field::matrix::FieldMatrix;
    /// # use gf2_core::gfp::Fp;
    /// # let matrix = FieldMatrix::<Fp<7>>::identity(2);
    /// let path = std::env::temp_dir().join("gf2-field-matrix-example.bin");
    /// matrix.save_to_file(&path)?;
    /// let restored = FieldMatrix::<Fp<7>>::load_from_file(&path, &Fp::<7>::new(0))?;
    /// assert_eq!(restored, matrix);
    /// let _ = std::fs::remove_file(path);
    /// # Ok::<(), gf2_core::io::IoError>(())
    /// ```
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = encode(self)?;
        atomic_write(path.as_ref(), &bytes)
    }

    /// Loads a matrix from `path` and validates it against `expected_field`.
    ///
    /// The expected field supplies both the algebraic identity check and the
    /// witness used to reconstruct each element. Runtime-configured fields
    /// must therefore pass an element from the desired runtime field.
    ///
    /// # Errors
    ///
    /// Returns a typed [`IoError`] for every rejected version, malformed or
    /// truncated stream, identity or representation mismatch, inconsistent
    /// dimensions, checksum failure, I/O failure, or invalid coordinate.
    pub fn load_from_file<P: AsRef<Path>>(path: P, expected_field: &F) -> Result<Self> {
        let mut file = File::open(path)?;
        read_from(&mut file, expected_field)
    }

    /// Writes this matrix to an arbitrary writer in the canonical format.
    ///
    /// # Errors
    ///
    /// Returns the same serialization and writer errors as [`write_to`]. This
    /// method is not atomic; use [`Self::save_to_file`] for filesystem output.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_to(self, writer)
    }

    /// Reads a matrix from an arbitrary reader and validates its field.
    ///
    /// # Errors
    ///
    /// Returns the same typed validation, corruption, and reader errors as
    /// [`read_from`].
    pub fn read_from<R: Read>(reader: R, expected_field: &F) -> Result<Self> {
        read_from(reader, expected_field)
    }
}

fn encode<F: FieldIdentity>(matrix: &FieldMatrix<F>) -> Result<Vec<u8>> {
    let (rows, cols) = matrix.shape();
    let element_count = rows
        .checked_mul(cols)
        .ok_or_else(|| IoError::InvalidData("matrix dimensions overflow usize".to_string()))?;
    let field_id = matrix_identity(matrix)?;
    let coord_width = checked_coord_width(&field_id)?;
    let degree = checked_field_degree(&field_id)?;
    let element_width = degree.checked_mul(coord_width as usize).ok_or_else(|| {
        IoError::InvalidData("element representation size overflows usize".to_string())
    })?;
    let payload_len = element_count
        .checked_mul(element_width)
        .ok_or_else(|| IoError::InvalidData("matrix payload size overflows usize".to_string()))?;
    let field_id_bytes = field_id.encode();
    let field_id_len = u32::try_from(field_id_bytes.len())
        .map_err(|_| IoError::InvalidData("field identity encoding is too long".to_string()))?;
    let rows = u64::try_from(rows)
        .map_err(|_| IoError::InvalidData("row count does not fit in u64".to_string()))?;
    let cols = u64::try_from(cols)
        .map_err(|_| IoError::InvalidData("column count does not fit in u64".to_string()))?;
    let payload_len_u64 = u64::try_from(payload_len)
        .map_err(|_| IoError::InvalidData("payload length does not fit in u64".to_string()))?;

    let mut bytes = Vec::new();
    bytes.reserve(
        FIELD_MATRIX_HEADER_SIZE
            .checked_add(field_id_bytes.len())
            .and_then(|length| length.checked_add(payload_len))
            .and_then(|length| length.checked_add(CHECKSUM_SIZE))
            .ok_or_else(|| {
                IoError::InvalidData("serialized matrix size overflows usize".to_string())
            })?,
    );
    write_header(
        &mut bytes,
        rows,
        cols,
        field_id_len,
        payload_len_u64,
        coord_width,
    );
    bytes.extend_from_slice(&field_id_bytes);

    let characteristic = field_id.characteristic();
    let mut coordinates = Vec::with_capacity(degree);
    for row in 0..matrix.rows() {
        for element in matrix.row(row) {
            let found = element.field_id();
            if found != field_id {
                return Err(IoError::FieldIdentityMismatch {
                    expected: field_id,
                    found,
                });
            }
            coordinates.clear();
            element.write_prime_coords(&mut coordinates);
            if coordinates.len() != degree {
                return Err(IoError::InvalidData(format!(
                    "element wrote {} coordinates, expected {}",
                    coordinates.len(),
                    degree
                )));
            }
            for (index, &coordinate) in coordinates.iter().enumerate() {
                if coordinate >= characteristic {
                    return Err(IoError::Field(FieldError::CoordinateOutOfRange {
                        index,
                        value: coordinate,
                        characteristic,
                    }));
                }
                bytes.extend_from_slice(&coordinate.to_le_bytes()[..coord_width as usize]);
            }
        }
    }
    debug_assert_eq!(
        bytes.len(),
        FIELD_MATRIX_HEADER_SIZE + field_id_bytes.len() + payload_len
    );
    append_checksum(&mut bytes);
    Ok(bytes)
}

fn decode<F: FieldIdentity>(bytes: &[u8], expected_field: &F) -> Result<FieldMatrix<F>> {
    let mut cursor = Cursor::new(bytes);
    if cursor.remaining() < FIELD_MATRIX_HEADER_SIZE + CHECKSUM_SIZE {
        return Err(IoError::InvalidData(
            "truncated FieldMatrix header".to_string(),
        ));
    }
    let magic = cursor.take(8)?;
    if magic != FIELD_MATRIX_MAGIC {
        return Err(IoError::InvalidMagic);
    }
    let version = cursor.take_u16()?;
    if version != FIELD_MATRIX_FORMAT_VERSION {
        return Err(IoError::UnsupportedVersion(version));
    }
    let field_id_version = cursor.take_u8()?;
    if field_id_version != FIELD_ID_ENCODING_VERSION {
        return Err(IoError::UnsupportedFieldIdEncodingVersion(field_id_version));
    }
    let element_repr_version = cursor.take_u8()?;
    if element_repr_version != ELEMENT_REPR_VERSION {
        return Err(IoError::UnsupportedElementRepresentationVersion(
            element_repr_version,
        ));
    }
    let element_repr_tag = cursor.take_u8()?;
    let coord_width = cursor.take_u8()?;
    if cursor.take(2)? != [0, 0] {
        return Err(IoError::InvalidData(
            "non-zero reserved header bytes".to_string(),
        ));
    }
    let rows_u64 = cursor.take_u64()?;
    let cols_u64 = cursor.take_u64()?;
    let field_id_len = cursor.take_u32()?;
    let payload_len = cursor.take_u64()?;

    let field_id_len = usize::try_from(field_id_len).map_err(|_| {
        IoError::InvalidData("field identity length does not fit usize".to_string())
    })?;
    let payload_len_usize = usize::try_from(payload_len)
        .map_err(|_| IoError::InvalidData("payload length does not fit usize".to_string()))?;
    let expected_file_len = FIELD_MATRIX_HEADER_SIZE
        .checked_add(field_id_len)
        .and_then(|length| length.checked_add(payload_len_usize))
        .and_then(|length| length.checked_add(CHECKSUM_SIZE))
        .ok_or_else(|| {
            IoError::InvalidData("serialized matrix size overflows usize".to_string())
        })?;
    if bytes.len() < expected_file_len {
        return Err(IoError::InvalidData(
            "truncated FieldMatrix payload".to_string(),
        ));
    }
    if bytes.len() > expected_file_len {
        return Err(IoError::InvalidData(
            "trailing bytes after FieldMatrix checksum".to_string(),
        ));
    }

    let checksum_offset = expected_file_len - CHECKSUM_SIZE;
    let field_id_end = FIELD_MATRIX_HEADER_SIZE
        .checked_add(field_id_len)
        .ok_or_else(|| IoError::InvalidData("field identity range overflows usize".to_string()))?;
    let field_id_bytes = &bytes[FIELD_MATRIX_HEADER_SIZE..field_id_end];
    let encoded_version = field_id_bytes
        .first()
        .copied()
        .ok_or_else(|| IoError::InvalidData("empty field identity encoding".to_string()))?;
    if encoded_version != FIELD_ID_ENCODING_VERSION {
        return Err(IoError::UnsupportedFieldIdEncodingVersion(encoded_version));
    }
    let field_id = std::panic::catch_unwind(|| FieldId::decode(field_id_bytes))
        .map_err(|_| {
            IoError::InvalidData("field identity encoding overflows field degree".to_string())
        })?
        .map_err(IoError::Field)?;
    let stored_repr = match element_repr_tag {
        ELEMENT_REPR_TAG_PRIME_COORDS_LE => ElementRepr::PrimeCoordsLe { coord_width },
        tag => {
            return Err(IoError::InvalidData(format!(
                "unknown element representation tag: {tag}"
            )))
        }
    };
    let expected_coord_width = checked_coord_width(&field_id)?;
    let expected_repr = ElementRepr::PrimeCoordsLe {
        coord_width: expected_coord_width,
    };
    if stored_repr != expected_repr {
        return Err(IoError::ElementRepresentationMismatch {
            expected: expected_repr,
            found: stored_repr,
        });
    }

    let expected_identity = expected_field.field_id();
    if field_id != expected_identity {
        return Err(IoError::FieldIdentityMismatch {
            expected: expected_identity,
            found: field_id,
        });
    }

    let rows = usize::try_from(rows_u64)
        .map_err(|_| IoError::InvalidData("row count does not fit usize".to_string()))?;
    let cols = usize::try_from(cols_u64)
        .map_err(|_| IoError::InvalidData("column count does not fit usize".to_string()))?;
    let degree = checked_field_degree(&field_id)?;
    let element_width = degree
        .checked_mul(expected_coord_width as usize)
        .ok_or_else(|| {
            IoError::InvalidData("element representation size overflows usize".to_string())
        })?;
    let element_count = rows
        .checked_mul(cols)
        .ok_or_else(|| IoError::InvalidData("matrix dimensions overflow usize".to_string()))?;
    let expected_payload = element_count
        .checked_mul(element_width)
        .ok_or_else(|| IoError::InvalidData("matrix payload size overflows usize".to_string()))?;
    let expected_payload_u64 = u64::try_from(expected_payload)
        .map_err(|_| IoError::InvalidData("payload length does not fit u64".to_string()))?;
    if payload_len != expected_payload_u64 {
        return Err(IoError::DimensionMismatch {
            expected: expected_payload_u64,
            found: payload_len,
        });
    }

    let expected_checksum = &bytes[checksum_offset..];
    let actual_checksum = blake3::hash(&bytes[..checksum_offset]);
    if expected_checksum != actual_checksum.as_bytes() {
        return Err(IoError::ChecksumMismatch);
    }

    let payload = &bytes[field_id_end..checksum_offset];
    let mut payload_cursor = Cursor::new(payload);
    let mut matrix = FieldMatrix::new(rows, cols, expected_field.zero_like());
    let mut coordinates = Vec::with_capacity(degree);
    for row in 0..rows {
        for col in 0..cols {
            coordinates.clear();
            for _ in 0..degree {
                let coordinate = payload_cursor.take_coordinate(expected_coord_width as usize)?;
                coordinates.push(coordinate);
            }
            let element = expected_field
                .from_prime_coords(&coordinates)
                .map_err(IoError::Field)?;
            matrix.set(row, col, element);
        }
    }
    if payload_cursor.remaining() != 0 {
        return Err(IoError::DimensionMismatch {
            expected: expected_payload_u64,
            found: payload_len,
        });
    }
    Ok(matrix)
}

fn matrix_identity<F: FieldIdentity>(matrix: &FieldMatrix<F>) -> Result<FieldId> {
    if matrix.rows() != 0 && matrix.cols() != 0 {
        return Ok(matrix.row(0)[0].field_id());
    }
    F::field_id_hint().ok_or_else(|| {
        IoError::InvalidData(
            "an empty runtime FieldMatrix needs a field identity hint to serialize".to_string(),
        )
    })
}

fn checked_coord_width(field_id: &FieldId) -> Result<u8> {
    u8::try_from(field_id.coordinate_width())
        .map_err(|_| IoError::InvalidData("field coordinate width does not fit u8".to_string()))
}

fn checked_field_degree(field_id: &FieldId) -> Result<usize> {
    std::panic::catch_unwind(|| field_id.degree())
        .map_err(|_| IoError::InvalidData("field identity degree overflows usize".to_string()))
}

fn write_header(
    bytes: &mut Vec<u8>,
    rows: u64,
    cols: u64,
    field_id_len: u32,
    payload_len: u64,
    coord_width: u8,
) {
    bytes.extend_from_slice(FIELD_MATRIX_MAGIC);
    bytes.extend_from_slice(&FIELD_MATRIX_FORMAT_VERSION.to_le_bytes());
    bytes.push(FIELD_ID_ENCODING_VERSION);
    bytes.push(ELEMENT_REPR_VERSION);
    bytes.push(ELEMENT_REPR_TAG_PRIME_COORDS_LE);
    bytes.push(coord_width);
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(&rows.to_le_bytes());
    bytes.extend_from_slice(&cols.to_le_bytes());
    bytes.extend_from_slice(&field_id_len.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
}

fn append_checksum(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(blake3::hash(bytes).as_bytes());
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().ok_or_else(|| {
        IoError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FieldMatrix path must name a file",
        ))
    })?;
    let temporary = parent.join(format!(
        "{}.{}.tmp",
        stem.to_string_lossy(),
        std::process::id()
    ));
    let mut guard = TemporaryFileGuard::new(temporary.clone());

    {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)?;
    guard.keep();
    File::open(parent)?.sync_all()?;
    Ok(())
}

struct TemporaryFileGuard {
    path: PathBuf,
    keep: bool,
}

impl TemporaryFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| IoError::InvalidData("input offset overflows usize".to_string()))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| IoError::InvalidData("truncated FieldMatrix input".to_string()))?;
        self.offset = end;
        Ok(slice)
    }

    fn take_u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn take_u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn take_u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn take_u64(&mut self) -> Result<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn take_coordinate(&mut self, width: usize) -> Result<u64> {
        if width == 0 || width > 8 {
            return Err(IoError::InvalidData("invalid coordinate width".to_string()));
        }
        let bytes = self.take(width)?;
        let mut coordinate = [0u8; 8];
        coordinate[..width].copy_from_slice(bytes);
        Ok(u64::from_le_bytes(coordinate))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::extension::{FieldExtension, FieldIdentity};
    use crate::field::{ConstField, FieldPoly};
    use crate::gfp::Fp;
    use crate::gfpn::{ExtConfig, QuadraticExt, QuotientElement, QuotientField};
    use std::io::Cursor as IoCursor;

    fn tempdir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gf2-field-matrix-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn round_trip<F: FieldIdentity>(matrix: &FieldMatrix<F>, witness: &F) {
        let mut bytes = Vec::new();
        write_to(matrix, &mut bytes).unwrap();
        let restored = read_from(IoCursor::new(bytes), witness).unwrap();
        assert_eq!(&restored, matrix);
    }

    #[test]
    fn round_trip_gf2_and_prime_field() {
        let mut binary = FieldMatrix::<Fp<2>>::zeros(2, 3);
        binary.set(0, 1, Fp::new(1));
        binary.set(1, 2, Fp::new(1));
        round_trip(&binary, &Fp::<2>::new(0));

        let mut prime = FieldMatrix::<Fp<7>>::zeros(2, 2);
        prime.set(0, 0, Fp::new(6));
        prime.set(0, 1, Fp::new(2));
        prime.set(1, 0, Fp::new(4));
        prime.set(1, 1, Fp::new(1));
        round_trip(&prime, &Fp::<7>::new(0));
    }

    fn gf4() -> QuotientField<Fp<2>> {
        QuotientField::new(
            Fp::<2>::zero(),
            FieldPoly::new(vec![Fp::new(1), Fp::new(1), Fp::new(1)]),
        )
        .unwrap()
    }

    #[test]
    fn round_trip_runtime_quotient_field() {
        let field = gf4();
        let zero = field.element(vec![Fp::new(0)]).unwrap();
        let one = field.element(vec![Fp::new(1)]).unwrap();
        let x = field.element(vec![Fp::new(0), Fp::new(1)]).unwrap();
        let matrix = FieldMatrix::from_rows(vec![
            vec![zero.clone(), one.clone()].into(),
            vec![x.clone(), (one + x)].into(),
        ]);
        round_trip(&matrix, &zero);
    }

    struct Gf49Config;

    impl ExtConfig for Gf49Config {
        type BaseField = Fp<7>;
        const NON_RESIDUE: Fp<7> = Fp::<7>::new(3);
    }

    #[test]
    fn runtime_and_compile_time_field_identities_match_for_round_trip() {
        let runtime = QuotientField::new(
            Fp::<7>::zero(),
            FieldPoly::new(vec![Fp::new(4), Fp::new(0), Fp::new(1)]),
        )
        .unwrap();
        let compile_time_id = <QuadraticExt<Gf49Config> as FieldIdentity>::field_id_hint().unwrap();
        assert_eq!(runtime.ext_id(), &compile_time_id);

        let witness = runtime.element(vec![Fp::new(2)]).unwrap();
        let matrix = FieldMatrix::new(1, 2, witness.clone());
        round_trip(&matrix, &witness);
    }

    #[test]
    fn identity_mismatch_is_typed() {
        let matrix = FieldMatrix::<Fp<7>>::identity(1);
        let mut bytes = Vec::new();
        write_to(&matrix, &mut bytes).unwrap();
        let error = read_from(IoCursor::new(bytes), &Fp::<5>::new(0)).unwrap_err();
        assert!(matches!(error, IoError::FieldIdentityMismatch { .. }));
    }

    #[test]
    fn representation_and_container_versions_are_typed() {
        let matrix = FieldMatrix::<Fp<7>>::identity(1);
        let mut bytes = Vec::new();
        write_to(&matrix, &mut bytes).unwrap();

        let mut representation_version = bytes.clone();
        representation_version[11] = ELEMENT_REPR_VERSION.wrapping_add(1);
        assert!(matches!(
            read_from(IoCursor::new(representation_version), &Fp::<7>::new(0)),
            Err(IoError::UnsupportedElementRepresentationVersion(_))
        ));

        let mut representation = bytes.clone();
        representation[13] = 2;
        assert!(matches!(
            read_from(IoCursor::new(representation), &Fp::<7>::new(0)),
            Err(IoError::ElementRepresentationMismatch { .. })
        ));

        let mut container_version = bytes;
        container_version[8] = 2;
        assert!(matches!(
            read_from(IoCursor::new(container_version), &Fp::<7>::new(0)),
            Err(IoError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn payload_truncation_header_corruption_and_flip_are_typed() {
        let matrix = FieldMatrix::<Fp<7>>::identity(2);
        let mut bytes = Vec::new();
        write_to(&matrix, &mut bytes).unwrap();

        let truncated = bytes[..bytes.len() - 1].to_vec();
        assert!(matches!(
            read_from(IoCursor::new(truncated), &Fp::<7>::new(0)),
            Err(IoError::InvalidData(_))
        ));

        let mut payload = bytes.clone();
        let payload_offset = FIELD_MATRIX_HEADER_SIZE + 10;
        payload[payload_offset] ^= 1;
        assert!(matches!(
            read_from(IoCursor::new(payload), &Fp::<7>::new(0)),
            Err(IoError::ChecksumMismatch)
        ));

        let mut header = bytes.clone();
        header[0] ^= 1;
        assert!(matches!(
            read_from(IoCursor::new(header), &Fp::<7>::new(0)),
            Err(IoError::InvalidMagic)
        ));

        let mut dimensions = bytes;
        dimensions[16..24].copy_from_slice(&1u64.to_le_bytes());
        assert!(matches!(
            read_from(IoCursor::new(dimensions), &Fp::<7>::new(0)),
            Err(IoError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn atomic_replacement_and_failed_write_clean_up_temporary_file() {
        let directory = tempdir("atomic");
        let path = directory.join("matrix.fm");
        let first = FieldMatrix::<Fp<7>>::identity(1);
        first.save_to_file(&path).unwrap();

        let mut second = FieldMatrix::<Fp<7>>::identity(1);
        second.set(0, 0, Fp::new(3));
        second.save_to_file(&path).unwrap();
        assert_eq!(
            FieldMatrix::load_from_file(&path, &Fp::<7>::new(0)).unwrap(),
            second
        );

        let destination_directory = directory.join("blocked");
        fs::create_dir(&destination_directory).unwrap();
        assert!(matches!(
            first.save_to_file(&destination_directory),
            Err(IoError::Io(_))
        ));
        let temporary = directory.join(format!("blocked.{}.tmp", std::process::id()));
        assert!(!temporary.exists());

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn malformed_field_identity_is_typed_without_panicking() {
        let matrix = FieldMatrix::<Fp<7>>::identity(1);
        let mut bytes = Vec::new();
        write_to(&matrix, &mut bytes).unwrap();
        bytes[FIELD_MATRIX_HEADER_SIZE] = 0xFF;
        let error = read_from(IoCursor::new(bytes), &Fp::<7>::new(0)).unwrap_err();
        assert!(matches!(
            error,
            IoError::ChecksumMismatch
                | IoError::Field(_)
                | IoError::UnsupportedFieldIdEncodingVersion(_)
        ));
    }

    #[allow(dead_code)]
    fn _assert_element_type(_: QuotientElement<Fp<2>>) {}
}
