//! Tests verifying BitVec is Send + Sync for parallel operations.

#[cfg(test)]
mod tests {
    use crate::BitVec;
    use std::sync::Arc;

    /// Test that BitVec is Send (can be moved across threads)
    #[test]
    fn test_bitvec_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BitVec>();
    }

    /// Test that BitVec is Sync (can be shared across threads via &T)
    #[test]
    fn test_bitvec_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BitVec>();
    }

    /// Test that BitVec can be shared across threads with Arc
    #[test]
    fn test_bitvec_arc_sharing() {
        let bv = Arc::new(BitVec::ones(100));
        let bv_clone = Arc::clone(&bv);
        
        let handle = std::thread::spawn(move || {
            assert_eq!(bv_clone.len(), 100);
            assert_eq!(bv_clone.count_ones(), 100);
        });
        
        handle.join().unwrap();
    }

    /// Test that rank operation works across threads
    #[test]
    fn test_rank_across_threads() {
        let bv = Arc::new(BitVec::from_bytes_le(&[0b10101010, 0b11110000]));
        
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let bv = Arc::clone(&bv);
                std::thread::spawn(move || {
                    // Multiple threads can call rank concurrently
                    let r = bv.rank(8);
                    assert_eq!(r, 4); // 4 ones in first byte
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test that select operation works across threads
    #[test]
    fn test_select_across_threads() {
        let bv = Arc::new(BitVec::from_bytes_le(&[0b11111111, 0b00000000]));
        
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let bv = Arc::clone(&bv);
                std::thread::spawn(move || {
                    let pos = bv.select(i).unwrap();
                    assert_eq!(pos, i); // First 8 ones are at positions 0-7
                })
            })
            .collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test concurrent rank/select queries (stress test for lock contention)
    #[test]
    #[cfg(feature = "parallel")]
    fn test_concurrent_rank_select_stress() {
        use rayon::prelude::*;
        
        let bv = Arc::new(BitVec::from_bytes_le(&vec![0xFF; 1000]));
        
        // 1000 concurrent rank queries
        let results: Vec<_> = (0..1000)
            .into_par_iter()
            .map(|i| {
                let idx = i * 8;
                if idx >= bv.len() {
                    return (i, 0); // Skip out of bounds
                }
                let r = bv.rank(idx);
                (i, r)
            })
            .collect();
        
        // Verify correctness
        for (i, r) in results {
            let idx = i * 8;
            if idx < bv.len() {
                assert_eq!(r, idx + 1, "rank({}) should be {}", idx, idx + 1); // All bits are 1, rank is inclusive
            }
        }
    }
}
