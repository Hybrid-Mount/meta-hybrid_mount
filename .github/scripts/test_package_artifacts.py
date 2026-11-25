# SPDX-License-Identifier: GPL-3.0-only
import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("package_artifacts", Path(__file__).with_name("package_artifacts.py"))
packages = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packages)


class PackageArtifactsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def package(self, flavor, archs=None, version="6.2.6", code="602006999"):
        suffix = "_Wear" if flavor == "wear" else ""
        path = self.root / f"Hybrid-Mount{suffix}-{version}-2000.zip"
        archs = archs or (["arm64", "armv7"] if flavor == "wear" else ["arm64", "armv7", "x86_64", "riscv64"])
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("module.prop", f"id=hybrid_mount\nversion={version}\nversionCode={code}\npackageFlavor={flavor}\nupdateJson=https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/{'update-wear.json' if flavor == 'wear' else 'update.json'}\n")
            for arch in archs:
                archive.writestr(f"binaries/hybrid-mount-{arch}", b"binary")
        return path

    def test_pair_preserves_names_and_digests(self):
        self.package("normal")
        self.package("wear")
        result = packages.inspect_packages(self.root)
        self.assertEqual([p["flavor"] for p in result], ["normal", "wear"])
        self.assertTrue(all(len(p["sha256"]) == 64 for p in result))

    def test_missing_extra_wrong_arch_and_mismatched_version_are_rejected(self):
        self.package("normal")
        with self.assertRaises(ValueError):
            packages.inspect_packages(self.root)
        wear = self.package("wear", ["arm64", "armv7", "x86_64"])
        with self.assertRaises(ValueError):
            packages.inspect_packages(self.root)
        wear.unlink()
        wear = self.package("wear", version="6.2.7")
        with self.assertRaises(ValueError):
            packages.inspect_packages(self.root)
        wear.unlink()
        self.package("wear")
        (self.root / "stale.zip").write_bytes(b"stale")
        with self.assertRaises(ValueError):
            packages.inspect_packages(self.root)

    def test_crossed_update_channel_is_rejected(self):
        self.package("normal")
        path = self.package("wear")
        with zipfile.ZipFile(path) as archive:
            contents = {n: archive.read(n) for n in archive.namelist()}
        contents["module.prop"] = contents["module.prop"].replace(b"update-wear.json", b"update.json")
        with zipfile.ZipFile(path, "w") as archive:
            for name, content in contents.items():
                archive.writestr(name, content)
        with self.assertRaises(ValueError):
            packages.inspect_packages(self.root)

    def test_catalog_star_pattern_selects_only_normal(self):
        # Pinned upstream matcher supports '*' and '?' only, no character classes.
        import re
        manifest = Path(__file__).resolve().parents[2] / "module.yaml"
        pattern = next(line.split(":", 1)[1].strip() for line in manifest.read_text(encoding="utf-8").splitlines() if "assetPattern:" in line)
        regex = "^" + "".join(".*" if c == "*" else "." if c == "?" else re.escape(c) for c in pattern) + "$"
        normal = self.package("normal")
        wear = self.package("wear")
        self.assertIsNotNone(re.fullmatch(regex, normal.name))
        self.assertIsNone(re.fullmatch(regex, wear.name))

    def test_prechecks_preserve_previous_release_selection(self):
        import re
        workflow = Path(__file__).resolve().parents[1] / "workflows/release.yml"
        patterns = [line.split(":", 1)[1].strip().strip('"') for line in workflow.read_text(encoding="utf-8").splitlines() if "asset-pattern:" in line]
        self.assertEqual(len(patterns), 2)
        for pattern in patterns:
            regex = "^" + "".join(".*" if c == "*" else "." if c == "?" else re.escape(c) for c in pattern) + "$"
            self.assertIsNotNone(re.fullmatch(regex, "hybrid_mount-602006999-6.2.6.zip"))
            self.assertIsNotNone(re.fullmatch(regex, "Hybrid-Mount-6.2.6-2000.zip"))
            self.assertIsNone(re.fullmatch(regex, "Hybrid-Mount_Wear-6.2.6-2000.zip"))


if __name__ == "__main__":
    unittest.main()
