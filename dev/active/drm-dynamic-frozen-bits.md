# Design: Implement proper dRM(32,21) with dynamic frozen bits

**Issue**: TBD
**Status**: Draft
**Date**: 2026-04-11

## Problem

Our dRM(32,21) implementation uses a monomial-degree-based construction
(first 21 monomials in degree-then-lexicographic order) with frozen bits
set to zero. This produces a code with d_min=4 and A4=152 weight-4
codewords.

The paper (Yuan et al., arxiv:2310.10737) uses a **dynamic Reed-Muller**
(dRM) code as defined by Coskun & Pfister (arxiv:2103.16680). In this
construction, frozen bits are set to **random linear combinations of
preceding information bits**, which eliminates low-weight codewords and
increases d_min.

Note: the Hamming sphere-packing bound proves d_min >= 8 is impossible
for any (32, 21) code (V(32,3) = 5489 > 2^11). The maximum achievable
is d_min = 6, which this implementation achieves via greedy extension
row selection (seed=3).

With d_min=6, the product code distance is d^2=36 vs our old d^2=16.
This improvement explains the dramatic waterfall slope change (46x/0.5dB
vs 2.3x/0.5dB) and the dRM product code now outperforming LDPC BP at
1.75+ dB Eb/N0.

## Background

### Reed-Muller codes and the polar transform

The RM(r,m) code of length N=2^m is defined by the rows of the polar
transform matrix G_N = B_N * G_2^{otimes m} (Kronecker product of the
2x2 Hadamard matrix, with bit-reversal). The information set A consists
of row indices whose binary representation has Hamming weight >= m-r.

For m=5:
- RM(2,5): A = {indices with weight >= 3}, |A|=16, d_min=8
- RM(3,5): A = {indices with weight >= 2}, |A|=26, d_min=4

For k=21: we need 5 positions from the weight-2 indices (degree-3
rows) added to RM(2,5)'s 16 positions. Without constraints on the
frozen bits, adding ANY degree-3 row introduces weight-4 codewords,
reducing d_min to 4.

### Dynamic frozen bits

Definition 1 from Coskun & Pfister: for each frozen position i in F,
set u_i = sum_{j in A^{i-1}} v_{j,i} * u_j, where v_{j,i} are random
coefficients in GF(2). This makes each frozen bit a linear combination
of preceding information bits.

These constraints act as extra parity equations. With 11 frozen positions
each having a random linear constraint, the probability of any specific
weight-4 codeword surviving is ~(1/2)^{11} ~ 1/2048. Since our code
has only 152 weight-4 codewords, essentially all are eliminated,
increasing d_min from 4 to 6 (the maximum achievable for (32,21)).

## Design

### Step 1: Compute the RM(2,5) information set

The polar transform for N=32 uses G_32 = B_32 * G_2^{otimes 5}. The
information set for RM(2,5) consists of 16 indices with binary weight
>= 3:

```
Weight 3: 7, 11, 13, 14, 19, 21, 22, 25, 26, 28  (C(5,3) = 10)
Weight 4: 15, 23, 27, 29, 30                       (C(5,4) = 5)
Weight 5: 31                                        (C(5,5) = 1)
```

Total: 16 positions (matches k=16 for RM(2,5)).

### Step 2: Select 5 additional positions for k=21

The weight-2 indices (degree-3 in RM) are:
```
3, 5, 6, 9, 10, 12, 17, 18, 20, 24  (C(5,2) = 10)
```

We need 5 of these. The selection should be based on reliability
ordering (Bhattacharyya parameters or density evolution for the AWGN
channel at a design SNR). Following the paper's approach, we select
the 5 most reliable weight-2 channels.

For N=32 BIAWGN at a design Eb/N0 near capacity (e.g., 0.5 dB for
rate 0.656), the Bhattacharyya parameters can be computed via density
evolution. The most reliable weight-2 channels are typically those
with higher indices (later in the polar transform).

### Step 3: Find extension rows by greedy search

For each candidate extension row (a random XOR of G_32 rows with a
given seed), verify that adding it to the existing generator preserves
d_min >= 6 by checking the weight of the new coset. Accept the row
only if d_min is maintained. Repeat until 5 rows are added.

The accepted rows are hardcoded as `DYNAMIC_DRM_32_21_ROWS` for
deterministic, O(1) construction at runtime.

### Step 4: Construct G and H matrices

1. Load the 21 precomputed generator row words
2. Build a 21 x 32 BitMatrix from them
3. Apply Gaussian elimination to get systematic form G = [I_k | P]
4. Derive H = [P^T | I_r]

### Step 5: Verify d_min ≥ 6

Enumerate all 2^21 codewords (Gray code) and verify A4 = A5 = 0.
If d_min < 6, try a different seed and regenerate extension rows.
Seed=3 was found to work.

### Step 6: Integration (DONE)

`DrmCode::drm_32_21()` now returns the (32, 21, 6) code.
`DrmCode::drm_32_21_dynamic()` is an alias for the same.
`sim_runner` routes `component = "drm_32_21"` to this code.

## Verification approach

1. **Unit test**: d_min >= 6 (enumerate all 2^21 codewords, verify A4=A5=0)
2. **Unit test**: G * H^T = 0 (orthogonality)
3. **Unit test**: encode-decode roundtrip with BCJR
4. **Simulation**: BCJR turbo at 1.0 dB should give BLER <= 0.15
   (within 2x of paper's 0.072)
5. **Simulation**: product code should outperform LDPC BP at some SNR point

## Risks

- The specific reliability ordering for selecting the 5 weight-2
  positions may differ from the paper. If so, the code will have
  different properties. Mitigation: try multiple orderings and verify
  d_min>=6 for each.
- Dynamic frozen bits change the encoder structure. The product code
  encoder must be updated to handle the constraints. However, for
  GRAND-based decoding, only H is needed (the encoder can still use
  standard systematic form derived from G).
- The paper may use a specific random seed for the frozen bit constraints.
  Without knowing the exact seed, our code will be a different instance
  from the same dRM ensemble. This should still give similar performance
  since the ensemble members have similar distance properties.

## References

- Yuan et al., "Soft-output (SO) GRAND and Iterative Decoding to
  Outperform LDPCs," arxiv:2310.10737, 2024.
- Coskun & Pfister, "An Information-Theoretic Perspective on Successive
  Cancellation List Decoding and Polar Code Design," arxiv:2103.16680, 2022.
- Galligan et al., "Block turbo decoding with ORBGRAND,"
  arxiv:2207.11149, 2022.
