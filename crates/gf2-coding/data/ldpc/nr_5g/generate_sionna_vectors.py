#!/usr/bin/env python3
"""Writes `sionna_ldpc5g_vectors.txt`: messages and the rate-matched codewords
Sionna's `LDPC5GEncoder` produces for them.

Run with the environment pinned in `PROVENANCE.md`:

    python -B generate_sionna_vectors.py           # rewrite the file
    python -B generate_sionna_vectors.py --check   # compare, exit 1 on a difference

The format is documented in `PROVENANCE.md`.
"""

import argparse
import hashlib
import sys
from importlib.metadata import version
from pathlib import Path

import torch
from sionna.phy.fec.ldpc.encoding import LDPC5GEncoder

OUTPUT = Path(__file__).resolve().with_name("sionna_ldpc5g_vectors.txt")

# (base graph, n, k): the rate-matched tuples of `tests/nr5g_external_vectors.rs`.
CONFIGURATIONS = [
    (1, 1584, 528),
    (1, 880, 440),
    (1, 924, 616),
    (1, 792, 660),
    (2, 600, 200),
    (2, 704, 352),
    (2, 540, 360),
]

# One full wave of the batch decoder and a final wave of one frame.
FRAMES = 9


def message_bits(bg, n, k, frame):
    """The first `k` bits of a SHA-256 counter stream keyed by the tuple and
    frame, each digest read most significant bit first."""
    bits = []
    counter = 0
    while len(bits) < k:
        seed = f"gf2/nr5g/sionna-vectors/bg{bg}/n{n}/k{k}/frame{frame}/{counter}"
        for byte in hashlib.sha256(seed.encode("ascii")).digest():
            bits.extend((byte >> shift) & 1 for shift in range(7, -1, -1))
        counter += 1
    return bits[:k]


def to_hex(bits):
    """Hex digit `j` holds bits `4j..4j+3`, the lowest index in the most
    significant position; the final digit is zero-padded."""
    padded = bits + [0] * (-len(bits) % 4)
    return "".join(
        f"{padded[i] << 3 | padded[i + 1] << 2 | padded[i + 2] << 1 | padded[i + 3]:x}"
        for i in range(0, len(padded), 4)
    )


def render():
    lines = [
        "# 5G NR LDPC messages and rate-matched codewords of Sionna's LDPC5GEncoder.",
        "# Format, licence and provenance: PROVENANCE.md beside this file.",
        f"# sionna-no-rt {version('sionna-no-rt')}",
    ]
    for bg, n, k in CONFIGURATIONS:
        # No output interleaver (`num_bits_per_symbol=None`) and no redundancy
        # version: the codeword is the start of the circular buffer.
        encoder = LDPC5GEncoder(k, n, num_bits_per_symbol=None, bg=f"bg{bg}", device="cpu")
        lines.append(
            f"config bg={bg} n={n} k={k} z={encoder.z} k_filler={encoder.k_filler}"
        )
        messages = [message_bits(bg, n, k, frame) for frame in range(FRAMES)]
        codewords = encoder(torch.tensor(messages, dtype=torch.float32))
        for message, codeword in zip(messages, codewords.to(torch.int64).tolist()):
            assert len(codeword) == n and set(codeword) <= {0, 1}
            lines.append(f"frame {to_hex(message)} {to_hex(codeword)}")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare instead of writing")
    args = parser.parse_args()
    text = render()
    if args.check:
        if OUTPUT.read_text(encoding="ascii") != text:
            print(f"{OUTPUT.name} differs from the regenerated vectors", file=sys.stderr)
            return 1
        return 0
    OUTPUT.write_text(text, encoding="ascii")
    return 0


if __name__ == "__main__":
    sys.exit(main())
