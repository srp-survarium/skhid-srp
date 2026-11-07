#!/usr/bin/env bash
# worlds.sh check|drift: tools/check's world-check and world-drift, over the world corpus,
# private/world-corpus of the main checkout ($SKHID_WORLD_CORPUS names another; its layout and
# refresh: srp-026g/docs/notes/packet-validation.md, "The world corpus"):
#   check  the server game data of this tree (release/build-content's quick build of the next
#          patch, or the offered-patch list's when the manifest ships nothing new), then the match
#          server's own upload check (srp-world-check) of every recording in live/ and local/*/:
#          one world refused, or not checked, fails it
#   drift  the game's world check (the arena's, under Unicorn: check_world_check_026g.py --drift)
#          against the host's (srp-validate's drift_dump) on a sample of the corpus: one upload in
#          $WORLD_DRIFT_ONE_IN (default 50) of each live recording, every upload of the local
#          ones and every kept local dump (dumps/), in $WORLD_DRIFT_JOBS parallel chunks (default
#          the cores less 4); needs the original exe ($SKHID_ORIGINAL_EXE, default
#          private/function-map/inputs/026g/survarium.exe of the main checkout)
# Run by tools/check: $CHECK_WORK is its folder.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../../release/lib.sh
. "$here/../../release/lib.sh"
work=${CHECK_WORK:?run by tools/check}
main=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")
corpus=${SKHID_WORLD_CORPUS:-$main/private/world-corpus}
[[ -d $corpus ]] || die "$corpus: no world corpus (release/world-corpus makes one)"

# The corpus's folders of recordings: live/ and each local/<config>/.
recording_folders() { # <folder>...
    local folder
    for folder in "$@"; do
        [[ -d $folder ]] || continue
        find "$folder" -name '*.srprec' -printf '%h\n'
    done | LC_ALL=C sort -u
}

# The server game data of the stock db and the offered-patch list's patches (cached).
list_data() { # <out>
    local bin args=() name urls
    load_env SKHID_STOCK_DB
    use_patches live
    read_patch_list
    cache_patches >&2
    bin=$(game_data_bin)
    while read -r name _ _ urls; do
        [[ -n $name ]] || continue
        args+=(--patch "$PATCH_CACHE/$name=$urls")
    done <<<"$LIST"
    "$bin" build --db "$SKHID_STOCK_DB" "${args[@]}" --out "$1" >/dev/null
}

check() {
    local data folders bin status=0
    say "the server game data of this tree (release/build-content, a quick build) ..."
    "$here/build_content.sh" >"$work/build-content.log" 2>&1 ||
        { tail -n 20 "$work/build-content.log"; die "build-content failed ($work/build-content.log)"; }
    data=$work/content/$DATA_NAME
    if [[ ! -f $data ]]; then
        say "the manifest ships no new patch: the list's server game data"
        data=$work/$DATA_NAME
        list_data "$data"
    fi
    mapfile -t folders < <(recording_folders "$corpus/live" "$corpus/local")
    ((${#folders[@]})) || die "$corpus: no recording in live/ or local/"
    bin=$(world_check_bin)
    say "srp-world-check of ${#folders[@]} folder(s) of $corpus ..."
    "$bin" --data "$data" --save "$work/refused" "${folders[@]}" >"$work/world-check.log" || status=$?
    grep -E '^  REFUSED |: UNREAD|not checked: no|^FAILED' "$work/world-check.log" | head -n 20 || true
    sed -n '/^--- refusals by kind/,$p' "$work/world-check.log"
    ((status == 0)) || die "the world check failed: $work/world-check.log, the refused worlds in $work/refused"
}

drift() {
    local one_in=${WORLD_DRIFT_ONE_IN:-50} jobs=${WORLD_DRIFT_JOBS:-$(($(nproc) - 4))}
    local exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
    local data bin folders status worlds=$work/worlds
    [[ $one_in =~ ^[1-9][0-9]*$ ]] || die "WORLD_DRIFT_ONE_IN: $one_in is not a number above 0"
    ((jobs >= 1)) || jobs=1
    [[ -f $exe ]] || die "$exe: the original 0.26g survarium.exe is needed (SKHID_ORIGINAL_EXE)"
    # The data only reads the recordings' rosters here: the uploads they hold are the same with any.
    data=$work/$DATA_NAME
    list_data "$data"
    bin=$(world_check_bin)
    mkdir -p "$worlds/live" "$worlds/local"
    sample() { # <into> <one in> <folder>...: the uploads, whatever the check says of them
        local into=$1 n=$2
        shift 2
        (($#)) || return 0
        status=0
        "$bin" --data "$data" --worlds "$into" --one-in "$n" "$@" >>"$work/sample.log" || status=$?
        ((status <= 1)) || die "srp-world-check could not read the corpus ($work/sample.log)"
    }
    mapfile -t folders < <(recording_folders "$corpus/live")
    sample "$worlds/live" "$one_in" "${folders[@]}"
    mapfile -t folders < <(recording_folders "$corpus/local")
    sample "$worlds/local" 1 "${folders[@]}"
    say "the sample: $(find "$worlds/live" -type f | wc -l) live upload(s) (one in $one_in), $(find "$worlds/local" -type f | wc -l) local, $(find "$corpus/dumps" -type f -name 'match026g_*.bin' 2>/dev/null | wc -l) kept local dump(s)"
    say "the host's verdicts (drift_dump) ..."
    (cd "$ROOT/srp-026g" && nix develop -c cargo run --quiet --release -p srp-validate --example drift_dump -- \
        "$worlds" "$corpus/dumps") >"$work/drift.tsv" 2>"$work/drift_dump.log" ||
        die "drift_dump failed ($work/drift_dump.log)"
    [[ -s $work/drift.tsv ]] || die "drift_dump gave no world"
    split -n "l/$jobs" -d -a 3 "$work/drift.tsv" "$work/chunk."
    say "the game's verdicts: $(wc -l <"$work/drift.tsv") world(s) in $(find "$work" -maxdepth 1 -name 'chunk.*' | wc -l) chunk(s) ..."
    (cd "$ROOT/srp-026g" && nix develop -c nix-shell -p "python3.withPackages(ps: [ps.unicorn])" \
        --run "$(printf '%q ' bash "$here/worlds.sh" chunks "$exe" "$work"/chunk.*)") ||
        die "the drift check failed (each chunk's run: $work/run.*)"
}

# In the dev shell with Unicorn: each chunk under check_world_check_026g.py --drift, in parallel;
# every chunk's cases passed, and the two builds agree on every world.
chunks() { # <exe> <chunk>...
    local exe=$1 chunk run line failed=0 total=0 pids=()
    shift
    for chunk in "$@"; do
        (cd "$ROOT/srp-026g/client-patches" && exec python3 -u check_world_check_026g.py "$exe" --drift "$chunk") \
            >"${chunk%/*}/run.${chunk##*.}" 2>&1 &
        pids+=($!)
    done
    for chunk in "${pids[@]}"; do wait "$chunk" || true; done
    for chunk in "$@"; do
        run=${chunk%/*}/run.${chunk##*.}
        line=$(grep -E 'drift: [0-9]+ worlds' "$run" | tail -n 1 || true)
        if [[ -z $line ]] || ! grep -q '^all cases passed' "$run"; then
            say "${run##*/}: FAILED"
            grep -E 'FAIL|Error|error' "$run" | head -n 5 || tail -n 5 "$run"
            failed=$((failed + 1))
            continue
        fi
        total=$((total + $(sed -E 's/.*drift: ([0-9]+) worlds.*/\1/' <<<"$line")))
    done
    say "drift: $total world(s) and their mutated copies, the game's check and the host's agree; $failed of $# chunk(s) failed"
    ((failed == 0))
}

case ${1:-} in
check) check ;;
drift) drift ;;
chunks) shift; chunks "$@" ;;
*) die "usage: worlds.sh check|drift" ;;
esac
