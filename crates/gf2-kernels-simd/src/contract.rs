//! The host precondition of a kernel test: the implementations the host
//! provides, named in the output, at least one required.

/// Prints each available candidate and returns their labels.
///
/// # Panics
///
/// Panics when no candidate is available, so a host without the kernel's
/// instruction set fails the test instead of passing without comparing.
pub fn require<'a>(candidates: &[(&'a str, bool)]) -> Vec<&'a str> {
    let available: Vec<&str> = candidates
        .iter()
        .filter(|(_, present)| *present)
        .map(|(label, _)| *label)
        .collect();
    for label in &available {
        println!("implementation: {label}");
    }
    assert!(
        !available.is_empty(),
        "no implementation was enumerated: this host lacks {}",
        candidates
            .iter()
            .map(|(label, _)| *label)
            .collect::<Vec<_>>()
            .join(", ")
    );
    available
}

/// The bundle a `detect` function published, under `label`.
///
/// # Panics
///
/// Panics when `detected` is `None`.
pub fn require_detected<T>(label: &str, detected: Option<T>) -> T {
    require(&[(label, detected.is_some())]);
    detected.expect("require checked presence")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "no implementation was enumerated")]
    fn a_host_without_the_instruction_set_is_refused() {
        require(&[("avx2", false), ("avx512f", false)]);
    }

    #[test]
    fn available_candidates_are_returned_by_label() {
        assert_eq!(require(&[("avx2", true), ("avx512f", false)]), ["avx2"]);
    }
}
