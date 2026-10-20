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

## Class binding and debugger layout

The completed method-order batch accepted **81 of 172** compiled scripts.
A subsequent batch with class-binding emission, automatic script slots and
optional debugger parameter names accepted **239 of 292** selected scripts;
52 were refused and one (`SrpRooms`) had failed compilation. Both batches took
snapshots of the still-running full compiler sweep. Neither is a whole-tree
rebuild.

The class-binding difference is `getscopeobject 0` versus `findpropstrict` for
the script's declared class. ASC's global-constant assignment branch emits the
former ([Apache compiler source](https://github.com/apache/flex-sdk/blob/1644730bd54105c8aab234af7c1a08b2af17865d/modules/asc/src/java/macromedia/asc/semantics/CodeGenerator.java#L1335)).
The narrow emission rule and its refused shapes are documented in the README.
`BlueTexture` and `BtnChangeName` now pass complete compressed-file equality.

Two additional failures, `ButtonBottomInviteArrow` and `ButtonChatTabBottom`,
had matching executable instructions but original debugger file/line directives
that were absent from compiler output. Straight-line source-map emission now
reproduces their 1,297-byte and 2,021-byte ABCs, respectively. Both reconstruct
the complete movie to the pinned SHA-256 above. These targeted checks happened
after the 292-script batch and are not included in its 239 successes.

Sixteen `LobbyMenu_*Icons` classes were refused because the importer appended
an identical `ExcludeClass` annotation at a new metadata index. Runtime metadata
comparison now resolves all names, keys and values before retaining the original
index. The completed batch with this fix and debugger-map emission accepted
**316 of 370** selected scripts; 53 were refused and one had failed compilation.
Synthetic tests verify duplicate annotation relocation and reject changed values,
scope operands, slots, defaults, executable instructions and verifier metadata.
All eight tests, including the real Flex integration test, pass.

Remaining observed issues include anonymous method ownership in `LobbyMenu`,
class/script private namespace identity and namespace-set order, and decompiler
source defects in `SrpRooms` (missing return paths and implicit int-to-String
coercions rejected by strict Flex). The complete 878-script sweep is unfinished.

## Namespace and control-flow relocation

The next completed batch accepted **434 of 440** selected scripts, with five
refused and one compile failure. This batch maps unique class/file private
namespace owners, compares namespace-set membership, and relocates equivalent
multiname references. `CreateClanWindow`'s 2,296-byte ABC is one newly exact
example. The batch predates operand-width relocation and native-import repair.

Reference relocation now also updates branch/switch offsets and exception
boundaries when a pool or method index changes width. Trait ordering follows
qualified owner/name and kind, retaining the actual compiler records. The real
compiler regression covers wide references across forward and backward branches,
switch tables and handlers, and rejects changed control flow and namespaces.

That regression exposed FFDec 25.1.3 rewriting unreachable jump operands while
decoding an unchanged compiler output. A linear decoder now requires exact
instruction-byte preservation. Applied to the original release, it preserves
all **7,758 method records** and every ABC block; it corrects 23 diagnostic
disassemblies from the former control-flow decoder. Earlier exact-file successes
remain valid; their final comparisons included the actual complete movie bytes.

Native Flex output is now retained and imported with that decoder before symbol
merging. The importer preserves other scripts/classes sharing the ABC. An actual
recompile-and-link regression, including private members, loops, switch,
try/catch and an unrelated retained script, reproduces its complete input movie.

All 18 feature scripts compile with the repaired native importer. Their current
linker pass still refuses all 18: remaining findings include ambiguous file-private
namespaces from earlier movie patches, nonempty debugger method-name differences,
and executable/record differences. These are unresolved; the synthetic fixture
and the 440-script subset do not establish completion of the source workflow.

## Nested functions, debugger control flow, and source corrections

Nested functions now acquire identities through uniquely owned `newfunction`
sites, recursively including nested closures. Shared or unrooted functions
remain ambiguous. Original debugger names can follow these proven identities.
Debugger directive reconstruction now checks executable branch/switch targets
and exception ranges, then relocates them around the original source map.
Regression cases reject altered closure references, branch targets, handler
targets and instructions; they do not replace executable bodies.

The first pinned source correction is `survarium.messages.MessagePremium`.
Its reconstructed source compiles with strict Flex and links to the exact
**5,199-byte ABC**. Reassembling and compressing the complete movie reproduces
SHA-256 `3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60`
and all original file bytes. The normal measurement and linking commands
reproduce that result with the saved, independently verified patch manifest.

The source diff corrects these specific decompiler artifacts:

- Hoisted default initializers become implicit typed declarations, restoring
  their prologue order without redundant conversions.
- A function local is declared at its assignment, restoring register order.
- Object literal keys use identifiers instead of quoted strings, avoiding
  compiler-added string conversions.
- Direct truth tests and typed assignments replace decompiler-added explicit
  conversion calls where the original used implicit coercion.

The explicit `uint(_data.matchId)` argument passed to `ExternalInterface.call`
and the explicit integer conversion used for button positioning are preserved.
The entire-file match verifies those choices, rather than assuming all casts
can be removed. Pristine source remains unchanged; the tracked manifest pins
source, diff and reconstructed-source hashes. Patch tests reject moved context,
wrong hunk positions, wrong release hashes, changed content and escaping paths.
All **11 tests**, including actual Flex recompilation and linking, pass.

The initial 878-script strict Flex sweep has finished: 866 compiled and 12
failed compilation. This run predates native-import repair. The corrected native
sweep remains in progress. These counts and the single corrected feature class
are not a whole-tree result. For example, `ItemData` and `TopMenu` still differ
in protected-field assignment encoding; `TopMenu` also contains manually added
room hooks whose call encoding differs from native Flex. Those differences are
still refused, not normalized away by the source-patch mechanism.

## External player-API catalog dependencies

The generated dependency catalog also lacked references to definitions outside
the movie. The adapter now uses the pinned player API's catalog to recognize
those references. Retesting the twelve full-sweep compile failures resolves
eight without source changes: `ButtonClick`, `ButtonQuestFaction`, `ButtonShort`,
`LobbyChat`, `ListItemRendererStyled`, `PlayerSearchComponent`, `SrvLabel` and
`SrvTextInput`. Their apparent missing `Timer`/`TextField` methods, incompatible
inheritance and three compiler null dereferences came from incomplete type
loading. All eight still fail the exact layout check; compilation alone is not
acceptance. The real compiler fixture now covers inherited `TextField` and
`Timer` fields and method calls, including an exact recompile-and-link round trip.
All eleven tests pass with this extension.

Four source failures remain in this targeted run: `SrpRooms` has missing return
paths and strict numeric-to-string errors, `AmmoShareControll` contains a
decompiler goto marker, `ManageClanWindow` contains escaped keyword-property
syntax, and `ItemCompare` has an implicit downcast rejected by strict Flex.

## Authored room source recovery

Recompiling the repository's `client-patches/rooms/SrpRooms.as` with FFDec 25.1.3
reproduces **every original ABC block directly**, with zero changed method
records. The final reassembled and recompressed movie is byte-for-byte identical
to the pinned release. A per-class diff now records that recovery over the
pristine decompile. This restores authored declarations and explicit `String`
calls lost by decompilation; it does not use old executable records to replace
compiler output. The reconstruction profile records FFDec for this class and
strict Flex for `MessagePremium`.

Private candidate corrections also make `AmmoShareControll`, `ManageClanWindow`
and `ItemCompare` compile, by reconstructing a loop break, removing FFDec's
keyword escape markers, and making the downcast explicit. All three still fail
exact linking. They have not been added to the tracked reconstruction set.

## Source feature build with untouched-code gates

`build_feature.py` now compiles only the classes named by a pinned feature
manifest. Each build first recompiles the corresponding unchanged reconstruction
and requires complete compressed-file equality. The feature linker then preserves
all original constant-pool entries, definitions and signatures, checks every
untouched method record, and checks other ABC blocks. Added constant names retain
the class's original lexical namespace identities. An edited method's ordered
call/construction/cast/coercion/throw sequence must remain identical unless its
precise before/after fingerprints were recorded for review.

The `premium-affordability` source diff adds a balance check when a premium
subscription is selected. The fully reparsed result has **7,758 methods**, with
exactly one changed method: `MessagePremium.onPremiumSelected` (ABC 80, method 5).
All other method records match; only one DoABC tag changes, and the assets,
frame settings, ABC names and loading flags remain unchanged. The edited method's
call/cast fingerprint is unchanged. The resulting local candidate's SHA-256 is
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`.
It has not been installed, shipped or verified in the game.

The full linking pass over the earlier 878-script strict Flex sweep has finished:
**613 exact compressed movies, 253 refused outputs, and 12 compilation failures**.
This pass uses the newer linker but the older compiler import path. The separate
native-import sweep subsequently finished with 864 compiled scripts and 14
failures, including the two non-class imports described below. Its saved compiler
predates the external-player-API fix. The native layout pass finished with
**614 exact compressed movies, 250 refused outputs and 14 compilation failures**;
its linker snapshot predates compact-operation and interface support below.
Neither report proves that all classes
can rebuild together; whole-tree reconstruction remains incomplete.

All **13 tests** pass with the feature gate, including real Flex builds. The
feature regression preserves an unrelated class in the same ABC, accepts a new
string constant, and verifies that an appended dynamic-property lookup uses the
original private namespace identities. Negative cases cover untouched code,
stack metadata, original constant-pool content, another class's body, unreviewed
call changes and incorrect review fingerprints. Container tests separately reject
changes to opaque assets, tag order, DoABC names and loading flags.

An isolated native-client runtime attempt packaged the candidate as a one-file
local overlay and rebuilt server data from the same release patches plus that
overlay. The server started successfully. Wine 11.8 / DXVK 2.7.1 on a separate
Xvfb display reported missing DRI3/EDID display information and crashed with a
division-by-zero error during graphics initialization. The movie's behavior was
not reached, so this is not a runtime pass or evidence of a movie defect. The
managed client, server and display were stopped. Private logs, package hashes,
server data and a startup screenshot are retained for the next runtime attempt.

## Namespace and global-function imports

The native sweep exposed two import failures after successful compilation:
`mx.core.mx_internal` is a namespace script and `stackTrace` is a global function.
Neither has a class definition. The importer and linker now find their owning
script directly and still require a unique simple script from the compiler.

Both now import. The namespace reproduces the complete compressed release movie
after the sole-script-slot rule maps explicit slot 1 to automatic slot 0. Its
name, kind, type and namespace value still match exactly. `stackTrace` remains
refused: its method bodies match, but its script method dispatch ID differs
(original 1, compiled 21). No dispatch-ID rewrite has been added.

The real Flex fixture now compiles and links both a namespace and a global
function, tests the namespace's automatic-slot layout, and rejects slot 2.

## TopMenu source reconstruction and mixed compiler encodings

`TopMenu` now reproduces the **complete compressed release movie** after strict
Flex compilation and layout emission. Its per-class diff restores unqualified
instance-field assignments, identifier object-literal keys and direct truth
tests. Removing the decompiler's explicit `this.` on these assignments makes
ASC emit the original `initproperty` instructions directly. No assignment
opcode normalization was necessary.

Only the two `SrpRooms` hooks then differed. They use compact lookup/call
instructions within otherwise expanded methods. The linker now implements the
two corresponding adjacent-instruction folds from Apache Flex's pinned Encoder:
same-QName lookup pairs and calls whose result is immediately discarded. It
requires the complete method to match after these folds and proves that all
branches, switches and exception boundaries still identify the same output
instruction. It refuses any boundary entering a folded pair. The resulting
instructions retain the compiler's operands; no original method body is copied.

The real compiler fixture exercises both folds in a method containing a loop,
a switch table and an exception handler. It tests rejection of a changed call argument count,
a branch into the discarded-result instruction, and a handler boundary there.
All 13 tests pass. A fresh build from the tracked reconstruction manifest again
reproduces the exact movie for `TopMenu`. Rebuilding `premium-affordability`
with this linker produces the same candidate SHA-256 recorded above: one changed
method, all 7,758 methods reparsed, and identical non-code content. Runtime
verification remains outstanding.

## Interface initializers and debugger parameter names

Five freshly compiled, unmodified framework interfaces now reproduce the complete
compressed release movie: `IAssetLayoutFeatures`, `IFlexAsset`,
`IFlexDisplayObject`, `ILayoutDirectionElement` and `IRepeaterClient` in `mx.core`.
Their shorter interface initializer uses `pushnull` as the base class. The class
binding rule now recognizes that exact seven-instruction shape and locates the
reference binding independently of debugger directives.

Debugger parameter names now follow the original source map for uniquely matched
methods, including when the compiler omitted that optional metadata. Positional
types, defaults, method identity and all non-debug flags remain checked. These
interfaces previously differed in their generated `param1` names as well as the
initializer encoding. No source correction or executable-body substitution is
needed.

All 13 tests pass, including a real compiled interface, debugger directives before
its binding, rejection of a changed scope operand, and restoration of omitted
parameter-name metadata while preserving the original constant pool.

The separate 15-class feature-source probe applied candidate field-assignment,
object-key and truth-test corrections. Every candidate compiled, but none passed
exact linking. Those patches remain private and unpromoted.

## File namespaces left by earlier method patches

`ItemCard` contains both `ItemCard.as$148` and `ItemCard.as$0` file namespaces.
The later `isUpgradeable` method uses the second; native recompilation emits one
new file namespace. Four other feature classes have the same initial ambiguity.

Baseline linking now maps these empty file namespaces by uniquely owned method,
with checks for namespace values, qualified names, declarations and public
namespace-set membership. It refuses methods using multiple original file
namespaces. The real Flex fixture recompiles a class with two such lexical
contexts to the identical movie, and refuses a mixed context or exposed private
QName. Feature output continues to refuse this ambiguity.

The latest targeted run reaches ordinary method differences in `ItemCard`,
`UpgradeWindow`, `ItemControlPanel` and `ItemFactory`; `BottomPanel` reaches a
trait-count mismatch. All five remain rejected. In particular, the corrected
`ItemCard` has six differing method bodies and `ItemControlPanel` has three;
the namespace mapping is not counted as a new exact class rebuild.
All 13 tests pass after the change, including the existing checks for private
namespace ownership, original pool preservation and feature lookup scopes.
