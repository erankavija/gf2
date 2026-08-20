# Verification receipt: the layout ensemble's axes

This records the build-time verification behind §3 of
[`layout-attribution-verdict-v1.md`](/dev/benchmarks/tuning_profiles/layout-attribution-verdict-v1.md),
the predeclared layout-attribution amendment of issue `972e2b88`. Every figure
that document states about the ensemble's axes comes from the run below.

**This receipt takes no timed measurement.** It builds the bench target under
each candidate setting and inspects the resulting binaries. It holds no bench
lock, sets no `GF2_BENCH`, and runs no cell. Nothing here is a performance
claim about `gf2-core`; the quantities are binary sizes, symbol addresses, and
build wall-clock.

## Provenance

| Item | Value |
|---|---|
| Revision | `183379074223f96215276ab3e26b7c2cbb1e8c82` |
| Working tree | clean; `git status --porcelain --untracked-files=all` empty before and after the run |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, invoked as `cargo +1.95.0` |
| Target | `-p gf2-core --features simd --bench selector_non_regression --no-run` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux` |
| Window | 2026-08-20 20:29:40–20:31:45 UTC for the axis sweep; the four candidate builds of §4 follow it in the same session |
| Host load | one-minute load average 3.90 as the sweep starts and 67.50 as it ends: other work shares the host throughout |

**The host is loaded, so the build wall-clock figures are indicative rather than
quiescent-host measurements.** Binary sizes, SHA-256 digests and symbol
addresses are properties of the build and carry no such caveat.

## Method

Each setting is built by putting it in `RUSTFLAGS`, and the binary it produced
is read from Cargo's own JSON rather than guessed, because a changed
`RUSTFLAGS` changes the artifact's filename hash:

```sh
RUSTFLAGS="<setting>" cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression --no-run --message-format=json |
  jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable'
```

Its size comes from `stat -c%s`, its digest from `sha256sum`, and its wall-clock
from the shell's `SECONDS` around the invocation.

**Placement movement** is measured against the ordinary build by comparing
symbol addresses: `nm --defined-only` on both binaries, restricted to the `t`
and `T` text symbols, gives 805 symbols common to every pair, and the reported
figure is how many of them sit at a different address than in the ordinary
build. It answers the question the ensemble depends on — whether a setting
moves code — rather than the weaker question of whether it changes bytes.

## The sweep

Growth is against the ordinary build's 795,624 bytes.

| Setting | `RUSTFLAGS` | Bytes | Growth | Symbols moved | Build | SHA-256 |
|---|---|---:|---:|---:|---:|---|
| ordinary | (none) | 795,624 | — | — | 5 s | `c1c374bf07f608ea2bdfdb3ae331f32dbcb02eb6ac6a71e40d889c39e933c7bd` |
| A = 1 | `-C llvm-args=-align-all-functions=1` | 792,808 | −0.35 % | 799/805 (99.3 %) | 7 s | `da1680c55bb1b3ab0e8ef7c67a3b3643afeead2130bd55b5d9a776463251ec6e` |
| A = 4 | `-C llvm-args=-align-all-functions=4` | 796,552 | +0.12 % | 800/805 (99.4 %) | 7 s | `ee353b238185f08195597da60614c0edd02b2f823e0aeb6347510e02131fa227` |
| A = 7 | `-C llvm-args=-align-all-functions=7` | 847,000 | +6.46 % | 805/805 (100 %) | 8 s | `daeca53c570220403fd6f951def1e232fe8fae2b5b3cc88bda177e7bf2d292f2` |
| B = 1 | `-C llvm-args=-align-all-nofallthru-blocks=1` | 794,584 | −0.13 % | 796/805 (98.9 %) | 7 s | `e17ad54775f6c8bb5827c3ca1fc1b336fe598d0d470aff7ad09a8a7d28d66146` |
| B = 2 | `-C llvm-args=-align-all-nofallthru-blocks=2` | 801,384 | +0.72 % | 805/805 (100 %) | 32 s | `5e711c8a360edda213dcb7598ae846928c7a28594d93947ea90b52e67e3e8d81` |
| B = 3 | `-C llvm-args=-align-all-nofallthru-blocks=3` | 814,680 | +2.40 % | 805/805 (100 %) | 8 s | `87e899720ea90a7e563a5a688dc3375f6af16c197ce2e03dcc765b7e8685b6af` |
| B = 5 | `-C llvm-args=-align-all-nofallthru-blocks=5` | 903,192 | +13.52 % | not measured | 7 s | `9e5bca57dc6c27d65b15487180f5f9152f95466b033e4bd89b350837adc1d03d` |
| B = 7 | `-C llvm-args=-align-all-nofallthru-blocks=7` | 1,390,232 | +74.74 % | not measured | 7 s | `638cae34fcfcfa71f5efec36dea29eee885a6e92d881a9285168c331f532334a` |
| C = 1 | `-C llvm-args=-align-all-blocks=1` | 796,376 | +0.09 % | 784/805 (97.4 %) | 8 s | `6de3c1e3c528bc3f6660cbe4af92c7bc6713c30ef3e9227c2d4d96727882208e` |
| C = 2 | `-C llvm-args=-align-all-blocks=2` | 822,488 | +3.38 % | 805/805 (100 %) | 7 s | `ec44400835989a5c378c32b82a2e428a122b9be658245c83c2cea38651355728` |
| C = 3 | `-C llvm-args=-align-all-blocks=3` | 869,704 | +9.31 % | 805/805 (100 %) | 7 s | `1a6799dcbf1c80d5de7fa8133d12393bc2def88c58d64ffe87d2030755df0feb` |
| A = 7, B = 3, C = 3 | all three at their bounds | 919,832 | +15.61 % | 805/805 (100 %) | 8 s | `fc00ad6e057307385f68300670f0a910a2d3be89479e6b95bed99e8769135d70` |
| `--sort-section=name` | `-C link-arg=-Wl,--sort-section=name` | 795,624 | 0.00 % | **0/805 (0 %)** | 7 s | `51769826368fa88e103e766463b1e4d960018a7465e9b796b7ba31579cbd93b0` |

Every one of the fourteen builds succeeded and produced a distinct SHA-256.

## What the sweep establishes

- **The three axes A, B and C are accepted at the MSRV and move code.** At every
  sampled level they re-place 97 % to 100 % of the 805 common text symbols, so
  a member of the ensemble is a different placement of the same program rather
  than the same placement with different bytes.
- **The levels the amendment admits keep the binary close to the ordinary
  build.** The widest single-axis level is C = 3 at +9.31 %, and the member that
  perturbs most, A = 7 with B = 3 and C = 3, is +15.61 %.
- **The levels the amendment excludes bloat rather than move.**
  `-align-all-nofallthru-blocks=5` grows the binary by 13.52 % and `=7` by
  74.74 %, which is why B stops at 3.
- **The worst member still satisfies the harness's own protocol check.** Built
  at A = 7, B = 3, C = 3, `--self-check` prints:

  ```
  protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
  self-check PASS
  ```

- **Build wall-clock is 5 s to 8 s across the sweep, with one 32 s observation**
  at B = 2 while other work shared the host. §4's four builds, taken minutes
  later at a one-minute load average near 67, each took 15 s to 16 s. The
  amendment's budget takes the upper end of the captured range.

## 4. The rejected axis, and its replacement

The amendment's ladder needs a fourth axis to double the ensemble from 128
members to 256. `-C link-arg=-Wl,--sort-section=name` was the first candidate,
and **it is rejected by this receipt**: it produces a different binary, at a
different SHA-256, of exactly the same size, and it moves none of the 805 text
symbols. An ensemble doubled along it would hold each of the 128 placements
twice rather than 256 placements, so its measured dispersion would describe
half the ensemble it claims.

Four replacements were built and measured the same way:

| Candidate | `RUSTFLAGS` | Bytes | Growth | Symbols moved | Build | SHA-256 |
|---|---|---:|---:|---:|---:|---|
| branch boundaries | `-C llvm-args=-x86-branches-within-32B-boundaries` | 814,104 | +2.32 % | 805/805 (100 %) | 15 s | `2dcd5ed4af77557df0fa00fbd154417e5414a256bd05eaeca626742c227260e4` |
| A = 8 | `-C llvm-args=-align-all-functions=8` | 908,952 | +14.24 % | 805/805 (100 %) | 15 s | `27a05f1770de36feb00bec18fe1819e11536e939307c405f9826f88f18bf7e12` |
| B = 4 | `-C llvm-args=-align-all-nofallthru-blocks=4` | 841,640 | +5.78 % | 805/805 (100 %) | 16 s | `a1d0a628fd5f5408ef09945579461dd93add095552f5e7fc8456bd1c4576c1d9` |
| C = 4 | `-C llvm-args=-align-all-blocks=4` | 962,232 | +20.94 % | 805/805 (100 %) | 16 s | `b7325dbcf6767b610982fdae4366817b1626619c3d4ef6fab07555a3b2561f1c` |

`-x86-branches-within-32B-boundaries` is the axis the amendment adopts: it is a
two-level switch, which is exactly what doubling the enumeration needs, it
moves every text symbol, and at +2.32 % it grows the binary less than extending
any existing axis by one level would.

## What this receipt does not establish

- **It measures no cell.** Whether the ensemble's members produce the
  across-build dispersion the amendment's coverage precondition requires is
  decided by the measured run, not here. Moving a symbol's address is necessary
  for a layout effect and does not by itself produce one.
- **It samples levels rather than enumerating members.** Fourteen settings were
  built, not the 128 members of the enumeration, so "every member builds" is
  not shown; §3.5 of the amendment carries the rule for a member that fails to
  build.
- **The build wall-clock figures are taken on a loaded host** and are not
  quiescent-host measurements.
- **Symbol movement is counted, not weighted.** A moved symbol may sit at an
  address whose cache-line offset is unchanged; the count bounds nothing about
  the size of a layout effect.
