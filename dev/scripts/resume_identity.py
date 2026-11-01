#!/usr/bin/env python3
"""Resume gate shared by the append-only profile logs.

An identity is the executables' digests and `configuration`, the digest of the
settings, the parsed input files and the launcher scripts' measurement commands,
so comments, formatting and documentation never decide resume. Prints the identity
the log's session records carry; exit 2 refuses.

Usage: resume_identity.py --log <file> [--recorded-digests <file>]...
           [--executable <name>=<file>]... [--script <name>=<file>]...
           [--measured-lines <regex>] [--setting <name>=<value>]...
           [--observed <name>=<value>]...  (a value no older script states)
           [--table <name>=<file>]... [--table-drop <line-prefix>]...
           [--table-file-name-column <separator> <index>]
           [--json <name>=<file>]... [--shell <name>=<file>]...
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
FIELD = re.compile(r"(\S+?)=([0-9a-f]{64})(?=\s|$)")
DIGEST_LINE = re.compile(r"^([0-9a-f]{64})\s+(\S+)$")
# bash prints a function's parsed form, which holds no shell comment and one line
# per simple command.
CANONICAL_SHELL = 'body=$(cat); eval "resume_identity_body() {\n$body\n}" && declare -f resume_identity_body'
PYTHON_HEREDOC = re.compile(r"(python3?\b[^\n]*<<\s*['\"]?(\w+)['\"]?[^\n]*\n)(.*?)(\n\2$)",
                            re.DOTALL | re.MULTILINE)
PYTHON_ARGUMENT = re.compile(r"(python3?\s+(?:-\S+\s+)*-c\s+')([^']*)(')")
ASSIGNMENT = re.compile(r"^    ([A-Z_]+)=([^$`()\n]*?);?$", re.MULTILINE)
MEASURED_COMMAND = r"\bperf (?:stat|record|report|annotate)\b|\btaskset\b|--full-host"
HOST_SCOPE = "--full-host"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def significant_lines(text):
    return [line.rstrip() for line in text.splitlines()
            if line.strip() and not line.lstrip().startswith("#")]


def without_documentation(value):
    if isinstance(value, dict):
        return {key: without_documentation(item) for key, item in value.items()
                if key != "description"}
    if isinstance(value, list):
        return [without_documentation(item) for item in value]
    return value


def table_row(line, file_name_column):
    """A table line whose file column, if declared, names the file by its name alone."""
    if not file_name_column:
        return line
    separator, index = file_name_column[0], int(file_name_column[1])
    columns = line.split(separator)
    if len(columns) > index:
        columns[index] = Path(columns[index]).name
    return separator.join(columns)


def python_program(source):
    """Python's syntax tree, free of comments; a program that does not parse stays as bytes."""
    try:
        return ast.dump(ast.parse(source))
    except SyntaxError:
        return source


def canonical_shell(data):
    """A script's parsed form, with its embedded Python programs as syntax trees."""
    canonical = subprocess.run(["bash", "--noprofile", "--norc", "-c", CANONICAL_SHELL],
                               input=data.decode(), capture_output=True, check=True,
                               text=True).stdout
    canonical = PYTHON_HEREDOC.sub(
        lambda match: match[1] + python_program(match[3]) + match[4], canonical)
    return PYTHON_ARGUMENT.sub(
        lambda match: match[1] + python_program(match[2]) + match[3], canonical)


def assignments(canonical):
    return {name: value.strip("\"'") for name, value in ASSIGNMENT.findall(canonical)}


def measured_commands(data, extra, full_dispatch=True):
    """A script's measurement commands with its constants expanded.

    An older script holds as literals what the current one holds in constants, and
    its dispatch passes other arguments, so `full_dispatch=False` compares only up
    to the host scope.
    """
    canonical = canonical_shell(data)
    constants = assignments(canonical)
    selected = re.compile(f"{MEASURED_COMMAND}|{extra}" if extra else MEASURED_COMMAND)
    commands = []
    for line in map(str.strip, canonical.splitlines()):
        if (not selected.search(line) or line.startswith(("printf ", "echo ", "#"))
                or "resume_identity" in line):
            continue
        for _ in range(3):
            line = re.sub(r"\$\{([A-Z_]+)\}", lambda m: constants.get(m[1], m[0]), line)
        line = line.replace('"', "")
        if not full_dispatch and HOST_SCOPE in line:
            line = line[:line.index(HOST_SCOPE) + len(HOST_SCOPE)]
        commands.append(line)
    return commands


def parsed_input(kind, data, arguments):
    if kind == "json":
        return without_documentation(json.loads(data))
    if kind == "table":
        return [table_row(line, arguments.table_file_name_column)
                for line in significant_lines(data.decode())
                if not line.startswith(tuple(arguments.table_drop))]
    return canonical_shell(data).splitlines()


def committed_bytes(path, digest):
    """The bytes of `path`'s history whose SHA-256 is `digest`, if any."""
    resolved = Path(path).resolve()
    git = ["git", "-C", str(resolved.parent)]
    history = subprocess.run([*git, "log", "--all", "--follow", "--format=%H", "--name-only",
                              "--", resolved.name], capture_output=True, text=True)
    names = history.stdout.split() if history.returncode == 0 else []
    for revision, name in zip(names[::2], names[1::2]):
        blob = subprocess.run([*git, "show", f"{revision}:{name}"], capture_output=True)
        if blob.returncode == 0 and sha256(blob.stdout) == digest:
            return blob.stdout
    return None


def log_records(log):
    """The identity text of each of the log's records, in log order."""
    path = Path(log)
    matches = map(RECORD.match, path.read_text().splitlines() if path.exists() else ())
    return [match.group("line") or match.group("start") or "" for match in matches if match]


def refuse(name, recorded, current):
    print(f"resume refused: {name} differs from the recorded identity\n"
          f"recorded: {recorded}\ncurrent:  {current}", file=sys.stderr)
    raise SystemExit(2)


def recorded_fields(record, digest_files):
    """Each field's digests, in record order, then any name only the digest files state."""
    fields = {}
    for name, digest in FIELD.findall(record):
        fields.setdefault(name, []).append(digest)
    listed = {}
    for digest_file in map(Path, digest_files):
        for line in digest_file.read_text().splitlines() if digest_file.exists() else ():
            row = DIGEST_LINE.match(line)
            if row:
                listed.setdefault(Path(row.group(2)).name, [row.group(1)])
    return {**listed, **fields}


def bytes_named(digest, path):
    data = Path(path).read_bytes()
    return data if sha256(data) == digest else committed_bytes(path, digest)


def check_older_scripts(record, current, fields, arguments):
    """Read the older record's settings and commands from the scripts it names by digest."""
    if not arguments.script:
        refuse("launcher script", record, current)
    for name, path in (item.split("=", 1) for item in arguments.script):
        digest = fields.get(name, [None])[0]
        older = bytes_named(digest, path) if digest else None
        if older is None:
            refuse(name, record, current)
        try:
            constants = assignments(canonical_shell(older))
            commands = measured_commands(older, arguments.measured_lines, full_dispatch=False)
        except subprocess.CalledProcessError:
            refuse(name, record, current)
        if commands != measured_commands(Path(path).read_bytes(), arguments.measured_lines,
                                         full_dispatch=False):
            refuse(f"{name} measurement commands", record, current)
        if name == arguments.script[0].split("=", 1)[0]:
            for setting, value in (item.split("=", 1) for item in arguments.setting):
                if constants.get(setting) != value:
                    refuse(setting, record, current)


def check_older_record(record, current, executables, inputs, arguments):
    fields = recorded_fields(record, arguments.recorded_digests)
    for name, digest in executables:
        if fields.get(name, [digest]) != [digest]:
            refuse(name, record, current)
    for name in dict.fromkeys(name for _, name, _ in inputs):
        same_name = [item for item in inputs if item[1] == name]
        digests = fields.get(name)
        if digests is None:
            continue
        if len(digests) != len(same_name):
            refuse(name, record, current)
        for digest, (kind, _, path) in zip(digests, same_name):
            data = Path(path).read_bytes()
            older = bytes_named(digest, path)
            if older is None or (parsed_input(kind, older, arguments)
                                 != parsed_input(kind, data, arguments)):
                refuse(name, record, current)
    check_older_scripts(record, current, fields, arguments)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", required=True)
    parser.add_argument("--recorded-digests", action="append", default=[])
    parser.add_argument("--table-drop", action="append", default=[])
    parser.add_argument("--table-file-name-column", nargs=2)
    parser.add_argument("--measured-lines", default="")
    for option in ("executable", "script", "setting", "observed", "table", "json", "shell"):
        parser.add_argument(f"--{option}", action="append", default=[])
    arguments = parser.parse_args()

    executables = []
    for item in arguments.executable:
        name, path = item.split("=", 1)
        executables.append((name, sha256(Path(path).read_bytes())))
    inputs = [(kind, *item.split("=", 1)) for kind in ("table", "json", "shell")
              for item in getattr(arguments, kind)]
    configuration = json.dumps(
        [arguments.setting, arguments.observed,
         [[kind, name, parsed_input(kind, Path(path).read_bytes(), arguments)]
          for kind, name, path in inputs],
         [measured_commands(Path(item.split("=", 1)[1]).read_bytes(), arguments.measured_lines)
          for item in arguments.script]],
        sort_keys=True, separators=(",", ":"),
    )
    current = " ".join([*(f"{name}={digest}" for name, digest in executables),
                        f"configuration={sha256(configuration.encode())}"])

    records = log_records(arguments.log)
    bound = [record for record in records if "configuration=" in record]
    if bound:
        if bound[-1] != current:
            differing = set(FIELD.findall(bound[-1])) ^ set(FIELD.findall(current))
            refuse(", ".join(sorted({name for name, _ in differing})), bound[-1], current)
    elif records:
        # Migration boundary, removed when no older log is resumed (@/issue/101604ef):
        # the appended record binds the rest of the log, the first record stays.
        check_older_record(records[0], current, executables, inputs, arguments)
        with Path(arguments.log).open("a") as log:
            log.write(f"identity {current}\n")
    print(records[0] if records else current)


if __name__ == "__main__":
    main()
