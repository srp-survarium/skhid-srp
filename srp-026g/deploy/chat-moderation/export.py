#!/usr/bin/env python3
"""Export a Hugging Face text classifier to the folder server.py serves:
model.onnx (int8, dynamic quantization), tokenizer.json and config.json.

    python export.py <model id or folder> <out folder>

Needs torch (CPU), transformers, onnx and onnxruntime (requirements-train.txt).
Run it on a PC, not on the game host; copy the out folder to the gate's host.
"""

import os
import shutil
import sys

import torch
from onnxruntime.quantization import QuantType, quantize_dynamic
from transformers import AutoModelForSequenceClassification, AutoTokenizer


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    source, out = sys.argv[1], sys.argv[2]
    if os.path.exists(out):
        raise SystemExit(f"{out} is there already")
    os.makedirs(out)
    tokenizer = AutoTokenizer.from_pretrained(source)
    if not tokenizer.is_fast:
        raise SystemExit(f"{source} has no fast tokenizer (tokenizer.json), which server.py reads")
    model = AutoModelForSequenceClassification.from_pretrained(source).eval()
    sample = tokenizer("пример строки чата", return_tensors="pt")
    names = [name for name in ("input_ids", "attention_mask", "token_type_ids") if name in sample]
    dynamic = {name: {0: "batch", 1: "tokens"} for name in names}
    dynamic["logits"] = {0: "batch"}
    plain = os.path.join(out, "model.fp32.onnx")
    torch.onnx.export(
        model,
        tuple(sample[name] for name in names),
        plain,
        input_names=names,
        output_names=["logits"],
        dynamic_axes=dynamic,
        opset_version=17,
    )
    quantize_dynamic(plain, os.path.join(out, "model.onnx"), weight_type=QuantType.QInt8)
    os.remove(plain)
    tokenizer.backend_tokenizer.save(os.path.join(out, "tokenizer.json"))
    model.config.to_json_file(os.path.join(out, "config.json"))
    size = os.path.getsize(os.path.join(out, "model.onnx")) / 1e6
    print(f"{out}: model.onnx {size:.0f} MB; labels {model.config.id2label}")
    for extra in ("LICENSE", "README.md"):
        path = os.path.join(source, extra)
        if os.path.isfile(path):
            shutil.copy(path, out)


if __name__ == "__main__":
    main()
