"""sites.py : print VA, file offset and original bytes of every patch site named in
notes/launcher-026g.md, plus the URL/string table with slack. Read-only."""
import os, re, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lpe import Pe, L026
pe = Pe(L026)
d = pe.data
SITES = [
    (0x1474A, 6, "run_as_copy: jne (debugger present -> skip copy)"),
    (0x14788, 6, "run_as_copy: jne (already the temp copy -> skip copy)"),
    (0x14442, 6, "launcher ctor: je (not the temp copy -> no form)"),
    (0x15900, 2, "start_updater: jne (temp copy -> <temp>/patch_temp)"),
    (0x13FFF, 5, "registry_key::open: push HKLM"),
    (0x140D7, 5, "product GUID: push HKLM"),
    (0x15194, 5, "first install version: push HKLM"),
    (0x15231, 5, "local version (file to execute): push HKLM"),
    (0x159A4, 5, "start_updater update server: push HKLM"),
    (0x16301, 5, "restart_launcher launcher: push HKLM"),
    (0x170AD, 5, "play parameters: push HKLM"),
    (0x15223, 5, "local version: call get_product_registry_path (jump to its read 0x15252)"),
    (0x15997, 5, "start_updater: call get_product_registry_path (jump to its read 0x159D1)"),
    (0x162F4, 5, "restart_launcher: call get_product_registry_path (jump to its read 0x1631D)"),
    (0x1709F, 5, "play parameters: call get_product_registry_path (jump to its reads 0x170CE)"),
    (0x153FE, 9, "local version, no version: push key / call RegCloseKey"),
    (0x1541E, 9, "local version, a version: push key / call RegCloseKey"),
    (0x15A4D, 9, "start_updater end: push key / call RegCloseKey"),
    (0x16338, 13, "restart_launcher: push key / test al / jne 0x163A7 / call RegCloseKey"),
    (0x163A7, 6, "restart_launcher, value read: call RegCloseKey (key pushed at 0x16338)"),
    (0x17123, 9, "play parameters, failed: push key / call RegCloseKey"),
    (0x1714F, 9, "play parameters, done: push key / call RegCloseKey"),
    (0x12D55, 5, "check_cpu CentralProcessor: push HKLM (leave)"),
    (0x1544B, 5, "UninstallString: push HKCU"),
    (0x1E321, 5, "InstallLocation: push HKCU"),
    (0x1D4E3, 5, "settings reload: push HKCU"),
    (0x1D72F, 5, "settings save: push HKCU"),
    (0x1D871, 5, "language reload: push HKCU"),
    (0x1D8E8, 5, "language save: push HKCU"),
    (0x15764, 2, "start_updater: jne first-install-version ok"),
    (0x15778, 2, "start_updater: jne local version ok"),
    (0x159BE, 2, "start_updater: jne registry key opened"),
    (0x159E7, 2, "start_updater: jne update server read"),
    (0x15A2C, 2, "start_updater: jne updater started"),
    (0x1B15D, 6, "form ctor: lea edx,[remote URL] for browser B"),
    (0x1AFA1, 5, "form ctor: push remote base URL"),
    (0x1B04A, 5, "form ctor: push local page path fragment"),
    (0x1BBEF, 3, "shop click handler entry"),
    (0x1BC45, 3, "create-account click handler entry"),
    (0x1BCAD, 3, "project-website click handler entry"),
    (0x1BD03, 5, "youtube click handler entry"),
    (0x1BD16, 5, "facebook click handler entry"),
    (0x1BD29, 5, "twitter click handler entry"),
    (0x1BD3C, 5, "vk click handler entry"),
    (0x1B810, 2, "settings gear click handler entry"),
    (0x1BB95, 3, "open_url (cmd.exe /c start) entry"),
    (0x19B71, 4, "NewWindow3 handler start (ShellExecute open)"),
    (0x145C5, 6, "check_hardware entry"),
    (0x14E7B, 9, "execute: Vista check call + test + jne"),
    (0x14E25, 9, "execute: single-instance check call + test + jne"),
    (0x15BB5, 10, "on_updater_message: lea ecx,[type-1]; jmp table (no bounds check)"),
    (0x1609E, 100, "on_updater_message jump table (25 dwords)"),
    (0x17208, 15, "play: CreateProcessW args"),
    (0x3A63D, 3, "settings gear rect: add eax,14h (width)"),
]
for va, n, what in SITES:
    print("%08X file %06X  %-48s %s" % (va, pe.off(va), d[pe.off(va):pe.off(va) + n].hex(" ") if n <= 16 else "(table)", what))
print()
_, tva, tvs, tro, trs = pe.sec(".text")
text = d[tro:tro + tvs]
def refs(va):
    n = struct.pack("<I", va); out = []; p = 0
    while (p := text.find(n, p)) != -1:
        out.append("%X" % (tva + p)); p += 1
    return out
U16 = [0x3DF6C, 0x3DF98, 0x3E784, 0x3E7B8, 0x3E808, 0x3E828, 0x3E908, 0x3E9A8, 0x3E9D8, 0x3E9E4, 0x3E760,
       0x3DFD4, 0x3DFE8, 0x3E014, 0x3E030, 0x3E044, 0x3E064, 0x3E078, 0x3EC5C, 0x3FAF0, 0x3DE30, 0x3DE48, 0x3DE64,
       0x3ED0C, 0x3ED24, 0x3ED34, 0x3ED44, 0x3ED7C, 0x3F070, 0x3F0A4, 0x3F1C0, 0x3F1F0, 0x3F200, 0x3F210, 0x3F224, 0x3F234,
       0x3F490, 0x3F498, 0x3F4A0, 0x3F4A8, 0x3F4B0, 0x3F4C0, 0x3F504, 0x3F514, 0x3F548, 0x3F558, 0x3F564,
       0x3F58C, 0x3F5C4, 0x3F600, 0x3F630, 0x3F660, 0x3F6B0, 0x3F6F8, 0x3F738, 0x3F98C, 0x3F9A0, 0x3F394]
ASC = [0x3DB6C, 0x3DB94, 0x3DBB4, 0x3DBE0, 0x3FD24, 0x3FD34, 0x3FE88, 0x3EC3C]
for va in U16:
    o = pe.off(va); e = o
    while d[e:e + 2] != b"\0\0":
        e += 2
    s = d[o:e].decode("utf-16-le"); z = 0
    while d[e + z] == 0:
        z += 1
    print("U16 %08X file %06X len=%2d zeros=%2d max_new_len=%2d refs=%s %r" % (va, o, len(s), z, len(s) + z // 2 - 1, ",".join(refs(va)), s))
for va in ASC:
    o = pe.off(va); e = d.index(b"\0", o); s = d[o:e].decode("latin-1"); z = 0
    while d[e + z] == 0:
        z += 1
    print("ASC %08X file %06X len=%2d zeros=%2d max_new_len=%2d refs=%s %r" % (va, o, len(s), z, len(s) + z - 1, ",".join(refs(va)), s))
