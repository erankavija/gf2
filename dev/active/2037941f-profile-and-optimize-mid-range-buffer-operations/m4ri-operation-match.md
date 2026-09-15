# M4RI dense matvec operation match

> **Diátaxis Type:** Reference

This qualification defines the only M4RI arm that the mid-range dense-parity
family may place in a future frozen comparator cell. It is semantic evidence,
not a performance receipt and not a production-selection decision.

## Pinned external arm

The external implementation is M4RI [AlbrechtBard2026] release `20260122`, source archive
`m4ri-20260122.tar.gz`, SHA-256
`7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404`.
Its license is `GPL-2.0-or-later`: `m4ri/mzd.h` states GPL version 2 or
higher. The earlier external-comparator source evidence uses the same archive
and configure command in [`fetch-build.sh`](../6fb89a3c/survey/fetch-build.sh).
This qualification's runner repeats only that M4RI build, with the following
exact configure and identity-qualified installation path:

```text
CFLAGS='-O3 -march=native -fPIC' ./configure --prefix=<ext>/prefix-qualified-v1 --disable-static
make -j"${CARGO_BUILD_JOBS:-1}"
make install
```

The probe builds with `gcc -std=c11 -O3 -march=native -Wall -Wextra -Werror`
and links `<ext>/prefix-qualified-v1/lib/libm4ri.so` using its matching headers.
The runner always verifies the archive digest. A retained build is reused only
when its provenance record matches that archive, the current compiler identity,
the exact build and configure flags, and the installed library digest; an
incomplete or mismatched cache fails closed. The source
archive digest, installed library digest, compiler version and actual host
features belong in any later timed receipt; this qualification names no
measured arm or speed claim.

## Matched operation and mapping

The gf2 consumer is [`BitMatrix::matvec`](../../../crates/gf2-core/src/matrix.rs),
whose result is one parity bit per matrix row. For $A \in \mathrm{GF}(2)^{m\times n}$
and $x \in \mathrm{GF}(2)^n$, both arms compute

$$
y_r = \bigoplus_{c=0}^{n-1} A_{r,c}x_c.
$$

The M4RI call is `mzd_mul(y, A, x, 0)`, where `A` is an `m` by `n` `mzd_t`,
`x` is an `n` by one `mzd_t`, and preallocated `y` is an `m` by one `mzd_t`.
The fourth argument is the library's Strassen cutoff; zero is the exact value
used by the established in-tree M4RI matvec benchmark. This is ordinary dense
matrix multiplication with a one-column right operand, not transpose,
echelonization, BCH construction, or a raw logical kernel.

gf2 canonical bit $c$ is stored in logical word $\lfloor c / 64 \rfloor$ at
mask $1 \ll (c \bmod 64)$. The adapter writes that logical bit to M4RI public
coordinate `(row, c)` through `mzd_write_bit`; it reads M4RI `(row, 0)` into
gf2 output bit `row` through `mzd_read_bit`. Thus it does not compare physical
word layout or alignment. M4RI's public matrix contract permits padded physical
row stride and requires non-window excess bits to be zero, while gf2's output
has canonical zero tail padding. A future adapter must preserve both contracts
and must not expose M4RI storage as a gf2 word slice.

The fixed semantic cases are `1×1`, `63×63`, `64×64`, `65×65`, `65×512`, and
`65×4096`. The last two input widths are exactly 8 and 64 gf2 `u64` words;
the three adjacent cases cover the public tail boundary. Zero dimensions are
not an M4RI arm: this probe treats them as a gf2-only edge case because the
external build's allocation API is not used to define an empty-matrix contract.

## Whole-consumer cost boundary

The eventual comparator reports the following components separately and their
sum as the external whole-consumer cost:

1. gf2 input construction or retained input state;
2. `mzd_init` allocation for `A`, `x`, and `y` plus zero initialization;
3. bit-wise packing from canonical gf2 rows and vector into public M4RI
   coordinates;
4. `mzd_mul(y, A, x, 0)` execution with the same `A`, `x`, and `y` aliasing
   contract as the probe;
5. `mzd_read_bit` unpacking of the $m$ logical output bits to a gf2 `BitVec`;
6. disposal of every `mzd_t` when the consumer owns its converted state.

Amortizing prepared M4RI matrices is permitted only in a separately named
retained-state cell whose gf2 arm retains the analogous state. It cannot
replace the fresh whole-consumer cell. M4RI dispatch, table preparation and
output allocation remain inside the chosen external operation; any future
receipt records them as part of the arm rather than treating the product alone
as an equivalent raw parity kernel.

## Reproducible semantic check

Run the following from the repository root outside the benchmark window. It
builds dependencies and executes only deterministic correctness checks; it
does not collect timings.

```text
bash dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-m4ri-matvec-probe.sh
```

The probe seeds each case deterministically, builds canonical gf2-word input
buffers, writes the same logical coordinates to M4RI, runs `mzd_mul`, and
compares every M4RI output bit with the canonical parity calculation. A passing
run prints one `ok` line for every fixed shape. It supplies the semantic
precondition for a future comparator protocol; the protocol still freezes
cache state, sample policy, construction mode, and acceptance thresholds
before any timed campaign.
