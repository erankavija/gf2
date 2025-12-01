# gf2-coding Documentation

This directory contains technical documentation for the `gf2-coding` crate.

## Documentation Index

### Implementation & Verification

- **[DVB_T2.md](DVB_T2.md)** - DVB-T2 LDPC/BCH implementation, test vectors, and verification status
- **[LDPC_VERIFICATION_TESTS.md](LDPC_VERIFICATION_TESTS.md)** - Comprehensive LDPC test suite description

### Performance

- **[LDPC_PERFORMANCE.md](LDPC_PERFORMANCE.md)** - LDPC optimization progress, profiling results, and performance targets
- **[PARALLELIZATION.md](PARALLELIZATION.md)** - Overall parallelization strategy (CPU, SIMD, future GPU)
- **[SIMD_PERFORMANCE_GUIDE.md](SIMD_PERFORMANCE_GUIDE.md)** - SIMD acceleration details and usage

### Integration & Conventions

- **[SDR_INTEGRATION.md](SDR_INTEGRATION.md)** - Software-defined radio integration guide
- **[SYSTEMATIC_ENCODING_CONVENTION.md](SYSTEMATIC_ENCODING_CONVENTION.md)** - Bit ordering conventions for systematic codes

## Quick Links

**Getting Started**: See [../README.md](../README.md) for API overview and examples  
**Contributing**: See [../ROADMAP.md](../ROADMAP.md) for planned features  
**Test Vectors**: Set `DVB_TEST_VECTORS_PATH` environment variable

## Document Organization

Previously scattered information has been consolidated:
- DVB-T2 content: 4 files → `DVB_T2.md`
- LDPC performance: 5 files → `LDPC_PERFORMANCE.md`
- Parallelization: 3 files → `PARALLELIZATION.md`

Implementation details evident from code have been removed to reduce maintenance burden.
