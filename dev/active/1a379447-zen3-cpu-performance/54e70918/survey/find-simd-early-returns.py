#!/usr/bin/env python3
"""Lists the tests that return early when a SIMD probe finds nothing (jit:54e70918).

Writes `simd-early-returns.json` beside itself: RULE, the probe names it
derives, and one row per matching test function of the live Rust sources, with
the package that owns the file and whether the root workspace builds it.

Usage: find-simd-early-returns.py [--self-test]
"""

import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
)


def _shared_scripts():
    """Root-relative directory of the one `repository_files.py` outside receipt snapshots."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--", ":(glob)**/repository_files.py"],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    live = [path for path in listing if "inputs" not in Path(path).parts[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live repository_files.py files; exactly one must exist")
    return Path(live[0]).parent


sys.path.insert(0, str(ROOT / _shared_scripts()))
import repository_files  # noqa: E402
import rust_code_text  # noqa: E402

ISSUE = "54e70918"
RULE = (
    "Scope: every live `.rs` file git lists under the repository root, tracked or "
    "untracked and not ignored, outside receipt input snapshots. Comments are "
    "blanked before matching, and names are matched outside literals. "
    "Probe: a CPU-feature macro (`is_x86_feature_detected!`, "
    "`is_aarch64_feature_detected!`, `cfg!(target_feature ...)`, "
    "`cfg!(feature = \"simd\")`) or a derived probe name. A function is a derived "
    "probe when it takes no parameter, its return type opens with `bool` or "
    "`Option`, and its body holds a probe; an item-level `static` or `const` is "
    "one when its type or initializer holds a probe. Derivation runs to a fixed "
    "point over every scanned file, and a name is matched as a whole word "
    "wherever it appears. Inside one function, a `let` binding of a plain name "
    "whose initializer holds a probe is a probe for the rest of that function. "
    "Test function: a `fn` item that carries an attribute whose path ends in "
    "`test`. "
    "Early return: a `return` in the body of a test function, outside closures "
    "and nested functions, in the branch a missing probe takes. The header of a "
    "block is the text from the end of the previous statement to its opening "
    "brace. The branch is one of: the block of an `if` or `else if` whose header "
    "holds a probe together with a negation (`!`, `.is_none()`, `.is_err()`, "
    "`== false`, `== None`) and no `let Some` or `let Ok`; the `else` block of a "
    "`let ... else` whose header holds a probe; the `else` block of an `if` "
    "whose header holds a probe without such a negation; a match arm whose "
    "pattern opens with none of `Some`, `Ok`, `true`, in a `match` whose header "
    "holds a probe; a block or a `return` statement that carries a `#[cfg(not(...))]` "
    "attribute naming `feature = \"simd\"` or `target_feature`. A test function that calls, "
    "by bare name, a function of the same file that has no return type and "
    "returns early under this definition is listed with that function as its "
    "guard. "
    "A test whose assertions sit inside a probe conditional and that holds no "
    "such `return` is outside this definition."
)

SEED = re.compile(
    r"is_x86_feature_detected\s*!|is_aarch64_feature_detected\s*!"
    r"|cfg\s*!\s*\(\s*(?:not\s*\(\s*)?target_feature\b"
    r"|cfg\s*!\s*\(\s*(?:not\s*\(\s*)?feature\s*=\s*\"simd\""
)
TEST_ATTRIBUTE = re.compile(r"^#\s*\[\s*(?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)*test\s*(?:\]|\()")
FUNCTION = re.compile(r"\bfn\s+([A-Za-z_$][A-Za-z0-9_]*)")
PROBE_SIGNATURE = re.compile(
    r"\bfn\s+[A-Za-z_$][A-Za-z0-9_]*\s*(?:<[^()]*>)?\s*\(\s*\)\s*->\s*(?:bool|Option)\b"
)
GLOBAL = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\s*\([^)]*\))?\s+)?(?:static|const)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:([^;]*);"
)
LET = re.compile(r"\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*(?::[^=;]*)?=([^;]*);")
RETURN = re.compile(r"\breturn\b")
CLOSURE = re.compile(r"\|[^|{};]*\|\s*(?:->\s*[^{]+)?$")
DISABLED_BUILD = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*not\s*\(\s*(?:feature\s*=\s*\"simd\"|target_feature\b)[^\]]*\]\s*$"
)
NEGATION = re.compile(r"(?<!\w)!(?!=)|\.\s*is_none\s*\(|\.\s*is_err\s*\(|==\s*(?:false|None)\b")
POSITIVE_LET = re.compile(r"\blet\s+(?:Some|Ok)\b")
POSITIVE_ARM = re.compile(r"^(?:Some|Ok|true)\b")
OPEN, CLOSE = "([{", ")]}"


class Source:
    """One file: `masked` blanks comments and literals, `text` comments only."""

    def __init__(self, source):
        kept, position = [], 0
        for start, end, kind in rust_code_text.lexemes(source):
            kept.append(source[position:start])
            body = source[start:end]
            kept.append(body if kind == "literal" else re.sub(r"[^\n]", " ", body))
            position = end
        kept.append(source[position:])
        self.text = "".join(kept)
        self.masked = rust_code_text.masked(source)
        self.functions = functions(self.masked)

    def holds_probe(self, start, end, names):
        if SEED.search(self.text, start, end):
            return True
        span = self.masked[start:end]
        return any(re.search(rf"\b{re.escape(name)}\b", span) for name in names)

    def line(self, position):
        return self.masked.count("\n", 0, position) + 1


def matching(masked, opening):
    """Index of the bracket closing the one at `opening`."""
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] in OPEN:
            depth += 1
        elif masked[index] in CLOSE:
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("unbalanced bracket")


def opening_of(masked, closing):
    """Index of the bracket the one at `closing` closes."""
    depth = 0
    for index in range(closing, -1, -1):
        if masked[index] in CLOSE:
            depth += 1
        elif masked[index] in OPEN:
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("unbalanced bracket")


class Function:
    """One `fn` item with a body."""

    def __init__(self, name, start, body_open, body_close, is_test):
        self.name = name
        self.start = start
        self.body_open = body_open
        self.body_close = body_close
        self.is_test = is_test


def attributes_before(masked, position):
    """The attributes directly above the item at `position`, nearest first."""
    found = []
    while True:
        end = position
        while end > 0 and masked[end - 1].isspace():
            end -= 1
        if end == 0 or masked[end - 1] != "]":
            # Visibility and qualifiers sit between the attributes and `fn`.
            word = re.search(
                r"(?:\bpub(?:\s*\([^)]*\))?|\basync|\bconst|\bunsafe)\s*$", masked[:end]
            )
            if word is None:
                return found
            position = word.start()
            continue
        hash_at = opening_of(masked, end - 1) - 1
        while hash_at >= 0 and masked[hash_at].isspace():
            hash_at -= 1
        if hash_at < 0 or masked[hash_at] != "#":
            return found
        found.append(masked[hash_at:end])
        position = hash_at


def functions(masked):
    """Every `fn` item of `masked` that has a body, in source order."""
    found = []
    for match in FUNCTION.finditer(masked):
        index, depth = match.end(), 0
        while index < len(masked):
            char = masked[index]
            if char in "([":
                depth += 1
            elif char in ")]":
                depth -= 1
            elif depth == 0 and char in "{;":
                break
            index += 1
        if index >= len(masked) or masked[index] == ";":
            continue
        is_test = any(TEST_ATTRIBUTE.match(a) for a in attributes_before(masked, match.start()))
        found.append(Function(match[1], match.start(), index, matching(masked, index), is_test))
    return found


def header_start(masked, block_open, floor):
    """Where the header of the block opening at `block_open` starts, above `floor`."""
    depth, index = 0, block_open - 1
    while index > floor:
        char = masked[index]
        if char in ")]":
            depth += 1
        elif char in "([":
            depth -= 1
        elif depth == 0 and char in ";{}":
            break
        index -= 1
    return index + 1


def enclosing_blocks(masked, body_open, position):
    """Opening indices of the brace blocks around `position`, innermost first."""
    stack = []
    for index in range(body_open, position):
        if masked[index] == "{":
            stack.append(index)
        elif masked[index] == "}":
            stack.pop()
    return stack[::-1]


def arm_pattern(masked, arrow):
    """The pattern of the match arm whose `=>` starts at `arrow`."""
    depth, index = 0, arrow - 1
    while index >= 0:
        char = masked[index]
        if char in ")]}":
            depth += 1
        elif char in "([{":
            if depth == 0:
                break
            depth -= 1
        elif depth == 0 and char == ",":
            break
        index -= 1
    return masked[index + 1:arrow].strip()


def missing_probe_branch(source, function, position, names):
    """The header span governing the `return` at `position` when RULE lists it."""
    masked, floor = source.masked, function.body_open
    blocks = enclosing_blocks(masked, floor, position)[:-1]
    spans = [(header_start(masked, block, floor), block) for block in blocks]
    headers = [masked[start:end].strip() for start, end in spans]
    if any(CLOSURE.search(h) or FUNCTION.search(h) for h in headers):
        return None
    statement = header_start(masked, position, floor)
    if DISABLED_BUILD.search(source.text[statement:position].rstrip()):
        return statement, position
    if not blocks:
        return None
    if DISABLED_BUILD.search(source.text[spans[0][0]:spans[0][1]].rstrip()):
        return spans[0]
    level = 0
    pattern = None
    if headers[0].endswith("=>"):
        pattern = arm_pattern(masked, masked.rindex("=>", 0, blocks[0]))
        level = 1
    else:
        before = masked[blocks[0] + 1:position].rstrip()
        if before.endswith("=>"):
            pattern = arm_pattern(masked, blocks[0] + 1 + len(before) - 2)
    if level >= len(blocks):
        return None
    start, end = spans[level]
    head = headers[level]
    if pattern is not None:
        listed = (
            re.search(r"\bmatch\b", head)
            and not POSITIVE_ARM.match(pattern)
            and source.holds_probe(start, end, names)
        )
        return (start, end) if listed else None
    if head == "else":
        closing = start - 1
        while masked[closing].isspace():
            closing -= 1
        if masked[closing] != "}":
            return None
        end = opening_of(masked, closing)
        start = header_start(masked, end, floor)
        head = masked[start:end].strip()
        listed = (
            re.match(r"(?:else\s+)?if\b", head)
            and not NEGATION.search(head)
            and source.holds_probe(start, end, names)
        )
        return (start, end) if listed else None
    if re.match(r"let\b", head) and re.search(r"\belse$", head):
        return (start, end) if source.holds_probe(start, end, names) else None
    if re.match(r"(?:else\s+)?if\b", head):
        listed = (
            NEGATION.search(head)
            and not POSITIVE_LET.search(head)
            and source.holds_probe(start, end, names)
        )
        return (start, end) if listed else None
    return None


def early_returns(source, function, names):
    """`(offset, header)` of each early return of `function` under RULE."""
    local = set(names)
    for binding in LET.finditer(source.masked, function.body_open, function.body_close):
        if source.holds_probe(binding.start(2), binding.end(2), local):
            local.add(binding[1])
    found = []
    for match in RETURN.finditer(source.masked, function.body_open, function.body_close):
        span = missing_probe_branch(source, function, match.start(), local)
        if span:
            found.append((match.start(), " ".join(source.text[span[0]:span[1]].split())))
    return found


def derive_probes(sources):
    """Probe names under RULE, to a fixed point over `sources`."""
    names = set()
    while True:
        grown = set(names)
        for source in sources.values():
            for item in source.functions:
                signature = source.masked[item.start:item.body_open]
                if PROBE_SIGNATURE.match(signature) and source.holds_probe(
                    item.body_open, item.body_close, names
                ):
                    grown.add(item.name)
            for match in GLOBAL.finditer(source.masked):
                if source.holds_probe(match.start(2), match.end(2), names):
                    grown.add(match[1])
        if grown == names:
            return names
        names = grown


def packages():
    """`(directory, name, member)` of every live Cargo package, deepest directory first."""
    root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    members = set(root_manifest["workspace"]["members"])
    found = []
    for manifest in repository_files.tracked_files(ROOT, "Cargo.toml"):
        package = tomllib.loads((ROOT / manifest).read_text()).get("package")
        if package:
            directory = str(Path(manifest).parent)
            found.append((directory, package["name"], directory in members))
    return sorted(found, key=lambda entry: -len(Path(entry[0]).parts))


def owner(path, known):
    for directory, name, member in known:
        if Path(path).is_relative_to(directory):
            return name, member
    return None, False


def scan(texts):
    """Rows and probe names for `texts`, a map of path to source."""
    sources = {path: Source(source) for path, source in texts.items()}
    names = derive_probes(sources)
    rows = []
    for path in sorted(sources):
        source = sources[path]
        guarded = {}
        for item in source.functions:
            found = early_returns(source, item, names)
            if found and (item.is_test or "->" not in source.masked[item.start:item.body_open]):
                guarded.setdefault(item.name, found[0])
        for item in source.functions:
            if not item.is_test:
                continue
            via = None
            if item.name not in guarded or not early_returns(source, item, names):
                body = source.masked[item.body_open:item.body_close]
                called = sorted(
                    name for name in guarded
                    if name != item.name and re.search(
                        rf"(?<![.:\w]){re.escape(name)}\s*(?:::\s*<[^()]*>\s*)?\(", body
                    )
                )
                if not called:
                    continue
                via = called[0]
                position, governing = guarded[via]
            else:
                position, governing = early_returns(source, item, names)[0]
            rows.append({
                "path": path,
                "test": item.name,
                "test_line": source.line(item.start),
                "guard_function": via,
                "return_line": source.line(position),
                "header": governing,
            })
    return rows, sorted(names)


def self_test():
    sample = {
        "a.rs": '''
fn has() -> bool { is_x86_feature_detected!("avx2") }
static FNS: OnceLock<Option<Fns>> = OnceLock::new(detect);
fn detect() -> Option<Fns> { if has() { Some(Fns) } else { None } }
fn helper() { if !has() { return; } }
#[test] fn direct() { if !has() { return; } assert!(true); }
#[test] fn let_else() { let Some(f) = detect() else { return; }; }
#[test] fn arm() { let f = match detect() { Some(f) => f, None => { return; } }; }
#[test] fn alias() { let found = detect(); if found.is_none() { return; } }
#[test] fn through() { helper(); }
#[test] fn closure() { let c = || { if !has() { return; } }; c(); }
#[test] fn unrelated(x: bool) { if x { return; } }
#[test] fn conditional() { if let Some(f) = detect() { assert!(true); } }
#[test] fn in_else() { if let Some(f) = detect() { assert!(true); } else { return; } }
#[test] fn bare_arm() { let f = match detect() { Some(f) => f, None => return }; }
#[test] fn found_branch() { if let Some(f) = detect() { return; } assert!(true); }
#[test] fn in_literal() { if x != "has" { return; } }
fn dispatches() { if let Some(f) = detect() { return; } }
fn lane(kind: u8) -> Option<Fns> { if !has() { return None; } Some(Fns) }
#[test] fn calls_lane() { lane(1); }
#[test] fn disabled_block() { #[cfg(not(feature = "simd"))] { return; } }
#[test] fn disabled_statement() { #[cfg(not(feature = "simd"))] return; }
#[test] fn enabled_block() { #[cfg(feature = "simd")] { return; } }
#[test] fn turbofish() { helper::<3>(); }
#[test] fn calls_dispatch() { dispatches(); }
// #[test] fn commented() { if !has() { return; } }
fn dispatch() { if !has() { return; } }
''',
    }
    rows, names = scan(sample)
    assert names == ["FNS", "detect", "has"], names
    listed = [(row["test"], row["guard_function"]) for row in rows]
    expected = [("direct", None), ("let_else", None), ("arm", None), ("alias", None),
                ("through", "helper"), ("in_else", None), ("bare_arm", None),
                ("disabled_block", None), ("disabled_statement", None),
                ("turbofish", "helper")]
    assert listed == expected, listed
    print("self-test: ok")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return 0
    if sys.argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    known = packages()
    texts = {
        path: (ROOT / path).read_text()
        for path in repository_files.tracked_files(ROOT, "*.rs")
    }
    rows, names = scan(texts)
    counts = {}
    for row in rows:
        row["package"], row["workspace_member"] = owner(row["path"], known)
        key = row["package"] or "(no package)"
        counts[key] = counts.get(key, 0) + 1
    record = {
        "issue": ISSUE,
        "rule": RULE,
        "files_scanned": len(texts),
        "derived_probe_names": names,
        "tests_by_package": dict(sorted(counts.items())),
        "tests_in_workspace_members": sum(1 for row in rows if row["workspace_member"]),
        "tests": rows,
    }
    (HERE / "simd-early-returns.json").write_text(json.dumps(record, indent=1) + "\n")
    print(f"{len(rows)} tests return early on a missing SIMD probe; "
          f"{record['tests_in_workspace_members']} in workspace members")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
