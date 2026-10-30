//! The owner counts of three features of the bots' senses that change little in a score and so need their own mark
//! (A122): `sense.ranges.game` (hearing in 3D, the crouched step), `sense.sprint_body_mps` (the sprint sound follows the
//! body) and `sense.use_ray_m` (the length of the use ray in the tracker's carrier guess). By the team of the body that hears,
//! makes the sound or presses the use. A count of 0 on an arm that has the feature on says it never ran; the arm with it off
//! must read 0 where the feature adds (crouched steps, refused-by-game sounds, quiet sprints).

use super::*;
use crate::v026g::bots::sense::HeardCounts;
use crate::v026g::bots::view::SenseActivity;

/// The match-wide counts as the last tick left them (the polls and the tracker keep them; the topic copies).
#[derive(Default)]
pub(super) struct SenseCounts {
    heard: [HeardCounts; 2],
    activity: [SenseActivity; 2],
}

impl Topic for SenseCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.heard = bots.polls().heard_by_team;
        self.activity = bots.tracker.sense_activity();
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("sense", self.part())]
    }
}

impl SenseCounts {
    fn part(&self) -> Part {
        let [h0, h1] = self.heard;
        let [a0, a1] = self.activity;
        let team_json = |h: &HeardCounts, a: &SenseActivity| {
            serde_json::json!({
                "steps": h.steps,
                "steps_crouched": h.steps_crouched,
                "sounds_refused_by_game": h.sounds_refused,
                "steps_refused_by_game": h.steps_refused,
                "sprint_sounds": a.sprint_sounds,
                "sprint_quiet": a.sprint_quiet,
                "use_judged": a.use_judged,
                "use_short": a.use_short,
            })
        };
        let json = serde_json::json!({
            "sense_steps_heard": h0.steps + h1.steps,
            "sense_steps_crouched_heard": h0.steps_crouched + h1.steps_crouched,
            "sense_refused_by_game": h0.sounds_refused + h1.sounds_refused + h0.steps_refused + h1.steps_refused,
            "sense_sprint_sounds": a0.sprint_sounds + a1.sprint_sounds,
            "sense_sprint_quiet": a0.sprint_quiet + a1.sprint_quiet,
            "sense_use_judged": a0.use_judged + a1.use_judged,
            "sense_use_short": a0.use_short + a1.use_short,
            "sense": { "by_team": { "0": team_json(&h0, &a0), "1": team_json(&h1, &a1) } },
        });
        let text = format!(
            "owner-test sense: steps heard {} upright, {} crouched (the game's ranges); refused by the game's ranges where the flat ones carry: {} sounds, {} steps; sprint sounds made {}, held keys kept quiet by the body {}; carrier guesses from a use press {}, ray ended short of the battery {}",
            h0.steps + h1.steps,
            h0.steps_crouched + h1.steps_crouched,
            h0.sounds_refused + h1.sounds_refused,
            h0.steps_refused + h1.steps_refused,
            a0.sprint_sounds + a1.sprint_sounds,
            a0.sprint_quiet + a1.sprint_quiet,
            a0.use_judged + a1.use_judged,
            a0.use_short + a1.use_short,
        );
        Part::new(json, text)
    }
}

/// The topic copies the two sets of counts and gives them by team and summed.
#[test]
pub(crate) fn the_sense_topic_gives_the_hearing_and_the_use_ray_counts() {
    let mut topic = SenseCounts::default();
    topic.heard[0] = HeardCounts { steps: 5, steps_crouched: 2, sounds_refused: 1, steps_refused: 3 };
    topic.heard[1].steps = 4;
    topic.activity[1] = SenseActivity { sprint_sounds: 7, sprint_quiet: 9, use_judged: 6, use_short: 2 };
    let part = topic.part();
    assert_eq!(part.json["sense_steps_heard"], 9);
    assert_eq!(part.json["sense_steps_crouched_heard"], 2);
    assert_eq!(part.json["sense_refused_by_game"], 4);
    assert_eq!(part.json["sense"]["by_team"]["1"]["sprint_quiet"], 9);
    assert_eq!(part.json["sense"]["by_team"]["0"]["steps_refused_by_game"], 3);
    assert!(part.text.contains("steps heard 9 upright, 2 crouched") && part.text.contains("held keys kept quiet by the body 9"), "{}", part.text);
    assert!(part.text.contains("ray ended short of the battery 2"), "{}", part.text);
}
