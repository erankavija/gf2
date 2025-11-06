#[test]
fn build_with_bitmatrix_macro() {
    let m = gf2_core::bitmatrix![
        1, 0, 1, 1;
        0, 1, 0, 1;
    ];
    assert_eq!(m.rows(), 2);
    assert_eq!(m.cols(), 4);
    assert!(m.get(0, 0));
    assert!(m.get(0, 2));
    assert!(m.get(0, 3));
    assert!(m.get(1, 1));
    assert!(m.get(1, 3));
    assert!(!m.get(1, 0));
}

#[test]
fn build_with_bracketed_rows() {
    let m = gf2_core::bitmatrix![[1, 0, 0], [0, 1, 1],];
    assert_eq!(m.rows(), 2);
    assert_eq!(m.cols(), 3);
    assert!(m.get(1, 2));
    assert!(!m.get(0, 1));
}

#[test]
fn build_with_bitmatrix_bin_macro() {
    let m = gf2_core::bitmatrix_bin!["1011", "0101", "0000",];
    assert_eq!(m.rows(), 3);
    assert_eq!(m.cols(), 4);
    assert!(m.get(0, 0));
    assert!(m.get(0, 3));
    assert!(m.get(1, 1));
    assert!(m.get(1, 3));
    assert!(!m.get(1, 0));
    assert!(!m.get(1, 2));
    assert!(!m.get(2, 1));
}

#[test]
#[should_panic]
fn mismatched_row_lengths_panic() {
    let _ = gf2_core::bitmatrix![
        1, 0, 1;
        0, 1;
    ];
}

#[test]
#[should_panic]
fn invalid_char_in_bin_panic() {
    let _ = gf2_core::bitmatrix_bin!["10x1"]; // invalid 'x'
}
