//! Thread safety tests for GF(2^m) fields and elements.
//!
//! Phase 15: GF(2^m) Thread Safety
//! These tests verify that Gf2mField and Gf2mElement can be safely shared across threads.

#[cfg(test)]
mod tests {
    use crate::gf2m::Gf2mField;

    /// Test that Gf2mField implements Send trait.
    ///
    /// Send allows transferring ownership of a value between threads.
    #[test]
    fn test_field_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Gf2mField>();
    }

    /// Test that Gf2mField implements Sync trait.
    ///
    /// Sync allows multiple threads to have immutable references to the same value.
    #[test]
    fn test_field_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Gf2mField>();
    }

    /// Test that Gf2mElement implements Send trait.
    #[test]
    fn test_element_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<crate::gf2m::Gf2mElement>();
    }

    /// Test that Gf2mElement implements Sync trait.
    #[test]
    fn test_element_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<crate::gf2m::Gf2mElement>();
    }

    /// Test cloning a field across thread boundaries.
    ///
    /// This verifies that Arc-based field sharing works correctly.
    #[test]
    fn test_field_clone_across_threads() {
        use std::thread;

        // Create field with tables (heavier object)
        let field = Gf2mField::new(8, 0b100011101).with_tables();

        // Clone field and send to 4 threads
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let field = field.clone();
                thread::spawn(move || {
                    // Use field in separate thread
                    let a = field.element(0x42);
                    let b = field.element(0x17);
                    let product = &a * &b;
                    product.value()
                })
            })
            .collect();

        // All threads should compute the same result
        let results: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify all results are identical
        for result in &results[1..] {
            assert_eq!(*result, results[0]);
        }
    }

    /// Test concurrent field operations from multiple threads.
    ///
    /// Verifies that shared field reference is safely accessible.
    #[test]
    fn test_concurrent_field_operations() {
        use std::sync::Arc;
        use std::thread;

        // Create field wrapped in Arc (explicit sharing)
        let field = Arc::new(Gf2mField::new(8, 0b100011101).with_tables());

        // Spawn multiple threads that all share the same field
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let field = Arc::clone(&field);
                thread::spawn(move || {
                    let a = field.element((i * 17) as u64);
                    let b = field.element((i * 23) as u64);
                    let sum = &a + &b;
                    let product = &a * &b;
                    (sum.value(), product.value())
                })
            })
            .collect();

        // All threads should complete successfully
        let results: Vec<(u64, u64)> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify we got results from all threads
        assert_eq!(results.len(), 8);
    }

    /// Test parallel element creation and arithmetic.
    #[test]
    fn test_parallel_element_arithmetic() {
        use std::sync::Arc;
        use std::thread;

        let field = Arc::new(Gf2mField::gf256());

        // Create 100 elements in parallel
        let handles: Vec<_> = (1..=100)
            .map(|i| {
                let field = Arc::clone(&field);
                thread::spawn(move || {
                    let a = field.element(i);
                    let b = field.element(i ^ 0xFF);
                    let sum = &a + &b;
                    let product = &a * &b;
                    (a.value(), sum.value(), product.value())
                })
            })
            .collect();

        let results: Vec<(u64, u64, u64)> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.len(), 100);

        // Verify XOR property: a + (a ^ 0xFF) = 0xFF
        for (i, (a_val, sum_val, _)) in results.iter().enumerate() {
            let expected_a = (i + 1) as u64;
            assert_eq!(*a_val, expected_a);
            assert_eq!(*sum_val, 0xFF);
        }
    }

    /// Test field with tables can be safely shared.
    ///
    /// Tables are large (~128KB for GF(2^16)), so Arc sharing is important.
    #[test]
    fn test_field_with_tables_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        // GF(2^16) with tables is ~128KB
        let field = Arc::new(Gf2mField::new(16, 0x1002D).with_tables());

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let field = Arc::clone(&field);
                thread::spawn(move || {
                    // Table-based multiplication
                    let a = field.element(0x1234);
                    let b = field.element(0x5678);
                    let product = &a * &b;
                    product.value()
                })
            })
            .collect();

        let results: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should get the same result
        for result in &results[1..] {
            assert_eq!(*result, results[0]);
        }
    }

    /// Test that cloned fields share the same underlying parameters.
    ///
    /// With Arc, clones should reference the same FieldParams (cheap clone).
    #[test]
    fn test_cloned_fields_share_params() {
        let field1 = Gf2mField::new(8, 0b100011101).with_tables();
        let field2 = field1.clone();

        // Both fields should be equal
        assert_eq!(field1, field2);

        // Elements from both should be compatible
        let a = field1.element(42);
        let b = field2.element(17);
        let sum = &a + &b;
        assert_eq!(sum.value(), 42 ^ 17);
    }

    /// Test cross-thread element operations with different fields.
    ///
    /// Verifies that elements maintain field identity across threads.
    #[test]
    fn test_cross_thread_different_fields() {
        use std::thread;

        // GF(2^8) with standard primitive: x^8 + x^4 + x^3 + x^2 + 1
        let field1 = Gf2mField::gf256();
        // GF(2^8) with different primitive: x^8 + x^4 + x^3 + x + 1
        let field2 = Gf2mField::new(8, 0b100011011);

        let handle1 = {
            let field = field1.clone();
            thread::spawn(move || {
                let a = field.element(100);
                let b = field.element(200);
                (&a * &b).value()
            })
        };

        let handle2 = {
            let field = field2.clone();
            thread::spawn(move || {
                let a = field.element(100);
                let b = field.element(200);
                (&a * &b).value()
            })
        };

        let result1 = handle1.join().unwrap();
        let result2 = handle2.join().unwrap();

        // Different primitive polynomials should give different results
        assert_ne!(result1, result2);
    }
}
