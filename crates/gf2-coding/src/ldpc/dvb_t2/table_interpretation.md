# DVB-T2 LDPC Table Format Interpretation

## Table Structure

ETSI DVB-T2 LDPC parity check matrix tables define connections between information
bits and parity bits in a block-structured format.

Each table row represents base parity indices for one 360-bit information block.

### Example: Table A.1 (Rate 1/2, N=64800)

**Row 0**: `54 9318 14392 27561 26909 10219 2534 8597`

This row corresponds to information bits 0-359. For information bit `j` in this block:

- **Bit 0** (j=0) connects to parity bits: 54, 9318, 14392, ...
- **Bit 1** (j=1) connects to parity bits: (54+90), (9318+90), (14392+90), ...
- **Bit j** connects to parity bits: (54+j×90) mod m, (9318+j×90) mod m, ...

Where:
- **90** = step size (q) for rate 1/2, N=64800
- **m = 32400** = number of parity bits
- **mod m** = addition performed modulo m

### Parameters by Configuration

| Frame  | Rate | n     | k     | m     | q  | Z   | Blocks |
|--------|------|-------|-------|-------|----|----|--------|
| Normal | 1/2  | 64800 | 32400 | 32400 | 90 | 360| 90     |
| Normal | 3/5  | 64800 | 38880 | 25920 | 96 | 360| 108    |
| Normal | 2/3  | 64800 | 43200 | 21600 | 60 | 360| 120    |
| Normal | 3/4  | 64800 | 48600 | 16200 | 45 | 360| 135    |
| Normal | 4/5  | 64800 | 51840 | 12960 | 36 | 360| 144    |
| Normal | 5/6  | 64800 | 54000 | 10800 | 30 | 360| 150    |

## Dual-Diagonal Parity Structure

DVB-T2 LDPC codes use a **dual-diagonal structure** for parity bit connections,
which enables efficient encoding. This structure is NOT specified in the tables
but is part of the standard design.

For each parity bit p (where p ∈ [0, m)):
- **Diagonal**: Parity bit p connects to check equation p
- **Sub-diagonal**: Parity bit p connects to check equation (p-1) mod m

The sub-diagonal wraps: parity bit 0 connects to check equation (m-1).

### Visual Representation

```
Parity portion of H matrix (m×m submatrix):
       p₀  p₁  p₂  ...  p_{m-1}
    ┌──────────────────────────┐
 c₀ │  1   0   0  ...   1      │  Diagonal + wrap
 c₁ │  1   1   0  ...   0      │
 c₂ │  0   1   1  ...   0      │
    │  ...                     │
c_{m-1}│  0   0   0  ...   1   │
    └──────────────────────────┘
```

This structure ensures:
- Each parity check involves exactly 2 parity bits (sparse)
- Enables triangular back-substitution for encoding
- Efficient systematic encoding algorithm

## Why DVB-T2 is Not Pure Quasi-Cyclic

Unlike pure QC-LDPC codes, DVB-T2 matrices:
- Have **multiple circulants per block position** (from table expansion)
- Use **variable column weights** (rows have different lengths)
- Combine **structured info-to-parity** (from tables) with **structured parity-to-parity** (dual-diagonal)

This requires direct sparse matrix construction rather than simple QC expansion.

## Implementation

See:
- `builder.rs`: Edge list construction algorithm
- `params.rs`: Configuration parameters
- `dvb_t2_matrices.rs`: Standard tables
