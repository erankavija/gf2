#!/usr/bin/env python3
"""Inventory the unsafe boundaries of `gf2-kernels-simd` (jit:7d44b71f).

Writes `unsafe-inventory.json` beside itself: one row per `unsafe fn` and per
`unsafe` block of the package's Rust sources, with whether its safety contract
is present under RULE, for the working tree and for the task anchor that
`anchor-baseline.json` identifies. Exits nonzero after writing when a
working-tree boundary lacks its contract.

Usage: make-unsafe-inventory.py [--self-test]
"""

import json
import re
import sys

from locate import ANCHOR, HERE, ISSUE, PACKAGE, ROOT, rust_code_text, tracked

RULE = (
    "Every `unsafe` keyword outside comments and literals is one boundary. "
    "`unsafe fn <name>` is an unsafe function; its contract is a `/// # Safety` "
    "line in the doc comment attached to it, which is the unbroken run of doc "
    "comment, comment and attribute lines above its first line. `unsafe {` is an "
    "unsafe block; its contract is a `// SAFETY:` comment in the unbroken run of "
    "comment lines directly above the line holding the keyword or above the "
    "first line of the statement that holds it. `unsafe fn(` is a function "
    "pointer type and carries no contract of its own. Any other use fails the "
    "script."
)
KEYWORD = re.compile(r"\bunsafe\b\s*(?:(fn)\s*([A-Za-z_$][A-Za-z0-9_]*)?|(\{))?")
FUNCTION = re.compile(r"\bfn\s+([A-Za-z_$][A-Za-z0-9_]*)")
SAFETY_SECTION = re.compile(r"^///\s*# Safety\s*$")
SAFETY_COMMENT = re.compile(r"^//\s*SAFETY:")
STATEMENT_END = (";", "{", "}", ",", "=>")


def run_above(lines, index, accept):
    """The unbroken run of stripped lines above `index` that `accept` admits, nearest first."""
    run = []
    while index > 0 and accept(lines[index - 1].strip()):
        index -= 1
        run.append(lines[index].strip())
    return run


def function_contract(lines, code, index):
    """Whether the doc comment attached to the item on line `index` has a safety section."""
    run, inside_attribute = [], False
    while index > 0:
        text, plain = lines[index - 1].strip(), code[index - 1].strip()
        if inside_attribute:
            inside_attribute = not plain.startswith("#[")
        elif text.startswith("//") or plain.startswith("#["):
            pass
        elif plain.endswith("]"):
            inside_attribute = True
        else:
            break
        index -= 1
        run.append(text)
    return any(SAFETY_SECTION.match(text) for text in run)


def block_contract(lines, code, index):
    """Whether a safety comment stands above line `index` or above its statement's first line."""
    first = index
    while first > 0:
        above = code[first - 1].strip()
        if not above or above.endswith(STATEMENT_END):
            break
        first -= 1
    comment = lambda text: text.startswith("//")  # noqa: E731
    return any(
        SAFETY_COMMENT.match(text)
        for start in {index, first}
        for text in run_above(lines, start, comment)
    )


def boundaries(source):
    """One row per unsafe boundary of `source`, in source order."""
    masked = rust_code_text.masked(source)
    lines, code = source.splitlines(), masked.splitlines()
    rows = []
    for found in KEYWORD.finditer(masked):
        line = masked.count("\n", 0, found.start())
        enclosing = FUNCTION.findall(masked[: found.start()])
        if found[1] and found[2]:
            row = {"kind": "unsafe fn", "item": found[2],
                   "contract": function_contract(lines, code, line)}
        elif found[1]:
            row = {"kind": "unsafe fn pointer type",
                   "item": enclosing[-1] if enclosing else None, "contract": None}
        elif found[3]:
            row = {"kind": "unsafe block", "item": enclosing[-1] if enclosing else None,
                   "contract": block_contract(lines, code, line)}
        else:
            raise SystemExit(f"line {line + 1}: unclassified use of `unsafe`")
        rows.append({"line": line + 1} | row)
    return rows


def summary(rows):
    counts = {}
    for row in rows:
        entry = counts.setdefault(row["kind"], {"total": 0, "with_contract": 0, "without_contract": 0})
        entry["total"] += 1
        if row["contract"] is not None:
            entry["with_contract" if row["contract"] else "without_contract"] += 1
    return counts


def inventory(read, paths):
    rows = [{"path": path} | row for path in paths for row in boundaries(read(path))]
    return {"counts": summary(rows), "boundaries": rows}


SELF_TEST = '''
/// Doc.
///
/// # Safety
///
/// Contract.
#[target_feature(enable = "avx2")]
#[cfg(any(
    target_arch = "x86_64",
))]
pub unsafe fn documented(p: *const u8) -> u8 { *p }

/// No section. `unsafe { }` and "unsafe fn text" stay prose.
unsafe fn bare() {}

fn caller(f: unsafe fn(*const u8) -> u8) -> u8 {
    let p = b"unsafe {".as_ptr();
    // SAFETY: `p` points at one byte.
    let a = unsafe { documented(p) };
    let b = unsafe { f(p) };
    // SAFETY: as above.
    let c = wrap(
        unsafe { documented(p) },
    );
    a + b + c
}
'''


def self_test():
    got = [(row["kind"], row["item"], row["contract"]) for row in boundaries(SELF_TEST)]
    want = [
        ("unsafe fn", "documented", True),
        ("unsafe fn", "bare", False),
        ("unsafe fn pointer type", "caller", None),
        ("unsafe block", "caller", True),
        ("unsafe block", "caller", False),
        ("unsafe block", "caller", True),
    ]
    if got != want:
        raise SystemExit(f"self-test: {got}")
    print("self-test: ok")


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    current = inventory(lambda path: (ROOT / path).read_text(), tracked(".rs"))
    record = {
        "schema": "unsafe-boundary-inventory-v1",
        "issue": ISSUE,
        "package": PACKAGE,
        "rule": RULE,
        "current": current,
    }
    if ANCHOR.baseline.is_file():
        paths = sorted(path for path in ANCHOR.digests() if path.endswith(".rs"))
        record["anchor"] = ANCHOR.identity() | inventory(
            lambda path: ANCHOR.bytes(path).decode(), paths
        )
    output = HERE / "unsafe-inventory.json"
    output.write_text(json.dumps(record, indent=2) + "\n")
    missing = [row for row in current["boundaries"] if row["contract"] is False]
    print(f"{output.relative_to(ROOT)}: {json.dumps(current['counts'])}")
    if missing:
        raise SystemExit(f"{len(missing)} unsafe boundaries lack a safety contract")


if __name__ == "__main__":
    main()
