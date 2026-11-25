# SPDX-License-Identifier: GPL-3.0-only
"""Validate the complete normal/Wear artifact pair before upload or notification."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import zipfile


def inspect_packages(directory):
    paths = sorted(Path(directory).glob("*.zip"))
    if len(paths) != 2:
        raise ValueError(f"expected exactly normal and Wear ZIPs, found {len(paths)}")
    result = {}
    for path in paths:
        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            if len(set(names)) != len(names):
                raise ValueError(f"duplicate ZIP entries: {path.name}")
            prop = dict(line.split("=", 1) for line in archive.read("module.prop").decode().splitlines() if "=" in line)
            flavor = prop.get("packageFlavor")
            if flavor not in ("normal", "wear") or flavor in result:
                raise ValueError(f"missing, duplicate or unknown flavor: {path.name}")
            suffix = "_Wear" if flavor == "wear" else ""
            version = prop["version"]
            if prop.get("id") != "hybrid_mount" or not re.fullmatch(rf"Hybrid-Mount{suffix}-{re.escape(version)}-[0-9]+\.zip", path.name):
                raise ValueError(f"package identity mismatch: {path.name}")
            update = "update-wear.json" if flavor == "wear" else "update.json"
            if prop.get("updateJson") != f"https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/{update}":
                raise ValueError(f"wrong update channel: {path.name}")
            archs = {"arm64", "armv7"} if flavor == "wear" else {"arm64", "armv7", "x86_64", "riscv64"}
            binaries = {n for n in names if n.startswith("binaries/") and not n.endswith("/")}
            if binaries != {f"binaries/hybrid-mount-{arch}" for arch in archs}:
                raise ValueError(f"incorrect userspace architectures: {path.name}")
            if any(archive.getinfo(n).file_size == 0 for n in binaries):
                raise ValueError(f"empty binary: {path.name}")
            result[flavor] = dict(flavor=flavor, path=str(path.resolve()), name=path.name,
                                  version=version, version_code=prop["versionCode"],
                                  sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    normal, wear = result["normal"], result["wear"]
    if (normal["version"], normal["version_code"]) != (wear["version"], wear["version_code"]):
        raise ValueError("normal and Wear versions differ")
    if normal["name"].removeprefix("Hybrid-Mount-") != wear["name"].removeprefix("Hybrid-Mount_Wear-"):
        raise ValueError("normal and Wear source revisions differ")
    return [normal, wear]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    packages = inspect_packages(args.directory)
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as output:
            for package in packages:
                for key in ("path", "name", "sha256"):
                    output.write(f"{package['flavor']}_{key}={package[key]}\n")
    payload = json.dumps(packages, indent=2) + "\n"
    print(payload, end="")


if __name__ == "__main__":
    main()
