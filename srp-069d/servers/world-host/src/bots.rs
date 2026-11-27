//! Online bots driven by the exe's own AI, run inside the world host.
//!
//! The controllers world `ai_player_controllers_world` is a sub-object of the game, `game+0x1691170`, built in place by
//! the game ctor (an inline pool of 20 controllers of 0xA98 bytes, free list filled). Every client has it, un-started.
//! The host starts it once per match and ticks it after every DF step; the controllers then insert the bots' inputs
//! into the host world with the DF handler's own flags. This module collects those inputs per tick (`BotTick`), the
//! list the relay puts on the DF path.
//!
//! Every call is the exe's own (PROVEN entry points):
//! * start `0x2002D6B50(ctl, u32 start_time, &core_ptr, &boost::function, &graph_res, &ai_sounds_res, u8 use_graphs)`:
//!   loads the team patrol graphs and the AI sound schemes, seeds the sound player with the match id and builds one
//!   controller per options record with `ai_id (+0x2E0) != -1`, calling the functor once per controller (an empty
//!   functor is fatal). The host passes a no-op functor: the client's one (0x2001F4BE0) inserts a profile input with
//!   uninitialised bytes, which the relay's DFs do instead.
//! * tick, after DF t is stepped: `0x20036DB20(W)` (the own-motion ring), then `0x2002D7220(ctl, &core_ptr, t)`, as the
//!   local frame does. Each controller inserts `change_input<player_input>(W, {input, t + tick, i}, 0, 1, 0, 0)` when
//!   its actions differ from the newest node of the player's list or the rotation delta is not zero; voice lines go in
//!   as `ai_player_input {2, code}` at t + tick.
//! * PvE: the match user data's +0x38 = the controllers world before `pvp_match_resource`: the scene ctor copies it to
//!   scene+0x1BB10, the only gate of the scene script's AI calls (`set_ai_input`, type 15 patrols, patrol-at-spawn,
//!   assign_graph). Clients keep 0 there.
//!
//! Input lists (PROVEN: the DF handler's ai insert 0x2001B49B5, the tick's read 0x2002D7357):
//! player inputs `L = [W+0x7DF0] + i*0x4218`, ai inputs `[W+0xFFBD8] + i*0x4218`; header +0x10 newest node; node +8 the
//! next older, time +0x148; player node payload u16 +0x10, u16 +0x12, u64 +0x18; ai node payload u16 type +0x10, u16
//! param +0x12. After DF t every node with time > t is host-made (every DF entry up to t has a time <= t).
//!
//! Switch: `HOST_BOTS=pve|pvp` (in-process replay and daemon children; off by default). `HOST_BOTS_REC=FILE` writes a copy
//! of the replayed recording with every collected bot entry merged into the DF of its time (the "inputs are the only
//! exit" check: a second host without the AI replays it).

use crate::hlog;
use crate::run::{rd64, rd8};
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub const CONTROLLERS: u64 = 0x1691170;
const AI_START: u64 = 0x2002D6B50;
const AI_TICK: u64 = 0x2002D7220;
const AI_MOTION_RING: u64 = 0x20036DB20;
/// how far back the collection checks the lists each tick: late inputs reach at most 255 ms back (dt is a u8), plus a tick
const WINDOW: u32 = 400;
const AI_SOUNDS: &str = "resources/gameplay/players/ai_sounds.options";

/// Which bots the host drives in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Off,
    /// PvE: `use_graphs` 0 (the script owns the patrols; with 1 the controller's re-init claims a random team graph and
    /// drops the script's) and the user data's +0x38
    Pve,
    /// PvP: `use_graphs` 1 (each controller claims a free team graph in its ctor and at each respawn)
    Pvp,
}

static PLAN: AtomicU32 = AtomicU32::new(0);

/// The exe's "harmless bots" byte (a plain global, not a cvar): the client writes it from the bot match settings'
/// `ignore_player` at Play, sets it for the shooting range and clears it in `close_current_match`. Set, the AI's vision
/// target filter 0x20032CA70 and sound filter 0x200368B50 ignore the human players (records with `ai_id == -1`). Its
/// other readers need `single_player` too, which an online match's options never set, so the host's world stays the
/// clients' world.
const IGNORE_PLAYERS: u64 = 0x20F43FE53;

/// Protocol 7 `Create.ai_ignore_players` (a custom room's "bots ignore players"): set the exe's byte before the AI
/// runs; returns the byte read back.
pub fn set_ignore_players(on: bool) -> u8 {
    unsafe {
        *(IGNORE_PLAYERS as *mut u8) = u8::from(on);
        rd8(IGNORE_PLAYERS)
    }
}

pub fn set_plan(p: Plan) {
    PLAN.store(
        match p {
            Plan::Off => 0,
            Plan::Pve => 1,
            Plan::Pvp => 2,
        },
        Ordering::SeqCst,
    );
}

pub fn plan() -> Plan {
    match PLAN.load(Ordering::SeqCst) {
        1 => Plan::Pve,
        2 => Plan::Pvp,
        _ => Plan::Off,
    }
}

/// `HOST_BOTS`: `pve`, `pvp`, or `1` / `on` = by the match kind; anything else (or unset) = off.
pub fn plan_from_env(kind_pve: bool) -> Plan {
    match std::env::var("HOST_BOTS").unwrap_or_default().as_str() {
        "pve" => Plan::Pve,
        "pvp" => Plan::Pvp,
        "1" | "on" => {
            if kind_pve {
                Plan::Pve
            } else {
                Plan::Pvp
            }
        }
        _ => Plan::Off,
    }
}

/// Called by the match creation with the user data before the `pvp_match_resource` query: in a PvE match with host bots
/// +0x38 = the controllers world (the local path's store 0x2001F2396; online it stays 0).
pub fn user_data_hook(ud: u64, game: u64) {
    if plan() == Plan::Pve {
        unsafe { *((ud + 0x38) as *mut u64) = game + CONTROLLERS };
        hlog!("bots: match user data +0x38 := {:#x} (the controllers world, PvE)", game + CONTROLLERS);
        // the scene's address, for the re-simulation gate: a breakpoint at the scene ctor's store of +0x1BB10
        crate::bp::arm(&[SCENE_STORE]);
    }
}

/// The scene ctor's store of the controllers world into scene+0x1BB10 (0x2003C761F); at this address the scene's
/// address is in the register `on_assert_trap` reads.
const SCENE_STORE: u64 = 0x2003C761F;
static SCENE: AtomicU64 = AtomicU64::new(0);
static ROLLBACK_STEPS: AtomicU64 = AtomicU64::new(0);
static RESIM_SPLITS: AtomicU64 = AtomicU64::new(0);
static RESIM_FAULTS: AtomicU64 = AtomicU64::new(0);

/// `HOST_BOTS_RESIM`: `split` (default) or `plain`.
fn resim_split() -> bool {
    std::env::var("HOST_BOTS_RESIM").map(|v| v != "plain").unwrap_or(true)
}

/// Called by `MatchWorld::step_df` after a DF's inputs went in and before `move(W, t, t)`. When the DF carried an
/// input at or before the present (`late_from`), the world will re-simulate the ticks since then. On a client the scene
/// script re-runs there without controllers (scene+0x1BB10 = 0): its `set_ai_input` (0x2003CEE2D), type-15 patrols
/// and patrol-at-spawn are no-ops. On the AI host they would insert ai inputs at past times (no DF can carry them) and
/// overwrite entries already sent (client-mode duplicate insert). So with PvE host bots the
/// re-simulated part is stepped first, to the old present, with the gate at 0, exactly as a client steps it; then the
/// gate is restored and the caller's `move` steps the new tick (whose grid end may insert at t + tick, sendable).
pub fn before_move(w: u64, late_from: Option<u32>, present: u32) {
    if plan() != Plan::Pve || late_from.is_none() {
        return;
    }
    ROLLBACK_STEPS.fetch_add(1, Ordering::SeqCst);
    let scene = SCENE.load(Ordering::SeqCst);
    if !resim_split() || scene == 0 {
        return;
    }
    unsafe {
        let gate = (scene + 0x1BB10) as *mut u64;
        let v = *gate;
        *gate = 0;
        if let Err(c) = crate::k32::call_fault_guarded3(0x2002F6610, w, present as u64, present as u64) {
            RESIM_FAULTS.fetch_add(1, Ordering::SeqCst);
            hlog!("bots: re-simulation move(W, {present}) left after a fault (code {c:#x})");
        }
        *gate = v;
    }
    RESIM_SPLITS.fetch_add(1, Ordering::SeqCst);
}

/// The player-sound resource stand-in (`game+0x10FD98`, a zeroed 0x2000-byte object) gets what the voice-line player
/// needs to return without a sound. A bot's `ai_player_input` type 2 (voice) runs, in every world, the player's handler
/// 0x2002292F0, which plays the line through the `sound_commands` resource (class 0x205) before its world part goes on.
/// That part is presentation: it needs a listener at `[res+0xB40]` (alive byte +0x74C, transform), and when its command
/// list has no such entry it returns only if the assert site's "ignore" byte 0x20F43FC7D is set (else it traps). The
/// host has no sound scene and an empty list, so +0xB40 gets a host listener object (alive, transform zero) and the
/// ignore byte is set: the call returns without a world write. Off unless host bots run (or `HOST_VOICE_STANDIN=1`: a
/// host without the AI replaying DFs that carry bot voice entries).
pub fn sound_commands_hook(standin: u64) {
    if plan() == Plan::Off && std::env::var("HOST_VOICE_STANDIN").is_err() {
        return;
    }
    unsafe {
        let listener = crate::mem::heap_alloc(0x24000, true) as u64;
        *((listener + 0x74C) as *mut u8) = 1;
        let r = crate::mem::heap_alloc(0x1000, true) as u64;
        let vt_a = crate::mem::heap_alloc(0x200, true) as u64;
        let vt_b = crate::mem::heap_alloc(0x200, true) as u64;
        for k in 0..0x40u64 {
            *((vt_a + k * 8) as *mut u64) = vc_zero as *const () as u64;
            *((vt_b + k * 8) as *mut u64) = vc_zero as *const () as u64;
        }
        *((vt_a + 0xB8) as *mut u64) = vc_listener as *const () as u64;
        *((r + 0x38) as *mut u64) = vt_a;
        *((r + 0x3A0) as *mut u64) = vt_b;
        VOICE_LISTENER.store(listener, Ordering::SeqCst);
        *((standin + 0xB40) as *mut u64) = r;
        *(0x20F43FC7Du64 as *mut u8) = 1;
        arm_assert_count();
        ASSERTS_ARMED.store(1, Ordering::SeqCst);
        hlog!("bots: sound_commands stand-in {standin:#x}: +0xB40 = host listener object {r:#x}, assert ignore byte 0x20F43FC7D = 1 (voice lines play nothing)");
    }
}

/// The exe's two assert reporters 0x2000CF670 and 0x2000CF320 (called by every assert whose site byte is 0, before the
/// debugger trap). With host bots both get a counting execution breakpoint, so an assert anywhere (AI, world, resource
/// threads) is counted with its caller in the summary: the voice site's ignore byte 0x20F43FC7D silences only that one
/// site (inside 0x2001B8450), and the voice path itself is counted by `VOICE_CALLS`.
const ASSERT_REPORTERS: [u64; 2] = [0x2000CF670, 0x2000CF320];
static ASSERT_HITS: AtomicU64 = AtomicU64::new(0);
static ASSERT_CALLERS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

pub fn arm_assert_count() {
    crate::bp::arm(&ASSERT_REPORTERS);
}

/// Called by bp.rs for every hardware breakpoint hit: true when it was an assert reporter (counted, caller kept).
pub unsafe fn on_assert_trap(rip: u64, g: &[libc::greg_t]) -> bool {
    if rip == SCENE_STORE {
        SCENE.store(g[libc::REG_RBX as usize] as u64, Ordering::SeqCst);
        return true;
    }
    if !ASSERT_REPORTERS.contains(&rip) {
        return false;
    }
    let n = ASSERT_HITS.fetch_add(1, Ordering::SeqCst) as usize;
    if n < ASSERT_CALLERS.len() {
        let ret = unsafe { *(g[libc::REG_RSP as usize] as u64 as *const u64) };
        ASSERT_CALLERS[n].store(ret, Ordering::SeqCst);
    }
    true
}

static ASSERTS_ARMED: AtomicU64 = AtomicU64::new(0);

/// The voice stand-in's line for a host without the AI (the copy replay): voice calls and assert hits.
pub fn log_voice_and_asserts() {
    if ASSERTS_ARMED.load(Ordering::SeqCst) != 0 {
        hlog!("VOICE voice-line listener calls {}; {}", VOICE_CALLS.load(Ordering::SeqCst), assert_summary());
    }
}

pub fn assert_summary() -> String {
    let n = ASSERT_HITS.load(Ordering::SeqCst);
    let c: Vec<String> = ASSERT_CALLERS.iter().take(n as usize).map(|x| format!("{:#x}", x.load(Ordering::SeqCst))).collect();
    format!("assert reporter hits {n}{}", if c.is_empty() { String::new() } else { format!(" (callers {})", c.join(" ")) })
}

static VOICE_LISTENER: AtomicU64 = AtomicU64::new(0);
pub static VOICE_CALLS: AtomicU64 = AtomicU64::new(0);

unsafe extern "win64" fn vc_zero(_a: u64, _b: u64) -> u64 {
    0
}

unsafe extern "win64" fn vc_listener(_a: u64) -> u64 {
    VOICE_CALLS.fetch_add(1, Ordering::SeqCst);
    VOICE_LISTENER.load(Ordering::SeqCst)
}

// -------------------------------------------------------------------------------------------------------------------
// skip-and-compare: the DF copies of the host's own bot entries

/// The host-driven bot slots (bit i = record i) and every entry the collection took: (slot, list 0 player / 1 ai,
/// time) -> payload. A DF that carries one of these entries (the relay filed the host's `Bots` into it, or a restarted
/// child replays the logged DFs) must not insert it again: the host world already has the node (the AI or the script
/// made it), and a second insert returns "duplicated" (client-mode overwrite), which the session
/// counts as a divergence.
static OWN: std::sync::Mutex<Option<(u32, HashMap<(u8, u8, u32), ((u64, u64), bool)>)>> = std::sync::Mutex::new(None);
/// With `Create.drive_bots` every entry the host reports comes back in a DF (the relay files it into DF `t_for`, or late
/// with its own time within 255 ms): an entry not seen again within [`ECHO_MS`] is one the clients never got.
static EXPECT_ECHO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static ECHO_MISSING: AtomicU64 = AtomicU64::new(0);
const ECHO_MS: u32 = 300;

pub fn set_expect_echo(on: bool) {
    EXPECT_ECHO.store(on, Ordering::SeqCst);
}

/// Entries older than `t - ECHO_MS` that no DF brought back (only with [`set_expect_echo`]); they leave the registry.
fn own_echo_check(t: u32) -> u64 {
    if !EXPECT_ECHO.load(Ordering::SeqCst) {
        return 0;
    }
    let lo = t.saturating_sub(ECHO_MS);
    let mut g = OWN.lock().unwrap();
    let Some((_, m)) = g.as_mut() else { return 0 };
    let missing: Vec<(u8, u8, u32)> = m.iter().filter(|(k, v)| k.2 <= lo && !v.1).map(|(k, _)| *k).collect();
    for k in &missing {
        m.remove(k);
        if ECHO_MISSING.load(Ordering::SeqCst) < 20 {
            hlog!("bots: at {t}: entry slot {} list {} time {} never came back in a DF", k.0, k.1, k.2);
        }
        ECHO_MISSING.fetch_add(1, Ordering::SeqCst);
    }
    m.retain(|k, _| k.2 > lo);
    missing.len() as u64
}
pub static OWN_MATCHED: AtomicU64 = AtomicU64::new(0);
pub static OWN_DIFFERENT: AtomicU64 = AtomicU64::new(0);
pub static OWN_UNKNOWN: AtomicU64 = AtomicU64::new(0);

fn own_put(k: (u8, u8, u32), payload: (u64, u64)) {
    if let Some((_, m)) = OWN.lock().unwrap().as_mut() {
        m.insert(k, (payload, false));
    }
}

fn own_prune(lo: u32) {
    if let Some((_, m)) = OWN.lock().unwrap().as_mut() {
        m.retain(|k, _| k.2 > lo);
    }
}

/// What `step_df` does with one DF element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DfEntry {
    /// not a host-driven bot slot (or no host bots): insert it
    Insert,
    /// a copy of an entry the host made, equal to it: skip it
    Own,
    /// a host-driven bot slot with an entry the host did not make, or with a different payload: skip it and count it
    /// as a divergence (the clients have an input the host world does not)
    Diverged,
}

/// The key of a DF element, as the collection keys nodes. A player-list element (both DF lists, 0x18 bytes): u16
/// heading +0, u16 pitch +2, u64 actions +8, u32 time +0x10, u8 player +0x14 (DF reader 0x20032ED00). An ai element
/// (0xC bytes): u16 type +0, u16 param +2, u32 time +4, u8 player +8 (reader 0x20032EF51).
pub fn df_key(list: u8, e: &[u8]) -> ((u8, u8, u32), (u64, u64)) {
    let u16_at = |o: usize| u64::from(u16::from_le_bytes([e[o], e[o + 1]]));
    let u32_at = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
    if list == 0 {
        let actions = u64::from_le_bytes(e[8..16].try_into().unwrap());
        ((e[0x14], 0, u32_at(0x10)), (u16_at(0) << 16 | u16_at(2), actions))
    } else {
        ((e[8], 1, u32_at(4)), (u16_at(0) << 16 | u16_at(2), 0))
    }
}

/// The decision for one element against a registry (`mask`, entries); pure, unit-tested.
pub fn classify(mask: u32, own: &HashMap<(u8, u8, u32), ((u64, u64), bool)>, list: u8, e: &[u8]) -> DfEntry {
    let (k, pay) = df_key(list, e);
    if k.0 >= 32 || mask >> k.0 & 1 == 0 {
        return DfEntry::Insert;
    }
    match own.get(&k) {
        Some((p, _)) if *p == pay => DfEntry::Own,
        _ => DfEntry::Diverged,
    }
}

/// Called by `step_df` for every player-list element (`list` 0, both DF lists) and ai element (`list` 1), copied to
/// `e`. Profiles are never skipped: bot profiles come from the relay (the PvP spawn), not from the host.
pub fn df_entry(list: u8, e: u64) -> DfEntry {
    let mut g = OWN.lock().unwrap();
    let Some((mask, own)) = g.as_mut() else { return DfEntry::Insert };
    let b = unsafe { std::slice::from_raw_parts(e as *const u8, if list == 0 { 0x18 } else { 0xC }) };
    let d = classify(*mask, own, list, b);
    match d {
        DfEntry::Insert => {}
        DfEntry::Own => {
            OWN_MATCHED.fetch_add(1, Ordering::SeqCst);
            if let Some(v) = own.get_mut(&df_key(list, b).0) {
                v.1 = true;
            }
        }
        DfEntry::Diverged => {
            let (k, pay) = df_key(list, b);
            let n = if own.contains_key(&k) { OWN_DIFFERENT.fetch_add(1, Ordering::SeqCst) } else { OWN_UNKNOWN.fetch_add(1, Ordering::SeqCst) };
            if n < 20 {
                hlog!("bots: DF entry of driven slot {} list {} time {} {:x?}: the host has {:x?}", k.0, k.1, k.2, pay, own.get(&k).map(|v| v.0));
            }
        }
    }
    d
}

static FUNCTOR_CALLS: AtomicU64 = AtomicU64::new(0);

unsafe extern "win64" fn functor_manager(_a: u64, _b: u64, _op: u64) -> u64 {
    0
}

/// The start's per-bot callback (the client's 0x2001F4BE0 inserts a profile input): counted, nothing else.
unsafe extern "win64" fn functor_invoke(_buf: u64, _player: u64, _time: u64) -> u64 {
    FUNCTOR_CALLS.fetch_add(1, Ordering::SeqCst);
    0
}

#[repr(C)]
struct StartArgs {
    ctl: u64,
    start_t: u64,
    core_slot: u64,
    functor: u64,
    graph_slot: u64,
    sounds_slot: u64,
    use_graphs: u64,
}

/// Host trampoline for the 7-argument start (the fault guard passes one argument).
unsafe extern "win64" fn start_tramp(a: u64, _b: u64) -> u64 {
    unsafe {
        let a = &*(a as *const StartArgs);
        let f: extern "win64" fn(u64, u32, u64, u64, u64, u64, u8) = std::mem::transmute(AI_START);
        f(a.ctl, a.start_t as u32, a.core_slot, a.functor, a.graph_slot, a.sounds_slot, a.use_graphs as u8);
    }
    0
}

/// One bot input the AI (or the script) put into the host world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotInput {
    pub slot: u8,
    pub time: u32,
    pub heading: u16,
    pub pitch: u16,
    pub actions: u64,
}

impl BotInput {
    /// The DF form (reader 0x2003276E0): u16 heading LE, u16 pitch LE, the low 56 bits of the actions big-endian.
    pub fn wire(&self) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&self.heading.to_le_bytes());
        b[2..4].copy_from_slice(&self.pitch.to_le_bytes());
        b[4..11].copy_from_slice(&self.actions.to_be_bytes()[1..8]);
        b
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotAi {
    pub slot: u8,
    pub time: u32,
    pub kind: u16,
    pub param: u16,
}

/// The bot entries found after one step: `inputs` / `ai` at exactly t + tick (the next DF), `offgrid` anything else
/// later than t (no DF can carry an ai entry off its own time; counted).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotTick {
    pub t: u32,
    pub t_for: u32,
    pub inputs: Vec<BotInput>,
    pub ai: Vec<BotAi>,
    pub offgrid_inputs: Vec<BotInput>,
    pub offgrid_ai: Vec<BotAi>,
}

#[derive(Clone, Debug)]
pub struct Counters {
    pub ticks: u64,
    pub inputs: u64,
    pub ai: u64,
    /// ai entries by type (0 wake, 1 scripted animation, 2 voice, ...)
    pub ai_by_type: HashMap<u16, u64>,
    pub offgrid: u64,
    /// a node seen again at a time already collected with a different payload (client mode overwrote it)
    pub mismatch: u64,
    /// actions with a bit above 55 set (the DF carries 56 bits)
    pub high_bits: u64,
    /// inputs per action bit (0..63)
    pub bit_counts: [u64; 64],
    pub faults: u64,
    /// per list (0 player, 1 ai): host nodes first seen at or before the stepped tick (made by a re-simulation)
    pub past_new: [u64; 2],
    /// per list: collected nodes whose payload changed later
    pub past_changed: [u64; 2],
    /// per list: collected nodes that disappeared from the list
    pub lost: [u64; 2],
    /// steps with late inputs (a rollback) / of them split (re-simulation with the scene gate at 0)
    pub rollback_steps: u64,
    pub resim_splits: u64,
}

impl Default for Counters {
    fn default() -> Self {
        Counters { ticks: 0, inputs: 0, ai: 0, ai_by_type: HashMap::new(), offgrid: 0, mismatch: 0, high_bits: 0, bit_counts: [0; 64], faults: 0, past_new: [0; 2], past_changed: [0; 2], lost: [0; 2], rollback_steps: 0, resim_splits: 0 }
    }
}

pub struct HostBots {
    pub plan: Plan,
    ctl: u64,
    w: u64,
    core_slot: u64,
    tick_ms: u32,
    pub slots: Vec<u8>,
    seen: HashMap<(u8, u8, u32), (u64, u64)>,
    pub c: Counters,
    /// per tick: AI CPU ns (ring + tick + collection)
    pub cpu: Vec<u64>,
    /// per bot slot: (alive ticks, path length m, last position)
    motion: HashMap<u8, (u64, f64, Option<[f32; 3]>)>,
    /// per bot slot: times it became alive (the first spawn included)
    spawns: HashMap<u8, u32>,
    tsv: Option<std::io::BufWriter<std::fs::File>>,
    rec: Option<RecWriter>,
    pending: Option<BotTick>,
}

fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

impl HostBots {
    /// Start the controllers of the match `core` (created, not yet stepped) for the current [`plan`]; None when off.
    /// `out` = the folder for `bots.tsv`.
    pub fn start(core: u64, out: &str) -> Result<Option<HostBots>, String> {
        let plan = plan();
        if plan == Plan::Off {
            return Ok(None);
        }
        let game = crate::run::GAME.load(Ordering::Relaxed);
        if game == 0 {
            return Err("bots: no game object".into());
        }
        unsafe {
            let nc = rd64(game + 0x10FE30);
            if nc == 0 || rd64(nc + 0x7A68D0) != core {
                return Err(format!("bots: the network client's core slot does not hold the match core {core:#x}"));
            }
            let w = core + 0x230;
            let ctl = game + CONTROLLERS;
            let opts = rd64(w + 0x180A40);
            let n = rd8(opts + 0x1080) as u64;
            let recs = rd64(opts + 8);
            let tick_ms = rd8(opts + 0x1083) as u32;
            let slots: Vec<u8> = (0..n).filter(|i| *((recs + i * 0x300 + 0x2E0) as *const i32) != -1).map(|i| i as u8).collect();
            hlog!(
                "bots: plan {plan:?}; options {opts:#x} (core+0x208 {:#x}), {n} records at {recs:#x}, tick {tick_ms}; AI records {slots:?}; pool count {} cap {} free {:#x}",
                rd64(core + 0x208),
                *((ctl + 0x38) as *const u32),
                rd64(ctl + 0x40),
                rd64(ctl + 0x30)
            );
            if slots.is_empty() {
                return Err("bots: the options have no AI record (ai_id != -1)".into());
            }
            // PvP: the per-team graph check follows the start (the graph lists exist only after it)
            let (snd, fl) = crate::run::query_one(AI_SOUNDS, crate::run::CLASS_BINARY_CONFIG, 0);
            hlog!("bots: {AI_SOUNDS} -> {snd:#x} (entry flags {fl:#x})");
            if snd == 0 {
                return Err("bots: ai_sounds.options did not load".into());
            }
            let sounds_slot = crate::mem::heap_alloc(8, true) as u64;
            *(sounds_slot as *mut u64) = snd;
            // the client's patrol-graph-data request always fails (no such resource in any volume): a slot holding 0
            let graph_slot = crate::mem::heap_alloc(8, true) as u64;
            let vt = crate::mem::heap_alloc(16, true) as u64;
            *(vt as *mut u64) = functor_manager as *const () as u64;
            *((vt + 8) as *mut u64) = functor_invoke as *const () as u64;
            let functor = crate::mem::heap_alloc(0x30, true) as u64;
            *(functor as *mut u64) = vt | 1;
            let use_graphs = u64::from(plan == Plan::Pvp);
            let start_t = *((w + 0x180AA0) as *const u32) as u64;
            let args = StartArgs { ctl, start_t, core_slot: nc + 0x7A68D0, functor, graph_slot, sounds_slot, use_graphs };
            let t0 = thread_cpu_ns();
            if let Err(c) = crate::k32::call_fault_guarded(start_tramp as *const () as u64, &args as *const StartArgs as u64) {
                return Err(format!("bots: the AI start 0x2002D6B50 left after a fault (code {c:#x})"));
            }
            let made: Vec<u8> = (0..20u64).filter(|i| rd64(ctl + 0x3DC20 + i * 8) != 0).map(|i| i as u8).collect();
            hlog!(
                "bots: AI started in {:.2} ms: use_graphs {use_graphs}, start time {start_t}, controllers in slots {made:?}, functor calls {}, pool count {}, global graph lists {}",
                (thread_cpu_ns() - t0) as f64 / 1e6,
                FUNCTOR_CALLS.load(Ordering::SeqCst),
                *((ctl + 0x38) as *const u32),
                (rd64(0x200AB9DC8) - rd64(0x200AB9DC0)) / 0x18
            );
            if made != slots {
                return Err(format!("bots: controllers {made:?} != AI records {slots:?}"));
            }
            // team graph lists: the start's loader appends one vector {begin, end, cap} per team (0, 1) to the global
            // 0x200AB9DC0 (graphs 0x158 bytes); the controller ctor takes the list of its record's team (+0x2EC). With
            // use_graphs 1 each bot claims a free graph of its team (in the ctor and at every respawn); with none free
            // the pick reads an uninitialised index: so in PvP a team must be 0 / 1 and have at least as many graphs as bots
            let lists = rd64(0x200AB9DC0);
            let graphs: Vec<u64> = (0..2u64).map(|k| (rd64(lists + k * 0x18 + 8) - rd64(lists + k * 0x18)) / 0x158).collect();
            let teams: Vec<u8> = slots.iter().map(|&i| rd8(recs + i as u64 * 0x300 + 0x2EC)).collect();
            hlog!("bots: patrol graphs per team {graphs:?}; AI record teams {teams:?}");
            if plan == Plan::Pvp {
                for team in 0..2u8 {
                    let k = teams.iter().filter(|&&x| x == team).count() as u64;
                    if k > graphs[team as usize] {
                        return Err(format!("bots: {k} bots on team {team} but {} patrol graphs (a bot without a free graph is undefined in the exe)", graphs[team as usize]));
                    }
                }
                if teams.iter().any(|&x| x > 1) {
                    return Err(format!("bots: an AI record has a team other than 0 / 1: {teams:?}"));
                }
            }
            if plan == Plan::Pve {
                let scene = SCENE.load(Ordering::SeqCst);
                let gate = if scene != 0 { rd64(scene + 0x1BB10) } else { 0 };
                hlog!("bots: scene {scene:#x}, scene+0x1BB10 = {gate:#x} (controllers {ctl:#x}); re-simulation mode {}", if resim_split() { "split" } else { "plain" });
                if gate != ctl {
                    return Err(format!("bots: scene+0x1BB10 = {gate:#x}, not the controllers world {ctl:#x}"));
                }
            }
            let tsv = std::fs::File::create(format!("{out}/bots.tsv")).ok().map(|f| {
                let mut b = std::io::BufWriter::new(f);
                let _ = writeln!(b, "t_step\tt\tslot\tkind\tv1\tv2\tv3");
                b
            });
            let rec = match std::env::var("HOST_BOTS_REC") {
                Ok(p) => Some(RecWriter::create(&p)?),
                Err(_) => None,
            };
            let mask = slots.iter().fold(0u32, |m, &i| m | 1 << i);
            *OWN.lock().unwrap() = Some((mask, HashMap::new()));
            Ok(Some(HostBots {
                plan,
                ctl,
                w,
                core_slot: nc + 0x7A68D0,
                tick_ms,
                slots,
                seen: HashMap::new(),
                c: Counters::default(),
                cpu: Vec::new(),
                motion: HashMap::new(),
                spawns: HashMap::new(),
                tsv,
                rec,
                pending: None,
            }))
        }
    }

    /// After DF `t` was stepped (and its E0 / captures taken): run the AI tick and collect the host-made bot entries.
    pub fn after_step(&mut self, t: u32) -> BotTick {
        let c0 = thread_cpu_ns();
        if let Err(c) = crate::k32::call_fault_guarded(AI_MOTION_RING, self.w) {
            self.c.faults += 1;
            hlog!("bots: 0x20036DB20(W) at {t} left after a fault (code {c:#x})");
        }
        if let Err(c) = crate::k32::call_fault_guarded3(AI_TICK, self.ctl, self.core_slot, t as u64) {
            self.c.faults += 1;
            hlog!("bots: the AI tick 0x2002D7220 at {t} left after a fault (code {c:#x})");
        }
        let bt = self.collect(t);
        own_echo_check(t);
        self.cpu.push(thread_cpu_ns() - c0);
        self.track_motion();
        self.c.ticks += 1;
        if let Some(w) = self.tsv.as_mut() {
            for x in bt.inputs.iter().chain(&bt.offgrid_inputs) {
                let _ = writeln!(w, "{t}\t{}\t{}\tin\t{:04x}\t{:04x}\t{:016x}", x.time, x.slot, x.heading, x.pitch, x.actions);
            }
            for x in bt.ai.iter().chain(&bt.offgrid_ai) {
                let _ = writeln!(w, "{t}\t{}\t{}\tai\t{}\t{}\t-", x.time, x.slot, x.kind, x.param);
            }
        }
        if self.rec.is_some() {
            self.pending = Some(bt.clone());
        }
        bt
    }

    fn collect(&mut self, t: u32) -> BotTick {
        let t_for = t + self.tick_ms;
        let mut bt = BotTick { t, t_for, ..Default::default() };
        // the walk goes back WINDOW ms: nodes at or before t that were never collected (made during a re-simulated
        // step: no DF can carry them any more), payload changes of collected nodes, and collected nodes that are gone
        let lo = t.saturating_sub(WINDOW);
        let mut walked: std::collections::HashSet<(u8, u8, u32)> = std::collections::HashSet::new();
        unsafe {
            let pl = rd64(self.w + 0x7DF0);
            let al = rd64(self.w + 0xFFBD8);
            for i in self.slots.clone() {
                let mut n = rd64(pl + i as u64 * 0x4218 + 0x10);
                let mut got = Vec::new();
                while n != 0 && *((n + 0x148) as *const u32) > lo {
                    let time = *((n + 0x148) as *const u32);
                    got.push(BotInput { slot: i, time, heading: *((n + 0x10) as *const u16), pitch: *((n + 0x12) as *const u16), actions: rd64(n + 0x18) });
                    n = rd64(n + 8);
                }
                for x in got.into_iter().rev() {
                    walked.insert((i, 0, x.time));
                    let pay = (u64::from(x.heading) << 16 | u64::from(x.pitch), x.actions);
                    if x.time <= t {
                        self.past((i, 0, x.time), pay, t);
                        continue;
                    }
                    if !self.fresh((i, 0, x.time), pay) {
                        continue;
                    }
                    if x.actions >> 56 != 0 {
                        self.c.high_bits += 1;
                    }
                    for b in 0..64 {
                        if x.actions >> b & 1 != 0 {
                            self.c.bit_counts[b] += 1;
                        }
                    }
                    if x.time == t_for {
                        self.c.inputs += 1;
                        bt.inputs.push(x);
                    } else {
                        self.c.offgrid += 1;
                        bt.offgrid_inputs.push(x);
                    }
                }
                let mut n = rd64(al + i as u64 * 0x4218 + 0x10);
                let mut got = Vec::new();
                while n != 0 && *((n + 0x148) as *const u32) > lo {
                    got.push(BotAi { slot: i, time: *((n + 0x148) as *const u32), kind: *((n + 0x10) as *const u16), param: *((n + 0x12) as *const u16) });
                    n = rd64(n + 8);
                }
                for x in got.into_iter().rev() {
                    walked.insert((i, 1, x.time));
                    let pay = (u64::from(x.kind) << 16 | u64::from(x.param), 0);
                    if x.time <= t {
                        self.past((i, 1, x.time), pay, t);
                        continue;
                    }
                    if !self.fresh((i, 1, x.time), pay) {
                        continue;
                    }
                    *self.c.ai_by_type.entry(x.kind).or_default() += 1;
                    if x.time == t_for {
                        self.c.ai += 1;
                        bt.ai.push(x);
                    } else {
                        self.c.offgrid += 1;
                        bt.offgrid_ai.push(x);
                    }
                }
            }
        }
        // collected nodes inside the window that the lists no longer hold
        let gone: Vec<(u8, u8, u32)> = self.seen.keys().filter(|k| k.2 > lo && k.2 <= t && !walked.contains(k)).copied().collect();
        for k in gone {
            self.c.lost[k.1 as usize] += 1;
            self.seen.remove(&k);
            if self.c.lost[0] + self.c.lost[1] <= 20 {
                hlog!("bots: at {t}: collected node slot {} list {} time {} is gone", k.0, k.1, k.2);
            }
        }
        if t % 10_000 == 0 {
            self.seen.retain(|k, _| k.2 > lo);
            own_prune(lo);
        }
        bt
    }

    /// A node at or before t: never collected = made in the past (re-simulation), a different payload = overwritten.
    fn past(&mut self, k: (u8, u8, u32), payload: (u64, u64), t: u32) {
        match self.seen.get(&k) {
            Some(p) if *p == payload => {}
            Some(_) => {
                self.c.mismatch += 1;
                self.c.past_changed[k.1 as usize] += 1;
                if self.c.mismatch <= 20 {
                    hlog!("bots: at {t}: slot {} list {} time {}: payload changed after it was collected ({:x?})", k.0, k.1, k.2, payload);
                }
                self.seen.insert(k, payload);
                own_put(k, payload);
            }
            None => {
                self.c.past_new[k.1 as usize] += 1;
                if self.c.past_new[0] + self.c.past_new[1] <= 20 {
                    hlog!("bots: at {t}: slot {} list {} time {}: a node made at or before the present ({:x?})", k.0, k.1, k.2, payload);
                }
                self.seen.insert(k, payload);
            }
        }
    }

    /// true for a node not collected before; a known (slot, list, time) with another payload counts as a mismatch.
    fn fresh(&mut self, k: (u8, u8, u32), payload: (u64, u64)) -> bool {
        match self.seen.insert(k, payload) {
            None => {
                own_put(k, payload);
                true
            }
            Some(p) if p == payload => false,
            Some(_) => {
                self.c.mismatch += 1;
                hlog!("bots: slot {} list {} time {}: payload changed after it was collected", k.0, k.1, k.2);
                false
            }
        }
    }

    /// Path length of every bot while alive: the player's matrix at player+0x1298, translation +0x30 (what the AI's own
    /// ring copies); player = `[e + [e+0x48]*8]`, `e = [W+0x17C2A0] + i*0xB8`, alive byte player+0x74C.
    fn track_motion(&mut self) {
        unsafe {
            let ents = rd64(self.w + 0x17C2A0);
            if ents == 0 {
                return;
            }
            for i in self.slots.clone() {
                let e = ents + i as u64 * 0xB8;
                let p = rd64(e + rd8(e + 0x48) as u64 * 8);
                let m = self.motion.entry(i).or_insert((0, 0.0, None));
                if p == 0 || rd8(p + 0x74C) == 0 {
                    m.2 = None;
                    continue;
                }
                let pos = [*((p + 0x12C8) as *const f32), *((p + 0x12CC) as *const f32), *((p + 0x12D0) as *const f32)];
                m.0 += 1;
                if m.2.is_none() {
                    *self.spawns.entry(i).or_default() += 1;
                }
                if let Some(q) = m.2 {
                    let d = ((pos[0] - q[0]).powi(2) + (pos[1] - q[1]).powi(2) + (pos[2] - q[2]).powi(2)).sqrt() as f64;
                    // a respawn jumps: count walking only
                    if d < 5.0 {
                        m.1 += d;
                    }
                }
                m.2 = Some(pos);
            }
        }
    }

    /// Before DF `t` is stepped: the recording copy gets this DF with the bot entries collected for time `t` merged in.
    pub fn on_df(&mut self, t: u32, df: &[u8]) {
        let Some(rec) = self.rec.as_mut() else { return };
        let mut extra = BotTick::default();
        if let Some(p) = self.pending.take() {
            if p.t_for == t {
                extra = p;
            } else if !p.inputs.is_empty() || !p.ai.is_empty() {
                rec.unsent += (p.inputs.len() + p.ai.len()) as u64;
                hlog!("bots: {} entries for {} have no DF (next DF {t})", p.inputs.len() + p.ai.len(), p.t_for);
            }
        }
        rec.unsent += (extra.offgrid_inputs.len() + extra.offgrid_ai.len()) as u64;
        match merge_df(df, &extra.inputs, &extra.ai) {
            Ok(b) => {
                rec.merged += (extra.inputs.len() + extra.ai.len()) as u64;
                rec.df(t, &b)
            }
            Err(e) => {
                rec.conflicts += 1;
                hlog!("bots: DF {t}: {e}; the DF goes into the copy unchanged");
                rec.df(t, df);
            }
        }
    }

    pub fn rec_header(&mut self, head: &[u8]) {
        if let Some(r) = self.rec.as_mut() {
            r.raw(head);
        }
    }

    pub fn rec_tail(&mut self, tail: &[u8]) {
        if let Some(r) = self.rec.as_mut() {
            r.raw(tail);
            r.finish();
        }
    }

    /// The summary lines (counters, AI CPU, per-bot motion and statistics).
    pub fn report(&self, results: Option<&crate::world::Results>) {
        let c = &self.c;
        let mut types: Vec<_> = c.ai_by_type.iter().collect();
        types.sort();
        hlog!(
            "BOTS {} ticks: inputs {}, ai {} (by type {:?}), offgrid {}, mismatch {}, high bits {}, faults {}",
            c.ticks, c.inputs, c.ai, types, c.offgrid, c.mismatch, c.high_bits, c.faults
        );
        hlog!(
            "BOTS rollbacks: steps with late inputs {}, split re-simulations {} (faults {}); past-made nodes player {} ai {}; changed after collection player {} ai {}; lost player {} ai {}",
            ROLLBACK_STEPS.load(Ordering::SeqCst),
            RESIM_SPLITS.load(Ordering::SeqCst),
            RESIM_FAULTS.load(Ordering::SeqCst),
            c.past_new[0],
            c.past_new[1],
            c.past_changed[0],
            c.past_changed[1],
            c.lost[0],
            c.lost[1]
        );
        let bits: Vec<String> = (0..64).filter(|&b| c.bit_counts[b] != 0).map(|b| format!("{b}:{}", c.bit_counts[b])).collect();
        hlog!("BOTS inputs per action bit: {}; voice-line listener calls {}; {}", bits.join(" "), VOICE_CALLS.load(Ordering::SeqCst), assert_summary());
        if !self.cpu.is_empty() {
            let mut x = self.cpu.clone();
            x.sort_unstable();
            let n = x.len();
            let mean = x.iter().sum::<u64>() as f64 / n as f64;
            let q = |f: f64| x[((n as f64 * f) as usize).min(n - 1)] as f64 / 1e6;
            hlog!("BOTSTIME AI CPU per tick (ring + tick + collection): mean {:.3} ms, p50 {:.3}, p99 {:.3}, max {:.3} ms over {n} ticks", mean / 1e6, q(0.5), q(0.99), x[n - 1] as f64 / 1e6);
        }
        let mut s: Vec<_> = self.motion.iter().collect();
        s.sort_by_key(|x| *x.0);
        let mv: Vec<String> = s.iter().map(|(i, m)| format!("{i}:{}t/{:.0}m", m.0, m.1)).collect();
        hlog!("BOTS alive ticks / path length per bot: {}", mv.join(" "));
        let mut sp: Vec<_> = self.spawns.iter().collect();
        sp.sort();
        hlog!("BOTS spawns per bot (first spawn included): {}", sp.iter().map(|(i, n)| format!("{i}:{n}")).collect::<Vec<_>>().join(" "));
        if let Some(r) = results {
            let k: Vec<String> = r.players.iter().map(|p| format!("{}:{}/{}", p.slot, p.kills, p.deaths)).collect();
            hlog!("BOTS kills/deaths per slot (statistics): {}", k.join(" "));
        }
        hlog!(
            "BOTS DF copies of own bot entries: matched and skipped {}, different {}, never made {}; reported entries no DF brought back {}",
            OWN_MATCHED.load(Ordering::SeqCst),
            OWN_DIFFERENT.load(Ordering::SeqCst),
            OWN_UNKNOWN.load(Ordering::SeqCst),
            ECHO_MISSING.load(Ordering::SeqCst)
        );
        if let Some(r) = self.rec.as_ref() {
            hlog!("BOTS recording copy: {} DFs written, {} with bot entries, {} entries merged, {} unsent, {} conflicts", r.dfs, r.dfs_with, r.merged, r.unsent, r.conflicts);
        }
    }

    /// Everything that makes the host world differ from the clients' (each needs `Diverged`): entries off the grid,
    /// made in a re-simulation, changed or lost after collection, never brought back by a DF, and DF bot entries the
    /// host did not make or made differently.
    pub fn divergence(&self) -> u64 {
        let c = &self.c;
        c.offgrid
            + c.mismatch
            + c.past_new[0]
            + c.past_new[1]
            + c.lost[0]
            + c.lost[1]
            + ECHO_MISSING.load(Ordering::SeqCst)
            + OWN_DIFFERENT.load(Ordering::SeqCst)
            + OWN_UNKNOWN.load(Ordering::SeqCst)
    }

    pub fn flush(&mut self) {
        if let Some(w) = self.tsv.as_mut() {
            let _ = w.flush();
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// offline test: a synthetic PvP match

/// `HOST_BOTS_SYNTH=FILE` with `--match MAP,MODE` (step 3): write a SRPREC01 recording of a PvP match with bots on
/// both teams that no human played, then exit. The replay of that file (with `HOST_BOTS=pvp`) is the offline bots test.
/// * Options: the Training writer (`local_match::training_options`, 1 human + `HOST_BOTS_SYNTH_PLAYERS - 1` bots, teams
///   alternating from team 1; the records the client accepts in local Training) with the three header bytes that
///   differ from an online match patched: max delay +2 (0 -> `max_delay_ms(tick)`), single_player +11 (1 -> 0), match
///   type +13 (-> plain). Training's ammo storage (the human's ammo counts raised) stays; the human never moves.
/// * World: the host's own start world (`MatchWorld::create_own`, protocol 4), written as the DE.
/// * DFs every tick for `HOST_BOTS_SYNTH_SECONDS` (default 300): every record enters at present + 300 ms, and gets a
///   profile input at present + 350 ms with the bytes the local start's functor gives each AI player (0x2001F4BEA:
///   profile 0, then 1, 1, 0, no entries; wire `00 01 01 00 00`). The human gets the same profile (he spawns and
///   stands). Nothing else: the bots' inputs come from the host's AI in the replay.
pub fn synth_pvp(map: u32, mode: u32, out: &str) -> Result<(), String> {
    let gd = crate::run::game_tables()?;
    let v = (0..16u32).find(|&v| gd.matches.resolve_map(map, mode, v).is_some_and(|r| v == 0 || r.variation == v)).ok_or("no mode row")?;
    let n: usize = std::env::var("HOST_BOTS_SYNTH_PLAYERS").ok().and_then(|x| x.parse().ok()).unwrap_or(12).clamp(2, 20);
    let secs: u32 = std::env::var("HOST_BOTS_SYNTH_SECONDS").ok().and_then(|x| x.parse().ok()).unwrap_or(300);
    let level: u32 = std::env::var("HOST_BOTS_SYNTH_LEVEL").ok().and_then(|x| x.parse().ok()).unwrap_or(game_data::matches::DEFAULT_BOT_LEVEL);
    let match_id = 1_795_500_001u32;
    let tick = local_match::options::TICK_MS as u8;
    let gun = mode == game_data::gunrace::MODE;
    let blob = if gun {
        // the gun race (docs/notes/gun-race.md): the production writer (`local_match::gunrace_options`), n placeholder humans
        // (2..=8, each its own team, the nine race kits of HOST_BOTS_SYNTH_LEVEL), no AI records (arena.lua spawns humans
        // only); they enter and stand: the script's time end (10 minutes after the start) ends the match
        let n = n.clamp(2, game_data::gunrace::MAX_PLAYERS);
        let r = &gd.gunrace.row;
        let params = local_match::OnlineParams {
            map: r.map,
            mode: r.mode,
            weather: r.variation,
            match_id,
            tick_ms: tick,
            send_period_ms: local_match::options::SEND_PERIOD_MS,
            match_type: local_match::options::MATCH_TYPE_PLAIN,
            max_delay_ms: local_match::options::max_delay_ms(tick),
        };
        let humans: Vec<local_match::OnlinePlayer> = (0..n)
            .map(|i| {
                let mut h = local_match::HumanLoadout::default_loadout();
                h.name = format!("racer{i}");
                local_match::OnlinePlayer { team: 0, human: h, hero: None, pve_budget: (0, 0) }
            })
            .collect();
        // HOST_GUNRACE_LEVELS=a,b,..: a test aid, the records' profiles are those race levels' kits in that order
        // (repeats allowed; written by the generic online writer, +0x1081 = 1; arena.lua needs nine profiles)
        let blob = match std::env::var("HOST_GUNRACE_LEVELS") {
            Ok(list) => {
                let keep: Vec<usize> = list.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let mut race = local_match::gunrace::race_players(&gd, &humans, level, match_id).map_err(|e| format!("gun race players: {e}"))?;
                for p in &mut race {
                    let all = std::mem::take(&mut p.human.profiles);
                    p.human.profiles = keep.iter().filter_map(|&l| all.get(l).cloned()).enumerate().map(|(i, mut q)| { q.id = i as u32 + 1; q }).collect();
                }
                hlog!("synth: gun race bisect: race levels {keep:?} only");
                local_match::online_options_row(&gd, &params, r, &race, &[]).map_err(|e| format!("options: {e}"))?
            }
            Err(_) => local_match::gunrace_options(&gd, &params, &humans, level).map_err(|e| format!("gun race options: {e}"))?.0,
        };
        for i in 0..n {
            hlog!("synth: gun race record {i}: kits {:?}", local_match::gunrace::kit_names(&gd, level, match_id, i));
        }
        hlog!("synth: gun race options {} bytes, {n} humans, equipment level {level}", blob.len());
        blob
    } else if std::env::var("HOST_BOTS_SYNTH_ONLINE").is_ok_and(|x| x == "1") {
        // bots: the production writer (`online_options_bots`): 1 human on team 0, AI players up to n / 2 per team, picked
        // by `pick_online_bots` at `level`; HOST_BOTS_SYNTH_WEAPON=k:weapon:ammo gives bot k another weapon1 / ammo 1
        // HOST_BOTS_SYNTH_HUMANS=k (default 1): k idle humans, teams alternating from team 0 (k = n: no AI player)
        let k: usize = std::env::var("HOST_BOTS_SYNTH_HUMANS").ok().and_then(|x| x.parse().ok()).unwrap_or(1).clamp(1, n);
        let half = n / 2;
        let (h0, h1) = (k.div_ceil(2), k / 2);
        let mut bots = local_match::pick_online_bots(&gd, [half.saturating_sub(h0), (n - half).saturating_sub(h1)], level);
        if let Ok(w) = std::env::var("HOST_BOTS_SYNTH_WEAPON") {
            let p: Vec<&str> = w.split(':').collect();
            let (Some(k), Some(wpn)) = (p.first().and_then(|x| x.parse::<usize>().ok()), p.get(1)) else { return Err(format!("HOST_BOTS_SYNTH_WEAPON={w}: use k:weapon[:ammo]")) };
            let b = bots.get_mut(k).ok_or("HOST_BOTS_SYNTH_WEAPON: no such bot")?;
            let mut prof = b.profile(&gd).ok_or("bot without a kit")?.with_item("weapon1_slot", wpn);
            if let Some(a) = p.get(2) {
                prof = prof.with_item("ammo_slot_1", a);
            }
            let kit = gd.matches.kit(&gd.items, &prof);
            hlog!("synth: bot {k} (ai {}) gets weapon {wpn} / ammo {:?}: kit items {:?}", b.ai, p.get(2), kit.items);
            b.kit = Some(prof);
        }
        let params = local_match::OnlineParams {
            map,
            mode,
            weather: v,
            match_id,
            tick_ms: tick,
            send_period_ms: local_match::options::SEND_PERIOD_MS,
            match_type: local_match::options::MATCH_TYPE_PLAIN,
            max_delay_ms: local_match::options::max_delay_ms(tick),
        };
        let humans: Vec<local_match::OnlinePlayer> =
            (0..k).map(|i| local_match::OnlinePlayer { team: (i % 2) as u8, human: local_match::HumanLoadout::default_loadout(), hero: None, pve_budget: (0, 0) }).collect();
        let blob = local_match::online_options_bots(&gd, &params, &humans, &bots).map_err(|e| format!("online options: {e}"))?;
        hlog!("synth: online options {} bytes, {k} human(s) + {} AI players, level {level}: (team, ai) {:?}", blob.len(), bots.len(), bots.iter().map(|b| (b.team, b.ai)).collect::<Vec<_>>());
        blob
    } else {
        let tp = local_match::options::TrainingParams { map, mode, weather: v, n_players: Some(n), bot_level: level, match_id };
        let (mut blob, roster) = local_match::options::training_options(&gd, &tp, &local_match::HumanLoadout::default_loadout()).map_err(|e| format!("training options: {e}"))?;
        blob[2..4].copy_from_slice(&local_match::options::max_delay_ms(tick).to_le_bytes());
        blob[11] = 0;
        blob[13] = local_match::options::MATCH_TYPE_PLAIN;
        hlog!("synth: options {} bytes, {n} records (1 human + {} bots, level {level}): {:?}", blob.len(), roster.bots.len(), roster.bots.iter().map(|b| (b.team, b.ai)).collect::<Vec<_>>());
        blob
    };
    // the host's own start world (protocol 4, world.rs `create_own`: the first item loaded back as a DE of time
    // >= 16 ticks, which clears the frozen flag), written as the recording's DE
    let (mw, de) = crate::world::MatchWorld::create_own(&blob, false)?;
    let p0 = mw.present();
    let it = u32::from_le_bytes(de.get(..4).ok_or("empty start item")?.try_into().unwrap());
    let (records, n_rec) = unsafe {
        let opts = rd64(mw.core + 0x230 + 0x180A40);
        let recs = rd64(opts + 8);
        let n = rd8(opts + 0x1080) as u64;
        ((0..n).map(|i| (*((recs + i * 0x300 + 0x2E0) as *const i32) != -1, rd8(recs + i * 0x300 + 0x2EC))).collect::<Vec<(bool, u8)>>(), n as u32)
    };
    hlog!("synth: world present {p0}, first item {it} ({} bytes); records (bot, team) {records:?}", de.len());
    let mut f = Vec::new();
    f.extend_from_slice(b"SRPREC01");
    f.extend_from_slice(&match_id.to_le_bytes());
    f.extend_from_slice(&it.to_le_bytes());
    f.push(tick);
    f.push(records.len() as u8);
    for (b, t) in &records {
        f.push(u8::from(*b));
        f.push(*t);
    }
    f.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    f.extend_from_slice(&blob);
    f.extend_from_slice(&(de.len() as u32).to_le_bytes());
    f.extend_from_slice(&de);
    let all: u32 = if n_rec >= 32 { u32::MAX } else { (1u32 << n_rec) - 1 };
    let (t_enter, t_prof) = (it + 300, it + 350);
    let mut t = it + tick as u32;
    let end = it + secs * 1000;
    while t <= end {
        let mut d = Vec::new();
        d.extend_from_slice(&t.to_le_bytes());
        d.push(0xFF);
        d.extend_from_slice(&(if t == t_enter { all } else { 0 }).to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        // the gun race: no profile input (arena.lua spawns every entered human with its race profile itself)
        let np = if t == t_prof && !gun { n_rec as u8 } else { 0 };
        d.extend_from_slice(&[0, np, 0, 0, 0]);
        d.extend_from_slice(&0u32.to_le_bytes());
        for k in 0..np {
            d.extend_from_slice(&[k, 0, 1, 1, 0, 0]);
        }
        f.push(1);
        f.extend_from_slice(&t.to_le_bytes());
        f.extend_from_slice(&(d.len() as u32).to_le_bytes());
        f.extend_from_slice(&d);
        t += tick as u32;
    }
    f.push(0xFF);
    f.extend_from_slice(&(end + tick as u32).to_le_bytes());
    std::fs::write(out, &f).map_err(|e| format!("{out}: {e}"))?;
    hlog!("synth: wrote {out}: {} bytes, DFs {}..{end}, enter {t_enter}, profiles {t_prof}", f.len(), it + tick as u32);
    Ok(())
}

/// The protocol-5 `Bots` frame of one step: the entries for DF `t_for`, and how many are off the grid.
pub fn to_proto(bt: &BotTick) -> world_host_proto::bots::Bots {
    world_host_proto::bots::Bots {
        t_for: bt.t_for,
        inputs: bt.inputs.iter().map(|x| (x.slot, x.wire())).collect(),
        ai: bt.ai.iter().map(|x| (x.slot, x.kind, x.param)).collect(),
        offgrid: (bt.offgrid_inputs.len() + bt.offgrid_ai.len()) as u32,
    }
}

// -------------------------------------------------------------------------------------------------------------------
// the recording copy (SRPREC01: header, then records `u8 kind, u32 t, u32 len, DF bytes`; kind 0xFF = end)

struct RecWriter {
    out: std::io::BufWriter<std::fs::File>,
    dfs: u64,
    dfs_with: u64,
    merged: u64,
    unsent: u64,
    conflicts: u64,
    last_merged: u64,
}

impl RecWriter {
    fn create(p: &str) -> Result<RecWriter, String> {
        let f = std::fs::File::create(p).map_err(|e| format!("{p}: {e}"))?;
        Ok(RecWriter { out: std::io::BufWriter::new(f), dfs: 0, dfs_with: 0, merged: 0, unsent: 0, conflicts: 0, last_merged: 0 })
    }
    fn raw(&mut self, b: &[u8]) {
        let _ = self.out.write_all(b);
    }
    fn df(&mut self, t: u32, b: &[u8]) {
        let _ = self.out.write_all(&[1u8]);
        let _ = self.out.write_all(&t.to_le_bytes());
        let _ = self.out.write_all(&(b.len() as u32).to_le_bytes());
        let _ = self.out.write_all(b);
        self.dfs += 1;
        if self.merged != self.last_merged {
            self.dfs_with += 1;
            self.last_merged = self.merged;
        }
    }
    fn finish(&mut self) {
        let _ = self.out.flush();
    }
}

// -------------------------------------------------------------------------------------------------------------------
// DF codec (the layout of the exe's reader 0x20032E790, as `match_msgs::GameUpdate` in the relay)

/// One DF: `u32 time, u8 code, u32 enter, u32 leave, u8 n_late, n_profile, n_ping, n_ai, n_conf, n_conf x {u32, u32},
/// n_late x {u8 dt, u8 player}, u32 input_mask, (n_late + popcount) x 11-byte input, n_profile x {u8, 0x?? bytes},
/// n_ping x {u8, u16}, n_ai x {u8, u16, u16}`. The list-2 order is input_mask bit 0..19 (reader 0x20032ED51).
/// Profiles are kept as raw bytes (`profile_raw`): their size is what remains before the pings and ai entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Df {
    pub head: [u8; 13],
    pub conf: Vec<u8>,
    pub late: Vec<(u8, u8)>,
    pub input_mask: u32,
    pub late_inputs: Vec<[u8; 11]>,
    pub inputs: Vec<[u8; 11]>,
    pub n_profile: u8,
    pub profile_raw: Vec<u8>,
    pub ping: Vec<(u8, u16)>,
    pub ai: Vec<(u8, u16, u16)>,
}

impl Df {
    pub fn decode(b: &[u8]) -> Result<Df, String> {
        let cut = || "DF cut".to_string();
        let g = |p: usize, n: usize| b.get(p..p + n).ok_or_else(cut);
        let head: [u8; 13] = g(0, 13)?.try_into().unwrap();
        let n = g(13, 5)?;
        let (nl, np, ng, na, nc) = (n[0], n[1], n[2], n[3], n[4]);
        let mut p = 18;
        let conf = g(p, nc as usize * 8)?.to_vec();
        p += nc as usize * 8;
        let late: Vec<(u8, u8)> = g(p, nl as usize * 2)?.chunks(2).map(|c| (c[0], c[1])).collect();
        p += nl as usize * 2;
        let input_mask = u32::from_le_bytes(g(p, 4)?.try_into().unwrap());
        p += 4;
        let mut late_inputs = Vec::new();
        for _ in 0..nl {
            late_inputs.push(g(p, 11)?.try_into().unwrap());
            p += 11;
        }
        let mut inputs = Vec::new();
        for _ in 0..input_mask.count_ones() {
            inputs.push(g(p, 11)?.try_into().unwrap());
            p += 11;
        }
        let tail_fixed = ng as usize * 3 + na as usize * 5;
        if b.len() < p + tail_fixed {
            return Err(cut());
        }
        let prof_end = b.len() - tail_fixed;
        let profile_raw = b[p..prof_end].to_vec();
        p = prof_end;
        let mut ping = Vec::new();
        for _ in 0..ng {
            ping.push((b[p], u16::from_le_bytes([b[p + 1], b[p + 2]])));
            p += 3;
        }
        let mut ai = Vec::new();
        for _ in 0..na {
            ai.push((b[p], u16::from_le_bytes([b[p + 1], b[p + 2]]), u16::from_le_bytes([b[p + 3], b[p + 4]])));
            p += 5;
        }
        Ok(Df { head, conf, late, input_mask, late_inputs, inputs, n_profile: np, profile_raw, ping, ai })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut o = self.head.to_vec();
        o.extend_from_slice(&[self.late.len() as u8, self.n_profile, self.ping.len() as u8, self.ai.len() as u8, (self.conf.len() / 8) as u8]);
        o.extend_from_slice(&self.conf);
        for (dt, pl) in &self.late {
            o.extend_from_slice(&[*dt, *pl]);
        }
        o.extend_from_slice(&self.input_mask.to_le_bytes());
        for i in self.late_inputs.iter().chain(&self.inputs) {
            o.extend_from_slice(i);
        }
        o.extend_from_slice(&self.profile_raw);
        for (pl, v) in &self.ping {
            o.push(*pl);
            o.extend_from_slice(&v.to_le_bytes());
        }
        for (pl, a, b) in &self.ai {
            o.push(*pl);
            o.extend_from_slice(&a.to_le_bytes());
            o.extend_from_slice(&b.to_le_bytes());
        }
        o
    }

    pub fn time(&self) -> u32 {
        u32::from_le_bytes(self.head[0..4].try_into().unwrap())
    }
}

/// The DF with the bot inputs (all at the DF's time) and ai entries added, as a relay would file them: an input sets
/// its player's input_mask bit (refused when the bit is already set: one input per player per DF time); the ai entries
/// are appended (at most 20 per DF, the client's buffer). A DF over 0x1000 bytes is refused
/// (the client copies at most 0x1000 bytes, 0x2005BB7B0).
pub fn merge_df(df: &[u8], inputs: &[BotInput], ai: &[BotAi]) -> Result<Vec<u8>, String> {
    if inputs.is_empty() && ai.is_empty() {
        return Ok(df.to_vec());
    }
    let mut d = Df::decode(df)?;
    if d.encode() != df {
        return Err("the DF does not re-encode to its own bytes".into());
    }
    let t = d.time();
    let mut by_bit: Vec<(u8, [u8; 11])> = Vec::new();
    let mut k = 0;
    for bit in 0..20u8 {
        if d.input_mask >> bit & 1 != 0 {
            by_bit.push((bit, d.inputs[k]));
            k += 1;
        }
    }
    for x in inputs {
        if x.time != t {
            return Err(format!("bot input of slot {} at {} in DF {t}", x.slot, x.time));
        }
        if x.slot >= 20 || d.input_mask >> x.slot & 1 != 0 {
            return Err(format!("slot {} already has an input in DF {t}", x.slot));
        }
        d.input_mask |= 1 << x.slot;
        by_bit.push((x.slot, x.wire()));
    }
    by_bit.sort_by_key(|e| e.0);
    d.inputs = by_bit.into_iter().map(|e| e.1).collect();
    for x in ai {
        if x.time != t {
            return Err(format!("bot ai entry of slot {} at {} in DF {t}", x.slot, x.time));
        }
        d.ai.push((x.slot, x.kind, x.param));
    }
    if d.ai.len() > 20 {
        return Err(format!("{} ai entries in DF {t} (client buffer 20)", d.ai.len()));
    }
    let out = d.encode();
    if out.len() > 0x1000 {
        return Err(format!("DF {t} would be {} bytes (client copy limit 0x1000)", out.len()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        // time 1100, code FF, enter bits 0..17, leave 0, 1 late, 1 profile (3 raw bytes), 1 ping, 1 ai, 1 conf
        let mut b = Vec::new();
        b.extend_from_slice(&1100u32.to_le_bytes());
        b.push(0xFF);
        b.extend_from_slice(&0x3FFFFu32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&[1, 1, 1, 1, 1]);
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(&9u32.to_le_bytes());
        b.extend_from_slice(&[50, 0]);
        b.extend_from_slice(&0b101u32.to_le_bytes());
        for k in 0..3u8 {
            b.extend_from_slice(&[k; 11]);
        }
        b.extend_from_slice(&[0, 0xAA, 0xBB]);
        b.extend_from_slice(&[0, 0x34, 0x12]);
        b.extend_from_slice(&[2, 1, 0, 3, 0]);
        b
    }

    #[test]
    fn own_bot_entries_are_skipped_and_compared() {
        let mut own = HashMap::new();
        // slot 3: a player input at 1150 (heading 0x0102, pitch 0xFFFE, actions 5) and an ai entry {1, 7} at 1150
        own.insert((3u8, 0u8, 1150u32), ((0x0102u64 << 16 | 0xFFFE, 5u64), false));
        own.insert((3u8, 1u8, 1150u32), ((1u64 << 16 | 7, 0u64), false));
        let mask = 1 << 3;
        let mut pe = [0u8; 0x18];
        pe[0..2].copy_from_slice(&0x0102u16.to_le_bytes());
        pe[2..4].copy_from_slice(&0xFFFEu16.to_le_bytes());
        pe[8..16].copy_from_slice(&5u64.to_le_bytes());
        pe[0x10..0x14].copy_from_slice(&1150u32.to_le_bytes());
        pe[0x14] = 3;
        assert_eq!(classify(mask, &own, 0, &pe), DfEntry::Own);
        let mut other = pe;
        other[8] = 6;
        assert_eq!(classify(mask, &own, 0, &other), DfEntry::Diverged);
        let mut later = pe;
        later[0x10..0x14].copy_from_slice(&1200u32.to_le_bytes());
        assert_eq!(classify(mask, &own, 0, &later), DfEntry::Diverged);
        let mut human = pe;
        human[0x14] = 0;
        assert_eq!(classify(mask, &own, 0, &human), DfEntry::Insert);
        let mut ae = [0u8; 0xC];
        ae[0..2].copy_from_slice(&1u16.to_le_bytes());
        ae[2..4].copy_from_slice(&7u16.to_le_bytes());
        ae[4..8].copy_from_slice(&1150u32.to_le_bytes());
        ae[8] = 3;
        assert_eq!(classify(mask, &own, 1, &ae), DfEntry::Own);
        ae[2] = 8;
        assert_eq!(classify(mask, &own, 1, &ae), DfEntry::Diverged);
        assert_eq!(classify(0, &own, 1, &ae), DfEntry::Insert);
    }

    #[test]
    fn a_df_reencodes_to_its_own_bytes() {
        let b = sample();
        let d = Df::decode(&b).unwrap();
        assert_eq!(d.time(), 1100);
        assert_eq!(d.late, vec![(50, 0)]);
        assert_eq!(d.inputs, vec![[1; 11], [2; 11]]);
        assert_eq!(d.profile_raw, vec![0, 0xAA, 0xBB]);
        assert_eq!(d.ping, vec![(0, 0x1234)]);
        assert_eq!(d.ai, vec![(2, 1, 3)]);
        assert_eq!(d.encode(), b);
    }

    #[test]
    fn bot_entries_go_in_bit_order_and_ai_is_appended() {
        let b = sample();
        let x = BotInput { slot: 1, time: 1100, heading: 0x0102, pitch: 0xFFFE, actions: 0x00AB_CDEF_0123_4567 };
        let a = BotAi { slot: 5, time: 1100, kind: 1, param: 4 };
        let m = merge_df(&b, &[x], &[a]).unwrap();
        let d = Df::decode(&m).unwrap();
        assert_eq!(d.input_mask, 0b111);
        assert_eq!(d.inputs, vec![[1; 11], x.wire(), [2; 11]]);
        assert_eq!(x.wire(), [0x02, 0x01, 0xFE, 0xFF, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67]);
        assert_eq!(d.ai, vec![(2, 1, 3), (5, 1, 4)]);
        assert_eq!(d.late_inputs, vec![[0; 11]]);
        // a second input for slot 0 (already in the DF) and an input at another time are refused
        assert!(merge_df(&b, &[BotInput { slot: 0, ..x }], &[]).is_err());
        assert!(merge_df(&b, &[BotInput { time: 1150, ..x }], &[]).is_err());
        assert_eq!(merge_df(&b, &[], &[]).unwrap(), b);
    }
}
