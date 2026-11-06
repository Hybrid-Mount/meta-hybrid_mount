#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Opt-in probes for a disposable kernel; print observations, never global-clear."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import shutil
from contextlib import nullcontext

if os.environ.get('HM_DISPOSABLE_TEST') != '1':
    raise SystemExit('Set HM_DISPOSABLE_TEST=1 only in a disposable kernel test environment')
if os.geteuid() != 0:
    raise SystemExit('root required')
binary = os.environ.get('HM_BINARY', 'hybrid-mount')


def cli(*args):
    return subprocess.check_output([binary, 'vfs', *map(str, args)], text=True).strip()


if cli('version') != 'hm1':
    raise SystemExit('SKIP: exact Hybrid Mount hm1 provider required')
owned = []
uid_owned = False
uid = int(os.environ.get('HM_TEST_UID', '61987'))
if uid <= 0:
    raise SystemExit('HM_TEST_UID must be a non-root UID')
directory = tempfile.mkdtemp(prefix='hm-device-', dir=os.environ.get('HM_TEST_ROOT', '/data/local/tmp'))


def add(target, source=None, opaque=False):
    # Track even if read-back verification fails after mutation.
    owned.append(target)
    if opaque:
        cli('rule', 'add', '--opaque', target)
    else:
        cli('rule', 'add', target, source)


def observe(label, target, fd):
    result = {'case': label, 'old_fd_inode': os.fstat(fd).st_ino,
              'old_fd_content': os.pread(fd, 64, 0).decode()}
    try:
        result.update(new_inode=target.stat().st_ino, new_content=target.read_text())
    except OSError as error:
        result['new_errno'] = error.errno
    print(json.dumps(result), flush=True)


try:
    with nullcontext(directory):
        root = Path(directory)
        root.chmod(0o755)
        if sys.argv[1] == 'source':
            for case in ['rename', 'recreate', 'atomic-replace', 'symlink']:
                source, target = root / (case + '-source'), root / (case + '-target')
                source.write_text('old')
                target.write_text('stock')
                supplied = source
                if case == 'symlink':
                    supplied = root / 'source-link'
                    supplied.symlink_to(source)
                add(target, supplied)
                fd = os.open(target, os.O_RDONLY)
                try:
                    if case == 'rename':
                        source.rename(root / 'renamed')
                    elif case == 'recreate':
                        source.unlink()
                        source.write_text('new')
                    elif case == 'atomic-replace':
                        replacement = root / 'replacement'
                        replacement.write_text('new')
                        replacement.replace(source)
                    else:
                        replacement = root / 'referent'
                        replacement.write_text('new')
                        supplied.unlink()
                        supplied.symlink_to(replacement)
                    observe(case, target, fd)
                finally:
                    os.close(fd)
                    cli('rule', 'del', target)
                    owned.remove(target)
        elif sys.argv[1] == 'uid':
            # JSON shape is deliberately inspected rather than assuming an empty table.
            existing = cli('uid', 'list', '--json')
            if str(uid) in existing:
                raise SystemExit('test UID already isolated; choose another HM_TEST_UID')
            native, nested, synthetic = root / 'native', root / 'native/child', root / 'synthetic'
            nested.mkdir(parents=True)
            (nested / 'stock').write_text('native')
            synthetic.mkdir()
            synthetic.rmdir()
            add(native, opaque=True)
            add(nested, opaque=True)
            # A pure synthetic opaque directory has no native counterpart.
            add(synthetic, opaque=True)
            handles = [(p.name, os.open(p, os.O_RDONLY | os.O_DIRECTORY)) for p in [native, nested, synthetic]]
            uid_owned = True
            cli('uid', 'add', uid)
            pid = os.fork()
            if pid == 0:
                try:
                    os.setgroups([])
                    os.setgid(uid)
                    os.setuid(uid)
                    for name, fd in handles:
                        row = {'directory': name}
                        try:
                            row['entries'] = os.listdir(fd)
                            child = os.open('stock', os.O_RDONLY, dir_fd=fd)
                            try:
                                row['openat'] = os.read(child, 64).decode()
                            finally:
                                os.close(child)
                        except OSError as error:
                            row['errno'] = error.errno
                        print(json.dumps(row), flush=True)
                finally:
                    os._exit(0)
            _, status = os.waitpid(pid, 0)
            for _, fd in handles:
                os.close(fd)
            if status:
                raise RuntimeError(f'UID probe child failed: {status}')
        else:
            raise SystemExit('expected source or uid')
        # Remove rules before TemporaryDirectory removes their native paths.
        for target in reversed(owned[:]):
            cli('rule', 'del', target)
            owned.remove(target)
finally:
    if uid_owned:
        cli('uid', 'del', uid)
    for target in reversed(owned):
        cli('rule', 'del', target)
    shutil.rmtree(directory)
