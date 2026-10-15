#!/usr/bin/env python3
"""Verify the distributable ZIP, before artifact upload or after download."""
import stat
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as archive:
    names = archive.namelist()
    assert names and all(
        name.startswith(('Peer.app/', '__MACOSX/')) for name in names
    ), 'ZIP must retain Peer.app as its root directory'
    for name in ('Contents/Info.plist', 'Contents/MacOS/peer', 'Contents/_CodeSignature/CodeResources'):
        assert archive.getinfo('Peer.app/' + name).file_size > 0, name
    mode = archive.getinfo('Peer.app/Contents/MacOS/peer').external_attr >> 16
    assert stat.S_ISREG(mode) and mode & 0o111, 'Executable mode was lost'
    assert archive.testzip() is None, 'ZIP CRC validation failed'
print('Verified Peer.app root, executable mode, signature resources and ZIP CRC')
