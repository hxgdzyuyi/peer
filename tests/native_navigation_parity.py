#!/usr/bin/env python3
"""Actual X11 window smoke. Run under a 1440x900 Xvfb or a disposable desktop.
Requires Git, xdotool and ImageMagick import. Does not substitute service calls for UI clicks.
Never run on a desktop being used by another person (this sends mouse/keyboard input).
"""
import json
import hashlib
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/peer').resolve()
output = Path(sys.argv[2] if len(sys.argv) > 2 else 'test-results/native-smoke').resolve()
output.mkdir(parents=True, exist_ok=True)
(output / 'result.json').unlink(missing_ok=True)
work = tempfile.TemporaryDirectory(prefix='peer-native-smoke-')
root = Path(work.name) / 'repository'
store = Path(work.name) / 'store'
root.mkdir()

def run(*args):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True).stdout

def git(*args):
    return run('git', '-C', str(root), *args)

git('init'); git('config', 'user.name', 'Native smoke'); git('config', 'user.email', 'native@example.invalid')
(root / 'a.rs').write_text('fn main() {\n\tprintln!("old");\n}\n')
(root / 'b.txt').write_text('before\n')
git('add', '.'); git('commit', '-m', 'Initial'); git('branch', 'baseline')
(root / 'a.rs').write_text('fn main() {\n\tprintln!("new");\n}\n')
git('add', 'a.rs')
(root / 'b.txt').write_text('after\n')
checks = []
log = (output / 'window.log').open('w')
process = None

def state():
    try:
        return json.loads((store / 'state.json').read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return {}

def wait(label, predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            checks.append(label)
            time.sleep(.35)  # Store commit precedes async UI refresh.
            return
        if process is not None and process.poll() is not None:
            raise AssertionError(f'Native process exited during {label}')
        time.sleep(.1)
    raise AssertionError(f'Timed out: {label}')

def start(with_path=True):
    global process
    process = subprocess.Popen([str(binary)] + ([str(root)] if with_path else []), env={**os.environ, 'PEER_DATA_DIR': str(store)}, stdout=log, stderr=log)
    def find_window():
        try:
            return run('xdotool', 'search', '--onlyvisible', '--pid', str(process.pid)).splitlines()
        except subprocess.CalledProcessError:
            return []
    wait('native-window-created', find_window)
    (output/'process.json').write_text(json.dumps({'executable':os.readlink(f'/proc/{process.pid}/exe'),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'display':os.environ.get('DISPLAY'),'method':'actual native process launched by the X11 interaction harness'},indent=2)+'\n')
    window = find_window()[0]
    run('xprop', '-id', window, '-f', '_MOTIF_WM_HINTS', '32c', '-set', '_MOTIF_WM_HINTS', '2, 0, 0, 0, 0')
    run('xdotool', 'windowmove', window, '0', '0', 'windowsize', window, '1440', '900', 'windowfocus', '--sync', window)
    run('wmctrl', '-i', '-r', hex(int(window)), '-b', 'add,fullscreen')
    run('wmctrl', '-i', '-a', hex(int(window)))
    wait('repository-opened', lambda: state().get('preferences', {}).get('lastRepository') == str(root))
    time.sleep(.8)

def click(x, y):
    run('xdotool', 'mousemove', str(x), str(y), 'click', '1')

def type_at(x, y, text, clear=False):
    click(x, y)
    if clear: run('xdotool', 'key', 'ctrl+a')
    # Separate type from key: xdotool's type consumes its remaining arguments.
    run('xdotool', 'type', '--clearmodifiers', '--delay', '18', text)

def screenshot(name):
    run('xdotool', 'mousemove', '1435', '890')
    time.sleep(1)
    run('import', '-window', 'root', str(output / f'{name}.png'))

def rendered(phrase):
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        screenshot('ready-controls')
        if phrase.lower() in run('tesseract',str(output/'ready-controls.png'),'stdout').lower(): return
    raise AssertionError('Rendered control did not appear: '+phrase)

def comments():
    return [c for q in state().get('queues', {}).values() for c in q['comments']]

def comparison():
    return state().get('preferences', {}).get('sessions', {}).get(str(root), {}).get('comparison', {})

def quit_app():
    run('xdotool', 'key', 'ctrl+q')
    process.wait(timeout=15)
    assert process.returncode == 0


def press(*keys): run('xdotool','key',*keys);time.sleep(.25)
def button(label):
    import csv,io
    screenshot('controls')
    words=[r for r in csv.DictReader(io.StringIO(run('tesseract',str(output/'controls.png'),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE) if r.get('text','').strip()]
    wanted=[re.sub(r'[^a-z0-9]','',w.lower()) for w in label.split()]
    for i in reversed(range(len(words)-len(wanted)+1)):
        rows=words[i:i+len(wanted)]
        if [re.sub(r'[^a-z0-9]','',r['text'].lower()) for r in rows]==wanted:
            r=rows[0];click(int(r['left'])+int(r['width'])//2,int(r['top'])+int(r['height'])//2);return
    raise AssertionError('Missing button '+label)
def draft(body): return any(v.get('body')==body for v in state().get('nativeState',{}).values())

(root/'old.py').write_text('def target():\n    return 17\n\nvalue = target()\n')
(root/'pyproject.toml').write_text('[project]\nname="parity"\nversion="0.0.0"\n')
git('add','old.py','pyproject.toml');git('commit','-m','Navigation baseline')
git('mv','old.py','new.py');(root/'new.py').write_text('# shifted version\ndef target():\n    return 17\n\nvalue = target()\n')
(root/'large.py').write_text('# padding\n'*22000+'def deep_target():\n    return 99\n')
(root/'caller.py').write_text('from large import deep_target\nvalue = deep_target()\n')
def selected(path):return state().get('preferences',{}).get('sessions',{}).get(str(root),{}).get('selectedPath')==path
def choose(path):
    press('ctrl+p');run('xdotool','type','--clearmodifiers',path);press('Return');wait('selected-'+path,lambda:selected(path));time.sleep(.5)
def text():
    screenshot('latest');return run('tesseract',str(output/'latest.png'),'stdout')
def search(query,reverse=False):
    type_at(390,120,query,True);time.sleep(.6);press('shift+Return' if reverse else 'Return');time.sleep(.5)
def column(value):
    import csv,io
    screenshot('controls')
    rows=csv.DictReader(io.StringIO(run('tesseract',str(output/'controls.png'),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE)
    row=next(r for r in rows if r.get('text')=='Definition')
    type_at(350,int(row['top'])+int(row['height'])//2,str(value),True)
def clipboard():return run('xclip','-selection','clipboard','-o')
try:
    start();choose('new.py')
    search('value = target()');rendered('old line 4');column(10);press('F12')
    rendered('old.py');screenshot('01-renamed-old-definition')
    press('ctrl+c')
    wait('definition-range-selected',lambda:clipboard()=='target')
    checks.append('renamed-original-path-resolves')
    button('Back');rendered('new.py');button('Modified B');column(10);press('F12');rendered('new.py')
    press('ctrl+c');wait('shared-line-new-side-mapped',lambda:clipboard()=='target');button('Back')
    # Search and explicit line jumps each have their own Back history entry.
    choose('caller.py');search('deep_target()');button('Line scope');type_at(1200,210,'Draft before definition')
    column(12);press('F12');rendered('Load large navigation source');screenshot('02-deferred-target')
    button('Load large navigation source');rendered('22001');screenshot('03-loaded-target-range')
    press('ctrl+c');wait('deferred-target-range-selected',lambda:clipboard()=='deep_target')
    search('return 99');rendered('line 22002');button('Back');press('ctrl+c');wait('search-back-restores-target-selection',lambda:clipboard()=='deep_target')
    button('Back');rendered('Comment on new line 2');click(1200,210);press('ctrl+Return')
    wait('navigation-keeps-comment-location',lambda:any(c['body']=='Draft before definition' and c['path']=='caller.py' and c['anchor'].get('line')==2 for c in comments()))
    # Comparison controls and restart semantics.
    type_at(390,72,'baseline',True);button('Compare branch');wait('branch-mode',lambda:comparison().get('mode')=='branch')
    button('Code');wait('code-mode',lambda:comparison().get('view')=='code');button('Changes');wait('resume-branch-comparison',lambda:comparison().get('mode')=='branch' and comparison().get('baseRef')=='baseline')
    button('Untracked');wait('untracked-disabled',lambda:comparison().get('includeUntracked')==False)
    quit_app();start(False);wait('comparison-restart-preserves-untracked',lambda:comparison().get('includeUntracked')==False and comparison().get('mode')=='branch')
    button('Branches');rendered('baseline');screenshot('04-branch-picker')
    quit_app();(output/'result.json').write_text(json.dumps({'passed':checks},indent=2)+'\n');print(json.dumps({'passed':len(checks)}))
except Exception:
    screenshot('failure');(output/'failure-state.json').write_text(json.dumps(state(),indent=2));raise
finally:
    if process is not None and process.poll() is None: process.terminate();process.wait(timeout=15)
    log.close()
