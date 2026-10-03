//! Macros for conveniently constructing BitMatrix values for GF(2).

/// Internal helper: turn tokens into bool for GF(2).
#[doc(hidden)]
#[macro_export]
macro_rules! __gf2_bit {
    (0) => {
        false
    };
    (1) => {
        true
    };
    (false) => {
        false
    };
    (true) => {
        true
    };
}

/// Create a BitMatrix using nalgebra-like row/column literals.
///
/// Supports two forms:
/// - Rows without brackets (nalgebra-like):
///   gf2_core::bitmatrix![ 1, 0, 1; 0, 1, 0 ];
/// - Rows with brackets:
///   gf2_core::bitmatrix![ \[1,0,1\], \[0,1,0\] ];
#[macro_export]
macro_rules! bitmatrix {
    // Bracketed rows come first: theirs is the more specific pattern.
    ( $( [ $($val:tt),+ $(,)? ] ),+ $(,)? ) => {{
        let __rows: &[&[bool]] = &[
            $(
                &[
                    $( $crate::__gf2_bit!($val) ),*
                ]
            ),+
        ];
        let __nrows = __rows.len();
        let __ncols = if __nrows == 0 { 0 } else { __rows[0].len() };
        let mut __m = $crate::matrix::BitMatrix::zeros(__nrows, __ncols);
        for (r, row) in __rows.iter().enumerate() {
            assert_eq!(
                row.len(), __ncols,
                "bitmatrix!: row {} has length {}, expected {}",
                r, row.len(), __ncols
            );
            for (c, &b) in row.iter().enumerate() {
                if b { __m.set(r, c, true); }
            }
        }
        __m
    }};
    ( $( $($val:tt),+ );+ $(;)? ) => {{
        let __rows: &[&[bool]] = &[
            $(
                &[
                    $( $crate::__gf2_bit!($val) ),*
                ]
            ),+
        ];
        let __nrows = __rows.len();
        let __ncols = if __nrows == 0 { 0 } else { __rows[0].len() };
        let mut __m = $crate::matrix::BitMatrix::zeros(__nrows, __ncols);
        for (r, row) in __rows.iter().enumerate() {
            assert_eq!(
                row.len(), __ncols,
                "bitmatrix!: row {} has length {}, expected {}",
                r, row.len(), __ncols
            );
            for (c, &b) in row.iter().enumerate() {
                if b { __m.set(r, c, true); }
            }
        }
        __m
    }};
}

/// Create a BitMatrix from binary string rows, e.g. "1011".
///
/// All rows must have the same length. Any character other than '0' or '1' will panic.
#[macro_export]
macro_rules! bitmatrix_bin {
    ( $( $row:literal ),+ $(,)? ) => {{
        let __rows: &[&str] = &[$($row),+];
        let __nrows = __rows.len();
        let __ncols = if __nrows == 0 { 0 } else { __rows[0].len() };
        let mut __m = $crate::matrix::BitMatrix::zeros(__nrows, __ncols);
        for (r, s) in __rows.iter().enumerate() {
            assert_eq!(
                s.len(), __ncols,
                "bitmatrix_bin!: row {} has length {}, expected {}",
                r, s.len(), __ncols
            );
            for (c, ch) in s.chars().enumerate() {
                match ch {
                    '0' => {}
                    '1' => __m.set(r, c, true),
                    _ => panic!("bitmatrix_bin!: invalid character '{}' at row {}, col {}", ch, r, c),
                }
            }
        }
        __m
    }};
}
