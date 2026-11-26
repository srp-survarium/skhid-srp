//! What differs between two worlds of one tick: each part of the body that
//! differs (the mask, each place's state and animation, each of the six
//! sections), in the body's order, with the first object of the part and
//! the field of it that differ, read with srp-validate's typed views (the
//! world format's one definition, `packet-validation.md`).
//!
//! The desync capture writes it beside the two worlds it asked for at a
//! split (`game/desync_capture.rs`, `docs/notes/desync_capture.md`);
//! `srp-world-diff` reads two saved worlds with it.
//!
//! A field is named by its path through the view's `Debug` form
//! (`rest.controller.position.1`): every view the validator reads is
//! there, with the game's own field names, and nothing is named twice.
//! What the views do not read (an animation's bit stream, the physics
//! section, the items of a place whose kits are not known) is given by its
//! first differing byte only.

use std::fmt;

use serde::{Serialize, Serializer};
use srp_validate::world::{
    Bullets, Context, Effects, GameObjects, PlayerState, Rules, Section, Statistics,
};
use survarium::v026g::match_msgs::players;
use world_frame::{Frame, Places, Section as Kind};

/// A part of a world body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The whole body: one of the two does not walk, so it is compared as
    /// bytes.
    Body,
    /// The players mask: a place is in one world only.
    Mask,
    State(u8),
    Animation(u8),
    Section(Kind),
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Body => f.write_str("body"),
            Part::Mask => f.write_str("mask"),
            Part::State(place) => write!(f, "place {place} state"),
            Part::Animation(place) => write!(f, "place {place} animation"),
            Part::Section(kind) => f.write_str(kind.name()),
        }
    }
}

fn as_text<T: fmt::Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// The first field of a part that differs, as its typed view reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Field {
    /// The object of the part: `head`, `item 3`, `rest`, `bullet 2`,
    /// `place 4`, `object 7`, `rules`...
    pub object: String,
    /// The field's path in it, its names and indices joined by dots; empty
    /// when the object is in one world only.
    pub path: String,
    /// Its value in each world, as the view shows it.
    pub values: [String; 2],
}

/// One part of the body that differs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    #[serde(serialize_with = "as_text")]
    pub part: Part,
    /// Its first differing byte, in each body.
    pub at: [usize; 2],
    /// The same, from the start of the part.
    pub in_part: usize,
    /// The part's length in each world.
    pub lengths: [usize; 2],
    /// Its first differing field, when the typed views read the part of
    /// both worlds and show the difference.
    pub field: Option<Field>,
}

/// What differs between two worlds, in the body's order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct WorldDiff {
    pub differences: Vec<Difference>,
}

impl WorldDiff {
    /// The first part that differs.
    pub fn first(&self) -> Option<&Difference> {
        self.differences.first()
    }

    /// The first difference with a field named, else the first.
    pub fn first_field(&self) -> Option<&Difference> {
        self.differences
            .iter()
            .find(|difference| difference.field.is_some())
            .or_else(|| self.first())
    }
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at +{}", self.part, self.in_part)?;
        if let Some(field) = &self.field {
            write!(f, ", {}", field.object)?;
            if !field.path.is_empty() {
                write!(f, ".{}", field.path)?;
            }
            write!(f, " {} vs {}", field.values[0], field.values[1])?;
        }
        Ok(())
    }
}

/// What differs between the worlds `a` and `b` (bodies from the mask on).
///
/// `contexts` are the match the worlds are of, as the server checks its
/// uploads (`world_validation::with_contexts`): the first for every part,
/// and each in turn for the game objects, whose list depends on the late
/// joiners a game installed. With none, a context of `places` places and
/// nothing else is used: each place's item count is the first that reads
/// both states, and the items past the inventory header are bytes.
pub fn diff(a: &[u8], b: &[u8], contexts: &[Context<'_>]) -> WorldDiff {
    let mut differences = Vec::new();
    let (Ok(frame_a), Ok(frame_b)) = (Frame::parse(a, Places::ALL), Frame::parse(b, Places::ALL))
    else {
        differences.extend(bytes(Part::Body, a, b, [0, 0], None));
        return WorldDiff { differences };
    };
    let (Some((blocks_a, _)), Some((blocks_b, _))) = (players::walk(a), players::walk(b)) else {
        differences.extend(bytes(Part::Body, a, b, [0, 0], None));
        return WorldDiff { differences };
    };
    let fallback = guessed_context(frame_a.slots().mask() | frame_b.slots().mask());
    let context = contexts.first().unwrap_or(&fallback);
    let mask = |body: &[u8]| body.get(..4).unwrap_or(&[]).to_vec();
    if frame_a.slots() != frame_b.slots() {
        differences.extend(bytes(Part::Mask, &mask(a), &mask(b), [0, 0], None));
    }
    let offset =
        |body: &[u8], part: &[u8]| (part.as_ptr() as usize).saturating_sub(body.as_ptr() as usize);
    for block_a in &blocks_a {
        let Some(block_b) = blocks_b.iter().find(|block| block.place == block_a.place) else {
            continue;
        };
        let place = block_a.place;
        let at = [offset(a, block_a.state), offset(b, block_b.state)];
        let field = || state_field(place, block_a.state, block_b.state, context);
        differences.extend(bytes(
            Part::State(place),
            block_a.state,
            block_b.state,
            at,
            Some(&field),
        ));
        let (anim_a, anim_b) = (
            block_a.animation.unwrap_or(&[]),
            block_b.animation.unwrap_or(&[]),
        );
        if block_a.animation.is_some() != block_b.animation.is_some() {
            differences.push(Difference {
                part: Part::Animation(place),
                at: [offset(a, anim_a), offset(b, anim_b)],
                in_part: 0,
                lengths: [anim_a.len(), anim_b.len()],
                field: Some(Field {
                    object: "animation".into(),
                    path: String::new(),
                    values: [
                        present(block_a.animation.is_some()),
                        present(block_b.animation.is_some()),
                    ],
                }),
            });
        } else {
            let at = [offset(a, anim_a), offset(b, anim_b)];
            differences.extend(bytes(Part::Animation(place), anim_a, anim_b, at, None));
        }
    }
    for kind in Kind::ALL {
        let (section_a, section_b) = (frame_a.section(kind), frame_b.section(kind));
        let field = || section_field(kind, section_a, section_b, context, contexts);
        let at = [section_a.at, section_b.at];
        differences.extend(bytes(
            Part::Section(kind),
            section_a.bytes,
            section_b.bytes,
            at,
            Some(&field),
        ));
    }
    WorldDiff { differences }
}

fn present(here: bool) -> String {
    if here { "present" } else { "absent" }.into()
}

/// A context with only the places the mask names.
fn guessed_context(mask: u32) -> Context<'static> {
    Context {
        time: 0,
        places: (32 - mask.leading_zeros()) as usize,
        game_mode: 0,
        batteries: 0,
        respawn_points: None,
        team_2_points: None,
        profiles: None,
        artefacts: None,
        objects: None,
        inventory: None,
        dict_ids: None,
        artefact_hunt: None,
    }
}

/// The difference of a part, if its bytes differ: where, and its field.
fn bytes(
    part: Part,
    a: &[u8],
    b: &[u8],
    at: [usize; 2],
    field: Option<&dyn Fn() -> Option<Field>>,
) -> Option<Difference> {
    let in_part = first_byte(a, b)?;
    Some(Difference {
        part,
        at: [at[0] + in_part, at[1] + in_part],
        in_part,
        lengths: [a.len(), b.len()],
        field: field.and_then(|field| field()),
    })
}

/// The first offset at which `a` and `b` differ, the shorter one's end if
/// one is the other's start; `None` if they are equal.
pub fn first_byte(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
}

/// One object of a part as a typed view reads it: its name and its pretty
/// `Debug` form.
struct Piece {
    object: String,
    text: String,
}

fn piece(object: impl Into<String>, view: &impl fmt::Debug) -> Piece {
    Piece {
        object: object.into(),
        text: format!("{view:#?}"),
    }
}

/// The first field that differs between two lists of pieces.
fn first_field(a: &[Piece], b: &[Piece]) -> Option<Field> {
    for index in 0..a.len().max(b.len()) {
        match (a.get(index), b.get(index)) {
            (Some(x), Some(y)) if x.object == y.object => {
                if let Some((path, values)) = debug_path_diff(&x.text, &y.text) {
                    return Some(Field {
                        object: x.object.clone(),
                        path,
                        values,
                    });
                }
            }
            (x, y) => {
                let object = x
                    .or(y)
                    .map(|piece| piece.object.clone())
                    .unwrap_or_default();
                return Some(Field {
                    object,
                    path: String::new(),
                    values: [present(x.is_some()), present(y.is_some())],
                });
            }
        }
    }
    None
}

/// Fields that say where a view is in the body, not what the game holds:
/// they move with any length before them.
fn is_offset(key: &str) -> bool {
    key == "at" || key.ends_with("_at")
}

/// The path and both values of the first line that differs between two
/// pretty `Debug` forms (`{:#?}`), its offset fields left out.
///
/// The form puts each field, element or variant on a line of its own,
/// indented four spaces a level, and a line that ends in `{`, `[` or `(`
/// opens the next level. The path is the names (`key: `) and the element
/// indices of the lines that opened each level.
fn debug_path_diff(a: &str, b: &str) -> Option<(String, [String; 2])> {
    struct Line<'t> {
        level: usize,
        text: &'t str,
    }
    fn lines(text: &str) -> Vec<Line<'_>> {
        text.lines()
            .map(|line| {
                let text = line.trim_start();
                Line {
                    level: (line.len() - text.len()) / 4,
                    text,
                }
            })
            .collect()
    }
    let (lines_a, lines_b) = (lines(a), lines(b));
    // The label of each open level, and the next element index of each.
    let mut path: Vec<String> = Vec::new();
    // The text of the line that opened each level.
    let mut openers: Vec<&str> = Vec::new();
    let mut counters: Vec<usize> = vec![0];
    let mut skip_below: Option<usize> = None;
    for index in 0..lines_a.len().max(lines_b.len()) {
        let (line_a, line_b) = (lines_a.get(index), lines_b.get(index));
        let Some(line) = line_a.or(line_b) else {
            break;
        };
        let level = line.level;
        path.truncate(level);
        openers.truncate(level);
        counters.truncate(level + 1);
        if counters.len() < level + 1 {
            counters.resize(level + 1, 0);
        }
        if skip_below.is_some_and(|skip| level > skip) {
            continue;
        }
        skip_below = None;
        let closing = line.text.starts_with(['}', ']', ')']);
        if closing {
            continue;
        }
        let (label, key) = match line.text.split_once(": ") {
            Some((key, _))
                if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') =>
            {
                (key.to_owned(), true)
            }
            // The view itself: no name of its own.
            _ if level == 0 => (String::new(), false),
            _ => {
                let count = counters[level];
                counters[level] += 1;
                (count.to_string(), false)
            }
        };
        let opens = line.text.ends_with(['{', '[', '(']);
        if key && is_offset(&label) {
            if opens {
                skip_below = Some(level);
            }
            continue;
        }
        let same = matches!((line_a, line_b), (Some(x), Some(y)) if x.level == y.level && x.text == y.text);
        if !same {
            // An f32 kept as its bits (`srp_validate::Finite`): its value.
            let float = !key
                && openers
                    .last()
                    .is_some_and(|opener| opener.ends_with("Finite("));
            let mut full = path.clone();
            if !float && !(opens && !key && line.text.len() == 1) {
                full.push(label);
            }
            let value = |line: Option<&Line>| {
                line.map_or_else(
                    || "absent".to_owned(),
                    |line| {
                        let text = line.text.trim_end_matches(',');
                        let text = text
                            .split_once(": ")
                            .filter(|_| key)
                            .map_or(text, |(_, v)| v);
                        match text.parse::<u32>() {
                            Ok(bits) if float => format!("{} ({bits:#010x})", f32::from_bits(bits)),
                            _ => text.chars().take(160).collect(),
                        }
                    },
                )
            };
            full.retain(|label| !label.is_empty());
            return Some((full.join("."), [value(line_a), value(line_b)]));
        }
        if opens {
            path.push(label);
            openers.push(line.text);
            counters.push(0);
        }
    }
    None
}

/// The item count of a place's head: its profile's, else the first count
/// with which both states read.
fn items_of(place: u8, a: &[u8], b: &[u8], context: &Context<'_>) -> Option<usize> {
    if let Some(profiles) = context.profiles {
        return profiles
            .get(usize::from(place))
            .map(|profile| profile.items);
    }
    (0..=69).find(|items| {
        PlayerState::read(a, *items, context.artefacts).is_ok()
            && PlayerState::read(b, *items, context.artefacts).is_ok()
    })
}

/// A place's state as pieces: the head, its effects, the active objects,
/// the stamina, the inventory header; with the kits, each item and the
/// rest of the state (the body, its damage model, the controller).
fn state_pieces(
    place: u8,
    state: &[u8],
    items: usize,
    context: &Context<'_>,
) -> Option<Vec<Piece>> {
    #[derive(Debug)]
    #[allow(dead_code)] // read through `Debug`
    struct ActiveObjects {
        current: Option<u8>,
        target: Option<u8>,
    }
    #[derive(Debug)]
    #[allow(dead_code)] // read through `Debug`
    struct Item<'a> {
        left: srp_validate::world::Left,
        owner: Option<srp_validate::world::ItemOwner>,
        body: srp_validate::world::ItemBody<'a>,
    }
    let read = PlayerState::read(state, items, context.artefacts).ok()?;
    let mut pieces = vec![piece("head", read.head())];
    pieces.extend(
        read.effects()
            .enumerate()
            .map(|(index, effect)| piece(format!("effect {index}"), &effect)),
    );
    pieces.extend(
        read.hit_effects()
            .enumerate()
            .map(|(index, effect)| piece(format!("hit effect {index}"), &effect)),
    );
    pieces.push(piece(
        "active objects",
        &ActiveObjects {
            current: read.current_active_object(),
            target: read.target_active_object(),
        },
    ));
    pieces.push(piece("stamina", &read.stamina()));
    pieces.push(piece("inventory header", read.inventory()));
    if let Ok(Some(rest)) = read.items_and_rest(place, context) {
        pieces.extend(rest.items().enumerate().map(|(index, item)| {
            piece(
                format!("item {index}"),
                &Item {
                    left: item.left(),
                    owner: item.owner(),
                    body: item.body(),
                },
            )
        }));
        pieces.push(piece("rest", rest.rest()));
    }
    Some(pieces)
}

fn state_field(place: u8, a: &[u8], b: &[u8], context: &Context<'_>) -> Option<Field> {
    let items = items_of(place, a, b, context)?;
    let field = first_field(
        &state_pieces(place, a, items, context)?,
        &state_pieces(place, b, items, context)?,
    );
    if field.is_some() || context.inventory.is_some() {
        return field;
    }
    // Equal to the inventory header: the difference is in the items, which
    // only the kits read.
    let header = |state: &[u8]| {
        let read = PlayerState::read(state, items, context.artefacts).ok()?;
        Some(format!("{:?}", read.inventory()))
    };
    (header(a)? == header(b)?).then(|| Field {
        object: "items (the kits unknown)".into(),
        path: String::new(),
        values: ["?".into(), "?".into()],
    })
}

fn section_pieces(kind: Kind, section: Section<'_>, context: &Context<'_>) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    match kind {
        Kind::Bullets => {
            let bullets = Bullets::read(&section).ok()?;
            pieces.extend(
                bullets
                    .iter()
                    .enumerate()
                    .map(|(index, bullet)| piece(format!("bullet {index}"), &bullet)),
            );
            pieces.push(piece("header", &bullets));
        }
        Kind::Statistics => {
            let batteries = if srp_validate::mode::WITH_BATTERIES.contains(&context.game_mode) {
                context.batteries
            } else {
                0
            };
            let statistics = Statistics::read(&section, context.places, batteries).ok()?;
            pieces.extend(
                statistics
                    .players()
                    .enumerate()
                    .map(|(place, player)| piece(format!("place {place}"), &player)),
            );
            pieces.extend(
                statistics
                    .batteries()
                    .enumerate()
                    .map(|(index, battery)| piece(format!("battery {index}"), &battery)),
            );
            pieces.push(piece("tail", &statistics.tail()));
            pieces.push(piece("weapons", &statistics.weapons()));
        }
        Kind::Objects => {
            let objects = GameObjects::read(&section, context.objects?).ok()?;
            pieces.extend(
                objects
                    .objects()
                    .enumerate()
                    .map(|(index, object)| piece(format!("object {index}"), &object)),
            );
        }
        Kind::Rules => pieces.push(piece("rules", &Rules::read(&section, context).ok()?)),
        Kind::Effects => pieces.push(piece("effects", &Effects::read(&section).ok()?)),
        Kind::Physics => return None,
    }
    Some(pieces)
}

fn section_field(
    kind: Kind,
    a: Section<'_>,
    b: Section<'_>,
    context: &Context<'_>,
    contexts: &[Context<'_>],
) -> Option<Field> {
    if kind == Kind::Objects {
        // The first list of objects that reads both.
        return contexts.iter().find_map(|context| {
            first_field(
                &section_pieces(kind, a, context)?,
                &section_pieces(kind, b, context)?,
            )
        });
    }
    first_field(
        &section_pieces(kind, a, context)?,
        &section_pieces(kind, b, context)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_debug_form_names_its_first_differing_field_by_path() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Inner {
            at: usize,
            position: [f32; 3],
        }
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Outer {
            alive: bool,
            list: Vec<Inner>,
            kit: Option<u8>,
        }
        let make = |at, y, kit| Outer {
            alive: true,
            list: vec![
                Inner {
                    at: 1,
                    position: [0.0, 1.0, 2.0],
                },
                Inner {
                    at,
                    position: [0.0, y, 2.0],
                },
            ],
            kit,
        };
        let a = format!("{:#?}", make(5, 1.0, Some(1)));
        // Only the offset differs: nothing.
        assert_eq!(
            debug_path_diff(&a, &format!("{:#?}", make(9, 1.0, Some(1)))),
            None
        );
        assert_eq!(
            debug_path_diff(&a, &format!("{:#?}", make(9, 1.5, Some(1)))),
            Some(("list.1.position.1".into(), ["1.0".into(), "1.5".into()]))
        );
        assert_eq!(
            debug_path_diff(&a, &format!("{:#?}", make(5, 1.0, None))),
            Some(("kit".into(), ["Some(".into(), "None".into()]))
        );
    }

    #[test]
    fn equal_worlds_have_no_difference_and_unreadable_ones_differ_as_bytes() {
        let body = include_bytes!("../../tests/data/desync_221600_a.bin");
        assert_eq!(diff(body, body, &[]), WorldDiff::default());
        let mut other = body.to_vec();
        other.truncate(10);
        let found = diff(body, &other, &[]);
        assert_eq!(
            found.first().map(|d| (d.part, d.in_part)),
            Some((Part::Body, 10))
        );
    }

    /// Live, 2026-11-25, match 0x421b579c (Freeze Tag): the split capture
    /// of that time asked place 8's game (out of step) and place 0's (the
    /// majority's) for their worlds, and both gave tick 221600. They
    /// differ in place 6's state alone, and its animation: the step's
    /// `m_linear_horizontal_speed`, which the rest of the state after the
    /// items holds. Place 6's kits as the match's own roster made them
    /// (`world_validation::kits`); its profile has 15 items.
    #[test]
    fn two_recorded_worlds_of_a_split_name_the_field_the_games_first_differ_in() {
        use srp_validate::world::{
            Ammo, Body, Chamber, Gun, GunState, Inventory, Item, Measure, Profile,
        };
        let a = include_bytes!("../../tests/data/desync_221600_a.bin");
        let b = include_bytes!("../../tests/data/desync_221600_b.bin");

        // Without the match: read to the inventory header, which is equal.
        let found = diff(a, b, &[]);
        let parts: Vec<Part> = found.differences.iter().map(|d| d.part).collect();
        assert_eq!(parts, [Part::State(6), Part::Animation(6)]);
        let first = found.first().unwrap();
        assert_eq!(
            (first.in_part, first.at, first.lengths),
            (426, [5241, 5241], [848, 848])
        );
        assert_eq!(
            first.field.as_ref().unwrap().object,
            "items (the kits unknown)"
        );

        // With place 6's kits: the field.
        use GunState::{Fire, FireTwice, Flag, FlagByte, Melee, ThrowGrenade};
        const STATES: [GunState; 7] = [
            FlagByte,
            FlagByte,
            Flag,
            FlagByte,
            Fire,
            ThrowGrenade,
            Melee,
        ];
        const STATES_2: [GunState; 7] = [
            FlagByte,
            FlagByte,
            Flag,
            FlagByte,
            FireTwice,
            ThrowGrenade,
            Melee,
        ];
        let kit = |states: &'static [GunState]| -> Vec<Item<'static>> {
            let plain = |slot| Item {
                slot,
                measure: Measure::Condition,
                body: Body::None,
            };
            let mut items: Vec<Item> = [0, 2, 3, 4, 5, 6].into_iter().map(plain).collect();
            items.push(Item {
                slot: 7,
                measure: Measure::Condition,
                body: Body::Weapon(Gun {
                    states,
                    chamber: Chamber::NotSaved,
                    ammo: Ammo::Kit(1),
                }),
            });
            items.push(Item {
                slot: 8,
                measure: Measure::Amount,
                body: Body::None,
            });
            items.push(Item {
                slot: 13,
                measure: Measure::Amount,
                body: Body::Medkit { influences: 8 },
            });
            items.push(Item {
                slot: 14,
                measure: Measure::Amount,
                body: Body::GrenadeSet,
            });
            items.push(Item {
                slot: 15,
                measure: Measure::Amount,
                body: Body::None,
            });
            items
        };
        let (first_kit, last_kit) = (kit(&STATES), kit(&STATES_2));
        let mut kits: Vec<[&[Item]; 3]> = vec![[&[], &[], &[]]; 20];
        kits[6] = [&first_kit, &first_kit, &last_kit];
        let mut profiles = [Profile::default(); 20];
        profiles[6] = Profile {
            usable_kits: 0b111,
            items: 15,
            ..Profile::default()
        };
        let inventory = Inventory {
            kits: &kits,
            body_parts: survarium::v026g::item_params::body_parts(),
        };
        let context = Context {
            time: 221_600,
            profiles: Some(&profiles),
            inventory: Some(&inventory),
            game_mode: srp_validate::mode::FREEZE_TAG,
            ..guessed_context(u32::MAX >> 12)
        };
        let found = diff(a, b, &[context]);
        let first = found.first_field().unwrap();
        assert_eq!(first.part, Part::State(6));
        assert_eq!(first.in_part, 426);
        let field = first.field.as_ref().unwrap();
        assert_eq!(
            (field.object.as_str(), field.path.as_str()),
            ("rest", "linear_horizontal_speed")
        );
        assert_eq!(
            field.values,
            [
                "2.0735257 (0x4004b4a5)".to_owned(),
                "1.9975309 (0x3fffaf18)".to_owned()
            ]
        );
        assert_eq!(
            first.to_string(),
            "place 6 state at +426, rest.linear_horizontal_speed 2.0735257 (0x4004b4a5) vs 1.9975309 (0x3fffaf18)"
        );
    }
}
