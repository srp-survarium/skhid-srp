# ActionScript source reconstruction

Work in progress toward a source rebuild of the lobby movie with **1:1 binary
equality**. The release movie is authoritative. Compiler success, a matching
decompile, or matching instruction bytes without their constant pools do not
prove compatibility.

This directory currently provides the reproducible compiler investigation and
byte comparison. It does **not yet provide a matching rebuild or a feature patch
builder**. The target includes the entire source tree; the 18-class list is an
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

Omit `--classes` to investigate every exported script. Output directories must
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
kind; anonymous methods are explicitly marked with their numeric IDs. Those
IDs can move. A method-level diagnostic is not a semantic or binary proof;
the complete movie comparison is the acceptance test.

## Flex integration

The default `flex` mode uses FFDec's Flex script replacer arguments, including
`strict=false` and `debug=true`. The bridge corrects three integration problems:

- FFDec 25.1.3 selects `mxmlc.exe` on Unix. The bridge invokes the same SDK's
  `mxmlc.jar` directly and records the arguments.
- The temporary dependency SWC lacks inheritance dependencies in its catalog.
  The bridge adds superclass/interface entries from its ABC definitions.
- Single-namespace multinames in the dependency library are represented as
  qualified names for Flex's type importer. This is applied only to the
  temporary dependency SWC. Both original and adapted SWCs are retained.

`flex-strict` additionally enables strict type binding, disables optimization
and debug output, and preserves `trace` calls. These settings are recorded;
they are an experiment toward matching the original compiler output.

## Remaining work

1. Finish compiler matching, including `LobbyMenu` and decompiler defects.
   Evaluate all 878 scripts after the initial 18-class experiment.
2. Build a verified ABC linker that maps compiled pools, private namespaces,
   methods and classes to original identities. Require each compiled method's
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
```

These tests cover the byte comparison and container preservation, including
negative cases where instruction bytes stay equal but a pool or method
metadata changes. Real compiler measurements are separate private artifacts.
