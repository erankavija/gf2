"""Repository and artifact paths shared by the comparison tools."""

import hashlib
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.check_output(
    ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], text=True).strip())
OUTPUT = ROOT / "target/aff3ct-ldpc-gaps"
INPUT_VAR = "GF2_LDPC_INPUT_ROOT"
# The shared campaign library's checkout location, which `cargo -p` and the
# addendum schema read; every other repository tool is found by `tracked`.
SUPPORT = ROOT / "dev/tools/tuning-campaign-support"


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".partial")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def input_root():
    value = os.environ.get(INPUT_VAR)
    if not value or not Path(value).is_dir():
        raise ValueError(f"{INPUT_VAR} must name the directory containing bundles/ and quality/")
    return Path(value).resolve()


def tracked(name):
    """The one tracked file named `name`, wherever the repository keeps it."""
    listing = subprocess.check_output(["git", "-C", str(ROOT), "ls-files", "-z"], text=True)
    found = [ROOT / entry for entry in listing.split("\0") if entry.rsplit("/", 1)[-1] == name]
    if len(found) != 1:
        raise ValueError(f"expected one tracked file named {name}, found {len(found)}")
    return found[0]


def repository_relative(path):
    """`path` as the plan and addendum schemas require: relative to the repository root."""
    path = Path(path).resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError(f"{path} is outside the repository, so a plan cannot name it")
    return str(path.relative_to(ROOT))


def output_path(path):
    path = Path(path).resolve()
    if not path.is_relative_to(ROOT / "target"):
        raise ValueError("campaign output must be under the repository target/ directory")
    return path


def build_tools():
    command = [str(ROOT / "scripts/cargo-budget.sh"), "cargo", "+1.95", "build", "--locked",
               "--release", "-p", "tuning-campaign-support", "--bin", "benchmark-ab-runner",
               "--bin", "benchmark-acceptance"]
    subprocess.run(command, cwd=ROOT, check=True)
    return ROOT / "target/release/benchmark-ab-runner"
