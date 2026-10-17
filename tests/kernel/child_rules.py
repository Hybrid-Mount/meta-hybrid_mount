#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Exercise production child publication, UID selection and cursor boundaries."""
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / "module/vfs/src/hybridmount.c").read_text(encoding="utf-8")


def extract(name):
    match = re.search(rf"^static[^;\n]*\b{re.escape(name)}\([^;]*?\)\s*\{{", source, re.MULTILINE)
    end, depth = match.end(), 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[match.start():end]


names = ["hybridmount_bsearch_child"]
if "hybridmount_child_for_uid(" in source:
    names.append("hybridmount_child_for_uid")
names += ["__hybridmount_get_rule_info", "__hybridmount_inject_child_locked",
          "__hybridmount_delete_child_locked", "hybridmount_emit_virtual_children"]
with tempfile.TemporaryDirectory(prefix="hm-children-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text("\n".join(map(extract, names)), encoding="utf-8")
    executable = directory / "children.exe"
    subprocess.run(shlex.split(os.environ.get("CC", "cc")) + [
        "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter",
        "-Wno-unused-function", "-I", str(directory),
        str(Path(__file__).with_suffix(".c")), "-o", str(executable)], check=True)
    failed = []
    for mode in ["uids", "whiteout", "preemption"]:
        if subprocess.run([str(executable), mode], timeout=30).returncode:
            failed.append(mode)
    if failed:
        raise SystemExit("failed child cases: " + ", ".join(failed))
