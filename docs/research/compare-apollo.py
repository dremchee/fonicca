"""Evaluate official Apollo on existing SCNet XL stems without modifying inputs.

Requires the pinned Apollo checkout/checkpoint described in apollo-comparison.md.
Run with --help for paths. Outputs FLOAT WAVs and measured JSON, no gain matching.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import sys
import time

os.environ.setdefault("PYTORCH_ENABLE_MPS_FALLBACK", "1")
import numpy as np
import soundfile as sf
import torch


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def si_sdr(reference, estimate):
    reference = reference.astype(np.float64).ravel()
    estimate = estimate.astype(np.float64).ravel()
    reference -= reference.mean()
    estimate -= estimate.mean()
    target = reference * np.dot(estimate, reference) / (np.dot(reference, reference) + 1e-20)
    return float(10 * np.log10((np.dot(target, target) + 1e-20) /
                              (np.sum((estimate - target) ** 2) + 1e-20)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apollo", type=Path, required=True, help="Cache with source/ and pytorch_model.bin")
    parser.add_argument("--stems", type=Path, required=True, help="SCNet XL outputs/<track>/<stem>.wav")
    parser.add_argument("--references", type=Path, required=True, help="<track>/reference/<stem>.wav")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--device", choices=["mps", "cpu"], default="mps")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    checkpoint = args.apollo / "pytorch_model.bin"
    manifest = json.loads((args.apollo / "manifest.json").read_text())
    assert checkpoint.stat().st_size == 66541845
    assert digest(checkpoint) == "99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76"
    assert args.output.resolve() != args.stems.resolve()
    sys.path.insert(0, str(args.apollo / "source"))
    from look2hear.models import Apollo

    torch.set_num_threads(4)
    torch.set_num_interop_threads(1)
    device = torch.device(args.device)
    if args.device == "mps":
        assert torch.backends.mps.is_available()
    # Official checkpoint includes OmegaConf metadata. Verify the pinned hash
    # before using the upstream loader's deserialization format.
    state = torch.load(checkpoint, map_location="cpu", weights_only=False)
    assert state["model_name"] == "Apollo"
    model = Apollo(sr=44100, win=20, feature_dim=256, layer=6)
    model.load_state_dict(state["state_dict"], strict=True)
    del state
    model = model.to(device).eval()
    report = {"manifest": manifest, "device": args.device, "torch": torch.__version__,
              "mode": "full file, float32, no external normalization or gain matching",
              "parameters": sum(p.numel() for p in model.parameters()), "rows": []}
    for folder in sorted(args.stems.iterdir()):
        if not folder.is_dir():
            continue
        for stem in ["vocals", "drums", "bass", "other"]:
            source = folder / (stem + ".wav")
            source_hash = digest(source)
            audio, rate = sf.read(source, dtype="float32", always_2d=True)
            reference, ref_rate = sf.read(args.references / folder.name / "reference" / source.name,
                                           dtype="float32", always_2d=True)
            assert rate == ref_rate == 44100 and audio.shape == reference.shape
            assert audio.shape[1] == 2 and np.isfinite(audio).all()
            tensor = torch.from_numpy(np.ascontiguousarray(audio.T)).unsqueeze(0)
            began = time.perf_counter()
            with torch.inference_mode():
                result = model(tensor.to(device)).cpu().numpy()[0].T.copy()
            elapsed = time.perf_counter() - began
            assert result.shape == audio.shape and np.isfinite(result).all()
            destination = args.output / folder.name / source.name
            destination.parent.mkdir(parents=True, exist_ok=True)
            assert destination.resolve() != source.resolve()
            sf.write(destination, result, rate, subtype="FLOAT")
            assert digest(source) == source_hash
            reloaded, saved_rate = sf.read(destination, dtype="float32", always_2d=True)
            assert saved_rate == rate and np.array_equal(result, reloaded)
            row = {"track": folder.name, "stem": stem, "frames": len(audio),
                   "audio_s": len(audio) / rate, "inference_s": elapsed,
                   "source_sha256": source_hash, "output_sha256": digest(destination),
                   "before_si_sdr_db": si_sdr(reference, audio),
                   "after_si_sdr_db": si_sdr(reference, result),
                   "before_peak": float(np.max(np.abs(audio))),
                   "after_peak": float(np.max(np.abs(result))),
                   "rms_change_db": float(20 * np.log10((np.sqrt(np.mean(result.astype(np.float64)**2)) + 1e-20) /
                                                        (np.sqrt(np.mean(audio.astype(np.float64)**2)) + 1e-20))),
                   "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}
            if args.device == "mps":
                row["mps_driver_allocated_bytes"] = torch.mps.driver_allocated_memory()
                torch.mps.empty_cache()
            report["rows"].append(row)
            (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(row), flush=True)
    assert len(report["rows"]) == 12, "Expected all four stems from the three comparison tracks"


if __name__ == "__main__":
    main()
