#!/usr/bin/env python3
"""SageMath oracle for the predeclared BCH conformance corpus.

Reads the corpus emitted by the `bch_oracle_messages` example, rebuilds each
row inside SageMath on *gf2's* field presentations and *gf2's* primitive n-th
root of unity, and writes the generator polynomials, dimensions, defining sets
and codewords SageMath derives.

Run it as ``python3 sage_oracle.py <corpus.json> <output.json>`` with the
SageMath library on the interpreter's path. The ``sage`` launcher of SageMath
10.9 forwards no arguments to a script, so the interpreter is invoked directly
and the SageMath version is read at run time from the library itself.

Oracle identity
---------------
A version label does not name a build, so the fixture's ``oracle`` object
carries ``sage.version.banner``, ``sage.version.version``, the interpreter's
own version, and ``sys.executable``, the path of the interpreter that produced
the fixture. ``run.sh`` hashes that executable into the receipt.

Field transport
---------------
gf2 pins a code by the base field ``B``, the splitting field ``E``, and the
element ``alpha`` of exact multiplicative order ``n`` in ``E``. A different
alpha gives a different, equivalent code, so the oracle must be handed gf2's.

``B`` is reconstructed with gf2's own defining polynomial, so canonical
coordinates in ``B`` agree by construction. ``E`` is SageMath's own
``GF(p^dE)`` and is related to gf2's ``E`` by the isomorphism ``Phi`` that
extends SageMath's embedding ``FE = Hom(B, Fsplit)[0]`` and sends gf2's
splitting-field generator ``z`` to a root ``Z0`` of ``FE(mE)`` in ``Fsplit``.
``Phi`` is a field isomorphism because ``FE(mE)`` is irreducible over
``FE(B)`` and ``Z0`` is one of its roots.

``B`` carries gf2's own defining polynomial, so generator coefficients and
codeword symbols are written directly in gf2's coordinates. The isomorphism
data -- ``alpha_index``, ``base_generator_image`` and
``splitting_field_generator_image`` -- are indices in SageMath's splitting
field, which is its own default presentation.

Which root is chosen does not matter. Two choices differ by an element of
``Gal(Fsplit / FE(B))``, so the two transported roots differ by that
automorphism, the two root sets differ by it coefficient-wise, and the
generator polynomial -- whose coefficients lie in ``FE(B)``, which the
automorphism fixes pointwise -- is the same either way. The script still fixes
the choice as the root of least canonical index and records it.

Canonical index
---------------
A field element is written as the integer whose base-p digits are its
polynomial-basis coordinates, coordinate zero least significant. That is the
numbering the corpus uses for gf2's elements, so indices are comparable across
the two systems exactly when the presentations are.
"""

import json
import sys

import sage.version
from sage.all import GF, Hom, PolynomialRing

NATIVE_ENCODER = "sage.coding.cyclic_code.CyclicCodePolynomialEncoder"
SYSTEMATIC_RULE = "x^r*m(x) - (x^r*m(x) mod g) over Sage's own generator"


def element_index(value, characteristic, degree):
    """Returns the canonical index of a field element."""
    if degree == 1:
        return int(value)
    return sum(
        int(digit) * characteristic**position
        for position, digit in enumerate(value.polynomial().list())
    )


def element_from_index(field, index, characteristic, degree):
    """Returns the field element a canonical index names."""
    prime = GF(characteristic)
    if degree == 1:
        return field(int(index))
    remaining = int(index)
    coordinates = []
    for _ in range(degree):
        coordinates.append(prime(remaining % characteristic))
        remaining //= characteristic
    return field(coordinates)


def decode_symbols(text, count, base_order):
    """Decodes the corpus hex convention into `count` canonical indices."""
    raw = bytes.fromhex(text)
    if base_order == 2:
        return [(raw[i >> 3] >> (i & 7)) & 1 for i in range(count)]
    return list(raw[:count])


def encode_symbols(symbols, base_order):
    """Encodes canonical indices under the corpus hex convention."""
    if base_order == 2:
        packed = bytearray((len(symbols) + 7) // 8)
        for position, symbol in enumerate(symbols):
            if symbol:
                packed[position >> 3] |= 1 << (position & 7)
        return packed.hex()
    return bytes(symbols).hex()


def build_row(row):
    """Rebuilds one corpus row in SageMath and returns its oracle record."""
    from sage.all import codes

    characteristic = row["characteristic"]
    base_degree = row["base_degree"]
    ext_degree = row["ext_degree"]
    relative_degree = row["relative_degree"]
    base_order = row["base_order"]
    length = row["n"]

    prime_ring = PolynomialRing(GF(characteristic), "T")
    if base_degree == 1:
        base = GF(characteristic)
    else:
        base = GF(
            characteristic**base_degree,
            name="y",
            modulus=prime_ring(row["base_modulus"]),
        )

    split = base if relative_degree == 1 else GF(characteristic**ext_degree, name="z")
    embedding = Hom(base, split)[0]

    checks = {}
    if relative_degree == 1:
        transport = "identity on B, composed with Sage's Hom(B, B)[0]"
        root_image = None

        def transport_index(index):
            return embedding(
                element_from_index(base, index, characteristic, base_degree)
            )

    else:
        split_ring = PolynomialRing(split, "Z")
        modulus = split_ring(
            [
                embedding(
                    element_from_index(base, coefficient, characteristic, base_degree)
                )
                for coefficient in row["ext_modulus"]
            ]
        )
        roots = sorted(
            modulus.roots(multiplicities=False),
            key=lambda value: element_index(value, characteristic, ext_degree),
        )
        if not roots:
            raise SystemExit(f"{row['id']}: gf2's splitting-field modulus has no root")
        chosen = roots[0]
        checks["ext_modulus_vanishes"] = bool(modulus(chosen).is_zero())
        checks["ext_root_candidates"] = len(roots)
        root_image = element_index(chosen, characteristic, ext_degree)
        transport = (
            "Sage's Hom(B, Fsplit)[0] extended by z -> the least-index root of "
            "its image of gf2's splitting-field modulus"
        )

        def transport_index(index):
            remaining = int(index)
            accumulated = split.zero()
            power = split.one()
            for _ in range(relative_degree):
                accumulated += (
                    embedding(
                        element_from_index(
                            base, remaining % base_order, characteristic, base_degree
                        )
                    )
                    * power
                )
                remaining //= base_order
                power *= chosen
            return accumulated

    if base_degree > 1:
        checks["base_generator_image"] = element_index(
            embedding(base.gen()), characteristic, ext_degree
        )

    alpha = transport_index(row["alpha"])
    checks["alpha_order"] = int(alpha.multiplicative_order())
    if checks["alpha_order"] != length:
        raise SystemExit(f"{row['id']}: transported alpha has the wrong order")

    code = codes.BCHCode(
        base,
        length,
        row["designed_distance"],
        primitive_root=alpha,
        offset=row["first_root"],
    )
    generator = code.generator_polynomial()
    redundancy = generator.degree()
    dimension = length - redundancy

    polynomial_ring = generator.parent()
    indeterminate = polynomial_ring.gen()
    native = codes.encoders.CyclicCodePolynomialEncoder(code)
    message_ring = native.message_space()

    native_words = []
    systematic_words = []
    for text in row["messages"]:
        symbols = decode_symbols(text, row["k"], base_order)
        message = message_ring(
            [
                element_from_index(base, symbol, characteristic, base_degree)
                for symbol in symbols
            ]
        )
        codeword = native.encode(message)
        native_words.append(
            encode_symbols(
                [
                    element_index(symbol, characteristic, base_degree)
                    for symbol in codeword
                ],
                base_order,
            )
        )
        shifted = indeterminate**redundancy * polynomial_ring(message)
        systematic = shifted - (shifted % generator)
        coefficients = systematic.list() + [base.zero()] * (
            length - len(systematic.list())
        )
        systematic_words.append(
            encode_symbols(
                [
                    element_index(symbol, characteristic, base_degree)
                    for symbol in coefficients
                ],
                base_order,
            )
        )

    split_modulus = [int(c) for c in split.modulus().list()]
    if base_degree == 1 and relative_degree == ext_degree:
        gf2_absolute = row["ext_modulus"]
    elif relative_degree == 1:
        gf2_absolute = row["base_modulus"]
    else:
        gf2_absolute = None

    return {
        "id": row["id"],
        "construction": (
            "codes.BCHCode(base_field, n, delta, primitive_root=alpha, "
            f"offset={row['first_root']})"
        ),
        "base_modulus": [int(c) for c in base.modulus().list()]
        if base_degree > 1
        else [],
        "base_modulus_matches_gf2": base_degree == 1
        or [int(c) for c in base.modulus().list()] == row["base_modulus"],
        "splitting_field_modulus": split_modulus,
        "splitting_field_modulus_comparable": gf2_absolute is not None,
        "splitting_field_matches_gf2": gf2_absolute == split_modulus,
        "transport": transport,
        "transport_checks": checks,
        "splitting_field_generator_image": root_image,
        "alpha_index": element_index(alpha, characteristic, ext_degree),
        "k": int(dimension),
        "defining_set": sorted(int(exponent) for exponent in code.defining_set()),
        "generator": [
            element_index(coefficient, characteristic, base_degree)
            for coefficient in generator.list()
        ],
        "native_encoder": NATIVE_ENCODER,
        "systematic_rule": SYSTEMATIC_RULE,
        "codewords_native": native_words,
        "codewords_systematic": systematic_words,
    }


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: python3 sage_oracle.py <corpus.json> <output.json>")
    corpus = json.load(open(sys.argv[1], encoding="utf-8"))
    fixture = {
        "oracle": {
            "system": "SageMath",
            "version": sage.version.banner,
            "library_version": sage.version.version,
            "interpreter": sys.version.split()[0],
            "interpreter_executable": sys.executable,
            "entry_point": "sage.coding.bch_code.BCHCode",
        },
        "seed": corpus["seed"],
        "rows": [build_row(row) for row in corpus["rows"]],
    }
    with open(sys.argv[2], "w", encoding="utf-8") as handle:
        json.dump(fixture, handle, indent=2, sort_keys=False)
        handle.write("\n")


if __name__ == "__main__":
    main()
