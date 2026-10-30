use super::*;
use crate::vostok::{Regs, context::Frame};
use weapon_pickup::targeting::Segment;

#[test]
fn captures_the_selected_draw_pose_and_filters_non_weapons_and_invalid_lives() {
    let mut player = std::vec![0u32; 0x340 / 4];
    let mut weapon = std::vec![0u32; 0x1814 / 4];
    let mut model = std::vec![0u32; 0x10C / 4];
    let mut instance = std::vec![0u32; 0x62B8 / 4];
    let mut original = std::vec![0u32; 0x120 / 4];
    let mut core = 0u32;
    let core_id = (&mut core as *mut u32) as u32;
    let mut pose = [
        1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0,
    ];
    let player = player.as_mut_ptr().cast();
    let weapon = weapon.as_mut_ptr().cast();
    let model = model.as_mut_ptr().cast();
    let instance = instance.as_mut_ptr().cast();
    let original = original.as_mut_ptr().cast();
    unsafe {
        raw::player::inserted(player).write(1);
        raw::player::core(player).write(core_id);
        raw::player::place(player).write(3);
        raw::player::spawned(player).write(700);
        raw::player::held_item(player).write(weapon as u32);
        raw::weapon_core::vtable(weapon).write(raw::image::WEAPON_ACTIVE_VTABLE);
        raw::weapon::model(weapon).write(model as u32);
        raw::weapon_model::render_instance(model).write(instance as u32);
        raw::skeleton_render_model_instance::original(instance).write(original as u32);
        raw::skeleton_render_model::bounds(original)
            .cast::<[f32; 6]>()
            .write([0.2, -0.1, 0.3, 0.8, 0.1, 0.9]);
    }
    let capture = |pose: &mut [f32; 16]| {
        let mut regs = Regs {
            eax: pose.as_mut_ptr() as u32,
            ebx: player as u32,
            ecx: 0,
            edx: 0,
            esi: 0,
            edi: 0,
            ebp: 0,
            esp: 0,
            eflags: 0,
        };
        let saved = regs;
        let mut fxsave = [0u8; 512];
        let mut frame = Frame {
            regs: &mut regs,
            fxsave: fxsave.as_mut_ptr(),
        };
        let result = DrawnWeapon::at_held_draw(&unsafe { Context::new(&mut frame) });
        assert_eq!(regs, saved);
        result
    };
    let captured = capture(&mut pose).unwrap();
    assert_eq!(captured.core.0.get(), core_id);
    assert_eq!(captured.view.id.0.place.get(), 3);
    assert_eq!(captured.view.id.0.spawned, 700);
    // Asymmetric resource box proves exporter x/z are mirrored. The
    // selected world matrix, not player's transform or a hand approximation,
    // positions it at (10,20,30).
    let hit = Segment::parse([9.5, 20.0, 28.0], [9.5, 20.0, 30.0]).unwrap();
    let miss = Segment::parse([10.5, 20.0, 28.0], [10.5, 20.0, 32.0]).unwrap();
    assert!(captured.view.bounds.intersection(hit).is_some());
    assert_eq!(captured.view.bounds.intersection(miss), None);
    pose[12] = 40.0;
    assert_eq!(
        capture(&mut pose).unwrap().view.bounds.intersection(hit),
        None
    );
    // Captured data is a copy, immune to subsequent drawing/stack changes.
    assert!(captured.view.bounds.intersection(hit).is_some());
    for (field, invalid, valid) in unsafe {
        [
            (raw::player::alive(player), 1, 0),
            (raw::player::inserted(player), 0, 1),
            (raw::player::place(player), 20, 3),
        ]
    } {
        unsafe { field.write(invalid) };
        assert!(capture(&mut pose).is_none());
        unsafe { field.write(valid) };
    }
    for (field, invalid, valid) in unsafe {
        [
            (raw::player::spawned(player), u32::MAX, 700),
            (raw::player::core(player), 0, core_id),
            (raw::player::held_item(player), 0, weapon as u32),
            (
                raw::weapon_core::vtable(weapon),
                raw::image::WEAPON_CORE_ACTIVE_VTABLE,
                raw::image::WEAPON_ACTIVE_VTABLE,
            ),
            (
                raw::weapon_core::vtable(weapon),
                0,
                raw::image::WEAPON_ACTIVE_VTABLE,
            ),
            (raw::weapon::model(weapon), 0, model as u32),
            (
                raw::weapon_model::render_instance(model),
                0,
                instance as u32,
            ),
            (
                raw::skeleton_render_model_instance::original(instance),
                0,
                original as u32,
            ),
        ]
    } {
        unsafe { field.write(invalid) };
        assert!(capture(&mut pose).is_none());
        unsafe { field.write(valid) };
    }
    pose[0] = f32::NAN;
    assert!(capture(&mut pose).is_none());
    pose[0] = 1.0;
    unsafe { raw::skeleton_render_model::bounds(original).write(f32::NAN) };
    assert!(capture(&mut pose).is_none());
}
