#!/usr/bin/env python3
"""Capture the four owner-reported views using resource-porter's camera protocol.

Start either reference client with the existing resource-porter workflow first.
This script sends camera requests only; it does not launch a game or a server.
"""
import argparse
import importlib.util
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--porter-tree', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--buffers', action='store_true', help='request diagnostic GPU buffers')
    args = parser.parse_args()
    source = args.porter_tree / 'tools/render-compare/capture.py'
    spec = importlib.util.spec_from_file_location('porter_capture', source)
    capture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(capture)
    poses = json.loads(Path(__file__).with_name('poses.json').read_text())
    for pose in poses:
        pose['buffers'] = args.buffers
    args.out.mkdir(parents=True, exist_ok=False)
    capture.capture(args.control, [dict(poses[0], name='warmup')], args.out / 'warmup', timeout=120)
    capture.capture(args.control, poses, args.out / 'poses', timeout=120)


if __name__ == '__main__':
    main()
