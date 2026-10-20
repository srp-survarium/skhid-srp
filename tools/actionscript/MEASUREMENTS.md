# Revision 24 compiler investigation

Measured locally against the release pinned in `releases/026g-rev24.json` on
2026-10-20. These are compiler experiments, not a compatibility approval.

| Compiler path | Selected scripts | Compiled | Exact decompressed movies |
| --- | ---: | ---: | ---: |
| FFDec 25.1.3 | 18 | 17 | 0 |
| Flex 4.16.1 through FFDec, default arguments | 18 | 17 | 0 |
| Flex, strict binding, no optimization/debug, traces retained | 18 | 15 | 0 |

The input list is `feature-classes.txt`. Each compilation starts from the
original movie. The entire resulting movie is compared, not just the selected
class. Raw reports, per-method records, disassembly, compiler dependencies and
candidate movies are private artifacts.

FFDec fails on `LobbyMenu`. Flex's default path also fails there, reporting an
ABC decoding error for its dependency `survarium.char.CharWindow`. Strict Flex
additionally rejects `survarium.controls.InventoryList` and
`survarium.messages.MessageApply`.

The Flex integration needed the Unix launcher fix and dependency SWC changes
described in the README before these measurements could run.

## Evidence guiding the next step

- FFDec replaces original `callproperty; pop` pairs with `callpropvoid` and
  replaces some `initproperty` instructions with `setproperty`. A previous
  matching decompile is therefore not evidence of matching code.
- Default Flex uses broad namespace-set lookups and inserts debug file/line
  instructions. Strict binding and disabled debug output remove these particular
  differences without rewriting the source.
- With strict Flex and traces preserved, `ItemCard.buyCurrentItem` has identical
  instruction bytes **and** identical resolved disassembly. Its method record
  still differs: the original debug method name is the empty string, while
  Flex emits `survarium.controls:ItemCard/protected:buyCurrentItem`. Both have
  flags 0, one register and scope depths 10/11. This is a concrete example of
  why code-only equality is insufficient.
- The ItemCard constructor differs in the identity of a private namespace,
  even where the instruction sequence otherwise matches. This needs a verified
  namespace/linker mapping, not a text comparison that discards namespaces.
- The unchanged container split/reassembly reproduces the real release movie
  exactly after decompression. This verifies preservation of opaque Scaleform
  assets; it does not count as recompilation.
- A fresh code-only export produced all 878 scripts and 7,758 method bodies.
  All 18 selected class sources match the previous full-movie export. Some
  asset classes omit FFDec-generated `[Embed]` annotations when exported from
  the code-only movie; their original graphic tags are kept separately.

Next work is to retain this strict byte gate while correcting decompiled
source and implementing compiler-output relocation/metadata handling. No
class is accepted merely because it compiles or its decompiled text looks
equivalent. Whole-tree recompilation, reviewed source corrections, feature
diff builds, untouched-class verification and the runtime check remain open.
