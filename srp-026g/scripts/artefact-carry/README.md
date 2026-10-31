# Artefact carry assets and HUD check

The source is `pose.json`: each arm and finger bone's lowered and raised
(open palm) rotation over the pinned stock battery idle clip. No clip is
shipped; `compile_pose.py` generates the runtime arm/finger curves
(`client-patches/rust/patches/src/artefact_carry/curves.rs`) from it, and the
exe blends them and adds the hand's breathing (`pose.rs`). `preview.py` draws
an offline joint preview, not a game screenshot.

```sh
python scripts/artefact-carry/compile_pose.py --clip battery_idle.b-spline --names battery_idle.bones_names
python scripts/artefact-carry/compile_pose.py --clip battery_idle.b-spline --names battery_idle.bones_names --check
scripts/artefact-carry/check.sh /path/to/built/hud.swf /tmp/carry-ruffle
```

The Ruffle check loads the actual HUD movie, invokes its localized take
prompt and checks target loss, chat and kit-selector visibility. It uses
an isolated Xvfb display and CPU rendering. It does not open the game.

The native integration check is `client-patches/check_artefact_carry_026g.py`.
See `docs/notes/held_artefact_transfer.md` for gameplay controls, ownership,
snapshot compatibility and the remaining in-game visual verification.

The open right-palm curves also drive damage collision bones. The artefact
hovers above the palm; Larkspur uses the separate flower-only model built by
artefact-carry-flower-1.json, while the stock ground model keeps its stem.

## Skinned mesh preview (no game windows)

`render_mesh.py` reads the stock glove, sleeve and artefact geometry, applies
inverse-bind skinning, and executes the candidate's installed pose hooks in
Unicorn on the full stock rig. It compares the damage output with render
bones while deliberately leaving unused collision bones uncomputed. The
camera is the stock animation's `Item` bone, which the native
`computed_head_transform` uses. NumPy, Pillow and Unicorn are required.

```sh
python scripts/artefact-carry/render_mesh.py --db /path/to/resources.db \
  --porter /path/to/vostok-port --exe /tmp/artefact-carry-cupped.exe \
  --out /tmp/artefact-carry-preview
```

The output `framing.png` covers all three artefacts at vertical FOV 50, 75
and 100. `pitch.png` keeps the source torso fixed and changes the eye pitch
by -40/0/+40 degrees as a reach stress case. `teammate.png` shows front
and side views, including the relaxed finger curl. It is a shaded geometry study, without engine materials or dynamic
locomotion. This does not replace the owner's in-game appearance check.
