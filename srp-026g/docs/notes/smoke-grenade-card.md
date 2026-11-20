# The smoke grenade's card: how long, how big (open)

The owner, 2026-11-10: "i wish smokes showed info about smokes too", "how long it stays", "how big
the radius is", "we for some reason don't have that"; the data must flow from the game's files,
not be typed into the lobby ("we might play with values there"). On what was read below: "suspicious,
need testing in game, we can do grenades later". Nothing is built yet. This note is where to go on.

## What was read (2026-11-10)

- **The item has neither number.** `gameplay/items/weapons/smoke_grenade.options` (the live one,
  `server-game-data.db` of a revision 55 test build): `data/` holds the throw and the fuse
  (`ttl_ms` 4000, `throw_power` 17, `min_roll_time_ms` 400), every splash value 0, and
  `particle_explosion` = `fx/explosions/smoke_grenade`. So the card has no line because the config
  has no key: the smoke is the effect.
- **The effect** `particles/fx/explosions/smoke_grenade.particle` (stock `resources.db`, 60592
  bytes, CRC c1728948; a Vostok binary config, `content_edit.walk` reads it). Root `TotalLength`
  700. Three emitters under `LOD 0`:

  | emitter | `Duration` | particle `LifeTime` | size | spawn domain |
  |---|---|---|---|---|
  | `smoke` | 30 | 29 | `InitSize` 3.62 to 6.57 | `Cylinder` RadiusOuter 8, Height 2 |
  | `smoke_centr` | 30 | 25 | `InitSize` 3.06 to 5.92 | (a domain node; its values 1) |
  | `flash` | 30 | 0.1 | `SizeLife` 0.54 to 3.96 | a light, `Radius` 20 |

- **Read as seconds and metres** that is smoke fed for 30 s, the last puffs gone about a minute
  after the burst, over a disc of about 8 m plus half a puff: about 10 m.

## Not known (why this is open)

- The units: `Duration` 30 against `TotalLength` 700 and `ColorLife`/`SizeLife` `Duration` 400 do
  not read as one unit. Nobody timed a smoke in a match.
- Which `DomainType` an emitter uses: the file lists every shape's values (Box, Sphere, Cylinder,
  Disc, Cone, Blob); the selected one was not read.
- Whether the puffs move (velocity nodes were not read), so whether the cloud grows past its
  spawn disc.
- Whether bots or anything in the simulation reads a smoke's size (LIKELY not: the item's splash
  values are 0 and the effect is the client's), so whether a retune is visual only.

## To do, in this order

1. Time and measure a smoke in a match (a replay will do): from the burst to the last puff, and
   its width against something of known size. Settle the units and the domain with it.
2. Choose where the numbers live:
   - `throwables.tsv` gets the smoke's time and radius, and the content build writes them into
     the effect file and the card (the owner can then tune them in one place); or
   - the card only: the content build reads the effect file as it is and writes the card's two
     lines.
3. The card: two UI properties after 170 (`throwable-cards-1.json` is the pattern: time in
   seconds, radius in metres), on the smoke grenade only; texts in every
   `client-patches/localization/*.tsv`. A grenade's card lists only its own stats
   (`one-pocket-a-type.md`), so the RGD-5's card gets no smoke rows.
