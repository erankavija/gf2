"""Gate reading shared by scripts/check-landing and scripts/check-landed.

Runs `jit ... --json` from PATH with the caller's environment, classifies a
gate definition by how its verdict is reached, and follows a reused verdict to
the run that executed the checker.
"""

import json
import subprocess
from dataclasses import dataclass
from enum import Enum


class JitUnavailable(Exception):
    pass


class JitError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


class GateKind(Enum):
    EXECUTED = "executed"
    MANUAL = "manual"
    BUILTIN = "builtin"


class GateMode(Enum):
    AUTO = "auto"
    MANUAL = "manual"


class CheckerType(Enum):
    EXEC = "exec"
    ISSUE_VALIDATION = "issue_validation"
    LABEL_TARGET_VALIDATION = "label_target_validation"
    REPOSITORY_VALIDATION = "repository_validation"


class RunStatus(Enum):
    PASSED = "passed"
    FAILED = "failed"
    PENDING = "pending"


class RunDerivation(Enum):
    EXECUTED = "executed"
    REUSED = "reused"
    ATTESTED = "attested"


class GateKeyKind(Enum):
    REQUIRED = "required"
    LANDED = "landed"


@dataclass(frozen=True)
class GateKey:
    value: str
    kind: GateKeyKind


class GateLabel(Enum):
    COUNTS = "counts"
    REFUSED = "refused"
    ATTESTED = "attested"
    STORE_JUDGED = "store-judged"
    JUDGED_AFTER_MERGE = "judged after the merge"
    ON_MAIN = "on main"
    NOT_LANDED = "not landed"


def _parse(enum_type, value, vocabulary):
    try:
        return enum_type(value)
    except (TypeError, ValueError) as error:
        raise JitError(None, f"jit returned unknown {vocabulary} {value!r}") from error


def parse_gate_mode(value):
    """Parses the optional mode from a jit gate definition."""
    return None if value is None else _parse(GateMode, value, "gate mode")


def parse_checker_type(value):
    """Parses the optional checker type from a jit gate definition."""
    return None if value is None else _parse(CheckerType, value, "checker type")


def parse_run_status(value):
    """Parses a run status, preserving the absent status of a never-run gate."""
    return None if value is None else _parse(RunStatus, value, "gate run status")


def parse_run_derivation(value):
    """Parses a run origin derivation when one is present."""
    return None if value is None else _parse(RunDerivation, value, "run derivation")


def parse_gate_key(value):
    """Parses a registry key and identifies the special post-merge gate."""
    if not isinstance(value, str):
        raise JitError(None, f"jit returned invalid gate key {value!r}")
    kind = GateKeyKind.LANDED if value == GateKeyKind.LANDED.value else GateKeyKind.REQUIRED
    return GateKey(value, kind)


def parse_gate_run(run):
    """Validates run vocabularies at the jit JSON boundary."""
    parse_run_status(run.get("status"))
    parse_run_derivation((run.get("origin") or {}).get("derivation"))
    return run


def run(argv):
    """Returns (exit status, standard output, standard error) as text."""
    with subprocess.Popen(
        argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
    ) as proc:
        out, err = proc.communicate()
    return proc.returncode, out, err


def jit(*args):
    try:
        status, out, err = run(["jit", *args, "--json"])
    except OSError as error:
        raise JitUnavailable(f"cannot run jit: {error}") from error
    try:
        data = json.loads(out)
    except json.JSONDecodeError:
        data = None
    if status != 0:
        error = data.get("error", {}) if isinstance(data, dict) else {}
        message = error.get("message") or err.strip() or out.strip()
        raise JitError(
            error.get("code"), f"jit {' '.join(args)} exited {status}: {message}"
        )
    if not isinstance(data, dict):
        raise JitError(None, f"jit {' '.join(args)} printed no JSON object")
    return data


def git(*args):
    return run(["git", *args])


def gate_kind(definition):
    """Returns the typed kind of a gate definition.

    Only an executed command's run records the commit it ran at.
    """
    mode = parse_gate_mode(definition.get("mode"))
    checker = parse_checker_type((definition.get("checker") or {}).get("type"))
    if mode is GateMode.MANUAL:
        return GateKind.MANUAL
    if checker is CheckerType.EXEC:
        return GateKind.EXECUTED
    return GateKind.BUILTIN


def is_reused(run):
    derivation = parse_run_derivation((run.get("origin") or {}).get("derivation"))
    return derivation is RunDerivation.REUSED


def executing_run(key, run):
    """Follows reused verdicts to the run that executed the checker, or None."""
    seen = set()
    while is_reused(run):
        origin = run["origin"]
        source = (origin.get("source_issue"), origin.get("source_run"))
        if None in source or source in seen:
            return None
        seen.add(source)
        try:
            history = jit("gate", "status", source[0], "--all", "--gate", key.value)
        except JitError as err:
            if err.code == "ISSUE_NOT_FOUND":
                return None
            raise
        run = next(
            (r for r in history.get("results", []) if r.get("run_id") == source[1]),
            None,
        )
        if run is None:
            return None
        parse_gate_run(run)
    return run
