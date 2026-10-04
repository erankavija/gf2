#!/usr/bin/env python3
"""Write this issue's code claims with verbatim source lines (jit:f63a2464).

Each claim names the project, the file, a fragment the line must contain and
why the line matters for the candidate decision. The script locates the line by
its fragment in the working tree, refuses a claim whose fragment matches other
than exactly one line, and records the verbatim text with the commit that last
changed the cited file, so every mechanism this issue's reports cite can be checked at its
location instead of through a line number in prose.

Usage: make-source-evidence.py > survey/source-evidence.json
"""

import json
import subprocess

CORE = "crates/gf2-coding/src/ldpc/core.rs"
LLR = "crates/gf2-coding/src/llr.rs"
MINSUM = "crates/gf2-coding/src/ldpc/min_sum.rs"
LAYOUT = "crates/gf2-coding/src/ldpc/edge_layout.rs"
NR = "crates/gf2-coding/src/ldpc/nr_5g/mod.rs"
DVB = "crates/gf2-coding/src/ldpc/dvb_t2/builder.rs"
LEDGER = "dev/tools/tuning-campaign-support/src/trial_ledger.rs"
RECEIPT = "dev/tools/tuning-campaign-support/src/receipt.rs"
PROTOCOL = "dev/tools/tuning-campaign-support/src/protocol.rs"

# (path, fragment, topic, why)
CLAIMS = [
    (MINSUM, "sign_product = if value >= 0.0 {", "sign",
     "the canonical sign rule is a comparison against zero, so a negative-zero input counts as positive and a NaN input counts as negative"),
    (MINSUM, "let magnitude = value.abs();", "magnitude",
     "the magnitude entering the fold is the float absolute value, which has no overflow case"),
    (MINSUM, "if magnitude < min1 {", "tie",
     "a strict comparison keeps the first of two equal smallest magnitudes at the smallest position, which fixes the tie rule"),
    (MINSUM, "let magnitude = if index == arg1 { min2 } else { min1 };", "tie",
     "an output takes the second smallest exactly at the smallest position, which is the leave-one-out minimum including under a tie"),
    (MINSUM, "Self::Offset(beta) => Llr::new(sign * (magnitude - beta).max(0.0)),", "clipping",
     "offset min-sum floors the excluded magnitude at zero; this is the only clipping the float contract performs inside the check update"),
    (MINSUM, "outputs[0] = Llr::zero();", "degree",
     "a degree-one check emits the zero LLR that a degree-zero reduction means"),
    (CORE, "belief = Llr::new(belief.value() + self.check_to_var[edge].value());", "accumulation",
     "the variable-node belief accumulates sequentially over the variable's canonical layout slots in the parity-check matrix's column order"),
    (CORE, "self.var_to_check[edge] = Llr::new(belief.value() - incoming.value());", "accumulation",
     "a variable-to-check message is the belief minus that edge's incoming message, so the subtraction inherits the accumulated belief's rounding"),
    (CORE, "*message = llrs[var as usize];", "initialisation",
     "every variable-to-check message starts at its variable's channel LLR unscaled, so the float contract applies no channel scaling of its own"),
    (CORE, "// Reset all messages: check-to-variable to zero, and every", "initialisation",
     "check-to-variable messages start at zero at the top of every decode"),
    (CORE, "if early_termination && self.syndrome_passes() {", "termination",
     "per-frame termination is a syndrome test after the variable update of each iteration"),
    (CORE, "parity ^= self.hard_bits[edge_var[edge] as usize];", "termination",
     "a check's parity is the exclusive or of its edges' hard decisions over the canonical layout"),
    (LLR, "self.0 < 0.0", "hard-decision",
     "the hard decision is a strict comparison, so a zero or negative-zero belief decides to bit zero"),
    (LLR, "Llr(self.0.clamp(-max, max))", "saturation",
     "the float alphabet's only saturation is this explicit caller-invoked clamp; the decoder itself applies none"),
    (NR, "const FILLER_LLR: f32 = 15.0;", "fillers",
     "a filler position enters as a finite positive magnitude, not an infinity, so a quantized alphabet maps it by the same scale as any other channel LLR"),
    (NR, "let mut full_llrs = vec![Llr::zero(); p.full_n];", "puncturing",
     "every position the rate matching does not transmit, punctured or untransmitted parity alike, enters at the zero LLR"),
    (CORE, "let col = col_offset + ((i + self.shift) % self.size);", "qc-structure",
     "a circulant block places lifted position i of a check block on lifted position (i + shift) mod Z of a variable block, which is the rotation an intra-frame block update performs"),
    (DVB, "let parity_bit = (base_parity + j * q) % m;", "qc-structure",
     "the DVB-T2 information part groups columns into blocks of Z whose check indices advance by the step q, so its check rows carry no equal-degree block partition"),
    (DVB, "edges.push((p, k + p - 1));", "qc-structure",
     "the DVB-T2 parity part is a staircase accumulator rather than a circulant block"),
    (LAYOUT, "var_edge_to_check_edge: Vec<u32>,", "layout",
     "a variable's slots resolve to canonical check-major edge ids, which is the map any alternative layout must reproduce"),
    (LEDGER, "let attempts = entries.iter().filter(|e| e.comparisons > 0).count().max(1) as f64;",
     "budget",
     "the sequential attempt index counts ledger entries that reserve a comparison, so exploratory reservations do not advance it"),
    (LEDGER, "Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))", "budget",
     "attempt t spends alpha / (t (t + 1)), so a family's first confirmation spends half the frozen alpha"),
    (RECEIPT, "|| f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;", "budget",
     "a cell is not-confirmatory when its corrected alpha leaves fewer than twenty expected draws in a bootstrap tail, which caps the cell count of an attempt"),
    (PROTOCOL, "family_alpha: 0.05,", "budget",
     "the frozen family-wise error rate the cap is computed from"),
    (PROTOCOL, "bootstrap_resamples: 10_000,", "budget",
     "the frozen resample count the tail-support rule is computed from"),
    (PROTOCOL, "max_confirmatory_attempts_per_candidate: 1,", "budget",
     "one confirmatory attempt per candidate identity and protocol version"),
]


def locate(text, path, fragment):
    matches = [(i, line.strip()) for i, line in enumerate(text.splitlines(), 1) if fragment in line]
    if len(matches) != 1:
        raise SystemExit(f"{path}: {len(matches)} lines contain {fragment!r}, want exactly one")
    return matches[0]


def main():
    cache, revisions = {}, {}
    claims = []
    for path, fragment, topic, why in CLAIMS:
        if path not in cache:
            cache[path] = open(path, encoding="utf-8").read()
            # The commit that last changed the cited file, so the record is
            # stable until that file changes.
            revisions[path] = subprocess.run(
                ["git", "log", "-1", "--format=%H", "--", path],
                capture_output=True, text=True, check=True,
            ).stdout.strip()
        line, text = locate(cache[path], path, fragment)
        claims.append({
            "project": "gf2",
            "commit": revisions[path],
            "path": path,
            "line": line,
            "text": text,
            "topic": topic,
            "why": why,
        })

    print(json.dumps({
        "schema": "ldpc-candidate-source-evidence-v1",
        "note": ("Claims are read from the working tree. The revision is navigation metadata: "
                 "receipts pin the measured bytes."),
        "claims": claims,
    }, indent=2))


if __name__ == "__main__":
    main()
