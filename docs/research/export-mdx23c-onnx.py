"""Developer-only reproducible export. Requires torch, onnx, onnxruntime, numpy,
PyYAML. Download the checkpoint URL in vocal-models-NOTICE.txt into --cache as
mdx23c.ckpt first. Downloads pinned MIT source, never runs in the application.
The 319 learned initializers reference the unchanged official checkpoint.
Output is --cache/mdx23c.onnx; copy the verified graph into model-packs/v1.
"""
from pathlib import Path
import torch,onnx,numpy as np,yaml,sys,hashlib,zipfile,struct,json,types
from onnx import numpy_helper,external_data_helper
import argparse,urllib.request
parser=argparse.ArgumentParser(description="Export MDX23C spectral graph; reference original checkpoint byte ranges, no learned weights bundled.")
parser.add_argument('--cache', type=Path, required=True)
args=parser.parse_args()
R=args.cache.resolve();R.mkdir(parents=True,exist_ok=True);W=R/'mdx23c.ckpt'
revision='43d939e7671d8ff6cf1922f98c2f2e4b56908e47'
(R/'source').mkdir(exist_ok=True)
for source in ['models_without_stft/mdx23c_tfc_tdf_v3_no_stft.py','configs/config_vocals_mdx23c.yaml','LICENSE']:
    target=R/'source'/source.split('/')[-1]
    target.write_bytes(urllib.request.urlopen(f'https://raw.githubusercontent.com/ZFTurbo/MSS_ONNX_TensorRT/{revision}/{source}').read())
(R/'source/revision.txt').write_text(revision)
assert W.stat().st_size==448101203
assert hashlib.sha256(W.read_bytes()).hexdigest()=='49d51472769e34a2501cd1da782346a3212555c3a5619fc2c53507445528d816'
utils=types.ModuleType('utils');utils.prefer_target_instrument=lambda c: [c.training.target_instrument] if c.training.target_instrument else c.training.instruments
sys.modules['utils']=utils
sys.path.insert(0,str(R/'source'))
from mdx23c_tfc_tdf_v3_no_stft import TFC_TDF_net
def ns(d):return types.SimpleNamespace(**{k:ns(v) if isinstance(v,dict) else v for k,v in d.items()})
config=ns(yaml.load((R/'source/config_vocals_mdx23c.yaml').read_text(),Loader=yaml.FullLoader))
torch.set_num_threads(4)
m=TFC_TDF_net(config).eval();state=torch.load(W,map_location='cpu',weights_only=True)
print('state keys',list(state)[:4],flush=True)
m.load_state_dict(state,strict=True);del state
x=torch.randn(1,4,4096,256)*.05
with torch.inference_mode():
 y=m(x).numpy();np.savez(R/'mdx-reference.npz',input=x.numpy(),output=y)
 torch.onnx.export(m,x,str(R/'mdx-full.onnx'),opset_version=17,dynamo=False,do_constant_folding=False,input_names=['mix_spec'],output_names=['stems_spec'])
print('exported',flush=True)
model=onnx.load(R/'mdx-full.onnx');data=W.read_bytes();entries={}
with zipfile.ZipFile(W) as z:
 for e in z.infolist():
  if '/data/' not in e.filename:continue
  assert e.compress_type==0
  h=struct.unpack_from('<4s5H3I2H',data,e.header_offset);offset=e.header_offset+30+h[-2]+h[-1]
  b=z.read(e.filename);assert data[offset:offset+len(b)]==b
  entries[hashlib.sha256(b).digest()]=(offset,len(b))
manifest=[];embedded=0
for t in model.graph.initializer:
 b=numpy_helper.to_array(t).tobytes();match=entries.get(hashlib.sha256(b).digest())
 if match:
  offset,length=match
  if not t.HasField('raw_data'):t.raw_data=b
  external_data_helper.set_external_data(t,location='mdx23c.ckpt',offset=offset,length=length)
  t.ClearField('raw_data');manifest.append({'name':t.name,'offset':offset,'length':length})
 else:
  embedded+=len(b);assert len(b)<16000,(t.name,len(b))
onnx.save(model,R/'mdx23c.onnx')
info={'source_revision':(R/'source/revision.txt').read_text(),'checkpoint_sha256':hashlib.sha256(data).hexdigest(),'graph_sha256':hashlib.sha256((R/'mdx23c.onnx').read_bytes()).hexdigest(),'graph_bytes':(R/'mdx23c.onnx').stat().st_size,'embedded_bytes':embedded,'tensors':manifest}
(R/'mdx-manifest.json').write_text(json.dumps(info,indent=2))
print({k:v for k,v in info.items() if k!='tensors'},'external tensors',len(manifest),flush=True)
import onnxruntime as ort,time
o=ort.SessionOptions();o.intra_op_num_threads=4;o.enable_cpu_mem_arena=False
s=ort.InferenceSession(str(R/'mdx23c.onnx'),o,providers=['CPUExecutionProvider']);t=__import__('time').perf_counter();v=s.run(None,{'mix_spec':x.numpy()})[0]
print('parity SNR',10*np.log10(np.sum(y*y)/max(np.sum((y-v)**2),1e-30)),'time',__import__('time').perf_counter()-t,flush=True)
