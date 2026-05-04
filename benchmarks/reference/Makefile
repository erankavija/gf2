# benchmarks/reference/Makefile
#
# Builds the reference benchmark binaries inside the container.
# Assumes fflas-ffpack, givaro, m4ri, ntl, and flint are installed
# under /usr/local/ (the Containerfile arranges this).
#
# Recipes are silenced (`@`-prefixed) so `make -B` does NOT echo the
# compile command to stdout. Earlier versions echoed each compile
# line; run.sh's COMPILE_CMD redirects stdout into the run's CSV
# file, so an echoed compile line corrupts the CSV with non-data
# content that breaks `analyze.py` parsing. Echoing the command to
# **stderr** would also work (since run.sh only redirects stdout to
# the CSV), but `@`-prefix is the conventional Make idiom and keeps
# the harness self-documenting via run.sh's own status output.

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

# NTL: header at /usr/local/include/NTL, library at /usr/local/lib/libntl.so.
# NTL was built with NTL_GMP_LIP=on so it depends on GMP at runtime.
NTL_CFLAGS := -I/usr/local/include
NTL_LIBS   := -L/usr/local/lib -lntl -lgmp -lm

# FLINT: header at /usr/local/include/flint, library at /usr/local/lib/libflint.so.
# FLINT depends on GMP and MPFR.
FLINT_CFLAGS := -I/usr/local/include
FLINT_LIBS   := -L/usr/local/lib -lflint -lmpfr -lgmp -lm

.PHONY: all clean
all: fflas_bench m4ri_bench linbox_bench m4rie_bench ntl_bench flint_bench ntl_flint_smoke charpoly_minpoly_smoke ntl_gf2pow32_smoke fflas_sparse_bench linbox_sparse_bench sparse_smoke

fflas_bench: fflas_bench.cpp
	@$(CXX) $(CXXSTD) $(OPT) -fopenmp $(FFLAS_CFLAGS) $< $(FFLAS_LIBS) -lopenblas -o $@

# fflas-ffpack sparse SpMV / SpMM harness (jit:47698404). Same link line
# as `fflas_bench` since both pull in fflas-ffpack + givaro + openblas.
fflas_sparse_bench: fflas_sparse_bench.cpp
	@$(CXX) $(CXXSTD) $(OPT) -fopenmp $(FFLAS_CFLAGS) $< $(FFLAS_LIBS) -lopenblas -o $@

# LinBox sparse harness (jit:47698404) — sparse-elim canonical, SpMV
# cross-check. Uses the same link line as `linbox_bench`.
linbox_sparse_bench: linbox_sparse_bench.cpp
	@$(CXX) $(CXXSTD) $(OPT) -fopenmp $(LINBOX_CFLAGS) $(FFLAS_CFLAGS) $< $(LINBOX_LIBS) $(FFLAS_LIBS) -lopenblas -o $@

# Cross-equality oracle for sparse cells at n=16 (jit:47698404, protocol § 6).
sparse_smoke: sparse_smoke.cpp
	@$(CXX) $(CXXSTD) $(OPT) $(FFLAS_CFLAGS) $< $(FFLAS_LIBS) -lopenblas -o $@

m4ri_bench: m4ri_bench.c
	@$(CC) $(CSTD) $(OPT) $(M4RI_CFLAGS) $< $(M4RI_LIBS) -o $@

ntl_bench: ntl_bench.cpp
	@$(CXX) $(CXXSTD) $(OPT) $(NTL_CFLAGS) $< $(NTL_LIBS) -o $@

flint_bench: flint_bench.c
	@$(CC) $(CSTD) $(OPT) $(FLINT_CFLAGS) $< $(FLINT_LIBS) -o $@

# Cross-equality oracle exercising the protocol §6 contracts at n=16
# for every cell that BOTH NTL and FLINT cover, plus FLINT-only invariants
# for pluq/echelon/minpoly. Linked against both libraries.
ntl_flint_smoke: ntl_flint_smoke.cpp
	@$(CXX) $(CXXSTD) $(OPT) $(NTL_CFLAGS) $(FLINT_CFLAGS) $< \
	  $(NTL_LIBS) $(FLINT_LIBS) -o $@

# GF(2^32) bitwise-equality oracle (jit:b13799ac). Cross-checks NTL
# `mat_GF2E` matmul at n=16 against an independent scalar reference
# defined purely from the gf2-core Conway-polynomial bits. Linked
# against NTL only — the scalar reference is self-contained.
ntl_gf2pow32_smoke: ntl_gf2pow32_smoke.cpp
	@$(CXX) $(CXXSTD) $(OPT) $(NTL_CFLAGS) $< $(NTL_LIBS) -o $@

linbox_bench: linbox_bench.cpp
	@$(CXX) $(CXXSTD) $(OPT) -fopenmp $(LINBOX_CFLAGS) $(FFLAS_CFLAGS) $< $(LINBOX_LIBS) $(FFLAS_LIBS) -lopenblas -o $@

m4rie_bench: m4rie_bench.c
	@$(CC) $(CSTD) $(OPT) $(M4RIE_CFLAGS) $< $(M4RIE_LIBS) -o $@

# Single-cell driver used to capture perf-stat for the m4rie promotion
# evidence (jit:507b0036). Pinned to matmul × GF(2^16) × n=1024, uniform.
m4rie_one_cell: m4rie_one_cell.c
	@$(CC) $(CSTD) $(OPT) $(M4RIE_CFLAGS) $< $(M4RIE_LIBS) -o $@

# Cross-library bitwise-equality oracle for charpoly + minpoly between
# LinBox and FLINT (issue c3e79272). Complements ntl_flint_smoke (which
# covers charpoly NTL ↔ FLINT) by adding the minpoly cross-check, since
# NTL does not expose a user-facing matrix-minpoly API. Linked against
# both LinBox and FLINT.
charpoly_minpoly_smoke: charpoly_minpoly_smoke.cpp
	@$(CXX) $(CXXSTD) $(OPT) -fopenmp $(LINBOX_CFLAGS) $(FFLAS_CFLAGS) $(FLINT_CFLAGS) $< \
	  $(LINBOX_LIBS) $(FFLAS_LIBS) $(FLINT_LIBS) -lopenblas -o $@

clean:
	rm -f fflas_bench m4ri_bench linbox_bench m4rie_bench m4rie_one_cell ntl_bench flint_bench ntl_flint_smoke charpoly_minpoly_smoke ntl_gf2pow32_smoke fflas_sparse_bench linbox_sparse_bench sparse_smoke
