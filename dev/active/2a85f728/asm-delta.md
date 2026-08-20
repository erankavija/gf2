# Bit-backend selection-boundary instruction delta

This receipt compares the current-main boundary with issue `2a85f728` at the
production `popcount` and `xor_inplace` entry points. The probe wraps those
entry points in `#[inline(never)]` functions so the inlined selector remains
visible in the emitted function body. The probe source is temporary and is not
part of the library API.

## Reproduction

From the worktree root, with the probe at
`crates/gf2-core/examples/asm_boundary_probe.rs`, run:

```sh
cargo rustc --offline -p gf2-core --example asm_boundary_probe --features simd --release -- \
  --emit=asm -Cllvm-args=--x86-asm-syntax=intel
```

The relevant source is compiled with the repository's optimized release
profile. The emitted `.s` file is the `asm_boundary_probe.*.rcgu.s` file under
`target/release/examples/`; the excerpts below are the complete selection
boundary and its immediately following size comparison, with unrelated kernel
body omitted.

The temporary probe contains exactly these wrappers and call-site retention:

```rust
#[inline(never)]
pub fn popcount_probe(buf: &[u64]) -> u64 {
    gf2_core::kernels::ops::popcount(buf)
}

#[inline(never)]
pub fn xor_probe(dst: &mut [u64], src: &[u64]) {
    gf2_core::kernels::ops::xor_inplace(dst, src);
}

fn main() {
    let mut dst = [0_u64; 8];
    let src = [1_u64; 8];
    std::hint::black_box(popcount_probe(std::hint::black_box(&src)));
    xor_probe(std::hint::black_box(&mut dst), std::hint::black_box(&src));
    std::hint::black_box(dst);
}
```

## `popcount` / words path

### Before — current main (`fd04f37c`)

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS_RESOLVED@GOTPCREL]
movzx  eax, byte ptr [rax]
test   al, al
je     .Lresolve
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jae    .Lsimd
```

### After — issue `2a85f728`

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jae    .Lsimd
```

## `xor_inplace` / words path

### Before — current main (`fd04f37c`)

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS_RESOLVED@GOTPCREL]
movzx  eax, byte ptr [rax]
test   al, al
je     .Lresolve
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jb     .Lscalar
```

### After — issue `2a85f728`

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jb     .Lscalar
```

The delta is one relaxed threshold load plus the existing comparison on the
resolved fast path. The resolved-flag load, its test, the resolution branch,
and the cold resolver call disappear from both pinned entry points. The
comparison sense remains unchanged (`>=` for the displayed popcount path and
the equivalent `<` scalar fallback for XOR).
