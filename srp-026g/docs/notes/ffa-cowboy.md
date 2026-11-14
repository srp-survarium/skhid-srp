# Free for all: Saloon, everyone with a Winchester and a Colt Python, or a Colt Python alone

The owner (2026-11-14): a version of Free for All (`ffa.md`) "where everyone has colt Python", a twist of it (`ffa.md`, "Twists": 10 % of the free-for-all cards, "Saloon" in the lobby), and "saloon should have two variations with winchesters and colts or just with colts".
srp rewrites each player's match profile before the static info is sent
(`survarium::v026g::cowboy::arm`); every game cooks each life from those profiles, so every client,
a late joiner and a replay see the same kits.

## Rules

- **Two variations** (`cowboy::Variation`), drawn with the card, half and half (my pick: 5 % each
  of the free-for-all cards, `twist::SHARES`); one name ("FFA: Saloon") and one tick in the create
  window for both. A match plays one for every kit:
  - **Winchester and Colt:** the Winchester 1894 in slot 7 and the Colt Python in slot 10;
  - **the Colt alone:** the Colt Python in slot 7, slot 10 emptied (everyone has the same one gun,
    and a second Python would only save a reload).
- **The guns:** the dictionary items named `winchester_1894` (Winchester 1894, a sniper rifle by
  category, 13) and `magnum` (Colt Python), looked up by name. srp refuses to start on game data
  without either of them, its states, its fit for its slots (the Winchester 7, the Colt 7 and 10),
  its ammunition or that ammunition's clip, or the Colt's category's amount (`cowboy::check`).
- **A kit that carries one weapon** (Deep Pockets or Supply Pocket, `disable_second_weapon`) has
  the Winchester alone in the Winchester and Colt variation.
- **Every kit**, loaded or not, and a kit with nothing in it too, gets the guns. A kit's state
  stays: a kit that was not loaded is still not loaded.
- **Ammunition:** each gun's first ammunition of the dictionary it takes (`compatible`) that fits its
  ammo slots: `ammo_.30-30` for the Winchester (slot 8), `ammo_.357m` for the Colt (slot 8 alone,
  slot 11 beside the Winchester). Every other ammo slot (9, 12, 19..22, and 11 or 8) is emptied.
- **Rounds:**
  - the Colt alone: the dictionary's `optimal_count` of its category (pistols, 17): 40, two clips
    of 20, in slot 8;
  - Winchester and Colt: a clip for every ammo bag of the kit, as Racket gives its AK-12's
    (`ffa.md`, "Twists"; the kit's bags as the lobby counts them: the player's 3, its worn items'
    `ammo_bags_modifier` and its skills'), the Winchester taking the first and every other one
    and the Colt the rest (my pick): with 3 bags 40 rounds of `.30-30` (two clips of 20) and 20 of
    `.357` (one); with Supply Pocket the Winchester has them all (60). A gun with no clip has its
    magazine alone.
- **Everything else stays:** armour, pockets (medicine, grenades, boosters, traps), artefacts, the
  kit's skills (modifiers), its name and id. The pockets stay packed.
- **Items:** each gun and each ammunition given is one new item in the profile's table, held by
  every kit that has it: whole (`FULL_CONDITION`; an ammunition's store is the largest kit's rounds
  of it 1000 times, `twist::STORE_LIVES`, so that every life has its rounds, `ffa.md` "Twists"), no
  modifications, no upgrades. Their ids are the lowest ones from 1 that the profile's items did not
  have, the Colt's and its ammunition's first. An item no kit holds any more (the old guns and
  ammunition) leaves the table, so a profile never grows past the 69 items a profile holds.
