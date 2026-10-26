//! A topic of the owner counts: a type that owns its counts, reads what it counts and gives its part of
//! the `owner` object and of the owner-test text at the end. A new topic is its file, its `mod` and `use`
//! lines and one line of [`topics`]: the tally calls every topic at the same points, in the order of the list.
//!
//! What a topic reads: the record of every body ([`Run`]); the game and its bodies; where only the bot knows
//! it, the bot itself ([`Topic::read`], a shared reference: the topic cannot change it), with a method on
//! `Bot` in the topic's file for a derived value.
//!
//! The list is a static list of constructors, not a registry of the `inventory` or `linkme` kind:
//! those register by code run before `main` or by link order and promise no order of iteration, and the topics'
//! hooks run in a fixed order on one tick (campers' kills are read once the tick's hits are in, for one).

use std::any::Any;

use super::*;

/// What a topic is given for one body at one tick, once the body is recorded.
pub(super) struct BodyAt<'a> {
    pub(super) game: &'a Game,
    pub(super) place: Place,
    pub(super) tick: u32,
    /// The record just made and the one before it.
    pub(super) rec: &'a Rec,
    pub(super) prev: &'a Rec,
    pub(super) sight: &'a mapdata::RayTree,
}

pub(super) trait Topic: Any {
    /// A topic as `Any`, for a test that reaches into one.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// What only the bot knows, for each place each tick (none: not a bot): the topic keeps what it needs.
    fn read(&mut self, _place: Place, _bot: Option<&Bot>, _tick: u32) {}

    /// The same from a test's [`BotNow`].
    #[cfg(test)]
    fn inject(&mut self, _place: Place, _now: Option<&BotNow>) {}

    /// A bot's note (takes, stuck watches, grenades thrown, ...).
    fn note(&mut self, _run: &Run, _note: &Note, _kind: &NoteKind<'_>, _game: &Game) {}

    /// Before the bodies of a tick: what the game logged this tick (hits, landings).
    fn begin(&mut self, _run: &Run, _game: &mut Game) {}

    /// One body at one tick, after it was recorded. `rays` counts the rays cast.
    fn body(&mut self, _run: &Run, _at: &BodyAt<'_>, _rays: &mut u64) {}

    /// After the bodies of a tick.
    fn end(&mut self, _run: &Run, _game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {}

    /// The topic's parts at the end, each under its name (the golden counts keep one file a name).
    fn parts(&mut self, run: &Run, game: &Game, scale: &Scale) -> Vec<(&'static str, Part)>;
}

/// The topics (`n` places), in the order of the `owner-test` lines of the bench log (the first line, `owner-test:`, is
/// the fight's: the log tools read it) and of the hooks' calls: campers last, for their kills are read once the tick's
/// hits are in.
pub(super) fn topics(n: usize) -> Vec<Box<dyn Topic>> {
    vec![
        Box::new(FightCounts::new(n)),
        Box::new(JumpCounts::default()),
        Box::new(MoveCounts::new(n)),
        Box::new(GrenadeCounts::default()),
        Box::new(StockCounts),
        Box::new(CamperCounts::default()),
    ]
}

/// What a test fabricates of a bot at a tick ([`OwnerTally::inject`]): the topics' [`Topic::inject`] take what
/// they read of it.
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(super) struct BotNow {
    pub(super) dry: bool,
    pub(super) reload: Option<(u32, u32)>,
    pub(super) jump: Option<(&'static str, Option<(Vec3, Vec3)>)>,
    pub(super) play: String,
    pub(super) step: Option<String>,
    pub(super) goal_m: Option<f32>,
    pub(super) range_held: Option<u32>,
    pub(super) grenades: GrenadeStats,
}

#[cfg(test)]
impl BotNow {
    pub(super) fn doing(&self) -> Doing {
        Doing {
            play: self.play.clone(),
            step: self.step.clone(),
            goal_m: self.goal_m,
            jump: self.jump,
        }
    }
}
