#!/usr/bin/env python3
"""make_pack.py: the content pack of 0.21's death sound (exe patch 196; ../../docs/notes/corpse-shot-dark.md).

usage: python3 -I make_pack.py --donor <folder> --out <packs folder>/death-sound-021d-<version>

--donor  0.21d's files at their game paths, every DONOR path below (its resources.db:
         `vostok-port extract --db <0.21d resources.db> --path <path> --output <folder>/<path>`)
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv)

0.21d's death sound as others heard it at the body, `animations/death_animation_m` (the torso's
second sound in gameplay/bodyparts, played at frame 5 of the death clip), is a composite of a body
fall (its thud 1.1 s in) and a male scream at once, each a random take of its collection. 0.26g
keeps the same composite with other screams, and never plays it (its death clips have no sound
event). Here it is its scream alone (the owner, 2026-11-23: the drop cut out; version 1 had the
fall too), 0.21d's at new paths, so nothing of 0.26g's changes:

- the scream's takes byte for byte (Vorbis 44.1 kHz mono and their RMS caches: voice only, no
  impact in any), their `.raw_options` as 0.21d's but for the distance preset's name;
- the preset 0.21d's (`death_scream_close`: silent from 9 m; 0.26g's own reaches 15 m), as
  `death_scream_close_021d`;
- the collection 0.21d's, its items' names the new ones; the composite 0.21d's without its fall
  item, its scream item's name the new one.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "lobby-bulb"))
from make_pack import parse, serialize, T_NAMED, T_INDEXED, T_STR  # noqa: E402  (the binary config codec)

PACK_ID = "death-sound-021d"
VERSION = "2"
DEATH = "animations/death/"
NEW = DEATH + "v021d/"
# each take: (0.21d's sample, its new name, its preset); its three files' sha256 (ogg, rms, options)
TAKES = [
    (DEATH + "male_scream/death_01_m", "scream_01_m", "death_scream_close",
     "b80ad1581421b93532a5a32d7aeb906eaf671eb05fce5deee1cfd4b074a4837b",
     "d04b70724130e1b648990fa84f572c9de3db159e5e6607d9bfa9eb511c6426e1",
     "2d16a14266a97a1b893888a7b5f79c91ea4a005752ada0a889ac6c33cb41616f"),
    (DEATH + "male_scream/death_02_m", "scream_02_m", "death_scream_close",
     "b06dffcad81bbeb6afa149e3ba45ec2f98dee0dc834aa120b14d62fac5f45c0c",
     "812933cad240b1965d28f637870d3536c1c87dc8f0b2182ccab200cd68302015",
     "2d16a14266a97a1b893888a7b5f79c91ea4a005752ada0a889ac6c33cb41616f"),
    (DEATH + "male_scream/death_03_m", "scream_03_m", "death_scream_close",
     "8db6ff4de5b522d8b73f5b5d08f94ec1e0765e1e01a525201e75be349a12dd56",
     "fcdf0fb5445467a7c504eb8c576fdc7977b6df21871a1c87ad27da738ef99c76",
     "2d16a14266a97a1b893888a7b5f79c91ea4a005752ada0a889ac6c33cb41616f"),
    (DEATH + "male_scream/death_04_m", "scream_04_m", "death_scream_close",
     "26dcd64e219b0b4155433b27916bd75c8dfdbe94b9c9d4004f9676c95a5dc359",
     "d244239c80c75077f6f5daf44cd925b9a6a7135999c64e4833e04c04b0afabbe",
     "2d16a14266a97a1b893888a7b5f79c91ea4a005752ada0a889ac6c33cb41616f"),
]
# each preset: its .spl's sha256
PRESETS = {
    "death_scream_close": "a01535ea343210ddcb815bbbc33cb85bd5719b8762a9307151a8f9cab7f2d281",
}
# each collection: (0.21d's, the new one, the folder of its takes, its sha256)
COLLECTIONS = [
    (DEATH + "death_male_scream_m", "animations/death/v021d_scream_m", "male_scream",
     "0b6ea1faf7153a05c583171c6ece41385dfa4d0991decd65ad757e152dad1002"),
]
# 0.21d's composite, the new one (the name exe patch 196 plays), its sha256, its item left out
COMPOSITE = ("animations/death_animation_m", "animations/death_animation_021d_m",
             "9f60f5db2fda0f1960c487bb35a30752cc7899ec7df5a1bcc2d4448398789d63", DEATH + "death_fall_m")
SUFFIX = "_021d"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read(folder, path, want):
    data = (folder / path).read_bytes()
    if sha256(data) != want:
        sys.exit(f"{path}: sha256 {sha256(data)}, not the pinned {want}")
    return data


def renamed(config, table, names):
    """`config` with each string value of `table` found in `names` replaced (every other byte's
    meaning kept); every value of `names` must be met."""
    root = parse(bytearray(config))
    met = set()

    def walk(node, under):
        t, v = node
        if t == T_STR and under and v in names:
            met.add(v)
            return (t, names[v])
        if t == T_NAMED:
            return (t, [(k, walk(kid, under or k == table)) for k, kid in v])
        if t == T_INDEXED:
            return (t, [walk(kid, under) for kid in v])
        return node

    out = walk(root, False)
    if met != set(names):
        sys.exit(f"{table}: {sorted(set(names) - met)} not found")
    return serialize(out)


def without(config, item):
    """the composite `config` without its sound item `item` (every other byte's meaning kept)."""
    t, [(name, (ct, fields))] = parse(bytearray(config))
    it, items = dict(fields)["sound_items"]
    kept = [i for i in items if dict(i[1])["filename"] != (T_STR, item)]
    if len(kept) != len(items) - 1:
        sys.exit(f"{name}: no item {item}")
    return serialize((t, [(name, (ct, [(k, (it, kept) if k == "sound_items" else v) for k, v in fields]))]))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--donor", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    a = ap.parse_args()
    if a.out.exists():
        sys.exit(f"{a.out}: a new folder")
    files = {}
    for old, new, preset, ogg, rms, options in TAKES:
        files[f"sounds/single/{NEW}{new}.high.ogg"] = read(a.donor, f"sounds/single/{old}.high.ogg", ogg)
        files[f"sounds/single/{NEW}{new}.high.rms"] = read(a.donor, f"sounds/single/{old}.high.rms", rms)
        files[f"sounds/single/{NEW}{new}.raw_options"] = renamed(
            read(a.donor, f"sounds/single/{old}.raw_options", options), "options", {preset: preset + SUFFIX})
    for preset, want in PRESETS.items():
        files[f"sounds/spl/{preset}{SUFFIX}.spl"] = read(a.donor, f"sounds/spl/{preset}.spl", want)
    takes = {old: NEW + new for old, new, *_ in TAKES}
    for old, new, folder, want in COLLECTIONS:
        data = read(a.donor, f"sounds/collections/{old}.sound_collection", want)
        mine = {k: v for k, v in takes.items() if k.startswith(f"{DEATH}{folder}/")}
        files[f"sounds/collections/{new}.sound_collection"] = renamed(data, "collection", mine)
    old, new, want, fall = COMPOSITE
    files[f"sounds/composite/{new}.composite_sound"] = renamed(
        without(read(a.donor, f"sounds/composite/{old}.composite_sound", want), fall), "composite_sound",
        {o: n for o, n, *_ in COLLECTIONS})
    for path, data in files.items():
        (a.out / "files" / path).parent.mkdir(parents=True, exist_ok=True)
        (a.out / "files" / path).write_bytes(data)
    pack = {
        "format_version": 1, "id": PACK_ID, "version": VERSION, "target_build": "0.26g",
        "description": "0.21d's death scream heard at the body (its death_animation_m without its body fall: "
                       "the male scream, 0.21d's takes and distance preset) at new paths, "
                       "sounds/composite/animations/death_animation_021d_m, for exe patch 196",
        "operations": [{"op": "add_file", "path": path} for path in sorted(files)],
        "notes": ["made by srp-026g/scripts/death-sound-021d/make_pack.py from 0.21d's resources.db "
                  "(docs/notes/corpse-shot-dark.md)"],
    }
    (a.out / "pack.json").write_text(json.dumps(pack, indent=2) + "\n")
    size = sum(len(d) for d in files.values())
    print(f"{a.out}: {len(files)} files, {size} bytes; pack.json sha256 {sha256((a.out / 'pack.json').read_bytes())}")


if __name__ == "__main__":
    main()
