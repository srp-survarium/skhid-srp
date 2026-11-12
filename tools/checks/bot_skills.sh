#!/usr/bin/env bash
# bot_skills.sh: tools/check's bot-skills-live: the bots' builds (survarium bot_skills, A372) resolved against the players'
# skills tree of this tree's server game data (release/build-content's quick build of the next patch, or the offered-patch
# list's when the manifest ships nothing new, as worlds.sh check takes it): every tier's set is one the skills window allows,
# with the carry path, the ammo bag and the medical block at their ranks
# (`on_the_live_data_every_build_is_a_set_the_tree_allows`), and the bots' kits stay under their carry
# (`on_the_live_data_no_rusher_kit_is_over_its_capacity`).
# Run by tools/check: $CHECK_WORK is its folder.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../../release/lib.sh
. "$here/../../release/lib.sh"
work=${CHECK_WORK:?run by tools/check}

# The server game data of the stock db and the offered-patch list's patches (cached), as worlds.sh's.
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

say "the server game data of this tree (release/build-content, a quick build) ..."
"$here/build_content.sh" >"$work/build-content.log" 2>&1 ||
    { tail -n 20 "$work/build-content.log" >&2; die "build-content failed ($work/build-content.log)"; }
data=$work/content/$DATA_NAME
if [[ ! -f $data ]]; then
    say "the manifest ships no new patch: the list's server game data"
    data=$work/$DATA_NAME
    list_data "$data"
fi
say "the bots' builds against $data ..."
cd "$ROOT/srp-026g"
SRP_GAME_DATA=$data nix develop . --command cargo test --release -p survarium --lib -- --ignored --nocapture \
    bot_skills::tests::on_the_live_data 2>&1 | tee "$work/bot-skills.log" | grep -E '^A372 build|^test |test result' || true
grep -q 'test result: ok. 2 passed' "$work/bot-skills.log" || die "the bots' builds on the live tree: $work/bot-skills.log"
