#!/usr/bin/env python3
"""Create a separate Wine 11.8 runtime using the rebuilt X11 input driver."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wine", type=Path, help="installed Wine package directory")
    parser.add_argument("driver", type=Path, help="driver.nix output directory")
    parser.add_argument("output", type=Path, help="new runtime directory; must not exist")
    args = parser.parse_args()
    source, driver, target = args.wine.resolve(), args.driver.resolve(), args.output.absolute()
    version = subprocess.check_output([source / "bin/wine", "--version"], text=True).strip()
    if version.split()[0] != "wine-11.8":
        parser.error(f"expected Wine 11.8, got {version}")
    driver_file = driver / "lib/wine/x86_64-unix/winex11.so"
    # These files must be real copies: Wine resolves loader/preloader/ntdll paths
    # with realpath(), which would send symlinks back to the unpatched package.
    copies = ["bin/wine", "lib/wine/x86_64-unix/wine",
              "lib/wine/x86_64-unix/wine-preloader", "lib/wine/x86_64-unix/ntdll.so"]
    for path in [driver_file, *(source / name for name in copies)]:
        if not path.is_file():
            parser.error(f"missing runtime file: {path}")
    target.mkdir(parents=True, exist_ok=False)
    try:
        for root, dirs, files in os.walk(source):
            destination = target / Path(root).relative_to(source)
            destination.mkdir(parents=True, exist_ok=True)
            for name in dirs[:]:
                path = Path(root) / name
                if path.is_symlink():
                    (destination / name).symlink_to(path.resolve())
                    dirs.remove(name)
            for name in files:
                (destination / name).symlink_to(Path(root) / name)
        for name in copies:
            (target / name).unlink()
            shutil.copy2(source / name, target / name)
        replacement = target / "lib/wine/x86_64-unix/winex11.so"
        replacement.unlink()
        replacement.symlink_to(driver_file)
    except BaseException:
        # Only the new directory this invocation owns is removed on failure.
        shutil.rmtree(target)
        raise
    print(target / "bin/wine")


if __name__ == "__main__":
    main()
