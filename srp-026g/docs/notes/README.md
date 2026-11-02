# Client 0.26g: the notes

**Version scope:** client **0.26g** (build string "0.26g0", build 2777,
compiled 2015-01-14, `resources.db` 2015-01-17) compared with **0.21d**
(build 2010, April 2014). Every address in these notes is for the build it
names. 0.26g has **no PDB**, and neither does any public build after 0.21d
(see `E:\sur-res-pro\RE-TOOLKIT.md`, "Symbols by build"). 0.26g functions are
named by their assert and `__FUNCSIG__` strings, or by matching them to 0.21d
(`tools/re-kit/v026g/functions_matched.txt`).

All findings come from reading the two exes and the two `resources.db` files
(2026-10-12) and from the live server since. Running the server:
`../SRP-026G.md`.
Each note tags its findings PROVEN (read in the code or data at the address
given), LIKELY or GUESS.

## The notes

| Note | Covers |
|---|---|
| [login_chat.md](login_chat.md) | Command line, login over TLS, the browser server (removed), presence, chat transport and layouts |
| [lobby.md](lobby.md) | Lobby transport, every server→client message, **crash hazards and the safe push order** (§3) |
| [lobby_requests.md](lobby_requests.md) | Every client→server lobby request and the reply it expects |
| [social.md](social.md) | Chat channels, friends, squads, clans, online stats |
| [squads.md](squads.md) | Squads of two (the owner, 2026-10-31): Play and Join take the mate along into one room and team, or the queue together; invites stop at two; the "ready" the game's Play gating reads; each pair marked in the rooms' lists |
| [premium.md](premium.md) | Premium time, the shop's offers and bundles, items for gold, the daily bonus, the tutorial, a nickname change, autobuy and auto-repair, the level notice; their settings |
| [results.md](results.md) | Match results screen, player details, drops, daily bonus, craft tables |
| [drop_cards.md](drop_cards.md) | Drop cards: one per player of a match, own matches only, finished (the owner, 2026-10-20; the server refusal ships, the greyed button does not), the original screen's rule, what `allow_drop_card` is, the logging of sales and exchanges |
| [progression_pace.md](progression_pace.md) | How long levels, faction ranks and gear tiers take, measured on the players' server, and the pay rates chosen from it |
| [items.md](items.md) | Modifications, defects, cosmetics, reroll, craft, repair: the data, the wire, the rules |
| [weapon_scopes.md](weapon_scopes.md) | Weapon scopes: how the client builds one, scope items bought once and put on any number of weapons (variants per weapon from the game data), what a release still needs |
| [weapon-camo.md](weapon-camo.md) | Camouflage on weapons: which weapons and where the paint goes (premium weapons' masks), slot 2 beside the scope, the engine's bake and exe patch 44, the lobby slot still to come |
| [weapon-kit.md](weapon-kit.md) | A weapon's six upgrades (two decals, camouflage, scope, muzzle device, grip) in the client's three records: the kit record's layout, client patch 68, the server's packing |
| [decals-069d.md](decals-069d.md) | The 0.69d decals (34 flags, Herbs, Clover ornament, Horseshoe, Jarilo) and the Steam camouflage: ids 1824-1862, prices, icon cells, the pack |
| [match.md](match.md) | Match protocol: lockstep, handshake, message ids, snapshot and hash, time sync, input rules |
| [world_frame_check.md](world_frame_check.md) | A world no game could load (its frame against the game's fixed buffers): refused by the match server and by the game itself (exe patch 87), one shared crate (`world-frame`) |
| [transport.md](transport.md) | The UDP transport that matches **and now the lobby** use: header, acks, per-id lengths, multipart |
| [static_info.md](static_info.md) | Match start (0xE4): match options, profiles with kits, byte layout |
| [kd-balance.md](kd-balance.md) | The sides of a match by K/D at the match's equipment level (the owner, 2026-11-01): squads 1.3 times, a small noise, late joiners, what a match counts |
| [matchmaking.md](matchmaking.md) | The player's level as the client computes it, the search screen, srp's queue rules (sides of 8, level range, match level, least-played map) |
| [modes_maps.md](modes_maps.md) | Maps, game modes, map ids, rule classes, match settings now in data |
| [data.md](data.md) | `resources.db`: items, skills, drops, quests, item configs |
| [client_patch.md](client_patch.md) | Porting the srp client patch, the function map |
| [rust_patches.md](rust_patches.md) | Writing an exe patch in Rust: the workspace, the hook ABI, the engine bindings, typed game memory, panics and the log, testing, the equivalence gate a port must pass, the size budget |
| [replay_sur.md](replay_sur.md) | The client's own replay file (`.sur`, `-match_replay`): its layout, and `srp-recording-to-sur`, which makes one from a match recording |
| [replay_watch.md](replay_watch.md) | "Watch replay" from the lobby's History, the server side: conversion at match end, the lobby's `R1` answers and `9E` link, the download tokens, `/replays/<token>`, what is and is not reachable over HTTP |
| [password-reset.md](password-reset.md) | The admin's password reset while srp runs: the page's Password tab (confirm step), a script's one POST and its answers, the curl from the jump host, the owner's rule (2026-10-18) |
| [account-moderation.md](account-moderation.md) | The admin's Money and Take items tabs (the owner, 2026-10-20): the account's live silver, gold and items changed while srp runs, preview and confirm, never below 0, kits kept valid, the audit trail `g26_admin_actions` |
| [admin-chat-mute.md](admin-chat-mute.md) | The admin's chat mute (2026-11-01): an account's lines on every chat channel reach nobody, for some hours or for good; it is told `muted for <reason>` instead; kept with the account, each mute in `g26_admin_actions` |
| [launcher.md](launcher.md) | The game's launcher (`survarium_launcher.exe`): how it starts, its registry values, the pipe to its updater and every message on it, PLAY, the page, the buttons, the language. What was built from it: ../../client-patches/launcher/README.md |
| [fall_damage.md](fall_damage.md) | Fall damage: the formula, the values (fatal_damage 1.0 as in 0.27, the owner, 2026-10-19), the lethal heights stock and now; how speed counts (vertical only; what a jump or a steep slope adds) |
| [battery_pickup.md](battery_pickup.md) | The battery pick-up 20 % faster: take_time 0.6 -> 0.5 (the owner, 2026-10-20), why the animation plays faster and is not cut, what the release needs |
| [battery_respawn.md](battery_respawn.md) | Battery modes: respawn 10 s (the owner, 2026-10-21): respawn_time 8 -> 10 in the configs of modes 2 and 3 (Team Deathmatch keeps 8), where the exe reads it, the modifiers on top, the HUD countdown, the simulation change |
| [bandages.md](bandages.md) | Bandages (the owner, 2026-10-24): hands and legs fully healed, pain, body and head 35 %; how a drug heals (influences per second over its activity time, medkit::tick 0x33D790), the values before and after; the lobby description and the card without "Heal amount" (2026-10-25) |
| [drug-interrupt.md](drug-interrupt.md) | A bullet stops a drug (the owner, 2026-10-31; exe patch 84, unreleased): a ranged hit that took health stops the hit player's healing drugs in use, in the use time (it never starts) or the active time (it stops, what it healed stays); the unit stays used; teammates' bullets too; not the painkiller (it heals nothing), not armour-stopped bullets, not other hit types; in the match simulation (`simulation_changes.tsv`); the stopped drug's quick slot red for a second (display only, patch 77's hook) |
| [medicines.md](medicines.md) | Medicines (the owner, 2026-10-31): the bandages, the Medikit (40 % of pain) and the Military (blue), Black Market (green) and Science (yellow) medkits from 0.27d0 and 0.46c2 (the old box), each standing on its own, all made whole from one table (`scripts/medicines/medicines.tsv`, pack `medicines`); sold by their config's faction; yellow cures intoxication and radiation |
| [v011e_spawn_protection.md](v011e_spawn_protection.md) | The v0.11 maps (Vostok Radar Station, Chemical Plant): spawn protection 6 s (the owner, 2026-10-21): invulnerability_time 2 -> 6 on their 174 respawn points, why it was 2 s, how protection works (respawn point, base zones), the simulation change |
| [grenade_bounce.md](grenade_bounce.md) | Grenade bounces louder (the owner, 2026-10-21): where the bounce sound comes from (material pairs, three grenade-only collections), its reload-click distance curve, the `mid_range` curve it gets (retuned after the owner's test of revision 27), who hears it |
| [crouch_steps.md](crouch_steps.md) | Crouched steps audible at 40 % of walking (the owner, 2026-10-21): why crouching was silent (the crouch foot materials' empty pairs, and, T27, no step events in the crouch clips), what 0.31a2 did, the copies, who hears them |
| [golden_weapons.md](golden_weapons.md) | Golden weapons (the owner, 2026-10-21): only the silver bonus, as the weapon's own first modification; upgraded, rerolled and crafted as the plain weapon, with it as the donor; the owned ones given it once |
| [premium_armour.md](premium_armour.md) | Premium armour (the owner, 2026-10-22): the golden weapons' treatment for the Bandit and Survivor jackets and trousers; their experience and reputation bonuses their own rerollable modifications (rank 2), the plain piece's default kept, the plain piece their donor; the owned ones given them once; the per-item table |
| [built-in-bonuses.md](built-in-bonuses.md) | Built-in bonuses (the owner, 2026-10-29): every equipment item's default of a rolling kind at that kind's second-strongest strength, computed from the data; weapons keep their presets; why |
| [exo-gear.md](exo-gear.md) | Exoskeleton boots and gloves (packs exo-gear, edit list exo-boots-1): the STK2 black areas painted over, the boots' lobby icons, and the boots worn under full-length trousers (data_full; the owner, 2026-10-30) |
| [exo-suits.md](exo-suits.md) | Exoskeleton suits' built-in bonuses (the owner, 2026-10-29): Freedom movement +4 %, energy costs -8 %, stamina regeneration +8 %; Duty weight limit +5 kg, regeneration +15 %; owned suits get them at once |
| [equipment-046.md](equipment-046.md) | Equipment from 0.31a2 and 0.46c2 (pack equipment-046, proposals): the owner's pick of 0.69d armour live Skhid lacks, 35 of 38 pieces (the Black Market's Heavy armored level-15 set, Veles, Sniper, OZK-UV, missing pieces of existing sets, a premium Bandit's jacket "MK II"), the donor conversions, balance, the 3 0.69d-only pieces left out, open questions |
| [backpacks-069.md](backpacks-069.md) | Backpacks from 0.69d (pack equipment-069, proposals): the Medic's bag and the Soldier's backpack, models by resource-porter's build-armour-069d (surfaces, PBR materials on native templates, the fit against the stock torsos), levels, built-in bonuses mapped, balance, what needs a look in game |
| [stk2-outfits.md](stk2-outfits.md) | The rest of the STK2 characters export (packs stk2-bandit, stk2-novice, stk2-loner; resource-porter build-stk2-outfits-026): 14 items of the bandit, Novice and loner outfits, their factions, stat donors, fits, what was skipped and why, the in-game checks left |
| [tech-debt.md](tech-debt.md) | Known shortcuts and special cases to revisit, one bullet each |
| [one-pocket-a-type.md](one-pocket-a-type.md) | One pocket an item type (the owner, 2026-10-21): a grenade, medkit or booster in at most one pocket of a kit (boosters added their bonus once a pocket); the server refuses a second, kits saved before are merged at load, the lobby lights up only the type's own pocket; the "4th grenade empties the slot" client bug |
| [presets.md](presets.md) | Kit presets (the owner, 2026-10-23): five sets of the three kits an account, one active (the kits themselves, so every kit change is its own), chosen from a dropdown in the Character panel through the kit rules (gone items leave slots empty, nothing bought or made); a room join offers the highest preset the room allows ("Enter anyway" / "Exit" or "Switch and enter") |
| [reroll-keep-old.md](reroll-keep-old.md) | Keeping the old modification after a reroll (the owner, 2026-10-21): the window's Cancel (the game's own, 2026-10-23) takes none of the three offered, the price stays paid; the offer kept until answered, one answer, no second roll meanwhile. Any modification may be rerolled (the owner, 2026-10-22): no lock on the first one changed |
| [flat-reroll.md](flat-reroll.md) | Flat modification change price (the owner, 2026-10-25): every change costs the item level's price, the first change's; no 1.1 growth per change; the window shows what is taken; old items' counts kept, no migration; server only |
| [battery_credit.md](battery_credit.md) | A player's battery events (put in, found, delivery shares, quests) paid per match, per kind, at most as many times as the map has batteries: the owner's rule against farming (2026-10-20), its catch |
| [damage_calculator.md](damage_calculator.md) | The site's damage calculator (`/wiki/calculator`, a wiki section): the owner's rules for it, the data it is built from, how it is served and kept up to date |
| [research_mode.md](research_mode.md) | Research (mode 5, capture points) as 0.27d shipped it: the rules, the data, the inventory of the builds (what is ported, what is left out and why), exe patch 53, the pack, the server's part |
| [rudnya-069d.md](rudnya-069d.md) | Rudnya (v0.69): 0.69d's level_01 as map 204 (level 14, team deathmatch, day), full-size textures, on native Rudnya's sky, probes and minimap; the pack, the ids, what a release still needs |
| [london.md](london.md) | London (level_09): no anomalies, the owner's first-floor map-editor spawns in each team's base building (content-edit `london-1.json`) |
| [maps_tab.md](maps_tab.md) | The site's maps (`/wiki/maps`, a wiki section): spawns, batteries, chargers, containers and artefacts on every map; the owner's rules, rebuilt per release |
| [spotting.md](spotting.md) | Spotting: no cooldown, a battery's body and its use box both spot it, the markers (the owner, 2026-10-19; exe patch 38) |
| [hide-hit-markers-scores.md](hide-hit-markers-scores.md) | The Settings' HUD group: crosshair list (Dynamic, Static, Hidden), hide hit markers, hide player scores, hide quests, static minimap (the owner, 2026-10-30): two Settings checkboxes kept in user.cfg, off by default, at once; the crosshair without its hit marker, the Tab list names only by name; with the Interface level, F11 and Blackout (exe patch 83) |
| [kill-feed-icons.md](kill-feed-icons.md) | The kill feed's wall, ricochet and pain icons (the owner, 2026-10-25): weapon, wall or ricochet (0.69d0's icons and rules), pain (a white lightning bolt for pain shock), cause; when each shows (exe patch 67, the hud movie) |
| [baked-textures.md](baked-textures.md) | Freezes in a match from camouflage and decal bakes (2026-10-25): why every game baked again (the stock delete of `streamed_textures`) and each player object baked the same textures again; the rules of exe patch 70 (the folder kept per revision, one bake or load for many queries) |
| [renderer.md](renderer.md) | The renderer as a light bake needs it: stage order and targets, the scene's models and lights, vertex layouts (static, four-bone, terrain), G-buffer material parameters (Fresnel and roughness, the specular scale, wetness), emission and its flicker, sun, point and spot lights, water, the sky dome and scattering tables |
| [fov_lod.md](fov_lod.md) | Models at a high field of view: why they went out sooner (LOD and screen factor by size on the screen), the scale table, the fix (exe patch 40) |
| [death-camera.md](death-camera.md) | The camera on the death screen: a held mouse button turns the view from the corpse's eyes, the wheel does nothing (the owner, 2026-10-21; exe patch 46) |
| [ragdoll.md](ragdoll.md) | **Being built (patch 76, not run in a game).** Ragdolls on death (the owner, 2026-10-25): 0.26g has no joints in its Bullet; corpses are out of the simulation's physics; option C3, a visual-only XPBD ragdoll (crate `ragdoll`) reading the level by ray_test; phase 2: the bone array, the queries, the death key (+328h), the eye (+44Ch is "Item"), the battery, the cull, the threads; the Bodycam recipe; tests and costs |
| [death-eyes.md](death-eyes.md) | The view fading on the death screen (the owner, 2026-10-26): over the respawn countdown, at its speed, the view loses brightness and colour under a narrowing vignette, three faint heartbeat dims, never fully dark, clear at once at the respawn; how the hud knows the time (the hud movie) |
| [music.md](music.md) | The music (the owner, 2026-10-28 and 2026-10-31): the main theme on the login screen, the lobby theme, the map's theme to 15 s past the match-start announcement, 2 s fades, one song at a time, its own bus at -24 LUFS; retail 0.69d's music research; exe patch 88, pack music@1, edit list music-1.json |
| [tab-map-names.md](tab-map-names.md) | The names on the Tab map (the owner, 2026-10-30): with Alt held, each other player's icon shows its name; which players, the own icon, Blackout (none: no Tab map); exe patch 82 tells the hud movie Alt |
| [required-patches.md](required-patches.md) | The game requires its release's patches (2026-10-21 crashes; exe patch 49): a release's exe stops at its start without a patch its server offers, built in from the offered-patch list by `release/build-zip`, checked by `release.sh` and `deploy` |
| [tab-cursor-friends.md](tab-cursor-friends.md) | A cursor on the Tab list (Ctrl, the owner, 2026-10-30) and "Add to friends" from its rows' menu: the keys, the menu, the checks the client and the lobby share (exe patch 86) |
| [chat-mute.md](chat-mute.md) | Mute chat (the owner, 2026-10-28 and 10-31): hide a player's lines or show them as `<muted>`, one list the server keeps, from the lobby's player menu and the Tab list's (exe patch 86) |
| [item-search.md](item-search.md) | Item search (the owner, 2026-10-22): Ctrl+F over the shop, the inventory or the craft and upgrade windows' list opens a box above the item panel's buttons; the list follows each key, by the shown name, accents and Cyrillic folded, typos tolerated; Esc, Enter and the exe's window keys |
| [transient-windows.md](transient-windows.md) | Transient windows (the owner, 2026-10-30: as an OS's or a browser's menus): History, the match settings under Play, the presets list, a dropdown's list; one at a time, closed by a press outside (which still arrives), Esc or any window opening, drawn over the windows on their own layer (`Transient`) |
| [esc.md](esc.md) | Esc in the lobby (the owner, 2026-10-23): Esc closes the topmost modal only; every modal (windows, popups, the Upgrade and rooms windows, messages, the right-click menu) registers on open and leaves on close (`Modals`); a message's Esc is its own close (Cancel, OK) |
| [max-bonus.md](max-bonus.md) | The best bonus in gold (the owner, 2026-10-22): a bonus at the strongest of the five values its type rolls shows gold, not green, everywhere bonuses are listed; the premium bonuses (silver, experience, reputation) gold too; the reroll window shows a bonus's range on hover; the movie's table checked against the data |
| [no-reliability-roll.md](no-reliability-roll.md) | No reliability rolls (the owner, 2026-10-23): upgrades, rerolls and drops never give the "Reliability" modification (wear is off since #35); owned ones stay and reroll into others; the 20 items it left with two types get a third (masks Armor, oxygen packs Weight limit and Energy costs, the Pilgrim container Energy costs; `third-mod-1.json`), so every live item rolls three or more |
| [weapon-rolls.md](weapon-rolls.md) | Weapon rolls that take effect (the owner, 2026-10-29): the recoil (2080-2084, defect 3060) and aim-time (2070-2074) modifications on the keys the game reads, the same percentages (`weapon-rolls-1.json`); the card scales its kick rows and aim-time bar, and shows the chambering time with its rolls |
| [alt-weapon-stats.md](alt-weapon-stats.md) | Weapon stats in a match (the owner, 2026-11-01): Alt held over a weapon card shows each stat as "original -> current" with the kit's armour and the skills (and the weapon's own rolls), to the decimal at which they differ; what is left out; the match's formulas by address |
| [craft-menu.md](craft-menu.md) | The craft (Upgrade) window (the owner, 2026-10-20 and 22): Buy and Sell on its list, Continue and Exit after an upgrade, and the compare card shows the item being upgraded, not the equipped one |
| [shop-character-menu.md](shop-character-menu.md) | The character menu (kits) shown on the left in the Shop tab too (the owner, 2026-10-22) |
| [shop-owned-mark.md](shop-owned-mark.md) | The shop's owned mark (the owner, 2026-11-01): a check mark on the shop entries of items the account owns, every kind, updated after each buy and sale; a "Hide owned" box (the Autobuy box, left of Buy) that leaves owned items out |
| [ui_scale.md](ui_scale.md) | The UI at high resolutions: how each Flash movie follows the screen size, what is drawn 1:1 at 4K and how small, the HUD centring fixes |
| [launcher_updater.md](launcher_updater.md) | The game's own updater (`survarium_updater.exe`), which the Skhid launcher does not use: its arguments, what it said on the pipe, what it did on the network |
| [download-mirrors.md](download-mirrors.md) | Download mirrors (the owner, 2026-11-01, after Google Drive's quota stopped new players): the game's archive and the patches from many addresses in order, srp's `/game` and `mirrors.tsv`, `release/add-mirror`, resume at the next address, the addresses built into the updater and the .bat, one log line a try |
| [armour-params.md](armour-params.md) | Armour configs: what each key does |
| [artefact-score.md](artefact-score.md) | Match score for an artefact put into a slot |
| [artefacts.md](artefacts.md) | Artefacts: patch 54 and the artefacts-1 edit list |
| [blackout.md](blackout.md) | Blackout: the mode with the UI off |
| [breakable_lamps.md](breakable_lamps.md) | Breakable lamps |
| [buy-gold.md](buy-gold.md) | "Get Gold" opens the exchange |
| [buy-modified.md](buy-modified.md) | Buy from a modified item's right-click menu |
| [death-poses.md](death-poses.md) | Death poses |
| [exchange.md](exchange.md) | The exchange window: amounts |
| [held_artefact_transfer.md](held_artefact_transfer.md) | Held artefact transfer (patch 78) |
| [hide-ui-key.md](hide-ui-key.md) | A key that hides the interface |
| [kill_cap.md](kill_cap.md) | Repeated-kill payment limit |
| [kit-change-before-start.md](kit-change-before-start.md) | A kit chosen before the match starts (patch 31) |
| [language-option.md](language-option.md) | Language in Settings |
| [lobby-keys.md](lobby-keys.md) | Keys the lobby movie takes |
| [lobby-packs-046-069.md](lobby-packs-046-069.md) | Lobby packs: the 0.46c2 hangar and the 0.69d camp |
| [lobby-packs-stk2.md](lobby-packs-stk2.md) | Lobby pack: the STK2 bomb shelter |
| [lobby-scenes.md](lobby-scenes.md) | Lobby scenes: the lobby selection window |
| [room-weather.md](room-weather.md) | The Room lobby's weather: day, rain, night |
| [lobby-source.md](lobby-source.md) | The Flash movies from source |
| [masks-pack.md](masks-pack.md) | Ported masks: the Army balaclava and five 0.69d masks |
| [match-hud-options.md](match-hud-options.md) | Match HUD options |
| [no-tutorial.md](no-tutorial.md) | No tutorial; what a new account starts with |
| [player_bots.md](player_bots.md) | Player bots driven by the server |
| [player_bots_behaviour.md](player_bots_behaviour.md) | Player bots: behaviour design |
| [player_bots_mapdata.md](player_bots_mapdata.md) | Player bots: map data (`crates/mapdata`) |
| [player_bots_research.md](player_bots_research.md) | Player bots: research |
| [player_gear.md](player_gear.md) | A player's equipment from the Rankings and the rooms' lists |
| [progression_sources.md](progression_sources.md) | Progression and economy: what the sources of the time say |
| [rooms.md](rooms.md) | Rooms |
| [serbia-flag.md](serbia-flag.md) | The Flag of Serbia decal |
| [snapshot_rules.md](snapshot_rules.md) | The snapshot's game rules and statistics blobs |
| [tab-list-left-places.md](tab-list-left-places.md) | The Tab list shows the places in the match (patches 39, 43) |
| [vegetation-runtime.md](vegetation-runtime.md) | Runtime vegetation |
| [../DEPLOY.md](../DEPLOY.md) | The Docker files for the 0.26g server beside the live 0.21d one (ports table), certificates, the players' overlay |

The client patch is in `srp-026g/client-patches/`. The data dumps
are in `tools/re-kit/v026g/data/`.
