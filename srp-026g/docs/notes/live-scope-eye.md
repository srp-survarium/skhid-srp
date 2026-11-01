# Live scope variants: the reticle at the screen's centre (2026-11-01)

The owner's "properly port all weapons and their attachments". Pack `live-scope-eye`, edit list
`live-scope-eye-1`, made by `scripts/live-scope-eye/make_pack.py`; before/after views of every variant
by `review.py`. Context: [weapon_scopes.md](weapon_scopes.md).

- **The fault (PROVEN in the game, 2026-11-01):** a scope's view (`hide_weapon_on_aim`) is its aimed
  model drawn at the weapon model's scope_point, seen from its `aim` locator. Where that locator is
  turned about x the view is tilted: the AK family's 0.004 rad put the rifle scopes' TA01 reticle 10 px
  and the Leupold 3-7's 16 px above the centre, where the bullets go. Patch 19 set its scopes'
  eyepieces along the bore: on the A-545 it allowed for the turn, on the Ash-12 most of it, on the
  AK family and the Saiga not.
- **The fix:** each such variant's aimed model again, moved onto the tilted line of sight at the
  reticle's distance (`models/weapons/attach/fit6/<aimed model>_<weapon model>`), and its config
  naming it (`replace_string` of `data/aimed_model`). Nothing else changes: not the weapon, its
  model, sights or hip view, nor the scope's outside look. 15 variant configs (ACOG TA31, PSO-1 and
  ELCAN on the AK-74M, AK-74N, AKMN and its two premiums; ACOG TA31 and ELCAN on the Ash-12; PSO-1
  on the Saiga-12 and its premium). The PSO-1's chevron keeps the 0.4 mm drop below the line patch 19
  gave it; only the turn is added.
- **Left as they are:** the A-91's, SR-3M's, Val's, VSS's, Vepr's and RPD's scope variants. Their
  reticles sit 1 to 2 cm off the aim locator in the files (by this reading hundreds of pixels off the
  centre, which players would have reported), so the game must show them otherwise than this
  reading; the A-545's are on the tilted line already. To be seen in the game before any change.
- **Not affected:** collimators (the weapon's own sights stay in view) and the rifle scopes, whose
  scope variants set a model with an unturned eye (#516 @2).
