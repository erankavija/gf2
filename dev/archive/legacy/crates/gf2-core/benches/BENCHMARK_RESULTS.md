# Montgomery vs Naive Benchmark Results

Measured on commit 179be78, AMD Ryzen / Intel x86-64.
Naive baselines use `black_box(p)` to prevent compiler constant-division optimization.

## Fp<2^61 - 1> (Mersenne-61)

| Operation    | Montgomery (ns) | Naive `%` (ns) | Speedup |
|--------------|-----------------|----------------|---------|
| mul          | 1.03            | 1.86           | 1.8x    |
| add          | 0.62            | 1.66           | 2.7x    |
| sub          | 0.62            | 1.85           | 3.0x    |
| inv          | 159             | 411            | 2.6x    |
| mul_chain_100| 256             | 356            | 1.4x    |

## Acceptance vs Criteria

| Criterion             | Target | Measured | Status |
|-----------------------|--------|----------|--------|
| mul speedup           | >= 1.5x | 1.8x    | PASS   |
| add speedup           | >= 2x   | 2.7x    | PASS   |
| sub speedup           | >= 2x   | 3.0x    | PASS   |
| inv speedup           | >= 2x   | 2.6x    | PASS   |

## Reproduce

```bash
cargo bench -p gf2-core --bench fp_montgomery -- "mersenne61"
```
