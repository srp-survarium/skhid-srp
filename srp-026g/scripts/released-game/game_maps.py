"""The maps of the released game, as the site's builds read them from the
files released.py extracts (`raw`, '/' as '__'): which maps can be played and
their modes, and each project's HUD minimap. Shared by the Maps tab
(srp-026g/scripts/maps-tab/build.py) and the top-down renders
(srp-026g/scripts/damage-calc/assets/maps/tools/build_maps.py).

    maps = playable(raw)                      # {folder: {"map_name", "modes": {mode: {...}}}}
    minimaps = hud_minimaps(ffdec, raw, work)  # {project_name: (frame, bitmap png)}
"""
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "gen-data-026g"))
import bincfg  # noqa: E402
from released import die  # noqa: E402

# The modes a match can be (2 batteries, 3 batteries with chargers, 4 team
# deathmatch, 5 Research), as the server plays them
# (crates/survarium/src/v026g/maps.rs).
MODES = (2, 3, 4, 5)
# What a build must extract for these.
WANTED = ["client_project", "gameplay/game_modes/", "gameplay/db_static_dictionaries",
          "localization/english/localization", "localization/ukrainian/localization", "flash_movies/hud.swf"]


def load(raw, path):
    file = Path(raw) / path.replace("/", "__")
    return bincfg.parse(file.read_bytes()) if file.is_file() else None


def project_file(raw, project_path):
    return Path(raw) / f"projects__{project_path.replace('/', '__')}__client_project"


def playable(raw):
    """The playable maps by project folder: a `maps_dict` id whose
    `gameplay/game_modes/` config is of mode 2 to 5 and whose project
    (`match_making_params.map_name`) exists. Of each map: the name key
    (`map_name`) and per mode the ids, their projects and configs, the mode's
    name key, parameters and rules."""
    dictionary = load(raw, "gameplay/db_static_dictionaries")
    maps = defaultdict(lambda: {"modes": defaultdict(lambda: {"ids": [], "projects": [], "configs": []})})
    for entry in dictionary["maps_dict"]:
        config = load(raw, "gameplay/game_modes/" + entry["cfg_mode_name"])
        if not config or config["mode"].get("mode") not in MODES:
            continue
        mode = config["mode"]
        project_path = mode["match_making_params"]["map_name"]
        if not project_file(raw, project_path).is_file():
            continue
        folder = project_path.split("/")[0]
        m = maps[folder]
        m["map_name"] = mode["ui"]["map_name"]
        entry_mode = m["modes"][mode["mode"]]
        entry_mode["ids"].append(entry["id"])
        entry_mode["projects"].append(project_path)
        entry_mode["configs"].append({"id": entry["id"], "config": entry["cfg_mode_name"], "name": mode.get("name"),
                                      "weather": mode["match_making_params"].get("map_weather_id"),
                                      "project": project_path})
        entry_mode["mode_name"] = mode["ui"]["mode_name"]
        entry_mode["parameters"] = mode.get("parameters") or {}
        entry_mode["rules"] = set(mode.get("rules") or [])
    if not maps:
        die("no playable map in the data")
    return maps


def _number(text):
    v = float(text)
    return int(v) if v == int(v) else v


def hud_minimaps(ffdec, raw, work):
    """{project_name: (frame {size, posX, posY}, bitmap png path)} from
    hud.swf's class survarium.controls.Minimap: `_minimapProps` (the frame:
    2*size metres around posX, posY) and `_levels` (the bitmap), both keyed by
    the project_name the engine passes. Exported with FFDec into `work`."""
    swf = Path(raw) / "flash_movies__hud.swf"
    work = Path(work)
    for what in ("script", "image"):
        cmd = ["java", "-jar", str(ffdec)]
        if what == "script":
            cmd += ["-selectclass", "survarium.controls.Minimap"]
        subprocess.run(cmd + ["-export", what, str(work / what), str(swf)], check=True, capture_output=True)
    source = (work / "script" / "scripts" / "survarium" / "controls" / "Minimap.as").read_text()
    num = r"(-?[\d.]+)"
    props = {m.group(1): {"size": _number(m.group(2)), "posX": _number(m.group(3)), "posY": _number(m.group(4))}
             for m in re.finditer(r"_minimapProps\.(\w+) = \{\s*\"size\":" + num + r",\s*\"posX\":" + num
                                  + r",\s*\"posY\":" + num, source)}
    for alias, of in re.findall(r"_minimapProps\.(\w+) = this\._minimapProps\.(\w+);", source):
        props[alias] = props[of]
    fields = dict(re.findall(r"private var (\w+):BitmapData = new (\w+)\(\);", source))
    levels = {}
    block = re.search(r"_levels(?::Object)? = \{(.*?)\};", source, re.S)
    if not block:
        die("hud.swf: Minimap has no _levels table")
    for key, cls, field in re.findall(r"\"(\w+)\":(?:new (\w+)\(\)|this\.(\w+))", block.group(1)):
        levels[key] = cls or fields[field]
    images = os.listdir(work / "image")
    out = {}
    for key, cls in levels.items():
        found = [n for n in images if re.fullmatch(rf"\d+_{re.escape(cls)}(_.*)?\.png", n)]
        if len(found) != 1 or key not in props:
            continue
        out[key] = (props[key], work / "image" / found[0])
    return out
