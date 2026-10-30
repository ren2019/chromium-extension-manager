"""Exercise the release gate using actual Cargo metadata and Git ancestry."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("release-metadata.py").resolve()


class ReleaseMetadataTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "src").mkdir()
        (self.root / "src/main.rs").write_text("fn main() {}\n")
        self.version("0.1.0")
        self.git("init", "-b", "main")
        self.git("config", "user.email", "release-test@example.invalid")
        self.git("config", "user.name", "Release test")
        self.git("add", ".")
        self.git("commit", "-m", "initial")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def version(self, value):
        (self.root / "Cargo.toml").write_text(
            f'[package]\nname = "cepm-app"\nversion = "{value}"\nedition = "2024"\n'
        )
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, check=True, capture_output=True)

    def run_gate(self, ref):
        return subprocess.run(
            ["python3", str(SCRIPT)], cwd=self.root, capture_output=True, text=True,
            env={**os.environ, "GITHUB_REF": ref, "GITHUB_OUTPUT": str(self.root / "outputs")},
        )

    def test_manual_build_does_not_require_a_tag(self):
        result = self.run_gate("refs/heads/feature")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "outputs").read_text(), "version=0.1.0\ntag=v0.1.0\nprerelease=false\n")

    def test_matching_tag_on_main_is_accepted(self):
        result = self.run_gate("refs/tags/v0.1.0")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_mismatched_tag_is_rejected(self):
        result = self.run_gate("refs/tags/v0.2.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match", result.stderr)
        self.assertFalse((self.root / "outputs").exists())

    def test_unmerged_tag_is_rejected(self):
        (self.root / "change.txt").write_text("unmerged")
        self.git("add", "change.txt")
        self.git("commit", "-m", "unmerged")
        result = self.run_gate("refs/tags/v0.1.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "outputs").exists())

    def test_prerelease_is_marked(self):
        self.version("0.2.0-rc.1")
        self.git("add", "Cargo.toml")
        self.git("commit", "-m", "prerelease")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        result = self.run_gate("refs/tags/v0.2.0-rc.1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("prerelease=true\n", result.stdout)

    def test_version_inspection_does_not_rewrite_lockfile(self):
        lock = (self.root / "Cargo.lock").read_bytes()
        manifest = self.root / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('"0.1.0"', '"0.1.1"'))
        result = self.run_gate("refs/heads/main")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("version=0.1.1\n", result.stdout)
        self.assertEqual((self.root / "Cargo.lock").read_bytes(), lock)


if __name__ == "__main__":
    unittest.main()
