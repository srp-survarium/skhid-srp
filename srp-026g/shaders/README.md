# 0.26g shader sources

HLSL sources of shipped 0.26g shaders, rewritten from the shipped bytecode, and the tools to compile
them into blobs the game loads. The game cannot compile HLSL: it reads the DXBC blobs in
`resources.db` (`shaders/sm_4_0/<family>.<ps|vs>/<permutation>`). What the shaders do and how the
frame uses them: [renderer-026g.md](../docs/notes/renderer-026g.md), sections 4, 12 and 13.

| File | What |
|---|---|
| `sm_4_0/point_light_accumulator.ps` | the point light's deferred accumulator, all 16 permutations |
| `sm_4_0/spot_light_accumulator.ps` | the spot light's, all 16 permutations |
| `sm_4_0/pcss.h` | soft shadows (PCSS) for both, compiled in only with `/D PCSS=1` |
| `fxc.sh` | the compiler, fxc under Wine |
| `fxc.nix` | fetches and unpacks that compiler from Microsoft's installer |
| `lights.sh` | `check`: rebuild and compare with the shipped blobs; `build`: blobs for a resources patch |

## The compiler

Every 0.26g blob's RDEF names `Microsoft (R) HLSL Shader Compiler 9.29.952.3111`: fxc.exe and
D3DCompiler_43.dll of the June 2010 DirectX SDK, flags `/Zpr /O3 /T ps_4_0 /E main`. Nothing of
the SDK is in this repository. `fxc.nix` takes it from Microsoft's own installer, pinned by hash:

```
url     https://download.microsoft.com/download/A/E/7/AE743F1F-632B-4809-87A9-AA1BB3458E31/DXSDK_Jun10.exe
sha256  705271dc83bfee54d9b94e028426e288d5f070784b7446d164f48ecfbb2a02cb  (SRI cFJx3IO/7lTZuU4ChCbiiNXwcHhLdEbRZPSOz7sqAss=)
fxc.exe             0e81a142e52f2852a6e4fa438c8617a4b230d92c306d42a24d5a5d4ea6c73a7b
D3DCompiler_43.dll  2f23182ec6f4889397ac4bf03d62536136c5bdba825c7d2c4ef08c827f3a8a1c
```

`nix build -f srp-026g/shaders/fxc.nix` builds it (the vostok flake's `dxsdk-shader-compiler` is
the same derivation); `fxc.sh` does so itself when `DXSDK_SHADER_COMPILER` is unset. fxc runs
under Wine with the native DLL beside it; Wine's builtin d3dcompiler is a different compiler and
must not be used.

## Checking the sources

```
lights.sh check <stock resources.db>
```

compiles all 32 permutations and compares them with the shipped DXBC. Expected, and the exit
status checks it:

```
                                point   spot
byte-identical (container, checksum and all)   12/16   12/16
B = 1 and C = 1 (SSDO + shadowed)                4/16    4/16
```

The four in the last row have the same instructions, declarations and reflection as the shipped
ones; fxc schedules one `mov r4.w, l(1.000000)` one instruction later. No source order tried moved it.

`lights.sh` reads the shipped blobs with `tools/re-kit/common/dbgrab` (build it with cargo; it needs
`tools/vostok-resources-db`, see `docs/RE-TOOLKIT.md`), or `DBGRAB=<path>`.

## Building blobs

```
lights.sh build <stock resources.db> <out dir>              # stock
lights.sh build <stock resources.db> <out dir> --pcss       # soft shadows
lights.sh build ... --pcss /D PCSS_LIGHT_RADIUS=0.15f       # with an option of pcss.h
```

write `<out dir>/shaders/sm_4_0/<family>.ps/<permutation>`: the shipped blob's include records with
the new DXBC. That folder is what `release/build-patch` packs into a resources patch for a local test.

Nothing here is in `release/content.tsv` yet. Shipping one would be a content source kind of its
own, like `movie`: a row naming the shader family, built by `release/build-content` with
`lights.sh build` and compared with what players have like every other source.
