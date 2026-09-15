#!/usr/bin/env python3
"""Emit the source evidence behind the 53c5a8c0 study's code claims.

Usage:
  inspect-sources.py --gf2-commit <sha> --gf2x-source <gf2x checkout> --output <json>

Every claim names a project, a pinned commit, a path and a needle. For gf2 the
script reads the file at that commit with `git show`; for gf2x it reads the
pinned export's working tree after checking its commit. The needle must occur
on exactly the stated occurrence, and the script records the line number and
the verbatim line, so no line number or quotation is transcribed by hand.

The search entries record the uses the study's claims about absent selectors
rest on: every call of the two crossover entry points and every mention of the
field-vector selector family in the gf2 crates at the pinned commit. An absence
claim is only as good as the search that failed to find a counterexample, so
the command and its complete output are recorded rather than summarised.
"""

import argparse
import json
import subprocess
import sys

GF2X_COMMIT = "27ba588f03bf6e1e74763903bab25e6e8bb6d0f0"

BATCH = "crates/gf2-core/src/gf2m/batch.rs"
VEC = "crates/gf2-core/src/field/vec.rs"
WIDE = "crates/gf2-core/src/gf2m/wide.rs"
TUNING = "crates/gf2-core/src/tuning/mod.rs"
BAKED = "crates/gf2-core/src/tuning/baked.rs"
KERNEL_GF2M = "crates/gf2-kernels-simd/src/gf2m.rs"
KERNEL_CLMUL = "crates/gf2-kernels-simd/src/x86/clmul.rs"
KERNEL_BATCH = "crates/gf2-kernels-simd/src/x86/gf2m_batch.rs"

# claim, project, path, needle, occurrence (1-based), why
CLAIMS = [
    ("batch-mul-rustdoc-crossover-claim", "gf2", BATCH,
     "throughput advantage for `n < ~32` elements.", 1,
     "The batch module's rustdoc states a crossover length at which the batch "
     "kernel stops paying. The study measures that boundary; the claim itself "
     "carries no receipt."),
    ("batch-mul-has-no-length-gate", "gf2", BATCH,
     "if matches!(m_u32, 8 | 16 | 32) {", 1,
     "batch_mul_raw selects the kernel on the field degree alone: no branch "
     "consults the batch length, so the rustdoc's crossover is advice to the "
     "caller rather than a selector in the code."),
    ("batch-mul-takes-the-kernel", "gf2", BATCH,
     "if let Some(fns) = crate::simd::maybe_gf2m_batch() {", 1,
     "The supported degrees take the dispatched batch kernel whenever the host "
     "publishes one."),
    ("dot-product-has-no-length-gate", "gf2", VEC,
     "self.simd_dot_product_chunked::<DOT_CHUNK_LEN_SELECTED>(rhs)", 1,
     "simd_dot_product consults only the compile-time chunk extent; no branch "
     "consults the vector length, so a short vector takes the dispatched path."),
    ("dot-product-chunk-extent", "gf2", VEC,
     "pub(crate) const DOT_CHUNK_LEN: usize = 256;", 1,
     "The chunk extent that sizes the dot product's stack scratch buffers."),
    ("dot-product-extracts-element-values", "gf2", VEC,
     "a_buf[i] = a.value();", 1,
     "Each chunk extracts both operands' element values into flat buffers "
     "before the batch call; the stage diagnostic reconstructs this stage."),
    ("dot-product-xor-accumulates", "gf2", VEC,
     "acc ^= p;", 1,
     "The batch products are XOR-accumulated into one 128-bit value without "
     "reduction."),
    ("dot-product-single-reduction", "gf2", VEC,
     "let result = reducer.reduce_with_clmul(acc, clmul_fn);", 1,
     "The sole reduction receives the raw accumulator; the stage diagnostic "
     "times this operation on that value."),
    ("raw-batch-default-sequential", "gf2", KERNEL_GF2M,
     "detect_with_clmul_batch_preference(ClmulBatchLane::Sequential)", 1,
     "The default detection selects the sequential PCLMULQDQ raw-batch lane, "
     "the lane 1d0da41f's confirmation retained."),
    ("raw-batch-lane-tag", "gf2", KERNEL_CLMUL,
     'pub(crate) const CLMUL_BATCH_PATH_XMM: &str = "pclmulqdq-scalar-xmm";', 1,
     "The lane tag the study's arms report for the sequential lane."),
    ("public-long-product-dispatches", "gf2", WIDE,
     "clmul_wide_dispatch::<N>(a, b, out, ProductWrite::Accumulate);", 1,
     "The public clmul_wide_slice routes through the capability dispatch, so "
     "the study's long-product arm measures the dispatched path at the widths "
     "a kernel covers."),
    ("wide-field-product-allocates-scratch", "gf2", WIDE,
     "let mut product = vec![0u64; 2 * N];", 1,
     "Gf2mWide::mul_ref allocates its unreduced-product scratch on every call; "
     "the whole-consumer wide-field cell includes that allocation."),
    ("wide-field-product-reduces", "gf2", WIDE,
     "let reduced = reducer.reduce_slice(&product);", 1,
     "mul_ref composes the dispatched product with BarrettReducerWide, which "
     "is the operation the composed gf2x arm reproduces."),
    ("field-vec-selector-family", "gf2", TUNING,
     "pub struct FieldVecSelectors {", 1,
     "The canonical tuning mechanism's field-vector selector family, whose one "
     "field is the chunk extent."),
    ("field-vec-selector-only-field", "gf2", TUNING,
     "pub fn try_new(dot_chunk_len: usize) -> Result<Self, ProfileError> {", 1,
     "The family's constructor takes the chunk extent alone: the mechanism "
     "carries no dot-product length threshold today."),
    ("dot-chunk-len-is-baked", "gf2", BAKED,
     "pub(crate) const DOT_CHUNK_LEN: usize = 256;", 1,
     "The chunk extent reaches production through the bake mechanism, which is "
     "the mechanism a recommendation about it must name."),
    ("soa-batch-selector-family", "gf2", TUNING,
     "pub struct SoaBatchSelectors {", 1,
     "The batch selector family the mechanism already carries, which a "
     "raw-batch length threshold would join."),
    ("batch-kernel-unrolls-four", "gf2", KERNEL_BATCH,
     "while i + 4 <= n {", 1,
     "The dispatched reduced-batch kernel consumes four elements per iteration, "
     "which is the amortisation the batch module's rustdoc claims."),
    ("batch-kernel-tail-is-scalar", "gf2", KERNEL_BATCH,
     "out[i] = clmul_barrett_reduce_inline(a[i], b[i], mu, modulus, degree);", 1,
     "The dispatched batch kernel finishes a partial vector with a scalar "
     "tail, so a short vector pays the kernel's prologue for a mostly scalar "
     "run."),
    ("gf2x-public-entry", "gf2x", "gf2x.h.in.in",
     "extern int GF2X_EXPORTED gf2x_mul_r(unsigned long *c,", 1,
     "gf2x_mul_r is gf2x's reentrant top-level product of two word-array "
     "polynomials, the operation the polynomial cells compare against."),
    ("gf2x-basecase-branch", "gf2x", "gf2x.c",
     "if (sa < GF2X_MUL_KARA_THRESHOLD) {", 1,
     "Below the Karatsuba threshold gf2x_mul_r takes its tuned basecase."),
]

# label, command argv, why
SEARCHES = [
    ("batch-mul-callers",
     ["git", "grep", "-n", "-E", r"batch_mul(_raw)?\(", "{commit}", "--", "crates/"],
     "Every call of the batch product in the gf2 crates. A length threshold "
     "would have to be read at one of these sites."),
    ("simd-dot-product-callers",
     ["git", "grep", "-n", "-E", r"simd_dot_product(_chunked)?(::<|\()", "{commit}", "--", "crates/"],
     "Every call of the dispatched dot product in the gf2 crates."),
    ("field-vec-selector-uses",
     ["git", "grep", "-n", "-E", r"dot_chunk_len|FieldVecSelectors", "{commit}", "--", "crates/"],
     "Every mention of the field-vector selector family. The absence of a "
     "length threshold in the canonical mechanism rests on this output."),
]


def line_of(text, needle, occurrence, where):
    seen = 0
    for number, line in enumerate(text.splitlines(), start=1):
        if needle in line:
            seen += 1
            if seen == occurrence:
                return number, line
    raise SystemExit(f"{where}: needle {needle!r} occurrence {occurrence} not found")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--gf2-commit", required=True)
    parser.add_argument("--gf2x-source", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    head = subprocess.run(
        ["git", "-C", args.gf2x_source, "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True).stdout.strip()
    if head != GF2X_COMMIT:
        raise SystemExit(f"gf2x source is at {head}, not the pinned {GF2X_COMMIT}")
    gf2_commit = subprocess.run(
        ["git", "rev-parse", args.gf2_commit],
        check=True, capture_output=True, text=True).stdout.strip()

    claims = []
    for claim, project, path, needle, occurrence, why in CLAIMS:
        if project == "gf2":
            text = subprocess.run(
                ["git", "show", f"{gf2_commit}:{path}"],
                check=True, capture_output=True, text=True).stdout
            commit = gf2_commit
        else:
            with open(f"{args.gf2x_source}/{path}") as handle:
                text = handle.read()
            commit = GF2X_COMMIT
        number, line = line_of(text, needle, occurrence, f"{project}:{path}")
        claims.append({
            "claim": claim,
            "project": project,
            "commit": commit,
            "path": path,
            "line": number,
            "text": line,
            "why": why,
        })

    searches = []
    for label, argv, why in SEARCHES:
        command = [part.format(commit=gf2_commit) for part in argv]
        result = subprocess.run(command, capture_output=True, text=True)
        matches = []
        for raw in result.stdout.splitlines():
            # `git grep <rev> -- <path>` prefixes each hit with the revision.
            revision, path, number, text = raw.split(":", 3)
            if revision != gf2_commit:
                raise SystemExit(f"search hit names {revision}, not the pinned commit")
            matches.append({"path": path, "line": int(number), "text": text})
        searches.append({
            "label": label,
            "command": " ".join(command),
            "why": why,
            "matches": matches,
        })

    evidence = {
        "schema": "53c5a8c0-source-evidence-v1",
        "commits": {"gf2": gf2_commit, "gf2x": GF2X_COMMIT},
        "claims": claims,
        "searches": searches,
    }
    with open(args.output, "w") as handle:
        json.dump(evidence, handle, indent=2)
        handle.write("\n")
    print(f"{len(claims)} claims, {len(searches)} searches -> {args.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
