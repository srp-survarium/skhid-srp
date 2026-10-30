# Players put into a loaded world (patch 82)

- A client that loads a world from the server (a late join, or a resync) puts
  every player that is alive in it into the world, as that player's spawn did
  on the other clients: on its minimap and Tab map, with its 3D icon, its
  physics and its place in the rule's living list.
- Before (since late join, patch revision 5): a late joiner's map lacked every
  ally who had not died since it joined (seen live in revision 38, match
  0x107a2189; the owner: "icons of teammates disappeared on map").
- A dead player, or one already in the world, is left as it is.
- It changes what the joiner computes (its living list and physics): a
  release's revision, older replays refused.
- Code and proof: client_patch.md, "Players put into a loaded world (patch 82)".
