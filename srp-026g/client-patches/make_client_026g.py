"""Makes what a player needs to play Survarium 0.26g on an srp server, as a
small overlay folder or ZIP. The ORIGINAL game folder is only read; nothing in it
is changed, and nothing is run.

USAGE

    cd srp-026g/client-patches
    python make_client_026g.py <game folder> <output> --server=<host>:<port>
        [--cert-only | --cave=hash-mismatch]   another build of the exe (see patch_026g.py)
        [--no-late-join]             the exe without the late join: patch revision 4,
                                     for a server that asks for no later one
        [--revision=<n>]             the default exe says patch revision <n>, a
                                     release's, instead of REVISION (patch_026g.py)
        [--required-patches=<n>,...] the default exe stops at its start without
                                     these resources.db.patch_<n> (patch 49): a
                                     release's offered patches (patch_026g.py)
        [--downloads=<list.tsv>]...  the release's files and their URLs
                                     (release/game.tsv, release/patches.tsv: the
                                     game's archive, the patches) written into
                                     Skhid-without-updater.bat, which uses them
                                     when the server cannot be reached
        [--check]                    every check, nothing written
        [--pack-zip]                 write the output as a ZIP instead of a folder
        [--bugtrap=<BugTrap.dll>]     defaults to the Rust release build in this repo
        [--updater=<updater_skhid.exe>]  defaults to the release build in this repo
        [--extractor=<7zr.exe>]      defaults to the copy in this repo; it must be
                                     the file the updater is built to run
        [--site=https://<host>]      the server's web page; https://<host of --server>
                                     when not given
        [--no-launcher]              without the launcher and its updater

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
        50 ms hash interval, snapshot upload, default login server, the
        battery rule and, in the default build, the late join: patch
        revision 5, which plays only with its own kind).
        The original survarium.exe is not in the
        overlay and stays as it is.
    <output>/game/binaries/x86/play_skhid.bat
        starts it from that folder (the client finds its certificate by a
        path relative to the working directory) with the launch line:
        survarium_skhid.exe -mt_allocator=768 -minimum_resources_memory_size=768
        Full builds use the embedded server; -server=host:port can still
        override it. Cert-only passes -server= at launch as before.
        (-mt_allocator: the global multithreaded arena, 376 MiB by
        default, which map loading ran out of in upstream's live runs;
        -minimum_resources_memory_size: the resources arenas' floor.
        Two different arenas: docs/SRP-026G.md.)
    <output>/game/binaries/x86/bugtrap.dll
        the Rust SRP crash reporter; saves zipped dumps locally; Submit Bug opens SRP Discord in the browser.
    <output>/game/binaries/x86/play_skhid.sh
        the Linux launcher: creates a local Wine prefix, installs DXVK through
        Winetricks and launches with the same server and language as the batch file.
    <output>/LINUX.txt
        Linux setup and troubleshooting instructions.
    <output>/game/resources/ssl/survarium_login_server.crt
        srp's login certificate (beside this script; the same file as in ../certs): the client accepts the
        login server only with this very file.
    <output>/game/binaries/x86/launcher_skhid.exe
        the game's own launcher, patched (launcher/patch_launcher_026g.py):
        it runs from the game folder with no installer, shows the server's
        page, starts updater_skhid.exe and then survarium_skhid.exe in the
        language chosen in it. The game folder must hold the ORIGINAL
        survarium_launcher.exe (its sha256 is checked).
    <output>/game/binaries/x86/updater_skhid.exe
        the updater (../../client/updater): asks the server which archive
        it offers and installs it when it is another one than the last.
    <output>/game/binaries/x86/7zr.exe
        7-Zip's standalone extractor, unchanged (../../client/updater/extractor):
        the updater unpacks the original game with it when that is missing,
        and runs it only when it is the file whose size and sha256 are
        compiled into the updater. This tool refuses another file.
    <output>/game/binaries/x86/updater_skhid.url
        the server it asks: --site.
    <output>/game/binaries/x86/launcher_skhid/index-*.html
        what the launcher shows while and when the server's page does not
        load (launcher/launcher_pages.py).
    <output>/Skhid.bat
        the one file a player starts: it starts launcher_skhid.exe, and
        says where the files belong when it is not found.
    <output>/Skhid-without-updater.bat
        for a player whose antivirus deletes updater_skhid.exe: installs and
        updates the game and the Skhid files with curl, tar, PowerShell and
        7zr.exe, without the launcher and the updater, and starts the game
        (Skhid-without-updater.bat here, with the files of --downloads
        written in at its end: `#skhid-file <name> <sha256> <size> <url>...`).
    <output>/Skhid.sh
        the same for Linux (Skhid.sh beside this script): `bash Skhid.sh`
        runs play_skhid.sh from wherever it is called, with its arguments.
    <output>/VPN.txt
        for a player whose provider blocks the server (Russia): which
        programs to put in a VPN's split tunnelling, how in the usual VPN
        programs, the hosts and ports (English and Russian; VPN.txt here).
    <output>/Skhid-network-setup.cmd
        Windows Firewall outbound allow rules for the three programs; --undo
        removes them, --dry-run prints the commands. It asks Windows for
        administrator rights itself (the UAC prompt) and says everything in
        Russian on a Windows in Russian, Ukrainian or Belarusian, in English
        on any other (Skhid-network-setup.cmd here).
    <output>/Skhid-defender-setup.cmd
        a Microsoft Defender exclusion of game\binaries\x86, then the Skhid
        files Defender deleted put back, with --undo and --dry-run, the
        rights and the languages the same way (Skhid-defender-setup.cmd here).
    <output>/Skhid-collect-logs.cmd
        for a player for whom nothing works: the logs (credential lines
        removed), the crash reports' text, the game folder's listing and
        hashes, the servers' answers and Windows' state, in one ZIP on the
        Desktop to send; changes nothing, takes no password, no user.cfg, no
        dump; no administrator rights; the languages the same way
        (Skhid-collect-logs.cmd here).
    <output>/README.txt
        what to do with it, and where the original game is had.

CONTENT

    New content reaches the players as patches of resources.db
    (game/resources.db.patch_<n>, which the server lists and the updater
    downloads: ../../client/updater/README.md, "Patches"), never in this
    overlay. The overlay has no file of resources.db either: an archive that
    did (updater_skhid.resources, a plan of entries rewritten in place) is
    of before the patches; the updater gives a db changed by it its stock
    bytes back, once.

A player unpacks the overlay into an empty folder, or into the game's folder
(the one that holds `game`), and starts Skhid.bat (or play_skhid.bat: the game alone, no
updates), or bash Skhid.sh on Linux. --pack-zip directly makes the
download the web page offers (SRP_DOWNLOAD_FILE), which is also what the
updater installs from then on. Only the player files are included: no
source, PDBs, tests or development tools. Build the Rust programs first
with ../../client/bugtrap/build.sh and ../../client/updater/build.sh. The
updater downloads the original game from its pinned mirror when it is missing.
"""

import hashlib
import os
from pathlib import Path
import re
import sys
import zipfile

import patch_026g as P

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "launcher"))
import launcher_pages  # noqa: E402
import patch_launcher_026g as L  # noqa: E402

UPDATER_EXE = os.path.abspath(os.path.join(
    HERE, "..", "..", "client", "updater", "exe", "target", "i686-win7-windows-msvc", "release", L.UPDATER_NAME))
# The extractor the updater runs, and the size and sha256 the updater is
# built with (EXTRACTOR in client/updater/logic/src/game.rs; a test here
# reads them from that file and compares).
EXTRACTOR_NAME = "7zr.exe"
EXTRACTOR_EXE = os.path.abspath(os.path.join(HERE, "..", "..", "client", "updater", "extractor", EXTRACTOR_NAME))
EXTRACTOR_SIZE = 602624
EXTRACTOR_SHA256 = "256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282"
BUGTRAP_DLL = os.path.abspath(os.path.join(
    HERE, "..", "..", "client", "bugtrap", "target", "i686-win7-windows-msvc", "release", "BugTrap.dll"))
# Where the original game is had by hand: the patches server's address of its
# row in release/game.tsv (no download quota), the bytes
# client/updater/logic/src/game.rs pins (the
# updater and Skhid-without-updater.bat take every address of that row).
GAME_URL = "http://2.28.121.133:8090/survarium-026g.7z"
SRP_CERT = os.path.join(HERE, "survarium_login_server.crt")
EXE_DIR = os.path.join("binaries", "x86")
EXE_DIR_WINDOWS = "binaries\\x86"
# The one file a player starts: beside the `game` folder.
STARTER = "Skhid.bat"
# The same without the launcher and the updater, for a player whose
# antivirus deletes updater_skhid.exe: it installs and updates by itself.
WITHOUT_UPDATER = "Skhid-without-updater.bat"
CERT_REL = os.path.join("resources", "ssl", "survarium_login_server.crt")
LINUX_LAUNCHER = os.path.join("game", EXE_DIR, "play_skhid.sh")
# The same for Linux, run as `bash Skhid.sh` beside the `game` folder.
LINUX_STARTER = "Skhid.sh"
# For a player who cannot reach the server: what to put in a VPN's split
# tunnelling, and the firewall rules (beside Skhid.bat).
VPN_HELP = "VPN.txt"
NETWORK_SETUP = "Skhid-network-setup.cmd"
# For a player whose antivirus takes the game's files for a threat: the
# Defender exclusion of game\binaries\x86 (beside Skhid.bat).
DEFENDER_SETUP = "Skhid-defender-setup.cmd"
# For a player for whom nothing works: the logs and the folder's state in one
# ZIP to send (beside Skhid.bat).
COLLECT_LOGS = "Skhid-collect-logs.cmd"
# What a Linux unpacker makes executable.
SCRIPTS = (LINUX_LAUNCHER, LINUX_STARTER)


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


def read_updater(path):
    try:
        with open(path, "rb") as exe:
            data = exe.read()
    except OSError as e:
        sys.exit(f"{path}: cannot read the updater ({e}). Build client/updater/build.sh first, "
                 "pass --updater=<path>, or leave the launcher out with --no-launcher.")
    pe = int.from_bytes(data[0x3c:0x40], "little")
    if (len(data) < 64 or data[:2] != b"MZ" or pe + 26 > len(data)
            or data[pe:pe + 4] != b"PE\0\0"
            or int.from_bytes(data[pe + 4:pe + 6], "little") != 0x14c
            or int.from_bytes(data[pe + 24:pe + 26], "little") != 0x10b
            or int.from_bytes(data[pe + 22:pe + 24], "little") & 0x2000):
        sys.exit(f"{path}: expected a 32-bit x86 Windows program")
    # Without this in its manifest Windows takes a program named "updater"
    # for an installer and asks for administrator rights at every start.
    if not any(b"requestedExecutionLevel level=" + quote + b"asInvoker" + quote in data for quote in (b"'", b'"')):
        sys.exit(f"{path}: the updater has no asInvoker manifest")
    return data


def read_extractor(path):
    try:
        with open(path, "rb") as exe:
            data = exe.read()
    except OSError as e:
        sys.exit(f"{path}: cannot read the extractor ({e}). Pass --extractor=<path to the pinned 7zr.exe>, "
                 "or leave the launcher out with --no-launcher.")
    # The updater runs no other file than this one, so no other is shipped.
    if len(data) != EXTRACTOR_SIZE or sha256(data) != EXTRACTOR_SHA256:
        sys.exit(f"{path}: not the extractor the updater is built to run "
                 f"({EXTRACTOR_SIZE} bytes, sha256 {EXTRACTOR_SHA256})")
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


def game_arguments(server, embedded=False):
    """What the game is started with after its language."""
    return (("" if embedded else f"-server={server} ")
            + "-mt_allocator=768 -minimum_resources_memory_size=768")


def launch_line(server, embedded=False):
    """The game's language is its own (Settings, patch 79): no -localization."""
    return "survarium_skhid.exe " + game_arguments(server, embedded)


def bat(server, embedded=False):
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
        "rem It must run from this folder (game\\binaries\\x86): the game finds",
        "rem ..\\..\\resources\\ssl\\survarium_login_server.crt from where it is started.",
        'cd /d "%~dp0"',
        "rem resources.db's patches, the ones the server lists, must be in place",
        "rem first: the exe of a release needs the content of its patches. The",
        "rem updater checks them (it reads only; no updater beside this file, as",
        "rem without the launcher: no check).",
        'if not exist "updater_skhid.exe" goto play',
        '"updater_skhid.exe" ready',
        "if errorlevel 1 goto not_ready",
        ":play",
        'start "" ' + launch_line("%SERVER%", embedded),
        "exit /b 0",
        ":not_ready",
        "echo Skhid has not finished updating the game's files (game\\resources.db.patch_*),",
        "echo so the game is not started. Start Skhid.bat (the launcher): it finishes",
        "echo the update. Details: game\\binaries\\x86\\updater_skhid.log",
        "pause",
        "exit /b 1",
        "",
    ]
    return "\r\n".join(lines).encode("ascii")


def starter():
    """`Skhid.bat`, beside the game's `game` folder: one file to double-click
    in the place of a walk to game\\binaries\\x86. It starts the launcher and
    nothing else; a player who unpacked the archive somewhere else is told
    where it belongs."""
    launcher = f"%~dp0game\\{EXE_DIR_WINDOWS}\\{L.LAUNCHER_NAME}"
    lines = [
        "@echo off",
        "rem Starts the Skhid launcher. This file belongs in the game's folder,",
        "rem beside the folder named game.",
        f'if not exist "{launcher}" goto missing',
        f'start "" /d "%~dp0game\\{EXE_DIR_WINDOWS}" "{launcher}"',
        "exit /b 0",
        ":missing",
        f"echo {L.LAUNCHER_NAME} was not found in",
        f'echo   "%~dp0game\\{EXE_DIR_WINDOWS}"',
        "echo.",
        "echo Unpack the Skhid files into the game's folder, the one that holds the",
        "echo folder named game, and start Skhid.bat from there.",
        "echo.",
        "pause",
        "exit /b 1",
        "",
    ]
    return "\r\n".join(lines).encode("ascii")


def crlf(data):
    """A text for Windows: CRLF line ends, whatever the checkout's."""
    return data.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")


def read_downloads(path):
    """The rows of a release's list (release/patches.tsv): `<name> <sha256>
    <size> <urls>`, tabs between, the URLs one space apart; `#` starts a
    comment. Each as the line Skhid-without-updater.bat reads at its end."""
    lines = []
    for number, line in enumerate(Path(path).read_text(encoding="ascii").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if (len(fields) != 4 or not re.fullmatch(r"[A-Za-z0-9._-]+", fields[0])
                or not re.fullmatch(r"[0-9a-f]{64}", fields[1]) or not re.fullmatch(r"[1-9][0-9]*", fields[2])
                or not re.fullmatch(r"https?://[!-~]+( https?://[!-~]+)*", fields[3])):
            sys.exit(f"{path}:{number}: not <name> <sha256> <size> <urls> with tabs between")
        lines.append(f"#skhid-file {' '.join(fields)}")
    return lines


def without_updater(downloads):
    """`Skhid-without-updater.bat` with `downloads` (lines of read_downloads)
    at its end, in ASCII with CRLF line ends."""
    text = Path(HERE, WITHOUT_UPDATER).read_bytes().replace(b"\r\n", b"\n")
    if downloads:
        text = text.rstrip(b"\n") + b"\n" + "".join(line + "\n" for line in downloads).encode("ascii")
    return crlf(text)


def vpn_help():
    """`VPN.txt`: UTF-8 with a byte order mark, so that an old Notepad reads
    its Russian half as UTF-8."""
    return b"\xef\xbb\xbf" + crlf(Path(HERE, VPN_HELP).read_bytes().removeprefix(b"\xef\xbb\xbf"))


def russian_script(name):
    """`Skhid-network-setup.cmd`, `Skhid-defender-setup.cmd` or
    `Skhid-collect-logs.cmd`: kept in
    UTF-8, shipped in code page 866 (DOS Cyrillic) with CRLF line ends. cmd
    reads each line of a script in the console's code page at the time; the
    script switches it to 866 (`chcp 866`) before its first Russian line, so
    everything above that line is ASCII."""
    text = Path(HERE, name).read_bytes().decode("utf-8")
    script = crlf(text.encode("cp866"))
    switch = script.index(b"\r\nchcp 866 >nul 2>&1\r\n")
    script[:switch].decode("ascii")
    return script


def sh(server, embedded=False):
    with open(os.path.join(HERE, "play_skhid.sh"), "rb") as source:
        # Windows checkouts may use CRLF; Bash and the substitutions need LF.
        script = source.read().replace(b"\r\n", b"\n")
    return script.replace(
        b"server=\n", f"server={'' if embedded else server}\n".encode("ascii"), 1)


def readme(server, description, files, embedded=False, site=None):
    """`site`: the bundle has the launcher and its updater, which ask it."""
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
    ]
    if site:
        lines += [
            "1. Unpack this archive into a new folder (or into an existing Survarium",
            "   folder, beside its `game` folder). Allow about 10 GB of free space",
            "   for a new installation. In an existing game, two files are replaced:",
            "   game\\binaries\\x86\\bugtrap.dll and",
            "   game\\resources\\ssl\\survarium_login_server.crt. Back them up first if",
            "   you want the originals. The game's own survarium.exe is not changed.",
            f"2. Windows: double-click {STARTER} in that folder (Windows may show it as",
            f"   \"{STARTER.rsplit('.', 1)[0]}\"). The launcher opens: choose your language at its top and",
            "   press PLAY. Missing original game files are downloaded automatically",
            "   from Google Drive (2.9 GB), verified with a pinned SHA-256, and installed.",
            "   No Google sign-in is needed. The download is unpacked with 7zr.exe,",
            "   which is in this archive.",
            "   If Windows warns about an unknown publisher, let it run:",
            "   these files are not signed.",
            f"   If Microsoft Defender blocks or deletes the game's files, run {DEFENDER_SETUP}:",
            "   it excludes game\\binaries\\x86 from its scans and puts back the files",
            "   it deleted.",
            "   If your antivirus deletes updater_skhid.exe, start",
            f"   {WITHOUT_UPDATER} instead: it installs and updates the game itself.",
            "   Linux: install Wine and Winetricks, open a terminal in that folder and",
            f"   run bash {LINUX_STARTER} (the game) or bash {LINUX_STARTER} launcher (the launcher,",
            "   which downloads a missing game and looks for updates first). For a new",
            "   installation, use the launcher first. See LINUX.txt for setup and",
            "   troubleshooting.",
            "3. Sign in with any user name and password. A new name is registered on its",
            "   first sign-in; remember the password.",
            "",
            "You download this archive once: each time the launcher starts, it looks",
            "for newer Skhid files and installs them by itself.",
            "",
            "CANNOT REACH THE SERVER?",
            f"Allow the game in your firewall: run {NETWORK_SETUP} (Windows, in",
            "Russian) and answer Yes when Windows asks for administrator rights. In",
            f"Russia, use a VPN and add the game's .exe files to it: see {VPN_HELP}.",
            "",
            "STILL NOT WORKING?",
            f"Run {COLLECT_LOGS}: it puts the game's logs and the state of its",
            "folder into one ZIP on your Desktop (no password, nothing changed). Send",
            "that file to #bugreports in Discord: https://discord.gg/Pxa4prXfwu",
            "",
            "WHAT THE ARCHIVE ADDS",
            f"{STARTER} beside the `game` folder: it starts the launcher, nothing else.",
            f"{WITHOUT_UPDATER} beside it: installs, updates and starts the game without",
            "the launcher and the updater.",
            f"{LINUX_STARTER} beside it, for Linux: it runs play_skhid.sh, nothing else.",
            "In game\\binaries\\x86: survarium_skhid.exe (the game for this server),",
            "launcher_skhid.exe, updater_skhid.exe with updater_skhid.url, the folder",
            "launcher_skhid (the launcher's own pages), and play_skhid.bat and",
            "play_skhid.sh, which start the game alone, without looking for updates.",
            "7zr.exe is the standalone extractor of 7-Zip, unchanged: the updater",
            "unpacks the original game with it, and runs it only if it is exactly that",
            "file. 7-Zip Copyright (C) 1999-2025 Igor Pavlov. It is free software under",
            "the GNU Lesser General Public License, version 2.1 or later, and comes",
            "WITHOUT ANY WARRANTY: https://www.7-zip.org/license.txt (the licence),",
            "https://www.7-zip.org (the source code), http://www.gnu.org/ (the LGPL).",
            "",
            "THE LAUNCHER AND ITS UPDATES",
            "launcher_skhid.exe is the game's own launcher, changed to work without an",
            "installer and to ask this server only. Each time it starts, updater_skhid.exe",
            f"asks {site} whether the files of this archive are still the",
            "current ones. If not, it downloads the current archive, checks it and",
            "replaces the files, the launcher and itself included; nothing else of the",
            "existing game is touched. A missing game is installed from the verified",
            "original archive first. It needs no administrator rights. What it did is in",
            "game\\binaries\\x86\\updater_skhid.log. If it cannot reach the server but the",
            "files are complete, PLAY still works.",
            "The game's language is chosen in its Settings (Language), the launcher's",
            "at its top. A changed play_skhid.bat or play_skhid.sh is kept until the",
            "server offers a newer archive.",
            "",
        ]
    else:
        lines += [
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
        ]
    lines += [
        "The game's language is chosen in its Settings (Language); it applies at the",
        "next start. On Linux, set server=host:port near the top of play_skhid.sh",
        "to override the login address." if site
        else "to override the embedded login address.",
        "",
        "OPTIONS (in play_skhid.bat)",
        (f"  Default server: {server} (embedded in survarium_skhid.exe)" if embedded
         else f"  Server: {server} (SERVER in play_skhid.bat)"),
        "  -server=         override the login server (host:port)",
        "  -mt_allocator=768, -minimum_resources_memory_size=768",
        "                   memory for the game, MB (maps need more than the default)",
        "",
        "The rules of the server are on its page:",
        f"https://{server.split(':')[0]}",
        "",
        "survarium_skhid.exe is the game's survarium.exe patched to work with this",
        "server. " + ("The updater of the original game is not used." if site
                      else "The launcher and the updater of the original game are not needed."),
        "",
        "CRASH REPORTS",
        "Crash dumps are saved, zipped (SRP-crash-....zip), beside",
        "survarium_skhid.exe, or in the Windows temporary folder if that folder",
        "cannot be written. The crash dialog shows the file and its location:",
        "upload it in SRP Discord. Nothing is uploaded automatically.",
        "",
    ]
    return "\r\n".join(lines).encode("utf-8")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    options = [a for a in sys.argv[1:] if a.startswith("--")]
    mode, check, server = "section", False, None
    bugtrap_path = BUGTRAP_DLL
    archive = False
    launcher, updater_path, site = True, UPDATER_EXE, None
    extractor_path = EXTRACTOR_EXE
    late_join, revision, required = True, None, None
    downloads = []
    for option in options:
        if option == "--check":
            check = True
        elif option == "--no-late-join":
            late_join = False
        elif option.startswith("--revision="):
            revision = P.parse_revision(option[len("--revision="):])
        elif option.startswith("--required-patches="):
            required = P.parse_required_patches(option[len("--required-patches="):])
        elif option.startswith("--downloads="):
            downloads += read_downloads(option[len("--downloads="):])
        elif option == "--pack-zip":
            archive = True
        elif option == "--cert-only":
            mode = "cert-only"
        elif option.startswith("--cave=") and option[7:] in P.CAVES:
            mode = option[7:]
        elif option.startswith("--server="):
            server = parse_server(option[9:])
        elif option.startswith("--bugtrap="):
            bugtrap_path = option[len("--bugtrap="):]
        elif option.startswith("--updater="):
            updater_path = option[len("--updater="):]
        elif option.startswith("--extractor="):
            extractor_path = option[len("--extractor="):]
        elif option.startswith("--site="):
            site = L.parse_site(option[len("--site="):])
        elif option == "--no-launcher":
            launcher = False
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
    # The late join is of the default build alone (patch revision 5).
    late_join_code = Path(P.late_join_bin()).read_bytes() if mode == "section" and late_join else None
    if (revision is not None or required is not None) and late_join_code is None:
        sys.exit("--revision and --required-patches go with the default build only "
                 "(not --no-late-join, --cave=hash-mismatch or --cert-only)")
    exe, description = P.build(original, mode, code, server if embedded else None, late_join_code, revision=revision,
                               required_patches=required)
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
        (os.path.join("game", EXE_DIR, "play_skhid.bat"), bat(server, embedded)),
        (LINUX_LAUNCHER, sh(server, embedded)),
        (os.path.join("game", CERT_REL), srp_cert),
        ("LINUX.txt", Path(HERE, "LINUX.txt").read_bytes()),
    ]
    launcher_description = None
    if launcher:
        # The launcher: patch_launcher_026g.py refuses anything but the
        # original. It starts the exe and asks the site named here.
        site = site or L.parse_site("https://" + server.split(":")[0])
        launcher_path = Path(source, EXE_DIR, "survarium_launcher.exe")
        if not launcher_path.is_file():
            sys.exit(f"{launcher_path}: the game folder has no launcher (--no-launcher leaves it out)")
        launcher_exe, launcher_description = L.build(
            launcher_path.read_bytes(), Path(L.default_bin()).read_bytes(), site=site,
            game_exe="survarium_skhid.exe", arguments=game_arguments(server, embedded))
        files += [
            (os.path.join("game", EXE_DIR, L.LAUNCHER_NAME), bytes(launcher_exe)),
            (os.path.join("game", EXE_DIR, L.UPDATER_NAME), read_updater(updater_path)),
            (os.path.join("game", EXE_DIR, EXTRACTOR_NAME), read_extractor(extractor_path)),
            (os.path.join("game", EXE_DIR, "updater_skhid.url"), (site + "\r\n").encode("ascii")),
            (STARTER, starter()),
            (WITHOUT_UPDATER, without_updater(downloads)),
            # Windows checkouts may use CRLF; Bash needs LF.
            (LINUX_STARTER, Path(HERE, LINUX_STARTER).read_bytes().replace(b"\r\n", b"\n")),
            (VPN_HELP, vpn_help()),
            (NETWORK_SETUP, russian_script(NETWORK_SETUP)),
            (DEFENDER_SETUP, russian_script(DEFENDER_SETUP)),
            (COLLECT_LOGS, russian_script(COLLECT_LOGS)),
        ]
        files += [(os.path.join("game", EXE_DIR, L.PAGES_DIR, name), data)
                  for name, data in sorted(launcher_pages.pages(site).items())]
    listed = [(name.replace(os.sep, "/"), sha256(data)) for name, data in files]
    files.append(("README.txt", readme(server, description, listed, embedded,
                                       site if launcher else None)))

    if not check and archive:
        os.makedirs(os.path.dirname(output), exist_ok=True)
        with zipfile.ZipFile(output, "x", zipfile.ZIP_DEFLATED) as out:
            for name, data in files:
                entry = zipfile.ZipInfo(name.replace(os.sep, "/"))
                entry.create_system = 3
                entry.external_attr = (0o100755 if name in SCRIPTS else 0o100644) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                out.writestr(entry, data)
    for name, data in files:
        target = (output + "!/" + name.replace(os.sep, "/") if archive else os.path.join(output, name))
        if not check and not archive:
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "xb") as out:   # never over a file that is there
                out.write(data)
            if name in SCRIPTS:
                os.chmod(target, 0o755)
        print(f"{'would write' if check else 'wrote'} {target} ({len(data)} bytes, sha256 {sha256(data)})")
    print(f"exe: {description}")
    if launcher_description:
        print(f"launcher: {launcher_description}")
    print("launch line: " + launch_line(server, embedded))
    if check:
        print("--check: nothing was written")


if __name__ == "__main__":
    main()
