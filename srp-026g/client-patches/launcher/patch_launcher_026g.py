"""Builds the Skhid launcher from the ORIGINAL launcher of Survarium 0.26g
(build 2777): the game's own window, made to work for a game that was only
unpacked, to ask one server only, and to start the Skhid updater.

USAGE (this script never touches the source exe):

    cd srp-026g/client-patches/launcher
    python patch_launcher_026g.py <game>/binaries/x86/survarium_launcher.exe <out>/launcher_skhid.exe
        [--site=https://skhid.link]     the server's web page (HTTPS)
        [--game-exe=survarium_skhid.exe] what PLAY starts, beside the launcher
        [--arguments="-mt_allocator=768 -minimum_resources_memory_size=768"]
        [--bin=<file>]                  another launcher_values_026g.bin
    python patch_launcher_026g.py <survarium_launcher.exe> --check [...]
        every check and patch in memory, nothing written
    python selftest_launcher_026g.py
        builds it, re-parses and disassembles every patched site

    <survarium_launcher.exe> must be the ORIGINAL 0.26g launcher (the sha256
    is checked); the output must be another file. The new code comes from
    launcher_values_026g.asm:
        nasm -f bin -DCAVE=0x49A000 launcher_values_026g.asm -o launcher_values_026g.bin
    (the .bin is checked in).

What is changed and why, patch by patch, with the evidence for every
address: README.md beside this file. Each site is compared with the original
bytes first, so another build or an already patched file is refused.
"""

import hashlib
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import patch_026g as P  # noqa: E402  (Headers, pe_checksum: the same PE helpers)

IMAGE_BASE = 0x10000
EXE_SHA256 = "710956d2347864effb353064dd7beaee1dd2773c74756d3f6677df71431c5266"
EXE_SIZE = 4_740_728

# The names the patched launcher uses, all beside itself in game\binaries\x86.
LAUNCHER_NAME = "launcher_skhid.exe"
UPDATER_NAME = "updater_skhid.exe"
PAGES_DIR = "launcher_skhid"
DEFAULT_SITE = "https://skhid.link"
DEFAULT_GAME_EXE = "survarium_skhid.exe"
DEFAULT_ARGUMENTS = "-mt_allocator=768 -minimum_resources_memory_size=768"

# The new section, right after .reloc (the original image ends there).
SECTION_NAME = b".skhid\0\0"
SECTION_RVA = 0x48A000
SECTION_VA = IMAGE_BASE + SECTION_RVA           # 0x49A000, the CAVE of the .bin
SECTION_CHARACTERISTICS = 0x60000020            # code, execute, read (as .text)

# launcher_values_026g.bin: the code, three zero-filled ASCII slots, then the
# language code (LANGUAGE_ENTRIES_AT on) with the generated language table.
SLOTS = {"exe_name": (304, 32), "arguments": (336, 160), "server": (496, 64)}
INSTALL_STATUS_AT = 560
INSTALL_STATUS = 0x16D06

# registry_key::get_string: push ebp / mov ebp,esp / push ecx / lea eax,[esi+esi]
# become a jump to the code of the new section and two nops; the code comes
# back to 0x1402E for every name it does not answer itself.
GET_STRING = (0x14027, bytes.fromhex("558bec518d0436"))
GET_STRING_BACK = 0x1402E

# launcher::execute, the message loop's body: cmp dword [ebp-1Ch],113h
# becomes a jump to `enter_key` of the .bin and two nops. Enter presses PLAY
# there; every other message comes back to the jne after the cmp, and Enter
# goes on to launcher::on_message_processed as a handled message does.
ENTER_KEY = (0x14EF0, bytes.fromhex("817de413010000"))
ENTER_KEY_AT = 224                              # its offset in the .bin
ENTER_KEY_BACK = 0x14EF7
ENTER_KEY_DONE = 0x14F26

# The language is the game's own `localization` line of user.cfg, every
# language of the game in the launcher's menu (launcher_values_026g.asm,
# "The language"). Each site becomes a call or a jump to its entry of
# `language_entries`, 8 bytes each from LANGUAGE_ENTRIES_AT, in this order;
# the relocations of the replaced instructions are disabled.
# (VA, original bytes, "call" or "jmp", what)
LANGUAGE_ENTRIES_AT = 0x260
LANGUAGE_HOOKS = [
    (0x19663, "8b04c580820400", "call", "language_menu: the button's label of every language"),
    (0x150B3, "0fb7344598e00300", "call", "on_language_changed: the LANGID of every language"),
    (0x1A84B, "a19c810400", "jmp", "get_language_code: a page's code for every language"),
    (0x1D852, "558bec5151", "jmp", "settings::reload_language: from user.cfg"),
    (0x1D8DA, "558bec56e8", "jmp", "settings::save_language: to user.cfg"),
    (0x19A08, "568b742408", "jmp", "language_menu: the button opens a menu of every language"),
]
LANGUAGE_RELOCATIONS = [0x19666, 0x150B7, 0x1A84C]

# (VA, original bytes, new bytes, what): code and read-only data, in place.
PATCHES = [
    # A State message leaves id 2 in the install-info field. The native
    # status formatter only has entries for ids 6..9; guard its stack index.
    (INSTALL_STATUS, "8b86ec510000",
     (b"\xe9" + struct.pack("<i", SECTION_VA + INSTALL_STATUS_AT - INSTALL_STATUS - 5) + b"\x90").hex(),
     "refresh_status: validate install-info id before selecting its format"),
    # 1. Run where it is. The original launcher asks its updater to copy both
    # of them to <install>\temp, starts the copy and ends; only the copy shows
    # a window.
    (0x1474A, "0f8558060000", "e95906000090",
     "run_as_copy: jne (a debugger is present -> no copy) becomes jmp: never a copy"),
    (0x14442, "0f84c5000000", "909090909090",
     "launcher::launcher: je (not the temp copy -> no window) removed"),
    # 2. No hardware warnings before the window (RAM, CPU by registry, video
    # memory by WMI): the game checks what it needs itself.
    (0x145C5, "81ec04", "33c0c3",
     "check_hardware: sub esp,304h becomes xor eax,eax / ret"),
    # 3. Never through "Survarium Update Service" (an old installation's
    # service would start the old updater as SYSTEM).
    (0x15521, "7420", "eb20",
     "run_survarium_executable_as_administrator: je (no service asked) becomes jmp"),
    # 4. The installer's values are looked for in the user's own key, which
    # the launcher makes itself for its language setting, not in the
    # machine's, which only an installer could write. (The values themselves
    # come from the new code: patch 5.)
    (0x15231, "6802000080", "6801000080", "get_local_version: HKLM becomes HKCU"),
    (0x159A4, "6802000080", "6801000080", "start_updater, update server: HKLM becomes HKCU"),
    (0x16301, "6802000080", "6801000080", "restart_launcher: HKLM becomes HKCU"),
    (0x170AD, "6802000080", "6801000080", "get_game_launch_parameters: HKLM becomes HKCU"),
    # 6. Buttons that led to the game's old sites, and SHOP, whose exchange
    # is in the lobby now: they do nothing (and are moved out of sight below).
    (0x1BBEF, "55", "c3", "SHOP click: ret"),
    (0x1BC45, "55", "c3", "CREATE ACCOUNT click: ret"),
    (0x1BD03, "83", "c3", "YouTube click: ret"),
    (0x1BD16, "83", "c3", "Facebook click: ret"),
    (0x1BD29, "83", "c3", "Twitter click: ret"),
    (0x1BD3C, "83", "c3", "VK click: ret"),
]

# start_updater, when no version could be read (the game exe PLAY starts is
# not there): the original shows "Can't retrieve first install version or
# local version" and never starts the updater, which is the one program
# that could bring the missing file. The three pushes of that message box
# become: the version is an empty string, go on.
#     and word [ebp-0ECh],0 / jmp 0x157A4 / nop nop
# Two of the pushes hold relocated addresses; their relocations are disabled.
NO_VERSION = (0x1577A, "6a006844e9030068a0ec0300", "6683a514ffffff00eb209090")
NO_VERSION_GOES_ON = 0x157A4

# (VA, original text, new text, what): UTF-16 strings replaced in place; the
# new one is never longer.
STRINGS = [
    (0x3DF6C, "survarium_updater.exe", UPDATER_NAME,
     "start_updater: the updater's file name"),
    (0x3F514, "\\..\\game\\launcher\\index-", f"\\{PAGES_DIR}\\index-",
     "the local pages: beside the launcher, not the game's own"),
    (0x3EA90, "Can't run survarium_updater from service nor directly",
     f"Cannot start {UPDATER_NAME}. Copy the files again.",
     "the message when the updater cannot be started"),
    # The game keeps its language in user.cfg (Settings, exe patch 79): PLAY
    # passes none, so the choice made there holds; the game falls back to
    # this menu's language until one is made. swprintf ignores the language
    # word it is still given.
    (0x3F1C0, "\"%s\" -localization=%s ", "\"%s\" ",
     "get_game_launch_parameters: PLAY's command line without -localization"),
]

# The title of the box that shows an updater's error text (ASCII).
ERROR_TITLE = (0x3EF80, "Updater error:", "Skhid updater")

# (VA, original text): ASCII strings handed to BugTrap only; emptied.
EMPTIED = [
    (0x3DB6C, "launcher_crash_reports@survarium.com"),
    (0x3DB94, "http://support.survarium.com"),
    (0x3DBB4, "http://crasheat.survarium.com/process.php"),
]

# (VA, original text): UTF-16 addresses of the buttons that are gone; emptied,
# so that no address of the game's old sites is left in the file.
EMPTIED_WIDE = [
    (0x3F58C, "https://shop.survarium.com/"),
    (0x3F5C4, "https://forum.survarium.com/"),
    (0x3F600, "/ucp.php?mode=register"),
    (0x3F660, "http://www.youtube.com/user/vostokgames"),
    (0x3F6B0, "https://www.facebook.com/Survarium"),
    (0x3F6F8, "https://twitter.com/vostokgames"),
    (0x3F738, "http://vk.com/survarium"),
]

# (operand VA, original string VA, original text, what follows the site):
# `push <string>` operands pointed at a new string in the new section. Each
# is a relocated dword, and stays one.
URLS = [
    (0x1AFA2, 0x3F4C0, "http://survarium.com/launcher/v2/", "/launcher/v2/",
     "the page in the window"),
    (0x1BCBA, 0x3F630, "http://survarium.com/", "/?lang=", "PROJECT WEBSITE"),
]

# (rect VA, original left, original top, new left, what): where a button is
# on its panel. 30000 is far outside the panel: the button is never drawn
# and never under the mouse.
GONE = 30000
RECTS = [
    (0x45814, 819, 12, GONE, "settings (torrent, port): nothing of it applies"),
    (0x45844, 750, 47, GONE, "SHOP: silver and gold are exchanged in the lobby"),
    (0x45854, 614, 47, GONE, "CREATE ACCOUNT: an account is made by the first sign-in"),
    (0x45864, 478, 47, 750, "PROJECT WEBSITE: moved to SHOP's place, at the right"),
    (0x458A4, 32, 28, GONE, "YouTube"),
    (0x458B4, 68, 28, GONE, "Facebook"),
    (0x458C4, 104, 28, GONE, "Twitter"),
    (0x458D4, 140, 28, GONE, "VK"),
]


def parse_site(text):
    """`https://host[:port][/path]`, of characters that mean nothing to
    cmd.exe (the launcher opens a button's address with `cmd /c start`).
    Plain http is taken for this computer only (a server under test)."""
    text = text.rstrip("/")
    rest = "(?::[0-9]{1,5})?(?:/[A-Za-z0-9._~/-]*)?"
    if not (re.fullmatch("https://[A-Za-z0-9](?:[A-Za-z0-9.-]{0,61}[A-Za-z0-9])?" + rest, text)
            or re.fullmatch("http://(?:127[.]0[.]0[.]1|localhost)" + rest, text)) or len(text) > 100:
        sys.exit("site must be https://host[:port][/path] (letters, digits, . - _ ~ / only, 100 characters at most)")
    return text


def parse_file_name(text):
    if not re.fullmatch(r"[A-Za-z0-9._-]{1,27}\.exe", text):
        sys.exit("the game exe is a file name of letters, digits, . _ - ending in .exe, 31 characters at most")
    return text


def parse_arguments(text):
    if not re.fullmatch(r"[\x20-\x7e]{0,159}", text):
        sys.exit("the arguments are printable ASCII, 159 characters at most")
    return text


class Image:
    """The file and its section table: addresses to file offsets."""

    def __init__(self, data):
        self.data = data
        self.headers = P.Headers(data)

    def offset(self, va, size):
        for _, rva, vsize, raw, rsize, _ in self.headers.sections(self.data):
            if rva <= va - IMAGE_BASE and va - IMAGE_BASE + size <= rva + min(vsize, rsize):
                return va - IMAGE_BASE - rva + raw
        sys.exit(f"{va:#x}: in no section")

    def expect(self, va, expected, what):
        at = self.offset(va, len(expected))
        found = bytes(self.data[at:at + len(expected)])
        if found != expected:
            sys.exit(f"{va:#x} ({what}): expected {expected.hex()}, found {found.hex()}")
        return at

    def patch(self, va, expected, new, what):
        assert len(new) <= len(expected), what
        at = self.expect(va, expected, what)
        self.data[at:at + len(new)] = new


def relocated(data):
    """The VAs of every dword the loader would adjust (HIGHLOW entries)."""
    headers = P.Headers(data)
    rva, size = struct.unpack_from("<II", data, headers.reloc_at)
    start = next(rva - srva + raw for _, srva, vsize, raw, rsize, _ in headers.sections(data)
                 if srva <= rva < srva + max(vsize, rsize))
    found, at = set(), start
    while at < start + size:
        page, block = struct.unpack_from("<II", data, at)
        if block < 8:
            break
        for (entry,) in struct.iter_unpack("<H", data[at + 8:at + block]):
            if entry >> 12 == 3:
                found.add(IMAGE_BASE + page + (entry & 0xFFF))
        at += block
    return found


def utf16(text):
    return text.encode("utf-16-le")


def enter_key_targets(code):
    """Where `enter_key` of the .bin goes on to: (after Enter pressed PLAY,
    for every other message). Its replaced `cmp` and the jump back follow
    the jump of the first."""
    other = code.index(ENTER_KEY[1] + b"\xE9", ENTER_KEY_AT)
    done, back = other - 5, other + len(ENTER_KEY[1])
    if code[done] != 0xE9:
        sys.exit("launcher_values_026g.bin: no jump before enter_key's replaced instruction")
    return tuple(SECTION_VA + at + 5 + struct.unpack_from("<i", code, at + 1)[0] for at in (done, back))


def add_section(data, content):
    """Takes the Authenticode signature off (it cannot be right for a changed
    file, and the launcher never reads it) and appends the .skhid section in
    its place. Returns the new file contents."""
    headers = P.Headers(data)
    sections = headers.sections(data)
    _, last_rva, last_vsize, last_raw, last_rsize, _ = sections[-1]
    align = lambda value, to: (value + to - 1) // to * to
    slot = headers.table + 40 * len(sections)
    if slot + 40 > headers.size_of_headers or any(data[slot:slot + 40]):
        sys.exit("no free section table entry")
    if align(last_rva + last_vsize, headers.section_alignment) != SECTION_RVA \
            or struct.unpack_from("<I", data, headers.size_of_image_at)[0] != SECTION_RVA:
        sys.exit("the image does not end where the new section starts")
    raw = last_raw + last_rsize
    security_offset, security_size = struct.unpack_from("<II", data, headers.security_at)
    # The certificate table is the only thing after the last section.
    if security_offset != raw or security_offset + security_size != len(data) or raw % headers.file_alignment:
        sys.exit("unexpected layout after the last section")
    if len(content) > headers.section_alignment:
        sys.exit(f"the new section is {len(content)} bytes; one page fits")
    raw_size = align(len(content), headers.file_alignment)
    out = bytearray(data[:raw]) + content + b"\0" * (raw_size - len(content))
    out[slot:slot + 40] = SECTION_NAME + struct.pack(
        "<IIIIIIHHI", len(content), SECTION_RVA, raw_size, raw, 0, 0, 0, 0, SECTION_CHARACTERISTICS)
    struct.pack_into("<H", out, headers.count_at, len(sections) + 1)
    struct.pack_into("<I", out, headers.size_of_image_at,
                     align(SECTION_RVA + len(content), headers.section_alignment))
    struct.pack_into("<I", out, headers.size_of_code_at,
                     struct.unpack_from("<I", data, headers.size_of_code_at)[0] + raw_size)
    struct.pack_into("<II", out, headers.security_at, 0, 0)
    return out


def build(original, code, site=DEFAULT_SITE, game_exe=DEFAULT_GAME_EXE, arguments=DEFAULT_ARGUMENTS):
    """Returns (patched file, description). `code` = launcher_values_026g.bin."""
    if len(original) != EXE_SIZE or hashlib.sha256(original).hexdigest() != EXE_SHA256:
        sys.exit("the source is not the original 0.26g survarium_launcher.exe (sha256 mismatch)")
    site, game_exe, arguments = parse_site(site), parse_file_name(game_exe), parse_arguments(arguments)
    image = Image(bytearray(original))
    relocations = relocated(original)

    for va, expected, new, what in PATCHES:
        expected, new = bytes.fromhex(expected), bytes.fromhex(new)
        if any(va <= at + 3 and at < va + len(expected) for at in relocations):
            sys.exit(f"{va:#x} ({what}): a relocation overlaps the patch")
        image.patch(va, expected, new, what)

    va, expected, new = NO_VERSION[0], bytes.fromhex(NO_VERSION[1]), bytes.fromhex(NO_VERSION[2])
    assert va + 8 + 2 + new[9] == NO_VERSION_GOES_ON
    image.patch(va, expected, new, "start_updater: no version")
    disabled = P.neutralize_relocs(image.data, [(va, va + len(expected))])
    if disabled != [va + 3, va + 8]:
        sys.exit(f"{va:#x}: relocations {[hex(at) for at in disabled]} where two pushes were expected")

    va, old, new = ERROR_TITLE
    at = image.expect(va, old.encode("ascii") + b"\0", "the error box's title")
    image.data[at:at + len(old)] = new.encode("ascii") + b"\0" * (len(old) - len(new))

    for va, old, new, what in STRINGS:
        image.patch(va, utf16(old) + b"\0\0", utf16(new) + b"\0\0", what)
        # The rest of the old text goes too: nothing of it is left to find.
        at = image.offset(va, len(utf16(old)))
        image.data[at + len(utf16(new)):at + len(utf16(old))] = b"\0" * (len(utf16(old)) - len(utf16(new)))

    for va, old in EMPTIED:
        at = image.expect(va, old.encode("ascii") + b"\0", "a string for BugTrap")
        image.data[at:at + len(old)] = b"\0" * len(old)

    for va, old in EMPTIED_WIDE:
        at = image.expect(va, utf16(old) + b"\0\0", "the address of a removed button")
        image.data[at:at + len(utf16(old))] = b"\0" * len(utf16(old))

    for va, left, top, new_left, what in RECTS:
        image.patch(va, struct.pack("<ii", left, top), struct.pack("<i", new_left), what)

    # The language's sites call or jump to their entries.
    if len(code) < LANGUAGE_ENTRIES_AT + 8 * len(LANGUAGE_HOOKS):
        sys.exit(f"launcher_values_026g.bin is {len(code)} bytes: no language entries")
    for number, (va, expected, kind, what) in enumerate(LANGUAGE_HOOKS):
        entry = LANGUAGE_ENTRIES_AT + 8 * number
        if code[entry] != 0xE9:
            sys.exit(f"launcher_values_026g.bin: language entry {number} at {entry:#x} is not a jump")
        expected = bytes.fromhex(expected)
        opcode = b"\xE8" if kind == "call" else b"\xE9"
        new = opcode + struct.pack("<i", SECTION_VA + entry - va - 5) + b"\x90" * (len(expected) - 5)
        image.patch(va, expected, new, what)
    disabled = P.neutralize_relocs(image.data, [(va, va + len(bytes.fromhex(expected)))
                                                for va, expected, _, _ in LANGUAGE_HOOKS])
    if sorted(disabled) != sorted(LANGUAGE_RELOCATIONS):
        sys.exit(f"language sites: relocations {[hex(at) for at in disabled]}, "
                 f"not {[hex(at) for at in LANGUAGE_RELOCATIONS]}")

    # The new section: the code with its three slots filled, then the
    # addresses the launcher opens.
    section = bytearray(code)
    host = re.fullmatch("https?://([^/]+).*", site).group(1)
    for name, text in (("exe_name", game_exe), ("arguments", arguments), ("server", host)):
        at, size = SLOTS[name]
        if any(section[at:at + size]):
            sys.exit(f"launcher_values_026g.bin: the {name} slot is not empty")
        if len(text) >= size:
            sys.exit(f"{name} is {len(text)} characters; {size - 1} fit")
        section[at:at + len(text)] = text.encode("ascii")
    for operand, old_va, old, suffix, what in URLS:
        image.expect(old_va, utf16(old) + b"\0\0", what)
        if operand not in relocations:
            sys.exit(f"{operand:#x} ({what}): the operand is not a relocated address")
        at = image.expect(operand, struct.pack("<I", old_va), what)
        section += b"\0" * (len(section) % 2)
        struct.pack_into("<I", image.data, at, SECTION_VA + len(section))
        section += utf16(site + suffix) + b"\0\0"
        # The old address is of no use any more: it is emptied.
        old_at = image.offset(old_va, len(utf16(old)))
        image.data[old_at:old_at + len(utf16(old))] = b"\0" * len(utf16(old))

    # registry_key::get_string starts in the new code.
    va, expected = GET_STRING
    if any(va <= at + 3 and at < va + len(expected) for at in relocations):
        sys.exit(f"{va:#x}: a relocation overlaps the hook")
    hook = b"\xE9" + struct.pack("<i", SECTION_VA - va - 5) + b"\x90" * (len(expected) - 5)
    image.patch(va, expected, hook, "registry_key::get_string")
    back = SECTION_VA + code.index(b"\x5B\x55\x89\xE5\x51\x8D\x04\x36\xE9") + 8
    if back + 5 + struct.unpack_from("<i", code, back - SECTION_VA + 1)[0] != GET_STRING_BACK:
        sys.exit("launcher_values_026g.bin does not come back to get_string "
                 f"(was it assembled with -DCAVE={SECTION_VA:#x}?)")

    # The message loop goes through `enter_key`.
    va, expected = ENTER_KEY
    if any(va <= at + 3 and at < va + len(expected) for at in relocations):
        sys.exit(f"{va:#x}: a relocation overlaps the Enter key's hook")
    if code[ENTER_KEY_AT:ENTER_KEY_AT + 7] != bytes.fromhex("817de400010000"):
        sys.exit(f"launcher_values_026g.bin: enter_key is not at {ENTER_KEY_AT}")
    hook = b"\xE9" + struct.pack("<i", SECTION_VA + ENTER_KEY_AT - va - 5) + b"\x90" * (len(expected) - 5)
    image.patch(va, expected, hook, "launcher::execute, the message loop")
    if enter_key_targets(code) != (ENTER_KEY_DONE, ENTER_KEY_BACK):
        sys.exit("launcher_values_026g.bin does not come back to the message loop "
                 f"(was it assembled with -DCAVE={SECTION_VA:#x}?)")

    data = add_section(image.data, bytes(section))
    headers = P.Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, P.pe_checksum(data, headers.checksum_at))
    return data, (f"runs in place, no hardware warnings, no service, {len(PATCHES)} code sites, "
                  f"the installer's values from {len(code)} bytes of new code in .skhid at {SECTION_VA:#x}, "
                  f"site {site}, PLAY (or Enter) starts {game_exe} {arguments}, updater {UPDATER_NAME}, "
                  f"the language in user.cfg ({len(LANGUAGE_HOOKS)} sites), "
                  f"local pages in {PAGES_DIR}, {sum(1 for r in RECTS if r[3] == GONE)} buttons removed, "
                  "signature removed")


def default_bin():
    return os.path.join(HERE, "launcher_values_026g.bin")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--") or a == "--check"]
    options = [a for a in sys.argv[1:] if a.startswith("--") and a != "--check"]
    settings, bin_path = {}, None
    for option in options:
        name, _, value = option.partition("=")
        if name == "--site":
            settings["site"] = value
        elif name == "--game-exe":
            settings["game_exe"] = value
        elif name == "--arguments":
            settings["arguments"] = value
        elif name == "--bin":
            bin_path = value
        else:
            sys.exit(f"unknown option {option}\n\n{__doc__}")
    if len(args) != 2:
        sys.exit(__doc__)
    source, target = args
    if target != "--check" and os.path.exists(target) and os.path.samefile(source, target):
        sys.exit("the output must not be the original exe")
    with open(bin_path or default_bin(), "rb") as file:
        code = file.read()
    with open(source, "rb") as file:
        original = file.read()
    data, description = build(original, code, **settings)
    if target != "--check":
        with open(target, "wb") as out:
            out.write(data)
    print(f"{target}: {description}")
    print(f"{len(data)} bytes, sha256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
