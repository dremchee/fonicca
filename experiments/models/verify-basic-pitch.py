"""Compare native diagnostic exports to Spotify's pinned reference package.

Install basic-pitch reference dependencies in a separate Python environment.
Expose the pinned Spotify source package on PYTHONPATH, then pass the diagnostic
output directory and the pinned nmp.onnx path. No audio leaves this computer.
"""
import argparse, json, pathlib, numpy as np, onnxruntime as ort
from scipy.io import wavfile
from basic_pitch.note_creation import output_to_notes_polyphonic, model_frames_to_time
import pretty_midi
parser=argparse.ArgumentParser()
parser.add_argument('output', type=pathlib.Path)
parser.add_argument('model', type=pathlib.Path)
args=parser.parse_args()
root=args.output
rate,audio=wavfile.read(root/'input.wav')
assert rate==22050
s=ort.InferenceSession(str(args.model), providers=['CPUExecutionProvider'])
padded=np.pad(audio,(3840,0)); frames=[]; onsets=[]
for start in range(0,len(padded),36164):
    window=np.pad(padded[start:start+43844],(0,max(0,start+43844-len(padded))))
    f,o=s.run(['StatefulPartitionedCall:1','StatefulPartitionedCall:2'],{'serving_default_input_2:0':window.reshape(1,43844,1)})
    frames.append(f[0,15:-15]); onsets.append(o[0,15:-15])
n=len(audio)*142//36164
frames=np.concatenate(frames)[:n]; onsets=np.concatenate(onsets)[:n]
native=json.loads((root/'predictions.json').read_text())
errors={key:float(np.max(np.abs(val-np.array(native[key]).reshape(n,88)))) for key,val in [('frames',frames),('onsets',onsets)]}
assert max(errors.values())<1e-5, errors
# Use exact native activations to isolate note decoding from runtime numerical differences.
f=np.array(native['frames'],dtype=np.float32).reshape(n,88); o=np.array(native['onsets'],dtype=np.float32).reshape(n,88)
events=output_to_notes_polyphonic(f,o,.5,.3,11,True,None,None)
times=model_frames_to_time(n+1)
expected=sorted([(float(times[a]),min(float(times[b]),len(audio)/rate),int(p),int(np.clip(round(127*v),1,127))) for a,b,p,v in events])
actual=sorted([(x['start'],x['end'],x['pitch'],x['velocity']) for x in json.loads((root/'notes.json').read_text())])
assert len(expected)==len(actual),(len(expected),len(actual))
for e,a in zip(expected,actual):
    assert e[2:]==a[2:] and np.max(np.abs(np.array(e[:2])-a[:2]))<1e-9,(e,a)
midi=pretty_midi.PrettyMIDI(str(root/'notes.mid'))
readback=sorted([(x.start,x.end,x.pitch,x.velocity) for inst in midi.instruments for x in inst.notes])
assert len(readback)==len(actual)
for e,a in zip(actual,readback):
    assert e[2:]==a[2:] and np.max(np.abs(np.array(e[:2])-a[:2]))<=1/960,(e,a)
print(json.dumps({'notes':len(actual),'activationMaxAbsError':errors,'spotifyNoteDecoderMatches':True,'midiReadbackMatches':True},indent=2))
