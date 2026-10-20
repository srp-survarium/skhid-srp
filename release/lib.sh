# shellcheck shell=bash
# What the release scripts share. Sourced, never run. How they are used:
# .claude/skills/skhid-release/SKILL.md; their tests: release/test.sh.
set -euo pipefail
shopt -s inherit_errexit

PROG=${0##*/}
# The checkout these scripts are in.
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
DRY_RUN=${DRY_RUN:-0}

# The two files of a release in <target dir>/download.
ZIP_NAME=Survarium-0.26g-srp-files.zip
DATA_NAME=server-game-data.db
# The stock 0.26g resources.db (build 2777).
# shellcheck disable=SC2034 # used by the scripts that source this
STOCK_DB_SHA256=c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c
# The players' exe in the ZIP.
ZIP_EXE=game/binaries/x86/survarium_skhid.exe
# The image the compose file runs srp26 from.
IMAGE=srp-026g:latest
# From this revision on the exe expects the menus of resources.db.patch_12: a
# release of revision 12 or later that does not offer patch_12 gives players
# stock menus.
MENUS_PATCH=12

die() {
    printf '%s: %s\n' "$PROG" "$*" >&2
    exit 1
}

say() { printf '%s\n' "$*"; }

# `--help`: the comment block at the top of the script, without the `# `.
usage() {
    sed -n '2,/^[^#]/{/^#/{s/^# \{0,1\}//;p}}' "$0"
    exit "${1:-0}"
}

need_cmds() {
    local cmd
    for cmd in "$@"; do
        command -v "$cmd" >/dev/null || die "$cmd is needed and not found"
    done
}

# Loads private/release/release.env of the main checkout (or the file
# SKHID_RELEASE_ENV names), once, and checks the settings given are set. The
# file is the owner's; nothing of it is printed.
RELEASE_ENV=
load_env() {
    if [[ -z $RELEASE_ENV ]]; then
        local env=${SKHID_RELEASE_ENV:-}
        if [[ -z $env ]]; then
            local common
            common=$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir) ||
                die "$ROOT is not in a git checkout"
            env=$(dirname "$common")/private/release/release.env
        fi
        [[ -f $env ]] || die "$env is missing: copy .claude/skills/skhid-release/release.env.example there and fill it in"
        # shellcheck disable=SC1090
        . "$env"
        RELEASE_ENV=$env
    fi
    local name
    for name in "$@"; do
        [[ -n ${!name:-} ]] || die "$name is not set in $RELEASE_ENV"
    done
}

# A word quoted for a remote shell.
q() { printf '%q' "$1"; }

sha256_of() { sha256sum <"$1" | cut -d' ' -f1; }

size_of() { stat -c %s "$1"; }

# `resources.db.patch_<n>`, n from 1 to 255 as the game writes it (no
# leading zero): the number, or nothing.
patch_number() {
    local name=$1
    [[ $name =~ ^resources\.db\.patch_([1-9][0-9]{0,2})$ ]] || return 1
    ((BASH_REMATCH[1] <= 255)) || return 1
    printf '%s\n' "${BASH_REMATCH[1]}"
}

is_revision() { [[ $1 =~ ^[1-9][0-9]{0,2}$ ]] && (($1 <= 255)); }

# GET <url> into <file>: the HTTP status (000 when there was no answer).
http_get() {
    curl -sS --max-time "${HTTP_TIMEOUT:-600}" -o "$2" -w '%{http_code}' "$1" || true
}

# The most URLs a patch may have (the players' updater takes up to 8).
MAX_PATCH_URLS=8

# Whether <url> has the form of a patch's URL: http:// or https://, a host
# name or a bare IPv4 address, an optional port, a path; no credentials, no
# #, no space, quote or backslash; at most 1024 bytes. srp-game-data checks
# each with the updater's own rules when the server data is built
# (client/updater/logic/tests/data/patch_urls.txt); this refuses the plain
# mistakes early.
is_patch_url() {
    # Bytes, whatever the locale: printable ASCII only.
    local LC_ALL=C url=$1 form='^https?://[A-Za-z0-9.-]+(:[0-9]{1,5})?/[!-~]+$'
    ((${#url} <= 1024)) && [[ $url =~ $form && $url != *[\\\"#]* ]]
}

# The players' updater follows a patch's redirects, up to 5, to http:// and
# https:// only; so do the checks here.
CURL_AS_UPDATER=(-L --max-redirs 5 --proto "=http,https" --proto-redir "=http,https")

# What release/release.sh, build-server-db and add-mirror found good: one
# `<sha256> <size> <url>` line each URL that gave that patch whole (200,
# not a web page, its size and sha256), kept across runs so that a big
# patch is not downloaded again at every release.
URL_CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release/patch-urls.checked

# check_patch_url <url> <sha256> <size> <scratch folder> [full]: whether
# <url> gives the players that patch, as their updater fetches it. Its
# first check is a whole download: 200, not a web page (Content-Type
# text/html: Google Drive's virus-scan page, a sign-in page), the size and
# the sha256; then the line in URL_CACHE. A URL in the cache for that
# sha256 is asked for its first 13 bytes only: 200 or 206, not a web page,
# the whole file's size the row's when the answer says it (a 206's
# Content-Range, a 200's Content-Length: a file replaced since by one of
# another size), the game archive's `little-endian`. A file replaced by
# one of the same size and header is seen only by a whole download: the
# players' updater checks the sha256 all the same and moves on to the next
# URL. `full`: the whole download all the
# same (add-mirror). False, with URL_PROBLEM saying why, otherwise; CHECKED
# says how it was checked (`downloaded`, `as checked before`).
URL_PROBLEM='' CHECKED=''
# shellcheck disable=SC2034 # CHECKED: the callers read it
check_patch_url() {
    local url=$1 sha=$2 size=$3 dir=$4 full=${5:-} out head status type total got
    out=$dir/url-check.$$ head=$dir/url-check.$$.head
    URL_PROBLEM='' CHECKED=''
    if [[ -z $full ]] && grep -qxF -- "$sha $size $url" "$URL_CACHE" 2>/dev/null; then
        { curl -sS "${CURL_AS_UPDATER[@]}" --max-time 60 -r 0-12 -D "$head" "$url" 2>/dev/null || true; } |
            head -c 13 >"$out"
        # The last answer's (a redirect's head comes first, with its own).
        status=$(awk 'toupper($1) ~ /^HTTP\// { s = $2 } END { print s }' "$head" 2>/dev/null || true)
        type=$(tr -d '\r' <"$head" 2>/dev/null |
            awk 'toupper($1) ~ /^HTTP\// { t = "" } tolower($1) == "content-type:" { t = tolower($2) } END { print t }' || true)
        # The whole file's size, as the last answer says it: a 206's
        # Content-Range (`bytes 0-12/<size>`), a 200's Content-Length;
        # nothing when it does not say.
        total=$(tr -d '\r' <"$head" 2>/dev/null |
            awk -v status="$status" 'toupper($1) ~ /^HTTP\// { r = ""; l = "" }
                tolower($1) == "content-range:" { n = split($0, part, "/"); r = part[n] }
                tolower($1) == "content-length:" { l = $2 }
                END { print (status == 206 ? r : l) }' || true)
        got=$(cat "$out")
        rm -f "$out" "$head"
        if [[ $status != 200 && $status != 206 ]]; then
            URL_PROBLEM="$url answers ${status:-nothing} now (it gave the file before)"
        elif [[ $type == text/html* ]]; then
            URL_PROBLEM="$url answers with a web page now (it gave the file before)"
        elif [[ $total =~ ^[0-9]+$ && $total != "$size" ]]; then
            URL_PROBLEM="$url says the file is $total bytes now, not $size (it gave the file before)"
        elif [[ $got != little-endian ]]; then
            URL_PROBLEM="$url does not start as a patch now (it gave the file before)"
        else
            CHECKED='as checked before: its first bytes now'
            return 0
        fi
        return 1
    fi
    got=$(curl -sS "${CURL_AS_UPDATER[@]}" --max-time "${HTTP_TIMEOUT:-600}" -o "$out" -w '%{http_code} %{content_type}' "$url" 2>/dev/null || true)
    status=${got%% *} type=${got#* }
    type=${type,,}
    if [[ $status != 200 ]]; then
        URL_PROBLEM="$url answers ${status:-nothing}, not 200"
    elif [[ $type == text/html* ]]; then
        URL_PROBLEM="$url answers with a web page ($type), not the file (Google Drive: use https://drive.usercontent.google.com/download?id=<id>&export=download&confirm=t)"
    elif [[ $(size_of "$out") != "$size" ]]; then
        URL_PROBLEM="$url gave $(size_of "$out") bytes, not $size"
    elif [[ $(sha256_of "$out") != "$sha" ]]; then
        URL_PROBLEM="$url gave sha256 $(sha256_of "$out"), not $sha"
    fi
    rm -f "$out"
    [[ -z $URL_PROBLEM ]] || return 1
    CHECKED='downloaded whole'
    if ! grep -qxF -- "$sha $size $url" "$URL_CACHE" 2>/dev/null; then
        { mkdir -p "${URL_CACHE%/*}" && printf '%s %s %s\n' "$sha" "$size" "$url" >>"$URL_CACHE"; } 2>/dev/null || true
    fi
}

# Our srp-game-data: $SRP_GAME_DATA_BIN, else built from this checkout (at
# nice 10).
game_data_bin() {
    if [[ -n ${SRP_GAME_DATA_BIN:-} ]]; then
        printf '%s\n' "$SRP_GAME_DATA_BIN"
        return
    fi
    say "building srp-game-data (srp-026g, nix develop) ..." >&2
    (cd "$ROOT" && nice -n 10 nix develop ./srp-026g -c bash -c \
        'cd srp-026g && cargo build --quiet --release -p resources-db --bin srp-game-data') >&2 ||
        die "srp-game-data did not build"
    printf '%s\n' "$ROOT/srp-026g/target/release/srp-game-data"
}

# The building steps (build-patch, build-server-db, build-zip) run at nice
# 10, with every child: the release PC goes on doing other things.
lower_priority() { renice -n 10 -p $$ >/dev/null 2>&1 || true; }

# python3, or nixpkgs' when the system has none.
python() {
    if command -v python3 >/dev/null; then
        python3 "$@"
    else
        nix shell nixpkgs#python3 -c python3 "$@"
    fi
}

# `<sha256> <name>` of every file entry of a ZIP, in name order. (Python's
# zipfile, not unzip: an unzip may be sandboxed to its working folder.)
zip_hashes() {
    python -I -c '
import hashlib, sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    for info in sorted(z.infolist(), key=lambda i: i.filename):
        if not info.is_dir():
            print(hashlib.sha256(z.read(info)).hexdigest(), info.filename)
' "$1"
}

# One entry of a ZIP into a file; fails when the ZIP has none.
zip_extract() {
    python -I -c '
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z, open(sys.argv[3], "wb") as out:
    out.write(z.read(sys.argv[2]))
' "$1" "$2" "$3" 2>/dev/null
}

# The patch revision of a players' survarium_skhid.exe: the byte of
# `mov byte [esp+27h],<revision>` that patch_026g.py writes at REVISION_VA
# 0x380862 (.text: VA 0x11000 at file 0x400), or `unknown`.
exe_revision() {
    local at=$((0x380862 - 0x11000 + 0x400)) bytes=''
    if [[ -f $1 ]]; then bytes=$(od -An -tx1 -v -j "$at" -N 6 "$1" | tr -d ' \n'); fi
    if [[ $bytes =~ ^c6442427([0-9a-f]{2})90$ ]]; then
        printf '%d\n' "0x${BASH_REMATCH[1]}"
    else
        printf 'unknown\n'
    fi
}

# The revision of the exe in a ZIP (<scratch> holds it), or `unknown`.
zip_revision() {
    if zip_extract "$1" "$ZIP_EXE" "$2"; then exe_revision "$2"; else printf 'unknown\n'; fi
}

# GET <site>/download (the players' ZIP) into <file>; dies unless 200.
fetch_site_zip() {
    local url=${1%/}/download status
    status=$(curl -sSL --max-time 600 -o "$2" -w '%{http_code}' "$url" || true)
    [[ $status == 200 ]] || die "GET $url answered $status, not 200"
}

# `REVISION = <n>` of patch_026g.py at <commit> of the checkout <repo>.
revision_at() {
    git -C "$1" show "$2:srp-026g/client-patches/patch_026g.py" 2>/dev/null |
        sed -n 's/^REVISION = \([0-9][0-9]*\)$/\1/p'
}

# Asks before a write: shows where and the command; anything but y or yes is
# no. Reads the terminal (else stdin, as `yes | ...` would give).
# SKHID_RELEASE_GO_AHEAD=1 (release/release.sh sets it for the scripts it
# runs, after its own y/N) shows the command and goes ahead.
confirm() {
    local where=$1 command=$2
    if [[ ${SKHID_RELEASE_GO_AHEAD:-} == 1 ]]; then
        printf '\n%s, running (go-ahead given to release.sh):\n  %s\n' "$where" "$command" >&2
        return 0
    fi
    printf '\n%s, about to run:\n  %s\n' "$where" "$command" >&2
    ask_yes ""
}

# y/N from the terminal, else stdin; true only for y or yes. [seconds]: no
# answer in that time is a no.
ask_yes() {
    local answer='' timeout=()
    if [[ -n ${1:-} ]]; then timeout=(-t "$1"); fi
    if { exec 3</dev/tty; } 2>/dev/null; then
        read -r "${timeout[@]}" -p "Go ahead? [y/N] " answer <&3 || answer=''
        exec 3<&-
    else
        printf 'Go ahead? [y/N] ' >&2
        read -r "${timeout[@]}" answer || answer=''
        printf '%s\n' "$answer" >&2
    fi
    [[ $answer == y || $answer == yes ]]
}

# The patches server a release publishes to and offers from, set by
# use_patches:
#   live  $SKHID_PATCHES_SSH, $SKHID_PATCHES_DIR, $SKHID_PATCHES_URL; the
#         list release/patches.tsv ($SKHID_PATCH_LIST names another);
#   test  $SKHID_TEST_PATCHES_SSH, $SKHID_TEST_PATCHES_DIR,
#         $SKHID_TEST_PATCHES_URL, all three set and none of it the
#         production folder or URL; the list release/patches-test.tsv
#         ($SKHID_TEST_PATCH_LIST names another).
PATCHES_SSH='' PATCHES_DIR='' PATCHES_URL='' PATCH_LIST='' PATCHES_LABEL=''
use_patches() {
    case $1 in
    live)
        load_env SKHID_PATCHES_SSH SKHID_PATCHES_DIR SKHID_PATCHES_URL
        PATCHES_SSH=$SKHID_PATCHES_SSH PATCHES_DIR=${SKHID_PATCHES_DIR%/} PATCHES_URL=${SKHID_PATCHES_URL%/}
        PATCH_LIST=${SKHID_PATCH_LIST:-$ROOT/release/patches.tsv}
        PATCHES_LABEL="the patches server ($PATCHES_SSH)"
        ;;
    test)
        load_env SKHID_TEST_PATCHES_SSH SKHID_TEST_PATCHES_DIR SKHID_TEST_PATCHES_URL
        PATCHES_SSH=$SKHID_TEST_PATCHES_SSH PATCHES_DIR=${SKHID_TEST_PATCHES_DIR%/} PATCHES_URL=${SKHID_TEST_PATCHES_URL%/}
        if [[ $PATCHES_URL == "${SKHID_PATCHES_URL%/}" ]]; then
            die "SKHID_TEST_PATCHES_URL is the production patches URL: the test environment's patches go to a folder and URL of their own"
        fi
        if [[ $PATCHES_SSH == "${SKHID_PATCHES_SSH:-}" && $PATCHES_DIR == "${SKHID_PATCHES_DIR%/}" ]]; then
            die "SKHID_TEST_PATCHES_DIR is the production patches folder: the test environment's patches go to a folder and URL of their own"
        fi
        PATCH_LIST=${SKHID_TEST_PATCH_LIST:-$ROOT/release/patches-test.tsv}
        PATCHES_LABEL="the TEST patches server ($PATCHES_SSH, $PATCHES_DIR)"
        ;;
    *) die "use_patches: $1" ;;
    esac
}

# One command on the patches server (use_patches first); its stdin is $2
# (an upload) or nothing.
# shellcheck disable=SC2029 # expanded here on purpose: the command is built quoted
on_patches() { ssh "$PATCHES_SSH" "$1" <"${2:-/dev/null}"; }

# The offered-patch list (use_patches first), a committed file: one patch a
# line, `<name> <sha256> <size> <urls>` with tabs between, in revision order;
# `#` starts a comment. <urls> is the patch's URLs, one space apart, in the
# order the players' updater tries them: the patches server's first, then
# mirrors (release/add-mirror), 1 to 8, none twice. The server data is built
# of exactly these and /patches offers exactly these; a file on the patches
# server that is not in it is offered to no one. Checks every row; sets LIST
# (the rows, spaces between) and LIST_TOP (the highest revision in it, 0 for
# none).
LIST='' LIST_TOP=0
read_patch_list() {
    local line name sha size url extra n=0 number rows=() urls=() seen u
    [[ -f $PATCH_LIST ]] || die "$PATCH_LIST: no such file (the offered-patch list)"
    LIST_TOP=0
    while IFS= read -r line || [[ -n $line ]]; do
        n=$((n + 1))
        line=${line%$'\r'}
        [[ -n $line && $line != '#'* ]] || continue
        IFS=$'\t' read -r name sha size url extra <<<"$line"
        [[ -n $url && -z $extra ]] || die "$PATCH_LIST:$n: not <name> <sha256> <size> <urls> with tabs between"
        number=$(patch_number "$name") || die "$PATCH_LIST:$n: $name is not resources.db.patch_<n>"
        ((number > LIST_TOP)) || die "$PATCH_LIST:$n: $name is not above the row before it (revision order, each patch once)"
        [[ $sha =~ ^[0-9a-f]{64}$ ]] || die "$PATCH_LIST:$n: $sha is not a sha256"
        [[ $size =~ ^[1-9][0-9]*$ ]] || die "$PATCH_LIST:$n: $size is not a size in bytes"
        [[ $url != *'  '* && $url != ' '* && $url != *' ' ]] || die "$PATCH_LIST:$n: the URLs are one space apart"
        read -r -a urls <<<"$url"
        ((${#urls[@]} <= MAX_PATCH_URLS)) || die "$PATCH_LIST:$n: ${#urls[@]} URLs; the players' updater takes at most $MAX_PATCH_URLS"
        [[ ${urls[0]} == "$PATCHES_URL/$name" ]] || die "$PATCH_LIST:$n: ${urls[0]} is not $PATCHES_URL/$name (the list of another patches server?)"
        seen=' '
        for u in "${urls[@]}"; do
            is_patch_url "$u" || die "$PATCH_LIST:$n: $u is not a URL the players' updater takes"
            [[ $seen != *" $u "* ]] || die "$PATCH_LIST:$n: $u is there twice"
            seen+="$u "
        done
        LIST_TOP=$number
        rows+=("$name $sha $size $url")
    done <"$PATCH_LIST"
    LIST=''
    if ((${#rows[@]})); then LIST=$(printf '%s\n' "${rows[@]}"); fi
}

# The row of <name> in LIST, or nothing.
list_row() { grep -m1 -- "^$1 " <<<"$LIST" || true; }

# What /patches offers for LIST [+ one more row]: `patch_<n> <sha256>
# <urls>` lines (the URLs one space apart), as check_release_files reads
# them from server data.
offered_of_list() {
    local name sha _ url
    while read -r name sha _ url; do
        if [[ -n $name ]]; then printf '%s %s %s\n' "${name#resources.db.}" "$sha" "$url"; fi
    done <<<"$LIST${1:+$'\n'$1}"
}

# The site's pages made from the game (the Maps tab, the calculator), for
# release.sh and build-site alike: the patches a page's built-from.tsv names
# (built_from <file>), and the ones it must name (site_wants [<patch file>]:
# LIST's and that file, not listed yet), each as `<name> <sha256>` lines in
# one order (sorted), so the two compare equal whatever order each is in.
built_from() { grep -v '^#' "$1" | tr '\t' ' ' | grep . | LC_ALL=C sort -V || true; }
site_wants() {
    { cut -d' ' -f1,2 <<<"$LIST"; if [[ -n ${1:-} ]]; then printf '%s %s\n' "${1##*/}" "$(sha256_of "$1")"; fi; } |
        grep . | LC_ALL=C sort -V || true
}

# Where a release goes, set by use_target:
#   live  $SKHID_LIVE through $SKHID_JUMP (which holds its key), in
#         $SKHID_LIVE_DIR, its site $SKHID_SITE;
#   test  the test environment: $SKHID_TEST (through $SKHID_TEST_VIA when that
#         is set), in $SKHID_TEST_DIR, its site $SKHID_TEST_SITE, its compose
#         command $SKHID_TEST_COMPOSE (default: project srptest).
TARGET='' TARGET_SSH='' TARGET_VIA='' TARGET_DIR='' TARGET_SITE='' TARGET_COMPOSE='' TARGET_LABEL='' D=''
use_target() {
    case $1 in
    live)
        load_env SKHID_JUMP SKHID_LIVE SKHID_LIVE_DIR SKHID_SITE
        TARGET=live TARGET_SSH=$SKHID_LIVE TARGET_VIA=$SKHID_JUMP TARGET_DIR=$SKHID_LIVE_DIR
        TARGET_SITE=${SKHID_SITE%/} TARGET_COMPOSE='docker compose'
        TARGET_LABEL="On the live host ($SKHID_LIVE through $SKHID_JUMP)"
        ;;
    test)
        load_env SKHID_JUMP SKHID_TEST SKHID_TEST_DIR SKHID_TEST_SITE
        TARGET=test TARGET_SSH=$SKHID_TEST TARGET_VIA=${SKHID_TEST_VIA:-} TARGET_DIR=$SKHID_TEST_DIR
        TARGET_SITE=${SKHID_TEST_SITE%/}
        TARGET_COMPOSE=${SKHID_TEST_COMPOSE:-docker compose -p srptest}
        TARGET_LABEL="On the test environment ($SKHID_TEST${TARGET_VIA:+ through $TARGET_VIA})"
        ;;
    *) die "use_target: $1" ;;
    esac
    D=$(q "$TARGET_DIR")
}

# The command the first ssh runs: through the jump host, one more ssh.
target_command() {
    if [[ -n $TARGET_VIA ]]; then printf 'ssh %s %s' "$(q "$TARGET_SSH")" "$(q "$1")"; else printf '%s' "$1"; fi
}
# One command on the target; its stdin is $2 (an upload) or nothing.
# shellcheck disable=SC2029
on_target() { ssh "${TARGET_VIA:-$TARGET_SSH}" "$(target_command "$1")" <"${2:-/dev/null}"; }
on_live() { on_target "$@"; }

# A write: printed only with DRY_RUN=1; else asked for, then run; a no
# stops the script. <where> is `live` (the target) or `patches`; [file] is
# sent as the command's stdin (an upload).
write_on() {
    local where=$1 command=$2 file=${3:-} label shown
    case $where in
    live) label=$TARGET_LABEL ;;
    patches) label="On $PATCHES_LABEL" ;;
    *) die "write_on: $where" ;;
    esac
    shown=$command${file:+ < $file}
    if ((DRY_RUN)); then
        printf '[dry run] %s:\n  %s\n' "$label" "$shown"
        return 0
    fi
    confirm "$label" "$shown" || die "stopped: not confirmed (nothing more was done)"
    case $where in
    live) on_target "$command" "$file" || die "failed on the $TARGET host: $command" ;;
    patches) on_patches "$command" "$file" || die "failed on $PATCHES_LABEL: $command" ;;
    esac
}

# A read on the target: printed with DRY_RUN=1 (then it prints nothing
# else), else run.
read_live() {
    if ((DRY_RUN)); then
        printf '[dry run] read on the %s host:\n  %s\n' "$TARGET" "$1" >&2
        return 0
    fi
    on_target "$1"
}

# One release at a time to a target, whoever runs it: a lock folder on the
# jump host ($SKHID_RELEASE_LOCK, default skhid-release-<target>.lock in its
# home), made with mkdir (atomic), holding who took it. lock_owner prints who
# holds it (nothing: free); lock_take dies when it is held; lock_drop frees
# the one this run took.
LOCK_HELD=0
lock_path() { printf '%s' "${SKHID_RELEASE_LOCK:-skhid-release-$TARGET.lock}"; }
# shellcheck disable=SC2029
lock_owner() {
    local L
    L=$(q "$(lock_path)")
    ssh "$SKHID_JUMP" "if test -e $L; then cat $L/owner 2>/dev/null || echo 'someone (no owner file)'; fi" </dev/null
}
# shellcheck disable=SC2029
lock_take() {
    local L owner
    L=$(q "$(lock_path)")
    owner="$(id -un)@${HOSTNAME:-$(uname -n)}, pid $$, since $(date -u '+%Y-%m-%d %H:%M UTC'): $PROG to the $TARGET server"
    if ssh "$SKHID_JUMP" "mkdir $L 2>/dev/null && printf '%s\n' $(q "$owner") > $L/owner" </dev/null; then
        LOCK_HELD=1
        return 0
    fi
    die "another release to the $TARGET server holds the lock on the jump host: $(lock_owner || echo unknown). If that run is gone (its terminal closed, its PC off), free it: ssh $SKHID_JUMP rm -r $(lock_path)"
}
# shellcheck disable=SC2029
lock_drop() {
    if ((LOCK_HELD)); then
        if ssh "$SKHID_JUMP" "rm -rf $(q "$(lock_path)")" </dev/null; then
            LOCK_HELD=0
        else
            say "the release lock was NOT freed: ssh $SKHID_JUMP rm -r $(lock_path)"
        fi
    fi
}

# Checks a release's two files here, before anything goes anywhere: the
# ZIP's exe says patch revision <n>, the server data reads (srp-game-data
# info), offers no patch above <n>, and offers patch_12 (the menus) from
# revision 12 on. Sets ZIP_SHA ZIP_SIZE DATA_SHA, OFFERED (`patch_<n>
# <sha256> <urls>` lines) and EXPECTED (/patches as JSON). <scratch> is a
# folder for the exe.
check_release_files() {
    local zip=$1 data=$2 revision=$3 scratch=$4 got bin info key
    got=$(zip_revision "$zip" "$scratch/exe")
    [[ $got == "$revision" ]] || die "$zip: its exe says patch revision $got, not $revision"
    bin=$(game_data_bin)
    info=$("$bin" info "$data") || die "$data: srp-game-data info refused it"
    OFFERED=$(offered_in "$info")
    while read -r key _; do
        [[ -n $key ]] || continue
        ((${key#patch_} <= revision)) || die "$data offers $key, above revision $revision"
    done <<<"$OFFERED"
    if ((revision >= MENUS_PATCH)) && ! grep -q "^patch_$MENUS_PATCH " <<<"$OFFERED"; then
        die "$data does not offer patch_$MENUS_PATCH (the menus): a game of revision $revision would show the stock menus"
    fi
    # shellcheck disable=SC2034 # the callers use them
    ZIP_SHA=$(sha256_of "$zip") ZIP_SIZE=$(size_of "$zip") DATA_SHA=$(sha256_of "$data")
    EXPECTED=$(patches_json "$OFFERED")
}

# The patches `srp-game-data info` (or build, or check) says data offers:
# `patch_<n> <sha256> <urls>` lines.
offered_in() {
    sed -n 's/^  resources\.db\.\(patch_[0-9]*\) [0-9]* bytes sha256 \([0-9a-f]*\) urls \(.*\)$/\1 \2 \3/p' <<<"$1"
}

# /patches as JSON (keys sorted; each patch's URLs in their order) of
# `patch_<n> <sha256> <urls>` lines, as srp writes it.
patches_json() {
    jq -cnS --arg list "$1" '$list | split("\n") | map(select(length > 0) | split(" ")
        | {key: .[0], value: {sha256: .[1], urls: .[2:]}}) | from_entries'
}

# What /patches will offer, one line each (or `nothing ({})`).
say_offered() {
    local line
    while read -r line; do
        if [[ -n $line ]]; then say "  $line"; fi
    done <<<"$OFFERED"
    if [[ -z $OFFERED ]]; then say "  nothing ({})"; fi
}

# /patches of the target now, as JSON with sorted keys, or nothing.
served_patches() {
    local file status
    file=$(mktemp)
    status=$(HTTP_TIMEOUT=20 http_get "$TARGET_SITE/patches" "$file")
    if [[ $status == 200 ]]; then jq -cS . "$file" 2>/dev/null || true; fi
    rm -f "$file"
}

# Every release keeps a folder of its own on the target:
# $TARGET_DIR/release-rev<n>-<UTC YYYYMMDD-HHMM>[-<label>] (REL; REL_DIR, the
# path quoted for the remote shell). In it:
#   state            staging, swapping, released, stopped (before the swap),
#                    rolled-back or rollback-refused
#   release.log      what the run printed (no secret is ever printed)
#   Survarium-0.26g-srp-files.zip, server-game-data.db
#                    the new files, staged here (sha256 checked, mode 644);
#                    the swap hard-links each into download/ and renames it
#                    over the old one (same file system: atomic)
#   new.sha old.sha  sha256sum of the new files; of download/'s before
#   old-Survarium-0.26g-srp-files.zip, old-server-game-data.db
#                    download/'s files before the swap (hard links)
#   srp-<unix>.db    srp.db as it was before the release: the copy the new
#                    srp made at its start (srp-db.name names it)
#   image.old image.new  the image ID that ran before the swap, and after
#   min.old          SRP_MIN_PATCH_REVISION before (`none`: no line)
#   changed.txt      the server source files that changed, the ZIP's changed
#                    entries (release.sh)
#   src.tar src/ old-src.tgz  with server code: main's source (src/, where
#                    the image is built), the tree's files before
#   rollback.sh rollback.env  release/host-rollback.sh and what it puts back
REL='' REL_DIR='' BEFORE_TAG='' STAGED=0
release_folder() { # <revision> <stamp> [label]
    REL=release-rev$1-$2${3:+-$3}
    REL_DIR=$D/$REL
    BEFORE_TAG=srp-026g:before-rev$1-$2${3:+-$3}
}

# The folder made (never one that is there), state staging.
stage_begin() {
    write_on live "cd $D && mkdir $REL && echo staging > $REL/state"
    ((DRY_RUN)) || STAGED=1
}

# Uploads <file> as <name> into the release folder: <name>.part, its sha256
# checked there, chmod 644, renamed; its line added to new.sha.
stage_upload() {
    local file=$1 name=$2 sha=$3 there
    write_on live "cat > $REL_DIR/$name.part" "$file"
    if ((!DRY_RUN)); then
        there=$(on_target "sha256sum < $REL_DIR/$name.part")
        [[ ${there%% *} == "$sha" ]] || die "$REL/$name.part has sha256 ${there%% *} on the host, not $sha"
        say "$REL/$name: sha256 $sha, as here"
    else
        say "[dry run] then: sha256sum < $REL_DIR/$name.part must be $sha"
    fi
    write_on live "cd $REL_DIR && chmod 644 $name.part && mv $name.part $name && sha256sum $name >> new.sha"
}

# A stop before the swap: the staged files go (the folder stays, with its
# log), state stopped.
stage_abandon() {
    ((STAGED)) || return 0
    if on_target "cd $REL_DIR && rm -rf $ZIP_NAME $DATA_NAME ./*.part src src.tar && echo stopped > state && rm -f ../download/.$ZIP_NAME.new ../download/.$DATA_NAME.new"; then
        say "staged files: removed ($TARGET_DIR/$REL stays, state stopped)"
    else
        say "staged files: NOT removed: $TARGET_DIR/$REL"
    fi
}

# The run's output also into <file> (log_to), until log_close, which waits
# for the copy to be complete: what goes in the folder as release.log.
LOG_PID=''
log_to() {
    exec 8>&1 9>&2
    exec > >(tee "$1") 2>&1
    LOG_PID=$!
}
log_close() {
    if [[ -n $LOG_PID ]]; then
        exec 1>&8 2>&9
        wait "$LOG_PID" 2>/dev/null || true
        LOG_PID=''
    fi
}

# The run's output into the folder as release.log, once it is there.
save_release_log() {
    log_close
    if ((STAGED)) && [[ -s $1 ]]; then
        on_target "cat > $REL_DIR/release.log" "$1" || say "release.log: NOT saved in $TARGET_DIR/$REL"
    fi
}

# The running srp26 on the target: RUN_CONTAINER, RUN_IMAGE (the ID of its
# image: what a rollback tags srp-026g:latest again), RUN_VOLUME (its volume
# at /srv/srp/data, which holds srp.db and srp's backups/), RUN_VOLUME_DIR
# (that volume's folder on the host, where a rollback puts srp.db back with
# srp26 stopped) and LATEST_IMAGE (the ID srp-026g:latest names now, what a
# restart runs). Fails when srp26 has no container.
inspect_target() {
    local out
    out=$(on_target "cd $D && id=\$($TARGET_COMPOSE ps -aq srp26 | head -n 1) && test -n \"\$id\" && echo \"container \$id\" && docker inspect -f 'image {{.Image}}' \"\$id\" && vol=\$(docker inspect -f '{{range .Mounts}}{{if eq .Destination \"/srv/srp/data\"}}{{.Name}}{{end}}{{end}}' \"\$id\") && echo \"volume \$vol\" && docker volume inspect -f 'mount {{.Mountpoint}}' \"\$vol\" && docker image inspect -f 'latest {{.Id}}' $IMAGE") ||
        return 1
    RUN_CONTAINER=$(sed -n 's/^container //p' <<<"$out")
    RUN_IMAGE=$(sed -n 's/^image //p' <<<"$out")
    RUN_VOLUME=$(sed -n 's/^volume //p' <<<"$out")
    RUN_VOLUME_DIR=$(sed -n 's/^mount //p' <<<"$out")
    LATEST_IMAGE=$(sed -n 's/^latest //p' <<<"$out")
    [[ -n $RUN_CONTAINER && $RUN_IMAGE =~ ^sha256:[0-9a-f]+$ && -n $RUN_VOLUME && $RUN_VOLUME_DIR == /* && -n $LATEST_IMAGE ]]
}

# Settings srp reads that the compose file did not pass on before #105
# (deploy/env-026g.example documents them). Once a compose file passes one,
# the recreate takes its value from the target's .env, where it may have sat
# unused. env_settings_set <compose file text> prints the names of those the
# compose file passes and the .env sets (never a value).
NEW_SETTINGS=(SRP_ITEM_WEAR SRP_DROP_CONDITION SRP_DROP_DEFECT SRP_DROP_PLAYTIME_BONUS SRP_DAILY_BONUS
    SRP_DAILY_BONUS_RESET_HOUR SRP_MATCH_SEARCH_STEP_SECONDS SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS
    SRP_LOBBY_SESSION_GRACE_SECONDS)
env_settings_set() {
    local name passed=()
    for name in "${NEW_SETTINGS[@]}"; do
        if grep -qE "^[[:space:]]*- ${name}([=[:space:]]|\$)" <<<"$1"; then passed+=("$name"); fi
    done
    ((${#passed[@]})) || return 0
    on_target "cd $D && { grep -oE '^($(IFS='|' && printf '%s' "${passed[*]}"))=' .env || true; } | tr -d = | LC_ALL=C sort -u | tr '\n' ' '" | sed 's/ $//'
}

# The original menus' blobs that download/resources/ on the target must keep:
# the updater's one-time restore to the stock resources.db (from the region
# releases before #105) fetches them from /resources/<sha256>. Prints the
# missing ones, `<name> <sha256 prefix>` a line.
ORIGINAL_BLOBS=(main_menu:0ff118a8 lobby_menu:ab6f22d8 login_menu:805ea5f8)
missing_original_blobs() {
    local there blob
    there=$(on_target "cd $D && ls download/resources 2>/dev/null; true")
    for blob in "${ORIGINAL_BLOBS[@]}"; do
        grep -q "^${blob#*:}[0-9a-f]*$" <<<"$there" || say "${blob%%:*} ${blob#*:}…"
    done
}

# The value of SRP_MIN_PATCH_REVISION in the target's .env, `none` when the
# line is missing; dies when it is there more than once.
current_min_revision() {
    local lines
    lines=$(on_target "cd $D && grep -E '^SRP_MIN_PATCH_REVISION=' .env || true")
    [[ $(grep -c . <<<"$lines" || true) -le 1 ]] || die ".env has SRP_MIN_PATCH_REVISION more than once; fix it by hand first"
    if [[ -z $lines ]]; then say none; else say "${lines#*=}"; fi
}

# The command that sets SRP_MIN_PATCH_REVISION=<n> in .env: that line only
# (added on a line of its own when missing); prints the line.
min_revision_command() {
    printf '%s' "cd $D && if grep -q '^SRP_MIN_PATCH_REVISION=' .env; then sed -i 's/^SRP_MIN_PATCH_REVISION=.*/SRP_MIN_PATCH_REVISION=$1/' .env; else { test -z \"\$(tail -c 1 .env)\" || echo; echo SRP_MIN_PATCH_REVISION=$1; } >> .env; fi && grep -E '^SRP_MIN_PATCH_REVISION=' .env"
}

# The restart: srp26 made anew on what is in place now (--force-recreate:
# even when neither its image nor its settings changed, it reads the new
# server data).
restart_command() {
    printf '%s' "cd $D && $TARGET_COMPOSE up -d --no-deps --force-recreate srp26"
}

# The swap, shared by release/deploy and release/release.sh. Before it:
# use_target, inspect_target (RUN_*), check_release_files (ZIP_SHA ...,
# EXPECTED), release_folder and the files staged in it, and these:
#   OLD_MIN            SRP_MIN_PATCH_REVISION before (`none`: no line)
#   HAVE_ZIP HAVE_DATA 1 when download/ has that file now
#   OLD_PATCHES        /patches now (JSON, or empty): checked after a rollback
#   SWAP_SRC           1: src.tar of the folder (main's source) goes into the
#                      tree too, SWAP_SRC_INPUTS (quoted paths) kept first
#   SWAP_BEFORE_STOP   a function run right before srp26 is stopped (it
#                      dies to stop the release there), or empty
# In order:
#   swap_prepare <revision>  srp26 stopped; srp.db, the old files, the image
#                            ID, the setting kept in the folder; the image
#                            tagged; the rollback written
#   swap_server <revision>   the server data and the setting (and source) in
#                            place, srp26 made anew on them, checked:
#                            running, its game data line, /patches
#   swap_zip                 then the ZIP in place, /api/client-files
# SWAP_STATE says how far it got, for swap_failed: `` (nothing), stopped
# (only srp26 stopped and copies made: it is started again), moved (the
# rollback runs), finished.
SWAP_STATE='' SWAP_SRC=0 SWAP_SRC_INPUTS='' SWAP_BEFORE_STOP=''
OLD_MIN=none HAVE_ZIP=0 HAVE_DATA=0 OLD_PATCHES=''
RUN_CONTAINER='' RUN_IMAGE='' RUN_VOLUME='' RUN_VOLUME_DIR='' LATEST_IMAGE=''

# The command that keeps the old files in the folder: their sha256sum,
# hard links (same file system as download/), the image ID, the setting (and
# the source the release replaces).
archive_command() {
    local c
    c="cd $D && echo $(q "$RUN_IMAGE") > $REL/image.old && echo $OLD_MIN > $REL/min.old && : > $REL/old.sha"
    ((HAVE_ZIP)) && c+=" && sha256sum download/$ZIP_NAME >> $REL/old.sha && ln download/$ZIP_NAME $REL/old-$ZIP_NAME"
    ((HAVE_DATA)) && c+=" && sha256sum download/$DATA_NAME >> $REL/old.sha && ln download/$DATA_NAME $REL/old-$DATA_NAME"
    if ((SWAP_SRC)); then
        c+=" && tar -czf $REL/old-src.tgz \$(for p in $SWAP_SRC_INPUTS; do if test -e \$p; then echo \$p; fi; done)"
    fi
    printf '%s' "$c && ls -l $REL"
}

# What the folder's rollback.sh puts back (sourced by it; no secret).
rollback_env() {
    printf '%s=%q\n' TARGET_DIR "$TARGET_DIR" COMPOSE "$TARGET_COMPOSE" IMAGE "$IMAGE" \
        OLD_IMAGE "$RUN_IMAGE" VOLUME_DIR "$RUN_VOLUME_DIR" BACKUP_BEFORE "$BACKUP_BEFORE" OLD_MIN "$OLD_MIN" NEW_MIN "$1" \
        HAVE_ZIP "$HAVE_ZIP" HAVE_DATA "$HAVE_DATA" SRC "$SWAP_SRC" ZIP_NAME "$ZIP_NAME" DATA_NAME "$DATA_NAME"
}

# The command that puts <name> of the folder into download/: a hard link
# renamed over the old file (atomic).
put_command() {
    printf '%s' "cd $D && if test ! $REL/$1 -ef download/$1; then ln -f $REL/$1 download/.$1.new && mv -f download/.$1.new download/$1; fi"
}

# srp.db: at every start srp copies it with VACUUM INTO to
# /srv/srp/data/backups/srp-<unix>.db before it opens it (and before any
# schema change), so the start of the new srp makes the copy of the state
# just before the release. BACKUP_BEFORE is the newest copy before the swap
# (noted while the old srp runs); keep_db_backup takes the one the new srp
# made, with docker exec ... cat, into the folder (srp-db.name names it) and,
# from the live host, to the jump host too: $SKHID_BACKUP_DIR (default
# /srv/skhid-backups/srp-db; folder 700, files 600, its line added to
# SHA256SUMS there), fetched by the jump host's own ssh to the live host,
# the sha256 compared on both ends. No other container and no sqlite3 is run.
BACKUP_BEFORE=''
# The newest srp-<unix>.db in backups/ (nothing: none), srp26 running.
newest_backup() {
    on_target "cd $D && id=\$($TARGET_COMPOSE ps -q srp26 | head -n 1) && test -n \"\$id\" && docker exec \"\$id\" sh -c 'ls -1 /srv/srp/data/backups 2>/dev/null; true'" |
        { grep -E '^srp-[0-9]+\.db$' || true; } | sort -t- -k2 -n | tail -n 1
}
backup_number() { local n=${1#srp-}; n=${n%.db}; printf '%s' "${n:-0}"; }

keep_db_backup() {
    local new here there dir=${SKHID_BACKUP_DIR:-/srv/skhid-backups/srp-db} cat
    new=$(newest_backup) || new=''
    if [[ -z $new ]] || (($(backup_number "$new") <= $(backup_number "$BACKUP_BEFORE"))); then
        say "srp made no backup of srp.db at its start (backups/: newest ${new:-none}, before the swap ${BACKUP_BEFORE:-none})"
        return 1
    fi
    cat="cd $(q "$TARGET_DIR") && id=\$($TARGET_COMPOSE ps -q srp26 | head -n 1) && docker exec \"\$id\" cat /srv/srp/data/backups/$new"
    here=$(on_target "$cat > $REL/$new.part && chmod 600 $REL/$new.part && mv $REL/$new.part $REL/$new && echo $new > $REL/srp-db.name && sha256sum < $REL/$new") || {
        say "srp.db: the backup $new could not be copied into $REL"
        return 1
    }
    say "srp.db as it was before the release: backups/$new, kept in $TARGET_DIR/$REL/$new (sha256 ${here%% *})"
    [[ $TARGET == live ]] || return 0
    # shellcheck disable=SC2029 # built quoted
    if there=$(ssh "$SKHID_JUMP" "umask 077 && mkdir -p $(q "$dir") && chmod 700 $(q "$dir") && cd $(q "$dir") && test ! -e $new && ssh $(q "$TARGET_SSH") $(q "$cat") > $new.part && chmod 600 $new.part && mv $new.part $new && sha256sum $new >> SHA256SUMS && sha256sum < $new" </dev/null) &&
        [[ ${there%% *} == "${here%% *}" ]]; then
        say "srp.db off the live host: $SKHID_JUMP:$dir/$new, sha256 ${there%% *} on both ends (in SHA256SUMS there)"
    else
        say "WARNING: srp.db was NOT copied off the live host (sha256 there: ${there:-none}, here: ${here%% *}); copy it by hand:"
        say "  ssh $SKHID_JUMP 'cd $dir && ssh $TARGET_SSH \"cd $TARGET_DIR && cat $REL/$new\" > $new && chmod 600 $new && sha256sum $new >> SHA256SUMS'"
        DB_COPY_FAILED=1
    fi
}
# shellcheck disable=SC2034 # release.sh reads it
DB_COPY_FAILED=0

swap_prepare() {
    local file
    if ((DRY_RUN)); then
        say "[dry run] note the newest of srp's backups (docker exec <srp26> ls /srv/srp/data/backups)"
    else
        BACKUP_BEFORE=$(newest_backup) || die "srp's backups/ cannot be listed (docker exec)"
        say "srp's newest backup of srp.db before the swap: ${BACKUP_BEFORE:-none}"
    fi
    if [[ -n $SWAP_BEFORE_STOP ]]; then "$SWAP_BEFORE_STOP"; fi
    write_on live "cd $D && $TARGET_COMPOSE stop srp26"
    ((DRY_RUN)) || SWAP_STATE=stopped
    write_on live "$(archive_command)"
    write_on live "docker tag $(q "$RUN_IMAGE") $BEFORE_TAG"
    file=$(mktemp)
    rollback_env "$1" >"$file"
    write_on live "cat > $REL_DIR/rollback.env" "$file"
    write_on live "cat > $REL_DIR/rollback.sh && echo swapping > $REL_DIR/state" "$(dirname "${BASH_SOURCE[0]}")/host-rollback.sh"
    rm -f "$file"
}

# The container after a restart: `ok <state>` once it is running (healthy,
# when it has a health check) and has not restarted, `failed <state>` as soon
# as it exited or restarted, `failed ...` after ~90 s of neither.
wait_container() {
    local state='' status health restarts _
    for _ in $(seq "${CHECK_TRIES:-45}"); do
        state=$(on_target "cd $D && id=\$($TARGET_COMPOSE ps -aq srp26 | head -n 1) && test -n \"\$id\" && docker inspect -f '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}} {{.RestartCount}} {{.State.StartedAt}}' \"\$id\"" || true)
        read -r status health restarts _ <<<"$state"
        if [[ $status == running && ($health == healthy || $health == none) && $restarts == 0 ]]; then
            say "ok $state"
            return 0
        fi
        if [[ $status == exited || $status == dead || $status == restarting || ${restarts:-0} != 0 ]]; then
            say "failed $state"
            return 0
        fi
        sleep 2
    done
    say "failed ${state:-no container}"
}

# srp's last lines, and the lines where it says why it does not start:
# the game data lacks what its code or srp.db names, or SRP_MIN_PATCH_REVISION
# is below the data's highest patch.
show_srp_log() {
    local log why
    log=$(on_target "cd $D && $TARGET_COMPOSE logs --no-log-prefix --tail=${1:-40} srp26 2>&1" || true)
    say "srp26's last log lines:"
    # shellcheck disable=SC2001 # every line
    sed 's/^/  | /' <<<"$log"
    why=$(grep -E "srp: (the game's files|SRP_MIN_PATCH_REVISION is|cannot load the 0\.26g accounts)" <<<"$log" | sort -u || true)
    if [[ -n $why ]]; then
        say "srp refused to start:"
        # shellcheck disable=SC2001
        sed 's/^/  > /' <<<"$why"
    fi
}

# After a restart: the container up, its log's game data line of the new
# server data, $TARGET_SITE/patches exactly EXPECTED (retried while srp
# starts). Prints what failed and returns 1.
check_server() {
    local state line count patches='' status=000 _
    state=$(wait_container)
    if [[ $state != ok* ]]; then
        say "srp26 is not up: ${state#failed }"
        return 1
    fi
    say "srp26: ${state#ok }"
    line=$(on_target "cd $D && $TARGET_COMPOSE logs --no-log-prefix --tail=500 srp26 2>&1 | grep 'game data:' | tail -n 1" || true)
    count=$(grep -c . <<<"$OFFERED" || true)
    if [[ $line != *"; $count patch(es) offered at /patches"* ]]; then
        say "srp26's log has no game data line of the new server data ($count patch(es)): ${line:-none}"
        return 1
    fi
    say "log: $line"
    for _ in $(seq "${CHECK_TRIES:-30}"); do
        status=$(HTTP_TIMEOUT=20 http_get "$TARGET_SITE/patches" "$SWAP_WORK/patches")
        if [[ $status == 200 ]]; then
            patches=$(jq -cS . "$SWAP_WORK/patches" 2>/dev/null || true)
            [[ -n $patches ]] && break
        fi
        sleep 2
    done
    if [[ -z $patches ]]; then
        say "GET $TARGET_SITE/patches gave no list (status $status)"
        return 1
    fi
    if [[ $patches != "$EXPECTED" ]]; then
        say "/patches is $patches, not $EXPECTED"
        return 1
    fi
    say "/patches: $patches, as the server data says"
}

# /api/client-files names the new ZIP (sha256, size); a few tries (it may be
# reading the file). Prints what failed and returns 1.
check_client_files() {
    local status _
    for _ in 1 2 3 4 5; do
        status=$(HTTP_TIMEOUT=60 http_get "$TARGET_SITE/api/client-files" "$SWAP_WORK/client-files")
        if [[ $status == 200 ]] && grep -qx "sha256 $ZIP_SHA" "$SWAP_WORK/client-files" &&
            grep -qx "size $ZIP_SIZE" "$SWAP_WORK/client-files"; then
            say "/api/client-files: the new ZIP (sha256 $ZIP_SHA, $ZIP_SIZE bytes)"
            return 0
        fi
        sleep 2
    done
    say "/api/client-files is not the new ZIP (status $status): $(tr '\n' ' ' <"$SWAP_WORK/client-files" 2>/dev/null)"
    return 1
}

# <revision>: the server's files in place, srp26 made anew, check_server. A
# failure shows srp's log and runs the rollback (swap_failed).
swap_server() {
    ((DRY_RUN)) || SWAP_STATE=moved
    write_on live "$(put_command "$DATA_NAME")"
    SWAP_DONE+=("in place: download/$DATA_NAME")
    write_on live "$(min_revision_command "$1")"
    SWAP_DONE+=("SRP_MIN_PATCH_REVISION=$1 in .env (was $OLD_MIN)")
    if ((SWAP_SRC)); then
        write_on live "cd $D && tar -xf $REL/src.tar && rm -rf $REL/src"
        SWAP_DONE+=("main's source unpacked into $TARGET_DIR")
    fi
    write_on live "$(restart_command)"
    SWAP_DONE+=("srp26 made anew")
    write_on live "cd $D && id=\$($TARGET_COMPOSE ps -aq srp26 | head -n 1) && docker inspect -f '{{.Image}}' \"\$id\" > $REL/image.new"
    ((DRY_RUN)) && return 0
    check_server || die "the new server did not come up as it must"
    keep_db_backup || die "the pre-release copy of srp.db is not kept: rolling back rather than going on without it"
}

swap_zip() {
    write_on live "$(put_command "$ZIP_NAME")"
    SWAP_DONE+=("in place: download/$ZIP_NAME")
    ((DRY_RUN)) && return 0
    check_client_files || die "the site does not hand out the new ZIP"
    on_target "echo released > $REL_DIR/state"
    SWAP_STATE=finished
}

# For an exit trap, when the swap did not finish: srp26 started again as it
# was (only stopped), or the folder's rollback run and checked (things were
# moved).
SWAP_DONE=() SWAP_WORK=${TMPDIR:-/tmp}
swap_failed() {
    local state now
    case $SWAP_STATE in
    stopped)
        say
        say "STOPPED in the swap, before anything was moved: starting srp26 again as it was"
        if on_target "cd $D && $TARGET_COMPOSE start srp26"; then
            say "srp26: started again, on the files from before"
        else
            say "srp26 did NOT start: on the $TARGET host, cd $TARGET_DIR && $TARGET_COMPOSE start srp26"
        fi
        stage_abandon
        ;;
    moved)
        say
        say "THE SWAP FAILED. Done:"
        if ((${#SWAP_DONE[@]})); then printf '  %s\n' "${SWAP_DONE[@]}"; fi
        show_srp_log 40
        say
        say "ROLLING BACK, on the $TARGET host: sh $TARGET_DIR/$REL/rollback.sh $TARGET_DIR/$REL (srp.db too: as it was before the release)"
        if on_target "sh $REL_DIR/rollback.sh $REL_DIR" 2>&1 | sed 's/^/  /'; then
            state=$(wait_container)
            now=$(served_patches)
            if [[ $state == ok* && (-z $OLD_PATCHES || $now == "$OLD_PATCHES") ]]; then
                say "rolled back: srp26 ${state#ok }, /patches ${now:-(none)} as before. Nothing of this release is in place."
            else
                say "the rollback ran, but srp26 is ${state#* } and /patches is ${now:-(none)} (before: ${OLD_PATCHES:-none})"
                show_srp_log 20
                say "Look at it by hand on the $TARGET host ($TARGET_DIR/$REL, docker compose logs srp26)"
            fi
        else
            say "THE ROLLBACK DID NOT BRING srp26 UP (above). On the $TARGET host: read its advice above; do not run it in a loop"
        fi
        ;;
    esac
}
