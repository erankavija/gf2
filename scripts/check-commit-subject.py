#!/usr/bin/env python3
"""Check commit subjects against the convention documented in AGENTS.md."""
# Check the first line verbatim; commit-msg cannot see CLI --cleanup overrides.
# Leading comments/blank lines are rejected even when Git cleanup would strip them.
# Scope: mistakes in ordinary `git commit` use, not crafted message bytes.

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TYPE_SENTENCE = re.compile(r"with type one of(?P<types>.*?)the list is closed", re.DOTALL)
TYPE_NAME = re.compile(r"[a-z][a-z0-9-]*\Z")
SUBJECT_SYNTAX = re.compile(
    r"(?P<type>[a-z][a-z0-9-]*)\((?P<scope>[^()\s]+)\): \S.*\Z"
)
MAX_SUBJECT_LENGTH = 71


def parse_allowed_types(agents_text: str) -> tuple[str, ...]:
    match = TYPE_SENTENCE.search(agents_text)
    if match is None:
        raise ValueError("could not find the closed commit type list in AGENTS.md")
    types = tuple(re.findall(r"`([^`]+)`", match.group("types")))
    if not types:
        raise ValueError("the closed commit type list in AGENTS.md is empty")
    if any(TYPE_NAME.fullmatch(name) is None for name in types):
        raise ValueError("the closed commit type list in AGENTS.md has an invalid entry")
    if len(set(types)) != len(types):
        raise ValueError("the closed commit type list in AGENTS.md has duplicate entries")
    return types


def load_allowed_types(root: Path = ROOT) -> tuple[str, ...]:
    return parse_allowed_types((root / "AGENTS.md").read_text(encoding="utf-8"))


def first_subject(message: str) -> str | None:
    # Git ends a line only at "\n"; its default cleanup drops trailing "\r".
    return message.split("\n", 1)[0].rstrip("\r") if message else None


def allowed_subject_pattern(allowed_types: tuple[str, ...]) -> re.Pattern[str]:
    types = "|".join(re.escape(name) for name in allowed_types)
    return re.compile(rf"(?P<type>{types})\((?P<scope>[^()\s]+)\): \S.*\Z")


def subject_faults(message: str, allowed_types: tuple[str, ...]) -> list[str]:
    subject = first_subject(message)
    if subject is None or subject == "":
        return ["subject is empty"]

    faults = []
    if len(subject) > MAX_SUBJECT_LENGTH:
        faults.append(f"subject has {len(subject)} characters; maximum is {MAX_SUBJECT_LENGTH}")

    match = allowed_subject_pattern(allowed_types).fullmatch(subject)
    if match is None:
        syntax_match = SUBJECT_SYNTAX.fullmatch(subject)
        if syntax_match is None:
            faults.append("subject must match <type>(<scope>): <summary>")
        else:
            faults.append(f"type {syntax_match.group('type')!r} is not allowed")
    return faults


def self_test() -> int:
    hook_types = load_allowed_types(ROOT)
    for name, text in (
        ("missing sentence", "Commit subjects follow a convention.\n"),
        ("empty list", "with type one of nothing; the list is closed.\n"),
    ):
        try:
            parse_allowed_types(text)
        except ValueError:
            continue
        raise SystemExit(f"check-commit-subject: self-test: {name}: expected a parse error")

    prefix = "feat(jit:abcd1234): "
    subject_71 = prefix + "x" * (MAX_SUBJECT_LENGTH - len(prefix))
    cases = (
        ("issue subject", "fix(jit:c34e8203): reject nonconforming subjects\n", True),
        (
            "merge commit subject",
            "chore(jit:1b7228b9): merge host-wide GPU lock for concurrent test runs\n",
            True,
        ),
        ("71-character subject", subject_71 + "\n", True),
        (
            "editor template with trailing comments",
            "fix(jit:c34e8203): subject before comments\n\n# commit message template\n# another comment\n",
            True,
        ),
        (
            "-m leading comment retained by default cleanup",
            "# x\nfeat(jit:abc): y\n",
            False,
        ),
        (
            "-m leading comment paragraph retained by default cleanup",
            "# x\n\nfeat(jit:abc): y\n",
            False,
        ),
        (
            "-F leading comments retained by default cleanup",
            "# commit message template\n# another comment\nfix(jit:c34e8203): subject after comments\n",
            False,
        ),
        (
            "custom comment character on first line",
            "; comment\nfix(jit:c34e8203): subject after comments\n",
            False,
        ),
        ("72-character subject", subject_71 + "x\n", False),
        ("overlength subject with a Unicode line separator", subject_71[:40] + "\u2028" + subject_71[40:] + "\n", False),
        ("CRLF line ending", subject_71 + "\r\n", True),
        ("unknown type", "wip(jit:abcd1234): x\n", False),
        ("default merge subject", "Merge branch 'feature' into main\n", False),
        ("missing scope", "fix: missing scope\n", False),
        ("empty scope", "fix(): empty scope\n", False),
        ("capitalized type", "Fix(jit:abcd1234): capitalised type\n", False),
        ("empty message", "", False),
    )
    for name, message, accepted in cases:
        faults = subject_faults(message, hook_types)
        if bool(faults) == accepted:
            expectation = "acceptance" if accepted else "rejection"
            raise SystemExit(
                f"check-commit-subject: self-test: {name}: expected {expectation}, got {faults}"
            )
    print("check-commit-subject: self-test ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("message_file", nargs="?")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.message_file is None:
        parser.error("message_file is required unless --self-test is used")

    try:
        allowed_types = load_allowed_types()
    except (OSError, ValueError) as error:
        print(f"check-commit-subject: cannot load allowed types: {error}", file=sys.stderr)
        return 2
    try:
        # Bytes first: text mode would turn a lone "\r" into a line break.
        message = Path(args.message_file).read_bytes().decode("utf-8")
    except (OSError, UnicodeError) as error:
        print(f"check-commit-subject: cannot read {args.message_file}: {error}", file=sys.stderr)
        return 2

    faults = subject_faults(message, allowed_types)
    if faults:
        types = ", ".join(allowed_types)
        print(
            "check-commit-subject: rejected: "
            f"{'; '.join(faults)}. Allowed types: {types}.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
