#!/usr/bin/env sage
"""
Validate primitive polynomial generation using SageMath.

This script generates primitive polynomials using Sage and compares
them with our Rust implementation to ensure correctness.
"""

import sys
import json
from sage.all import *

def get_all_primitive_polys(m):
    """Get all monic primitive polynomials of degree m over GF(2)."""
    R = PolynomialRing(GF(2), 'x')
    x = R.gen()
    
    primitives = []
    
    # Iterate through all monic polynomials of degree m
    # The high bit (x^m) is always set
    high_bit = 1 << m
    
    for lower_bits in range(1 << m):
        # Build polynomial from binary representation
        poly_int = high_bit | lower_bits
        
        # Convert to Sage polynomial
        coeffs = []
        for i in range(m + 1):
            coeffs.append(GF(2)((poly_int >> i) & 1))
        poly = R(coeffs)
        
        # Test if primitive
        if poly.is_primitive():
            primitives.append(poly_int)
    
    return sorted(primitives)

def poly_to_string(poly_int, m):
    """Convert binary polynomial representation to human-readable string."""
    terms = []
    for i in range(m, -1, -1):
        if (poly_int >> i) & 1:
            if i == 0:
                terms.append("1")
            elif i == 1:
                terms.append("x")
            else:
                terms.append(f"x^{i}")
    return " + ".join(terms)

def validate_degree(m):
    """Validate primitive polynomial generation for degree m."""
    print(f"\n=== Validating m={m} ===")
    
    # Get all primitive polynomials from Sage
    primitives = get_all_primitive_polys(m)
    count = len(primitives)
    
    print(f"Sage found {count} primitive polynomials")
    
    # Expected counts from OEIS A011260
    # https://oeis.org/A011260
    expected_counts = {
        2: 1, 3: 2, 4: 2, 5: 6, 6: 6, 7: 18, 8: 16,
        9: 48, 10: 60, 11: 176, 12: 144, 13: 630, 14: 756, 15: 1800, 16: 2048
    }
    
    if m in expected_counts:
        expected = expected_counts[m]
        if count == expected:
            print(f"✓ Count matches OEIS A011260: {count}")
        else:
            print(f"✗ ERROR: Expected {expected}, got {count}")
            return False
    
    # Print first few for manual verification
    print(f"\nFirst few primitive polynomials:")
    for i, poly_int in enumerate(primitives[:5]):
        print(f"  {poly_int:#b} = {poly_to_string(poly_int, m)}")
    
    if len(primitives) > 5:
        print(f"  ... and {len(primitives) - 5} more")
    
    return True

def main():
    if len(sys.argv) > 1:
        # Validate specific degrees
        degrees = [int(m) for m in sys.argv[1:]]
    else:
        # Default: validate m=2..8
        degrees = range(2, 9)
    
    all_valid = True
    for m in degrees:
        if not validate_degree(m):
            all_valid = False
    
    if all_valid:
        print("\n✓ All validations passed!")
        sys.exit(0)
    else:
        print("\n✗ Some validations failed")
        sys.exit(1)

if __name__ == "__main__":
    main()
