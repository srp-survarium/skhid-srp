//! Patch 156: the console command `collision` draws the level's collision
//! geometry near the camera as lines, each filter group in its own colour
//! (docs/notes/collision-view.md; client_patch.md section 3.82).
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

#![forbid(unsafe_code)]

use core::f32::consts::PI;
use core::ffi::CStr;

use ::ragdoll::math::sin_cos;

use crate::vostok::camera;
use crate::vostok::console::{self, Command, ConsoleArguments, ConsoleCommand, Line};
use crate::vostok::debug_lines::{DebugLines, LineBuffer, LineColor};
use crate::vostok::geometry::{Distance, Pose, WorldPoint};
use crate::vostok::log::{self, Piece};
use crate::vostok::shape::{Shape, Triangle, TriangleMethods, TriangleSink};
use crate::vostok::world::{Motion, PhysicsWorld};
use crate::vostok::{Context, Flow, Lock, NoExit, OrigCcDelegateExecute, OrigTriangleProcess, Thread,
                    collision_view_methods_vtable_handle, collision_view_triangles_vtable_handle};

const NAME: &CStr = c"collision";

static COMMAND: Command = Command::new();

static STATE: Lock<Collision> = Lock::new(Collision { view: View::Off, lines: None });

/// The radii tried (m), the first whose lines fit the cap drawn.
const RADII: [f32; 3] = [20.0, 10.0, 5.0];
/// The most lines a frame draws (2 MiB of vertices, made the first time
/// the view is turned on and kept).
const MOST: usize = 64 * 1024;
/// A circle's segments.
const CIRCLE: usize = 12;

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
}

const GROUPS: [LevelGroup; 3] = [LevelGroup::Walk, LevelGroup::Shot, LevelGroup::Solid];

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
        }
    }

    fn name(self) -> &'static str {
        ["walk", "shot", "solid"][self.index()]
    }

    /// Green, magenta and white: the same whichever of red and blue the
    /// vertex format puts first.
    fn color(self) -> LineColor {
        [LineColor::opaque(0, 255, 0), LineColor::opaque(255, 0, 255), LineColor::opaque(255, 255, 255)][self.index()]
    }

    fn color_name(self) -> &'static str {
        ["green", "magenta", "white"][self.index()]
    }

    fn meets(self) -> &'static str {
        ["stops walkers only", "stops bullets and the camera only", "stops walkers, bullets and the camera"][self.index()]
    }
}

/// Whether `group` is drawn under the filter `only` (none: every group): a
/// filter shows everything that stops what it names, so `walk` and `shot`
/// show `solid` too.
fn shows(only: Option<LevelGroup>, group: LevelGroup) -> bool {
    match only {
        None => true,
        Some(only) => only == group || group == LevelGroup::Solid,
    }
}

/// The level group of a static object of one; none for anything else.
fn level_group(group: u16, motion: Motion) -> Option<LevelGroup> {
    match motion {
        Motion::Static => LevelGroup::of(group),
        Motion::Moving => None,
    }
}

/// What the command was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    /// No argument.
    Toggle,
    Only(LevelGroup),
    Unknown,
}

impl Request {
    fn parse(text: &[u8]) -> Request {
        let text = text.trim_ascii();
        if text.is_empty() {
            return Request::Toggle;
        }
        GROUPS.into_iter().find(|group| text.eq_ignore_ascii_case(group.name().as_bytes()))
            .map_or(Request::Unknown, Request::Only)
    }
}

/// A line of Bullet's frame into `lines`, in the game's (z negated).
fn push(lines: &mut LineBuffer, [ax, ay, az]: [f32; 3], [bx, by, bz]: [f32; 3], color: LineColor) {
    lines.push([ax, ay, -az], [bx, by, -bz], color);
}

/// How many objects of each level group the world holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Census([u32; 3]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Off,
    /// Turned on: its first frame tells the census.
    Starting { only: Option<LevelGroup> },
    On { only: Option<LevelGroup> },
}

/// The view and the lines it read (none while it is off; their room made
/// the first time it is turned on).
struct Collision {
    view: View,
    lines: Option<LineBuffer>,
}

/// What the console is told of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reply {
    On(Option<LevelGroup>),
    Off,
    Usage,
    /// No room for the lines: still off.
    NoRoom,
}

/// What a frame reads: the level near the eye.
trait Level {
    fn census(&self) -> Census;
    /// The lines of the objects of `only` (or every level group) within
    /// `reach` of the eye.
    fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut LineBuffer);
}

impl Collision {
    /// `make` gives the lines' room when the view first turns on.
    fn command(&mut self, request: Request, make: impl FnOnce() -> Option<LineBuffer>) -> Reply {
        let (view, reply) = match (self.view, request) {
            (View::Off, Request::Toggle) => (View::Starting { only: None }, Reply::On(None)),
            (_, Request::Toggle) => (View::Off, Reply::Off),
            (View::Starting { only: Some(shown) } | View::On { only: Some(shown) }, Request::Only(group))
                if shown == group => (View::Off, Reply::Off),
            (_, Request::Only(group)) => (View::Starting { only: Some(group) }, Reply::On(Some(group))),
            (view, Request::Unknown) => (view, Reply::Usage),
        };
        if view != View::Off && self.lines.is_none() {
            self.lines = make();
            if self.lines.is_none() {
                return Reply::NoRoom;
            }
        }
        self.view = view;
        if let (View::Off, Some(lines)) = (view, &mut self.lines) {
            lines.clear();
        }
        reply
    }

    /// The filter shown (none: every group, or off).
    fn only(&self) -> Option<LevelGroup> {
        match self.view {
            View::Off => None,
            View::Starting { only } | View::On { only } => only,
        }
    }

    /// A frame's camera: the lines of the first radius that fits the cap
    /// (the smallest's, cut at the cap, when none does); the census on the
    /// first frame.
    fn frame(&mut self, level: &impl Level) -> Option<Census> {
        let (only, census) = match self.view {
            View::Off => return None,
            View::Starting { only } => (only, Some(level.census())),
            View::On { only } => (only, None),
        };
        let lines = self.lines.as_mut()?;
        self.view = View::On { only };
        for reach in RADII {
            lines.clear();
            level.outline(reach, only, lines);
            if !lines.is_full() {
                break;
            }
        }
        census
    }
}

/// The camera's player's physics world around the eye (Bullet's frame).
struct LiveLevel<'w, 't> {
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
        census
    }

    fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut LineBuffer) {
        let [x, y, z] = self.eye;
        let (Some(eye), Some(distance)) = (WorldPoint::parse([x, y, -z]), Distance::parse(reach)) else { return };
        let near = Near { eye: self.eye, reach, triangles: self.triangles };
        self.world.for_each_object(|object| {
            let Some(group) = level_group(object.group, object.motion) else { return };
            if lines.is_full() || !shows(only, group) || !object.aabb.within(eye, distance) {
                return;
            }
            if let Some(shape) = object.shape() {
                outline(shape, &object.pose(), group.color(), &near, lines);
            }
        });
    }
}

/// Where a mesh's triangles are asked for: within `reach` of `eye`.
struct Near {
    eye: [f32; 3],
    reach: f32,
    triangles: TriangleMethods,
}

/// A shape's lines where `pose` puts it.
fn outline(shape: Shape<'_>, pose: &Pose, color: LineColor, near: &Near, lines: &mut LineBuffer) {
    match shape {
        Shape::Mesh(mesh) => {
            let eye = pose.local(near.eye);
            let low = eye.map(|v| v - near.reach);
            let high = eye.map(|v| v + near.reach);
            let mut previous = None;
            mesh.triangles_in(near.triangles, low, high, &mut |triangle| {
                if triangle_near(triangle, eye, near.reach) {
                    for (a, b) in new_edges(triangle, previous) {
                        push(lines, pose.apply(a), pose.apply(b), color);
                    }
                    previous = Some(triangle);
                }
            });
        }
        Shape::Compound(compound) => {
            compound.for_each_child(|child, shape| outline(shape, &pose.then(&child), color, near, lines))
        }
        primitive => primitive_lines(primitive, pose, color, lines),
    }
}

/// The triangle's edges but one it shares with the triangle drawn just
/// before it (a strip's: the same two corners).
fn new_edges([a, b, c]: [[f32; 3]; 3], previous: Option<[[f32; 3]; 3]>) -> impl Iterator<Item = ([f32; 3], [f32; 3])> {
    let shared = move |p: [f32; 3], q: [f32; 3]| previous.is_some_and(|t| t.contains(&p) && t.contains(&q));
    [(a, b), (b, c), (c, a)].into_iter().filter(move |&(p, q)| !shared(p, q))
}

/// A box's, sphere's, capsule's or cylinder's outline where `pose` puts it.
fn primitive_lines(shape: Shape<'_>, pose: &Pose, color: LineColor, lines: &mut LineBuffer) {
    let mut line = |a: [f32; 3], b: [f32; 3]| push(lines, pose.apply(a), pose.apply(b), color);
    match shape {
        Shape::Box { half } => box_lines(half, &mut line),
        Shape::Sphere { radius } => sphere_lines(radius, &mut line),
        Shape::Capsule { up, radius, half_height } => capsule_lines(up, radius, half_height, &mut line),
        Shape::Cylinder { radius, half_height } => cylinder_lines(radius, half_height, &mut line),
        Shape::Mesh(_) | Shape::Compound(_) | Shape::Other(_) => {}
    }
}

/// Whether the triangle's box is within `reach` of `eye`.
fn triangle_near(corners: [[f32; 3]; 3], eye: [f32; 3], reach: f32) -> bool {
    let square: f32 = (0..3)
        .map(|k| {
            let low = corners[0][k].min(corners[1][k]).min(corners[2][k]);
            let high = corners[0][k].max(corners[1][k]).max(corners[2][k]);
            let d = (low - eye[k]).max(eye[k] - high).max(0.0);
            d * d
        })
        .sum();
    square <= reach * reach
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

/// The console executes `collision`.
pub fn on_execute(_: &OrigCcDelegateExecute, command: &mut ConsoleCommand<'_>, arguments: &ConsoleArguments<'_>) {
    let thread = command.thread();
    let request = Request::parse(arguments.text().map_or(&[][..], CStr::to_bytes));
    let reply = STATE.with(thread, |state| state.command(request, || LineBuffer::make(thread, MOST)));
    for text in reply_lines(reply) {
        let mut line = Line::new();
        line.push(text);
        tell(thread, &line);
    }
}

/// update_camera after the view's update: the lines near the eye made.
pub fn on_viewed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if STATE.with(thread, |state| state.view == View::Off) {
        return Flow::Continue;
    }
    let (Some(player), Some(view)) = (camera::viewer(ctx), camera::viewed(ctx)) else { return Flow::Continue };
    let Some(world) = player.physics_world() else { return Flow::Continue };
    let [x, y, z] = view.eye;
    let level = LiveLevel { world: &world, eye: [x, y, -z], triangles: collision_view_triangles_vtable_handle() };
    if let Some((census, only)) = STATE.with(thread, |state| Some((state.frame(&level)?, state.only()))) {
        let shown = GROUPS.into_iter().filter(|&group| shows(only, group));
        for group in core::iter::once(None).chain(shown.map(Some)) {
            let mut line = Line::new();
            describe_census(census, group, &mut line);
            tell(thread, &line);
        }
    }
    Flow::Continue
}

/// processAllTriangles' callback of ours: the triangle to its query.
pub fn on_triangle(_: &OrigTriangleProcess, sink: &mut TriangleSink<'_>, triangle: &Triangle<'_>, _: u32, _: u32) {
    sink.deliver(triangle);
}

/// renderer::render's depth-tested debug lines: the last lines read, drawn
/// once.
pub fn on_debug_lines(ctx: &mut Context<'_>) -> Flow<NoExit> {
    STATE.with(ctx.thread(), |state| {
        let Some(lines) = state.lines.as_mut().filter(|lines| lines.count() > 0) else { return };
        if let Some(debug) = DebugLines::at(ctx) {
            debug.draw(lines);
        }
        lines.clear();
    });
    Flow::Continue
}

/// A line to the console (the game's log) and to srp_patches.log.
fn tell(thread: &Thread, line: &Line) {
    console::game_line(thread, line.c_str());
    log::line(&[Piece::Text(line.text())]);
}

/// The options, after `collision` turns the view on and after a wrong argument.
const HELP: [&str; 4] = [
    "  collision walk  - what stops walking (green + white)",
    "  collision shot  - what stops bullets and the camera (magenta + white)",
    "  collision solid - what stops both (white)",
    "  collision       - again to turn off",
];

/// What the console is told of a command, a line each.
fn reply_lines(reply: Reply) -> impl Iterator<Item = &'static str> {
    let (first, help): (&str, &[&str]) = match reply {
        Reply::On(None) => ("collision: on, all (walk green, shot magenta, solid white)", &HELP),
        Reply::On(Some(LevelGroup::Walk)) => ("collision: on, walk (green + white)", &[]),
        Reply::On(Some(LevelGroup::Shot)) => ("collision: on, shot (magenta + white)", &[]),
        Reply::On(Some(LevelGroup::Solid)) => ("collision: on, solid (white)", &[]),
        Reply::Off => ("collision: off", &[]),
        Reply::Usage => ("collision: collision [walk | shot | solid]", &HELP),
        Reply::NoRoom => ("collision: no room for its lines, still off", &[]),
    };
    core::iter::once(first).chain(help.iter().copied())
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
        fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut LineBuffer) {
            if !shows(only, LevelGroup::Walk) {
                return;
            }
            for i in 0..(self.lines)(reach) {
                push(lines, [i as f32, 0.0, 1.0], [i as f32, 1.0, 2.0], LevelGroup::Walk.color());
            }
        }
    }

    fn buffer(lines: usize) -> LineBuffer {
        let vertices = Box::leak(vec![LineVertex::default(); 2 * lines].into_boxed_slice());
        let indices = Box::leak((0..32 * 1024).map(|i| i as u16).collect::<Vec<_>>().into_boxed_slice());
        LineBuffer::over(vertices, indices)
    }

    fn state() -> Collision {
        Collision { view: View::Off, lines: None }
    }

    fn room() -> Option<LineBuffer> {
        Some(buffer(MOST))
    }

    fn count(s: &Collision) -> usize {
        s.lines.as_ref().map_or(0, LineBuffer::count)
    }

    fn fake(lines: fn(f32) -> usize) -> Fake {
        Fake { census: Census([541, 511, 105]), lines }
    }

    #[test]
    fn requests_as_typed() {
        assert_eq!(Request::parse(b""), Request::Toggle);
        assert_eq!(Request::parse(b"  "), Request::Toggle);
        assert_eq!(Request::parse(b"walk"), Request::Only(LevelGroup::Walk));
        assert_eq!(Request::parse(b" shot "), Request::Only(LevelGroup::Shot));
        assert_eq!(Request::parse(b"SOLID"), Request::Only(LevelGroup::Solid));
        for other in [&b"2"[..], b"0x8", b"a", b"walker", b"walk shot", b"all"] {
            assert_eq!(Request::parse(other), Request::Unknown, "{other:?}");
        }
    }

    #[test]
    fn the_command_turns_it_on_and_off() {
        let mut s = state();
        assert_eq!(s.command(Request::Toggle, room), Reply::On(None));
        assert_eq!(s.view, View::Starting { only: None });
        assert_eq!(s.command(Request::Unknown, room), Reply::Usage);
        assert_eq!(s.view, View::Starting { only: None });
        s.frame(&fake(|_| 3));
        assert_eq!(s.view, View::On { only: None });
        assert_eq!(s.command(Request::Only(LevelGroup::Shot), room), Reply::On(Some(LevelGroup::Shot)));
        assert_eq!(s.view, View::Starting { only: Some(LevelGroup::Shot) });
        s.frame(&fake(|_| 3));
        assert_eq!(s.command(Request::Only(LevelGroup::Walk), room), Reply::On(Some(LevelGroup::Walk)), "another name: switched");
        assert_eq!(s.command(Request::Only(LevelGroup::Walk), room), Reply::Off, "the shown name again: off");
        assert_eq!(s.view, View::Off);
        s.command(Request::Only(LevelGroup::Shot), room);
        assert_eq!(s.command(Request::Toggle, room), Reply::Off);
        assert_eq!((s.view, count(&s)), (View::Off, 0), "off: the lines read dropped");
    }

    #[test]
    fn off_reads_and_draws_nothing() {
        let mut s = state();
        assert_eq!(s.frame(&fake(|_| 3)), None);
        assert_eq!(count(&s), 0);
    }

    #[test]
    fn the_census_on_the_first_frame_only() {
        let mut s = state();
        s.command(Request::Toggle, room);
        assert_eq!(s.frame(&fake(|_| 3)), Some(Census([541, 511, 105])));
        assert_eq!(s.frame(&fake(|_| 3)), None);
        assert_eq!(count(&s), 3, "each frame reads its own lines anew");
    }

    #[test]
    fn the_largest_radius_that_fits() {
        let mut s = state();
        s.command(Request::Toggle, room);
        s.frame(&fake(|reach| if reach > 15.0 { MOST + 1 } else { 40_000 }));
        assert_eq!(count(&s), 40_000, "20 m is over the cap: 10 m's");
        s.frame(&fake(|_| MOST - 1));
        assert_eq!(count(&s), MOST - 1, "under the cap: 20 m's");
        s.frame(&fake(|_| 100_000));
        assert_eq!(count(&s), MOST, "over it at 5 m too: cut at the cap");
    }

    #[test]
    fn no_room_stays_off() {
        let mut s = state();
        assert_eq!(s.command(Request::Toggle, || None), Reply::NoRoom);
        assert_eq!(s.view, View::Off);
        assert_eq!(s.command(Request::Toggle, room), Reply::On(None));
        let mut made = false;
        s.command(Request::Toggle, || { made = true; None });
        s.command(Request::Toggle, || { made = true; None });
        assert!(!made, "the room made once, kept");
        assert_eq!(s.view, View::Starting { only: None });
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
    fn lines_in_the_games_frame() {
        let mut lines = buffer(4);
        push(&mut lines, [1.0, 2.0, 3.0], [4.0, 5.0, -6.0], LevelGroup::Solid.color());
        let v = lines.vertices();
        assert_eq!((v[0].at, v[1].at), ([1.0, 2.0, -3.0], [4.0, 5.0, 6.0]));
        assert_eq!(v[0].color, LevelGroup::Solid.color());
    }

    #[test]
    fn a_filter_shows_what_stops_it() {
        let shown = |only| GROUPS.into_iter().filter(|&g| shows(only, g)).collect::<Vec<_>>();
        assert_eq!(shown(None), GROUPS);
        assert_eq!(shown(Some(LevelGroup::Walk)), [LevelGroup::Walk, LevelGroup::Solid]);
        assert_eq!(shown(Some(LevelGroup::Shot)), [LevelGroup::Shot, LevelGroup::Solid]);
        assert_eq!(shown(Some(LevelGroup::Solid)), [LevelGroup::Solid]);
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
        let pose = Pose { rows: [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]], origin: [10.0, 0.0, 5.0] };
        assert_eq!(pose.apply([1.0, 2.0, 3.0]), [13.0, 2.0, 4.0]);
        assert_eq!(pose.local([13.0, 2.0, 4.0]), [1.0, 2.0, 3.0]);
        let child = Pose { rows: Pose::IDENTITY.rows, origin: [1.0, 0.0, 0.0] };
        assert_eq!(pose.then(&child).apply([0.0; 3]), [10.0, 0.0, 4.0]);
        let mut lines = buffer(64);
        primitive_lines(Shape::Box { half: [1.0, 1.0, 1.0] }, &pose, LevelGroup::Walk.color(), &mut lines);
        assert_eq!(lines.count(), 12);
        assert!(lines.vertices().iter().all(|v| [9.0, 11.0].contains(&v.at[0]) && [-4.0, -6.0].contains(&v.at[2])),
                "in the game's frame, z negated");
        primitive_lines(Shape::Other(24), &pose, LevelGroup::Walk.color(), &mut lines);
        assert_eq!(lines.count(), 12, "another shape type draws nothing");
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
        let said = |reply| reply_lines(reply).collect::<Vec<_>>();
        assert_eq!(said(Reply::On(None)), [
            "collision: on, all (walk green, shot magenta, solid white)",
            "  collision walk  - what stops walking (green + white)",
            "  collision shot  - what stops bullets and the camera (magenta + white)",
            "  collision solid - what stops both (white)",
            "  collision       - again to turn off",
        ]);
        assert_eq!(said(Reply::On(Some(LevelGroup::Walk))), ["collision: on, walk (green + white)"]);
        assert_eq!(said(Reply::On(Some(LevelGroup::Shot))), ["collision: on, shot (magenta + white)"]);
        assert_eq!(said(Reply::On(Some(LevelGroup::Solid))), ["collision: on, solid (white)"]);
        assert_eq!(said(Reply::Off), ["collision: off"]);
        assert_eq!(said(Reply::Usage)[0], "collision: collision [walk | shot | solid]");
        assert_eq!(said(Reply::Usage)[1..], HELP);
        assert_eq!(said(Reply::NoRoom), ["collision: no room for its lines, still off"]);
        let census: Vec<_> = [None, Some(LevelGroup::Walk), Some(LevelGroup::Shot), Some(LevelGroup::Solid)]
            .into_iter()
            .map(|group| {
                let mut line = Line::new();
                describe_census(Census([541, 511, 105]), group, &mut line);
                line.text().to_string()
            })
            .collect();
        assert_eq!(census, [
            "collision: the level's static collision within 20 m (less where that is over 65536 lines):",
            "collision: walk (green) 541 in the world: stops walkers only",
            "collision: shot (magenta) 511 in the world: stops bullets and the camera only",
            "collision: solid (white) 105 in the world: stops walkers, bullets and the camera",
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
