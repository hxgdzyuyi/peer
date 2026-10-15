#!/usr/bin/env python3
"""All-files identity regression through an actual X11 window. Run under a 1440x900 Xvfb or a disposable desktop.
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
output = Path(sys.argv[2] if len(sys.argv) > 2 else 'test-results/native-identity').resolve()
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
    # Synthetic key-downs must not auto-repeat if the shared CI CPU stalls.
    run('xset','r','off')
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
        if re.sub(r'[^a-z0-9]','',phrase.lower()) in re.sub(r'[^a-z0-9]','',run('tesseract',str(output/'ready-controls.png'),'stdout').lower()): return
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
    wanted=re.sub(r'[^a-z0-9]','',label.lower())
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        screenshot('controls')
        words=[r for r in csv.DictReader(io.StringIO(run('tesseract',str(output/'controls.png'),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE) if r.get('text','').strip()]
        for i in reversed(range(len(words))):
            for size in range(1,len(label.split())+2):
                rows=words[i:i+size]
                if ''.join(re.sub(r'[^a-z0-9]','',r['text'].lower()) for r in rows)==wanted:
                    r=rows[0];click(int(r['left'])+int(r['width'])//2,int(r['top'])+int(r['height'])//2);return
    raise AssertionError('Missing button '+label)
def draft(body): return any(v.get('body')==body for v in state().get('nativeState',{}).values())

# Every source line identifies its own file. The oracle reads ONLY center pixels,
# never the selected session path, tree, draft, or an application diagnostic.
git('rm','-f','a.rs','b.txt')
paths = {'main': 'app/main.rs', 'config': 'config/app.json'}
paths.update({f'module{i:02}': f'src/module{i:02}.rs' for i in range(40)})
for token, path in paths.items():
    dest=root/path;dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_text(f'// IDENTITY {token} old source\n')
git('add','.');git('commit','-m','Identity baseline')
(root/'huge.rs').write_text('// deferred source\n'*22001)
for token, path in paths.items():
    (root/path).write_text(f'// IDENTITY {token} start_review new source\n')

def selected_path():
    return state().get('preferences',{}).get('sessions',{}).get(str(root),{}).get('selectedPath')
def choose(path):
    press('ctrl+p');wait('palette-open-'+path,palette_visible);run('xdotool','type','--clearmodifiers',path);press('Return')
    wait('selected-'+path,lambda:selected_path()==path)
    wait('palette-closed-'+path,lambda:not palette_visible())
def palette_visible():
    screenshot('palette-state')
    crop=output/'palette-title.png'
    run('convert',str(output/'palette-state.png'),'-crop','740x42+345+105','-resize','200%',str(crop))
    return 'gotofile' in re.sub(r'[^a-z0-9]','',run('tesseract',str(crop),'stdout','--psm','6').lower())
def central_file():
    screenshot('center-source')
    crop=output/'center-crop.png'
    run('convert',str(output/'center-source.png'),'-crop','870x644+250+144','-resize','300%',str(crop))
    value=run('tesseract',str(crop),'stdout','--psm','6').lower()
    (output/'center-ocr.txt').write_text(value)
    found=re.search(r'(?:identity\s+|src/)(main|config|module[0-9od@sl]{2})|app/main\.rs|config/app\.json',value)
    assert found, 'No independent source identity in center pixels: '+value
    token=found[1] or ('main' if 'main' in found[0] else 'config')
    if token.startswith('module'):
        token='module'+token[6:].translate(str.maketrans({'o':'0','d':'0','@':'0','s':'8','l':'1'}))
    return paths[token]
def tree_selected(path):
    # Check the selected row's green accent, then OCR that row alone.
    # ImageMagick reports matching accent pixels; no state-derived crop y.
    raw=run('convert',str(output/'center-source.png'),'-crop','3x660+0+172','txt:-')
    ys=[int(m[1]) for line in raw.splitlines() if (m:=re.match(r'0,(\d+):.*#(?:[0-9A-F]{2}){3}',line)) and '#22C55E' in line.upper()]
    # The exact accent is theme-owned; derive selected row via nonwhite green pixels.
    if not ys:
        ys=[]
        for line in raw.splitlines():
            m=re.match(r'0,(\d+):.*\((\d+),(\d+),(\d+)\)',line)
            if m and int(m[3])>int(m[2])+30 and int(m[3])>int(m[4])+20: ys.append(int(m[1]))
    assert ys, 'No tree selection accent in pixels'
    y=min(ys)+172
    crop=output/'selected-tree-row.png'
    run('convert',str(output/'center-source.png'),'-crop',f'240x32+0+{y}','-resize','300%',str(crop))
    value=re.sub(r'[^a-z0-9]','',run('tesseract',str(crop),'stdout','--psm','7').lower())
    expected=re.sub(r'[^a-z0-9]','',Path(path).name.lower())
    assert expected in value, (expected,value)
def assert_identity(label, expected=None, previous=None):
    deadline=time.monotonic()+20
    last=None
    while time.monotonic()<deadline:
        try:
            visible=central_file()
        except AssertionError as error:
            last=str(error)
            time.sleep(.2)
            continue
        last=(visible,selected_path())
        if (expected is None or visible==expected) and (previous is None or visible!=previous) and selected_path()==visible:
            try: tree_selected(visible)
            except AssertionError:
                time.sleep(.2);continue
            checks.extend([label+'-session-matches-center',label+'-tree-matches-center'])
            screenshot(label)
            return visible
        time.sleep(.2)
    raise AssertionError((label,'center/session',last,'expected',expected,'previous',previous))

def stored_draft(path):
    return next((v.get('body','') for k,v in state().get('nativeState',{}).items() if k.startswith('draft-v2:') and k.endswith(':'+path)),None)
def editor_text():
    screenshot('draft-editor')
    crop=output/'draft-editor-crop.png'
    run('convert',str(output/'draft-editor.png'),'-crop','290x100+1140+180','-resize','200%',str(crop))
    return run('tesseract',str(crop),'stdout','--psm','6').strip()
def search(query):
    type_at(390,120,query,True);time.sleep(.4);press('Return');time.sleep(.6)
def reviewed(): return {p for q in state().get('queues',{}).values() for p in q['reviewed']}
try:
    start()
    wait('initial-source-ready',lambda:selected_path()==paths['main'],timeout=40)
    assert_identity('initial-source',paths['main'])
    if os.getenv('PEER_IDENTITY_SCROLL_ONLY'):
        choose(paths['main']);button('All files');rendered('files loaded')
    else:
        choose(paths['main']);type_at(1200,210,'main latest draft')
        wait('main-draft-entered',lambda:editor_text()=='main latest draft')
        choose(paths['config']);type_at(1200,210,'config latest draft')
        wait('config-draft-entered',lambda:editor_text()=='config latest draft')
        choose(paths['main']);button('All files');rendered('files loaded')
        search('start_review');choose(paths['config'])
        assert_identity('quickopen-config',paths['config'])
        wait('config-draft-restored',lambda:editor_text()=='config latest draft')
        type_at(1200,210,'config edited after checkpoint',True)
        button('Back');assert_identity('all-files-back-main',paths['main'])
        wait('back-restores-main-draft',lambda:editor_text()=='main latest draft')
        wait('departing-config-draft-saved',lambda:stored_draft(paths['config'])=='config edited after checkpoint')
        button('Toggle Reviewed');wait('back-reviewed-writes-main-only',lambda:reviewed()=={paths['main']})
        click(1200,210);press('ctrl+Return')
        wait('back-comment-writes-main-only',lambda:[(c['path'],c['body']) for c in comments()]==[(paths['main'],'main latest draft')])
        choose(paths['config'])
        wait('latest-config-draft-restored',lambda:editor_text()=='config edited after checkpoint')
        button('Back');assert_identity('back-after-save',paths['main'])
        wait('submitted-draft-does-not-resurrect',lambda:editor_text()=='Review comment' and stored_draft(paths['main']) in (None,''))
        choose('huge.rs');button('Back');assert_identity('back-restores-all-files-mode',paths['main'])
        # Cancel a new asynchronous source load by immediately going Back.
        press('ctrl+p');run('xdotool','type','--clearmodifiers','huge.rs')
        run('xdotool','key','Return','mousemove','1320','24','click','1');time.sleep(1)
        assert_identity('back-invalidates-pending-source-load',paths['main'])
    # Real keyboard scroll, with no source click to repair the identity afterward.
    choose(paths['module05']);assert_identity('explicit-module05',paths['module05'])
    press('Next');after=assert_identity('pagedown',previous=paths['module05'])
    assert after!=paths['module05'];checks.append('pagedown-central-source-moved')
    before=reviewed();button('Toggle Reviewed')
    wait('pagedown-reviewed-exact-target',lambda:reviewed()==before|{after})
    type_at(1200,210,'PageDown target comment');press('ctrl+Return')
    wait('pagedown-comment-exact-target',lambda:any(c['path']==after and c['body']=='PageDown target comment' for c in comments()))
    assert not any(c['path']==paths['module05'] for c in comments());checks.append('old-file-not-written')
    # Palette restores Diff focus without a source click. These commands keep it.
    choose(paths['module20']);assert_identity('explicit-module20',paths['module20'])
    press('Prior');previous=assert_identity('pageup',previous=paths['module20'])
    press('Home');home=assert_identity('home',expected=paths['main'],previous=previous)
    press('End');assert_identity('end',previous=home)
    press('Home');assert_identity('home-again',paths['main'])
    run('xdotool','key','--repeat','45','--delay','25','Down');previous=assert_identity('down-arrow',previous=home)
    run('xdotool','key','--repeat','25','--delay','25','Up');assert_identity('up-arrow',previous=previous)
    # Preserve explicit selection at a clamped bottom, where top != selected file.
    choose(paths['module39']);time.sleep(1)
    wait('explicit-last-file-not-overridden',lambda:selected_path()==paths['module39'])
    screenshot('explicit-bottom-selection')
    before=reviewed();button('Toggle Reviewed')
    wait('explicit-bottom-review-target',lambda:reviewed()==before|{paths['module39']})
    # A scrollbar drag must resume viewport tracking too.
    previous=central_file()
    run('xdotool','mousemove','1118','760');time.sleep(.4)
    run('xdotool','mousedown','1','mousemove','1118','310','mouseup','1');time.sleep(.7)
    assert_identity('scrollbar',previous=previous)
    screenshot('final');(output/'state.json').write_text(json.dumps(state(),indent=2))
    quit_app();(output/'result.json').write_text(json.dumps({'passed':checks,'oracle':'source identity from center-only OCR, selected tree row pixels, actual persisted comment/review paths'},indent=2)+'\n')
    print(json.dumps({'passed':len(checks)}))
except Exception:
    screenshot('failure');(output/'failure-state.json').write_text(json.dumps(state(),indent=2));raise
finally:
    if process is not None and process.poll() is None:process.terminate();process.wait(timeout=15)
    log.close()
