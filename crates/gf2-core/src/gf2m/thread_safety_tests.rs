#[cfg(test)]
mod tests {
    use crate::gf2m::Gf2mField;

    #[test]
    fn test_field_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Gf2mField>();
    }

    #[test]
    fn test_field_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Gf2mField>();
    }

    #[test]
    fn test_element_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<crate::gf2m::Gf2mElement>();
    }

    #[test]
    fn test_element_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<crate::gf2m::Gf2mElement>();
    }

    #[test]
    fn test_field_clone_across_threads() {
        use std::thread;

        let field = Gf2mField::new(8, 0b100011101).with_tables();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let field = field.clone();
                thread::spawn(move || {
                    let a = field.element(0x42);
                    let b = field.element(0x17);
                    let product = &a * &b;
                    product.value()
                })
            })
            .collect();

        let results: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        for result in &results[1..] {
            assert_eq!(*result, results[0]);
        }
    }

    #[test]
    fn test_concurrent_field_operations() {
        use std::sync::Arc;
        use std::thread;

        let field = Arc::new(Gf2mField::new(8, 0b100011101).with_tables());

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

        let results: Vec<(u64, u64)> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.len(), 8);
    }

    #[test]
    fn test_parallel_element_arithmetic() {
        use std::sync::Arc;
        use std::thread;

        let field = Arc::new(Gf2mField::gf256());

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

        for (i, (a_val, sum_val, _)) in results.iter().enumerate() {
            let expected_a = (i + 1) as u64;
            assert_eq!(*a_val, expected_a);
            assert_eq!(*sum_val, 0xFF);
        }
    }

    #[test]
    fn test_field_with_tables_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        let field = Arc::new(Gf2mField::new(16, 0x1002D).with_tables());

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let field = Arc::clone(&field);
                thread::spawn(move || {
                    let a = field.element(0x1234);
                    let b = field.element(0x5678);
                    let product = &a * &b;
                    product.value()
                })
            })
            .collect();

        let results: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        for result in &results[1..] {
            assert_eq!(*result, results[0]);
        }
    }

    #[test]
    fn test_cloned_fields_share_params() {
        let field1 = Gf2mField::new(8, 0b100011101).with_tables();
        let field2 = field1.clone();

        assert_eq!(field1, field2);

        let a = field1.element(42);
        let b = field2.element(17);
        let sum = &a + &b;
        assert_eq!(sum.value(), 42 ^ 17);
    }

    #[test]
    fn test_cross_thread_different_fields() {
        use std::thread;

        let field1 = Gf2mField::gf256();
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

        assert_ne!(result1, result2);
    }
}
