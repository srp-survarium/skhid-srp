# Scarcity

The owner (2026-11-02): "everyone starts each life with only the ammo in the gun's magazine plus one
extra magazine"; ammo is scarce, so a dead player's gun (patch 85) matters. On the crates (the
owner, the same day): "in scarcity, no refill crates are available".

The owner (2026-11-07): "let's make it even more scarce where everyone gets a single mag and that's
it": no gun gets a magazine in reserve any more (before: one more a gun, two more a shotgun, none
for a machine gun). And: "let's also make it in lobby to be either blackout or scarcity": the
vote's third card is Blackout or Scarcity at random (before: always Blackout, `blackout.md`).

## Spec

- **Its own mode in the lobby:** "Protective device: scarcity" (Захисний пристрій: дефіцит, Защитное
  устройство: дефицит, Schutzeinrichtung: Knappheit, ...: as each language writes Blackout's),
  lobby mode 7: a tick of the rooms' create window, a card of the vote, a row of the room and match
  lists, its tag in Blackout's grey. Listed after Blackout: plain, chargers, blackout, scarcity,
  then team deathmatch and Research.
- **Its own maps, as Blackout's:** each playable battery map once more as Scarcity, in the match
  pool and the rooms' pool (the create window's defaults, every room Play makes); a room's maker may
  untick it. Never drawn for a match of no room (a room's mode only, by its card). Not offered
  against bots (Play vs Bots lists its own maps).
- **The vote's third card** (the owner, 2026-11-07): Blackout or Scarcity, one of the two at random,
  then one of that variant's maps at random, on a map cards 1 and 2 are not while it has one. Cards 1
  and 2 are never either. A pool with only one of the two: card 3 is that one. A pool with neither
  (a private room's maker unticked both) or of variants alone: its three cards drawn as before.
- **Rules:** the plain battery mode (mode 2: batteries from the spawners, no chargers), on its maps;
  the battery rules unchanged. Points and pay as a plain match (not Blackout's twice).
- **A life's ammo, each weapon slot (7 and 10)** (the owner, 2026-11-07): its gun's loaded magazine
  (`parameters/magazine_capacity` of the gun's config) and nothing more: no magazine in reserve, for
  rifles, SMGs, bolt-actions, pistols, shotguns and machine guns alike. Every life, the first and
  each respawn.
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
  the whole match: not drawn, no collision, no prompt, no refill (exe patch 119 takes them out of the
  map's project as the game loads it). Copies of the battery projects without their crates were
  rejected (the owner, 2026-11-02): copies could drift from the maps they copy and desync later, and
  they add about 49 MB to the download. Scarcity's maps copy the configs only, not the projects.
- **A picked-up gun** (patch 85, the owner, 2026-11-07: as today): as in any match, the donor's
  magazine and one magazine in reserve; no crate refills it. The exe's pickup code is not changed.
- **Pay and autobuy:** a match takes off the stacks what it used, the rounds it gave less what is
  left, and the autobuy buys that back; never the kit's full counts.
- **Late joiners and replays** get the same kits and the same crates: the kits are those of the static
  info, the crates follow its variant entry.
- **A simulation change** (`simulation_changes.tsv`, the `next` row): the release that ships it
  refuses older games (a game without patch 119 would have the crates in a Scarcity match and leave
  its world; one without the content has no maps 12 to 41).
- **Every Scarcity map is offered:** the rooms' pool holds 256 entries (`lobby::rooms::POOL_BITS`:
  16 bits requests of 16 entries, the wire's, the lobby movie's `RoomsWire` and the server's drafts),
  160 until then. Revision 51's data has 112 playable entries and 30 plain battery maps: 172 with
  Blackout's and Scarcity's.
- **Mode introduction:** none yet (`mode_introductions.md`: Blackout's alone; Scarcity's is left for
  a PR of its own, with its text).
- **Open for the owner:** the name.

## How

- **Its own maps (content):** `scripts/content-edit/scarcity-mode-1.json`, as Blackout's
  `blackout-mode-1.json`: each playable battery config (mode 2) once more under
  `gameplay/game_modes/scarcity/`, with `mode/srp_variant` "scarcity" and the name
  `st_srp_mode_scarcity`, and a `maps_dict` row each: ids 12 to 41, in the order of the battery ids
  (the first free run of 30: Blackout's 219 to 248 leave no 30 ids under 256 above them, and the
  client looks a map id up in `maps_dict` one by one, `map_descriptor_by_id` 0x21F910, so the rows'
  order does not matter). The configs name the battery maps' own projects (`<weather>_2`): the client
  loads one as the battery map it copies, crates and all, and the exe takes the crates out.
- **One mode a map (srp):** `maps::GameMode::Scarcity`, read from the config (`maps::VARIANTS`),
  lobby number 7 (`GameMode::lobby_number`, `SRP_MATCH_MODES`); `GameMode::is_variant` names
  Blackout and Scarcity, the modes the third card offers and a match of no room never gets. The
  lobby, the match server, the results and the replays know a Scarcity match by its map's id alone.
- **The vote:** `session::v026g::rooms::Rooms::draw`.
- **The ammo (the match server):** the game gives a life its kit's slot counts
  (`inventory::setup_from_profile` 0x2FCB70: each ammo slot `min(the kit's count, amount_in_storage)`,
  the store losing it; a respawn puts the slots and magazines back into the store first, 0x2FD2D0),
  and the kits are the static info's. So `MatchRegistry` cuts every profile of a match on a Scarcity
  map (`survarium::v026g::scarcity::ration`) before the static info is made, a late joiner's too:
  every game, the replay and the server's own read of the snapshots (`Supplies`, against
  `Formed::profiles`) see the same counts. No client code gives ammo.
  - The lobby's computation it replaces: `process_default_relocate_item` (0x288D60) gives a weapon put
    in a kit `(bags div required_bags) x clip_size` rounds (bags `player.ammo_bags_count` of
    default.player with the kit's modifier 16, split 70/30 between the two guns): the account's kit
    keeps those counts and Scarcity leaves them (only the match's copy is cut).
  - Pay: the store keeps the account's whole stack, so `Supplies::left` is what is left of it and
    `autobuy::used` (what was owned less what is left, at most a life's count a life and the shots
    fired) charges what the cut kits fired. Nothing in the pay changed.
- **Static info:** the variant is tag 2 of srp's block (one byte: 1 Blackout, 2 Scarcity;
  `static_info::variant_byte`), written from the map's mode; read back, the entry and the map's mode
  must agree. A late joiner's keeps it. The client check of the options
  (`srp_validate::messages::options`) takes both.
- **The crates, exe patch 119** (the Rust module `patches::scarcity`, `scarcity.rs` and its hooks
  `scarcity.toml`; `client_patch.md` 3.72):
  - the options reads (`static_info.options_read` 0x27B92D, `replay.options_read` 0x1F0971; the
    chains of patches 65, 86, 83 and 88, order 4): the variant, parsed once by
    `vostok::options::variant_in` (Blackout's reader too);
  - `project.cook` (project_cooker_simple::create_game_objects' entry, 0x22C010): in a Scarcity
    match the map's client_project loses its crates before the game makes anything of it: each
    `resupply_zone` and every client object of its folder (its use geometry, its decals), every
    `item/quest/container_closed` render visual and collision object (the box: the crates' model
    alone in the game data); the lamps' visual indexes (`srp_lamps`, patch 81) follow.
  - **Why not the game data:** a crate is a `resupply_zone` of the project's `client_objects` (with its
    `team`; Rudnya's `rudnya-ammo-1` removes some), and a project is shared by every match on that map
    and weather: Scarcity plays the plain battery maps' own projects, so taking the zones out of them
    takes them out of the plain battery mode and Blackout too. Doing it by data would need a copy of
    each battery project without its zones (about 49 MB, the patches being uncompressed); every later
    edit of a battery project would have to reach its copy too. The owner rejected it (2026-11-02).
- **Checks:** `check_scarcity_026g.py` (Unicorn: the options read, then the project's cook from its
  entry on a made-up project, the original beside), the `scarcity` tests of the survarium crate (the
  counts) and its maps' and static info's tests, the match server's (the kits of a match and a late
  joiner), the session's (the third card), the lobby's (the pool and the match), the rooms' Ruffle
  harness (the create window's six modes in every language, a 200-entry pool).
