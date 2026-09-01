# Systematic Encoding Convention

## Overview

Systematic codes in the `gf2-coding` crate present codewords in
`[message | parity]` form. For the canonical BCH surface the internal
polynomial convention and the user-facing layout are two separate, explicitly
connected contracts; for the other code families the convention is the direct
`[message | parity]` bit layout described below.

## Standard Format

**Systematic codewords use [message | parity] format:**

```
[s₀ s₁ ... sₖ₋₁ | sₖ sₖ₊₁ ... sₙ₋₁]
 ←─ message ───→   ←─── parity ────→
```

- **Positions 0..(k-1)**: Original message symbols (unchanged)
- **Positions k..(n-1)**: Computed parity/check symbols

The systematic positions are `0..k` under every declared layout; layouts
differ only in how user positions map to internal polynomial coordinates.

## Canonical BCH surface (`bch::spec` + `bch::encode`)

### Internal convention

Coordinate `i` is the coefficient of `x^i`. Systematic encoding computes
`parity = -(x^(n-k) · m(x) mod g(x))` and forms `c(x) = x^(n-k) · m(x) + parity`,
so `g(x) | c(x)` and the message occupies the high-degree coefficients
internally.

### User layouts

The user layout is a separate, explicit mapping consumed from the shared
layout contract (`bch::encode::SystematicLayout`):

- `MessageParityAscending` (default): user position `u` maps to internal
  coordinate `(u + n - k) mod n`; the inverse maps internal `i` to
  `(i + k) mod n`.
- `MessageParityDescending` (declared alternative): user position `u` maps to
  internal coordinate `n - 1 - u`; the map is self-inverse. This is the
  DVB-T2 transmission order.

Both layouts are `[message | parity]`; the descending order is a declared
layout, not what makes the code systematic.

### Representation and entry points

The surface is generic over the code symbol field and representation: packed
`BitVec` for binary codes and `FieldVec` for the field-generic path. Entry
points are `bch::spec::BchCode::{encode_systematic, encode_systematic_into,
systematic_message}` and the canonical `traits::block::BlockEncoder`
implementation.

### Errors

Misuse (buffer length, field identity, coordinate range) reports typed
`CodeError` values; panics are reserved for violated internal invariants. The
`traits::compat::binary_v1` boundary keeps its documented panic behavior.

## Legacy binary BCH surface (`bch::core`)

- **Encoding**: `BchEncoder::encode()` produces `[message | parity]`
  codewords with bit position 0 holding the highest polynomial coefficient
  (the DVB-T2 transmission order — the same convention the canonical surface
  expresses as `MessageParityDescending`).
- **Decoding**: `BchDecoder::decode()` expects that layout; the hardened
  `BinaryBchDecoder` consumes the canonical construction model's ascending
  coordinates.
- **Generator Matrix**: `BchCode::generator_matrix()` produces systematic
  generators.
- **DVB-T2**: compliant with ETSI EN 302 755.

## Linear Codes (`src/linear.rs`)

- **Encoding**: `LinearBlockCode::encode()` produces `[message | parity]` for
  systematic codes
- **Systematic positions**: Stored as `0..k` for systematic codes
- **Hamming codes**: Use systematic encoding by default

## LDPC Codes (`src/ldpc/`)

- LDPC codes may or may not be systematic depending on construction
- When systematic, follow `[message | parity]` convention

## Validation

The convention is validated structurally by the test suites beside each
surface: the `bch::encode` in-file property and agreement tests (divisibility,
message recovery, layout round-trips, legacy-encoder agreement), the DVB-T2
verification suite against the ETSI EN 302 755 reference blocks, and the
round-trip property tests in each code family's module.

## References

- ETSI EN 302 755: DVB-T2 Standard
- [DVB_T2.md](DVB_T2.md): DVB-T2 implementation and verification status
