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
def text(value):
    subprocess.run(['xclip','-selection','clipboard'],input=str(value),text=True,check=True);key('ctrl+v')
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
    rows=csv.DictReader(io.StringIO(run('tesseract',str(path),'stdout','tsv')),delimiter='\t')
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
    run('xdotool','windowfocus','--sync',dialog_id())
    if Path(path).is_dir(): click_text('peer',last=True)
    key('ctrl+l');key('ctrl+a');subprocess.run(['xclip','-selection','clipboard'],input=str(path),text=True,check=True);key('ctrl+v')
    if not Path(path).is_dir(): key('Return')
    time.sleep(.4)
    if dialog_id():
        if Path(path).is_dir(): click_text('Open repository',last=True)
        else: click_text('Save' if save else 'Open',last=True)
    wait('native-dialog-accepted',lambda:not dialog_id())
    run('xdotool','windowfocus','--sync',peer_window)
    run('wmctrl','-i','-a',hex(int(peer_window)))

git('init');git('config','user.name','Native tree test');git('config','user.email','tree@example.invalid')
for name in ['app/components/card.rs','app/components/dialog.rs','app/lib/codec.rs','docs/guide.md','docs/deleted.md','docs/renamed.md','z-notes.txt']:
    path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text('old source\n')
git('add','.');git('commit','-m','Tree baseline')
for path in root.rglob('*'):
    if path.is_file() and '.git' not in path.parts:path.write_text('new source\n')
(root/'docs/deleted.md').unlink();git('mv','docs/renamed.md','docs/moved.md');(root/'docs/moved.md').write_text('old source\n')
(root/'added.txt').write_text('added source\n');git('add','added.txt')
longpath='app/components/deep/one/two/three/four/five/six/very-long-unicode-猫-file-name.rs'
path=root/longpath;path.parent.mkdir(parents=True,exist_ok=True);path.write_text('deep source\n')
(root/'many').mkdir()
for i in range(1000):(root/'many'/f'file{i:04}.rs').write_text(f'source {i}\n')
def tree_text():
    shot('tree-latest');run('convert',str(out/'tree-latest.png'),'-crop','244x668+0+168',str(out/'tree-crop.png'))
    return norm(run('tesseract',str(out/'tree-crop.png'),'stdout','--psm','6'))
def selected():return state().get('preferences',{}).get('sessions',{}).get(str(root),{}).get('selectedPath')
def choose(query):
    enter(100,142,query);key('Return');wait('selected-'+query,lambda: selected()==query)
def clear_filter():click(100,142);key('ctrl+a');key('BackSpace');key('Tab');time.sleep(.5)
if os.getenv('PEER_TREE_BASELINE'):
    process=subprocess.Popen([os.environ['PEER_TREE_BASELINE'],str(root)],env={**os.environ,'PEER_DATA_DIR':str(base/'baseline-store')},stdout=log,stderr=log)
    time.sleep(3)
    old_window=run('xdotool','search','--onlyvisible','--pid',str(process.pid)).splitlines()[0]
    run('wmctrl','-i','-r',hex(int(old_window)),'-b','add,fullscreen');run('wmctrl','-i','-a',hex(int(old_window)))
    shot('00-db2e860-before');key('ctrl+q');process.wait(timeout=15)
recorder=None
try:
    process=subprocess.Popen([str(binary),str(root)],env={**os.environ,'PEER_DATA_DIR':str(store)},stdout=log,stderr=log)
    def find_peer():
        try:return run('xdotool','search','--onlyvisible','--pid',str(process.pid)).splitlines()[0]
        except (subprocess.CalledProcessError,IndexError):return None
    wait('native-window',find_peer);peer_window=find_peer()
    run('xprop','-id',peer_window,'-f','_MOTIF_WM_HINTS','32c','-set','_MOTIF_WM_HINTS','2, 0, 0, 0, 0')
    run('xdotool','windowmove',peer_window,'0','0','windowsize',peer_window,'1440','900','windowfocus','--sync',peer_window)
    run('wmctrl','-i','-r',hex(int(peer_window)),'-b','add,fullscreen');run('wmctrl','-i','-a',hex(int(peer_window)));time.sleep(1)
    if os.getenv('PEER_TREE_RECORD'):
        recorder=subprocess.Popen(['ffmpeg','-y','-f','x11grab','-video_size','1440x900','-framerate','15','-i',os.environ['DISPLAY'],'-c:v','libx264','-preset','veryfast','-crf','22','-pix_fmt','yuv420p','-movflags','+faststart',str(out/'tree-interaction.mp4')],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=(out/'recording.log').open('w'))
    if not os.getenv('PEER_TREE_WHEEL_ONLY'):
        choose('app/components/card.rs');clear_filter();shot('01-rich-tree')
        assert 'cardrs' in tree_text();checks.append('selected-tree-row-visible')
        click(400,211);enter(1200,210,'Tree comment indicator');click(1200,310);click(1200,115);time.sleep(.7);shot('01b-comment-reviewed-icons')
        # Collapse and use real four-way tree keyboard operations.
        click(65,184);shot('02-collapsed-app');assert 'cardrs' not in tree_text();checks.append('folder-collapse-hides-descendants')
        key('Right');assert 'cardrs' in tree_text();checks.append('right-expands-directory')
        key('Right');key('Left');assert 'cardrs' not in tree_text();checks.append('left-collapses-child-directory')
        key('Right');key('Down')
        shot('03-keyboard-hierarchy')
        # Filtering is flat, shows full paths, and preserves collapsed unrelated folders.
        click(65,184);assert 'cardrs' not in tree_text();checks.append('palette-starts-with-collapsed-ancestor')
        key('ctrl+p');text('very-long-unicode');wait('independent-go-to-file-visible',lambda:pixels_contain('Go to file'));shot('03b-go-to-file')
        key('Return');wait('palette-reveals-collapsed-deep-file',lambda:selected()==longpath);shot('03c-palette-revealed');assert 'cardrs' in tree_text()
        key('ctrl+p');text('many/');run('xdotool','key','--repeat','50','--delay','20','Down');shot('03d-palette-keyboard-scroll');key('Return');wait('palette-keyboard-opens-fiftieth-result',lambda:selected()=='many/file0050.rs')
        choose(longpath);shot('04-full-path-filter');run('xdotool','mousemove','120','184');time.sleep(1.5);shot('04b-full-path-tooltip');clear_filter();shot('05-deep-revealed')
        # Selection outside the tree must reveal a previously collapsed ancestor.
        run('xdotool','mousemove','100','220','click','4','click','4','click','4','click','4','click','4','click','4','click','4','click','4');time.sleep(.5)
        click(60,184) # app folder
        choose('docs/guide.md');clear_filter();click(1320,24)
        wait('back-restores-deep-file',lambda:selected()==longpath);shot('06-back-expanded')
        choose('many/file0999.rs');clear_filter();shot('07-last-file-revealed')
        key('ctrl+p');text('many/');key('Down');key('Escape');checks.append('palette-cancel-keeps-selection');assert selected()=='many/file0999.rs'
        wait('selected-row-visible',lambda:re.search(r'file[0o]999rs',tree_text()))
        run('xdotool','mousemove','238','820');time.sleep(.5);shot('07b-native-scrollbar')
        run('xdotool','mousedown','1','mousemove','238','170','mouseup','1');time.sleep(.5)
        wait('scrollbar-drag-reaches-root',lambda:'components' in tree_text());shot('07c-scrollbar-drag')
        choose('many/file0999.rs');clear_filter()
        # Parent and Home/End move keyboard focus and keep the selected row visible.
        click(110,820);key('Left');shot('08-parent-focused');key('Right');key('End')
        wait('keyboard-end-opens-last-file',lambda:selected()=='z-notes.txt');shot('09-keyboard-end');key('Up');wait('keyboard-up-opens-previous-file',lambda:selected()=='added.txt');key('Down');wait('keyboard-down-opens-next-file',lambda:selected()=='z-notes.txt')
    # Multi-file scrolling drives selection, without jumping the Diff back.
    choose('app/components/card.rs');clear_filter();click(800,72);wait('multifile-load-completed',lambda:pixels_contain('1009 files loaded'));before_scroll=selected()
    run('xdotool','mousemove','800','400','click','--repeat','45','--delay','35','5');time.sleep(1)
    wait('multifile-wheel-updates-selected',lambda:selected()!=before_scroll and selected() is not None)
    shot('10-multifile-scroll-sync')
    key('ctrl+q');process.wait(timeout=15);assert process.returncode==0
    (out/'result.json').write_text(json.dumps({'passed':checks,'files':1009,'method':'native keyboard/mouse + OCR + persistent state; screenshots visually inspected'},indent=2))
    print(json.dumps({'passed':len(checks)}))
except Exception:
    shot('failure');raise
finally:
    if recorder is not None:recorder.communicate(b'q',timeout=30)
    if process is not None and process.poll() is None:process.terminate();process.wait(timeout=15)
    log.close();work.cleanup()
