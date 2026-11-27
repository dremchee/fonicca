"""Reproducible Apollo FP32 export. Requires torch, onnx, numpy and upstream
Apollo dependencies. The graph contains no learned tensor data: all 644
initializers reference byte ranges in the official, hash-verified checkpoint.
Run this developer tool against a checkout of Apollo revision
 e84bcacc59d5455f05d86a5c97dd4aeb3c14dbb6 in <cache>/source/.
"""
from pathlib import Path
import sys,json,hashlib,zipfile,struct,time,argparse,tempfile,shutil
import numpy as np,torch,onnx
from onnx import numpy_helper, external_data_helper
parser=argparse.ArgumentParser(description="Export Apollo's spectral graph with external tensors in the pinned official checkpoint.")
parser.add_argument('--apollo',type=Path,required=True,help='Cache with source/ and official pytorch_model.bin')
args=parser.parse_args()
ROOT=args.apollo.resolve()
WEIGHT=ROOT/'pytorch_model.bin';DATA=WEIGHT.read_bytes()
assert hashlib.sha256(DATA).hexdigest()=='99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76'
sys.path.insert(0,str(ROOT/'source'))
from look2hear.models import Apollo
torch.set_num_threads(4)
m=Apollo(sr=44100,win=20,feature_dim=256,layer=6).eval()
state=torch.load(WEIGHT,map_location='cpu',weights_only=False);m.load_state_dict(state['state_dict'],strict=True);del state
class Spectral(torch.nn.Module):
 def __init__(self,model):super().__init__();self.model=model
 def forward(self,real,imag):
  offset=0;bands=[]
  for i,width in enumerate(self.model.band_width):
   r=real[:,offset:offset+width];j=imag[:,offset:offset+width]
   power=((r*r+j*j).sum(1,keepdim=True)+self.model.eps).sqrt()
   bands.append(self.model.BN[i](torch.cat([r/power,j/power,power.log()],dim=1)))
   offset+=width
  x=self.model.net(torch.stack(bands,1));outs=[]
  for i,width in enumerate(self.model.band_width):
   outs.append(self.model.output[i](x[:,i]).reshape(real.shape[0],2,width,-1))
  y=torch.cat(outs,dim=2)
  return y[:,0],y[:,1]
wrapper=Spectral(m).eval()
r=torch.randn(1,442,101);i=torch.randn_like(r)
raw=ROOT/'apollo-full.onnx'
def matrix_transpose(g,x):
 rank=torch.onnx.symbolic_helper._get_tensor_rank(x)
 assert rank is not None
 axes=list(range(rank));axes[-1],axes[-2]=axes[-2],axes[-1]
 return g.op('Transpose',x,perm_i=axes)
torch.onnx.register_custom_op_symbolic('aten::mT',matrix_transpose,18)
with torch.inference_mode():
 torch.onnx.export(wrapper,(r,i),str(raw),input_names=['spec_real','spec_imag'],output_names=['out_real','out_imag'],opset_version=18,dynamo=False,do_constant_folding=False,dynamic_axes={k:{2:'frames'} for k in ['spec_real','spec_imag','out_real','out_imag']})
model=onnx.load(str(raw));zip_entries={}
with zipfile.ZipFile(WEIGHT) as z:
 for entry in z.infolist():
  if '/data/' not in entry.filename:continue
  assert entry.compress_type==0
  header=struct.unpack_from('<4s5H3I2H',DATA,entry.header_offset)
  start=entry.header_offset+30+header[-2]+header[-1]
  payload=z.read(entry.filename);assert DATA[start:start+len(payload)]==payload
  zip_entries.setdefault(hashlib.sha256(payload).digest(),[]).append((start,len(payload)))
external=0;embedded=0;manifest=[]
for tensor in model.graph.initializer:
 value=numpy_helper.to_array(tensor);raw_bytes=value.tobytes()
 entries=zip_entries.get(hashlib.sha256(raw_bytes).digest(),[])
 if entries:
  offset,length=entries[0];assert length==len(raw_bytes)
  if not tensor.HasField('raw_data'):tensor.raw_data=raw_bytes
  external_data_helper.set_external_data(tensor,location='pytorch_model.bin',offset=offset,length=length)
  tensor.ClearField('raw_data');external+=length
  manifest.append({'name':tensor.name,'offset':offset,'length':length})
 else:
  embedded+=len(raw_bytes)
  assert len(raw_bytes)<16000,('unmapped weights',tensor.name,len(raw_bytes))
print('external',external,'embedded',embedded,'initializers',len(manifest),flush=True)
model.metadata_props.add(key='checkpoint_sha256',value=hashlib.sha256(DATA).hexdigest())
model.metadata_props.add(key='contract',value='Apollo FP32 spectral network; Hann 882, hop 441, centered reflection, unnormalized; mono channels independently')
graph=ROOT/'apollo.onnx';onnx.save(model,str(graph))
# ONNX's checker rejects multiply-linked external data. Use an isolated copy for
# validation while leaving the user's cached checkpoint and its aliases intact.
with tempfile.TemporaryDirectory() as checked:
 checked=Path(checked);shutil.copyfile(graph,checked/graph.name);shutil.copyfile(WEIGHT,checked/WEIGHT.name)
 onnx.checker.check_model(str(checked/graph.name))
info={'checkpoint_sha256':hashlib.sha256(DATA).hexdigest(),'graph_sha256':hashlib.sha256(graph.read_bytes()).hexdigest(),'graph_bytes':graph.stat().st_size,'external_bytes':external,'embedded_bytes':embedded,'tensors':manifest}
(ROOT/'onnx-manifest.json').write_text(json.dumps(info,indent=2))
print('GRAPH',graph.stat().st_size,info['graph_sha256'],flush=True)
