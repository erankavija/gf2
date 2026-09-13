# Byte-field external-baseline harness (jit:6c6b09b1).
#
# `fetch-build.sh` builds M4RI, M4RIE, GF-Complete and ISA-L from the pins it
# carries; `stage-externals.sh` verifies that prefix against the committed
# digests and copies it into $(EXT)/prefix. This Makefile compiles the
# uniform C shim over that prefix, the standalone conformance binary that
# validates every adapter before any timing happens, and the backend
# provenance probe, all into $(BUILD). The Rust external arm links the
# archive this produces.
#
# Build flags mirror benchmarks/Containerfile (-O3 -march=native) so the
# reference code and the gf2 release build see the same host ISA.

HERE    := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
REPO    := $(abspath $(HERE)/../../../..)
PRIMARY := $(patsubst %/.git,%,$(shell git -C $(REPO) rev-parse --path-format=absolute --git-common-dir))
SRC    ?= $(PRIMARY)/.agents/ext/6c6b09b1
EXT    ?= $(REPO)/target/6c6b09b1-ext
PREFIX ?= $(EXT)/prefix
BUILD  ?= $(REPO)/target/6c6b09b1-survey/c

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
GFCOMPLETE_VER := $(shell git -C $(SRC)/gf-complete rev-parse --short=12 HEAD 2>/dev/null)

VERSION_DEFS := -DM4RIE_VERSION_STR='"$(M4RIE_VER)"' \
                -DM4RI_VERSION_STR='"$(M4RI_VER)"' \
                -DGFCOMPLETE_VERSION_STR='"ceph/gf-complete@$(GFCOMPLETE_VER)"'

CFLAGS  := $(STD) $(OPT) -fPIC -Wall -Wextra -I$(PREFIX)/include $(VERSION_DEFS)
LDLIBS  := -L$(PREFIX)/lib -lisal -lgf_complete -lm4rie -lm4ri -lm \
           -Wl,-rpath,$(PREFIX)/lib

.PHONY: all clean identities
all: $(BUILD)/libbytefieldext.a $(BUILD)/byte_field_conformance $(BUILD)/backend_provenance

$(BUILD):
	mkdir -p $@

$(BUILD)/libbytefieldext.a: $(BUILD)/byte_field_ext.o
	$(AR) rcs $@ $^

$(BUILD)/byte_field_ext.o: $(HERE)/byte_field_ext.c $(HERE)/byte_field_ext.h | $(BUILD)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD)/byte_field_conformance: $(HERE)/byte_field_conformance.c $(BUILD)/libbytefieldext.a $(HERE)/byte_field_ext.h
	$(CC) $(CFLAGS) -I$(HERE) $< $(BUILD)/libbytefieldext.a $(LDLIBS) -o $@

# `-rdynamic` puts this program's symbols in the global dynamic table, which
# is what lets its M4RIE wrappers answer the library's own PLT calls and so
# observe the recursion M4RIE takes. `-ldl` is for dladdr and dlsym.
$(BUILD)/backend_provenance: $(HERE)/backend_provenance.c | $(BUILD)
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
	rm -f $(BUILD)/byte_field_ext.o $(BUILD)/libbytefieldext.a \
	      $(BUILD)/byte_field_conformance $(BUILD)/backend_provenance
