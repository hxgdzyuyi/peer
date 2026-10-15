#!/usr/bin/env python3
"""Audit the extracted app's actual load commands; run on the macOS builder.

System frameworks are supplied by macOS (not bundled). This records their load
commands, but does not claim to prove every dynamically looked-up API available
on an older OS. That requires a real macOS 14.5 run.
"""
import hashlib
import json
from pathlib import Path
import plistlib
import re
import subprocess
import sys


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def version(value):
    return tuple(int(x) for x in value.split('.')) + (0,) * (3 - len(value.split('.')))


def minimums(load_commands):
    values = []
    for block in re.split(r'Load command \d+\n', load_commands)[1:]:
        if re.search(r'cmd LC_BUILD_VERSION\b', block):
            assert re.search(r'platform (?:1|MACOS)\b', block), block
            values.append(re.search(r'\bminos ([\d.]+)', block).group(1))
        elif re.search(r'cmd LC_VERSION_MIN_MACOSX\b', block):
            values.append(re.search(r'\bversion ([\d.]+)', block).group(1))
    assert values, 'No macOS minimum version load command'
    return values


def audit(bundle, target):
    with (bundle / 'Contents/Info.plist').open('rb') as f:
        manifest = plistlib.load(f)
    assert manifest['LSMinimumSystemVersion'] == target
    images = []
    for path in sorted(bundle.rglob('*')):
        if not path.is_file() or 'Mach-O' not in command('file', '-b', str(path)):
            continue
        loads = command('otool', '-l', str(path))
        mins = minimums(loads)
        assert all(version(v) <= version(target) for v in mins), (path, mins, target)
        archs = command('lipo', '-archs', str(path)).split()
        assert 'arm64' in archs, (path, archs)
        # This bundle currently has no embedded dylibs. Fail closed if a future
        # dependency needs an rpath/loader resolution audit instead of system libs.
        deps = re.findall(r'^\s+(.+?) \(compatibility version', command('otool', '-L', str(path)), re.M)
        assert all(d.startswith(('/System/Library/', '/usr/lib/')) for d in deps), deps
        if path.name == 'peer':
            assert all(version(v) == version(target) for v in mins), mins
        images.append(dict(path=str(path.relative_to(bundle)), architectures=archs,
                           minimum_versions=mins, dependencies=deps,
                           sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                           load_commands=loads))
    assert any(i['path'] == 'Contents/MacOS/peer' for i in images)
    command('codesign', '--verify', '--deep', '--strict', str(bundle))
    return dict(deployment_target=target, plist_minimum=manifest['LSMinimumSystemVersion'],
                images=images, signature='strict verification passed',
                real_macos_14_5_runtime='not tested by this build audit')


if __name__ == '__main__':
    print(json.dumps(audit(Path(sys.argv[1]), sys.argv[2]), indent=2))
