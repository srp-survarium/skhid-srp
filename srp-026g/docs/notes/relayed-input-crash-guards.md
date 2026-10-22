# Relayed inputs that kill every game (exe patches 50 and 51)

The rules. The code and the evidence: `client-patches/README.md` 50 and 51, `client_patch.md` 3.47 and 3.48, `match.md` sections 20 and 23.

- Every game of a match applies every player's input as the server relays it. An input that crashes one game crashes all of them, the sender's too.
- Owner, 2026-10-22: guard the two known crashes in the exe, since the server cannot tell those inputs apart.
- Patch 50, the weapon keys (0xEB/0xEC relays, action bit 26):
  - "previous weapon" from a hand that holds something other than the two weapons takes no slot;
  - from slot 10 it still takes slot 7, and from slot 7 slot 10; bit 25 is unchanged;
  - the original read slot 47 of 23 and called through whatever lay past the inventory.
- Patch 51, the kit choice (0xF3 relays):
  - a choice is taken only for a kit whose player has a weapon in slot 7 or slot 10;
  - any other choice leaves the next kit as it was, as a choice for a kit the place has no player for always did;
  - the server still sends unplayable kits as `KIT_NOT_LOADED` (`play.rs::match_profile`), and a place's first kit is still the server's to keep playable.
- Both are in every full build, with no row in `simulation_changes.tsv`: a match plays out otherwise only where the original crashed.
- They protect a match only once every game in it has them: they ship with the next client release.
