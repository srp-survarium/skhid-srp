# ActionScript source reconstruction

Revision 24's complete lobby source tree now rebuilds with **1:1 binary
equality**: 878 scripts, 7,758 method bodies and the complete compressed movie.
The rebuilt 40,366,973-byte file has SHA-256
`3ca94f2aa3d56da554e25c3ce24e3c5e6620368942fab728a90a2528c8b18a60`,
identical to the shipped movie.
[Verification evidence](reconstruction/026g-rev24/verification.json) records
the compiler profiles, source/tool hashes, complete reports and artifact.

This directory provides pinned private source exports, 233 reconstruction
diffs over that pristine tree, a verified layout linker, a whole-tree build
command, and a source-feature builder that recompiles only touched classes.
Features may edit method bodies, add methods, traits, nested functions,
constants and whole classes, and change the declarations they name; the
builder checks every untouched method and class byte for byte. The lobby movie
of skhid-srp is built this way from `srp-026g/lobby/` (see
`srp-026g/docs/notes/lobby-source.md`). The original 18-class list remains
an optional compiler probe; the baseline proof covers all 878 scripts.

## Private inputs and pinned tools

Use Python 3.10+, JDK 17+, FFDec 25.1.3's complete distribution, and
[Apache Flex 4.16.1](https://flex.apache.org/download-binaries.html). Keep the
movie, exported source, SDKs, compiler outputs and diagnostics under the ignored
repository-root `private/` directory. Only project-authored tools, release
identities, verification records and source diffs belong in git.

`releases/026g-rev24.json` pins the shipped movie and FFDec exporter. The movie
was checked against the actual `flash_movies/lobby_menu.swf` entry in the
released patch, whose archive hash also matches `release/patches.tsv`.
The private manifest records every exported source hash and the compiler files.
Reports record the exact driver and Java bridge source used for that run.

Create `private/actionscript/tools.json` with absolute paths:

```json
{
  "java": "/path/to/jdk/bin/java",
  "javac": "/path/to/jdk/bin/javac",
  "ffdec": "/path/to/ffdec/ffdec.jar",
  "flex": "/path/to/apache-flex-sdk-4.16.1-bin"
}
```

For Flex, place the player API SWC shipped with FFDec at
`<flex>/frameworks/libs/player/32.0/playerglobal.swc` (FFDec calls it
`flashlib/playerglobal32_0.swc`). Set `target-player` in the SDK's
`frameworks/flex-config.xml` to `32.0`. The player API is a compiler dependency;
the original movie's SWF version and non-code tags are retained. This setup
does not establish that new player APIs are supported by Scaleform.

```sh
python3 tools/actionscript/movie_source.py --tools private/actionscript/tools.json \
  init /path/to/released/lobby_menu.swf private/actionscript/rev24 --release 026g-rev24

python3 tools/actionscript/movie_source.py --tools private/actionscript/tools.json \
  measure private/actionscript/rev24 private/actionscript/ffdec-probe \
  --compiler ffdec --classes tools/actionscript/feature-classes.txt

python3 tools/actionscript/movie_source.py --tools private/actionscript/tools.json \
  measure private/actionscript/rev24 private/actionscript/flex-probe \
  --compiler flex --classes tools/actionscript/feature-classes.txt

python3 tools/actionscript/movie_source.py --tools private/actionscript/tools.json \
  measure private/actionscript/rev24 private/actionscript/flex-strict-probe \
  --compiler flex-strict --classes tools/actionscript/feature-classes.txt
```

Omit `--classes` to investigate every exported script. `--jobs 4` runs independent
compilers concurrently. `--compact` retains compiled code and comparisons but
omits the full candidate movies, which can be reconstructed from those code
files and the pinned original. Unchanged diagnostic directories are shared with
the run's baseline only after every file has compared equal.

Output directories must
be new. Each class is compiled independently from the original movie, including
when a previous compilation fails. Every attempted class gets a log; successful
ones also get the saved candidate, reparsed ABC blocks, method records,
disassembly and a comparison. `report.json` is updated after each class and
records whether the requested run finished. Exit status 1 means at least one
class failed to compile or did not reproduce the uncompressed movie exactly.

The importer currently exports 878 scripts and inventories 7,758 method bodies
from revision 24. `pristine/` is immutable: source additions, deletions or edits
cause subsequent measurements to refuse the project.
Native import and linking identify namespace and global-function scripts by
their exported script path, without requiring a class definition.

## What the comparison proves

`file_exact` compares the complete SWF bytes, including compression.
`movie_exact` compares the complete decompressed SWF, including its version,
frame header, every tag, ABC pools, methods, traits and metadata. Compression
differences are reported separately; decompressed equality does not claim
compressed file equality.

Only ABC tags enter FFDec. The raw headers and bytes of all other tags are
retained, including Scaleform tags FFDec cannot parse correctly. ABC count and
order must remain fixed. Candidates are diagnostic artifacts, never installed
or packaged by this tool.

Method comparisons include code, signatures, stack/register/scope limits,
exception tables and body traits. The disassembly resolves pool operands for
diagnosis. Methods are paired by qualified class/trait name and getter/setter
kind. Nested functions use their uniquely owned `newfunction` call site and
lexical ordinal; shared or unrooted functions remain marked with numeric IDs.
Those IDs can move. A method-level diagnostic is not a semantic or binary proof;
the complete movie comparison is the acceptance test.

## Flex integration

The default `flex` mode uses FFDec's Flex script replacer arguments, including
`strict=false` and `debug=true`. The bridge corrects these integration problems:

- FFDec 25.1.3 selects `mxmlc.exe` on Unix. The bridge invokes the same SDK's
  `mxmlc.jar` directly and records the arguments.
- The temporary dependency SWC lacks inheritance and expression/type dependencies
  in its catalog. The bridge derives these from its ABC definitions and references,
  including external definitions from the pinned player API catalog. Without the
  external edges, Flex can partially load inherited `TextField` or `Timer` types,
  reporting missing methods or crashing during type resolution.
- Single-namespace multinames in the dependency library are represented as
  qualified names for Flex's type importer. This is applied only to the
  temporary dependency SWC. Both original and adapted SWCs are retained.
- Flex's SWC reader crashes on multiname zero in `Vector.<*>`. The dependency
  adapter uses an explicit public `*` name for the same wildcard type. This fixes
  the `CharWindow` dependency needed to compile `LobbyMenu`.
- FFDec's instruction decoder rewrites unreachable jump operands, including
  while importing native compiler output. The bridge retains `native-compiler.swf`
  and imports its instructions through a linear decoder that must reproduce
  their exact bytes before any relocation. Other scripts sharing the target
  ABC are retained. Snapshots use the same exact decoder for disassembly.

`flex-strict` additionally enables strict type binding, disables optimization
and debug output, and preserves `trace` calls. These settings are recorded;
they are an experiment toward matching the original compiler output.

`flex-library` uses those same settings but compiles the requested class as an
imported source unit. A temporary entry class references it, and inheritance-only
linking is disabled so the target is included in the native output. This restores
the compiler's lexical self-class lookups where entry-script compilation would
emit global-slot access. No lookup opcode is rewritten by the layout linker.
The full `native-compiler.swf`, temporary `entry-source.as`, and selected
`native-script.swf` are retained. Selection requires a uniquely named class in a
single-script ABC, and verifies that the selected ABC's bytes survive
serialization unchanged. Only that native ABC replaces the requested script;
the temporary entry and its other linked dependencies never enter the result.
This profile requires a class, rather than a global function or namespace script.

## Exact layout linking

After a measurement, try linking its compiled outputs back into the original
ABC layout. This also works on the finished rows of a running measurement;
the linking report explicitly records that the compilation run is incomplete.

```sh
python3 tools/actionscript/link_layout.py --tools private/actionscript/tools.json \
  --jobs 4 --compact private/actionscript/rev24 private/actionscript/flex-strict-probe \
  private/actionscript/linked-probe
```

The linker verifies each input against its recorded compiler-output movie hash.
Within a linking run, byte-identical complete linked code movies share one
serialization result. The first result is freshly transplanted and compressed;
the original compressed payload is never reused. Different code bytes are
serialized separately, and every row still checks full movie equality. This
avoids repeatedly compressing unchanged assets during a whole-tree measurement.
It keeps the compiler's method bodies, signatures, class definitions, scope and
stack limits, and exception tables. It restores named method and trait ordering.
Class/file private namespaces are mapped by their compiler-generated owners,
requiring unique identities in both pools; ambiguous ownership is refused.
For a file-private helper class, its generated protected namespace follows the
already mapped file identity. Both class names, namespace kinds and generated
owner labels must agree, and each protected namespace must have exactly one
class owner.
For exact baseline linking, earlier method patches may have left several empty
file-private namespaces in one class. The linker can select a method's original
file namespace when that method used exactly one, and the namespaces have no
qualified names, declarations or namespace values. Every containing namespace
set must include the empty public namespace. A method using multiple original
file namespaces is refused. Feature linking still requires one namespace identity
for the whole class, because newly introduced names need an unambiguous scope.
Equivalent multinames can then use their original pool indices. Namespace-set
members must match after relocation; their ordering is not lookup precedence.
Changing reference widths relocates conditional/unconditional branches, switch
tables and exception boundaries by instruction identity. Invalid boundaries
and unsupported control-flow forms are refused. The linker checks every
existing pool entry before retaining its index and removes unused additions.
For a sole global function in a single script without classes, a positive
compiler dispatch hint can be restored to the release's hint of 1. The named
method must match and neither body set may contain `callmethod`. This is not a
general class dispatch-table relocation. Adobe's
[ABC parser](https://raw.githubusercontent.com/adobe/avmplus/master/core/AbcParser.cpp)
ignores this encoded hint when resolving the named method. Complete ABC and
movie equality remain mandatory.
Empty debug method names and the Adobe editor's `__go_to_definition_help` and
`__go_to_ctor_definition_help` annotations follow the original layout. Runtime
annotations are compared by their complete resolved names, keys and values;
duplicate records may reuse the original index, but changed values are refused.
Optional debugger parameter names follow the original source map for a uniquely
matched method; parameter types, defaults and other flags still have to match.
A sole script class, variable or constant's explicit slot 1 can
use the original automatically assigned slot encoding (0), with its name, kind,
type and initial value still checked. Other slot numbers remain unchanged.

For a script defining exactly
one class, the generated global binding can use `findpropstrict` for that class
instead of `getscopeobject 0`. The matcher requires the complete straight-line
ASC class-initializer shape, a singleton public namespace, one register, and no
exception handlers or body traits. It emits a new lookup instruction from the
declared class binding; it never copies the original initializer. Unknown
shapes, a different scope operand, and other instruction changes remain rejected.
Interfaces use the corresponding seven-instruction initializer with `pushnull`
as the base class. Debugger directives preceding the reference binding are
handled separately by the source-map check.
The original method bodies are never substituted for failed compiler output.

An ordinary instance method's initial `getlocal0; pushscope` can be omitted or
deferred past an early-return prefix when that prefix does not need instance
scope. Every lexical lookup must name a uniquely defined public class and be
unshadowed by the declaring class or any ancestor. Missing ancestors, method
aliases, handlers, activations, nested functions, dynamic names and scope
accesses are refused. Branches cannot enter the removed prologue, return into
the prefix or bypass deferred scope setup. The complete method and control-flow
comparison must still match. Both void and value-returning guards are supported;
the returned expression remains the compiler's actual instruction sequence.
This follows ordinary instance lookup in
[Adobe's MethodEnv](https://github.com/adobe/avmplus/blob/master/core/MethodEnv.cpp#L1221);
`with` scopes have different lookup rules and are excluded.

That proof can also defer native compiler initializers for zero-valued locals
and null `Array` locals across the same guards. The crossed prefix may read
only the receiver and parameters; it cannot read or write the delayed locals.
Each initializer must be a complete, distinct local store, and every path
entering the subsequent body must pass through the moved initializers.
Branches into the old initializer block, later backedges into the guards,
other values/types and partial initializer matches are rejected. Scope setup
and local initialization may occur at the same boundary or separate boundaries.
All actual compiler instructions and capacity records are retained. An edited
method that no longer matches retains its ordinary compiler layout; a changed
call/cast ordering still requires explicit feature review.

Some previously patched methods retain spare stack or local capacity that
neither source compiler allocates. An authenticated reconstruction entry can
declare `method_reservations`, mapping full method owner names to positive
`stack` and/or `locals` minimums. These are exact compiler-output identities,
including any disambiguating private-namespace suffix; missing or ambiguous
owners are refused. The separate compiler finishing step keeps
the greater of the actual compiler requirement and each explicit minimum.
It never reads a reference body or changes instructions, scope bounds or
other method records. Measurements retain and hash the raw
`unreserved-code.swf`, the reservation plan and the finished compiler output.
The linker authenticates both reservation and mixed-compiler plans against the
source configuration. Feature builds apply the same minimums to baseline and
edited source, allowing a larger actual compiler requirement to remain larger.

The emitter also supports adjacent-instruction folds used by
[Apache Flex's pinned Encoder](https://github.com/apache/flex-sdk/blob/1644730bd54105c8aab234af7c1a08b2af17865d/modules/asc/src/java/macromedia/abc/Encoder.java):
`findpropstrict` plus `getproperty` of the same QName becomes `getlex`, and
`callproperty` plus `pop` becomes `callpropvoid`. Comparisons followed by a
Boolean branch can use the corresponding combined comparison branch, including
an optional `not` and `convert_b`. This follows the same SDK's
[CodeGenerator.If](https://github.com/apache/flex-sdk/blob/1644730bd54105c8aab234af7c1a08b2af17865d/modules/asc/src/java/macromedia/asc/semantics/CodeGenerator.java)
and [Adobe's interpreter](https://github.com/adobe/avmplus/blob/master/core/Interpreter.cpp).
Operand evaluation order is retained; negated ordered comparisons use `ifnlt`,
`ifnle`, `ifngt` or `ifnge` to preserve unordered `NaN` behavior.
The equivalent combined branch can also be expanded back into that comparison
and Boolean branch, including the same optional negation and conversion.
Both directions can occur within one method.
A whole method must match after only these
encoding changes, including names, argument counts, branches, switch targets and
exception boundaries. No control-flow or exception boundary may enter an
intermediate instruction of a folded or expanded group. The output uses the
compiler's operands and relocated control flow;
verifier limits and all other method records must still pass the exact checks.
The rule does not reconcile other changes within an edited feature method.

The same whole-method check can remove the redundant local store emitted for
`n = ++n` or `n = --n`. It requires a numeric increment/decrement, identical
`int` or `uint` conversions, and the same local on the read and both stores.
That numeric rule does not remove repeated conversions of arbitrary objects. Compiler debugger
directives may label retained executable boundaries; they cannot allow entry
inside a removed instruction group. Register changes, conversion changes,
interior branch targets and interior exception boundaries remain rejected.

Integer local increments/decrements can also use their equivalent
load/integer-operation/`convert_i`/store encoding. The existing stack bound
must accommodate that encoding. A redundant `convert_i` can be emitted before
storing an integer increment/decrement result in a local or activation slot.
Generic Number operations and unsigned conversions are excluded. Both rules
retain the whole-method control-flow and exception checks.

A jump proven unreachable from method entry and every exception handler can
use the fixed zero displacement emitted by older compilers. Reachability
includes conditional branches and all switch targets. The emitted target is
the next compiled instruction boundary; no reference target is copied. A
reachable jump, a different requested displacement, or any other control-flow
mismatch remains refused.

Constructor writes to an own private/protected mutable slot can use
`initproperty` in place of `setproperty`. The emitter proves a direct `this`
receiver through the compiled stack expression and rejects register-zero
reassignment, foreign receivers, const/accessor/inherited traits, dynamic names,
joins, and ambiguous stack operations. Both instructions use the same mutable
slot assignment in [Adobe's runtime](https://github.com/adobe/avmplus/blob/master/core/MethodEnv.cpp#L879).
Two adjacent `coerce` instructions with the same QName type can also fold to
one: the first successful coercion already has that type. The whole-method
branch and exception checks reject entry into the second coercion. A pair
already present in both inputs is retained while other encodings are matched.
An edited
method retains its actual compiler output and still requires call/cast review
when that output differs from the baseline.

Original debugger file/line/register directives can also be emitted after every
non-debug instruction compares equal to the compiler output. Compiler-emitted
directives are replaced or removed only after that comparison. Branch and switch
targets and exception boundaries must identify the same executable instruction;
then their offsets are relocated around the debugger directives. This cannot
repair an executable instruction, branch target, or handler mismatch. Nested
function debugger names require a matching, uniquely owned function identity.

The entire reconstructed ABC must equal the original, and every other ABC must
also remain identical. Only then is linked code written. Original graphics are
reassembled with that code, and CWS movies are recompressed with zlib level 9.
The final SHA-256 and byte comparison must prove full file equality, including
compression. If compression differs, the result is still reported as different.

The current linker refuses unresolved anonymous method identities, ambiguous
private namespace ownership, unmatched constant references, class reordering,
and instruction or verifier-metadata differences. Individual class results
are followed by the separate whole-tree assembly proof described below.
`MEASUREMENTS.md` records the experiments and final verification.

## Source reconstruction diffs

`reconstruction/026g-rev24/manifest.json` pins per-class corrections separately
from future feature changes. Each entry records the pristine, patch and result
hashes and explains the correction. The patch applicator requires exact hunk
positions and context; it does not shift hunks or use fuzzy matching. It preserves
the pristine source's line endings and rejects mismatched release identities.

Pass `--source-patches tools/actionscript/reconstruction/026g-rev24/manifest.json`
to `measure` to compile corrected copies. The run saves the manifest, patches,
corrected inputs and their hashes. `link_layout.py` independently reapplies the
saved patches and verifies the compiler input before linking. The immutable
`pristine/` directory remains the reference for every correction.

The correction for `MessagePremium` reproduces the complete compressed
revision 24 movie exactly after compilation and layout linking. Its changes
recover implicit local initialization, declaration order, identifier object
keys, direct truth tests and typed assignments. Explicit casts on external-call
arguments remain present. `SrpRooms` uses its repository-authored source and
FFDec 25.1.3; it reproduces the original ABCs directly, and the complete compressed
movie after reassembly. Its explicit conversion calls and original declarations
avoid defects introduced by decompilation.
`TopMenu` also reproduces the complete compressed movie: its diff recovers
unqualified instance-field assignments, identifier object keys and direct truth
tests, while the emitter reproduces the room hooks' compact lookup/call encoding.
`MessageApply` restores local declaration order and implicit typed defaults in
`updatePattern`, plus direct truth tests. It also reproduces the complete
compressed release movie.
`MessageAsk` restores lexical declaration order, implicit numeric defaults and
typed expressions, alongside field assignments, object keys and truth tests.
Its ordered comparison also uses the verified compact branch encoding. The
complete compressed release movie matches, including its explicit external-call
argument conversions.

Use `--compiler reconstruction` with the pinned patch manifest to select these
per-class compilers in one measurement. Classes without an override use
`flex-strict`; every report row records the compiler actually used. Explicit
`--compiler ffdec`, `ffdec-source`, `flex`, `flex-strict` or `flex-library` selects that compiler for every
class, for comparison experiments. The per-class profile currently proves 233
reconstructed classes, including easing, button, list, text, message, clan,
debug and tutorial classes. Source fixes also recover local declaration order,
static field slot order and explicit switch breaks where required.
`FontDebugControl.addTextField` needs two initialized declarations after its
unconditional return: Flex retains the unused register slots but emits no
unreachable instructions. Removing those declarations changes the binary.
`QuestWindow` similarly retains an unused target register before its implicit
locals. Typed return temporaries, explicit loop initializers and switch default
fallthrough can also be necessary to reproduce the original instructions.
Some constructors need an explicit `: void` annotation to retain their original
method signatures. Easing functions can require assignments inside expressions,
and loops can require their original update clause and `continue` paths.
Each tracked
patch reproduces its independently
verified compiler input and the complete compressed release movie. Whole-tree
reconstruction remains incomplete.

`flex-mixed` compiles the same authenticated source with both `flex-library` and
`ffdec`. An entry's `composition.ffdec_methods` explicitly lists complete method
owner names to take from FFDec, including their nested functions. Other methods
remain native compiler records. Optional `composition.script_traits: "ffdec"`
selects FFDec's script declaration when both outputs contain the same sole class.
`ItemCard` uses this profile for `isUpgradeable` and its script declaration.
Selections are pinned in the reconstruction manifest; the tool never chooses
methods automatically by comparing them with the release.

Composition imports live constant references, method signatures, activation
traits and exception records from the selected compiler output. It preserves
the authenticated original constant-pool prefix and relocates branches when
operand widths change. Unknown owners, incompatible method layouts and ambiguous
private namespace identities are rejected. Original executable bodies are never
copied. The regular exact-baseline and feature-permission gates run afterward.
This profile supports matching existing method layouts; it does not enable
arbitrary additions of classes or methods.

The measurement retains `compiler-composition.tsv` and its hash. Each
`snapshot-mixed/` directory contains the shared source, selection file, both
compiler outputs and their diagnostics. Feature builds use the same selection
for fresh baseline and edited-source compilations and retain both sets of inputs.

`ffdec-source` uses FFDec's parser and generator with an adapter for explicit
source `String(value)` calls. Once FFDec resolves the callee to its String
intrinsic, the adapter emits `convert_s`, which stringifies null and undefined.
Ordinary String assignments retain `coerce_s`. Local functions shadowing `String`
and `new` expressions keep their original compiler behavior. This operates on
source call nodes before instruction offsets are assigned; it does not rewrite
unidentified coercions in compiled bytecode. `SourceCompiler.java` and the pinned
FFDec files are retained and hashed with the compiler artifacts. The profile
currently requires a simple script with its dependencies embedded in the movie.
`SrpExchange` uses it with the repository-authored revision 24 source.
Mixed builds can explicitly set `composition.ffdec_compiler: "ffdec-source"`
to use this generator for their selected methods. The default remains `ffdec`;
the selected backend is included in the authenticated composition plan and its
named SWF and diagnostics are retained alongside the native compilation.

The source adapter also honors explicit `public::name(...)` calls with no
receiver by selecting the public namespace. It leaves unqualified calls,
other namespace qualifiers, constructors and property assignments alone.
The qualifier comes from the parsed source; no reference opcode determines
the lookup. This allows `public::Object(value)` to retain an actual public
call, while ordinary `Object(value)` keeps the compiler's usual coercion.

With that backend, `composition.ffdec_hidden_static` can name public static
dependency methods as `package.Class/method`. Their declarations are temporarily
hidden from the compiler index to reproduce calls compiled before those APIs
were visible. This preserves the compiler's unresolved namespace lookup.
The declarations are restored before serialization, and composition requires
every dependency ABC to remain byte-identical. Target ABC declarations,
non-method traits, missing methods and ambiguous selections are refused.
The explicit choices are included in the retained, authenticated plan.

## Assembling a whole-tree baseline

Compile every exported script with the reconstruction manifest, link every
result, then assemble one movie from their compiled records:

```sh
python3 tools/actionscript/build_baseline.py --tools private/actionscript/tools.json \
  --reconstruction tools/actionscript/reconstruction/026g-rev24/manifest.json \
  --jobs 4 private/actionscript/rev24 private/actionscript/full-build
```

The final artifact is `full-build/assembled/rebuilt.swf`. The three stages retain
their own inputs, reports and diagnostics. They can also be run separately:

```sh
python3 tools/actionscript/movie_source.py --tools private/actionscript/tools.json \
  measure private/actionscript/rev24 private/actionscript/full-compiled \
  --compiler reconstruction \
  --source-patches tools/actionscript/reconstruction/026g-rev24/manifest.json \
  --jobs 4 --compact

python3 tools/actionscript/link_layout.py --tools private/actionscript/tools.json \
  --jobs 4 --compact private/actionscript/rev24 private/actionscript/full-compiled \
  private/actionscript/full-linked

python3 tools/actionscript/assemble_baseline.py --tools private/actionscript/tools.json \
  private/actionscript/rev24 private/actionscript/full-compiled \
  private/actionscript/full-linked private/actionscript/full-assembled
```

The measurement can exit 1 because ordinary compiler layouts differ before
linking. Its report must nevertheless be complete with every script compiled.
Linking and assembly must both succeed. A subset of scripts, an incomplete run,
a changed source or compiler artifact, or a non-exact linked result is refused.

Assembly starts with empty method-signature, method-body, class and script
tables. Each exported script supplies its own records from its authenticated
linked compilation. Ownership follows script/class traits, initializers and
nested `newfunction`/`newclass` sites. Shared ABCs receive each script's records
from that script's separate compiler output. Every table entry must have exactly
one owner; original executable records cannot fill a missing entry.

Constant pools and non-executable metadata retain the verified linked layout.
The assembled ABCs are serialized and reparsed, then transplanted into the raw
original container and freshly compressed. Acceptance requires equality of the
complete compressed movie. `rebuilt.swf` is the resulting artifact; `report.json`,
`inputs.tsv` and `coverage.tsv` record its source build, hashes and per-script
record coverage. No release installation is performed.

## Building a source feature

Feature patches are separate from reconstruction patches and apply to the
reconstructed source. Their manifest pins the release, reconstruction manifest,
baseline source, patch and resulting source. It names the qualified methods
whose bodies the feature may change; copy those identities from `methods.tsv`.

```sh
python3 tools/actionscript/build_feature.py --tools private/actionscript/tools.json \
  --reconstruction tools/actionscript/reconstruction/026g-rev24/manifest.json \
  private/actionscript/rev24 \
  tools/actionscript/features/premium-affordability/manifest.json \
  private/actionscript/premium-affordability
```

The builder compiles only the classes named by the source diffs. For each, it
first recompiles its unchanged reconstruction and requires the complete
compressed release movie to match exactly. It then compiles the feature source
and checks all other ABCs and every untouched method in the affected ABC,
including other classes sharing that block. Signatures, class/script definitions
and traits must remain exact. Every original pool entry retains its index and
value; new constants are allowed, with their lexical namespaces relocated.

Edited methods also retain the ordered sequence of call, construction, cast,
coercion and throw instructions by default. A feature intentionally changing
that sequence must record the reviewed `calls_casts.before` and
`calls_casts.after` SHA-256 fingerprints on its method permission. Inspect the
before/after assembly before recording an exception; mismatched fingerprints
are refused. These fingerprints check the named instruction sequence, not all
argument data flow or runtime behavior. The exact baseline and untouched-method
checks remain mandatory even with such an exception.

The final movie is independently reparsed. Its method changes must be confined
to the permitted owners; non-code tags, frame settings, ABC names and loading
flags must remain unchanged. A successful build writes `feature.swf`,
`comparison.json`, and `report.json`, with source inputs, compiler artifacts and
per-method hashes retained for review. No release files are installed or packaged.

The included `premium-affordability` example updates the purchase button when
the selected premium subscription exceeds the player's current premium balance.
It changes only `MessagePremium.onPremiumSelected` and preserves its call/cast
sequence. The build has passed these byte checks and a native-game UI check:
Buy is disabled at 999 gold for the 1,000-gold option, enabled at 1,000 and 1,001,
and updates when switching between the 1,000- and 2,500-gold options.
[Runtime evidence](features/premium-affordability/runtime-verification.json)
pins the tested movie, tools and private screenshots. The check completed no
purchase; the current server disables tutorials for all accounts, so the test
exercised its normal lobby flow. Build reports retain `runtime_verified: false`
because compilation alone cannot establish these separate UI observations.

### Build lists: additions, whole classes and new classes

A schema 2 build list (`srp-026g/lobby/lobby.json`) names features in order;
each feature's patches apply to the source the features before it left, and
each touched class is compiled once, from its final source:

```sh
python3 tools/actionscript/build_feature.py --tools private/actionscript/tools.json \
  --reconstruction tools/actionscript/reconstruction/026g-rev24/manifest.json --jobs 4 \
  private/actionscript/rev24 srp-026g/lobby/lobby.json private/actionscript/lobby-build
```

A feature's `feature.json` lists its classes, each of one kind:

- a game class: `{"class", "patch", "methods", "members"}`. `methods` names
  the existing methods whose bodies change (by name; a getter/setter pair
  both); `members` names members the feature adds or redeclares, whose traits,
  signatures and bodies (with nested functions) may change or be new. New
  nested functions of an edited method belong to it.
- a repository-authored class of the release: `{"class", "source"}`. The whole
  file is the source and the feature owns the class: its methods, traits and
  declarations may change and grow.
- a new class: `{"class", "new": true, "source", "abc_with"}`. FFDec compiles it
  into the ABC of class `abc_with`, after all of that ABC's records (the
  `add-script` bridge operation), as the lobby's own classes always were.

New classes compile first, then repository-authored classes, then game
classes (Flex needs the new classes as dependencies). Each existing class
first recompiles its unchanged reconstruction and must reproduce the complete
compressed release movie (`--jobs` runs these in parallel); then the feature
source compiles over the movie the classes before it left, and links.

The link keeps every original record at its index: new methods, bodies,
traits, classes, scripts and metadata come after the original ones, so every
original operand keeps its meaning. The gate (`FeatureGate.java`) then
requires every record the permissions do not name to be byte-identical:
method signatures and bodies, class headers, traits and script records.
Removing a method, a trait or a class is refused, as is changing a trait's
kind; an edited method's activation (closure) traits may change with its body.
Each link writes `feature-linked.swf.methods.tsv` (changed, added, unreviewed).
The final movie is reparsed and compared with the release: every changed or
added method must be one a class link reported, and `changes.tsv` lists them.

Call/cast review works as for schema 1, by method: the build list's
`reviewed_calls` maps `Class.method` to the reviewed `before`/`after`
fingerprints of the final, combined source. `--review-calls` builds without
them, writes every unreviewed change with its before/after assembly to
`review-calls.tsv`, and stops before writing a movie.

`compare_decompiled.py --tools T A.swf B.swf OUT` exports two movies with the
same FFDec and compares them script by script and method by method (with
parameter and local names numbered, so dropped debug names do not count);
`OUT/report.txt` lists every difference for review. It is how the lobby's
source build was compared with the bytecode driver's last movie.

`feature_source.py` shows a class's source as the features leave it, and
records a feature's patch from an edited copy:

```sh
python3 tools/actionscript/feature_source.py --reconstruction R PROJECT BUILD_LIST show CLASS [--before FEATURE] -o FILE
python3 tools/actionscript/feature_source.py --reconstruction R PROJECT BUILD_LIST diff FEATURE CLASS FILE
```

## Verification and boundaries

The complete revision-24 run compiled all 878 scripts afresh, linked all 878
exactly, and assembled every method signature, body, class and script from
the corresponding compiled records. Both a direct file comparison and SHA-256
confirm complete compressed-file equality. The 645 scripts without source
corrections and all 233 reconstructed scripts participate in this build.

The baseline, source-feature example and recorded Scaleform UI check are
verified. The existing shipping patchers remain in place; adopting this
workflow in a release is a separate change. A different release needs its own
pinned export, reconstruction and whole-tree proof. Removed methods, traits
or classes and changed trait kinds are rejected explicitly.

```sh
python3 -m unittest discover -s tools/actionscript -v
# Include the actual Flex dependency and linker regression test:
SRP_AS_TOOLS=/absolute/path/to/private/actionscript/tools.json \
  python3 -m unittest discover -s tools/actionscript -v
```

These tests cover the byte comparison and container preservation, including
negative cases where instruction bytes stay equal but a pool or method
metadata changes. Real compiler measurements are separate private artifacts.
The optional compiler regression uses project-authored ActionScript. It reproduces
the wildcard dependency failure, verifies its repair, tests method reordering,
and checks that changed instructions or stack metadata produce no linked output.
It also checks class-binding emission, automatic script slots and optional
debugger parameter names, including rejection of a different scope, slot,
parameter default or initializer instruction.
The source-map test rejects an altered executable instruction in the method
whose debugger directives would otherwise be restored.
Duplicate runtime metadata records are accepted only when all their values match;
the compiler regression also checks rejection of a changed annotation value.
The real compiler fixture includes private members, runtime property lookup,
forward/backward branches, a switch table and exception handlers. It exercises
native recompilation and exact linking, including a second script/class in the
same ABC. A separately constructed wide-reference fixture exercises instruction
growth/shrinkage; changed owners, namespace members, branch targets and handler
targets are refused. Unreachable jump bytes are included in the exact comparison.
Comparison fixtures exercise folding, expansion and mixed encodings in one
method, including true and false branches, negation, exception boundaries, and
rejection of incorrect NaN behavior or entry into an intermediate instruction.
Feature tests compile an actual source edit with a new constant and an unrelated
class in the same ABC. They reject changes to untouched instructions, verifier
metadata, original pool entries and the retained class. They also reject an
unreviewed constructor-to-call change and incorrect review fingerprints.
`test_feature_additions.py` (also with `SRP_AS_TOOLS`) compiles a real
feature adding a member, a nested function and a constant, and a new class;
it checks that undeclared additions, an unpermitted redeclaration, a changed
untouched method and a removed method are refused, and that an owned class
may change its signatures. `test_feature.py` and `test_feature_source.py`
cover build-list parsing, feature chaining and patch recording.
The scope fixture checks omitted and deferred instance setup, inherited and
local shadowing, incomplete hierarchies, duplicate classes, method aliases,
unsafe scopes and branches. Its reservation checks prove that only the named
capacity fields change, reject invalid plans, retain larger compiler limits
and preserve a separately compiled source edit.
