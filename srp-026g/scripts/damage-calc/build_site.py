#!/usr/bin/env python3
"""Builds the website's calculator (`/damage`) from the game as Skhid's players have it.

    python3 -I build_site.py --stock <resources.db> --patches <dir> --dbgrab <dbgrab> --ffdec <ffdec-cli.jar>
                             [--with <resources.db.patch_n>] [--list release/patches.tsv] [--work <dir>]

The stock resources.db with exactly the patches of release/patches.tsv over
it, each checked by its sha256 (srp-026g/scripts/released-game). Every step
of the README's build, in one: the configs and texts (dump_json.py,
gen_calc_data.py), the lobby icons (lobby_menu.swf through FFDec,
assets/tools/build_icons.py), then build_page.py --site into
servers/web-server/damage/, and built-from.tsv there: release/release.sh
rebuilds the page (release/build-site) when it offers other patches, and a
web-server test fails a PR whose pages and list differ. Run the Maps tab's build first (srp-026g/scripts/maps-tab): the
calculator takes its maps' names, order and minimaps from it; the renders are
the ones in servers/web-server/damage/maps/. Needs pillow, numpy and java on
PATH.
"""
import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRP = HERE.parents[1]
sys.path.insert(0, str(SRP / "scripts" / "released-game"))
from released import LIST, extract, offered, revision, write_built_from  # noqa: E402

# What the calculator reads of the game.
WANTED = ["gameplay/", "localization/english/localization", "localization/ukrainian/localization",
          "localization/flash_movies_localizations.options", "flash_movies/lobby_menu.swf"]
SITE = SRP / "servers" / "web-server" / "damage"


def run(*args):
    subprocess.run([sys.executable, "-I", *map(str, args)], check=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--stock", required=True)
    ap.add_argument("--patches", required=True, help="the folder with the list's resources.db.patch_<n>")
    ap.add_argument("--list", default=str(LIST))
    ap.add_argument("--with", dest="with_patch", metavar="PATCH",
                    help="the next release's resources.db.patch_<n>, not in the list yet")
    ap.add_argument("--dbgrab", required=True)
    ap.add_argument("--ffdec", required=True, help="ffdec-cli.jar")
    ap.add_argument("--work", help="keep the extracted files here and reuse them")
    args = ap.parse_args()
    rows = offered(args.list, args.with_patch)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(args.work or tmp)
        raw, json_dir, images, icons = (work / d for d in ("raw", "json", "lobby_images", "icons"))
        if not (args.work and raw.is_dir()):
            extract(args.dbgrab, args.stock, rows, args.patches, WANTED, raw)
        json_dir.mkdir(parents=True, exist_ok=True)
        run(HERE / "dump_json.py", raw, json_dir, SRP / "scripts" / "gen-data-026g")
        run(HERE / "gen_calc_data.py", json_dir, work / "calc-data.json")
        if not images.is_dir():
            subprocess.run(["java", "-jar", args.ffdec, "-export", "image", str(images),
                            str(raw / "flash_movies__lobby_menu.swf")], check=True, capture_output=True)
        run(HERE / "assets" / "tools" / "build_icons.py", json_dir, images, icons, "--webp")
        run(HERE / "build_page.py", "--site", work / "calc-data.json", SITE, icons / "icons", SITE / "maps", revision(rows))
    write_built_from(SITE / "built-from.tsv", rows, "the calculator (srp-026g/scripts/damage-calc/build_site.py)")
    print(f"{SITE}: revision {revision(rows)}")


if __name__ == "__main__":
    main()
