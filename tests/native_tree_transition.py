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

git('init');git('config','user.name','Native transition');git('config','user.email','tree@example.invalid')
(root/'swap').write_text('previous file\n');git('add','.');git('commit','-m','Before directory replacement')
(root/'swap').unlink();(root/'swap').mkdir();(root/'swap'/'child.rs').write_text('new nested source\n');(root/'z.txt').write_text('last file\n')
def selected():return state().get('preferences',{}).get('sessions',{}).get(str(root),{}).get('selectedPath')
def tree_text():
    shot('latest');run('convert',str(out/'latest.png'),'-crop','244x668+0+168',str(out/'crop.png'));return norm(run('tesseract',str(out/'crop.png'),'stdout','--psm','6'))
try:
    process=subprocess.Popen([str(binary),str(root)],env={**os.environ,'PEER_DATA_DIR':str(store)},stdout=log,stderr=log)
    def window_id():
        try:return run('xdotool','search','--onlyvisible','--pid',str(process.pid)).splitlines()[0]
        except (subprocess.CalledProcessError,IndexError):return None
    wait('native-window',window_id);window=window_id()
    run('wmctrl','-i','-r',hex(int(window)),'-b','add,fullscreen');run('wmctrl','-i','-a',hex(int(window)))
    wait('deleted-file-selected',lambda:selected()=='swap');shot('01-file-directory-collision')
    click(60,184);wait('folder-collapse-keeps-deleted-file',lambda:'childrs' not in tree_text() and 'swap' in tree_text());shot('02-collapsed-directory')
    click(60,184);click(100,216);wait('child-file-selected',lambda:selected()=='swap/child.rs')
    click(60,184);key('ctrl+p');text('child.rs');key('Return');wait('palette-reveals-child',lambda:'childrs' in tree_text());shot('03-palette-child-reveal')
    click(100,248);wait('distinct-deleted-file-selected',lambda:selected()=='swap');shot('04-deleted-file')
    key('ctrl+q');process.wait(timeout=15);assert process.returncode==0
    (out/'result.json').write_text(json.dumps({'passed':checks,'scenario':'tracked file replaced by directory'},indent=2));print(json.dumps({'passed':len(checks)}))
except Exception:
    shot('failure');raise
finally:
    if process is not None and process.poll() is None:process.terminate();process.wait(timeout=15)
    log.close();work.cleanup()
