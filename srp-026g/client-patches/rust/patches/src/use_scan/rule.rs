//! The use scan's rule (base_player::detect_usable_object, 0x2D84C0, with
//! patch 23's battery under the floor), over any world: no game address, no
//! binding. [`scan`] walks the view ray's hits nearest first and asks the
//! world only what the game asks, in its order.
//!
//! The game's scan: each hit's collision group is ORed into the groups seen
//! so far (the 0.10b enum: walkable 2, hittable 8, usable 80h). At a usable
//! hit its usable object is the answer if its can_use says yes; one that
//! says no is kept for the prompt (the first such) and the walk goes on. At
//! any other hit the game stops once walkable and hittable were both seen.
//! Patch 23 makes the ray twice as long and goes on there instead (the walk
//! has crossed a wall or a floor: both bits cleared, the next such hit seen
//! as one too); a usable hit beyond the native reach or past a crossing is
//! then only a battery whose centre is below the feet and at most 1 m from
//! them across, its point the feet. Anything else there is not seen at all
//! (not kept for the prompt). Every float test keeps the game's answer for
//! a NaN (comiss: no is no).

/// The collision groups of the hits so far (a u16 the game ORs each hit's
/// into).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Groups(u16);

impl Groups {
    const WALKABLE: u16 = 0x2;
    const HITTABLE: u16 = 0x8;
    const USABLE: u16 = 0x80;

    fn with(self, group: u16) -> Groups {
        Groups(self.0 | group)
    }

    fn usable(self) -> bool {
        self.0 & Self::USABLE != 0
    }

    /// A wall or a floor: walkable and hittable both seen.
    fn solid(self) -> bool {
        self.0 & (Self::WALKABLE | Self::HITTABLE) == Self::WALKABLE | Self::HITTABLE
    }

    fn without_usable(self) -> Groups {
        Groups(self.0 & !Self::USABLE)
    }

    fn without_solid(self) -> Groups {
        Groups(self.0 & !(Self::WALKABLE | Self::HITTABLE))
    }
}

/// What the scan asks of the world, each when the game does.
pub trait Scene {
    type Hit: Copy;
    type Object: Copy;
    fn group(&mut self, hit: Self::Hit) -> u16;
    fn point(&self, hit: Self::Hit) -> [f32; 3];
    /// The usable object the hit belongs to (None: the game would read a
    /// null; the hit is then no usable object).
    fn usable(&mut self, hit: Self::Hit) -> Option<Self::Object>;
    /// Its centre when it is a battery, None for anything else.
    fn battery_centre(&mut self, object: Self::Object) -> Option<[f32; 3]>;
    fn can_use(&mut self, object: Self::Object) -> bool;
}

/// The scanning player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct User {
    pub eye: [f32; 3],
    pub feet: [f32; 3],
    /// usable_objects_detection_distance: the game's reach.
    pub reach: f32,
}

/// The ray's length: patch 23's, twice the reach (fld / fadd st0,st0).
pub fn ray_length(reach: f32) -> f32 {
    reach + reach
}

/// How far across from the feet a battery under the floor may be (1 m,
/// compared squared).
const ACROSS_SQUARED: f32 = 1.0;

/// The scan's answer: the object Use takes, or (for the prompt) the first
/// usable object that refused; each with its point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Found<O> {
    Usable { object: O, at: [f32; 3] },
    Refused { object: O, at: [f32; 3] },
}

impl<O: Copy> Found<O> {
    pub fn object(&self) -> O {
        match *self {
            Found::Usable { object, .. } | Found::Refused { object, .. } => object,
        }
    }

    pub fn at(&self) -> [f32; 3] {
        match *self {
            Found::Usable { at, .. } | Found::Refused { at, .. } => at,
        }
    }
}

/// Where the walk is: the game's own reach, or past a wall or a floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Past {
    Clear,
    Crossed,
}

struct Walk<O> {
    groups: Groups,
    past: Past,
    refused: Option<Found<O>>,
}

enum Next<O> {
    On(Walk<O>),
    Took(Found<O>),
}

/// The scan of `hits` (nearest first) by `user`.
pub fn scan<S: Scene>(scene: &mut S, user: &User, hits: impl IntoIterator<Item = S::Hit>) -> Option<Found<S::Object>> {
    let mut walk = Walk { groups: Groups::default(), past: Past::Clear, refused: None };
    for hit in hits {
        match step(scene, user, walk, hit) {
            Next::On(next) => walk = next,
            Next::Took(found) => return Some(found),
        }
    }
    walk.refused
}

fn step<S: Scene>(scene: &mut S, user: &User, walk: Walk<S::Object>, hit: S::Hit) -> Next<S::Object> {
    let groups = walk.groups.with(scene.group(hit));
    let object = if groups.usable() { scene.usable(hit) } else { None };
    let Some(object) = object else {
        if !groups.usable() && groups.solid() {
            return Next::On(Walk { groups: groups.without_solid(), past: Past::Crossed, ..walk });
        }
        let groups = if groups.usable() { groups.without_usable() } else { groups };
        return Next::On(Walk { groups, ..walk });
    };
    let at = scene.point(hit);
    let beyond = distance_squared(at, user.eye) > user.reach * user.reach;
    let at = if beyond || walk.past == Past::Crossed {
        match scene.battery_centre(object).filter(|&centre| under(centre, user.feet)) {
            Some(_) => user.feet,
            None => return Next::On(Walk { groups: groups.without_usable(), ..walk }),
        }
    } else {
        at
    };
    if scene.can_use(object) {
        return Next::Took(Found::Usable { object, at });
    }
    let refused = walk.refused.or(Some(Found::Refused { object, at }));
    Next::On(Walk { groups: groups.without_usable(), past: walk.past, refused })
}

/// Patch 23's battery: its centre below the feet, at most 1 m across.
fn under(centre: [f32; 3], feet: [f32; 3]) -> bool {
    let below = feet[1] - centre[1];
    let x = centre[0] - feet[0];
    let z = centre[2] - feet[2];
    let across = x * x + z * z;
    below > 0.0 && across <= ACROSS_SQUARED
}

/// (dx² + dy²) + dz², in that order (0x2D85BE's mulss/addss).
fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    let z = a[2] - b[2];
    x * x + y * y + z * z
}

/// Who a Use press is for, given what the game's scan takes: an object Use
/// would take now is the game's; else Use is free for another target (a
/// dropped gun's pickup).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOwner {
    Game,
    Free,
}

pub fn use_owner<O: Copy>(found: Option<Found<O>>) -> UseOwner {
    match found {
        Some(Found::Usable { .. }) => UseOwner::Game,
        Some(Found::Refused { .. }) | None => UseOwner::Free,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Fake {
        group: u16,
        point: [f32; 3],
        /// None: no usable object.
        object: Option<u8>,
        battery: Option<[f32; 3]>,
        can_use: bool,
    }

    struct World {
        hits: Vec<Fake>,
        asked: Vec<&'static str>,
    }

    impl Scene for World {
        type Hit = usize;
        type Object = u8;
        fn group(&mut self, hit: usize) -> u16 {
            self.asked.push("group");
            self.hits[hit].group
        }
        fn point(&self, hit: usize) -> [f32; 3] {
            self.hits[hit].point
        }
        fn usable(&mut self, hit: usize) -> Option<u8> {
            self.asked.push("usable");
            self.hits[hit].object
        }
        fn battery_centre(&mut self, object: u8) -> Option<[f32; 3]> {
            self.asked.push("centre");
            self.hits.iter().find(|h| h.object == Some(object)).and_then(|h| h.battery)
        }
        fn can_use(&mut self, object: u8) -> bool {
            self.asked.push("can_use");
            self.hits.iter().find(|h| h.object == Some(object)).is_some_and(|h| h.can_use)
        }
    }

    const FEET: [f32; 3] = [12.0, 10.0, -7.0];
    const USER: User = User { eye: [12.0, 11.7, -7.0], feet: FEET, reach: 2.0 };

    fn wall(y: f32) -> Fake {
        Fake { group: 0xA, point: [12.0, y, -7.0], object: None, battery: None, can_use: false }
    }
    fn usable(id: u8, point: [f32; 3], can_use: bool) -> Fake {
        Fake { group: 0x80, point, object: Some(id), battery: None, can_use }
    }
    fn battery(id: u8, point: [f32; 3], centre: [f32; 3], can_use: bool) -> Fake {
        Fake { battery: Some(centre), ..usable(id, point, can_use) }
    }
    fn run(hits: Vec<Fake>) -> (Option<Found<u8>>, Vec<&'static str>) {
        let n = hits.len();
        let mut world = World { hits, asked: Vec::new() };
        let found = scan(&mut world, &USER, 0..n);
        (found, world.asked)
    }

    #[test]
    fn the_first_usable_one_in_reach_is_taken() {
        let at = [13.0, 10.1, -7.0];
        assert_eq!(run(vec![usable(1, at, true)]).0, Some(Found::Usable { object: 1, at }));
        assert_eq!(run(vec![]).0, None);
    }

    #[test]
    fn a_refusal_is_kept_for_the_prompt_and_the_walk_goes_on() {
        let (a, b) = ([12.5, 10.5, -7.0], [12.8, 10.4, -7.0]);
        assert_eq!(run(vec![usable(1, a, false), usable(2, b, true)]).0, Some(Found::Usable { object: 2, at: b }));
        assert_eq!(run(vec![usable(1, a, false), usable(2, b, false)]).0, Some(Found::Refused { object: 1, at: a }));
    }

    #[test]
    fn walkable_and_hittable_seen_apart_are_a_wall_too() {
        let walkable = Fake { group: 0x2, ..wall(11.0) };
        let hittable = Fake { group: 0x8, ..wall(10.8) };
        // past the crossing only a battery under the floor is seen
        let (found, _) = run(vec![walkable, hittable, usable(1, [12.0, 10.5, -7.0], true)]);
        assert_eq!(found, None);
        let (found, _) = run(vec![walkable, usable(1, [12.0, 10.5, -7.0], true)]);
        assert_eq!(found, Some(Found::Usable { object: 1, at: [12.0, 10.5, -7.0] }));
    }

    #[test]
    fn a_battery_under_the_floor_is_taken_at_the_feet() {
        let below = |across: f32| battery(1, [12.0 + across, 9.8, -7.0], [12.0 + across, 9.7, -7.0], true);
        for (across, taken) in [(0.0, true), (0.99, true), (1.0, true), (1.01, false)] {
            let found = run(vec![wall(10.0), below(across)]).0;
            assert_eq!(found, taken.then_some(Found::Usable { object: 1, at: FEET }), "{across}");
        }
        // centre above the feet, a NaN centre, another object: not seen at all
        assert_eq!(run(vec![wall(10.0), battery(1, [12.0, 9.9, -7.0], [12.0, 10.05, -7.0], true)]).0, None);
        assert_eq!(run(vec![wall(10.0), battery(1, [12.0, 9.8, -7.0], [f32::NAN, 9.7, -7.0], true)]).0, None);
        assert_eq!(run(vec![wall(10.0), usable(1, [12.0, 9.8, -7.0], false)]).0, None);
    }

    #[test]
    fn beyond_the_reach_only_a_battery_under_the_feet() {
        assert_eq!(run(vec![usable(1, [14.5, 10.4, -7.0], true)]).0, None);
        // a NaN point is not beyond: the game's way
        let nan = [f32::NAN, 10.0, -7.0];
        assert!(matches!(run(vec![usable(1, nan, true)]).0, Some(Found::Usable { object: 1, .. })));
    }

    #[test]
    fn the_game_is_asked_in_its_order() {
        let (_, asked) = run(vec![wall(10.0), battery(1, [12.0, 9.8, -7.0], [12.0, 9.7, -7.0], false)]);
        assert_eq!(asked, ["group", "group", "usable", "centre", "can_use"]);
    }

    #[test]
    fn use_is_the_games_only_for_an_object_it_takes() {
        let at = [0.0; 3];
        assert_eq!(use_owner(Some(Found::Usable { object: 1u8, at })), UseOwner::Game);
        assert_eq!(use_owner(Some(Found::Refused { object: 1u8, at })), UseOwner::Free);
        assert_eq!(use_owner::<u8>(None), UseOwner::Free);
    }
}
