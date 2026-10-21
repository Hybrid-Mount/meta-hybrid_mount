#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
import hashlib
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / "module/vfs/src/hybridmount.c").read_text(encoding="utf-8")
name = "hybridmount_bsearch_child"
match = re.search(rf"^static[^;\n]*\b{name}\([^;]*?\)\s*\{{", source, re.MULTILINE)
if match is None:
    raise RuntimeError(f"missing production function: {name}")
end, depth = match.end(), 1
while depth:
    depth += (source[end] == "{") - (source[end] == "}")
    end += 1
production = source[match.start():end]
compiler = shlex.split(os.environ.get("CC", "cc"))
print("# userspace only; synthetic prehashed names; no hash, credential, RCU, SRCU, seqcount, allocator or kernel timing", flush=True)
print("# compiler:", " ".join(compiler), flush=True)
print("# extracted search sha256:", hashlib.sha256(production.encode()).hexdigest(), flush=True)
with tempfile.TemporaryDirectory(prefix="hm-lookup-bench-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text(production, encoding="utf-8")
    executable = directory / ("bench.exe" if os.name == "nt" else "bench")
    subprocess.run(compiler + ["-O2", "-std=gnu11", "-Wall", "-Wextra", "-Werror",
                   "-I", str(directory), str(Path(__file__).with_suffix(".c")),
                   "-o", str(executable)], check=True)
    subprocess.run([str(executable)], check=True)
