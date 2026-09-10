# C external-baseline harnesses for jit:6fb89a3c.

EXT ?= $(shell git -C $(CURDIR) rev-parse --show-toplevel)/.agents/ext/6fb89a3c
PREFIX := $(EXT)/prefix
CC ?= gcc
OPT ?= -O3 -march=native
CSTD ?= -std=c11
WARN := -Wall -Wextra
COMMON := json_min.c json_min.h harness_common.h

M4RI_VER := $(shell PKG_CONFIG_PATH=$(PREFIX)/lib/pkgconfig pkg-config --modversion m4ri 2>/dev/null)
M4RI_CFLAGS := -I$(PREFIX)/include -DM4RI_VERSION_STR='"$(M4RI_VER)"'
M4RI_LIBS := -L$(PREFIX)/lib -lm4ri -lm -Wl,-rpath,$(PREFIX)/lib

BITSHUFFLE_CFLAGS := -I$(EXT)/bitshuffle/src
BITSHUFFLE_LIBS := $(PREFIX)/lib/libbitshuffle_min.a
# The full ISA-L build needs NASM (unavailable on this host); this harness
# links only ISA-L's portable C reference (raid/raid_base.c, xor_gen_base),
# not the NASM-built AVX/AVX512 multi-binary dispatcher xor_gen. See
# fetch-build.sh and findings.md for the recorded limitation.
ISAL_CFLAGS := -I$(EXT)/isa-l/include
ISAL_LIBS := $(PREFIX)/lib/libisal_base.a

BINS := m4ri_transpose_arm m4ri_genmatrix_arm bitshuffle_transpose_arm isal_xor_arm

.PHONY: all clean
all: $(BINS)

m4ri_transpose_arm: m4ri_transpose_arm.c $(COMMON)
	$(CC) $(CSTD) $(OPT) $(WARN) $(M4RI_CFLAGS) -o $@ $< json_min.c $(M4RI_LIBS)

m4ri_genmatrix_arm: m4ri_genmatrix_arm.c $(COMMON)
	$(CC) $(CSTD) $(OPT) $(WARN) $(M4RI_CFLAGS) -o $@ $< json_min.c $(M4RI_LIBS)

bitshuffle_transpose_arm: bitshuffle_transpose_arm.c $(COMMON)
	$(CC) $(CSTD) $(OPT) $(WARN) $(BITSHUFFLE_CFLAGS) -o $@ $< json_min.c $(BITSHUFFLE_LIBS)

isal_xor_arm: isal_xor_arm.c $(COMMON)
	$(CC) $(CSTD) $(OPT) $(WARN) $(ISAL_CFLAGS) -o $@ $< json_min.c $(ISAL_LIBS)

clean:
	rm -f $(BINS)
