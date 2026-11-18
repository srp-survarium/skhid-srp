Original retail Survarium 0.11e, build 884.

identity.json pins the executable. All va values are preferred virtual
addresses (image base 0x10000), not RVAs and not 0.26 addresses.

functions.tsv: va, status, size, name. verified-pdb means the original build's
function-structure.jsonl.gz provides the exact identity, RVA and size; it does
not certify an ABI for a new caller.

Source catalog:
vostok-versions/versions/0.11e/builds/884/function-structure.jsonl.gz
and source-files.tsv. The reconstructed C++ tree is labeled 0.100b; compare
original PDB source hashes and 0.11e instructions before borrowing its logic.

The rain entries support the Vostok evening/rain port investigation. Original
render_rain_shadow_map is 0x00627bb0, 1863 bytes. Its PDB statement ranges put
projection/alignment at +0x59..+0x2a8 and caster filtering/draws at +0x490..+0x536.
These are original-build findings; they are not hooks into the shipped client.
