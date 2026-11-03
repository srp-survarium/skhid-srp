#!/usr/bin/env bash
# Put beside survarium_skhid.exe, then run: bash play_skhid.sh
# With the Skhid launcher beside it too: bash play_skhid.sh launcher
# (it looks for newer Skhid files, and its PLAY button starts the game).
set -euo pipefail

# The game's language is chosen in its Settings. Server: empty when the
# executable has an embedded default.
server=

die() { echo "Error: $*" >&2; exit 1; }
require_command() { command -v "$1" >/dev/null || die "Install $1 first."; }

detect_resolution() {
    local resolution output status
    # On X11, prefer the primary monitor's preferred (usually native) mode.
    if [[ ${XDG_SESSION_TYPE-} != wayland ]] && command -v xrandr >/dev/null; then
        resolution=$(xrandr --query 2>/dev/null | awk '
            /^[^ ]/ { active = ($2 == "connected" && $0 ~ / [0-9]+x[0-9]+[+-]/); primary = ($3 == "primary") }
            active && $1 ~ /^[1-9][0-9]*x[1-9][0-9]*$/ && /\+/ {
                if (!first) first = $1
                if (primary && !preferred) preferred = $1
            }
            END { print preferred ? preferred : first }
        ') || resolution=
        if [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
            echo "$resolution"
            return
        fi
    fi
    # Wayland/no xrandr: the first enabled connector lists its preferred mode first.
    for output in /sys/class/drm/card*-*; do
        [[ -r "$output/status" && -r "$output/enabled" && -r "$output/modes" ]] || continue
        read -r status < "$output/status" || continue
        [[ $status == connected ]] || continue
        read -r status < "$output/enabled" || continue
        [[ $status == enabled ]] || continue
        read -r resolution < "$output/modes" || continue
        if [[ $resolution =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
            echo "$resolution"
            return
        fi
    done
    echo 1920x1080
}

configure_wine() {
    local version
    require_command wine
    require_command wineserver
    export WINEPREFIX="$PWD/wineprefix" WINEDEBUG=-all
    WINE=$(command -v wine)
    WINESERVER=$(command -v wineserver)
    export WINE WINESERVER
    # Keep HTML enabled; install Gecko explicitly instead of Wine's addon dialogs.
    # qwave remains disabled for the lobby-entry crash workaround.
    export WINEDLLOVERRIDES='mscoree,qwave=;mshtml=b;appwiz.cpl=d;winemenubuilder.exe=d'
    # Wine 11 supports forcing the new WoW64 loader even in a mixed installation.
    # win64 alone selects a prefix architecture, not the loader used for 32-bit EXEs.
    export WINEARCH=wow64
    unset WINELOADER
    log="$PWD/skhid-wine.log"

    version=$(LC_ALL=C "$WINE" --version 2>&1) || die "Cannot query Wine: $version
Install Wine 11.0 or newer with modern WoW64 support (tested: 11.8 Staging WoW64).
See LINUX.txt."
    [[ $version =~ ^wine-([0-9]+)\.([0-9]+)([^0-9].*)?$ ]] || die "Unrecognized Wine version: $version
Use Wine 11.0 or newer with modern WoW64 support; see LINUX.txt."
    (( 10#${BASH_REMATCH[1]} >= 11 )) || die "Unsupported Wine: $version
Wine 11.0 or newer with modern WoW64 is required (tested: 11.8 Staging WoW64).
Select its wine and wineserver in PATH; see LINUX.txt."

    # Wine 11 uses the kernel's ntsync driver when /dev/ntsync exists. Without it
    # wineserver does all the thread synchronisation and the lobby can crawl.
    if [[ ! -e /dev/ntsync ]] && command -v modinfo >/dev/null && modinfo ntsync >/dev/null 2>&1; then
        echo "Info: /dev/ntsync is missing, so the lobby may run very slowly. Run: sudo modprobe ntsync" >&2
        echo "  (LINUX.txt: how to keep it after a restart.)" >&2
    fi

    printf 'Wine runtime: %s; required mode: WINEARCH=wow64\n' "$version" >>"$log"
}

verify_prefix_arch() {
    local prefix_arch
    prefix_arch=$(awk '/^#arch=/ { sub(/^#arch=/, ""); sub(/\r$/, ""); print; exit }' "$WINEPREFIX/system.reg")
    [[ $prefix_arch == win64 ]] || die "This Wine prefix is not a supported 64-bit prefix for modern WoW64.
Close the game and launcher, rename wineprefix to an unused backup name, and run again.
Your existing prefix has been kept. See LINUX.txt for preserving game settings."
}

prepare_prefix() {
    if [[ -f "$WINEPREFIX/system.reg" ]]; then
        verify_prefix_arch
    fi
    if [[ ! -f "$WINEPREFIX/.skhid-ready" || ! -f "$WINEPREFIX/system.reg" ]]; then
        rm -f -- "$WINEPREFIX/.skhid-dxvk"
        echo 'Initializing Wine prefix...'
        timeout 180 "$WINE" wineboot.exe --init >>"$log" 2>&1 || die "Wine setup failed in modern WoW64 mode. See LINUX.txt and $log"
        timeout 180 "$WINESERVER" -w >>"$log" 2>&1 || die "Wine setup timed out. See $log"
        [[ -f "$WINEPREFIX/system.reg" ]] || die "Wine registry missing. See $log"
        verify_prefix_arch
        touch "$WINEPREFIX/.skhid-ready"
    fi
    # Exercise a 32-bit PE under the forced modern loader. A 64-bit cmd.exe would
    # not prove the mode needed by the game. Refuse failures without an old-mode retry.
    timeout 60 "$WINE" 'C:\windows\syswow64\cmd.exe' /d /c 'exit 0' >>"$log" 2>&1 || die "Modern WoW64 could not run a 32-bit Windows program.
Use Wine 11.0 or newer with modern WoW64 support and matching wineserver.
For an old prefix, keep it as a backup and create a fresh one; see LINUX.txt and $log."
}

prepare_settings() {
    local config_home=${XDG_CONFIG_HOME:-$HOME/.config} settings config documents resolution
    # XDG paths must be absolute; ignore invalid overrides.
    [[ $config_home == /* ]] || config_home="$HOME/.config"
    config="$config_home/skhid/user.cfg"
    documents="$WINEPREFIX/drive_c/users/$(id -un)/Documents"
    [[ ! -L "$documents" ]] || rm -- "$documents"
    mkdir -p -- "$documents/Survarium" "$config_home/skhid"
    settings="$documents/Survarium/user.cfg"
    if [[ ! -e "$config" && ! -L "$config" ]]; then
        resolution=$(detect_resolution)
        printf 'r_fullscreen on\r\nr_resolution %s\r\n' "$resolution" > "$config"
        echo "Fullscreen resolution: $resolution"
    fi
    [[ -f "$config" && -w "$config" ]] || die "Settings file is not writable: $config"
    ln -sfnT -- "$config" "$settings"
}

install_dxvk() {
    if [[ ! -f "$WINEPREFIX/.skhid-dxvk" ]]; then
        require_command winetricks
        echo 'Installing DXVK...'
        winetricks -q dxvk >>"$log" 2>&1 || die "DXVK installation failed. See $log"
        touch "$WINEPREFIX/.skhid-dxvk"
    fi

    export WINEDLLOVERRIDES="$WINEDLLOVERRIDES;d3d11,dxgi=n,b"
}

install_gecko() (
    local version=2.47.4 checksum=26cecc47706b091908f7f814bddb074c61beb8063318e9efc5a7f789857793d6
    local gecko="$WINEPREFIX/drive_c/windows/syswow64/gecko/$version/wine_gecko" temporary
    # The launcher is 32-bit; Wine's own MSHTML declares the Gecko version it needs.
    LC_ALL=C grep -aqF "Wine Gecko $version" "$WINEPREFIX/drive_c/windows/syswow64/mshtml.dll" \
        || die "This Wine needs another Gecko version; update the Skhid setup script. See $log"
    if [[ -f "$gecko/xul.dll" && -f "$gecko/VERSION" && $(<"$gecko/VERSION") == "Wine Gecko $version" ]]; then
        return
    fi
    require_command curl
    require_command sha256sum
    temporary=$(mktemp -d "$WINEPREFIX/drive_c/skhid-gecko.XXXXXX")
    trap 'rm -rf -- "$temporary"' EXIT
    echo 'Installing Wine Gecko for the launcher...'
    curl --fail --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 15 --max-time 300 --retry 2 --output "$temporary/gecko.msi" \
        "https://dl.winehq.org/wine/wine-gecko/$version/wine-gecko-$version-x86.msi" >>"$log" 2>&1 \
        || die "Gecko download failed. Run again to retry; see $log"
    printf '%s  %s\n' "$checksum" "$temporary/gecko.msi" | sha256sum -c - >>"$log" 2>&1 \
        || die "Gecko download checksum failed. Run again to retry; see $log"
    WINEDLLOVERRIDES="$WINEDLLOVERRIDES;winedbg.exe=d" \
        timeout 180 "$WINE" msiexec /i "C:\\${temporary##*/}\\gecko.msi" /qn /norestart >>"$log" 2>&1 \
        || die "Gecko installation failed. Run again to retry; see $log"
    [[ -f "$gecko/xul.dll" && -f "$gecko/VERSION" && $(<"$gecko/VERSION") == "Wine Gecko $version" ]] \
        || die "Gecko installation is incomplete. Run again to retry; see $log"
)

start_game() {
    local -a args
    # resources.db's patches, the ones the server lists, must be in place first:
    # the exe of a release needs the content of its patches. The updater checks
    # them (it reads only); no updater beside this script, as without the
    # launcher: no check.
    if [[ -f updater_skhid.exe && ! -L updater_skhid.exe ]] \
        && ! "$WINE" updater_skhid.exe ready quiet >>"$log" 2>&1; then
        die "Skhid has not finished updating the game's files (game/resources.db.patch_*), so the game is not started.
Start the launcher (bash Skhid.sh launcher, or bash play_skhid.sh launcher): it finishes the update.
Details: game/binaries/x86/updater_skhid.log"
    fi
    echo "Starting Survarium. Log: $log"
    args=(-mt_allocator=768 -minimum_resources_memory_size=768)
    [[ -z "$server" ]] || args+=("-server=$server")
    "$WINE" "$exe" "${args[@]}" >>"$log" 2>&1 \
        || die "Game exited with an error. See $log"
}

main() {
    local exe=survarium_skhid.exe mode=${1-} log file
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
    [[ -f "$exe" && ! -L "$exe" ]] || die "Put this script beside $exe."
    [[ $# -le 1 && ( -z $mode || $mode == launcher ) ]] || die 'Usage: bash play_skhid.sh [launcher]'
    if [[ $mode == launcher ]]; then
        for file in launcher_skhid.exe updater_skhid.exe; do
            [[ -f "$file" && ! -L "$file" ]] || die "Put $file beside this script to use the launcher."
        done
    fi
    exec 9>.skhid-launcher.lock
    flock -n 9 || die 'This launcher is already running.'

    configure_wine
    prepare_prefix
    prepare_settings
    install_dxvk
    if [[ $mode == launcher ]]; then
        install_gecko
        echo "Starting the Skhid launcher. Log: $log"
        "$WINE" launcher_skhid.exe >>"$log" 2>&1 \
            || die "Launcher exited with an error. See $log"
    else
        start_game
    fi
}

main "$@"
