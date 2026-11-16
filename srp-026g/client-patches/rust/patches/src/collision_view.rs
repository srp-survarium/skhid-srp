//! Patch 156: the console command `collision` draws the boxes of the level's
//! collision near the camera, each filter group (its whole value) in its own
//! colour (docs/notes/collision-view.md; client_patch.md section 3.82).
//!
//! The command (a cc_delegate of ours, as patch 133's `where`) turns the view
//! on and off; `collision 2`, `8` or `a` shows that group alone. While it is
//! on, each frame's camera (`camera.viewed`, in mt_draw after the
//! simulation, on the thread that steps the physics world) reads the boxes
//! the broadphase holds for the camera's player's physics world, and the
//! render thread's next frame draws them once, among the game's own debug
//! lines (`render.debug_lines`). The first frame after the view is turned on
//! tells how many objects of each group the world holds, a line a group.
//! Each group's boxes are moved in by 1 cm times its place in the colour
//! table, so the same box in two groups shows as two nested outlines.
//!
//! Only the level's collision is drawn: static objects of the walkers' group
//! (2), the bullets' (8) or both (0Ah), the map's `collision_objects`. A
//! player's bodies (the hit model's group 40h, the character controller's
//! 824h) and whatever moves (a battery 0A18h, a grenade 818h) never are, so
//! the view shows nothing the map does not.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::camera;
use crate::vostok::collection::BoundedVec;
use crate::vostok::console::{self, Command, ConsoleArguments, ConsoleCommand, Line};
use crate::vostok::debug_lines::{DebugLines, LineColor};
use crate::vostok::geometry::{Aabb, Distance, WorldPoint};
use crate::vostok::log::{self, Piece};
use crate::vostok::world::{CollisionObject, Motion};
use crate::vostok::{Context, Flow, Lock, NoExit, OrigCcDelegateExecute, Thread, collision_view_methods_vtable_handle};

const NAME: &CStr = c"collision";

static COMMAND: Command = Command::new();

static VIEW: Lock<View> = Lock::new(View::Off);

/// Boxes at most this far from the eye are drawn (m).
const RADIUS: f32 = 50.0;
/// The most boxes a frame draws.
const MOST: usize = 1024;

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

    /// How far its boxes are moved in: 1 cm a place in the table.
    fn inset(self) -> Distance {
        Distance::parse(0.01 * self.index() as f32).expect("a finite inset")
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

/// A box to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Shown {
    group: LevelGroup,
    aabb: Aabb,
}

type Boxes = BoundedVec<Shown, MOST>;

/// How many objects of each level group the world holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Census([u32; 3]);

#[derive(Clone, Copy)]
enum View {
    Off,
    /// Turned on: its first frame tells the census.
    Starting { only: Option<LevelGroup> },
    /// The boxes of the last frame read, until the render thread draws them.
    On { only: Option<LevelGroup>, boxes: Boxes },
}

/// What the console is told of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reply {
    On(Option<LevelGroup>),
    Off,
    Usage,
}

impl View {
    fn command(self, request: Request) -> (View, Reply) {
        match (self, request) {
            (View::Off, Request::Toggle) => (View::Starting { only: None }, Reply::On(None)),
            (_, Request::Toggle) => (View::Off, Reply::Off),
            (_, Request::Only(group)) => (View::Starting { only: Some(group) }, Reply::On(Some(group))),
            (view, Request::Unknown) => (view, Reply::Usage),
        }
    }

    /// A frame's camera at `eye`: the boxes near it read from the world's
    /// objects (`objects` visits each); the census on the first frame.
    fn frame(&mut self, eye: WorldPoint, objects: impl FnOnce(&mut dyn FnMut(CollisionObject))) -> Option<Census> {
        let starting = match *self {
            View::Off => return None,
            View::Starting { only } => {
                *self = View::On { only, boxes: Boxes::new() };
                true
            }
            View::On { .. } => false,
        };
        let View::On { only, boxes } = self else { return None };
        *boxes = Boxes::new();
        let mut census = Census::default();
        let reach = Distance::parse(RADIUS).expect("a finite radius");
        objects(&mut |object| {
            let Some(group) = level_group(object) else { return };
            census.0[group.index()] += 1;
            if only.is_none_or(|only| only == group) && object.aabb.within(eye, reach) {
                let _ = boxes.push(Shown { group, aabb: object.aabb.inset(group.inset()) });
            }
        });
        starting.then_some(census)
    }

    /// The boxes to draw now, once: none when off or none were read since.
    fn take_boxes(&mut self) -> Option<Boxes> {
        match self {
            View::On { boxes, .. } if !boxes.is_empty() => Some(core::mem::take(boxes)),
            _ => None,
        }
    }
}

/// The level group of a static object of one; none for anything else.
fn level_group(object: CollisionObject) -> Option<LevelGroup> {
    match object.motion {
        Motion::Static => LevelGroup::of(object.group),
        Motion::Moving => None,
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
    let reply = VIEW.with(thread, |view| {
        let (next, reply) = view.command(request);
        *view = next;
        reply
    });
    let mut line = Line::new();
    describe_reply(reply, &mut line);
    tell(thread, &line);
}

/// update_camera after the view's update: the boxes near the eye read.
pub fn on_viewed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if VIEW.with(thread, |view| matches!(view, View::Off)) {
        return Flow::Continue;
    }
    let (Some(player), Some(view)) = (camera::viewer(ctx), camera::viewed(ctx)) else { return Flow::Continue };
    let (Some(world), Some(eye)) = (player.physics_world(), WorldPoint::parse(view.eye)) else {
        return Flow::Continue;
    };
    let census = VIEW.with(thread, |view| view.frame(eye, |visit| world.for_each_object(visit)));
    if let Some(census) = census {
        for at in 0..=GROUPS.len() {
            let mut line = Line::new();
            describe_census(census, at, &mut line);
            tell(thread, &line);
        }
    }
    Flow::Continue
}

/// renderer::render's depth-tested debug lines: the last boxes read, drawn.
pub fn on_debug_lines(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(boxes) = VIEW.with(ctx.thread(), View::take_boxes) else { return Flow::Continue };
    if let Some(lines) = DebugLines::at(ctx) {
        boxes.iter().for_each(|shown| lines.draw_box(shown.aabb, shown.group.color()));
    }
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
        line.number(RADIUS as u32);
        line.push(" m, a colour a group:");
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

    fn point(p: [f32; 3]) -> WorldPoint {
        WorldPoint::parse(p).unwrap()
    }

    fn object(group: u16, motion: Motion, low: [f32; 3], high: [f32; 3]) -> CollisionObject {
        CollisionObject { group, motion, aabb: Aabb::parse(point(low), point(high)).unwrap() }
    }

    fn near(group: u16, motion: Motion) -> CollisionObject {
        object(group, motion, [-1.0, 0.0, -1.0], [1.0, 2.0, 1.0])
    }

    fn frame(view: &mut View, objects: &[CollisionObject]) -> Option<Census> {
        view.frame(point([0.0, 1.0, 0.0]), |visit| objects.iter().copied().for_each(visit))
    }

    fn drawn(view: &mut View) -> Vec<Shown> {
        view.take_boxes().map(|boxes| boxes.iter().copied().collect()).unwrap_or_default()
    }

    fn level_inset(object: CollisionObject) -> Aabb {
        object.aabb.inset(level_group(object).unwrap().inset())
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
        let (on, reply) = View::Off.command(Request::Toggle);
        assert_eq!(reply, Reply::On(None));
        assert!(matches!(on, View::Starting { only: None }));
        let (off, reply) = on.command(Request::Toggle);
        assert_eq!(reply, Reply::Off);
        assert!(matches!(off, View::Off));
        let (only, reply) = View::Off.command(Request::Only(LevelGroup::Bullets));
        assert_eq!(reply, Reply::On(Some(LevelGroup::Bullets)));
        assert!(matches!(only, View::Starting { only: Some(LevelGroup::Bullets) }));
        let (same, reply) = only.command(Request::Unknown);
        assert_eq!(reply, Reply::Usage);
        assert!(matches!(same, View::Starting { only: Some(LevelGroup::Bullets) }));
        let mut on = View::Off.command(Request::Toggle).0;
        frame(&mut on, &[near(2, Motion::Static)]);
        assert!(matches!(on.command(Request::Toggle).0, View::Off));
        assert!(matches!(on.command(Request::Only(LevelGroup::Walkers)).0,
                         View::Starting { only: Some(LevelGroup::Walkers) }));
    }

    #[test]
    fn off_reads_and_draws_nothing() {
        let mut view = View::Off;
        assert_eq!(frame(&mut view, &[near(2, Motion::Static)]), None);
        assert!(view.take_boxes().is_none());
    }

    #[test]
    fn only_the_level_is_drawn_never_a_player() {
        let mut view = View::Off.command(Request::Toggle).0;
        let objects = [
            near(0x2, Motion::Static),
            near(0x8, Motion::Static),
            near(0xA, Motion::Static),
            near(0x40, Motion::Static),     // a living player's hit model
            near(0x40, Motion::Moving),
            near(0x824, Motion::Moving),    // a character controller
            near(0xA18, Motion::Moving),    // a battery
            near(0x818, Motion::Moving),    // a grenade
            near(0x2, Motion::Moving),      // anything of a level group that moves
            near(0x20, Motion::Static),     // the lobby's decal markers
        ];
        assert_eq!(frame(&mut view, &objects), Some(Census([1, 1, 1])));
        let groups: Vec<_> = drawn(&mut view).iter().map(|s| s.group).collect();
        assert_eq!(groups, GROUPS);
    }

    #[test]
    fn near_the_eye_only() {
        let mut view = View::Off.command(Request::Toggle).0;
        let objects = [
            object(2, Motion::Static, [49.0, 0.0, 0.0], [60.0, 1.0, 1.0]),       // 49 m: in
            object(2, Motion::Static, [51.0, 0.0, 0.0], [60.0, 1.0, 1.0]),       // 51 m: out
            object(8, Motion::Static, [-500.0, -10.0, -500.0], [500.0, 100.0, 500.0]),  // around the eye: in
            object(8, Motion::Static, [30.0, 1.0, 30.0], [31.0, 1.0, 31.0]),     // flat, 42.4 m
            object(8, Motion::Static, [36.0, 1.0, 36.0], [37.0, 2.0, 37.0]),     // 50.9 m: out
        ];
        assert_eq!(frame(&mut view, &objects), Some(Census([2, 3, 0])));
        let shown = drawn(&mut view);
        let wanted: Vec<_> = [0, 2, 3].iter().map(|&i| level_inset(objects[i])).collect();
        assert_eq!(shown.iter().map(|s| s.aabb).collect::<Vec<_>>(), wanted);
    }

    #[test]
    fn one_group_alone() {
        let mut view = View::Off.command(Request::Only(LevelGroup::Both)).0;
        let objects = [near(0x2, Motion::Static), near(0xA, Motion::Static), near(0x8, Motion::Static)];
        assert_eq!(frame(&mut view, &objects), Some(Census([1, 1, 1])));
        assert_eq!(drawn(&mut view).iter().map(|s| s.group).collect::<Vec<_>>(), [LevelGroup::Both]);
    }

    #[test]
    fn the_census_once_and_the_boxes_drawn_once() {
        let mut view = View::Off.command(Request::Toggle).0;
        let objects = [near(2, Motion::Static)];
        assert!(frame(&mut view, &objects).is_some());
        assert_eq!(frame(&mut view, &objects), None, "the census only on the first frame");
        assert_eq!(drawn(&mut view).len(), 1, "a frame read twice draws its last read once");
        assert!(view.take_boxes().is_none(), "nothing again until the next frame is read");
        frame(&mut view, &objects);
        assert_eq!(drawn(&mut view).len(), 1);
    }

    #[test]
    fn at_most_a_frame_of_boxes() {
        let mut view = View::Off.command(Request::Toggle).0;
        let objects = vec![near(2, Motion::Static); MOST + 10];
        assert_eq!(frame(&mut view, &objects), Some(Census([MOST as u32 + 10, 0, 0])));
        assert_eq!(drawn(&mut view).len(), MOST);
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
        let census: Vec<_> = (0..4).map(|at| {
            let mut line = Line::new();
            describe_census(Census([541, 511, 105]), at, &mut line);
            line.text().to_string()
        }).collect();
        assert_eq!(census, [
            "collision: the level's static collision within 50 m, a colour a group:",
            "collision: 0x2 green, 541 in the world (walkers only)",
            "collision: 0x8 magenta, 511 in the world (bullets and the camera only)",
            "collision: 0xa white, 105 in the world (walkers, bullets and the camera)",
        ]);
    }

    #[test]
    fn the_same_box_in_two_groups_nests() {
        let mut view = View::Off.command(Request::Toggle).0;
        let objects = [near(0x2, Motion::Static), near(0x8, Motion::Static), near(0xA, Motion::Static),
                       object(0x8, Motion::Static, [5.0, 1.0, 5.0], [6.0, 1.0, 5.005])];
        frame(&mut view, &objects);
        let corners: Vec<_> = drawn(&mut view).iter().map(|s| s.aabb.corners()).collect();
        let close = |got: [f32; 6], want: [f32; 6]| got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-5);
        assert_eq!(corners[0], [-1.0, 0.0, -1.0, 1.0, 2.0, 1.0]);
        assert!(close(corners[1], [-0.99, 0.01, -0.99, 0.99, 1.99, 0.99]), "{:?}", corners[1]);
        assert!(close(corners[2], [-0.98, 0.02, -0.98, 0.98, 1.98, 0.98]), "{:?}", corners[2]);
        assert!(close(corners[3], [5.01, 1.0, 5.0025, 5.99, 1.0, 5.0025]), "a thin axis kept at its middle");
    }

    #[test]
    fn colours_packed_red_lowest() {
        assert_eq!(LevelGroup::Walkers.color().packed().to_le_bytes(), [0, 255, 0, 255]);
        assert_eq!(LevelGroup::Bullets.color().packed().to_le_bytes(), [255, 0, 255, 255]);
        assert_eq!(LevelGroup::Both.color().packed().to_le_bytes(), [255; 4]);
        assert_eq!(LineColor::opaque(255, 0, 0).packed(), u32::from_be_bytes([255, 0, 0, 255]), "the stages' red: 0xFF0000FF");
    }
}
