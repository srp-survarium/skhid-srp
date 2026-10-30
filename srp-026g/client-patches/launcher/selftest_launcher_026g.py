"""Builds the Skhid launcher in memory from the original and checks the
result, without running it and without writing into the game folder.

    cd srp-026g/client-patches/launcher
    python selftest_launcher_026g.py <game>/binaries/x86/survarium_launcher.exe [--nasm=<nasm>]

Checks:
 1. every byte that differs from the original is in a place the README
    names: nothing else was touched;
 2. the file is a well-formed PE again: six sections, the new one where the
    image ended, the checksum right, no signature left, nothing after the
    last section;
 3. the jump in registry_key::get_string lands on the new code, and the new
    code comes back to the byte after the replaced instructions; the same
    for the Enter key's jump in the message loop and PLAY's after
    CreateProcessW (which also goes on to the assertion), and the box for
    Smart App Control has its five texts;
 4. the two addresses the launcher opens are the site's, where their
    operands point, and the operands are still relocated;
 5. no relocation touches a changed instruction (but the two disabled ones);
 6. no address of the game's old sites is left in the file;
 7. with --nasm (or NASM in the environment): launcher_values_026g.asm
    assembles to the checked-in .bin byte for byte;
 8. all original native artwork and localized text resources are unchanged.
"""

import hashlib
import os
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import patch_launcher_026g as L  # noqa: E402
P = L.P

SITE = "https://selftest.example/srp"


def ranges_of_the_patch(original, patched, section_size):
    """[(file offset, length, what)]: every place the patch may differ."""
    image = L.Image(bytearray(original))
    headers = P.Headers(original)
    allowed = []
    for va, expected, new, what in L.PATCHES:
        allowed.append((image.offset(va, 1), len(bytes.fromhex(new)), what))
    allowed.append((image.offset(L.NO_VERSION[0], 1), len(bytes.fromhex(L.NO_VERSION[2])), "no version"))
    allowed.append((image.offset(L.GET_STRING[0], 1), len(L.GET_STRING[1]), "get_string hook"))
    allowed.append((image.offset(L.ENTER_KEY[0], 1), len(L.ENTER_KEY[1]), "Enter key hook"))
    allowed.append((image.offset(L.PLAY_STARTED[0], 1), len(L.PLAY_STARTED[1]), "PLAY hook"))
    for va, old, new, what in L.STRINGS:
        allowed.append((image.offset(va, 1), len(L.utf16(old)), what))
    for va, old in L.EMPTIED:
        allowed.append((image.offset(va, 1), len(old), "BugTrap string"))
    for va, old in L.EMPTIED_WIDE:
        allowed.append((image.offset(va, 1), len(L.utf16(old)), "removed button's address"))
    allowed.append((image.offset(L.ERROR_TITLE[0], 1), len(L.ERROR_TITLE[1]), "error title"))
    for operand, old_va, old, suffix, what in L.URLS:
        allowed.append((image.offset(operand, 1), 4, what + " operand"))
        allowed.append((image.offset(old_va, 1), len(L.utf16(old)), what + " old text"))
    for va, left, top, new_left, what in L.RECTS:
        allowed.append((image.offset(va, 1), 4, what))
    # Headers: section count, size of code, size of image, checksum, the
    # security directory, the sixth section table entry.
    allowed += [(headers.count_at, 2, "section count"), (headers.size_of_code_at, 4, "size of code"),
                (headers.size_of_image_at, 4, "size of image"), (headers.checksum_at, 4, "checksum"),
                (headers.security_at, 8, "security directory"),
                (headers.table + 40 * 5, 40, "section table entry")]
    return allowed


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    nasm = next((a[7:] for a in sys.argv[1:] if a.startswith("--nasm=")), os.environ.get("NASM"))
    if len(args) != 1:
        sys.exit(__doc__)
    with open(args[0], "rb") as file:
        original = file.read()
    with open(L.default_bin(), "rb") as file:
        code = file.read()
    patched, description = L.build(original, code, site=SITE)
    patched = bytes(patched)
    print(description)
    failures = []

    def check(number, ok, text):
        print(f"{'ok  ' if ok else 'FAIL'} {number}. {text}")
        if not ok:
            failures.append(number)

    headers, old_headers = P.Headers(patched), P.Headers(original)
    sections = headers.sections(patched)
    name, rva, vsize, raw, rsize, characteristics = sections[-1]
    old_end = sections[-2][3] + sections[-2][4]          # where .reloc's bytes end

    # 1. Nothing but the named places differs.
    allowed = ranges_of_the_patch(original, patched, vsize)
    reloc_rva, reloc_size = struct.unpack_from("<II", original, old_headers.reloc_at)
    reloc_at = next(reloc_rva - srva + sraw for _, srva, _, sraw, _, _ in old_headers.sections(original)
                    if srva <= reloc_rva < srva + reloc_size + 1)
    stray, disabled = [], []
    for at in range(old_end):
        if original[at] != patched[at]:
            if reloc_at <= at < reloc_at + reloc_size:
                disabled.append(at)
            elif not any(start <= at < start + length for start, length, _ in allowed):
                stray.append(at)
    check(1, not stray and len(patched) == raw + rsize,
          f"{sum(a != b for a, b in zip(original[:old_end], patched))} bytes differ before the new section, "
          f"all in named places" + (f"; NOT named: {[hex(at) for at in stray[:10]]}" if stray else ""))

    # 2. A well-formed PE.
    security = struct.unpack_from("<II", patched, headers.security_at)
    check(2, len(sections) == 6 and name == L.SECTION_NAME and rva == L.SECTION_RVA and raw == old_end
          and rsize % headers.file_alignment == 0 and vsize <= rsize
          and characteristics == L.SECTION_CHARACTERISTICS and security == (0, 0)
          and struct.unpack_from("<I", patched, headers.size_of_image_at)[0] == L.SECTION_RVA + 0x1000
          and struct.unpack_from("<I", patched, headers.checksum_at)[0] == P.pe_checksum(patched, headers.checksum_at),
          f"six sections, .skhid of {vsize} bytes at {L.SECTION_VA:#x} (file {raw:#x}), checksum right, no signature")

    # 3. The hook and its way back.
    image = L.Image(bytearray(patched))
    at = image.offset(L.GET_STRING[0], 7)
    jump = patched[at:at + 7]
    target = L.GET_STRING[0] + 5 + struct.unpack_from("<i", jump, 1)[0]
    section = patched[raw:raw + vsize]
    back_at = section.index(b"\x5B\x55\x89\xE5\x51\x8D\x04\x36\xE9") + 8
    back = L.SECTION_VA + back_at + 5 + struct.unpack_from("<i", section, back_at + 1)[0]
    check(3, jump[0] == 0xE9 and jump[5:] == b"\x90\x90" and target == L.SECTION_VA
          and back == L.GET_STRING_BACK == L.GET_STRING[0] + len(L.GET_STRING[1]),
          f"get_string jumps to {target:#x}, the new code comes back to {back:#x}")
    at = image.offset(L.ENTER_KEY[0], 7)
    jump = patched[at:at + 7]
    target = L.ENTER_KEY[0] + 5 + struct.unpack_from("<i", jump, 1)[0]
    done, back = L.enter_key_targets(section)
    check("3a", jump[0] == 0xE9 and jump[5:] == b"\x90\x90" and target == L.SECTION_VA + L.ENTER_KEY_AT
          and section[L.ENTER_KEY_AT:L.ENTER_KEY_AT + 7] == bytes.fromhex("817de400010000")
          and back == L.ENTER_KEY_BACK == L.ENTER_KEY[0] + len(L.ENTER_KEY[1])
          and done == L.ENTER_KEY_DONE,
          f"the message loop jumps to {target:#x}, the new code comes back to {back:#x} "
          f"and, after Enter, to {done:#x}")
    at = image.offset(L.PLAY_STARTED[0], 7)
    jump = patched[at:at + 7]
    target = L.PLAY_STARTED[0] + 5 + struct.unpack_from("<i", jump, 1)[0]
    back, failed = L.play_started_targets(section)
    check("3c", jump[0] == 0xE9 and jump[5:] == b"\x90\x90" and target == L.SECTION_VA + L.PLAY_STARTED_AT
          and back == L.PLAY_STARTED_BACK == L.PLAY_STARTED[0] + len(L.PLAY_STARTED[1])
          and failed == L.PLAY_ASSERT,
          f"PLAY's failure jumps to {target:#x}, the new code comes back to {back:#x} "
          f"and, for any error but 4551, to the assertion at {failed:#x}")
    texts = [section[L.BLOCKED_TEXTS_AT + n * L.BLOCKED_TEXT_SIZE:][:L.BLOCKED_TEXT_SIZE]
             .decode("utf-16-le").split("\0")[0] for n in range(5)]
    check("3d", texts == L.blocked_texts(L.DEFAULT_GAME_EXE)
          and all(L.DEFAULT_GAME_EXE in text and "4551" in text for text in texts)
          and section[L.BLOCKED_TEXTS_AT - 12:L.BLOCKED_TEXTS_AT] == "Skhid\0".encode("utf-16-le"),
          f"the box for error 4551: title Skhid, {len(texts)} languages, "
          f"{max(map(len, texts))} of {L.BLOCKED_TEXT_SIZE // 2 - 1} characters at most")
    slots = {}
    for slot, (start, size) in L.SLOTS.items():
        slots[slot] = section[start:start + size].split(b"\0")[0].decode("ascii")
    check("3b", slots == {"exe_name": L.DEFAULT_GAME_EXE, "arguments": L.DEFAULT_ARGUMENTS,
                          "server": "selftest.example"},
          f"the new code's values: {slots}")

    # 4. The addresses the launcher opens.
    relocations = L.relocated(patched)
    opened = []
    for operand, old_va, old, suffix, what in L.URLS:
        va = struct.unpack_from("<I", patched, image.offset(operand, 4))[0]
        text = section[va - L.SECTION_VA:].decode("utf-16-le", "replace").split("\0")[0]
        opened.append((what, text, operand in relocations, va % 2 == 0))
    check(4, all(text == SITE + suffix and is_relocated and even
                 for (what, text, is_relocated, even), (_, _, _, suffix, _) in zip(opened, L.URLS)),
          "; ".join(f"{what}: {text}" for what, text, _, _ in opened))

    # 5. Relocations and changed code.
    code_ranges = [(va, len(bytes.fromhex(new))) for va, _, new, _ in L.PATCHES] + [(L.GET_STRING[0], 7),
                                                                                      (L.ENTER_KEY[0], 7),
                                                                                      (L.PLAY_STARTED[0], 7)]
    overlapping = [hex(at) for at in relocations for va, length in code_ranges if va <= at + 3 and at < va + length]
    no_version = L.NO_VERSION[0]
    still = [hex(at) for at in relocations if no_version <= at + 3 and at < no_version + 12]
    check(5, not overlapping and not still and len(disabled) == 2 and len(relocations) == len(L.relocated(original)) - 2,
          f"no relocation in changed code; {len(disabled)} relocation bytes changed (the two disabled entries)")

    # 6. No old address left.
    left = []
    for word in ("survarium.com", "vostokgames", "facebook.com", "twitter.com", "vk.com", "youtube.com"):
        for encoded in (word.encode("ascii"), word.encode("utf-16-le")):
            if encoded in patched:
                left.append(word)
    check(6, not left, "no address of the game's old sites in the file" + (f"; LEFT: {sorted(set(left))}" if left else ""))

    # 7. The source gives the .bin.
    if nasm:
        with tempfile.TemporaryDirectory() as folder:
            out = os.path.join(folder, "values.bin")
            subprocess.run([nasm, "-f", "bin", f"-DCAVE={L.SECTION_VA:#x}",
                            os.path.join(HERE, "launcher_values_026g.asm"), "-o", out], check=True)
            with open(out, "rb") as file:
                assembled = file.read()
        check(7, assembled == code, f"launcher_values_026g.asm assembles to the checked-in .bin ({len(code)} bytes)")
    else:
        print("skip 7. no --nasm: the .bin was not assembled again")

    # The page skin is separate from the native window's bitmaps and text.
    # Compare the whole resource section, including every language and all
    # normal/hover/pressed/disabled button and progress-bar images.
    resource = next(section for section in old_headers.sections(original)
                    if section[0].rstrip(b"\0") == b".rsrc")
    resource_at, resource_size = resource[3], resource[4]
    check(8, original[resource_at:resource_at + resource_size]
          == patched[resource_at:resource_at + resource_size],
          f"all {resource_size} bytes of the native window's artwork and text resources are unchanged")

    print(f"result: {len(patched)} bytes, sha256 {hashlib.sha256(patched).hexdigest()}")
    if failures:
        sys.exit(f"FAILED: checks {failures}")
    print("ALL CHECKS PASSED")


if __name__ == "__main__":
    main()
