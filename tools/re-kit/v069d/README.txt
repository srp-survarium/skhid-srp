Survarium v0.69d x64 reverse-engineering reference

identity.json identifies the exact retail executable read from
Survarium/game/binaries/x64/survarium.exe in the archived 0.69d Survarium.zip.
Preferred-image VAs; runtime VA = module base + VA - 0x200000000.

functions.tsv: va, status, name, evidence.
  observed: the executable's diagnostic gives this qualified name.
  reviewed: a descriptive name assigned from the inspected body; not a PDB name.
abi.tsv / abi_inputs.tsv use the columns documented in ../v026g/README.txt.
These are analysis labels, not permission to reuse addresses in another build.

The music behaviour and its call-site evidence are in
../../../srp-026g/docs/notes/music.md. No client patch calls these x64 functions.

Another build's addresses. The Ghidra project private/re-069/net069-gh (p069.gpr) and the decompiled
netcode beside it are of ANOTHER executable (s069.exe, MD5 5585767e...), not the identity exe (MD5
6f77752e...): the same functions stand 0x260 to 0x310 higher here (get_slot_transform 0x2006cf0c0
there, 0x2006cf320 here). Twelve weapon-slot rows of this table carried that project's addresses
until 2026-11-21 (commit 214a7312b); they are corrected. Anything taken from that project needs the
same check before it becomes a row: the address must be a function start of the identity exe (its
.pdata) and the body must be the one described.
