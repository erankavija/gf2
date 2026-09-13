# GPU byte-identity of the canonical edge layout (jit:07ca8585)

> **Diátaxis Type:** Reference

The change routes the GPU device layout through the same `EdgeLayout` the CPU
decoder uses, so the GPU LDPC stage has to keep reaching the CPU decoder's
hard decisions bit for bit. This file records the runs that show it does, on
this host's device. Nothing here is timed.

## Host device

| Fact | Value |
|---|---|
| Agent | gfx1030, AMD Radeon RX 6950 XT, 80 compute units |
| Code objects | `amdgcn-amd-amdhsa--gfx1030`, `amdgcn-amd-amdhsa--gfx10-3-generic` |
| ROCm | 7.2.4 |
| hipcc | HIP 7.2.53211-9999, AMD clang 22.0.0git |
| Host CPU | AMD Ryzen 9 5900X, 24 logical CPUs |

`hipcc --offload-arch=gfx940` fails its probe on this host, so the build skips
that architecture and compiles the gfx1030 kernels; the warning is the build
reporting that, not a failure.

## Commands

```
./scripts/cargo-budget.sh cargo build --release --features hip \
    --manifest-path crates/gf2-kernels-hip/Cargo.toml

./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --features hip \
    --release --profile slow --run-ignored ignored-only \
    --test gpu_ldpc_byte_identity --test gpu_byte_identity \
    --test gpu_nr_5g_byte_identity

./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --features hip \
    --release --test gpu_nr_5g_byte_identity
```

The first is the kernel build. The second runs the slow tier's gfx1030-gated
byte-identity legs, which carry `#[ignore]` and therefore need
`--run-ignored ignored-only`. The third runs the one leg of the same files that
is un-ignored, which the ignored-only selector skips.

## Results

| Test | Binary | Verdict | Wall clock |
|---|---|---|---:|
| `gpu_ldpc_hard_decision_byte_identical_to_cpu` | `gpu_ldpc_byte_identity` | pass | 61.626 s |
| `gpu_chain_verdict_byte_identical_r12_16qam` | `gpu_byte_identity` | pass | 8.247 s |
| `gpu_chain_verdict_byte_identical_r23_64qam` | `gpu_byte_identity` | pass | 7.805 s |
| `gpu_chain_verdict_byte_identical_r34_16qam` | `gpu_byte_identity` | pass | 8.260 s |
| `gpu_nr_5g_bg1_z384_r12_byte_identical_to_cpu` | `gpu_nr_5g_byte_identity` | pass | 7.169 s |
| `gpu_nr_5g_smoke_byte_identical_to_cpu` | `gpu_nr_5g_byte_identity` | pass | 0.075 s |

Six tests run, six passed, none failed and none skipped across the two
selectors. The first five are the ignored slow-tier legs, run under the `slow`
nextest profile; the sixth is the un-ignored smoke, run under the default
profile. No divergence appeared, so the GPU path needed no change.

What each covers: the LDPC leg decodes 200 frames of DVB-T2 r1/2 (n = 64800) at
each of three SNRs through min-sum, normalized min-sum at 0.75 and sum-product,
and asserts the GPU hard-decision codeword equals the CPU decoder's bit for bit;
the three chain legs assert the same for the whole DVB-T2 BICM chain verdict at
three MODCODs, whose LDPC stage is this decoder; the two 5G NR legs assert it for
BG1 at lifting 384, the second frozen workload of this issue's families.
