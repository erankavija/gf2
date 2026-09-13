//! Byte-oriented GF(2^8) multiplication tables (jit:19513245).
//!
//! Both tables are built by the shift-and-reduce recurrence over the field's
//! own reduction polynomial, so they depend on no gf2-core arithmetic and
//! serve the validation binary as an independent oracle as well as the
//! prototype as its kernel.
//!
//! The recurrence is the linearity of multiplication by `x`. With
//! `t[v] = a * v`, `t[0] = 0` and `t[1] = a`, an even index `2u` satisfies
//! `t[2u] = a * (x * u) = x * t[u]`, and an odd index `2u + 1` satisfies
//! `t[2u + 1] = t[2u] ^ a`. Building a coefficient table therefore costs one
//! doubling or one XOR per entry.

/// Multiplies a GF(2^8) element by `x` modulo `polynomial`.
///
/// `polynomial` is the full reduction polynomial with its degree-8 bit set,
/// for example `0x11D` for `x^8 + x^4 + x^3 + x^2 + 1` or `0x11B` for
/// `x^8 + x^4 + x^3 + x + 1`.
#[inline]
fn xtime(value: u8, polynomial: u16) -> u8 {
    let shifted = u16::from(value) << 1;
    let reduced = if value & 0x80 != 0 {
        shifted ^ polynomial
    } else {
        shifted
    };
    (reduced & 0xFF) as u8
}

/// Products of one fixed GF(2^8) coefficient with all 256 field elements.
///
/// This is the table a reused coefficient earns: preparing it costs 256
/// operations and every subsequent multiplication by that coefficient costs
/// one indexed load.
#[derive(Clone)]
pub struct CoefficientTable {
    entries: [u8; 256],
}

impl CoefficientTable {
    /// Builds the table of `coefficient` over the field of `polynomial`.
    pub fn new(coefficient: u8, polynomial: u16) -> Self {
        let mut entries = [0u8; 256];
        entries[1] = coefficient;
        for value in 2..256usize {
            entries[value] = if value % 2 == 0 {
                xtime(entries[value / 2], polynomial)
            } else {
                entries[value - 1] ^ coefficient
            };
        }
        CoefficientTable { entries }
    }

    /// Overwrites this table with the products of `coefficient`, so a
    /// consumer that changes coefficient allocates nothing.
    pub fn refill(&mut self, coefficient: u8, polynomial: u16) {
        self.entries[0] = 0;
        self.entries[1] = coefficient;
        for value in 2..256usize {
            self.entries[value] = if value % 2 == 0 {
                xtime(self.entries[value / 2], polynomial)
            } else {
                self.entries[value - 1] ^ coefficient
            };
        }
    }

    /// The product of the coefficient with `value`.
    #[inline]
    pub fn get(&self, value: u8) -> u8 {
        self.entries[usize::from(value)]
    }

    /// The table's entries, for the validation binary.
    pub fn entries(&self) -> &[u8; 256] {
        &self.entries
    }
}

impl Default for CoefficientTable {
    /// The zero coefficient over `0x11D`.
    fn default() -> Self {
        CoefficientTable::new(0, 0x11D)
    }
}

/// The full 256-by-256 GF(2^8) multiplication table, 64 KiB of bytes in row
/// order.
///
/// A workload whose coefficients do not repeat cannot amortise a coefficient
/// table, so its byte-oriented form is one lookup in this table. It is also
/// the table the dense product uses, because a product reuses every
/// coefficient of the left operand across a whole row of the right one and
/// so wants every coefficient's table resident at once.
pub struct ProductTable {
    rows: Vec<u8>,
    polynomial: u16,
}

impl ProductTable {
    /// Builds the full table over `polynomial`.
    pub fn new(polynomial: u16) -> Self {
        let mut rows = vec![0u8; 256 * 256];
        for coefficient in 0..256usize {
            let table = CoefficientTable::new(coefficient as u8, polynomial);
            rows[coefficient * 256..(coefficient + 1) * 256].copy_from_slice(table.entries());
        }
        ProductTable { rows, polynomial }
    }

    /// The reduction polynomial this table was built over.
    pub fn polynomial(&self) -> u16 {
        self.polynomial
    }

    /// The 256 products of `coefficient`.
    #[inline]
    pub fn row(&self, coefficient: u8) -> &[u8] {
        let start = usize::from(coefficient) * 256;
        &self.rows[start..start + 256]
    }

    /// The product `left * right`.
    #[inline]
    pub fn get(&self, left: u8, right: u8) -> u8 {
        self.rows[usize::from(left) * 256 + usize::from(right)]
    }

    /// The table's bytes, so a caller can report its size from the object
    /// rather than from a typed constant.
    pub fn bytes(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{xtime, CoefficientTable, ProductTable};

    /// Bit-by-bit GF(2^8) multiplication, written without the recurrence the
    /// tables use, so it tests them rather than restating them.
    fn reference(left: u8, right: u8, polynomial: u16) -> u8 {
        let mut result = 0u16;
        let mut shifted = u16::from(left);
        for bit in 0..8 {
            if (right >> bit) & 1 == 1 {
                result ^= shifted;
            }
            shifted <<= 1;
            if shifted & 0x100 != 0 {
                shifted ^= polynomial;
            }
        }
        (result & 0xFF) as u8
    }

    #[test]
    fn coefficient_tables_match_the_reference_for_every_pair() {
        for polynomial in [0x11Du16, 0x11B] {
            for coefficient in 0..=255u8 {
                let table = CoefficientTable::new(coefficient, polynomial);
                for value in 0..=255u8 {
                    assert_eq!(
                        table.get(value),
                        reference(coefficient, value, polynomial),
                        "coefficient {coefficient} value {value} polynomial {polynomial:#X}"
                    );
                }
            }
        }
    }

    #[test]
    fn refill_writes_what_a_fresh_table_holds() {
        let mut table = CoefficientTable::new(0xFF, 0x11D);
        for coefficient in [0u8, 1, 2, 0x53, 0xCA] {
            table.refill(coefficient, 0x11D);
            assert_eq!(
                table.entries(),
                CoefficientTable::new(coefficient, 0x11D).entries()
            );
        }
    }

    #[test]
    fn the_full_table_holds_every_coefficient_table() {
        for polynomial in [0x11Du16, 0x11B] {
            let full = ProductTable::new(polynomial);
            assert_eq!(full.bytes(), 256 * 256);
            for coefficient in 0..=255u8 {
                assert_eq!(
                    full.row(coefficient),
                    CoefficientTable::new(coefficient, polynomial).entries()
                );
                for value in 0..=255u8 {
                    assert_eq!(full.get(coefficient, value), full.row(coefficient)[usize::from(value)]);
                }
            }
        }
    }

    #[test]
    fn doubling_reduces_at_the_high_bit() {
        assert_eq!(xtime(0x80, 0x11D), 0x1D);
        assert_eq!(xtime(0x80, 0x11B), 0x1B);
        assert_eq!(xtime(0x01, 0x11D), 0x02);
    }
}
