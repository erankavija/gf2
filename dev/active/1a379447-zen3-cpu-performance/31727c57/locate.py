"""Resolve shared family tools through the repository file locator."""

import importlib.util
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.check_output(
    ["git", "rev-parse", "--show-toplevel"], text=True).strip())
listing = subprocess.check_output([
    "git", "ls-files", "--", ":(glob)**/repository_files.py",
    ":(exclude,glob)**/inputs/**"], text=True).splitlines()
sys.path.insert(0, str((ROOT / min(listing, key=len)).parent))
import repository_files  # noqa: E402


def load(suffix, name):
    path = ROOT / repository_files.live_file(ROOT, suffix)
    sys.path.insert(0, str(path.parent))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def live(suffix):
    return repository_files.live_file(ROOT, suffix)
