"""Independent NumPy/ORT reference for Fonicca's standard HTDemucs framing.

Whole-track buffers deliberately differ from the native streaming overlap-add.
Inputs: TRACK/reference/{mix,drums,bass,other,vocals}.wav at 44.1 kHz.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time

import numpy as np
import onnxruntime as ort
import soundfile as sf

parser = argparse.ArgumentParser()
parser.add_argument('--model', type=Path, required=True)
parser.add_argument('--inputs', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
assert hashlib.sha256(args.model.read_bytes()).hexdigest() == '68d0bf16428ef66e692cdff8a9ccf28f1ef3f69440d57e58605a4cc55fcc5e74'
length = 343980
step = length * 3 // 4
window = np.minimum(np.arange(1, length + 1), np.arange(length, 0, -1)).astype(np.float32)
window /= length // 2
options = ort.SessionOptions()
options.intra_op_num_threads = 4
options.enable_cpu_mem_arena = False
options.add_session_config_entry('session.intra_op.allow_spinning', '0')
session = ort.InferenceSession(str(args.model), options, providers=['CPUExecutionProvider'])
report = []
for source in sorted(args.inputs.glob('*/reference/mix.wav')):
    x, rate = sf.read(source, dtype='float32', always_2d=True)
    assert rate == 44100 and x.shape[1] == 2
    n = len(x)
    mono = x.astype(np.float64).mean(axis=1)
    mean = mono.mean()
    scale = max(mono.std(ddof=1), 1e-8)
    normalized = ((x.T.astype(np.float64) - mean) / scale).astype(np.float32)
    result = np.zeros((4, 2, n), np.float32)
    weights = np.zeros(n, np.float32)
    started = time.perf_counter()
    for offset in range(0, n, step):
        count = min(length, n - offset)
        pad = (length - count) // 2
        left = offset - pad
        begin, end = max(0, left), min(n, left + length)
        block = np.zeros((1, 2, length), np.float32)
        block[0, :, begin-left:end-left] = normalized[:, begin:end]
        prediction = session.run(['stems'], {'mix': block})[0]
        assert prediction.shape == (1, 4, 2, length) and np.isfinite(prediction).all()
        result[:, :, offset:offset+count] += prediction[0, :, :, pad:pad+count] * window[:count]
        weights[offset:offset+count] += window[:count]
    elapsed = time.perf_counter() - started
    assert (weights > 0).all()
    result = (result.astype(np.float64) / weights * scale).astype(np.float32)
    target = args.output / source.parent.parent.name
    target.mkdir(parents=True, exist_ok=True)
    row = {'track': target.name, 'audio_s': n/rate, 'inference_s': elapsed, 'si_sdr_db': {}}
    for stem, wave in zip(['drums', 'bass', 'other', 'vocals'], result):
        sf.write(target / (stem + '.wav'), wave.T, rate, subtype='FLOAT')
        truth, _ = sf.read(source.parent / (stem + '.wav'), dtype='float64', always_2d=True)
        r, e = truth.ravel(), wave.T.astype(np.float64).ravel()
        r, e = r-r.mean(), e-e.mean()
        projection = r * np.dot(e, r) / max(np.dot(r, r), 1e-20)
        row['si_sdr_db'][stem] = float(10*np.log10(max(np.dot(projection, projection), 1e-20) / max(np.sum((e-projection)**2), 1e-20)))
    report.append(row)
    (args.output / 'results.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(row), flush=True)
