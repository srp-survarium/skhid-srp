# A reload left after the magazine is in (patch 205, edit list reload-commit-1)

The owner (2026-11-26): "quite often you put the ammo magazine into the weapon, then do a melee hit
and the ammo is reset to 0. This is badly designed." A reload never loses ammunition because it was
left: before the new magazine is in, the gun keeps what it had; after, the new magazine counts.

## What 0.26g does (PROVEN in the code)

- **One load, at the clip's end.** A magazine reload is one of three state classes (config
  `states/reload/state_type` 303 `weapon_core_reload_state`, 314 `pistol_weapon_core_reload_state`,
  311 `double_barreled_weapon_core_reload_state`), each in a plain vtable (0xA14D14, 0xA151FC,
  0xA15088) and a sound handler's (`weapon_sound_events_handler_state<...>`: 0xA04DEC, 0xA051CC,
  0xA050D4), all six with slot 9 = `weapon_core_reload_state_base::on_animation_end_impl`
  (0x310E40). That is the only place the reload loads: `weapon_core::load_magazine` (0x2E17D0), a
  top-up: `take = min(stack, capacity +5E4h - loaded +5E5h)` from the retained ammunition stack
  (+538h, its count +260h) into the magazine, then, for a gun that chambers on reload
  (`chamber_a_round_on_reload`, +5F2h: the VSSK, the TOZ-122), one into the chamber (+5F1h); then
  the recoil's reset (0x332140) and `m_need_to_auto_reload` (+5F5h) 0. The same in 0.21d
  (`load_magazine` 0x1E2DC6 from `on_animation_end_impl` 0x1F34E8, PDB names).
- **The end is the clip's end.** The state's initialize (0x30DA60, then 0x30FDB0 / 0x250F30 /
  0x252F90) subscribes it to its clip's end (channel `"\x01"` of the user's animation player +750h);
  the callback 0x30DB60 calls slot 9 when the clip it waits for ends and marks the state ended
  (+12Dh), which the state machine's transitions read.
- **Taken out early: nothing.** The initialize takes nothing out of the gun (a chamber-on-reload gun's
  chambered round goes back into the magazine, 0x30FDD2); the finalize (0x30E420, the sound
  handlers' 0x251030 / 0x253090 jump to it) unsubscribes and touches no ammunition. So a reload left
  before its end (any transition out of the reload state: a melee, a sprint, a weapon switch, a
  grenade) leaves the magazine as it was, and loads nothing, however far the clip went.
- **So "reset to 0".** An empty reload (the common one) left after the magazine is seen in the gun
  leaves the gun empty: the HUD's 0 (weapon::draw 0x2450E5 shows loaded + chambered). A tactical
  reload left so keeps its old rounds; the new magazine seen is not counted. No round is lost either
  way: what was not loaded is still in the stack. No Skhid patch or pack caused it: no client patch
  touches the reload's ammunition, and the tactical clips (`tactical-clips-029`, `-069`,
  `-rebased-069`) and the retimed clips (`reload-timings-1`) only move the clip's end.
- **Death** does not leave the state (weapon_core::on_player_death 0x2E3390 wears the condition and
  clears +5F5h); the respawn's unload_ammo (0x2E18E0) gives the stack the magazine and the chamber.
- **Which guns:** every gun of state 303, 311 or 314, whichever clips: 91 of the 103 live weapon
  configs (78 of 314, 8 of 303: the PKP, RPD, Fort-401, Minimi, Magnum, TOZ-122, VSSK and its camo; 5
  of 311: the TOZ-34, TOZ-66 and the three IZh-27s), the 0.29a3/0.69d tactical clips, the 0.69
  remodels and the stock clips alike, empty and tactical reloads alike. Not the 12 of state 306
  (the pump and bolt guns: `weapon_core_shotgun_reload_state`, a round at each sub-clip's end,
  `reload_one_round` 0x2E1A00), whose interrupted reload keeps every round already in.

## What 0.69d0 does (PROVEN in the code, `tools/re-kit/v069d`)

It loads at an event of the reload's item clip. `weapon_core_reload_state_base` (0x20038E340)
subscribes `on_user_may_shoot_event` (0x20038EBB0, its diagnostic's name) to the item clip's
`shoot` channel; for the state's weapon and clip, domain 1 loads the magazine (0x2006D6960: its
load_magazine 0x2006D6C50, the recoil, `m_need_to_auto_reload` 0, 0.26g's 0x310E40 body) and sets
the state's loaded byte (+268h), domain 0 lets the user shoot (weapon +44CEh); the end
(0x20038EB70) loads only if that byte is 0. The byte is serialized (0x20038ECC0). Its clips put
domain 1 at the seat of a tactical reload (AK-12: `ak_12_mag_set` at 73 of 120, the event at 74)
and after the bolt of an empty one (the event at 114 of 159, the bolt's `shutter_in` at 104).
Many live clips carry those channels already (the 0.69d ports'); 0.26g reads none of them in a
reload.

## The change

- **The event (content, `reload-commit-1.json`, `content_edit.py reload_commit`).** Every item reload
  clip of a 303, 311 or 314 reload gets a channel `reload_commit` (type 2, one knot, domain 1) at its
  seat: the last of its own sound events (decoded as the game plays them: domain 255 the list's next
  sound from 0FFh as a byte, 0x297D11; another its index) whose first-view sound is the new
  ammunition going in: a magazine in or set (`mag_in`, `mag_set`, `mag_slide_in`, `mag_in_set`,
  `mag_set_in`, `mag_in_rnd`, `mag_power_set`, not their `_ready`), a belt laid (`belt_set`,
  `belt_on`: the PKP, Minimi, RPD, Fort-401), a shell loaded (`load_1`, `load_2`: the double barrels),
  a revolver's shells or a stripper's rounds in (`shells_in`, `bullets_in`: the Magnum, the SKS). 154
  clips of 91 guns, every one with a seat; a clip read by two guns at two seats, or one without a seat
  sound, stops the build. It runs after `reload-timings-1` and reads the clips as that leaves them (a
  retune moves the sounds and so the seat). The tracks stay byte for byte. Examples (frames):
  AK-74M tactical 45.7 of 72, empty 46.4 of 102; AK-12 51.1 of 84 (0.69d 51.8), 51.1 of 111.3;
  pistols' tactical `mag_set` 50 of 65, empty `mag_power_set` 73 of 125; PKP `belt_set` 250 of 479;
  TOZ-34 both barrels `load_2` 87 of 110. **The seat, not 0.69d's empty reloads' post-bolt point**:
  the owner's rule ("after it is seated: the new magazine counts"); an empty reload left after its
  seat counts the magazine without its bolt (0.26g chambers nothing for these guns).
- **The load (exe patch 205, `rust/patches/src/reload_commit.rs`, `vostok/reload.rs`).** Five hooks:
  - `reload.listen` 0x30DB0F (the animation-end-aware initialize, once it subscribed the clip's end)
    and `reload.listen_again` 0x30DC7C (its deserialize, a world's state): a state whose slot 9 is
    0x310E40 is subscribed to its user's `reload_commit` channel, the state the key, as the game
    subscribes it to its end (a functor of the manager 0xA14CBC, the patch's entry its function);
  - `reload.stop_listening` 0x30E476 (the reload's finalize, after its own unsubscriptions):
    unsubscribed;
  - `reload.commit_event` (a runtime callback in 0x30DB60's convention): an event of the state's
    weapon or its user, domain 1: the game's own 0x310E40 runs with a done byte of ours, so the
    magazine is loaded, the recoil reset and the auto-reload cleared as at the end, and the state is
    not ended (+12Dh untouched): the reload goes on, its clip plays, and its end comes as before;
  - `reload.load_at_end` 0x310E41 (0x310E40 before its load): for a gun that chambers on reload
    with a round chambered (an earlier load of this reload chambered it), the round back into the
    magazine first, as the initialize does. Then a second load is the first's: the top-up takes
    nothing more, the chamber gets the same round. Without it the end would take one more round from
    the stack and chamber it over the one there (the check sees 29 + 1 + 79: a round lost).

  A load is the same twice as once (a top-up), so the commit needs no state: nothing new in a
  world, no parser change. A world read mid-reload subscribes the reload again; its event, whether
  the clip plays it again or not, loads what is due. A clip without the channel loads at its end, as
  before.
- **Left before the seat:** the old magazine, as the game: nothing was taken out (the rounds seen
  pulled out are in the count; a chamber-on-reload gun goes to its chamber-a-round state, as before).
- **Changes the simulation:** `simulation_changes.tsv`'s `next` row.

## Checked

- `check_reload_commit_026g.py` (`tools/check reload-commit`, Unicorn): the four kinds of reload
  state (303, 311 and 314 sound handlers, the plain 314) run whole in both exes, with the game's own
  subscriptions, functors and invokers on a made-up player's channel list: the whole reload; a melee
  (the finalize) at 10, 30, 45 % (before the seat): the old magazine in both; at 55, 80, 95 %: the
  original's old magazine (the bug), the patched exe's new one, no round made or lost; empty,
  tactical, a short stack, no stack; a second seat and the end take nothing more; another object's
  or domain's event nothing; death's unload after the seat; a world read mid-reload; the VSSK-like
  chamber-on-reload gun. Without the end-load hook it fails (a round lost).
- `test_reload_commit.py` (`tools/check content-edit`): the seat rule, the index decoding, belts,
  shells, strippers, no seat, the channel added with the tracks kept.
- The edit list run over players' files (stock db and patches 12..73): 154 clips of 103 configs, 0
  unexpected differences.
- **Not run in a game.** To test: a local match; empty and tactical reloads of a rifle (AK-74M), a
  pistol, the PKP and a double barrel, each left by a melee just after the magazine goes in (the
  `mag_in`/`mag_set` sound) and just before; the count after each.
