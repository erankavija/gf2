# Popcount/fused-reduction baseline survey arms (jit:26465e6c).
#
# Builds the external reference drivers from the pinned, git-excluded checkouts
# under .agents/ext/26465e6c. Both binaries use the same native host ISA flags.

# `.agents/ext` is untracked and lives beside the main checkout, not inside
# each worktree; resolve the true repository root via the shared git dir
# rather than a fixed relative depth from $(CURDIR).
REPO_ROOT := $(shell dirname "$$(git -C $(CURDIR) rev-parse --path-format=absolute --git-common-dir)")
EXT ?= $(REPO_ROOT)/.agents/ext/26465e6c
LIBPOPCNT ?= $(EXT)/libpopcnt
MULA ?= $(EXT)/mula-sse-popcount

CC ?= gcc
CXX ?= g++
OPT ?= -O3 -march=native
CFLAGS ?= $(OPT) -Wall -Wextra -std=c11
CXXFLAGS ?= $(OPT) -Wall -Wextra -std=c++17 -include cstdint -include immintrin.h

.PHONY: all clean
all: libpopcnt-arm mula-avx2-harleyseal-arm

libpopcnt-arm: libpopcnt_arm.c wire_common.h splitmix64.h
	$(CC) $(CFLAGS) -I$(LIBPOPCNT) -o $@ libpopcnt_arm.c

mula-avx2-harleyseal-arm: mula_arm.cpp wire_common.h splitmix64.h vendor/mula/popcnt-lookup.cpp vendor/mula/popcnt-avx2-harley-seal.cpp vendor/mula/lookup-decl.h
	$(CXX) $(CXXFLAGS) -include vendor/mula/lookup-decl.h -I$(MULA) -Ivendor/mula -o $@ mula_arm.cpp vendor/mula/popcnt-lookup.cpp vendor/mula/popcnt-avx2-harley-seal.cpp

clean:
	rm -f libpopcnt-arm mula-avx2-harleyseal-arm
