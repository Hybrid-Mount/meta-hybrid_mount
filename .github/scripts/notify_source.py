# SPDX-License-Identifier: GPL-3.0-only

import hashlib
import json
from pathlib import Path
import tomllib


def fingerprint(root, packages):
    by_name = {}
    for package in packages:
        by_name.setdefault(package["name"], []).append(package)
    pending = ["notify"]
    selected = {}
    while pending:
        dependency = pending.pop()
        name, *qualifiers = dependency.split(" ", 2)
        matches = by_name.get(name, [])
        if qualifiers:
            matches = [package for package in matches if package["version"] == qualifiers[0]]
        if len(matches) != 1:
            raise ValueError(f"Unresolved notifier dependency: {dependency}")
        package = matches[0]
        identity = (name, package["version"], package.get("source", ""))
        if identity not in selected:
            selected[identity] = package
            pending.extend(package.get("dependencies", []))

    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    digest = hashlib.sha256()
    digest.update(json.dumps({
        "packages": [selected[key] for key in sorted(selected)],
        "profile": manifest.get("profile", {}).get("release", {}),
    }, sort_keys=True).encode())
    files = [root / "tools/notify/Cargo.toml", root / ".github/workflows/notify-tool.yml"]
    files.extend((root / "tools/notify").rglob("*.rs"))
    for name in [".github/scripts/notify_source.py", ".cargo/config.toml", "rust-toolchain.toml"]:
        path = root / name
        if path.is_file():
            files.append(path)
    for path in sorted(files):
        contents = path.read_bytes().replace(b"\r\n", b"\n")
        digest.update(path.relative_to(root).as_posix().encode() + b"\0")
        digest.update(str(len(contents)).encode() + b"\0" + contents)
    return digest.hexdigest()


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[2]
    packages = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))["package"]
    print(fingerprint(root, packages))
