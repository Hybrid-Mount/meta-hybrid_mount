# SPDX-License-Identifier: GPL-3.0-only

import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "notify_source", Path(__file__).with_name("notify_source.py")
)
notify_source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notify_source)


class NotifySourceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name, content in {
            "Cargo.toml": '[package]\nversion = "1.0.0"\n[profile.release]\nstrip = "symbols"\n',
            "tools/notify/Cargo.toml": '[package]\nname = "notify"\nversion = "0.1.0"\n',
            "tools/notify/src/main.rs": "fn main() {}\n",
            ".github/workflows/notify-tool.yml": "build: notifier\n",
        }.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.packages = [
            {"name": "notify", "version": "0.1.0", "dependencies": ["tls 1.0.1"]},
            {"name": "tls", "version": "1.0.1", "checksum": "old", "dependencies": ["roots"]},
            {"name": "roots", "version": "1.0.0", "checksum": "roots"},
            {"name": "xtask", "version": "0.1.0"},
            {"name": "tls", "version": "2.0.0", "checksum": "unrelated"},
        ]

    def fingerprint(self):
        return notify_source.fingerprint(self.root, self.packages)

    def test_release_version_and_unrelated_dependencies_do_not_rebuild(self):
        before = self.fingerprint()
        (self.root / "Cargo.toml").write_text(
            '[package]\nversion = "9.0.0"\n[profile.release]\nstrip = "symbols"\n',
            encoding="utf-8",
        )
        self.packages[3]["version"] = "3.0.0"
        self.packages[4]["checksum"] = "new-unrelated"
        self.assertEqual(before, self.fingerprint())

    def test_source_change_rebuilds(self):
        before = self.fingerprint()
        (self.root / "tools/notify/src/main.rs").write_text("fn main() { todo!() }\n", encoding="utf-8")
        self.assertNotEqual(before, self.fingerprint())

    def test_transitive_dependency_change_rebuilds(self):
        before = self.fingerprint()
        self.packages[2]["checksum"] = "updated-roots"
        self.assertNotEqual(before, self.fingerprint())

    def test_profile_and_build_recipe_changes_rebuild(self):
        before = self.fingerprint()
        (self.root / "Cargo.toml").write_text('[profile.release]\nstrip = false\n', encoding="utf-8")
        changed = self.fingerprint()
        self.assertNotEqual(before, changed)
        (self.root / ".github/workflows/notify-tool.yml").write_text("build: changed\n", encoding="utf-8")
        self.assertNotEqual(changed, self.fingerprint())

    def test_missing_or_ambiguous_dependencies_fail(self):
        self.packages[0]["dependencies"] = ["tls"]
        with self.assertRaises(ValueError):
            self.fingerprint()
        self.packages[0]["dependencies"] = ["missing"]
        with self.assertRaises(ValueError):
            self.fingerprint()

    def test_lockfile_order_and_line_endings_do_not_rebuild(self):
        before = self.fingerprint()
        self.packages.reverse()
        path = self.root / "tools/notify/src/main.rs"
        path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        self.assertEqual(before, self.fingerprint())


if __name__ == "__main__":
    unittest.main()
