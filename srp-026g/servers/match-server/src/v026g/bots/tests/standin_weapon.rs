//! The weapon in a stand-in body's hands: its magazine, its ammo and its
//! reload, worked as the 0.26g client works them, in place of "a body fires
//! whenever its fire key is down" (the rounds were never counted, a bot
//! fired through its own reloads, and an empty weapon shot on).
//!
//! The rules are the client's weapon state graph (fighting's
//! `fact-a-reload-cut.md`, read from the exe: `weapon_core`'s transitions
//! built at 0x2DF780, PROVEN unless marked) and `bots::guns::Load` already
//! counts them for a bot's own ledger, so the same `Load` is the body's
//! truth here, driven by what reaches the client (the keys of the tick, the
//! shots that happen) and not by what the bot means:
//! - a shot needs a round in the weapon (0x2E3CB0) and the fire state of the
//!   last shot over (the shot's clip and a bolt's chamber: [`Gun::cycle_ms`]);
//! - the reload key (bit 6), or the fire key with nothing in the weapon,
//!   starts a reload when the weapon has room and the body has ammo
//!   (0x2E3D60, 0x2E3DA0), from the idle state: not while one runs;
//! - a magazine reload (states 303, 314, 311) takes its clip and moves the
//!   rounds at the end; a fire press does not cut it (PROVEN). A round by
//!   round reload (306) puts a round in at each loop end and goes on to its
//!   finish clip when the magazine is full, the ammo out, or the fire or
//!   aim key is held at a round end with a round in the weapon (0x339DE0);
//!   no shot until the finish clip is over;
//! - the ammo: a life starts with the kit's slot rounds, loaded from it
//!   (LIKELY: [`Load::of`]); the kit has only [`BOT_AMMO_LIVES`] lives' worth
//!   for the match (`Arms::reserve`, the profile's storage; a spawn takes
//!   `min(count, storage)` again: `fact-b-resupply.md` §5), so a body that
//!   fires it all goes on without.
//!
//! - the resupply box (`fact-b-resupply.md`, PROVEN; `use_update` 0x2F8120,
//!   `inventory::resupply` 0x2FDC10): a use starts on a USE edge, alive, with
//!   none running, when the one ray from the eye along the view meets the
//!   box of the body's own team within 2 m ([`Stock::resupply`]) and
//!   something is missing and the storage has it; each tick after, the ray
//!   must still meet the box and USE be held, or the use ends with nothing
//!   applied; at [`RESUPPLY_MS`] after the edge the ammo stack is topped up
//!   from the storage to the kit's count, once ([`Stock::top_up`]); the key
//!   is let go and pressed again for another. The magazine is not written (a
//!   reload loads it), health never. The storage is what the match's
//!   unfired rounds are, less those in the weapon and the stack, so the
//!   kit's total for the match is the same.
//!
//! Not modelled: a reload refused while jumping or sprinting (0x2E4140,
//! 0x2E4170: not decoded), a grenade cutting a reload (0x2E4090), a weapon
//! switch (the bots carry one weapon), the pocket stacks the box also fills
//! (grenades), a carrier's use going to his battery rather than the box
//! (a carrier does not resupply here), and a reload's rounds moving at the
//! clip's end rather than its start (the shots wait for the clip either
//! way).

use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits};

use crate::v026g::bots::guns::{Arms, Gun, Load, Reload};
use crate::v026g::drive::key;

use super::bench_settings;

/// `SRP_BENCH_STOCK=off`: the stock gates nothing (the old stand-in, whose
/// bodies fire whenever the fire key is down); it still counts.
fn gates() -> bool {
    !bench_settings().var("SRP_BENCH_STOCK").is_ok_and(|v| v == "off")
}

/// `SRP_BENCH_RESUPPLY=off`: the box does nothing (the stand-in before
/// `Stock::resupply`).
fn boxes() -> bool {
    !bench_settings().var("SRP_BENCH_RESUPPLY").is_ok_and(|v| v == "off")
}

/// How long a use of the box takes, ms: the edge tick and 60 ticks of 50 ms
/// (`now - start >= 3000`, `fact-b-resupply.md`).
pub(crate) const RESUPPLY_MS: u32 = 3_000;

/// A use of the resupply box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Using {
    Idle,
    /// Begun at this time, ms.
    Box { began: u32 },
}

/// What the weapon is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Busy {
    /// Idle, or in the fire or chamber state of the last shot (see
    /// `Stock::ready_at`).
    Free,
    /// A clip under way until this time, ms.
    Until(f32),
    /// A round by round reload's loop ([`Load::advance`]).
    Loop,
}

/// What the stock counted, for the bench.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Reloads begun.
    pub(crate) reloads: u32,
    /// Uses of the box begun, those that topped up, those that ended with
    /// nothing (the key let go or the ray off the box), and the rounds the
    /// tops-up put in the stack.
    pub(crate) box_uses: u32,
    pub(crate) box_topped: u32,
    pub(crate) box_cut: u32,
    pub(crate) box_rounds: u32,
    /// Round by round reloads the fire or aim key ended before the
    /// magazine was full or the ammo out.
    pub(crate) cut_loops: u32,
    /// Ticks a living body was out of rounds in the weapon and in the ammo
    /// it carried (its ammo for the match, not only its life's).
    pub(crate) dry_ticks: u32,
    /// Ticks the fire key was down and the weapon could not fire (out of
    /// rounds, reloading or between shots).
    pub(crate) held_no_shot: u32,
    /// Of those, the ticks with the weapon empty or reloading.
    pub(crate) held_empty_or_reloading: u32,
}

/// A body's weapon and its ammo.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stock {
    /// The weapon in hand, its rounds and its ammo carried.
    pub(crate) load: Load,
    /// Rounds of each kit's weapon the body has not fired in the match
    /// (the weapon's, the carried ammo and the storage): the first spawn's
    /// `Arms::reserve`.
    left: Vec<(&'static str, u32)>,
    busy: Busy,
    /// The earliest the weapon can fire again, ms: the last shot's fire
    /// clip, and the bolt after it.
    ready_at: f32,
    pub(crate) counts: Counts,
    /// It gates the shots ([`gates`]).
    gating: bool,
    /// The box works ([`boxes`]).
    boxed: bool,
    using: Using,
    /// The kit's count of the weapon: rounds in it and in the stack at a
    /// spawn, and what the box tops up to.
    kit: u32,
}

/// The kit of `tier` whose weapon is `item` (the bots' default equipment:
/// the rusher in its old table).
fn arms_with(tier: BotTier, item: &str) -> Option<&'static Arms> {
    let equipment = BotEquipment::of(tier, RusherKits::default());
    BotRole::ALL
        .into_iter()
        .flat_map(|role| Arms::kits_of(equipment, role))
        .find(|arms| arms.primary.0.item == item)
}

impl Stock {
    /// A body at a spawn with `gun` in hand.
    pub(crate) fn new(tier: BotTier, gun: &'static Gun) -> Self {
        let mut stock = Self {
            load: Load::of(gun, 0),
            left: Vec::new(),
            busy: Busy::Free,
            ready_at: 0.0,
            counts: Counts::default(),
            gating: gates(),
            boxed: boxes(),
            using: Using::Idle,
            kit: 0,
        };
        stock.spawn(tier, gun);
        stock
    }

    /// The rounds of the match left to the kit with `gun`.
    fn left_of(&mut self, tier: BotTier, gun: &'static Gun) -> (u32, u32) {
        let count = arms_with(tier, gun.item).map_or(gun.magazine * 6, |arms| arms.primary.1);
        let entry = match self.left.iter().position(|(item, _)| *item == gun.item) {
            Some(i) => i,
            None => {
                let reserve = arms_with(tier, gun.item)
                    .map_or(count * 6, |arms| arms.reserve);
                self.left.push((gun.item, reserve));
                self.left.len() - 1
            }
        };
        (count, entry as u32)
    }

    /// A life begins (or a weapon is taken) with `gun`: loaded from what the
    /// kit has left, whole.
    pub(crate) fn spawn(&mut self, tier: BotTier, gun: &'static Gun) {
        let (count, entry) = self.left_of(tier, gun);
        let left = self.left[entry as usize].1;
        self.load = Load::of(gun, count.min(left));
        self.kit = count;
        self.using = Using::Idle;
        self.busy = Busy::Free;
        self.ready_at = 0.0;
    }

    /// The bot has taken another weapon (a kit of its role): a new life's
    /// worth of that kit.
    pub(crate) fn switch(&mut self, tier: BotTier, gun: &'static Gun) {
        if self.load.gun.item != gun.item {
            self.spawn(tier, gun);
        }
    }

    fn left_mut(&mut self) -> &mut u32 {
        let item = self.load.gun.item;
        &mut self
            .left
            .iter_mut()
            .find(|(each, _)| *each == item)
            .expect("the weapon's kit is counted")
            .1
    }

    /// Rounds of the weapon's kit in the storage: the match's unfired ones
    /// not in the weapon or the stack.
    fn stored(&self) -> u32 {
        let item = self.load.gun.item;
        let left = self.left.iter().find(|(each, _)| *each == item).map_or(0, |e| e.1);
        left.saturating_sub(self.load.rounds + self.load.spare)
    }

    /// What the box would put in the stack now: the kit's count less what
    /// the weapon and the stack hold, as far as the storage has it.
    fn missing(&self) -> u32 {
        self.kit
            .saturating_sub(self.load.rounds + self.load.spare)
            .min(self.stored())
    }

    /// One tick of the box at `now` ms: `edge` is the USE key pressed this
    /// tick, `held` it down, `meets` the view's ray meeting the body's own
    /// box (`false` when it has none), `free` the body able to use it
    /// (alive, not carrying a battery).
    pub(crate) fn resupply(&mut self, now: u32, edge: bool, held: bool, meets: bool, free: bool) {
        if !self.boxed || !self.gating {
            return;
        }
        self.using = match self.using {
            Using::Idle if edge && free && meets && self.missing() > 0 => {
                self.counts.box_uses += 1;
                Using::Box { began: now }
            }
            Using::Idle => Using::Idle,
            Using::Box { .. } if !(free && held && meets) => {
                self.counts.box_cut += 1;
                Using::Idle
            }
            Using::Box { began } if now - began >= RESUPPLY_MS => {
                let put = self.missing();
                self.load.spare += put;
                self.counts.box_topped += 1;
                self.counts.box_rounds += put;
                Using::Idle
            }
            running => running,
        };
    }

    /// The weapon can fire at `now` ms: nothing under way, a round in it,
    /// its last shot's clips over.
    pub(crate) fn may_fire(&self, now: u32) -> bool {
        !self.gating
            || self.busy == Busy::Free && self.load.rounds > 0 && self.ready_at <= now as f32 + 1e-3
    }

    /// Why the weapon cannot fire at `now` ms (a reason code, [`REFUSALS`]): `None` when it can ([`Self::may_fire`]).
    pub(crate) fn why_not(&self, now: u32) -> Option<u8> {
        if !self.gating {
            None
        } else if self.busy != Busy::Free {
            Some(5)
        } else if self.load.rounds == 0 {
            Some(6)
        } else if self.ready_at > now as f32 + 1e-3 {
            Some(7)
        } else {
            None
        }
    }

    /// A shot at `at` ms: a round out of the weapon. `false`: it cannot
    /// fire (nothing is spent).
    pub(crate) fn fired(&mut self, at: f32) -> bool {
        if !self.gating {
            self.load.rounds = self.load.rounds.saturating_sub(1);
            return true;
        }
        if self.busy != Busy::Free || self.load.rounds == 0 || self.ready_at > at + 1e-3 {
            return false;
        }
        self.load.rounds -= 1;
        *self.left_mut() -= 1;
        self.ready_at = at + self.load.gun.shot_ms() + self.load.gun.chamber_ms as f32;
        true
    }

    /// The reload key went down at `now` ms: a reload begins if the weapon
    /// has room, there is ammo and nothing is under way.
    pub(crate) fn reload_key(&mut self, now: u32) {
        if self.busy == Busy::Free && self.ready_at <= now as f32 + 1e-3 && self.load.loadable() > 0 {
            self.begin(now);
        }
    }

    fn begin(&mut self, now: u32) {
        self.counts.reloads += 1;
        let ms = self.load.reload(now);
        self.busy = if matches!(self.load.gun.reload, Reload::Rounds { .. }) && self.load.loading() {
            Busy::Loop
        } else {
            Busy::Until(now as f32 + ms as f32)
        };
    }

    /// One tick at `now` ms with the body's `keys`: clips end, a round by
    /// round loop goes on, a fire key held on an empty weapon asks for a
    /// reload; and the counts.
    pub(crate) fn tick(&mut self, now: u32, keys: u32, alive: bool) {
        if !alive {
            return;
        }
        match self.busy {
            Busy::Until(end) if end <= now as f32 + 1e-3 => self.busy = Busy::Free,
            Busy::Loop => {
                let held = keys & (key::FIRE | key::AIM) != 0;
                if let Some(end) = self.load.advance(now, held) {
                    self.counts.cut_loops += u32::from(end.cut());
                    let finish = match self.load.gun.reload {
                        Reload::Rounds { finish_ms, .. } => finish_ms,
                        _ => 0,
                    };
                    self.busy = Busy::Until(end.ended as f32 + finish as f32);
                }
            }
            _ => {}
        }
        let fire = keys & key::FIRE != 0;
        if self.busy == Busy::Free && fire && self.load.rounds == 0 && self.ready_at <= now as f32 + 1e-3 && self.load.loadable() > 0 {
            self.begin(now);
        }
        if self.load.rounds == 0 && self.load.spare == 0 {
            self.counts.dry_ticks += 1;
        }
        if fire && !self.may_fire(now) {
            self.counts.held_no_shot += 1;
            self.counts.held_empty_or_reloading +=
                u32::from(self.busy != Busy::Free || self.load.rounds == 0);
        }
    }
}

/// The counts of every body, summed.
pub(crate) fn counts_of<'a>(stocks: impl Iterator<Item = &'a Stock>) -> Counts {
    stocks.fold(Counts::default(), |mut sum, stock| {
        sum.reloads += stock.counts.reloads;
        sum.box_uses += stock.counts.box_uses;
        sum.box_topped += stock.counts.box_topped;
        sum.box_cut += stock.counts.box_cut;
        sum.box_rounds += stock.counts.box_rounds;
        sum.cut_loops += stock.counts.cut_loops;
        sum.dry_ticks += stock.counts.dry_ticks;
        sum.held_no_shot += stock.counts.held_no_shot;
        sum.held_empty_or_reloading += stock.counts.held_empty_or_reloading;
        sum
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::gun;

    const LOW: BotTier = BotTier::Low;

    fn stock(item: &str) -> Stock {
        Stock::new(LOW, gun(item).unwrap())
    }

    /// A body starts with its weapon loaded from the kit's slot rounds, and
    /// the kit's storage is the lives' worth.
    #[test]
    fn a_body_starts_loaded_from_its_kit() {
        let stock = stock("ak_74u");
        let arms = arms_with(LOW, "ak_74u").unwrap();
        assert_eq!(stock.load.rounds, 30);
        assert_eq!(stock.load.rounds + stock.load.spare, arms.primary.1);
        assert_eq!(stock.left[0], ("ak_74u", arms.reserve));
        assert!(stock.may_fire(0));
    }

    /// Each shot takes a round and the clip's time; a semi-automatic weapon
    /// cannot fire again inside its clip, a bolt not inside its chamber.
    #[test]
    fn a_shot_takes_a_round_and_waits_its_clip() {
        let mut ak = stock("ak_74u");
        assert!(ak.fired(1_000.0));
        assert_eq!(ak.load.rounds, 29);
        // The AK-74U's clip is 60000 / 650 rpm = 92.3 ms.
        assert!(!ak.fired(1_050.0));
        assert!(ak.fired(1_093.0));
        let mut mosin = stock("rem_700");
        assert!(mosin.fired(0.0));
        let cycle = mosin.load.gun.cycle_ms() as f32;
        assert!(!mosin.fired(cycle - 50.0));
        assert!(mosin.fired(cycle + 1.0));
    }

    /// An empty weapon cannot fire; the fire key held on it starts the
    /// reload; the shots wait for the clip; the rounds are in after it.
    #[test]
    fn an_empty_weapon_reloads_on_the_fire_key_and_fires_after_the_clip() {
        let mut ak = stock("ak_74u");
        let mut t = 0.0;
        for _ in 0..30 {
            assert!(ak.fired(t));
            t += 100.0;
        }
        assert_eq!(ak.load.rounds, 0);
        assert!(!ak.fired(t));
        let now = t as u32;
        ak.tick(now, key::FIRE, true);
        assert_eq!(ak.counts.reloads, 1);
        let ms = ak.load.gun.reload_ms(0, 30);
        assert!(!ak.may_fire(now + ms - 10));
        ak.tick(now + ms - 10, key::FIRE, true);
        assert!(!ak.may_fire(now + ms - 10));
        ak.tick(now + ms + 25, key::FIRE, true);
        assert!(ak.may_fire(now + ms + 25), "{ak:?}");
        // The key held through the clip did not cut it.
        assert_eq!(ak.load.rounds, 30);
        assert!(ak.counts.held_empty_or_reloading > 0);
    }

    /// The reload key with room in the magazine: the clip, then the rounds;
    /// not with a full magazine, nor with a clip under way.
    #[test]
    fn the_reload_key_needs_room_and_an_idle_weapon() {
        let mut ak = stock("ak_74u");
        ak.reload_key(100);
        assert_eq!(ak.counts.reloads, 0, "a full magazine");
        assert!(ak.fired(200.0));
        ak.reload_key(500);
        assert_eq!(ak.counts.reloads, 1);
        ak.reload_key(600);
        assert_eq!(ak.counts.reloads, 1, "one under way");
        assert!(!ak.fired(900.0));
    }

    /// A round by round reload (the SPAS-12, 8 rounds): a round at each loop
    /// end, the finish clip after; the sights held at a round end cut it
    /// after one round (fighting's X3: 38 % of the SPAS-12 bots' reloads).
    #[test]
    fn a_shotgun_loads_round_by_round_and_the_held_aim_cuts_the_loop() {
        let spas = gun("spas_12").unwrap();
        let Reload::Rounds { start_ms, round_ms, finish_ms } = spas.reload else {
            panic!("round by round");
        };
        let run = |keys: u32| {
            let mut s = stock("spas_12");
            for i in 0..8 {
                assert!(s.fired(i as f32 * 300.0));
            }
            assert_eq!(s.load.rounds, 0);
            let t0 = 3_000;
            s.reload_key(t0);
            assert_eq!(s.counts.reloads, 1);
            let mut now = t0;
            // Tick through the whole load (8 rounds are 8 x round_ms).
            while now < t0 + start_ms + 9 * round_ms + finish_ms + 200 {
                s.tick(now, keys, true);
                now += 25;
            }
            s
        };
        let full = run(0);
        assert_eq!(full.load.rounds, 8);
        assert!(full.may_fire(3_000 + start_ms + 9 * round_ms + finish_ms + 200));
        assert_eq!(full.counts.cut_loops, 0);
        let cut = run(key::AIM);
        assert_eq!(cut.load.rounds, 1, "one round, then the finish clip");
        assert_eq!(cut.counts.cut_loops, 1);
    }

    /// Only the match's ammo is finite: after the kit's lives' worth of
    /// shots the next spawn has nothing, and the body is dry.
    #[test]
    fn the_storage_is_the_lives_worth() {
        let mut s = stock("ak_74u");
        let reserve = s.left[0].1;
        for i in 0..reserve {
            if s.load.rounds == 0 {
                s.load = Load::of(s.load.gun, (reserve - i).min(30));
                s.ready_at = 0.0;
            }
            assert!(s.fired(i as f32 * 100.0), "shot {i}");
        }
        assert_eq!(s.left[0].1, 0);
        s.spawn(LOW, gun("ak_74u").unwrap());
        assert_eq!((s.load.rounds, s.load.spare), (0, 0));
        s.tick(10, key::FIRE, true);
        assert!(s.counts.dry_ticks > 0 && !s.may_fire(10));
    }

    /// The box: a use needs the edge and a ray on the box, takes
    /// `RESUPPLY_MS`, tops the stack up from the storage once and never
    /// writes the magazine; the key let go before the end applies nothing.
    #[test]
    fn the_box_tops_the_stack_up_after_its_time() {
        let mut s = stock("ak_74u");
        let kit = s.kit;
        let reserve = s.left[0].1;
        // Nothing missing: a use does not begin.
        s.resupply(0, true, true, true, true);
        assert_eq!(s.counts.box_uses, 0);
        // 20 rounds fired: the magazine holds 10, the stack the rest.
        for i in 0..20 {
            assert!(s.fired(i as f32 * 100.0));
        }
        let (rounds, spare) = (s.load.rounds, s.load.spare);
        assert_eq!(s.missing(), 20);
        assert_eq!(s.stored(), reserve - kit);
        // No edge: no use.
        s.resupply(5_000, false, true, true, true);
        assert_eq!(s.counts.box_uses, 0);
        s.resupply(5_000, true, true, true, true);
        assert_eq!(s.counts.box_uses, 1);
        s.resupply(5_000 + RESUPPLY_MS - 50, false, true, true, true);
        assert_eq!(s.load.spare, spare);
        s.resupply(5_000 + RESUPPLY_MS, false, true, true, true);
        assert_eq!((s.load.rounds, s.load.spare), (rounds, spare + 20));
        assert_eq!(s.counts.box_rounds, 20);
        assert_eq!(s.load.rounds + s.load.spare, kit);
        // Held on: another needs a new edge.
        assert!(s.fired(10_000.0));
        s.resupply(10_050, false, true, true, true);
        assert_eq!(s.counts.box_uses, 1);
        // The key let go, or the ray off the box, ends a use with nothing.
        for (held, meets) in [(false, true), (true, false)] {
            let mut s = stock("ak_74u");
            assert!(s.fired(0.0));
            s.resupply(100, true, true, true, true);
            s.resupply(150, false, held, meets, true);
            s.resupply(100 + RESUPPLY_MS, false, true, true, true);
            assert_eq!((s.counts.box_cut, s.counts.box_topped), (1, 0));
            assert_eq!(s.load.rounds + s.load.spare, kit - 1);
        }
    }

    /// The storage bounds the top-up, and a spawn takes the kit's count
    /// again from the same unfired total.
    #[test]
    fn the_box_gives_no_more_than_the_storage_has() {
        let mut s = stock("ak_74u");
        let kit = s.kit;
        // Leave only 5 rounds in the storage.
        let reserve = s.left[0].1;
        s.left[0].1 = kit + 5;
        for i in 0..kit {
            if s.load.rounds == 0 {
                s.load = Load::of(s.load.gun, s.load.spare);
            }
            s.ready_at = 0.0;
            assert!(s.fired(i as f32 * 100.0), "{i} of {reserve}");
        }
        assert_eq!(s.load.rounds + s.load.spare, 0);
        assert_eq!(s.stored(), 5);
        s.resupply(0, true, true, true, true);
        s.resupply(RESUPPLY_MS, false, true, true, true);
        assert_eq!(s.load.spare, 5);
        assert_eq!(s.stored(), 0);
        assert_eq!(s.left[0].1, 5, "the match's unfired total is the same");
    }

    /// `SRP_BENCH_RESUPPLY=off` and a carrier: the box does nothing.
    #[test]
    fn a_carrier_or_an_off_box_gets_nothing() {
        let mut s = stock("ak_74u");
        assert!(s.fired(0.0));
        s.resupply(100, true, true, true, false);
        assert_eq!(s.counts.box_uses, 0);
        s.boxed = false;
        s.resupply(100, true, true, true, true);
        assert_eq!(s.counts.box_uses, 0);
    }

    /// Another weapon taken is another kit; the same one leaves the stock.
    #[test]
    fn a_switch_to_another_kit_loads_that_kit() {
        let mut s = stock("ak_74u");
        assert!(s.fired(0.0));
        s.switch(LOW, gun("ak_74u").unwrap());
        assert_eq!(s.load.rounds, 29);
        s.switch(LOW, gun("rem_700").unwrap());
        assert_eq!(s.load.gun.item, "rem_700");
        assert_eq!(s.load.rounds, gun("rem_700").unwrap().magazine);
    }
}
