#!/usr/bin/env python3
"""Makes src/sounds.rs: the retail radio sounds skhid_voice.dll plays with a
far (radio) voice, as 16 kHz mono int16 arrays.

    python3 make_radio_sounds.py <folder> > src/sounds.rs

<folder> holds the three source files below, taken out of Survarium 0.69d0's
resources.02.db (vostok-port extract --db .../src069/resources.02.db --path
sounds/single/vo/...). Each is checked against its SHA-256; the output says
where each sound came from. Needs Python 3 with numpy, scipy and soundfile
(nix shell --impure --expr 'with import <nixpkgs> {}; python3.withPackages
(p: [p.numpy p.scipy p.soundfile])').

Retail has no separate squelch or click: the quick commands' `_radio` takes
(0.46c2 on) have the radio's static baked in, a static lead-in before the
words and a static tail after them. The squelch-in is such a lead-in, the
squelch-out such a tail, and the bed under the voice is the walkie-talkie
static of the tutorial (sounds/collections/vo/tutorial/noises/
walkietalkie_noises.sound_collection), at the level of the takes' static.
"""
import hashlib
import os
import sys

import numpy as np
import soundfile as sf
from scipy import signal

BUILD = "Survarium 0.69d0, resources.02.db"
RATE = 16000
SOURCES = {
    "squelch_in": ("sounds/single/vo/en/head_00/en_right_flank_radio_01.high.ogg",
                   "139f974f7e8e8b74f7f77fece4edbed87b1badb0371195a2113f82bf449120cf"),
    "squelch_out": ("sounds/single/vo/en/head_00/en_caution_shoot_radio_02.high.ogg",
                    "ae4ea003f2b939c4f73aff4b5dfefac76d6e4a4bbdb00efe93b87c36a5f128f6"),
    "static_bed": ("sounds/single/vo/tutorial_interference/tutorial_interference_02.high.ogg",
                   "8fa41762b18dfdd2861eb7961cdad1ec33ba093079dc363868ee06a72f84dbce"),
}
SQUELCH_IN_MS = 160        # the take's static before its words (180 ms) less the words' onset
SPEECH_DB = -28            # a 5 ms frame this loud is words
TAIL_SKIP_MS = 20          # the words' decay after their last loud frame
BED_DB = -44.0             # the takes' static between words: -42..-48 dBFS
LOOP_FADE_MS = 40


def load(folder, key):
    path, sha = SOURCES[key]
    data = open(os.path.join(folder, os.path.basename(path)), "rb").read()
    if hashlib.sha256(data).hexdigest() != sha:
        sys.exit(f"{path}: not the retail file (SHA-256 {sha})")
    x, rate = sf.read(os.path.join(folder, os.path.basename(path)), dtype="float64", always_2d=True)
    x = x.mean(axis=1)
    return signal.resample_poly(x, RATE, rate)


def frames_db(x, ms=5):
    n = RATE * ms // 1000
    return n, np.array([20 * np.log10(np.sqrt(np.mean(x[i * n:(i + 1) * n] ** 2)) + 1e-12)
                        for i in range(len(x) // n)])


def fade(x, ms, out=True):
    n = min(len(x), RATE * ms // 1000)
    ramp = np.linspace(1.0, 0.0, n) if out else np.linspace(0.0, 1.0, n)
    x = x.copy()
    if out:
        x[len(x) - n:] *= ramp
    else:
        x[:n] *= ramp
    return x


def main():
    folder = sys.argv[1]
    sounds = {}
    take = load(folder, "squelch_in")
    sounds["squelch_in"] = fade(take[:RATE * SQUELCH_IN_MS // 1000], 10)
    take = load(folder, "squelch_out")
    n, e = frames_db(take)
    last = len(e) - 1 - int(np.argmax(e[::-1] > SPEECH_DB))
    tail = take[(last + 1) * n + RATE * TAIL_SKIP_MS // 1000:]
    sounds["squelch_out"] = fade(fade(tail, 5, out=False), 30)
    bed = load(folder, "static_bed")
    # a seamless loop: its end faded over its start
    k = RATE * LOOP_FADE_MS // 1000
    ramp = np.linspace(0.0, 1.0, k)
    loop = bed[:len(bed) - k].copy()
    loop[:k] = bed[:k] * ramp + bed[len(bed) - k:] * (1 - ramp)
    loop *= 10 ** (BED_DB / 20) / np.sqrt(np.mean(loop ** 2))
    sounds["static_bed"] = loop
    out = ["// Made by make_radio_sounds.py: do not edit. The retail radio sounds of",
           f"// {BUILD}, 16 kHz mono (resampled from 44.1 kHz, the channels averaged).",
           "// Retail has no separate squelch: the quick commands' _radio takes have",
           "// their static lead-in and tail baked in (make_radio_sounds.py says more).",
           ""]
    for name, x in sounds.items():
        path, sha = SOURCES[name]
        pcm = np.clip(np.round(x * 32767), -32768, 32767).astype(np.int16)
        rms = 20 * np.log10(np.sqrt(np.mean(x ** 2)) + 1e-12)
        out.append(f"// {path} (sha256 {sha}): {len(pcm) * 1000 // RATE} ms, rms {rms:.1f} dBFS")
        out.append(f"pub const {name.upper()}: &[i16] = &[")
        for i in range(0, len(pcm), 16):
            out.append("    " + ", ".join(str(int(v)) for v in pcm[i:i + 16]) + ",")
        out.append("];")
        out.append("")
    sys.stdout.write("\n".join(out))


if __name__ == "__main__":
    main()
