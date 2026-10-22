"""Tests of skhid-steam.ps1 in PowerShell 7 on Linux, against a made-up
Steam folder: its text VDF (localconfig.vdf, libraryfolders.vdf,
loginusers.vdf) and binary VDF (shortcuts.vdf) reading and writing, checked
with the independent readers and writers here, and both modes end to end.

    nix shell nixpkgs#python3 nixpkgs#powershell -c python3 client/steam/test_skhid_steam.py [-v]

Not covered here (Windows only): the registry lookup of Steam, the .bat
files, Windows PowerShell 5.1 itself, and Steam reading what was written.
"""

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "skhid-steam.ps1")
PWSH = shutil.which("pwsh")
APP = "355840"
APP_PATH = ["UserLocalConfigStore", "Software", "Valve", "Steam", "apps", APP]
FAKE_STEAM = "skhidfakesteam"


# ------------------------------------------------------------ text VDF oracle

def parse_text_vdf(data):
    """bytes -> nested dict {key: str | dict} (keys as written)."""
    text = data.decode("utf-8")
    pos = 0

    def token():
        nonlocal pos
        while pos < len(text):
            if text[pos] in " \t\r\n":
                pos += 1
            elif text.startswith("//", pos):
                pos = text.find("\n", pos) if "\n" in text[pos:] else len(text)
            elif text[pos] == "[":
                pos = text.index("]", pos) + 1
            else:
                break
        if pos >= len(text):
            return None
        c = text[pos]
        if c in "{}":
            pos += 1
            return c
        if c == '"':
            out, pos = [], pos + 1
            while text[pos] != '"':
                if text[pos] == "\\":
                    pos += 1
                    out.append({"n": "\n", "t": "\t", "\\": "\\", '"': '"'}.get(text[pos], "\\" + text[pos]))
                else:
                    out.append(text[pos])
                pos += 1
            pos += 1
            return ("s", "".join(out))
        start = pos
        while pos < len(text) and text[pos] not in ' \t\r\n{}"':
            pos += 1
        return ("s", text[start:pos])

    def body(nested):
        node = {}
        while True:
            key = token()
            if key is None:
                assert not nested, "cut short"
                return node
            if key == "}":
                assert nested
                return node
            value = token()
            node[key[1]] = body(True) if value == "{" else value[1]

    return body(False)


def vdf_get(tree, path, key=None):
    for p in path + ([key] if key else []):
        match = [k for k in tree if k.lower() == p.lower()]
        if not match:
            return None
        tree = tree[match[0]]
    return tree


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def localconfig(app_block, nl="\n", persona="Hone"):
    """A localconfig.vdf as Steam writes it; app_block is the text of
    apps/355840's lines (None: no such block)."""
    lines = [
        '"UserLocalConfigStore"', "{",
        '\t"friends"', "\t{", f'\t\t"PersonaName"\t\t"{esc(persona)}"', "\t}",
        "\t// a comment", '\t"Software"', "\t{", '\t\t"Valve"', "\t\t{", '\t\t\t"Steam"', "\t\t\t{",
        '\t\t\t\t"apps"', "\t\t\t\t{",
        '\t\t\t\t\t"440"', "\t\t\t\t\t{", '\t\t\t\t\t\t"LaunchOptions"\t\t"-novid \\"C:\\\\x y\\""',
        '\t\t\t\t\t\t"LastPlayed"\t\t"1700000000"', "\t\t\t\t\t}",
    ]
    if app_block is not None:
        lines += [f'\t\t\t\t\t"{APP}"', "\t\t\t\t\t{"] + ["\t\t\t\t\t\t" + l for l in app_block] + ["\t\t\t\t\t}"]
    lines += ["\t\t\t\t}", '\t\t\t\t"Other"\t\t"1" [$WIN32]', "\t\t\t}", "\t\t}", "\t}", "}", ""]
    return nl.join(lines).encode("utf-8")


# ---------------------------------------------------------- binary VDF oracle

def bin_write(entries):
    """entries: list of (type, key, value); 0 maps hold lists."""
    out = bytearray()
    for t, k, v in entries:
        out += bytes([t]) + k.encode("utf-8") + b"\0"
        if t == 0:
            out += bin_write(v)
        elif t == 1:
            out += v.encode("utf-8") + b"\0"
        elif t == 2:
            out += struct.pack("<i", v)
        elif t == 3:
            out += struct.pack("<f", v)
        elif t == 7:
            out += struct.pack("<Q", v)
    return bytes(out + b"\x08")


def bin_read(data):
    pos = 0

    def cstr():
        nonlocal pos
        end = data.index(b"\0", pos)
        s, pos = data[pos:end].decode("utf-8"), end + 1
        return s

    def body():
        nonlocal pos
        out = []
        while True:
            t = data[pos]
            pos += 1
            if t == 8:
                return out
            k = cstr()
            if t == 0:
                v = body()
            elif t == 1:
                v = cstr()
            elif t == 2:
                v, = struct.unpack_from("<i", data, pos)
                pos += 4
            elif t == 3:
                v, = struct.unpack_from("<f", data, pos)
                pos += 4
            elif t == 7:
                v, = struct.unpack_from("<Q", data, pos)
                pos += 8
            else:
                raise ValueError(t)
            out.append((t, k, v))

    tree = body()
    assert pos == len(data), "trailing data"
    return tree


def as_dict(entries):
    return {k: v for _, k, v in entries}


def other_shortcut(name, exe):
    return [(2, "appid", -123456), (1, "AppName", name), (1, "Exe", exe), (1, "StartDir", '"C:\\"'),
            (1, "icon", ""), (2, "IsHidden", 0), (3, "Weight", 1.5), (7, "Big", 2**63 + 5),
            (2, "LastPlayTime", 1690000000), (0, "tags", [(1, "0", "favorite")])]


def shortcuts_file(*maps):
    return bin_write([(0, "shortcuts", [(0, str(i), m) for i, m in enumerate(maps)])])


# ------------------------------------------------------------------ helpers

def pwsh(code, *args):
    """Runs code with the script dot-sourced; $args[i] are the arguments."""
    with tempfile.TemporaryDirectory(prefix="skhid-steam-ps-") as d:
        wrapper = os.path.join(d, "t.ps1")
        with open(wrapper, "w", encoding="utf-8-sig") as f:
            f.write(f". '{SCRIPT}'\n{code}\n")
        r = subprocess.run([PWSH, "-NoProfile", "-NonInteractive", "-File", wrapper] + list(args),
                           capture_output=True, text=True, timeout=120)
    if r.returncode != 0:
        raise AssertionError(f"pwsh failed ({r.returncode}):\n{r.stdout}\n{r.stderr}")
    return r.stdout


class Tmp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="skhid-steam-")
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def path(self, *parts):
        return os.path.join(self.tmp, *parts)

    def write(self, data, *parts):
        p = self.path(*parts)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            f.write(data)
        return p

    def read(self, *parts):
        with open(self.path(*parts), "rb") as f:
            return f.read()


@unittest.skipUnless(PWSH, "pwsh is not on PATH (nix shell nixpkgs#powershell)")
class TextVdf(Tmp):
    def set_value(self, src, value):
        p = self.write(src, "in.vdf")
        pwsh("Write-ByteText $args[0] (Set-TextVdfValue (Read-ByteText $args[0]) $AppPath 'LaunchOptions' "
             "(ConvertTo-ByteString $args[1]))", p, value)
        return self.read("in.vdf")

    def remove_value(self, src):
        p = self.write(src, "in.vdf")
        pwsh("Write-ByteText $args[0] (Remove-TextVdfValue (Read-ByteText $args[0]) $AppPath 'LaunchOptions')", p)
        return self.read("in.vdf")

    def test_read_matches_oracle(self):
        src = localconfig(['"LaunchOptions"\t\t"-dx9"'], persona='Хон "the" \\ one')
        p = self.write(src, "in.vdf")
        out = pwsh("$r = Read-TextVdf (Read-ByteText $args[0]); @{"
                   "  p = ConvertFrom-ByteString (Get-TextVdfValue $r @('UserLocalConfigStore','friends') 'PersonaName');"
                   "  a = Get-TextVdfValue $r @('UserLocalConfigStore','Software','Valve','Steam','apps','440') 'LaunchOptions';"
                   "  s = Get-TextVdfValue $r $AppPath 'LaunchOptions';"
                   "  o = Get-TextVdfValue $r @('UserLocalConfigStore','Software','Valve','Steam') 'other' } | ConvertTo-Json", p)
        got = json.loads(out)
        tree = parse_text_vdf(src)
        self.assertEqual(got["p"], vdf_get(tree, ["UserLocalConfigStore", "friends"], "PersonaName"))
        self.assertEqual(got["p"], 'Хон "the" \\ one')
        self.assertEqual(got["a"], '-novid "C:\\x y"')
        self.assertEqual(got["s"], "-dx9")
        self.assertEqual(got["o"], "1")

    def test_replace_changes_only_that_value(self):
        src = localconfig(['"LastPlayed"\t\t"1"', '"LaunchOptions"\t\t"-dx9"', '"Playtime"\t\t"5"'])
        value = '"C:\\Ігри\\Skhid\\skhid-steam-run.bat" %command%'
        out = self.set_value(src, value)
        self.assertEqual(vdf_get(parse_text_vdf(out), APP_PATH, "LaunchOptions"), value)
        diff = [(a, b) for a, b in zip(src.split(b"\n"), out.split(b"\n")) if a != b]
        self.assertEqual(len(src.split(b"\n")), len(out.split(b"\n")))
        self.assertEqual(diff, [(b'\t\t\t\t\t\t"LaunchOptions"\t\t"-dx9"',
                                b'\t\t\t\t\t\t"LaunchOptions"\t\t"\\"C:\\\\' + "Ігри".encode() +
                                b'\\\\Skhid\\\\skhid-steam-run.bat\\" %command%"')])
        # The old value again: the original bytes.
        self.assertEqual(self.set_value(out, "-dx9"), src)

    def test_insert_then_remove_is_identity(self):
        for nl in ("\n", "\r\n"):
            with self.subTest(nl=repr(nl)):
                src = localconfig(['"LastPlayed"\t\t"1"'], nl=nl)
                out = self.set_value(src, '"/x/skhid-steam-run.bat" %command%')
                self.assertEqual(vdf_get(parse_text_vdf(out), APP_PATH, "LaunchOptions"),
                                 '"/x/skhid-steam-run.bat" %command%')
                self.assertIn(f'\t\t\t\t\t\t"LastPlayed"\t\t"1"{nl}\t\t\t\t\t\t"LaunchOptions"\t\t'.encode(), out)
                self.assertNotIn(b"\r\n" if nl == "\n" else b"\r\r", out)
                self.assertEqual(self.remove_value(out), src)

    def test_missing_blocks_are_made(self):
        src = localconfig(None)
        out = self.set_value(src, "x skhid-steam-run.bat")
        tree = parse_text_vdf(out)
        self.assertEqual(vdf_get(tree, APP_PATH, "LaunchOptions"), "x skhid-steam-run.bat")
        self.assertEqual(vdf_get(tree, APP_PATH[:-1] + ["440"], "LastPlayed"), "1700000000")
        self.assertIn(f'\t\t\t\t\t"{APP}"\n\t\t\t\t\t{{\n\t\t\t\t\t\t"LaunchOptions"\t\t"x skhid-steam-run.bat"\n'
                      f'\t\t\t\t\t}}\n\t\t\t\t}}'.encode(), out)
        removed = parse_text_vdf(self.remove_value(out))
        self.assertIsNone(vdf_get(removed, APP_PATH, "LaunchOptions"))
        # No apps block at all (Steam spells it Apps in some versions).
        src = b'"UserLocalConfigStore"\n{\n\t"Software"\n\t{\n\t\t"Valve"\n\t\t{\n\t\t\t"Steam"\n\t\t\t{\n' \
              b'\t\t\t\t"Apps"\n\t\t\t\t{\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n'
        self.assertEqual(vdf_get(parse_text_vdf(self.set_value(src, "v")), APP_PATH, "LaunchOptions"), "v")
        src = b'"UserLocalConfigStore"\n{\n}\n'
        self.assertEqual(vdf_get(parse_text_vdf(self.set_value(src, "v")), APP_PATH, "LaunchOptions"), "v")

    def test_unreadable_files_are_refused(self):
        for bad in (b'"UserLocalConfigStore"\n{\n\t"a"\t"b"\n', b'"a" "b" }', b'"a" { "b" "c" } "x', b""):
            with self.subTest(bad=bad):
                p = self.write(bad, "in.vdf")
                r = subprocess.run([PWSH, "-NoProfile", "-NonInteractive", "-Command",
                                    f". '{SCRIPT}'; Set-TextVdfValue (Read-ByteText '{p}') $AppPath 'LaunchOptions' 'v'"],
                                   capture_output=True, text=True)
                self.assertNotEqual(r.returncode, 0)
                self.assertEqual(self.read("in.vdf"), bad)


@unittest.skipUnless(PWSH, "pwsh is not on PATH (nix shell nixpkgs#powershell)")
class BinaryVdf(Tmp):
    def test_round_trip_is_exact(self):
        src = shortcuts_file(other_shortcut("Doom", '"C:\\doom.exe"'), other_shortcut("Сталкер ☢", '"D:\\s.exe"'))
        self.write(src, "s.vdf")
        pwsh("[IO.File]::WriteAllBytes($args[1], (Write-BinaryVdf (Read-BinaryVdf ([IO.File]::ReadAllBytes($args[0])))))",
             self.path("s.vdf"), self.path("out.vdf"))
        self.assertEqual(self.read("out.vdf"), src)
        self.assertEqual(bin_read(src)[0][2][1][2][1], (1, "AppName", "Сталкер ☢"))

    def test_empty_and_bad(self):
        src = shortcuts_file()
        self.write(src, "s.vdf")
        pwsh("[IO.File]::WriteAllBytes($args[0], (Write-BinaryVdf (Read-BinaryVdf ([IO.File]::ReadAllBytes($args[0])))))",
             self.path("s.vdf"))
        self.assertEqual(self.read("s.vdf"), src)
        for bad in (src[:-1], src + b"\0", b"\x00shortcuts\x00\x05x\x00ab\x00\x08\x08"):
            with self.subTest(bad=bad):
                p = self.write(bad, "b.vdf")
                r = subprocess.run([PWSH, "-NoProfile", "-NonInteractive", "-Command",
                                    f". '{SCRIPT}'; Read-BinaryVdf ([IO.File]::ReadAllBytes('{p}'))"],
                                   capture_output=True, text=True)
                self.assertNotEqual(r.returncode, 0)

    def test_crc_app_id(self):
        exe = '"C:\\Ігри\\skhid-steam-run.bat"'
        out = pwsh("Get-ShortcutAppId (ConvertTo-ByteString $args[0]) 'Survarium'", exe)
        want = (zlib.crc32((exe + "Survarium").encode()) | 0x80000000) - 2**32
        self.assertEqual(int(out), want)


# --------------------------------------------------------------- end to end

@unittest.skipUnless(PWSH, "pwsh is not on PATH (nix shell nixpkgs#powershell)")
class EndToEnd(Tmp):
    USERS = {"111": "Hone", "222": "Other"}

    def setUp(self):
        super().setUp()
        self.steam = self.path("Steam")
        self.game = self.path("Skhid")
        self.write(b"MZ", "Skhid", "game", "binaries", "x86", "launcher_skhid.exe")
        self.write(b"MZ", "Skhid", "game", "binaries", "x86", "survarium_skhid.exe")
        for name in ("skhid-steam.ps1", "skhid-steam-run.bat"):
            shutil.copy(os.path.join(HERE, name), self.game)
        self.run_bat = os.path.join(self.game, "skhid-steam-run.bat")
        lib = self.path("Library2")
        self.write(f'"libraryfolders"\n{{\n\t"0"\n\t{{\n\t\t"path"\t\t"{esc(self.steam)}"\n\t}}\n'
                   f'\t"1"\n\t{{\n\t\t"path"\t\t"{esc(lib)}"\n\t\t"apps"\n\t\t{{\n\t\t}}\n\t}}\n}}\n'.encode(),
                   "Steam", "steamapps", "libraryfolders.vdf")
        os.makedirs(os.path.join(lib, "steamapps"))
        login = "".join(f'\t"{76561197960265728 + int(i)}"\n\t{{\n\t\t"AccountName"\t\t"a{i}"\n'
                        f'\t\t"PersonaName"\t\t"{n}"\n\t}}\n' for i, n in self.USERS.items())
        self.write(f'"users"\n{{\n{login}}}\n'.encode(), "Steam", "config", "loginusers.vdf")
        self.original = {}
        for i in self.USERS:
            self.original[i] = localconfig(['"LastPlayed"\t\t"1"', '"LaunchOptions"\t\t"-dx9"'] if i == "111"
                                           else ['"LastPlayed"\t\t"2"'])
            self.write(self.original[i], "Steam", "userdata", i, "config", "localconfig.vdf")
        os.makedirs(self.path("Steam", "userdata", "0"))
        self.library2 = lib

    def install_survarium(self):
        self.write(b'"AppState"\n{\n\t"appid"\t\t"355840"\n}\n', "Library2", "steamapps", f"appmanifest_{APP}.acf")

    def script(self, *args, stdin="", ok=True, script_dir=None):
        script = os.path.join(script_dir or self.game, "skhid-steam.ps1")
        r = subprocess.run([PWSH, "-NoProfile", "-File", script, "-SteamPath", self.steam,
                            "-SteamProcessName", FAKE_STEAM] + list(args),
                           input=stdin, capture_output=True, text=True, timeout=120)
        if ok and r.returncode != 0:
            raise AssertionError(f"script failed ({r.returncode}):\n{r.stdout}\n{r.stderr}")
        return r

    def localconfig(self, user):
        return self.read("Steam", "userdata", user, "config", "localconfig.vdf")

    def shortcuts(self, user):
        return self.path("Steam", "userdata", user, "config", "shortcuts.vdf")

    # Mode 1 ---------------------------------------------------------------

    def test_installed_sets_and_restores_launch_options(self):
        self.install_survarium()
        out = self.script("-User", "111").stdout
        self.assertIn("Hone (111): Play on Survarium now starts Skhid.", out)
        want = f'"{self.run_bat}" %command%'
        self.assertEqual(vdf_get(parse_text_vdf(self.localconfig("111")), APP_PATH, "LaunchOptions"), want)
        self.assertEqual(self.localconfig("222"), self.original["222"])
        backup = self.path("Steam", "userdata", "111", "config", "localconfig.vdf.before-skhid")
        with open(backup, "rb") as f:
            self.assertEqual(f.read(), self.original["111"])
        self.assertFalse(os.path.exists(self.shortcuts("111")))
        # Again: nothing changes, the backup stays the one before Skhid.
        changed = self.localconfig("111")
        self.assertIn("already set", self.script("-User", "111").stdout)
        self.assertEqual(self.localconfig("111"), changed)
        with open(backup, "rb") as f:
            self.assertEqual(f.read(), self.original["111"])
        # Undo: the launch options before Skhid, byte for byte.
        out = self.script("-Remove", "-User", "111").stdout
        self.assertIn("Survarium launch options restored", out)
        self.assertEqual(self.localconfig("111"), self.original["111"])
        self.assertIn("nothing to remove", self.script("-Remove", "-User", "111").stdout)

    def test_installed_all_users_without_launch_options_before(self):
        self.install_survarium()
        out = self.script("-User", "all").stdout
        for i in self.USERS:
            self.assertIn(f"({i}): Play on Survarium", out)
            self.assertEqual(vdf_get(parse_text_vdf(self.localconfig(i)), APP_PATH, "LaunchOptions"),
                             f'"{self.run_bat}" %command%')
        self.script("-Remove", "-Yes")
        for i in self.USERS:
            self.assertEqual(self.localconfig(i), self.original[i])

    def test_old_libraryfolders_format(self):
        self.write(f'"LibraryFolders"\n{{\n\t"TimeNextStatsReport"\t\t"1"\n\t"ContentStatsID"\t\t"-1"\n'
                   f'\t"1"\t\t"{esc(self.library2)}"\n}}\n'.encode(), "Steam", "steamapps", "libraryfolders.vdf")
        self.install_survarium()
        self.assertIn("Play on Survarium", self.script("-User", "222").stdout)

    # Mode 2 ---------------------------------------------------------------

    def test_not_installed_adds_and_removes_shortcut(self):
        out = self.script("-User", "111").stdout
        self.assertIn("Hone (111): 'Survarium' added to the library (non-Steam game).", out)
        self.assertEqual(self.localconfig("111"), self.original["111"])
        with open(self.shortcuts("111"), "rb") as f:
            tree = bin_read(f.read())
        self.assertEqual([k for _, k, _ in tree], ["shortcuts"])
        [(t, key, fields)] = tree[0][2]
        self.assertEqual((t, key), (0, "0"))
        d = as_dict(fields)
        exe = f'"{self.run_bat}"'
        self.assertEqual(d["AppName"], "Survarium")
        self.assertEqual(d["Exe"], exe)
        self.assertEqual(d["StartDir"], f'"{self.game}/"')
        self.assertEqual(d["icon"], os.path.join(self.game, "game", "binaries", "x86", "survarium_skhid.exe"))
        self.assertEqual(d["appid"], (zlib.crc32((exe + "Survarium").encode()) | 0x80000000) - 2**32)
        self.assertEqual(d["AllowOverlay"], 1)
        self.assertEqual(d["tags"], [])
        self.assertIn("already in the library", self.script("-User", "111").stdout)
        self.assertIn("shortcut removed", self.script("-Remove", "-User", "111").stdout)
        with open(self.shortcuts("111"), "rb") as f:
            self.assertEqual(f.read(), shortcuts_file())

    def test_shortcut_beside_others(self):
        a, b = other_shortcut("Doom", '"C:\\doom.exe"'), other_shortcut("Сталкер", '"D:\\s.exe"')
        src = shortcuts_file(a, b)
        self.write(src, "Steam", "userdata", "222", "config", "shortcuts.vdf")
        self.script("-User", "222")
        with open(self.shortcuts("222"), "rb") as f:
            entries = bin_read(f.read())[0][2]
        self.assertEqual([k for _, k, _ in entries], ["0", "1", "2"])
        self.assertEqual(entries[:2], bin_read(src)[0][2])
        with open(self.shortcuts("222") + ".before-skhid", "rb") as f:
            self.assertEqual(f.read(), src)
        self.script("-Remove", "-User", "222")
        with open(self.shortcuts("222"), "rb") as f:
            self.assertEqual(f.read(), src)
        # Ours in the middle (another shortcut added after it): the rest is renumbered.
        self.script("-User", "222")
        with open(self.shortcuts("222"), "rb") as f:
            ours = bin_read(f.read())[0][2][2][2]
        self.write(shortcuts_file(a, ours, b), "Steam", "userdata", "222", "config", "shortcuts.vdf")
        self.script("-Remove", "-User", "222")
        with open(self.shortcuts("222"), "rb") as f:
            self.assertEqual(f.read(), src)

    def test_moved_files_update_the_shortcut(self):
        self.script("-User", "111")
        moved = self.path("Skhid", "steam")
        shutil.copytree(self.game, moved, ignore=shutil.ignore_patterns("game", "steam"))
        out = self.script("-User", "111", script_dir=moved).stdout
        self.assertIn("shortcut updated to this folder", out)
        with open(self.shortcuts("111"), "rb") as f:
            entries = bin_read(f.read())[0][2]
        self.assertEqual(len(entries), 1)
        self.assertEqual(as_dict(entries[0][2])["Exe"], f'"{os.path.join(moved, "skhid-steam-run.bat")}"')

    # Questions, Steam running, mistakes --------------------------------------

    def test_asks_which_user(self):
        r = self.script(stdin="2\n")
        self.assertIn("1) Hone (111)", r.stdout)
        self.assertIn("2) Other (222)", r.stdout)
        self.assertTrue(os.path.exists(self.shortcuts("222")))
        self.assertFalse(os.path.exists(self.shortcuts("111")))
        r = self.script(stdin="a\nn\n", ok=False)
        self.assertEqual(r.returncode, 1)
        self.assertIn("Nothing was changed.", r.stdout)
        self.assertFalse(os.path.exists(self.shortcuts("111")))
        self.script(stdin="A\ny\n")
        self.assertTrue(os.path.exists(self.shortcuts("111")))
        r = self.script(stdin="7\n", ok=False)
        self.assertIn("No such user", r.stdout)

    def test_waits_for_steam_to_close(self):
        self.install_survarium()
        fake = self.write(b"#!/bin/sh\nsleep 60\n", "bin", FAKE_STEAM)
        os.chmod(fake, 0o755)
        steam = subprocess.Popen([fake])
        try:
            proc = subprocess.Popen([PWSH, "-NoProfile", "-File", os.path.join(self.game, "skhid-steam.ps1"),
                                     "-SteamPath", self.steam, "-SteamProcessName", FAKE_STEAM, "-User", "111"],
                                    stdout=subprocess.PIPE, text=True)
            time.sleep(4)
            self.assertIsNone(proc.poll())
            self.assertEqual(self.localconfig("111"), self.original["111"])
        finally:
            steam.kill()
            steam.wait()
        out, _ = proc.communicate(timeout=60)
        self.assertEqual(proc.returncode, 0)
        self.assertIn("Steam is running. Close it", out)
        self.assertNotEqual(self.localconfig("111"), self.original["111"])

    def test_refuses_without_the_game(self):
        lone = self.path("Elsewhere")
        os.makedirs(lone)
        shutil.copy(SCRIPT, lone)
        r = self.script("-User", "111", script_dir=lone, ok=False)
        self.assertEqual(r.returncode, 1)
        self.assertIn("beside Skhid.bat", r.stdout)
        self.assertFalse(os.path.exists(self.shortcuts("111")))
        r = self.script("-User", "999", ok=False)
        self.assertIn("Steam user 999 was not found", r.stdout)
        os.remove(self.run_bat)
        r = self.script("-User", "111", ok=False)
        self.assertIn("skhid-steam-run.bat is missing", r.stdout)
        self.assertFalse(os.path.exists(self.shortcuts("111")))
        r = subprocess.run([PWSH, "-NoProfile", "-File", SCRIPT, "-SteamPath", self.path("nowhere")],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 1)
        self.assertIn("Steam was not found", r.stdout)


@unittest.skipUnless(PWSH, "pwsh is not on PATH (nix shell nixpkgs#powershell)")
class WaitForGame(Tmp):
    """-WaitForGame (what skhid-steam-run.bat waits with) against stand-in
    processes with the game's names."""

    def proc(self, name, seconds, after=0):
        """A process named `name` from `after` s on, for `seconds` s. It is
        reaped at once when it ends: a zombie still has its name."""
        script = self.write(f"#!/bin/sh\nsleep {seconds}\n".encode(), name, name)
        os.chmod(script, 0o755)
        p = subprocess.Popen(["sh", "-c", f"sleep {after}; exec '{script}'"])
        threading.Thread(target=p.wait, daemon=True).start()
        self.addCleanup(p.wait)
        self.addCleanup(p.kill)
        return p

    def wait(self):
        start = time.monotonic()
        subprocess.run([PWSH, "-NoProfile", "-File", SCRIPT, "-WaitForGame", "-PollSeconds", "0.5"],
                       check=True, timeout=60)
        return time.monotonic() - start

    def test_waits_for_launcher_then_game(self):
        self.proc("launcher_skhid", 3)
        self.proc("survarium_skhid", 6.5, after=2.5)
        took = self.wait()
        self.assertGreaterEqual(took, 9)
        self.assertLess(took, 13)

    def test_launcher_restart_gap_and_updater(self):
        # The launcher closes and the updater opens it again a moment later.
        self.proc("launcher_skhid", 2)
        self.proc("updater_skhid", 1, after=2.2)
        self.proc("launcher_skhid", 2, after=2.4)
        took = self.wait()
        self.assertGreaterEqual(took, 4.4)
        self.assertLess(took, 8)

    def test_launcher_only(self):
        self.proc("launcher_skhid", 3)
        took = self.wait()
        self.assertGreaterEqual(took, 3)
        self.assertLess(took, 7)

    def test_game_closed_ends_it_even_with_launcher_open(self):
        self.proc("launcher_skhid", 30)
        self.proc("survarium_skhid", 3)
        self.assertLess(self.wait(), 7)


if __name__ == "__main__":
    unittest.main()
