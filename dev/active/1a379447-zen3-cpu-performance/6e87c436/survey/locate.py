"""Shared locators for this task's generators (jit:6e87c436).

Importing this module puts the shared `repository_files` and the dense story's
`repo_artifacts` on `sys.path`; both resolve files under the root git reports.
"""

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
import asm_listing  # noqa: E402
import content_anchor  # noqa: E402
import repository_files  # noqa: E402
import rust_code_text  # noqa: E402

sys.path.insert(0, str(ROOT / Path(repository_files.live_file(ROOT, "repo_artifacts.py")).parent))
import repo_artifacts  # noqa: E402

# The dense families' receipts, by the campaign each records.
CAMPAIGNS = [
    "v4-r1-2037941f-dense-isolated-fused-parity",
    "v4-r1-2037941f-dense-allocated-matvec",
    "v4-r1-2037941f-dense-matvec-vs-m4ri",
    "v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri",
]

# The production packages the dense campaigns measure.
PACKAGES = ["gf2-core", "gf2-kernels-simd"]

# The task anchor by content: per-path digests, and byte snapshots of the
# paths this task changes. An `inputs` directory is outside every live lookup.
ANCHOR = content_anchor.Anchor(ROOT, HERE / "anchor-baseline.json", HERE / "inputs" / "anchor")

# The tree the verdict's records describe, by per-path digest. The file is
# located by name once it exists; the freezer writes it beside the records.
END_STATE_FILE = "dense-verdict-end-state.json"
END_STATE_SCHEMA = "dense-verdict-end-state-v1"


def end_state():
    """The pinned end state of the packages the records describe."""
    return content_anchor.Anchor(ROOT, ROOT / repository_files.live_file(ROOT, END_STATE_FILE))


def byte_class(path, before, after):
    """`comment-or-blank-only` for a Rust source whose code text agrees, else `code-differs`."""
    same = path.endswith(".rs") and rust_code_text.code_text(
        before.decode()
    ) == rust_code_text.code_text(after.decode())
    return "comment-or-blank-only" if same else "code-differs"
