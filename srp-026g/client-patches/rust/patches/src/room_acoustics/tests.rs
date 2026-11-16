use super::model::*;
use crate::vostok::resources::Serial;
use crate::vostok::sound::TrackPath;

const AXES: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn cube(centre: [f32; 3], half: [f32; 3]) -> ZoneShape {
    ZoneShape::parse(centre, AXES, Form::Box { half }).expect("a box")
}

#[test]
fn names_are_numbered_once_and_cut_as_0_69d_keeps_them() {
    let mut names = RoomNames::new();
    let outdoors = names.outdoors();
    assert_eq!(names.room(b"default"), Some(outdoors));
    let class = names.room(b"school_class").expect("a number");
    assert_ne!(class, outdoors);
    assert_eq!(names.room(b"school_class"), Some(class));
    assert_eq!(names.name(class), "school_class");
    assert_eq!(names.name(outdoors), "default");
    assert_eq!(names.room(b""), None);
    let long = [b'a'; 40];
    assert_eq!(names.room(&long), names.room(&long[..NAME_MOST]));
    assert_eq!(names.item(None), ItemRoom::Any);
    assert_eq!(names.item(Some(b"")), ItemRoom::Any);
    assert_eq!(names.item(Some(b"school_class")), ItemRoom::Room(class));
}

#[test]
fn the_name_table_full_leaves_an_item_silent() {
    let mut names = RoomNames::new();
    for i in 0..ROOMS_MOST {
        assert!(names.room(format!("room{i}").as_bytes()).is_some());
    }
    assert_eq!(names.room(b"one_more"), None);
    assert_eq!(names.item(Some(b"one_more")), ItemRoom::Unnumbered);
    assert!(!ItemRoom::Unnumbered.plays_in(names.outdoors()));
}

#[test]
fn item_bytes_round_trip() {
    let mut names = RoomNames::new();
    let class = ItemRoom::Room(names.room(b"school_class").expect("a number"));
    for item in [ItemRoom::Any, class, ItemRoom::Unnumbered, ItemRoom::Room(names.outdoors())] {
        assert_eq!(ItemRoom::from_byte(item.byte()), item);
    }
}

#[test]
fn an_item_plays_untagged_or_in_its_own_room_with_no_fallback() {
    let mut names = RoomNames::new();
    let (outdoors, class, tunnel) = (names.outdoors(), names.room(b"school_class").unwrap(), names.room(b"cp_tunnels").unwrap());
    assert!(ItemRoom::Any.plays_in(class));
    assert!(ItemRoom::Room(class).plays_in(class));
    assert!(!ItemRoom::Room(class).plays_in(tunnel));
    assert!(!ItemRoom::Room(class).plays_in(outdoors));
    assert!(ItemRoom::Room(outdoors).plays_in(outdoors));
}

#[test]
fn boxes_refuse_what_is_not_one() {
    assert!(ZoneShape::parse([0.0; 3], AXES, Form::Box { half: [1.0, 0.0, 1.0] }).is_none());
    assert!(ZoneShape::parse([f32::NAN, 0.0, 0.0], AXES, Form::Box { half: [1.0; 3] }).is_none());
    assert!(ZoneShape::parse([0.0; 3], [[1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], Form::Box { half: [1.0; 3] }).is_none());
    assert!(ZoneShape::parse([0.0; 3], [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], Form::Box { half: [1.0; 3] }).is_none());
}

#[test]
fn the_ray_up_meets_a_box_from_inside_and_from_below_not_from_above() {
    let room = cube([0.0, 5.0, 0.0], [2.0, 1.0, 2.0]);
    assert_eq!(room.up([0.0, 5.0, 0.0]), Some(1.0)); // inside: the ceiling
    assert_eq!(room.up([0.0, 0.0, 0.0]), Some(4.0)); // below: its floor
    assert_eq!(room.up([0.0, 7.0, 0.0]), None); // above it
    assert_eq!(room.up([3.0, 5.0, 0.0]), None); // beside it
    assert_eq!(room.up([0.0, -97.0, 0.0]), None); // 100 m is the ray
    assert_eq!(room.up([0.0, -95.0, 0.0]), Some(99.0));
}

#[test]
fn a_turned_box_is_tested_in_its_own_axes() {
    // a quarter turn about y: its long side along z
    let s = core::f32::consts::FRAC_1_SQRT_2;
    let turned = ZoneShape::parse([0.0; 3], [[s, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, s]], Form::Box { half: [4.0, 1.0, 0.5] }).unwrap();
    assert!(turned.up([2.0, 0.0, -2.0]).is_some()); // along its x axis
    assert!(turned.up([2.0, 0.0, 2.0]).is_none());
    // tilted: the ray leaves by a slanted face
    let c = 0.8f32;
    let tilted = ZoneShape::parse([0.0; 3], [[c, 0.6, 0.0], [-0.6, c, 0.0], [0.0, 0.0, 1.0]], Form::Box { half: [1.0; 3] }).unwrap();
    let t = tilted.up([0.0, 0.0, 0.0]).unwrap();
    assert!((t - 1.25).abs() < 1e-4, "{t}");
}

fn zones(names: &mut RoomNames) -> (Zones, EnvIndex, EnvIndex) {
    let mut zones = Zones::new();
    let house = zones.add(Environment { room: names.room(b"rudnya_houses").unwrap(), bed: None }).unwrap();
    let cellar = zones.add(Environment { room: names.room(b"cellar").unwrap(), bed: None }).unwrap();
    zones.add_shape(house, cube([0.0, 3.0, 0.0], [10.0, 3.0, 10.0])).unwrap();
    zones.add_shape(cellar, cube([0.0, 1.0, 0.0], [2.0, 1.0, 2.0])).unwrap(); // nested, low
    (zones, house, cellar)
}

#[test]
fn the_nearest_surface_above_decides_nested_zones() {
    let mut names = RoomNames::new();
    let (zones, house, cellar) = zones(&mut names);
    assert_eq!(zones.place([0.0, 0.5, 0.0]), Place::In(cellar)); // its ceiling at 2 m is nearer than the house's at 6
    assert_eq!(zones.place([0.0, 3.0, 0.0]), Place::In(house)); // above the cellar
    assert_eq!(zones.place([5.0, 0.5, 0.0]), Place::In(house));
    assert_eq!(zones.place([20.0, 0.5, 0.0]), Place::Outdoors);
    assert_eq!(zones.room(Place::Outdoors, names.outdoors()), names.outdoors());
    assert_eq!(zones.room(Place::In(cellar), names.outdoors()), names.room(b"cellar").unwrap());
}

#[test]
fn a_tie_goes_to_the_later_box() {
    let mut names = RoomNames::new();
    let mut zones = Zones::new();
    let a = zones.add(Environment { room: names.room(b"a").unwrap(), bed: None }).unwrap();
    let b = zones.add(Environment { room: names.room(b"b").unwrap(), bed: None }).unwrap();
    zones.add_shape(a, cube([0.0; 3], [1.0; 3])).unwrap();
    zones.add_shape(b, cube([0.0; 3], [1.0; 3])).unwrap();
    assert_eq!(zones.place([0.0; 3]), Place::In(b));
}

fn bed(name: &core::ffi::CStr, fade_in: u32, fade_out: u32) -> Bed {
    Bed { sound: TrackPath::new(name).unwrap(), fade_in: FadeMs::new(fade_in), fade_out: FadeMs::new(fade_out) }
}

#[derive(Default)]
struct Game {
    loads: Vec<TrackPath>,
    plays: Vec<usize>,
    fades: Vec<(usize, Toward, u32)>,
    cuts: Vec<usize>,
    silent: [bool; SLOTS],
    serial: Option<Serial>,
}

impl Engine for Game {
    type Emitter = u32;
    type Voice = u32;
    fn load(&mut self, sound: &TrackPath) -> Serial {
        self.loads.push(*sound);
        let serial = self.serial.map_or(Serial::FIRST, Serial::next);
        self.serial = Some(serial);
        serial
    }
    fn play(&mut self, slot: usize, _: &u32) -> Option<u32> {
        self.plays.push(slot);
        Some(slot as u32)
    }
    fn fade(&mut self, slot: usize, toward: Toward, ms: FadeMs) {
        self.fades.push((slot, toward, ms.get()));
    }
    fn cut(&mut self, slot: usize) {
        self.cuts.push(slot);
    }
    fn silent(&self, slot: usize) -> bool {
        self.silent[slot]
    }
}

#[test]
fn entering_a_room_loads_its_bed_and_fades_it_in_once_it_is_here() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    let class = bed(c"scapes/rooms/school_class", 250, 300);
    beds.step(BedEvent::Moved { left: None, entered: Some(class) }, &mut game);
    assert_eq!(game.loads, [class.sound]);
    assert_eq!(game.fades, [(0, Toward::Full, 250)]);
    beds.step(BedEvent::Arrived(Serial::FIRST, Some(7)), &mut game);
    assert_eq!(game.plays, [0]);
    // a load nobody waits for is let go
    beds.step(BedEvent::Arrived(Serial::FIRST.next(), Some(8)), &mut game);
    assert_eq!(game.plays, [0]);
}

#[test]
fn a_room_change_crossfades_each_bed_at_its_own_pace() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    let (class, hall) = (bed(c"scapes/rooms/school_class", 250, 300), bed(c"scapes/rooms/school_hall", 200, 400));
    beds.step(BedEvent::Moved { left: None, entered: Some(class) }, &mut game);
    beds.step(BedEvent::Arrived(Serial::FIRST, Some(1)), &mut game);
    game.fades.clear();
    beds.step(BedEvent::Moved { left: Some(class), entered: Some(hall) }, &mut game);
    // the class bed out over its 300 ms, the hall's in over its 200, in the same step
    assert_eq!(game.fades, [(0, Toward::Silence, 300), (1, Toward::Full, 200)]);
    beds.step(BedEvent::Arrived(Serial::FIRST.next(), Some(2)), &mut game);
    assert_eq!(game.plays, [0, 1]);
    // the class bed let go once silent
    beds.step(BedEvent::Frame, &mut game);
    assert_eq!(game.loads.len(), 2);
    game.silent[0] = true;
    beds.step(BedEvent::Frame, &mut game);
    // back into the class: loaded again (its slot is free)
    beds.step(BedEvent::Moved { left: Some(hall), entered: Some(class) }, &mut game);
    assert_eq!(game.loads.len(), 3);
}

#[test]
fn the_same_bed_is_handed_over_and_a_fading_one_fades_back() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    let (a, a_again, b) = (bed(c"scapes/rooms/house", 250, 300), bed(c"scapes/rooms/house", 100, 100), bed(c"scapes/rooms/hall", 200, 200));
    beds.step(BedEvent::Moved { left: None, entered: Some(a) }, &mut game);
    beds.step(BedEvent::Arrived(Serial::FIRST, Some(1)), &mut game);
    game.fades.clear();
    // another environment with the same sound: no fade out, no new load
    beds.step(BedEvent::Moved { left: Some(a), entered: Some(a_again) }, &mut game);
    assert_eq!(game.loads.len(), 1);
    assert_eq!(game.fades, [(0, Toward::Full, 100)]);
    // out to b, and back before the house bed is silent: it fades back in, not reloaded
    beds.step(BedEvent::Moved { left: Some(a_again), entered: Some(b) }, &mut game);
    beds.step(BedEvent::Moved { left: Some(b), entered: Some(a) }, &mut game);
    assert_eq!(game.loads.len(), 2);
    assert_eq!(game.fades.last(), Some(&(0, Toward::Full, 250)));
}

#[test]
fn outdoors_fades_the_bed_out_and_a_late_arrival_is_let_go() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    let class = bed(c"scapes/rooms/school_class", 250, 300);
    beds.step(BedEvent::Moved { left: None, entered: Some(class) }, &mut game);
    beds.step(BedEvent::Moved { left: Some(class), entered: None }, &mut game);
    beds.step(BedEvent::Arrived(Serial::FIRST, Some(1)), &mut game);
    assert!(game.plays.is_empty());
    assert_eq!(game.cuts, [0, 0]);
}

#[test]
fn a_cut_stops_every_bed() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    beds.step(BedEvent::Moved { left: None, entered: Some(bed(c"scapes/rooms/a", 1, 1)) }, &mut game);
    beds.step(BedEvent::Arrived(Serial::FIRST, Some(1)), &mut game);
    game.cuts.clear();
    beds.step(BedEvent::Cut, &mut game);
    assert_eq!(game.cuts, (0..SLOTS).collect::<Vec<_>>());
}

#[test]
fn more_rooms_than_slots_take_a_fading_one() {
    let (mut beds, mut game) = (Beds::<u32, u32>::new(), Game::default());
    let names = [c"scapes/rooms/a", c"scapes/rooms/b", c"scapes/rooms/c", c"scapes/rooms/d", c"scapes/rooms/e"];
    let mut left = None;
    for name in names {
        let b = bed(name, 10, 10);
        beds.step(BedEvent::Moved { left, entered: Some(b) }, &mut game);
        left = Some(b);
    }
    assert_eq!(game.loads.len(), 5);
    assert_eq!(game.fades.last(), Some(&(0, Toward::Full, 10)));
}

#[test]
fn the_mix_steps_linearly_toward_its_target() {
    let mut mix = Mix::new();
    mix.fade(0, Toward::Full, FadeMs::new(200));
    assert_eq!(mix.tick(50)[0], 0.25);
    assert_eq!(mix.tick(1000)[0], 1.0);
    mix.fade(0, Toward::Silence, FadeMs::new(400));
    assert_eq!(mix.tick(100)[0], 0.75);
    assert!(!mix.silent(0));
    mix.tick(400);
    assert!(mix.silent(0));
    mix.fade(1, Toward::Full, FadeMs::new(0)); // a 0 ms fade is 1 ms
    assert_eq!(mix.tick(1)[1], 1.0);
    mix.cut(1);
    assert!(mix.silent(1));
}

#[test]
fn cylinders_and_spheres_are_met_as_0_69d_meets_their_meshes() {
    let cylinder = ZoneShape::parse([0.0, 2.0, 0.0], AXES, Form::Cylinder { radius: 3.0, half_height: 2.0 }).unwrap();
    assert_eq!(cylinder.up([1.0, 1.0, 1.0]), Some(3.0)); // inside: its top
    assert_eq!(cylinder.up([1.0, -1.0, 1.0]), Some(1.0)); // below: its bottom
    assert_eq!(cylinder.up([2.5, 1.0, 2.5]), None); // beside it
    // lying along x: the ray crosses its round side
    let lying = ZoneShape::parse([0.0; 3], [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                                 Form::Cylinder { radius: 1.0, half_height: 5.0 }).unwrap();
    let t = lying.up([0.0, 0.0, 0.6]).unwrap();
    assert!((t - 0.8).abs() < 1e-5, "{t}");
    let sphere = ZoneShape::parse([0.0; 3], AXES, Form::Sphere { radius: 5.0 }).unwrap();
    let t = sphere.up([3.0, 0.0, 0.0]).unwrap();
    assert!((t - 4.0).abs() < 1e-5, "{t}");
    assert_eq!(sphere.up([3.0, -10.0, 0.0]), Some(6.0));
    assert_eq!(sphere.up([6.0, 0.0, 0.0]), None);
    assert!(ZoneShape::parse([0.0; 3], AXES, Form::Sphere { radius: 0.0 }).is_none());
}
