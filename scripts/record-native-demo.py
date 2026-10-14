#!/usr/bin/env python3
"""Record real X11 native input. Use only in a disposable 1440x900 desktop.
Usage: record-native-demo.py INSTALLED_RELEASE_BINARY OUTPUT_DIRECTORY
The fixture and state remain in OUTPUT_DIRECTORY for inspection.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
root = out / 'demo-repository'
store = out / 'demo-state'
if root.exists() or store.exists():
    raise SystemExit('Use a fresh output directory to preserve previous evidence')
root.mkdir()

def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout

def git(*args):
    return run('git', '-C', str(root), *args)

git('init'); git('config', 'user.name', 'Peer demonstration'); git('config', 'user.email', 'demo@example.invalid')
(root / 'a.rs').write_text('fn main() {\n    println!("Hello, reviewer!");\n}\n')
(root / 'readme.md').write_text('# Native review demo\n\nA small read-only review workspace.\n')
(root / 'config.json').write_text('{\n  "theme": "light",\n  "review": false\n}\n')
git('add', '.'); git('commit', '-m', 'Demonstration baseline')
(root / 'a.rs').write_text('fn main() {\n    println!("Hello, native Peer!");\n}\n')
(root / 'readme.md').write_text('# Native review demo\n\nReview Rust, Markdown and JSON with native GPUI.\n')
(root / 'config.json').write_text('{\n  "theme": "dark",\n  "review": true\n}\n')
log = (out / 'window.log').open('w')
process = subprocess.Popen([str(binary)], env={**os.environ, 'PEER_DATA_DIR': str(store)}, stdout=log, stderr=log)
recorder = None
scenes = []
started = 0

def click(x, y):
    run('xdotool', 'mousemove', str(x), str(y)); time.sleep(.3)
    run('xdotool', 'click', '1')

def type_at(x, y, text):
    click(x, y); run('xdotool', 'key', 'ctrl+a')
    run('xdotool', 'type', '--clearmodifiers', '--delay', '45', text)

def scene(label, seconds=4):
    scenes.append({'seconds': round(time.monotonic() - started, 2), 'action': label})
    time.sleep(seconds)

def shot(name):
    run('import', '-window', 'root', str(out / (name + '.png')))

try:
    deadline = time.monotonic() + 30
    while True:
        try:
            window = run('xdotool', 'search', '--onlyvisible', '--pid', str(process.pid)).splitlines()[0]
            break
        except (subprocess.CalledProcessError, IndexError):
            if time.monotonic() > deadline or process.poll() is not None:
                raise RuntimeError('Release native window did not open')
            time.sleep(.2)
    run('xprop', '-id', window, '-f', '_MOTIF_WM_HINTS', '32c', '-set', '_MOTIF_WM_HINTS', '2, 0, 0, 0, 0')
    run('xdotool', 'windowmove', window, '0', '0', 'windowsize', window, '1440', '900')
    run('wmctrl', '-i', '-r', hex(int(window)), '-b', 'add,fullscreen')
    run('wmctrl', '-i', '-a', hex(int(window)))
    time.sleep(2)
    provenance = {'binary': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                  'pid': process.pid, 'proc_exe': os.readlink(f'/proc/{process.pid}/exe'),
                  'proc_cmdline': Path(f'/proc/{process.pid}/cmdline').read_bytes().decode().split('\0'),
                  'build_profile': 'release', 'source_commit': run('git', 'rev-parse', 'HEAD').strip(),
                  'capture': 'ffmpeg x11grab of actual native process; no synthetic frames'}
    (out / 'provenance.json').write_text(json.dumps(provenance, indent=2))
    recorder = subprocess.Popen(['ffmpeg', '-y', '-f', 'x11grab', '-video_size', '1440x900', '-framerate', '20', '-i', os.environ['DISPLAY'], '-c:v', 'libx264', '-preset', 'veryfast', '-crf', '22', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(out / 'peer-release-demo.mp4')], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=(out / 'recording.log').open('w'))
    started = time.monotonic()
    scene('Release native application, empty workspace', 2)
    # Paste the long workspace path; type the actual review text visibly later.
    click(400, 24)
    subprocess.run(['xclip', '-selection', 'clipboard'], input=str(root), text=True, check=True)
    run('xdotool', 'key', 'ctrl+v'); click(1010, 24)
    scene('Open demonstration repository and inspect file tree', 5); shot('01-open')
    click(800, 72); scene('Virtual native Diff showing multiple changed files', 5)
    click(813, 119); scene('Switch to side-by-side differences', 5); shot('02-split')
    click(813, 119); click(800, 72); click(120, 184)
    type_at(370, 118, 'native'); click(620, 118)
    scene('Search original source and highlight match', 5); shot('03-search')
    click(410, 231)
    type_at(1210, 210, 'Please keep this native greeting.')
    click(1200, 310); scene('Save a comment attached to original source line', 5)
    click(1200, 115); scene('Mark file Reviewed', 5); shot('04-review')
    click(1390, 24); scene('Native appearance settings and theme previews', 5); shot('05-appearance')
    click(390, 111); scene('Native font, tab width, whitespace and layout controls', 5); shot('06-viewer')
    click(490, 111); scene('Language server enable and executable controls', 5); shot('07-navigation')
    click(425, 835); scene('Return to saved review workspace', 3)
    run('xdotool', 'key', 'ctrl+q'); process.wait(timeout=15)
    assert process.returncode == 0
    state = json.loads((store / 'state.json').read_text())
    assert any(c['body'] == 'Please keep this native greeting.' for q in state['queues'].values() for c in q['comments'])
    assert any('a.rs' in q['reviewed'] for q in state['queues'].values())
    (out / 'scenes.json').write_text(json.dumps(scenes, indent=2))
finally:
    if recorder is not None:
        recorder.communicate(b'q', timeout=30)
    if process.poll() is None:
        process.terminate(); process.wait(timeout=15)
    log.close()
