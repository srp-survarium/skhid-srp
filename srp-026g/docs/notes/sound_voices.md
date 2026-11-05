# Shots stay audible under heavy fire (patch 96)

The owner (2026-11-05): "too many bullets shooting break sound, and the sound disappears".

- **What players get:** under sustained automatic fire (several players, full-auto shotguns, MGs)
  new shots, impacts and steps keep sounding; the oldest, quietest tails are cut first.
- **The cause** (client_patch's README, patch 96): the game world's sound scene has 512 sound
  instances and 196 propagators, the world 64 voices; each shot layer is one propagator for its
  length plus its way to the listener, and the ported guns carry 9.2 to 9.9 s tails
  (`rudnya_outdoor*`), the AKs a 4.2 s echo. Full pools drop new sounds (no propagator) or keep
  them silent (no voice) until the tails end.
- **Pools:** 1024 sound instances and 1024 propagators in the game world's scene. Voices stay 64
  (XAudio2's mono and stereo pools are 64 each).
- **Voices on demand:** a sound refused a voice takes the voice of the quietest playing sound of
  its scene when that one is under half as loud now (gain by distance x the sample's amplitude at
  its offset). The taken one is let go as the engine's own HDR cull does it.
- **Unchanged:** the content (the tails stay as ported); the music (its own scene); what a match
  computes (no `simulation_changes.tsv` row).
- **If tails still crowd:** the content fix is the ported composites' tail layer (a distance
  window like the stock `_m` composites', or a shorter sample), the owner's ears to decide.
