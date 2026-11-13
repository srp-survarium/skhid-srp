"""Verify the optional voice DLL's typed ABI and system-only dependencies."""
from pathlib import Path
import re
import subprocess
import sys

path = Path(sys.argv[1])
text = subprocess.check_output(
    ["llvm-readobj", "--coff-exports", "--coff-imports", str(path)], text=True
)
abi = (Path(__file__).parent.parent / "crates/voice-abi/src/lib.rs").read_text()
expected = set(re.findall(r'load!\(c"(sv_[a-z_]+)"', abi))
actual = set(re.findall(r"Ordinal: \d+\s+Name: (\S+)", text))
assert len(expected) == 11 and actual == expected, (actual, expected)
imports = {name.lower() for name in re.findall(r"Import \{\s+Name: (\S+)", text)}
# As BugTrap, the CRT is static. No codec, GCC, MSVC or UCRT DLL is shipped.
allowed = {"kernel32.dll", "ntdll.dll", "userenv.dll", "ws2_32.dll", "winmm.dll",
           "advapi32.dll", "bcryptprimitives.dll"}
assert imports <= allowed, imports
assert "Arch: i386" in text, "voice DLL must match the game's Win32 ABI"
print(f"PASS ABI: {len(actual)} typed exports; Win32; system imports {', '.join(sorted(imports))}")
