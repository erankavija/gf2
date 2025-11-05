# Custom GitHub Copilot Agents

This directory contains custom agent configurations for GitHub Copilot, following the [GitHub custom agents documentation](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/create-custom-agents).

## Available Agents

### HPC Specialist (`hpc-specialist.yml`)

An expert agent focused on high-performance computing implementations for the gf2 library.

**Specializations:**
- Clean, composable API design that hides implementation complexity
- High-performance kernel implementations (SIMD, cache-aware algorithms)
- Comprehensive benchmarking with baseline comparisons
- Performance optimization while maintaining correctness

**When to use:**
- Implementing or optimizing performance-critical code
- Adding new operations that need both clean APIs and fast implementations
- Benchmarking existing implementations against baselines
- Designing SIMD-accelerated kernels

**Key principles:**
- API cleanliness is non-negotiable
- Benchmarking is mandatory for all optimizations
- Always provide baseline comparisons in `cargo bench`
- Profile before optimizing
- Correctness before speed

## Usage

GitHub Copilot will automatically detect and use these agent configurations when working on the repository. The agents provide specialized guidance and code patterns specific to their domain of expertise.
