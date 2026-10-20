#!/usr/bin/env bash
# usage: release/release.sh [--resources <resources.db.patch_<rev>>] [--test]
#                           [--revision <n>] [--server-code auto|yes|no]
#                           [--minutes <n> | --no-maintenance] [--now]
#                           [--match-wait <minutes>] [--label <word>]
#                           [--ref <branch>] [--dry-run] [--yes --i-know]
#
# A whole release of origin/main in one run, and its record. Its revision
# is the live exe's + 1 (--test: the test exe's + 1; or --revision <n>),
# built so without editing anything (build-zip --revision); after it went
# out live, the release sets REVISION and tags itself (step 7). A feature PR
# never changes REVISION or the revision table. After the preflight it waits
# for the go; at the go players get a maintenance countdown at once, to end
# 30 seconds after the last running match does (or one of --minutes <n>),
# so Play locks and no match starts after it; during the countdown
# everything is prepared and staged, and at the later of its end and the
# staging's, once no match runs, it is all swapped in at once and srp is
# made anew (the countdown hides the build). With --no-maintenance there is
# no countdown: staged, then swapped as soon as no match runs (the downtime
# is the recreate; lobby players reconnect). With --now there is no
# countdown either, and no wait: staged, then swapped at once, matches
# running or not (they end without results):
#   1. preflight, nothing players see: ssh to the jump host and through it to
#      the live host, the patches server, nix, $SKHID_GAME, $SKHID_STOCK_DB
#      (its sha256), free disk here and there, srp-game-data, the
#      release's revision <n> (the target exe's + 1, or --revision <n> above
#      it), not below main's REVISION; live: main's REVISION (the last
#      release's) not below the live exe's and below <n>, the tag
#      026g-rev<n> and the branch release/record-rev<n> not on origin, a git
#      user here (user.name/email) and gh signed in; the admin page answering the jump
#      host (not with --no-maintenance or --now), no maintenance planned already, no
#      other release running (the lock on the jump host), no release folder
#      left unfinished, download/ on the same file system as the folder;
#      srp26's container, its image ID the one srp-026g:latest names; which
#      of the settings srp takes since #105 the .env sets (names only); the
#      offered-patch list
#      (release/patches.tsv): every row on the patches server with its size
#      and sha256, every URL of it giving that file as the players' updater
#      fetches it (redirects followed, not a web page, the size and sha256:
#      downloaded whole once, then checked by its first bytes; the cache is
#      ~/.cache/skhid-release/patch-urls.checked), none above the release's
#      revision, and patch_12 (the menus) in it from revision 12 on (or the
#      --resources file); with
#      --resources, the file is resources.db.patch_<the release's revision>,
#      srp-game-data reads it, and it is neither published nor listed yet;
#      live: the site's pages made from the game data (the Maps tab, the
#      calculator): built from other patches than the list (and the
#      --resources file) on main, they are rebuilt from those, last in the
#      preflight and only when nothing else failed (release/build-site,
#      about 2 min, in a worktree of the commit; a failure stops the run),
#      and the server's source carries them (the server code is then built;
#      --server-code no is refused, the dry run too);
#      which server code differs between main and the target's tree;
#      whether main changes what a match computes: a `next` row of
#      srp-026g/crates/session/src/v026g/simulation_changes.tsv, which a PR
#      adds (it cannot know its revision): the server takes `next` for its
#      SRP_MIN_PATCH_REVISION, this release's, so recordings of older games
#      are refused from the swap on (the player is told why), and the record
#      writes the number in; with one, the server code must be built (and
#      also when the target's tree still has one: its server would take
#      this release's revision for it); with none, a note when the exe's
#      patches changed since the last tag;
#   2. takes the release lock, then waits for the go: the owner's word that
#      players may feel the release from now on (y at its prompt, n stops;
#      or touch the go file it names under ~/.cache/skhid-release/; --yes
#      does not give it; SKHID_RELEASE_GO=1 gives it beforehand). Nothing is
#      planned, built or staged before it;
#   3. at once after the go, plans the maintenance on the admin page, POSTed
#      from the jump host (its address is in the SRP_ADMIN_IPS of the
#      target; the token and the page's path go to curl on stdin), and sees
#      the countdown on $SKHID_SITE/launcher/places/en. Its minutes:
#      ceil((last_match_ends_in + 30) / 60), at least 1 (the admin page
#      takes whole minutes, 1-10080), where last_match_ends_in is what
#      $SKHID_SITE/api/stats says (the seconds until the last running match
#      ends at the latest; 0: none runs). From an srp that does not send it
#      yet: 1 with "matches":0; else the most "Time left" (<= <m> min) on the
#      launcher page's matches + 1 (30 s, rounded up), when the page gives
#      one for every running match; else a whole match and 30 s, 16. With
#      --minutes <n>: <n>. Play locks from the plan on (in the countdown's
#      last 30 min), so no match starts after it. With --no-maintenance or
#      --now: nothing is planned;
#   4. during the countdown: release/upload-patch (with --resources:
#      publishes it and adds its row to the list), release/build-server-db
#      (of the list), release/build-zip (--ref: the commit checked in step
#      1; --revision: the release's); checks the two files as release/deploy does; makes the release
#      folder $TARGET_DIR/release-rev<rev>-<UTC YYYYMMDD-HHMM>[-<label>] and
#      stages both in it (sha256 checked there, chmod 644), with
#      changed.txt; with server code to build: tags the running image (by
#      its ID) srp-026g:before-rev<rev>-<stamp>, unpacks main's source in the
#      folder's src/ (with the rebuilt pages) and runs docker compose build srp26 there;
#   5. at the later of the planned time and the end of step 4 (staged after
#      the planned time: at once), once the launcher page says the server is
#      down and $SKHID_SITE/api/stats says "matches":0 (waiting up to
#      --match-wait more): shows what changes, y/N (no answer in
#      $SKHID_SWAP_ANSWER_MINUTES, default 10: no), then the swap of
#      release/deploy. With --no-maintenance: once "matches":0 (waiting up
#      to --match-wait from the end of step 4), y/N, and "matches":0 once
#      more right before the stop (else it waits again, within the same
#      limit). With --now: right after step 4, no notice, no wait for
#      "matches":0, y/N; right before the stop it says how many matches run,
#      which the stop ends without results (the log keeps the number).
#      The swap: srp26 stopped; srp.db, the old ZIP and server data (hard
#      links), their sha256, the image ID, the setting (and the source) kept
#      in the folder with its rollback, srp.db copied to the jump host too
#      (live only); the server data (and source) in place,
#      SRP_MIN_PATCH_REVISION=<rev>, srp26 made anew;
#   6. checks the container, the log's game data line and /patches (exactly
#      the list's patches), and only then puts the ZIP in place and checks
#      /api/client-files. A failed check prints srp's log and runs the
#      folder's rollback (srp.db, image, data, ZIP, setting, source: as
#      before), with --now too. The run's output is the folder's release.log;
#   7. live only, once it all went out (never after a stop or a rollback):
#      the record. An annotated tag 026g-rev<n> on the commit that was built
#      (the UTC time, the ZIP's and the server data's sha256, the exe's),
#      pushed; then the commit "Release revision <n>" on that commit:
#      REVISION = <n> in patch_026g.py, its row in the revision table of
#      srp-026g/client-patches/README.md (the time, the exe's sha256 with
#      and without the server, the ZIP's, what it shipped: the merged PRs
#      since the tag before), an entry in CHANGELOG-SKHID-026G.md for the
#      owner to word, release/patches.tsv as offered, the pages step 1
#      rebuilt, and <n> in place of
#      a `next` row of simulation_changes.tsv (step 1); pushed as
#      release/record-rev<n>, its PR opened and merged as a merge commit
#      (gh). What gh could not do (the merge, when the token may not) is said
#      at the end, loudly, and the run ends with 1: players have the
#      release, main does not say so yet.
# Stopped before the swap (a failure, a no, Ctrl-C) at any point after the
# go: the maintenance cancelled when it was planned, the staged files
# removed (the folder stays, state stopped), the image tag put back; the
# server is as it was (a published patch stays, with its row in the list:
# none is ever deleted). A stop before the go only frees the lock.
#
#   --resources <file>  new content: resources.db.patch_<rev>, published first
#   --revision <n>      the release's revision instead of the target exe's + 1;
#                       live: above it (one released once is never used again);
#                       --test: any from REVISION up
#   --test              to the test environment ($SKHID_TEST in $SKHID_TEST_DIR,
#                       compose project srptest, $SKHID_TEST_SITE), with the
#                       test patches server ($SKHID_TEST_PATCHES_*, never the
#                       production one) and its list release/patches-test.tsv;
#                       its ZIP (build-zip --test); the same steps as live
#   --server-code auto  (default) build when main's srp-026g differs from the
#                       target's tree; yes: build anyway; no: never
#   --minutes <n>       a countdown of <n> minutes, 1-10080, instead of the
#                       one from the matches; planned right after the go too
#   --no-maintenance    no countdown, the admin page never asked: staged
#                       after the go, swapped as soon as $SKHID_SITE/api/stats
#                       says "matches":0
#   --now               no maintenance, no notice, no wait: after the go,
#                       staged, then swapped at once, even with matches
#                       running (they end without results; the log says how
#                       many); still the go, the y/N and the rollback.
#                       --no-maintenance with it is the same; not with
#                       --minutes
#   --match-wait <min>  how long to wait for "matches":0, then stop (nothing
#                       swapped): after the planned time (default
#                       $SKHID_MATCH_WAIT_MINUTES, else 15); with
#                       --no-maintenance, after step 4 (default 120);
#                       ignored with --now
#   --label <word>      ends the release folder's name
#   --ref <branch>      with --test only: release that pushed branch instead
#                       of main (a branch under review); live is always main
#   --dry-run           runs the read-only checks of step 1, prints every
#                       other command; plans no maintenance, writes nothing
#   --yes --i-know      answer the y/N before the swap yes (both flags or
#                       neither); never the go
#
# Settings: release.env (.claude/skills/skhid-release/release.env.example).
. "$(dirname "$0")/lib.sh"
here=$(cd "$(dirname "$0")" && pwd)

# For release/test.sh only: another checkout, stand-ins for build-zip and
# build-site, a shorter minute, another stock db.
REPO=${SKHID_TEST_REPO:-$ROOT}
BUILD_ZIP=${SKHID_TEST_BUILD_ZIP:-$here/build-zip}
BUILD_SITE=${SKHID_TEST_BUILD_SITE:-$here/build-site}
MINUTE=${SKHID_TEST_SECONDS_PER_MINUTE:-60}
STOCK_SHA=${SKHID_TEST_STOCK_SHA256:-$STOCK_DB_SHA256}

ref=main resources='' revision_arg='' test_server=0 server_code=auto minutes='' label='' yes=0 i_know=0
no_maint=0 match_wait='' now=0
while (($#)); do
    case $1 in
    -h | --help) usage ;;
    --resources) (($# >= 2)) || usage 1; resources=$2; shift 2 ;;
    --revision) (($# >= 2)) || usage 1; revision_arg=$2; shift 2 ;;
    --test) test_server=1; shift ;;
    --server-code) (($# >= 2)) || usage 1; server_code=$2; shift 2 ;;
    --minutes) (($# >= 2)) || usage 1; minutes=$2; shift 2 ;;
    --no-maintenance) no_maint=1; shift ;;
    --now) now=1; shift ;;
    --match-wait) (($# >= 2)) || usage 1; match_wait=$2; shift 2 ;;
    --label) (($# >= 2)) || usage 1; label=$2; shift 2 ;;
    --ref) (($# >= 2)) || usage 1; ref=$2; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --yes) yes=1; shift ;;
    --i-know) i_know=1; shift ;;
    *) die "unknown argument $1 (--help)" ;;
    esac
done
((yes == i_know)) || die "--yes goes with --i-know (both or neither): without a terminal to ask, this restarts the live server"
[[ $server_code =~ ^(auto|yes|no)$ ]] || die "--server-code is auto, yes or no"
# Absolute, for the steps that run elsewhere (build-site in a worktree).
if [[ -n $resources && -e $resources ]]; then resources=$(realpath -s -- "$resources"); fi
[[ -z $revision_arg ]] || is_revision "$revision_arg" || die "--revision $revision_arg: a revision is a whole number from 1 to 255"
[[ -z $minutes || $no_maint == 0 ]] || die "--minutes is the countdown, and --no-maintenance has none"
[[ -z $minutes || $now == 0 ]] || die "--minutes is the countdown, and --now has none"
# --now: no maintenance at all, the swap as soon as all is staged.
if ((now && no_maint)); then say "note: --now plans no maintenance anyway: --no-maintenance with it says nothing more"; fi
((!now)) || no_maint=1
[[ $ref == main || $test_server == 1 ]] || die "--ref goes with --test only: a live release is of main"
[[ $ref =~ ^[A-Za-z0-9._/-]+$ && $ref != -* ]] || die "$ref: not a branch name"
[[ -z $label || $label =~ ^[A-Za-z0-9._-]+$ ]] || die "$label: a label is letters, digits, . _ -"
need_cmds ssh curl jq sha256sum od git tar awk df
load_env SKHID_JUMP SKHID_SITE SKHID_STOCK_DB SKHID_GAME SKHID_LOCALIZATION
step_flags=()
if ((test_server)); then
    use_target test
    use_patches test
    load_env SKHID_TEST_GAME_SERVER
    step_flags=(--test)
else
    use_target live
    use_patches live
    load_env SKHID_GAME_SERVER
fi
read_patch_list
if [[ -n $minutes ]]; then
    if [[ ! $minutes =~ ^[1-9][0-9]{0,4}$ ]] || ((minutes > 10080)); then die "--minutes: a whole number from 1 to 10080"; fi
fi
if [[ -n ${SKHID_MAINT_MINUTES:-} ]]; then
    say "note: SKHID_MAINT_MINUTES is not read any more: the countdown ends 30 s after the last match (or --minutes <n>)"
fi
# SKHID_MATCH_WAIT_MINUTES is the wait after a countdown, when most matches
# have ended; without one a match may have just begun (they last up to 15).
if [[ -n $match_wait ]]; then
    [[ $match_wait =~ ^[0-9]{1,5}$ ]] || die "--match-wait: a whole number of minutes"
    ((!now)) || say "note: --match-wait is ignored with --now (it waits for no match to end)"
elif ((no_maint)); then
    match_wait=120
else
    match_wait=${SKHID_MATCH_WAIT_MINUTES:-15}
    [[ $match_wait =~ ^[0-9]{1,5}$ ]] || die "SKHID_MATCH_WAIT_MINUTES: a whole number"
fi
answer_wait=${SKHID_SWAP_ANSWER_MINUTES:-10}
[[ $answer_wait =~ ^[1-9][0-9]*$ ]] || die "SKHID_SWAP_ANSWER_MINUTES: a whole number from 1"
# What the image is built of at <commit> (the sources of its Dockerfile's
# COPY lines, but --from=), the Dockerfile and the compose file: one path of
# srp-026g a line, those it has.
build_inputs() {
    local path
    {
        printf '%s\n' Dockerfile .dockerignore docker-compose.yml
        git -C "$REPO" show "$1:srp-026g/Dockerfile" | awk 'toupper($1) == "COPY" {
            from = 0; n = 0
            for (i = 2; i <= NF; i++) if ($i ~ /^--from/) from = 1; else if ($i !~ /^--/) word[++n] = $i
            if (!from) for (i = 1; i < n; i++) print word[i]
        }'
    } | sed 's|^\./||; s|/$||' | while read -r path; do
        if [[ $path == . || $path == *[*?]* ]]; then printf '%s\n' '!unsupported'; continue; fi
        if git -C "$REPO" cat-file -e "$1:srp-026g/$path" 2>/dev/null; then printf '%s\n' "$path"; fi
    done | LC_ALL=C sort -u
}

work=$(mktemp -d "${TMPDIR:-/tmp}/skhid-release.XXXXXX")
SWAP_WORK=$work
log_to "$work/release.log"
say "working in $work"

# Where the run is: preflight (up to the go), planned (the go given, going
# ahead, nothing moved: a stop puts things back), swapping (a stop rolls
# back), finished. maint_planned: the maintenance was asked for (a stop
# cancels it). ENDED: with --now, the matches the stop ended.
phase=preflight start=0 end=0 image_tagged=0 maint_planned=0 ENDED=''

clock() { date -u -d "@$1" +%H:%M:%S; }
span() { local s=$1; printf '%d:%02d' $((s / 60)) $((s % 60)); }
# The clock: to (or past) the planned time; with --no-maintenance the time
# since the go.
progress() {
    local s
    if ((end == 0 && start != 0)); then
        say "== [$(clock "$(date +%s)") UTC, $(span $(($(date +%s) - start))) since the go] $*"
        return
    fi
    if ((end == 0)); then
        say "== $*"
        return
    fi
    s=$((end - $(date +%s)))
    if ((s >= 0)); then
        say "== [$(clock "$(date +%s)") UTC, $(span "$s") to the planned time] $*"
    else
        say "== [$(clock "$(date +%s)") UTC, $(span $((-s))) past the planned time] $*"
    fi
}
# A sleep a stop ends at once: bash runs a trap only after a foreground
# command, but `wait` returns to it as the signal comes (the sleep is killed
# on the way out).
NAP_PID=''
nap() {
    sleep "$1" >/dev/null 2>&1 &
    NAP_PID=$!
    wait "$NAP_PID"
    NAP_PID=''
}
pause() { nap "$(awk -v m="$MINUTE" -v f="$1" 'BEGIN { print m * f }')"; }

# The admin page's path and token, from the target's .env, into ADMIN_PATH
# and ADMIN_TOKEN; never printed. The command shown in `ps` is a grep.
read_admin() {
    local lines line name value
    lines=$(on_target "cd $D && grep -E '^SRP_ADMIN_(PATH|TOKEN)=' .env" || true)
    ADMIN_PATH='' ADMIN_TOKEN=''
    while IFS= read -r line; do
        [[ -n $line ]] || continue
        name=${line%%=*} value=${line#*=}
        value=${value%$'\r'}
        value=${value#"${value%%[![:space:]]*}"}
        value=${value%"${value##*[![:space:]]}"}
        if ((${#value} >= 2)) && [[ $value == \"*\" || $value == \'*\' ]]; then value=${value:1:-1}; fi
        case $name in
        SRP_ADMIN_PATH) [[ -z $ADMIN_PATH ]] || die "SRP_ADMIN_PATH is in the .env more than once"; ADMIN_PATH=$value ;;
        SRP_ADMIN_TOKEN) [[ -z $ADMIN_TOKEN ]] || die "SRP_ADMIN_TOKEN is in the .env more than once"; ADMIN_TOKEN=$value ;;
        esac
    done <<<"$lines"
    ADMIN_PATH=${ADMIN_PATH#/} ADMIN_PATH=${ADMIN_PATH%/}
    [[ -n $ADMIN_PATH && -n $ADMIN_TOKEN ]] || return 1
    [[ $ADMIN_PATH =~ ^[A-Za-z0-9._~/-]+$ ]] || die "SRP_ADMIN_PATH in the .env has characters this script does not send (not shown)"
    [[ $ADMIN_TOKEN != *[\"\\]* ]] || die "SRP_ADMIN_TOKEN in the .env has a quote or a backslash (not shown)"
}

# The admin page, asked from the jump host (the address in SRP_ADMIN_IPS):
# curl there reads its URL (the secret path) and the form (the token, then
# <name=value>...) from a config on stdin, so neither is in a command line.
# No fields: a GET. Prints the answer's body, then its HTTP status on a line
# of its own. The body holds the token: never print it.
admin_request() {
    local field
    {
        printf 'url = "%s/%s"\n' "$TARGET_SITE" "$ADMIN_PATH"
        if (($#)); then
            printf 'data-urlencode = "token=%s"\n' "$ADMIN_TOKEN"
            for field; do printf 'data = "%s"\n' "$field"; done
        fi
    } | ssh "$SKHID_JUMP" "curl -sS --max-time 30 -w '\n%{http_code}' -K -"
}
# The admin page's answer to a form, without the body: what it said.
admin_said() {
    local answer=$1 status body
    status=${answer##*$'\n'} body=${answer%$'\n'*}
    case $body in
    *"Maintenance starts in"*) say planned ;;
    *"Maintenance cancelled."*) say cancelled ;;
    *"No maintenance was planned."*) say none ;;
    *"Wrong token."*) say "wrong token" ;;
    *"Too many wrong tokens"*) say "locked: too many wrong tokens" ;;
    *) say "HTTP $status, no answer of the maintenance tab" ;;
    esac
}
plan_command() {
    say "POST $TARGET_SITE/<SRP_ADMIN_PATH> token=<SRP_ADMIN_TOKEN> tab=maint $1 (ssh $SKHID_JUMP curl -K - < the form)"
}

cancel_maintenance() {
    local answer said
    answer=$(admin_request tab=maint cancel=1 lang=en) || true
    said=$(admin_said "$answer")
    case $said in
    cancelled) say "maintenance: cancelled" ;;
    none) say "maintenance: none was planned any more" ;;
    *) say "maintenance: NOT cancelled ($said): cancel it on the admin page (tab Maintenance)" ;;
    esac
}

# The public launcher page: `planned <minutes>`, `down`, `none` or `unknown`.
maintenance_state() {
    local page
    page=$(curl -sS --max-time 20 "$TARGET_SITE/launcher/places/en" 2>/dev/null || true)
    if [[ $page =~ Maintenance\ in\ ([0-9]+)\ min ]]; then
        say "planned ${BASH_REMATCH[1]}"
    elif [[ $page == *"Maintenance: the server is down"* ]]; then
        say down
    elif [[ $page == *"On the server now"* ]]; then
        say none
    else
        say unknown
    fi
}

matches_now() {
    curl -sS --max-time 20 "$TARGET_SITE/api/stats" 2>/dev/null | jq -r '.matches | numbers' 2>/dev/null || true
}

# The maintenance's minutes into MAINT_MINUTES, and where they come from
# into MAINT_WHY: --minutes <n>; else 30 s after the last running
# match ends at the latest, as /api/stats says (last_match_ends_in,
# seconds), rounded up to whole minutes (the admin page takes those), at
# least 1. From an srp that does not send it: 1 with "matches":0; else the
# launcher page's most "Time left" + 1 (its minutes are rounded up, so
# + 30 s is a minute more), when the page gives a time left for every
# running match; else a whole match and 30 s, 16.
LAST_MATCH_MARGIN=30 WHOLE_MATCH_MINUTES=15
maintenance_minutes() {
    local stats ends matches page cell count=0 most=0 older why
    local re='^<td class=e>&le; ([0-9]+) min</td>$'
    if [[ -n $minutes ]]; then
        MAINT_MINUTES=$minutes MAINT_WHY="--minutes $minutes"
        return 0
    fi
    stats=$(curl -sS --max-time 20 "$TARGET_SITE/api/stats" 2>/dev/null || true)
    ends=$(jq -r '.last_match_ends_in | numbers | ceil' <<<"$stats" 2>/dev/null || true)
    if [[ $ends =~ ^[0-9]{1,9}$ ]]; then
        MAINT_MINUTES=$(((ends + LAST_MATCH_MARGIN + 59) / 60))
        ((MAINT_MINUTES >= 1)) || MAINT_MINUTES=1
        if ((ends)); then
            MAINT_WHY="/api/stats: the last match ends in $(span "$ends") at the latest (last_match_ends_in $ends), then 30 s, rounded up to whole minutes"
        else
            MAINT_WHY="/api/stats: no match runs (last_match_ends_in 0), then 30 s, rounded up to a minute"
        fi
        return 0
    fi
    matches=$(jq -r '.matches | numbers' <<<"$stats" 2>/dev/null || true)
    older="/api/stats has no last_match_ends_in (an srp from before it)"
    if [[ $matches == 0 ]]; then
        MAINT_MINUTES=1 MAINT_WHY="$older; \"matches\":0: no match runs, then 30 s, rounded up to a minute"
        return 0
    fi
    page=$(curl -sS --max-time 20 "$TARGET_SITE/launcher/places/en" 2>/dev/null || true)
    why=''
    while read -r cell; do
        count=$((count + 1))
        if [[ $cell =~ $re ]]; then
            ((BASH_REMATCH[1] <= most)) || most=${BASH_REMATCH[1]}
        else
            why="a match with no time left yet (still loading)"
        fi
    done < <(grep -o '<td class=e>[^<]*</td>' <<<"$page" || true)
    if [[ -z $why && ! $matches =~ ^[1-9][0-9]*$ ]]; then
        why="no match count"
    elif [[ -z $why ]] && ((count != matches)); then
        why="$count time(s) left for $matches match(es)"
    fi
    if [[ -z $why ]]; then
        MAINT_MINUTES=$((most + 1))
        MAINT_WHY="$older; the launcher page's \"Time left\": $count match(es), the last ending in <= $most min, then 30 s, rounded up to a minute"
    else
        MAINT_MINUTES=$((WHOLE_MATCH_MINUTES + 1))
        MAINT_WHY="$older, and the launcher page's \"Time left\" does not say ($why): a whole match, $WHOLE_MATCH_MINUTES min, then 30 s, rounded up to a minute"
    fi
}

# Right after the preflight and the lock: nothing players see happens
# without the go, the owner's word that players may feel the release from
# now on; the maintenance is planned at once after it. y at this prompt (n
# stops), or the go file (touch it from anywhere: another shell, the
# release skill). Without a terminal (--yes) only the file.
# SKHID_RELEASE_GO=1 gives it beforehand (release/test.sh). Ctrl-C stops:
# nothing was done, the lock is freed.
GO_DIR=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release
wait_for_go() {
    local go="$GO_DIR/go-$REL" answer tty=0
    if [[ ${SKHID_RELEASE_GO:-} == 1 ]]; then say "go: given beforehand (SKHID_RELEASE_GO=1)"; return; fi
    mkdir -p "$GO_DIR" && rm -f "$go"
    progress "2. waiting for the go (players see nothing until it is given; then at once: $1)"
    say "Go: answer y here, or run: touch $go"
    if ((!yes)) && { exec 3</dev/tty; } 2>/dev/null; then tty=1; printf "Go? [y/n] " >&2; fi
    while :; do
        if [[ -e $go ]]; then rm -f "$go"; say "go: $go"; break; fi
        if ((tty)); then
            if read -r -t 2 answer <&3; then
                case $answer in
                y | yes) say "go: y"; break ;;
                n | no) die "stopped: no go (nothing was done)" ;;
                *) printf "Go? [y/n] " >&2 ;;
                esac
            fi
        else
            sleep 2 & wait $!
        fi
    done
    if ((tty)); then exec 3<&-; fi
}

# At once after the go, before anything is built or staged (that happens
# during the countdown): the maintenance planned on the admin page, seen on
# the launcher page; the planned time into end.
plan_maintenance() {
    local answer said state seen='' _
    state=$(maintenance_state)
    case $state in planned* | down) die "the launcher page says \"$state\" before this run planned anything: a maintenance planned meanwhile (the admin page)? Nothing was swapped" ;; esac
    maintenance_minutes
    progress "3. planning the maintenance in $MAINT_MINUTES min ($MAINT_WHY)"
    end=$(($(date +%s) + MAINT_MINUTES * MINUTE))
    maint_planned=1
    answer=$(admin_request tab=maint "minutes=$MAINT_MINUTES" lang=en) || answer=000
    said=$(admin_said "$answer")
    [[ $said == planned ]] || die "the admin page did not plan the maintenance: $said"
    for _ in 1 2 3 4 5; do
        state=$(maintenance_state)
        if [[ $state == planned* ]]; then seen=${state#planned }; break; fi
        nap 2
    done
    [[ -n $seen ]] || die "$TARGET_SITE/launcher/places/en does not show the maintenance ($state)"
    progress "3. maintenance planned: the launcher says \"Maintenance in $seen min\"$( ((MAINT_MINUTES > 30)) || printf '; Play is locked'); building and staging now; the swap at $(clock "$end") UTC at the earliest"
}

# Back once $TARGET_SITE/api/stats says "matches":0; at $limit (a time) a
# stop, "... match(es) still running <when>: nothing was swapped".
wait_no_match() {
    local matches
    while :; do
        matches=$(matches_now)
        [[ $matches == 0 ]] && return 0
        (($(date +%s) < limit)) || die "${matches:-an unknown number of} match(es) still running $1: nothing was swapped"
        progress "5. ${matches:-? (no answer from /api/stats)} match(es) still running; waiting (until $(clock "$limit") UTC at most)"
        pause 0.25
    done
}
# --no-maintenance: swap_prepare runs it right before it stops srp26, so the
# stop follows a "matches":0 as closely as it can (a match may have begun
# during the y/N).
match_check_before_stop() {
    if ((DRY_RUN)); then
        say "[dry run] GET $TARGET_SITE/api/stats: \"matches\":0 once more right before the stop (else wait for it, within the same limit)"
        return 0
    fi
    wait_no_match "$match_wait min after the wait began (--match-wait), at the check before the stop"
    say "\"matches\":0 once more: stopping srp26 now"
}

# --now: right before the stop, the matches it ends (without results), said
# and kept in ENDED.
count_ended_matches() {
    if ((DRY_RUN)); then
        say "[dry run] GET $TARGET_SITE/api/stats: the matches running now, which the stop ends without results (--now)"
        return 0
    fi
    ENDED=$(matches_now)
    if [[ $ENDED == 0 ]]; then
        say "--now: no match is running: the stop ends none"
    else
        say "--now: ${ENDED:-an unknown number of (no answer from /api/stats)} match(es) running: the stop ends them now, WITHOUT RESULTS"
    fi
}

# 7. The record of a live release, once it went out: the tag on the commit
# that was built, and the commit "Release revision <n>" (REVISION, the
# revision table's row, a changelog entry to word, the offered-patch list)
# on a branch, its PR merged as a merge commit. Nothing of it before the
# release went out: a stop or a rollback leaves none. What could not be done
# is said in RECORD_PROBLEMS (the run then ends with 1).
release_tag() { printf '026g-rev%s' "$1"; }
record_branch() { printf 'release/record-rev%s' "$1"; }
# The changes of what a match computes, by revision (a PR adds `next`).
SIMULATION_CHANGES=srp-026g/crates/session/src/v026g/simulation_changes.tsv
# The exe's patch files of srp-026g/client-patches changed from <a> to <b>,
# one a line: not the notes, tests and checks, nor patch_026g.py when only
# its REVISION line changed (a record's).
exe_patch_changes() { # <a> <b>
    local path
    git -C "$REPO" diff --name-only "$1" "$2" -- srp-026g/client-patches 2>/dev/null | while read -r path; do
        case ${path##*/} in
        *.md | *.txt | test_* | check_* | selftest_*) continue ;;
        patch_026g.py)
            git -C "$REPO" diff -U0 "$1" "$2" -- "$path" | grep -E '^[-+][^-+]' | grep -qvE '^[-+]REVISION = [0-9]+$' || continue ;;
        esac
        printf '%s\n' "${path#srp-026g/client-patches/}"
    done || true
}
RECORD_PROBLEMS=() RECORDING=0
record_problem() {
    RECORD_PROBLEMS+=("$*")
    say "  NOT DONE  $*"
}

# The merged PRs of main from the last release's tag (before the first tag:
# from the last change of REVISION on main) to <commit>, oldest first: one
# `#<n> <title>` a line, the record PRs left out. SHIPPED_SINCE: that start.
SHIPPED_SINCE=''
shipped_prs() {
    local commit=$1 sha name n best=0
    declare -A tags=()
    while read -r sha name; do
        [[ $name =~ ^refs/tags/026g-rev([0-9]+)(\^\{\})?$ ]] || continue
        n=${BASH_REMATCH[1]}
        # An annotated tag's commit is the peeled (^{}) line.
        if [[ -n ${BASH_REMATCH[2]} || -z ${tags[$n]:-} ]]; then tags[$n]=$sha; fi
    done < <(git -C "$REPO" ls-remote origin 'refs/tags/026g-rev*' 2>/dev/null || true)
    SHIPPED_SINCE=''
    for n in "${!tags[@]}"; do
        if ((n < rev && n > best)); then best=$n SHIPPED_SINCE=${tags[$n]}; fi
    done
    if [[ -z $SHIPPED_SINCE ]] || ! git -C "$REPO" cat-file -e "$SHIPPED_SINCE^{commit}" 2>/dev/null; then
        SHIPPED_SINCE=$(git -C "$REPO" log --first-parent --diff-merges=first-parent -s -1 --format=%H \
            -G '^REVISION = ' "$commit" -- srp-026g/client-patches/patch_026g.py 2>/dev/null || true)
    fi
    git -C "$REPO" log --first-parent --merges --format='%s%x1f%b%x1e' "${SHIPPED_SINCE:+$SHIPPED_SINCE..}$commit" |
        awk 'BEGIN { RS = "\036"; FS = "\037" }
            {
                subject = $1; sub(/^\n+/, "", subject)
                if (subject !~ /^Merge pull request #[0-9]+ from [^ ]+/) next
                n = subject; sub(/^Merge pull request #/, "", n); sub(/ .*/, "", n)
                from = subject; sub(/^Merge pull request #[0-9]+ from /, "", from); sub(/ .*/, "", from)
                if (from ~ /\/release\/record-rev[0-9]+$/) next
                title = $2; sub(/^\n+/, "", title); sub(/\n.*/, "", title)
                if (title == "") title = from
                print "#" n " " title
            }' | tac
}

# The row of the revision table (client-patches/README.md) of this release.
readme_row() { # <shipped: lines>
    local shipped='' line content
    while IFS= read -r line; do
        [[ -n $line ]] || continue
        shipped+="${shipped:+; }${line//|/\\|}"
    done <<<"$1"
    if [[ -n ${published:-} ]]; then
        content="Its content: \`${resources##*/}\` ($(size_of "$resources") bytes, sha256 \`$(sha256_of "$resources")\`)"
    else
        content="No content patch of its own"
    fi
    # shellcheck disable=SC2016 # the backticks are Markdown
    printf '| Revision %s | Released %s UTC (tag `%s`, `main` %s). Shipped: %s. Without a server sha256 `%s`, with `--server=%s` `%s`. The ZIP: %s bytes, sha256 `%s`; the server data: sha256 `%s`. %s |\n' \
        "$rev" "$(date -u -d "@$released_at" '+%Y-%m-%d %H:%M')" "$(release_tag "$rev")" "${main_commit:0:7}" \
        "${shipped:-nothing merged since the release before}" "$EXE_WITHOUT" "$SKHID_GAME_SERVER" "$EXE_WITH" \
        "$ZIP_SIZE" "$ZIP_SHA" "$DATA_SHA" "$content"
}

# The record's files in the worktree <wt>: REVISION, the table's row, the
# changelog's entry, the list. False when one cannot be written.
record_files() { # <wt> <shipped>
    local wt=$1 shipped=$2 file line stub
    file=$wt/srp-026g/client-patches/patch_026g.py
    sed -i "s/^REVISION = [0-9][0-9]*\$/REVISION = $rev/" "$file" &&
        [[ $(sed -n 's/^REVISION = \([0-9][0-9]*\)$/\1/p' "$file") == "$rev" ]] || {
        record_problem "REVISION = $rev could not be written in patch_026g.py"
        return 1
    }
    # A change of what a match computes: this revision for its `next`.
    file=$wt/$SIMULATION_CHANGES
    if [[ -n $SIM_NEXT ]]; then
        if ! { sed -i "s/^next\t/$rev\t/" "$file" 2>/dev/null && ! grep -q $'^next\t' "$file" && grep -q "^$rev"$'\t' "$file"; }; then
            record_problem "revision $rev could not be written for \`next\` in $SIMULATION_CHANGES (the next release's server would take its own revision for it)"
            return 1
        fi
    fi
    file=$wt/srp-026g/client-patches/README.md
    if [[ -f $file ]]; then
        ROW=$(readme_row "$shipped") awk '{ line[NR] = $0 }
            /^\| Revision [0-9]+ \|/ { if ($3 + 0 >= best) { best = $3 + 0; at = NR } }
            END {
                for (i = 1; i <= NR; i++) { print line[i]; if (i == at) print ENVIRON["ROW"] }
                if (!at) print ENVIRON["ROW"]
            }' "$file" >"$file.new" && mv "$file.new" "$file"
    else
        record_problem "no srp-026g/client-patches/README.md: the revision table's row is not written"
    fi
    file=$wt/CHANGELOG-SKHID-026G.md
    stub="## $(LC_ALL=C date -u -d "@$released_at" '+%-d %b, %H:%M') — TODO: the owner's words (update required)"$'\n\n'
    stub+="<!-- Revision $rev ($(release_tag "$rev")), released $(date -u -d "@$released_at" '+%Y-%m-%d %H:%M') UTC. To be worded for players; what it shipped: -->"$'\n\n'
    while IFS= read -r line; do
        [[ -z $line ]] || stub+="- TODO: $line"$'\n'
    done <<<"$shipped"
    [[ -n $shipped ]] || stub+="- TODO: nothing was merged since the release before (the server and the ZIP as they were, a new revision)"$'\n'
    [[ -f $file ]] || printf '# Skhid 0.26g — what changed\n\n---\n' >"$file"
    STUB=$stub awk '{ print } $0 == "---" && !done { print ""; printf "%s", ENVIRON["STUB"]; done = 1 }
        END { if (!done) { print ""; printf "%s", ENVIRON["STUB"] } }' "$file" >"$file.new" && mv "$file.new" "$file"
    mkdir -p "$wt/release"
    LIST_RECORDED=0
    if ! cmp -s "$PATCH_LIST" "$wt/release/patches.tsv"; then
        cp "$PATCH_LIST" "$wt/release/patches.tsv"
        LIST_RECORDED=1
    fi
    # The site's pages the preflight rebuilt, as the server has them now.
    if [[ -n $site_tree ]]; then
        for page in "${SITE_PAGES[@]}"; do
            if ! { rm -rf "${wt:?}/$page" && cp -a "$site_tree/$page" "$wt/$page"; }; then
                record_problem "the rebuilt $page could not be copied into the record"
                return 1
            fi
        done
    fi
}

record_release() {
    local tag branch wt shipped url answer line
    tag=$(release_tag "$rev") branch=$(record_branch "$rev") wt=$work/record
    progress "7. the record: tag $tag on $(git -C "$REPO" log -1 --format=%h "$main_commit"), the commit \"Release revision $rev\" on main"
    EXE_WITH=$(awk '$2 == "with-server" { print $1 }' "$work/zip/exe.sha256" 2>/dev/null || true)
    EXE_WITHOUT=$(awk '$2 == "without-server" { print $1 }' "$work/zip/exe.sha256" 2>/dev/null || true)
    EXE_WITH=${EXE_WITH:-unknown} EXE_WITHOUT=${EXE_WITHOUT:-unknown}
    shipped=$(shipped_prs "$main_commit")
    say "  shipped (merged PRs since ${SHIPPED_SINCE:0:7}): $(if [[ -n $shipped ]]; then tr '\n' ';' <<<"$shipped" | sed 's/;$//; s/;/; /g'; else printf 'none'; fi)"
    # The tag, on the commit that was built.
    {
        say "Revision $rev, released $(date -u -d "@$released_at" '+%Y-%m-%d %H:%M:%S') UTC"
        say
        say "The players' ZIP, $ZIP_NAME: $ZIP_SIZE bytes, sha256 $ZIP_SHA"
        say "The server data, $DATA_NAME: sha256 $DATA_SHA"
        say "The exe: sha256 $EXE_WITH (--server=$SKHID_GAME_SERVER), $EXE_WITHOUT without a server"
        if [[ -n ${published:-} ]]; then say "Its content: ${resources##*/}, sha256 $(sha256_of "$resources")"; fi
        say
        say "Built from this commit with patch revision $rev (release/build-zip --revision $rev);"
        say "patch_026g.py here says REVISION $main_rev, the release before."
    } >"$work/tag.msg"
    if ! git -C "$REPO" tag -a "$tag" "$main_commit" -F "$work/tag.msg" 2>"$work/tag.log"; then
        record_problem "the tag $tag: $(tr '\n' ' ' <"$work/tag.log")— by hand: git -C $(q "$REPO") tag -a $tag $main_commit -F $(q "$work/tag.msg") && git -C $(q "$REPO") push origin refs/tags/$tag"
    elif git -C "$REPO" push --quiet origin "refs/tags/$tag" 2>>"$work/tag.log"; then
        say "  done     tag $tag on ${main_commit:0:7}, pushed"
    else
        # The tag is made here: only its push is left.
        record_problem "the push of the tag $tag: $(tr '\n' ' ' <"$work/tag.log")— by hand: git -C $(q "$REPO") push origin refs/tags/$tag"
    fi
    # The commit, on a branch from the commit that was built, and its PR.
    if ! git -C "$REPO" worktree add --quiet --detach "$wt" "$main_commit" 2>"$work/record.log"; then
        record_problem "the record's worktree: $(tr '\n' ' ' <"$work/record.log")"
        return 0
    fi
    record_files "$wt" "$shipped" || { git -C "$REPO" worktree remove --force "$wt"; return 0; }
    {
        say "Release revision $rev"
        say
        say "Released $(date -u -d "@$released_at" '+%Y-%m-%d %H:%M') UTC by release/release.sh from main ${main_commit:0:7} (tag $tag):"
        say "REVISION $rev in patch_026g.py, its row in the revision table of"
        if ((LIST_RECORDED)); then
            say "srp-026g/client-patches/README.md, a changelog entry for the owner to word,"
            say "the offered-patch list (release/patches.tsv) as the server offers it now."
        else
            say "srp-026g/client-patches/README.md, a changelog entry for the owner to word."
        fi
        if [[ -n $site_tree ]]; then
            say
            say "The Maps tab and the calculator (srp-026g/servers/web-server/maps/ and"
            say "damage/) rebuilt from the offered patches by release/build-site, as the"
            say "server has them now."
        fi
        if [[ -n $SIM_NEXT ]]; then
            say
            say "It changes what a match computes ($SIM_NEXT): $rev in place of"
            say "\`next\` in $SIMULATION_CHANGES; recordings of games before"
            say "revision $rev are no longer given (the player is told why)."
        fi
        say
        say "Shipped:"
        if [[ -n $shipped ]]; then while IFS= read -r line; do say "- $line"; done <<<"$shipped"; else say "- nothing merged since the release before"; fi
        say
        say "The ZIP: sha256 $ZIP_SHA; the server data: sha256 $DATA_SHA."
    } >"$work/record.msg"
    if ! { git -C "$wt" add -A && git -C "$wt" commit --quiet -F "$work/record.msg"; } >>"$work/record.log" 2>&1; then
        record_problem "the record's commit: $(tail -n 3 "$work/record.log" | tr '\n' ' ')"
    elif ! git -C "$wt" push --quiet origin "HEAD:refs/heads/$branch" >>"$work/record.log" 2>&1; then
        record_problem "the push of $branch: $(tail -n 3 "$work/record.log" | tr '\n' ' ')— the commit is $(git -C "$wt" rev-parse HEAD) (in $REPO); by hand: git -C $(q "$REPO") push origin $(git -C "$wt" rev-parse HEAD):refs/heads/$branch && gh pr create --base main --head $branch --title $(q "Release revision $rev") --body-file $(q "$work/record.msg") && gh pr merge $branch --merge"
    else
        say "  done     the commit \"Release revision $rev\" ($(git -C "$wt" log -1 --format=%h)) on $branch, pushed"
        if ! answer=$(cd "$wt" && gh pr create --base main --head "$branch" --title "Release revision $rev" --body-file "$work/record.msg" 2>&1); then
            record_problem "the record's PR: gh pr create: $(tail -n 2 <<<"$answer" | tr '\n' ' ')— open it by hand: $branch to main"
        else
            url=$(grep -Eo 'https?://[^ ]+' <<<"$answer" | tail -n 1)
            say "  done     the PR: ${url:-$answer}"
            if answer=$(cd "$wt" && gh pr merge "${url:-$branch}" --merge 2>&1); then
                say "  done     the PR merged (a merge commit): main has REVISION $rev"
            else
                record_problem "the record's PR ${url:-$branch} is OPEN, NOT MERGED: gh could not merge it ($(tail -n 2 <<<"$answer" | tr '\n' ' ')): merge it (a merge commit) before the next release"
            fi
        fi
    fi
    git -C "$REPO" worktree remove --force "$wt" 2>/dev/null || true
}

# What a stop does, by phase.
on_exit() {
    local status=$?
    set +e
    # The cleanup must finish: a second Ctrl-C, a closed terminal (HUP to
    # the whole group) or the death of the release.log copy must not cut it
    # short (a maintenance left planned would put the server down with
    # nothing swapped).
    trap 'printf "release.sh: cleaning up, wait\n" >&2' INT TERM HUP
    trap '' PIPE
    if [[ -n $LOG_PID ]] && ! kill -0 "$LOG_PID" 2>/dev/null; then exec 1>&8 2>&9; fi
    if [[ -n $NAP_PID ]]; then kill "$NAP_PID" 2>/dev/null; fi
    case $phase in
    planned)
        say
        say "STOPPED before the swap; putting the $TARGET server back as it was:"
        swap_failed
        if [[ -z $SWAP_STATE ]]; then stage_abandon; fi
        if ((image_tagged)); then
            if on_target "docker tag $(q "$RUN_IMAGE") $IMAGE && docker rmi $BEFORE_TAG >/dev/null"; then
                say "image: $IMAGE is the running one again ($RUN_IMAGE)"
            else
                say "image: NOT put back: docker tag $RUN_IMAGE $IMAGE"
            fi
        fi
        if ((maint_planned)); then cancel_maintenance; fi
        if [[ -n $resources && -n ${published:-} ]]; then
            say "${resources##*/} stays published on $PATCHES_LABEL, with its row in $PATCH_LIST (patches are never deleted); the server does not offer it yet. Next time: no --resources."
        fi
        ;;
    swapping) swap_failed ;;
    finished)
        if ((RECORDING)); then
            say
            say "RELEASED, BUT THE RECORD IS NOT COMPLETE: stopped during step 7 (players have revision $rev). On origin, see what is there of the tag $(release_tag "$rev"), the branch $(record_branch "$rev") and its PR, and finish the record by hand (the PR merged as a merge commit) before the next release."
        fi
        ;;
    esac
    case $phase in
    planned | swapping | finished) save_release_log "$work/release.log" ;;
    esac
    lock_drop
    if [[ -d $work/zip/src ]]; then git -C "$REPO" worktree remove --force "$work/zip/src" 2>/dev/null; fi
    if [[ -d $work/record ]]; then git -C "$REPO" worktree remove --force "$work/record" 2>/dev/null; fi
    if [[ -d $work/site ]]; then git -C "$REPO" worktree remove --force "$work/site" 2>/dev/null; fi
    exit "$status"
}
trap on_exit EXIT
trap 'die "interrupted"' INT TERM HUP

# 1. Preflight.
problems=()
problem() {
    problems+=("$*")
    say "  MISSING  $*"
}
okay() { say "  ok       $*"; }
ssh_check() { ssh -o BatchMode=yes -o ConnectTimeout=20 "$@" </dev/null >/dev/null 2>&1; }
free_mb() { df -Pk "$1" | awk 'NR == 2 { print int($4 / 1024) }'; }

progress "1. preflight (nothing players see)"
jump_ok=0 target_ok=0 patches_ok=0
if ssh_check "$SKHID_JUMP" true; then jump_ok=1; okay "ssh to the jump host ($SKHID_JUMP)"; else problem "ssh to the jump host ($SKHID_JUMP) fails (ssh -o BatchMode=yes $SKHID_JUMP true)"; fi
if [[ -n $TARGET_VIA && $TARGET_VIA != "$SKHID_JUMP" ]]; then
    if ssh_check "$TARGET_VIA" true; then okay "ssh to $TARGET_VIA"; else problem "ssh to $TARGET_VIA (the way to the $TARGET host) fails"; fi
fi
if [[ -n $TARGET_VIA ]]; then
    ssh_check "$TARGET_VIA" "ssh -o BatchMode=yes -o ConnectTimeout=20 $(q "$TARGET_SSH") true" && target_ok=1
else
    ssh_check "$TARGET_SSH" true && target_ok=1
fi
if ((target_ok)); then okay "ssh to the $TARGET host ($TARGET_SSH${TARGET_VIA:+ through $TARGET_VIA})"; else problem "ssh to the $TARGET host ($TARGET_SSH${TARGET_VIA:+ through $TARGET_VIA}) fails"; fi
if ssh_check "$PATCHES_SSH" true; then patches_ok=1; okay "ssh to $PATCHES_LABEL"; else problem "ssh to $PATCHES_LABEL fails"; fi
if ((jump_ok)); then
    owner=$(lock_owner || echo "unknown (the lock could not be read)")
    if [[ -z $owner ]]; then okay "no other release to the $TARGET server is running (the lock on the jump host is free)"; else problem "a release to the $TARGET server holds the lock on the jump host: $owner. Gone? ssh $SKHID_JUMP rm -r $(lock_path)"; fi
fi
if command -v nix >/dev/null; then okay "nix"; else problem "nix is not installed here (the ZIP and srp-game-data are built with it)"; fi
command -v python3 >/dev/null || command -v nix >/dev/null || problem "python3 (or nix to get it)"
if [[ -d $SKHID_GAME ]]; then okay "SKHID_GAME: $SKHID_GAME"; else problem "SKHID_GAME: $SKHID_GAME is not a folder (the original 0.26g install)"; fi
if [[ -f $SKHID_STOCK_DB ]]; then
    cache=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release/stock-db.sha256
    key="$(stat -c '%s %Y' "$SKHID_STOCK_DB") $(readlink -f "$SKHID_STOCK_DB")"
    stock=$(grep -F -- "$key " "$cache" 2>/dev/null | tail -n 1 || true)
    stock=${stock##* }
    if [[ -z $stock ]]; then
        say "  ...      sha256 of $SKHID_STOCK_DB ($(size_of "$SKHID_STOCK_DB") bytes; once, then cached by size and time)"
        stock=$(sha256_of "$SKHID_STOCK_DB")
        { mkdir -p "${cache%/*}" && printf '%s %s\n' "$key" "$stock" >>"$cache"; } 2>/dev/null || true
    fi
    if [[ $stock == "$STOCK_SHA" ]]; then okay "SKHID_STOCK_DB: the stock resources.db (sha256 ${STOCK_SHA:0:12}…)"; else problem "SKHID_STOCK_DB: $SKHID_STOCK_DB has sha256 $stock, not the stock 0.26g one ($STOCK_SHA)"; fi
else
    problem "SKHID_STOCK_DB: $SKHID_STOCK_DB: no such file (the stock 0.26g resources.db)"
fi
need=${SKHID_MIN_FREE_MB:-4096}
have=$(free_mb "$work")
if ((have >= need)); then okay "free here: $have MB in ${work%/*}"; else problem "free here: $have MB in ${work%/*}, $need wanted (SKHID_MIN_FREE_MB)"; fi
if bin=$(game_data_bin 2>"$work/game-data-build.log"); then okay "srp-game-data: $bin"; else bin=''; problem "srp-game-data does not build (log: $work/game-data-build.log)"; fi

# The revision of this release: the target exe's + 1 (or --revision), built
# so (build-zip --revision); main's REVISION is the last release's, which
# the record of this one sets (live only).
main_rev='' main_commit='' live_rev=unknown rev=''
if git -C "$REPO" fetch --quiet origin "$ref:refs/remotes/origin/$ref" 2>"$work/fetch.log"; then
    main_commit=$(git -C "$REPO" rev-parse "origin/$ref")
    main_rev=$(revision_at "$REPO" "$main_commit")
    if is_revision "${main_rev:-x}"; then okay "origin/$ref: $(git -C "$REPO" log -1 --format='%h %s' "$main_commit"), REVISION $main_rev (the last release's)"; else problem "no REVISION in patch_026g.py of origin/$ref"; main_rev=''; fi
else
    problem "git fetch origin $ref failed ($(tr '\n' ' ' <"$work/fetch.log"))"
fi
if (fetch_site_zip "$TARGET_SITE" "$work/live.zip") 2>"$work/live-zip.log"; then
    live_rev=$(zip_revision "$work/live.zip" "$work/live.exe")
    if [[ $live_rev == unknown ]]; then
        problem "the exe in $TARGET_SITE/download says no revision this script can read"
    elif [[ -n $revision_arg ]] && ((revision_arg <= live_rev && !test_server)); then
        problem "--revision $revision_arg is not above the live exe's $live_rev: a release's revision is above it (without --revision: $((live_rev + 1)))"
    elif ((live_rev >= 255)) && [[ -z $revision_arg ]]; then
        problem "the $TARGET exe is revision 255, the last one a byte holds"
    else
        rev=${revision_arg:-$((live_rev + 1))}
        if [[ -n $revision_arg ]] && ((rev <= live_rev)); then
            okay "the test exe is revision $live_rev; this release --revision $rev (not above it: fine for the test environment)"
        else
            okay "the $TARGET exe is revision $live_rev: this release is revision $rev$([[ -n $revision_arg ]] && printf ' (--revision)')"
        fi
    fi
else
    problem "$(cat "$work/live-zip.log")"
fi
if [[ -n $rev && -n $main_rev ]]; then
    if ((rev < main_rev)); then
        problem "revision $rev is below REVISION $main_rev of $ref, the last release's (patch_026g.py builds none below it): --revision $((main_rev + 1))"
    elif ((!test_server && main_rev >= rev)); then
        problem "main's REVISION $main_rev says revision $main_rev went out already, and the live exe says $live_rev (rolled back since?): a revision once released is never used again: --revision $((main_rev + 1))"
    elif ((!test_server && main_rev < live_rev)); then
        problem "main's REVISION $main_rev is below the live exe's $live_rev: the record of revision $live_rev is not on main (its PR still open? the end of that release's log says what was not done): merge it, or finish it by hand, first"
    fi
fi
# The record of a live release: its tag and branch free, gh signed in.
if ((!test_server)) && [[ -n $rev && -n $main_commit ]]; then
    taken=$(git -C "$REPO" ls-remote origin "refs/tags/$(release_tag "$rev")" "refs/heads/$(record_branch "$rev")" 2>"$work/ls-remote.log") ||
        taken="(git ls-remote origin failed: $(tr '\n' ' ' <"$work/ls-remote.log"))"
    if [[ -n $taken ]]; then
        problem "the record of revision $rev is on origin already: $(awk '{ print $NF }' <<<"$taken" | tr '\n' ' ')— released before? --revision $((rev + 1))"
    elif git -C "$REPO" rev-parse -q --verify "refs/tags/$(release_tag "$rev")" >/dev/null; then
        problem "a tag $(release_tag "$rev") is in $REPO, not on origin (left by a run?): git -C $REPO tag -d $(release_tag "$rev")"
    else
        okay "the record: tag $(release_tag "$rev") and branch $(record_branch "$rev") are free on origin"
    fi
    if ! git -C "$REPO" var GIT_COMMITTER_IDENT >/dev/null 2>&1; then
        problem "git in $REPO has no user.name and user.email: the record's tag and commit are made as the git user (git config user.name/user.email)"
    fi
    if ! command -v gh >/dev/null; then
        problem "gh is not installed here: the record of a release is a PR it opens and merges"
    elif ! (cd "$REPO" && gh auth status) >"$work/gh-auth.log" 2>&1; then
        problem "gh is not signed in (gh auth status): the record of a release is a PR it opens and merges"
    else
        okay "gh is signed in (the record's PR)"
    fi
fi

# The offered patches: the list, each on the patches server and at every
# URL of its row.
published=''
listed=$(grep -c . <<<"$LIST" || true)
if ((patches_ok && listed)); then
    names=$(cut -d' ' -f1 <<<"$LIST" | tr '\n' ' ')
    there=$(on_patches "cd $(q "$PATCHES_DIR") && for f in $names; do if test -f \"\$f\"; then echo \"\$f \$(stat -c %s \"\$f\") \$(sha256sum < \"\$f\" | cut -d' ' -f1)\"; else echo \"\$f missing\"; fi; done" || echo "unreadable")
    bad=0
    urls_checked=0
    while read -r name sha size urls; do
        [[ -n $name ]] || continue
        if ! grep -qx "$name $size $sha" <<<"$there"; then
            bad=1
            problem "$PATCH_LIST: $name ($size bytes, sha256 ${sha:0:12}…) is not so on $PATCHES_LABEL: $(grep "^$name " <<<"$there" || echo "$there")"
            continue
        fi
        read -r -a each <<<"$urls"
        for url in "${each[@]}"; do
            say "  ...      $name at $url"
            if check_patch_url "$url" "$sha" "$size" "$work"; then
                urls_checked=$((urls_checked + 1))
            else
                bad=1
                problem "$PATCH_LIST: $name: $URL_PROBLEM"
            fi
        done
    done <<<"$LIST"
    ((bad)) || okay "the offered-patch list: $listed patch(es), each on $PATCHES_LABEL with its sha256, and its $urls_checked URL(s) giving it"
elif ((patches_ok)); then
    okay "the offered-patch list ($PATCH_LIST) is empty"
fi
if [[ -n $rev ]] && ((LIST_TOP > rev)); then
    problem "$PATCH_LIST offers resources.db.patch_$LIST_TOP, above this release's revision $rev"
fi
if ((!test_server)) && [[ -n $main_commit ]] && ! git -C "$REPO" show "$main_commit:release/patches.tsv" 2>/dev/null | cmp -s - "$PATCH_LIST"; then
    say "  note     $PATCH_LIST is not main's release/patches.tsv: this release's record commits it"
fi

# The new patch.
if [[ -n $resources ]]; then
    name=${resources##*/}
    if [[ -n $(list_row "$name") ]]; then
        problem "--resources: $name is in $PATCH_LIST already: run without --resources (the list is what is offered)"
    elif [[ -n $rev && $name != "resources.db.patch_$rev" ]]; then
        problem "--resources: $name must be called resources.db.patch_$rev (this release's revision; the game mounts patch_<n> of revision n)"
    elif [[ ! -f $resources ]]; then
        problem "--resources: $resources: no such file"
    elif [[ $(head -c 13 "$resources") != little-endian ]]; then
        problem "--resources: $resources is not an archive of the game (no \"little-endian\" header)"
    elif [[ -n $bin && -f $SKHID_STOCK_DB ]]; then
        if "$bin" build --db "$SKHID_STOCK_DB" --patch "$resources=$PATCHES_URL/$name" \
            --out "$work/patch-check.db" >"$work/patch-check.log" 2>&1; then
            okay "--resources: $name, $(size_of "$resources") bytes, sha256 $(sha256_of "$resources"); srp-game-data reads it"
        else
            problem "--resources: srp-game-data cannot read $resources over the stock db: $(tail -n 3 "$work/patch-check.log" | tr '\n' ' ')"
        fi
        rm -f "$work/patch-check.db"
    fi
    if ((patches_ok)) && [[ -f $resources ]]; then
        there=$(on_patches "cd $(q "$PATCHES_DIR") && if test -e $(q "$name"); then sha256sum < $(q "$name"); else echo none; fi" || echo "unknown")
        if [[ $there == none ]]; then
            okay "--resources: not on $PATCHES_LABEL yet"
        elif [[ ${there%% *} == "$(sha256_of "$resources")" ]]; then
            problem "--resources: $name is already published, these same bytes, but not in $PATCH_LIST: add its row with release/upload-patch${step_flags[*]:+ ${step_flags[*]}} $resources, then run without --resources"
        else
            problem "--resources: $name is already published with OTHER bytes (${there%% *}); a published patch is never replaced: make the next revision"
        fi
    fi
fi
# The menus: from revision 12 on, patch_12 must be offered.
if [[ -n $rev ]] && ((rev >= MENUS_PATCH)) && [[ -z $(list_row "resources.db.patch_$MENUS_PATCH") &&
    ${resources##*/} != "resources.db.patch_$MENUS_PATCH" ]]; then
    problem "revision $rev needs resources.db.patch_$MENUS_PATCH (the menus) offered, and $PATCH_LIST has no row of it: release it (--resources) first"
fi
# The site's pages built from the game (the Maps tab, the calculator): each
# from exactly the patches the release offers (with --resources, that one
# too), so neither shows stale data. Built from others on main (a release
# with new content always is): rebuilt below, at the end of the preflight
# (release/build-site, in a worktree of the commit), nothing players see; the
# server's source carries them (so the server code is built) and the record
# commits them. A ref without them has nothing to build.
SITE_PAGES=(srp-026g/servers/web-server/maps srp-026g/servers/web-server/damage)
site_tree='' site_stale=0 want=''
if ((!test_server)) && [[ -n $main_commit ]]; then
    site_with=''
    if [[ -n $resources && -f $resources ]]; then site_with=$resources; fi
    want=$(site_wants "$site_with")
    for page in "the Maps tab:maps" "the calculator:damage"; do
        IFS=: read -r what dir <<<"$page"
        built=srp-026g/servers/web-server/$dir/built-from.tsv
        if ! git -C "$REPO" cat-file -e "$main_commit:$built" 2>/dev/null; then
            say "  note     no $built on $ref: nothing to build"
        elif [[ $(built_from <(git -C "$REPO" show "$main_commit:$built")) == "$want" ]]; then
            okay "$what: built from the patches this release offers"
        else
            say "  note     $what ($built on $ref) was built from other patches than this release offers: the release rebuilds it"
            site_stale=1
        fi
    done
    if ((site_stale)) && [[ $server_code == no ]]; then
        problem "the Maps tab and the calculator are rebuilt by this release, and --server-code no builds no server code: the site would go on showing the old ones (leave --server-code out)"
    fi
fi

# The target: disk, leftovers, settings, the container, the admin page.
if ((target_ok)); then
    have=$(on_target "df -Pk $D | awk 'NR == 2 { print int(\$4 / 1024) }'" || echo 0)
    need=${SKHID_LIVE_MIN_FREE_MB:-2048}
    if ((have >= need)); then okay "free on the $TARGET host: $have MB"; else problem "free on the $TARGET host: $have MB in $TARGET_DIR, $need wanted (SKHID_LIVE_MIN_FREE_MB)"; fi
    left=$(on_target "cd $D && for f in release-rev*/state; do if test -f \"\$f\"; then case \$(cat \"\$f\") in staging | swapping) echo \"\${f%/state} (\$(cat \"\$f\"))\" ;; esac; fi; done; ls -d download/.*.new 2>/dev/null; true")
    if [[ -z $left ]]; then okay "no release folder left unfinished"; else problem "left unfinished in $TARGET_DIR: $(tr '\n' ' ' <<<"$left")— look at it (its release.log), finish or roll it back (release/rollback${step_flags[*]:+ ${step_flags[*]}} <folder>), then set its state to stopped"; fi
    fs=$(on_target "cd $D && stat -L -c %d . download" || true)
    if [[ $(sort -u <<<"$fs" | grep -c .) == 1 ]]; then okay "download/ is on the file system of $TARGET_DIR (a release folder's files go in by rename)"; else problem "download/ is not on the file system of $TARGET_DIR (or not there): the swap hard-links the release folder's files into it"; fi
    blobs=$(missing_original_blobs)
    if [[ -z $blobs ]]; then okay "download/resources/ keeps the original menus' blobs (the updater's restore to stock)"; else problem "download/resources/ on the $TARGET host lacks the original blobs the updater's restore fetches: $(tr '\n' ' ' <<<"$blobs")"; fi
    present=$(on_target "cd $D && for f in download/$ZIP_NAME download/$DATA_NAME; do if test -f \$f; then echo \$f; fi; done")
    [[ $present == *"$ZIP_NAME"* ]] && HAVE_ZIP=1
    [[ $present == *"$DATA_NAME"* ]] && HAVE_DATA=1
    if OLD_MIN=$(current_min_revision 2>"$work/min.log"); then
        okay "SRP_MIN_PATCH_REVISION now: $OLD_MIN"
    else
        OLD_MIN=none
        problem "$(cat "$work/min.log")"
    fi
    if inspect_target; then
        if [[ $LATEST_IMAGE == "$RUN_IMAGE" ]]; then
            okay "srp26: image $RUN_IMAGE (what $IMAGE names), data volume $RUN_VOLUME"
        else
            problem "$IMAGE is $LATEST_IMAGE, but srp26 runs $RUN_IMAGE: a restart would run an image no release made (built by hand?); tag the running one back (docker tag $RUN_IMAGE $IMAGE) or release that one first"
        fi
    else
        problem "srp26's container on the $TARGET host cannot be read (docker compose ps / docker inspect)"
    fi
    compose_text=$(if [[ -n $main_commit ]]; then git -C "$REPO" show "$main_commit:srp-026g/docker-compose.yml" 2>/dev/null; fi || true)
    set_now=$(env_settings_set "$compose_text" || echo "(not readable)")
    if [[ -n $set_now ]]; then say "  note     main's compose file now passes on $set_now, which the $TARGET .env sets (names only): those values take effect at the recreate"; fi
    # The admin page plans the maintenance: --no-maintenance never asks it.
    if ((no_maint)); then
        okay "the admin page: not needed ($( ((now)) && printf -- '--now' || printf -- '--no-maintenance'))"
    else
        proxy=$(on_target "cd $D && grep -E '^SRP_WEB_TRUST_PROXY=' .env | tail -n 1" || true)
        if [[ ${proxy#*=} != on ]]; then
            problem "SRP_WEB_TRUST_PROXY is not on in the $TARGET .env: the admin page would see Caddy's address, not the jump host's"
        fi
        if read_admin; then
            answer=$(admin_request) || answer=000
            if [[ ${answer##*$'\n'} == 200 ]]; then
                okay "the admin page answers the jump host"
            else
                problem "the admin page does not answer the jump host (HTTP ${answer##*$'\n'}): its address must be in SRP_ADMIN_IPS"
            fi
        else
            problem "SRP_ADMIN_PATH and SRP_ADMIN_TOKEN are not both set in the $TARGET .env (the admin page is off)"
        fi
    fi
fi

# The site.
state=$(maintenance_state)
case $state in
none) okay "no maintenance planned on $TARGET_SITE" ;;
planned*) problem "a maintenance is planned already on $TARGET_SITE (${state#planned } min): another release? cancel it first" ;;
*) problem "$TARGET_SITE/launcher/places/en: maintenance state $state" ;;
esac
matches=$(matches_now)
if [[ -n $matches ]]; then okay "$TARGET_SITE/api/stats: $matches match(es) now"; else problem "$TARGET_SITE/api/stats gives no match count"; fi
OLD_PATCHES=$(served_patches)
say "  now      /patches: ${OLD_PATCHES:-(no list: a server from before patches)}"

# The site's pages rebuilt (about 2 min), last in the preflight so far, and
# only when nothing of it failed: a real run with a problem stops anyway.
if ((site_stale)); then
    build_site=("$BUILD_SITE" --tree "$work/site")
    if [[ -n $resources ]]; then build_site+=(--with "$resources"); fi
    if ((DRY_RUN)); then
        say "  [dry run] would rebuild the Maps tab and the calculator (about 2 min; the patches from ~/.cache/skhid-release/patches/, else downloaded) in a worktree of ${main_commit:0:7}: ${build_site[*]}"
    elif ((${#problems[@]})); then
        say "  note     the Maps tab and the calculator: not rebuilt, as the preflight has ${#problems[@]} problem(s) already"
    else
        say "  ...      rebuilding the Maps tab and the calculator (release/build-site, about 2 min; its log: $work/build-site.log)"
        site_start=$(date +%s)
        if ! git -C "$REPO" worktree add --quiet --detach "$work/site" "$main_commit" 2>"$work/build-site.log"; then
            problem "the Maps tab and the calculator: no worktree to rebuild them in: $(tr '\n' ' ' <"$work/build-site.log")"
        elif ! "${build_site[@]}" </dev/null >"$work/build-site.log" 2>&1; then
            problem "the Maps tab and the calculator: the rebuild failed (release/build-site; its log: $work/build-site.log): $(tail -n 3 "$work/build-site.log" | tr '\n' ' ')"
        else
            site_tree=$work/site
            for page in "the Maps tab:maps" "the calculator:damage"; do
                IFS=: read -r what dir <<<"$page"
                if [[ $(built_from "$site_tree/srp-026g/servers/web-server/$dir/built-from.tsv") != "$want" ]]; then
                    problem "$what, rebuilt: its built-from.tsv does not name the patches this release offers"
                    site_tree=''
                fi
            done
            if [[ -n $site_tree ]]; then
                okay "the Maps tab and the calculator: rebuilt from the patches this release offers ($(span $(($(date +%s) - site_start)))); the server's source carries them, the record commits them"
            fi
        fi
    fi
fi

# The server code: main's srp-026g build inputs against the live tree.
build_code=0 code_report=''
src_tar=$work/srp-026g-src.tar
if [[ -n $main_commit ]]; then
    mapfile -t build_paths < <(build_inputs "$main_commit")
    if [[ " ${build_paths[*]} " == *" !unsupported "* ]]; then
        problem "main's Dockerfile copies the whole folder or a pattern: say --server-code yes or no"
    elif git -C "$REPO" archive --format=tar -o "$src_tar" "$main_commit:srp-026g" "${build_paths[@]}" 2>"$work/archive.log"; then
        mkdir -p "$work/src"
        tar -xf "$src_tar" -C "$work/src"
        # The pages rebuilt above in place of main's, in the source too.
        if [[ -n $site_tree ]]; then
            for page in "${SITE_PAGES[@]}"; do
                rm -rf "$work/src/${page#srp-026g/}"
                cp -a "$site_tree/$page" "$work/src/${page#srp-026g/}"
            done
            tar -cf "$src_tar" -C "$work/src" "${build_paths[@]}"
        fi
        inputs=$(printf '%q ' "${build_paths[@]}")
        list="find $inputs -type f ! -path '*/__pycache__/*' ! -path '*/target/*' -print0 2>/dev/null | LC_ALL=C sort -z | xargs -0 -r sha256sum"
        (cd "$work/src" && bash -c "$list") >"$work/src-main.sha256"
        if ((target_ok)); then
            on_target "cd $D && $list" >"$work/src-live.sha256" || true
            code_report=$(awk 'NR == FNR { main[$2] = $1; next } { live[$2] = $1 }
                END {
                    for (p in main) if (!(p in live)) print "ADDED     " p; else if (main[p] != live[p]) print "CHANGED   " p
                    for (p in live) if (!(p in main)) print "only live " p
                }' "$work/src-main.sha256" "$work/src-live.sha256" | LC_ALL=C sort -k2)
            # The dry run rebuilds nothing: the pages a real run rebuilds
            # (and its source carries), as changed all the same.
            if ((site_stale && DRY_RUN)); then
                for page in "${SITE_PAGES[@]}"; do
                    code_report+="${code_report:+$'\n'}CHANGED   ${page#srp-026g/}/ (a real run rebuilds it)"
                done
                code_report=$(LC_ALL=C sort -k2 <<<"$code_report")
            fi
            code_changes=$(grep -cE '^(ADDED|CHANGED)' <<<"$code_report" || true)
            if ((code_changes)); then
                okay "server code: $code_changes file(s) of main differ from the live tree:"
                grep -E '^(ADDED|CHANGED)' <<<"$code_report" | sed -n '1,30s/^/             /p'
                ((code_changes <= 30)) || say "             ... ($code_changes in all)"
            else
                okay "server code: the live tree is main's"
            fi
            if grep -q '^only live' <<<"$code_report"; then
                say "             (and $(grep -c '^only live' <<<"$code_report") file(s) only in the live tree, left as they are)"
            fi
            case $server_code in
            auto) ((code_changes == 0)) || build_code=1 ;;
            yes) build_code=1 ;;
            no) ((code_changes == 0)) || say "  WARNING  --server-code no: the image is NOT built although the code differs" ;;
            esac
        fi
    else
        problem "git archive of main's srp-026g failed: $(cat "$work/archive.log")"
    fi
fi

# Replays: a change of what a match computes (a `next` row of the list,
# which a PR adds) makes this release's revision the oldest whose
# recordings are given; its server takes `next` for its
# SRP_MIN_PATCH_REVISION, and the record writes the number in.
SIM_NEXT='' sim_last=''
if [[ -n $main_commit ]]; then
    if sim=$(git -C "$REPO" show "$main_commit:$SIMULATION_CHANGES" 2>/dev/null); then
        SIM_NEXT=$(awk -F '\t' '$1 == "next" { print $2 }' <<<"$sim")
        sim_last=$(awk -F '\t' '$1 ~ /^[0-9]+$/ { n = $1 } END { print n }' <<<"$sim")
        if [[ -n $SIM_NEXT ]]; then
            okay "replays: this release changes what a match computes ($SIM_NEXT): recordings of games before revision ${rev:-<n>} are refused from it on (the player is told why); the record writes ${rev:-<n>} for \`next\` in $SIMULATION_CHANGES"
            if ((target_ok && !build_code)); then
                problem "replays: $SIMULATION_CHANGES has a \`next\` row, but the server code is not built (--server-code no): the server would go on giving recordings of older games to the new one"
            fi
        else
            okay "replays: no change of what a match computes is marked ($SIMULATION_CHANGES): recordings from revision ${sim_last:-?} on stay watchable"
            # The running server was built from a tree with `next` (the
            # release before changed what a match computes; its record
            # wrote the number on main since): made anew with this
            # release's SRP_MIN_PATCH_REVISION and not rebuilt, it would
            # take this revision for `next`.
            if ((target_ok && !build_code)) &&
                on_target "cd $D && awk -F '\t' '\$1 == \"next\" { found = 1 } END { exit !found }' ${SIMULATION_CHANGES#srp-026g/}" 2>/dev/null; then
                problem "replays: the live server was built with a \`next\` row of $SIMULATION_CHANGES, and the server code is not built (--server-code no): made anew with SRP_MIN_PATCH_REVISION=${rev:-<n>}, it would take ${rev:-<n>} for \`next\` and refuse the recordings of the games before it, which stay watchable"
            fi
            # The exe's patches changed since the last release: said, in
            # case a change of the simulation was not marked.
            if [[ -n $main_rev ]] && since=$(git -C "$REPO" rev-parse -q --verify "refs/tags/$(release_tag "$main_rev")^{commit}"); then
                exe_changed=$(exe_patch_changes "$since" "$main_commit")
                if [[ -n $exe_changed ]]; then
                    say "  note     the exe's patches changed since $(release_tag "$main_rev") ($(tr '\n' ' ' <<<"$exe_changed" | sed 's/ $//')), and no change of what a match computes is marked: if a match plays out otherwise now, stop, and have a PR add a \`next\` row to $SIMULATION_CHANGES first"
                fi
            fi
        fi
    else
        problem "no $SIMULATION_CHANGES in origin/$ref (the changes of what a match computes, for the replays)"
    fi
fi

if ((${#problems[@]})); then
    say
    say "preflight: ${#problems[@]} problem(s), nothing was done:"
    printf '  - %s\n' "${problems[@]}"
    # A dry run goes on, to show the rest, when it knows what to release.
    ((DRY_RUN)) && [[ -n $rev && -n $main_commit ]] || exit 1
    say "dry run: a real run stops here; what it would do after:"
fi
release_folder "$rev" "$(date -u +%Y%m%d-%H%M)" "$label"
SWAP_SRC=$build_code SWAP_SRC_INPUTS=${inputs:-}
new_row=''
if [[ -n $resources && -f $resources ]]; then
    new_row="${resources##*/} $(sha256_of "$resources") $(size_of "$resources") $PATCHES_URL/${resources##*/}"
fi
OFFERED=$(offered_of_list "$new_row")
say
say "the release: revision $live_rev -> $rev to the $TARGET server ($TARGET_SITE), its folder $TARGET_DIR/$REL"
say "  new content: ${resources:-none}"
say "  /patches will offer:"
say_offered
say "  server code: $(if ((build_code)); then say "built before the swap (docker compose build srp26)"; else say "not built"; fi)"
if [[ -n $site_tree ]]; then
    say "  the Maps tab and the calculator: rebuilt from what /patches will offer (in the server's source; the record commits them)"
elif ((site_stale)); then
    say "  the Maps tab and the calculator: a real run rebuilds them in its preflight (before the go) from what /patches will offer"
fi
if [[ -n $SIM_NEXT ]]; then
    say "  replays: what a match computes changes ($SIM_NEXT): recordings of games before revision $rev are refused from the swap on"
else
    say "  replays: what a match computes does not change (recordings from revision ${sim_last:-?} on stay watchable)"
fi
scale=""
((MINUTE == 60)) || scale=" (a minute here is $MINUTE s)"
if ((now)); then
    say "  maintenance: NONE, NO NOTICE (--now)$scale: after the go, prepared and staged, then the swap at once, matches running or not (they end without results; it says how many); lobby players get a ~30 s \"server sync\" and reconnect"
elif ((no_maint)); then
    say "  maintenance: none (--no-maintenance)$scale; after the go, prepared and staged, then the swap as soon as $TARGET_SITE/api/stats says \"matches\":0 (up to $match_wait min, --match-wait); lobby players get a ~30 s \"server sync\" and reconnect"
elif [[ -n $minutes ]]; then
    say "  maintenance: in $minutes min (--minutes)$scale, planned right after the go; players see the countdown, Play is locked in its last 30 min; prepared and staged during it; the swap at the later of its end and the staging's, once \"matches\":0"
else
    say "  maintenance: planned right after the go, to start 30 s after the last running match ends$scale; players see the countdown, Play is locked from then on; prepared and staged during it; the swap at the later of its end and the staging's, once \"matches\":0"
fi
# What comes right after the go.
if ((now)); then
    after_go="the build and the staging, then the swap, NO notice (--now)"
elif ((no_maint)); then
    after_go="the build and the staging (--no-maintenance: no notice)"
else
    after_go="the maintenance planned, then the build and the staging during the countdown"
fi

# 2. The lock, then the go: nothing before it.
if ((DRY_RUN)); then
    say
    say "[dry run] 2. take the release lock on the jump host ($(lock_path)), then wait for the go (y at the prompt, or touch $GO_DIR/go-$REL; --yes does not give it); nothing before it. Then at once: $after_go"
    if ((!no_maint)); then
        maintenance_minutes
        say "[dry run] 3. right after the go: plan the maintenance in $MAINT_MINUTES min (now: $MAINT_WHY), from the jump host:"
        say "  $(plan_command "minutes=$MAINT_MINUTES")"
        say "[dry run] then: $TARGET_SITE/launcher/places/en must say \"Maintenance in $MAINT_MINUTES min\""
    elif ((now)); then
        say "[dry run] 3. no maintenance planned (--now: the admin page is never asked)"
    else
        say "[dry run] 3. no maintenance planned (--no-maintenance: the admin page is never asked)"
    fi
else
    lock_take
    say
    if ((now)); then
        say "Next, at the go, with NO maintenance and NO notice (--now): the patch, the server data, the ZIP and the image prepared and staged,"
        say "then the swap at once, after one more y/N: running matches end WITHOUT RESULTS."
    elif ((no_maint)); then
        say "Next, at the go, with no maintenance: the patch, the server data, the ZIP and the image prepared and staged,"
        say "then the swap as soon as $TARGET_SITE/api/stats says \"matches\":0 (up to $match_wait min), after one more y/N."
    else
        say "Next, at the go: the maintenance planned on $TARGET_SITE at once, $(if [[ -n $minutes ]]; then printf 'in %s min (--minutes)' "$minutes"; else printf 'to start 30 s after the last running match ends'; fi): Play locks;"
        say "during the countdown the patch, the server data, the ZIP and the image prepared and staged; the swap at the later of"
        say "the planned time and the staging's end, once no match runs, after one more y/N. From the plan on, a stop cancels it."
    fi
    wait_for_go "$after_go"
    phase=planned
    start=$(date +%s)
    # The scripts run from here are covered by the go.
    export SKHID_RELEASE_GO_AHEAD=1
    if ((no_maint)); then
        progress "2. the go: preparing now$( ((now)) && printf ' (--now: NO maintenance, NO notice)' || printf ' (--no-maintenance)')"
    else
        progress "2. the go: planning the maintenance now"
        plan_maintenance
    fi
fi

# 4. Prepared and staged, during the countdown.
dry=()
((DRY_RUN)) && dry=(--dry-run)
if [[ -n $resources ]]; then
    progress "4. publishing ${resources##*/} on $PATCHES_LABEL"
    "$here/upload-patch" "${step_flags[@]}" "${dry[@]}" "$resources" </dev/null || die "upload-patch failed"
    ((DRY_RUN)) || published=1
fi
data=$work/$DATA_NAME
zip=$work/zip/$ZIP_NAME
build_zip=("$BUILD_ZIP" --ref "$main_commit" --revision "$rev" --work "$work/zip" "${step_flags[@]}")
if ((DRY_RUN)); then
    say "[dry run] 4. $( ((no_maint)) && printf 'after the go' || printf 'during the countdown'), build here: release/build-server-db${step_flags[*]:+ ${step_flags[*]}} --out $data"
    say "[dry run]    and: ${build_zip[*]}"
    say "[dry run]    then the checks of release/deploy on both, and the release folder:"
    stage_begin
    write_on live "cat > $REL_DIR/changed.txt" "$work/changed.txt"
    stage_upload "$data" "$DATA_NAME" "<its sha256>"
    stage_upload "$zip" "$ZIP_NAME" "<its sha256>"
else
    progress "4. building the server data (release/build-server-db)"
    "$here/build-server-db" "${step_flags[@]}" --out "$data" </dev/null || die "build-server-db failed"
    progress "4. building the ZIP of $(git -C "$REPO" log -1 --format=%h "$main_commit") (release/build-zip)"
    "${build_zip[@]}" </dev/null 2>&1 | tee "$work/build-zip.log" || die "build-zip failed"
    if [[ -d $work/zip/src ]]; then git -C "$REPO" worktree remove --force "$work/zip/src"; fi
    planned_offer=$OFFERED
    check_release_files "$zip" "$data" "$rev" "$work"
    [[ $OFFERED == "$planned_offer" ]] ||
        die "$data does not offer what the preflight said (the list and the new patch): $(tr '\n' ' ' <<<"$OFFERED")"
    say "ZIP: $ZIP_SIZE bytes, sha256 $ZIP_SHA, revision $rev; server data: sha256 $DATA_SHA; /patches will offer:"
    say_offered
    progress "4. staging in the release folder $TARGET_DIR/$REL (not in place yet)"
    {
        say "release $REL: origin/$ref $(git -C "$REPO" log -1 --format='%h %s' "$main_commit"), revision $live_rev -> $rev"
        say
        say "server source, main's srp-026g against the $TARGET tree$( ((build_code)) || printf ' (not built)'):"
        if [[ -n $code_report ]]; then say "$code_report"; else say "  (the same)"; fi
        say
        say "the players' ZIP, against the one the site hands out now:"
        grep -E '^  (CHANGED|ADDED|REMOVED) |^updater_skhid.exe' "$work/build-zip.log" || say "  (no entry changed)"
    } >"$work/changed.txt"
    stage_begin
    write_on live "cat > $REL_DIR/changed.txt" "$work/changed.txt"
    stage_upload "$data" "$DATA_NAME" "$DATA_SHA"
    stage_upload "$zip" "$ZIP_NAME" "$ZIP_SHA"
fi
if ((build_code)); then
    progress "4. the server code: building the image from main's source"
    write_on live "cat > $REL_DIR/src.tar" "$src_tar"
    if ((!DRY_RUN)); then
        there=$(on_target "sha256sum < $REL_DIR/src.tar")
        [[ ${there%% *} == "$(sha256_of "$src_tar")" ]] || die "$REL/src.tar has sha256 ${there%% *} on the host, not the one here"
    fi
    write_on live "cd $REL_DIR && mkdir src && tar -xf src.tar -C src && docker tag $(q "$RUN_IMAGE") $BEFORE_TAG"
    ((DRY_RUN)) || image_tagged=1
    write_on live "cd $D && docker compose --env-file $D/.env -f $REL/src/docker-compose.yml build srp26"
fi

# 5. The swap, at the later of the planned time and the end of the staging,
# once no match runs (--no-maintenance: no planned time; --now: at once).
if ((DRY_RUN && now)); then
    say "[dry run] 5. once staged: the swap at once, NO wait for matches (--now: the running ones end without results), y/N (no answer in $answer_wait min: no) and:"
    SWAP_BEFORE_STOP=count_ended_matches
elif ((DRY_RUN && no_maint)); then
    say "[dry run] 5. once staged: wait for \"matches\":0 on $TARGET_SITE/api/stats (up to $match_wait min), then y/N (no answer in $answer_wait min: no) and:"
    SWAP_BEFORE_STOP=match_check_before_stop
elif ((DRY_RUN)); then
    say "[dry run] 5. at the later of the planned time and the end of the staging (staged after it: at once): the launcher page saying the server is down, and \"matches\":0 on $TARGET_SITE/api/stats (up to $match_wait min more), then y/N (no answer in $answer_wait min: no) and:"
elif ((now)); then
    progress "5. staged; --now: the swap at once, NO wait for matches, NO notice was given"
elif ((no_maint)); then
    progress "5. staged; waiting for \"matches\":0"
    limit=$(($(date +%s) + match_wait * MINUTE))
    wait_no_match "after $match_wait min of waiting (--match-wait)"
else
    if (($(date +%s) < end)); then
        progress "5. staged before the planned time"
    else
        progress "5. staged after the planned time: the swap at once, once no match runs"
    fi
    while (($(date +%s) < end)); do
        progress "5. waiting for the planned time ($(clock "$end") UTC)"
        left=$((end - $(date +%s)))
        if ((left > MINUTE)); then pause 1; elif ((left > 0)); then nap "$left"; fi
    done
    limit=$(($(date +%s) + 5 * MINUTE))
    while :; do
        state=$(maintenance_state)
        [[ $state == down ]] && break
        [[ $state != none ]] || die "the maintenance is no longer planned on $TARGET_SITE (cancelled on the admin page, or srp restarted)"
        (($(date +%s) < limit)) || die "the launcher page still says \"$state\" 5 min after the planned time (the clocks?)"
        pause 0.25
    done
    limit=$(($(date +%s) + match_wait * MINUTE))
    wait_no_match "$match_wait min after the planned time (--match-wait)"
fi
if ((!DRY_RUN)); then
    if ((now)); then
        progress "5. --now: no wait for matches; $(m=$(matches_now || true); printf %s "${m:-?}") match(es) running now (counted again right before the stop)"
    else
        progress "5. no match is running"
    fi

    # What changes.
    say
    say "On the $TARGET host now:"
    on_target "cd $D && ls -l download/$ZIP_NAME download/$DATA_NAME 2>&1; sha256sum download/$ZIP_NAME download/$DATA_NAME 2>&1; true" | sed 's/^/  /'
    say "It becomes:"
    say "  download/$ZIP_NAME: $ZIP_SIZE bytes, sha256 $ZIP_SHA (revision $live_rev -> $rev)"
    say "  download/$DATA_NAME: sha256 $DATA_SHA"
    say "  .env: SRP_MIN_PATCH_REVISION $OLD_MIN -> $rev"
    say "  /patches: ${OLD_PATCHES:-(no list)} -> $EXPECTED"
    if ((build_code)); then say "  the image: main's source, built (the running one, $RUN_IMAGE, is tagged $BEFORE_TAG)"; fi
    say "  the ZIP's entries, against the live ZIP:"
    grep -E '^  (CHANGED|ADDED|REMOVED) |^updater_skhid.exe' "$work/build-zip.log" | sed 's/^ */    /' || say "    (no entry changed)"
    say "Next: srp26 stopped, the old files kept in the release folder, the server data in place, the revision set,"
    say "srp26 made anew (every lobby link drops; its start copies srp.db to backups/), checked; srp.db's copy kept;"
    say "then the ZIP. A failed check rolls it all back."
    if ((now)); then
        say "(--now: every match running at the stop ends WITHOUT RESULTS; how many is said right before it)"
    elif ((no_maint)); then
        say "(--no-maintenance: \"matches\":0 once more right before the stop; a match begun meanwhile is waited for, within the limit)"
    fi
    if ((yes)); then
        say "(--yes --i-know: going ahead)"
    else
        say "(no answer in $answer_wait min is a no)"
        ask_yes "$((answer_wait * MINUTE))" || die "stopped: not confirmed (nothing was swapped)"
    fi
    if ((now)); then
        SWAP_BEFORE_STOP=count_ended_matches
    elif ((no_maint)); then
        SWAP_BEFORE_STOP=match_check_before_stop
    fi
fi

progress "5. the swap"
swap_prepare "$rev"
((DRY_RUN)) || phase=swapping
swap_server "$rev"
progress "6. the server is up on the new data; the ZIP"
swap_zip

# 6. After.
if ((DRY_RUN)); then
    say "[dry run] 6. after the restart: srp26 running, its log's \"game data:\" line, GET $TARGET_SITE/patches is exactly the list's; srp's new backup of srp.db kept in the folder$( [[ $TARGET == live ]] && printf ' and copied to the jump host'); then the ZIP, GET $TARGET_SITE/api/client-files names it"
    say "[dry run] a failed check: srp's log, then sh $TARGET_DIR/$REL/rollback.sh $TARGET_DIR/$REL on the $TARGET host (srp.db, image, files, setting back)"
    if ((test_server)); then
        say "[dry run] 7. no record (--test: no commit, no tag)"
    else
        say "[dry run] 7. once it went out, the record: git tag -a $(release_tag "$rev") ${main_commit:0:7} (the UTC time, the ZIP's and the server data's sha256) and git push origin refs/tags/$(release_tag "$rev");"
        say "[dry run]    the commit \"Release revision $rev\" on ${main_commit:0:7} (REVISION = $rev, the README revision table's row, a changelog entry to word, release/patches.tsv as offered$( ((!site_stale)) || printf ", the rebuilt Maps tab and calculator")$([[ -z $SIM_NEXT ]] || printf ", %s for \`next\` in %s" "$rev" "$SIMULATION_CHANGES")), pushed as $(record_branch "$rev"), gh pr create, gh pr merge --merge"
        say "[dry run]    it would ship (merged PRs): $(shipped_prs "$main_commit" | tr '\n' ';' | sed 's/;$//; s/;/; /g')"
    fi
    say
    say "dry run: nothing was planned, uploaded or changed"
    if ((${#problems[@]})); then
        say "dry run: preflight found ${#problems[@]} problem(s) (above): a real run would stop before the lock"
        exit 1
    fi
    exit 0
fi
phase=finished
released_at=$(date -u +%s)
say
if ((now)); then say "--now: the stop ended ${ENDED:-an unknown number of} match(es) without results"; fi
say "released revision $rev to the $TARGET server. Its folder: $TARGET_DIR/$REL (the files before it, srp.db before it, the image IDs, changed.txt, release.log); the old image is $BEFORE_TAG ($RUN_IMAGE)"
if ((test_server)); then
    say "No record (--test): no commit, no tag."
else
    say
    # RECORDING: a stop in the middle of it (Ctrl-C, a closed terminal) is
    # said loudly too (on_exit).
    RECORDING=1
    if ! record_release; then record_problem "the record stopped part way (above)"; fi
    RECORDING=0
fi
say
say "Rollback: release/rollback${step_flags[*]:+ ${step_flags[*]}} $REL (on the $TARGET host: sh $TARGET_DIR/$REL/rollback.sh $TARGET_DIR/$REL)"
say "  (srp.db back as it was before the release; --keep-db keeps today's)"
say "Kept here: $zip, $data."
if ((test_server)) && [[ -n ${published:-} ]]; then say "$PATCH_LIST has the new row of ${resources##*/}: commit it (a test release makes no record)."; fi
if ((DB_COPY_FAILED)); then say "WARNING: srp.db's copy is NOT on the jump host (above): copy it by hand."; fi
say "Now: a game updates, downloads the patches, signs in; an old game is refused."
if ((${#RECORD_PROBLEMS[@]})); then
    say
    say "RELEASED, BUT THE RECORD IS NOT COMPLETE (players have revision $rev; main and the tags do not say so yet):"
    printf '  - %s\n' "${RECORD_PROBLEMS[@]}"
    exit 1
fi
