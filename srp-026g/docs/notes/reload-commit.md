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

## The clips stay the reload's (the fix after rev74)

The owner, 2026-11-26, playing rev74: "there is problem w/reload animation right now, it is broken
when weapon shoots all bullets and does full reload". PROVEN in the code and under Unicorn; not run
in a game.

- **A two-clip reload picks its clips from the rounds now, at every selection.** The player's
  animation is selected again (`base_player::select_animations` 0x2D9690, the state's slot 8 through
  `weapon_core::selected_animations` 0x2E10B0, then the animation player's set_target) whenever
  tick_animations (0x2D8EE0) finds m_need_to_select_animations (+33Eh) set: an input that changes
  the actions (set_input 0x2D899D: a move key, an aim, a crouch), the aim's tick (weapon_core::tick
  0x2E0A59), the weapon's logic (0x2E0B6E), the player's states. The weapon's clips of state 314
  (`get_weapon_lexeme` 0x30FE90) are the empty ones for `weapon+5E5h == 0`, the hands' (slot 8
  0x30FDF0) the empty set +168h the same way, else the tactical +164h; state 311's
  (0x30F5A0) the two shells' for `+5E5h != 1`, its hands (0x30F4F0) for that and a stack (retained
  +538h, entry +22Ah) whose count (+260h) is not 1. State 303 (0x30E1F0) picks by the posture alone.
  The weapon pick is written to m_index_of_animation_to_wait (+134h, `(pick*8 + state+144h -
  [+130h]) >> 2`, 0x30FEE4, 0x30F5FA): the handle whose end the end callback 0x30DB60 waits for.
- **The seat changed the rounds mid-clip.** In the stock game nothing does, so the picks never
  changed within a reload. After the seat, an empty 314 reload's next selection picked the tactical
  clips (Unicorn: loaded 0 gives +134h 2 and hands +168h, 30 gives 0 and +164h): the animation
  player started them as new clips, and the end callback waited for the tactical clip's end. A
  player who moved or aimed after the magazine went in saw the reload again from its start as a
  tactical one (the magazine out and in, its sounds), no bolt, longer (AK-74M: the seat at 46.4 of
  102 frames, then the tactical 72); one who stood still saw it whole. A tactical reload kept its
  clips (rounds before and after). A 311 one-shell reload went on as a two-shell one; a two-shell
  one with a stack of 3 got the one shell's hands on the two shells' gun, with a stack of 1 the one
  shell's gun.
- **The fix: the first pick.** `reload.pick_empty` 0x30FEC9 (EBX the pick, before +134h is
  written), `reload.pick_empty_hands` 0x30FE24 (EAX, after), `reload.pick_two_shells` 0x30F5D7
  (ECX, before), `reload.pick_two_shells_hands` 0x30F571 (ESI, after), EDI the state each: once
  +134h names a clip set of the table (not 0FFh), the pick is that one. +134h is 0FFh from the
  reload's initialize (0x30DA60) and its finalize (0x30E476); the first selection, at the reload's
  start, writes the game's own pick from the live counts; a world carries it (written 0x30DC90, read
  0x30DC7C), so a late joiner keeps the pick the others play. No state of the patch's, nothing new in
  a world.
- **311's hands, without a state.** The game's first hands pick has one exception to the weapon's:
  an empty gun with one shell left, the two shells' gun and the one shell's hands. With the two
  shells' clips the gun held none at the start (a reload starts only below capacity, 0x2E3E01), so
  now: 0 rounds, before the seat, the game's own pick (the counts are the start's); 1, the stack's
  last shell loaded, the one shell's hands; 2, the two shells'. With the one shell's clips, the one
  shell's hands.
- **0.69d0 keeps its pick** in a byte of the two-clip states (+2D8h, +2E0h): its expressions
  0x2003904A0 and 0x200390B60 index by it, its finalize sets it to 0FFh (0x2002123A0, 0x200212A40),
  its deserialize reads it (0x200390440, 0x200390AB0).

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
- The two-clip picks (`check_reload_commit_026g.py`, the same check): slot 8's weapon and hands
  picks run in the game's code before and after the seat, aimed and not, in the plain and the
  sound-handler classes of 314 and 311: 314 empty and tactical; 311 one shell, two shells with
  stacks of 10, 3 and 1; a world read before and after the seat; the first pick the original's, and
  the end of its clip ends the reload. Without the four pick hooks the empty, one-shell, stack-3,
  stack-1 and world cases fail (the tactical and the stack of 10 are the same either way).
- **Not run in a game.** To test: a local match; empty and tactical reloads of a rifle (AK-74M), a
  pistol, the PKP and a double barrel, each left by a melee just after the magazine goes in (the
  `mag_in`/`mag_set` sound) and just before; the count after each.
