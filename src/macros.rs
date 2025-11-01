//! Macros for conveniently constructing BitMatrix values for GF(2).
//!
//! Provided macros:
//! - `bitmatrix![ … ]`: nalgebra-like syntax where rows are separated by `;` and
//!   elements by `,`. Elements may be `0`, `1`, `false`, or `true`.
//!
//!   Example:
//!   ```
//!   let m = gf2::bitmatrix![
//!       1, 0, 1, 1;
//!       0, 1, 0, 1;
//!       1, 0, 0, 0;
//!   ];
//!
//!   assert_eq!(m.rows(), 3);
//!   assert_eq!(m.cols(), 4);
//!   assert!(m.get(0, 0));
//!   assert!(m.get(1, 3));
//!   assert!(!m.get(2, 3));
//!   ```
//!
//! - `bitmatrix_bin![ … ]`: rows as binary strings, e.g. "1011", handy for GF(2).
//!
//!   Example:
//!   ```
//!   let m = gf2::bitmatrix_bin![
//!       "1011",
//!       "0101",
//!   ];
//!   assert_eq!(m.rows(), 2);
//!   assert_eq!(m.cols(), 4);
//!   assert!(m.get(0, 0));
//!   assert!(m.get(0, 3));
//!   assert!(m.get(1, 1));
//!   assert!(m.get(1, 3));
//!   assert!(!m.get(1, 0));
//!   assert!(!m.get(1, 2));
//!   ```

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
///   gf2::bitmatrix![ 1, 0, 1; 0, 1, 0 ];
/// - Rows with brackets:
///   gf2::bitmatrix![ \[1,0,1\], \[0,1,0\] ];
#[macro_export]
macro_rules! bitmatrix {
    // Bracketed rows: gf2::bitmatrix![ [1,0,1], [0,1,0] ];
    // This must come first to match more specific pattern
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
        let mut __m = $crate::matrix::BitMatrix::new_zero(__nrows, __ncols);
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
    // nalgebra-like form: rows separated by `;`, elements by `,`
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
        let mut __m = $crate::matrix::BitMatrix::new_zero(__nrows, __ncols);
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
        let mut __m = $crate::matrix::BitMatrix::new_zero(__nrows, __ncols);
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
