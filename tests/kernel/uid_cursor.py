#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Compile the production GET_UIDS command against hostile wire cursors."""
import os
from pathlib import Path
import shlex
import subprocess
import tempfile

source = (Path(__file__).resolve().parents[2] / "module/vfs/src/hybridmount.c").read_text()
body = source.split("case HM_CMD_GET_UIDS: {", 1)[1].split("\n        }", 1)[0]
body = body.rsplit("break;", 1)[0]
program = r"""
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef uint32_t u32;
struct hm_uid_array { int count; u32 uids[3]; };
struct hm_payload { u32 arg1, data_size; char buffer[4068]; };
struct hm_uid_array *hybridmount_uids;
#define READ_ONCE(x) (x)
#define rcu_read_lock() ((void)0)
#define rcu_read_unlock() ((void)0)
#define rcu_dereference(x) (x)
static void query(struct hm_payload *payload) {
""" + body + r"""
}
int main(int argc, char **argv) {
    struct hm_uid_array populated = {3, {1, 3, 7}}, empty = {0, {0}};
    u32 cursors[] = {0, 3, 4, 0x80000000U, UINT32_MAX};
    struct hm_payload payload = {0};
    int table = atoi(argv[1]), index = atoi(argv[2]);
    hybridmount_uids = table == 0 ? NULL : table == 1 ? &empty : &populated;
    payload.arg1 = cursors[index];
    query(&payload);
    u32 expected_size = table == 2 && index == 0 ? 12 : 0;
    u32 expected_cursor = expected_size ? 3 : cursors[index];
    if (payload.data_size != expected_size || payload.arg1 != expected_cursor) return 1;
    if (expected_size && memcmp(payload.buffer, populated.uids, 12)) return 2;
    printf("PASS: UID table %d cursor %u\n", table, cursors[index]);
    return 0;
}
"""
with tempfile.TemporaryDirectory(prefix="hm-uids-") as temp:
    root = Path(temp)
    (root / "test.c").write_text(program)
    executable = root / "test.exe"
    compiler = shlex.split(os.environ.get("CC", "cc"))
    subprocess.run(compiler + [
        "-std=gnu11", "-Wno-unused-parameter", str(root / "test.c"),
        "-o", str(executable),
    ], check=True)
    failures = []
    for table in range(3):
        for index in range(5):
            result = subprocess.run([str(executable), str(table), str(index)], timeout=10)
            if result.returncode:
                failures.append((table, index, result.returncode))
    if failures:
        raise SystemExit(f"failed UID queries: {failures}")
