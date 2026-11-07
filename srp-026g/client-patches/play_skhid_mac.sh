#!/usr/bin/env bash
# macOS (Apple silicon): put beside survarium_skhid.exe, then run: bash play_skhid_mac.sh
# With the Skhid launcher beside it too: bash play_skhid_mac.sh launcher
# The first start downloads Wine (with DXMT, Direct3D 11 on Metal), pinned
# below, into ~/Library/Application Support/Skhid; later starts reuse it.
# SKHID_CRASH_DUMP=full: a crash writes the game's whole memory beside it.
# Bash 3.2 (macOS's own): no mapfile, flock or GNU options here.
set -euo pipefail

# The game's language is chosen in its Settings. Server: empty when the
# executable has an embedded default.
server=

# Wine 11 from CrossOver 26.3's sources, built by athei/wine-build: msync
# (Mach semaphores instead of wineserver round trips) and DXMT. It runs under
# Rosetta, whose x87 is bit-exact: a match's world hashes stay those of a
# Windows game. Its x87sidecar (faster, not bit-exact) desyncs and is kept off.
ENGINE=cx-26.3.0-8
ENGINE_URL=https://github.com/athei/wine-build/releases/download/$ENGINE/wine-$ENGINE-macos-x86_64.tar.xz
ENGINE_SHA256=1976c3310a1f8be2f8e9a46fcb524f77b4316df3d281d94af2c8542bef60456d
# CrossOver's Wine names the Windows user so.
WINE_USER=crossover
# Wine's Mac driver built from those sources with SWTOR on Mac's raw mouse
# patches (LGPL; jdrly/swtor-on-mac, prebuilt/winemac): while the game hides
# the pointer, mouse-look reads the mouse's own counts from GameController,
# without macOS's acceleration.
MOUSE_BASE=https://raw.githubusercontent.com/jdrly/swtor-on-mac/7e008e69eec222cb754479e7b81c70a202e66c14/prebuilt/winemac
MOUSE_FILES="winemac.so:x86_64-unix:c8fa90497077d3e03b9fab6c2ecbe6ce5a25b075804e9b0e7e144a1117ca1468
x86_64-windows/winemac.drv:x86_64-windows:603077293cb95f68ac9c675b5bf43f1e216587d3e9544ac426d006bc52d4d308
i386-windows/winemac.drv:i386-windows:723c6addf3c49d2a76ddfb947dc805f70adb59eae049f8a708231fee7e09d414"
# Microsoft's PowerPoint Viewer 2007, which carries the font Calibri (the
# game's UI asks for it); its cabinet starts at VIEWER_CAB in the file.
VIEWER_URL=https://downloads.sourceforge.net/project/mscorefonts2/cabs/PowerPointViewer.exe
VIEWER_SHA256=c4e753548d3092ffd7dd3849105e0a26d9b5a1afe46e6e667fe7c6887893701f
VIEWER_CAB=592896
GECKO_VERSION=2.47.4
GECKO_SHA256=26cecc47706b091908f7f814bddb074c61beb8063318e9efc5a7f789857793d6

SUPPORT="$HOME/Library/Application Support/Skhid"
RUNTIME="$SUPPORT/runtime/$ENGINE-rawmouse-1"
# The engine is laid out as this app's Contents, its loader the app's
# executable: every Wine process (each starts from that loader) belongs to an
# app of the games category with LSSupportsGameMode, so macOS turns Game Mode
# on for the fullscreen game, and the Dock names it.
WINE_ROOT="$RUNTIME/Survarium.app/Contents"
# The engine's compatdb rules. No x87sidecar for the game: its x87 results
# then differ from Windows players' and lockstep matches desync. The launcher's
# pages need Wine's HTML engine, which the built-in rules turn off (also in
# the wineboot that makes the prefix, so regsvr32 registers it later).
COMPATDB='v=5
name=skhid-game;exe=survarium_skhid.exe;x87_sidecar=false
name=skhid-launcher;exe=launcher_skhid.exe;dll_overrides=mshtml%3Db
name=skhid-regsvr;exe=regsvr32.exe;dll_overrides=mshtml%3Db'

die() { echo "Error: $*" >&2; exit 1; }

fetch() {
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --connect-timeout 15 \
        --retry 2 --output "$3" "$1" >>"$log" 2>&1 || die "Download failed: $1. Run again to retry; see $log"
    [[ $(shasum -a 256 "$3" | cut -d' ' -f1) == "$2" ]] || die "Download is not the pinned file: $1"
}

install_runtime() (
    local temporary contents entry
    [[ -f "$RUNTIME/.skhid-ready" ]] && exit 0
    mkdir -p "$SUPPORT/runtime"
    temporary=$(mktemp -d "$SUPPORT/runtime/.download.XXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    echo 'Downloading Wine for macOS (235 MB, first launch only)...'
    fetch "$ENGINE_URL" "$ENGINE_SHA256" "$temporary/engine.tar.xz"
    mkdir -p "$temporary/runtime/Survarium.app"
    tar -xJf "$temporary/engine.tar.xz" -C "$temporary" wine
    contents="$temporary/runtime/Survarium.app/Contents"
    mv "$temporary/wine" "$contents"
    while IFS=: read -r entry dir sha; do
        fetch "$MOUSE_BASE/$entry" "$sha" "$temporary/${entry##*/}"
        mv "$temporary/${entry##*/}" "$contents/lib/wine/$dir/${entry##*/}"
    done <<<"$MOUSE_FILES"
    # The loader moves to MacOS and is linked back; ntdll.so is found beside it.
    mkdir "$contents/MacOS"
    mv "$contents/lib/wine/x86_64-unix/wine" "$contents/MacOS/wine"
    ln -s ../../../MacOS/wine "$contents/lib/wine/x86_64-unix/wine"
    ln -s ../lib/wine/x86_64-unix/ntdll.so "$contents/MacOS/ntdll.so"
    cat > "$contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key><string>wine</string>
	<key>CFBundleIdentifier</key><string>link.skhid.survarium</string>
	<key>CFBundleName</key><string>Survarium</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.action-games</string>
	<key>LSSupportsGameMode</key><true/>
	<key>NSLocalNetworkUsageDescription</key><string>Not needed to play online: Survarium signs in, plays and updates over the internet.</string>
</dict>
</plist>
PLIST
    touch "$temporary/runtime/.skhid-ready"
    rm -rf -- "$RUNTIME"
    mv "$temporary/runtime" "$RUNTIME"
)

configure_wine() {
    export PATH="$WINE_ROOT/bin:$PATH" WINEPREFIX="$PWD/wineprefix" WINEDEBUG=-all
    export WINEMSYNC=1 WINEBOOT_HIDE_DIALOG=1 WINE_COMPATDB="$COMPATDB"
    # DXMT's shader cache where macOS does not purge it.
    export DXMT_SHADER_CACHE_PATH="$SUPPORT/shader-cache"
    # qwave disabled as on Linux (the lobby's QoS calls fail and stall it).
    export WINEDLLOVERRIDES='qwave=;winemenubuilder.exe=d'
    WINE="$WINE_ROOT/bin/wine"
    WINESERVER="$WINE_ROOT/bin/wineserver"
    mkdir -p "$DXMT_SHADER_CACHE_PATH"
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

set_reg() {
    grep -qsF "\"$2\"=\"$3\"" "$WINEPREFIX/user.reg" && return
    "$WINE" reg add "HKCU\\$1" /v "$2" /d "$3" /f >>"$log" 2>&1 || die "Could not change Wine's settings. See $log"
}

# Wine's Mac driver: a window covering a display captures it (no menu bar or
# Dock over the game, frames straight to the display); Retina mode, so Windows
# and macOS count the same pixels (else the game draws and reads the pointer at
# two scales: a quarter of the picture, the pointer stuck in a quarter); Option
# is Alt (the game's held Alt); raw mouse-look for the game.
configure_mac_driver() {
    local setting
    for setting in CaptureDisplaysForFullscreen LeftOptionIsAlt RightOptionIsAlt RetinaMode; do
        set_reg 'Software\Wine\Mac Driver' "$setting" y
    done
    set_reg 'Software\Wine\AppDefaults\survarium_skhid.exe\Mac Driver' RawMouseWhenCursorHidden y
}

# The main display (the one with the menu bar, where Wine puts its primary
# monitor) in points: with Retina mode a quarter of its pixels, drawn by the
# game and scaled to the display.
detect_resolution() {
    local resolution
    resolution=$(system_profiler SPDisplaysDataType -json 2>/dev/null | awk -F'"' '
        $2 == "_spdisplays_resolution" { size = $4 }
        /"spdisplays_main" : "spdisplays_yes"/ { print size; exit }' | sed -E 's/^([0-9]+) x ([0-9]+).*/\1x\2/')
    [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]] && echo "$resolution" || echo 1440x900
}

prepare_settings() {
    local config="$SUPPORT/user.cfg" documents resolution
    documents="$WINEPREFIX/drive_c/users/$WINE_USER/Documents"
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

# The game's UI asks Windows for Calibri for some text (item levels); without
# it Wine draws boxes. Playable without, so a failure warns and the game starts.
install_calibri() {
    [[ -f "$WINEPREFIX/drive_c/windows/Fonts/calibri.ttf" ]] && return
    echo 'Installing the font Calibri (27 MB download, first launch only)...'
    calibri_from_viewer || echo "Warning: the font Calibri could not be installed (see $log)." \
        'Some UI text, such as the item levels on item cards, may show as boxes. The next start tries again.' >&2
}

calibri_from_viewer() (
    local temporary name lower fonts="$WINEPREFIX/drive_c/windows/Fonts"
    temporary=$(mktemp -d "$SUPPORT/.calibri.XXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    fetch "$VIEWER_URL" "$VIEWER_SHA256" "$temporary/viewer.exe"
    # Called in an || list, where set -e does not apply: each step exits itself.
    # macOS's tar (libarchive) reads cabinets; the viewer's starts at VIEWER_CAB.
    tail -c +$((VIEWER_CAB + 1)) "$temporary/viewer.exe" > "$temporary/viewer.cab" || exit 1
    tar -xf "$temporary/viewer.cab" -C "$temporary" ppviewer.cab 2>>"$log" || exit 1
    tar -xf "$temporary/ppviewer.cab" -C "$temporary" 'CALIBRI*.TTF' 2>>"$log" || exit 1
    printf 'REGEDIT4\r\n\r\n[HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts]\r\n' \
        > "$temporary/calibri.reg"
    for name in CALIBRI:Calibri CALIBRIB:'Calibri Bold' CALIBRII:'Calibri Italic' CALIBRIZ:'Calibri Bold Italic'; do
        lower=$(echo "${name%%:*}" | tr '[:upper:]' '[:lower:]').ttf
        cp "$temporary/${name%%:*}.TTF" "$fonts/$lower.part" || exit 1
        printf '"%s (TrueType)"="%s"\r\n' "${name#*:}" "$lower" >> "$temporary/calibri.reg"
    done
    "$WINE" regedit /S "Z:${temporary//\//\\}\\calibri.reg" >>"$log" 2>&1 || exit 1
    # calibri.ttf last: it is what says the font is in place.
    for lower in calibrib calibrii calibriz calibri; do
        mv "$fonts/$lower.ttf.part" "$fonts/$lower.ttf" || exit 1
    done
)

# The launcher's pages need Wine Gecko, which this engine does not carry.
install_gecko() (
    local gecko="$WINEPREFIX/drive_c/windows/syswow64/gecko/$GECKO_VERSION/wine_gecko" temporary
    [[ -f "$gecko/xul.dll" ]] && exit 0
    temporary=$(mktemp -d "$WINEPREFIX/drive_c/skhid-gecko.XXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    echo 'Installing Wine Gecko for the launcher...'
    fetch "https://dl.winehq.org/wine/wine-gecko/$GECKO_VERSION/wine-gecko-$GECKO_VERSION-x86.msi" \
        "$GECKO_SHA256" "$temporary/gecko.msi"
    "$WINE" 'C:\windows\syswow64\regsvr32.exe' /s mshtml.dll >>"$log" 2>&1 \
        || die "Registering Wine's HTML engine failed. Run again to retry; see $log"
    "$WINE" msiexec /i "C:\\${temporary##*/}\\gecko.msi" /qn /norestart >>"$log" 2>&1 \
        || die "Gecko installation failed. Run again to retry; see $log"
    [[ -f "$gecko/xul.dll" ]] || die "Gecko installation is incomplete. Run again to retry; see $log"
)

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
    configure_mac_driver
    prepare_settings
    install_calibri
    if [[ $mode == launcher ]]; then
        install_gecko
        echo "Starting the Skhid launcher. Log: $log"
        "$WINE" launcher_skhid.exe >>"$log" 2>&1 || die "Launcher exited with an error. See $log"
    else
        start_game
    fi
}

main "$@"
