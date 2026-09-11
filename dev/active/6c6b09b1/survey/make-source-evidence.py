#!/usr/bin/env python3
"""Write `source-evidence.json`: every code claim the byte-field survey relies on.

Usage: dev/active/6c6b09b1/survey/make-source-evidence.py   (from the repo root)

Each claim names a project, its pinned revision, a path, a line and the
verbatim text of that line. The script reads the sources, locates the line by
an exact substring and its occurrence index, and refuses to write a claim
whose text has moved, so the committed evidence is regenerated rather than
transcribed. External paths are relative to the survey's external staging
(`fetch-build.sh`), whose pins `stage-externals.sh` re-verifies.

One entry is a derived scan rather than a single line: every GF(2^8)
`Gf2mWideConfig` implementation (`const M: usize = 8;`) under `crates/*/src`,
with the `#[cfg(test)]` line that opens the test module containing it. The
script fails if any such implementation sits outside a test module, which is
what the survey's claim that gf2 ships no GF(2^8) compile-time field rests on.
"""

import json
import os
import pathlib
import re
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path(subprocess.check_output(
    ["git", "-C", HERE, "rev-parse", "--show-toplevel"], text=True).strip())
PRIMARY = pathlib.Path(subprocess.check_output(
    ["git", "-C", HERE, "rev-parse", "--path-format=absolute", "--git-common-dir"],
    text=True).strip()).parent
EXT = pathlib.Path(os.environ.get("GF2_SURVEY_SRC", PRIMARY / ".agents/ext/6c6b09b1"))
OUTPUT = HERE / "source-evidence.json"

PROJECTS = {
    "gf2": {
        "root": REPO,
        "revision": subprocess.check_output(["git", "-C", REPO, "rev-parse", "HEAD"],
                                            text=True).strip(),
        "revision_kind": "git commit (navigation only; receipts pin these files by content)",
    },
    "isa-l": {"root": EXT / "isa-l", "revision": "7c3479e0a9dac17f448603ec1ad64c7c625f530c",
              "revision_kind": "git commit (tag v2.32.1)"},
    "gf-complete": {"root": EXT / "gf-complete",
                    "revision": "a6862d10c9db467148f20eef2c6445ac9afd94d8",
                    "revision_kind": "git commit (ceph/gf-complete master)"},
    "m4rie": {"root": EXT, "revision": "release tarball m4rie-20250128.tar.gz sha256 "
              "96f1adafd50e6a0b51dc3aa1cb56cb6c1361ae7c10d97dc35c3fa70822a55bd7",
              "revision_kind": "release tarball digest; m4rie-src/ is the unpacked tarball"},
    "m4ri": {"root": EXT, "revision": "release tarball m4ri-20260122.tar.gz sha256 "
             "7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404",
             "revision_kind": "release tarball digest; m4ri-src/ is the unpacked tarball, "
             "prefix/ the configured install"},
}

G = "crates/gf2-core/src"
# (claim id, project, path, exact substring, occurrence index, why it matters)
CLAIMS = [
    # Which polynomial each gf2 GF(2^8) type uses.
    ("gf2-gf256-constructor", "gf2", f"{G}/gf2m/field.rs", "pub fn gf256() -> Self {", 0,
     "The runtime GF(2^8) field every Gf2mElement cell measures."),
    ("gf2-gf256-polynomial", "gf2", f"{G}/gf2m/field.rs", "Gf2mField::new(8, 0b100011101)", 0,
     "gf256() builds GF(2^8) modulo 0x11D = x^8+x^4+x^3+x^2+1, the polynomial ISA-L compiles "
     "in, so the Gf2mElement cells and ISA-L share one field."),
    ("gf2-gf256-doc-polynomial", "gf2", f"{G}/gf2m/field.rs",
     "/// Creates a GF(2^8) field with standard primitive polynomial x^8 + x^4 + x^3 + x + 1.", 0,
     "The rustdoc of gf256() names x^8+x^4+x^3+x+1 (0x11B), which is not the polynomial the "
     "next lines build: a documentation defect reported to the lead, not a second field."),
    ("gf2-gf256-doc-aes", "gf2", f"{G}/gf2m/field.rs",
     "/// This is the standard field used in AES and many error-correcting codes.", 0,
     "The same rustdoc attributes the 0x11D field to a use of 0x11B; part of the same "
     "documentation defect."),
    ("gf2-catalogue-degree-8", "gf2", f"{G}/primitive_polys.rs",
     "8 => Some(0b100011101),          // x^8 + x^4 + x^3 + x^2 + 1 (primitive trinomial)", 0,
     "The primitive-polynomial catalogue's GF(2^8) entry is 0x11D as well. Its comment calls "
     "the five-term polynomial a trinomial, a second documentation defect."),
    ("gf2-wide-config-trait", "gf2", f"{G}/gf2m/wide_config.rs",
     "pub trait Gf2mWideConfig<const N: usize>: 'static {", 0,
     "A Gf2mWide consumer supplies its own polynomial through this trait; the survey's "
     "Gf256x11d supplies 0x11D (gf2-side/src/workload.rs)."),
    ("gf2-wide-test-config-0x11b", "gf2", f"{G}/field/matrix.rs",
     "const MODULUS: [u64; 1] = [0x1B];", 0,
     "gf2's own GF(2^8) Gf2mWide test configurations use 0x11B (see the scan entry for all "
     "of them); none is part of the library API."),
    ("gf2-wide-bench-config-0x11b", "gf2", "crates/gf2-core/benches/field_matrix_gemm.rs",
     "const MODULUS: [u64; 1] = [0x1B];", 0,
     "gf2's GF(2^8) Gf2mWide gemm benchmark also uses 0x11B; the survey's 0x11D configuration "
     "runs the same code with a different reduction constant."),
    # Representations.
    ("gf2-element-value", "gf2", f"{G}/gf2m/field.rs", "    value: V,", 0,
     "A Gf2mElement stores its value in a u64 (V = u64) ..."),
    ("gf2-element-params", "gf2", f"{G}/gf2m/field.rs", "    params: Arc<FieldParams_<V>>,", 1,
     "... beside a reference-counted handle on its field, so FieldVec<Gf2mElement> spends 16 "
     "bytes per GF(2^8) element and clones an Arc per element it creates."),
    ("gf2-element-alias", "gf2", f"{G}/gf2m/field.rs",
     "pub type Gf2mElement = Gf2mElement_<u64>;", 0,
     "The element type the survey names Gf2mElement."),
    ("gf2-wide-words", "gf2", f"{G}/gf2m/wide.rs", "    words: [u64; N],", 0,
     "Gf2mWide<1, _> stores one u64 per element: 8 bytes per GF(2^8) element."),
    # FieldVec::axpy.
    ("gf2-axpy-simd-hook", "gf2", f"{G}/field/vec.rs",
     "if F::try_simd_axpy(self.data.as_mut_slice(), a, rhs.data.as_slice()) {", 0,
     "FieldVec::axpy tries a field-specific kernel first ..."),
    ("gf2-axpy-default-false", "gf2", f"{G}/field/traits.rs",
     "fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {", 0,
     "... whose default implementation returns false; only Fp<P> overrides it (next claim), so "
     "both GF(2^8) representations take the element loop."),
    ("gf2-axpy-fp-override", "gf2", f"{G}/gfp/mod.rs",
     "fn try_simd_axpy(y: &mut [Self], a: &Self, x: &[Self]) -> bool {", 0,
     "The only override of try_simd_axpy in gf2-core."),
    ("gf2-axpy-element-loop", "gf2", f"{G}/field/vec.rs",
     "*y += a.clone() * x.clone();", 0,
     "The element loop FieldVec::axpy runs for Gf2mElement and Gf2mWide: one field multiply "
     "and one add per element, with the coefficient reused across the region."),
    # field::matrix::gemm.
    ("gf2-gemm-entry", "gf2", f"{G}/field/matrix.rs",
     "pub fn gemm<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {", 0,
     "The public dense product both square and generator-encode gf2 cells call."),
    ("gf2-gemm-transpose", "gf2", f"{G}/field/matrix.rs", "let b_t = b.transpose();", 0,
     "Every gemm call transposes B into a fresh matrix before multiplying."),
    ("gf2-gemm-whole-hook", "gf2", f"{G}/field/matrix.rs", "if F::try_simd_gemm_classical(", 0,
     "gemm first offers the whole product to a field-specific kernel ..."),
    ("gf2-gemm-per-cell-hook", "gf2", f"{G}/field/matrix.rs",
     "if let Some(value) = F::try_gf2m_u64_batch_dot_product(", 0,
     "... and otherwise computes each output cell as a batched dot product."),
    ("gf2-wide-gemm-kernel", "gf2", f"{G}/gf2m/wide.rs",
     "if let Some(fns) = crate::simd::maybe_gf2m_gemm() {", 0,
     "Gf2mWide<1, _> with m = 8 takes the whole product through the dispatched GEMM kernel "
     "when the host has it (the gf2 arm records the route it observes) ..."),
    ("gf2-wide-gemm-fallback", "gf2", f"{G}/gf2m/wide.rs",
     "Self::scalar_panelized_gemm_fallback_inline(a, b_t, m, k, n, out);", 0,
     "... and its scalar panelized GEMM otherwise, never the per-cell path."),
    ("gf2-element-batch-dot", "gf2", f"{G}/gf2m/field.rs",
     "crate::gf2m::batch::batch_mul_raw(", 0,
     "Gf2mElement answers the per-cell hook by widening the row and column into u64 scratch and "
     "calling the batch multiply, then XOR-folding the products."),
    # gf2m::batch::batch_mul.
    ("gf2-batch-mul-entry", "gf2", f"{G}/gf2m/batch.rs",
     "pub fn batch_mul(field: &Gf2mField, a: &[u64], b: &[u64], out: &mut [u64]) {", 0,
     "The pairwise gf2 arm: distinct operand pairs on u64 lanes, one element per lane."),
    ("gf2-batch-mul-dispatch", "gf2", f"{G}/gf2m/batch.rs",
     "if let Some(fns) = crate::simd::maybe_gf2m_batch() {", 0,
     "batch_mul reaches the dispatched batch kernel for m = 8 when the host has it."),
    ("gf2-simd-feature", "gf2", "crates/gf2-core/Cargo.toml", "simd = []", 0,
     "The gf2 arm enables this feature, which gates the runtime kernel dispatch above."),
    # ISA-L.
    ("isal-polynomial", "isa-l", "erasure_code/ec_base.c",
     "unsigned char c2 = (c << 1) ^ ((c & 0x80) ? 0x1d : 0);   // Mult by GF{2}", 0,
     "ISA-L's table preparation reduces with 0x1D, i.e. GF(2^8) modulo 0x11D, compiled in; "
     "the shim refuses any other polynomial for ISA-L."),
    ("isal-init-tables", "isa-l", "include/erasure_code.h",
     "ec_init_tables(int k, int rows, unsigned char *a, unsigned char *gftbls);", 0,
     "The separate table preparation every ISA-L region and encode call consumes."),
    ("isal-vect-mad", "isa-l", "include/erasure_code.h",
     "gf_vect_mad(int len, int vec, int vec_i, unsigned char *gftbls, unsigned char *src,", 0,
     "The region multiply-accumulate mapped to FieldVec::axpy (vec = 1, vec_i = 0)."),
    ("isal-vect-mad-length", "isa-l", "include/erasure_code.h",
     " * @param len    Length of each vector in bytes. Must be >= 64.", 0,
     "gf_vect_mad requires regions of at least 64 bytes; every region cell is 4 KiB or more."),
    ("isal-dot-prod-doc", "isa-l", "include/erasure_code.h",
     " * Does a GF(2^8) dot product across each byte of the input array and a constant", 1,
     "gf_vect_dot_prod produces a region (one dot product per byte position across source "
     "regions), not the single field element FieldVec::dot_product returns."),
    ("isal-encode", "isa-l", "include/erasure_code.h",
     "ec_encode_data(int len, int k, int rows, unsigned char *gftbls, unsigned char **data,", 0,
     "The generator-matrix region encode mapped to gemm at the encode shape."),
    ("isal-gf-mul", "isa-l", "include/erasure_code.h", "gf_mul(unsigned char a, unsigned char b);", 0,
     "ISA-L's only arbitrary-pair multiply is this exported single-element function."),
    ("isal-gf-mul-tables", "isa-l", "erasure_code/ec_base.c",
     "return gff_base[(i = gflog_base[a] + gflog_base[b]) > 254 ? i - 255 : i];", 0,
     "gf_mul is a log/antilog table lookup unless GF_LARGE_TABLES is defined, which the pinned "
     "build does not do."),
    ("isal-license-clause-3", "isa-l", "LICENSE",
     "* Neither the name of Intel Corporation nor the names of its", 0,
     "The third clause of the BSD-3-Clause licence ISA-L carries."),
    # GF-Complete.
    ("gfcomplete-default-polynomial", "gf-complete", "src/gf_w8.c", "h->prim_poly = 0x11d;", 0,
     "GF-Complete's default w = 8 polynomial is 0x11D; the shim passes the cell's polynomial "
     "to gf_init_hard explicitly."),
    ("gfcomplete-init-hard", "gf-complete", "include/gf_complete.h",
     "extern int gf_init_hard(GFP gf, ", 0,
     "The configuration entry point the shim calls with the polynomial and backend."),
    ("gfcomplete-region", "gf-complete", "include/gf_complete.h",
     "gf_region      multiply_region;", 0,
     "The region multiply(-accumulate) mapped to FieldVec::axpy (add = 1)."),
    ("gfcomplete-default-ssse3", "gf-complete", "src/gf_w8.c",
     "(gf_cpu_supports_intel_ssse3 || gf_cpu_supports_arm_neon)) {", 0,
     "The default configuration selects its SSSE3 region tables when the CPU reports SSSE3."),
    ("gfcomplete-license-clause-3", "gf-complete", "License.txt",
     " - Neither the name of the University of Tennessee nor the names of its", 0,
     "The third clause of the BSD-3-Clause licence GF-Complete carries."),
    # M4RIE.
    ("m4rie-init", "m4rie", "m4rie-src/m4rie/gf2e.h", "gf2e *gf2e_init(const word minpoly);", 0,
     "M4RIE takes the field polynomial as a parameter, so 0x11D (and 0x11B) are available."),
    ("m4rie-mul-table", "m4rie", "m4rie-src/m4rie/gf2e.c",
     "ff->_mul[i] = (word *)m4ri_mm_calloc(order, sizeof(word));", 0,
     "For degree <= 8 gf2e_init builds a full product table of 64-bit words: 256 x 256 x 8 "
     "bytes for GF(2^8), the table M4RIE's pairwise arm walks."),
    ("m4rie-word", "m4ri", "prefix/include/m4ri/misc.h", "typedef uint64_t word;", 0,
     "The table entry type."),
    ("m4rie-scalar-mul", "m4rie", "m4rie-src/m4rie/gf2e.h",
     "static inline word gf2e_mul(const gf2e *ff, const word a, const word b) {", 0,
     "M4RIE's only arbitrary-pair multiply: an inline function of its public header."),
    ("m4rie-mzed-mul", "m4rie", "m4rie-src/m4rie/mzed.h",
     "mzed_t *mzed_mul(mzed_t *C, const mzed_t *A, const mzed_t *B);", 0,
     "The dense product mapped to gemm at the square and encode shapes."),
    ("m4rie-row-axpy", "m4rie", "m4rie-src/m4rie/mzed.h",
     "void mzed_add_multiple_of_row(mzed_t *A, rci_t ar, const mzed_t *B, rci_t br, word x, rci_t start_col);", 0,
     "M4RIE's region multiply-accumulate exists only on matrix rows; the axpy arm uses one-row "
     "matrices and pays their pack and unpack."),
    ("m4rie-conversion-scope", "m4rie", "m4rie-src/m4rie/conversion.h",
     " * \\brief Conversion between mzed_t and mzd_slice_t", 0,
     "M4RIE's conversion module changes the storage layout (packed to bitsliced) within one "
     "field; it offers no change of polynomial basis."),
    ("m4rie-license-header", "m4rie", "m4rie-src/m4rie/gf2e.h",
     "*  Distributed under the terms of the GNU General Public License (GPL)", 0,
     "M4RIE's headers license it under the GPL ..."),
    ("m4rie-license-version", "m4rie", "m4rie-src/m4rie/gf2e.h", "*  version 2 or higher.", 0,
     "... version 2 or higher: GPL-2.0-or-later."),
    ("m4rie-license-readme", "m4rie", "m4rie-src/README.md",
     "M4RIE is available under the General Public License Version 2 or later (GPLv2+).", 0,
     "The README states GPLv2+; COPYING carries the GPLv2 text."),
    # M4RI.
    ("m4ri-license-header", "m4ri", "m4ri-src/m4ri/mzd.h",
     "*  Distributed under the terms of the GNU General Public License (GPL)", 0,
     "M4RI's headers license it under the GPL ..."),
    ("m4ri-license-version", "m4ri", "m4ri-src/m4ri/mzd.h", "*  version 2 or higher.", 0,
     "... version 2 or higher: GPL-2.0-or-later, the label sibling survey 6fb89a3c records for "
     "the same M4RI release."),
    ("m4ri-no-openmp", "m4ri", "prefix/include/m4ri/m4ri_config.h",
     "#define __M4RI_HAVE_OPENMP\t\t0", 0,
     "The pinned M4RI build has no OpenMP, so M4RIE's products run on one thread."),
]


def locate(project, path, needle, occurrence):
    text = (PROJECTS[project]["root"] / path).read_text(encoding="utf-8", errors="replace")
    seen = 0
    for number, line in enumerate(text.splitlines(), start=1):
        if needle in line:
            if seen == occurrence:
                return number, line
            seen += 1
    raise SystemExit(f"{project}:{path}: {needle!r} occurrence {occurrence} not found")


def wide_config_scan():
    """Every GF(2^8) Gf2mWideConfig under crates/*/src and its test module."""
    found = []
    for source in sorted((REPO / "crates").glob("*/src/**/*.rs")):
        lines = source.read_text(encoding="utf-8").splitlines()
        for number, line in enumerate(lines, start=1):
            if line.strip() != "const M: usize = 8;":
                continue
            opener = next((n for n in range(number - 1, 0, -1)
                           if lines[n - 1].strip() == "#[cfg(test)]"), None)
            # The `mod` line follows the attribute, after any further attributes.
            module_index = opener
            while (module_index is not None and module_index < number - 1
                   and lines[module_index].strip().startswith("#[")):
                module_index += 1
            module = None if module_index is None else lines[module_index]
            # The module stays open from its `{` to the configuration when the
            # brace depth counted over the lines between never returns to zero.
            depth, enclosed = 0, module is not None
            if enclosed:
                for text in lines[module_index:number - 1]:
                    depth += text.count("{") - text.count("}")
                    if depth <= 0:
                        enclosed = False
                        break
            if not enclosed or not re.match(r"\s*mod \w+\s*\{", module):
                raise SystemExit(f"{source.relative_to(REPO)}:{number}: GF(2^8) "
                                 "Gf2mWideConfig outside a #[cfg(test)] module")
            found.append({"path": str(source.relative_to(REPO)), "line": number,
                          "text": line, "test_module_line": module_index + 1,
                          "test_module_text": module})
    return found


def main():
    claims = []
    for claim, project, path, needle, occurrence, why in CLAIMS:
        number, line = locate(project, path, needle, occurrence)
        claims.append({"claim": claim, "project": project,
                       "revision": PROJECTS[project]["revision"], "path": path,
                       "line": number, "text": line, "why": why})
    evidence = {
        "schema": "6c6b09b1-source-evidence-v1",
        "projects": {name: {"revision": spec["revision"], "revision_kind": spec["revision_kind"]}
                     for name, spec in PROJECTS.items()},
        "claims": claims,
        "gf2_gf256_wide_config_scan": {
            "method": ("every line `const M: usize = 8;` under crates/*/src, with the "
                       "`#[cfg(test)]` attribute and `mod` line of the nearest enclosing test "
                       "module above it; the generator fails on any implementation outside one"),
            "found": wide_config_scan(),
        },
    }
    with open(OUTPUT, "w") as handle:
        json.dump(evidence, handle, indent=2)
        handle.write("\n")
    print(f"{OUTPUT.relative_to(REPO)}: {len(claims)} claims, "
          f"{len(evidence['gf2_gf256_wide_config_scan']['found'])} scanned configurations")


if __name__ == "__main__":
    main()
