"""Shared locators of the decoder calibration generators (jit:0a77ecfc).

Importing this module puts the shared `repository_files` and
`producing_closure` on `sys.path`; every path resolves under the root git
reports.
"""

import hashlib
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # a check writes nothing, however invoked

HERE = Path(__file__).resolve().parent
ROOT = Path(
    subprocess.run(
        ["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
)
ISSUE = "0a77ecfc"


def tracked(pattern):
    """Sorted root-relative tracked or unignored paths matching the glob `pattern`."""
    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--cached", "--others",
         "--exclude-standard", "--", f":(glob){pattern}"],
        capture_output=True, check=True, text=True,
    ).stdout
    return sorted(path for path in listing.split("\0") if path and (ROOT / path).is_file())


def _shared_scripts():
    """Root-relative directory of the one `repository_files.py` outside receipt snapshots."""
    live = [path for path in tracked("**/repository_files.py")
            if "inputs" not in Path(path).parts[:-1]]
    if len(live) != 1:
        raise SystemExit(f"{len(live)} live repository_files.py files; exactly one must exist")
    return str(Path(live[0]).parent)


SHARED = _shared_scripts()
sys.path.insert(0, str(ROOT / SHARED))
import producing_closure  # noqa: E402,F401
import repository_files  # noqa: E402

CALIBRATION = str(HERE.relative_to(ROOT))


def package(name):
    """Root-relative directory of the live Cargo package `name`."""
    return repository_files.package_directory(ROOT, name)


def sha256(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def emit(output, text, check, regenerate):
    """Writes `text` to `output`; with `check`, writes nothing and exits non-zero on a difference."""
    relative = output.relative_to(ROOT)
    if not check:
        output.write_text(text)
        print(f"wrote {relative}")
    elif not output.is_file() or output.read_text() != text:
        raise SystemExit(f"{relative} differs from its generator; regenerate it with {regenerate}")
    else:
        print(f"{relative}: matches its generator")
