# benchmarks/reference/Makefile
#
# Builds the two reference benchmark binaries inside the container.
# Assumes fflas-ffpack, givaro and m4ri are installed under /usr/local/
# (the Containerfile arranges this).

CXX      ?= g++
CC       ?= gcc
CXXSTD   ?= -std=c++17
CSTD     ?= -std=c11
OPT      ?= -O3 -march=native

# pkg-config drives include + link flags for fflas/givaro.
FFLAS_CFLAGS := $(shell pkg-config --cflags fflas-ffpack givaro)
FFLAS_LIBS   := $(shell pkg-config --libs   fflas-ffpack givaro)

# M4RI is a plain Autoconf install: header at /usr/local/include/m4ri,
# library at /usr/local/lib/libm4ri.so.
M4RI_CFLAGS := -I/usr/local/include
M4RI_LIBS   := -L/usr/local/lib -lm4ri -lm

# LinBox 1.7.1 ships a pkg-config file (linbox.pc); these flags pull in
# fflas-ffpack + givaro + openblas transitively. The harness only uses
# Modular<int64_t> from Givaro, so the optional NTL/FLINT/IML linkage
# advertised by linbox-config is sidestepped via direct pkg-config.
LINBOX_CFLAGS := $(shell pkg-config --cflags linbox 2>/dev/null || echo "-I/usr/local/include")
LINBOX_LIBS   := $(shell pkg-config --libs   linbox 2>/dev/null || echo "-L/usr/local/lib -llinbox")

# M4RIE installs alongside M4RI under /usr/local; it depends on M4RI
# transitively. Header at /usr/local/include/m4rie/, library at
# /usr/local/lib/libm4rie.so.
M4RIE_CFLAGS := -I/usr/local/include
M4RIE_LIBS   := -L/usr/local/lib -lm4rie -lm4ri -lm

.PHONY: all clean
all: fflas_bench m4ri_bench linbox_bench m4rie_bench

fflas_bench: fflas_bench.cpp
	$(CXX) $(CXXSTD) $(OPT) -fopenmp $(FFLAS_CFLAGS) $< $(FFLAS_LIBS) -lopenblas -o $@

m4ri_bench: m4ri_bench.c
	$(CC) $(CSTD) $(OPT) $(M4RI_CFLAGS) $< $(M4RI_LIBS) -o $@

linbox_bench: linbox_bench.cpp
	$(CXX) $(CXXSTD) $(OPT) -fopenmp $(LINBOX_CFLAGS) $(FFLAS_CFLAGS) $< $(LINBOX_LIBS) $(FFLAS_LIBS) -lopenblas -o $@

m4rie_bench: m4rie_bench.c
	$(CC) $(CSTD) $(OPT) $(M4RIE_CFLAGS) $< $(M4RIE_LIBS) -o $@

# Single-cell driver used to capture perf-stat for the m4rie promotion
# evidence (jit:507b0036). Pinned to matmul × GF(2^16) × n=1024, uniform.
m4rie_one_cell: m4rie_one_cell.c
	$(CC) $(CSTD) $(OPT) $(M4RIE_CFLAGS) $< $(M4RIE_LIBS) -o $@

clean:
	rm -f fflas_bench m4ri_bench linbox_bench m4rie_bench m4rie_one_cell
