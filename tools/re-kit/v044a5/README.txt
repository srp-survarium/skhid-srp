Survarium v0.44a5 x86 reverse-engineering reference

identity.json identifies the executable read: game/binaries/x86/survarium.exe of
the 0.44a5 installer, x86-BCJ decoded (the installer's .sup binaries are
BCJ-filtered; the file as unpacked is not runnable until decoded).
Preferred-image VAs (image base 0x10000, as 0.26g's).

functions.tsv: va, status, name, evidence.
  reviewed: a descriptive name assigned from the inspected body (strings,
            the configuration bits it writes); not a PDB name.
abi.tsv / abi_inputs.tsv use the columns documented in ../v026g/README.txt.
These are analysis labels for reading the donor's renderer, not addresses
any client patch calls: patch 169 (README "London's forward effects", client_patch.md)
does in 0.26g what these do in 0.44a5.
