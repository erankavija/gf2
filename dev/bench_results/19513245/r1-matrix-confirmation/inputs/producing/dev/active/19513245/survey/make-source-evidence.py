#!/usr/bin/env python3
"""Write the assessment's code claims with verbatim source lines (jit:19513245).

Every code claim the findings make is a row here: the file, the line, a
fragment the line must contain, and why the line matters for the consumer
question. The script reads each line from the working tree, refuses a claim
whose line lacks its fragment, and records the verbatim text, so each claim is
checkable at its location and the findings carry no line numbers of their own.

The `path` of a claim is repository-relative. The gf2 revision is navigation
metadata; the campaigns' producing manifests pin the source bytes.

Usage: make-source-evidence.py > dev/active/19513245/survey/source-evidence.json
"""

import json
import subprocess
import sys

VEC = "crates/gf2-core/src/field/vec.rs"
TRAITS = "crates/gf2-core/src/field/traits.rs"
MATRIX = "crates/gf2-core/src/field/matrix.rs"
FIELD = "crates/gf2-core/src/gf2m/field.rs"
WIDE = "crates/gf2-core/src/gf2m/wide.rs"
BATCH = "crates/gf2-core/src/gf2m/batch.rs"
GFP = "crates/gf2-core/src/gfp/mod.rs"
SURVEY = "dev/active/19513245/survey/consumer/src"
PROTO_LIB = f"{SURVEY}/lib.rs"
PROTO_TABLE = f"{SURVEY}/table.rs"

# (claim id, path, line, fragment, why)
CLAIMS = [
    # --- the vector consumer and the accelerated path it reaches ------------
    ("axpy-signature", VEC, 663, "pub fn axpy(&mut self, a: &F, rhs: &Self) {",
     "the vector consumer: one coefficient over the whole vector, destination &mut and source &, "
     "so the borrows forbid an aliasing call and fix the overlap contract"),
    ("axpy-hook", VEC, 675, "if F::try_simd_axpy(self.data.as_mut_slice(), a, rhs.data.as_slice()) {",
     "the one accelerated path FieldVec::axpy consults"),
    ("axpy-scalar-loop", VEC, 679, "*y += a.clone() * x.clone();",
     "what runs when the hook declines: one element clone, one element multiply and one "
     "add-assign per element"),
    ("axpy-hook-default-false", TRAITS, 610,
     "fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {",
     "the hook's default returns false, so a field that does not override it takes the scalar loop"),
    ("axpy-hook-only-fp", GFP, 811, "fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {",
     "Fp<P> is the only override of the axpy hook in the workspace, so both GF(2^8) "
     "representations take the scalar loop"),
    # --- the element representations ---------------------------------------
    ("element-arc-handle", FIELD, 208, "params: Arc<FieldParams_<V>>,",
     "a Gf2mElement carries a reference-counted handle on the field parameters beside its value"),
    ("element-clones-handle", FIELD, 368, "params: Arc::clone(&self.params),",
     "every element Gf2mField::element builds clones that handle, so writing an element back "
     "costs one atomic increment and one decrement"),
    ("gf256-builds-no-tables", FIELD, 303, "log_table: None,",
     "Gf2mField::new builds no log or antilog table, so Gf2mField::gf256() has none and "
     "with_tables is the only way to get them"),
    ("gf256-polynomial", FIELD, 850, "Gf2mField::new(8, 0b100011101)",
     "Gf2mField::gf256() builds GF(2^8) modulo 0b100011101 = 0x11D"),
    ("gf256-doc-polynomial", FIELD, 837,
     "Creates a GF(2^8) field with standard primitive polynomial x^8 + x^4 + x^3 + x + 1.",
     "the rustdoc of gf256() names x^8+x^4+x^3+x+1, which is 0x11B, while the code builds 0x11D: "
     "the documentation defect tracked as 835f34f0"),
    ("gf256-doc-aes", FIELD, 839, "This is the standard field used in AES and many error-correcting codes.",
     "the same rustdoc calls the field the one used in AES, which is the 0x11B field, not 0x11D"),
    ("element-mul-clmul-route", FIELD, 1154, "if let (Some(clmul_barrett_fn), Some(barrett)) = (",
     "with no log tables present, the element multiply reaches the carry-less-multiply and Barrett "
     "kernel through a function pointer: one indirect call per element multiplication"),
    ("wide-mul-allocates", WIDE, 930, "let mut product = vec![0u64; 2 * N];",
     "Gf2mWide::mul_ref allocates its unreduced product on the heap, so one Gf2mWide element "
     "multiplication allocates"),
    # --- the matrix consumers ----------------------------------------------
    ("gemm-fresh-output", MATRIX, 3003, "let mut out = FieldMatrix {",
     "field::matrix::gemm allocates its product matrix in every call"),
    ("gemm-transposes", MATRIX, 3032, "let b_t = b.transpose();",
     "gemm transposes the right operand in every call before the inner loop"),
    ("gemm-whole-hook", MATRIX, 3040, "if F::try_simd_gemm_classical(",
     "the whole-product hook gemm consults first"),
    ("gemm-per-cell-hook", MATRIX, 3082, "if let Some(value) = F::try_gf2m_u64_batch_dot_product(",
     "when the whole-product hook declines, gemm calls the GF(2^m) batch dot product once per "
     "output cell"),
    ("wide-whole-gemm-override", WIDE, 1677, "fn try_simd_gemm_classical(",
     "Gf2mWide<1, _> overrides the whole-product hook, so the wide representation takes the "
     "whole-product kernel and never the per-cell path"),
    ("wide-gemm-flattens", WIDE, 1698, "let mut a_flat: Vec<u64> = Vec::with_capacity(m * k);",
     "the wide whole-product path copies both operands into flat u64 buffers and the result out "
     "of a third, allocating three buffers per call"),
    ("element-batch-dot-override", FIELD, 1405, "fn try_gf2m_u64_batch_dot_product(",
     "Gf2mElement overrides the batch dot product, which is the accelerated path the element "
     "representation's gemm reaches"),
    ("matvec-hook", MATRIX, 1460, "&& F::try_simd_matvec(",
     "the one accelerated path FieldMatrix::matvec consults"),
    ("matvec-hook-default-false", TRAITS, 667,
     "fn try_simd_matvec(a: &[Self], x: &[Self], m: usize, k: usize, out: &mut [Self]) -> bool {",
     "that hook's default returns false and only Fp<P> overrides it, so matvec over GF(2^8) "
     "never reaches a batch kernel"),
    ("matvec-scalar-rows", MATRIX, 1478,
     "crate::field::vec::dot_product_slices(row, x.as_slice(), &zero),",
     "matvec falls through to one dot product per row"),
    ("dot-product-scalar-chain", VEC, 569, "let mut acc = a[0].mul_product_sum_wide(&b[0]);",
     "that dot product is a scalar multiply-and-accumulate chain for GF(2^m), where the delayed "
     "reduction bound is unlimited; it calls no batch kernel"),
    # --- the arbitrary-multiplication control ------------------------------
    ("batch-mul-raw-internal", BATCH, 103,
     "pub(crate) fn batch_mul_raw(m: usize, primitive_poly: u64, a: &[u64], b: &[u64], out: &mut [u64]) {",
     "the raw batch multiply is crate-internal, so only gf2-core's own consumers can reach it "
     "without the field context"),
    ("batch-mul-dispatch", BATCH, 115, "if let Some(fns) = crate::simd::maybe_gf2m_batch() {",
     "gf2m::batch::batch_mul dispatches to the carry-less-multiply batch kernel at run time when "
     "the simd feature is built and the host has it"),
    # --- the prototype ------------------------------------------------------
    ("prototype-table-recurrence", PROTO_TABLE, 46, "entries[value] = if value % 2 == 0 {",
     "the prototype builds a coefficient table by the doubling recurrence over the field's own "
     "reduction polynomial: 256 operations, no gf2-core arithmetic"),
    ("prototype-region-kernel", PROTO_LIB, 54,
     "pub fn axpy_region(y: &mut [u8], table: &CoefficientTable, x: &[u8]) {",
     "the prototype's region-shaped route: one indexed load and one XOR per element, with the "
     "same destination-&mut and source-& signature the consumer has"),
    ("prototype-in-place-write", PROTO_LIB, 84, "*target = field.element(value);",
     "the prototype's vector-shaped route writes back through the representation the vector "
     "already stores, so it converts nothing and inherits that representation's write cost"),
    ("prototype-gemm-reuse", PROTO_LIB, 116, "let row = table.row(a[i * k + p]);",
     "the prototype's product makes every left-operand element the reused coefficient of one "
     "whole right-operand row, which is why a product is k region multiply-accumulates per row"),
    ("prototype-pairwise-no-reuse", PROTO_LIB, 163, "*target = table.get(*left, *right);",
     "the control's route reuses no coefficient, so it reaches the full 64 KiB table and can "
     "amortise no coefficient table"),
]


def main():
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()
    seen = set()
    claims = []
    for claim_id, path, line, fragment, why in CLAIMS:
        if claim_id in seen:
            raise SystemExit(f"duplicate claim id {claim_id}")
        seen.add(claim_id)
        with open(path, encoding="utf-8") as handle:
            lines = handle.read().splitlines()
        if line > len(lines):
            raise SystemExit(f"{path} has {len(lines)} lines; claim {claim_id} names {line}")
        text = lines[line - 1]
        if fragment not in text:
            raise SystemExit(
                f"{claim_id}: {path}:{line} does not contain {fragment!r}: {text!r}")
        claims.append({"id": claim_id, "project": "gf2", "commit": revision, "path": path,
                       "line": line, "text": text.strip(), "why": why})
    json.dump({"schema": "bytefield-consumer-source-evidence-v1",
               "note": ("The gf2 revision is navigation metadata; each campaign's producing "
                        "manifest pins the source bytes it measured. Claims whose path is under "
                        "dev/active/19513245/survey are the prototype's own source."),
               "claims": claims}, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
