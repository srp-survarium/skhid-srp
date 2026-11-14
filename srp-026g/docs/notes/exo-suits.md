# Exoskeleton suits' bonuses

The Duty and Freedom exoskeleton suits (STK2 port, `duty_exo_026` / `freedom_exo_026`, torso
configs; worn in the back slot they cover the six others). Their built-in bonuses are their
`default_modifications`: the client adds them from the config, and the server counts them as held,
so a suit never rolls a kind it comes with (`Modifications::candidates`).

The owner's rules (2026-10-29; the bonus cells of content-edit `armor.tsv`, once `exo-suits-buff-1.json`, after
`built-in-bonuses-1.json`, docs/notes/built-in-bonuses.md, which already sets Freedom's movement
and energy costs to their second-strongest rows):

- Freedom: movement speed +4 % (1113), energy costs -8 % (1163), stamina regeneration +8 % (1103);
  its item slot, ammo bag and artefact container stay.
  - Buffed 2026-11-14 (the owner: "buff it", then "still weaker", against the level-15 vest,
    backpack and trousers a suit replaces: 26 kg, 88 armour, three item slots, four ammo bags,
    pocket capacity +3, nine rolls to the suit's three): the kinds' top rows, movement +5 % (1114),
    energy costs -10 % (1164), stamina regeneration +10 % (1104); two item slots (1031), two ammo
    bags (1051), pocket capacity +2 (1171); armour 0.88 (was 0.85); the suit 17 kg (was 26.2), its
    helmet 3.5 kg (was 4.5). Isolation unchanged. Its armour is now above Duty's 0.87.
  - The rolls (the owner: "my equipment can have 9 rolls, exo only 3"): the three pieces roll nine
    bonuses, the suit three, and its one armour roll covers what takes the pieces two. The other
    five are built in: the three mobility rows above, and weight limit +5 kg (1124) and
    regeneration +15 % (1094), Duty's own two. A suit that had rolled one of those kinds keeps the
    roll beside the built-in one (only new rolls leave out a kind the suit comes with).
- Duty: weight limit +5 kg (1124), regeneration +15 % (1094), the kinds' top rows, beside its item slot, ammo bag, pocket capacity and
  artefact container.
  - Buffed 2026-11-14 (the owner, after the Freedom suit's buff: "make sure duty is fine too, it
    can get more ammo I guess"), against the same level-15 vest, backpack and trousers: two item
    slots (1031), two ammo bags (1051), pocket capacity +2 (1171), as the Freedom suit; the suit
    22 kg (was 26.4). Where the Freedom suit has its three mobility rows, Duty has three gun rows,
    the kinds' top rows: aim time -10 % (1084), reload speed +15 % (1144), recoil -10 % (1154). So
    Freedom is the lighter suit that runs, Duty the heavier one (5 kg more) that carries the same
    and handles its gun better. Armour stays 0.87 (the owner: no armour change), one point under
    the Freedom suit's 0.88; isolation, the artefact container and the helmet (5 kg) unchanged.
- Suits already owned get the new bonuses at once: an item's record holds only its own
  modifications, the defaults come from the config.
- **Eight built-ins at most** (2026-11-14): the client holds eight default modifications an item
  (`item_params::MOST_DEFAULT_MODIFICATIONS`, the item's constructor 0x2CB310) and stops at start on a
  ninth; the two buffs above made nine each. Both suits lose regeneration +15 % (1094) (the owner's
  pick), eight each. srp and `srp-check-game-data` refuse an item with more.
