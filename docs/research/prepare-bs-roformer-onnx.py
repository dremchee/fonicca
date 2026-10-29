"""Developer-only ONNX compatibility repair. Requires onnx. Run with --cache
containing the two pinned files from xycld/BS-RoFormer-ONNX revision below.
The upstream graph puts a 62-element shape/split INT64 tensor in external data;
ONNX Runtime cannot read it during shape inference. Embed that constant only.
No learned float weights are bundled. Output is src-tauri/resources/bs-roformer.onnx.
"""
from pathlib import Path
import argparse,hashlib,onnx
parser=argparse.ArgumentParser()
parser.add_argument('--cache',type=Path,required=True)
args=parser.parse_args();root=args.cache
# https://huggingface.co/xycld/BS-RoFormer-ONNX/tree/f020228f4d25616c8ff4146c339c33687c0be43f
source=root/'bs_roformer_ep317_sdr12.9755.onnx'
weights=root/'bs_roformer_ep317_sdr12.9755.onnx.data'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='357837834747cbf3cd680bfab33f9bfd08b99d493d8dfabc84b92e59e4fcfe73'
assert hashlib.sha256(weights.read_bytes()).hexdigest()=='3f3677f7ed3801f7415db4b4c900e6e81f4f41d5b4bc83c0bf899c353b2f9995'
model=onnx.load(source,load_external_data=False)
embedded=0
with weights.open('rb') as f:
    for tensor in model.graph.initializer:
        if tensor.data_type==onnx.TensorProto.FLOAT:continue
        data={v.key:v.value for v in tensor.external_data}
        if not data:continue
        length=int(data['length']);assert length<=4096
        f.seek(int(data.get('offset',0)));raw=f.read(length);assert len(raw)==length
        tensor.ClearField('external_data');tensor.data_location=onnx.TensorProto.DEFAULT;tensor.raw_data=raw
        embedded+=length
assert embedded==496
output=root/'bs-roformer-native.onnx';onnx.save(model,output)
assert hashlib.sha256(output.read_bytes()).hexdigest()=='320e61bbb30a3bb0cba7b88c013b02b0acef444c3341667158b1bc1611664af9'
print(output,output.stat().st_size)
