Survarium v0.45a6 reverse-engineering reference

Build 3976 (internal id 1806, 2016-12-16), read to learn how a later build
shows a shot in the aimed view (../../../srp-026g/docs/notes/aim-pose.md).
identity.json identifies the executable: survarium.exe of the installer's
installer.sup, with the updater's x86 BCJ filter undone (lzma FILTER_X86,
start offset 0; as unpacked its call targets are absolute and the file is
not runnable). VAs are at the preferred image base 0x10000.

functions.tsv: va, status, name, evidence.
  inferred-high, inferred-medium: a name given from the inspected body and
  its likeness to the v0.26g function of that name ([descriptive]: no
  v0.26g counterpart, named for what it does). Not PDB names.
These are analysis labels, not permission to reuse addresses in another build.
No client patch calls these functions.
