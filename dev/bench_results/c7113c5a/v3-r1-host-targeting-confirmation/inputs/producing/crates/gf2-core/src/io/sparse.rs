//! Sparse matrix serialization and deserialization.

use super::{error::*, format::*};
use crate::{SpBitMatrix, SpBitMatrixDual};
use std::io::{Read, Write};

/// Metadata for SpBitMatrix serialization
#[derive(Debug)]
#[cfg_attr(feature = "io", derive(serde::Serialize, serde::Deserialize))]
struct SpBitMatrixMetadata {
    #[serde(rename = "type")]
    type_name: String,
    rows: usize,
    cols: usize,
    nnz: usize,
    format: String,
    version: u32,
}

/// Metadata for SpBitMatrixDual serialization
#[derive(Debug)]
#[cfg_attr(feature = "io", derive(serde::Serialize, serde::Deserialize))]
struct SpBitMatrixDualMetadata {
    #[serde(rename = "type")]
    type_name: String,
    rows: usize,
    cols: usize,
    nnz: usize,
    version: u32,
}

impl SpBitMatrix {
    /// Save SpBitMatrix to a file with specified format
    pub fn save_to_file_with_format<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        format: super::SerializationFormat,
    ) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to_with_format(&mut writer, format)
    }

    /// Save SpBitMatrix to a file (binary COO format)
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.save_to_file_with_format(path, super::SerializationFormat::Binary)
    }

    /// Load SpBitMatrix from a file (binary format)
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from(&mut reader)
    }

    /// Load SpBitMatrix from a file with specified format
    pub fn load_from_file_with_format<P: AsRef<std::path::Path>>(
        path: P,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from_with_format(&mut reader, format)
    }

    /// Write SpBitMatrix to a writer with specified format
    pub fn write_to_with_format<W: Write>(
        &self,
        writer: &mut W,
        format: super::SerializationFormat,
    ) -> Result<()> {
        match format {
            super::SerializationFormat::Binary => self.write_binary(writer),
            super::SerializationFormat::Text => self.write_text(writer),
            super::SerializationFormat::Hex => Err(IoError::InvalidData(
                "Hex format not supported for sparse matrices".to_string(),
            )),
        }
    }

    /// Write SpBitMatrix to a writer (binary COO format)
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.write_binary(writer)
    }

    /// Write SpBitMatrix in binary COO format
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<()> {
        let metadata = SpBitMatrixMetadata {
            type_name: "SpBitMatrix".to_string(),
            rows: self.rows(),
            cols: self.cols(),
            nnz: self.nnz(),
            format: "coo".to_string(),
            version: 1,
        };

        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| IoError::InvalidData(format!("Failed to serialize metadata: {}", e)))?;

        // Data: pairs of (row, col) as u32
        let data_len = self.nnz() * 8; // 2 u32s per edge

        let header = Header::new(
            TypeTag::SpBitMatrix,
            metadata_json.len() as u32,
            data_len as u64,
        );
        header.write_to(writer)?;

        writer.write_all(&metadata_json)?;

        // Write COO format: iterate through CSR and output (row, col) pairs
        for row in 0..self.rows() {
            for col in self.row_iter(row) {
                writer.write_all(&(row as u32).to_le_bytes())?;
                writer.write_all(&(col as u32).to_le_bytes())?;
            }
        }

        Ok(())
    }

    /// Write SpBitMatrix in text format (edge list)
    fn write_text<W: Write>(&self, writer: &mut W) -> Result<()> {
        writeln!(writer, "{} {} {}", self.rows(), self.cols(), self.nnz())?;

        for row in 0..self.rows() {
            for col in self.row_iter(row) {
                writeln!(writer, "{} {}", row, col)?;
            }
        }

        Ok(())
    }

    /// Read SpBitMatrix from reader with specified format
    pub fn read_from_with_format<R: Read>(
        reader: &mut R,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        match format {
            super::SerializationFormat::Binary => Self::read_binary(reader),
            super::SerializationFormat::Text => Self::read_text(reader),
            super::SerializationFormat::Hex => Err(IoError::InvalidData(
                "Hex format not supported for sparse matrices".to_string(),
            )),
        }
    }

    /// Read SpBitMatrix from reader (binary format)
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_binary(reader)
    }

    /// Read SpBitMatrix from binary COO format
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self> {
        let header = Header::read_from(reader)?;

        if header.type_tag != TypeTag::SpBitMatrix {
            return Err(IoError::InvalidData(format!(
                "Expected SpBitMatrix type tag, got {:?}",
                header.type_tag
            )));
        }

        let mut metadata_buf = vec![0u8; header.metadata_len as usize];
        reader.read_exact(&mut metadata_buf)?;

        let metadata: SpBitMatrixMetadata = serde_json::from_slice(&metadata_buf)
            .map_err(|e| IoError::InvalidData(format!("Failed to parse metadata: {}", e)))?;

        if metadata.type_name != "SpBitMatrix" {
            return Err(IoError::InvalidData(format!(
                "Expected SpBitMatrix type, got {}",
                metadata.type_name
            )));
        }

        if metadata.format != "coo" {
            return Err(IoError::InvalidData(format!(
                "Expected COO format, got {}",
                metadata.format
            )));
        }

        let expected_bytes = metadata.nnz * 8;
        if header.data_len as usize != expected_bytes {
            return Err(IoError::InvalidData(format!(
                "Data length mismatch: header says {} bytes, expected {} for {} edges",
                header.data_len, expected_bytes, metadata.nnz
            )));
        }

        // Read COO edges
        let mut edges = Vec::with_capacity(metadata.nnz);
        for _ in 0..metadata.nnz {
            let mut row_buf = [0u8; 4];
            let mut col_buf = [0u8; 4];
            reader.read_exact(&mut row_buf)?;
            reader.read_exact(&mut col_buf)?;
            let row = u32::from_le_bytes(row_buf) as usize;
            let col = u32::from_le_bytes(col_buf) as usize;
            edges.push((row, col));
        }

        Ok(SpBitMatrix::from_coo_deduplicated(
            metadata.rows,
            metadata.cols,
            &edges,
        ))
    }

    /// Read SpBitMatrix from text format (edge list)
    fn read_text<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);

        let mut first_line = String::new();
        reader.read_line(&mut first_line)?;
        let dims: Vec<&str> = first_line.split_whitespace().collect();
        if dims.len() != 3 {
            return Err(IoError::InvalidData(
                "Text format must start with 'rows cols nnz'".to_string(),
            ));
        }

        let rows: usize = dims[0]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid row count".to_string()))?;
        let cols: usize = dims[1]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid column count".to_string()))?;
        let nnz: usize = dims[2]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid nnz count".to_string()))?;

        let mut edges = Vec::with_capacity(nnz);
        for _ in 0..nnz {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(IoError::InvalidData(format!(
                    "Expected 'row col' pair, got: {}",
                    line
                )));
            }
            let row: usize = parts[0]
                .parse()
                .map_err(|_| IoError::InvalidData("Invalid row in edge".to_string()))?;
            let col: usize = parts[1]
                .parse()
                .map_err(|_| IoError::InvalidData("Invalid col in edge".to_string()))?;
            edges.push((row, col));
        }

        Ok(SpBitMatrix::from_coo_deduplicated(rows, cols, &edges))
    }
}

impl SpBitMatrixDual {
    /// Save SpBitMatrixDual to a file with specified format
    pub fn save_to_file_with_format<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        format: super::SerializationFormat,
    ) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to_with_format(&mut writer, format)
    }

    /// Save SpBitMatrixDual to a file (binary format)
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.save_to_file_with_format(path, super::SerializationFormat::Binary)
    }

    /// Load SpBitMatrixDual from a file (binary format)
    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from(&mut reader)
    }

    /// Load SpBitMatrixDual from a file with specified format
    pub fn load_from_file_with_format<P: AsRef<std::path::Path>>(
        path: P,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_from_with_format(&mut reader, format)
    }

    /// Write SpBitMatrixDual to a writer with specified format
    pub fn write_to_with_format<W: Write>(
        &self,
        writer: &mut W,
        format: super::SerializationFormat,
    ) -> Result<()> {
        match format {
            super::SerializationFormat::Binary => self.write_binary(writer),
            super::SerializationFormat::Text => self.write_text(writer),
            super::SerializationFormat::Hex => Err(IoError::InvalidData(
                "Hex format not supported for sparse matrices".to_string(),
            )),
        }
    }

    /// Write SpBitMatrixDual to a writer (binary format)
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.write_binary(writer)
    }

    /// Write SpBitMatrixDual in binary format (CSR + CSC)
    fn write_binary<W: Write>(&self, writer: &mut W) -> Result<()> {
        let metadata = SpBitMatrixDualMetadata {
            type_name: "SpBitMatrixDual".to_string(),
            rows: self.rows(),
            cols: self.cols(),
            nnz: self.nnz(),
            version: 1,
        };

        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| IoError::InvalidData(format!("Failed to serialize metadata: {}", e)))?;

        // Data: row offsets (rows+1) + row indices (nnz) + col offsets (cols+1) + col indices (nnz)
        let data_len = (self.rows() + 1 + self.nnz() + self.cols() + 1 + self.nnz()) * 4;

        let header = Header::new(
            TypeTag::SpBitMatrixDual,
            metadata_json.len() as u32,
            data_len as u64,
        );
        header.write_to(writer)?;

        writer.write_all(&metadata_json)?;

        // Access internal CSR structure
        // Row offsets
        let row_offsets = self.row_offsets();
        for &offset in row_offsets {
            writer.write_all(&(offset as u32).to_le_bytes())?;
        }

        // Row indices (column indices for each row)
        let row_indices = self.row_indices();
        for &idx in row_indices {
            writer.write_all(&(idx as u32).to_le_bytes())?;
        }

        // Col offsets
        let col_offsets = self.col_offsets();
        for &offset in col_offsets {
            writer.write_all(&(offset as u32).to_le_bytes())?;
        }

        // Col indices (row indices for each column)
        let col_indices = self.col_indices();
        for &idx in col_indices {
            writer.write_all(&(idx as u32).to_le_bytes())?;
        }

        Ok(())
    }

    /// Write SpBitMatrixDual in text format (edge list, same as SpBitMatrix)
    fn write_text<W: Write>(&self, writer: &mut W) -> Result<()> {
        writeln!(writer, "{} {} {}", self.rows(), self.cols(), self.nnz())?;

        for row in 0..self.rows() {
            for col in self.row_iter(row) {
                writeln!(writer, "{} {}", row, col)?;
            }
        }

        Ok(())
    }

    /// Read SpBitMatrixDual from reader with specified format
    pub fn read_from_with_format<R: Read>(
        reader: &mut R,
        format: super::SerializationFormat,
    ) -> Result<Self> {
        match format {
            super::SerializationFormat::Binary => Self::read_binary(reader),
            super::SerializationFormat::Text => Self::read_text(reader),
            super::SerializationFormat::Hex => Err(IoError::InvalidData(
                "Hex format not supported for sparse matrices".to_string(),
            )),
        }
    }

    /// Read SpBitMatrixDual from reader (binary format)
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_binary(reader)
    }

    /// Read SpBitMatrixDual from binary format
    fn read_binary<R: Read>(reader: &mut R) -> Result<Self> {
        let header = Header::read_from(reader)?;

        if header.type_tag != TypeTag::SpBitMatrixDual {
            return Err(IoError::InvalidData(format!(
                "Expected SpBitMatrixDual type tag, got {:?}",
                header.type_tag
            )));
        }

        let mut metadata_buf = vec![0u8; header.metadata_len as usize];
        reader.read_exact(&mut metadata_buf)?;

        let metadata: SpBitMatrixDualMetadata = serde_json::from_slice(&metadata_buf)
            .map_err(|e| IoError::InvalidData(format!("Failed to parse metadata: {}", e)))?;

        if metadata.type_name != "SpBitMatrixDual" {
            return Err(IoError::InvalidData(format!(
                "Expected SpBitMatrixDual type, got {}",
                metadata.type_name
            )));
        }

        let expected_bytes =
            (metadata.rows + 1 + metadata.nnz + metadata.cols + 1 + metadata.nnz) * 4;
        if header.data_len as usize != expected_bytes {
            return Err(IoError::InvalidData(format!(
                "Data length mismatch: header says {} bytes, expected {}",
                header.data_len, expected_bytes
            )));
        }

        // Read row offsets
        let mut row_offsets = Vec::with_capacity(metadata.rows + 1);
        for _ in 0..=metadata.rows {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            row_offsets.push(u32::from_le_bytes(buf) as usize);
        }

        // Read row indices
        let mut row_indices = Vec::with_capacity(metadata.nnz);
        for _ in 0..metadata.nnz {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            row_indices.push(u32::from_le_bytes(buf) as usize);
        }

        // Read col offsets
        let mut col_offsets = Vec::with_capacity(metadata.cols + 1);
        for _ in 0..=metadata.cols {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            col_offsets.push(u32::from_le_bytes(buf) as usize);
        }

        // Read col indices
        let mut col_indices = Vec::with_capacity(metadata.nnz);
        for _ in 0..metadata.nnz {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            col_indices.push(u32::from_le_bytes(buf) as usize);
        }

        // Reconstruct from CSR/CSC data
        Ok(SpBitMatrixDual::from_csr_csc(
            metadata.rows,
            metadata.cols,
            row_offsets,
            row_indices,
            col_offsets,
            col_indices,
        ))
    }

    /// Read SpBitMatrixDual from text format (edge list)
    fn read_text<R: Read>(reader: &mut R) -> Result<Self> {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);

        let mut first_line = String::new();
        reader.read_line(&mut first_line)?;
        let dims: Vec<&str> = first_line.split_whitespace().collect();
        if dims.len() != 3 {
            return Err(IoError::InvalidData(
                "Text format must start with 'rows cols nnz'".to_string(),
            ));
        }

        let rows: usize = dims[0]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid row count".to_string()))?;
        let cols: usize = dims[1]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid column count".to_string()))?;
        let nnz: usize = dims[2]
            .parse()
            .map_err(|_| IoError::InvalidData("Invalid nnz count".to_string()))?;

        let mut edges = Vec::with_capacity(nnz);
        for _ in 0..nnz {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(IoError::InvalidData(format!(
                    "Expected 'row col' pair, got: {}",
                    line
                )));
            }
            let row: usize = parts[0]
                .parse()
                .map_err(|_| IoError::InvalidData("Invalid row in edge".to_string()))?;
            let col: usize = parts[1]
                .parse()
                .map_err(|_| IoError::InvalidData("Invalid col in edge".to_string()))?;
            edges.push((row, col));
        }

        Ok(SpBitMatrixDual::from_coo_deduplicated(rows, cols, &edges))
    }
}

#[cfg(test)]
mod tests {

    use crate::{SpBitMatrix, SpBitMatrixDual};

    fn create_test_sparse() -> SpBitMatrix {
        let edges = vec![(0, 1), (0, 3), (1, 2), (2, 0), (2, 3)];
        SpBitMatrix::from_coo_deduplicated(3, 4, &edges)
    }

    #[test]
    fn test_spbitmatrix_binary_roundtrip() {
        let original = create_test_sparse();
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = SpBitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_spbitmatrix_text_roundtrip() {
        let original = create_test_sparse();
        let mut buffer = Vec::new();

        original
            .write_to_with_format(&mut buffer, super::super::SerializationFormat::Text)
            .unwrap();
        let loaded = SpBitMatrix::read_from_with_format(
            &mut buffer.as_slice(),
            super::super::SerializationFormat::Text,
        )
        .unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_spbitmatrix_empty() {
        let original = SpBitMatrix::zeros(0, 0);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = SpBitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_spbitmatrix_identity() {
        let original = SpBitMatrix::identity(10);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = SpBitMatrix::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_spbitmatrix_file_io() {
        let original = create_test_sparse();
        let temp_file = std::env::temp_dir().join("test_sparse.gf2");

        original.save_to_file(&temp_file).unwrap();
        let loaded = SpBitMatrix::load_from_file(&temp_file).unwrap();

        assert_eq!(original, loaded);

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_spbitmatrixdual_binary_roundtrip() {
        let edges = vec![(0, 1), (0, 3), (1, 2), (2, 0), (2, 3)];
        let original = SpBitMatrixDual::from_coo_deduplicated(3, 4, &edges);
        let mut buffer = Vec::new();

        original.write_to(&mut buffer).unwrap();
        let loaded = SpBitMatrixDual::read_from(&mut buffer.as_slice()).unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_spbitmatrixdual_text_roundtrip() {
        let edges = vec![(0, 1), (0, 3), (1, 2), (2, 0), (2, 3)];
        let original = SpBitMatrixDual::from_coo_deduplicated(3, 4, &edges);
        let mut buffer = Vec::new();

        original
            .write_to_with_format(&mut buffer, super::super::SerializationFormat::Text)
            .unwrap();
        let loaded = SpBitMatrixDual::read_from_with_format(
            &mut buffer.as_slice(),
            super::super::SerializationFormat::Text,
        )
        .unwrap();

        assert_eq!(original, loaded);
    }

    #[test]
    fn test_invalid_type_tag() {
        let bv = crate::BitVec::from_bytes_le(&[0xFF]);
        let mut buffer = Vec::new();
        bv.write_to(&mut buffer).unwrap();

        let result = SpBitMatrix::read_from(&mut buffer.as_slice());
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_ratio_dvb_t2_simulation() {
        // Simulate DVB-T2 Normal LDPC matrix: 32400 x 64800 with ~194400 nonzeros
        let rows = 32400;
        let cols = 64800;
        let target_nnz = 194400;

        // Create edges (3 per column on average)
        let mut edges = Vec::new();
        for col in 0..cols {
            for i in 0..3 {
                let row = (col * 7 + i * 13) % rows;
                edges.push((row, col));
            }
        }
        edges.truncate(target_nnz);

        let sparse = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);

        // Serialize
        let mut sparse_buf = Vec::new();
        sparse.write_to(&mut sparse_buf).unwrap();

        // Dense would be: rows * ceil(cols/64) * 8 bytes
        let dense_size = rows * cols.div_ceil(64) * 8;
        let sparse_size = sparse_buf.len();
        let compression = dense_size as f64 / sparse_size as f64;

        // Verify excellent compression
        assert!(
            compression > 100.0,
            "Expected >100x compression, got {:.1}x",
            compression
        );
        assert_eq!(sparse.nnz(), target_nnz);
    }

    // Property-based tests
    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn roundtrip_spbitmatrix_binary(nnz in 0..50usize) {
                let rows = 10;
                let cols = 20;
                let edges: Vec<(usize, usize)> = (0..nnz)
                    .map(|i| (i % rows, (i * 7) % cols))
                    .collect();
                let original = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);
                let mut buffer = Vec::new();

                original.write_to(&mut buffer)?;
                let loaded = SpBitMatrix::read_from(&mut buffer.as_slice())?;

                prop_assert_eq!(original.rows(), loaded.rows());
                prop_assert_eq!(original.cols(), loaded.cols());
                prop_assert_eq!(original.nnz(), loaded.nnz());
                prop_assert_eq!(original, loaded);
            }

            #[test]
            fn roundtrip_spbitmatrix_text(nnz in 0..50usize) {
                let rows = 10;
                let cols = 20;
                let edges: Vec<(usize, usize)> = (0..nnz)
                    .map(|i| (i % rows, (i * 7) % cols))
                    .collect();
                let original = SpBitMatrix::from_coo_deduplicated(rows, cols, &edges);
                let mut buffer = Vec::new();

                original.write_to_with_format(&mut buffer, super::super::super::SerializationFormat::Text)?;
                let loaded = SpBitMatrix::read_from_with_format(&mut buffer.as_slice(), super::super::super::SerializationFormat::Text)?;

                prop_assert_eq!(original, loaded);
            }

            #[test]
            fn roundtrip_spbitmatrixdual_binary(nnz in 0..50usize) {
                let rows = 10;
                let cols = 20;
                let edges: Vec<(usize, usize)> = (0..nnz)
                    .map(|i| (i % rows, (i * 7) % cols))
                    .collect();
                let original = SpBitMatrixDual::from_coo_deduplicated(rows, cols, &edges);
                let mut buffer = Vec::new();

                original.write_to(&mut buffer)?;
                let loaded = SpBitMatrixDual::read_from(&mut buffer.as_slice())?;

                prop_assert_eq!(original.rows(), loaded.rows());
                prop_assert_eq!(original.cols(), loaded.cols());
                prop_assert_eq!(original.nnz(), loaded.nnz());
                prop_assert_eq!(original, loaded);
            }
        }
    }
}
