#!/usr/bin/env python3
"""Resume gate shared by the append-only profile logs.

An identity is the executables' digests and `configuration`, the digest of the
outside values, the parsed input files and the scripts' whole canonical forms, so
comments, formatting and documentation never decide resume and every other script
change refuses. Prints the identity; exit 2 refuses.

Usage: resume_identity.py --log <file> [--executable <name>=<file>]...
           [--script <name>=<file>]... [--setting <name>=<value>]...
           [--table <name>=<file>]... [--table-drop <line-prefix>]...
           [--json <name>=<file>]...
"""

import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

RECORD = re.compile(r"^(?:identity (?P<line>.*)|rep-\d+ start \S+ (?P<start>.*?) load=.*)$")
# bash prints a function's parsed form, which holds no shell comment.
CANONICAL_SHELL = 'body=$(cat); eval "resume_identity_body() {\n$body\n}" && declare -f resume_identity_body'
PYTHON_HEREDOC = re.compile(r"(python3?\b[^\n]*<<\s*['\"]?(\w+)['\"]?[^\n]*\n)(.*?)(\n\2$)",
                            re.DOTALL | re.MULTILINE)
PYTHON_ARGUMENT = re.compile(r"(python3?\s+(?:-\S+\s+)*-c\s+')([^']*)(')")
AWK_ARGUMENT = re.compile(r"(\bawk\b[^'\n]*')([^']*)(')")
AWK_TOKEN = re.compile(r'''"(?:\\.|[^"\\])*"|/(?:\\.|\[(?:\\.|[^]\\])*\]|[^/\\\n])+/|\#[^\n]*|.''', re.DOTALL)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def without_documentation(value):
    if isinstance(value, dict):
        return {key: without_documentation(item) for key, item in value.items()
                if key != "description"}
    if isinstance(value, list):
        return [without_documentation(item) for item in value]
    return value


def python_program(source):
    """Python's syntax tree, free of comments; a program that does not parse stays as bytes."""
    try:
        return ast.dump(ast.parse(source))
    except SyntaxError:
        return source


def awk_program(source):
    """An awk program without comments; a `/` after an operand divides."""
    kept, position = "", 0
    while position < len(source):
        text = AWK_TOKEN.match(source, position)[0]
        if text[0] == "/" and re.search(r"[\w)\]\"$]$", kept.rstrip()):
            text = "/"
        position += len(text)
        if text[0] != "#":
            kept += text
    return "\n".join(line.rstrip() for line in kept.splitlines() if line.strip())


def canonical_shell(data):
    """A script's parsed form with its embedded programs normalized by language.

    Python compares as a syntax tree and awk without comments; other text as bytes.
    """
    canonical = subprocess.run(["bash", "--noprofile", "--norc", "-c", CANONICAL_SHELL],
                               input=data.decode(), capture_output=True, check=True,
                               text=True).stdout
    canonical = PYTHON_HEREDOC.sub(
        lambda match: match[1] + python_program(match[3]) + match[4], canonical)
    for pattern, program in ((PYTHON_ARGUMENT, python_program), (AWK_ARGUMENT, awk_program)):
        canonical = pattern.sub(lambda m, p=program: m[1] + p(m[2]) + m[3], canonical)
    return canonical


def first_record(log):
    """The identity text of the log's first record, or None."""
    path = Path(log)
    for line in path.read_text().splitlines() if path.exists() else ():
        record = RECORD.match(line)
        if record:
            return record["line"] or record["start"] or ""
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", required=True)
    parser.add_argument("--table-drop", action="append", default=[])
    for option in ("executable", "script", "setting", "table", "json"):
        parser.add_argument(f"--{option}", action="append", default=[])
    arguments = parser.parse_args()

    named = {option: [item.split("=", 1) for item in getattr(arguments, option)]
             for option in ("executable", "script", "table", "json")}
    tables = [[name, [line for line in map(str.rstrip, Path(path).read_text().splitlines())
                      if line.strip()
                      and not line.lstrip().startswith(("#", *arguments.table_drop))]]
              for name, path in named["table"]]
    documents = [[name, without_documentation(json.loads(Path(path).read_bytes()))]
                 for name, path in named["json"]]
    scripts = [[name, canonical_shell(Path(path).read_bytes()).splitlines()]
               for name, path in named["script"]]
    configuration = json.dumps([arguments.setting, tables, documents, scripts],
                               sort_keys=True, separators=(",", ":"))
    current = " ".join([*(f"{name}={sha256(Path(path).read_bytes())}"
                          for name, path in named["executable"]),
                        f"configuration={sha256(configuration.encode())}"])

    record = first_record(arguments.log)
    if record is not None and "configuration=" not in record:
        print(f"resume refused: {arguments.log} records an older identity form and does not "
              "resume; start a new log in a new output directory", file=sys.stderr)
        raise SystemExit(2)
    if record is not None and record != current:
        print(f"resume refused: the identity differs from the recorded one\n"
              f"recorded: {record}\ncurrent:  {current}", file=sys.stderr)
        raise SystemExit(2)
    print(current)


if __name__ == "__main__":
    main()
