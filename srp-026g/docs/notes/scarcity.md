# Scarcity

The owner (2026-11-02): "everyone starts each life with only the ammo in the gun's magazine plus one
extra magazine"; machine guns get no extra magazine; ammo is scarce, so a dead player's gun (patch
85) matters. On the crates (the owner, the same day): "in scarcity, no refill crates are available".

## Spec

- **Its own mode in the lobby:** "Protective device: scarcity" (Захисний пристрій: дефіцит, Защитное
  устройство: дефицит, Schutzeinrichtung: Knappheit, ...: as each language writes Blackout's), mode 7:
  a tick of the rooms' create window, a card of the vote, a row of the room and match lists. Listed
  after Blackout: plain, chargers, blackout, scarcity, then team deathmatch and Research.
- **Always offered, like Blackout:** the rooms' pool holds each plain battery map of the match pool
  once more as Scarcity, in the create window's defaults and in every room Play makes; a room's
  maker may untick it. Not offered against bots (Play vs Bots lists its own maps).
- **Rules:** the plain battery mode (mode 2: batteries from the spawners, no chargers), on its maps;
  the battery rules unchanged. Points and pay as a plain match (not Blackout's twice).
- **A life's ammo, each weapon slot (7 and 10):** its gun's magazine (`parameters/magazine_capacity`
  of the gun's config) and one magazine more; a machine gun (`items_dict` category 16, the shop's
  machine guns) its magazine alone. Every life, the first and each respawn.
  - The pistol too: its magazine and one more.
  - The chamber is inside these rounds, as the game counts a slot (a gun that chambers loads its
    magazine and a round from them).
  - A gun's ammunition in several ammo slots (two kinds of its calibre): the slots in order, each
    giving what is left of the gun's rounds, at most what the kit puts there.
  - Two guns of one calibre (PPSh-41 and TT-33): one pool, both guns' rounds.
  - Ammunition no gun of the kit fires: none (it could feed a picked-up gun).
  - Less owned than that: what is owned (the game's own `min(the kit's count, what is in store)`).
  - Grenades, medicines and every other item: as the kit has them.
- **No ammo crates** (the owner, 2026-11-02): no crate can be used by anyone, for the whole match;
  the hud shows no crate prompt. The crates' models stay where the map has them.
- **A picked-up gun** (patch 85): as in any match, its magazine and one magazine in reserve; no crate
  refills it.
- **Pay and autobuy:** a match takes off the stacks what it used, the rounds it gave less what is
  left, and the autobuy buys that back; never the kit's full counts.
- **Late joiners and replays** get the same kits and the same crates: the kits are those of the static
  info, the crates follow its variant.
- **A simulation change** (`simulation_changes.tsv`): the release that ships it refuses older games
  (a game without patch 94 would refill at the crates in a Scarcity match and leave its world).
- **Open for the owner:** the name; whether a shotgun's or a bolt-action's "magazine" (2 shells of a
  double barrel, 5 rounds of a Mosin) is meant as their whole ammo for a life.
- **Open: the rooms' pool holds 160 entries at most** (`lobby::rooms::POOL_BITS`, 8 chunks of 20
  bits; the bits' request ids end just below the create's, so more chunks is a wire change). The
  game data of revision 41 and main has 112 playable entries and 30 plain battery maps: 142 with
  Blackout, 172 with Scarcity, so with every map in the pool (no `SRP_MATCH_MAPS`) `room_entries`
  keeps the first 160 and the last 12 Scarcity maps (by id) are not offered. Either more pool bits
  or fewer variant maps; the owner's call.

## How

- **Lobby (srp):** `maps::Variant` (Plain, Blackout, Scarcity) is the one value a battery match's
  variant is. The variants' maps are named by ids of the lobby's own (`maps::variant_pairs`: ids
  `maps_dict` does not use, from 255 down, Blackout's first so its ids are those it had before
  Scarcity, then Scarcity's), modes 6 and 7 in the rooms' pool, cards and rows. A room's match on
  such a card is made on the battery map with `MatchSetup::variant`.
- **The ammo (the match server):** the game gives a life its kit's slot counts
  (`inventory::setup_from_profile` 0x2FCB70: each ammo slot `min(the kit's count, amount_in_storage)`,
  the store losing it; a respawn puts the slots and magazines back into the store first, 0x2FD2D0),
  and the kits are the static info's. So `MatchRegistry` cuts every profile of a Scarcity match
  (`survarium::v026g::scarcity::ration`) before the static info is made, a late joiner's too:
  every game, the replay and the server's own read of the snapshots (`Supplies`, against
  `Formed::profiles`) see the same counts. No client code gives ammo.
  - The lobby's computation it replaces: `process_default_relocate_item` (0x288D60) gives a weapon put
    in a kit `(bags div required_bags) x clip_size` rounds (bags `player.ammo_bags_count` of
    default.player with the kit's modifier 16, split 70/30 between the two guns): the account's kit
    keeps those counts and Scarcity leaves them (only the match's copy is cut).
  - Pay: the store keeps the account's whole stack, so `Supplies::left` is what is left of it and
    `autobuy::used` (what was owned less what is left, at most a life's count a life and the shots
    fired) charges what the cut kits fired. Nothing in the pay changed.
- **Static info:** the variant is tag 2 of srp's block (one byte: 1 Blackout, 2 Scarcity), on battery
  maps only (`Variant::on_mode`); a late joiner's keeps it.
- **The crates, exe patch 94** (the Rust module `patches::scarcity`, `scarcity.rs` and its hooks
  `scarcity.toml`; `client_patch.md` 3.61):
  - the options reads (`static_info.options_read` 0x27B92D, `replay.options_read` 0x1F0971; the
    chains of patches 65, 86, 83 and 88, order 4): the variant, parsed once by
    `vostok::options::variant_in` (Blackout's reader since);
  - `resupply_zone.can_use` (0x2F8080) and `resupply_zone.use_hint` (0x2F80C0), the starts of
    resupply_zone_core's two answers: in a Scarcity match every zone answers as another team's does
    to the game (the zone's team +54h is not the player's +45ECh, nor 2): can_use 0 (no use starts:
    nothing refills) and use_hint the empty key 0x9DB708 (no prompt).
  - **Why not the game data:** a crate is a `resupply_zone` of the project's `client_objects` (with its
    `team`; Rudnya's `rudnya-ammo-1` removes some), and a project is shared by every match on that map
    and weather: Scarcity plays the plain battery maps' own projects (`<weather>_2`), so taking the
    zones out of them takes them out of the plain battery mode and Blackout too. Doing it by data
    would need a copy of each battery project without its zones (25 projects of about 700 kB) and a
    `maps_dict` row and mode config for each, read by every per-map table of the lobby and the hud.
    The patch answers as the game's own rule for another team's crate, for the variant the static info
    already carries.
- **Checks:** `check_scarcity_026g.py` (Unicorn: the options read, then can_use and use_hint from
  their starts, the original beside), `selftest_026g.py`'s `test_scarcity`, the `scarcity` tests of
  the survarium crate (the counts), the match server's (the static info's kits), the lobby's (the
  pool and the match), the rooms' Ruffle harness (the create window's six modes in every language).
