#!/usr/bin/env python3
"""Fine-tune a small multilingual encoder on our own yes/no labels: may a chat
line go to translation (docs/notes/chat-translation.md, "The gate: the real
fit"). For the owner's PC (CPU is enough for a few thousand lines; a GPU is
used when there is one). Not run yet: there is no labelled data.

    python train.py labels.tsv <out folder> [--base <model>] [--epochs 8]
    python export.py <out folder> <gate model folder>

labels.tsv, one line each, tab-separated, no header:

    lang    label   text
    ru      yes     бля, промазал
    ru      yes     ты бездарь, удали игру
    en      no      <a threat against a real person>

- lang: en, ru, uk, de or pl (the writer's game language, or the line's);
- label: yes (may go: chat, slang, swearing, trash talk, banter) or no
  (threats against a real person, slurs and hate, sexual content, a line
  almost entirely obscenity);
- text: the line as typed. Lines come from the chat sample
  (SRP_CHAT_SAMPLE_DIR: public channels, no names), labelled by hand.

Base models: intfloat/multilingual-e5-small (the default: about 118 MB int8,
the five languages; its word embeddings frozen, as benchmarked), or
cointegrated/rubert-tiny-toxicity (about 12 MB, Russian first; twice the false
"no" on swearing and trash talk). A fifth of the lines is held out: the script prints, per
language, the threshold on the "no" probability that keeps recall on "no" at
0.95 or more with the fewest false "no", for SRP_CHAT_MODERATION_THRESHOLDS.
"""

import argparse
import csv
import random

import torch
from transformers import AutoModelForSequenceClassification, AutoTokenizer

LANGS = ("en", "ru", "uk", "de", "pl")


def read(path):
    rows = []
    with open(path, encoding="utf-8", newline="") as file:
        for number, row in enumerate(csv.reader(file, delimiter="\t", quoting=csv.QUOTE_NONE), 1):
            if not row or row[0].startswith("#"):
                continue
            if len(row) != 3 or row[0] not in LANGS or row[1] not in ("yes", "no"):
                raise SystemExit(f"{path}:{number}: a row is lang, yes|no, text")
            rows.append((row[0], 1 if row[1] == "no" else 0, row[2]))
    return rows


def thresholds(scored):
    """Per language: the lowest false-"no" threshold with recall on "no" >= 0.95."""
    out = {}
    for lang in LANGS:
        rows = [(score, label) for row_lang, label, score in scored if row_lang == lang]
        nos = sum(label for _, label in rows)
        if not nos:
            continue
        best = None
        for step in range(1, 100):
            threshold = step / 100
            caught = sum(1 for score, label in rows if label and score >= threshold)
            false = sum(1 for score, label in rows if not label and score >= threshold)
            if caught / nos >= 0.95:
                best = (threshold, caught / nos, false)
        if best:
            out[lang] = best
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("labels")
    parser.add_argument("out")
    parser.add_argument("--base", default="intfloat/multilingual-e5-small")
    parser.add_argument("--tokenizer", default=None, help="when the base has none (MiniLM: xlm-roberta-base)")
    parser.add_argument("--epochs", type=int, default=8)
    parser.add_argument("--lr", type=float, default=5e-5)
    parser.add_argument(
        "--train-embeddings", action="store_true", help="also train the word embeddings"
    )
    parser.add_argument("--seed", type=int, default=1)
    arguments = parser.parse_args()
    random.seed(arguments.seed)
    torch.manual_seed(arguments.seed)
    rows = read(arguments.labels)
    random.shuffle(rows)
    held = rows[: len(rows) // 5]
    train = rows[len(rows) // 5 :]
    tokenizer_name = arguments.tokenizer or (
        "xlm-roberta-base" if "Multilingual-MiniLM" in arguments.base else arguments.base
    )
    tokenizer = AutoTokenizer.from_pretrained(tokenizer_name)
    model = AutoModelForSequenceClassification.from_pretrained(
        arguments.base, num_labels=2, id2label={0: "yes", 1: "no"}, label2id={"yes": 0, "no": 1}
    )
    device = "cuda" if torch.cuda.is_available() else "cpu"
    model.to(device)
    if not arguments.train_embeddings:
        for parameter in model.get_input_embeddings().parameters():
            parameter.requires_grad = False
    optimizer = torch.optim.AdamW(
        [parameter for parameter in model.parameters() if parameter.requires_grad], lr=arguments.lr
    )
    for epoch in range(arguments.epochs):
        model.train()
        random.shuffle(train)
        total = 0.0
        for start in range(0, len(train), 16):
            batch = train[start : start + 16]
            encoded = tokenizer(
                [text for _, _, text in batch],
                padding=True,
                truncation=True,
                max_length=128,
                return_tensors="pt",
            ).to(device)
            labels = torch.tensor([label for _, label, _ in batch], device=device)
            loss = model(**encoded, labels=labels).loss
            loss.backward()
            optimizer.step()
            optimizer.zero_grad()
            total += loss.item()
        print(f"epoch {epoch + 1}: loss {total / max(1, len(train) // 16):.4f}")
    model.eval()
    scored = []
    with torch.no_grad():
        for lang, label, text in held:
            encoded = tokenizer(text, truncation=True, max_length=128, return_tensors="pt").to(device)
            probability = torch.softmax(model(**encoded).logits, dim=-1)[0, 1].item()
            scored.append((lang, label, probability))
    for lang, (threshold, recall, false) in thresholds(scored).items():
        print(f"{lang}: threshold {threshold:.2f}, recall on no {recall:.2f}, false no {false}")
    model.save_pretrained(arguments.out)
    tokenizer.save_pretrained(arguments.out)
    print(f"saved to {arguments.out}; next: python export.py {arguments.out} <gate model folder>")


if __name__ == "__main__":
    main()
