"""Release layout and failure checks; no provider, keyring or publishing calls."""
import hashlib
from pathlib import Path
import tempfile
import unittest
import zipfile

import release


class ReleasePackaging(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binaries = self.root / "bin"
        self.binaries.mkdir()
        for name in ("saucepan", "saucepan-shim", "saucepan.exe", "saucepan-shim.exe"):
            (self.binaries / name).write_bytes(f"fixture {name}".encode())
        self.tag = "v" + release.tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
        self.dist = self.root / "dist"

    def test_all_platform_packages_preserve_both_binaries_and_checksums(self):
        for target in release.TARGETS:
            path = release.package(self.tag, target, self.binaries, self.dist)
            suffix = ".exe" if "windows" in target else ""
            with zipfile.ZipFile(path) as archive:
                for name in (f"saucepan{suffix}", f"saucepan-shim{suffix}"):
                    self.assertEqual(archive.read(name), (self.binaries / name).read_bytes())
                    self.assertEqual(archive.getinfo(name).external_attr >> 16 & 0o777, 0o755)
        release.checksums(self.tag, self.dist)
        for line in (self.dist / "SHA256SUMS").read_text().splitlines():
            digest, name = line.split("  ")
            self.assertEqual(digest, hashlib.sha256((self.dist / name).read_bytes()).hexdigest())

    def test_missing_shim_cannot_publish_a_package(self):
        (self.binaries / "saucepan-shim.exe").unlink()
        with self.assertRaises(ValueError):
            release.package(self.tag, "x86_64-pc-windows-msvc", self.binaries, self.dist)
        self.assertFalse(self.dist.exists())

    def test_incomplete_matrix_cannot_publish_checksums(self):
        release.package(self.tag, release.TARGETS[0], self.binaries, self.dist)
        with self.assertRaises(ValueError):
            release.checksums(self.tag, self.dist)
        self.assertFalse((self.dist / "SHA256SUMS").exists())

    def test_existing_archive_is_not_overwritten(self):
        release.package(self.tag, release.TARGETS[0], self.binaries, self.dist)
        with self.assertRaises(FileExistsError):
            release.package(self.tag, release.TARGETS[0], self.binaries, self.dist)

    def test_tag_must_equal_package_version_and_cannot_be_a_path(self):
        for tag in ("main", "v99999.0.0", "../v0.1.0", self.tag + "\nother=value"):
            with self.assertRaises(ValueError):
                release.validate_tag(tag)


if __name__ == "__main__":
    unittest.main()
