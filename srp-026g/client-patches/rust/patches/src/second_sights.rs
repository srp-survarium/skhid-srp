//! Patch 206: a weapon's second sight, and the switch to it and back, as
//! 0.69d0 simulates its aim-slot switch (the rules:
//! docs/notes/second-sights.md, "The simulation (0.26g)"; the machine:
//! `second_sight`).
//!
//! The switch key (scope_next, default Y) sets bit 30 of the input on its
//! press. Every game applies every player's input, so each runs the same
//! machine for the weapon the player holds: a press starts a switch of
//! 500 ms, the weapon's tick ends it, putting the weapon away undoes it.
//! While a switch lowers the aim the shot's dispersion is the hip's: its
//! dynamic part is zeroed only at an effective aim of 1, and the dispersion
//! calculator counts the weapon as not aimed.
//!
//! The states are a player's, one for each of its two weapon slots, for
//! the gun the slot holds (a picked-up gun of another kind starts on its
//! main sight), for the player's life (a respawn starts on the main
//! sights). They travel in the world at the end of the player's state, so
//! a rollback, a resync, a late joiner and a replay have them as every game
//! does. A world without them (an older game's) leaves that player's sights
//! unkept: on the main sight, no press taken, nothing written, so this
//! game's worlds are that world's byte for byte until the player's next
//! life.
//!
//! Whether a weapon has a second sight is its config's (`data/
//! srp_second_sight`, read once when the weapon is made, patch 161's
//! field), kept by the weapon until it is destroyed.

#![forbid(unsafe_code)]

use second_sight::{Event, GameTime, Record, Sights, State, View, WeaponSlot};
use srp_validate::world::DictId;

use crate::vostok::carry_runtime::Key;
use crate::vostok::second_sight::{self as game, AimRead, Holder, WeaponId};
use crate::vostok::{Action, ActionMask, ActionState, Context, Flow, InputHandler, Lock, NoExit, Thread};

/// A weapon slot's state, for the gun it was made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    gun: Option<DictId>,
    state: State,
}

/// A player's sights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kept {
    /// Kept: both weapon slots' states (by [`WeaponSlot`]: first, second).
    Kept([Slot; 2]),
    /// From a world without them, for the rest of this life: on the main
    /// sights, and none written.
    Unkept,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Life {
    player: Key,
    kept: Kept,
}

const fn index(slot: WeaponSlot) -> usize {
    match slot {
        WeaponSlot::First => 0,
        WeaponSlot::Second => 1,
    }
}

/// The weapons with a second sight that are alive (made, not yet
/// destroyed): a match's are at most its places' kits' two weapon slots
/// (120), the lobby's and the previews' a few more.
const WEAPONS: usize = 512;
/// A life per place and kit (carry's [`Key`] index: place x 3 + kit).
const LIVES: usize = 60;

/// Every game's sights: the weapons that have two, and each player's states.
pub struct Table {
    two: [Option<WeaponId>; WEAPONS],
    lives: [Option<Life>; LIVES],
    /// A weapon with a second sight had no room (said once in the log).
    full: bool,
}

impl Table {
    pub const NEW: Table = Table { two: [None; WEAPONS], lives: [None; LIVES], full: false };

    fn sights(&self, weapon: WeaponId) -> Sights {
        if self.two.contains(&Some(weapon)) { Sights::Two } else { Sights::One }
    }

    /// `weapon` is made with `sights` (a weapon made anew at an address is
    /// nothing of the one before it). False when it has two and no room.
    fn made(&mut self, weapon: WeaponId, sights: Sights) -> bool {
        self.gone(weapon);
        if sights == Sights::One {
            return true;
        }
        match self.two.iter_mut().find(|at| at.is_none()) {
            Some(at) => {
                *at = Some(weapon);
                true
            }
            None => false,
        }
    }

    fn gone(&mut self, weapon: WeaponId) {
        for at in self.two.iter_mut().filter(|at| **at == Some(weapon)) {
            *at = None;
        }
    }

    /// `player`'s life: a new one (both slots on their main sights) when it
    /// is not the one kept.
    fn life(&mut self, player: Key) -> &mut Kept {
        let at = &mut self.lives[player.index];
        if at.is_none_or(|life| life.player != player) {
            *at = Some(Life { player, kept: Kept::Kept([Slot { gun: None, state: State::MAIN }; 2]) });
        }
        &mut at.as_mut().expect("just set").kept
    }

    /// The state of the weapon `holder` names: the main sight for a gun the
    /// slot's state was not made for, or an unkept life.
    fn state(&self, holder: &Holder) -> State {
        match self.lives[holder.player.index] {
            Some(Life { player, kept: Kept::Kept(slots) }) if player == holder.player => {
                let slot = slots[index(holder.slot)];
                if slot.gun == holder.gun { slot.state } else { State::MAIN }
            }
            _ => State::MAIN,
        }
    }

    /// `event` for the weapon `holder` names.
    fn step(&mut self, holder: &Holder, event: Event) {
        let sights = self.sights(holder.weapon);
        let state = self.state(holder);
        if let Kept::Kept(slots) = self.life(holder.player) {
            slots[index(holder.slot)] = Slot { gun: holder.gun, state: state.step(sights, event) };
        }
    }

    /// What `player`'s state ends with, its slots holding `guns`: none for
    /// an unkept life.
    fn record(&mut self, player: Key, guns: [Option<DictId>; 2]) -> Option<Record> {
        let Kept::Kept(slots) = *self.life(player) else { return None };
        let state = |slot: WeaponSlot| {
            let kept = slots[index(slot)];
            if kept.gun == guns[index(slot)] { kept.state } else { State::MAIN }
        };
        Some(Record::new(state(WeaponSlot::First), state(WeaponSlot::Second)))
    }

    /// `player`'s state was loaded, its slots holding `guns`, with `record`
    /// at its end or none.
    fn load(&mut self, player: Key, guns: [Option<DictId>; 2], record: Option<Record>) {
        let kept = match record {
            Some(record) => Kept::Kept([WeaponSlot::First, WeaponSlot::Second]
                .map(|slot| Slot { gun: guns[index(slot)], state: record.get(slot) })),
            None => Kept::Unkept,
        };
        self.lives[player.index] = Some(Life { player, kept });
    }

    fn view(&self, holder: &Holder, now: GameTime) -> Option<View> {
        (self.sights(holder.weapon) == Sights::Two).then(|| self.state(holder).view(now))
    }
}

static SIGHTS: Lock<Table> = Lock::new(Table::NEW);

/// What the view shows of the weapon `holder` names at `now`: `None` for a
/// weapon with no second sight (vostok's `second_sight::sight`).
pub fn view(thread: &Thread, holder: &Holder, now: GameTime) -> Option<View> {
    SIGHTS.with(thread, |table| table.view(holder, now))
}

fn step(ctx: &Context<'_>, holder: Option<(Holder, Event)>) -> Flow<NoExit> {
    if let Some((holder, event)) = holder {
        SIGHTS.with(ctx.thread(), |table| table.step(&holder, event));
    }
    Flow::Continue
}

/// The input builder's end (process_first_person_mode's epilogue), first
/// of its chain: the switch key's press is bit 30, in that one input.
pub fn on_input_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut handler) = InputHandler::at_builder_end(ctx)
        && handler.action_state(Action::SCOPE_NEXT) == Some(ActionState::Down)
    {
        handler.add_actions(ActionMask::SCOPE_NEXT);
    }
    Flow::Continue
}

/// apply_new_input: the press, in every game, for every player.
pub fn on_press(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx, game::pressed(ctx).map(|(holder, now)| (holder, Event::Press(now))))
}

/// weapon_core::tick: a switch ends at its time.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx, game::ticked(ctx).map(|(holder, now)| (holder, Event::Tick(now))))
}

/// weapon_core::deactivate: a switch going is undone.
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    step(ctx, game::deactivated(ctx).map(|holder| (holder, Event::Deactivate)))
}

/// weapon_core::weapon_core: whether the weapon has a second sight.
pub fn on_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((weapon, sights)) = game::made(ctx) {
        let said = SIGHTS.with(ctx.thread(), |table| {
            let room = table.made(weapon, sights);
            let first = !room && !table.full;
            table.full |= !room;
            first
        });
        if said {
            crate::vostok::log::line(&[crate::vostok::log::Piece::Text(
                "second_sights: no room for another weapon with a second sight; it switches nothing",
            )]);
        }
    }
    Flow::Continue
}

/// ~weapon_core.
pub fn on_destroyed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(weapon) = game::destroyed(ctx) {
        SIGHTS.with(ctx.thread(), |table| table.gone(weapon));
    }
    Flow::Continue
}

/// base_player::serialize's end, after patch 78's carry snapshot: the
/// player's sights, while kept.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(saving) = game::saving(ctx) {
        // the engine's writes run outside the lock
        let record = SIGHTS.with(ctx.thread(), |table| table.record(saving.player, saving.guns));
        if let Some(record) = record {
            game::write(ctx, &record.encode());
        }
    }
    Flow::Continue
}

/// base_player::deserialize's end, after patch 78's carry snapshot.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(loading) = game::loading(ctx) {
        SIGHTS.with(ctx.thread(), |table| table.load(loading.player, loading.guns, loading.record));
    }
    Flow::Continue
}

/// The state of the weapon whose aim `read` reads, while it switches.
fn switching(ctx: &Context<'_>, read: AimRead) -> Option<(State, GameTime)> {
    let (holder, now) = game::aiming(ctx, read)?;
    let state = SIGHTS.with(ctx.thread(), |table| table.state(&holder));
    matches!(state, State::Switching { .. }).then_some((state, now))
}

/// get_dispersed_bullet_direction: the shot compares the effective aim with
/// 1.0 while the weapon switches (the aim progress itself otherwise, as the
/// game).
pub fn on_bullet_aim(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((state, now)) = switching(ctx, AimRead::Bullet) {
        let aim = state.effective_aim(game::bullet_aim(ctx), now);
        game::set_bullet_aim(ctx, aim);
    }
    Flow::Continue
}

/// A read of m_aimed: not aimed while a switch lowers the aim.
fn aimed_read(ctx: &mut Context<'_>, read: AimRead) -> Flow<NoExit> {
    match switching(ctx, read) {
        Some((state, now)) if !state.aimed(now) => {
            game::take_as_unaimed(ctx, read);
            Flow::Skip
        }
        _ => Flow::Continue,
    }
}

/// dispersion_calculator::tick's choice of the aimed multiplier.
pub fn on_dispersion_aimed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    aimed_read(ctx, AimRead::Dispersion)
}

/// get_skill_influence's aimed influences.
pub fn on_skill_aimed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    aimed_read(ctx, AimRead::Skill)
}

/// get_target_koef's aimed values.
pub fn on_target_aimed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    aimed_read(ctx, AimRead::Target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use second_sight::Sight;

    const SIGHTED: WeaponId = WeaponId::numbered(0x1000);
    const PLAIN: WeaponId = WeaponId::numbered(0x2000);

    fn key(spawn: u32) -> Key {
        Key { object: 0x3000, world: 0x4000, spawn, index: 7 }
    }

    fn holder(weapon: WeaponId, slot: WeaponSlot, gun: u16, spawn: u32) -> Holder {
        Holder { player: key(spawn), slot, gun: DictId::new(gun), weapon }
    }

    fn table() -> Table {
        let mut table = Table::NEW;
        assert!(table.made(SIGHTED, Sights::Two) && table.made(PLAIN, Sights::One));
        table
    }

    const AK: u16 = 62;
    const T: GameTime = GameTime(104_520);

    #[test]
    fn a_press_switches_the_held_weapon_and_its_tick_ends_it() {
        let mut table = table();
        let ak = holder(SIGHTED, WeaponSlot::First, AK, 1);
        table.step(&ak, Event::Press(T));
        assert_eq!(table.state(&ak), State::Switching { to: Sight::Second, pressed: T });
        assert_eq!(table.view(&ak, GameTime(104_800)).map(|view| view.s), Some(0.5));
        table.step(&ak, Event::Tick(GameTime(105_050)));
        assert_eq!(table.state(&ak), State::Settled(Sight::Second));
        // the other slot is its own
        assert_eq!(table.state(&holder(SIGHTED, WeaponSlot::Second, AK, 1)), State::MAIN);
        // put away mid-switch: back where it was
        table.step(&ak, Event::Press(GameTime(200_000)));
        table.step(&ak, Event::Deactivate);
        assert_eq!(table.state(&ak), State::Settled(Sight::Second));
    }

    #[test]
    fn a_weapon_without_a_second_sight_switches_nothing_and_shows_none() {
        let mut table = table();
        let plain = holder(PLAIN, WeaponSlot::First, AK, 1);
        table.step(&plain, Event::Press(T));
        assert_eq!(table.state(&plain), State::MAIN);
        assert_eq!(table.view(&plain, T), None);
        // destroyed: an address made anew is nothing of the weapon before it
        table.gone(SIGHTED);
        assert_eq!(table.sights(SIGHTED), Sights::One);
        assert!(table.made(SIGHTED, Sights::Two));
        assert!(table.made(SIGHTED, Sights::One));
        assert_eq!(table.sights(SIGHTED), Sights::One);
    }

    #[test]
    fn a_new_life_and_another_gun_start_on_the_main_sight() {
        let mut table = table();
        let ak = holder(SIGHTED, WeaponSlot::First, AK, 1);
        table.step(&ak, Event::Press(T));
        table.step(&ak, Event::Tick(GameTime(105_050)));
        let picked = Holder { gun: DictId::new(AK + 1), ..ak };
        assert_eq!(table.state(&picked), State::MAIN);
        assert_eq!(table.record(key(1), [DictId::new(AK + 1), None]), Some(Record::MAIN));
        assert_eq!(table.record(key(1), [DictId::new(AK), None]),
                   Some(Record::new(State::Settled(Sight::Second), State::MAIN)));
        assert_eq!(table.state(&holder(SIGHTED, WeaponSlot::First, AK, 2)), State::MAIN, "respawned");
        assert_eq!(table.record(key(2), [DictId::new(AK), None]), Some(Record::MAIN));
    }

    #[test]
    fn a_load_gives_the_states_back_and_a_world_without_them_keeps_none() {
        let mut table = table();
        let ak = holder(SIGHTED, WeaponSlot::Second, AK, 1);
        let record = Record::new(State::MAIN, State::Switching { to: Sight::Main, pressed: T });
        table.load(key(1), [None, DictId::new(AK)], Some(record));
        assert_eq!(table.state(&ak), State::Switching { to: Sight::Main, pressed: T });
        assert_eq!(table.record(key(1), [None, DictId::new(AK)]), Some(record), "saved as loaded");
        table.load(key(1), [None, DictId::new(AK)], None);
        assert_eq!(table.state(&ak), State::MAIN);
        table.step(&ak, Event::Press(GameTime(300_000)));
        assert_eq!(table.state(&ak), State::MAIN, "an unkept life takes no press");
        assert_eq!(table.record(key(1), [None, DictId::new(AK)]), None, "and writes nothing");
        // the next life keeps them again
        assert_eq!(table.record(key(9), [None, DictId::new(AK)]), Some(Record::MAIN));
    }

    #[test]
    fn the_weapons_room_is_bounded() {
        let mut table = Table::NEW;
        for n in 0..WEAPONS as u32 {
            assert!(table.made(WeaponId::numbered(n), Sights::Two));
        }
        assert!(!table.made(WeaponId::numbered(9999), Sights::Two));
        assert!(table.made(WeaponId::numbered(9999), Sights::One));
        table.gone(WeaponId::numbered(3));
        assert!(table.made(WeaponId::numbered(9999), Sights::Two));
    }
}
