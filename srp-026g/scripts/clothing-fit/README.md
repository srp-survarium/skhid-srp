# Clothing fit checks

`release/build-content` runs `check_boot_classes.py` on the stock database
and final composed content, after every edit. A medium/tall boot must reach
the native shaft-height range of the trousers class it selects (2 cm
allowance for uneven cuffs). This prevents the original exoskeleton
`data_long` mistake from recurring on another imported boot. It reads the
actual config and geometry; no item ids or height table are maintained.

For extracted resources, trees are layered in command order. Supply the
stock configs **and their referenced models** under `stock/`:

```sh
python check_boot_classes.py --native stock --tree published \
  --tree packs/stk2-bandit-14/files --tree packs/stk2-novice-13/files \
  --tree packs/stk2-loner-13/files --out boot-classes.json
python -m unittest discover -s . -v
```

The regression fixture rejects a 22 cm boot selecting `data_long`, then
accepts the corrected `data_full` configuration, including an edit applied
on top of a pack in the release builder.

`render_pairs.py` renders the *selected* garment variant, including its
additional surfaces, with an accessory. Requires numpy, Pillow and
resource-porter's `scripts/render-duty-026-icons.py`. `pairs.json` is an
array of `[garment_config_path, accessory_config_path]` pairs. Use trousers
with boots, or jackets with gloves. It draws three views per pair; garment
and accessory have different solid colours so textures cannot conceal gaps.

```sh
python render_pairs.py --tree published --tree rebuilt/loner/overlay \
  --pairs pairs.json --renderer /path/to/resource-porter/scripts/render-duty-026-icons.py \
  --out pairs.png
```

Add `--pose /path/to/pose-stk2-outfits-026 --db /path/to/stock/resources.db
--clip ak/player/crouch/3rd_view/on_site_idle` to use a native animation
pose. The pose helper must include the 44-byte native three-bone layout
support added with the shared fitter correction.

The 2026-11-02 checks cover all three new trousers with low, medium, tall
and extra-tall boots, and all four jackets with wrist gloves and both
gauntlet classes, from front, side and back. All three new boots and both
new gloves were also rendered with native trousers/jackets (five pairs,
three views each). The class audit covers all 37
available boots, including both later-build equipment packs. The porter
has five cross-section regressions plus a three-bone skin-layout test and
the two existing outfit tests. An in-game fitting pass is still required
before release; these tools do not claim to verify every animated frame.
