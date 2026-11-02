# Scarcity

The owner (2026-11-02): "everyone starts each life with only the ammo in the gun's magazine plus one
extra magazine"; machine guns get no extra magazine; ammo is scarce, so a dead player's gun (patch
85) matters. On the crates (the owner, the same day): "in scarcity, no refill crates are available".

## Spec

- **Its own mode in the lobby:** "Protective device: scarcity" (Захисний пристрій: дефіцит, Защитное
  устройство: дефицит, Schutzeinrichtung: Knappheit, ...: as each language writes Blackout's), lobby mode 7 (the first free one; the free-for-all, `feat/ffa-mode`, merging later, will take the next free mode):
  a tick of the rooms' create window, a card of the vote, a row of the room and match lists. Listed
  after Blackout: plain, chargers, blackout, scarcity, then team deathmatch and Research.
- **Always offered, like Blackout:** the rooms' pool holds each plain battery map of the match pool
  once more as Scarcity, in the create window's defaults and in every room Play makes; a room's
  maker may untick it. Not offered against bots (Play vs Bots lists its own maps).
- **Rules:** the plain battery mode (mode 2: batteries from the spawners, no chargers), on its maps;
  the battery rules unchanged. Points and pay as a plain match (not Blackout's twice).
- **A life's ammo, each weapon slot (7 and 10):** its gun's magazine (`parameters/magazine_capacity`
  of the gun's config) and one magazine more; a machine gun (`items_dict` category 16, the shop's
  machine guns) its magazine alone; a shotgun (category 14) its magazine and two more (the owner,
  2026-11-02: a TOZ-34 6 shells). Every life, the first and each respawn.
  - Bolt-actions as any gun, the magazine and one more (the owner, 2026-11-02: "almost always
    one-shot kills"; a Mosin 10 rounds).
  - The pistol too: its magazine and one more.
  - The chamber is inside these rounds, as the game counts a slot (a gun that chambers loads its
    magazine and a round from them).
  - A gun's ammunition in several ammo slots (two kinds of its calibre): the slots in order, each
    giving what is left of the gun's rounds, at most what the kit puts there.
  - Two guns of one calibre (PPSh-41 and TT-33): one pool, both guns' rounds.
  - Ammunition no gun of the kit fires: none (it could feed a picked-up gun).
  - Less owned than that: what is owned (the game's own `min(the kit's count, what is in store)`).
  - Grenades, medicines and every other item: as the kit has them.
- **No ammo crates** (the owner, 2026-11-02, on testing a first version whose crates stayed but
  refused: "crates should be completely removed, not just become unusable"): the map has none, for
  the whole match: not drawn, no collision, no prompt, no refill (exe patch 94 takes them out of the
  map's project as the game loads it). A mode of its own by data (each battery project copied
  without its crates) was rejected (the owner, 2026-11-02): copies could drift from the maps they
  copy and desync later, and they add about 49 MB to the download.
- **A picked-up gun** (patch 85): as in any match, its magazine and one magazine in reserve; no crate
  refills it.
- **Pay and autobuy:** a match takes off the stacks what it used, the rounds it gave less what is
  left, and the autobuy buys that back; never the kit's full counts.
- **Late joiners and replays** get the same kits and the same crates: the kits are those of the static
  info, the crates follow its variant.
- **A simulation change** (`simulation_changes.tsv`): the release that ships it refuses older games
  (a game without patch 94 would have the crates in a Scarcity match and leave its world).
- **Open for the owner:** the name.
- **Every Scarcity map is offered** (the owner, 2026-11-02: raise the pool's limit): the rooms'
  pool holds 256 entries (`lobby::rooms::POOL_BITS`: 16 bits requests of 16 entries, the wire's
  and the lobby movie's `RoomsWire`, the server's drafts), 160 until then. Revision 41's data has
  112 playable entries and 30 plain battery maps: 172 with Blackout's and Scarcity's. A game of an
  older revision is refused anyway (one client version).

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
  `scarcity.toml`; `client_patch.md` 3.62):
  - the options reads (`static_info.options_read` 0x27B92D, `replay.options_read` 0x1F0971; the
    chains of patches 65, 86, 83 and 88, order 4): the variant, parsed once by
    `vostok::options::variant_in` (Blackout's reader since);
  - `project.cook` (project_cooker_simple::create_game_objects' entry, 0x22C010): in a Scarcity
    match the map's client_project loses its crates before the game makes anything of it: each
    `resupply_zone` and every client object of its folder (its use geometry, its decals), every
    `item/quest/container_closed` render visual and collision object (the box: the crates' model
    alone in the game data); the lamps' visual indexes (`srp_lamps`, patch 81) follow.
  - **Why not the game data:** a crate is a `resupply_zone` of the project's `client_objects` (with its
    `team`; Rudnya's `rudnya-ammo-1` removes some), and a project is shared by every match on that map
    and weather: Scarcity plays the plain battery maps' own projects (`<weather>_2`), so taking the
    zones out of them takes them out of the plain battery mode and Blackout too. Doing it by data
    would need a copy of each battery project without its zones (31 battery configs' projects, about
    49 MB, the patches being uncompressed) and a `maps_dict` row and mode config for each; every later
    edit of a battery project would have to reach its copy too. The owner rejected it (2026-11-02:
    copies could drift and desync later, and the download). The patch takes the crates out of the
    project in memory, as it is loaded, for the variant the static info already carries.
- **Checks:** `check_scarcity_026g.py` (Unicorn: the options read, then the project's cook from its
  entry on a made-up project, the original beside), `selftest_026g.py`'s `test_scarcity`, the `scarcity` tests of
  the survarium crate (the counts), the match server's (the static info's kits), the lobby's (the
  pool and the match), the rooms' Ruffle harness (the create window's six modes in every language).
