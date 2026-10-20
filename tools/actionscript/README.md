# ActionScript source reconstruction

Work in progress toward a source rebuild of the lobby movie with **1:1 binary
equality**. The release movie is authoritative. Compiler success, a matching
decompile, or matching instruction bytes without their constant pools do not
prove compatibility.

This directory provides reproducible compiler investigation, exact byte checks,
and a conservative layout linker. Some classes now rebuild to the identical
release movie. It does **not yet provide a whole-tree matching rebuild or a
feature patch builder**. The target includes the entire source tree; the 18-class list is an
initial compiler probe, not the completion boundary.

## Private inputs and pinned tools

Use Python 3.10+, JDK 17+, FFDec 25.1.3's complete distribution, and optionally
[Apache Flex 4.16.1](https://flex.apache.org/download-binaries.html). Keep the
movie, exported source, SDKs, compiler outputs and diagnostics under the ignored
repository-root `private/` directory. Only project-authored tools, release
identities and eventual source diffs belong in git.

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
It keeps the compiler's method bodies, signatures, class definitions, scope and
stack limits, and exception tables. It restores named method and trait ordering.
Class/file private namespaces are mapped by their compiler-generated owners,
requiring unique identities in both pools; ambiguous ownership is refused.
Equivalent multinames can then use their original pool indices. Namespace-set
members must match after relocation; their ordering is not lookup precedence.
Changing reference widths relocates conditional/unconditional branches, switch
tables and exception boundaries by instruction identity. Invalid boundaries
and unsupported control-flow forms are refused. The linker checks every
existing pool entry before retaining its index and removes unused additions.
Empty debug method names and the Adobe editor's `__go_to_definition_help` and
`__go_to_ctor_definition_help` annotations follow the original layout. Runtime
annotations are compared by their complete resolved names, keys and values;
duplicate records may reuse the original index, but changed values are refused.
Added optional debugger parameter names are removed
when the original has none; parameter types, defaults and other flags still
have to match. A sole script class's explicit slot 1 can use the original
automatically assigned slot encoding (0).

There is also one explicit code-generation rule: for a script defining exactly
one class, the generated global binding can use `findpropstrict` for that class
instead of `getscopeobject 0`. The matcher requires the complete straight-line
ASC class-initializer shape, a singleton public namespace, one register, and no
exception handlers or body traits. It emits a new lookup instruction from the
declared class binding; it never copies the original initializer. Unknown
shapes, a different scope operand, and other instruction changes remain rejected.
The original method bodies are never substituted for failed compiler output.
Original debugger file/line/register directives can also be emitted after every
non-debug instruction compares equal to the compiler output. Branch and switch
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
and remaining instruction or verifier-metadata differences. Its success on individual classes
does not prove a whole-tree rebuild. `MEASUREMENTS.md` records the current evidence.

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

The first correction, `MessagePremium`, reproduces the complete compressed
revision 24 movie exactly after compilation and layout linking. Its changes
recover implicit local initialization, declaration order, identifier object
keys, direct truth tests and typed assignments. Explicit casts on external-call
arguments remain present. This proves one corrected class, not the whole tree.

## Remaining work

1. Finish compiler matching and decompiler defects.
   Evaluate all 878 scripts after the initial 18-class experiment.
2. Complete the verified ABC linker's remaining pool, private-namespace,
   anonymous-method and class mappings. Require each compiled method's
   instructions **and metadata** to match after relocation. Never copy an old
   body as a substitute for a failed compilation.
3. Preserve source reconstruction corrections as reviewed diffs over each
   release's pristine tree; keep feature diffs separate from those corrections.
4. Add the feature builder: compile only touched classes and reject changes in
   every untouched class, including untouched methods inside touched classes.
   Calls, casts, exception handling and constant references all need coverage.
5. Prove an unchanged source rebuild reproduces the full movie, demonstrate a
   real source feature edit, and run the in-game Scaleform check. Until then,
   the existing shipping patchers remain the release path.

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
