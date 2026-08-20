# Bit-backend selection-boundary instruction delta

This receipt compares the selection boundary at all five pinned production
entry points for issue `2a85f728`: `xor_inplace`, `and_inplace`, `or_inplace`,
`not_inplace`, and `popcount`. The probe wraps each entry point in an
`#[inline(never)]` function so the inlined selector remains visible in the
emitted function body. The probe source is temporary and is not part of the
library API.

## Reproduction

From the worktree root, with the temporary probe at
`crates/gf2-core/examples/asm_boundary_probe.rs`, run the same command for
each source state:

```sh
cargo +1.95.0 rustc --offline -p gf2-core --example asm_boundary_probe --features simd --release -- \
  --emit=asm -Cllvm-args=--x86-asm-syntax=intel
```

The relevant source is compiled with the repository's optimized release
profile. The emitted `.s` file is the `asm_boundary_probe.*.rcgu.s` file under
`target/release/examples/`; the excerpts below are the threshold selection
sequence and its comparison, with unrelated kernel body omitted. Local labels
and hash-suffixed symbols are normalized in the excerpts.

The temporary probe contains these wrappers and retains every call site:

```rust
#[inline(never)]
pub fn xor_probe(dst: &mut [u64], src: &[u64]) {
    gf2_core::kernels::ops::xor_inplace(dst, src);
}

#[inline(never)]
pub fn and_probe(dst: &mut [u64], src: &[u64]) {
    gf2_core::kernels::ops::and_inplace(dst, src);
}

#[inline(never)]
pub fn or_probe(dst: &mut [u64], src: &[u64]) {
    gf2_core::kernels::ops::or_inplace(dst, src);
}

#[inline(never)]
pub fn not_probe(buf: &mut [u64]) {
    gf2_core::kernels::ops::not_inplace(buf);
}

#[inline(never)]
pub fn popcount_probe(buf: &[u64]) -> u64 {
    gf2_core::kernels::ops::popcount(buf)
}
```

The `before` excerpts were generated from the pre-change source at main
`fd04f37c`. Because the worktree is intentionally not rebased or switched, I
temporarily restored the corresponding `tuning/mod.rs` resolver region from
that commit — including `ACTIVE_SIMD_MIN_WORDS_RESOLVED`, the cold resolver,
and its flag stores — compiled the unchanged probe with the command above,
then restored the current source and reran the command. No generated assembly
file is claimed as a committed artifact; each excerpt was read from the
compiler output immediately after its respective run.

## `xor_inplace`

### Before — main at `fd04f37c`

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

### After — issue `2a85f728`, including the F1 fix

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jae    .Lsimd
lea    rax, [rip + scalar_xor_inplace]
jmp    rax
```

## `and_inplace`

### Before — main at `fd04f37c`

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

### After — issue `2a85f728`, including the F1 fix

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jb     .Lscalar
```

## `or_inplace`

### Before — main at `fd04f37c`

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

### After — issue `2a85f728`, including the F1 fix

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jb     .Lscalar
```

## `not_inplace`

### Before — main at `fd04f37c`

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

### After — issue `2a85f728`, including the F1 fix

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jae    .Lsimd
```

## `popcount`

### Before — main at `fd04f37c`

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

### After — issue `2a85f728`, including the F1 fix

```asm
mov    rax, qword ptr [rip + ACTIVE_SIMD_MIN_WORDS@GOTPCREL]
mov    rax, qword ptr [rax]
cmp    rsi, rax
jae    .Lsimd
```

The resolved fast path at every pinned entry point is now one relaxed
threshold load followed by the existing size comparison. The resolved-flag
load, its test, the resolution branch, and the cold resolver call are absent
from that path. The comparison sense remains equivalent to the original
dispatch: `jb` selects the scalar route for `xor_inplace`, `and_inplace`, and
`or_inplace`, while `jae` selects the SIMD route for `not_inplace` and
`popcount`.
