#!/bin/sh
# RUSTC_WRAPPER for cargo-ci.sh: the sccache daemon a compile may spawn outlives
# the run, so it must not inherit the run's private TMPDIR, which the run deletes.
TMPDIR=$CARGO_CI_SYSTEM_TMPDIR exec sccache "$@"
