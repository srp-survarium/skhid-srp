//! Patch 156: the console command `collision` draws the level's collision
//! geometry near the camera as lines, each filter group in its own colour
//! (docs/notes/collision-view.md; client_patch.md section 3.82).
//!
//! The command (a cc_delegate of ours, as patch 133's `where`) turns the view
//! on and off; `collision 2`, `8` or `a` shows that group alone. While it is
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
use crate::vostok::debug_lines::{DebugLines, LineColor, LineVertex};
use crate::vostok::geometry::{Distance, Pose, WorldPoint};
use crate::vostok::log::{self, Piece};
use crate::vostok::shape::{Shape, Triangle, TriangleMethods, TriangleSink};
use crate::vostok::world::{Motion, PhysicsWorld};
use crate::vostok::{Context, Flow, Lock, NoExit, OrigCcDelegateExecute, OrigTriangleProcess, Thread,
                    collision_view_methods_vtable_handle, collision_view_triangles_vtable_handle};

const NAME: &CStr = c"collision";

static COMMAND: Command = Command::new();

static STATE: Lock<Collision> = Lock::new(Collision { view: View::Off, lines: Lines::new() });

/// The radii tried (m), the first whose lines fit the cap drawn.
const RADII: [f32; 3] = [10.0, 5.0, 2.5];
/// The most lines a frame draws.
const MOST: usize = 6144;
/// A circle's segments.
const CIRCLE: usize = 12;

/// A group of the level's collision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LevelGroup {
    /// 2: what walkers meet and bullets pass (walker meshes, invisible walls,
    /// walkable lids).
    Walkers,
    /// 8: what bullets and the camera meet and walkers pass (the hittable
    /// meshes, camera walls).
    Bullets,
    /// 0Ah: both.
    Both,
}

const GROUPS: [LevelGroup; 3] = [LevelGroup::Walkers, LevelGroup::Bullets, LevelGroup::Both];

impl LevelGroup {
    fn of(group: u16) -> Option<LevelGroup> {
        match group {
            0x2 => Some(LevelGroup::Walkers),
            0x8 => Some(LevelGroup::Bullets),
            0xA => Some(LevelGroup::Both),
            _ => None,
        }
    }

    fn index(self) -> usize {
        match self {
            LevelGroup::Walkers => 0,
            LevelGroup::Bullets => 1,
            LevelGroup::Both => 2,
        }
    }

    fn name(self) -> &'static str {
        ["0x2", "0x8", "0xa"][self.index()]
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
        ["walkers only", "bullets and the camera only", "walkers, bullets and the camera"][self.index()]
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
        let digits = text.strip_prefix(b"0x").or_else(|| text.strip_prefix(b"0X")).unwrap_or(text);
        match digits {
            [] if text.is_empty() => Request::Toggle,
            [b'2'] => Request::Only(LevelGroup::Walkers),
            [b'8'] => Request::Only(LevelGroup::Bullets),
            [b'a' | b'A'] => Request::Only(LevelGroup::Both),
            _ => Request::Unknown,
        }
    }
}

/// The lines of the last frame read, in the game's frame, until the render
/// thread draws them.
struct Lines {
    vertices: [LineVertex; 2 * MOST],
    len: usize,
}

impl Lines {
    const fn new() -> Lines {
        Lines { vertices: [LineVertex::NONE; 2 * MOST], len: 0 }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn count(&self) -> usize {
        self.len / 2
    }

    fn is_full(&self) -> bool {
        self.count() == MOST
    }

    /// A line from `a` to `b` (Bullet's frame); none past the cap.
    fn push(&mut self, a: [f32; 3], b: [f32; 3], color: LineColor) {
        if self.is_full() {
            return;
        }
        let game = |[x, y, z]: [f32; 3]| LineVertex { at: [x, y, -z], color };
        self.vertices[self.len] = game(a);
        self.vertices[self.len + 1] = game(b);
        self.len += 2;
    }

    fn vertices(&self) -> &[LineVertex] {
        &self.vertices[..self.len]
    }
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

/// The view and the lines it read (none while it is off).
struct Collision {
    view: View,
    lines: Lines,
}

/// What the console is told of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reply {
    On(Option<LevelGroup>),
    Off,
    Usage,
}

/// What a frame reads: the level near the eye.
trait Level {
    fn census(&self) -> Census;
    /// The lines of the objects of `only` (or every level group) within
    /// `reach` of the eye.
    fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut Lines);
}

impl Collision {
    fn command(&mut self, request: Request) -> Reply {
        let (view, reply) = match (self.view, request) {
            (View::Off, Request::Toggle) => (View::Starting { only: None }, Reply::On(None)),
            (_, Request::Toggle) => (View::Off, Reply::Off),
            (_, Request::Only(group)) => (View::Starting { only: Some(group) }, Reply::On(Some(group))),
            (view, Request::Unknown) => (view, Reply::Usage),
        };
        self.view = view;
        if view == View::Off {
            self.lines.clear();
        }
        reply
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
        self.view = View::On { only };
        for reach in RADII {
            self.lines.clear();
            level.outline(reach, only, &mut self.lines);
            if !self.lines.is_full() {
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

    fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut Lines) {
        let [x, y, z] = self.eye;
        let (Some(eye), Some(distance)) = (WorldPoint::parse([x, y, -z]), Distance::parse(reach)) else { return };
        let near = Near { eye: self.eye, reach, triangles: self.triangles };
        self.world.for_each_object(|object| {
            let Some(group) = level_group(object.group, object.motion) else { return };
            if lines.is_full() || only.is_some_and(|only| only != group) || !object.aabb.within(eye, distance) {
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
fn outline(shape: Shape<'_>, pose: &Pose, color: LineColor, near: &Near, lines: &mut Lines) {
    match shape {
        Shape::Mesh(mesh) => {
            let eye = pose.local(near.eye);
            let low = eye.map(|v| v - near.reach);
            let high = eye.map(|v| v + near.reach);
            mesh.triangles_in(near.triangles, low, high, &mut |[a, b, c]| {
                if triangle_near([a, b, c], eye, near.reach) {
                    for (from, to) in [(a, b), (b, c), (c, a)] {
                        lines.push(pose.apply(from), pose.apply(to), color);
                    }
                }
            });
        }
        Shape::Compound(compound) => {
            compound.for_each_child(|child, shape| outline(shape, &pose.then(&child), color, near, lines))
        }
        primitive => primitive_lines(primitive, pose, color, lines),
    }
}

/// A box's, sphere's, capsule's or cylinder's outline where `pose` puts it.
fn primitive_lines(shape: Shape<'_>, pose: &Pose, color: LineColor, lines: &mut Lines) {
    let mut line = |a: [f32; 3], b: [f32; 3]| lines.push(pose.apply(a), pose.apply(b), color);
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
    let reply = STATE.with(thread, |state| state.command(request));
    let mut line = Line::new();
    describe_reply(reply, &mut line);
    tell(thread, &line);
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
    if let Some(census) = STATE.with(thread, |state| state.frame(&level)) {
        for at in 0..=GROUPS.len() {
            let mut line = Line::new();
            describe_census(census, at, &mut line);
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
        if state.lines.count() > 0 {
            if let Some(lines) = DebugLines::at(ctx) {
                lines.draw(state.lines.vertices());
            }
            state.lines.clear();
        }
    });
    Flow::Continue
}

/// A line to the console (the game's log) and to srp_patches.log.
fn tell(thread: &Thread, line: &Line) {
    console::game_line(thread, line.c_str());
    log::line(&[Piece::Text(line.text())]);
}

fn describe_reply(reply: Reply, line: &mut Line) {
    line.push("collision: ");
    match reply {
        Reply::On(None) => line.push("on"),
        Reply::On(Some(group)) => {
            line.push("on, ");
            line.push(group.name());
            line.push(" only");
        }
        Reply::Off => line.push("off"),
        Reply::Usage => line.push("collision [2 | 8 | a] (walkers, bullets, both)"),
    }
}

/// The census's line `at`: its head, then a line a group.
fn describe_census(census: Census, at: usize, line: &mut Line) {
    line.push("collision: ");
    let Some(group) = at.checked_sub(1).and_then(|i| GROUPS.get(i)) else {
        line.push("the level's static collision within ");
        line.number(RADII[0] as u32);
        line.push(" m (less where that is over ");
        line.number(MOST as u32);
        line.push(" lines), a colour a group:");
        return;
    };
    line.push(group.name());
    line.push(" ");
    line.push(group.color_name());
    line.push(", ");
    line.number(census.0[group.index()]);
    line.push(" in the world (");
    line.push(group.meets());
    line.push(")");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up level: `lines(reach)` lines a radius, of the walkers' group.
    struct Fake {
        census: Census,
        lines: fn(f32) -> usize,
    }

    impl Level for Fake {
        fn census(&self) -> Census {
            self.census
        }
        fn outline(&self, reach: f32, only: Option<LevelGroup>, lines: &mut Lines) {
            if only.is_some_and(|only| only != LevelGroup::Walkers) {
                return;
            }
            for i in 0..(self.lines)(reach) {
                lines.push([i as f32, 0.0, 1.0], [i as f32, 1.0, 2.0], LevelGroup::Walkers.color());
            }
        }
    }

    fn state() -> Box<Collision> {
        Box::new(Collision { view: View::Off, lines: Lines::new() })
    }

    fn fake(lines: fn(f32) -> usize) -> Fake {
        Fake { census: Census([541, 511, 105]), lines }
    }

    #[test]
    fn requests_as_typed() {
        assert_eq!(Request::parse(b""), Request::Toggle);
        assert_eq!(Request::parse(b"  "), Request::Toggle);
        assert_eq!(Request::parse(b"2"), Request::Only(LevelGroup::Walkers));
        assert_eq!(Request::parse(b" 0x8 "), Request::Only(LevelGroup::Bullets));
        assert_eq!(Request::parse(b"0XA"), Request::Only(LevelGroup::Both));
        assert_eq!(Request::parse(b"a"), Request::Only(LevelGroup::Both));
        for other in [&b"0x"[..], b"10", b"40", b"0x824", b"walkers", b"2 8"] {
            assert_eq!(Request::parse(other), Request::Unknown, "{other:?}");
        }
    }

    #[test]
    fn the_command_turns_it_on_and_off() {
        let mut s = state();
        assert_eq!(s.command(Request::Toggle), Reply::On(None));
        assert_eq!(s.view, View::Starting { only: None });
        assert_eq!(s.command(Request::Unknown), Reply::Usage);
        assert_eq!(s.view, View::Starting { only: None });
        s.frame(&fake(|_| 3));
        assert_eq!(s.view, View::On { only: None });
        assert_eq!(s.command(Request::Only(LevelGroup::Bullets)), Reply::On(Some(LevelGroup::Bullets)));
        assert_eq!(s.view, View::Starting { only: Some(LevelGroup::Bullets) });
        assert_eq!(s.command(Request::Toggle), Reply::Off);
        assert_eq!((s.view, s.lines.count()), (View::Off, 0), "off: the lines read dropped");
    }

    #[test]
    fn off_reads_and_draws_nothing() {
        let mut s = state();
        assert_eq!(s.frame(&fake(|_| 3)), None);
        assert_eq!(s.lines.count(), 0);
    }

    #[test]
    fn the_census_on_the_first_frame_only() {
        let mut s = state();
        s.command(Request::Toggle);
        assert_eq!(s.frame(&fake(|_| 3)), Some(Census([541, 511, 105])));
        assert_eq!(s.frame(&fake(|_| 3)), None);
        assert_eq!(s.lines.count(), 3, "each frame reads its own lines anew");
    }

    #[test]
    fn the_largest_radius_that_fits() {
        let mut s = state();
        s.command(Request::Toggle);
        s.frame(&fake(|reach| if reach > 6.0 { MOST + 1 } else { 4000 }));
        assert_eq!(s.lines.count(), 4000, "10 m is over the cap: 5 m's");
        s.frame(&fake(|_| MOST - 1));
        assert_eq!(s.lines.count(), MOST - 1, "under the cap: 10 m's");
        s.frame(&fake(|_| 50_000));
        assert_eq!(s.lines.count(), MOST, "over it at 2.5 m too: cut at the cap");
    }

    #[test]
    fn lines_in_the_games_frame() {
        let mut lines = Box::new(Lines::new());
        lines.push([1.0, 2.0, 3.0], [4.0, 5.0, -6.0], LevelGroup::Both.color());
        let v = lines.vertices();
        assert_eq!((v[0].at, v[1].at), ([1.0, 2.0, -3.0], [4.0, 5.0, 6.0]));
        assert_eq!(v[0].color, LevelGroup::Both.color());
    }

    #[test]
    fn only_static_level_groups() {
        for (group, motion, want) in [
            (0x2, Motion::Static, Some(LevelGroup::Walkers)),
            (0x8, Motion::Static, Some(LevelGroup::Bullets)),
            (0xA, Motion::Static, Some(LevelGroup::Both)),
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
        let mut lines = Box::new(Lines::new());
        primitive_lines(Shape::Box { half: [1.0, 1.0, 1.0] }, &pose, LevelGroup::Walkers.color(), &mut lines);
        assert_eq!(lines.count(), 12);
        assert!(lines.vertices().iter().all(|v| [9.0, 11.0].contains(&v.at[0]) && [-4.0, -6.0].contains(&v.at[2])),
                "in the game's frame, z negated");
        primitive_lines(Shape::Other(24), &pose, LevelGroup::Walkers.color(), &mut lines);
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
        let said = |reply| {
            let mut line = Line::new();
            describe_reply(reply, &mut line);
            line.text().to_string()
        };
        assert_eq!(said(Reply::On(None)), "collision: on");
        assert_eq!(said(Reply::On(Some(LevelGroup::Both))), "collision: on, 0xa only");
        assert_eq!(said(Reply::Off), "collision: off");
        assert_eq!(said(Reply::Usage), "collision: collision [2 | 8 | a] (walkers, bullets, both)");
        let census: Vec<_> = (0..4)
            .map(|at| {
                let mut line = Line::new();
                describe_census(Census([541, 511, 105]), at, &mut line);
                line.text().to_string()
            })
            .collect();
        assert_eq!(census, [
            "collision: the level's static collision within 10 m (less where that is over 6144 lines), a colour a group:",
            "collision: 0x2 green, 541 in the world (walkers only)",
            "collision: 0x8 magenta, 511 in the world (bullets and the camera only)",
            "collision: 0xa white, 105 in the world (walkers, bullets and the camera)",
        ]);
    }

    #[test]
    fn colours_packed_red_lowest() {
        assert_eq!(LevelGroup::Walkers.color().packed().to_le_bytes(), [0, 255, 0, 255]);
        assert_eq!(LevelGroup::Bullets.color().packed().to_le_bytes(), [255, 0, 255, 255]);
        assert_eq!(LevelGroup::Both.color().packed().to_le_bytes(), [255; 4]);
        assert_eq!(LineColor::opaque(255, 0, 0).packed(), u32::from_be_bytes([255, 0, 0, 255]),
                   "the stages' red: 0xFF0000FF");
    }
}
