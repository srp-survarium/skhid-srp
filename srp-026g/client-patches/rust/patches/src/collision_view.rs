//! Patch 156: the console command `collision` draws the level's collision
//! geometry near the camera as lines, each filter group in its own colour
//! (docs/notes/collision-view.md; client_patch.md section 3.112).
//!
//! The command (a cc_delegate of ours, as patch 133's `where`) turns the view
//! on and off; `collision walk` shows what stops walkers (the walk and solid
//! groups), `shot` what stops bullets and the camera (shot and solid), `solid`
//! the solid group alone (the same name again turns it off). While it is
//! on, each frame's camera (`camera.viewed`, in mt_draw after the
//! simulation, on the thread that steps the physics world) outlines the
//! shapes of the camera's player's physics world near the eye: a mesh's
//! triangle edges (its BVH asked for the triangles near the eye), a box's,
//! sphere's, capsule's and cylinder's outline where the object stands, a
//! compound's children. The render thread's next frame draws them once,
//! among the game's own debug lines (`render.debug_lines`). The first frame
//! after the view is turned on tells how many objects of each group the
//! world holds, a line a group.
//!
//! Only the level's collision is drawn: static objects of the walkers' group
//! (2), the bullets' (8) or both (0Ah), the map's `collision_objects`. A
//! player's bodies (the hit model's group 40h, the character controller's
//! 824h) and whatever moves (a battery 0A18h, a grenade 818h) never are, so
//! the view shows nothing the map does not.
//!
//! The whole view only where the match shown allows it (the owner,
//! 2026-11-28: "collision fill only works in private lobbies", then: not
//! the line view everywhere either): a private room's match, which the
//! server says with srp's block's entry 8 after the options
//! (`static_info.options_read`; tag 8, one byte 1), or any replay
//! (`replay.options_read`: not live). The options last read decide, as for
//! patches 65 and 142; none read yet, or a match without the entry (an open
//! room's, a clan match, Player vs Bots, an older server's): nothing. There
//! the command turns nothing on (it still turns a view off); a view on from
//! before draws nothing in such a match, the console told once, and comes
//! back in the next match that allows it.

#![forbid(unsafe_code)]

use core::f32::consts::PI;
use core::ffi::CStr;

use ::ragdoll::math::{Pose, Vec3, sin_cos, sqrt};

use crate::vostok::camera;
use crate::vostok::console::{self, Command, ConsoleArguments, ConsoleCommand, Line};
use crate::vostok::debug_lines::{DebugLines, LineBuffer, LineColor};
use crate::vostok::geometry::{Distance, WorldPoint, box_within};
use crate::vostok::log::{self, Piece};
use crate::vostok::parse::Reader;
use crate::vostok::shape::{Shape, Triangle, TriangleMethods, TriangleSink};
use crate::vostok::project::PhysicsId;
use crate::vostok::world::{Motion, PhysicsWorld, ZoneState, for_each_damage_zone, match_shown};
use crate::vostok::{Context, Flow, NoExit, OptionsRead, OrigCcDelegateExecute, OrigTriangleProcess, Thread,
                    YieldingLock, collision_view_methods_vtable_handle, collision_view_triangles_vtable_handle};

const NAME: &CStr = c"collision";

static COMMAND: Command = Command::new();

/// Held by the render thread through its draws: a yielding lock.
static STATE: YieldingLock<Collision> =
    YieldingLock::new(Collision { view: View::Off, rooms: Rooms::Unmade, shown: Shown::Public });

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`.
const SRPX: [u8; 4] = *b"SRPX";
/// A private room's match: one byte, 1 (the server's `SRPX_PRIVATE_ROOM`).
const TAG_PRIVATE_ROOM: u8 = 8;

/// The match the options last read are of, as the view sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    /// A live match without the private room entry (an open room's, a clan
    /// match, Player vs Bots, an older server's), or none read yet.
    Public,
    /// A live match of a room with a PIN: the server's entry.
    PrivateRoom,
    /// A replay, of any match.
    Replay,
}

impl Shown {
    /// What a match's options say: srp's block (the bytes after the
    /// options' squads) with the private room entry, or not; an entry of
    /// another tag or length is stepped over, a cut one ends the block.
    fn of_match(mut rest: Reader) -> Shown {
        if rest.array::<4>() != Some(SRPX) {
            return Shown::Public;
        }
        let mut shown = Shown::Public;
        while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
            let Some(entry) = rest.take(usize::from(len)) else { break };
            if (tag, entry) == (TAG_PRIVATE_ROOM, &[1][..]) {
                shown = Shown::PrivateRoom;
            }
        }
        shown
    }

    /// Whether the view may be on in it.
    fn allows(self) -> bool {
        match self {
            Shown::PrivateRoom | Shown::Replay => true,
            Shown::Public => false,
        }
    }
}

/// The radii a read tries (m), the largest whose lines fit the room drawn.
const RADII: [f32; 3] = [20.0, 10.0, 5.0];
/// The vertices a room holds (4 MiB; two rooms, made the first time the view
/// is turned on and kept): lines take two, filled triangles three.
const VERTICES: usize = 256 * 1024;
const MOST: usize = VERTICES / 2;
/// A circle's segments.
const CIRCLE: usize = 12;
/// The eye moved this far (m) since the last read: read again.
const MOVE: f32 = 0.5;
/// A filled surface's alpha (of 255): translucent, the scene seen through it.
const FILL_ALPHA: u8 = 77;

/// A group of the level's collision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LevelGroup {
    /// `walk`, group 2: what walkers meet and bullets pass (walker meshes,
    /// invisible walls, walkable lids).
    Walk,
    /// `shot`, group 8: what bullets and the camera meet and walkers pass
    /// (the hittable meshes, camera walls).
    Shot,
    /// `solid`, group 0Ah: both.
    Solid,
    /// `anomaly`: the anomalies' damage zones (not a filter group: their
    /// sensors' shapes).
    Anomaly,
}

const GROUPS: [LevelGroup; 4] = [LevelGroup::Walk, LevelGroup::Shot, LevelGroup::Solid, LevelGroup::Anomaly];

impl LevelGroup {
    fn of(group: u16) -> Option<LevelGroup> {
        match group {
            0x2 => Some(LevelGroup::Walk),
            0x8 => Some(LevelGroup::Shot),
            0xA => Some(LevelGroup::Solid),
            _ => None,
        }
    }

    fn index(self) -> usize {
        match self {
            LevelGroup::Walk => 0,
            LevelGroup::Shot => 1,
            LevelGroup::Solid => 2,
            LevelGroup::Anomaly => 3,
        }
    }

    fn name(self) -> &'static str {
        ["walk", "shot", "solid", "anomaly"][self.index()]
    }

    /// Green, magenta and white: the same whichever of red and blue the
    /// vertex format puts first; red for an anomaly (LIKELY red: the
    /// format's red is the low byte).
    fn color(self) -> LineColor {
        [LineColor::opaque(0, 255, 0), LineColor::opaque(255, 0, 255), LineColor::opaque(255, 255, 255), ACTIVE_ZONE]
            [self.index()]
    }

    fn color_name(self) -> &'static str {
        ["green", "magenta", "white", "red"][self.index()]
    }

    fn meets(self) -> &'static str {
        ["stops walkers only", "stops bullets and the camera only", "stops walkers, bullets and the camera",
         "damage zones, dim red while off"][self.index()]
    }
}

/// Whether `group` is drawn under the filter `only` (none: every group): a
/// filter shows everything that stops what it names, so `walk` and `shot`
/// show `solid` too.
fn shows(only: Option<LevelGroup>, group: LevelGroup) -> bool {
    match only {
        None => true,
        Some(LevelGroup::Anomaly) => group == LevelGroup::Anomaly,
        Some(only) => only == group || group == LevelGroup::Solid,
    }
}

/// An anomaly's damage zone while it damages, and while it is off.
const ACTIVE_ZONE: LineColor = LineColor::opaque(255, 0, 0);
const OFF_ZONE: LineColor = LineColor::opaque(96, 0, 0);

/// The level group of a static object of one; none for anything else.
fn level_group(group: u16, motion: Motion) -> Option<LevelGroup> {
    match motion {
        Motion::Static => LevelGroup::of(group),
        Motion::Moving => None,
    }
}

/// Lines only, or the surfaces filled under them too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fill {
    Lines,
    Filled,
}

/// What is shown: the groups (none: every group) and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Filter {
    only: Option<LevelGroup>,
    fill: Fill,
}

impl Filter {
    const ALL: Filter = Filter { only: None, fill: Fill::Lines };

}

/// What the command was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    /// No argument.
    Toggle,
    /// A group's name, `fill`, or both (the name first).
    Show(Filter),
    Unknown,
}

impl Request {
    fn parse(text: &[u8]) -> Request {
        let mut words = text.split(u8::is_ascii_whitespace).filter(|word| !word.is_empty());
        let group = |word: &[u8]| GROUPS.into_iter().find(|group| word.eq_ignore_ascii_case(group.name().as_bytes()));
        let fill = |word: &[u8]| word.eq_ignore_ascii_case(b"fill");
        match (words.next(), words.next(), words.next()) {
            (None, ..) => Request::Toggle,
            (Some(word), None, _) if fill(word) => Request::Show(Filter { only: None, fill: Fill::Filled }),
            (Some(word), None, _) => group(word).map_or(Request::Unknown, |g| {
                Request::Show(Filter { only: Some(g), fill: Fill::Lines })
            }),
            (Some(word), Some(second), None) if fill(second) => group(word).map_or(Request::Unknown, |g| {
                Request::Show(Filter { only: Some(g), fill: Fill::Filled })
            }),
            _ => Request::Unknown,
        }
    }
}

/// A point of Bullet's frame in the game's (z negated).
fn game([x, y, z]: [f32; 3]) -> [f32; 3] {
    [x, y, -z]
}

/// A line of Bullet's frame into `lines`, in the game's.
fn push(lines: &mut LineBuffer, a: [f32; 3], b: [f32; 3], color: LineColor) {
    lines.push(game(a), game(b), color);
}

/// A filled triangle of Bullet's frame into `lines`, in the game's: `color`
/// translucent, brighter the more it faces up or down.
fn push_triangle(lines: &mut LineBuffer, corners: [[f32; 3]; 3], color: LineColor) {
    let [a, b, c] = corners.map(game);
    let (u, v) = (Vec3::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]), Vec3::new(c[0] - a[0], c[1] - a[1], c[2] - a[2]));
    let up = u.cross(v).direction().map_or(0.0, |n| n.y.abs());
    lines.push_triangle([a, b, c], color.shaded(0.4 + 0.6 * up, FILL_ALPHA));
}

/// `p` where `pose` puts it.
fn place(pose: &Pose, [x, y, z]: [f32; 3]) -> [f32; 3] {
    let v = pose.apply(Vec3::new(x, y, z));
    [v.x, v.y, v.z]
}

/// How many objects of each level group the world holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Census([u32; 4]);

/// Where a read was made: the eye (the game's frame), the world and its
/// damage zones' states.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ReadAt {
    eye: WorldPoint,
    world: PhysicsId,
    /// Which damage zones were on (zone_signature).
    zones: u32,
}

impl ReadAt {
    /// Whether a read at `now` would differ: the eye moved more than MOVE,
    /// a zone went on or off, or another world.
    fn stale(self, now: ReadAt) -> bool {
        let moved = Distance::parse(MOVE).is_some_and(|step| !self.eye.within(now.eye, step));
        moved || self.world != now.world || self.zones != now.zones
    }
}

/// A read's place and the radius the next one starts at (RADII's index).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fit {
    at: ReadAt,
    start: usize,
}

/// What a read follows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Previous {
    /// The view's start or its filter's change: nothing shown, the census due.
    Start,
    /// The lines read with this fit, shown meanwhile.
    Shown(Fit),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum View {
    Off,
    /// On, waiting for a match's camera to read (just turned on, or its
    /// filter changed).
    Starting { filter: Filter },
    /// Its lines read, drawn every frame until a read is stale.
    On { filter: Filter, fit: Fit },
    /// A read out on the camera's thread, the spare room lent to it.
    Reading { filter: Filter, after: Previous },
}

/// The two rooms for lines, made the first time the view is turned on.
enum Rooms {
    Unmade,
    /// The lines shown and the spare a read fills.
    Home { shown: LineBuffer, spare: LineBuffer },
    /// The spare out with a read (View::Reading's).
    Lent { shown: LineBuffer },
}

/// The view (its filter what was asked), its rooms, and the match the
/// options last read are of (what of the filter is drawn).
struct Collision {
    view: View,
    rooms: Rooms,
    shown: Shown,
}

/// A read the camera's thread makes outside the lock, into the spare room.
struct Read {
    filter: Filter,
    after: Previous,
    at: ReadAt,
    lines: LineBuffer,
}

/// What the console is told of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reply {
    On(Filter),
    Off,
    Usage,
    /// No room for the lines: still off.
    NoRoom,
    /// Asked to turn on where the match shown does not allow the view:
    /// nothing changed.
    Refused,
    /// A match that does not allow the view came while it is on: nothing
    /// drawn in it.
    HiddenHere,
}

/// What a read reads: the level near the eye.
trait Level {
    fn census(&self) -> Census;
    /// The lines (and filled surfaces) `filter` shows within `reach` of the eye.
    fn outline(&self, reach: f32, filter: Filter, lines: &mut LineBuffer);
}

impl Collision {
    /// Whether turning the view on needs the rooms made first (outside the lock).
    fn unmade(&self) -> bool {
        matches!(self.rooms, Rooms::Unmade)
    }

    /// The view `request` makes of this one, or the reply that changes
    /// nothing: a wrong argument, or one that would turn the view on (or
    /// switch it) where the match shown does not allow it. Turning it off is
    /// allowed anywhere.
    fn asked(&self, request: Request) -> Result<View, Reply> {
        let shown = match self.view {
            View::Off => None,
            View::Starting { filter } | View::On { filter, .. } | View::Reading { filter, .. } => Some(filter),
        };
        match (shown, request) {
            (_, Request::Unknown) => Err(Reply::Usage),
            (Some(_), Request::Toggle) => Ok(View::Off),
            (Some(shown), Request::Show(filter)) if shown == filter => Ok(View::Off),
            _ if !self.shown.allows() => Err(Reply::Refused),
            (None, Request::Toggle) => Ok(View::Starting { filter: Filter::ALL }),
            (_, Request::Show(filter)) => Ok(View::Starting { filter }),
        }
    }

    /// Whether `request` needs the rooms made first: one that turns the view
    /// on while they are unmade.
    fn needs_rooms(&self, request: Request) -> bool {
        matches!(self.asked(request), Ok(View::Starting { .. })) && self.unmade()
    }

    /// `rooms`, made by the caller when unmade, are taken when the view turns on.
    fn command(&mut self, request: Request, rooms: Option<(LineBuffer, LineBuffer)>) -> Reply {
        let view = match self.asked(request) {
            Ok(view) => view,
            Err(reply) => return reply,
        };
        if view != View::Off && self.unmade() {
            let Some((shown, spare)) = rooms else { return Reply::NoRoom };
            self.rooms = Rooms::Home { shown, spare };
        }
        self.view = view;
        match view {
            View::Starting { filter } => Reply::On(filter),
            _ => Reply::Off,
        }
    }

    /// A match's or a replay's options read: it is the match shown from now
    /// on. A view on whose match now allows it or no longer does starts again
    /// (nothing of the last match shown; a read out is dropped at its end),
    /// read once the match allows it; one no longer allowed tells the console.
    fn options_read(&mut self, shown: Shown) -> Option<Reply> {
        let was = core::mem::replace(&mut self.shown, shown);
        let filter = match self.view {
            View::Off => return None,
            View::Starting { filter } | View::On { filter, .. } | View::Reading { filter, .. } => filter,
        };
        if was.allows() == shown.allows() {
            return None;
        }
        self.view = View::Starting { filter };
        (!shown.allows()).then_some(Reply::HiddenHere)
    }

    /// The read a match's camera at `at` makes: after a start or a filter's
    /// change, or when the last read is stale; none while one is out, nor
    /// where the match shown does not allow the view.
    fn next_read(&mut self, at: ReadAt) -> Option<Read> {
        if !self.shown.allows() {
            return None;
        }
        let (filter, after) = match self.view {
            View::Off | View::Reading { .. } => return None,
            View::Starting { filter } => (filter, Previous::Start),
            View::On { filter, fit } if fit.at.stale(at) => (filter, Previous::Shown(fit)),
            View::On { .. } => return None,
        };
        match core::mem::replace(&mut self.rooms, Rooms::Unmade) {
            Rooms::Home { shown, spare } => {
                self.rooms = Rooms::Lent { shown };
                self.view = View::Reading { filter, after };
                Some(Read { filter, after, at, lines: spare })
            }
            other => {
                self.rooms = other;
                None
            }
        }
    }

    /// A read made, the next start `start`: its lines shown when the view
    /// still waits for it (else back to spare); the census, for a start's.
    fn finish(&mut self, read: Read, start: usize, census: Option<Census>) -> Option<Census> {
        let Rooms::Lent { shown } = core::mem::replace(&mut self.rooms, Rooms::Unmade) else {
            unreachable!("a read out lends the spare");
        };
        if self.view != (View::Reading { filter: read.filter, after: read.after }) {
            self.rooms = Rooms::Home { shown, spare: read.lines };
            return None;
        }
        self.rooms = Rooms::Home { shown: read.lines, spare: shown };
        self.view = View::On { filter: read.filter, fit: Fit { at: read.at, start } };
        census
    }

    /// The lines to draw now.
    fn drawn(&self) -> Option<&LineBuffer> {
        let shown = match &self.rooms {
            Rooms::Home { shown, .. } | Rooms::Lent { shown } => shown,
            Rooms::Unmade => return None,
        };
        match self.view {
            View::On { .. } | View::Reading { after: Previous::Shown(_), .. } => Some(shown),
            _ => None,
        }
        .filter(|lines| lines.used() > 0)
    }
}

/// `read`'s lines from the level, from the radius it starts at to the first
/// whose lines fit (the smallest's, cut, when none does), each line's end
/// moved toward the eye by 1 cm or 0.5 % of its distance and each filled
/// corner by half that (so a line on a surface is drawn over it, the fill
/// over the surface and under the line): the radius the next read starts at
/// (one larger when these used under a quarter of the room), and the census
/// for a start's.
fn fill(level: &impl Level, read: &mut Read) -> (usize, Option<Census>) {
    let mut index = match read.after {
        Previous::Start => 0,
        Previous::Shown(fit) => fit.start,
    };
    loop {
        read.lines.clear();
        level.outline(RADII[index], read.filter, &mut read.lines);
        if !read.lines.is_cut() || index + 1 == RADII.len() {
            break;
        }
        index += 1;
    }
    let start = if !read.lines.is_cut() && read.lines.used() < VERTICES / 4 { index.saturating_sub(1) } else { index };
    let eye = read.at.eye.coordinates();
    read.lines.move_points(|p| toward(p, eye, 1.0), |p| toward(p, eye, 0.5));
    (start, (read.after == Previous::Start).then(|| level.census()))
}

/// `p` moved toward `eye` by `scale` times 1 cm or 0.5 % of its distance, the more.
fn toward(p: [f32; 3], eye: [f32; 3], scale: f32) -> [f32; 3] {
    let d: [f32; 3] = core::array::from_fn(|k| eye[k] - p[k]);
    let distance = sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if distance == 0.0 {
        return p;
    }
    let step = (scale * (0.01f32).max(0.005 * distance)).min(distance) / distance;
    core::array::from_fn(|k| p[k] + d[k] * step)
}

/// The camera's player's physics world around the eye (Bullet's frame).
struct LiveLevel<'w, 't> {
    thread: &'t Thread,
    world: &'w PhysicsWorld<'t>,
    eye: [f32; 3],
    triangles: TriangleMethods,
}

impl Level for LiveLevel<'_, '_> {
    fn census(&self) -> Census {
        let mut census = Census::default();
        self.world.for_each_object(|object| {
            if let Some(group) = level_group(object.group, object.motion) {
                census.0[group.index()] += 1;
            }
        });
        for_each_damage_zone(self.thread, |_| census.0[LevelGroup::Anomaly.index()] += 1);
        census
    }

    /// The damage zones first, so the room never drops them for the level.
    fn outline(&self, reach: f32, filter: Filter, lines: &mut LineBuffer) {
        let near = Near { eye: self.eye, reach, fill: filter.fill, triangles: self.triangles };
        if shows(filter.only, LevelGroup::Anomaly) {
            for_each_damage_zone(self.thread, |zone| {
                let color = match zone.state() {
                    ZoneState::Active => ACTIVE_ZONE,
                    ZoneState::Off => OFF_ZONE,
                };
                zone.for_each_shape(|pose, shape| outline(shape, &pose, color, &near, lines));
            });
        }
        let [x, y, z] = self.eye;
        let (Some(eye), Some(distance)) = (WorldPoint::parse([x, y, -z]), Distance::parse(reach)) else { return };
        self.world.for_each_object(|object| {
            let Some(group) = level_group(object.group, object.motion) else { return };
            if lines.is_full() || !shows(filter.only, group) || !object.aabb.within(eye, distance) {
                return;
            }
            if let Some(shape) = object.shape() {
                outline(shape, &object.pose(), group.color(), &near, lines);
            }
        });
    }
}

/// Where lines are wanted: within `reach` of `eye` (Bullet's frame); filled or not.
struct Near {
    eye: [f32; 3],
    reach: f32,
    fill: Fill,
    triangles: TriangleMethods,
}

/// A shape's lines where `pose` puts it, the parts within reach.
fn outline(shape: Shape<'_>, pose: &Pose, color: LineColor, near: &Near, lines: &mut LineBuffer) {
    if lines.is_full() {
        return;
    }
    match shape {
        Shape::Mesh(mesh) => {
            let eye = place(&pose.inverse(), near.eye);
            let low = eye.map(|v| v - near.reach);
            let high = eye.map(|v| v + near.reach);
            let mut previous = None;
            mesh.triangles_in(near.triangles, low, high, &mut |triangle| {
                if lines.is_full() || !triangle_near(triangle, eye, near.reach) {
                    return;
                }
                if near.fill == Fill::Filled {
                    push_triangle(lines, triangle.map(|p| place(pose, p)), color);
                }
                for (a, b) in new_edges(triangle, previous) {
                    push(lines, place(pose, a), place(pose, b), color);
                }
                previous = Some(triangle);
            });
        }
        Shape::Compound(compound) => {
            compound.for_each_child(|child, shape| outline(shape, &pose.then(child), color, near, lines))
        }
        primitive => {
            let away = (pose.position - Vec3::new(near.eye[0], near.eye[1], near.eye[2])).length();
            if away - extent(primitive) <= near.reach {
                primitive_lines(primitive, pose, color, near.fill, lines);
            }
        }
    }
}

/// How far a primitive reaches from its centre.
fn extent(shape: Shape<'_>) -> f32 {
    match shape {
        Shape::Box { half: [x, y, z] } => sqrt(x * x + y * y + z * z),
        Shape::Sphere { radius } => radius,
        Shape::Capsule { radius, half_height, .. } => radius + half_height,
        Shape::Cylinder { radius, half_height } => sqrt(radius * radius + half_height * half_height),
        Shape::Mesh(_) | Shape::Compound(_) | Shape::Other(_) => 0.0,
    }
}

/// The triangle's edges but one it shares with the triangle drawn just
/// before it (a strip's: the same two corners).
fn new_edges([a, b, c]: [[f32; 3]; 3], previous: Option<[[f32; 3]; 3]>) -> impl Iterator<Item = ([f32; 3], [f32; 3])> {
    let shared = move |p: [f32; 3], q: [f32; 3]| previous.is_some_and(|t| t.contains(&p) && t.contains(&q));
    [(a, b), (b, c), (c, a)].into_iter().filter(move |&(p, q)| !shared(p, q))
}

/// A box's, sphere's, capsule's or cylinder's outline where `pose` puts it;
/// a box's faces filled too when `fill` asks (the round ones are outlines only).
fn primitive_lines(shape: Shape<'_>, pose: &Pose, color: LineColor, fill: Fill, lines: &mut LineBuffer) {
    if let (Shape::Box { half }, Fill::Filled) = (shape, fill) {
        box_triangles(half, &mut |corners| push_triangle(lines, corners.map(|p| place(pose, p)), color));
    }
    let mut line = |a: [f32; 3], b: [f32; 3]| push(lines, place(pose, a), place(pose, b), color);
    match shape {
        Shape::Box { half } => box_lines(half, &mut line),
        Shape::Sphere { radius } => sphere_lines(radius, &mut line),
        Shape::Capsule { up, radius, half_height } => capsule_lines(up, radius, half_height, &mut line),
        Shape::Cylinder { radius, half_height } => cylinder_lines(radius, half_height, &mut line),
        Shape::Mesh(_) | Shape::Compound(_) | Shape::Other(_) => {}
    }
}

/// Whether the triangle's box is within `reach` of `eye`.
fn triangle_near([a, b, c]: [[f32; 3]; 3], eye: [f32; 3], reach: f32) -> bool {
    let low = core::array::from_fn(|k| a[k].min(b[k]).min(c[k]));
    let high = core::array::from_fn(|k| a[k].max(b[k]).max(c[k]));
    box_within(low, high, eye, reach)
}

/// The twelve edges of a box of half extents `half` about the origin.
fn box_lines(half: [f32; 3], line: &mut impl FnMut([f32; 3], [f32; 3])) {
    let corner = |i: usize| -> [f32; 3] { core::array::from_fn(|k| if i >> k & 1 == 1 { half[k] } else { -half[k] }) };
    for i in 0..8 {
        for k in 0..3 {
            if i >> k & 1 == 0 {
                line(corner(i), corner(i | 1 << k));
            }
        }
    }
}

/// The twelve triangles of a box's six faces (two a face).
fn box_triangles(half: [f32; 3], triangle: &mut impl FnMut([[f32; 3]; 3])) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in [-1.0, 1.0] {
            let corner = |a: f32, b: f32| -> [f32; 3] {
                let mut p = [0.0; 3];
                p[axis] = side * half[axis];
                p[u] = a * half[u];
                p[v] = b * half[v];
                p
            };
            let quad = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            triangle([quad[0], quad[1], quad[2]]);
            triangle([quad[0], quad[2], quad[3]]);
        }
    }
}

/// An arc of `radius` about `centre` in the plane of the axes `u` and `v`,
/// from `from` to `to` (radians), CIRCLE segments a full turn.
fn arc(centre: [f32; 3], (u, v): (usize, usize), radius: f32, (from, to): (f32, f32),
       line: &mut impl FnMut([f32; 3], [f32; 3])) {
    let steps = ((to - from) / (2.0 * PI) * CIRCLE as f32 + 0.5) as usize;
    let point = |i: usize| {
        let t = from + (to - from) * i as f32 / steps as f32;
        let (sin, cos) = sin_cos(t);
        let mut p = centre;
        p[u] += radius * cos;
        p[v] += radius * sin;
        p
    };
    for i in 0..steps {
        line(point(i), point(i + 1));
    }
}

const TURN: (f32, f32) = (0.0, 2.0 * PI);

fn sphere_lines(radius: f32, line: &mut impl FnMut([f32; 3], [f32; 3])) {
    for axes in [(0, 1), (1, 2), (2, 0)] {
        arc([0.0; 3], axes, radius, TURN, line);
    }
}

/// A cylinder along y: its two rims and four sides.
fn cylinder_lines(radius: f32, half_height: f32, line: &mut impl FnMut([f32; 3], [f32; 3])) {
    for y in [-half_height, half_height] {
        arc([0.0, y, 0.0], (2, 0), radius, TURN, line);
    }
    for t in 0..4 {
        let (sin, cos) = sin_cos(t as f32 * PI / 2.0);
        let (x, z) = (radius * sin, radius * cos);
        line([x, -half_height, z], [x, half_height, z]);
    }
}

/// A capsule along `up`: the rims where its caps meet its side, four sides,
/// and each cap's two half circles.
fn capsule_lines(up: usize, radius: f32, half_height: f32, line: &mut impl FnMut([f32; 3], [f32; 3])) {
    let (u, v) = ((up + 1) % 3, (up + 2) % 3);
    let at = |h: f32| -> [f32; 3] { core::array::from_fn(|k| if k == up { h } else { 0.0 }) };
    for (h, from) in [(-half_height, PI), (half_height, 0.0)] {
        arc(at(h), (u, v), radius, TURN, line);
        arc(at(h), (u, up), radius, (from, from + PI), line);
        arc(at(h), (v, up), radius, (from, from + PI), line);
    }
    for t in 0..4 {
        let (sin, cos) = sin_cos(t as f32 * PI / 2.0);
        let mut side = [0.0; 3];
        side[u] = radius * cos;
        side[v] = radius * sin;
        let (mut low, mut high) = (side, side);
        low[up] = -half_height;
        high[up] = half_height;
        line(low, high);
    }
}

/// on_configs_loaded's start: the command registered (once).
pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    COMMAND.register(ctx.thread(), NAME, collision_view_methods_vtable_handle());
    Flow::Continue
}

/// The options read: the match shown from now on, and what the console is told of it.
fn options_read(thread: &Thread, shown: Shown) -> Flow<NoExit> {
    if let Some(reply) = STATE.with(thread, |state| state.options_read(shown)) {
        for (text, suffix) in reply_lines(reply) {
            let mut line = Line::new();
            line.push(text);
            line.push(suffix);
            tell(thread, &line);
        }
    }
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D): a live
/// match, private when srp's block says so.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let shown = OptionsRead::at_static_info(ctx).map_or(Shown::Public, |read| Shown::of_match(read.rest()));
    options_read(ctx.thread(), shown)
}

/// A replay's chunk 1 after its deserialize (0x1F0971): a replay, whatever
/// its options say.
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx.thread(), Shown::Replay)
}

/// The console executes `collision`.
pub fn on_execute(_: &OrigCcDelegateExecute, command: &mut ConsoleCommand<'_>, arguments: &ConsoleArguments<'_>) {
    let thread = command.thread();
    let request = Request::parse(arguments.text().map_or(&[][..], CStr::to_bytes));
    let make = STATE.with(thread, |state| state.needs_rooms(request));
    let rooms = if make { LineBuffer::make_pair(thread, VERTICES) } else { None };
    let reply = STATE.with(thread, |state| state.command(request, rooms));
    for (text, suffix) in reply_lines(reply) {
        let mut line = Line::new();
        line.push(text);
        line.push(suffix);
        tell(thread, &line);
    }
}

/// update_camera after the view's update: in a match, the lines near the
/// eye read when the view wants a read.
pub fn on_viewed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if STATE.with(thread, |state| state.view == View::Off) || !match_shown(thread) {
        return Flow::Continue;
    }
    let (Some(player), Some(view)) = (camera::viewer(ctx), camera::viewed(ctx)) else { return Flow::Continue };
    let (Some(world), Some(eye)) = (player.physics_world(), WorldPoint::parse(view.eye)) else {
        return Flow::Continue;
    };
    let at = ReadAt { eye, world: world.id(), zones: zone_signature(thread) };
    let Some(mut read) = STATE.with(thread, |state| state.next_read(at)) else { return Flow::Continue };
    let [x, y, z] = view.eye;
    let level =
        LiveLevel { thread, world: &world, eye: [x, y, -z], triangles: collision_view_triangles_vtable_handle() };
    let (start, census) = fill(&level, &mut read);
    let only = read.filter.only;
    if let Some(census) = STATE.with(thread, |state| state.finish(read, start, census)) {
        let shown = GROUPS.into_iter().filter(|&group| shows(only, group));
        for group in core::iter::once(None).chain(shown.map(Some)) {
            let mut line = Line::new();
            describe_census(census, group, &mut line);
            tell(thread, &line);
        }
    }
    Flow::Continue
}

/// Which damage zones are on, as a number that changes when one goes on or off.
fn zone_signature(thread: &Thread) -> u32 {
    let mut signature = 1u32;
    for_each_damage_zone(thread, |zone| {
        signature = signature.wrapping_mul(31).wrapping_add(u32::from(zone.state() == ZoneState::Active));
    });
    signature
}

/// processAllTriangles' callback of ours: the triangle to its query.
pub fn on_triangle(_: &OrigTriangleProcess, sink: &mut TriangleSink<'_>, triangle: &Triangle<'_>, _: u32, _: u32) {
    sink.deliver(triangle);
}

/// renderer::render's depth-tested debug lines: the last lines read, drawn
/// every frame while a match's world is shown. Off: one lock, one compare.
pub fn on_debug_lines(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    STATE.with(thread, |state| {
        if state.view == View::Off || !match_shown(thread) {
            return;
        }
        if let (Some(lines), Some(debug)) = (state.drawn(), DebugLines::at(ctx)) {
            debug.draw(lines);
        }
    });
    Flow::Continue
}

/// A line to the console (the game's log) and to srp_patches.log.
fn tell(thread: &Thread, line: &Line) {
    console::game_line(thread, line.c_str());
    log::line(&[Piece::Text(line.text())]);
}

/// The options, after `collision` turns the view on and after a wrong argument.
const HELP: [&str; 6] = [
    "  collision walk    - what stops walking (green + white)",
    "  collision shot    - what stops bullets and the camera (magenta + white)",
    "  collision solid   - what stops both (white)",
    "  collision anomaly - anomaly damage zones (red, dim when off)",
    "  ... fill          - after any of these (or alone): surfaces filled, to find holes",
    "  collision         - again to turn off",
];

/// What the console is told of a command, a line each (its text and a suffix).
fn reply_lines(reply: Reply) -> impl Iterator<Item = (&'static str, &'static str)> {
    let filled = |filter: Filter| if filter.fill == Fill::Filled { ", filled" } else { "" };
    let (first, help): ((&str, &str), &[&str]) = match reply {
        Reply::On(filter @ Filter { only: None, .. }) => {
            (("collision: on, all (walk green, shot magenta, solid white, anomaly red)", filled(filter)), &HELP)
        }
        Reply::On(filter @ Filter { only: Some(group), .. }) => {
            let text = match group {
                LevelGroup::Walk => "collision: on, walk (green + white)",
                LevelGroup::Shot => "collision: on, shot (magenta + white)",
                LevelGroup::Solid => "collision: on, solid (white)",
                LevelGroup::Anomaly => "collision: on, anomaly (red, dim when off)",
            };
            ((text, filled(filter)), &[])
        }
        Reply::Off => (("collision: off", ""), &[]),
        Reply::Usage => (("collision: collision [walk | shot | solid | anomaly] [fill]", ""), &HELP),
        Reply::NoRoom => (("collision: no room for its lines, still off", ""), &[]),
        Reply::Refused => (("collision: only in private rooms and replays", ""), &[]),
        Reply::HiddenHere => (("collision: only in private rooms and replays: nothing drawn in this match", ""), &[]),
    };
    core::iter::once(first).chain(help.iter().map(|text| (*text, "")))
}

/// The census's head (`None`) or a group's line.
fn describe_census(census: Census, group: Option<LevelGroup>, line: &mut Line) {
    line.push("collision: ");
    let Some(group) = group else {
        line.push("the level's static collision within ");
        line.number(RADII[0] as u32);
        line.push(" m (less where that is over ");
        line.number(MOST as u32);
        line.push(" lines):");
        return;
    };
    line.push(group.name());
    line.push(" (");
    line.push(group.color_name());
    line.push(") ");
    line.number(census.0[group.index()]);
    line.push(" in the world: ");
    line.push(group.meets());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::debug_lines::LineVertex;

    /// A made-up level: `lines(reach)` lines a radius, of the walkers' group.
    struct Fake {
        census: Census,
        lines: fn(f32) -> usize,
    }

    impl Level for Fake {
        fn census(&self) -> Census {
            self.census
        }
        fn outline(&self, reach: f32, filter: Filter, lines: &mut LineBuffer) {
            if !shows(filter.only, LevelGroup::Walk) {
                return;
            }
            for i in 0..(self.lines)(reach) {
                push(lines, [i as f32, 0.0, 1.0], [i as f32, 1.0, 2.0], LevelGroup::Walk.color());
            }
        }
    }

    fn buffer(lines: usize) -> LineBuffer {
        LineBuffer::over(Box::leak(vec![LineVertex::default(); 2 * lines].into_boxed_slice()))
    }

    /// A view in a private room's match: the other tests' (the gate is its own tests').
    fn state() -> Collision {
        Collision { view: View::Off, rooms: Rooms::Unmade, shown: Shown::PrivateRoom }
    }

    fn state_in(shown: Shown) -> Collision {
        Collision { shown, ..state() }
    }

    fn rooms() -> Option<(LineBuffer, LineBuffer)> {
        Some((buffer(MOST), buffer(MOST)))
    }

    fn at(x: f32, world: u32) -> ReadAt {
        ReadAt { eye: WorldPoint::parse([x, 1.0, 0.0]).unwrap(), world: PhysicsId::made_up(world), zones: 1 }
    }

    /// A match's camera at `at` over `level`: whether it read, and the census
    /// when it read a start's.
    fn frame(s: &mut Collision, at: ReadAt, level: &Fake) -> (bool, Option<Census>) {
        let Some(mut read) = s.next_read(at) else { return (false, None) };
        let (start, census) = fill(level, &mut read);
        (true, s.finish(read, start, census))
    }

    fn drawn(s: &Collision) -> usize {
        s.drawn().map_or(0, LineBuffer::count)
    }

    fn fake(lines: fn(f32) -> usize) -> Fake {
        Fake { census: Census([541, 511, 105, 12]), lines }
    }

    fn read(after: Previous, size: usize) -> Read {
        Read { filter: Filter::ALL, after, at: at(0.0, 1), lines: buffer(size) }
    }

    fn filled(only: Option<LevelGroup>) -> Filter {
        Filter { only, fill: Fill::Filled }
    }

    #[test]
    fn the_private_room_entry_in_srps_block() {
        let shown = |bytes: &[u8]| Shown::of_match(Reader::new(bytes));
        assert_eq!(shown(b"SRPX\x08\x01\x01"), Shown::PrivateRoom);
        assert_eq!(shown(b"SRPX\x02\x01\x01\x07\x02\x00\x01\x08\x01\x01"), Shown::PrivateRoom, "after others");
        assert_eq!(shown(b"SRPX\x09\x03xyz\x08\x01\x01"), Shown::PrivateRoom, "after an unknown tag");
        assert_eq!(shown(b"SRPX\x08\x01\x01\x04\x02\x2c\x01"), Shown::PrivateRoom, "before others");
        // No block, a block without it, another value or length, a cut entry,
        // another magic: a public match.
        for bytes in [&b""[..], b"SRPX", b"SRPX\x02\x01\x01", b"SRPX\x08\x01\x00", b"SRPX\x08\x02\x01\x01",
                      b"SRPX\x08\x01", b"SRPY\x08\x01\x01", b"SRPX\x05\x03\x08\x01\x01"] {
            assert_eq!(shown(bytes), Shown::Public, "{bytes:?}");
        }
        assert!(Shown::PrivateRoom.allows() && Shown::Replay.allows() && !Shown::Public.allows());
    }

    #[test]
    fn the_view_only_in_a_private_room_or_a_replay() {
        let walk = Some(LevelGroup::Walk);
        let lines = |only| Filter { only, fill: Fill::Lines };
        let requests = [Request::Toggle, Request::Show(lines(walk)), Request::Show(lines(Some(LevelGroup::Anomaly))),
                        Request::Show(filled(None)), Request::Show(filled(walk))];
        for request in requests {
            let mut s = state_in(Shown::Public);
            assert!(!s.needs_rooms(request), "refused: no rooms made for it");
            assert_eq!(s.command(request, rooms()), Reply::Refused, "no match read yet: {request:?}");
            assert_eq!((s.view, s.unmade()), (View::Off, true), "nothing changed");
            assert_eq!(s.options_read(Shown::Public), None);
            assert_eq!(s.command(request, rooms()), Reply::Refused, "a public match");
            assert_eq!(s.command(Request::Unknown, None), Reply::Usage, "a wrong argument: the usage, as anywhere");
            assert_eq!(frame(&mut s, at(0.0, 1), &fake(|_| 3)), (false, None), "no read");
            for allowed in [Shown::PrivateRoom, Shown::Replay] {
                let mut s = state_in(Shown::Public);
                assert_eq!(s.options_read(allowed), None);
                assert!(s.needs_rooms(request));
                let filter = match request { Request::Show(filter) => filter, _ => Filter::ALL };
                assert_eq!(s.command(request, rooms()), Reply::On(filter), "{allowed:?}");
                assert!(frame(&mut s, at(0.0, 1), &fake(|_| 3)).0, "read");
                assert!(matches!(s.view, View::On { .. }), "on");
            }
        }
    }

    #[test]
    fn a_view_on_draws_nothing_in_a_match_that_does_not_allow_it() {
        let level = fake(|_| 3);
        let mut s = state_in(Shown::PrivateRoom);
        s.command(Request::Show(filled(None)), rooms());
        assert_eq!(frame(&mut s, at(0.0, 1), &level), (true, Some(Census([541, 511, 105, 12]))));
        assert_eq!(s.options_read(Shown::PrivateRoom), None, "another private match: as it was");
        assert!(matches!(s.view, View::On { .. }));
        assert_eq!(s.options_read(Shown::Public), Some(Reply::HiddenHere), "a public match: told once");
        assert_eq!(s.view, View::Starting { filter: filled(None) }, "the asked filter kept");
        assert_eq!((drawn(&s), frame(&mut s, at(0.0, 2), &level)), (0, (false, None)), "nothing drawn, no read");
        assert_eq!(s.options_read(Shown::Public), None, "the next public match: nothing to tell");
        assert_eq!(s.command(Request::Show(lines_of(Some(LevelGroup::Shot))), None), Reply::Refused,
                   "another filter there: refused, nothing changed");
        assert_eq!(s.view, View::Starting { filter: filled(None) });
        assert_eq!(s.options_read(Shown::Replay), None, "a replay: back, nothing told");
        assert_eq!(frame(&mut s, at(0.0, 3), &level), (true, Some(Census([541, 511, 105, 12]))), "read, its census");
        assert_eq!(drawn(&s), 3);
        assert_eq!(s.options_read(Shown::PrivateRoom), None, "a replay then a private match: on in both");
        assert!(matches!(s.view, View::On { .. }));
        // A read out while a public match's options are read: dropped.
        let out = s.next_read(at(5.0, 3)).unwrap();
        assert_eq!(s.options_read(Shown::Public), Some(Reply::HiddenHere));
        assert_eq!(s.finish(out, 0, None), None);
        assert_eq!((s.view, drawn(&s)), (View::Starting { filter: filled(None) }, 0), "the read dropped");
        assert_eq!(s.command(Request::Show(filled(None)), None), Reply::Off, "what is shown again: off, not refused");
        assert_eq!(s.options_read(Shown::PrivateRoom), None, "off: nothing comes back");
        assert_eq!(s.view, View::Off);
        s.command(Request::Toggle, None);
        s.options_read(Shown::Public);
        assert_eq!(s.command(Request::Toggle, None), Reply::Off, "collision there: off");
    }

    fn lines_of(only: Option<LevelGroup>) -> Filter {
        Filter { only, fill: Fill::Lines }
    }

    #[test]
    fn requests_as_typed() {
        let show = |only, fill| Request::Show(Filter { only, fill });
        assert_eq!(Request::parse(b""), Request::Toggle);
        assert_eq!(Request::parse(b"  "), Request::Toggle);
        assert_eq!(Request::parse(b"walk"), show(Some(LevelGroup::Walk), Fill::Lines));
        assert_eq!(Request::parse(b" shot "), show(Some(LevelGroup::Shot), Fill::Lines));
        assert_eq!(Request::parse(b"SOLID"), show(Some(LevelGroup::Solid), Fill::Lines));
        assert_eq!(Request::parse(b"anomaly"), show(Some(LevelGroup::Anomaly), Fill::Lines));
        assert_eq!(Request::parse(b"fill"), show(None, Fill::Filled));
        assert_eq!(Request::parse(b"walk  fill"), show(Some(LevelGroup::Walk), Fill::Filled));
        assert_eq!(Request::parse(b"Shot FILL"), show(Some(LevelGroup::Shot), Fill::Filled));
        for other in [&b"2"[..], b"0x8", b"a", b"walker", b"walk shot", b"all", b"fill walk", b"walk fill x"] {
            assert_eq!(Request::parse(other), Request::Unknown, "{other:?}");
        }
    }

    #[test]
    fn the_command_turns_it_on_and_off() {
        let mut s = state();
        let lines = |group| Filter { only: Some(group), fill: Fill::Lines };
        assert_eq!(s.command(Request::Toggle, rooms()), Reply::On(Filter::ALL));
        assert_eq!(s.view, View::Starting { filter: Filter::ALL });
        assert_eq!(s.command(Request::Unknown, None), Reply::Usage);
        assert_eq!(s.view, View::Starting { filter: Filter::ALL });
        frame(&mut s, at(0.0, 1), &fake(|_| 3));
        assert_eq!(s.command(Request::Show(lines(LevelGroup::Shot)), None), Reply::On(lines(LevelGroup::Shot)));
        assert_eq!(s.view, View::Starting { filter: lines(LevelGroup::Shot) });
        frame(&mut s, at(0.0, 1), &fake(|_| 3));
        let walk = lines(LevelGroup::Walk);
        assert_eq!(s.command(Request::Show(walk), None), Reply::On(walk), "switched");
        assert_eq!(s.command(Request::Show(lines(LevelGroup::Walk)), None), Reply::Off, "the shown name again: off");
        assert_eq!((s.view, drawn(&s)), (View::Off, 0));
        assert_eq!(s.command(Request::Toggle, None), Reply::On(Filter::ALL), "the rooms kept");
        assert_eq!(s.command(Request::Toggle, None), Reply::Off);
    }

    #[test]
    fn no_room_stays_off() {
        let mut s = state();
        assert!(s.unmade());
        assert_eq!(s.command(Request::Toggle, None), Reply::NoRoom);
        assert_eq!(s.view, View::Off);
        assert_eq!(s.command(Request::Toggle, rooms()), Reply::On(Filter::ALL));
        assert!(!s.unmade(), "made once, kept");
    }

    #[test]
    fn a_read_on_a_start_a_move_a_zone_or_another_world() {
        let mut s = state();
        let level = fake(|_| 3);
        assert_eq!(frame(&mut s, at(0.0, 1), &level), (false, None), "off: no read");
        s.command(Request::Toggle, rooms());
        assert_eq!(drawn(&s), 0);
        assert_eq!(frame(&mut s, at(0.0, 1), &level), (true, Some(Census([541, 511, 105, 12]))), "a start: its census");
        assert_eq!(drawn(&s), 3);
        assert_eq!(frame(&mut s, at(0.4, 1), &level), (false, None), "0.4 m on: the same lines");
        assert_eq!(frame(&mut s, at(0.6, 1), &level), (true, None), "0.6 m on: read again, no census");
        assert_eq!(frame(&mut s, at(0.6, 1), &level), (false, None));
        assert_eq!(frame(&mut s, at(0.6, 2), &level), (true, None), "another match's world");
        let zone_off = ReadAt { zones: 2, ..at(0.6, 2) };
        assert_eq!(frame(&mut s, zone_off, &level), (true, None), "a damage zone went on or off");
        assert_eq!(frame(&mut s, zone_off, &level), (false, None));
        s.command(Request::Show(Filter { only: Some(LevelGroup::Walk), fill: Fill::Lines }), None);
        assert_eq!(drawn(&s), 0, "a filter's change: nothing until its read");
        assert_eq!(frame(&mut s, at(0.6, 2), &level), (true, Some(Census([541, 511, 105, 12]))), "its census too");
        assert_eq!(drawn(&s), 3, "drawn every frame until the next read");
    }

    #[test]
    fn a_read_out_is_whole_or_dropped() {
        let mut s = state();
        s.command(Request::Toggle, rooms());
        frame(&mut s, at(0.0, 1), &fake(|_| 3));
        let mut out = s.next_read(at(5.0, 1)).unwrap();
        assert!(s.next_read(at(9.0, 1)).is_none(), "one read out at a time");
        let (start, census) = fill(&fake(|_| 7), &mut out);
        assert_eq!(drawn(&s), 3, "the last read drawn while the next is out");
        s.command(Request::Toggle, None);
        s.command(Request::Toggle, None);
        assert_eq!(s.finish(out, start, census), None, "turned off and on meanwhile: dropped");
        assert_eq!(s.view, View::Starting { filter: Filter::ALL }, "the start still due, its census with it");
        assert_eq!(frame(&mut s, at(5.0, 1), &fake(|_| 7)), (true, Some(Census([541, 511, 105, 12]))));
        assert_eq!(drawn(&s), 7);
    }

    #[test]
    fn the_largest_radius_that_fits() {
        let fit = |after, count: fn(f32) -> usize| {
            let mut read = read(after, MOST);
            let (start, _) = fill(&fake(count), &mut read);
            (read.lines.count(), start)
        };
        let shown = |start| Previous::Shown(Fit { at: at(0.0, 1), start });
        let cut_at_20 = |r: f32| if r > 15.0 { MOST + 1 } else { 50_000 };
        assert_eq!(fit(Previous::Start, cut_at_20), (50_000, 1), "20 m cut: 10 m's");
        assert_eq!(fit(Previous::Start, |_| MOST), (MOST, 0), "exactly the room at 20 m: it fits, not cut");
        assert_eq!(fit(Previous::Start, |_| 200_000), (MOST, 2), "cut at 5 m too: the room's");
        assert_eq!(fit(shown(1), |_| 50_000), (50_000, 1), "from the last fit, 10 m: one pass");
        assert_eq!(fit(shown(1), |_| 1_000), (1_000, 0), "under a quarter of the room: the next starts larger");
    }

    #[test]
    fn lines_drawn_over_their_surface() {
        let eye = [0.0, 0.0, 0.0];
        let near = toward([1.0, 0.0, 0.0], eye, 1.0);
        assert!((near[0] - 0.99).abs() < 1e-6, "1 m away: 1 cm nearer");
        let far = toward([0.0, 0.0, 20.0], eye, 1.0);
        assert!((far[2] - 19.9).abs() < 1e-5, "20 m away: 0.5 % nearer");
        let fill = toward([0.0, 0.0, 20.0], eye, 0.5);
        assert!((fill[2] - 19.95).abs() < 1e-5, "a filled corner half as near: under its lines");
        assert_eq!(toward(eye, eye, 1.0), eye);
        assert_eq!(toward([0.005, 0.0, 0.0], eye, 1.0), eye, "nearer than the step: at the eye, not past it");
        let mut read = read(Previous::Start, 4);
        push(&mut read.lines, [2.0, 1.0, -3.0], [4.0, 5.0, -6.0], LevelGroup::Solid.color());
        let v = read.lines.vertices()[0].at;
        assert_eq!(v, [2.0, 1.0, 3.0], "Bullet's frame in, the game's out (z negated)");
    }

    #[test]
    fn a_shared_edge_once() {
        let first = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let second = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        assert_eq!(new_edges(first, None).count(), 3);
        let edges: Vec<_> = new_edges(second, Some(first)).collect();
        assert_eq!(edges, [([0.0, 1.0, 0.0], [1.0, 1.0, 0.0]), ([1.0, 1.0, 0.0], [1.0, 0.0, 0.0])]);
        assert_eq!(new_edges(second, Some(second.map(|p| p.map(|v| v + 5.0)))).count(), 3);
    }

    #[test]
    fn a_filter_shows_what_stops_it() {
        let shown = |only| GROUPS.into_iter().filter(|&g| shows(only, g)).collect::<Vec<_>>();
        assert_eq!(shown(None), GROUPS);
        assert_eq!(shown(Some(LevelGroup::Walk)), [LevelGroup::Walk, LevelGroup::Solid]);
        assert_eq!(shown(Some(LevelGroup::Shot)), [LevelGroup::Shot, LevelGroup::Solid]);
        assert_eq!(LevelGroup::of(0x81), None, "no filter group is an anomaly's");
        assert_eq!(shown(Some(LevelGroup::Solid)), [LevelGroup::Solid]);
        assert_eq!(shown(Some(LevelGroup::Anomaly)), [LevelGroup::Anomaly]);
    }

    #[test]
    fn only_static_level_groups() {
        for (group, motion, want) in [
            (0x2, Motion::Static, Some(LevelGroup::Walk)),
            (0x8, Motion::Static, Some(LevelGroup::Shot)),
            (0xA, Motion::Static, Some(LevelGroup::Solid)),
            (0x40, Motion::Static, None),     // a living player's hit model
            (0x824, Motion::Moving, None),    // a character controller
            (0xA18, Motion::Moving, None),    // a battery
            (0x818, Motion::Moving, None),    // a grenade
            (0x2, Motion::Moving, None),      // anything of a level group that moves
            (0x20, Motion::Static, None),     // the lobby's decal markers
        ] {
            assert_eq!(level_group(group, motion), want, "{group:#x}");
        }
    }

    fn collect(draw: impl FnOnce(&mut dyn FnMut([f32; 3], [f32; 3]))) -> Vec<([f32; 3], [f32; 3])> {
        let mut out = vec![];
        draw(&mut |a, b| out.push((a, b)));
        out
    }

    fn length(a: [f32; 3]) -> f32 {
        (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
    }

    #[test]
    fn a_box_its_twelve_edges() {
        let edges = collect(|line| box_lines([1.0, 2.0, 3.0], &mut |a, b| line(a, b)));
        assert_eq!(edges.len(), 12);
        for (a, b) in &edges {
            let differ = (0..3).filter(|&k| a[k] != b[k]).count();
            assert_eq!(differ, 1, "an edge along one axis");
            assert!((0..3).all(|k| a[k].abs() == [1.0, 2.0, 3.0][k] && b[k].abs() == [1.0, 2.0, 3.0][k]));
        }
    }

    #[test]
    fn round_shapes() {
        let sphere = collect(|line| sphere_lines(2.0, &mut |a, b| line(a, b)));
        assert_eq!(sphere.len(), 3 * CIRCLE);
        assert!(sphere.iter().all(|(a, b)| (length(*a) - 2.0).abs() < 1e-5 && (length(*b) - 2.0).abs() < 1e-5));
        let cylinder = collect(|line| cylinder_lines(0.5, 1.5, &mut |a, b| line(a, b)));
        assert_eq!(cylinder.len(), 2 * CIRCLE + 4);
        assert!(cylinder.iter().all(|(a, _)| (a[0].hypot(a[2]) - 0.5).abs() < 1e-5 && a[1].abs() == 1.5));
        let capsule = collect(|line| capsule_lines(0, 0.4, 0.9, &mut |a, b| line(a, b)));
        assert_eq!(capsule.len(), 2 * CIRCLE + 2 * CIRCLE + 4);
        let reach = capsule.iter().map(|(a, _)| a[0].abs()).fold(0.0, f32::max);
        assert!((reach - 1.3).abs() < 1e-5, "the caps reach half height + radius along x: {reach}");
        assert!(capsule.iter().all(|(a, _)| a[1].hypot(a[2]) <= 0.4 + 1e-5));
    }

    #[test]
    fn shapes_where_their_pose_puts_them() {
        // turned a quarter about y, then moved: local x is world -z
        let turned = ::ragdoll::math::Quat::from_axis_angle(Vec3::Y, PI / 2.0);
        let pose = Pose::new(turned, Vec3::new(10.0, 0.0, 5.0));
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(close(place(&pose, [1.0, 2.0, 3.0]), [13.0, 2.0, 4.0]));
        assert!(close(place(&pose.inverse(), [13.0, 2.0, 4.0]), [1.0, 2.0, 3.0]));
        let child = Pose::new(::ragdoll::math::Quat::IDENTITY, Vec3::new(1.0, 0.0, 0.0));
        assert!(close(place(&pose.then(child), [0.0; 3]), [10.0, 0.0, 4.0]));
        let mut lines = buffer(64);
        primitive_lines(Shape::Box { half: [1.0, 1.0, 1.0] }, &pose, LevelGroup::Walk.color(), Fill::Lines, &mut lines);
        assert_eq!(lines.count(), 12);
        assert!(lines.vertices().iter().all(|v| [9.0f32, 11.0].iter().any(|x| (x - v.at[0]).abs() < 1e-5)
                                              && [-4.0f32, -6.0].iter().any(|z| (z - v.at[2]).abs() < 1e-5)),
                "in the game's frame, z negated");
        primitive_lines(Shape::Other(24), &pose, LevelGroup::Walk.color(), Fill::Filled, &mut lines);
        assert_eq!(lines.count(), 12, "another shape type draws nothing");
        let walk = LevelGroup::Walk.color();
        primitive_lines(Shape::Box { half: [1.0, 2.0, 3.0] }, &pose, walk, Fill::Filled, &mut lines);
        assert_eq!((lines.count(), lines.triangles()), (24, 12), "a filled box: its edges and its faces' 12 triangles");
        primitive_lines(Shape::Sphere { radius: 1.0 }, &pose, LevelGroup::Walk.color(), Fill::Filled, &mut lines);
        assert_eq!(lines.triangles(), 12, "a round primitive: its outline alone");
    }

    #[test]
    fn triangles_near_the_eye() {
        let big = [[-100.0, 0.0, -100.0], [100.0, 0.0, -100.0], [0.0, 0.0, 100.0]];
        assert!(triangle_near(big, [0.0, 9.0, 0.0], 10.0), "a big triangle under the eye");
        assert!(!triangle_near(big, [0.0, 11.0, 0.0], 10.0));
        let far = [[20.0, 0.0, 0.0], [21.0, 0.0, 0.0], [20.0, 1.0, 0.0]];
        assert!(!triangle_near(far, [0.0; 3], 10.0));
    }

    #[test]
    fn what_the_console_is_told() {
        let said = |reply| reply_lines(reply).map(|(text, suffix)| format!("{text}{suffix}")).collect::<Vec<_>>();
        let on = |only, fill| Reply::On(Filter { only, fill });
        let mut all = vec!["collision: on, all (walk green, shot magenta, solid white, anomaly red)".to_string()];
        all.extend(HELP.map(String::from));
        assert_eq!(said(Reply::On(Filter::ALL)), all);
        assert_eq!(HELP[4], "  ... fill          - after any of these (or alone): surfaces filled, to find holes");
        assert_eq!(said(on(None, Fill::Filled))[0],
                   "collision: on, all (walk green, shot magenta, solid white, anomaly red), filled");
        assert_eq!(said(on(Some(LevelGroup::Anomaly), Fill::Lines)), ["collision: on, anomaly (red, dim when off)"]);
        assert_eq!(said(on(Some(LevelGroup::Walk), Fill::Lines)), ["collision: on, walk (green + white)"]);
        assert_eq!(said(on(Some(LevelGroup::Walk), Fill::Filled)), ["collision: on, walk (green + white), filled"]);
        assert_eq!(said(on(Some(LevelGroup::Shot), Fill::Lines)), ["collision: on, shot (magenta + white)"]);
        assert_eq!(said(on(Some(LevelGroup::Solid), Fill::Lines)), ["collision: on, solid (white)"]);
        assert_eq!(said(Reply::Off), ["collision: off"]);
        assert_eq!(said(Reply::Usage)[0], "collision: collision [walk | shot | solid | anomaly] [fill]");
        assert_eq!(said(Reply::Usage)[1..], HELP);
        assert_eq!(said(Reply::NoRoom), ["collision: no room for its lines, still off"]);
        assert_eq!(said(Reply::Refused), ["collision: only in private rooms and replays"]);
        assert_eq!(said(Reply::HiddenHere), ["collision: only in private rooms and replays: nothing drawn in this match"]);
        let census: Vec<_> = core::iter::once(None)
            .chain(GROUPS.map(Some))
            .map(|group| {
                let mut line = Line::new();
                describe_census(Census([541, 511, 105, 12]), group, &mut line);
                line.text().to_string()
            })
            .collect();
        assert_eq!(census, [
            "collision: the level's static collision within 20 m (less where that is over 131072 lines):",
            "collision: walk (green) 541 in the world: stops walkers only",
            "collision: shot (magenta) 511 in the world: stops bullets and the camera only",
            "collision: solid (white) 105 in the world: stops walkers, bullets and the camera",
            "collision: anomaly (red) 12 in the world: damage zones, dim red while off",
        ]);
    }

    #[test]
    fn colours_packed_red_lowest() {
        assert_eq!(LevelGroup::Walk.color().packed().to_le_bytes(), [0, 255, 0, 255]);
        assert_eq!(LevelGroup::Shot.color().packed().to_le_bytes(), [255, 0, 255, 255]);
        assert_eq!(LevelGroup::Solid.color().packed().to_le_bytes(), [255; 4]);
        assert_eq!(LineColor::opaque(255, 0, 0).packed(), u32::from_be_bytes([255, 0, 0, 255]),
                   "the stages' red: 0xFF0000FF");
    }
}
