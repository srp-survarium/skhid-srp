# Artefact score

The owner (2026-10-29): an artefact picked up from an anomaly gives 15 points of match score when it
goes into an empty artefact slot, 5 when every artefact slot is full.

- **Why** (the owner): "this forces people to share them with others through the feature we
  implemented". A player whose slots are full gets 5 for the next one; handing an artefact to a
  teammate (carry and share, #751) frees a slot, and the anomaly's next artefact gives 15.
- **When it is decided:** at the moment of the anomaly pickup, by the user's artefact slots then.
  Nothing scores the hand-over itself.
- **Only an anomaly's artefacts** (`artefact_container_core::use_execute`). An artefact a player
  dropped, carried or handed over scores nothing when it is picked up (#751's pickups go through
  `inventory::take_artefact` directly, not the anomaly's use; and the anomaly can be used only
  while its artefact is in state 2, `can_use` 0x31C6EC, never #751's dropped state 6).
- **A pickup with every slot full** is what the game did already, unchanged: allowed, and the new
  artefact takes the first slot's place (`take_artefact` 0x2FC897), the one there leaves the
  inventory.
- **Every mode**, Research too.
- **How:** the game counts event 5 for every anomaly pickup; exe patch 80 counts event 16 too
  when a slot is empty (0.26g counts 16 nowhere else). Weights: event 5 5, event 16 10
  (`static_info::ARTEFACT_SCORE`, `ARTEFACT_SCORE_SLOTS_FULL`). The scoreboard and the pay's other
  score both follow; was 2 an artefact.
- **Results' artefact count** (explorer): event 5, every anomaly pickup, as before.
