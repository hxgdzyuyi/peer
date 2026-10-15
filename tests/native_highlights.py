#!/usr/bin/env python3
"""Native Tree-sitter colors, theme restoration and file-switch response through X11.
Run only in a disposable 1440x900 display; sends real mouse and keyboard input."""
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
output = Path(sys.argv[2] if len(sys.argv) > 2 else 'test-results/native-highlights').resolve()
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
observations = []
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
    (output/'process.json').write_text(json.dumps({'executable':os.readlink(f'/proc/{process.pid}/exe'),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'display':os.environ.get('DISPLAY'),'method':'actual native process launched by the X11 interaction harness'},indent=2)+'\n')
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

# Syntax fixtures are selected through the real palette, never an application API.
git('rm','-f','a.rs','b.txt')
fixtures={
    'sample.swift': 'func greet() { print("HIGHLIGHT_SWIFT marker") }\n',
    'sample.cs': 'class Greeter { string Message = "HIGHLIGHT_CSHARP marker"; }\n',
    'sample.graphql': 'query { user(name: "HIGHLIGHT_GRAPHQL marker") { id } }\n',
    'sample.proto': 'syntax = "proto3";\nimport "HIGHLIGHT_PROTO_marker.proto";\nmessage Hello { string name = 1; }\n',
    'CMakeLists.txt': 'set(MESSAGE "HIGHLIGHT_CMAKE marker")\n',
    'sample.html': '<script>const answer = 42; const message = "HIGHLIGHT_HTML marker";</script>\n<style>.card { color: red; }</style>\n',
    'sample.md': '```rust\nfn greet() { let message = "HIGHLIGHT_MARKDOWN marker"; }\n```\n',
}
for path,source in fixtures.items(): (root/path).write_text(source.replace('marker','old'))
git('add','.');git('commit','-m','Highlight baseline')
for path,source in fixtures.items(): (root/path).write_text(source.replace('marker','new'))
(root/'workload.html').write_text('<script>const answer = {value: 42};</script>\n'*15000)
# Seed a previously imported, deliberately partial theme before launching Peer.
# Theme switching itself is performed through Settings; no live store mutation.
store.mkdir()
(store/'state.json').write_text(json.dumps({'themes':[{
    'id':'partial-import-a','name':'Partial imported A','kind':'dark',
    'definition':{'name':'Partial imported A','type':'dark',
        'colors':{'editor.background':'#202124','editor.foreground':'#f4f4f4'},
        'tokenColors':[{'scope':'comment','settings':{'foreground':'#123456'}}]}
}]}))
measurements=[]
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

def source_ocr(path):
    # Inline diff backgrounds can cause Tesseract to omit pale string glyphs.
    # Retain normal OCR and an independent thresholded rendering of the same pixels.
    normal=run('tesseract',str(path),'stdout').lower()
    contrast=output/'source-contrast.png'
    run('convert',str(path),'-colorspace','Gray','-threshold','40%',str(contrast))
    return normal+'\n'+run('tesseract',str(contrast),'stdout').lower()

def center_text(name):
    screenshot(name)
    run('convert',str(output/f'{name}.png'),'-crop','870x610+250+178','-resize','200%',str(output/'source-crop.png'))
    return source_ocr(output/'source-crop.png')
def source_ready(path):
    wanted=re.search(r'HIGHLIGHT_[A-Z]+',fixtures[path])[0].lower().replace('_','')
    wait('source-visible-'+path,lambda:wanted in re.sub(r'[^a-z]','',center_text('source-current')))
def source_color(name, color):
    rgb=tuple(int(color[i:i+2],16) for i in (0,2,4))
    raw=run('convert',str(output/f'{name}.png'),'-crop','870x400+250+220','-depth','8','-format','%c','histogram:info:-')
    count=0
    for line in raw.splitlines():
        m=re.search(r'^\s*(\d+):\s*\((\d+),(\d+),(\d+)',line)
        if m and max(abs(int(m[i+2])-rgb[i]) for i in range(3))<=8:count+=int(m[1])
    return count

def measured_choose(path):
    press('ctrl+p');wait('palette-open-'+path,palette_visible)
    run('xdotool','type','--clearmodifiers',path)
    wanted=re.search(r'HIGHLIGHT_[A-Z]+',fixtures[path])[0].lower().replace('_','')
    begin=time.monotonic();run('xdotool','key','Return')
    deadline=begin+40
    while time.monotonic()<deadline:
        # No fixed sleep in this measurement: first correct center-source pixels,
        # including screenshot and OCR overhead, are the response upper bound.
        run('import','-window','root',str(output/'response.png'))
        run('convert',str(output/'response.png'),'-crop','870x610+250+178','-resize','200%',str(output/'response-crop.png'))
        actual=source_ocr(output/'response-crop.png')
        if wanted in re.sub(r'[^a-z]','',actual):break
    else:raise AssertionError('No measured source response: '+path)
    elapsed=time.monotonic()-begin
    wait('selected-'+path,lambda:selected_path()==path)
    checks.append('independent-source-response-'+path)
    return elapsed, begin

def theme(name):
    click(1395,24);rendered('Appearance')
    # All seven fixture themes fit on screen; choose the rendered card directly.
    # Let its preview paint before sending a separate Save click.
    button(name);screenshot('theme-preview-'+name.replace(' ','-'));click(310,836)
    wait('theme-'+name,lambda:state()['preferences']['appearance']['darkTheme']=={'GitHub Dark':'github-dark','Dracula Theme':'dracula','Partial imported A':'partial-import-a'}[name])

try:
    start();rendered('files loaded');button('Single file')
    theme('GitHub Dark');source_ready('CMakeLists.txt')
    if os.getenv('PEER_HIGHLIGHT_BASELINE'):click(390,144)  # Old settings left focus on a hidden input.
    for path in fixtures:
        begin=time.monotonic();response,enter=measured_choose(path)
        name='syntax-'+path.replace('.','-')
        deadline=time.monotonic()+40
        while True:
            run('import','-window','root',str(output/f'{name}.png'))
            colors=source_color(name,'9ecbff')
            injected=source_color(name,{'sample.html':'79b8ff','sample.md':'b392f0'}[path]) if path in ['sample.html','sample.md'] else None
            if os.getenv('PEER_HIGHLIGHT_BASELINE') or (colors>2 and (injected is None or injected>2)):break
            assert time.monotonic()<deadline,(path,'source visible but expected syntax colors absent',colors,injected)
        color_response=time.monotonic()-enter
        measurements.append({'path':path,'interaction_to_ocr_seconds':round(time.monotonic()-begin,3),'enter_to_source_ocr_seconds':round(response,3),'enter_to_colored_pixels_seconds':round(color_response,3),'string_color_pixels':colors,'injected_number_or_function_pixels':injected})
        if not os.getenv('PEER_HIGHLIGHT_BASELINE'):
            assert colors>2,(path,colors)
            checks.append('actual-source-string-capture-color-'+path)
            if injected is not None:
                assert injected>2,(path,'missing injected number/function color',injected)
                checks.append('actual-injected-number-or-function-color-'+path)
    if os.getenv('PEER_HIGHLIGHT_BASELINE'):
        quit_app();(output/'result.json').write_text(json.dumps({'passed':checks,'measurements':measurements,'baseline':True},indent=2));sys.exit(0)
    choose('sample.swift');source_ready('sample.swift');theme('Partial imported A');source_ready('sample.swift');click(390,144);screenshot('theme-a')
    theme('Dracula Theme');source_ready('sample.swift');click(390,144);screenshot('theme-b')
    theme('Partial imported A');source_ready('sample.swift');click(390,144);screenshot('theme-a-again')
    for name in ['theme-a','theme-b','theme-a-again']:
        run('convert',str(output/f'{name}.png'),'-crop','870x400+250+220',str(output/f'{name}-crop.png'))
    same=subprocess.run(['compare','-metric','AE',str(output/'theme-a-crop.png'),str(output/'theme-a-again-crop.png'),'null:'],capture_output=True,text=True)
    different=subprocess.run(['compare','-metric','AE',str(output/'theme-a-crop.png'),str(output/'theme-b-crop.png'),'null:'],capture_output=True,text=True)
    assert same.returncode==0,same.stderr
    assert different.returncode==1,different.stderr
    checks.append('partial-import-a-dracula-partial-import-a-source-pixels-identical')
    # A real large-file selection and immediate return exercises obsolete work cancellation.
    begin=time.monotonic();press('ctrl+p');wait('rapid-palette-visible',palette_visible)
    run('xdotool','type','--clearmodifiers','--delay','0','workload.html')
    run('xdotool','key','Return')
    deadline=time.monotonic()+10
    while selected_path()!='workload.html' and time.monotonic()<deadline:time.sleep(.01)
    assert selected_path()=='workload.html','Large source selection was not observed'
    checks.append('large-source-session-observed-before-return')
    run('xdotool','key','ctrl+p')
    run('xdotool','type','--clearmodifiers','--delay','0','sample.swift')
    run('xdotool','key','Return')
    wait('rapid-return-swift-session',lambda:selected_path()=='sample.swift');source_ready('sample.swift')
    measurements.append({'action':'large-html-then-swift','interaction_to_ocr_seconds':round(time.monotonic()-begin,3)})
    time.sleep(2);source_ready('sample.swift');screenshot('after-background-generation-change')
    (output/'state.json').write_text(json.dumps(state(),indent=2))
    quit_app()
    (output/'result.json').write_text(json.dumps({'passed':checks,'measurements':measurements,'timing_note':'enter_to_source_ocr_seconds measures Return to first independent center-source OCR with no fixed sleep, including screenshot/OCR overhead. enter_to_colored_pixels_seconds also includes the source-identity and session checks before polling color pixels; it is an upper bound. Other times include palette, input and synchronization. These are software-rendered response upper bounds, not isolated render latency or incremental parsing benchmarks.'},indent=2))
    print(json.dumps({'passed':len(checks),'measurements':measurements}))
except Exception:
    screenshot('failure');(output/'failure-state.json').write_text(json.dumps(state(),indent=2));raise
finally:
    if process is not None and process.poll() is None:process.terminate();process.wait(timeout=15)
    log.close()
