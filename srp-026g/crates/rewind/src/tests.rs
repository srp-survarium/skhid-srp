use super::*;

fn t(ms: u32) -> Time {
    Time(ms)
}

fn age(ms: u32) -> Age {
    Age::new(ms).unwrap()
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

/// A runner along +x at 5 m/s from x = 0 at 1000 ms: his roots at 1000..=1150.
fn runner() -> Ring {
    let mut ring = Ring::EMPTY;
    for n in 0..4 {
        ring.record(t(1000 + 50 * n), [0.25 * n as f32, 0.0, 0.0]);
    }
    ring
}

#[test]
fn the_ring_keeps_the_last_four_grid_points() {
    let mut ring = runner();
    assert_eq!(ring.roots().count(), 4);
    // the fifth overwrites the first's slot
    ring.record(t(1200), [1.0, 0.0, 0.0]);
    let mut times: [u32; 4] = [0; 4];
    for (n, root) in ring.roots().enumerate() {
        times[n] = root.at.0;
    }
    times.sort();
    assert_eq!(times, [1050, 1100, 1150, 1200]);
    // off the grid: nothing
    ring.record(t(1210), [9.0, 9.0, 9.0]);
    assert!(ring.roots().all(|r| r.feet != [9.0; 3]));
    // a step taken again (a rollback) writes its own slot again
    ring.record(t(1200), [1.5, 0.0, 0.0]);
    assert!(ring.roots().any(|r| r.at == t(1200) && r.feet == [1.5, 0.0, 0.0]));
    assert_eq!(ring.roots().count(), 4);
}

#[test]
fn a_frame_lerps_between_the_roots() {
    // a frame at 1170, the runner at x = 0.85 (he goes on at 5 m/s)
    let path = Path::new(&runner(), t(1170), [0.85, 0.0, 0.0], Knot::NOW, t(0));
    assert_eq!(path.reach(), 170);
    assert!(close(path.back_at(0.0), [0.0; 3]));
    // 10 ms back, between now and the root at 1150 (20 ms back): half of 0.1
    assert!(close(path.back_at(10.0), [0.05, 0.0, 0.0]));
    // 45 ms back: at 1125, between 1150 (x 0.75) and 1100 (x 0.5), x 0.625
    assert!(close(path.back_at(45.0), [0.85 - 0.625, 0.0, 0.0]), "{:?}", path.back_at(45.0));
    // exactly a root: its place
    assert!(close(path.back_at(120.0), [0.85 - 0.25, 0.0, 0.0]));
    // beyond the oldest root (1000, 170 ms back): there, no further
    assert!(close(path.back_at(250.0), [0.85, 0.0, 0.0]));
    // no number of ms: where he is
    assert!(close(path.back_at(f32::NAN), [0.0; 3]));
}

#[test]
fn a_bullets_step_starts_at_its_move() {
    // the step 1150 -> 1200, its move 0.25 (the controller's): the ring's
    // roots before the step's start, the start's own root left to the move
    let mut ring = runner();
    ring.record(t(1200), [1.0, 0.0, 0.0]); // a root of the step itself: not before its start
    let first = Knot { ms: 50, back: [0.25, 0.0, 0.0] };
    let path = Path::new(&ring, t(1200), [1.0, 0.0, 0.0], first, t(0));
    assert_eq!(path.knots[..path.len].iter().map(|k| k.ms).collect::<std::vec::Vec<_>>(), [50, 100, 150]);
    assert!(close(path.back_at(50.0), [0.25, 0.0, 0.0]));
    assert!(close(path.back_at(75.0), [0.375, 0.0, 0.0]));
    assert!(close(path.back_at(100.0), [0.5, 0.0, 0.0]));
    assert!(close(path.back_at(150.0), [0.75, 0.0, 0.0]));
    assert!(close(path.back_at(200.0), [0.75, 0.0, 0.0]), "the ring's end (1000's slot is 1200's now): no further");
}

#[test]
fn a_life_starts_the_path() {
    // spawned at 1050: the roots of 1000 and 1050 may be his last life's
    let path = Path::new(&runner(), t(1170), [0.85, 0.0, 0.0], Knot::NOW, t(1050));
    assert_eq!(path.reach(), 70, "only the roots after his spawn");
    assert!(close(path.back_at(150.0), [0.85 - 0.5, 0.0, 0.0]), "held at the first root of his life");
    // spawned now: no root, where he is
    let path = Path::new(&runner(), t(1170), [0.85, 0.0, 0.0], Knot::NOW, t(1170));
    assert!(close(path.back_at(100.0), [0.0; 3]));
}

#[test]
fn the_clock_wraps() {
    // grid times across the wrap: 2^32 is no multiple of 50, slots may
    // clash, each root keeps its time
    let base = u32::MAX - (u32::MAX % 50) - 100; // a grid time 100+ ms before the wrap
    let mut ring = Ring::EMPTY;
    let mut at = base;
    let mut times = std::vec::Vec::new();
    for _ in 0..4 {
        ring.record(t(at), [times.len() as f32, 0.0, 0.0]);
        times.push(at);
        at = at.wrapping_add(50);
    }
    let now = t(times[3].wrapping_add(20));
    let path = Path::new(&ring, now, [3.0, 0.0, 0.0], Knot::NOW, t(base.wrapping_sub(1000)));
    // every root read back by its time through the wrap, those kept
    for (n, &time) in times.iter().enumerate() {
        let ms = now.since(t(time)).unwrap();
        if ring.roots().any(|r| r.at.0 == time) {
            assert!(close(path.back_at(ms as f32), [3.0 - n as f32, 0.0, 0.0]), "{n}");
        }
    }
    // the time difference itself
    assert_eq!(t(10).since(t(u32::MAX - 9)), Some(20));
    assert_eq!(t(u32::MAX - 9).since(t(10)), None, "set back");
    assert!(t(10).after(t(u32::MAX - 9)) && !t(10).after(t(10)));
    // a root from a clock set back (after now) is none
    let mut ring = Ring::EMPTY;
    ring.record(t(2000), [5.0, 0.0, 0.0]);
    let path = Path::new(&ring, t(1170), [0.0; 3], Knot::NOW, t(0));
    assert_eq!(path.reach(), 0);
    // nor one further than the reach
    let path = Path::new(&runner(), t(1500), [0.0; 3], Knot::NOW, t(0));
    assert_eq!(path.reach(), 0);
}

#[test]
fn ages_and_the_cap() {
    assert_eq!(Age::new(150), Some(Age::MAX));
    assert_eq!(Age::new(151), None);
    let cap = Cap::from_entry(&[100]).unwrap();
    assert_eq!(age(140).capped(cap), age(100));
    assert_eq!(age(60).capped(cap), age(60));
    assert_eq!(Cap::from_entry(&[200]).unwrap().age(), Age::MAX, "past the ring: 150");
    assert_eq!(Cap::from_entry(&[]), None);
    assert_eq!(Cap::from_entry(&[1, 2]), None);
    assert_eq!(age(150).capped(Cap::DEFAULT), age(150));
    assert_eq!(Level::at_least(age(0)).age(), age(0));
    assert_eq!(Level::at_least(age(1)).age(), age(15));
    assert_eq!(Level::at_least(age(30)).age(), age(30));
    assert_eq!(Level::at_least(age(149)).age(), age(150));
    assert_eq!(Level::at_least(age(30)).younger(2), Some(Level::at_least(age(0))));
    assert_eq!(Level::at_least(age(15)).younger(2), None);
}

#[test]
fn declarations_decode_v1_and_the_pair_form() {
    // kits are no declaration
    for kit in 0..3 {
        assert_eq!(Declaration::decode(kit), None);
    }
    // version 1: every place, 3..=13
    assert_eq!(Declaration::decode(3), Some(Declaration::every(Level::at_least(age(0)))));
    assert_eq!(Declaration::decode(13), Some(Declaration::every(Level::at_least(age(150)))));
    // the pair form: place 0 at 14..=24, place 19 at 223..=233
    let pair = Declaration::decode(14 + 4).unwrap();
    assert_eq!(pair.target, Target::Place(Place::new(0).unwrap()));
    assert_eq!(pair.level.age(), age(60));
    let last = Declaration::decode(233).unwrap();
    assert_eq!((last.target, last.level.age()), (Target::Place(Place::new(19).unwrap()), age(150)));
    // nobody's
    for value in 234..=255 {
        assert_eq!(Declaration::decode(value), None, "{value}");
    }
    // every code round trips
    for value in 3..=233u8 {
        assert_eq!(Declaration::decode(value).unwrap().encode(), value);
    }
}

#[test]
fn a_declaration_is_in_force_after_its_tick() {
    let at = t(5000);
    let v60 = Declaration::every(Level::at_least(age(60))).encode();
    let declared = Declared::Never.step(Some((at, v60)));
    let p = Place::new(3).unwrap();
    assert_eq!(declared.age_for(t(4990), p), Age::ZERO, "fired before it: the age before");
    assert_eq!(declared.age_for(at, p), Age::ZERO, "at its tick: before");
    assert_eq!(declared.age_for(t(5001), p), age(60));
    // found again at the next steps: nothing changes
    assert_eq!(declared.step(Some((at, v60))), declared);
    // a kit choice newer than it, or nothing: as it was
    assert_eq!(declared.step(Some((t(5100), 1))), declared);
    assert_eq!(declared.step(None), declared);
    // one place's age (a later version's): as it was
    assert_eq!(declared.step(Some((t(5100), 14 + 2))), declared);
    // a newer one: the old for shots before it
    let v120 = Declaration::every(Level::at_least(age(120))).encode();
    let next = declared.step(Some((t(6000), v120)));
    assert_eq!(next, Declared::Since { before: age(60), now: age(120), at: t(6000) });
    assert_eq!(next.age_for(t(5999), p), age(60));
    assert_eq!(next.age_for(t(6020), p), age(120));
    // across the wrap
    let wrapped = Declared::Never.step(Some((t(u32::MAX - 49), v60)));
    assert_eq!(wrapped.age_for(t(10), p), age(60));
    assert_eq!(wrapped.age_for(t(u32::MAX - 60), p), Age::ZERO);
}

#[test]
fn records_round_trip_and_refuse_what_they_never_write() {
    let record = Record {
        ring: runner(),
        declared: Declared::Since { before: age(30), now: age(90), at: t(4000) },
    };
    let bytes = record.encode();
    assert_eq!(bytes.len(), 73);
    assert_eq!(Record::decode(&bytes), Some(record));
    assert_eq!(Record::decode(&Record::EMPTY.encode()), Some(Record::EMPTY));
    // floats bit for bit, a NaN's payload too
    let mut ring = Ring::EMPTY;
    ring.record(t(50), [f32::from_bits(0x7FC0_1234), -0.0, 1.5]);
    let odd = Record { ring, declared: Declared::Never };
    let back = Record::decode(&odd.encode()).unwrap();
    assert_eq!(back.ring.roots().next().unwrap().feet.map(f32::to_bits), [0x7FC0_1234, 0x8000_0000, 1.5f32.to_bits()]);
    let wrong = |at: usize, value: u8| {
        let mut b = bytes;
        b[at] = value;
        Record::decode(&b)
    };
    assert_eq!(wrong(0, 2), None, "another version");
    assert_eq!(wrong(1, 0x1F), None, "a fifth slot");
    assert_eq!(wrong(2, 0x01), None, "a root off the grid");
    assert_eq!(wrong(66, 2), None, "no such declared state");
    assert_eq!(wrong(68, 151), None, "an age past the most");
    let mut empty = Record::EMPTY.encode();
    empty[10] = 1;
    assert_eq!(Record::decode(&empty), None, "an empty slot's bytes are zeros");
    // a root in another slot than its time's
    let mut moved = bytes;
    moved[2..6].copy_from_slice(&1050u32.to_le_bytes());
    assert_eq!(Record::decode(&moved), None);
}
