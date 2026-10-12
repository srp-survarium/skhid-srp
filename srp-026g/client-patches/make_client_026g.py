"""Makes what a player needs to play Survarium 0.26g on an srp server, as a
small overlay folder or ZIP. The ORIGINAL game folder is only read; nothing in it
is changed, and nothing is run.

USAGE

    cd srp-026g/client-patches
    python make_client_026g.py <game folder> <output> --server=<host>:<port>
        [--localization=ukrainian]   english, russian, ukrainian, german, polish
        [--cert-only | --cave=hash-mismatch]   another build of the exe (see patch_026g.py)
        [--check]                    every check, nothing written
        [--pack-zip]                 write the output as a ZIP instead of a folder
        [--bugtrap=<BugTrap.dll>]     defaults to the Rust release build in this repo

    <game folder>   the original 0.26g game: the folder that holds
                    binaries/x86/survarium.exe (the installer's `game`
                    folder), or its parent. The exe must be the original
                    (patch_026g.py checks its sha256).
    <output>        a new/empty folder, or a new ZIP with --pack-zip,
                    OUTSIDE the game folder.
    --server        where the srp login server is reached: the host players
                    use and SRP_LOGIN_PORT (1234 by default; 2234 with the
                    0.26g docker-compose.yml beside the 0.21d server).
                    Embedded in full builds; cert-only passes it at launch.

WHAT IT WRITES (the overlay mirrors the game's layout)

    <output>/game/binaries/x86/survarium_skhid.exe
        the patched client (patch_026g.py: the certificate check, and in
        the full builds the single-instance check, minimum VRAM bypass,
        50 ms hash interval, snapshot upload and default login server).
        The original survarium.exe is not in the
        overlay and stays as it is.
    <output>/game/binaries/x86/play_skhid.bat
        starts it from that folder (the client finds its certificate by a
        path relative to the working directory) with the launch line:
        survarium_skhid.exe -localization=<..>
            -mt_allocator=768 -minimum_resources_memory_size=768
        Full builds use the embedded server; -server=host:port can still
        override it. Cert-only passes -server= at launch as before.
        (-mt_allocator: the global multithreaded arena, 376 MiB by
        default, which map loading ran out of in upstream's live runs;
        -minimum_resources_memory_size: the resources arenas' floor.
        Two different arenas: docs/notes/README.md.)
    <output>/game/binaries/x86/bugtrap.dll
        the Rust SRP crash reporter; saves dumps locally; Submit Bug opens SRP Discord in the browser.
    <output>/game/binaries/x86/play_skhid.sh
        the Linux launcher: creates a local Wine prefix, installs DXVK through
        Winetricks and launches with the same server and language as the batch file.
    <output>/LINUX.txt
        Linux setup and troubleshooting instructions.
    <output>/game/resources/ssl/survarium_login_server.crt
        srp's login certificate (beside this script; the same file as in ../certs): the client accepts the
        login server only with this very file.
    <output>/README.txt
        what to do with it, and where the original game is had.

A player copies the overlay's `game` folder over the game's own `game`
folder and starts play_skhid.bat, or bash play_skhid.sh on Linux. --pack-zip directly makes
the download the web page offers (SRP_DOWNLOAD_FILE). Only the seven player
files are included: no source, PDBs, tests or development tools. Build the
Rust DLL first with ../../client/bugtrap/build.sh. The game itself is had
from SRP_GAME_URL.
"""

import hashlib
import os
from pathlib import Path
import sys
import zipfile

import patch_026g as P

HERE = os.path.dirname(os.path.abspath(__file__))
BUGTRAP_DLL = os.path.abspath(os.path.join(
    HERE, "..", "..", "client", "bugtrap", "target", "i686-pc-windows-msvc", "release", "BugTrap.dll"))
# Where the original game is had: the web archive's copy of build 2777.
GAME_URL = (
    "https://archive.org/download/"
    "vostok_engine_v0.26g0_build_2777_internal_id_1104_jan_14_2015/"
    "survarium_full_026g0.7z"
)
SRP_CERT = os.path.join(HERE, "survarium_login_server.crt")
EXE_DIR = os.path.join("binaries", "x86")
CERT_REL = os.path.join("resources", "ssl", "survarium_login_server.crt")
LINUX_LAUNCHER = os.path.join("game", EXE_DIR, "play_skhid.sh")
# The localizations the client knows (login_chat.md: the language byte).
LOCALIZATIONS = ("english", "russian", "ukrainian", "german", "polish")


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read_bugtrap(path):
    try:
        with open(path, "rb") as dll:
            data = dll.read()
    except OSError as e:
        sys.exit(f"{path}: cannot read Rust BugTrap DLL ({e}). Build client/bugtrap/build.sh first, "
                 "or pass --bugtrap=<path>.")
    pe = int.from_bytes(data[0x3c:0x40], "little")
    if (len(data) < 64 or data[:2] != b"MZ" or pe + 26 > len(data)
            or data[pe:pe + 4] != b"PE\0\0"
            or int.from_bytes(data[pe + 4:pe + 6], "little") != 0x14c
            or int.from_bytes(data[pe + 24:pe + 26], "little") != 0x10b
            or not int.from_bytes(data[pe + 22:pe + 24], "little") & 0x2000):
        sys.exit(f"{path}: expected a 32-bit x86 Windows DLL")
    return data


def game_root(folder):
    """The folder that holds binaries/x86/survarium.exe: `folder` or its `game`."""
    for root in (folder, os.path.join(folder, "game")):
        if os.path.isfile(os.path.join(root, EXE_DIR, "survarium.exe")):
            return os.path.abspath(root)
    sys.exit(f"{folder}: no {os.path.join(EXE_DIR, 'survarium.exe')} in it or in its `game` folder")


def inside(path, folder):
    path, folder = (os.path.normcase(os.path.realpath(p)) for p in (path, folder))
    return path == folder or path.startswith(folder + os.sep)


def parse_server(text):
    return P.parse_server(text)


def launch_line(server, localization, embedded=False):
    return (f"survarium_skhid.exe -localization={localization} "
            + ("" if embedded else f"-server={server} ")
            + "-mt_allocator=768 -minimum_resources_memory_size=768")


def bat(server, localization, embedded=False):
    lines = [
        "@echo off",
    ]
    if embedded:
        lines += [f"rem Default login server embedded in the exe: {server}",
                  "rem To override it, add -server=host:port to the start command below."]
    else:
        lines += ["rem Put the server address (host:1234) here, as given on the server page.",
                  f"set SERVER={server}"]
    lines += [
        "rem Game language: english, russian, ukrainian, german or polish.",
        f"set LANGUAGE={localization}",
        "rem It must run from this folder (game\\binaries\\x86): the game finds",
        "rem ..\\..\\resources\\ssl\\survarium_login_server.crt from where it is started.",
        'cd /d "%~dp0"',
        'start "" ' + launch_line("%SERVER%", "%LANGUAGE%", embedded),
        "",
    ]
    return "\r\n".join(lines).encode("ascii")


def sh(server, localization, embedded=False):
    with open(os.path.join(HERE, "play_skhid.sh"), "rb") as source:
        script = source.read()
    return script.replace(b"localization=english\n", f"localization={localization}\n".encode("ascii"), 1).replace(
        b"server=\n", f"server={'' if embedded else server}\n".encode("ascii"), 1)


def readme(server, localization, description, files, embedded=False):
    lines = [
        "SURVARIUM 0.26g - SKHID COMMUNITY SERVER FILES (UNOFFICIAL)",
        "",
        "Not affiliated with or endorsed by Vostok Games. Survarium is a trademark",
        "of its owners. The server is built on the open source SRP project:",
        "https://github.com/srp-survarium/srp",
        "",
        "This archive holds only the few files that go over the original game; the",
        "game itself is not in it.",
        "",
        "This is an early test server. It has bugs and may not work fully. Your",
        "progress, items and accounts can be lost or reset at any time, and the",
        "server may be stopped at any time. By playing you accept this.",
        "",
        "HOW TO PLAY",
        "1. Get the original Survarium 0.26g (build 2777), a .7z archive of about",
        "   2.9 GB (7-Zip opens it), and unpack it:",
        f"   {GAME_URL}",
        "   The game is in its Survarium\\game folder.",
        "2. Copy the `game` folder of this archive over the game's own `game`",
        "   folder. It adds survarium_skhid.exe, play_skhid.bat and play_skhid.sh to",
        "   game\\binaries\\x86 and replaces bugtrap.dll there, plus",
        "   game\\resources\\ssl\\survarium_login_server.crt. Back up those two",
        "   original files first. The game's own survarium.exe is not changed.",
        "3. Windows: double-click game\\binaries\\x86\\play_skhid.bat.",
        "   Linux: install Wine and Winetricks, then run bash play_skhid.sh",
        "   from game/binaries/x86. See LINUX.txt for setup and troubleshooting.",
        "   These launchers supply the options the game needs.",
        "4. Sign in with any user name and password. A new name is registered on its",
        "   first sign-in; remember the password.",
        "",
        "To change the language, open play_skhid.bat in a text editor and set",
        "LANGUAGE. On Linux, edit localization near the top of play_skhid.sh;",
        "set server=host:port there to override the embedded login address.",
        "",
        "OPTIONS (in play_skhid.bat)",
        (f"  Default server: {server} (embedded in survarium_skhid.exe)" if embedded
         else f"  Server: {server} (SERVER in play_skhid.bat)"),
        "  -server=         override the login server (host:port)",
        "  -localization=   language: english, russian, ukrainian, german or polish",
        "  -mt_allocator=768, -minimum_resources_memory_size=768",
        "                   memory for the game, MB (maps need more than the default)",
        "",
        "The rules of the server are on its page:",
        f"https://{server.split(':')[0]}",
        "",
        "survarium_skhid.exe is the game's survarium.exe patched to work with this",
        "server. The launcher and the updater of the original game are not needed.",
        "",
        "CRASH REPORTS",
        "Crash dumps are saved beside survarium_skhid.exe, or in the Windows",
        "temporary folder if that folder cannot be written. The crash dialog",
        "shows the location. Nothing is uploaded automatically.",
        "",
    ]
    return "\r\n".join(lines).encode("utf-8")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    options = [a for a in sys.argv[1:] if a.startswith("--")]
    mode, check, server, localization = "section", False, None, "ukrainian"
    bugtrap_path = BUGTRAP_DLL
    archive = False
    for option in options:
        if option == "--check":
            check = True
        elif option == "--pack-zip":
            archive = True
        elif option == "--cert-only":
            mode = "cert-only"
        elif option.startswith("--cave=") and option[7:] in P.CAVES:
            mode = option[7:]
        elif option.startswith("--server="):
            server = parse_server(option[9:])
        elif option.startswith("--localization=") and option[15:] in LOCALIZATIONS:
            localization = option[15:]
        elif option.startswith("--bugtrap="):
            bugtrap_path = option[len("--bugtrap="):]
        else:
            sys.exit(f"unknown option {option}\n\n{__doc__}")
    if len(args) != 2 or server is None:
        sys.exit(__doc__)
    source, output = game_root(args[0]), os.path.abspath(args[1])
    # Never into the game: not its folder, nor the install that holds it.
    if inside(output, source) or inside(output, os.path.dirname(source)):
        sys.exit(f"{output}: the output folder must be outside the game folder {os.path.dirname(source)}")
    if not check and os.path.lexists(output):
        if archive or not os.path.isdir(output) or os.listdir(output):
            sys.exit(f"{output}: output already exists; use a new ZIP or an empty folder")

    bugtrap = read_bugtrap(bugtrap_path)

    # The exe: patch_026g.py refuses anything but the original.
    original = Path(source, EXE_DIR, "survarium.exe").read_bytes()
    code = None if mode == "cert-only" else Path(P.default_bin(mode)).read_bytes()
    embedded = mode != "cert-only"
    exe, description = P.build(original, mode, code, server if embedded else None)
    exe = bytes(exe)

    # The certificates.
    if not os.path.isfile(SRP_CERT):
        sys.exit(f"{SRP_CERT}: srp's certificate is missing")
    srp_cert = Path(SRP_CERT).read_bytes()
    if b"BEGIN CERTIFICATE" not in srp_cert:
        sys.exit(f"{SRP_CERT}: not a PEM certificate")
    game_cert_path = os.path.join(source, CERT_REL)
    if not os.path.isfile(game_cert_path):
        sys.exit(f"{game_cert_path}: the game folder has no login certificate")

    files = [
        (os.path.join("game", EXE_DIR, "survarium_skhid.exe"), exe),
        (os.path.join("game", EXE_DIR, "bugtrap.dll"), bugtrap),
        (os.path.join("game", EXE_DIR, "play_skhid.bat"), bat(server, localization, embedded)),
        (LINUX_LAUNCHER, sh(server, localization, embedded)),
        (os.path.join("game", CERT_REL), srp_cert),
        ("LINUX.txt", Path(HERE, "LINUX.txt").read_bytes()),
    ]
    listed = [(name.replace(os.sep, "/"), sha256(data)) for name, data in files]
    files.append(("README.txt", readme(server, localization, description, listed, embedded)))

    if not check and archive:
        os.makedirs(os.path.dirname(output), exist_ok=True)
        with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED) as out:
            for name, data in files:
                entry = zipfile.ZipInfo(name.replace(os.sep, "/"))
                entry.create_system = 3
                entry.external_attr = (0o100755 if name == LINUX_LAUNCHER else 0o100644) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                out.writestr(entry, data)
    for name, data in files:
        target = (output + "!/" + name.replace(os.sep, "/") if archive else os.path.join(output, name))
        if not check and not archive:
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "xb") as out:   # never over a file that is there
                out.write(data)
            if name == LINUX_LAUNCHER:
                os.chmod(target, 0o755)
        print(f"{'would write' if check else 'wrote'} {target} ({len(data)} bytes, sha256 {sha256(data)})")
    print(f"exe: {description}")
    print(f"launch line: {launch_line(server, localization, embedded)}")
    if check:
        print("--check: nothing was written")


if __name__ == "__main__":
    main()
