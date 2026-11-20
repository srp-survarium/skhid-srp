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
