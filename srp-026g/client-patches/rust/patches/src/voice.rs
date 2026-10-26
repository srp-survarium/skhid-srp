//! Team voice policy. The game/DLL boundary is Vostok; no game address or
//! unchecked pointer reaches this module. A stopped match cannot retain PTT.
#![forbid(unsafe_code)]
use crate::vostok::{self, Context, Flow, GameCell, NoExit, Thread};
use vostok::voice::{
    self as engine, Connection, Dll, Key, MUTES_SIZE, Match, MenuIdentity, Settings, VoiceObject,
    VoiceOptions, VoiceText, VoiceValues, VoiceWorld,
};

#[derive(Clone, Copy)]
enum State {
    Unloaded,
    Missing,
    Ready { dll: Dll, running: Option<Running> },
}
#[derive(Clone, Copy)]
struct Running {
    settings: Option<Settings>,
    key: Option<Key>,
    identity: Option<MenuIdentity>,
    sent_masks: Option<(u32, u32)>,
    shown: Option<(u32, u32)>,
}
impl Running {
    const NEW: Self = Self {
        settings: None,
        key: None,
        identity: None,
        sent_masks: None,
        shown: None,
    };
}
static STATE: GameCell<State> = GameCell::new(State::Unloaded);

pub fn register(ctx: &mut Context<'_>) -> Flow<NoExit> {
    vostok::controls::initialize(ctx.thread());
    engine::register(ctx.thread());
    Flow::Continue
}
pub fn connect(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(connection) = Connection::at(ctx) else {
        return Flow::Continue;
    };
    let dll = match STATE.get(thread) {
        State::Unloaded => match Dll::load(thread) {
            Some(dll) => dll,
            None => {
                STATE.set(thread, State::Missing);
                return Flow::Continue;
            }
        },
        State::Missing => return Flow::Continue,
        State::Ready { dll, running } => {
            if running.is_some() {
                dll.ptt(false);
                dll.stop();
            }
            dll
        }
    };
    dll.ptt(false);
    let running = dll.start(&connection).then_some(Running::NEW);
    STATE.set(thread, State::Ready { dll, running });
    Flow::Continue
}
fn stop(thread: &Thread) {
    if let State::Ready { dll, running } = STATE.get(thread) {
        dll.ptt(false);
        if running.is_some() {
            dll.stop();
        }
        STATE.set(thread, State::Ready { dll, running: None });
    }
}
pub fn finish(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread());
    Flow::Continue
}
pub fn deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    stop(ctx.thread());
    Flow::Continue
}

fn far_slots(data: &Match) -> u32 {
    let me = data.local.and_then(|slot| data.players[slot].position);
    let mut mask = 0;
    for slot in 0..data.count {
        if data.local == Some(slot) {
            continue;
        }
        let far = match (me, data.players[slot].position) {
            (Some(a), Some(b)) => {
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                let distance = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                !distance.is_finite() || distance > 25.0 * 25.0
            }
            _ => true,
        };
        if far {
            mask |= 1 << slot;
        }
    }
    mask
}

const RECORD: usize = 9;
fn record(id: u32) -> [u8; RECORD] {
    let mut out = [b'|'; RECORD];
    for (i, c) in out[..8].iter_mut().enumerate() {
        *c = b"0123456789abcdef"[((id >> (28 - 4 * i)) & 15) as usize];
    }
    out
}
fn mute_position(mutes: &[u8; MUTES_SIZE], id: u32) -> Option<usize> {
    let wanted = record(id);
    let used = mutes.iter().position(|&c| c == 0).unwrap_or(MUTES_SIZE);
    mutes[..used]
        .chunks_exact(RECORD)
        .position(|r| r == wanted)
        .map(|n| n * RECORD)
}
fn is_muted(mutes: &[u8; MUTES_SIZE], id: u32) -> bool {
    id != 0 && mute_position(mutes, id).is_some()
}
fn set_mute(mutes: &mut [u8; MUTES_SIZE], id: u32, muted: bool) {
    if id == 0 {
        return;
    }
    let at = mute_position(mutes, id);
    let used = mutes.iter().position(|&c| c == 0).unwrap_or(MUTES_SIZE);
    let mut used = used - used % RECORD;
    mutes[used] = 0;
    match (muted, at) {
        (true, None) => {
            if used + RECORD + 1 > MUTES_SIZE {
                mutes.copy_within(RECORD..=used, 0);
                used -= RECORD;
            }
            mutes[used..used + RECORD].copy_from_slice(&record(id));
            mutes[used + RECORD] = 0;
        }
        (false, Some(at)) => {
            mutes.copy_within(at + RECORD..=used, at);
        }
        _ => {}
    }
}
fn frame(world: &VoiceWorld<'_>) {
    let thread = world.thread();
    let State::Ready {
        dll,
        running: Some(mut running),
    } = STATE.get(thread)
    else {
        return;
    };
    running.identity = world.identity();
    let settings = engine::settings(thread);
    if running.settings != Some(settings) {
        dll.settings(settings.volume, settings.microphone, settings.sensitivity);
        running.settings = Some(settings);
    }
    if running
        .key
        .is_some_and(|key| !settings.microphone || !key.held(thread) || !dll.focused())
    {
        dll.ptt(false);
        running.key = None;
    }
    let data = world.snapshot();
    let mutes = engine::mutes(thread);
    let mut mute = 0;
    for slot in 0..data.count {
        if dll.id(slot).is_some_and(|id| is_muted(&mutes, id)) {
            mute |= 1 << slot;
        }
    }
    let radio = far_slots(&data);
    if running.sent_masks.map(|m| m.0) != Some(mute) {
        dll.mute(mute);
    }
    if running.sent_masks.map(|m| m.1) != Some(radio) {
        dll.radio(radio);
    }
    running.sent_masks = Some((mute, radio));
    let mut talking = if settings.disabled { 0 } else { dll.talking() };
    if talking & (1 << 31) != 0 {
        talking &= !(1 << 31);
        if let Some(me) = data.local {
            talking |= 1 << me;
        }
    }
    talking &= !mute & ((1 << data.count) - 1);
    let shown = (talking, radio & talking);
    if running.shown != Some(shown) && world.talking(&data, talking, radio) {
        running.shown = Some(shown);
    }
    STATE.set(
        thread,
        State::Ready {
            dll,
            running: Some(running),
        },
    );
}
pub fn tick(
    orig: &vostok::OrigWorldTick,
    world: &mut VoiceWorld<'_>,
    first: u32,
    second: u32,
    third: u32,
) {
    frame(world);
    orig.call(world, first, second, third);
}
pub fn key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let State::Ready {
        dll,
        running: Some(mut running),
    } = STATE.get(thread)
    else {
        return Flow::Continue;
    };
    let Some(key) = Key::from_context(ctx) else {
        return Flow::Continue;
    };
    let settings = engine::settings(thread);
    match key.action {
        0x47 => {
            if key.down && running.key.is_none() && settings.microphone {
                dll.ptt(true);
                running.key = Some(key);
            } else if !key.down && running.key.is_some_and(|held| held.code == key.code) {
                dll.ptt(false);
                running.key = None;
            }
        }
        0x37 if key.down && !settings.disabled => {
            let data = key.identity().snapshot(thread);
            if let Some(slot) = dll.last().filter(|&slot| data.teammate(slot))
                && let Some(id) = dll.id(slot)
            {
                let mut mutes = engine::mutes(thread);
                let muted = !is_muted(&mutes, id);
                set_mute(&mut mutes, id, muted);
                engine::write_mutes(thread, &mutes);
                key.notice(muted, &data.players[slot].name);
            }
        }
        _ => {}
    }
    STATE.set(
        thread,
        State::Ready {
            dll,
            running: Some(running),
        },
    );
    Flow::Continue
}
pub fn menu(orig: &vostok::OrigOptionsActivate, options: &mut VoiceOptions<'_>, state: u32) -> u32 {
    let answer = orig.call(options, state);
    let thread = options.thread();
    let settings = engine::settings(thread);
    let State::Ready {
        dll,
        running: Some(_),
    } = STATE.get(thread)
    else {
        options.panel(state, false, |_| None);
        return answer;
    };
    let mutes = engine::mutes(thread);
    options.panel(state, !settings.disabled, |slot| {
        dll.id(slot).map(|id| is_muted(&mutes, id))
    });
    answer
}
pub fn menu_call(
    orig: &vostok::OrigOptionsCallback,
    handler: &mut VoiceObject<'_>,
    unknown: u32,
    name: &mut VoiceText<'_>,
    args: &mut VoiceValues<'_>,
    count: u32,
) {
    let one = name.is(c"srp_voice_mute");
    let all = name.is(c"srp_voice_mute_all");
    if !one && !all {
        orig.call(handler, unknown, name, args, count);
        return;
    }
    let thread = handler.thread();
    let State::Ready {
        dll,
        running: Some(running),
    } = STATE.get(thread)
    else {
        return;
    };
    let Some(identity) = running.identity else {
        return;
    };
    let data = identity.snapshot(thread);
    let mut mutes = engine::mutes(thread);
    if one {
        let (Some(slot), Some(muted)) = (args.number(0, count), args.number(1, count)) else {
            return;
        };
        if data.teammate(slot as usize)
            && let Some(id) = dll.id(slot as usize)
        {
            set_mute(&mut mutes, id, muted != 0);
        }
    } else if let Some(muted) = args.number(0, count) {
        for slot in 0..data.count {
            if data.teammate(slot)
                && let Some(id) = dll.id(slot)
            {
                set_mute(&mut mutes, id, muted != 0);
            }
        }
    }
    engine::write_mutes(thread, &mutes);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutes_follow_ids_and_drop_oldest_at_capacity() {
        let mut m = [0; MUTES_SIZE];
        set_mute(&mut m, 1, true);
        set_mute(&mut m, 2, true);
        set_mute(&mut m, 1, false);
        assert!(!is_muted(&m, 1));
        assert!(is_muted(&m, 2));
        set_mute(&mut m, 0, true);
        assert!(!is_muted(&m, 0));
        for id in 3..100 {
            set_mute(&mut m, id, true);
        }
        assert!(!is_muted(&m, 2));
        assert!(is_muted(&m, 99));
        assert!(m.contains(&0));
        let before = m;
        set_mute(&mut m, 99, true);
        assert_eq!(before, m);
    }
    #[test]
    fn radio_boundary_and_dead_players() {
        let mut m = Match::EMPTY;
        m.count = 3;
        m.local = Some(0);
        m.players[0].position = Some([0.0; 3]);
        m.players[1].position = Some([25.0, 0.0, 0.0]);
        assert_eq!(far_slots(&m), 4);
        m.players[1].position = Some([25.01, 0.0, 0.0]);
        assert_eq!(far_slots(&m), 6);
        m.players[0].position = None;
        assert_eq!(far_slots(&m), 6);
    }
}
