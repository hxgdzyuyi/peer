#!/usr/bin/env python3
"""Actual X11 window smoke. Run under a 1440x900 Xvfb or a disposable desktop.
Requires Git, xdotool and ImageMagick import. Does not substitute service calls for UI clicks.
Never run on a desktop being used by another person (this sends mouse/keyboard input).
"""
import json
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

def comments():
    return [c for q in state().get('queues', {}).values() for c in q['comments']]

def comparison():
    return state().get('preferences', {}).get('sessions', {}).get(str(root), {}).get('comparison', {})

def quit_app():
    run('xdotool', 'key', 'ctrl+q')
    process.wait(timeout=15)
    assert process.returncode == 0

try:
    start()
    screenshot('01-source')
    click(405, 231)  # Original source line 2; not a row index passed to the service.
    type_at(1200, 210, 'Native smoke old-side comment')
    click(1200, 310)
    wait('old-line-comment-saved', lambda: any(c['body'] == 'Native smoke old-side comment' and c['anchor'].get('side') == 'old' and c['anchor'].get('line') == 2 for c in comments()))
    click(1170, 416)
    type_at(1200, 210, 'Edited native comment', clear=True); click(1200, 310)
    wait('comment-edit-saved', lambda: any(c['body'] == 'Edited native comment' for c in comments()))
    click(1225, 416)
    wait('comment-deleted', lambda: not comments())
    click(405, 231)
    type_at(1200, 210, 'Native smoke old-side comment'); click(1200, 310)
    wait('old-comment-recreated', lambda: any(c['body'] == 'Native smoke old-side comment' and c['anchor'].get('side') == 'old' for c in comments()))
    click(410, 313)
    type_at(1200, 210, 'Native smoke new-side comment'); click(1200, 310)
    wait('new-line-comment-saved', lambda: any(c['body'] == 'Native smoke new-side comment' and c['anchor'].get('side') == 'new' and c['anchor'].get('line') == 2 for c in comments()))
    click(1300, 310)
    type_at(1200, 210, 'Native smoke file comment'); click(1200, 310)
    wait('file-comment-saved', lambda: any(c['body'] == 'Native smoke file comment' and not c['anchor'].get('line') for c in comments()))
    click(1200, 116)
    wait('reviewed-saved', lambda: any('a.rs' in q['reviewed'] for q in state().get('queues', {}).values()))
    screenshot('02-comment-reviewed')
    type_at(1200, 210, 'Unsubmitted native draft')
    click(120, 192); time.sleep(.4); click(120, 160)
    screenshot('03-draft-restored')
    wait('draft-persisted', lambda: any(v.get('body') == 'Unsubmitted native draft' for v in state().get('nativeState', {}).values()))
    for x, mode in [(190, 'staged'), (270, 'unstaged'), (110, 'all')]:
        click(x, 72)
        wait('comparison-' + mode, lambda mode=mode: comparison().get('mode') == mode)
    type_at(400, 72, 'baseline', clear=True); click(560, 72)
    wait('comparison-branch', lambda: comparison().get('mode') == 'branch')
    type_at(400, 72, git('rev-parse', 'HEAD').strip(), clear=True); click(690, 72)
    wait('comparison-commit', lambda: comparison().get('mode') == 'commit')
    click(110, 72); wait('return-to-working-tree', lambda: comparison().get('mode') == 'all')
    for i in range(12): click(190 if i % 2 else 270, 72)
    click(110, 72); wait('rapid-switch-final-state', lambda: comparison().get('mode') == 'all')
    type_at(380, 118, 'new', clear=True); run('xdotool', 'key', 'Return'); screenshot('04-search')
    quit_app(); checks.append('clean-shutdown')
    start(with_path=False)
    assert any(c['body'] == 'Native smoke old-side comment' for c in comments())
    assert any('a.rs' in q['reviewed'] for q in state()['queues'].values())
    screenshot('05-restarted'); checks.append('restart-review-restored')
    (root / 'a.rs').write_text('fn main() {\n\tprintln!("changed again");\n}\n')
    click(1248, 24)
    wait('outdated-comment-retained', lambda: any(c['outdated'] for c in comments()))
    wait('reviewed-invalidated', lambda: all('a.rs' not in q['reviewed'] for q in state()['queues'].values()))
    screenshot('06-outdated')
    type_at(450, 24, str(root / 'missing'), clear=True); run('xdotool', 'key', 'Return'); screenshot('07-invalid-root')
    type_at(450, 24, str(root), clear=True); run('xdotool', 'key', 'Return'); screenshot('08-recovered')
    click(1395, 24); time.sleep(.4)
    screenshot('09-settings-appearance')
    click(390, 111); time.sleep(.2)
    click(454, 164); click(454, 208); click(680, 251)
    screenshot('10-settings-viewer')
    click(490, 111); time.sleep(.2)
    click(340, 199)
    type_at(700, 365, '/usr/bin/python3', clear=True)
    screenshot('11-settings-navigation')
    click(589, 111); time.sleep(.2)
    type_at(550, 195, '/usr/bin/git', clear=True)
    screenshot('12-settings-git')
    click(307, 835)
    wait('native-viewer-controls-saved', lambda: state()['preferences']['codeFontSize'] == 13 and state()['preferences']['tabWidth'] == 5 and state()['preferences']['showWhitespace'])
    wait('native-git-control-saved', lambda: state()['preferences']['gitExecutable'] == '/usr/bin/git')
    wait('native-navigation-controls-saved', lambda: not state()['preferences']['languageServers'][0]['enabled'] and next(s for s in state()['preferences']['languageServers'] if s['id'] == 'python')['settings']['python']['pythonPath'] == '/usr/bin/python3')
    click(1395, 24); time.sleep(.3)
    type_at(500, 207, 'Dracula', clear=True); screenshot('13-theme-search')
    click(375, 444); time.sleep(.3)
    click(307, 835)
    wait('theme-selection-saved', lambda: state()['preferences']['appearance']['darkTheme'] == 'dracula')
    quit_app(); start(with_path=False)
    wait('native-settings-restart', lambda: state()['preferences']['codeFontSize'] == 13 and state()['preferences']['appearance']['darkTheme'] == 'dracula')
    # Quick-open must bring keyboard candidates beyond the first viewport into view.
    (root / 'quick').mkdir()
    for i in range(75): (root / 'quick' / f'file{i:03}.txt').write_text(''.join(f'column{n:03} ' for n in range(100)) + '\n')
    (root / 'binary.dat').write_bytes(b'\x00\x01binary')
    (root / 'link.txt').symlink_to('a.rs')
    (root / 'deferred.txt').write_text('deferred source\n' * 22000)
    git('mv', 'b.txt', 'renamed.txt')
    module=root/'module';module.mkdir()
    run('git','-C',str(module),'init');run('git','-C',str(module),'config','user.name','Native smoke');run('git','-C',str(module),'config','user.email','native@example.invalid')
    (module/'source.txt').write_text('submodule source\n')
    run('git','-C',str(module),'add','.');run('git','-C',str(module),'commit','-m','Submodule fixture')
    git('update-index', '--add', '--cacheinfo', '160000,' + run('git','-C',str(module),'rev-parse','HEAD').strip() + ',module')
    blobs=[]
    for value in ['base\n','ours\n','theirs\n']:
        blobs.append(subprocess.run(['git','-C',str(root),'hash-object','-w','--stdin'],input=value,text=True,check=True,stdout=subprocess.PIPE).stdout.strip())
    subprocess.run(['git','-C',str(root),'update-index','--index-info'],input=''.join(f'100644 {blob} {stage}\tconflict.txt\n' for stage,blob in enumerate(blobs,1)),text=True,check=True)
    (root / 'conflict.txt').write_text('<<<<<<< ours\nours\n=======\ntheirs\n>>>>>>> theirs\n')

    click(1248, 24); time.sleep(1)
    type_at(120, 118, 'quick/', clear=True)
    run('xdotool', 'key', '--repeat', '50', '--delay', '25', 'Down')
    def candidate_visible():
        screenshot('14-keyboard-candidate-visible')
        return re.search(r'file[0O][5S][0O]', run('tesseract', str(output / '14-keyboard-candidate-visible.png'), 'stdout'))
    wait('quick-open-candidate-pixels', candidate_visible)
    run('xdotool', 'key', 'Return')
    wait('keyboard-quick-open-beyond-viewport', lambda: state()['preferences']['sessions'][str(root)]['selectedPath'] == 'quick/file050.txt')
    screenshot('15-long-line')
    click(700, 230)
    run('xdotool', 'key', '--repeat', '12', '--delay', '40', 'Right')
    time.sleep(.4)
    screenshot('15b-horizontal-scrolled')
    quit_app()
    wait('horizontal-scroll-persisted', lambda: any(abs(v.get('horizontal', 0)) > 0 for v in state().get('nativeState', {}).values()))
    saved_horizontal = next(v['horizontal'] for k,v in state()['nativeState'].items() if 'file050' in k)
    start(with_path=False); screenshot('16-horizontal-restored')
    quit_app()
    assert next(v['horizontal'] for k,v in state()['nativeState'].items() if 'file050' in k) == saved_horizontal
    checks.append('horizontal-scroll-restored-after-layout')
    start(with_path=False)
    for query in ['binary.dat','link.txt','renamed.txt','module','conflict.txt','deferred.txt']:
        type_at(120, 118, query, clear=True); run('xdotool', 'key', 'Return')
        wait('special-file-' + query, lambda query=query: state()['preferences']['sessions'][str(root)]['selectedPath'] == query)
        screenshot('17-' + query)
    click(680,215)
    time.sleep(1)
    screenshot('18-explicit-large-source')
    assert 'deferredsource' in re.sub(r'[^a-z]', '', run('tesseract', str(output / '18-explicit-large-source.png'), 'stdout').lower())
    checks.append('large-source-explicitly-loaded')
    quit_app()
    (output / 'result.json').write_text(json.dumps({'passed': checks, 'scope': 'actual X11 window input plus persisted-state assertions; screenshots require visual inspection'}, indent=2))
    print(json.dumps({'passed': len(checks), 'output': str(output)}))
except Exception:
    screenshot("failure")
    raise
finally:
    if process is not None and process.poll() is None:
        process.terminate(); process.wait(timeout=10)
    log.close()
    work.cleanup()
