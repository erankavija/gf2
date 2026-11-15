#!/usr/bin/env python3
"""Self-tests for contrib/gates/docs-mechanical.py over synthetic fixture trees and a fake jit."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "docs-mechanical.py"
RENDERED_GUIDE = "# Guide\n\n<!-- region:begin -->\n- rule-a\n<!-- region:end -->\n"

FAKE_JIT = """#!{python}
import json, os, pathlib, sys
args = sys.argv[1:]
root = pathlib.Path(os.environ.get("JIT_WORKTREE_ROOT", pathlib.Path.cwd()))
data = pathlib.Path(os.environ.get("JIT_DATA_DIR", root / ".jit"))
if os.environ.get("FAKE_JIT_BROKEN"):
    print("internal failure", file=sys.stderr)
    sys.exit(101)
if args[:2] == ["project", "render"]:
    (data / "render-touched").write_text("render ran here")
    for target, text in json.loads((data / "fake-render.json").read_text()).items():
        path = data / target[5:] if target.startswith(".jit/") else root / target
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    print("{{}}")
    sys.exit(0)
reference = args[2] if args[0] == "item" else "@/issue/" + args[2]
if reference in (data / "fake-items").read_text().split():
    print(json.dumps({{"id": reference}}))
    sys.exit(0)
code = "ITEM_COMMAND_FAILED" if args[0] == "item" else "ISSUE_NOT_FOUND"
print(json.dumps({{"error": {{"code": code, "message": "not found"}}}}))
sys.exit(1 if args[0] == "item" else 3)
"""

FIXTURE = {
    "contrib/gates/docs-mechanical.toml": """
        markdown = ["README.md", "docs/**/*.md", "dev/**/*.md"]
        citations = ["README.md", "docs/**/*.md", "dev/**/*.md"]
        rustdoc = ["crates/**/*.rs"]
        exclude = ["dev/archive"]
        """,
    ".jit/config.toml": """
        [projection.guide]
        target = "GUIDE.md"
        [projection.registry]
        target = ".jit/reference/projection.md"
        """,
    ".jit/fake-render.json": json.dumps({"GUIDE.md": RENDERED_GUIDE,
                                         ".jit/reference/projection.md": RENDERED_GUIDE}),
    ".jit/reference/projection.md": RENDERED_GUIDE,
    ".jit/fake-items": "@/inv/rule-a\n@/issue/abcd1234\n",
    ".gitignore": "out/\n",
    "out/previous.txt": "runtime output\n",
    "GUIDE.md": RENDERED_GUIDE,
    "README.md": """
        # Fixture

        Read [the guide](docs/guide.md#usage-notes), [this page](#fixture) and
        [lines](docs/guide.md#L3-L4).
        The library lives in `crates/demo/src/lib.rs:1`; runs write `out/report.txt`.
        It applies `@/inv/rule-a` under @/issue/abcd1234; `GUIDE.md` holds the projection.
        """,
    "docs/guide.md": """
        # Guide

        ## Usage notes

        Text.
        """,
    "crates/demo/src/lib.rs": """
        //! See [the guide](../../../docs/guide.md) and [`Thing`](crate::Thing).
        pub struct Thing;
        """,
    "dev/active/x/plan.md": """
        # Plan

        Back to [the readme](../../../README.md).

        ```text
        [not a link](missing.md) `docs/missing.md` @/inv/missing
        ```
        """,
    "dev/archive/old/plan.md": "[gone](missing.md) `docs/gone.md` @/inv/gone\n",
}


def parse(stdout: str) -> set[tuple[str, int, str]]:
    findings = set()
    for line in stdout.splitlines():
        if line.startswith("docs-mechanical:"):
            continue
        file, number, kind, _ = line.split(":", 3)
        findings.add((file, int(number), kind.strip()))
    return findings


class DocsMechanicalTest(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="gf2-docs-mechanical-test-")
        self.root = Path(self.scratch.name) / "repo"
        for relative, text in FIXTURE.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text if relative.endswith(".json") or text == RENDERED_GUIDE
                            else textwrap.dedent(text).lstrip())
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        self.jit = Path(self.scratch.name) / "jit"
        self.jit.write_text(FAKE_JIT.format(python=sys.executable))
        self.jit.chmod(0o755)

    def tearDown(self):
        self.scratch.cleanup()

    def run_check(self, jit: Path | None = None, **env: str) -> subprocess.CompletedProcess:
        environment = {key: value for key, value in os.environ.items()
                       if key not in {"JIT_DATA_DIR", "JIT_WORKTREE_ROOT"}}
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), "--jit", str(jit or self.jit)],
            capture_output=True, text=True, env={**environment, **env})

    def append(self, relative: str, text: str) -> int:
        path = self.root / relative
        with path.open("a") as handle:
            handle.write(text + "\n")
        return len(path.read_text().splitlines())

    def assert_only_finding(self, relative: str, line: int, kind: str):
        result = self.run_check()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(parse(result.stdout), {(relative, line, kind)})

    def test_clean_tree_passes(self):
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(parse(result.stdout), set())

    def test_broken_markdown_link(self):
        line = self.append("docs/guide.md", "See [absent](absent.md).")
        self.assert_only_finding("docs/guide.md", line, "broken-link")

    def test_broken_rustdoc_link(self):
        line = self.append("crates/demo/src/lib.rs", "/// See [notes](../NOTES.md).")
        self.assert_only_finding("crates/demo/src/lib.rs", line, "broken-link")

    def test_broken_heading_anchor(self):
        line = self.append("README.md", "See [setup](docs/guide.md#setup).")
        self.assert_only_finding("README.md", line, "broken-anchor")

    def test_line_anchor_past_end_of_file(self):
        line = self.append("README.md", "See [tail](docs/guide.md#L90).")
        self.assert_only_finding("README.md", line, "broken-anchor")

    def test_missing_citation(self):
        line = self.append("dev/active/x/plan.md", "Inputs: `docs/inputs.md`.")
        self.assert_only_finding("dev/active/x/plan.md", line, "missing-citation")

    def test_missing_bare_file_citation(self):
        line = self.append("README.md", "History lives in `CHANGELOG.md`.")
        self.assert_only_finding("README.md", line, "missing-citation")

    def test_unresolved_item_reference(self):
        line = self.append("docs/guide.md", "Applies @/inv/rule-z.")
        self.assert_only_finding("docs/guide.md", line, "unresolved-item")

    def test_unresolved_issue_reference(self):
        line = self.append("docs/guide.md", "Tracked by @/issue/ffff0000.")
        self.assert_only_finding("docs/guide.md", line, "unresolved-item")

    def test_projection_drift(self):
        (self.root / "GUIDE.md").write_text(RENDERED_GUIDE.replace("rule-a", "rule-b"))
        self.assert_only_finding("GUIDE.md", 4, "stale-projection")

    def snapshot(self, root: Path) -> dict[str, bytes]:
        return {path.relative_to(root).as_posix(): path.read_bytes()
                for path in root.rglob("*") if path.is_file()
                and ".git" not in path.relative_to(root).parts}

    def test_external_store_has_the_same_findings(self):
        self.append("docs/guide.md", "Applies @/inv/rule-z.")
        self.append("README.md", "See [registry](.jit/reference/projection.md#guide), "
                    "cited as `.jit/reference/projection.md`.")
        (self.root / "GUIDE.md").write_text(RENDERED_GUIDE.replace("rule-a", "rule-b"))
        expected = self.run_check()
        self.assertEqual(expected.returncode, 1, expected.stdout + expected.stderr)
        source = Path(self.scratch.name) / "source"
        (self.root / ".jit").rename(source)
        for nested in (False, True):
            with self.subTest(nested=nested):
                repository = Path(self.scratch.name) / ("nested" if nested else "standalone")
                data = repository / "tracker" if nested else repository
                data.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(source, data)
                subprocess.run(["git", "init", "-q", str(repository)], check=True)
                before_code, before_data = self.snapshot(self.root), self.snapshot(data)
                result = self.run_check(JIT_DATA_DIR=str(data))
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(result.stdout, expected.stdout)
                self.assertEqual(self.snapshot(self.root), before_code)
                self.assertEqual(self.snapshot(data), before_data)
                self.assertFalse((self.root / ".jit").exists())

    def test_external_tracker_projection_drift(self):
        (self.root / ".jit/reference/projection.md").write_text(RENDERED_GUIDE.replace("rule-a", "rule-b"))
        expected = self.run_check()
        self.assertEqual(parse(expected.stdout), {(".jit/reference/projection.md", 4, "stale-projection")})
        data = Path(self.scratch.name) / "tracker"
        (self.root / ".jit").rename(data)
        subprocess.run(["git", "init", "-q", str(data)], check=True)
        result = self.run_check(JIT_DATA_DIR=str(data))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(result.stdout, expected.stdout)
        self.assertFalse((self.root / ".jit").exists())

    def test_inherited_roots_cannot_redirect_scratch_render(self):
        for external in (False, True):
            with self.subTest(external=external):
                data = self.root / ".jit"
                if external:
                    data = Path(self.scratch.name) / "external"
                    (self.root / ".jit").rename(data)
                    subprocess.run(["git", "init", "-q", str(data)], check=True)
                (self.root / "GUIDE.md").write_text(RENDERED_GUIDE.replace("rule-a", "rule-b"))
                before_code, before_data = self.snapshot(self.root), self.snapshot(data)
                result = self.run_check(JIT_DATA_DIR=str(data), JIT_WORKTREE_ROOT=str(self.root))
                self.assertEqual(self.snapshot(self.root), before_code)
                self.assertEqual(self.snapshot(data), before_data)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(parse(result.stdout), {("GUIDE.md", 4, "stale-projection")})

    def test_root_overrides_inherited_worktree_for_item_resolution(self):
        other = Path(self.scratch.name) / "other-checkout"
        other.mkdir()
        result = self.run_check(JIT_DATA_DIR=str(self.root / ".jit"), JIT_WORKTREE_ROOT=str(other))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(other), {})

    def test_terminal_newline_drift(self):
        (self.root / "GUIDE.md").write_text(RENDERED_GUIDE.rstrip("\n"))
        self.assert_only_finding("GUIDE.md", 5, "stale-projection")

    def test_baseline_suppresses_only_recorded_findings(self):
        recorded = self.append("dev/active/x/plan.md", "Old [link](gone.md).")
        unrecorded = self.append("dev/active/x/plan.md", "New [link](absent.md).")
        config = self.root / "contrib/gates/docs-mechanical.toml"
        config.write_text(config.read_text().replace(
            'exclude = ["dev/archive"]', 'exclude = ["dev/archive"]\nbaseline = "dev/active/x/baseline.md"'))
        (self.root / "dev/active/x/baseline.md").write_text(
            "| Location | Class | Target |\n|---|---|---|\n"
            f"| `dev/active/x/plan.md:{recorded}` | `broken-link` | `gone.md` |\n")
        self.assertNotEqual(recorded, unrecorded)
        self.assert_only_finding("dev/active/x/plan.md", unrecorded, "broken-link")

    def test_missing_jit_is_an_environment_error(self):
        result = self.run_check(jit=Path(self.scratch.name) / "absent-jit")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(parse(result.stdout), set())

    def test_failing_jit_is_an_environment_error(self):
        result = self.run_check(FAKE_JIT_BROKEN="1")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(parse(result.stdout), set())


if __name__ == "__main__":
    unittest.main()
