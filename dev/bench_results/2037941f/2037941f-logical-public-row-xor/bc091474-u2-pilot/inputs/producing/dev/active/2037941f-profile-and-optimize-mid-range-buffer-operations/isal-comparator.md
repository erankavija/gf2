# ISA-L XOR comparator qualification

> **Diátaxis Type:** Reference

This comparator specification qualifies ISA-L's XOR interface for the
mid-range logical-buffer protocol. It identifies a scalar base arm and keeps
the public SIMD-dispatched arm unavailable on this host; the two routes are
not interchangeable.

## Source and build identity

ISA-L [IsaL2026] is pinned to tag `v2.32.1`, commit
`7c3479e0a9dac17f448603ec1ad64c7c625f530c`, under BSD-3-Clause. The
committed probe verifies SHA-256 values `4fb636b16cebebadfb52236871421f1a143d3d7e488e7bc9b23b2fc25bd04aeb`
for `raid/raid_base.c`,
`5e51c4abcd86ade41426cef509c3eeb06b0c0b3f9ef28080ddedb6af02a5cd03`
for `include/raid.h`, and
`bc8fd4a3d031e65e05e9c9e2add2c3f336ce527fa85c1e31031c808b58216217`
for `LICENSE`.

The qualified scalar symbol is `xor_gen_base`. Its reproducible build and
semantic check are:

```bash
git clone --branch v2.32.1 https://github.com/intel/isa-l.git isa-l
git -C isa-l checkout 7c3479e0a9dac17f448603ec1ad64c7c625f530c
ISAL_SOURCE="$PWD/isa-l" bash \
  dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/run-isal-xor-probe.sh
```

The script compiles `raid/raid_base.c` and the probe with
the resolved GCC compiler with `-std=c11 -O3 -march=native -Wall -Wextra -Werror`, verifies the pinned
source and license bytes before compilation, records the resolved compiler
path and version, clears inherited GNU Make override channels, and retains
the verbose transcript and each object’s `.comment` producer. The
[`isal-arm-build-record.txt`](isal-arm-build-record.txt) gives the observed
compiler path and version for the campaign arm. It proves the linked
`xor_gen_base` symbol with `nm`, and emits its disassembly with `objdump`.
It is an untimed check.

`xor_gen` is the public route that ISA-L documents as selecting an appropriate
instruction-set implementation at runtime. Its x86 multibinary assembly
selects `xor_gen_base`, SSE, AVX, and AVX-512 implementations. `nasm` is
absent on the qualifying host, so its NASM-built multibinary route has no
reproduced executable or selected runtime symbol. This result is an
unavailability record for the dispatched comparator, not evidence that
`xor_gen_base` is SIMD or a substitute for it.

## Semantic and representation contract

`xor_gen_base(vects, len, array)` accepts at least two sources plus one
destination. `array[0..vects-2]` supplies the sources and `array[vects-1]`
is the fresh destination. The ISA-L header requires 32-byte-aligned source
and destination pointers for this XOR interface. The selected output is the
bytewise XOR of all inputs; for canonical gf2 word $w$ and bit $b$, its
observable value is

$$
d_{w,b} = \bigoplus_{s=0}^{n-1} x_{s,w,b},\qquad 0 \le b < 64.
$$

The probe executes two- and three-source cases at 1, 7, 8, 9, 63, 64, and
65 words. It creates the exact source-then-destination pointer array,
requires 32-byte alignment, poisons the destination before the call, checks
each output word against a wordwise XOR oracle, and independently checks bit
$b$ by `word >> b`, and confirms that the sources stay unchanged. It therefore
demonstrates the canonical LSB-first mapping without relying on host byte-order
prose.

## Comparison cost boundary

The operation-equivalent gf2 arm allocates a fresh destination, copies the
first source into it, and applies the canonical in-place XOR to every
remaining source. An ISA-L arm creates its `void *` pointer array and writes
the fresh destination directly. A future whole-operation comparison includes
both arms' destination allocation/setup, ISA-L's pointer-array construction,
the 32-byte-alignment arrangement, any representation conversion, and output
observation/validation. Source/destination aliasing, unaligned inputs, and a
comparison that drops either arm's arrangement cost are outside this
comparator contract.

The future protocol may compare the scalar `xor_gen_base` arm only with that
label. It may admit public `xor_gen` as a SIMD comparator only after a host
records a NASM version, a reproducible multibinary build, the linked public
symbol, and the observed dispatch route under the same semantic and cost
boundary.

## Validation record

Run the command above with the pinned checkout. A passing run prints the
defined `xor_gen_base` symbol, its assembly, fourteen LSB-first semantic
cases, the observed NASM status, and the cost-audit categories. Its output is
deterministic and contains no timing or performance conclusion.
