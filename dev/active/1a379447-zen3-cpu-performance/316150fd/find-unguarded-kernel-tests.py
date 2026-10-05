#!/usr/bin/env python3
"""Lists the kernel-crate tests that run an AVX2 or PCLMULQDQ kernel without a host check (jit:316150fd).

Writes `unguarded-kernel-tests.json` beside itself: RULE, and one row per
matching test function of the live Rust sources of the `gf2-kernels-simd`
package. The lexer, the test-function definition and the probe derivation are
those of `find-simd-early-returns.py`, which lists the tests that return early.

Usage: find-unguarded-kernel-tests.py [--self-test]
"""

import importlib.util
import json
import re
import subprocess
import sys
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

_early = importlib.util.spec_from_file_location(
    "find_simd_early_returns", ROOT / repository_files.live_file(ROOT, "find-simd-early-returns.py")
)
early = importlib.util.module_from_spec(_early)
_early.loader.exec_module(early)

ISSUE = "316150fd"
PACKAGE_NAME = "gf2-kernels-simd"
RULE = (
    "Scope: every live `.rs` file git lists under the `gf2-kernels-simd` package "
    "directory, outside receipt input snapshots. Comments are blanked before "
    "matching, and names are matched outside literals. "
    "Test function and probe: as in the rule of `find-simd-early-returns.py`, with "
    "probes derived over the scanned files. "
    "Target-feature function: a `fn` item that carries a `#[target_feature(...)]` "
    "attribute. "
    "Bundle getter: a `fn` outside a `#[cfg(test)]` module whose return type is a "
    "type named `...Fns`, that is not an `Option`, and whose body holds a call to "
    "a target-feature function. "
    "Test helper: a `fn` inside a `#[cfg(test)]` module that is not a test. "
    "Kernel use: the body of a test, or of a test helper it calls by bare name "
    "(to a fixed point), calls a target-feature function or a bundle getter by "
    "bare name. "
    "Host check: the body of the test, or of a test helper it calls, holds a probe. "
    "Listed: a test with a kernel use and no host check; `via` names the helper "
    "that holds the kernel use when the test does not."
)

TARGET_FEATURE = re.compile(r"#\s*\[\s*target_feature\b")
CFG_TEST = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
MODULE = re.compile(r"\bmod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{")
RETURN_TYPE = re.compile(r"->\s*((?:\w+::)*\w+)\s*$")


def call(name):
    return re.compile(rf"(?<![.:\w]){re.escape(name)}\s*(?:::\s*<[^()]*>\s*)?\(")


def test_modules(source):
    """`(open, close)` offsets of every `#[cfg(test)] mod name { ... }` of `source`."""
    found = []
    for match in MODULE.finditer(source.masked):
        attributes = early.attributes_before(source.masked, match.start())
        if any(CFG_TEST.match(attribute) for attribute in attributes):
            opening = match.end() - 1
            found.append((opening, early.matching(source.masked, opening)))
    return found


def inside(spans, position):
    return any(start < position < end for start, end in spans)


def target_feature_names(sources):
    names = set()
    for source in sources.values():
        for item in source.functions:
            attributes = early.attributes_before(source.masked, item.start)
            if any(TARGET_FEATURE.match(attribute) for attribute in attributes):
                names.add(item.name)
    return names


def bundle_getters(sources, kernels, modules):
    names = set()
    for path, source in sources.items():
        for item in source.functions:
            signature = source.masked[item.start:item.body_open].strip()
            returned = RETURN_TYPE.search(signature)
            if (
                returned
                and returned[1].endswith("Fns")
                and not inside(modules[path], item.start)
                and any(call(name).search(source.masked, item.body_open, item.body_close)
                        for name in kernels)
            ):
                names.add(item.name)
    return names


def scan(texts):
    """Rows and the derived names for `texts`, a map of path to source."""
    sources = {path: early.Source(text) for path, text in texts.items()}
    probes = early.derive_probes(sources)
    modules = {path: test_modules(source) for path, source in sources.items()}
    kernels = target_feature_names(sources)
    getters = bundle_getters(sources, kernels, modules)
    rows = []
    for path in sorted(sources):
        source = sources[path]
        helpers = {
            item.name: item for item in source.functions
            if not item.is_test and inside(modules[path], item.start)
        }

        def uses(item):
            body = source.masked[item.body_open:item.body_close]
            return any(call(name).search(body) for name in kernels | getters)

        def checks(item):
            return source.holds_probe(item.body_open, item.body_close, probes)

        def reach(item):
            """`item` and the helpers it calls, to a fixed point."""
            seen, todo = [], [item]
            while todo:
                current = todo.pop()
                if current in seen:
                    continue
                seen.append(current)
                body = source.masked[current.body_open:current.body_close]
                todo += [h for name, h in helpers.items() if h is not current and call(name).search(body)]
            return seen

        for item in source.functions:
            if not item.is_test:
                continue
            chain = reach(item)
            using = [step for step in chain if uses(step)]
            if not using or any(checks(step) for step in chain):
                continue
            rows.append({
                "path": path,
                "test": item.name,
                "test_line": source.line(item.start),
                "via": None if using[0] is item else using[0].name,
            })
    return rows, sorted(probes), sorted(kernels), sorted(getters)


def self_test():
    sample = {
        "a.rs": '''
fn has() -> bool { is_x86_feature_detected!("avx2") }
fn detect() -> Option<Fns> { if has() { Some(fns()) } else { None } }
#[target_feature(enable = "avx2")]
unsafe fn kernel(x: &mut [u64]) {}
pub(crate) fn fns() -> Fns { fn f(x: &mut [u64]) { unsafe { kernel(x) } } Fns { f } }
pub(crate) fn scalar() -> Fns { Fns { f: noop } }
#[cfg(test)]
mod tests {
    fn guarded<F: Fn()>(f: F) { if !has() { return; } f(); }
    fn bare() { unsafe { kernel(&mut []) } }
    #[test] fn direct_kernel() { unsafe { kernel(&mut []) } }
    #[test] fn bundle() { let f = fns(); }
    #[test] fn scalar_bundle() { let f = scalar(); }
    #[test] fn checked() { if !has() { return; } unsafe { kernel(&mut []) } }
    #[test] fn through_guard() { guarded(|| unsafe { kernel(&mut []) }); }
    #[test] fn optional() { let Some(f) = detect() else { return; }; }
    #[test] fn through_bare() { bare(); }
    #[test] fn unrelated() { assert!(true); }
    // #[test] fn commented() { fns(); }
}
''',
    }
    rows, probes, kernels, getters = scan(sample)
    assert kernels == ["kernel"], kernels
    assert getters == ["fns"], getters
    listed = [(row["test"], row["via"]) for row in rows]
    expected = [("direct_kernel", None), ("bundle", None), ("through_bare", "bare")]
    assert listed == expected, listed
    print("self-test: ok")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return 0
    if sys.argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    package = repository_files.package_directory(ROOT, PACKAGE_NAME)
    texts = {
        path: (ROOT / path).read_text()
        for path in repository_files.tracked_files(ROOT, "*.rs")
        if Path(path).is_relative_to(package)
    }
    rows, probes, kernels, getters = scan(texts)
    counts = {}
    for row in rows:
        counts[row["path"]] = counts.get(row["path"], 0) + 1
    record = {
        "issue": ISSUE,
        "rule": RULE,
        "files_scanned": len(texts),
        "derived_probe_names": probes,
        "target_feature_functions": len(kernels),
        "bundle_getters": getters,
        "tests_by_file": counts,
        "tests": rows,
    }
    (HERE / "unguarded-kernel-tests.json").write_text(json.dumps(record, indent=1) + "\n")
    print(f"{len(rows)} tests run a target-feature kernel without a host check")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
