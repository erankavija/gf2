# Byte-field external-baseline harness (jit:6c6b09b1).
#
# `fetch-build.sh` builds M4RI, M4RIE, GF-Complete and ISA-L from the pins it
# carries into $(EXT)/prefix; this Makefile compiles the uniform C shim over
# them and the standalone conformance binary that validates every adapter
# before any timing happens. The Rust arms link the archive this produces.
#
# Build flags mirror benchmarks/Containerfile (-O3 -march=native) so the
# reference code and the gf2 release build see the same host ISA.

PRIMARY := $(patsubst %/.git,%,$(abspath $(shell git rev-parse --git-common-dir)))
EXT    ?= $(PRIMARY)/.agents/ext/6c6b09b1
PREFIX ?= $(EXT)/prefix

CC  ?= gcc
OPT ?= -O3 -march=native
STD ?= -std=c11

PKG := PKG_CONFIG_PATH=$(PREFIX)/lib/pkgconfig pkg-config

# Library identities are read from the installed packages rather than
# restated here, so the compiled-in strings describe the build actually
# linked. GF-Complete ships no pkg-config file, so its identity is the
# commit `fetch-build.sh` pins and verifies with `git rev-parse`.
M4RIE_VER      := $(shell $(PKG) --modversion m4rie)
M4RI_VER       := $(shell $(PKG) --modversion m4ri)
ISAL_VER       := $(shell $(PKG) --modversion libisal)
GFCOMPLETE_VER := $(shell git -C $(EXT)/gf-complete rev-parse --short=12 HEAD 2>/dev/null)

VERSION_DEFS := -DM4RIE_VERSION_STR='"$(M4RIE_VER)"' \
                -DM4RI_VERSION_STR='"$(M4RI_VER)"' \
                -DGFCOMPLETE_VERSION_STR='"ceph/gf-complete@$(GFCOMPLETE_VER)"'

CFLAGS  := $(STD) $(OPT) -fPIC -Wall -Wextra -I$(PREFIX)/include $(VERSION_DEFS)
LDLIBS  := -L$(PREFIX)/lib -lisal -lgf_complete -lm4rie -lm4ri -lm \
           -Wl,-rpath,$(PREFIX)/lib

.PHONY: all clean identities
all: libbytefieldext.a byte_field_conformance backend_provenance

libbytefieldext.a: byte_field_ext.o
	$(AR) rcs $@ $^

byte_field_ext.o: byte_field_ext.c byte_field_ext.h
	$(CC) $(CFLAGS) -c $< -o $@

byte_field_conformance: byte_field_conformance.c libbytefieldext.a byte_field_ext.h
	$(CC) $(CFLAGS) $< libbytefieldext.a $(LDLIBS) -o $@

# `-rdynamic` puts this program's symbols in the global dynamic table, which
# is what lets its M4RIE wrappers answer the library's own PLT calls and so
# observe the recursion M4RIE takes. `-ldl` is for dladdr and dlsym.
backend_provenance: backend_provenance.c
	$(CC) $(CFLAGS) -rdynamic $< $(LDLIBS) -ldl -o $@

# Prints the library identities this build links, for the survey record.
identities:
	@echo "m4ri=$(M4RI_VER)"
	@echo "m4rie=$(M4RIE_VER)"
	@echo "isa-l=$(ISAL_VER)"
	@echo "gf-complete=ceph/gf-complete@$(GFCOMPLETE_VER)"
	@echo "cc=$(shell $(CC) --version | head -1)"
	@echo "cflags=$(CFLAGS)"

clean:
	rm -f byte_field_ext.o libbytefieldext.a byte_field_conformance backend_provenance
