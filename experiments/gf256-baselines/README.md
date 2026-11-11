# GF(2^8) region and matrix products against ISA-L and M4RIE

[`run.sh`](run.sh) times gf2's byte-region routes, `Gf2mField::gf256_axpy`
and `Gf2mField::gf256_gemm`, and the generic `FieldVec::axpy` and
`FieldMatrix` product over one-byte `Gf256` elements, against ISA-L's `gf_vect_mad` and M4RIE's
`mzed_mul`, `mzed_mul_newton_john` and bitsliced `mzd_slice_mul_karatsuba`,
on one pinned core and the modulus 0x11D. Cells are region multiply-add at
4 KiB, 64 KiB, 1 MiB and 16 MiB and square products at n = 64, 256 and 1024.
Both arms draw their inputs from one splitmix64 stream seeded per cell, run
the same repetitions, and print an FNV-1a checksum of their output, for a
region of one update of the drawn destination, since repeated equal updates
cancel in pairs; the script fails when the checksums of a cell differ. A cell's time is the best
of five batches after one untimed batch; a row reports the byte-region and
generic routes, their ratio, and the fastest reference configuration.

The libraries are built outside the tree and named by
`GF256_BASELINE_PREFIX`, which holds their `include/` and `lib/`.
[`src/main.rs`](src/main.rs) is gf2's arm and [`ext/arm.c`](ext/arm.c) the
references'.
