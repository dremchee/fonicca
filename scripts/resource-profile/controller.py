"""Private native host controller. Only generated fixtures and this child are touched."""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sqlite3
import subprocess
import sys
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

root = Path(sys.argv[1]).resolve()
if not root.name.startswith('fonicca-resources-'):
    raise SystemExit('Refusing a non-fixture directory')
config = json.loads((root / 'controller.json').read_text())
reports = []
done = threading.Event()
seeded = False
host = None


def locked():
    if sys.platform != 'darwin':
        return False
    text = subprocess.check_output(['ioreg', '-n', 'Root', '-d1'], text=True)
    match = re.search(r'"IOConsoleLocked" = (Yes|No)', text)
    if not match:
        raise RuntimeError('Cannot verify desktop state')
    return match[1] == 'Yes'


def processes():
    if sys.platform == 'win32':
        text = subprocess.check_output(['powershell', '-NoProfile', '-Command',
            'Get-Process | Select-Object Id,ProcessName,WorkingSet64 | ConvertTo-Json -Compress'], text=True)
        return [{'pid': p['Id'], 'parent': None, 'rssBytes': p['WorkingSet64'], 'name': p['ProcessName']}
                for p in json.loads(text)]
    rows = []
    for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid=,rss=,comm='], text=True).splitlines():
        parts = line.strip().split(None, 3)
        if len(parts) == 4:
            rows.append({'pid': int(parts[0]), 'parent': int(parts[1]), 'rssBytes': int(parts[2]) * 1024, 'name': parts[3]})
    return rows


def webkit(row):
    return 'com.apple.WebKit.' in row['name']


def seed(info):
    global seeded
    if Path(info['dbPath']).resolve() != root / 'data/library.db' or Path(info['modelsDir']).resolve() != root / 'data/models':
        raise RuntimeError('Unsafe app directories')
    if seeded:
        return {'seeded': True}
    connection = sqlite3.connect(root / 'data/library.db', timeout=30)
    def natural(a, b):
        def key(s):
            return [(1, int(p), len(p)) if p.isdigit() else (0, p.lower()) for p in re.split(r'(\d+)', s)]
        return (key(a) > key(b)) - (key(a) < key(b))
    connection.create_collation('FONICCA_NATURAL', natural)
    if connection.execute('select count(*) from samples').fetchone()[0] or connection.execute('select count(*) from sources').fetchone()[0]:
        raise RuntimeError('Fixture database must start empty')
    connection.execute('insert into sources(id,path,added_at) values(1,?,?)', (str(root / 'audio'), int(time.time())))
    rows = []
    for i in range(1, config['rows'] + 1):
        seconds = (30 if i == 1 else config['seconds']) if i <= 2 else 1
        path = root / 'audio' / (('short.wav' if i == 1 else 'long.wav') if i <= 2 else f'metadata/{i:08d}.wav')
        size = path.stat().st_size if i <= 2 else 100
        rows.append((i, str(path), f'{i:08d} synthetic', 'wav', size, 1, float(seconds), 44100, 2, 32, i,
                     1, 'long' if seconds > 1 else 'oneshot', 999))
    connection.executemany('insert into samples(id,source_id,path,name,ext,size,mtime,duration,sample_rate,channels,bit_depth,fingerprint,analysis_version,kind,category_version) values(?,1,?,?,?,?,?,?,?,?,?,?,?,?,?)', rows)
    connection.commit()
    connection.close()
    seeded = True
    return {'seeded': True}


class Handler(SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        with (root / 'http.log').open('a') as log:
            log.write(fmt % args + '\n')

    def reply(self, code, value):
        data = json.dumps(value).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', self.headers.get('Origin', config['controllerUrl']))
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_OPTIONS(self):
        if self.headers.get('Origin') not in ['tauri://localhost', 'http://tauri.localhost', 'https://tauri.localhost', config['controllerUrl']]:
            return self.reply(403, {'error': 'Unexpected origin'})
        self.send_response(204)
        self.send_header('Access-Control-Allow-Origin', self.headers['Origin'])
        self.send_header('Access-Control-Allow-Methods', 'POST')
        self.send_header('Access-Control-Allow-Headers', 'content-type,x-fixture-token')
        self.end_headers()

    def do_POST(self):
        try:
            if self.headers.get('x-fixture-token') != config['token']:
                raise RuntimeError('Wrong fixture token')
            length = int(self.headers.get('Content-Length', '0'))
            if length > 1024 * 1024:
                raise RuntimeError('Oversized probe report')
            value = json.loads(self.rfile.read(length))
            if self.path == '/seed':
                return self.reply(200, seed(value['info']))
            if self.path == '/checkpoint':
                if locked() or value.get('visibility') != 'visible':
                    raise RuntimeError('Foreground scene is not visible/unlocked')
                current = processes()
                cohort = [p for p in current if p['pid'] == host.pid or (sys.platform == 'darwin' and webkit(p) and p['pid'] not in prior)]
                footprint = None
                if sys.platform == 'darwin' and not config['smoke']:
                    if not any('WebContent' in p['name'] for p in cohort):
                        raise RuntimeError('No new WebContent process attributable to this host')
                    output = root / f'{len(reports):02d}-footprint.json'
                    command = ['footprint']
                    for p in cohort:
                        command += ['-p', str(p['pid'])]
                    subprocess.run(command + ['-j', str(output), '--noCategories', '-f', 'bytes'], check=True, capture_output=True, timeout=25)
                    footprint = json.loads(output.read_text())
                report = {**value, 'screenLocked': False, 'capturedAt': time.time(), 'processes': cohort, 'footprint': footprint}
                reports.append(report)
                (root / 'checkpoints.json').write_text(json.dumps(reports, indent=2))
                print(json.dumps({'checkpoint': value['stage'], 'window': value['window'], 'callbacks': value['callbacks'], 'resources': value['resources'], 'processes': [(p['pid'], p['rssBytes']) for p in cohort]}), flush=True)
                return self.reply(200, {'captured': True})
            if self.path == '/finished':
                (root / 'finished.json').write_text(json.dumps(value, indent=2))
                done.set()
                return self.reply(200, {'finished': True})
            raise RuntimeError('Unknown route')
        except Exception as error:
            print('Controller: ' + str(error), flush=True)
            self.reply(500, {'error': str(error)})


if locked():
    raise SystemExit('Desktop locked: foreground profiling cannot proceed')
prior = {p['pid'] for p in processes() if webkit(p)}
server = ThreadingHTTPServer(('127.0.0.1', config['port']), partial(Handler, directory=str(root / 'frontend/dist')))
threading.Thread(target=server.serve_forever, daemon=True).start()
binary = Path(config['binary'])
binary_data = binary.read_bytes()
# Tauri may embed the private path as a JSON string (escaped separators on Windows).
# The runtime app_info check below still requires the exact private directories.
private_paths = [config['data'].encode(), json.dumps(config['data'])[1:-1].encode()]
if config['identifier'].encode() not in binary_data or not any(path in binary_data for path in private_paths):
    raise SystemExit('Refusing to launch a binary without the isolated identifier and private data root')
finished = None
failure = None
def interrupted(*_):
    raise RuntimeError('Controller interrupted')
signal.signal(signal.SIGTERM, interrupted)
with (root / 'native.log').open('w') as log:
    launch_env = os.environ.copy()
    if sys.platform.startswith('linux'):
        # AppRun/GStreamer must not replace the user's shared plugin registry.
        launch_env['GST_REGISTRY_1_0'] = str(root / 'gstreamer-registry.bin')
        launch_env.setdefault('GST_DEBUG', '2')
    host = subprocess.Popen([config.get('launch', str(binary))], cwd=binary.parent, stdout=log, stderr=subprocess.STDOUT, env=launch_env)
    try:
        deadline = time.monotonic() + (240 if config['smoke'] else 600)
        while not done.wait(.5):
            if host.poll() is not None:
                raise RuntimeError('Native host exited: ' + str(host.returncode))
            if time.monotonic() > deadline:
                raise RuntimeError('Profile timed out')
        finished = json.loads((root / 'finished.json').read_text())
        if not finished.get('ok'):
            raise RuntimeError('Probe: ' + finished.get('error', 'failed'))
    except Exception as error:
        failure = str(error)
    finally:
        host.terminate()
        try:
            host.wait(timeout=8)
        except subprocess.TimeoutExpired:
            host.kill()
            host.wait()
        server.shutdown()
cohort = {p['pid'] for stage in reports for p in stage['processes']}
remaining = cohort
for _ in range(60):
    remaining = cohort & {p['pid'] for p in processes()}
    if not remaining:
        break
    time.sleep(.5)
result = {'schema': 1, 'status': 'failed' if failure else 'measured', 'error': failure,
          'platform': sys.platform, 'bundle': config.get('bundle'), 'nativeProfile': 'release', 'mode': 'smoke' if config['smoke'] else 'memory',
          'privateBrowsing': config.get('privateBrowsing'),
          'identifier': config['identifier'], 'nativeHostPid': host.pid,
          'sourceRevision': config['sourceRevision'], 'binarySha256': hashlib.sha256(binary_data).hexdigest(),
          'beforeWebKitPids': sorted(prior), 'cohortPids': sorted(cohort), 'cohortRemainingAfterHostExit': sorted(remaining),
          'checkpoints': reports, 'finished': finished,
          'limits': ['Generated private audio/SQLite and production UI/native IPC; probe and renderer counters are injected only in a copied checkout.',
                     'Native RSS and per-process WebKit RSS/footprint are distinct observations, not disjoint physical totals.',
                     'No forced JavaScript GC; residual RSS alone is not proof of a leak.',
                     'Non-macOS smoke mode checks the scene but does not attribute WebView child memory.',
                     'Cohort attribution is accepted only if all candidate processes disappear after this host stops.']}
(root / 'result.json').write_text(json.dumps(result, indent=2))
print(json.dumps({'complete': not failure, 'remainingPids': sorted(remaining), 'result': str(root / 'result.json')}), flush=True)
if failure or remaining:
    raise SystemExit(1)
