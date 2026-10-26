"""Resolve profile preparation inputs through the repository file locator."""
from functools import cache
import importlib.util
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
files = subprocess.check_output(['git', 'ls-files', '--', ':(glob)**/repository_files.py',
                                 ':(exclude,glob)**/inputs/**'], text=True).splitlines()
(helper,) = files
sys.path.insert(0, str((ROOT / helper).parent))
import repository_files


@cache
def live(suffix):
    return ROOT / repository_files.live_file(ROOT, suffix)


def load(suffix, name):
    path = live(suffix)
    sys.path.insert(0, str(path.parent))
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(name, path))
    module.__spec__.loader.exec_module(module)
    return module
