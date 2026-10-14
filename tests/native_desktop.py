#!/usr/bin/env python3
"""Native desktop integration; run in disposable Xvfb + D-Bus + GTK portal session.
Uses real input, OCR of rendered pixels, and resulting application/export files.
Requires xdotool, ImageMagick, tesseract, Git and a running desktop portal.
"""
import csv
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve(); out.mkdir(parents=True, exist_ok=True)
(out / 'result.json').unlink(missing_ok=True)
work = tempfile.TemporaryDirectory(prefix='peer-desktop-')
base = Path(work.name); root = base / 'repo'; root.mkdir(); store = base / 'store'
checks = []; process = None
log = (out / 'window.log').open('w')
def run(*args): return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL)
def git(*args): return run('git', '-C', str(root), *args)
def click(x,y): run('xdotool','mousemove',str(x),str(y),'click','1'); time.sleep(.25)
def key(*args): run('xdotool','key',*args); time.sleep(.2)
def text(value): run('xdotool','type','--clearmodifiers','--delay','8',str(value)); time.sleep(.2)
def enter(x,y,value): click(x,y); key('ctrl+a'); text(value)
def state():
    try: return json.loads((store/'state.json').read_text())
    except (FileNotFoundError,json.JSONDecodeError): return {}
def wait(name, predicate, timeout=40):
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        if predicate(): checks.append(name);time.sleep(.4);return
        if process and process.poll() is not None: raise AssertionError('Native process exited: '+name)
        time.sleep(.15)
    raise AssertionError('Timeout: '+name)
def shot(name):
    time.sleep(.5); path=out/(name+'.png');run('import','-window','root',str(path));return path
def words(inverted=False):
    path=shot('latest')
    if inverted:
        run('convert',str(path),'-negate',str(out/'ocr-inverted.png'));path=out/'ocr-inverted.png'
    rows=csv.DictReader(io.StringIO(run('tesseract',str(path),'stdout','tsv')),delimiter='\t',quoting=csv.QUOTE_NONE)
    return [r for r in rows if r.get('text','').strip()]
def norm(s): return re.sub(r'[^a-z0-9]','',s.lower())
def click_text(label,last=False):
    expected=[norm(t) for t in label.split()];matches=[]
    for inverted in [False,True]:
        rows=words(inverted)
        for i in range(len(rows)-len(expected)+1):
            if [norm(r['text']) for r in rows[i:i+len(expected)]]==expected:matches.append(rows[i:i+len(expected)])
        if matches:break
    if not matches: raise AssertionError('Rendered control missing: '+label)
    m=matches[-1 if last else 0];left=min(int(r['left']) for r in m);top=min(int(r['top']) for r in m)
    right=max(int(r['left'])+int(r['width']) for r in m);bottom=max(int(r['top'])+int(r['height']) for r in m)
    click((left+right)//2,(top+bottom)//2)
def pixels_contain(label):
    if norm(label) in norm(' '.join(r['text'] for r in words())): return True
    # OCR's automatic segmentation skips light text on the colored error banner.
    crop=out/'banner.png'
    run('convert',str(out/'latest.png'),'-crop','1440x40+0+96','-colorspace','Gray','-negate',str(crop))
    if norm(label) in norm(run('tesseract',str(crop),'stdout','--psm','7')): return True
    run('convert',str(out/'latest.png'),'-crop','1440x40+0+860',str(crop))
    return norm(label) in norm(run('tesseract',str(crop),'stdout','--psm','7'))
def dialog_id():
    try: return run('xdotool','search','--onlyvisible','--name','^(Open|Save)').splitlines()[-1]
    except (subprocess.CalledProcessError,IndexError): return None
def choose_path(path,save=False):
    wait('native-dialog-visible',dialog_id)
    run('wmctrl','-i','-a',hex(int(dialog_id())));run('xdotool','windowfocus','--sync',dialog_id())
    if Path(path).is_dir(): click(220,100)  # GTK Home exits Recent before editing a location.
    key('ctrl+l');key('ctrl+a');subprocess.run(['xclip','-selection','clipboard'],input=str(path),text=True,check=True);key('ctrl+v')
    time.sleep(.7)
    if not Path(path).is_dir(): key('Return');time.sleep(.5)
    if dialog_id(): click_text('Open repository' if Path(path).is_dir() else ('Save' if save else 'Open'),last=True)
    wait('native-dialog-accepted',lambda:not dialog_id())
    run('xdotool','windowfocus','--sync',peer_window)
    run('wmctrl','-i','-a',hex(int(peer_window)))

git('init');git('config','user.name','Native desktop');git('config','user.email','native@example.invalid')
(root/'a.rs').write_text('fn main() {\n    println!("old");\n}\n')
git('add','.');git('commit','-m','Initial')
(root/'a.rs').write_text('fn main() {\n    println!("new");\n}\n')
scenario=os.getenv('PEER_NATIVE_SCENARIO','desktop')
(root/'caller.py').write_text('from definitions import target\nvalue = target()\n')
(root/'definitions.py').write_text('def target():\n    # frozen source\n    return 42\n')
(root/'pyproject.toml').write_text('[project]\nname = "native-lsp-test"\nversion = "0.0.0"\n')
theme=base/'native-theme.json';theme.write_text(json.dumps({'name':'Portal proof','type':'dark','colors':{'editor.background':'#172032','editor.foreground':'#eeeeee'},'tokenColors':[{'scope':'comment','settings':{'foreground':'#abc123','fontStyle':'italic bold'}}]}))
try:
    process=subprocess.Popen([str(binary)] + ([str(root)] if scenario!='desktop' else []),env={**os.environ,'PEER_DATA_DIR':str(store),**({'DBUS_SESSION_BUS_ADDRESS':'unix:path=/tmp/peer-nonexistent-test-bus'} if scenario=='errors' else {})},stdout=log,stderr=log)
    def find_peer():
        try:return run('xdotool','search','--onlyvisible','--pid',str(process.pid)).splitlines()[0]
        except (subprocess.CalledProcessError,IndexError):return None
    wait('native-window',find_peer);peer_window=find_peer()
    run('xprop','-id',peer_window,'-f','_MOTIF_WM_HINTS','32c','-set','_MOTIF_WM_HINTS','2, 0, 0, 0, 0')
    run('xdotool','windowmove',peer_window,'0','0','windowsize',peer_window,'1440','900','windowfocus','--sync',peer_window)
    run('wmctrl','-i','-r',hex(int(peer_window)),'-b','add,fullscreen');run('wmctrl','-i','-a',hex(int(peer_window)));time.sleep(.6)
    if scenario != 'desktop':
        wait('repository-opened',lambda:state().get('preferences',{}).get('lastRepository')==str(root))
        if scenario == 'navigation':
            enter(120,142,'caller.py');key('Return')
            wait('caller-selected',lambda:state()['preferences']['sessions'][str(root)]['selectedPath']=='caller.py')
            click(395,231);click_text('Line scope');enter(1200,210,'Draft survives frozen navigation');enter(350,811,'8');key('F12')
            wait('single-definition-opens-directly',lambda:pixels_contain('frozen source'));shot('01-definition-result')
            wait('frozen-source-visible',lambda:pixels_contain('frozen source'));shot('02-frozen-source')
            enter(380,118,'return');key('Return')
            wait('frozen-source-search',lambda:pixels_contain('Match 1/1') and pixels_contain('line 3'))
            enter(706,118,'3');click(764,118);shot('03-frozen-search-jump')
            click(1320,24);time.sleep(.4);click(1320,24);time.sleep(.4);click(1320,24)
            wait('original-anchor-restored',lambda:pixels_contain('Comment on new line 2'));shot('04-back-anchor')
            click(1200,310)
            wait('draft-saved-to-original-anchor',lambda:any(c['body']=='Draft survives frozen navigation' and c['path']=='caller.py' and c['anchor'].get('line')==2 and c['anchor'].get('side')=='new' for q in state()['queues'].values() for c in q['comments']))
            # Saving clears the comment anchor; select the original call again.
            click(395,231);enter(350,811,'8');key('shift+F12')
            wait('real-reference-results',lambda:pixels_contain('2 results'));shot('05-references')
        else:
            click_text('Browse')
            wait('folder-error-visible',lambda:pixels_contain('No such file') or pixels_contain('connection'))
            shot('01-folder-error');click_text('Refresh');time.sleep(.8)
            click_text('Export')
            wait('export-error-visible',lambda:pixels_contain('Save dialog'))
            shot('02-save-error');click_text('Refresh');time.sleep(.8)
            click_text('Settings');click_text('Import theme')
            wait('import-error-visible',lambda:pixels_contain('Theme import dialog'));shot('03-import-error')
        key('ctrl+q');process.wait(timeout=15);assert process.returncode==0
        (out/'result.json').write_text(json.dumps({'passed':checks,'scenario':scenario},indent=2));print(json.dumps({'passed':len(checks),'scenario':scenario}));sys.exit(0)
    click(1090,24);choose_path(root)
    wait('folder-opened',lambda:state().get('preferences',{}).get('lastRepository')==str(root));shot('01-folder-opened')
    click(405,231);click_text('Line scope');enter(1200,210,'Portal export comment');click(1200,310)
    wait('comment-created',lambda:any(q['comments'] for q in state().get('queues',{}).values()))
    click(1300,116);clipboard=run('xclip','-selection','clipboard','-o')
    click(1360,116);export=base/'feedback.json';choose_path(export,save=True)
    wait('export-file-written',export.exists);assert json.loads(export.read_text())==json.loads(clipboard);checks.append('native-export-equals-clipboard');shot('02-exported')
    click(1395,24);click_text('Import theme');choose_path(theme)
    wait('theme-imported',lambda:any(t['name']=='Portal proof' for t in state().get('themes',[])))
    enter(500,207,'Portal proof');time.sleep(.5);click(375,444);click(307,835)
    wait('imported-theme-selected',lambda:state()['preferences']['appearance']['darkTheme'].startswith('custom-'))
    shot('03-import-selected')
    click(1395,24);enter(500,207,'Portal proof');time.sleep(.5);click(375,476)
    wait('imported-theme-removed',lambda:not state().get('themes'));shot('04-theme-removed');click(425,835)
    oldroot=root;oldid=state()['repositoryIds'][str(oldroot)]
    root=base/'relocated';oldroot.rename(root)
    enter(450,24,str(root));click(1172,24);click_text('Move to path above')
    wait('repository-relocated',lambda:state()['repositoryIds'].get(str(root))==oldid and state()['preferences']['lastRepository']==str(root))
    assert any(q['comments'] for q in state()['queues'].values());checks.append('relocation-preserved-review');shot('05-relocated')
    # The recent list can remain open after relocation; close/reopen by pixels.
    if not pixels_contain('Remove from list'):click(1172,24)
    click_text('Remove from list')
    wait('recent-removed',lambda:str(root) not in state()['preferences']['recentRepositories'])
    assert (root/'a.rs').exists() and any(q['comments'] for q in state()['queues'].values());checks.append('remove-preserved-source-and-review');shot('06-removed')
    click(1172,24);enter(450,24,str(root));key('Return')
    wait('removed-repository-reopened',lambda:state()['preferences']['lastRepository']==str(root))
    enter(120,142,'caller.py');key('Return')
    wait('other-file-selected',lambda:state()['preferences']['sessions'][str(root)]['selectedPath']=='caller.py')
    click(1200,362)
    wait('comment-location-jump',lambda:state()['preferences']['sessions'][str(root)]['selectedPath']=='a.rs')
    assert pixels_contain('Saved source');checks.append('saved-comment-context-visible')
    enter(1200,210,'Discard this native draft');click_text('Discard draft')
    wait('draft-discarded',lambda:not any(v.get('body')=='Discard this native draft' for v in state().get('nativeState',{}).values()))
    click(931,24);shot('07-feedback-hidden')
    assert not pixels_contain('Save comment');checks.append('feedback-panel-hidden')
    click(931,24);wait('feedback-panel-restored',lambda:pixels_contain('Portal export comment'))
    click(1200,116);wait('reviewed-before-clear',lambda:any(q['reviewed'] for q in state()['queues'].values()))
    click_text('Clear comments');wait('clear-confirm-visible',lambda:pixels_contain('Confirm clear comments'));click_text('Cancel',last=True)
    assert any(q['comments'] for q in state()['queues'].values());checks.append('clear-cancel-retains-comments')
    click_text('Clear comments');click_text('Confirm clear comments')
    wait('clear-confirm-deletes-comments',lambda:all(not q['comments'] for q in state()['queues'].values()))
    assert any(q['reviewed'] for q in state()['queues'].values());checks.append('clear-retains-reviewed')
    shot('08-clear-confirmed')
    key('ctrl+q');process.wait(timeout=15);assert process.returncode==0
    (out/'result.json').write_text(json.dumps({'passed':checks,'method':'real native input + OCR controls + persisted state/export'},indent=2));print(json.dumps({'passed':len(checks)}))
except Exception:
    shot('failure')
    raise
finally:
    if process and process.poll() is None:process.terminate();process.wait(timeout=10)
    log.close();work.cleanup()
