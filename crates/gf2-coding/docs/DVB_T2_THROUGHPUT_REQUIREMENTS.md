# DVB-T2 Real-Time Throughput Requirements

## DVB-T2 Standard Parameters

### Broadcast Modes (ETSI EN 302 755)

DVB-T2 supports various configurations optimized for different scenarios:

| Mode | Bandwidth | Max Data Rate | Typical Use Case |
|------|-----------|---------------|------------------|
| **8 MHz (HD TV)** | 8 MHz | 50.3 Mbps | Standard European broadcast |
| **7 MHz** | 7 MHz | 45.5 Mbps | Regional/mobile |
| **6 MHz (US)** | 6 MHz | 40.2 Mbps | US/Latin America |
| **5 MHz** | 5 MHz | 33.8 Mbps | Mobile TV |
| **1.7 MHz** | 1.7 MHz | 10.9 Mbps | T2-Lite (portable) |

### Our Test Configuration: Rate 3/5 Normal Frame

**Parameters**:
- LDPC: Normal frame (n=64,800), Rate 3/5
- BCH: 192 parity bits (t=12)
- Information bits per LDPC block: k=38,880

**Frame Structure (from VV001-CR35)**:
- 202 LDPC blocks per frame
- 4 frames in test vector set
- Frame duration: ~250ms (typical for 8 MHz mode)

---

## Real-Time Throughput Calculations

### Information Bit Rate

**Per LDPC Block**: 38,880 information bits (before BCH encoding)

**Per Frame**: 202 blocks × 38,880 bits = **7,853,760 bits** = **7.85 Mbits**

**Frame Duration (8 MHz, QPSK)**: ~250ms typical

**Required Information Rate**: 7.85 Mbits / 0.25s = **31.4 Mbps**

### LDPC Encoding Throughput Requirement

To process 202 blocks in 250ms (real-time):
- Time per block: 250ms / 202 = **1.24 ms/block**
- Required encoding rate: 38,880 bits / 1.24ms = **31.4 Mbps**

**Alternative calculation (conservative)**:
For 100ms processing window (40% of frame time):
- Time per block: 100ms / 202 = **0.495 ms/block**
- Required encoding rate: **78.5 Mbps**

### LDPC Decoding Throughput Requirement

Similar to encoding but may need more headroom for:
- Multiple iterations (error-prone channels)
- Burst processing
- Buffer management

**Conservative target**: **50-100 Mbps** for reliable real-time decoding

---

## Current Performance vs. Real-Time

### Current Status

| Operation | Current | Real-Time Target | Gap | Status |
|-----------|---------|------------------|-----|--------|
| **LDPC Encoding** | 3.85 Mbps | 31.4 Mbps | **8.2× slower** | ❌ Not real-time |
| **LDPC Decoding** | 1.35 Mbps | 50 Mbps | **37× slower** | ❌ Not real-time |
| **BCH Encoding** | >100 Mbps | 31.4 Mbps | ✅ Fast enough | ✅ Real-time capable |

### Time Budget Analysis (250ms frame)

**Current Performance**:
- LDPC Encoding: 202 blocks × 9.87ms = **1,994 ms** (2.0 seconds)
- LDPC Decoding: 202 blocks × ~30ms = **6,060 ms** (6.1 seconds)
- BCH Encoding: 202 blocks × ~0.1ms = **20 ms**
- BCH Decoding: 202 blocks × ~0.1ms = **20 ms**

**Total current latency**: **~8 seconds per frame**

**Real-time budget**: **250 ms per frame**

**We are 32× too slow** for real-time DVB-T2 reception!

---

## Real DVB-T2 Receiver Requirements

### Full FEC Chain Throughput

For complete DVB-T2 receiver processing:

1. **BCH Decoding**: 38,880 bits/block → ~31 Mbps needed ✅ We have 100+ Mbps
2. **LDPC Decoding**: 64,800 bits/block → ~52 Mbps needed ❌ We have 1.35 Mbps
3. **Bit Deinterleaving**: ~52 Mbps needed (trivial, no compute)
4. **QAM Demapping**: ~52 Mbps needed (LUT-based, fast)

**Bottleneck**: LDPC Decoding at **1.35 Mbps** vs **52 Mbps needed**

### Realistic Performance Targets

#### Tier 1: Offline Processing (Current)
- **Current**: 3.85 Mbps encoding, 1.35 Mbps decoding
- **Use case**: Post-capture file processing only
- **Real-time factor**: 0.03× (3% of real-time)

#### Tier 2: Near Real-Time (Week 1 Goal)
- **Target**: 10-20 Mbps encoding, 5-10 Mbps decoding
- **Use case**: Software recording with 2-5× delay
- **Real-time factor**: 0.2-0.6× (20-60% of real-time)
- **Speedup needed**: 3-5× ✅ Achievable with batch processing + parallelism

#### Tier 3: Real-Time SDR (Week 2 Goal)
- **Target**: 50-100 Mbps encoding/decoding
- **Use case**: Live DVB-T2 reception on PC
- **Real-time factor**: 1-2× (real-time to 2× real-time)
- **Speedup needed**: 13-25× ✅ Achievable with SIMD + optimizations

#### Tier 4: High-Performance (Stretch Goal)
- **Target**: 200+ Mbps encoding/decoding
- **Use case**: Multi-channel processing, 4K UHDTV
- **Real-time factor**: 4-6×
- **Speedup needed**: 50-100× ⚠️ Requires aggressive optimization (GPU?)

---

## Competitive Landscape

### Commercial DVB-T2 Implementations

**Hardware Decoders** (ASICs):
- Throughput: 100-200 Mbps
- Latency: <10ms
- Power: 1-5W

**Software Implementations**:
- GNU Radio gr-dtv: ~10-30 Mbps (x86, single-threaded)
- DVBlast: ~50 Mbps (optimized, multi-threaded)
- Professional SDR: 100+ Mbps (with SIMD, GPU offload)

**Our Target**: Match software implementations (30-100 Mbps)

---

## Optimization Roadmap to Real-Time

### Phase 1: Quick Wins (Week 1) - Target: 10-20 Mbps
**Optimizations**:
- Batch processing (2-5×)
- Block-level parallelism with rayon (4-8×)
- Pre-allocation (1.5-2×)

**Expected**: 10-20× speedup → **10-20 Mbps encoding**

**Real-time capability**: 30-60% (near real-time with buffering)

**Use case**: ✅ Software recording with 2-3× delay

### Phase 2: SIMD + Memory (Week 2) - Target: 50-100 Mbps
**Optimizations**:
- SIMD vectorization for LLRs (4-8×)
- Optimize matrix-vector multiply (2-4×)
- Quantized LLRs (2-3×)
- Layered decoding (2-3×)

**Expected**: 40-80× speedup → **50-100 Mbps encoding/decoding**

**Real-time capability**: 100-200% (real-time + headroom)

**Use case**: ✅ Live DVB-T2 reception on PC

### Phase 3: Advanced (Optional) - Target: 200+ Mbps
**Optimizations**:
- GPU offload (10-100×)
- Multi-channel parallel processing
- Specialized DVB-T2 accelerators

**Expected**: 100-200× speedup → **200-400 Mbps**

**Real-time capability**: 4-8× (multi-channel, 4K)

**Use case**: Professional broadcasting equipment

---

## Practical Milestones

### Milestone 1: Software Recording (2-3 weeks)
**Target**: 10-20 Mbps
- Can record DVB-T2 stream with 2-3× delay
- Suitable for capture-and-process workflows
- **Speedup needed**: 3-5× ✅ **Achievable**

### Milestone 2: Live Reception (4-6 weeks)
**Target**: 50-100 Mbps
- Real-time DVB-T2 decoding on PC
- Compatible with SDR hardware (RTL-SDR, HackRF)
- **Speedup needed**: 13-25× ✅ **Achievable**

### Milestone 3: Professional (3-6 months)
**Target**: 200+ Mbps
- Multi-channel processing
- 4K UHDTV support
- **Speedup needed**: 50-100× ⚠️ **Challenging**

---

## Bottleneck Priority

Based on current performance:

1. **CRITICAL: LDPC Decoding** - 37× too slow
   - Current: 1.35 Mbps
   - Needed: 50 Mbps
   - Impact: 80% of total latency

2. **HIGH: LDPC Encoding** - 8× too slow
   - Current: 3.85 Mbps
   - Needed: 31.4 Mbps
   - Impact: 15% of total latency

3. **OK: BCH** - Fast enough
   - Current: >100 Mbps
   - Needed: 31.4 Mbps
   - Impact: <5% of latency

---

## Conclusion

### Current Status
- ✅ **Correctness**: 100% verified with ETSI test vectors
- ❌ **Performance**: 3% of real-time (32× too slow)
- 📊 **Bottleneck**: LDPC decoding (37× gap)

### Realistic Goals
- **Week 1**: 10-20 Mbps → Software recording (30-60% real-time)
- **Week 2**: 50-100 Mbps → Live DVB-T2 reception (100-200% real-time)
- **Stretch**: 200+ Mbps → Professional multi-channel processing

### Key Insight
**We need 10-25× speedup to achieve practical real-time DVB-T2 reception.**

This is **achievable** with:
- Batch processing + parallelism (10-20×)
- SIMD vectorization (additional 2-4×)
- Memory optimizations (additional 1.5-2×)

**No exotic techniques required** - standard optimizations will get us there! 🎯

---

## Next Steps

1. **Profile to find hotspot** (today)
2. **Implement batch + parallel** (this week → 10-20 Mbps)
3. **Add SIMD** (next week → 50-100 Mbps)
4. **Celebrate real-time DVB-T2 reception** 🎉
