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
    for i in reversed(range(len(words))):
        for count in range(1,len(wanted)+1):
            rows=words[i:i+count]
            if ''.join(re.sub(r'[^a-z0-9]','',r['text'].lower()) for r in rows)==''.join(wanted):
                r=rows[0];click(int(r['left'])+int(r['width'])//2,int(r['top'])+int(r['height'])//2);return
    raise AssertionError('Missing button '+label)
def draft(body): return any(v.get('body')==body for v in state().get('nativeState',{}).values())
try:
    start()
    # Explicit comment start freezes old-side source position independently of navigation.
    click(405,231);button('Line scope')
    type_at(1200,210,'Anchor survives navigation')
    click(405,270);press('ctrl+Return')
    wait('comment-anchor-remains-old-line-2',lambda:any(c['body']=='Anchor survives navigation' and c['anchor'].get('side')=='old' and c['anchor'].get('line')==2 for c in comments()))
    type_at(1200,210,'Preserved new draft')
    button('Edit');type_at(1200,210,'Cancelled edit',True);press('Escape')
    wait('cancel-edit-restores-new-draft',lambda:draft('Preserved new draft'))
    screenshot('01-cancel-restores-draft')
    button('Edit');type_at(1200,210,'Discarded edit',True);button('Discard edit');wait('discard-edit-preserves-new-draft',lambda:draft('Preserved new draft'))
    button('Edit');type_at(1200,210,'Updated saved comment',True);press('ctrl+Return')
    wait('edit-save-updates-existing',lambda:len(comments())==1 and comments()[0]['body']=='Updated saved comment')
    wait('edit-save-restores-new-draft',lambda:draft('Preserved new draft'))
    # Persist full draft scope across single-file switch and restart.
    click(405,231);button('Line scope');click(120,216);time.sleep(.5);click(120,184)
    wait('draft-restored-after-file-switch',lambda:draft('Preserved new draft'))
    quit_app()
    # Recreate the first native release's fingerprint-keyed on-disk draft format.
    legacy=state()
    for k,v in list(legacy.get('nativeState',{}).items()):
        if k.startswith('draft-v2:') and v.get('body')=='Preserved new draft':
            legacy['nativeState'].pop(k);oldkey=k.removeprefix('draft-v2:')+':'+v['fingerprint'];v.pop('fingerprint');legacy['nativeState'][oldkey]=v
    (store/'state.json').write_text(json.dumps(legacy));start(False)
    wait('legacy-draft-survives-restart',lambda:draft('Preserved new draft'))
    # A changed fingerprint must not hide or silently re-anchor the draft.
    (root/'a.rs').write_text('fn main() {\n\tprintln!("changed again");\n}\n')
    button('Refresh');rendered('Draft source changed')
    screenshot('02-stale-draft-visible')
    click(1200,210);press('ctrl+Return');screenshot('stale-blocked');run('convert',str(output/'stale-blocked.png'),'-crop','1440x45+0+90','-negate',str(output/'error-crop.png'));assert 'explicitly choose' in run('tesseract',str(output/'error-crop.png'),'stdout').lower()
    assert len(comments())==1;checks.append('stale-draft-submit-blocked')
    # File scope is explicit confirmation and saves body after refresh.
    button('File scope');press('ctrl+Return')
    wait('explicit-reattach-saves-preserved-body',lambda:any(c['body']=='Preserved new draft' and not c['anchor'].get('line') for c in comments()))
    # All-files switching restores body, anchor, editing identity and the suspended new draft.
    type_at(390,120,'println',True);time.sleep(.5);press('Return');button('Line scope')
    type_at(1200,210,'Multi-file anchored draft');button('All files');rendered('files loaded')
    click(120,216);time.sleep(.4);click(120,184)
    wait('multi-file-draft-restored',lambda:any(v.get('body')=='Multi-file anchored draft' and v.get('line')==2 for v in state().get('nativeState',{}).values()))
    button('Edit');type_at(1200,210,'Suspended saved edit',True)
    click(120,216);time.sleep(.4);click(120,184)
    wait('multi-file-edit-identity-restored',lambda:any(v.get('body')=='Suspended saved edit' and v.get('editing') and v.get('pending_new',{}).get('body')=='Multi-file anchored draft' for v in state().get('nativeState',{}).values()))
    click(1200,210);press('Escape');wait('multi-file-cancel-restores-new-draft',lambda:draft('Multi-file anchored draft'))
    screenshot('03-multi-file-draft');button('Discard draft')
    type_at(1200,210,'Save then immediate switch');press('ctrl+Return');click(120,216);time.sleep(.3);click(120,184)
    wait('rapid-save-switch-does-not-duplicate',lambda:sum(c['body']=='Save then immediate switch' for c in comments())==1)
    wait('submitted-draft-not-resurrected',lambda:not draft('Save then immediate switch'))
    # Commit history closes both by toggle and Escape.
    button('Commit history');rendered('Initial');button('Commit history')
    screenshot('03-history-closed');assert 'Initial' not in run('tesseract',str(output/'03-history-closed.png'),'stdout');checks.append('history-button-closes')
    button('Commit history');press('Escape');screenshot('04-history-escape');assert 'Initial' not in run('tesseract',str(output/'04-history-escape.png'),'stdout');checks.append('history-escape-closes')
    # Search changes recompute and clearing removes persisted visual hits.
    type_at(400,120,'main',True);time.sleep(.8);press('shift+Return');rendered('Match 2/2');checks.append('reverse-find-starts-at-last-match')
    type_at(400,120,'no-such-symbol',True);rendered('No matches')
    type_at(400,120,'',True);screenshot('05-find-cleared');checks.append('find-change-and-clear-live')
    quit_app()
    (output/'result.json').write_text(json.dumps({'passed':checks},indent=2)+'\n');print(json.dumps({'passed':len(checks)}))
except Exception:
    screenshot("failure");(output/"failure-state.json").write_text(json.dumps(state(),indent=2));raise
finally:
    if process is not None and process.poll() is None: process.terminate();process.wait(timeout=15)
    log.close()
