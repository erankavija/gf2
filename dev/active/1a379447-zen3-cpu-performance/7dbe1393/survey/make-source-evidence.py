#!/usr/bin/env python3
"""Freeze the source claims of the ragged-tail attribution and addendum (jit:7dbe1393).

Every claim has an identifier the documents cite, names a file by Cargo
package and package-relative path, a fragment of one line, how often the
fragment occurs in the file and why the claim matters. The script writes
`source-evidence.json` beside itself with the line the first occurrence holds,
the verbatim line, the file digest and the commit that last changed the file.
A fragment that is absent or changes its occurrence count fails the script.

Usage: make-source-evidence.py, with no arguments
"""

import hashlib
import json
import subprocess
import sys

sys.dont_write_bytecode = True

from locate import HERE, ROOT, repository_files  # noqa: E402

CORE, SIMD, SUPPORT, HARNESS = (
    "gf2-core", "gf2-kernels-simd", "tuning-campaign-support", "dense-parity-harness",
)

# (claim id, package, path, fragment, occurrences, why)
CLAIMS = [
    ("matvec-threshold-conservative", CORE, "src/matrix.rs",
     "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;", 1,
     "The conservative value of `bit_matrix.matvec_simd_min_words`: every stride of this "
     "family takes the SIMD lane."),
    ("matvec-selector", CORE, "src/matrix.rs",
     "        match matvec_route(self.stride_words) {", 1,
     "`matvec` selects its lane from the stride alone; this issue leaves the match unchanged."),
    ("matvec-simd-kernel-call", CORE, "src/matrix.rs",
     "            y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);", 1,
     "The SIMD lane calls the bundle's fused count once per row and keeps its low bit."),
    ("matvec-parity-kernel-call", CORE, "src/matrix.rs",
     "            y.push_bit((fns.and_parity_fn)(row, x_words));", 1,
     "The parity-kernel lane has the SIMD lane's row loop and calls the fused parity entry."),
    ("matvec-parity-entry", CORE, "src/matrix.rs",
     "    pub fn matvec_parity_kernel(&self, x: &crate::BitVec) -> crate::BitVec {", 1,
     "The public entry of the candidate lane, which `matvec` does not call."),
    ("scalar-accumulators", CORE, "src/matrix.rs",
     "            acc0 ^= r[0] & x[0];", 1,
     "The scalar lane XOR-folds the intersection in four accumulators and counts nothing "
     "per word."),
    ("scalar-word-count", CORE, "src/matrix.rs",
     "        acc.count_ones() & 1 == 1", 1,
     "The scalar lane takes one word count per row, of the folded word."),
    ("popcnt-kernel-signature", SIMD, "src/x86/avx2.rs",
     "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {", 1,
     "The SIMD lane's kernel; the attribute on the line above it enables `avx2` and not "
     "`popcnt`."),
    ("popcnt-tail-word-count", SIMD, "src/x86/avx2.rs",
     "        total += (*lhs.get_unchecked(j) & *rhs.get_unchecked(j)).count_ones() as u64;", 1,
     "The SIMD lane's kernel counts each word past its last whole vector on its own."),
    ("parity-masked-tail", SIMD, "src/x86/avx2.rs",
     "            _mm256_maskload_epi64(lhs_ptr.add(tail) as *const i64, mask),", 1,
     "The parity kernel folds the words past its last whole vector through one masked load "
     "of each operand."),
    ("parity-fold", SIMD, "src/x86/avx2.rs",
     "    folded.count_ones() & 1 == 1", 1,
     "The parity kernel takes one word count per row, of the folded word."),
    ("parity-bundle-field", SIMD, "src/lib.rs",
     "    pub and_parity_fn: fn(&[u64], &[u64]) -> bool,", 1,
     "The bundle entry of the parity kernel, a comparator no automatic dispatch selects."),
    ("p20-tail-rule", SUPPORT, "src/receipt.rs",
     "                || f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;",
     1,
     "P-20 leaves a confirmatory interval unresolved below twenty expected draws per "
     "bootstrap tail."),
    ("attempt-alpha", SUPPORT, "src/trial_ledger.rs",
     "    Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))", 1,
     "Attempt t of a family spends alpha / (t (t + 1)); the first spends half."),
    ("ragged-cell-arms", HARNESS, "src/campaign.rs",
     "        Question::MatvecRaggedTail => (Route::MatvecA.id(), Route::MatvecLaneParity.id()),",
     1,
     "Every ragged-tail cell takes the selected SIMD lane as baseline and the parity-kernel "
     "lane as candidate."),
    ("ragged-route-call", HARNESS, "src/routes.rs",
     "        return black_box(matrix).matvec_parity_kernel(black_box(vector));", 1,
     "The candidate arm times the public parity-kernel entry behind the baseline's "
     "optimisation barriers."),
    ("ragged-resolution-ceiling", HARNESS, "src/bin/dense-campaign.rs",
     "        Question::MatvecLaneThreshold | Question::MatvecRaggedTail => 0.035,", 1,
     "`verify-confirmation` refuses a ragged-tail confirmation above the family ceiling."),
]


def main():
    if len(sys.argv) != 1:
        raise SystemExit(__doc__)
    claims = []
    for identifier, package, path, fragment, occurrences, why in CLAIMS:
        relative = f"{repository_files.package_directory(ROOT, package)}/{path}"
        data = (ROOT / relative).read_bytes()
        lines = data.decode().splitlines()
        holding = [number for number, line in enumerate(lines, 1) if fragment in line]
        if len(holding) != occurrences:
            raise SystemExit(f"{identifier}: {len(holding)} occurrences in {relative}")
        commit = subprocess.run(
            ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", relative],
            capture_output=True, check=True, text=True,
        ).stdout.strip()
        claims.append({
            "id": identifier,
            "project": "gf2",
            "commit": commit,
            "path": relative,
            "line": holding[0],
            "occurrences": occurrences,
            "verbatim": lines[holding[0] - 1],
            "why": why,
            "sha256": hashlib.sha256(data).hexdigest(),
        })
    record = {"schema": "source-evidence-v1", "issue": "7dbe1393", "claims": claims}
    output = HERE / "source-evidence.json"
    output.write_text(json.dumps(record, indent=1) + "\n")
    print(output.relative_to(ROOT))


if __name__ == "__main__":
    main()
