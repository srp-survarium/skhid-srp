"""The updater's logs, a file each run (updater_logic::run_log):
updater_skhid.<YYYY-MM-DD_HH-MM-SS>[_<n>].log beside it. The tests read
them all, oldest first, as the one log they read before."""

import os
import re

NAME = re.compile(r"updater_skhid\.(\d{4}-\d\d-\d\d_\d\d-\d\d-\d\d)(?:_([1-9]\d?))?\.log", re.IGNORECASE)


def paths(folder):
    """The run files in `folder`, oldest first (by start, then number)."""
    runs = []
    for name in os.listdir(folder) if os.path.isdir(folder) else []:
        match = NAME.fullmatch(name)
        if match:
            runs.append(((match.group(1), int(match.group(2) or 1)), os.path.join(folder, name)))
    return [path for _, path in sorted(runs)]


def read(folder):
    """Every run's lines, oldest run first; b"" when there are none."""
    data = b""
    for path in paths(folder):
        with open(path, "rb") as file:
            data += file.read()
    return data


def remove(folder):
    for path in paths(folder):
        os.remove(path)
