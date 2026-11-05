# Wine 11.8: Russian selected before launch breaks movement

Reproduced with Wine WoW64 staging 11.8 on X11, independently of Survarium:
with `us,ru` configured and Russian selected at launch, W/A/S/D receive scan
codes `61/6c/6d/6e` instead of `11/1e/1f/20` (hex). Switching to English leaves
the bad codes in place. Digits and Enter remain correct.

`winex11.drv` detects the active XKB group, but `init_keycode_mappings` compares
that layout's table against group-zero symbols. The patch makes detection use
group zero too. Event text translation still uses the selected group: both
Latin and Cyrillic typing work before and after switching. See the
[Wine 11.8 source](https://github.com/wine-mirror/wine/blob/wine-11.8/dlls/winex11.drv/keyboard.c).
This is a local Wine fix; the game's Rust patches and Windows bindings do not
need a compensating remap. Recheck before using another Wine version.

Build the driver with Nixpkgs providing Wine 11.8, then create a separate runtime:

```sh
nix-build tools/wine/driver.nix --arg pkgs 'import <nixpkgs> {}' -o result-wine-driver
python3 tools/wine/make-runtime.py /path/to/wine-11.8 result-wine-driver /new/runtime
```

Point the local launcher at `/new/runtime/bin/wine`. The helper leaves the
installed Wine package untouched. Wine resolves its native loader, preloader
and ntdll paths; those files must be real copies. Other files are symlinked,
with only `winex11.so` replaced. `WINEDLLPATH` alone does not override the
installed driver's priority. Keep the driver output rooted while using it.

The regression probe needs Xvfb, setxkbmap, libX11, libXtst, Python 3 on Linux,
and a Windows Python executable. It sends keys only to its own isolated X
server and uses its own Wine prefixes. It never changes the desktop layout.

```sh
python3 tools/wine/check-keyboard.py --wine /new/runtime/bin/wine \
  --windows-python /path/to/windows/python.exe --work-dir /tmp/wine-keyboard-check \
  --libx11 /path/to/libX11.so.6 --libxtst /path/to/libXtst.so.6
```

Run against stock Wine with `--expect-broken` to confirm the original failure.
The fixed run checks both startup layouts, three layout states per run,
WASD/digits/Enter key-down and key-up scan codes, and actual `WM_CHAR` text.
