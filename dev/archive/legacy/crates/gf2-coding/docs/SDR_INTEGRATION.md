# SDR and DSP Framework Integration

This document outlines integration opportunities between `gf2-coding` and software-defined radio (SDR) frameworks like GNU Radio, LuaRadio, and related signal processing ecosystems.

## Overview

The `gf2-coding` library provides high-performance error-correcting codes with clean, functional APIs that are well-suited for integration with SDR frameworks. Key advantages:

- **Performance**: Optimized Rust kernels with SIMD support outperform typical C++ implementations
- **Correctness**: Comprehensive test coverage (60+ BCH tests, 31 LDPC tests) ensures reliability
- **Standard compliance**: DVB-T2 BCH/LDPC implementations follow ETSI EN 302 755
- **Streaming API**: `StreamingEncoder`/`StreamingDecoder` traits align with GNU Radio's block architecture
- **Soft-decision support**: LLR operations compatible with modern soft-symbol processing

## Integration Points

### 1. GNU Radio Forward Error Correction (FEC) Blocks

**Target modules**: `gr-fec`, `gr-dtv`

**High-value codecs**:
- **DVB-T2 LDPC** (Rate 1/2 Normal frame implemented, 11 configurations pending)
  - Belief propagation decoder with min-sum approximation
  - Soft-decision LLR input compatible with GNU Radio's `float` streams
  - Superior performance to existing GNU Radio LDPC implementations
  
- **DVB-T2 BCH** (All 12 configurations complete, verification pending)
  - Algebraic decoding with Berlekamp-Massey + Chien search
  - Systematic encoding for compatibility with LDPC inner code
  - Standard-compliant generator polynomials from ETSI tables

- **Convolutional codes** with Viterbi decoding
  - NASA/CCSDS standard generator polynomials (rate-1/2, K=3)
  - Drop-in replacement for `fec_decode_viterbi_fb` blocks
  - Hard-decision interface (soft-decision SOVA planned)

**Implementation approach**:
```rust
// C FFI wrapper for GNU Radio integration
#[no_mangle]
pub extern "C" fn gf2_ldpc_decoder_create(
    frame_size: u32,  // 0 = short (16200), 1 = normal (64800)
    code_rate: u32,   // 0 = 1/2, 1 = 3/5, etc.
    max_iterations: u32
) -> *mut LdpcDecoder;

#[no_mangle]
pub extern "C" fn gf2_ldpc_decode(
    decoder: *mut LdpcDecoder,
    llrs: *const f32,
    len: usize,
    output: *mut u8
) -> i32;  // Returns iteration count or -1 on failure

#[no_mangle]
pub extern "C" fn gf2_ldpc_decoder_destroy(decoder: *mut LdpcDecoder);
```

**GNU Radio block structure**:
```cpp
// gr-gf2/lib/dvb_t2_ldpc_decoder_impl.cc
class dvb_t2_ldpc_decoder_impl : public dvb_t2_ldpc_decoder {
private:
    void* rust_decoder_;  // Opaque pointer to Rust LdpcDecoder
    
public:
    int work(int noutput_items,
             gr_vector_const_void_star &input_items,
             gr_vector_void_star &output_items) {
        const float* llrs = (const float*)input_items[0];
        uint8_t* bits = (uint8_t*)output_items[0];
        
        int result = gf2_ldpc_decode(rust_decoder_, llrs, 64800, bits);
        return (result >= 0) ? 1 : 0;  // Consume 1 frame, produce 1 decoded block
    }
};
```

### 2. Out-of-Tree (OOT) Module: gr-gf2

**Proposed structure**:
```
gr-gf2/
├── lib/
│   ├── libgf2_coding.so          # Compiled Rust library
│   ├── dvb_t2_ldpc_decoder_impl.cc
│   ├── dvb_t2_bch_decoder_impl.cc
│   ├── viterbi_decoder_impl.cc
│   └── ffi_wrapper.h              # C header for FFI functions
├── grc/
│   ├── gf2_dvb_t2_ldpc_decoder.block.yml
│   └── gf2_viterbi_decoder.block.yml
├── python/
│   └── __init__.py
└── examples/
    └── dvb_t2_rx_chain.grc        # Complete DVB-T2 receiver flowgraph
```

**Development steps**:
1. Create OOT module skeleton: `gr_modtool newmod gf2`
2. Add C FFI layer to `gf2-coding` crate (new module `src/ffi.rs`)
3. Implement GNU Radio block wrappers in C++
4. Create GRC block definitions for visual programming
5. Add example flowgraphs demonstrating DVB-T2 reception

**Expected performance**:
- **Throughput**: 10-50 Mbps LDPC decoding on modern CPU (vs 1-5 Mbps for existing gr-fec)
- **Latency**: Single-frame processing (<10ms for 64800-bit codewords)
- **CPU usage**: 30-50% reduction due to optimized kernels and SIMD

### 3. Streaming API for Real-Time Processing

The existing `StreamingEncoder`/`StreamingDecoder` traits are ideal for GNU Radio's sample-by-sample processing model:

```rust
pub trait StreamingEncoder {
    fn encode_bit(&mut self, bit: bool) -> Vec<bool>;
    fn reset(&mut self);
}

pub trait StreamingDecoder {
    fn decode_symbols(&mut self, symbols: &[bool]) -> Vec<bool>;
    fn reset(&mut self);
}
```

**Use case**: Convolutional code integration
```cpp
// GNU Radio block with Rust Viterbi decoder
class viterbi_decoder_impl : public viterbi_decoder {
    void* rust_decoder_;
    
    int work(...) {
        const uint8_t* syms = (const uint8_t*)input_items[0];
        uint8_t* bits = (uint8_t*)output_items[0];
        
        for (int i = 0; i < noutput_items * 2; i += 2) {
            bool s1 = syms[i];
            bool s2 = syms[i+1];
            bool decoded = gf2_viterbi_decode_symbols(rust_decoder_, s1, s2);
            bits[i/2] = decoded ? 1 : 0;
        }
        return noutput_items;
    }
};
```

### 4. Channel Modeling and Simulation

**Existing capabilities**:
- AWGN channel with configurable Eb/N0
- BPSK modulation/demodulation
- Shannon capacity calculation
- Monte Carlo BER/FER simulation framework

**Integration with GNU Radio**:
- **Validation**: Cross-check GNU Radio channel models against `gf2-coding` simulations
- **Performance baseline**: Use Rust implementation to generate reference BER/FER curves
- **Hybrid approach**: GNU Radio for RF front-end, Rust for compute-intensive decoding

**Example workflow**:
```python
# GNU Radio Python script
from gnuradio import blocks, channels, fec
import gf2_coding  # Rust library via PyO3

# Capture IQ samples from USRP
usrp_source = uhd.usrp_source(...)

# Demodulate in GNU Radio
demod = digital.constellation_decoder_cb(constellation.qpsk())

# Decode in Rust for performance
rust_decoder = gf2_coding.DvbT2LdpcDecoder(frame_size="normal", rate=0.5)
decoded_bits = rust_decoder.decode(llr_samples)
```

### 5. LLR Operations for Soft-Decision Decoding

The `llr` module provides operations compatible with GNU Radio's soft-symbol processing:

**Available operations**:
- LLR type with numerical stability (`Llr::new()`, `Llr::from_prob()`)
- Box-plus for check node updates (exact and min-sum)
- Multi-operand operations for LDPC belief propagation
- Conversion utilities (symbol → LLR, LLR → hard decision)

**GNU Radio integration**:
```cpp
// Convert GNU Radio float LLRs to Rust representation
std::vector<float> gnu_radio_llrs = {...};
gf2_ldpc_decode(decoder, gnu_radio_llrs.data(), gnu_radio_llrs.size(), output);
```

**Advantage over gr-fec**: Optimized min-sum approximations with 2-3x speedup over naive implementations.

## Related Projects and Interfaces

### 1. LuaRadio
**Easier FFI than GNU Radio** due to Lua's C interop:
```lua
local ffi = require('ffi')
local gf2 = ffi.load('libgf2_coding.so')

ffi.cdef[[
    void* gf2_ldpc_decoder_create(uint32_t frame_size, uint32_t code_rate, uint32_t max_iter);
    int gf2_ldpc_decode(void* decoder, const float* llrs, size_t len, uint8_t* output);
]]

local decoder = gf2.gf2_ldpc_decoder_create(1, 0, 50)  -- Normal frame, rate 1/2
-- Use in LuaRadio flowgraph
```

### 2. gr-satellites
**Satellite decoder collection** - contribute Viterbi/BCH decoders for telemetry formats:
- AX.25 (amateur radio)
- CCSDS (deep space)
- DVB-S2 (broadcast satellites)

### 3. SigMF (Signal Metadata Format)
**Export decoded bitstreams** with metadata:
```json
{
  "global": {
    "core:datatype": "ru8",
    "core:sample_rate": 1000000,
    "gf2:decoder": "dvb-t2-ldpc",
    "gf2:code_rate": 0.5,
    "gf2:iterations": 12
  }
}
```

### 4. Inspectrum
**Offline signal analyzer** - export decoded symbols for visualization:
- Integrate as post-processing step
- Validate decoding against known test vectors
- Generate training data for ML-based decoders

### 5. SDRangel
**SDR GUI with plugin architecture** - similar integration path to GNU Radio but with Qt UI.

## Implementation Roadmap

### Phase 1: C FFI Layer (1-2 weeks)
- [ ] Create `src/ffi.rs` module with C-compatible API
- [ ] Expose LDPC decoder (create, decode, destroy)
- [ ] Expose BCH decoder
- [ ] Expose Viterbi decoder
- [ ] Add safety wrappers and error handling
- [ ] Write C header file (`gf2_coding.h`)
- [ ] Test FFI from standalone C program

### Phase 2: GNU Radio OOT Module (2-3 weeks)
- [ ] Initialize `gr-gf2` with `gr_modtool newmod`
- [ ] Implement `dvb_t2_ldpc_decoder` block
- [ ] Implement `dvb_t2_bch_decoder` block
- [ ] Implement `viterbi_decoder` block
- [ ] Create GRC block definitions
- [ ] Add example flowgraphs (DVB-T2 receiver chain)
- [ ] Write integration tests with simulated IQ data
- [ ] Documentation and installation guide

### Phase 3: Real-World Validation (2-4 weeks)
- [ ] Test with DVB-T2 IQ recordings (DVB-T2 Conformance Test Set)
- [ ] Compare against GNU Radio's existing FEC blocks
- [ ] Benchmark throughput and CPU usage
- [ ] Validate error correction capability with noisy signals
- [ ] Generate BER/FER curves for publication

### Phase 4: Python Bindings (1-2 weeks, optional)
- [ ] Create PyO3 wrapper for offline processing
- [ ] Integrate with NumPy/SciPy for signal processing
- [ ] Jupyter notebook examples
- [ ] Distribution via PyPI

### Phase 5: Extended SDR Support (ongoing)
- [ ] LuaRadio blocks
- [ ] SDRangel plugin
- [ ] gr-satellites contributions
- [ ] SigMF metadata extensions

## Performance Targets

Based on preliminary benchmarks, target performance for GNU Radio integration:

| Codec | Configuration | Throughput (Mbps) | Latency (ms) | CPU Usage |
|-------|--------------|-------------------|--------------|-----------|
| LDPC BP | DVB-T2 Normal 1/2 | 20-50 | <10 | 40-60% (1 core) |
| BCH | DVB-T2 Normal 1/2 | 100-200 | <1 | 10-20% (1 core) |
| Viterbi | Rate 1/2, K=3 | 50-100 | <1 | 20-30% (1 core) |

Compare to GNU Radio's existing implementations:
- `gr-fec` LDPC: 1-5 Mbps (10-50x slower)
- `gr-fec` Viterbi: 10-30 Mbps (2-10x slower)

## Testing Strategy

### Unit Tests (FFI layer)
- [ ] C program calling FFI functions
- [ ] Memory leak checks with Valgrind
- [ ] Error handling (null pointers, invalid parameters)

### Integration Tests (GNU Radio)
- [ ] Flowgraphs with simulated transmitter/receiver
- [ ] Inject known bit patterns, verify decoding
- [ ] Add noise, measure BER/FER
- [ ] Compare against Python reference implementation

### Real-World Tests
- [ ] DVB-T2 broadcast signals (capture with RTL-SDR/HackRF)
- [ ] Satellite telemetry (NOAA, amateur radio)
- [ ] Interoperability with commercial DVB-T2 equipment

## Documentation Deliverables

- [ ] C API reference (`docs/C_API.md`)
- [ ] GNU Radio block documentation (inline in GRC)
- [ ] Example flowgraphs with detailed comments
- [ ] Jupyter notebooks for offline analysis
- [ ] Performance benchmarking report
- [ ] Integration guide for other SDR frameworks

## Open Questions

1. **Soft-decision interface**: Use `float` LLRs (GNU Radio standard) or fixed-point quantization for performance?
2. **Threading model**: Single-threaded per block or parallel decoding of multiple frames?
3. **Buffer management**: Zero-copy via shared memory or explicit copy for safety?
4. **Error handling**: Return codes vs exceptions in C++ blocks?
5. **Licensing**: Ensure compatibility with GNU Radio's GPL (currently gf2-coding license TBD)

## References

- [GNU Radio FEC API](https://wiki.gnuradio.org/index.php/Forward_Error_Correction)
- [ETSI EN 302 755 DVB-T2 Standard](https://www.etsi.org/deliver/etsi_en/302700_302799/302755/)
- [gr-dtv Source](https://github.com/gnuradio/gnuradio/tree/main/gr-dtv)
- [LuaRadio](https://luaradio.io/)
- [SigMF Specification](https://github.com/gnuradio/SigMF)

## Contact and Collaboration

For integration discussions, reach out via:
- GitHub Issues: Feature requests and bug reports
- Mailing lists: GNU Radio Discuss (for gr-gf2 module)
- Matrix/IRC: Real-time coordination with SDR developers
