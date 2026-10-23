#!/usr/bin/env python3
"""The chat gate's local classifier (docs/notes/chat-translation.md, "What never
reaches the model"): srp asks it, before a chat line goes to translation, how
likely the line must not go.

    POST /score  {"text": "...", "lang": "ru" | "uk" | "en" | "de" | "pl" | null}
    ->           {"score": 0.0 .. 1.0}
    GET  /health -> {"ok": true}

srp compares the score with its threshold for the language
(SRP_CHAT_MODERATION_THRESHOLDS) and fails closed: no answer in time, no line.

Models: int8 ONNX text classifiers, one folder each (model.onnx, config.json,
and tokenizer.json or, for an XLM-R vocabulary, sentencepiece.bpe.model), made
by export.py or train.py. models.json says which model
serves which language and which of its labels mean "must not go":

    {"models": {"tiny": {"dir": "rubert-tiny-toxicity", "labels": ["insult", ...]}},
     "langs": {"ru": "tiny", "uk": "tiny"},
     "default": "tiny"}

Nothing is kept and nothing is logged but counts. Python 3.10+, onnxruntime,
numpy, sentencepiece or tokenizers (requirements.txt). Run it on a host of ours, bound to an
address only srp reaches; never expose it.
"""

import argparse
import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import numpy as np
import onnxruntime as ort


MAX_TOKENS = 128
MAX_BODY = 4096


def tokenizer(folder):
    """The text to ids of the model in `folder`. An XLM-R vocabulary (e5,
    MiniLM) is read with sentencepiece when `sentencepiece.bpe.model` is there:
    the same ids as tokenizer.json, in about 60 MB instead of about 280 MB."""
    model = os.path.join(folder, "sentencepiece.bpe.model")
    if os.path.isfile(model):
        import sentencepiece

        pieces = sentencepiece.SentencePieceProcessor(model_file=model)

        def encode(text):
            # XLM-R: <s>=0, <pad>=1, </s>=2, <unk>=3; a piece's id is one more.
            body = [3 if piece == 0 else piece + 1 for piece in pieces.encode(text)]
            return [0] + body[: MAX_TOKENS - 2] + [2]

        return encode
    from tokenizers import Tokenizer

    fast = Tokenizer.from_file(os.path.join(folder, "tokenizer.json"))
    fast.enable_truncation(MAX_TOKENS)
    return lambda text: fast.encode(text).ids


class Model:
    def __init__(self, folder, labels):
        options = ort.SessionOptions()
        options.intra_op_num_threads = int(os.environ.get("MODERATION_THREADS", "1"))
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(
            os.path.join(folder, "model.onnx"), options, providers=["CPUExecutionProvider"]
        )
        self.tokenizer = tokenizer(folder)
        with open(os.path.join(folder, "config.json"), encoding="utf-8") as file:
            config = json.load(file)
        id2label = {int(index): name for index, name in config["id2label"].items()}
        unknown = [label for label in labels if label not in id2label.values()]
        if unknown:
            raise SystemExit(f"{folder}: no label {unknown}; it has {sorted(id2label.values())}")
        self.bad = [index for index, name in id2label.items() if name in labels]
        self.multi = config.get("problem_type") == "multi_label_classification"
        self.inputs = {feed.name for feed in self.session.get_inputs()}

    def score(self, text):
        ids = self.tokenizer(text)
        feeds = {
            "input_ids": np.array([ids], dtype=np.int64),
            "attention_mask": np.ones((1, len(ids)), dtype=np.int64),
        }
        if "token_type_ids" in self.inputs:
            feeds["token_type_ids"] = np.zeros((1, len(ids)), dtype=np.int64)
        logits = self.session.run(None, {k: v for k, v in feeds.items() if k in self.inputs})[0][0]
        if self.multi:
            probabilities = 1.0 / (1.0 + np.exp(-logits))
            return float(max(probabilities[index] for index in self.bad))
        exp = np.exp(logits - logits.max())
        probabilities = exp / exp.sum()
        return float(sum(probabilities[index] for index in self.bad))


class Gate:
    def __init__(self, root, settings):
        self.models = {
            name: Model(os.path.join(root, model["dir"]), model["labels"])
            for name, model in settings["models"].items()
        }
        self.langs = settings.get("langs", {})
        self.default = settings["default"]
        self.lock = threading.Lock()
        self.counts = {"scored": 0, "errors": 0}

    def score(self, text, lang):
        model = self.models[self.langs.get(lang, self.default)]
        return model.score(text)


def handler(gate):
    class Handler(BaseHTTPRequestHandler):
        def reply(self, status, body):
            data = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path == "/health":
                self.reply(200, {"ok": True, **gate.counts})
            else:
                self.reply(404, {"error": "not found"})

        def do_POST(self):
            if self.path != "/score":
                return self.reply(404, {"error": "not found"})
            length = int(self.headers.get("content-length", "0"))
            if length <= 0 or length > MAX_BODY:
                return self.reply(400, {"error": "a body of 1 to 4096 bytes"})
            try:
                request = json.loads(self.rfile.read(length))
                text = request["text"]
                lang = request.get("lang")
                if not isinstance(text, str):
                    raise ValueError("text")
                score = gate.score(text, lang)
            except Exception as error:  # an answer srp reads as "no"
                with gate.lock:
                    gate.counts["errors"] += 1
                return self.reply(400, {"error": type(error).__name__})
            with gate.lock:
                gate.counts["scored"] += 1
            self.reply(200, {"score": score})

        def log_message(self, *_):
            # No text, no address in any log.
            pass

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--models", required=True, help="the folder holding models.json")
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8099)
    arguments = parser.parse_args()
    with open(os.path.join(arguments.models, "models.json"), encoding="utf-8") as file:
        settings = json.load(file)
    gate = Gate(arguments.models, settings)
    server = ThreadingHTTPServer((arguments.bind, arguments.port), handler(gate))
    print(f"chat gate on {arguments.bind}:{arguments.port}", file=sys.stderr, flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
