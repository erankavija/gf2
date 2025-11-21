# DVB-T2 BCH Implementation Plan

**Status**: Ready to implement  
**Priority**: HIGH  
**Estimated Effort**: 1-2 weeks  
**Dependencies**: ✅ All met (gf2-core GF(2^m) complete, basic BCH implementation exists)

## Overview

Organize DVB-T2-specific BCH codes into a dedicated submodule following the same pattern as `ldpc::dvb_t2`. This provides clean separation of standard-specific code, proper factory methods, and comprehensive parameter tables.

## Current State

**Existing BCH Implementation** (`src/bch.rs`):
- ✅ Core BCH encoding/decoding with Berlekamp-Massey and Chien search
- ✅ GF(2^m) algebraic operations via `gf2-core`
- ✅ Basic DVB-T2 factory methods: `BchCode::dvb_t2_normal()` and `dvb_t2_long()`
- ✅ Parameter tables embedded in methods
- ⚠️ No dedicated module structure
- ⚠️ Mixed general BCH and DVB-T2-specific code

**What LDPC DVB-T2 Module Provides** (as reference pattern):
```
src/ldpc/dvb_t2/
├── mod.rs                  // Module documentation and public API
├── params.rs               // DvbParams struct with all frame/rate combinations
├── builder.rs              // Matrix construction logic
└── dvb_t2_matrices.rs      // Standard tables with #[rustfmt::skip]
```

## Proposed Structure

### New Module Organization

```
src/bch/
├── mod.rs                  // General BCH code (existing bch.rs content)
└── dvb_t2/
    ├── mod.rs              // DVB-T2 BCH documentation and factory methods
    └── params.rs           // Complete DVB-T2 BCH parameter tables
```

### File Purposes

#### `src/bch/mod.rs`
- General BCH code implementation (current `bch.rs` renamed)
- Core encoder/decoder with algebraic algorithms
- Generic `BchCode::new()` constructor
- **Remove** DVB-T2-specific factory methods (move to submodule)

#### `src/bch/dvb_t2/mod.rs`
- Module-level documentation on DVB-T2 BCH codes
- Factory methods for standard configurations
- Public re-exports

#### `src/bch/dvb_t2/params.rs`
- `DvbBchParams` struct with (n, k, t, field_params)
- Complete parameter tables from ETSI EN 302 755
- `FrameSize` enum (Normal/Short)
- Factory function: `DvbBchParams::for_code(frame_size, rate)`

## Detailed Design

### 1. DVB-T2 BCH Parameters

From **ETSI EN 302 755 Tables 6a and 6b**:

#### Normal Frame (Short LDPC: n_ldpc = 16200)
| Rate | Kbch  | n_bch | t  | Field      | Primitive Poly                |
|------|-------|-------|----|------------|-------------------------------|
| 1/2  | 7032  | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |
| 3/5  | 9552  | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |
| 2/3  | 10632 | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |
| 3/4  | 11712 | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |
| 4/5  | 12432 | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |
| 5/6  | 13152 | 16200 | 12 | GF(2^14)   | x^14 + x^5 + 1                |

#### Long Frame (Normal LDPC: n_ldpc = 64800)
| Rate | Kbch  | n_bch | t  | Field      | Primitive Poly                |
|------|-------|-------|----|------------|-------------------------------|
| 1/2  | 32208 | 64800 | 12 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |
| 3/5  | 38688 | 64800 | 12 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |
| 2/3  | 43040 | 64800 | 10 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |
| 3/4  | 48408 | 64800 | 12 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |
| 4/5  | 51648 | 64800 | 12 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |
| 5/6  | 53840 | 64800 | 10 | GF(2^16)   | x^16 + x^5 + x^3 + x^2 + 1    |

**Key observations**:
- Normal frames use GF(2^14), Long frames use GF(2^16)
- Most codes use t=12, but some long frame codes use t=10
- BCH codeword length matches LDPC information length (BCH is outer code)
- n_bch values listed above should be verified against standard

### 2. Code Structure

#### `src/bch/dvb_t2/params.rs`

```rust
//! DVB-T2 BCH code parameters from ETSI EN 302 755.

use crate::bch::CodeRate;

/// DVB-T2 frame size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSize {
    /// Normal frame (short LDPC): n = 16200
    Normal,
    /// Long frame (normal LDPC): n = 64800  
    Long,
}

/// DVB-T2 BCH code parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DvbBchParams {
    /// BCH codeword length (matches LDPC k)
    pub n: usize,
    /// BCH information bits
    pub k: usize,
    /// BCH parity bits
    pub m: usize,
    /// Error correction capability
    pub t: usize,
    /// Extension field degree
    pub field_m: usize,
    /// Primitive polynomial (binary representation)
    pub primitive_poly: u32,
}

impl DvbBchParams {
    /// Get DVB-T2 BCH parameters for a given configuration.
    ///
    /// # Arguments
    ///
    /// * `frame_size` - Normal (16200) or Long (64800) frame
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::dvb_t2::{DvbBchParams, FrameSize};
    /// use gf2_coding::CodeRate;
    ///
    /// let params = DvbBchParams::for_code(FrameSize::Long, CodeRate::Rate1_2);
    /// assert_eq!(params.n, 64800);
    /// assert_eq!(params.k, 32208);
    /// assert_eq!(params.t, 12);
    /// ```
    pub fn for_code(frame_size: FrameSize, rate: CodeRate) -> Self {
        let (n, k, t, field_m, primitive_poly) = match (frame_size, rate) {
            // Normal frames: GF(2^14), t=12 for all rates
            (FrameSize::Normal, CodeRate::Rate1_2) => (16200, 7032, 12, 14, 0b100000000100001),
            (FrameSize::Normal, CodeRate::Rate3_5) => (16200, 9552, 12, 14, 0b100000000100001),
            (FrameSize::Normal, CodeRate::Rate2_3) => (16200, 10632, 12, 14, 0b100000000100001),
            (FrameSize::Normal, CodeRate::Rate3_4) => (16200, 11712, 12, 14, 0b100000000100001),
            (FrameSize::Normal, CodeRate::Rate4_5) => (16200, 12432, 12, 14, 0b100000000100001),
            (FrameSize::Normal, CodeRate::Rate5_6) => (16200, 13152, 12, 14, 0b100000000100001),
            
            // Long frames: GF(2^16), mostly t=12, some t=10
            (FrameSize::Long, CodeRate::Rate1_2) => (64800, 32208, 12, 16, 0b10000000000101101),
            (FrameSize::Long, CodeRate::Rate3_5) => (64800, 38688, 12, 16, 0b10000000000101101),
            (FrameSize::Long, CodeRate::Rate2_3) => (64800, 43040, 10, 16, 0b10000000000101101),
            (FrameSize::Long, CodeRate::Rate3_4) => (64800, 48408, 12, 16, 0b10000000000101101),
            (FrameSize::Long, CodeRate::Rate4_5) => (64800, 51648, 12, 16, 0b10000000000101101),
            (FrameSize::Long, CodeRate::Rate5_6) => (64800, 53840, 10, 16, 0b10000000000101101),
        };

        let m = n - k;

        Self {
            n,
            k,
            m,
            t,
            field_m,
            primitive_poly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_frame_rate_1_2() {
        let params = DvbBchParams::for_code(FrameSize::Normal, CodeRate::Rate1_2);
        assert_eq!(params.n, 16200);
        assert_eq!(params.k, 7032);
        assert_eq!(params.t, 12);
        assert_eq!(params.field_m, 14);
        assert_eq!(params.n, params.k + params.m);
    }

    #[test]
    fn test_long_frame_rate_2_3() {
        let params = DvbBchParams::for_code(FrameSize::Long, CodeRate::Rate2_3);
        assert_eq!(params.n, 64800);
        assert_eq!(params.k, 43040);
        assert_eq!(params.t, 10); // t=10 for this rate
        assert_eq!(params.field_m, 16);
    }

    // Test all 12 configurations exist
    #[test]
    fn test_all_configurations() {
        for &frame_size in &[FrameSize::Normal, FrameSize::Long] {
            for &rate in &[
                CodeRate::Rate1_2,
                CodeRate::Rate3_5,
                CodeRate::Rate2_3,
                CodeRate::Rate3_4,
                CodeRate::Rate4_5,
                CodeRate::Rate5_6,
            ] {
                let params = DvbBchParams::for_code(frame_size, rate);
                assert_eq!(params.n, params.k + params.m);
                assert!(params.t > 0);
            }
        }
    }
}
```

#### `src/bch/dvb_t2/mod.rs`

```rust
//! DVB-T2 BCH outer codes.
//!
//! This module provides factory methods for DVB-T2 BCH codes as specified
//! in ETSI EN 302 755 standard.
//!
//! DVB-T2 uses BCH codes as outer codes before LDPC encoding to reduce
//! the error floor of the concatenated system.
//!
//! # Frame Types
//!
//! - **Normal frames**: n=16200, GF(2^14), t=12 errors
//! - **Long frames**: n=64800, GF(2^16), t=10 or 12 errors
//!
//! # Code Rates
//!
//! Both frame types support 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
//!
//! # Usage
//!
//! ```
//! use gf2_coding::bch::BchCode;
//! use gf2_coding::bch::dvb_t2::FrameSize;
//! use gf2_coding::CodeRate;
//!
//! // Create DVB-T2 BCH code for long frame, rate 1/2
//! let code = BchCode::dvb_t2(FrameSize::Long, CodeRate::Rate1_2);
//! assert_eq!(code.n(), 64800);
//! assert_eq!(code.k(), 32208);
//! assert_eq!(code.t(), 12);
//! ```
//!
//! # Concatenation with LDPC
//!
//! BCH output (n_bch bits) becomes LDPC input (k_ldpc bits):
//!
//! ```text
//! Data (Kbch) → BCH Encode → LDPC Encode → Codeword
//!              (n_bch=k_ldpc)  (n_ldpc)
//! ```
//!
//! Example for Long Frame Rate 1/2:
//! ```text
//! 32208 bits → BCH(64800,32208,t=12) → 64800 bits
//!           → LDPC(64800,32400) → 64800 bits
//! ```

pub mod params;

pub use params::{DvbBchParams, FrameSize};

use super::BchCode;
use crate::CodeRate;
use gf2_core::gf2m::Gf2mField;

impl BchCode {
    /// Creates DVB-T2 BCH code for specified frame size and rate.
    ///
    /// # Arguments
    ///
    /// * `frame_size` - Normal (16200) or Long (64800) frame
    /// * `rate` - Code rate (1/2, 3/5, 2/3, 3/4, 4/5, 5/6)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::BchCode;
    /// use gf2_coding::bch::dvb_t2::FrameSize;
    /// use gf2_coding::CodeRate;
    ///
    /// let code = BchCode::dvb_t2(FrameSize::Long, CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 64800);
    /// ```
    pub fn dvb_t2(frame_size: FrameSize, rate: CodeRate) -> Self {
        let params = DvbBchParams::for_code(frame_size, rate);
        let field = Gf2mField::new(params.field_m, params.primitive_poly);
        
        Self::new(params.n, params.k, params.t, field.with_tables())
    }
}
```

#### Update `src/bch/mod.rs`

```rust
// At the top of the file, add:
pub mod dvb_t2;

// Remove or deprecate the old dvb_t2_normal() and dvb_t2_long() methods
// (keep them temporarily with #[deprecated] for backwards compatibility)
```

## Implementation Steps

### Step 1: Verify Parameters (1-2 days)
- [ ] Cross-reference ETSI EN 302 755 Tables 6a and 6b
- [ ] Verify Kbch values match LDPC k values
- [ ] Verify n_bch values (currently listed as 16200/64800)
- [ ] Confirm t values for all rate/frame combinations
- [ ] Document any discrepancies

### Step 2: Create Module Structure (1 day)
- [ ] Create `src/bch/dvb_t2/` directory
- [ ] Rename `src/bch.rs` → `src/bch/mod.rs`
- [ ] Create `src/bch/dvb_t2/mod.rs`
- [ ] Create `src/bch/dvb_t2/params.rs`
- [ ] Update module hierarchy in `src/lib.rs`

### Step 3: Implement Parameters Module (1 day)
- [ ] Implement `DvbBchParams` struct
- [ ] Implement `FrameSize` enum
- [ ] Implement `for_code()` factory function
- [ ] Add comprehensive parameter tests (all 12 configs)
- [ ] Add invariant tests (n = k + m)

### Step 4: Implement Factory Methods (1 day)
- [ ] Implement `BchCode::dvb_t2()` in dvb_t2/mod.rs
- [ ] Add module documentation with usage examples
- [ ] Add concatenation examples showing BCH → LDPC flow
- [ ] Deprecate old `dvb_t2_normal()` and `dvb_t2_long()` methods

### Step 5: Integration Tests (1-2 days)
- [ ] Test all 12 DVB-T2 BCH configurations
- [ ] Encode/decode roundtrip tests (no errors)
- [ ] Error correction tests (inject t errors)
- [ ] Performance tests for large frames
- [ ] Known answer tests (if available from standard)

### Step 6: Documentation and Examples (1 day)
- [ ] Update main BCH module documentation
- [ ] Add DVB-T2 BCH example to `examples/`
- [ ] Update `README.md` with DVB-T2 BCH usage
- [ ] Document BCH-LDPC concatenation pattern

### Step 7: Alignment with LDPC Module (1 day)
- [ ] Ensure `CodeRate` enum is shared between BCH and LDPC
- [ ] Verify frame size naming consistency
- [ ] Cross-check parameter values (BCH n_bch = LDPC k_ldpc)
- [ ] Add integration test showing BCH → LDPC chain

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameters_match_ldpc() {
        // BCH output length should match LDPC input length
        let bch = DvbBchParams::for_code(FrameSize::Long, CodeRate::Rate1_2);
        let ldpc = crate::ldpc::dvb_t2::DvbParams::for_code(
            crate::ldpc::dvb_t2::FrameSize::Normal, 
            CodeRate::Rate1_2
        );
        assert_eq!(bch.n, ldpc.k);
    }

    #[test]
    fn test_all_configurations_encodable() {
        for &frame_size in &[FrameSize::Normal, FrameSize::Long] {
            for &rate in &[/*all rates*/] {
                let code = BchCode::dvb_t2(frame_size, rate);
                let encoder = BchEncoder::new(code.clone());
                
                // Test encoding succeeds
                let msg = BitVec::zeros(code.k());
                let cw = encoder.encode(&msg);
                assert_eq!(cw.len(), code.n());
            }
        }
    }
}
```

### Integration Tests
```rust
// tests/dvb_t2_bch_tests.rs
#[test]
fn test_bch_ldpc_concatenation() {
    let rate = CodeRate::Rate1_2;
    
    // Create BCH and LDPC codes
    let bch = BchCode::dvb_t2(FrameSize::Long, rate);
    let ldpc = LdpcCode::dvb_t2_normal(rate);
    
    // Verify concatenation compatibility
    assert_eq!(bch.n(), ldpc.k(), "BCH output must match LDPC input");
}
```

## Success Criteria

- [ ] All 12 DVB-T2 BCH configurations work correctly
- [ ] Module structure mirrors `ldpc::dvb_t2` pattern
- [ ] Parameters match ETSI EN 302 755 standard
- [ ] BCH output length matches LDPC input length
- [ ] All tests pass (unit, integration, property-based)
- [ ] Documentation includes concatenation examples
- [ ] Zero clippy warnings
- [ ] Backwards compatibility maintained (deprecated methods work)

## Future Work

After this implementation:
1. Create concatenated encoder/decoder (`dvb_t2_fec.rs`)
2. Add bit interleaver between BCH and LDPC
3. Implement full DVB-T2 transmit/receive chain example
4. Add BER/FER simulation for concatenated system

## References

- ETSI EN 302 755 V1.4.1 (2015-07) - Tables 6a and 6b
- DVB-T2 standard specification section 5.3 (FEC structure)
- Current `docs/DVB_T2_DESIGN.md`
