#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Exercise production VFS rules, directory entries, teardown and xattr dispatch.

Fixtures replace kernel primitives; this does not validate the Linux ABI.
"""
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / "module/vfs/src/hybridmount.c").read_text(encoding="utf-8")
header = (ROOT / "module/vfs/src/hybridmount.h").read_text(encoding="utf-8")


def extract(name):
    match = re.search(rf"^static[^;\n]*\b{re.escape(name)}\([^;]*?\)\s*\{{",
                      source, re.MULTILINE)
    if match is None:
        raise RuntimeError(f"missing production function: {name}")
    end, depth = match.end(), 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[match.start():end]


functions = ["hm_alloc_rule", "hybridmount_emit_virtual_children",
             "hm_destroy_hijacked_inode", "hm_xattr_get"]
production = "\n\n".join(extract(name) for name in functions)
source, saved_source = header, source
production = "#define HM_SIG_16 0x686DULL\n" + "\n".join(
    extract(name) for name in ["hm_is_virtual_pos", "hm_pack_pos", "hm_unpack_pos"]
) + "\n" + production
source = saved_source
if "static int hm_xattr__get(" in source:
    production += "\n#if LINUX_VERSION_CODE >= KERNEL_VERSION(4, 14, 0) && LINUX_VERSION_CODE < KERNEL_VERSION(5, 4, 0)\n"
    production += extract("hm_xattr__get") + "\n#endif\n"
production += "\n" + extract("hybridmount_hijack_superblock")
if "static int hm_preserve_virtual_parent(" in source:
    production += "\n" + extract("hm_preserve_virtual_parent")
production += "\n" + extract("__hybridmount_del_rule")

failures = []
with tempfile.TemporaryDirectory(prefix="hm-semantics-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text(production, encoding="utf-8")
    for version in ["4, 14, 0", "4, 19, 0", "5, 4, 0", "5, 10, 0"]:
        label = version.replace(", ", "-")
        executable = directory / (f"semantics-{label}.exe" if os.name == "nt" and "zig" not in os.environ.get("CC", "").lower() else f"semantics-{label}")
        output = directory / f"semantics-{label}.obj" if os.name == "nt" and "zig" not in os.environ.get("CC", "").lower() else executable
        subprocess.run(shlex.split(os.environ.get("CC", "cc")) + [
            "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter", "-Wno-unused-function",
            "-DCONFIG_COMPAT",
            f"-DLINUX_VERSION_CODE=KERNEL_VERSION({version})", "-I", str(directory),
            str(ROOT / "tests/kernel/vfs_semantics.c"),
            *(["-c"] if os.name == "nt" and "zig" not in os.environ.get("CC", "").lower() else []), "-o", str(output)], check=True)
        if os.name == "nt" and "zig" not in os.environ.get("CC", "").lower():
            subprocess.run([os.environ.get("HM_TEST_LINK", "link"), "/nologo",
                            f"/out:{executable}", str(output), "libcmt.lib", "oldnames.lib"], check=True)
        for mode in ["hook-alloc-sop", "hook-alloc-xattr", "hook-pin", "source-flags", "dirent", "cursor-negative", "cursor-max", "cursor-compat-max", "dirent-compat", "parent", "parent-opaque",
                     "parent-empty", "parent-allocation", "parent-repeat",
                     "teardown", "xattr", "xattr-native", "xattr-virtual"]:
            result = subprocess.run([str(executable), mode], timeout=30)
            if result.returncode:
                failures.append(f"{version}: {mode}")
if failures:
    raise SystemExit("failed scenarios: " + ", ".join(failures))
