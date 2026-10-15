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
output = Path(sys.argv[2] if len(sys.argv) > 2 else 'test-results/native-original-parity').resolve()
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

def wait(label, predicate, timeout=40):
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
    deadline=time.monotonic()+40
    while time.monotonic()<deadline:
        screenshot('ready-controls')
        if re.sub(r'[^a-z0-9]','',phrase.lower()) in re.sub(r'[^a-z0-9]','',run('tesseract',str(output/'ready-controls.png'),'stdout').lower()): return
    raise AssertionError('Rendered control did not appear: '+phrase)

def absent(phrase):
    deadline=time.monotonic()+40
    while time.monotonic()<deadline:
        screenshot('absent-controls')
        if phrase not in run('tesseract',str(output/'absent-controls.png'),'stdout'): return
    raise AssertionError('Control did not close: '+phrase)

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
    # Fixed toolbar coordinates in the disposable 1440x900 fixture; outcomes
    # are still checked through source pixels and exact persisted identities.
    toolbar={'All files':(800,72),'Single file':(875,72),'Commit history':(980,72),'Back':(1320,24),'Refresh':(1250,24),'Toggle Reviewed':(1200,140)}
    if label in toolbar:
        click(*toolbar[label]);return
    import csv,io
    wanted=re.sub(r'[^a-z0-9]','',label.lower())
    deadline=time.monotonic()+40
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
paths = {'main': 'app/main.rs', 'config': 'config/app.json', 'longpath':'workspace/components/panels/review/very_long_path_for_full_draft_target.rs'}
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
    found=re.search(r'identity\s+(main|config|module[0-9od@sl]{2})\b',value)
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
    # OCR confuses the zero in numbered fixture filenames with lowercase o.
    value=re.sub(r'module([0-9o]{2})rs',lambda m:'module'+m[1].replace('o','0')+'rs',value)
    expected=re.sub(r'[^a-z0-9]','',Path(path).name.lower())
    assert expected in value, (expected,value)
def assert_identity(label, expected=None, previous=None):
    deadline=time.monotonic()+60
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
    # Move focus to Find without typing, so the caret cannot become OCR text.
    click(390,144)
    screenshot('draft-editor')
    crop=output/'draft-editor-crop.png'
    run('convert',str(output/'draft-editor.png'),'-crop','290x80+1140+208','-resize','200%',str(crop))
    return run('tesseract',str(crop),'stdout','--psm','6').strip().strip('|')
def search(query):
    type_at(390,144,query,True);time.sleep(.4);press('Return');time.sleep(.6)
def reviewed(): return {p for q in state().get('queues',{}).values() for p in q['reviewed']}
def header_action(path, action):
    import csv,io
    screenshot('header-action')
    rows=[r for r in csv.DictReader(io.StringIO(run('tesseract',str(output/'header-action.png'),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE) if r.get('text','').strip()]
    name=next(r for r in rows if re.sub(r'[^a-z0-9]','',r['text'].lower())==re.sub(r'[^a-z0-9]','',path.lower()) and 250<int(r['left'])<600)
    y=int(name['top'])+int(name['height'])//2
    click(900 if action=='review' else 1040,y)

def edit_body(value):
    import csv,io
    deadline=time.monotonic()+40
    while time.monotonic()<deadline:
        screenshot('edit-location')
        rows=[r for r in csv.DictReader(io.StringIO(run('tesseract',str(output/'edit-location.png'),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE) if r.get('text','').strip()]
        labels=[r for r in rows if int(r['left'])>1130 and int(r['top'])>360 and (
            re.sub(r'[^a-z]','',r['text'].lower())=='editcomment' or
            (r['text']=='Edit' and any(q['text'].lower()=='comment' and abs(int(q['top'])-int(r['top']))<5 and 0<int(q['left'])-int(r['left'])<60 for q in rows)))]
        if labels:
            label=min(labels,key=lambda r:int(r['top']))
            break
    else:
        raise AssertionError('No rendered card editor label')
    type_at(1200,int(label['top'])+40,value,True)

try:
    start()
    rendered('files loaded')
    assert state()['preferences']['allFiles'] is True
    assert central_file()==paths['main']
    screenshot('01-default-continuous')
    checks.append('changes-defaults-to-continuous-files')
    header_action(paths['config'],'review')
    wait('header-reviewed-bound-to-config',lambda:reviewed()=={paths['config']})
    assert central_file()==paths['main']
    wait('explicit-header-selection-survives-other-visible-file',lambda:selected_path()==paths['config'])
    header_action(paths['config'],'comment')
    type_at(1200,240,'config header comment')
    click(1200,340)
    wait('header-comment-exact-path',lambda:[(c['path'],c['body']) for c in comments()]==[(paths['config'],'config header comment')])
    screenshot('02-bound-file-controls')
    type_at(1200,240,'independent new draft')
    button('Edit')
    edit_body('edited config comment')
    wait('new-composer-visible-during-edit',lambda:editor_text()=='independent new draft')
    checks.append('editing-card-keeps-new-composer')
    screenshot('03-independent-editors')
    click(1200,340)
    wait('new-save-while-editing',lambda:any(c['body']=='independent new draft' for c in comments()))
    rendered('Save edit')
    type_at(1200,240,'new draft during edit save')
    button('Save edit')
    wait('card-edit-saved',lambda:any(c['body']=='edited config comment' for c in comments()))
    wait('latest-new-composer-visible',lambda:editor_text()=='new draft during edit save')
    checks.append('edit-save-preserves-latest-new-draft')
    button('Edit');edit_body('cancelled edit');button('Cancel edit')
    wait('latest-new-composer-visible',lambda:editor_text()=='new draft during edit save')
    assert all(c['body']!='cancelled edit' for c in comments())
    checks.append('cancel-edit-preserves-new-draft')
    (root/paths['config']).write_text('// IDENTITY config changed again source\n')
    button('Refresh');rendered('Outdated')
    screenshot('03b-outdated-comment-metadata')
    checks.append('changed-source-comment-outdated-tag')
    button('Commit history');rendered('Identity baseline')
    rendered('Native smoke')
    screenshot('04-history-author-date')
    click(90,240)
    absent('Identity baseline');screenshot('05-history-outside-tree')
    assert 'Identity baseline' not in run('tesseract',str(output/'05-history-outside-tree.png'),'stdout')
    checks.append('history-outside-left-tree-closes')
    button('Commit history');rendered('Identity baseline');press('Escape')
    absent('Identity baseline');screenshot('06-history-escape')
    assert 'Identity baseline' not in run('tesseract',str(output/'06-history-escape.png'),'stdout')
    checks.append('history-escape-closes')
    button('Commit history');rendered('Identity baseline');button('Identity baseline')
    wait('history-selects-commit-comparison',lambda:comparison().get('mode')=='commit')
    rendered('Native smoke');screenshot('06b-selected-commit-summary')
    button('Commit history');rendered('Identity baseline');screenshot('06c-current-commit-check')
    press('Escape');click(110,72)
    wait('return-to-working-comparison',lambda:comparison().get('mode')=='all')
    rendered('files loaded')
    button('Single file');wait('single-file-preference-persisted',lambda:state()['preferences']['allFiles'] is False)
    quit_app();start();wait('single-file-preference-survives-restart',lambda:state()['preferences']['allFiles'] is False)
    screenshot('07-explicit-single-file-restart')
    button('All files');rendered('files loaded')
    wait('all-files-preference-restored',lambda:state()['preferences']['allFiles'] is True)
    choose(paths['longpath'])
    screenshot('08-full-draft-target-path')
    run('convert',str(output/'08-full-draft-target-path.png'),'-crop','310x90+1130+160','-resize','200%',str(output/'full-target-crop.png'))
    target_text=re.sub(r'[^a-z0-9]','',run('tesseract',str(output/'full-target-crop.png'),'stdout','--psm','6').lower())
    assert re.sub(r'[^a-z0-9]','',paths['longpath']) in target_text,target_text
    checks.append('full-long-draft-target-visible')
    (output/'state.json').write_text(json.dumps(state(),indent=2))
    quit_app()
    (output/'result.json').write_text(json.dumps({'passed':checks},indent=2))
    print(json.dumps({'passed':len(checks)}))
except Exception:
    screenshot('failure');(output/'failure-state.json').write_text(json.dumps(state(),indent=2));raise
finally:
    if process is not None and process.poll() is None:process.terminate();process.wait(timeout=15)
    log.close()
