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

## Compiler-emitted debugger directives

The source-map pass now also handles a compiler that emitted its own debugger
directives. It compares both instruction streams with only debugger directives
excluded, proves matching branch/switch targets and exception boundaries, then
emits the original source map around the actual compiler instructions. An empty
original source map removes compiler-added directives under the same checks.

This makes the pristine-source FFDec results match the complete `isUpgradeable`
method records in both `ItemCard` and `ItemControlPanel`. Their whole classes
remain rejected because surrounding methods differ. The 18-class FFDec pass
still has 17 rejected outputs and one compilation failure; this is not a
whole-class matching gain. Applying the earlier broad candidate source changes
also changes these methods, so that candidate was not promoted.

Qualifying `this as ItemData` with the package name did not resolve that class's
remaining self-class lookup difference. That experiment remains private.
All 13 tests pass. The debugger regression covers replacement and removal of
emitted directives, duplicate directives around a loop/switch/exception handler,
and rejection of an executable change despite matching debugger metadata.

## MessageApply source reconstruction

`MessageApply` now reproduces the complete compressed release movie. Its final
mismatch was in `updatePattern`: decompilation hoisted local declarations ahead
of `offset` and inserted explicit typed defaults. Restoring declaration order
(`offset`, `_body`, `gr`, `rectW`, `rectH`, `offSize`) and implicit defaults recovers
the original registers and integer initialization instructions. Together with
direct truth tests, the tracked diff compiles and links exactly.

The tracked patch is byte-for-byte identical to the verified private compiler
input after application to the pinned pristine source. All four reconstruction
entries and the updated premium feature input manifest pass their hash checks.
Rebuilding the premium feature with the updated manifest and source-map linker
again produces SHA-256
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`, with exactly
one changed method and identical non-code content. Runtime remains unverified.

## Comparison branches and MessageAsk reconstruction

The compact emitter now handles all six comparisons followed by a Boolean
branch, with optional logical negation and Boolean conversion. It retains the
compiler's operand order and branch target. Negated ordered comparisons use the
unordered-aware instructions (`ifnlt`, `ifnle`, `ifngt`, `ifnge`). Whole-method
instruction, control-flow and exception-boundary matching is still required;
stack/register limits and every other method record remain exact checks.

The real Flex fixture covers equality, strict equality and all four ordered
comparisons under both branch polarities, including negated equality. It rejects
a branch or exception boundary entering a folded group, and rejects replacing
`ifnlt` with `ifge`, which would change NaN behavior. All 13 tests pass.

`MessageAsk` now rebuilds to the exact compressed release movie. Its source diff
restores lexical declaration order in `configUI` (outer `sliderpos`, `ammount`,
`totalW`, then nested `icon`), implicit numeric defaults in `update_slider`, and
the original typed count expression. Field assignments, identifier object keys
and truth tests also match. The compact emitter handles its ternary comparison;
explicit `uint` conversions on external-call arguments remain in the source.
The promoted patch reproduces the actual verified compiler input byte for byte.

The premium feature rebuild with all five reconstruction entries again produces
SHA-256 `1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. Whole-tree equality and
runtime behavior remain unverified.

## Broader source corrections

A private probe removed explicit default local initializers from 160 previously
unmatched classes, retaining earlier private proposals where available. Of
these, 157 compiled and three failed; none passed exact linking. Defaults alone
were insufficient. Forty candidates had at most two remaining differing
records. Combining the established own-field assignment, object-key and truth
expression corrections changed 34 of those candidates: all compiled, 21 rebuilt
to the exact compressed release movie, and 13 remained refused.

All 21 successful diffs were reviewed and promoted, bringing the reconstruction
manifest to 26 classes. Their applied bytes match the retained compiler inputs.
The rejected proposals remain private. The final premium feature build with
this manifest still produces
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with exactly one changed method and identical non-code content.

## Native-game premium UI verification

Wine's virtual desktop fixed the earlier Xvfb startup crash. The copied test
executable then proved obsolete (it reported revision 0). Rebuilding from the
hash-checked original with this worktree's patcher provided the full resource
mounting code and a private revision-25 marker for the test overlay. Neither
the release revision nor any published resource changed.

The game loaded the feature movie on the isolated display and created the
private test account. The purchase button was disabled at 999 gold for a
1,000-gold subscription, ignored a click there, and was enabled at exactly
1,000 and at 1,001. With 1,001 gold, changing the selection to the 2,500-gold
subscription disabled it, and changing back to the 1,000-gold option enabled
it. No purchase was completed. The server currently disables all tutorials;
the older housekeeping note's tutorial run could not be exercised through
that server, and no tutorial flags were manually changed.

`features/premium-affordability/runtime-verification.json` records the tested
movie, test executable, server, data and screenshot hashes. The overlay's movie
payload was independently checked against the source-feature output. The private
client, server and Xvfb display were stopped after the check. This verifies the
example's UI behavior; whole-tree source reconstruction remains incomplete.

## Declaration order and expanded exact source set

Twelve of the thirteen nearest rejected reconstructions now pass. Eight recover
local declaration order, loop-counter initialization, typed assignments, inline
object keys or original branch structure. `NameUtil` also removes a decompiler-
added `use namespace mx_internal` while retaining its explicit namespace use.
`ClanWindow` and `Logger` restore static declaration order to match slot IDs.
`TopMenuBar` restores final/default switch breaks. `ButtonOver` preserves both
explicit Boolean conversions on bitmask expressions. Each reviewed patch's
applied bytes match the actual compiler input and reconstruct the full compressed
release movie exactly.

`FontDebugControl` remains refused. After fixing its static slots and local
order, its empty `addTextField` retains five registers in the release while the
compiler emits three. Unused local declarations add executable initialization;
placing them in `if (false)` removes both their initialization and registers.
No register-count relaxation or copied executable body was introduced.

A separate broader probe applied the established source-spelling corrections to
170 previously unmatched classes: 166 compiled, 53 reproduced the full release
movie, 113 were refused by exact linking and four failed compilation. All 53
successful diffs were individually reviewed and promoted; rejected proposals
remain private. Together with the twelve targeted repairs, the tracked manifest
now contains 91 reconstructed classes. These are independent class rebuild
proofs, not a completed whole-tree build or a new whole-tree coverage measurement.

Rebuilding the premium feature with the 91-entry manifest again yields
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code data. The runtime evidence retains
the original test's manifest hashes and records this later build separately.
The game was not rerun: the newly produced movie is byte-identical to the tested
candidate.

## Setter spelling, loop exits and annotation order

A follow-up twenty-class probe produced fourteen exact reconstructions. These
recover unqualified setter assignments (including inherited setters), inline
identifier object keys, local declaration order, implicit wildcard defaults and
typed assignments. All fourteen successful patches were reviewed and their
applied bytes checked against the retained compiler inputs.

`MessageInfoBase` additionally matches after restoring its final switch-case
`break`. Adding an explicit default break as well was rejected because it added
one extra jump. `ButtonGroup` matches after restoring its first-entry loop break,
removing decompiler-exposed iteration temporaries, recovering local declaration
order and retaining required explicit defaults. Its repeated `Event` annotations
must appear in the reverse source order to reproduce the release's metadata
order with Flex. The original metadata-order check remains strict; four other
framework classes with this correction still have differing method bodies.

These sixteen reviewed patches bring the manifest to 107 classes. An isolated
Flex optimization probe for `FontDebugControl` still emits the unused-local
initialization instructions, so it does not repair that rejected reconstruction.

A broader declaration-order proposal run covered 69 remaining classes. Sixty-seven
compiled; two (`TextTipRendererHint` and `ItemDragSlot`) rebuilt exactly and were
reviewed and promoted, bringing the manifest to 109. Sixty-five failed exact
linking. Two failed compilation: `ItemCompare` still needs an explicit type
conversion, and the private `TweenLite` object-key proposal incorrectly removed
quotes around reserved words. No failed proposal was promoted. Declaration order
alone does not resolve the other remaining differences.

The 109-entry premium feature build again reproduces the tested candidate
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code data. Its manifest hashes are
recorded as a subsequent rebuild in the runtime evidence; no new game run is
claimed. Whole-tree reconstruction is still incomplete.

## Reusing serialization of identical linked code

The layout driver now retains one freshly serialized movie per run and reuses it
only when another class's complete linked code movie has identical bytes. Every
class still authenticates its compiler artifact and passes the exact linker;
changed input is serialized independently, and every row checks both raw and
compressed movie equality. The cache holds one entry and is protected across
parallel workers.

Nine container tests pass, including concurrent identical-input reuse, changed
ABC content, and a missing ABC after populating the cache. Re-linking the focused
ten-class source probe with this driver gives the same nine exact results and
one rejection as before, with identical compiled-code, linked-code and movie
hashes for every row. Eight serialization cache hits avoided repeated compression
without changing any result.

## Nineteen further exact source reconstructions

The next ten focused source repairs produced nine exact movies. They recover
implicit typed assignments, local declaration order, explicit loop initialization,
setter spelling, inline object keys and direct truth tests while retaining
required bitmask Boolean casts. `MessageCraftItem` then matched after restoring
its static field slot order: all method records already matched after restoring
its explicit chance initializers at their execution point. The accept-path uint
conversion remains intact. `SrvContextMenu` also needed its static fields reordered.

`FontDebugControl` is now exact. Ordinary unused locals added instructions and
`if (false)` removed their registers, but two initialized declarations after an
unconditional return make Flex retain five registers while omitting unreachable
instructions. The actual compiler output passes the unchanged register, scope,
stack and executable-byte gates. No linker exception was added.

An evidence-guided 76-class proposal run produced six exact movies, 69 linker
rejections and one compilation failure. It considered each method's original
conversion calls and assignment instructions before proposing source changes;
byte equality, rather than that heuristic, determined acceptance. An eleven-class
follow-up for implicit wildcard defaults and declaration order added
`ItemPaperDollSlot`; its other ten variants were rejected. Every successful diff
was reviewed, and its applied bytes were authenticated against the retained
compiler input. The tracked reconstruction manifest now contains 128 classes.

FFDec probes for `CLIK`, `DragManager` and `ItemData` did not produce exact movies.
Adding a separately compiled ItemData definition to a private dependency SWC also
left Flex's same-class global-slot access unchanged. Those experiments remain
private and introduce no instruction-rewriting exception.

The 128-entry premium example build again produces the previously tested movie
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. Runtime evidence records
this build separately from the earlier game observations; no new game run is
claimed. These independent class proofs do not establish whole-tree completion.

## Completed comparison of saved pristine compiler outputs

Re-linking all 878 saved `flex-native-all` compiler results with the current
linker completed with 642 exact movies, 222 rejected differences and 14 scripts
without a usable compiler result. Every exact result reproduced the pinned
compressed movie, not just the executable instructions. The earlier linker
produced 614 exact results from those same inputs.

This run reused authenticated compiler artifacts from the earlier pristine-source
measurement. It did not freshly compile the reconstruction manifest or combine
its classes into a whole-tree build. In particular, earlier compilation failures
remain failures in this report even when subsequent source repairs or dependency
fixes have resolved them in separate measurements. This run started before the
serialization-cache change and completed with its original uncached driver.

## Thirty more exact source reconstructions

A focused thirty-class source probe produced 27 exact movies. It restores typed
return temporaries, explicit local initialization at the original execution
point, local declaration order, assignment syntax, required Boolean conversions,
and final switch breaks. The successful `QuestWindow` source retains an
initialized target declaration after an unconditional return. Flex allocates
its unused register before the other locals without emitting the unreachable
assignment; the original register bounds and debug directives then match.

Three rejected candidates required a second source revision. `CoreList` needs
the zero-counter continue and subsequent break, followed by renderer insertion
outside the conditional. An else branch added an extra jump. `DropdownMenu`
needs fallthrough from its last case to an explicit default break; putting the
break in the case emitted the jump before the default label. `PlayerInfo` already
matched every method, but its `premiumLeft` field needed to follow
`premiumExchangeRate` to recover the original static slot order.

All three follow-ups now reproduce the complete compressed release movie.
The 30 successful patches were reviewed and their applied bytes checked against
the retained real compiler inputs, bringing the manifest to 158 classes.
The linker and its acceptance rules were unchanged. Fourteen local unit tests
pass; the optional standalone compiler integration test was not rerun for these
source-only changes. The 33 real compilation attempts and their independent
exact-link results are retained privately.

The premium example built against the 158-entry manifest again produces
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. Its manifest identities
are recorded separately in the runtime evidence. The game was not rerun because
the rebuilt movie is byte-identical to the previously tested candidate.
Whole-tree reconstruction remains incomplete.

## Equivalent expanded comparison branches

The layout emitter now also generates a Boolean comparison and branch from the
equivalent combined comparison-branch instruction. Optional logical negation and
Boolean conversion are recognized, with the existing comparison predicate table
preserving operand order and unordered NaN behavior. The complete method must
still match. Branches and exception boundaries cannot enter an intermediate
instruction introduced by expansion, and verifier limits remain unchanged.

All 15 tests pass, including the real compiler integration fixture. Independently
constructed references exercise both encoding directions, alternating compact
and expanded comparisons within one method, true and false branches, negation,
and rejection of invalid intermediate boundaries and incorrect NaN predicates.
Re-linking the saved 76-class evidence-guided probe changes only
`CreateClanWindow` from rejected to exact; its mixed comparison forms now match.
The other 75 outcomes are unchanged.

The subsequent 878-script regression completed with the same 642 exact results,
222 differences and 14 earlier compilation failures. Every previously exact
movie retains its hash. There were 641 serialization cache hits. This comparison
reused authenticated pristine compiler artifacts; it is not a fresh whole-tree
source build.

## Nineteen additional reviewed source repairs

Eight of nine focused proposals rebuilt exactly. `ToolTip` and `Extensions`
needed static field order restored. Three easing classes needed assignments
inside their comparison or return expressions to retain the compiler's temporary
registers. Two message classes needed their final switch breaks, and
`MessageApplyExternal` needed its identifier object key.

A five-class follow-up was entirely exact. `PropertiesDescriptor`,
`MessageAskInvite` and `MessageTutorialComplete` needed explicit `: void`
constructor annotations: their bodies already matched, but the omitted source
annotation produced a wildcard return signature. `ItemRendererQSlot` needed both
local defaults implicit in declaration order, and `MessageEnterName` needed an
explicit Boolean initializer and final switch break. `CreateClanWindow` from the
earlier source probe also passed with the verified comparison emitter.

Four of five further loop and expression repairs passed. `Slider` recovers its
switch exits and return temporary, `ItemDescriptor` its constructor signature
and loop continue, `LobbyGameSearch` its typed assignments, and `PaperDollBase`
its missing-slot continue paths. `Animation` then passed after restoring its
static field order; its constructor branches and required Boolean conversion
already matched. The final PaperDollBase source was recompiled after whitespace
cleanup. All successful patches were reviewed and their applied bytes checked
against the real compiler inputs. The manifest now contains 177 classes.

The current repository-authored `SrpExchange` has two methods absent from the
pinned release and was rejected. The source from tag `026g-rev24` matches its
definitions and all but two method bodies. Those methods emit `coerce_s` where
the release contains `convert_s`; this candidate remains private and rejected.
No string-conversion exception was added to the linker.

The premium example built against the 177-entry manifest again produces
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. The new manifest hashes
are recorded as a subsequent rebuild, separate from the earlier game test.
Whole-tree reconstruction remains incomplete.

## Fresh remaining-script compilation and six more source repairs

The 58 remaining non-exchange scripts were freshly compiled with the current
dependency importer, using their best existing source candidates. Two pristine
scripts, `MessageAskSell` and `mx_internal`, now reproduce the complete compressed
release movie. The other results were 53 differences and three compilation
failures. This refresh replaces obsolete dependency-related failures; it is not
a fresh compilation of all 878 scripts.

Six further source repairs pass the complete compressed-movie comparison:

- `MessageBox` restores its final keyboard switch break.
- `SrvLabel` restores the typed return temporary and its register order. The
  hoisted `ConstrainedElement` declaration follows the return, allowing the
  temporary's declaration to stay at its assignment without introducing an
  extra `NaN` initialization.
- `MessageRepair` restores local declaration order without moving executable
  assignments, and lexical compound writes for accumulated repair values.
- `Message` restores direct `for-in` loops, their compiler-owned iteration
  temporaries, the first-renderer break, and explicit coordinate initializers.
- `ManageClanWindow` replaces the decompiler's escaped `default` property with
  legal source, and restores declaration order, truth tests, identifier object
  keys, lexical field writes, and its final switch break.
- `AmmoShareControll` restores its constructor signature, declarations and
  original loop structure. The decompiler's `goto` had obscured an outer `for`
  loop containing item construction and an immediate return. The quick-slot
  failure path continues that loop; the ammunition-slot loop retains its
  explicit `else` and break.

The manifest now contains 183 reviewed patches, each checked against the actual
compiler input that produced its exact result. Together with the 644 distinct
pristine-script successes, the accumulated evidence covers 827 of 878 scripts
individually. The remaining 51 scripts still prevent a whole-tree matching
rebuild. No linker acceptance rule changed for these six repairs.

Fourteen local unit tests pass; the optional compiler integration fixture was
not rerun for these source-only changes. The premium example rebuilt against
the 183-entry manifest again produces
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. Its new manifest
identities are recorded separately from the previous native-game test.

## Imported-unit compiler profile

The `flex-library` profile uses the strict compiler settings with a temporary
entry class that references the requested source class. It disables the
inheritance-only linking option so the native output contains the imported
source unit. This makes ASC emit its native lexical self-class lookups instead
of the entry-script global-slot accesses that prevented several classes from
matching. The layout linker does not rewrite these lookup instructions.

The complete native compiler output, temporary entry source, and selected native
script are retained. Selection requires a unique class in a single-script ABC
and checks its complete bytes again after serialization. The temporary entry
and other linked dependencies are excluded from the imported result. Original
method bodies are never substituted.

All 16 tests pass, including the two real-compiler integration fixtures. The new
fixture reproduces the entry-script mismatch with a self-class cast, rebuilds
the same reference movie exactly through imported-unit compilation, and rejects
a deliberate executable-code change. Complete movie equality also checks that
the temporary entry does not leak into the output.

`ItemData` and `Constraints` now rebuild exactly using their reviewed source
repairs with this profile. `DragManager` additionally required its static field
order restored. `CLIK` needed an implicit wildcard default and its original
static field order. All four pass the complete compressed release-movie check.
The manifest contains 187 reconstructed classes, bringing the accumulated
individual-script evidence to 831 of 878. Whole-tree reconstruction remains
incomplete.

The premium example rebuilt against this manifest retains its tested movie hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. The new identities are
recorded as a subsequent rebuild rather than a new game test.

## File-private helpers, global dispatch hints, and four source repairs

`PopUpManager`, `UIComponent`, `Tween` and `FocusHandler` now reproduce the entire
compressed release movie through imported-unit compilation. Their source repairs
restore static/local declaration order, implicit defaults, lexical writes,
direct truth tests and the original loop/switch structure. `UIComponent` also
restores its typed event return temporary; `Tween` restores typed easing results
without extra conversion calls.

`Tween` contains a file-private helper whose protected namespace embeds the
compiler-generated file identity. The linker carries the already unique file
mapping into that protected namespace only when class names, namespace kinds,
exact generated owner labels and unique ownership all agree. A real-compiler
fixture exercises protected member access and rejects changed owners, namespace
kinds and shared ownership.

A separate rule restores a positive compiler dispatch hint to the release's
hint of 1 for a sole named global function, in one script without classes or
`callmethod` instructions. Named method identity must match. Adobe's
[ABC parser](https://raw.githubusercontent.com/adobe/avmplus/master/core/AbcParser.cpp)
ignores this encoded hint when resolving the named method; complete ABC and
compressed-movie equality remain the acceptance gate. The fixture rejects a
zero compiler hint, a getter in place of the function, and a changed method
reference. Freshly compiled pristine `stackTrace` now passes the complete movie
comparison: its native output used hint 21, while the release uses 1.

All 17 tests pass, including three real-compiler integration tests. The 58-script
saved fresh-compilation report now links three scripts exactly, with 52 differences
and three compilation failures. The manifest contains 191 reviewed source
patches. With 645 distinct pristine-script successes, accumulated individual
proofs cover 836 of 878 scripts. This is not yet a whole-tree matching build.

The premium example rebuilt against the 191-entry manifest retains its tested
movie hash `1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one changed method and identical non-code content. Its new manifest hashes
are recorded separately from the earlier native-game test.

Two more imported-unit source repairs subsequently pass the full movie check.
`ButtonBar` restores its captured parameter name, removing the decompiler's copy
into a second activation slot, along with explicit counter initializers and
final switch/default breaks. `IconContainer` restores its captured parameter
names, declaration order, typed count assignment, named local function and array
field initializer. Allowing the compiler to insert superclass initialization
places that local function's creation after the field initializer and superclass
call, exactly as in the release. No further linker rule was needed.

The manifest now has 193 patches and accumulated individual-script evidence
covers 838 of 878 scripts; 40 remain. Fourteen unit tests pass after these
source-only additions. The 193-entry premium rebuild again has the same tested
movie hash, one changed method and identical non-code content; its manifest
identities are recorded as another rebuild, without claiming another game run.

The final linker was also run over all 878 saved pristine compiler artifacts.
The result remains 642 exact, 222 different and 14 not compiled, with every
previously exact movie hash preserved and no status changes. This is a linker
regression over saved artifacts, separate from the newer compilation that proves
`MessageAskSell`, `mx_internal` and `stackTrace` exact.

## Ammunition window activation and loop reconstruction

`AmmoShareLite` now reproduces the complete compressed release movie. The source
restores its captured parameter name, activation/local declaration order,
executable initializers, named local function, typed assignments and direct
logical operands. Explicit loop counters, continue branches and else branches
recover the original control flow. Lexical compound writes and static field
order complete the match. Every method, activation trait, limit and exception
record passes the existing linker unchanged.

The manifest contains 194 reviewed patches. Together with 645 pristine-script
successes, the accumulated evidence covers 839 of 878 scripts individually;
39 remain before a whole-tree matching build. Fourteen unit tests pass after
this source-only addition. The premium example rebuilt against the final
194-entry manifest retains its earlier tested movie hash, one changed method
and identical non-code content. The new manifest identities are recorded as a
subsequent rebuild; the game was not rerun.

## Twelve further control and profile source reconstructions

Twelve more classes reproduce the complete compressed release movie with the
existing imported-unit compiler and linker:

- `Label`, `ScrollingListSmooth`, `ScrollingListSmoothDynam`, `TileListSmoothR`,
  `ScrollBar`, `ScrollingList`, `TileList` and `Button` restore local declaration
  order, explicit initializers, typed return temporaries, lexical compound writes
  and final switch breaks where required. `ScrollingList` also restores the
  original wrapping branch joins, including its empty STICK branch. Matching
  instruction names alone was insufficient: the branch targets must also match.
- `BitmapAsset` removes the decompiler-added `use namespace mx_internal` directive,
  while retaining explicit namespace qualification on `VERSION`. This restores
  dynamic lookup namespace sets. Its static and local declarations also return
  to their original order.
- `ScrollContent` restores explicit integer initialization and lexical compound
  writes to the scroll target and maximum.
- `Util` restores its typed color return temporary, embedded number-format
  assignments, explicit counters and an `else if` branch join.
- `LocalPlayerProfile` restores block-local declarations at their first
  assignments and the typed inventory-value expression. All debugger directives
  and executable bytes match through the existing verified source-map handling.

The manifest now contains 206 reviewed patches. With 645 distinct pristine
successes, accumulated individual-script evidence covers 851 of 878 scripts;
27 remain. This still does not constitute a whole-tree matching rebuild.
Fourteen unit tests pass after these source-only changes. The premium example
rebuilt against this manifest retains the previously tested movie hash, one
changed method and identical non-code content. Its new manifest identities are
recorded as a subsequent rebuild without claiming another native-game test.

## Paper-doll, notification and friends source reconstruction

`LocalPaperDoll`, `NotificationList` and `FriendsMenu` now reproduce the complete
compressed release movie through the existing imported-unit profile.
`LocalPaperDoll` restores branch order, an empty condition's polarity, explicit
counter initialization, loop continues and a named local function with the
original activation slot order. `NotificationList` restores declaration scopes
and explicit initializers. `FriendsMenu` restores static field order, counters,
constant logical guards and its final input switch break. No linker acceptance
rule changed for these repairs.

The reconstruction manifest contains 209 classes. Accumulated individual-script
proofs now cover 854 of 878 scripts, including 645 pristine successes. The
remaining 24 scripts still prevent a complete matching rebuild. Fourteen unit
tests pass after these source-only additions. The premium feature rebuilt against
the new manifest retains its previously tested movie hash, with one changed
method and identical non-code content; its manifest identities are recorded
separately from the earlier game test.

### TweenLite and victory-screen source reconstruction

`TweenLite` and `VictoryScreen` now pass complete compressed-movie equality
using the current imported-unit compiler and existing layout linker. TweenLite
recovers static slot order, implicit typed initialization, declaration order,
identifier object keys, direct truth tests and lexical field writes. Its overlap
calculation also recovers the assignment inside the final conditional-expression
arm and the original positive skip branch.

VictoryScreen recovers declaration scopes, explicit initializers, string
parameter accumulation and named local functions with the original activation
slots and initialization order. These repairs change source only; the compiler
and linker rules remain unchanged.

The reconstruction manifest now contains 211 authenticated patches. Together
with 645 pristine-script successes, accumulated individual-script evidence covers
856 of 878 scripts; 22 remain. This is not yet a whole-tree matching build.
Fourteen unit tests pass (three compiler integration tests are not rerun for this
source-only checkpoint). The premium feature rebuild retains the previously
runtime-tested movie hash `1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and all non-code bytes identical. The runtime
evidence records byte identity, not a new game run.

### Chat, shop and character source reconstruction

`LobbyChat`, `GangShop` and `CharWindow` pass complete compressed-movie equality
with source repairs and the existing compiler/linker rules. LobbyChat restores
counter initialization, a lexical increment and the early-return branch before
tab activation. GangShop restores static slot order, typed assignments, object
keys, declaration scopes and named local functions with the original activation
slots. CharWindow restores block-scoped declarations and accumulator initializers,
retains the explicit Boolean argument conversion in `handleAntiskills`, and
recovers the unlocked-level branch and missing-skill continue guard.

The manifest contains 214 authenticated patches. The accumulated individual-script
proofs now cover 859 of 878 scripts, including 645 pristine successes; 19 remain.
Whole-tree reconstruction is still incomplete. Fourteen unit tests pass, with
three compiler integration tests not rerun for these source-only edits. The
premium feature rebuild again produces the previously runtime-tested movie
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and identical non-code bytes. The runtime
record explicitly identifies this as a byte-identical rebuild, not a game rerun.

### Explicit compiler composition and ItemCard

`ItemCard` now reproduces the complete compressed release movie. Its source
repairs recover typed assignments and counter initialization. The `flex-mixed`
profile compiles the same source with Flex library mode and FFDec, selects the
actual FFDec `isUpgradeable` method record and sole script-class trait, and keeps
native records elsewhere. The manifest pins these choices. Constant references
are imported from actual compiler records, including signatures, nested
functions, activation traits and exceptions; no reference executable body is
substituted. Existing final equality and feature permission gates are unchanged.

All 19 tests pass, including four real-compiler integration tests. The new fixture
checks independent baseline reconstruction, an edited nested function with new
property/string/integer constants, optional arguments and exception handlers.
It also checks rejection of unknown or duplicate selections, a corrupted
retained constant and an unauthorized edit to a native method. A separate
ItemCard edit probe passes feature checks while failing baseline equality.

The manifest contains 215 authenticated patches. Together with 645 pristine
successes, accumulated individual-script evidence covers 860 of 878 scripts;
18 remain. This is still not a whole-tree matching build. The premium feature
rebuild preserves the previously runtime-tested movie hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and identical non-code bytes. The runtime
record documents byte identity; the game was not rerun.

### Filter and inventory compiler selections

`FiltersWindow`, `ItemRendererInv` and `InventoryItem` now each reproduce the
complete compressed release movie. Their manifests explicitly select FFDec's
filter initialization, scope-badge/rendering and inventory slot/drag methods
from the same source also compiled by Flex. Native source repairs recover local
declaration scopes, implicit defaults, typed assignments and lexical mouse-mask
updates. Both compiler outputs and the selection plans are retained privately.

The manifest contains 218 patches. Together with 645 pristine successes,
accumulated individual-script results cover 863 of 878 scripts; 15 remain.
Whole-tree reconstruction remains incomplete. Fifteen unit tests pass, with the
four real-compiler integration tests not rerun for this source-only checkpoint;
all four passed at the preceding tooling checkpoint. The premium rebuild again
has the previously runtime-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
one authorized method changed and identical non-code bytes. Its runtime record
identifies a byte-identical rebuild, not a new game run.

### Item control panel and redundant numeric assignments

`ItemControlPanel` now reproduces the complete compressed release movie after
fresh compilation. Its reconstruction selects FFDec's `updateData` and
`isUpgradeable` records from the same source also compiled by Flex. An explicit
Boolean conversion and numeric self assignment retain the original operations.
The emitter removes only the duplicated store in a numeric `n = ++n` or
`n = --n` when both conversions and all three local-register references agree.
The full method, branches and exception boundaries must still match. Debugger
directives can label retained boundaries but cannot admit entry to a removed
instruction. This does not copy reference instructions or change verifier limits.

All 19 tests pass. The real compiler fixture now additionally checks signed and
unsigned self assignments with debugger directives, and refuses changed
registers, changed conversions, a non-numeric producer, and branch or exception
boundaries inside the removed group.

The manifest contains 219 patches. Combined with 645 pristine successes, the
accumulated individual-script evidence covers 864 of 878 scripts; 14 remain.
Whole-tree reconstruction is incomplete. The premium rebuild retains the
previously runtime-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
one authorized method changed and identical non-code bytes. Its recorded evidence
is byte identity, not a new game run.

### Explicit String source calls and SrpExchange

The `ffdec-source` profile now reconstructs `SrpExchange` from its
repository-authored revision 24 source. The regular FFDec result differed at
five explicit `String(...)` calls. A source AST adapter lets FFDec resolve each
callee, then emits `convert_s` for its resolved String intrinsic. This preserves
the source's null/undefined stringification and leaves typed String coercions
alone. The adapter visits nested source fields and preserves constructor nodes,
source metadata and all other call generation. It never uses a reference
executable instruction to determine which conversion to emit.

Fresh compilation passes the complete compressed release equality check.
All 20 tests pass, including five real-compiler integration tests. The new
fixture verifies explicit null/undefined calls, nested calls and functions,
assignments through call results in both branches, ordinary String assignment,
a shadowing function parameter and static/dynamic constructors. It also checks
independent baseline reconstruction, an authorized source edit, rejection of
that edit as a baseline, and rejection of invalid source before output is saved.

There are now 220 authenticated reconstruction patches. With 645 pristine
successes, accumulated individual-script evidence covers 865 of 878 scripts;
13 remain. This is not a whole-tree matching build. The premium rebuild retains
the previously runtime-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
one authorized method changed and identical non-code bytes. The runtime record
documents byte identity without claiming another game run.

The mixed profile also accepts an explicit `ffdec_compiler: "ffdec-source"`
selection. Its plan pins that backend and retains the separately named output.
The mixed integration test passes independent baseline reconstruction with both
secondary backends, including nested functions; all 15 local unit tests pass.
Reserved plan directives cannot be supplied as method owners. No additional
reconstruction is counted from the ongoing notification/matchmaking probes.

### ItemInfoRenderer source reconstruction

`ItemInfoRenderer` now compiles to the exact complete compressed release movie.
The mixed plan explicitly selects `scopeHintText` and the data setter from the
`ffdec-source` backend. Source changes preserve explicit String conversion and
numeric self assignment, and place a Boolean conversion at the merge of the
whole short-circuit expression. No additional emitter rule was needed.

There are now 221 authenticated reconstruction patches. Together with 645
pristine successes, individual-script evidence covers 866 of 878 scripts;
12 remain, and the whole-tree matching build is still outstanding. The fresh
premium rebuild has the previously runtime-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and identical non-code bytes. Its runtime
record documents a verified rebuild, not another game run.

### Integer encodings and notification source reconstruction

`NotificationListScrollContent` now reproduces the entire compressed release
movie from source. Its mixed plan selects the feature methods from
`ffdec-source`; native UI methods remain compiled by Flex. The source recovers
static declaration order, typed Array return temporaries, explicit property
conversions and comparisons that preserve the original NaN behavior.

Two integer encoding rules cover local and activation-slot loops. Integer
local increments/decrements can expand to load/integer operation/`convert_i`/
store, with no increase to verifier limits. An already integer result can
retain the redundant conversion before its store. Adobe's interpreter uses the
same integer implementation for both local and stack operations
([source](https://github.com/adobe/avmplus/blob/master/core/Interpreter.cpp#L1426)).
Generic numeric operations and unsigned conversions remain excluded, and the
entire method's branch, switch and exception boundaries must agree.

The remaining notification difference was an unreachable jump after return.
The emitter can give such a jump the fixed zero displacement used by the
reference encoding only after proving it unreachable from method entry and
all exception handlers. Reachability follows all branches and switch targets.
The generated target is the next compiled boundary; no original executable
operand is copied. The complete method still has to match.

All 20 tests pass, including five real-compiler integration tests. Additional
fixtures cover short and wide local registers, activation slots, increment and
decrement, changed operands/conversions, generic numeric operations, stack
bounds, and branch/handler entry into generated instructions. A real compiler
try/return fixture exercises the unreachable jump rule; reachable jumps through
normal flow, an explicit branch, a switch or an exception handler are rejected.
The added switch case also passes in a separate rerun of the mixed integration
test.

There are now 222 authenticated reconstructions and 645 pristine successes:
867 of 878 scripts have individual exact proofs, leaving 11. A complete matching
whole-tree build is still outstanding. The fresh premium rebuild again has
hash `1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with exactly one authorized method changed and identical non-code bytes. This
records byte identity with the earlier game-tested build, not a new game run.

### Explicit dependency declarations and two more reconstructions

`UpgradeSlot` and `ItemFactory` now independently reproduce the complete
compressed release movie. Their mixed plans select `updateSlot` and
`setItemsCompatibility`, respectively, from `ffdec-source`. Native source
reconstruction recovers declaration order, typed temporaries and conversions,
unsigned loop counters, and retained loop-update structure.

Each plan names one public static dependency method in
`ffdec_hidden_static`: `UpgradeWindow/fitText` or
`UpgradeWindow/addCompatibility` (with its full package). Hiding that declaration
from the compiler index preserves the unresolved namespace lookup in the
original call. The dependency declaration is restored before output. No
executable dependency code is changed, and the existing composition gate checks
all non-target ABC bytes. Missing or ambiguous methods, non-method traits,
target ABC declarations, duplicate entries and malformed paths are refused.
The choices are pinned in the manifest and retained composition plan.

The real-compiler fixture builds separate dependency ABCs and checks independent
baseline equality, the intended unresolved call alongside an ordinary typed
call, unchanged dependency bytes, preservation of a deliberate source edit,
feature-gate acceptance and baseline-gate rejection of that edit. It also
checks rejection of invalid API selections and restoration after parser or
selection failure. The full run passed the other 20 tests; after correcting a
test input that the parser accepted, the new integration test passes too
(21 tests total, including six compiler integration tests).

There are now 224 authenticated reconstructions and 645 pristine successes:
869 of 878 scripts have individual exact proofs. Nine remain, and the complete
whole-tree matching build is still outstanding. The premium rebuild retains
hash `1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and unchanged non-code bytes. The runtime
record describes a rebuild identical to the earlier game-tested output.

### Explicit public source calls and MatchMakingSettings

`MatchMakingSettings` now rebuilds to the exact complete compressed release
movie. Source repairs recover the tournament-selection branch, static slot
order, a typed Array return temporary, explicit integer assignment, and
`String(play.label).toUpperCase()`. Its remaining conversion uses the explicit
source call `public::Object(this.parent)`.

For a receiver-free call with an explicit public qualifier, the source AST
adapter now resolves that callee in the public namespace. FFDec otherwise falls
back to a set of all open namespaces in this packaged class. The normal
compiler still emits the call and its arguments. This follows the source's
namespace qualification ([Adobe language guide](https://help.adobe.com/en_US/as3/learn/as3_learning.pdf));
no reference instruction chooses its namespace or opcode. Unqualified calls,
other qualifiers, constructors and property assignments retain their existing
generation paths.

All 21 tests pass, including six real-compiler integrations. The source fixture
now lives in a named package, exercises the public Object call and public
method calls with used/discarded results, and verifies unchanged ordinary
Object coercion, ordinary calls, private-qualified calls and constructors.
Independent baseline equality and source edits to both String conversion and
the public Object call pass their respective baseline/feature gates.

There are now 225 authenticated reconstructions and 645 pristine successes:
870 of 878 scripts have individual exact proofs. Eight remain, and the complete
whole-tree matching build is still outstanding. The premium rebuild has the
same previously game-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
one authorized method changed and identical non-code bytes. No new game run
is claimed for this byte-identical rebuild.

### BottomPanel and UpgradeWindow source reconstruction

Both classes now independently reproduce the complete compressed release movie.
`BottomPanel` uses the source compiler with restored static field order, direct
NaN initializers and a single Boolean conversion in `matchesCanJoin`.
`UpgradeWindow` uses an explicit mixed plan over the same source: native methods
recover local declarations and lexical assignments, while selected source
compiler methods retain unsigned loop assignments and explicit Boolean
conversions. Its static fields now occupy their original slots.

No compiler or emitter change was needed. The 15 unit checks pass; the six
compiler integrations remain covered by the preceding full test run. The fresh
premium build again changes exactly one authorized method, preserves non-code
bytes, and has the previously game-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`.
The runtime record identifies this as a byte-identical rebuild, not a new game run.

There are now 227 authenticated reconstructions and 645 pristine successes:
872 of 878 scripts have individual exact proofs. Six remain, and a complete
matching whole-tree build is still outstanding.

### ItemCompare constructor and captured declarations

`ItemCompare` now independently rebuilds the complete compressed release movie.
Its mixed plan selects only the actual source-compiled constructor; all other
methods come from native compilation of the same source. Source reconstruction
recovers named local functions, captured-variable slot order, ordinary local
declarations, static field order, typed assignments, explicit public `push`,
and an `else` branch retained by the original compiler.

Two guarded encodings complete the constructor. A write to an own mutable
private/protected slot may use `initproperty` instead of `setproperty` when a
backward stack walk proves its receiver is unchanged register zero. Const,
accessor, inherited, ambiguous and dynamic traits are excluded, as are foreign
receivers and control-flow joins inside that expression. Adobe's implementation
uses the same mutable-slot assignment for both instructions
([MethodEnv](https://github.com/adobe/avmplus/blob/master/core/MethodEnv.cpp#L879),
[Toplevel](https://github.com/adobe/avmplus/blob/master/core/Toplevel.cpp#L690)).
Adjacent coercions to the same QName type can fold to one; the first successful
coercion already has that type. Complete branch and exception checks still
reject entry into the removed instruction.

All 22 tests pass, including seven real-compiler integrations. The new fixture
checks independent baseline equality, rejected trait/receiver/control-flow
mutations, source-edit preservation, and call/cast review for an edited method
that retains the actual compiler's repeated coercion. The final register-operand
guard additionally passes its focused integration run, including writes,
increments, kills and `hasnext2` changes to register zero.

There are now 228 authenticated reconstructions and 645 pristine successes:
873 of 878 scripts have individual exact proofs. Five remain; a complete
matching whole-tree build is still outstanding. The fresh premium rebuild
retains the earlier game-tested hash
`1c7a3e7b9a51f4519d8bfa550cd48038f460fd4d9370232e844503039d1ce6e3`,
with one authorized method changed and identical non-code bytes. The runtime
record describes byte identity with that earlier run, not a new game check.
