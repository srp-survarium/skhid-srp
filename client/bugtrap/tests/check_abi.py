"""Check exports/ordinals and runtime imports using LLVM's PE inspector."""
from pathlib import Path
import re
import subprocess
import sys

path = Path(sys.argv[1])
text = subprocess.check_output(["llvm-readobj", "--coff-exports", "--coff-imports", str(path)], text=True)
actual = {name: int(ordinal) for ordinal, name in re.findall(r"Ordinal: (\d+)\s+Name: (\S+)", text)}
for line in Path(__file__).with_name("exports.txt").read_text().splitlines():
    ordinal, name = line.split()
    assert actual.get(name) == int(ordinal), (name, actual.get(name), ordinal)
imports = re.findall(r"Import \{\s+Name: (\S+)", text)
# The C runtime is linked in (no msvcr*, vcruntime*, ucrtbase or api-ms-win-crt-*);
# ntdll.dll and api-ms-win-core-synch-l1-2-0.dll are the standard library's
# (NtWriteFile, RtlNtStatusToDosError; WaitOnAddress, WakeByAddress*).
assert set(name.lower() for name in imports) == {"kernel32.dll", "user32.dll", "gdi32.dll", "shell32.dll", "ntdll.dll",
                                                 "api-ms-win-core-synch-l1-2-0.dll"}, imports
assert "Arch: i386" in text
print("PASS ABI: all 82 legacy exports and ordinals; Win32; imports only kernel32/user32/gdi32/shell32/ntdll/api-ms-win-core-synch")
