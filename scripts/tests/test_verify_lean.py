"""Failed extraction must not verify stale committed artifacts."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).parents[1] / "verify-lean.sh"


class FreshExtractionTests(unittest.TestCase):
    def run_failed_extraction(self, charon_writes: bool) -> str:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            shutil.copyfile(SCRIPT, root / "scripts/verify-lean.sh")
            binaries = root / "tools/bin"
            binaries.mkdir(parents=True)
            charon = binaries / "charon"
            charon.write_text(
                "#!/usr/bin/env python3\n"
                "import sys\nfrom pathlib import Path\n"
                + ("Path(sys.argv[sys.argv.index('--dest-file') + 1]).write_text('fresh')\n"
                   if charon_writes else "")
            )
            charon.chmod(0o755)
            shutil.copyfile(charon, binaries / "charon-driver")
            (binaries / "charon-driver").chmod(0o755)
            aeneas = binaries / "aeneas"
            aeneas.write_text("#!/bin/sh\nexit 1\n")
            aeneas.chmod(0o755)
            artifacts = root / "target/charon"
            artifacts.mkdir(parents=True)
            for crate in ("gf2_core", "gf2_algebra"):
                (artifacts / f"{crate}.llbc").write_text("stale")
            snapshots = {}
            for crate in ("Gf2Core", "Gf2Algebra"):
                generated = root / "proofs" / crate
                generated.mkdir(parents=True)
                for name in ("Types.lean", "Funs.lean", "FunsExternal_Template.lean",
                             "TypesExternal_Template.lean", "FunsExternal.lean"):
                    path = generated / name
                    path.write_text("stale")
                    snapshots[path] = path.read_bytes()
            env = dict(os.environ, LEAN_TOOLS_DIR=str(root / "tools"))
            env["PATH"] = f"{binaries}:{env['PATH']}"
            result = subprocess.run(
                ["bash", str(root / "scripts/verify-lean.sh")],
                env=env, text=True, capture_output=True, check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            for path, contents in snapshots.items():
                self.assertEqual(path.read_bytes(), contents)
            return result.stdout + result.stderr

    def test_missing_charon_output_cannot_reuse_previous_llbc(self):
        output = self.run_failed_extraction(charon_writes=False)
        self.assertIn("Charon did not produce", output)
        self.assertNotIn("Aeneas translation", output)

    def test_missing_aeneas_output_cannot_reuse_committed_lean(self):
        output = self.run_failed_extraction(charon_writes=True)
        self.assertIn("Aeneas failed to generate required files", output)
        self.assertNotIn("Post-processing", output)


if __name__ == "__main__":
    unittest.main()
