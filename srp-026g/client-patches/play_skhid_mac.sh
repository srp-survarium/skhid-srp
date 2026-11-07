#!/usr/bin/env bash
# macOS (Apple silicon): put beside survarium_skhid.exe, then run: bash play_skhid_mac.sh
# With the Skhid launcher beside it too: bash play_skhid_mac.sh launcher
# The first start downloads Wine and DXMT (Direct3D 11 on Metal), pinned
# below, into ~/Library/Application Support/Skhid; later starts reuse them.
# SKHID_CRASH_DUMP=full: a crash writes the game's whole memory beside it.
# Bash 3.2 (macOS's own): no mapfile, flock or GNU options here.
set -euo pipefail

# The game's language is chosen in its Settings. Server: empty when the
# executable has an embedded default.
server=

# Wine 11 built from CrossOver's sources by Sikarugir: msync (Mach
# semaphores instead of wineserver round trips) and the macdrv hooks DXMT
# needs. The template carries the libraries it links and DXMT's build for it.
ENGINE=WS12WineSikarugir11.0_1
ENGINE_URL=https://github.com/Sikarugir-App/Engines/releases/download/v1.0/$ENGINE.tar.xz
ENGINE_SHA256=67e29fb3d74f363af39c69ba11f9b13a79812c5db07cf4748672658e4a200a0e
TEMPLATE=Template-1.0.27
TEMPLATE_URL=https://github.com/Sikarugir-App/Template/releases/download/v1.0/$TEMPLATE.tar.xz
TEMPLATE_SHA256=dd0778f7b553d63df9b0f4e2cda3f3c6c005aabb75588c84d604afe80cdedbb9

SUPPORT="$HOME/Library/Application Support/Skhid"
RUNTIME="$SUPPORT/runtime/$ENGINE-$TEMPLATE"

die() { echo "Error: $*" >&2; exit 1; }

fetch() {
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --connect-timeout 15 \
        --retry 2 --output "$3" "$1" >>"$log" 2>&1 || die "Download failed: $1. Run again to retry; see $log"
    [[ $(shasum -a 256 "$3" | cut -d' ' -f1) == "$2" ]] || die "Download is not the pinned file: $1"
}

install_runtime() (
    local temporary dxmt arch name
    [[ -f "$RUNTIME/.skhid-ready" ]] && exit 0
    mkdir -p "$SUPPORT/runtime"
    temporary=$(mktemp -d "$SUPPORT/runtime/.download.XXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    echo 'Downloading Wine and DXMT for macOS (260 MB, first launch only)...'
    fetch "$ENGINE_URL" "$ENGINE_SHA256" "$temporary/engine.tar.xz"
    fetch "$TEMPLATE_URL" "$TEMPLATE_SHA256" "$temporary/template.tar.xz"
    mkdir "$temporary/runtime"
    tar -xJf "$temporary/engine.tar.xz" -C "$temporary/runtime" wswine.bundle
    dxmt="$TEMPLATE.app/Contents/SharedSupport/renderer/dxmt/wine"
    tar -xJf "$temporary/template.tar.xz" -C "$temporary" "$TEMPLATE.app/Contents/Frameworks" "$dxmt"
    mv "$temporary/$TEMPLATE.app/Contents/Frameworks" "$temporary/runtime/Frameworks"
    # DXMT replaces Wine's own Direct3D 11 (WINEDLLPATH cannot take over a builtin).
    for arch in i386-windows x86_64-windows; do
        for name in d3d10core.dll d3d11.dll dxgi.dll winemetal.dll; do
            cp "$temporary/$dxmt/$arch/$name" "$temporary/runtime/wswine.bundle/lib/wine/$arch/$name"
        done
    done
    cp "$temporary/$dxmt/x86_64-unix/winemetal.so" "$temporary/runtime/wswine.bundle/lib/wine/x86_64-unix/"
    touch "$temporary/runtime/.skhid-ready"
    rm -rf -- "$RUNTIME"
    mv "$temporary/runtime" "$RUNTIME"
)

configure_wine() {
    local wine_root="$RUNTIME/wswine.bundle"
    # Exported here, in Bash: a system binary between this script and Wine
    # (env, nohup, timeout from /usr/bin) would drop the DYLD_ variables.
    export CX_ROOT="$wine_root" PATH="$wine_root/bin:$PATH"
    export DYLD_FALLBACK_LIBRARY_PATH="$RUNTIME/Frameworks:/usr/lib"
    # Without it this engine's child processes end at their start.
    export SikarugirAppWine11=1
    export WINEMSYNC=1 WINEBOOT_HIDE_DIALOG=1 WINEPREFIX="$PWD/wineprefix" WINEDEBUG=-all
    # Keep HTML enabled (the launcher); qwave disabled as on Linux (the lobby-entry crash).
    export WINEDLLOVERRIDES='mscoree,qwave=;mshtml=b;appwiz.cpl=d;winemenubuilder.exe=d'
    WINE="$wine_root/bin/wine"
    WINESERVER="$wine_root/bin/wineserver"
    "$WINE" --version >>"$log" 2>&1 || die "Wine does not start. See $log"
}

prepare_prefix() {
    if [[ ! -f "$WINEPREFIX/.skhid-ready" || ! -f "$WINEPREFIX/system.reg" ]]; then
        echo 'Initializing Wine prefix...'
        "$WINE" wineboot.exe --init >>"$log" 2>&1 || die "Wine setup failed. See $log"
        "$WINESERVER" -w >>"$log" 2>&1 || true
        [[ -f "$WINEPREFIX/system.reg" ]] || die "Wine registry missing. See $log"
        touch "$WINEPREFIX/.skhid-ready"
    fi
    grep -q '^#arch=win64' "$WINEPREFIX/system.reg" || die "This Wine prefix is not a 64-bit prefix.
Close the game and launcher, rename wineprefix to an unused backup name, and run again."
    "$WINE" 'C:\windows\syswow64\cmd.exe' /d /c 'exit 0' >>"$log" 2>&1 \
        || die "Wine could not run a 32-bit Windows program. See $log"
}

# The main display in points, the size Wine's macOS driver reports.
detect_resolution() {
    local resolution
    resolution=$(system_profiler SPDisplaysDataType -json 2>/dev/null | awk -F'"' '
        /"_spdisplays_resolution"/ { size = $4 }
        /"spdisplays_main" : "spdisplays_yes"/ { print size; exit }' | sed -E 's/^([0-9]+) x ([0-9]+).*/\1x\2/')
    [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]] && echo "$resolution" || echo 1440x900
}

prepare_settings() {
    local config="$SUPPORT/user.cfg" documents resolution
    documents="$WINEPREFIX/drive_c/users/$(id -un)/Documents"
    # Wine links Documents to the Mac's own; the game's files stay in the prefix.
    [[ ! -L "$documents" ]] || rm -- "$documents"
    mkdir -p "$documents/Survarium" "$SUPPORT"
    if [[ ! -e "$config" && ! -L "$config" ]]; then
        resolution=$(detect_resolution)
        printf 'r_fullscreen on\r\nr_resolution %s\r\n' "$resolution" > "$config"
        echo "Fullscreen resolution: $resolution"
    fi
    [[ -f "$config" && -w "$config" ]] || die "Settings file is not writable: $config"
    ln -sfn "$config" "$documents/Survarium/user.cfg"
}

install_calibri() {
    # Optional, as on Linux: without it some UI text (item levels) shows as boxes.
    [[ -f "$WINEPREFIX/drive_c/windows/Fonts/calibri.ttf" ]] && return
    if command -v winetricks >/dev/null && command -v cabextract >/dev/null; then
        echo 'Installing the font Calibri (60 MB download, first launch only)...'
        WINE="$WINE" WINESERVER="$WINESERVER" winetricks -q calibri >>"$log" 2>&1 || true
    fi
    [[ -f "$WINEPREFIX/drive_c/windows/Fonts/calibri.ttf" ]] \
        || echo 'Note: the font Calibri is not installed; some UI text may show as boxes (see MACOS.txt).' >&2
}

# The game must opt in to DEP (NX_COMPAT): without it Wine maps its memory
# executable and Metal crawls, minutes before the first frame (DXMT #161).
check_nx_compat() {
    local pe characteristics
    pe=$(od -An -tu4 -j60 -N4 "$exe" | tr -d ' ')
    characteristics=$(od -An -tu2 -j$((pe + 24 + 70)) -N2 "$exe" | tr -d ' ')
    (( characteristics & 0x100 )) || echo "Warning: $exe does not opt in to DEP (NX_COMPAT);" \
        'on macOS it may take minutes to show its first frame.' >&2
}

start_game() {
    local -a args
    if [[ -f updater_skhid.exe && ! -L updater_skhid.exe ]] \
        && ! "$WINE" updater_skhid.exe ready quiet >>"$log" 2>&1; then
        die "Skhid has not finished updating the game's files (game/resources.db.patch_*), so the game is not started.
Start the launcher (bash Skhid.sh launcher): it finishes the update.
Details: game/binaries/x86/updater_skhid.log"
    fi
    check_nx_compat
    echo "Starting Survarium. Log: $log"
    args=(-mt_allocator=768 -minimum_resources_memory_size=768)
    [[ -z "$server" ]] || args+=("-server=$server")
    "$WINE" "$exe" "${args[@]}" >>"$log" 2>&1 || die "Game exited with an error. See $log"
}

main() {
    local exe=survarium_skhid.exe mode=${1-} log file
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
    [[ -f "$exe" && ! -L "$exe" ]] || die "Put this script beside $exe."
    [[ $# -le 1 && ( -z $mode || $mode == launcher ) ]] || die 'Usage: bash play_skhid_mac.sh [launcher]'
    [[ $(uname -s) == Darwin && $(uname -m) == arm64 ]] || die 'This script is for Apple silicon Macs; on Linux use play_skhid.sh.'
    arch -x86_64 /usr/bin/true 2>/dev/null || die 'Rosetta 2 is needed: softwareupdate --install-rosetta --agree-to-license'
    case ${SKHID_CRASH_DUMP-} in
        '') unset SKHID_CRASH_DUMP ;;
        full) export SKHID_CRASH_DUMP ;;
        *) die "SKHID_CRASH_DUMP=$SKHID_CRASH_DUMP: the one value is full (whole-memory crash dumps)." ;;
    esac
    if [[ $mode == launcher ]]; then
        for file in launcher_skhid.exe updater_skhid.exe; do
            [[ -f "$file" && ! -L "$file" ]] || die "Put $file beside this script to use the launcher."
        done
    fi
    shlock -f .skhid-launcher.lock -p $$ || die 'This launcher is already running.'
    trap 'rm -f .skhid-launcher.lock' EXIT

    log="$PWD/skhid-wine.log"
    install_runtime
    configure_wine
    prepare_prefix
    prepare_settings
    install_calibri
    if [[ $mode == launcher ]]; then
        echo "Starting the Skhid launcher. Log: $log"
        "$WINE" launcher_skhid.exe >>"$log" 2>&1 || die "Launcher exited with an error. See $log"
    else
        start_game
    fi
}

main "$@"
