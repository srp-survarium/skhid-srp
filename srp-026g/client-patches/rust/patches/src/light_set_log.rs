//! Patch 189: a diagnostic of which environment probes, ambient lights and
//! lights each frame draws (renderer-026g.md 4.3, 5.3, 5.4).
//!
//! The owner sees the light of a place change over a step (London (v0.42),
//! 2026-11-22) that a capture of the same two eyes does not show. Every
//! frame the culling finds the probes, ambient lights and lights in view,
//! and the frame leaves out each one that is disabled, occluded by the last
//! hi-Z answer, or (a probe) whose cubemaps are not ready
//! (`vostok::light_sets`). With `SKHID_LIGHT_SET_LOG=on` (read once) every
//! change of that is logged: a probe, ambient light or light in view that
//! starts or stops being drawn and why, a probe coming into or out of view,
//! and each probe the first time it is seen with its sort key (its draw
//! order: largest first, the key differs from run to run). Off, it reads
//! nothing.

#![forbid(unsafe_code)]

use crate::vostok::clock;
use crate::vostok::light_sets::{Culled, Light, LightState, MOST, Probe, ProbeState};
use crate::vostok::log::{Piece, Piece::*, line};
use crate::vostok::{Context, Flow, GameCell, Lock, NoExit, Thread};

/// What was in view the frame before, and the probes seen in the run.
struct Seen {
    probes: [Option<(u32, ProbeState)>; MOST],
    ambient: [Option<(u32, LightState)>; MOST],
    lights: [Option<(u32, LightState)>; MOST],
    known: [u32; MOST],
    counts: [u32; 6],
}

static SEEN: Lock<Seen> = Lock::new(Seen {
    probes: [None; MOST],
    ambient: [None; MOST],
    lights: [None; MOST],
    known: [0; MOST],
    counts: [0; 6],
});

static FRAME: Lock<Culled> = Lock::new(Culled { probes: [None; MOST], ambient: [None; MOST], lights: [None; MOST] });

/// `stage_visibility.culled`: once a frame, before the frame's filters.
pub fn on_culled(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !log_on(ctx.thread()) {
        return Flow::Continue;
    }
    let thread = ctx.thread();
    FRAME.with(thread, |frame| {
        if Culled::read_at_site(ctx, frame) {
            SEEN.with(thread, |seen| compare(thread, frame, seen));
        }
    });
    Flow::Continue
}

fn before<S: Copy>(last: &[Option<(u32, S)>], id: u32) -> Option<S> {
    last.iter().flatten().find(|(at, _)| *at == id).map(|&(_, state)| state)
}

fn compare(thread: &Thread, frame: &Culled, seen: &mut Seen) {
    let ms = clock::milliseconds(thread);
    for probe in frame.probes.iter().flatten() {
        if !seen.known.contains(&probe.id)
            && let Some(slot) = seen.known.iter_mut().find(|slot| **slot == 0)
        {
            *slot = probe.id;
            probe_line(ms, probe, "first seen, sort key ", &[]);
        }
        match before(&seen.probes, probe.id) {
            None => probe_line(ms, probe, "comes into view, ", &[Text(probe_state(probe.state))]),
            Some(was) if was != probe.state => {
                probe_line(ms, probe, "", &[Text(probe_state(was)), Text(" -> "), Text(probe_state(probe.state))])
            }
            Some(_) => {}
        }
    }
    for (id, was) in seen.probes.iter().flatten() {
        if !frame.probes.iter().flatten().any(|probe| probe.id == *id) {
            line(&[Text("light_sets: "), Dec(ms), Text(" ms probe "), Hex(*id), Text(" goes out of view, was "), Text(probe_state(*was))]);
        }
    }
    lights(ms, "ambient", &frame.ambient, &seen.ambient);
    lights(ms, "light", &frame.lights, &seen.lights);

    let counts = [
        count(&frame.probes, |_: &Probe| true),
        count(&frame.probes, |p: &Probe| p.state == ProbeState::Drawn),
        count(&frame.ambient, |_: &Light| true),
        count(&frame.ambient, |l: &Light| l.state == LightState::Drawn),
        count(&frame.lights, |_: &Light| true),
        count(&frame.lights, |l: &Light| l.state == LightState::Drawn),
    ];
    if counts != seen.counts {
        seen.counts = counts;
        line(&[
            Text("light_sets: "),
            Dec(ms),
            Text(" ms in view/drawn: probes "),
            Dec(counts[0]),
            Text("/"),
            Dec(counts[1]),
            Text(", ambient lights "),
            Dec(counts[2]),
            Text("/"),
            Dec(counts[3]),
            Text(", lights "),
            Dec(counts[4]),
            Text("/"),
            Dec(counts[5]),
        ]);
    }
    for (slot, probe) in seen.probes.iter_mut().zip(frame.probes.iter()) {
        *slot = probe.map(|p| (p.id, p.state));
    }
    for (slot, light) in seen.ambient.iter_mut().zip(frame.ambient.iter()) {
        *slot = light.map(|l| (l.id, l.state));
    }
    for (slot, light) in seen.lights.iter_mut().zip(frame.lights.iter()) {
        *slot = light.map(|l| (l.id, l.state));
    }
}

fn count<T>(items: &[Option<T>], keep: impl Fn(&T) -> bool) -> u32 {
    items.iter().flatten().filter(|item| keep(item)).count() as u32
}

/// An ambient light or light in view that starts or stops being drawn (or
/// comes into view not drawn).
fn lights(ms: u32, kind: &str, now: &[Option<Light>], last: &[Option<(u32, LightState)>]) {
    for light in now.iter().flatten() {
        let change: [Piece; 3] = match before(last, light.id) {
            None if light.state != LightState::Drawn => [Text("comes into view, "), Text(light_state(light.state)), Text("")],
            Some(was) if was != light.state => [Text(light_state(was)), Text(" -> "), Text(light_state(light.state))],
            _ => continue,
        };
        let (x, y, z, r, i) = (
            Fixed::new(light.location[0], 2),
            Fixed::new(light.location[1], 2),
            Fixed::new(light.location[2], 2),
            Fixed::new(light.radius, 2),
            Fixed::new(light.intensity, 2),
        );
        line(&[
            Text("light_sets: "),
            Dec(ms),
            Text(" ms "),
            Text(kind),
            Text(" at "),
            Text(x.text()),
            Text(" "),
            Text(y.text()),
            Text(" "),
            Text(z.text()),
            Text(" radius "),
            Text(r.text()),
            Text(" intensity "),
            Text(i.text()),
            Text(": "),
            change[0],
            change[1],
            change[2],
        ]);
    }
}

fn probe_line(ms: u32, probe: &Probe, lead: &str, rest: &[Piece]) {
    let key = Fixed::new(probe.radius, 4);
    let (x, y, z) = (Fixed::new(probe.location[0], 2), Fixed::new(probe.location[1], 2), Fixed::new(probe.location[2], 2));
    let mut pieces = [Text(""); 24];
    let head = [
        Text("light_sets: "),
        Dec(ms),
        Text(" ms probe "),
        Hex(probe.id),
        Text(" "),
        Text(probe.name()),
        Text(" at "),
        Text(x.text()),
        Text(" "),
        Text(y.text()),
        Text(" "),
        Text(z.text()),
        Text(": "),
        Text(lead),
    ];
    pieces[..head.len()].copy_from_slice(&head);
    let mut at = head.len();
    if lead.starts_with("first seen") {
        pieces[at] = Text(key.text());
        at += 1;
    }
    for piece in rest.iter().take(pieces.len() - at) {
        pieces[at] = *piece;
        at += 1;
    }
    line(&pieces[..at]);
}

fn probe_state(state: ProbeState) -> &'static str {
    match state {
        ProbeState::Drawn => "drawn",
        ProbeState::Disabled => "disabled",
        ProbeState::NoCubemap => "cubemap not ready",
        ProbeState::NoDiffuse => "diffuse cubemap not ready",
        ProbeState::Occluded => "occluded",
    }
}

fn light_state(state: LightState) -> &'static str {
    match state {
        LightState::Drawn => "drawn",
        LightState::Disabled => "disabled",
        LightState::Occluded => "occluded",
    }
}

/// A float as text with a fixed number of decimals (no `core::fmt`).
struct Fixed {
    bytes: [u8; 16],
    len: usize,
}

impl Fixed {
    fn new(value: f32, decimals: u32) -> Fixed {
        let mut bytes = [0u8; 16];
        let mut len = 0;
        let scale = 10u32.pow(decimals) as f32;
        let scaled = value * scale;
        let negative = scaled < 0.0;
        let magnitude = if negative { -scaled } else { scaled };
        let units = if magnitude.is_finite() && magnitude < 1.0e9 { (magnitude + 0.5) as u32 } else { u32::MAX };
        if negative && units != 0 {
            bytes[len] = b'-';
            len += 1;
        }
        let divisor = 10u32.pow(decimals);
        let mut digits = [0u8; 10];
        let mut whole = units / divisor;
        let mut n = 0;
        loop {
            digits[n] = b'0' + (whole % 10) as u8;
            n += 1;
            whole /= 10;
            if whole == 0 {
                break;
            }
        }
        for &digit in digits[..n].iter().rev() {
            bytes[len] = digit;
            len += 1;
        }
        if decimals > 0 {
            bytes[len] = b'.';
            len += 1;
            let mut fraction = units % divisor;
            let mut place = divisor / 10;
            while place > 0 {
                bytes[len] = b'0' + (fraction / place) as u8;
                len += 1;
                fraction %= place;
                place /= 10;
            }
        }
        Fixed { bytes, len }
    }

    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("?")
    }
}

/// SKHID_LIGHT_SET_LOG as the game was started with it, read at the first frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    Unread,
    Off,
    On,
}

static LOG: GameCell<Switch> = GameCell::new(Switch::Unread);

fn log_on(thread: &Thread) -> bool {
    let switch = match LOG.get(thread) {
        Switch::Unread => {
            let mut room = [0u8; 8];
            let len = crate::vostok::renderer::get_environment(thread, c"SKHID_LIGHT_SET_LOG", &mut room);
            let value = room.get(..len).unwrap_or(&[]);
            let read = if value.eq_ignore_ascii_case(b"on") || value == b"1" { Switch::On } else { Switch::Off };
            LOG.set(thread, read);
            read
        }
        known => known,
    };
    switch == Switch::On
}

#[cfg(test)]
mod tests {
    use super::Fixed;

    #[test]
    fn fixed_decimals() {
        assert_eq!(Fixed::new(-32.449, 2).text(), "-32.45");
        assert_eq!(Fixed::new(5.0734, 4).text(), "5.0734");
        assert_eq!(Fixed::new(0.004, 2).text(), "0.00");
        assert_eq!(Fixed::new(12.6, 2).text(), "12.60");
    }
}
