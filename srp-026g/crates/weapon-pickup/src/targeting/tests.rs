use super::*;
use crate::state::{HeldAtDeath, Kit, Life, Place};
use crate::{AmmoKind, CarryRule, Chamber, DroppedAmmo, Handedness, Loadout, WeaponSlot};

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];
const BOX: [f32; 6] = [-1.0, -0.1, -0.1, 1.0, 0.1, 0.1];

fn segment(start: [f32; 3], end: [f32; 3]) -> Segment {
    Segment::parse(start, end).unwrap()
}

#[test]
fn reach_parallel_rays_inside_and_endpoints() {
    let bounds = WeaponBox::parse(BOX, IDENTITY).unwrap();
    assert_eq!(
        bounds.intersection(segment([-2.0, 0.0, 0.0], [2.0, 0.0, 0.0])),
        Some(0.25)
    );
    assert_eq!(
        bounds.intersection(segment([2.0, 0.0, 0.0], [-2.0, 0.0, 0.0])),
        Some(0.25)
    );
    assert_eq!(
        bounds.intersection(segment([0.0, 0.0, 0.0], [2.0, 0.0, 0.0])),
        Some(0.0)
    );
    assert_eq!(
        bounds.intersection(segment([-2.0, 0.0, 0.0], [-1.0, 0.0, 0.0])),
        Some(1.0)
    );
    assert_eq!(
        bounds.intersection(segment([-2.0, 0.0, 0.0], [-1.01, 0.0, 0.0])),
        None
    );
    assert_eq!(
        bounds.intersection(segment([-2.0, 0.0, 0.0], [-3.0, 0.0, 0.0])),
        None
    );
    assert_eq!(
        bounds.intersection(segment([-2.0, 0.11, 0.0], [2.0, 0.11, 0.0])),
        None
    );
}

#[test]
fn tilted_weapon_does_not_select_the_empty_corners_of_its_world_aabb() {
    let q = core::f32::consts::FRAC_1_SQRT_2;
    let mut draw = IDENTITY;
    draw[0] = q;
    draw[1] = q;
    draw[4] = -q;
    draw[5] = q;
    draw[12] = 10.0;
    let bounds = WeaponBox::parse(BOX, draw).unwrap();
    assert!(
        bounds
            .intersection(segment([10.5, 0.5, -1.0], [10.5, 0.5, 1.0]))
            .is_some()
    );
    assert_eq!(
        bounds.intersection(segment([10.5, -0.5, -1.0], [10.5, -0.5, 1.0])),
        None
    );
    assert_eq!(
        bounds.intersection(segment([0.5, 0.5, -1.0], [0.5, 0.5, 1.0])),
        None
    );
}

#[test]
fn reflection_nonuniform_scale_and_shear_keep_the_same_segment_fraction() {
    let draw = [
        -2.0, 0.5, 0.0, 0.0, 0.0, 3.0, 0.25, 0.0, 0.25, 0.0, 0.5, 0.0, 15.0, -3.0, 25.0, 1.0,
    ];
    let transform = |p: [f32; 3]| {
        core::array::from_fn(|i| {
            p[0] * draw[i] + p[1] * draw[4 + i] + p[2] * draw[8 + i] + draw[12 + i]
        })
    };
    let bounds = WeaponBox::parse(BOX, draw).unwrap();
    let fraction = bounds
        .intersection(segment(
            transform([-2.0, 0.0, 0.0]),
            transform([2.0, 0.0, 0.0]),
        ))
        .unwrap();
    assert!((fraction - 0.25).abs() < 1e-12);
    assert_eq!(
        bounds.intersection(segment(
            transform([-2.0, 0.5, 0.0]),
            transform([2.0, 0.5, 0.0])
        )),
        None
    );
}

#[test]
fn malformed_geometry_cannot_become_a_target() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for i in 0..16 {
            let mut draw = IDENTITY;
            draw[i] = bad;
            assert!(WeaponBox::parse(BOX, draw).is_none());
        }
        for i in 0..6 {
            let mut bounds = BOX;
            bounds[i] = bad;
            assert!(WeaponBox::parse(bounds, IDENTITY).is_none());
        }
        assert!(Segment::parse([bad, 0.0, 0.0], [0.0; 3]).is_none());
    }
    for i in [3, 7, 11, 15] {
        let mut draw = IDENTITY;
        draw[i] += 0.1;
        assert!(WeaponBox::parse(BOX, draw).is_none());
    }
    let mut singular = IDENTITY;
    singular[0] = 0.0;
    assert!(WeaponBox::parse(BOX, singular).is_none());
    let mut reversed = BOX;
    reversed[0] = 2.0;
    assert!(WeaponBox::parse(reversed, IDENTITY).is_none());
    assert!(Segment::parse([0.0; 3], [0.0; 3]).is_none());
}

#[test]
fn extreme_finite_coordinates_do_not_overflow_into_a_false_hit() {
    let mut draw = IDENTITY;
    draw[0] = f32::MAX;
    let bounds = WeaponBox::parse(BOX, draw).unwrap();
    assert_eq!(
        bounds.intersection(segment([-f32::MAX, 0.0, 0.0], [f32::MAX, 0.0, 0.0])),
        Some(0.0)
    );
    assert_eq!(
        bounds.intersection(segment([-f32::MAX, 1.0, 0.0], [f32::MAX, 1.0, 0.0])),
        None
    );
}

#[test]
fn nearest_available_life_wins_independently_of_draw_order() {
    let mut world = World::new();
    let kit = Kit::parse(0).unwrap();
    let life = |place, spawned| Life {
        place: Place::parse(place).unwrap(),
        spawned,
    };
    let held = HeldAtDeath {
        slot: WeaponSlot::First,
        handedness: Handedness::Two,
        ammo: DroppedAmmo::parse(30, 17, Chamber::Loaded, AmmoKind::parse(0).unwrap()).unwrap(),
    };
    let view = |life, x| {
        let mut draw = IDENTITY;
        draw[12] = x;
        RenderedDrop {
            id: DropId(life),
            bounds: WeaponBox::parse(BOX, draw).unwrap(),
        }
    };
    let near = view(life(2, 100), 2.0);
    let far = view(life(1, 200), 4.0);
    for id in [near.id, far.id] {
        world.spawn(id.0, kit);
        world.die(id.0, Some(held));
    }
    let ray = segment([0.0; 3], [6.0, 0.0, 0.0]);
    let mut frames = Frames::new();
    // Out-of-frame draws and incomplete parallel batches never publish.
    frames.transition(FrameEvent::Draw(near));
    assert_eq!(frames.select(&world, ray), None);
    frames.transition(FrameEvent::Begin);
    frames.transition(FrameEvent::Draw(near));
    assert_eq!(frames.select(&world, ray), None);
    frames.transition(FrameEvent::Complete);
    assert_eq!(frames.select(&world, ray), Some(near.id));
    // An empty next frame removes the old selectable pose (cull/no weapon).
    frames.transition(FrameEvent::Begin);
    assert_eq!(frames.select(&world, ray), None);
    frames.transition(FrameEvent::Complete);
    assert_eq!(frames.select(&world, ray), None);
    frames.transition(FrameEvent::Begin);
    frames.transition(FrameEvent::Draw(near));
    // Restarting an unfinished batch also drops its partial observations.
    frames.transition(FrameEvent::Begin);
    frames.transition(FrameEvent::Draw(far));
    frames.transition(FrameEvent::Complete);
    assert_eq!(frames.select(&world, ray), Some(far.id));
    for views in [[near, far], [far, near]] {
        assert_eq!(select(&world, ray, views), Some(near.id));
    }
    let tie = view(far.id.0, 2.0);
    for views in [[near, tie], [tie, near]] {
        assert_eq!(select(&world, ray, views), Some(tie.id));
    }
    let recipient = life(0, 50);
    world.spawn(recipient, kit);
    let _ = world
        .pickup(
            recipient,
            near.id,
            Loadout::new(
                [Some(Handedness::Two), None],
                Some(WeaponSlot::First),
                CarryRule::OneDoubleHanded,
            ),
        )
        .unwrap();
    assert_eq!(select(&world, ray, [near, far]), Some(far.id));
    world.spawn(life(1, 300), kit);
    world.die(life(1, 300), Some(held));
    assert_eq!(select(&world, ray, [near, far]), None);
    let fresh = view(life(1, 300), 4.0);
    assert_eq!(select(&world, ray, [fresh]), Some(fresh.id));
    world.leave(fresh.id.0.place);
    assert_eq!(select(&world, ray, [fresh]), None);
}
