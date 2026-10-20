# Revision 24 compiler investigation

Measured locally against the release pinned in `releases/026g-rev24.json` on
2026-10-20. These are compiler experiments, not a compatibility approval.

| Compiler path | Selected scripts | Compiled | Exact decompressed movies |
| --- | ---: | ---: | ---: |
| FFDec 25.1.3 | 18 | 17 | 0 |
| Flex 4.16.1 through FFDec, default arguments | 18 | 17 | 0 |
| Flex, strict binding, no optimization/debug, traces retained; initial dependency adapter | 18 | 15 | 0 |
| Strict Flex with complete dependency catalog and wildcard adapter | 18 | 18 | 0 |

The input list is `feature-classes.txt`. Each compilation starts from the
original movie. The entire resulting movie is compared, not just the selected
class. Raw reports, per-method records, disassembly, compiler dependencies and
candidate movies are private artifacts.

FFDec fails on `LobbyMenu`. The initial Flex default path also failed there, reporting an
ABC decoding error for its dependency `survarium.char.CharWindow`. Strict Flex
initially also rejected `survarium.controls.InventoryList` and
`survarium.messages.MessageApply`. These three Flex failures are now fixed by
the dependency adapter, without editing those classes' source.

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

## Dependency fixes and exact linked output

The incomplete SWC catalog omitted expression/type dependencies as well as
inheritance. Supplying references to defined classes fixes `InventoryList`'s
apparently missing methods and `MessageApply`'s internal compiler error.
A Java debugger traced the `CharWindow` decoding failure to ASC dereferencing
multiname zero in its `_branches:Vector.<*>` field. An explicit public `*`
name in the temporary dependency library preserves that wildcard while avoiding
the decoder bug. `LobbyMenu` then compiles too. A synthetic ActionScript test
reproduces the failure and verifies the adapter's repair.

Strict Flex has now compiled all 18 feature classes successfully. The full
878-script compiler sweep is running separately; its partial results must not
be interpreted as a completed whole-tree check.

The first layout-linker pass evaluated 113 completed rows of that sweep:
**34 produced an exactly identical compressed movie**, and 79 were refused.
This pass predates method-order relocation. `BackBlack` is one verified example:
its compiled 384-byte ABC reproduces the original, and recompressing the entire
movie gives SHA-256
`3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60`.

The linker keeps compiler-emitted executable records, retains original pool
layout only after checking existing entries, preserves empty debug method names,
and restores the Adobe editor's navigation annotations. It compares the entire
resulting ABC and verifies that all other blocks are unchanged. It does not
fill a mismatch with the original method body. Changed runtime annotations,
instructions, signatures, verifier metadata or exception records still fail.

The next linker revision also restores named method ordering and method-index
operands when their encoded widths remain unchanged. With that addition,
`ArrowButtonFLR`'s compiled 808-byte ABC is exact too: Flex emits the constructor
before its frame functions, whereas the original stores it after them. The
synthetic regression test covers constructor/class-initializer reordering and
rejects altered instructions or stack metadata without creating an output movie.
