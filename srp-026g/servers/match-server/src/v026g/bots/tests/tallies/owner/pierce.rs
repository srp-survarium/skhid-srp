//! The owner counts of bullet penetration: what the stand-in's bullets met on the way to a body (the lines by
//! verdict, the armor that stopped them, the shots of the bots that went into cover and how much of the bullet they
//! kept), what the bots' cover tests answered (`Tactics::shot_reaches`) and the lines the pierce opened, and what the
//! bullet mesh is made of (faces with a material against faces of unknown material, which stop every bullet, as a file before format 5 gives them).

use super::*;

/// The bots' cover tests, as the last tick left them (the tally is the tactics' own; the topic copies it).
#[derive(Default)]
pub(super) struct PierceCounts {
    cover: [u64; 6],
}

impl Topic for PierceCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        use std::sync::atomic::Ordering::Relaxed;
        if let Some(tactics) = bots.tactics.as_ref() {
            let t = &tactics.cover_tally;
            self.cover = [
                t.plain.load(Relaxed),
                t.open_clear.load(Relaxed),
                t.open_through.load(Relaxed),
                t.shut_partial.load(Relaxed),
                t.shut_stopped.load(Relaxed),
                t.events_seen.load(Relaxed),
            ];
        }
    }

    fn parts(&mut self, _run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("pierce", self.part(game))]
    }
}

impl PierceCounts {
    fn part(&self, game: &Game) -> Part {
        let stats = game.pierce_stats.borrow();
        let census = game.world.sight.armor_census();
        let stopped_by: BTreeMap<String, u32> = stats
            .stopped_by
            .iter()
            .map(|(&bits, &n)| {
                let armor = f32::from_bits(bits);
                (if armor.is_nan() { "no material".to_owned() } else { format!("{armor}") }, n)
            })
            .collect();
        let [plain, open_clear, open_through, shut_partial, shut_stopped, events] = self.cover;
        let json = serde_json::json!({
            "pierce_strikes_open": stats.strikes_open,
            "pierce_strikes_partial": stats.strikes_partial,
            "pierce_strikes_stopped": stats.strikes_stopped,
            "pierce_faces_crossed": stats.faces_crossed,
            "pierce_stopped_by": stopped_by,
            "pierce_walled_bands_team0": stats.walled_bands[0],
            "pierce_walled_bands_team1": stats.walled_bands[1],
            "pierce_cover_hits": game.cover_hits_by,
            "pierce_cover_kills": game.cover_kills_by,
            "pierce_mesh_faces": census.total,
            "pierce_mesh_free": census.free,
            "pierce_mesh_armored": census.armored,
            "pierce_mesh_unknown": census.unknown,
            "pierce_tests_plain": plain,
            "pierce_tests_open_clear": open_clear,
            "pierce_tests_open_through": open_through,
            "pierce_tests_shut_partial": shut_partial,
            "pierce_tests_shut_stopped": shut_stopped,
            "pierce_open_events": events,
        });
        let walled = |team: usize| {
            let b = stats.walled_bands[team];
            format!("stopped {} / <=0.25 {} / <=0.5 {} / <1 {} / all {}", b[0], b[1], b[2], b[3], b[4])
        };
        let text = format!(
            "owner-test pierce: bullet mesh {} faces ({} with armor, {} free, {} no material = unknown, stops all); strikes' lines open {} through cover {} stopped {} (by armor {:?}), {} faces crossed; shots into cover by the share kept, team 0 [{}], team 1 [{}]; hits through cover {:?} kills {:?}; cover tests: sight-only {} clear {} opened by the pierce {} shut with some damage {} shut and stopped {} ({} opened lines noted)",
            census.total,
            census.armored,
            census.free,
            census.unknown,
            stats.strikes_open,
            stats.strikes_partial,
            stats.strikes_stopped,
            stopped_by,
            stats.faces_crossed,
            walled(0),
            walled(1),
            game.cover_hits_by,
            game.cover_kills_by,
            plain,
            open_clear,
            open_through,
            shut_partial,
            shut_stopped,
            events,
        );
        Part::new(json, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::gun;
    use crate::v026g::bots::tests::soak_world_with;
    use mapdata::{Armor, FaceArmor, RayTree, World};

    /// A shot of body 0 at body 1 behind a wall of armor `armor` (a closed box across z = 20), counted.
    fn shot_through(armor: f32) -> serde_json::Value {
        let (points, triangles) = mapdata::library::PrimitiveShape::Box { half: Vec3::new(5.0, 2.0, 0.2) }.tessellate();
        let centre = Vec3::new(10.0, 2.0, 20.0);
        let tree = RayTree::from_armored_corners(
            triangles
                .iter()
                .map(|t| (t.map(|i| points[i as usize] + centre), FaceArmor::Material(Armor::new(armor).unwrap()))),
        );
        let base = soak_world_with(&[]);
        let world = std::sync::Arc::new(World::assemble(base.map.clone(), base.grid.clone(), tree.into()));
        let mut game = Game::new(world, &[0, 1], 1, None, None);
        game.volume = crate::v026g::bots::tests::standin_hitbox::HitVolume::BOXES;
        for (place, z) in [(0, 10.0), (1, 29.0)] {
            let body = &mut game.bodies[place];
            body.alive = true;
            body.health = 1.0;
            body.feet = Vec3::new(10.0, 0.0, z);
            body.yaw = 0.0;
        }
        game.bodies[0].target = Some(1);
        game.bodies[0].gun = gun("ak_74u").expect("the gun");
        let eye = Game::eye(&game.bodies[0]);
        let to = Vec3::new(10.0, 1.22, 29.0) - eye;
        game.bodies[0].yaw = crate::v026g::bots::kin::yaw_of([to.x, to.z]);
        game.bodies[0].pitch = to.y.atan2(to.x.hypot(to.z));
        game.shot(0, 0.0);
        let pierce = crate::v026g::bots::tests::standin_damage::pierce_of(&game.damage, game.bodies[0].gun).unwrap();
        let counts = PierceCounts::default();
        let mut json = counts.part(&game).json;
        json.insert("pierce_of_the_gun".to_owned(), serde_json::json!(pierce.value()));
        serde_json::Value::Object(json)
    }

    #[test]
    fn the_counts_name_what_the_strike_met() {
        // Wood (0.1) is crossed whole: an open line (the share is 1), one face crossed, and the shot was walled on the solid mesh.
        let wood = shot_through(0.1);
        assert_eq!(wood["pierce_strikes_open"], 1);
        assert_eq!(wood["pierce_faces_crossed"], 1);
        assert_eq!(wood["pierce_walled_bands_team0"], serde_json::json!([0, 0, 0, 0, 1]));
        // Stone (0.7) stops it, and the count names the armor.
        let stone = shot_through(0.7);
        assert_eq!(stone["pierce_strikes_stopped"], 1);
        assert_eq!(stone["pierce_stopped_by"], serde_json::json!({"0.7": 1}));
        assert_eq!(stone["pierce_walled_bands_team0"], serde_json::json!([1, 0, 0, 0, 0]));
        // Armor at 0.7 of the gun's pierce keeps some, under all: a partial line, in the 0.5-1 band.
        let pierce = wood["pierce_of_the_gun"].as_f64().unwrap() as f32;
        let partial = shot_through(0.7 * pierce);
        assert_eq!(partial["pierce_strikes_partial"], 1);
        assert_eq!(partial["pierce_walled_bands_team0"], serde_json::json!([0, 0, 0, 1, 0]));
        assert_eq!(partial["pierce_cover_hits"], serde_json::json!([1, 0]));
        // The census names the mesh.
        assert_eq!(wood["pierce_mesh_armored"], 12);
    }
}
