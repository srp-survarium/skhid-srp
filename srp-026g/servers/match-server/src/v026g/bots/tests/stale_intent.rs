use super::*;

/// [stale intent] A play the commander gives makes the bot's think due at once, and the poll's thinks run after the commander and
/// before the ticks: after every poll a living bot's intent is its present play's (`Bot::intent_is_current`), and no drive tick
/// went for the play before's goal. Before, the drive kept the old play's goal until the bot's own next think (up to
/// `think_ms`: its first search and its first steps on the old play's goal). A lone bot in a team deathmatch, 60 s, its play
/// ended every 5 s for the commander to give another.
#[test]
pub(crate) fn a_new_play_is_driven_for_its_own_goal_from_the_poll_that_gives_it() {
    let world = soak_world();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x51A7,
        survarium::v026g::maps::RuleMode::TeamDeathmatch,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    let (mut given, mut thought_at_once) = (0u32, 0u32);
    while clock < 60_000 {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        // Every 5 s its play is ended and the commander is due: it gives another (a lone bot keeps its play for long).
        if clock % 5_000 == 0
            && let Some(bot) = bots.bots[0].as_mut()
            && bot.assignment.is_some()
        {
            bot.end_play(clock, "the test asks for another play", &mut Vec::new());
            bots.teams[0].next_commander = clock;
        }
        let before = bots.bots[0].as_ref().map(|bot| (bot.plays_given, bot.next_think));
        let (made, _) = bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let bot = bots.bots[0].as_ref().expect("bot 0");
        let alive = bots.tracker.body(0).and_then(|body| body.fix()).is_some_and(|fix| fix.alive);
        if let Some((plays_before, next_think_before)) = before
            && bot.plays_given != plays_before
        {
            given += 1;
            // Its think was not due by its own draw: the play's own made it.
            if next_think_before > clock {
                thought_at_once += 1;
            }
        }
        if alive {
            assert!(
                bot.intent_is_current(),
                "at {clock} ms the intent {:?} is for a play before its {:?}",
                bot.intent.go,
                bot.assignment.as_ref().map(|a| a.kind)
            );
        }
    }
    assert!(given >= 5, "{given} plays given in 60 s");
    assert!(thought_at_once >= 1, "no play given between two of its own thinks ({given} given)");
}
