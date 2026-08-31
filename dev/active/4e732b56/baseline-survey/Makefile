# External-baseline survey harness (jit:4e732b56).
#
# Builds the two reference drivers against baselines built from pinned
# upstream sources. `fetch-build.sh` produces both trees and this Makefile
# consumes them; EXT defaults to the location that script writes to.
#
# Build flags mirror benchmarks/Containerfile (-O3 -march=native) so the
# reference binaries and the gf2 release build see the same host ISA.

EXT       ?= $(CURDIR)/../../../../.agents/ext
AFF3CT    ?= $(EXT)/aff3ct
M4RI_PFX  ?= $(EXT)/prefix

CXX       ?= g++
CC        ?= gcc
OPT       ?= -O3 -march=native
# The AFF3CT static library is compiled as gnu++11 with the definitions below;
# a translation unit that links against it must match them exactly or the
# class layouts diverge (the definitions gate members) and the process
# corrupts its heap on the first Encoder construction.
CXXSTD    ?= -std=gnu++11
CSTD      ?= -std=c11

AFF3CT_DEFS     := -DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING \
                   -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE -DNDEBUG

AFF3CT_CXXFLAGS := $(AFF3CT_DEFS) -I$(AFF3CT)/include -I$(AFF3CT)/src \
                   -I$(AFF3CT)/lib/streampu/src \
                   -I$(AFF3CT)/lib/MIPP/include \
                   -I$(AFF3CT)/lib/streampu/include \
                   -I$(AFF3CT)/lib/streampu/lib/cli/src \
                   -I$(AFF3CT)/lib/streampu/lib/json/single_include \
                   -I$(AFF3CT)/lib/streampu/lib/rang/include \
                   -I$(AFF3CT)/lib/streampu/lib/cpptrace/include \
                   -I$(AFF3CT)/build/lib/streampu/include
AFF3CT_LIBS     := $(AFF3CT)/build/lib/libaff3ct-4.7.0.a \
                   $(AFF3CT)/build/lib/streampu/lib/cpptrace/lib/libcpptrace.a \
                   -ldwarf -lz -lzstd -lpthread -ldl

M4RI_VER    := $(shell PKG_CONFIG_PATH=$(M4RI_PFX)/lib/pkgconfig pkg-config --modversion m4ri)
M4RI_CFLAGS := -I$(M4RI_PFX)/include -DM4RI_VERSION_STR='"$(M4RI_VER)"'
M4RI_LIBS   := -L$(M4RI_PFX)/lib -lm4ri -lm -Wl,-rpath,$(M4RI_PFX)/lib

BCHLIB      ?= $(EXT)/bchlib
BCHLIB_VER  := $(shell git -C $(BCHLIB) describe --tags 2>/dev/null)
BCHLIB_CFLAGS := -I$(BCHLIB)/src -DBCHLIB_VERSION_STR='"$(BCHLIB_VER)"'

BINS := aff3ct_bch_bench m4ri_genmatrix_bench bchlib_bch_bench

.PHONY: all clean
all: $(BINS)

aff3ct_bch_bench: aff3ct_bch_bench.cpp
	$(CXX) $(CXXSTD) $(OPT) $(AFF3CT_CXXFLAGS) -o $@ $< $(AFF3CT_LIBS)

m4ri_genmatrix_bench: m4ri_genmatrix_bench.c
	$(CC) $(CSTD) $(OPT) $(M4RI_CFLAGS) -o $@ $< $(M4RI_LIBS)

bchlib_bch_bench: bchlib_bch_bench.c
	$(CC) $(CSTD) $(OPT) $(BCHLIB_CFLAGS) -o $@ $< $(BCHLIB)/src/bch.c

clean:
	rm -f $(BINS)
