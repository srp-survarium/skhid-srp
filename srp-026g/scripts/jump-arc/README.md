# jump-arc

The jump clips' root tables of `servers/match-server/src/v026g/bots/jump_arc.rs` (A318, `kin.jump_arc`): the client's
jump clips evaluated frame by frame (0..24 at 30 a second) as clamped cubic B-splines.

- `clip_table.py <anim dir>`: a summary per clip (apex, travel); `--rust` prints the rows.
- `make_jump_arc.py <anim dir> <out jump_arc.rs>`: writes the whole module (code, tables, tests).

The inputs are the stock `resources.db`'s rifle jump clips, `animations/single/weapons/ak/player/stand/3rd_view/jump/`
(`on_site`, `on_site_fwd`, `on_site_fwd_right`, `on_site_right`, `on_site_back_right`, `on_site_back`,
`on_site_back_left`, `on_site_left`, `on_site_fwd_left`, `sprint/sprint_from_left_leg`,
`sprint/sprint_right_from_left_leg`, `sprint/sprint_left_from_left_leg`), taken out flattened as
`animations__single__...__jump__<name>.b-spline` with `dbgrab <resources.db> get animations/single/weapons/ak/player/stand/3rd_view/jump/ <dir>`
(`tools/re-kit/common/dbgrab`). No row of `release/content.tsv` changes them.

Run with `python -I`; the module is written with LF line ends.
