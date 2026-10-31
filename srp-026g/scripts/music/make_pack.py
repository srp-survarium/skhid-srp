#!/usr/bin/env python3
"""make_pack.py: the content pack of the music (patch 88; ../../docs/notes/music.md).

usage: python3 -I make_pack.py --donor <folder> --stock <folder> --out <packs folder>/music-<version>

--donor  retail 0.46c2's files at their game paths: sounds/single/music/<track>.high.ogg and .high.rms
         of every track below (resources.07.db of the 0.46c2 build: `vostok-port extract --db
         <resources.07.db> --path <path> --output <folder>/<path>`)
--stock  the stock 0.26g resources.db's sounds/single/scapes/lobby/lobby_background.raw_options at its
         game path (`vostok-port extract`)
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv)

The tracks are retail's own, byte for byte: Vorbis, 44.1 kHz stereo, 112 kb/s nominal, the encoding of
0.26g's own long sounds (the lobby's room tone), each with its RMS cache (0.02 s windows, as the
engine's sound_rms reads it). 0.26g's single sounds take a `.raw_options` each, which 0.46c2's music
has not: the lobby room tone's own (`lobby_background.raw_options`), with

- `volume_k`: the track at -24 LUFS integrated (ffmpeg's ebur128 of the file, the LOUDNESS table):
  the room tone's level (-18.3 LUFS at its `volume_k` 0.5); the game's speech and shots stay above;
- `use_hdr` false: the music is not in the engine's loudness window (it never masks a quiet sound,
  a loud one never pumps it);
- `pan_3d` false: a stereo hud sound, the same wherever the camera is;
- `can_be_displaced` false: the engine's voice limit never takes a music voice.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "lobby-bulb"))
from make_pack import parse, serialize, T_BOOL, T_FLOAT, T_NAMED  # noqa: E402  (the binary config codec)

PACK_ID = "music"
VERSION = "1"
MUSIC = "sounds/single/music/"
STOCK_OPTIONS = "sounds/single/scapes/lobby/lobby_background.raw_options"
STOCK_OPTIONS_SHA = "487e4bd7463ca4d4a7e578846b97d3f2bce351d63742a00e5fd1bc31e8060602"
TARGET_LUFS = -24.0
# track: (its .high.ogg sha256, its .high.rms sha256, its integrated loudness in LUFS)
TRACKS = {
    "main_theme": ("b953ad99ab402c7b98381066fd8a4e99c5fbf0128f8eebae53722a53debc3f11",
                   "ae14714b9dd59eef063a0264657dfe7552e30540adf52cf1946b99b2185b7c0a", -10.1),
    "lobby_theme": ("f2f46a451062084aeef4851e4ecbceca364ad92180542be1fdbac0ed41e58297",
                    "9a2cfd34f3a7b58fb7059ad1d540b587f152f8f2eac82eec7b0f3df7a4449443", -21.8),
    "level_01_loading": ("68fe3efb0f5cfab8a1b05ac7ed312c066031ea53ad2c0115ae5caa37e1e50c94",
                         "e7e1803565a0d1dc8ce1688a34189696ee2baba24f99bb9bf398b31086608aff", -14.0),
    "level_02_loading": ("bc7332eeb4de47e15bbfc4020a39468fb355074d1cb3b134f00f46b86b2f88ed",
                         "8cb90b8eb5b86b3b880b843b117c16ed03b897090a66892585dd7a05ff82bbc9", -16.3),
    "level_03_loading": ("cd6c352f9f411a4aed4692468b496eaca1487c7e8489f6da691b23bd8304d7e1",
                         "6c68fbc75c48440c1efcecac817bd56ab84b661921981b040b79e1c961b1f1a9", -14.3),
    "level_04_loading": ("2f098094c2d0a6ba8f4366c23c1a60af33d8613ca3be9f896ba8fa5cc38a0a52",
                         "6999c48ded2b389a460dfbf663d895647bf6b7a3e171242da002e3b412911f0b", -13.7),
    "level_05_loading": ("657124bceafd80318a04758d9dd042bc66cb48ab4278ecf0d8feda19a4a47e07",
                         "2e7f9cae109c7585dd7c9d538eb7b9de751006e49fcb89c23f69c6dee99cc736", -11.8),
    "level_06_loading": ("3977b10e9eccd9aee0f8b0473214e62b9c89d4c5100d6bf2cafe1ebbeee30712",
                         "c504d84c4c5481b6a03a298d58e13d42c94d6b7316eed5a17bd425f11fc4a0a3", -22.9),
    "level_07_loading": ("fdd5ae81affc19602547bc26b07be5ccc26c360569e063f4804e2984f226a036",
                         "516008ad346bad0bc46c6a5e3d07de7dda2d3638e0eedc47e4827f0bc26b7fd4", -15.5),
    "level_08_loading": ("422ffb14b11998c4129913fac357aac9613d55080ae7d44da19c5c090b8611f2",
                         "26163154038caea1c5f49d632c814a323492434e3eaa296313e4d51381ebb97a", -14.8),
    "level_09_loading": ("67bf05309f8313e88e963404c665cb455bdfb8dafaf14781d6105a1905891307",
                         "47e81c18161e1a10fdb00d8389472e03d3d6d544c9aac7053e18131631b81c4d", -11.3),
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read(folder, path, want):
    data = (folder / path).read_bytes()
    if sha256(data) != want:
        sys.exit(f"{path}: sha256 {sha256(data)}, not the pinned {want}")
    return data


def float_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def options(stock, loudness):
    """The stock room tone's options with the music's values (names as they are, every key there)."""
    root = parse(bytearray(stock))
    assert root[0] == T_NAMED and [k for k, _ in root[1]] == ["options"], "the stock raw_options' shape"
    table = dict(root[1])["options"]
    want = {"volume_k": (T_FLOAT, float_bits(round(10 ** ((TARGET_LUFS - loudness) / 20), 4))),
            "use_hdr": (T_BOOL, 0), "pan_3d": (T_BOOL, 0), "can_be_displaced": (T_BOOL, 0)}
    names = [k for k, _ in table[1]]
    assert set(want) <= set(names), names
    kids = [(k, want.get(k, v)) for k, v in table[1]]
    for key, (kind, _) in want.items():
        assert dict(table[1])[key][0] == kind, (key, "keeps its type")
    return serialize((T_NAMED, [("options", (T_NAMED, kids))]))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--donor", required=True, type=Path)
    ap.add_argument("--stock", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    a = ap.parse_args()
    if a.out.exists():
        sys.exit(f"{a.out}: a new folder")
    stock = read(a.stock, STOCK_OPTIONS, STOCK_OPTIONS_SHA)
    files = {}
    for track, (ogg, rms, loudness) in TRACKS.items():
        files[f"{MUSIC}{track}.high.ogg"] = read(a.donor, f"{MUSIC}{track}.high.ogg", ogg)
        files[f"{MUSIC}{track}.high.rms"] = read(a.donor, f"{MUSIC}{track}.high.rms", rms)
        files[f"{MUSIC}{track}.raw_options"] = options(stock, loudness)
    for path, data in files.items():
        (a.out / "files" / path).parent.mkdir(parents=True, exist_ok=True)
        (a.out / "files" / path).write_bytes(data)
    pack = {
        "format_version": 1, "id": PACK_ID, "version": VERSION, "target_build": "0.26g",
        "description": "the music: retail 0.46c2's main, lobby and level 1-9 loading themes "
                       "(sounds/single/music, Vorbis 112 kb/s with their RMS), each with a .raw_options "
                       "(-24 LUFS, no HDR, not 3D, never displaced) for exe patch 88",
        "operations": [{"op": "add_file", "path": path} for path in sorted(files)],
        "notes": ["made by srp-026g/scripts/music/make_pack.py from retail 0.46c2's resources.07.db and the "
                  "stock resources.db's lobby_background.raw_options (docs/notes/music.md)"],
    }
    (a.out / "pack.json").write_text(json.dumps(pack, indent=2) + "\n")
    size = sum(len(d) for d in files.values())
    print(f"{a.out}: {len(files)} files, {size} bytes; pack.json sha256 {sha256((a.out / 'pack.json').read_bytes())}")


if __name__ == "__main__":
    main()
