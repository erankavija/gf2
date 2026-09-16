#!/usr/bin/env python3
"""Freeze the source claims the logical-buffer harness makes (jit:bb769456).

Every claim names a project, a repository-relative path, the line, the verbatim
line at that position and why the harness relies on it. The file is regenerated
rather than edited, so a claim that moves or disappears fails this script
instead of going stale in prose.
"""

import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[4]
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
OUTPUT = STORY / "survey" / "logical-source-evidence.json"

CLAIMS = [
    (
        "gf2",
        "crates/gf2-core/src/kernels/ops.rs",
        "pub fn xor_inplace(dst: &mut [u64], src: &[u64]) {",
        "The isolated-XOR baseline calls exactly this public entry point, so "
        "per-call backend selection stays inside the measured region.",
    ),
    (
        "gf2",
        "crates/gf2-core/src/kernels/ops.rs",
        "    let xor = resolve_xor_inplace(dst.len());",
        "Dispatch is resolved per call inside the public route; the resolved "
        "arm hoists this one line out of the loop and is attribution only.",
    ),
    (
        "gf2",
        "crates/gf2-core/src/kernels/ops.rs",
        "pub fn resolve_xor_inplace(word_len: usize) -> XorInplaceFn {",
        "The exploratory attribution route resolves once outside timing "
        "through this public function.",
    ),
    (
        "gf2",
        "crates/gf2-core/src/matrix.rs",
        "    pub fn row_xor(&mut self, dst: usize, src: usize) {",
        "One reported row operation is one call to this public method; row "
        "lookup, split, dispatch and the in-place XOR are inside timing.",
    ),
    (
        "gf2",
        "crates/gf2-core/src/matrix.rs",
        "    pub fn row_words(&self, row: usize) -> &[u64] {",
        "The row arm observes the production allocation's addresses through "
        "this public accessor rather than copying rows into a private layout.",
    ),
    (
        "gf2",
        "crates/gf2-coding/src/ldpc/nr_5g/mod.rs",
        "    pub fn nr_5g_rate_matched(",
        "The whole-consumer cell times this public constructor end to end.",
    ),
    (
        "gf2",
        "crates/gf2-coding/src/ldpc/nr_5g/mod.rs",
        "fn compute_mother_encoding(code: &LdpcCode, params: &NrRateMatchParams) -> MotherEncoding {",
        "The constructor's repeated unhoisted public row XORs are the "
        "dispatch-hoist consumer the frozen addendum admits.",
    ),
    (
        "gf2",
        "crates/gf2-coding/src/ldpc/nr_5g/mod.rs",
        "    let mut work = BitMatrix::zeros(m, n);",
        "The dense conversion inside the constructor fixes the row stride the "
        "selected-route table declares.",
    ),
    (
        "gf2",
        "dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs",
        "    command.env_clear();",
        "The runner clears the child environment, so an arm must not demand "
        "window variables before reading its request.",
    ),
    (
        "gf2",
        "dev/tools/tuning-campaign-support/src/trial_ledger.rs",
        "    let mut file = fs::OpenOptions::new().read(true).append(true).open(&path)?;",
        "Every protocol-v4 campaign reserves against its family ledger, so the "
        "four ledger files exist as committed empty genesis files.",
    ),
    (
        "isa-l",
        "raid/raid_base.c",
        "xor_gen_base(int vects, int len, void **array)",
        "The external arm links this scalar reference; the NASM-built "
        "multibinary xor_gen has no reproduced executable on this host.",
    ),
]


def isal_root():
    common = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        check=True,
        text=True,
    ).stdout.strip()
    primary = pathlib.Path(common).parent
    for candidate in sorted(primary.glob(".agents/ext/*/isa-l")):
        head = subprocess.run(
            ["git", "-C", str(candidate), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
        )
        if head.returncode == 0 and head.stdout.strip() == ISAL_REVISION:
            return candidate
    raise SystemExit(f"no ISA-L checkout at {ISAL_REVISION} under {primary}/.agents/ext")


ISAL_REVISION = "7c3479e0a9dac17f448603ec1ad64c7c625f530c"


def last_change(path):
    """The commit that last changed this file.

    HEAD would move with every unrelated commit and make this ledger churn;
    content identities decide validity, and the commit that last touched the
    file is the one a reader follows to see the claim in context.
    """
    return subprocess.run(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", str(path)],
        capture_output=True,
        check=True,
        text=True,
    ).stdout.strip()


def main():
    external = isal_root()
    records = []
    for project, path, fragment, why in CLAIMS:
        base = ROOT if project == "gf2" else external
        text = (base / path).read_text()
        lines = text.splitlines()
        positions = [index + 1 for index, line in enumerate(lines) if fragment in line]
        if not positions:
            raise SystemExit(f"{path}: the claimed line is absent: {fragment}")
        records.append(
            {
                "project": project,
                "commit": last_change(path) if project == "gf2" else ISAL_REVISION,
                "path": path,
                "line": positions[0],
                "verbatim": lines[positions[0] - 1],
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
                "why": why,
            }
        )
    document = {
        "schema": "logical-buffer-source-evidence-v1",
        "issue": "bb769456",
        "addendum_identity": "2037941f-logical-buffer-v1",
        "claims": records,
    }
    (ROOT / OUTPUT).write_text(json.dumps(document, indent=2) + "\n")
    print(f"{OUTPUT}: {len(records)} source claims")


if __name__ == "__main__":
    main()
