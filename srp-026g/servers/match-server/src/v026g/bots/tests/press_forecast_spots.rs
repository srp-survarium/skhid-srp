//! [press forecast] Mamayev Kurgan's rise (map 91, the gap jump (-22.60, 24.27, 57.88) -> (-21.85, 25.81, 57.88)) with the press
//! forecast on (`Feel::press_forecast`, the defaults): a bot on the stand-in over the real mapdata and the server's game data is
//! sent across the link from the starts `rest_spots` finds (up to 4, 3-8 m off, whose planned route takes it). Per crossing it
//! prints the forecast's cost (the press clock's controller steps and ns, its chains and flights), the motor's forecast lines
//! (`press forecast: ...`) and where the bot came down.
//!
//! Asserted: the link is pressed from a start; every motor press of the link is the forecast's (its line says so).
//! Printed against the +2 % bar (the design's §5.3: 22.8 s of bot time a 10-minute match, 194 crossings a match from the live
//! matches' visits, so 2.35 ms a crossing): the time a crossing at this box's step cost, and the controller steps a crossing
//! against the bar's steps at the bench's step cost (13-20 us, §5.1; a press step costs what a reckoning step does, #2285),
//! which no other load on the box moves.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release press_forecast_spots -- --ignored --nocapture --test-threads=1
//! ```

use super::*;

use super::rest_spots::{Map91, cross, map91};
use crate::v026g::bots::body::kept_clock;
use crate::v026g::bots::motor::press_casts;

/// A 10-minute match's bot time, ms (design §5.3: 12,000 ticks x 1.9 ms).
const MATCH_BOT_MS: f64 = 22_800.0;
/// Crossings of pressed links a match (design §5.3: 488 + 93 visits in 3 live matches).
const CROSSINGS_A_MATCH: f64 = 194.0;
/// The bar: the forecast's share of a match's bot time, %.
const BAR_PERCENT: f64 = 2.0;
/// The bench's controller step at its dearest, ns (design §5.1: 13,025-19,851 ns over four bench logs).
const BENCH_STEP_NS: f64 = 19_851.0;
/// The live box is this many times slower than the dev box (the coordinator's scale for the 1-core live host).
const LIVE_SCALE: f64 = 3.0;
/// The most controller steps one forecast of a poll plays: a chain (`motor` `CHAIN_TICKS_MAX`, 24) or a flight (the press step, 19
/// flight steps, 7 stay steps: 27); the match's one forecast a poll (`body::PressToken`) makes it the worst poll's.
const WORST_FORECAST_STEPS: f64 = 27.0;

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn press_forecast_crossings_on_map_91() {
    let Some(Map91 { world, a, b, goal, starts, .. }) = map91() else {
        return;
    };
    let tuning = Tuning::default();
    assert!(tuning.feel.press_forecast, "the forecast is on by default");
    let _ = kept_clock::take(kept_clock::Of::Press);
    let _ = press_casts::take_match();
    let (mut pressed, mut landed, mut not_forecast) = (0, 0, Vec::new());
    let (mut all_ms, mut all_steps, mut crossings) = (0.0f64, 0u64, 0u32);
    let (mut all_flights, mut all_chains, mut all_ns) = (0u64, 0u64, 0u64);
    let (mut clean, mut retry): (Vec<(u64, u64, f64)>, Vec<(u64, u64, f64)>) = (Vec::new(), Vec::new());
    let n_starts = starts.len();
    for start in starts {
        let c = cross(&world, start, a, b, goal, 40_000, tuning.clone());
        let (steps, ns) = kept_clock::take(kept_clock::Of::Press);
        let (chains, flights) = press_casts::take_match();
        let ms = ns as f64 / 1e6;
        all_ms += ms;
        all_steps += steps;
        all_flights += flights;
        let spent = c.notes.iter().any(|(_, text)| text.contains("press forecast: try") && text.contains(" spent"));
        if c.presses == 1 && !spent {
            clean.push((steps, flights, ms));
        } else {
            retry.push((steps, flights, ms));
        }
        all_chains += chains;
        all_ns += ns;
        crossings += 1;
        let lines: Vec<&(u32, String)> = c.notes.iter().filter(|(_, text)| text.contains("press forecast")).collect();
        eprintln!(
            "PRESS SPOT from ({:.2}, {:.2}, {:.2}) ({:.1} m): landed {:?} ms, presses {}; forecast {steps} steps, {} ns a step, {ms:.2} ms, {chains} chains, {flights} flights; {:.2} % of a match at {CROSSINGS_A_MATCH} crossings",
            start.x,
            start.y,
            start.z,
            start.distance_xz(a),
            c.landed,
            c.presses,
            ns.checked_div(steps).unwrap_or(0),
            ms * CROSSINGS_A_MATCH / MATCH_BOT_MS * 100.0
        );
        for (t, text) in &lines {
            eprintln!("PRESS SPOT   {t} ms: {text}");
        }
        // The stand-in's body (the game's, not the bot's reckoning) 2.5 s after each press: up on the link's landing or not.
        for &(t, feet, press) in &c.trail {
            if !press {
                continue;
            }
            if let Some(&(later, at, _)) = c.trail.iter().find(|&&(u, _, p)| !p && u >= t + 2_500) {
                eprintln!(
                    "PRESS SPOT   press at {t} ms from ({:.2}, {:.2}, {:.2}): the game's body at {later} ms ({:.2}, {:.2}, {:.2}), {}",
                    feet.x,
                    feet.y,
                    feet.z,
                    at.x,
                    at.y,
                    at.z,
                    if crate::v026g::bots::motor::is_up(at, a, b) { "up" } else { "not up" }
                );
            }
        }
        landed += usize::from(c.landed.is_some());
        let at_link = c.presses_at.iter().filter(|press| press.link.is_some_and(|(from, to)| (from - a).length() <= 0.3 && (to - b).length() <= 0.3)).count();
        let made = lines.iter().filter(|(_, text)| text.contains(" pressed")).count();
        pressed += usize::from(at_link > 0);
        if at_link > made {
            not_forecast.push((start, at_link, made));
        }
    }
    // Clean crossings (one press, no try spent) and the ones with retries, apart: their forecast steps and flights.
    for (name, list) in [("clean", &clean), ("retry", &retry)] {
        let n = list.len().max(1) as f64;
        let (steps, flights, ms) = list.iter().fold((0u64, 0u64, 0.0f64), |acc, &(s, f, m)| (acc.0 + s, acc.1 + f, acc.2 + m));
        eprintln!(
            "PRESS SPOT {name} crossings {}: {:.0} steps, {:.1} flights, {:.2} ms a crossing here; {:.3} % of the live core a match at {CROSSINGS_A_MATCH} crossings",
            list.len(),
            steps as f64 / n,
            flights as f64 / n,
            ms / n,
            ms / n * CROSSINGS_A_MATCH * LIVE_SCALE / 600_000.0 * 100.0
        );
    }
    let per_crossing = all_ms / f64::from(crossings.max(1));
    let share = per_crossing * CROSSINGS_A_MATCH / MATCH_BOT_MS * 100.0;
    let steps_crossing = all_steps as f64 / f64::from(crossings.max(1));
    let bar_steps = BAR_PERCENT / 100.0 * MATCH_BOT_MS / CROSSINGS_A_MATCH * 1e6 / BENCH_STEP_NS;
    eprintln!(
        "PRESS SPOT pressed from {pressed} of {n_starts} starts, came down on the landing from {landed}; {per_crossing:.2} ms a crossing here, {share:.2} % of a match; {steps_crossing:.0} steps a crossing against the bar's {bar_steps:.0} at the bench's dearest step, {:.2} % of a match",
        steps_crossing * BENCH_STEP_NS / 1e6 * CROSSINGS_A_MATCH / MATCH_BOT_MS * 100.0
    );
    // Against the bar, the figures the PR states: flights a crossing, ms a flight (27 steps at this box's step), the worst poll (one
    // forecast: 27 steps), and the share of a match's bot time, at this box and at the live box (3x).
    let step_ns = all_ns as f64 / (all_steps.max(1)) as f64;
    let n = f64::from(crossings.max(1));
    let flight_ms = WORST_FORECAST_STEPS * step_ns / 1e6;
    eprintln!(
        "PRESS SPOT BAR {:.1} flights and {:.1} chains a crossing; {:.0} ns a step here, {flight_ms:.2} ms a flight here, {:.2} ms live; worst poll one forecast: {flight_ms:.2} ms here, {:.2} ms live (of a 50 ms tick); {:.2} ms a crossing here: {share:.2} % of a match here, {:.2} % live (bar {BAR_PERCENT} %)",
        all_flights as f64 / n,
        all_chains as f64 / n,
        step_ns,
        flight_ms * LIVE_SCALE,
        flight_ms * LIVE_SCALE,
        per_crossing,
        share * LIVE_SCALE
    );
    // The 1-core budget: a 10-minute match on the live host's one core is 600 s; the live host runs up to 7 matches at once.
    let match_ms = per_crossing * CROSSINGS_A_MATCH;
    let core_share = match_ms * LIVE_SCALE / 600_000.0 * 100.0;
    eprintln!(
        "PRESS SPOT CORE {match_ms:.0} ms a match here ({CROSSINGS_A_MATCH} crossings), {:.0} ms live: {core_share:.3} % of the live core a match, {:.2} % for 7 matches (bar {BAR_PERCENT} %)",
        match_ms * LIVE_SCALE,
        core_share * 7.0
    );
    assert!(pressed > 0, "no start pressed the link");
    assert!(not_forecast.is_empty(), "motor presses of the link with no forecast line (start, presses, lines): {not_forecast:?}");
}

/// [press forecast] The stand-in game against the forecast on one press from rest at map 91: the same keys (at rest, one tick of
/// forward, forward and jump, forward through the flight, let go) from the same rest, the game's body each tick printed against
/// the forecast's outcome. A measurement: where the two flights part.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn the_stand_in_and_the_forecast_fly_a_rest_press_on_map_91() {
    use crate::v026g::bots::body::{AfterLanding, Estimate, PressPlan, PressWorld, press_forecast};
    let Some(Map91 { world, a, b, way, .. }) = map91() else {
        return;
    };
    let grid = &world.grid;
    let kin = Tuning::default().kin;
    let yaw = crate::v026g::bots::kin::yaw_of([way.0, way.1]);
    for rest in [Vec3::new(-23.07, 24.29, 58.01), Vec3::new(-22.69, 24.27, 57.88), Vec3::new(-22.97, 24.47, 57.57)] {
        let node = grid.locate(rest, 1.0);
        let rest = node.map_or(rest, |n| Vec3::new(rest.x, grid.position(n).y, rest.z));
        let at = Estimate { node, ..Estimate::at_rest(rest, yaw) };
        let plan = PressPlan { walk: key::FORWARD, press_in: 1, take_off: a, landing: b, after: AfterLanding::Hold };
        let outcome = press_forecast(&at, &plan, &PressWorld { kin: &kin, level: 1.0, grid });
        let teams = [0u8, 1];
        let mut game = Game::new(world.clone(), &teams, 3, None, None);
        game.bodies[0].node = node;
        game.bodies[0].feet = rest;
        game.bodies[0].yaw = yaw;
        game.bodies[1].alive = false;
        let gun = Arms::fallback().primary.0;
        let mut keys: Vec<u32> = vec![0; 6];
        keys.push(key::FORWARD);
        keys.push(key::FORWARD | key::JUMP);
        keys.extend(std::iter::repeat_n(key::FORWARD, 19));
        keys.extend(std::iter::repeat_n(0, 10));
        let mut trail = Vec::new();
        for k in keys {
            let t = game.tick + TICK_MS;
            game.inputs[0].push_back((t, k, (0.0, 0.0), None, gun));
            game.step();
            trail.push(game.bodies[0].feet);
        }
        let end = *trail.last().unwrap();
        eprintln!(
            "PRESS FLY from ({:.2}, {:.2}, {:.2}): forecast {outcome:?}; the game's body at the end ({:.2}, {:.2}, {:.2}) {}",
            rest.x,
            rest.y,
            rest.z,
            end.x,
            end.y,
            end.z,
            if crate::v026g::bots::motor::is_up(end, a, b) { "up" } else { "not up" }
        );
        for (i, p) in trail.iter().enumerate().skip(5) {
            eprintln!("PRESS FLY   tick {i}: ({:.3}, {:.3}, {:.3})", p.x, p.y, p.z);
        }
    }
}
