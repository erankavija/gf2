# Systematic Encoding Convention

## Overview

All systematic codes in the `gf2-coding` crate follow a unified convention for bit ordering in codewords.

## Standard Format

**Systematic codewords use [message | parity] format:**

```
[b₀ b₁ ... bₖ₋₁ | bₖ bₖ₊₁ ... bₙ₋₁]
 ←─ message ───→   ←─── parity ────→
```

- **Bits 0..(k-1)**: Original message bits (unchanged)
- **Bits k..(n-1)**: Computed parity/check bits

## Affected Code Modules

### BCH Codes (`src/bch/`)
- **Encoding**: `BchEncoder::encode()` produces [message | parity] codewords
- **Decoding**: `BchDecoder::decode()` expects [message | parity] codewords
- **Generator Matrix**: `BchCode::generator_matrix()` produces systematic generators
- **DVB-T2**: Fully compliant with ETSI EN 302 755

### Linear Codes (`src/linear.rs`)
- **Encoding**: `LinearBlockCode::encode()` produces [message | parity] for systematic codes
- **Systematic positions**: Stored as `0..k` for systematic codes
- **Hamming codes**: Use systematic encoding by default

### LDPC Codes (`src/ldpc/`)
- LDPC codes may or may not be systematic depending on construction
- When systematic, follow [message | parity] convention

## Polynomial-to-Bit Mapping

For BCH codes, the relationship between polynomial coefficients and bit positions follows the **DVB-T2 convention**:

```
Polynomial: m(x) = m₀ + m₁x + m₂x² + ... + mₖ₋₁x^(k-1)
BitVec:     [mₖ₋₁, mₖ₋₂, ..., m₁, m₀]
            bit 0   bit 1      ...  bit k-1
```

**Key principle**: Bit position 0 corresponds to the **highest** polynomial coefficient.

This ensures:
1. Systematic property holds in bitvec representation
2. Compatibility with DVB-T2 and other standards
3. Natural left-to-right message ordering

## Validation

The convention is validated by:

1. **Unit tests**: `test_bch_systematic_encoding_preserves_message()` verifies message preservation
2. **DVB-T2 verification**: All 202 test blocks from ETSI standard match exactly
3. **Property tests**: Roundtrip encoding/decoding tests in all modules

## Migration Notes

Previous versions of this codebase used [parity | message] format for BCH codes. As of the systematic encoding fix (November 2025):

- ✅ All encoding and decoding updated to [message | parity]
- ✅ DVB-T2 verification tests pass (100% match)
- ✅ Generator matrix computation updated
- ✅ Syndrome computation updated for correct bit ordering

No action required for code using the public API - the change is internal.

## References

- ETSI EN 302 755: DVB-T2 Standard
- [DVB_T2.md](DVB_T2.md): DVB-T2 implementation and verification status
