//! Patch 171: Artifact Hunt, mode 13 (docs/notes/artefact_hunt.md). The rules
//! are `artefact_hunt_rule`'s; this module turns the world into its events,
//! answers the game's own decisions from its state and shows it:
//!
//! - the rule: a cook of class 481 registered after the game's own
//!   (`game.query_base_resources`), our copies of the client tdm cook's and
//!   tdm_rule_core's vtables, the rule's tick in the world's second list;
//! - its slots: the match's start and the deaths (7), who enters and leaves
//!   (8, 9), the world's bytes (15, 16, `Rule::fields` / `Rule::read`), the
//!   hud state's points and seconds (17), completed (18);
//! - the respawn rule: a death's delay (the waves), and at its tick the
//!   places due held in a round's restart, everyone at its start;
//! - the input (patch 78's `carry.input`, asked first): the frozen
//!   countdown, the use key's take and delivery, the Drop key;
//! - the carrier: his two modifiers (patch 109's getter sites), no sprint;
//!   his marker for both teams;
//! - the hud: `root.artefact_hunt_update`, the messages, the welcome.
//!
//! Every game computes the same: what the simulation's hooks read is the
//! world's (the rule's state, the places, the eye, the level) and the map's
//! config; the hud's are presentation only.
#![forbid(unsafe_code)]

use core::ffi::CStr;

use artefact_hunt_rule::{
    Action, After, ArtefactView, BaseDef, Cue, Event, Hud, Line, Message, Params, Place, Position, RawParams,
    RawShape, Reason, Rule, Shift, Spawner, Spawning, SuddenShown, Team, Time, View, Viewer, MAX_SHAPES, MAX_SPAWNERS,
    PLAYERS, REACH, along,
};

use crate::vostok::artefact_hunt::{
    self as native, Cook, DeathDelay, HudState, HuntCook, HuntRule, HuntWorld, ModifierTerm, RespawnTick, RuleCore,
    RuleEvent, RuleKey, RuleQuery, RuleReader, RuleWriter, TickRule,
};
use crate::vostok::artefact_hunt::ArtefactScene;
use crate::vostok::config::{ConfigError, ConfigTable};
use crate::vostok::resources::{self, Arrived, ArrivedAssets, QueryResults, QueryTag, Serial};
use crate::vostok::sound::{Emitter, Voice};
use crate::vostok::log::{self, Piece};
use crate::vostok::statistics::ScoreEvent;
use crate::vostok::world::{GameWorld, Player, WorldKey};
use crate::vostok::{
    ArtefactHuntOnCameraStateExit, ArtefactHuntOnCanSprintExit, ArtefactHuntOnEnemyExit, ArtefactHuntOnMarkExit,
    ArtefactHuntOnNameExit, Context, Flow, GameCell, GameWorldUi,
    Lock, NoExit, OrigTdmCookCreate, OrigTdmCookSize, OrigTdmRuleCompleted, OrigTdmRuleDeserialize, OrigTdmRuleEntered,
    OrigTdmRuleFillHud, OrigTdmRuleLeft, OrigTdmRuleOnEvent, OrigTdmRuleSerialize, OrigTdmRuleSetCore,
    Thread,
};

/// A resource path or name of the config, kept as a C string (the config
/// may go before a query is asked).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Text([u8; TEXT]);

/// The longest path or name kept, its zero too.
const TEXT: usize = 128;

impl Text {
    fn of(text: &CStr) -> Option<Text> {
        let bytes = text.to_bytes_with_nul();
        let mut out = [0; TEXT];
        out.get_mut(..bytes.len())?.copy_from_slice(bytes);
        Some(Text(out))
    }
    fn get(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.0).unwrap_or(c"")
    }
}

/// The announcer's seven lines, in 0.46c2's order (its name table 0xB2E540,
/// `artefact_hunt_rule::Line`), and the folder they are under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Lines {
    base: Text,
    names: [Text; LINES],
}

const LINES: usize = 7;
const LINE_KEYS: [&CStr; LINES] = [
    c"artefact_spawned",
    c"artefact_picked_up",
    c"artefact_picked_up_by_enemy",
    c"artefact_dropped",
    c"artefact_dropped_by_enemy",
    c"artefact_brought",
    c"artefact_brought_by_enemy",
];

/// The carrier's own values and the artefact's assets: the item's (0.46c2's
/// `artefact.options`), the announcer's `mode.sound_notifications`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Item {
    /// The model (class 15) and its idle, spawn and destroy particles (61).
    model: Text,
    particles: [Text; 3],
    /// The carrier's glow: its particle systems (class 61), for the
    /// carrier seen by others and for his own first-person view, drawn by
    /// each game for itself and never in the simulation; none when the
    /// config names none (`item.carrier_particle`, `item.carrier_particle_fp`).
    glow: Option<[Text; 2]>,
    lines: Lines,
    /// apply_modifier's index 4 term (-0.2: 20 % slower).
    movement: f32,
    /// Index 2 (+0.05).
    regeneration: f32,
}

/// The modifier lists the carrier's terms are added to.
const MOVEMENT_LIST: u32 = 4;
const REGENERATION_LIST: u32 = 2;

/// The match's rule and what it plays with: one at a time (the cook's last
/// rule), its world once slot 11 gave it.
#[derive(Clone, Copy)]
struct Hunt {
    rule: RuleKey,
    world: Option<WorldKey>,
    params: Params,
    item: Item,
    state: Rule,
}

static COOK: Cook = Cook::new();
static HUNT: Lock<Option<Hunt>> = Lock::new(None);

/// The state of the hunt playing in `world`.
fn hunt_in(thread: &Thread, world: WorldKey) -> Option<Hunt> {
    HUNT.with(thread, |hunt| *hunt).filter(|hunt| hunt.world == Some(world))
}

/// The parameters of the match being played, for patch 87's check of a
/// world (srp_validate::world::Context::artefact_hunt).
pub fn params(thread: &Thread) -> Option<Params> {
    let world = HuntWorld::of_match(thread)?.key();
    hunt_in(thread, world).map(|hunt| hunt.params)
}

// ---- the mode config ---------------------------------------------------------

/// Where a config is refused: the key the pack has wrong.
fn refused(key: &'static str) -> ! {
    log::line(&[Piece::Text("artefact_hunt: the mode config's "), Piece::Text(key),
                Piece::Text(" is no value the rule plays with; the match cannot be played")]);
    panic!("artefact_hunt: mode config key {key}")
}

fn integer(table: &ConfigTable<'_>, key: &'static CStr) -> Result<u32, &'static CStr> {
    table.required(key).and_then(|value| value.integer()).map_err(|_| key)
}

fn number(table: &ConfigTable<'_>, key: &'static CStr) -> Result<f32, &'static CStr> {
    table.required(key).and_then(|value| value.number()).map_err(|_| key)
}

fn vector(table: &ConfigTable<'_>, key: &'static CStr) -> Result<[f32; 3], &'static CStr> {
    table.required(key).and_then(|value| value.vector()).map_err(|_| key)
}

fn table<'t>(table: &ConfigTable<'t>, key: &'static CStr) -> Result<ConfigTable<'t>, &'static CStr> {
    table.required(key).and_then(|value| value.table()).map_err(|_| key)
}

/// The carrier's glow's two particle systems (`item.carrier_particle`, on
/// his body; `item.carrier_particle_fp`, his first-person view): both or
/// neither.
fn glow(item: &ConfigTable<'_>) -> Result<Option<[Text; 2]>, &'static str> {
    const WHAT: &str = "artefact_hunt.item.carrier_particle and carrier_particle_fp (both texts, or neither)";
    let one = |name: &CStr| match item.get(name) {
        Ok(None) => Ok(None),
        Ok(Some(value)) => value.text().ok().and_then(Text::of).map(Some).ok_or(WHAT),
        Err(_) => Err(WHAT),
    };
    match (one(c"carrier_particle")?, one(c"carrier_particle_fp")?) {
        (Some(body), Some(first_person)) => Ok(Some([body, first_person])),
        (None, None) => Ok(None),
        _ => Err(WHAT),
    }
}

/// The mode config's values ("Data"), as `Params::parse` takes them.
fn read_config(root: &ConfigTable<'_>) -> Result<(Params, Item), &'static str> {
    let key = |key: &'static CStr| key.to_str().unwrap_or("?");
    let mode = table(root, c"mode").map_err(key)?;
    let parameters = table(&mode, c"parameters").map_err(key)?;
    let hunt = table(&mode, c"artefact_hunt").map_err(key)?;
    let item = table(&hunt, c"item").map_err(key)?;
    let int = |name| integer(&parameters, name).map_err(key);
    let given = RawParams {
        match_time: int(c"match_time")?,
        time_needed_to_continue_match: int(c"time_needed_to_continue_match")?,
        draw_additional_time: int(c"draw_additional_time")?,
        time_before_new_round: int(c"time_before_new_round")?,
        round_countdown_time: int(c"round_countdown_time")?,
        score: int(c"score")?,
        respawn_time: int(c"respawn_time")?,
        time_needed_to_respawn: int(c"time_needed_to_respawn")?,
        vi_spawn_time: int(c"vi_spawn_time")?,
        vi_spawn_variation: int(c"vi_spawn_variation")?,
        vi_not_picked_ttl: int(c"vi_not_picked_ttl")?,
        vi_dropped_ttl: int(c"vi_dropped_ttl")?,
        drop_lift: number(&item, c"drop_lift").map_err(key)?,
        usable_radius: number(&item, c"usable_radius").map_err(key)?,
    };
    let text = |table: &ConfigTable<'_>, name: &'static CStr| table.required(name).and_then(|value| value.text()).ok()
        .and_then(Text::of).ok_or_else(|| key(name));
    let notifications = table(&mode, c"sound_notifications").map_err(key)?;
    let mut names = [Text([0; TEXT]); LINES];
    // each line {class, name}: a sound collection (258) under base/<voice>/
    for (out, name) in names.iter_mut().zip(LINE_KEYS) {
        let pair = notifications.required(name).and_then(|value| value.entries()).map_err(|_| key(name))?;
        let mut values = pair.iter();
        let class = values.next().and_then(|value| value.ok()?.integer().ok());
        let line = values.next().and_then(|value| value.ok()?.text().ok()).and_then(Text::of);
        match (class, line, values.next()) {
            (Some(resources::SOUND_COLLECTION_CLASS), Some(line), None) => *out = line,
            _ => return Err(key(name)),
        }
    }
    let carrier = Item {
        model: text(&item, c"model")?,
        particles: [text(&item, c"particle_idle")?, text(&item, c"particle_spawn")?, text(&item, c"particle_destroy")?],
        glow: glow(&item)?,
        lines: Lines { base: text(&notifications, c"base_path")?, names },
        movement: number(&item, c"movement_speed_modifier").map_err(key)?,
        regeneration: number(&item, c"health_regeneration_speed_modifier").map_err(key)?,
    };
    if !carrier.movement.is_finite() || !carrier.regeneration.is_finite() {
        return Err("artefact_hunt.item modifiers");
    }
    let entries = |name: &'static CStr| {
        hunt.required(name).and_then(|value| value.entries()).map_err(|_| key(name))
    };
    let mut spawners = [Spawner { position: [0.0; 3] }; MAX_SPAWNERS];
    let listed = entries(c"spawners")?;
    if listed.len() > MAX_SPAWNERS {
        return Err("artefact_hunt.spawners");
    }
    for (out, entry) in spawners.iter_mut().zip(listed.iter()) {
        let entry = entry.and_then(|value| value.table()).map_err(|_: ConfigError| "artefact_hunt.spawners")?;
        out.position = vector(&entry, c"position").map_err(|_| "artefact_hunt.spawners[].position")?;
    }
    let none = RawShape { kind: 0, position: [0.0; 3], axes: [[0.0; 3]; 3], half: [0.0; 3] };
    let mut shapes = [[none; MAX_SHAPES]; 2];
    let mut bases = [(0u32, [0.0f32; 3], 0usize); 2];
    let listed_bases = entries(c"bases")?;
    if listed_bases.len() != 2 {
        return Err("artefact_hunt.bases");
    }
    for ((base, room), entry) in bases.iter_mut().zip(shapes.iter_mut()).zip(listed_bases.iter()) {
        let entry = entry.and_then(|value| value.table()).map_err(|_| "artefact_hunt.bases")?;
        base.0 = integer(&entry, c"team").map_err(|_| "artefact_hunt.bases[].team")?;
        base.1 = vector(&entry, c"position").map_err(|_| "artefact_hunt.bases[].position")?;
        let listed_shapes = entry.required(c"shapes").and_then(|value| value.entries())
            .map_err(|_| "artefact_hunt.bases[].shapes")?;
        if listed_shapes.len() > MAX_SHAPES {
            return Err("artefact_hunt.bases[].shapes");
        }
        base.2 = listed_shapes.len();
        for (out, shape) in room.iter_mut().zip(listed_shapes.iter()) {
            let shape = shape.and_then(|value| value.table()).map_err(|_| "artefact_hunt.bases[].shapes")?;
            let field = |name| vector(&shape, name).map_err(|_| "artefact_hunt.bases[].shapes[]");
            *out = RawShape {
                kind: integer(&shape, c"type").map_err(|_| "artefact_hunt.bases[].shapes[].type")?,
                position: field(c"position")?,
                axes: [field(c"axis_x")?, field(c"axis_y")?, field(c"axis_z")?],
                half: field(c"half")?,
            };
        }
    }
    let defs = [0, 1].map(|i| BaseDef { team: bases[i].0, position: bases[i].1, shapes: &shapes[i][..bases[i].2] });
    let params = Params::parse(&given, &spawners[..listed.len()], &defs).map_err(|error| match error {
        artefact_hunt_rule::ParamError::MatchTime => "parameters.match_time",
        artefact_hunt_rule::ParamError::TimeNeededToContinueMatch => "parameters.time_needed_to_continue_match",
        artefact_hunt_rule::ParamError::DrawAdditionalTime => "parameters.draw_additional_time",
        artefact_hunt_rule::ParamError::TimeBeforeNewRound => "parameters.time_before_new_round",
        artefact_hunt_rule::ParamError::RoundCountdownTime => "parameters.round_countdown_time",
        artefact_hunt_rule::ParamError::Score => "parameters.score",
        artefact_hunt_rule::ParamError::RespawnTime => "parameters.respawn_time",
        artefact_hunt_rule::ParamError::TimeNeededToRespawn => "parameters.time_needed_to_respawn",
        artefact_hunt_rule::ParamError::ViSpawnTime => "parameters.vi_spawn_time",
        artefact_hunt_rule::ParamError::ViNotPickedTtl => "parameters.vi_not_picked_ttl",
        artefact_hunt_rule::ParamError::ViDroppedTtl => "parameters.vi_dropped_ttl",
        artefact_hunt_rule::ParamError::DropLift => "artefact_hunt.item.drop_lift",
        artefact_hunt_rule::ParamError::UsableRadius => "artefact_hunt.item.usable_radius",
        artefact_hunt_rule::ParamError::Spawners => "artefact_hunt.spawners",
        artefact_hunt_rule::ParamError::Bases => "artefact_hunt.bases",
    })?;
    Ok((params, carrier))
}

// ---- the cook and the rule's slots -----------------------------------------

/// game::query_base_resources' start: the cook registered once, after the
/// game's own.
pub fn on_register(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // A taken class leaves the mode unloaded (its rule cannot be made).
    COOK.register(ctx, crate::vostok::artefact_hunt_cook_vtable_handle());
    Flow::Continue
}

/// The cook's create (slot 9): our rule, its config read and its state made
/// from the match's seed; a config the rule cannot play stops the game.
pub fn on_cook_create(_: &OrigTdmCookCreate, cook: &mut HuntCook<'_>, query: &RuleQuery<'_>) -> u32 {
    let thread = cook.thread();
    let Some(root) = query.config() else { refused("mode (no config)") };
    let (params, item) = read_config(&root).unwrap_or_else(|key| refused(key));
    let Some(rule) = cook.make_rule(crate::vostok::artefact_hunt_rule_vtable_handle()) else { return 0 };
    let state = Rule::new(query.seed().unwrap_or(0));
    HUNT.with(thread, |hunt| *hunt = Some(Hunt { rule, world: None, params, item, state }));
    SHOWN.set(thread, Shown::NEW);
    rule.address()
}

/// Slot 10: the rule's size.
pub fn on_cook_size(_: &OrigTdmCookSize, _: &mut HuntCook<'_>) -> u32 {
    native::RULE_SIZE
}

/// The hunt a slot is called for: the rule's own, none for any other object.
fn with_rule<R>(rule: &HuntRule<'_>, f: impl FnOnce(&mut Hunt) -> R) -> Option<R> {
    let key = rule.key();
    HUNT.with(rule.thread(), |hunt| hunt.as_mut().filter(|hunt| hunt.rule == key).map(f))
}

/// One event through the rule, its score events counted and its carrier's
/// animations re-selected when the carrier changed; the statistics' tail
/// written after it.
fn step(world: &HuntWorld<'_>, hunt: &mut Hunt, event: Event) {
    let before = hunt.state.carrier();
    let (state, actions) = hunt.state.step(&hunt.params, event);
    hunt.state = state;
    for action in actions.iter() {
        match action {
            Action::Put { place } => world.count(place.get(), ScoreEvent::PUT_VICTORY_ITEM_IN_CONTAINER, 1),
            Action::Bring { place, hundredths } => world.count(place.get(), ScoreEvent::BRING_VICTORY_ITEM, hundredths),
        }
    }
    let after = hunt.state.carrier();
    if before != after {
        for place in [before, after].into_iter().flatten() {
            if let Some(player) = world.player(place.get()) {
                player.select_animations();
            }
        }
    }
    publish(world, &hunt.state);
}

fn publish(world: &HuntWorld<'_>, state: &Rule) {
    world.publish(state.score(), state.leader().byte());
}

fn position(at: [f32; 3]) -> Option<Position> {
    Position::from_metres(at)
}

/// Where `place` lets go of the artefact: the ground under him, else his
/// feet (out of the level: the 60 s remove it).
fn ground_of(world: &HuntWorld<'_>, place: u8) -> Option<Position> {
    let at = world.ground_under(place).or_else(|| Some(world.body(place)?.feet))?;
    position(at)
}

/// Slot 7: the match's start (0), a death (1).
pub fn on_event(_: &OrigTdmRuleOnEvent, rule: &mut HuntRule<'_>, kind: u32, data: &RuleEvent<'_>, time: u32) {
    let Some(world) = rule.world() else { return };
    with_rule(rule, |hunt| match kind {
        0 => {
            if let Some(start) = world.play_start() {
                step(&world, hunt, Event::Start { now: Time::from_ms(start) });
            }
        }
        1 => {
            let Some((_, victim)) = data.kill() else { return };
            let Some(place) = Place::new(victim) else { return };
            let Some(ground) = ground_of(&world, victim) else { return };
            step(&world, hunt, Event::Died { place, now: Time::from_ms(time), ground });
        }
        _ => {}
    });
}

/// Slot 8: a player entered the match, of his profile's team. In a match the
/// place's player has no team of the two yet here (2026-11-21's test: nobody
/// was ever in the match): the tick takes him in once he has one.
pub fn on_entered(_: &OrigTdmRuleEntered, rule: &mut HuntRule<'_>, place: u32, time: u32) {
    let Some(world) = rule.world() else { return };
    let Some(place) = u8::try_from(place).ok().and_then(Place::new) else { return };
    let Some(team) = world.body(place.get()).and_then(|body| Team::from_raw(body.team)) else { return };
    with_rule(rule, |hunt| step(&world, hunt, Event::Entered { place, team, now: Time::from_ms(time) }));
}

/// The places in the world (the core's active places: a leaver's bit goes in
/// the same call as slot 9) whose player is of a team the rule does not have
/// him on: entered now.
fn take_in(world: &HuntWorld<'_>, hunt: &mut Hunt, now: Time) {
    for place in (0..PLAYERS as u8).filter_map(Place::new) {
        let Some(team) = world.body(place.get()).and_then(|body| Team::from_raw(body.team)) else { continue };
        if hunt.state.sides().team_of(place) != Some(team) {
            log::line(&[Piece::Text("artefact_hunt: place "), Piece::Dec(u32::from(place.get())),
                        Piece::Text(" is in the match, team "), Piece::Dec(team as u32)]);
            step(world, hunt, Event::Entered { place, team, now });
        }
    }
}

/// Slot 9: a player left (the place still holds his player).
pub fn on_left(_: &OrigTdmRuleLeft, rule: &mut HuntRule<'_>, place: u32, time: u32) {
    let Some(world) = rule.world() else { return };
    let Some(place) = u8::try_from(place).ok().and_then(Place::new) else { return };
    with_rule(rule, |hunt| {
        let ground = (hunt.state.carrier() == Some(place)).then(|| ground_of(&world, place.get())).flatten();
        step(&world, hunt, Event::Left { place, now: Time::from_ms(time), ground });
    });
}

/// Slot 11: the world core; the tick in its second list.
pub fn on_set_core(_: &OrigTdmRuleSetCore, rule: &mut HuntRule<'_>, core: &RuleCore<'_>) {
    rule.set_core(core, crate::vostok::artefact_hunt_on_tick_entry());
    let world = rule.world().map(|world| world.key());
    with_rule(rule, |hunt| hunt.world = world);
}

/// The tick (the delegate's function): the step's time, then the
/// blackboard's flags in play.
pub fn on_tick(rule: &TickRule<'_>, _dt: u32, time: u32, _step: u32) {
    let Some(rule) = rule.rule() else { return };
    let Some(world) = rule.world() else { return };
    let started = timer_start(world.thread());
    with_rule(&rule, |hunt| {
        take_in(&world, hunt, Time::from_ms(time));
        step(&world, hunt, Event::Tick { now: Time::from_ms(time) });
        if matches!(hunt.state.stage(), artefact_hunt_rule::Stage::Play(_)) {
            world.set_respawns(hunt.state.respawns());
        }
        round_countdown(&world, hunt);
    });
    timer_end(world.thread(), started, "the rule's tick");
}

/// A round about to start is the match start's countdown in the game (the
/// core's time state [`ROUND_COUNTDOWN`] for the next step: patch 31's kit
/// change, the client's warmup camera and kits); each kit switch patch 31
/// makes at the next step counted (a life for supplies). In the tick, after
/// the time rule's (list 1) and the places' (list 0: the step's kit choices
/// made the next kits): the respawn rule's tick of the next step switches
/// exactly these places.
fn round_countdown(world: &HuntWorld<'_>, hunt: &Hunt) {
    world.set_round_countdown(hunt.state.round_countdown());
}

/// `SKHID_HUNT_LOG=on`: one line for each decision about the carrier (off
/// by default; read once).
fn hunt_log(thread: &Thread) -> bool {
    use core::sync::atomic::{AtomicU8, Ordering};
    static ON: AtomicU8 = AtomicU8::new(0);
    match ON.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let mut room = [0u8; 8];
            let len = crate::vostok::renderer::get_environment(thread, c"SKHID_HUNT_LOG", &mut room);
            let on = room[..len] == *b"on";
            ON.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// The carrier decision's line (`SKHID_HUNT_LOG=on`).
fn carrier_line(world: &HuntWorld<'_>, carrier: Place, what: &str) {
    if hunt_log(world.thread()) {
        log::line(&[Piece::Text("artefact_hunt: tick "), Piece::Dec(world.now()), Piece::Text(" game "),
                    Piece::Dec(crate::vostok::gun_cook::process_id()), Piece::Text(": carrier "),
                    Piece::Dec(u32::from(carrier.get())), Piece::Text(" "), Piece::Text(what)]);
    }
}

/// Slot 15: the state field by field (`Rule::fields`).
pub fn on_serialize(_: &OrigTdmRuleSerialize, rule: &mut HuntRule<'_>, writer: &RuleWriter<'_>) {
    with_rule(rule, |hunt| {
        let mut line = 0;
        hunt.state.fields(&mut |field| {
            writer.field(field, line);
            line += 1;
        });
    });
}

/// Slot 16: the state read back (`Rule::read`); the world check (patch 87)
/// refused a world whose bytes it cannot read before the load.
pub fn on_deserialize(_: &OrigTdmRuleDeserialize, rule: &mut HuntRule<'_>, reader: &RuleReader<'_>, _: u32) {
    let world = rule.world();
    with_rule(rule, |hunt| {
        let params = hunt.params;
        let Some(state) = reader.take(|bytes| Rule::read(&params, bytes).ok()) else {
            log::line(&[Piece::Text("artefact_hunt: a world's rule state the rule cannot read")]);
            panic!("artefact_hunt: a world's rule state the rule cannot read");
        };
        hunt.state = state;
        // The time rule's deserialize sent event 0 first and wrote its
        // state: the tail and the round's countdown are the loaded state's
        // from here.
        if let Some(world) = world {
            publish(&world, &hunt.state);
            world.set_round_countdown(hunt.state.round_countdown());
        }
    });
}

/// Slot 17: the teams' points and the clock's seconds (the match time's,
/// sudden death's own), after the time rule's fill.
pub fn on_fill_hud(_: &OrigTdmRuleFillHud, rule: &mut HuntRule<'_>, hud: &HudState<'_>, time: u32) {
    with_rule(rule, |hunt| {
        let shown = hunt.state.hud(&hunt.params, Time::from_ms(time));
        hud.fill(hunt.state.score(), shown.clock_ms / 1000);
    });
}

/// Slot 18: true from the match's end on.
pub fn on_completed(_: &OrigTdmRuleCompleted, rule: &mut HuntRule<'_>, time: u32) -> bool {
    with_rule(rule, |hunt| hunt.state.completed(Time::from_ms(time))).unwrap_or(false)
}

// ---- the respawn rule ----------------------------------------------------------

/// A death's delay: the wave's time (`Rule::respawn_at`), the native grid
/// and store after it.
pub fn on_death_delay(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut delay) = DeathDelay::at(ctx) else { return Flow::Continue };
    let Some(world) = delay.world().map(|world| world.key()) else { return Flow::Continue };
    let Some(hunt) = hunt_in(delay.thread(), world) else { return Flow::Continue };
    let shift = Shift::new(delay.delay() as i32 - delay.respawn_time() as i32);
    let death = Time::from_ms(delay.time());
    if let Some(at) = hunt.state.respawn_at(&hunt.params, death, shift) {
        delay.set_delay(at.ms().wrapping_sub(death.ms()));
    }
    Flow::Continue
}

/// The respawn rule's tick: a round's restart holds the places due, its end
/// spawns everyone.
pub fn on_respawn_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let started = timer_start(ctx.thread());
    let flow = respawn_tick(ctx);
    timer_end(ctx.thread(), started, "the respawn tick");
    flow
}

fn respawn_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(tick) = RespawnTick::at(ctx) else { return Flow::Continue };
    let Some(world) = tick.world().map(|world| world.key()) else { return Flow::Continue };
    let Some(hunt) = hunt_in(ctx.thread(), world) else { return Flow::Continue };
    // A round about to start: patch 31's kit switch (its state test leaves a
    // round's countdown to this tick), each a life for supplies, counted.
    if hunt.state.round_countdown() {
        let world_now = tick.world();
        for place in (0..PLAYERS as u8).filter_map(Place::new) {
            if tick.switch_kit(place.get()) {
                HUNT.with(ctx.thread(), |hunt| {
                    if let (Some(hunt), Some(world)) = (hunt.as_mut(), world_now.as_ref()) {
                        step(world, hunt, Event::KitSwitched { place });
                    }
                });
                if hunt_log(ctx.thread()) {
                    log::line(&[Piece::Text("artefact_hunt: tick "), Piece::Dec(tick.time()), Piece::Text(" game "),
                                Piece::Dec(crate::vostok::gun_cook::process_id()), Piece::Text(": place "),
                                Piece::Dec(u32::from(place.get())), Piece::Text(" switches kit in the round countdown")]);
                }
            }
        }
    }
    match hunt.state.spawning(Time::from_ms(tick.time())) {
        Spawning::Native => {}
        Spawning::Held => tick.hold_due(),
        Spawning::Everyone => tick.everyone_now(),
    }
    Flow::Continue
}

// ---- the input -------------------------------------------------------------

/// What an input is to the hunt (asked by patch 78's `carry.input` first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A take or a delivery: the use key's press is the hunt's.
    Used,
    /// The game's (and patch 78's, 85's).
    Game,
}

/// The view's level hit along it within the reach, measured along it.
fn level_along(player: &Player<'_>, view: &View) -> Option<f32> {
    let hit = player.physics_world()?.ray_test(view.eye, view.forward, REACH, 0x24, 2)?;
    Some(along(view, hit.point))
}

/// A player's input at its time `now`: `use_pressed` and `drop_pressed` its
/// edges (the raw mask against the last).
pub fn input(player: &Player<'_>, now: u32, use_pressed: bool, drop_pressed: bool) -> Input {
    let started = timer_start(player.thread());
    let input = decide_input(player, now, use_pressed, drop_pressed);
    timer_end(player.thread(), started, "an input");
    input
}

fn decide_input(player: &Player<'_>, now: u32, use_pressed: bool, drop_pressed: bool) -> Input {
    let thread = player.thread();
    let Some(world) = HuntWorld::of_player(player) else { return Input::Game };
    let key = world.key();
    let decided = HUNT.with(thread, |hunt| {
        let hunt = hunt.as_mut().filter(|hunt| hunt.world == Some(key))?;
        let place = Place::new(player.hunt_place())?;
        if !player.alive() || !player.inserted() {
            return Some(Input::Game);
        }
        let now = Time::from_ms(now);
        let carrier = hunt.state.carrier();
        if drop_pressed && carrier == Some(place) {
            let ground = ground_of(&world, place.get())?;
            step(&world, hunt, Event::Drop { place, now, ground });
            decision(world.thread(), Decision::Drop, place, CARRIED.get(world.thread()), now);
        }
        if !use_pressed {
            return Some(Input::Game);
        }
        let (eye, forward) = player.carry_eye();
        let view = View { eye, forward };
        let level = level_along(player, &view);
        match (hunt.state.artefact(&hunt.params), carrier) {
            (ArtefactView::Lying { at, .. }, _) if hunt.params.takes(&view, at, level) => {
                step(&world, hunt, Event::Take { place, now });
                let taken = hunt.state.carrier() == Some(place);
                // The lying artefact's point is the take's own: a
                // re-simulation's take of it is the same one.
                let [x, y, z] = at.mm();
                let key = (x as u32).wrapping_mul(31).wrapping_add(y as u32).wrapping_mul(31).wrapping_add(z as u32);
                if taken {
                    CARRIED.set(world.thread(), key);
                }
                decision(world.thread(), if taken { Decision::Take } else { Decision::Refused }, place, key, now);
                Some(if taken { Input::Used } else { Input::Game })
            }
            (ArtefactView::Carried { by, team }, _) if by == place && hunt.params.delivers(&view, team, level) => {
                let ground = position(player.feet())?;
                step(&world, hunt, Event::Deliver { place, now, ground });
                let [a, b] = hunt.state.score();
                decision(world.thread(), Decision::Deliver, place, u32::from(a) << 8 | u32::from(b), now);
                Some(Input::Used)
            }
            _ => Some(Input::Game),
        }
    });
    decided.unwrap_or(Input::Game)
}

/// One line for a press the hunt decided on (a re-simulation logs it again).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Decision {
    Take,
    Refused,
    Deliver,
    Drop,
}

impl Decision {
    const fn text(self) -> &'static str {
        match self {
            Decision::Take => "takes it",
            Decision::Refused => "may not take it (not in the match, or fallen)",
            Decision::Deliver => "delivers it",
            Decision::Drop => "drops it",
        }
    }
}

/// The last decision logged, by its event (the lying artefact's point for a
/// take, the carried one's for a drop, the scores after a delivery), and the
/// carried artefact's point: a re-simulation's same decision is not logged
/// again.
static LOGGED: GameCell<Option<(Decision, Place, u32)>> = GameCell::new(None);
static CARRIED: GameCell<u32> = GameCell::new(0);

/// One line for a press the hunt decided on, once an event.
fn decision(thread: &Thread, what: Decision, place: Place, key: u32, now: Time) {
    let event = Some((what, place, key));
    if LOGGED.get(thread) == event {
        return;
    }
    LOGGED.set(thread, event);
    log::line(&[Piece::Text("artefact_hunt: place "), Piece::Dec(u32::from(place.get())), Piece::Text(" "),
                Piece::Text(what.text()), Piece::Text(" at "), Piece::Dec(now.ms()), Piece::Text(" ms")]);
}

// ---- the carrier -------------------------------------------------------------

/// The carrier of the match being played and his kit's modifiers.
fn carrier_term(term: &ModifierTerm<'_, '_>) -> Option<f32> {
    let world = HuntWorld::of_match(term.thread())?;
    let hunt = hunt_in(term.thread(), world.key())?;
    let carrier = hunt.state.carrier()?;
    if world.modifiers(carrier.get())? != term.modifiers() {
        return None;
    }
    let term = match term.index() {
        MOVEMENT_LIST => Some(hunt.item.movement),
        REGENERATION_LIST => Some(hunt.item.regeneration),
        _ => None,
    };
    if term.is_some() {
        carrier_line(&world, carrier, if term == Some(hunt.item.movement) { "movement term" } else { "regeneration term" });
    }
    term
}

/// apply_modifier's own float (patch 109's `artefact.shared_modifier`): the
/// carrier's terms.
pub fn shared_modifier(ctx: &mut Context<'_>) {
    if let Some(mut term) = ModifierTerm::at_shared_modifier(ctx)
        && let Some(add) = carrier_term(&term)
    {
        term.add(add);
    }
}

/// The same in apply_modifiers' loop (`artefact.shared_modifiers`).
pub fn shared_modifiers(ctx: &mut Context<'_>) {
    if let Some(mut term) = ModifierTerm::at_shared_modifiers(ctx)
        && let Some(add) = carrier_term(&term)
    {
        term.add(add);
    }
}

/// Whether `player` is the carrier of the hunt in his world.
fn carries(player: &Player<'_>) -> bool {
    let Some(world) = HuntWorld::of_player(player) else { return false };
    hunt_in(player.thread(), world.key())
        .and_then(|hunt| hunt.state.carrier())
        .is_some_and(|carrier| carrier.get() == player.hunt_place())
}

/// base_player::can_sprint: never the carrier.
pub fn on_can_sprint(ctx: &mut Context<'_>) -> Flow<ArtefactHuntOnCanSprintExit> {
    match native::sprinter(ctx) {
        Some(player) if carries(&player) => {
            if let (Some(world), Some(place)) = (HuntWorld::of_player(&player), Place::new(player.hunt_place())) {
                carrier_line(&world, place, "may not sprint");
            }
            Flow::Exit(ArtefactHuntOnCanSprintExit::Refused)
        }
        _ => Flow::Continue,
    }
}

/// render_name's spotted test of an enemy: the carrier's marker always.
pub fn on_name(ctx: &mut Context<'_>) -> Flow<ArtefactHuntOnNameExit> {
    match native::name_enemy(ctx) {
        Some(player) if carries(&player) => Flow::Exit(ArtefactHuntOnNameExit::Shown),
        _ => Flow::Continue,
    }
}

/// update_minimap_players' spotted test of an enemy: the carrier's dot always.
pub fn on_enemy(ctx: &mut Context<'_>) -> Flow<ArtefactHuntOnEnemyExit> {
    match native::minimap_enemy(ctx) {
        Some(player) if carries(&player) => Flow::Exit(ArtefactHuntOnEnemyExit::Shown),
        _ => Flow::Continue,
    }
}

/// initialize_match as it sends the icons movie its enemy list: at every
/// match's start the movie told whether it shows the carrier of either team
/// (true in a hunt, false in every other mode: the movie keeps the flag).
pub fn on_enemy_ids(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let hunting = HuntWorld::of_match(ctx.thread()).is_some_and(|world| hunt_in(ctx.thread(), world.key()).is_some());
    if let Some(movie) = GameWorldUi::at_enemy_ids(ctx).and_then(|ui| ui.icons_movie()) {
        movie.invoke_bool(c"root.srp_show_carriers", hunting);
    }
    Flow::Continue
}

// ---- the hud -------------------------------------------------------------------

/// What the hud was last told.
#[derive(Clone, Copy)]
struct Shown {
    /// The rule shown last (the cues are told from it).
    rule: Option<(RuleKey, Rule)>,
    /// The movie and the checksum of what it got, the frames since.
    told: Option<(usize, u32)>,
    frames: u32,
    /// A message the movie did not take, owed to the next frame.
    owed: Option<Message>,
    prompt: Option<(usize, Prompt)>,
}

impl Shown {
    const NEW: Shown = Shown { rule: None, told: None, frames: 0, owed: None, prompt: None };
}

static SHOWN: GameCell<Shown> = GameCell::new(Shown::NEW);

/// The use key's prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prompt {
    None,
    Take,
    Put,
}

impl Prompt {
    fn key(self) -> &'static CStr {
        match self {
            Prompt::None => c"",
            Prompt::Take => c"st_pickup_item",
            Prompt::Put => c"st_put_item",
        }
    }
}

/// The movie gets the whole update at most this often unchanged.
const RESEND_FRAMES: u32 = 60;
/// A message's time on the screen, ms (0.46c2's 4000).
const MESSAGE_MS: u32 = 4000;

fn message_key(message: Message) -> &'static CStr {
    match message {
        Message::Spawned => c"st_on_spawn_victory_artefact",
        Message::YouPicked => c"st_artefact_hunt_you_picked",
        Message::TeamDelivered => c"st_artefact_hunt_team_delivered",
        Message::EnemyDelivered => c"st_artefact_hunt_enemy_delivered",
        Message::Destroyed => c"st_on_destroyed_victory_artefact",
        Message::Won(Reason::Collected) => c"st_artefact_hunt_won_collected",
        Message::Won(Reason::TimeOut) => c"st_artefact_hunt_won_time_out",
        Message::Lost(Reason::Collected) => c"st_artefact_hunt_lost_collected",
        Message::Lost(Reason::TimeOut) => c"st_artefact_hunt_lost_time_out",
        Message::Won(Reason::Wiped) => c"st_artefact_hunt_won_wiped",
        Message::Lost(Reason::Wiped) => c"st_artefact_hunt_lost_wiped",
        Message::Draw => c"st_artefact_hunt_draw",
    }
}

/// The ms a value is told in: whole seconds, rounded up.
fn seconds(ms: u32) -> u32 {
    ms.div_ceil(1000)
}

/// The update call's 28 values ("HUD calls").
fn update_values(hud: &Hud, params: &Params, viewer: Viewer, viewed_carries: bool) -> [f64; 28] {
    let own = viewer.team.unwrap_or(Team::Zero);
    let own_team = viewer.team.map_or(3.0, |team| team.index() as f64);
    let (artefact, x, z, carrier, carrier_team) = match hud.artefact {
        ArtefactView::Nowhere => (0.0, 0.0, 0.0, 255.0, 2.0),
        ArtefactView::Lying { at, .. } => {
            let [x, _, z] = at.metres();
            (1.0, f64::from(x), -f64::from(z), 255.0, 2.0)
        }
        ArtefactView::Carried { by, team } => (2.0, 0.0, 0.0, f64::from(by.get()), team.index() as f64),
    };
    let after = match hud.after {
        None => 0.0,
        Some(After::End) => 1.0,
        Some(After::NewRound | After::SuddenDeath { .. }) => 2.0,
    };
    let base = |team: Team| {
        let [x, _, z] = params.base(team).metres();
        [team.index() as f64, f64::from(x), -f64::from(z)]
    };
    let [b0, b1] = [base(Team::Zero), base(Team::One)];
    let sudden = match hud.sudden_death {
        SuddenShown::Off => 0.0,
        SuddenShown::Armed => 1.0,
        SuddenShown::On => 2.0,
    };
    let phase = hud.phase as u8;
    [
        own_team,
        f64::from(hud.score[0]),
        f64::from(hud.score[1]),
        f64::from(hud.goal),
        f64::from(phase),
        f64::from(hud.phase_ms),
        f64::from(hud.phase_total_ms),
        after,
        f64::from(hud.clock_ms),
        f64::from(hud.clock_total_ms),
        sudden,
        f64::from(hud.alive[own.index()]),
        f64::from(hud.total[own.index()]),
        f64::from(hud.alive[own.other().index()]),
        f64::from(hud.total[own.other().index()]),
        artefact,
        x,
        z,
        carrier,
        carrier_team,
        f64::from(u8::from(viewed_carries)),
        2.0,
        b0[0], b0[1], b0[2],
        b1[0], b1[1], b1[2],
    ]
}

/// A checksum of what the movie was told: the ms values as whole seconds.
fn checksum(values: &[f64; 28]) -> u32 {
    values.iter().enumerate().fold(0u32, |sum, (i, &value)| {
        let value = if matches!(i, 5 | 8) { f64::from(seconds(value as u32)) } else { value };
        let bits = value.to_bits();
        (sum.rotate_left(7) ^ (bits as u32)).rotate_left(11) ^ ((bits >> 32) as u32)
    })
}

/// The prompt for the viewed player: the take or the put his view reaches.
/// Whether Use by `player` would take the artefact or put it in now (the
/// hunt's prompt): patch 78's and 85's prompts and patch 85's pickup command
/// give way to it.
pub fn offers(player: &Player<'_>) -> bool {
    let Some(world) = HuntWorld::of_player(player) else { return false };
    hunt_in(player.thread(), world.key()).is_some_and(|hunt| prompt_of(&hunt, player) != Prompt::None)
}

fn prompt_of(hunt: &Hunt, player: &Player<'_>) -> Prompt {
    let Some(place) = Place::new(player.hunt_place()) else { return Prompt::None };
    if !player.alive() || !player.inserted() || hunt.state.frozen() {
        return Prompt::None;
    }
    let (eye, forward) = player.carry_eye();
    let view = View { eye, forward };
    // The level's ray only near what Use could reach: a frame far from it
    // asks the physics world nothing.
    let near = |at: [f32; 3], within: f32| {
        let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= within * within
    };
    match hunt.state.artefact(&hunt.params) {
        ArtefactView::Lying { at, .. } if near(at.metres(), REACH + 1.0)
            && hunt.params.takes(&view, at, level_along(player, &view)) => Prompt::Take,
        ArtefactView::Carried { by, team } if by == place && near(hunt.params.base(team).metres(), BASE_NEAR)
            && hunt.params.delivers(&view, team, level_along(player, &view)) => Prompt::Put,
        _ => Prompt::None,
    }
}

/// How near its base's point a carrier's eye must be for the prompt to ask
/// whether Use puts it in (its shapes lie within a few metres of the point).
const BASE_NEAR: f32 = 12.0;

/// A handler's start, timed under `SKHID_HUNT_LOG=on` (the third test's
/// games stalled 0.5 to 2.5 s: which code of ours ran then).
fn timer_start(thread: &Thread) -> Option<u64> {
    hunt_log(thread).then(|| crate::vostok::clock::counter(thread))
}

/// Its end: a line when it took 20 ms or more.
fn timer_end(thread: &Thread, started: Option<u64>, what: &str) {
    let Some(start) = started else { return };
    // In f32, as patch 85's elapsed_ms: the arena has no 64-bit division.
    let frequency = crate::vostok::clock::frequency(thread).max(1);
    let ms = (crate::vostok::clock::counter(thread).wrapping_sub(start) as f32 * 1000.0 / frequency as f32) as u32;
    if ms >= 20 {
        log::line(&[Piece::Text("artefact_hunt: "), Piece::Text(what), Piece::Text(" took "), Piece::Dec(ms),
                    Piece::Text(" ms, game "), Piece::Dec(crate::vostok::gun_cook::process_id())]);
    }
}

/// update_player_icons' start (every frame of a match, after the frame's
/// steps): the update call, the cues' messages, the prompt.
pub fn on_update_icons(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let started = timer_start(ctx.thread());
    let flow = update_icons(ctx);
    timer_end(ctx.thread(), started, "the hud's update");
    flow
}

fn update_icons(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(world) = HuntWorld::of_match(thread) else { return Flow::Continue };
    let Some(hunt) = hunt_in(thread, world.key()) else { return Flow::Continue };
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let Some(movie) = ui.hud_movie() else { return Flow::Continue };
    let mut shown = SHOWN.get(thread);
    let viewed = ui.viewed_place().and_then(|place| Place::new(place.get()));
    let viewed_player = viewed.and_then(|place| world.player(place.get()));
    let team = viewed.and_then(|place| hunt.state.sides().team_of(place))
        .or_else(|| viewed.and_then(|place| Team::from_raw(world.body(place.get())?.team)));
    let viewer = Viewer { place: viewed, team };
    let now = world.now();
    let hud = hunt.state.hud(&hunt.params, Time::from_ms(now));
    let viewed_carries = viewed.is_some() && hunt.state.carrier() == viewed;
    let values = update_values(&hud, &hunt.params, viewer, viewed_carries);
    let sum = checksum(&values);
    shown.frames += 1;
    if (shown.told != Some((movie.id(), sum)) || shown.frames >= RESEND_FRAMES)
        && movie.invoke_numbers(c"root.artefact_hunt_update", &values)
    {
        shown.told = Some((movie.id(), sum));
        shown.frames = 0;
    }
    // The cues: the rule shown last against the rule now (the first frame
    // of a rule only remembers).
    let cues = match shown.rule {
        Some((key, seen)) if key == hunt.rule => Cue::between(&seen, &hunt.state),
        _ => [None, None],
    };
    let messages = cues.map(|cue| cue.and_then(|cue| cue.message(viewer)));
    // The announcer: its lines asked once; a cue's line for the viewer said.
    ask_lines(thread, &hunt.item.lines);
    if let Some(game_world) = ui.game_world(thread) {
        for line in cues.into_iter().flatten().filter_map(|cue| cue.line(viewer)) {
            say(thread, &game_world, line);
        }
    }
    shown.rule = Some((hunt.rule, hunt.state));
    for message in shown.owed.take().into_iter().chain(messages.into_iter().flatten()) {
        let mut text = [0; 0x200];
        if !movie.show_message(ui.translate(message_key(message), &mut text), MESSAGE_MS) {
            shown.owed = Some(message);
        }
    }
    let prompt = viewed_player.map_or(Prompt::None, |player| prompt_of(&hunt, &player));
    // The key, as patches 78 and 85 give theirs: the movie formats it with
    // the Use key into the game's own context line (a translated text there
    // was no key of its dictionary: "#Pick up (E)").
    if shown.prompt != Some((movie.id(), prompt)) {
        let key = if prompt == Prompt::None { c"" } else { prompt.key() };
        if movie.invoke_text(c"root.show_artefact_offer", key) {
            shown.prompt = Some((movie.id(), prompt));
        }
    }
    SHOWN.set(thread, shown);
    Flow::Continue
}

/// initialize_match's start: the movie is told everything again.
pub fn on_initialize_match(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SHOWN.update(ctx.thread(), |shown| Shown { told: None, prompt: None, owed: None, ..shown });
    Flow::Continue
}

/// update_ui's mark of the welcome message: mode 13's text.
pub fn on_welcome(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let hunting = HuntWorld::of_match(thread).is_some_and(|world| hunt_in(thread, world.key()).is_some());
    if hunting && let Some(ui) = GameWorldUi::at_welcome(ctx) && let Some(movie) = ui.hud_movie() {
        let mut text = [0; 0x200];
        movie.show_message(ui.translate(c"st_start_match_welcome_message_13", &mut text), 6000);
    }
    Flow::Continue
}

// ---- the artefact in the world, the glow, the announcer, the mark ------------

/// A process-wide load: asked once, at the first frame or tick that needs it.
#[derive(Clone, Copy)]
enum Load<T> {
    Unasked,
    Asked,
    Ready(T),
    /// The query failed (the pack lacks it): nothing of it this session.
    Missing,
}

static ASSETS: GameCell<Load<ArrivedAssets>> = GameCell::new(Load::Unasked);
/// The carrier's glow as this game draws it: whose, and which of the two.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Glow {
    Off,
    On { place: Place, view: GlowView },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GlowView {
    /// On his body: everyone else's view of him.
    Body,
    /// At the camera: his own first-person view.
    FirstPerson,
}

impl GlowView {
    const fn particle(self) -> usize {
        match self {
            GlowView::Body => GLOW,
            GlowView::FirstPerson => GLOW_FP,
        }
    }
}

static GLOWING: GameCell<Glow> = GameCell::new(Glow::Off);

/// The announcer's lines as they arrived (by Line), and the one playing.
struct Announcer {
    asked: bool,
    lines: [Option<Emitter>; LINES],
    playing: Option<Voice>,
}

static ANNOUNCER: Lock<Announcer> =
    Lock::new(Announcer { asked: false, lines: [const { None }; LINES], playing: None });

/// The serial of line `index`'s query.
fn line_serial(index: usize) -> Serial {
    (0..index).fold(Serial::FIRST, |serial, _| serial.next())
}

/// The artefact as the world shows it: where it lies (and whether at a
/// spawner) and which one-shot particles were played (to stop before the
/// next), of the rule it is shown for.
#[derive(Clone, Copy, PartialEq)]
struct WorldView {
    rule: Option<RuleKey>,
    lying: Option<[f32; 3]>,
    spawned: bool,
    destroyed: bool,
}

impl WorldView {
    const NEW: WorldView = WorldView { rule: None, lying: None, spawned: false, destroyed: false };
}

static VIEW: GameCell<WorldView> = GameCell::new(WorldView::NEW);

const IDLE: usize = 0;
const SPAWN: usize = 1;
const DESTROY: usize = 2;
const GLOW: usize = 3;
const GLOW_FP: usize = 4;

/// The patch's queries came back (on the game's thread).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    match results.arrived(tag) {
        Arrived::Assets(assets) => {
            let whole = assets.model.is_some() && assets.particles[..GLOW].iter().all(Option::is_some);
            if !whole {
                log::line(&[Piece::Text("artefact_hunt: the artefact's model or a particle did not load")]);
            }
            ASSETS.set(thread, if assets.model.is_some() { Load::Ready(assets) } else { Load::Missing });
        }
        Arrived::Track(serial, emitter) => ANNOUNCER.with(thread, |announcer| {
            if let Some(index) = (0..LINES).find(|&index| line_serial(index) == serial) {
                announcer.lines[index] = emitter;
            }
        }),
        Arrived::Bus(_) | Arrived::Effects(_) | Arrived::Particles(_) => {}
    }
}

/// game_world::tick after the simulation's move: the artefact's model and
/// particles reconciled with the rule's state, each frame.
pub fn on_after_move(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let started = timer_start(ctx.thread());
    let flow = after_move(ctx);
    timer_end(ctx.thread(), started, "the frame's artefact and glow");
    flow
}

fn after_move(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(game_world) = GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(world) = HuntWorld::of_match(thread) else { return Flow::Continue };
    let Some(hunt) = hunt_in(thread, world.key()) else { return Flow::Continue };
    let assets = match ASSETS.get(thread) {
        Load::Unasked => {
            let item = &hunt.item;
            let [idle, spawn, destroy] = item.particles;
            let mut paths = [idle.get(), spawn.get(), destroy.get(), c"", c""];
            let count = match &item.glow {
                Some([body, first_person]) => {
                    paths[GLOW] = body.get();
                    paths[GLOW_FP] = first_person.get();
                    5
                }
                None => 3,
            };
            resources::query_assets(thread, item.model.get(), &paths[..count],
                                    crate::vostok::artefact_hunt_on_loaded_entry());
            ASSETS.set(thread, Load::Asked);
            return Flow::Continue;
        }
        Load::Ready(assets) => assets,
        Load::Asked | Load::Missing => return Flow::Continue,
    };
    let Some(scene) = ArtefactScene::of(&game_world, thread) else { return Flow::Continue };
    let mut view = VIEW.get(thread);
    if view.rule != Some(hunt.rule) {
        clear(&scene, &assets, view);
        stop_glow(&scene, &assets, thread);
        view = WorldView { rule: Some(hunt.rule), ..WorldView::NEW };
    }
    draw_glow(&scene, &assets, &hunt, &world, &game_world, thread);
    let lying = match hunt.state.artefact(&hunt.params) {
        ArtefactView::Lying { spot, at } => Some((matches!(spot, artefact_hunt_rule::Spot::Spawner(_)), at.metres())),
        _ => None,
    };
    let particle = |index: usize| assets.particles[index];
    if view.lying != lying.map(|(_, at)| at) {
        if let (Some(model), Some(_)) = (assets.model, view.lying) {
            scene.hide(model);
        }
        if view.lying.is_some() && let Some(idle) = particle(IDLE) {
            scene.stop(idle);
        }
        // Removed by its time (nobody carries it): its destroy particle.
        if let (Some(at), None, None) = (view.lying, lying, hunt.state.carrier())
            && let Some(destroy) = particle(DESTROY)
        {
            if view.destroyed {
                scene.stop(destroy);
            }
            scene.play(destroy, at);
            view.destroyed = true;
        }
        if let Some((at_spawner, at)) = lying {
            if let Some(model) = assets.model {
                scene.show(model, at);
            }
            if let Some(idle) = particle(IDLE) {
                scene.play(idle, at);
            }
            // Appeared at a spawner (not dropped): its spawn particle.
            if at_spawner && view.lying.is_none() && let Some(spawn) = particle(SPAWN) {
                if view.spawned {
                    scene.stop(spawn);
                }
                scene.play(spawn, at);
                view.spawned = true;
            }
        }
        view.lying = lying.map(|(_, at)| at);
    }
    VIEW.set(thread, view);
    Flow::Continue
}

/// The carrier's glow, drawn by this game for itself each frame (never in the
/// simulation: the 2026-11-21 test's carrier's game parted from the others'
/// while the glow went into his effect list at a tick that hung on when each
/// game had loaded it): on his body for the others, at the camera for him.
fn draw_glow(scene: &ArtefactScene<'_>, assets: &ArrivedAssets, hunt: &Hunt, world: &HuntWorld<'_>,
             game_world: &GameWorld<'_>, thread: &Thread) {
    let local = game_world.network_client().and_then(|client| client.local_place());
    let target = hunt.state.carrier()
        .filter(|place| world.body(place.get()).is_some_and(|body| body.alive))
        .map_or(Glow::Off, |place| Glow::On {
            place,
            view: if local == Some(place.get()) { GlowView::FirstPerson } else { GlowView::Body },
        });
    let shown = GLOWING.get(thread);
    let matrix = |place: Place, view: GlowView| {
        let player = world.player(place.get())?;
        let (body, eye) = native::transforms(&player);
        Some(if view == GlowView::Body { body } else { eye })
    };
    if shown != target {
        stop_glow(scene, assets, thread);
        if let Glow::On { place, view } = target
            && let (Some(particle), Some(at)) = (assets.particles[view.particle()], matrix(place, view))
        {
            scene.play_at(particle, &at);
        }
        GLOWING.set(thread, target);
    } else if let Glow::On { place, view } = target
        && let (Some(particle), Some(at)) = (assets.particles[view.particle()], matrix(place, view))
    {
        scene.move_to(particle, &at);
    }
}

fn stop_glow(scene: &ArtefactScene<'_>, assets: &ArrivedAssets, thread: &Thread) {
    if let Glow::On { view, .. } = GLOWING.get(thread)
        && let Some(particle) = assets.particles[view.particle()]
    {
        scene.stop(particle);
    }
    GLOWING.set(thread, Glow::Off);
}

/// Everything of the view out of the scene.
fn clear(scene: &ArtefactScene<'_>, assets: &ArrivedAssets, view: WorldView) {
    if view.lying.is_some() {
        if let Some(model) = assets.model {
            scene.hide(model);
        }
        if let Some(idle) = assets.particles[IDLE] {
            scene.stop(idle);
        }
    }
    for (index, played) in [(SPAWN, view.spawned), (DESTROY, view.destroyed)] {
        if played && let Some(particle) = assets.particles[index] {
            scene.stop(particle);
        }
    }
}

/// game_world::on_deactivate: the match's scene is left; nothing of ours in it.
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let view = VIEW.get(thread);
    if view.rule.is_some()
        && let Load::Ready(assets) = ASSETS.get(thread)
        && let Some(world) = GameWorld::at_deactivate(ctx)
        && let Some(scene) = ArtefactScene::of(&world, thread)
    {
        clear(&scene, &assets, view);
        stop_glow(&scene, &assets, thread);
    }
    VIEW.set(thread, WorldView::NEW);
    GLOWING.set(thread, Glow::Off);
    ANNOUNCER.with(thread, |announcer| announcer.playing = None);
    Flow::Continue
}

/// The announcer's folder for the game's language: 0.46c2's seven voices,
/// "original" russian (its 0x5D35B4), any other english.
fn voice_folder(language: &[u8]) -> &'static [u8] {
    match language {
        b"english" | b"french" | b"german" | b"polish" | b"russian" | b"spanish" | b"ukrainian" => {
            const VOICES: [&[u8]; 7] = [b"english", b"french", b"german", b"polish", b"russian", b"spanish", b"ukrainian"];
            VOICES.into_iter().find(|voice| *voice == language).unwrap_or(b"english")
        }
        b"original" => b"russian",
        _ => b"english",
    }
}

/// `<base>/<voice>/<name>` into `out`, as a C string.
fn line_path<'o>(base: &[u8], voice: &[u8], name: &[u8], out: &'o mut [u8; 2 * TEXT]) -> Option<&'o CStr> {
    let mut at = 0;
    for piece in [base, b"/", voice, b"/", name, b"\0"] {
        out.get_mut(at..at + piece.len())?.copy_from_slice(piece);
        at += piece.len();
    }
    CStr::from_bytes_until_nul(&out[..at]).ok()
}

/// The lines asked for at a hunt's first hud frame (once a process).
fn ask_lines(thread: &Thread, lines: &Lines) {
    let asked = ANNOUNCER.with(thread, |announcer| core::mem::replace(&mut announcer.asked, true));
    if asked {
        return;
    }
    let language = crate::vostok::language::current(thread);
    let voice = voice_folder(language.as_bytes());
    for (index, name) in lines.names.iter().enumerate() {
        let mut path = [0; 2 * TEXT];
        if let Some(path) = line_path(lines.base.get().to_bytes(), voice, name.get().to_bytes(), &mut path) {
            resources::query_collection(thread, path, line_serial(index), crate::vostok::artefact_hunt_on_loaded_entry());
        }
    }
}

/// A line said: the one playing stopped first (0.46c2's 0x69FA40).
fn say(thread: &Thread, world: &GameWorld<'_>, line: Line) {
    ANNOUNCER.with(thread, |announcer| {
        announcer.playing = None;
        if let Some(emitter) = &announcer.lines[line as usize] {
            announcer.playing = native::announce(world, thread, emitter);
        }
    });
}

/// The client time rule's camera test: the countdown's state while the core
/// says a round is about to start (any mode whose rule makes it so).
pub fn on_camera_state(ctx: &mut Context<'_>) -> Flow<ArtefactHuntOnCameraStateExit> {
    native::camera_state(ctx);
    Flow::Exit(ArtefactHuntOnCameraStateExit::State)
}

/// The icon record's carry test: the carrier's icon is marked, of either team.
pub fn on_mark(ctx: &mut Context<'_>) -> Flow<ArtefactHuntOnMarkExit> {
    let carrier = native::marked(ctx).is_some_and(|player| carries(&player));
    if carrier {
        native::mark_carrier(ctx);
        return Flow::Exit(ArtefactHuntOnMarkExit::Carrier);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_update_has_28_values_in_the_contracts_order() {
        let spawners = [Spawner { position: [1.0, 0.0, 2.0] }];
        let crate_shape = [RawShape { kind: 1, position: [0.0, 0.5, 0.0], axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], half: [0.5; 3] }];
        let bases = [
            BaseDef { team: 0, position: [0.0, 0.0, 5.0], shapes: &crate_shape },
            BaseDef { team: 1, position: [50.0, 0.0, -7.0], shapes: &crate_shape },
        ];
        let raw = RawParams {
            match_time: 900, time_needed_to_continue_match: 90, draw_additional_time: 180, time_before_new_round: 3,
            round_countdown_time: 5, score: 5, respawn_time: 15, time_needed_to_respawn: 7, vi_spawn_time: 60,
            vi_spawn_variation: 10, vi_not_picked_ttl: 120, vi_dropped_ttl: 60, drop_lift: 0.13, usable_radius: 0.3,
        };
        let params = Params::parse(&raw, &spawners, &bases).unwrap();
        let rule = Rule::new(1);
        let hud = rule.hud(&params, Time::from_ms(0));
        let viewer = Viewer { place: None, team: Some(Team::One) };
        let values = update_values(&hud, &params, viewer, false);
        assert_eq!(values[0], 1.0);
        assert_eq!(values[3], 5.0);
        assert_eq!(values[4], 0.0, "before the start");
        assert_eq!((values[15], values[18], values[19]), (0.0, 255.0, 2.0));
        assert_eq!(values[21], 2.0);
        assert_eq!(values[22..28], [0.0, 0.0, -5.0, 1.0, 50.0, 7.0]);
        let none = Viewer { place: None, team: None };
        assert_eq!(update_values(&hud, &params, none, false)[0], 3.0);
        // Phase ids are 0.46c2's.
        assert_eq!(artefact_hunt_rule::Phase::Countdown as u8, 8);
        // The checksum takes the ms values in whole seconds.
        let mut shown = values;
        shown[8] = 5000.0;
        let mut later = shown;
        later[8] = 4600.0;
        assert_eq!(checksum(&shown), checksum(&later));
        later[8] = 3600.0;
        assert_ne!(checksum(&shown), checksum(&later));
    }
}
