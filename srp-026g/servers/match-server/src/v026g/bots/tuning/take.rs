//! How a bot takes a battery from the floor (`plays/collect.rs`, the Take): A281 (a battery under a tree root on Mamayev Kurgan, 7 failed
//! takes in the owner's match 0x24ddea92) and A282 (Collect plays ended "no way to its goal" at two zone batteries, 84 in two matches).
//! Each part is off by default (the old take) until its by-side bench decides, but `pitch` (view scale) and `drop_impossible` (the owner's
//! correctness fixes); `traces-20261028/fx-zones-a281-blocked-pickup.md` and
//! `fx-zones-a282-no-way-collect.md` have the traces.

use super::*;

/// A use is pressed at a stand only when the latest world puts the body there (A281 (a)): the reckoning had bots on the ground
/// slot beside the battery while the client had lifted them 1 m onto the root, and they pressed 19 times from up there (PROVEN,
/// the recording). The world's feet must be within `across_m` across and `up_m` up or down of the stand, in a world no older
/// than the arrival; a world that says otherwise gives the stand up (the body cannot hold it); none yet waits up to `wait_ms`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldStand {
    pub across_m: Metres,
    pub up_m: Metres,
    pub wait_ms: u32,
}

impl Default for WorldStand {
    fn default() -> Self {
        // Across: the take's own "arrived" tolerance (`AT_STAND_M` twice); up: a stand is reached within 0.4 m up or down.
        Self { across_m: Metres::of(0.5), up_m: Metres::of(0.4), wait_ms: 1_500 }
    }
}

/// What a team remembers of its failed takes (A281 (b), A282): the stands its bots gave up are the team's, not one bot's, for
/// `keep_s`; a battery whose takes failed `aside_after` plays in a row with no untried stand left, or a play that ended "no way
/// to its goal", is set aside for the whole team for `aside_s` (lifted when the battery moves). A teammate who says "can't get
/// it from there" is told knowledge (SESSION-RULES rule 4). Off: each bot remembers its own stands 90 s and sets the battery
/// aside alone (20 s after "no way", none after "could not take it" past the first 2 s); its mates take it again at once.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TeamMemory {
    pub keep_s: Positive,
    pub aside_after: u8,
    pub aside_s: Positive,
}

impl Default for TeamMemory {
    fn default() -> Self {
        // keep: the bot's own `FAILED_KEEP_MS` 90 s; aside: the commander's team "no way" keep, 60 s (`TEAM_NO_WAY_KEEP_MS`).
        Self { keep_s: Positive::of(90.0), aside_after: 2, aside_s: Positive::of(60.0) }
    }
}

/// Stands farther round a battery (A281 (c)): the ring's radii 0.45-1.2 m gain these, with the eye at most `use_m` from the
/// battery's point (the client's ray is 1.80 m). Live humans take from 1.72 m across (median, 280 tries, p10 0.77 m) looking
/// 15 degrees down; the near ring looks steeper and meets more of what lies over a battery. The nearest stand still comes first.
/// With `pitch` by the view's scale (the default) the 1.2 m stands pass and these are the same stands (within the ring's
/// 0.35 m `SAME_STAND_M`): they add nothing until the ring keeps one stand per radius (PROVEN by the far-stands test).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FarStands {
    pub radii_m: [Metres; 2],
    pub use_m: Metres,
}

impl Default for FarStands {
    fn default() -> Self {
        // 1.45 m: a crouched eye (1.04 m) is 1.73 m from a floor battery's point there, under the client's 1.80 m ray; at 1.5 m
        // it is 1.77 m, with no room for a body stopped a little off its stand.
        Self { radii_m: [Metres::of(1.35), Metres::of(1.45)], use_m: Metres::of(1.78) }
    }
}

/// The steepest look down a take's stand is judged by (A281 (d)). `ViewScale` is the default and the all-off arm keeps it
/// (`KEPT_RIGHT`): the old constants assume a view that stops at 1 rad, while the client's stops at 67.48 degrees down
/// (`bots/pitch.rs`, `PitchScale::Client`, #833; PROVEN), so they judged stands by a stop the game does not have. The owner,
/// 2026-11-03 (quiz): "ON as a correctness fix". Its by-side bench, 672 fresh pairs (seeds 8-35, 8 battery maps x 6/8/10,
/// tier 9-10): level, score -0.4 % z -0.5, wins 501 vs 501, deliveries -0.1 %, no guard FAIL; short use presses -66 % z -6.1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakePitch {
    /// The constants set when the view was taken to stop at 1 rad (a ring stand 0.95 rad, where it stopped 0.98).
    Fixed,
    /// From the view's real stop, `kin.pitch_scale`'s (67.48 degrees down for the client's): the same margins under it.
    #[default]
    ViewScale,
}

/// A crossing searched again from a zone floor (A282 (e)): a body that stepped onto a zone's floors by its crossing's edge
/// searched with the plain search (which keeps off the zone) and, the regions saying a plain way is there, no crossing's search
/// (`crossing::wanted`): no route, the goal "no way" (PROVEN, probe: 16-32 such floors by each edge). With this, a failed plain
/// search from a floor zones on all match mark asks the crossing's search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CrossFromZone {}

/// A battery the team believes lies where no floor can hold it is forgotten, and the team looks where it last lay on a floor (A296
/// item 1, the owner's match 0x33e42d77): battery 2 fell through the floor where its carrier died (251-255 s, the HUD following it
/// down to y -91.5) and the team kept it "known" there for 570 s, every bot refusing it "no stand to take it from" and none
/// looking for it (PROVEN, the log). A spot is impossible when it lies more than `below_m` under every floor of the grid within
/// `box_m` across of it, or no floor is there at all (off the level). The look (`LookSpot`) is where the HUD last showed it on a
/// floor: the game puts a battery that fell back near there (battery 1 of the same match, 6 s later, 2.4 m off).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DropImpossible {
    pub below_m: Metres,
    pub box_m: Metres,
}

impl Default for DropImpossible {
    fn default() -> Self {
        // 5 m: deeper than any step, drop or pit floor the grid misses under a lying battery; 4 m: a grid cell's error and a body.
        Self { below_m: Metres::of(5.0), box_m: Metres::of(4.0) }
    }
}

/// A spawner whose battery could not be taken (in a damage zone) is still searched (A296 item 6, A298 cause 1): the spawner search
/// skipped every such spot, so no bot went near, and a team never learned a battery lay there (Vostok Radar Station's 0, 3 and 4 in
/// the gas all match, the trailing team one point behind for the last 4 minutes with three free batteries; PROVEN, A298). With
/// this, the search walks to a dry floor within `within_m` of the spot that sees it in a clear line (`memo::Memo::zone_look`) and
/// looks at it from there: a battery seen is known (the sight rule, `sense.rs`, 35 m), and the take decides as for any known one
/// (its crossing, the zone's dose against the bot's health and armour).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoneLook {
    pub within_m: Metres,
}

impl Default for ZoneLook {
    fn default() -> Self {
        // 24 m: well inside the 35 m battery sight, so a look from the spot's floor sees it at the noticing delay of a near one.
        Self { within_m: Metres::of(24.0) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TakeTuning {
    pub world_stand: Option<WorldStand>,
    pub team_memory: Option<TeamMemory>,
    pub far_stands: Option<FarStands>,
    pub pitch: TakePitch,
    pub cross_from_zone: Option<CrossFromZone>,
    /// ON by default and kept in the all-off arm (`KEPT_RIGHT`): believing a battery lies where no floor holds it is proven wrong
    /// (A298: Mamayev Kurgan's battery 2 lay 91 m under the map from 255 s to the end, and the team kept it "known" there). The
    /// owner, 2026-11-04 (quiz): "Yes, turn it ON", a belief-correctness fix like `pitch`. Its by-side bench was identical (168
    /// pairs: the bench world drops no battery under a floor).
    pub drop_impossible: Option<DropImpossible>,
    pub zone_look: Option<ZoneLook>,
}

impl Default for TakeTuning {
    fn default() -> Self {
        Self {
            world_stand: None,
            team_memory: None,
            far_stands: None,
            pitch: TakePitch::default(),
            cross_from_zone: None,
            drop_impossible: Some(DropImpossible::default()),
            zone_look: None,
        }
    }
}

impl TakeTuning {
    pub(super) fn check(&self) -> Result<(), String> {
        if let Some(memory) = self.team_memory
            && memory.aside_after == 0
        {
            return Err("take.team_memory.aside_after".into());
        }
        if let Some(far) = self.far_stands
            && (far.use_m.get() > 1.8 || far.radii_m.iter().any(|r| r.get() > far.use_m.get()))
        {
            return Err("take.far_stands".into());
        }
        Ok(())
    }
}
