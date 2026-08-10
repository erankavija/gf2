# Permanent-wave GPU architecture compile evidence

schema_version: 1
source_revision: cb4cfb036786dd2dae294f08c31360cc56234588
worktree_clean: true
worktree_status_command: git status --porcelain=v1 --untracked-files=all
worktree_status: empty
worktree_status_sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
rocm_path: /opt/rocm
claimed_architectures: gfx1030

## Toolchain
HIP version: 7.2.53211-9999
AMD clang version 22.0.0git (/srcdest/rocm-llvm f58b06dce1f9c15707c5f808fd002e18c2accf7e)
Target: x86_64-pc-linux-gnu
Thread model: posix
InstalledDir: /opt/rocm/lib/llvm/bin
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
rustc 1.95.0 (59807616e 2026-04-14)
binary: rustc
commit-hash: 59807616e1fa2540724bfbac14d7976d7e4a3860
commit-date: 2026-04-14
host: x86_64-unknown-linux-gnu
release: 1.95.0
LLVM version: 22.1.2

## HIP source inventory
a938162c22133962b5d8ff40aa8eea9e6c4d0a670d7ed7cd687c7dde107c7d29  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-773a5378/dev/research/permanent_wave_gpu/hip/f5_wave_equivalence.hip
44c5cae2af84e8912f5b4216723fe33e32033337bd1c0ab2463e7307303e4e3f  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-773a5378/dev/research/permanent_wave_gpu/hip/f7_three_plane_equivalence.hip
c2646135b706bf3a242cd5b0770cc8a283494df003b6b66d9bdff4bdaa2345b9  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-773a5378/dev/research/permanent_wave_gpu/hip/probe.hip
a21ec7a235bcd277814aa15daaf1d515c9ed892f86d66f78808e5d5fe81ca82a  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-773a5378/dev/research/permanent_wave_gpu/hip/wave_gf3_equivalence.hip
20b522c24d44103412fe7021b90bb98581e0d8b86b7b9f817ae388330d107f84  /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-773a5378/dev/research/permanent_wave_gpu/hip/wave_gf7_equivalence.hip

## Architecture attempts
- architecture: gfx1030
  outcome: passed (compile evidence only)
  raw_log: hip/architecture_compile_evidence-v1.log

raw_log_sha256: db51eadb86102cc72339714a25684ff2a0f169ad278d00b10cd363b81b54229b

The claimed set is not widened automatically by this receipt. A failed
attempt remains part of the raw log, including the exact toolchain diagnostic.
Compile success does not establish runtime portability or performance.
