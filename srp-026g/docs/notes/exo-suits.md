# Exoskeleton suits' bonuses

The Duty and Freedom exoskeleton suits (STK2 port, `duty_exo_026` / `freedom_exo_026`, torso
configs; worn in the back slot they cover the six others). Their built-in bonuses are their
`default_modifications`: the client adds them from the config, and the server counts them as held,
so a suit never rolls a kind it comes with (`Modifications::candidates`).

The owner's rules (2026-10-29, content-edit `exo-suits-buff-1.json`):

- Freedom: movement speed +4 % (1113), energy costs -8 % (1163), stamina regeneration +8 % (1103);
  its item slot, ammo bag and artefact container stay.
- Duty: weight limit +5 kg (1124), regeneration +15 % (1094), beside its item slot, ammo bag,
  pocket capacity and artefact container.
- Suits already owned get the new bonuses at once: an item's record holds only its own
  modifications, the defaults come from the config.
