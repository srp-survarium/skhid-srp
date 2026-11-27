//! One match's world in this process: created from the match's options blob and its first world (DE item) on a
//! level the process already loaded (`run::step3` / the zygote), then stepped DF by DF exactly as the client's DF
//! handler does, with the read-outs a relay needs (E0 bytes, a DE body, the end state and the statistics).
//!
//! Everything here is the client's own code path; the host writes data only (stand-ins, bindings, the network
//! client's DF times). The world runs in **client mode** (the cook's server byte 0x20F44275C left 0): late inputs are
//! rolled back like on every client.
//! One match per process: the core is the static buffer 0x20F442760.

use crate::hlog;
use crate::mem;
use crate::run::{call2, call3, rd64, rd8, GAME, PROJECT};
use std::sync::atomic::{AtomicU32, Ordering};

/// `change_input<T>` (0x2001B3290 player, 0x2001383F0 profile, 0x2001B5230 ping, 0x2001B4330 ai):
/// (W, element, 0, 1, 0, a6, &u32 out).
type ChangeInput = extern "win64" fn(u64, u64, u64, u8, u8, u8, u64) -> u32;

/// The DF handler's list struct S (a local of 0x2001AFE30): buffer_vectors {begin, end, cap} + inline storage.
const S_SIZE: usize = 0x1600;
const LISTS: [(u64, u64, u64); 8] = [
    (0x08, 0x20, 0x34),       // enter (u8)
    (0x38, 0x50, 0x64),       // leave (u8)
    (0x68, 0x80, 0x620),      // first player-input list (0x18): late entries
    (0x620, 0x638, 0x818),    // second player-input list (0x18): this tick's inputs
    (0x818, 0x830, 0x10F0),   // profiles (0x70)
    (0x10F0, 0x1108, 0x11F8), // pings (0xC)
    (0x11F8, 0x1210, 0x1300), // ai (0xC)
    (0x1300, 0x1318, 0x1518), // confirmations (8)
];
/// The client copies at most 0x1000 bytes of a DF into its job (0x2005BB7B0); the host's copy buffer is larger.
const DF_MAX: usize = 0x2000;

/// Per-DF result of [`MatchWorld::step_df`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    pub t: u32,
    /// inserted elements: enter, leave, list 1, list 2, profile, ping, ai
    pub inserted: [u32; 7],
    /// `change_input` returned non-zero (the input was refused, merged or moved: a client could have done otherwise)
    pub rejected: u32,
    /// inputs at or before the present when inserted (client mode: rolled back, as on every client)
    pub rolled_back: u32,
    /// the earliest time of those inputs: the world simulates again from there (the kill log drops its deaths from
    /// this time on before it takes the step's own, kills.rs)
    pub late_from: Option<u32>,
    /// the rule state `W+0x180AE0` after the step
    pub state: u32,
}

/// One player's numbers from the statistics object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerNumbers {
    pub slot: u8,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    pub chests: u16,
    /// paid respawns in a PvE mission: keys and gold the world charged, from the player's world entry
    /// `[W+0x17C2A0] + i*0xB8` (+0x54 keys, +0x58 gold; respawn branch 0x2003CF625)
    pub keys_spent: u32,
    pub gold_spent: u32,
}

/// Ammo slots and pocket slots of a profile whose stacks the match draws from the options storage (spawn 0x20034AA30,
/// box 0x20034B320, death `unload_to_profile` 0x20034AE90: ammo table 0x200990EE0 = 8, 9; pockets 0x200990EF0 = 13..17,
/// 20 and the pocket objects of 18 / 19 through the same vcall).
pub const STACK_SLOTS: [u64; 10] = [8, 9, 13, 14, 15, 16, 17, 18, 19, 20];
/// Weapon slots whose magazine belongs to an ammo slot (table 0x200990F30 = 7, 10).
pub const WEAPON_SLOTS: [u64; 2] = [7, 10];

/// One ammo / pocket stack of a player at one moment: the options item record (`+0` uid, `+8` dict, `+0xC` storage)
/// and the units the player carries of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StackNow {
    pub uid: u32,
    pub dict: u16,
    /// the match's storage of the stack, options item `+0xC`
    pub storage: u32,
    /// units in the live character: the slot object's count (`obj+0x4D0`, 1 for a non-stack object `obj+0x4D4 == 0`)
    /// plus, for ammo, the magazine of every weapon whose ammo object `[w+0x1748]` is that slot object
    /// (`w+0x44BA` + a round in the chamber `w+0x44CD`); 0 when the character is dead (the death put them back)
    pub carried: u32,
}

/// One options record's stacks (every item a profile's ammo or pocket slot names).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerStacks {
    pub slot: u8,
    /// `ai_id` (record `+0x2E0`) is -1
    pub human: bool,
    /// the current profile's character is alive (`char+0x748`)
    pub alive: bool,
    /// the current profile index (`[W+0x17C2A0] + slot*0xB8` `+0x48`)
    pub profile: u8,
    pub stacks: Vec<StackNow>,
}

/// What one player used of his stacks over the match: `start storage - end storage - end carried` per item, the
/// formula of a world in which nothing else moves the storage (spawn takes a life's load out, the box refills from it,
/// the death puts the carried units and the magazines back, PROVEN 0x20034AA30 / 0x20034B320 / 0x20034AE90 /
/// 0x2006D8E00). Only items with a positive use are listed.
pub fn used_between(start: &[PlayerStacks], end: &[PlayerStacks]) -> Vec<(u8, Vec<world_host_proto::UsedStack>)> {
    let mut out = Vec::new();
    for e in end.iter().filter(|e| e.human) {
        let Some(s) = start.iter().find(|s| s.slot == e.slot) else { continue };
        let mut v = Vec::new();
        for st in &e.stacks {
            let Some(s0) = s.stacks.iter().find(|x| x.uid == st.uid && x.dict == st.dict) else { continue };
            let left = u64::from(st.storage) + u64::from(st.carried);
            let used = u64::from(s0.storage).saturating_sub(left);
            if used > 0 {
                v.push(world_host_proto::UsedStack { uid: st.uid, dict: st.dict, used: used.min(u64::from(u32::MAX)) as u32 });
            }
        }
        if !v.is_empty() {
            out.push((e.slot, v));
        }
    }
    out
}

/// The world's end state and numbers at one tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Results {
    /// world time of the newest item
    pub time: u32,
    /// rule state `W+0x180AE0` (3 running, 4 / 5 ended)
    pub state: u32,
    /// winner team `S+0xEBC0` (core +0xEBB8): 0 / 1 a team, 2 draw, 3 none
    pub winner: u8,
    /// the team values the mode rule keeps, u16 `S+0xEBDC` / `S+0xEBDE` (core +0xEBD4 / +0xEBD6): tdm tickets left
    /// (written at the end, 0x200366AD1), artefact hunt deliveries (0x20035E017), capture-the-point points, ...
    /// 0 in modes whose rule never writes them
    pub team_score: [u32; 2],
    pub players: Vec<PlayerNumbers>,
    /// the version-3 detail rows (event matrix, streaks, inactive flag, badge counts) of every record slot
    pub details: Vec<world_host_proto::PlayerDetail>,
}

/// Size of a player block of the statistics object (`B = S + 0x30 + i*0xA24`).
pub const BLOCK: usize = 0xA24;

/// The detail row of one player from his statistics block `blk` (`BLOCK` bytes; PROVEN offsets):
/// * `u16 count[74 events][9 profiles]` at +0 (event e, profile p at `e*0x12 + p*2`): summed over the profiles,
///   saturating at 0xFFFF;
/// * badge slot t at `+0x534 + 12t`: `+1..+3` = how many times state 1 / 2 / 3 was earned;
/// * last activity stamp u32 +0x9E8, not-counted time u32 +0x9EC, inactive flag u8 +0x9F0;
/// * current kill streak u16 +0xA08, best kill streak u16 +0xA1E.
pub fn detail_of_block(slot: u8, blk: &[u8]) -> world_host_proto::PlayerDetail {
    let u16_at = |o: usize| u16::from_le_bytes([blk[o], blk[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([blk[o], blk[o + 1], blk[o + 2], blk[o + 3]]);
    let mut d = world_host_proto::PlayerDetail { slot, ..Default::default() };
    for (e, v) in d.events.iter_mut().enumerate() {
        *v = (0..9).map(|p| u32::from(u16_at(e * 0x12 + p * 2))).sum::<u32>().min(0xFFFF) as u16;
    }
    for (t, b) in d.badges.iter_mut().enumerate() {
        let o = 0x534 + 12 * t;
        *b = [blk[o + 1], blk[o + 2], blk[o + 3]];
    }
    d.last_active_ms = u32_at(0x9E8);
    d.inactive_ext_ms = u32_at(0x9EC);
    d.inactive = blk[0x9F0] != 0;
    d.cur_streak = u16_at(0xA08);
    d.best_streak = u16_at(0xA1E);
    d
}

/// The E0 bytes of one tick (writer 0x2002FC5A0 with full = 0 on the item of the tick, as the client's E0 producer
/// at 0x2001AE5D9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickHash {
    pub t: u32,
    pub mask: u32,
    /// one byte per set mask bit, in player order
    pub hash: Vec<u8>,
    pub g: [u8; 3],
}

pub struct MatchWorld {
    pub core: u64,
    w: u64,
    nc: u64,
    n_records: u32,
    s: u64,
    rd: u64,
    msg: u64,
    elem: u64,
    ret: u64,
    df_times_set: bool,
    pub steps: u64,
    pub last_t: u32,
    /// totals over the match
    pub rejected: u64,
    pub rolled_back: u64,
    /// render commands retired without execution ([`retire_render_commands`]), and how many of them carried the
    /// consumer's "keep" marks (+0x28 set, or +0x6C past the channel's frame +0xC8)
    pub render_retired: u64,
    pub render_marked: u64,
    /// the most render-pool records in use (scene 0x2034C3F40 +0x40, capacity +0x48 = 8192) seen before a retire
    pub render_pool_peak: u32,
    /// the players' ammo and pocket stacks before the first DF (no one spawned yet): the start of [`MatchWorld::used`]
    start_stacks: Option<Vec<PlayerStacks>>,
    /// per record slot: bit p = the player's profile p character was alive after some step (he spawned with it)
    spawned: [u16; 20],
    /// steps with a late input in which a human's storage moved (see `step_df`)
    pub late_stack_steps: u32,
}

fn arg_set(k: &str) -> bool {
    std::env::args().any(|a| a == k)
}

impl MatchWorld {
    /// The process-wide objects a match needs, made once (idempotent): the network client game+0x10FE30, a zeroed
    /// 0x7A9AE0-byte object with the real vtable 0x200957650 and the game at +0x10 (the ctor needs the network world,
    /// which the host does not build; only the exe's own accessors run on it), its options object [nc+0x7A68E0]
    /// (0x8000 bytes) and the options' 16 MB player-record buffer [opts+0]. A zygote makes them before it forks, so its
    /// children share them (24 MB less private memory per match). Returns (nc, opts).
    pub fn prepare_process() -> Result<(u64, u64), String> {
        let game = GAME.load(Ordering::Relaxed);
        if game == 0 {
            return Err("no game object (the level is not loaded)".into());
        }
        unsafe {
            let mut nc = rd64(game + 0x10FE30);
            if nc == 0 {
                nc = mem::heap_alloc(0x7A9AE0, true) as u64;
                *(nc as *mut u64) = 0x200957650;
                *((nc + 0x10) as *mut u64) = game;
                *((game + 0x10FE30) as *mut u64) = nc;
            }
            // the options at [nc+0x7A68E0]+0x10 (as the DD path 0x2001F3842 places them); [opts+0] = the player-record buffer
            let mut ob = rd64(nc + 0x7A68E0);
            if ob == 0 {
                ob = mem::heap_alloc(0x8000, true) as u64;
                *((nc + 0x7A68E0) as *mut u64) = ob;
            }
            let opts = ob + 0x10;
            if rd64(opts) == 0 {
                *(opts as *mut u64) = mem::heap_alloc(16 << 20, true) as u64;
            }
            Ok((nc, opts))
        }
    }

    /// Read an options blob (the DD body) into `opts` with the client's reader 0x2001250E0 (reader {begin, cursor, u32
    /// size}, the game's dictionary game+0x10FE20). Returns the bytes the reader used.
    unsafe fn read_options(opts: u64, options: &[u8]) -> u64 {
        unsafe {
            let game = GAME.load(Ordering::Relaxed);
            let buf = mem::heap_alloc(options.len(), false) as u64;
            std::ptr::copy_nonoverlapping(options.as_ptr(), buf as *mut u8, options.len());
            let rd = mem::heap_alloc(0x18, true) as u64;
            *(rd as *mut u64) = buf;
            *((rd + 8) as *mut u64) = buf;
            *((rd + 0x10) as *mut u32) = options.len() as u32;
            call3(0x2001250E0, opts, rd, rd64(game + 0x10FE20));
            rd64(rd + 8) - buf
        }
    }

    /// Create the match from its options blob (the DD body) and load its first world (the decoded DE item), as the
    /// client's online match ready does. The level must be loaded (`PROJECT`, `GAME`).
    /// `de_item` None: no first world; the first history item is made as the local match does (0x2002F04F0, used by
    /// the bind-pose check).
    pub fn create(options: &[u8], de_item: Option<&[u8]>) -> Result<MatchWorld, String> {
        let start = match de_item {
            Some(de) => Start::De(de),
            None => Start::Bare,
        };
        Self::create_with(options, start).map(|(m, _)| m)
    }

    /// Create the match from its options blob alone and build its starting world itself: `on_match_ready` with these
    /// options (the seed is their `match_id`, opts+0x1074), the statistics binding, the first history item 0x2002F04F0
    /// (in the local match's order, as the cached captures were made), written with the full writer 0x2002FC5A0
    /// (full = 1), its time labelled at least 16 ticks (as the relay labels a cached world), and loaded back onto the
    /// first item the way the online client loads the server's DE. Returns the match and the start item: the clients'
    /// first DE.
    ///
    /// `first_step` (PvE): the world then takes one step with no inputs (time + tick) and that item is the start item,
    /// loaded back the same way. A t50 PvE world has the scripting rule's per-slot environment list at -1, which crashes
    /// a client that rolls back to it (0x2002AA1B3); the world after one step carries the list, as the original server
    /// sent.
    pub fn create_own(options: &[u8], first_step: bool) -> Result<(MatchWorld, Vec<u8>), String> {
        let (mut m, item) = Self::create_with(options, Start::Own)?;
        let mut item = item.unwrap_or_default();
        if first_step {
            let tick = unsafe { (rd8(m.opts() + 0x1083) as u32).max(1) };
            let t = m.present() + tick;
            m.step_empty(t)?;
            let (it, _, b) = crate::run::tick_e0(m.core, t, 1).ok_or("the stepped item did not serialize")?;
            if it != t {
                return Err(format!("the stepped item has time {it}, not {t}"));
            }
            unsafe { load_de(m.core, &b)? };
            let (_, _, again) = crate::run::tick_e0(m.core, t, 1).ok_or("the reloaded item did not serialize")?;
            hlog!("own start world: first step {t}, {} bytes, sha1 {}, reload {}", b.len(), sha1_hex(&b), if again == b { "equal" } else { "DIFFERENT" });
            if again != b {
                return Err("the stepped start item changed when loaded back".into());
            }
            item = b;
        }
        Ok((m, item))
    }

    /// The options object (the one `read_options` filled).
    fn opts(&self) -> u64 {
        unsafe { rd64(self.nc + 0x7A68E0) + 0x10 }
    }

    /// One step to `t` with no inputs: the DF times as the DE handler / DF handler set them ([`Self::step_df`]), then
    /// `move(W, t, t)` 0x2002F6610. Later DFs must be after `t`.
    fn step_empty(&mut self, t: u32) -> Result<(), String> {
        unsafe {
            if !self.df_times_set {
                let de_t = self.present();
                for o in [0x7A6930u64, 0x7A692C, 0x7A6998] {
                    *((self.nc + o) as *mut u32) = de_t;
                }
                self.df_times_set = true;
            }
            *((self.nc + 0x7A6928) as *mut u32) = t;
            if *((self.nc + 0x7A692C) as *const u32) < t {
                *((self.nc + 0x7A692C) as *mut u32) = t;
            }
            if let Err(c) = crate::k32::call_fault_guarded3(0x2002F6610, self.w, t as u64, t as u64) {
                return Err(format!("move(W, {t}) left after a fault (code {c:#x})"));
            }
            let (n, marked) = retire_render_commands();
            self.render_retired += n;
            self.render_marked += marked;
            self.steps += 1;
            self.last_t = t;
        }
        Ok(())
    }

    fn create_with(options: &[u8], start: Start<'_>) -> Result<(MatchWorld, Option<Vec<u8>>), String> {
        let game = GAME.load(Ordering::Relaxed);
        if game == 0 {
            return Err("no game object (the level is not loaded)".into());
        }
        // the :render thread's part during the creation: a consumer that retires the channel's commands while
        // on_match_ready / the first world push them (the producer's full callback frees them, as in the client)
        // diagnosis: HOST_POOL_CAP=n lowers the pool's capacity [scene+0x48] (a data write) so that a creation overflows
        // it; HOST_NO_CREATE_DRAIN=1 leaves the creation without the consumer (the stall the drain prevents)
        if let Some(cap) = std::env::var("HOST_POOL_CAP").ok().and_then(|v| v.parse::<u64>().ok()) {
            unsafe { *((0x2034C3F40u64 + 0x48) as *mut u64) = cap.clamp(64, 0x2000) };
            hlog!("diagnosis: render pool capacity set to {}", cap.clamp(64, 0x2000));
        }
        let drain = (std::env::var("HOST_NO_CREATE_DRAIN").is_err()).then(RenderDrain::start);
        let r = Self::create_inner(game, options, start);
        let (retired, peak) = drain.map(RenderDrain::stop).unwrap_or((0, 0));
        hlog!("creation: the render drain retired {retired} commands; pool peak {peak} of 8192 during the creation, {} in use now", render_pool_in_use());
        r
    }

    fn create_inner(game: u64, options: &[u8], start: Start<'_>) -> Result<(MatchWorld, Option<Vec<u8>>), String> {
        unsafe {
            let (nc, opts) = Self::prepare_process()?;
            let used = Self::read_options(opts, options);
            let dict = rd64(game + 0x10FE20);
            let name = std::ffi::CStr::from_ptr((opts + 0x74) as *const std::ffi::c_char).to_string_lossy().into_owned();
            let n_records = rd8(opts + 0x1080) as u32;
            hlog!(
                "match options: {used} of {} bytes, records {n_records}, tick {}, match id {}, project '{name}'",
                options.len(),
                rd8(opts + 0x1083),
                *((opts + 0x1074) as *const u32)
            );
            if used != options.len() as u64 {
                return Err(format!("the options reader used {used} of {} bytes", options.len()));
            }
            // user data as the online path builds it (0x2001F22E3): +0x38 = 0, +0x40 / +0x41 = 0
            let ud = call3(0x200097680, rd64(0x20F43E400), 0x48, 8);
            std::ptr::write_bytes(ud as *mut u8, 0, 0x48);
            *(ud as *mut u64) = game + 0x38;
            *((ud + 8) as *mut u64) = opts;
            *((ud + 0x10) as *mut u64) = game + 0x38 + 0x368;
            *((ud + 0x18) as *mut u64) = 0x20B522700;
            *((ud + 0x20) as *mut u64) = dict;
            *((ud + 0x28) as *mut u64) = game + 0xF8440;
            let row = call2(0x2002E0AA0, dict, opts + 0x74);
            *((ud + 0x30) as *mut u64) = row;
            if row == 0 {
                return Err(format!("no game-mode row for '{name}'"));
            }
            // host bots (bots.rs, off by default): PvE +0x38 = the controllers world, as the local path does
            crate::bots::user_data_hook(ud, game);
            // the variant's type id: the lazily numbered static 0x20F43FBEC (0x2001F23AD)
            let tid_slot = &*(0x20F43FBEC as *const AtomicU32);
            if tid_slot.load(Ordering::SeqCst) == 0 {
                let ctr = &*(0x200A9CA34 as *const AtomicU32);
                tid_slot.store(ctr.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
            }
            // variant<64> as the online path builds it (0x2001F2417)
            let var = mem::heap_alloc(0x58, true) as u64;
            *(var as *mut u64) = 0x200961CC8;
            *((var + 8) as *mut u64) = ud;
            *((var + 0x48) as *mut u64) = var;
            *((var + 0x50) as *mut u32) = tid_slot.load(Ordering::SeqCst);
            if arg_set("--server-mode") {
                // diagnosis only: the production host is client mode
                *(0x20F44275C as *mut u8) = 1;
                hlog!("server mode: byte 0x20F44275C := 1 (pvp_match_core_cook +0x3C)");
            }
            if arg_set("--bind-bp") {
                crate::bp::arm(&[crate::bp::BIND_CALL, crate::bp::BIND_AFTER]);
            }
            let (core, fl) = crate::run::query_one("pvp_match_resource", 0x68, var);
            if std::env::var("HOST_QUERY_LOG").is_ok() {
                crate::bp::query_report(&crate::run::out_dir());
            }
            if core != 0 {
                crate::bp::record_prewarm();
            }
            hlog!("pvp_match_resource -> core {core:#x} (entry flags {fl:#x}); null-cook answers: {}", crate::nullcook::summary());
            hlog!("render pool in use after the match resource: {}", render_pool_in_use());
            if core == 0 {
                return Err("pvp_match_resource did not load".into());
            }
            // the network client's match binding (online match ready 0x2001F0ACE): nc+0x7A68D0 = the
            // core (+1 reference), nc+0x7A68D8 = 0xFF (no local player)
            (*((core + 0x1B0) as *const AtomicU32)).fetch_add(1, Ordering::SeqCst);
            *((nc + 0x7A68D0) as *mut u64) = core;
            *((nc + 0x7A68D8) as *mut u8) = 0xFF;
            // the game's binding to the core: the first stores of 0x2001331D0; the rest of it is presentation
            let g = game + 0x38;
            *((g + 0xF4648) as *mut u8) = 0;
            *((g + 0x10FBAD) as *mut u8) = 0;
            if rd64(g + 0xF4640) == 0 {
                (*((core + 0x1B0) as *const AtomicU32)).fetch_add(1, Ordering::SeqCst);
                *((g + 0xF4640) as *mut u64) = core;
            }
            // the game's level project: 0x200130080(game+0x38, &[core+0x210], 0) (0x2001F0BB4)
            let pslot = mem::heap_alloc(8, true) as u64;
            *(pslot as *mut u64) = rd64(core + 0x210);
            call3(0x200130080, game + 0x38, pslot, 0);
            build_input_world(game);
            // set_game_world_core places the project's render objects in the render scene, which the device-less render
            // world does not have: the vector [proj+0x390, +0x398) is hidden during on_match_ready and restored after
            let proj = PROJECT.load(Ordering::Relaxed);
            let hidden = if proj != 0 {
                let e = rd64(proj + 0x398);
                *((proj + 0x398) as *mut u64) = rd64(proj + 0x390);
                Some(e)
            } else {
                None
            };
            client_ui_standins(game);
            let r = crate::k32::call_fault_guarded(0x2002EB0E0, core);
            if let Some(e) = hidden {
                *((proj + 0x398) as *mut u64) = e;
            }
            match r {
                Ok(_) => hlog!("on_match_ready returned; render pool in use {}", render_pool_in_use()),
                // the client UI rules' hooks may fault on the absent HUD after the world part: logged, not fatal
                Err(c) => hlog!("on_match_ready left after a fault (code {c:#x})"),
            }
            // the own start world's first item is saved where the local match saves it: after the statistics binding
            // (0x2001F46DF, then 0x2002F04F0), the order the cached captures were made in (byte-equal to the world the
            // clients of a real match loaded). Diagnosis: HOST_OWN_SAVE=early saves right after on_match_ready (the
            // online client's order), =late after the host's other match-start writes; both give other bytes
            let own_at = std::env::var("HOST_OWN_SAVE").unwrap_or_default();
            let own_late = own_at == "late";
            if matches!(start, Start::Own) && own_at == "early" {
                clear_medkit_owner_bytes(core);
                save_first_item(core)?;
            }
            let w210 = rd64(core + 0x210);
            let sum = if w210 != 0 { *((w210 + 0x380) as *const u32) } else { 0 };
            hlog!("collision world checksum [[core+0x210]+0x380] = {sum:08x}");
            if arg_set("--bind-bp") {
                crate::bp::report(&crate::run::out_dir());
            }
            // the statistics handler's match binding (0x2001F12E8): it changes the hashed state
            let h = game + 0xF8440;
            let f: extern "win64" fn(u64, u64, u64, u64) -> u64 = std::mem::transmute(rd64(rd64(h) + 0x108));
            f(h, core + 0x230, opts, dict);
            if matches!(start, Start::Own) && own_at != "early" && !own_late {
                clear_medkit_owner_bytes(core);
                save_first_item(core)?;
            }
            // the player-sound resource game+0x10FD98: a zeroed stand-in (no sound instances)
            if rd64(game + 0x10FD98) == 0 {
                *((game + 0x10FD98) as *mut u64) = mem::heap_alloc(0x2000, true) as u64;
                // host bots (bots.rs): what the voice-line player needs (bot voice entries, ai type 2)
                crate::bots::sound_commands_hook(rd64(game + 0x10FD98));
            }
            // the HUD's movie owner (game+0x5B8 +0x10): a null owner whose +0x208 is a null movie
            let hh = game + 0x5B8;
            if rd64(hh + 0x10) == 0 {
                let owner = crate::nullcook::null_object(0x1000);
                *((owner + 0x208) as *mut u64) = crate::nullcook::null_object(0x1000);
                *((hh + 0x10) as *mut u64) = owner;
            }
            crate::run::match_start_world_part(game);
            let mut start_item = None;
            match start {
                Start::De(de) => load_de(core, de)?,
                Start::Bare => save_first_item(core)?,
                Start::Own => {
                    if own_late {
                        clear_medkit_owner_bytes(core);
                        save_first_item(core)?;
                    }
                    let t0 = rd64(core + 0x8008);
                    if t0 == 0 {
                        return Err("no first history item".into());
                    }
                    let t0 = *((t0 + 0x106238) as *const u32);
                    let (it, _, mut item) = crate::run::tick_e0(core, t0, 1).ok_or("the first item did not serialize")?;
                    if item.len() < 4 {
                        return Err("the first item serialized empty".into());
                    }
                    hlog!("own start world: item of time {it}, {} bytes, sha1 {}", item.len(), sha1_hex(&item));
                    // the first DE's time: at least 16 ticks, the relay's label of a t = 50 world (match-server
                    // world.rs `for_clients`): with T below the options' horizon (+0x107A -> W+0x180ACC) the
                    // backfill after the load (0x2002F19C3) walks below 0 and faults on a null item
                    let tick = (rd8(opts + 0x1083) as u32).max(1);
                    let t_first = it.max(16 * tick);
                    item[..4].copy_from_slice(&t_first.to_le_bytes());
                    // the DE handler's load onto the client's own first item (0x2001F11B6): deserialize takes that
                    // item (newest <= T, 0x2002F16A0), reads the DE into it, backfills and clears the frozen flag
                    load_de(core, &item)?;
                    let (lt, _, again) = crate::run::tick_e0(core, t_first, 1).ok_or("the loaded item did not serialize")?;
                    hlog!("own start world: first DE time {t_first}; after the load item {lt}, {} bytes, {}", again.len(), if again == item { "equal" } else { "DIFFERENT" });
                    if again != item {
                        return Err("the start item changed when loaded back".into());
                    }
                    start_item = Some(item);
                }
            }
            hlog!("render pool in use after the first world: {} of 8192", render_pool_in_use());
            let s = mem::heap_alloc(S_SIZE, true) as u64;
            for (off, store, cap) in LISTS {
                *((s + off) as *mut u64) = s + store;
                *((s + off + 8) as *mut u64) = s + store;
                *((s + off + 0x10) as *mut u64) = s + cap;
            }
            Ok((MatchWorld {
                core,
                w: core + 0x230,
                nc,
                n_records,
                s,
                rd: mem::heap_alloc(0x18, true) as u64,
                msg: mem::heap_alloc(DF_MAX, false) as u64,
                elem: mem::heap_alloc(0x80, true) as u64,
                ret: mem::heap_alloc(8, true) as u64,
                df_times_set: false,
                steps: 0,
                last_t: 0,
                rejected: 0,
                rolled_back: 0,
                render_retired: 0,
                render_marked: 0,
                render_pool_peak: 0,
                start_stacks: None,
                spawned: [0; 20],
                late_stack_steps: 0,
            }, start_item))
        }
    }

    /// World time of the present (the newest item).
    pub fn present(&self) -> u32 {
        unsafe { *((self.w + 0x180AA0) as *const u32) }
    }

    pub fn records(&self) -> u32 {
        self.n_records
    }

    /// The collision world's checksum [[core+0x210]+0x380] (what the clients compare in their match start; Ready).
    pub fn collision_checksum(&self) -> u32 {
        unsafe {
            let w210 = crate::run::rd64(self.core + 0x210);
            if w210 != 0 { *((w210 + 0x380) as *const u32) } else { 0 }
        }
    }

    /// Feed one DF (its bytes as the relay's recorder writes them: before the per-recipient filtering) in the DF
    /// handler's order and step the world to its time: the exe's DF reader 0x20032E790, the network client's DF times,
    /// enter / leave, `change_input` per list (arguments 4..6 = 1, 0, and 1 for the first list, as the DF handler
    /// 0x2001AFE30 passes them), then `move(W, T, T)` 0x2002F6610 under the fault guard.
    pub fn step_df(&mut self, df: &[u8]) -> Result<Step, String> {
        if df.len() > DF_MAX {
            return Err(format!("DF of {} bytes (limit {DF_MAX})", df.len()));
        }
        if self.start_stacks.is_none() {
            self.start_stacks = Some(self.stacks());
        }
        unsafe {
            if !self.df_times_set {
                // the DE handler sets these to the DE time (0x2001AFD99)
                let de_t = self.present();
                for o in [0x7A6930u64, 0x7A692C, 0x7A6998] {
                    *((self.nc + o) as *mut u32) = de_t;
                }
                self.df_times_set = true;
            }
            for (off, store, _) in LISTS {
                *((self.s + off + 8) as *mut u64) = self.s + store; // empty every list
            }
            std::ptr::copy_nonoverlapping(df.as_ptr(), self.msg as *mut u8, df.len());
            *(self.rd as *mut u64) = self.msg;
            *((self.rd + 8) as *mut u64) = self.msg;
            *((self.rd + 0x10) as *mut u32) = df.len() as u32;
            let reader: extern "win64" fn(u64, u64, u64) -> u64 = std::mem::transmute(0x20032E790u64);
            reader(self.s, self.rd, 0);
            let t = *(self.s as *const u32);
            let consumed = rd64(self.rd + 8) - self.msg;
            if consumed != df.len() as u64 {
                return Err(format!("DF {t}: the reader used {consumed} of {} bytes", df.len()));
            }
            if self.steps > 0 && t <= self.last_t {
                return Err(format!("DF {t} after DF {}: not increasing", self.last_t));
            }
            let w = self.w;
            let present = self.present();
            // the newest DF time, before the DF's inputs (0x2001B03C3); read back by nc.vt[0xE8]
            *((self.nc + 0x7A6928) as *mut u32) = t;
            if *((self.nc + 0x7A692C) as *const u32) < t {
                *((self.nc + 0x7A692C) as *mut u32) = t;
            }
            let list = |off: u64, sz: u64| -> Vec<u64> {
                let (b, e) = (rd64(self.s + off), rd64(self.s + off + 8));
                (0..(e.saturating_sub(b) / sz)).map(|i| b + i * sz).collect()
            };
            let mut st = Step { t, ..Default::default() };
            let enter: extern "win64" fn(u64, u64, u64) -> u64 = std::mem::transmute(0x2002F2890u64);
            let leave: extern "win64" fn(u64, u64, u64) -> u64 = std::mem::transmute(0x2002F2CD0u64);
            for e in list(0x08, 1) {
                enter(w, rd8(e) as u64, t as u64);
                st.inserted[0] += 1;
            }
            for e in list(0x38, 1) {
                leave(w, rd8(e) as u64, t as u64);
                st.inserted[1] += 1;
            }
            let ci_player: ChangeInput = std::mem::transmute(0x2001B3290u64);
            let ci_profile: ChangeInput = std::mem::transmute(0x2001383F0u64);
            let ci_ping: ChangeInput = std::mem::transmute(0x2001B5230u64);
            let ci_ai: ChangeInput = std::mem::transmute(0x2001B4330u64);
            let (elem, ret) = (self.elem, self.ret);
            let mut ins = |ci: ChangeInput, e: u64, sz: usize, toff: u64, a6: u8, k: usize, st: &mut Step| {
                std::ptr::copy_nonoverlapping(e as *const u8, elem as *mut u8, sz);
                // host bots (bots.rs): a DF copy of the host's own bot entry is skipped and compared, not inserted again
                let own = match k {
                    2 | 3 => crate::bots::df_entry(0, elem),
                    6 => crate::bots::df_entry(1, elem),
                    _ => crate::bots::DfEntry::Insert,
                };
                match own {
                    crate::bots::DfEntry::Insert => {}
                    crate::bots::DfEntry::Own => return,
                    crate::bots::DfEntry::Diverged => {
                        st.rejected += 1;
                        return;
                    }
                }
                let it = *((elem + toff) as *const u32);
                if it <= present {
                    st.rolled_back += 1;
                    st.late_from = Some(st.late_from.map_or(it, |x| x.min(it)));
                }
                *(ret as *mut u32) = 0;
                if ci(w, elem, 0, 1, 0, a6, ret) != 0 {
                    st.rejected += 1;
                }
                st.inserted[k] += 1;
            };
            for e in list(0x68, 0x18) {
                ins(ci_player, e, 0x18, 0x10, 1, 2, &mut st);
            }
            for e in list(0x620, 0x18) {
                ins(ci_player, e, 0x18, 0x10, 0, 3, &mut st);
            }
            for e in list(0x818, 0x70) {
                ins(ci_profile, e, 0x70, 0x68, 0, 4, &mut st);
            }
            for e in list(0x10F0, 0xC) {
                ins(ci_ping, e, 0xC, 4, 0, 5, &mut st);
            }
            for e in list(0x11F8, 0xC) {
                ins(ci_ai, e, 0xC, 4, 0, 6, &mut st);
            }
            // host bots (bots.rs, PvE): a rolled-back step re-simulates the past without the scene's controllers first
            crate::bots::before_move(w, st.late_from, present);
            // ammo charge: a step that simulates the past again may run a spawn / box / death a second time on the
            // options storage (OPEN: whether the history item restores it); such steps are counted and logged so a
            // charge built on them can be told apart
            let storage_before = st.late_from.map(|_| self.storage_sum());
            if let Err(c) = crate::k32::call_fault_guarded3(0x2002F6610, w, t as u64, t as u64) {
                return Err(format!("move(W, {t}) left after a fault (code {c:#x})"));
            }
            if let Some(b) = storage_before {
                let a = self.storage_sum();
                if a != b {
                    self.late_stack_steps += 1;
                    hlog!("used: DF {t} simulated again from {:?} and the humans' storage moved ({b} -> {a}): the stack counts may carry a re-simulated draw", st.late_from);
                }
            }
            st.state = *((w + 0x180AE0) as *const u32);
            self.render_pool_peak = self.render_pool_peak.max(*((0x2034C3F40u64 + 0x40) as *const u32));
            let (n, marked) = retire_render_commands();
            self.render_retired += n;
            self.render_marked += marked;
            self.steps += 1;
            self.last_t = t;
            self.rejected += st.rejected as u64;
            self.rolled_back += st.rolled_back as u64;
            self.note_alive_profiles();
            Ok(st)
        }
    }

    /// E0 bytes of the newest item at or before `t` (None before the first item).
    pub fn tick_hash(&self, t: u32) -> Option<TickHash> {
        let (it, h, _) = crate::run::tick_e0(self.core, t, 0)?;
        let mask = h[20] as u32 | (h[21] as u32) << 8 | (h[22] as u32) << 16;
        let hash = (0..20).filter(|i| mask & (1u32 << i) != 0).map(|i| h[i]).collect();
        Some(TickHash { t: it, mask, hash, g: [h[23], h[24], h[25]] })
    }

    /// The full serialization of the newest item at or before `t` (writer 0x2002FC5A0, full = 1): a DE body, the same
    /// bytes as the client's world-save capture. Returns (item time, bytes).
    pub fn full_item(&self, t: u32) -> Option<(u32, Vec<u8>)> {
        crate::run::tick_e0(self.core, t, 1).map(|(it, _, st)| (it, st))
    }

    /// The resync DE of tick `t`: item t (full = 1) plus the per-player animation-history tail, checked
    /// ([`crate::run::de_with_tail`]).
    pub fn de(&self, t: u32) -> Result<crate::run::DeBody, String> {
        let opts = unsafe { rd64(self.nc + 0x7A68E0) } + 0x10;
        crate::run::de_with_tail(self.core, opts, t)
    }

    /// The options object `[W+0x180A40]`: records `[+8]` (0x300 bytes each), count u8 `+0x1080` (as the PvE nut give
    /// 0x2003CE977 reads them). 0 before the world has it.
    fn options_records(&self) -> (u64, u32) {
        unsafe {
            let o = rd64(self.w + 0x180A40);
            if o == 0 {
                return (0, 0);
            }
            (rd64(o + 8), (rd8(o + 0x1080) as u32).min(self.n_records).min(20))
        }
    }

    /// Every record's ammo and pocket stacks now (read-only). Record `rec`: profiles `[rec+0x18]` (0x2A08 each, count u8
    /// `rec+0x2F0`; slot s = `{u16 item index, u16 count}` at `+0xA0 + 4s`), items `[rec+0x20]` (0x7C0 each, count u8
    /// `rec+0x2F1`). Carried units: the current profile's character `[entry + entry[0x48]*8]` (entry =
    /// `[W+0x17C2A0] + slot*0xB8`), inventory `[char+0x248]`, slot object `[inv+0x210+8s]` (as 0x20034B320 and
    /// 0x20034AE90 read them), only while the character is alive (`char+0x748`; the death put everything back).
    pub fn stacks(&self) -> Vec<PlayerStacks> {
        let (recs, n) = self.options_records();
        let mut out = Vec::new();
        if recs == 0 {
            return out;
        }
        unsafe {
            let ent = rd64(self.w + 0x17C2A0);
            for i in 0..n as u64 {
                let rec = recs + i * 0x300;
                let (profs, n_prof) = (rd64(rec + 0x18), rd8(rec + 0x2F0) as u64);
                let (items, n_items) = (rd64(rec + 0x20), rd8(rec + 0x2F1) as u64);
                let slot_item = |p: u64, s: u64| -> Option<u64> {
                    let idx = *((profs + p * 0x2A08 + 0xA0 + 4 * s) as *const u16) as u64;
                    (idx != 0xFFFF && idx < n_items).then_some(idx)
                };
                let mut ps = PlayerStacks { slot: i as u8, human: *((rec + 0x2E0) as *const u32) == u32::MAX, ..Default::default() };
                let mut idxs: Vec<u64> = Vec::new();
                for p in 0..n_prof.min(9) {
                    for s in STACK_SLOTS {
                        if let Some(x) = slot_item(p, s) {
                            if !idxs.contains(&x) {
                                idxs.push(x);
                            }
                        }
                    }
                }
                idxs.sort_unstable();
                let mut carried = vec![0u32; idxs.len()];
                if ent != 0 && profs != 0 && items != 0 {
                    let e = ent + i * 0xB8;
                    let cur = rd8(e + 0x48) as u64;
                    ps.profile = cur as u8;
                    // every profile's character: a dead one keeps its units until `unload_to_profile` runs at the next
                    // spawn, which zeroes the stack counts and empties the magazines (0x2006D8E00); a non-stack object
                    // (+0x4D4 = 0) stays in place, so it counts only while its character is alive
                    for pslot in 0..9u64 {
                        let ch = rd64(e + pslot * 8);
                        if ch == 0 {
                            continue;
                        }
                        let p = rd8(ch + 0x259) as u64;
                        let alive = rd8(ch + 0x748) != 0;
                        if pslot == cur && alive {
                            ps.alive = true;
                        }
                        if p >= n_prof {
                            continue;
                        }
                        let inv = rd64(ch + 0x248);
                        if inv != 0 {
                            let count = |obj: u64| -> u32 {
                                if rd8(obj + 0x4D4) != 0 {
                                    *((obj + 0x4D0) as *const u16) as u32
                                } else {
                                    u32::from(alive)
                                }
                            };
                            for s in STACK_SLOTS {
                                let obj = rd64(inv + 0x210 + 8 * s);
                                if let (true, Some(x)) = (obj != 0, slot_item(p, s)) {
                                    if let Some(k) = idxs.iter().position(|&y| y == x) {
                                        carried[k] += count(obj);
                                    }
                                }
                            }
                            for s in WEAPON_SLOTS {
                                let wp = rd64(inv + 0x210 + 8 * s);
                                if wp == 0 {
                                    continue;
                                }
                                let wb = wp - 0x30;
                                let ammo = rd64(wb + 0x1748);
                                let mag = rd8(wb + 0x44BA) as u32 + u32::from(rd8(wb + 0x44CD) != 0);
                                for a in [8u64, 9] {
                                    if ammo != 0 && rd64(inv + 0x210 + 8 * a) == ammo {
                                        if let Some(k) = slot_item(p, a).and_then(|x| idxs.iter().position(|&y| y == x)) {
                                            carried[k] += mag;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for (k, &x) in idxs.iter().enumerate() {
                    let it = items + x * 0x7C0;
                    ps.stacks.push(StackNow { uid: *(it as *const u32), dict: *((it + 8) as *const u16), storage: *((it + 0xC) as *const u32), carried: carried[k] });
                }
                out.push(ps);
            }
        }
        out
    }

    /// The weapon dicts (slots 7 and 10) of profile `p` of record `slot` (the options' items, as the lobby wrote them).
    pub fn profile_weapons(&self, slot: u8, p: u8) -> Vec<u16> {
        let (recs, n) = self.options_records();
        let mut v = Vec::new();
        if recs == 0 || u32::from(slot) >= n {
            return v;
        }
        unsafe {
            let rec = recs + u64::from(slot) * 0x300;
            let (profs, n_prof) = (rd64(rec + 0x18), rd8(rec + 0x2F0));
            let (items, n_items) = (rd64(rec + 0x20), rd8(rec + 0x2F1) as u64);
            if p >= n_prof || profs == 0 || items == 0 {
                return v;
            }
            for s in WEAPON_SLOTS {
                let idx = *((profs + u64::from(p) * 0x2A08 + 0xA0 + 4 * s) as *const u16) as u64;
                if idx != 0xFFFF && idx < n_items {
                    v.push(*((items + idx * 0x7C0 + 8) as *const u16));
                }
            }
        }
        v
    }

    /// The sum of every human record's item storage (`+0xC`), for the re-simulation check of `step_df`.
    fn storage_sum(&self) -> u64 {
        self.stacks().iter().filter(|p| p.human).flat_map(|p| p.stacks.iter()).map(|s| u64::from(s.storage)).sum()
    }

    /// The (uid, dict) of the items in profile `p`'s ammo box slots 11 / 12 of record `slot`.
    pub fn profile_boxes(&self, slot: u8, p: u8) -> Vec<(u32, u16)> {
        let (recs, n) = self.options_records();
        let mut v = Vec::new();
        if recs == 0 || u32::from(slot) >= n {
            return v;
        }
        unsafe {
            let rec = recs + u64::from(slot) * 0x300;
            let (profs, n_prof) = (rd64(rec + 0x18), rd8(rec + 0x2F0));
            let (items, n_items) = (rd64(rec + 0x20), rd8(rec + 0x2F1) as u64);
            if p >= n_prof || profs == 0 || items == 0 {
                return v;
            }
            for s in [11u64, 12] {
                let e = profs + u64::from(p) * 0x2A08 + 0xA0 + 4 * s;
                let (idx, count) = (*(e as *const u16) as u64, *((e + 2) as *const u16));
                if idx != 0xFFFF && idx < n_items && count > 0 {
                    let it = items + idx * 0x7C0;
                    v.push((*(it as *const u32), *((it + 8) as *const u16)));
                }
            }
        }
        v
    }

    /// Marks, per record, the profile whose character is alive now (called after every step: a life lasts more than a
    /// tick, so every profile a player spawned with is seen).
    fn note_alive_profiles(&mut self) {
        unsafe {
            let ent = rd64(self.w + 0x17C2A0);
            if ent == 0 {
                return;
            }
            for i in 0..self.n_records.min(20) as u64 {
                let e = ent + i * 0xB8;
                let p = rd8(e + 0x48) as u64;
                if p >= 9 {
                    continue;
                }
                let ch = rd64(e + p * 8);
                if ch != 0 && rd8(ch + 0x748) != 0 {
                    self.spawned[i as usize] |= 1 << p;
                }
            }
        }
    }

    /// What every human used in the match: per record slot the profiles he spawned with (bit p), their weapons (each
    /// dict once) and his stacks' use ([`used_between`]). None before the first DF.
    pub fn used(&self) -> Option<Vec<world_host_proto::PlayerUse>> {
        let start = self.start_stacks.as_ref()?;
        let end = self.stacks();
        hlog!("used: {} step(s) with a late input moved the humans' storage", self.late_stack_steps);
        let stacks = used_between(start, &end);
        let mut out = Vec::new();
        for e in end.iter().filter(|e| e.human) {
            let mask = self.spawned[usize::from(e.slot)];
            let mut weapons: Vec<u16> = Vec::new();
            for p in 0..9u8 {
                if mask & (1 << p) != 0 {
                    for d in self.profile_weapons(e.slot, p) {
                        if !weapons.contains(&d) {
                            weapons.push(d);
                        }
                    }
                }
            }
            let mut used = stacks.iter().find(|(s, _)| *s == e.slot).map(|(_, v)| v.clone()).unwrap_or_default();
            // premium ammo boxes: one per spawned profile whose slot 11 / 12 holds one (the inventory cook applies the
            // box of slot 11 to weapon slot 7 and of slot 12 to slot 10 at the spawn, 0x2002D9B8F)
            for p in 0..9u8 {
                if mask & (1 << p) != 0 {
                    for (uid, dict) in self.profile_boxes(e.slot, p) {
                        match used.iter_mut().find(|x| x.uid == uid && x.dict == dict) {
                            Some(x) => x.used += 1,
                            None => used.push(world_host_proto::UsedStack { uid, dict, used: 1 }),
                        }
                    }
                }
            }
            out.push(world_host_proto::PlayerUse { slot: e.slot, profiles: mask, weapons, stacks: used });
        }
        Some(out)
    }

    /// End state and statistics of the present world (read-only).
    pub fn results(&self) -> Results {
        unsafe {
            let w = self.w;
            let mut r = Results { time: self.present(), state: *((w + 0x180AE0) as *const u32), winner: 3, team_score: [0, 0], players: Vec::new(), details: Vec::new() };
            let s = rd64(w + 0x180A38);
            if s == 0 {
                return r;
            }
            r.winner = rd8(s + 0xEBC0);
            r.team_score = [*((s + 0xEBDC) as *const u16) as u32, *((s + 0xEBDE) as *const u16) as u32];
            // u16 count[74 events][9 profiles] at block B = S+0x30+i*0xA24: kill 0, death 2, pve_open_chest 42
            let row = |b: u64, e: u64| -> u16 { (0..9u64).map(|p| *((b + e * 0x12 + p * 2) as *const u16) as u32).sum::<u32>().min(0xFFFF) as u16 };
            for i in 0..self.n_records.min(20) as u64 {
                let b = s + 0x30 + i * 0xA24;
                let ent = rd64(w + 0x17C2A0);
                let (keys_spent, gold_spent) = if ent != 0 { (*((ent + i * 0xB8 + 0x54) as *const u32), *((ent + i * 0xB8 + 0x58) as *const u32)) } else { (0, 0) };
                r.players.push(PlayerNumbers { slot: i as u8, kills: row(b, 0), deaths: row(b, 2), score: *((b + 0xA1C) as *const u16), chests: row(b, 42), keys_spent, gold_spent });
                r.details.push(detail_of_block(i as u8, std::slice::from_raw_parts(b as *const u8, BLOCK)));
            }
            r
        }
    }

    /// Each record slot's alive flag, health and artefact row after the present step (vitals.rs; read-only). The
    /// character `p = [e + [e+0x48]*8]`, `e = [W+0x17C2A0] + i*0xB8`; alive = `p+0x74C`; health = the node named
    /// "health" in the list `[[p+0x5F8]+0x210]` (next `+0`, name `+0x58`), current `+0x8C` / max `+0x84` (as
    /// 0x200344E70 reads them); artefact row = event 5 of the slot's block `S+0x30+i*0xA24`. A slot whose character or
    /// parameter is missing is left out.
    pub fn vitals(&self) -> Vec<crate::vitals::Sample> {
        let mut out = Vec::new();
        unsafe {
            let w = self.w;
            let s = rd64(w + 0x180A38);
            let ents = rd64(w + 0x17C2A0);
            if s == 0 || ents == 0 {
                return out;
            }
            for i in 0..self.n_records.min(20) as u64 {
                let e = ents + i * 0xB8;
                let p = rd64(e + rd8(e + 0x48) as u64 * 8);
                if p == 0 || rd64(p + 0x240) == 0 || rd64(rd64(p + 0x240) + 0x10) != 0x200226E40 {
                    continue;
                }
                let params = rd64(p + 0x5F8);
                if params == 0 {
                    continue;
                }
                let mut node = rd64(params + 0x210);
                let mut hp = None;
                for _ in 0..256 {
                    if node == 0 {
                        break;
                    }
                    let name = rd64(node + 0x58);
                    if name != 0 && std::slice::from_raw_parts(name as *const u8, 7) == b"health\0" {
                        let (cur, max) = (*((node + 0x8C) as *const f32), *((node + 0x84) as *const f32));
                        if max > 0.0 && cur.is_finite() {
                            hp = Some((cur / max * 10000.0).clamp(0.0, 10000.0) as u16);
                        }
                        break;
                    }
                    node = rd64(node);
                }
                let Some(hp) = hp else { continue };
                let b = s + 0x30 + i * 0xA24;
                let artefacts = (0..9u64).map(|q| *((b + 5 * 0x12 + q * 2) as *const u16) as u32).sum::<u32>().min(0xFFFF) as u16;
                out.push(crate::vitals::Sample { slot: i as u8, alive: rd8(p + 0x74C) != 0, hp, artefacts });
            }
        }
        out
    }

    /// The match end of the statistics object, once, as the client's `close_current_match(3)` calls it:
    /// `S.vt[0x110](S, time)` (0x2001FB320). It adds the end events (participating 11, won 12, played_first_game 27),
    /// runs the match-end inactivity check (`B+0x9F0`), closes every open badge progress into a count and picks the
    /// four post-match medals (badge slots 23..26). It clears `S+0xEC20 / +0xEC28 / +0xEC30` (the world link): the world
    /// must not be stepped after it. `time` = the present `W+0x180A98`. Returns false when there is no statistics
    /// object, the slot is not the expected function, or the call faulted (the Final then carries the numbers from
    /// before it).
    pub fn match_end(&mut self) -> bool {
        unsafe {
            let s = rd64(self.w + 0x180A38);
            if s == 0 || rd64(s + 0xEC20) == 0 {
                hlog!("match end: no statistics object (or already ended)");
                return false;
            }
            let f = rd64(rd64(s) + 0x110);
            if f != 0x2001FB320 {
                hlog!("match end: S.vt[0x110] = {f:#x}, not 0x2001FB320: not called");
                return false;
            }
            let time = *((self.w + 0x180A98) as *const u32);
            match crate::k32::call_fault_guarded3(f, s, time as u64, 0) {
                Ok(_) => {
                    hlog!("match end: S.vt[0x110](S, {time}) done");
                    true
                }
                Err(c) => {
                    hlog!("match end: S.vt[0x110](S, {time}) faulted (code {c:#x})");
                    false
                }
            }
        }
    }
}

/// A host thread in the role of the client's `:render` consumer while a match is created: it pops the channel's
/// commands (as the pop 0x2000FA890 does) and appends each old node to the processed list as the consumer 0x2000F7220
/// does, without running the command. Freeing stays with the producer: the pool's full callback 0x2005FEC90 frees
/// before it waits, and the per-step [`retire_render_commands`] after. Plain memory operations only (no exe code on
/// this thread); the channel is a single-producer / single-consumer queue built for this split. One consumer at a
/// time: [`RenderDrain::stop`] joins the thread before any step.
pub struct RenderDrain {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    h: Option<std::thread::JoinHandle<(u64, u32)>>,
}

impl RenderDrain {
    pub fn start() -> RenderDrain {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let st = stop.clone();
        let h = std::thread::spawn(move || {
            let (mut n, mut peak) = (0u64, 0u32);
            loop {
                peak = peak.max(render_pool_in_use());
                let took = unsafe { consume_one() };
                if took {
                    n += 1;
                    continue;
                }
                if st.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_micros(200));
            }
            (n, peak)
        });
        RenderDrain { stop, h: Some(h) }
    }

    /// Stop and join; returns (commands retired, highest pool use seen).
    pub fn stop(mut self) -> (u64, u32) {
        self.stop.store(true, Ordering::SeqCst);
        self.h.take().and_then(|h| h.join().ok()).unwrap_or((0, 0))
    }
}

impl Drop for RenderDrain {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.h.take() {
            let _ = h.join();
        }
    }
}

/// One consumer step on the channel 0x2034C3D90 (B = q+8: head [B], tail [B+0x40], node link +0x10 (44-bit pointer);
/// processed list tail [q+0x90], link +0x18): false when the queue is empty or its next node is not linked yet.
unsafe fn consume_one() -> bool {
    use std::sync::atomic::AtomicU64;
    const Q: u64 = 0x2034C3D90;
    unsafe {
        let b = Q + 8;
        let head_a = &*(b as *const AtomicU64);
        let head = head_a.load(Ordering::SeqCst);
        if head == 0 || head == (*((b + 0x40) as *const AtomicU64)).load(Ordering::SeqCst) || rd64(Q + 0x90) == 0 {
            return false;
        }
        let next = (*((head + 0x10) as *const AtomicU64)).load(Ordering::SeqCst) & 0xFFF_FFFF_FFFF;
        if next == 0 {
            return false;
        }
        let old = head_a.swap(next, Ordering::SeqCst);
        (*((old + 0x18) as *const AtomicU64)).store(0, Ordering::SeqCst);
        let last = rd64(Q + 0x90);
        (*((last + 0x18) as *const AtomicU64)).swap(old, Ordering::SeqCst);
        (*((Q + 0x90) as *const AtomicU64)).store(old, Ordering::SeqCst);
        true
    }
}

/// Records of the render scene's command pool in use (scene 0x2034C3F40 +0x40; capacity +0x48 = 8192).
pub fn render_pool_in_use() -> u32 {
    unsafe { *((0x2034C3F40u64 + 0x40) as *const u32) }
}

/// The render channel (`one_way_render_channel` at 0x2034C3D90, made in the null render init): game code allocates
/// each render command from the render scene's pool (`single_size_fixed_allocator<0x628, 0x2000>` at scene
/// 0x2034C3F40) and pushes it (0x200600380). In the client the :render thread consumes and runs the commands
/// (0x2000F7220) and the producer frees the processed nodes (0x2000FA0D0). The host has no :render thread, so without
/// this the pool fills (8192 commands, about 0.4 per tick) and the pool's full callback 0x2005FEC90 waits forever.
/// Here the world thread (the producer) pops every queued command and retires it the consumer's way WITHOUT running it
/// (no drawing, no render-side copies: the commands only feed the render thread), then frees the processed nodes with
/// the exe's 0x2000FA0D0. Returns (retired, marked): `marked` counts nodes the client's consumer would have kept back
/// (+0x28 set or +0x6C past the channel frame +0xC8).
pub fn retire_render_commands() -> (u64, u64) {
    const Q: u64 = 0x2034C3D90;
    unsafe {
        if rd64(Q + 8) == 0 || rd64(Q + 0x90) == 0 {
            return (0, 0);
        }
        let pop: extern "win64" fn(u64, *mut u64) -> u64 = std::mem::transmute(0x2000FA890u64);
        let frame = *((Q + 0xC8) as *const u32);
        let (mut n, mut marked) = (0u64, 0u64);
        loop {
            let mut old = 0u64;
            if pop(Q + 8, &mut old) == 0 || old == 0 {
                break;
            }
            if rd8(old + 0x28) != 0 || *((old + 0x6C) as *const u32) > frame {
                marked += 1;
            }
            *((old + 0x18) as *mut u64) = 0;
            let last = rd64(Q + 0x90);
            (*((last + 0x18) as *const std::sync::atomic::AtomicU64)).swap(old, Ordering::SeqCst);
            *((Q + 0x90) as *mut u64) = old;
            n += 1;
        }
        if n != 0 {
            let free: extern "win64" fn(u64) = std::mem::transmute(0x2000FA0D0u64);
            free(Q + 8);
        }
        (n, marked)
    }
}

fn sha1_hex(b: &[u8]) -> String {
    crate::crypt::sha1(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// How a match gets its first world.
#[derive(Clone, Copy)]
enum Start<'a> {
    /// a DE item from the relay, loaded as the client's DE handler does
    De(&'a [u8]),
    /// only the first history item 0x2002F04F0 (the bind-pose check; the world stays frozen in client mode)
    Bare,
    /// the host's own start world ([`MatchWorld::create_own`])
    Own,
}

/// `placeable_medkit` vtable at the object's +0x480, the node it appends to the world's object list (RTTI).
const MEDKIT_NODE_VT: u64 = 0x200982708;
/// The owner byte of a medkit nobody deployed: any index >= the record count (at most 20) reads as no owner.
const MEDKIT_NO_OWNER: u8 = 0xFF;

/// The own start world byte for byte the same in every run: every `placeable_medkit` in the world's object list
/// (W+0x17D778 count, +0x17D780 head, node +0x10 next) gets its owner byte M+0x60C := 0xFF (no owner). The medkit ctor
/// 0x2003D9F60 leaves +0x60C unset, so the saver wrote whatever the arena held there into the start item. The byte is
/// the deployer's record index; the owner getter 0x2003DAEB0 returns null for an index >= the record count, so 0xFF =
/// nobody. A fixed value makes the start item reproducible. Returns (medkits, how many held another value).
unsafe fn clear_medkit_owner_bytes(core: u64) -> (u32, u32) {
    let w = core + 0x230;
    let count = *((w + 0x17D778) as *const u32);
    let mut node = rd64(w + 0x17D780);
    let (mut n, mut dirty, mut seen) = (0u32, 0u32, 0u32);
    let mut vals = Vec::new();
    while node != 0 && seen < count.max(1) * 2 {
        seen += 1;
        if rd64(node) == MEDKIT_NODE_VT {
            let b = (node - 0x480 + 0x60C) as *mut u8;
            if *b != MEDKIT_NO_OWNER {
                dirty += 1;
                vals.push(format!("{:#x}", *b));
                *b = MEDKIT_NO_OWNER;
            }
            n += 1;
        }
        node = rd64(node + 0x10);
    }
    hlog!("own start world: {n} placeable medkit(s) in the object list of {count}; owner byte +0x60C := 0xFF ({dirty} held {})", if vals.is_empty() { "-".to_string() } else { vals.join(" ") });
    (n, dirty)
}

/// The first history item: 0x2002F04F0(W) (allocate 0x2002F0D10 + save 0x2002F4250), as the online client's match
/// ready (0x2001F11B6) and the local match (0x2001F46EB) call it.
unsafe fn save_first_item(core: u64) -> Result<(), String> {
    match crate::k32::call_fault_guarded(0x2002F04F0, core + 0x230) {
        Ok(_) => {
            hlog!("0x2002F04F0(W): first history item saved");
            Ok(())
        }
        Err(c) => Err(format!("0x2002F04F0(W) left after a fault (code {c:#x})")),
    }
}

/// The first world: deserialize(W, &{begin, cursor, u32 size}, 1, 9) 0x2002F1650, as the DE handler calls it
/// (0x2001AFC39); with no history item it allocates one, reads the item and loads it.
unsafe fn load_de(core: u64, de: &[u8]) -> Result<(), String> {
    if de.len() < 4 {
        return Err("empty DE item".into());
    }
    unsafe {
        let dbuf = mem::heap_alloc(de.len(), false) as u64;
        std::ptr::copy_nonoverlapping(de.as_ptr(), dbuf as *mut u8, de.len());
        let drd = mem::heap_alloc(0x18, true) as u64;
        *(drd as *mut u64) = dbuf;
        *((drd + 8) as *mut u64) = dbuf;
        *((drd + 0x10) as *mut u32) = de.len() as u32;
        let f: extern "win64" fn(u64, u64, u64, u64) -> u64 = std::mem::transmute(0x2002F1650u64);
        f(core + 0x230, drd, 1, 9);
        let consumed = rd64(drd + 8) - dbuf;
        hlog!("DE item: {} bytes, world time {}, consumed {consumed}", de.len(), *(dbuf as *const u32));
        if consumed != de.len() as u64 {
            return Err(format!("the DE loader consumed {consumed} of {} bytes", de.len()));
        }
    }
    Ok(())
}

/// What the client's match UI owns and the mode rules reach from the world step: made before `on_match_ready`, so a
/// rule's match-ready hook finds them too. Each slot is filled only when empty.
///
/// * `game+0x5B8 +0x20`: the HUD's second movie owner. br's battery spawn-point walk (0x2002568B0, in on_match_ready)
///   passes `[[game+0x5D8]+0x208]` to `flash_movie::Invoke` (the host's scaleform stand-in does nothing). A null
///   owner whose +0x208 is a null movie, as the first owner `+0x10` (below, after on_match_ready).
/// * `game+0x10FE10`: the client's current game state; the client's match-state entry 0x2001179D0 stores `game+0x38`
///   there. The host stores the same value without running the state's enter (the UI). The game event handler
///   0x200119BE0 (ctp capture events) skips the sound when `[[game+0x10FE10]+0x248]` is null.
/// * `game+0x38 +0x248` (= game+0x280): the match state's UI sound collection, which the client fills from the match's
///   loaded resources; the host loads none. Kill events (slaughter) take an instance from its pool at `+0x2A8` without
///   a null check. A null collection whose pool has count 0 and capacity 0 takes the exe's own "pool full" branch: no
///   sound, nothing else. Refcount pinned high so no release frees it.
///
/// `HOST_UI_STANDINS=0` turns them off (diagnosis).
unsafe fn client_ui_standins(game: u64) {
    if std::env::var("HOST_UI_STANDINS").is_ok_and(|v| v == "0") {
        hlog!("client UI stand-ins: off (HOST_UI_STANDINS=0)");
        return;
    }
    let hud2 = game + 0x5B8 + 0x20;
    if rd64(hud2) == 0 {
        let owner = crate::nullcook::null_object(0x1000);
        *((owner + 0x208) as *mut u64) = crate::nullcook::null_object(0x1000);
        *(hud2 as *mut u64) = owner;
    }
    let sounds = game + 0x38 + 0x248;
    if rd64(sounds) == 0 {
        let coll = crate::nullcook::null_object(0x400);
        *((coll + 0x1B0) as *mut u32) = 1 << 30;
        *((coll + 0x2A8) as *mut u64) = mem::heap_alloc(0x100, true) as u64;
        *(sounds as *mut u64) = coll;
    }
    let state = game + 0x10FE10;
    if rd64(state) == 0 {
        *(state as *mut u64) = game + 0x38;
    }
    hlog!(
        "client UI stand-ins: [game+0x5D8] {:#x}, [game+0x280] {:#x}, [game+0x10FE10] {:#x} (= game+0x38)",
        rd64(hud2),
        rd64(sounds),
        rd64(state)
    );
}

/// The input world: player_controls_rule's match-ready hook registers with [game+0x10FD18]. The client builds it in
/// the game's initialise: static object 0x200BC8920, 0x2005B2C60(obj, window, 0), which opens the host's null
/// DirectInput. The host replays exactly those writes.
unsafe fn build_input_world(game: u64) {
    unsafe {
        if rd64(game + 0x10FD18) != 0 {
            return;
        }
        let w64 = |a: u64, v: u64| *(a as *mut u64) = v;
        w64(0x200BC8930, 0);
        w64(0x200BC8920, 0x2009CE7C8);
        w64(0x200BC89A8, 0);
        w64(0x200BC8928, 0x2009CE838);
        *(0x200BC89BC as *mut u16) = 0;
        w64(0x200BC8938, 0x200BC8950);
        w64(0x200BC8940, 0x200BC8950);
        w64(0x200BC8948, 0x200BC8990);
        w64(0x200BC8990, game + 0x10);
        w64(0x200BC89B0, 0x2009CE848);
        w64(0x200BC8998, 0);
        w64(0x200BC89A0, 0);
        *(0x200BC89B8 as *mut u32) = 2;
        let hwnd = rd64(rd64(game + 0x10FCF8) + 0xE18);
        call3(0x2005B2C60, 0x200BC8920, hwnd, 0);
        (*(0x200BC89C8 as *const AtomicU32)).swap(1, Ordering::SeqCst);
        *((game + 0x10FD18) as *mut u64) = rd64(0x200BC89C0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// used = start storage - end storage - end carried; bots, unchanged and refilled-above-start stacks are not listed.
    #[test]
    fn used_is_the_storage_lost_minus_what_is_still_carried() {
        let st = |uid, dict, storage, carried| StackNow { uid, dict, storage, carried };
        let start = vec![
            PlayerStacks { slot: 0, human: true, alive: false, profile: 0, stacks: vec![st(1, 81, 3000, 0), st(2, 760, 3000, 0), st(3, 5, 3000, 0)] },
            PlayerStacks { slot: 1, human: false, alive: false, profile: 0, stacks: vec![st(9, 81, 200, 0)] },
        ];
        let end = vec![
            // alive: 2795 stored + 60 carried (30 in the slot, 30 in magazines) of 3000 = 145 rounds used; 2 medkits
            // used; the grenade stack untouched (2999 stored + 1 carried)
            PlayerStacks { slot: 0, human: true, alive: true, profile: 0, stacks: vec![st(1, 81, 2795, 60), st(2, 760, 2995, 3), st(3, 5, 2999, 1)] },
            PlayerStacks { slot: 1, human: false, alive: true, profile: 0, stacks: vec![st(9, 81, 0, 100)] },
        ];
        let u = used_between(&start, &end);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].0, 0);
        let v: Vec<(u16, u32)> = u[0].1.iter().map(|x| (x.dict, x.used)).collect();
        assert_eq!(v, vec![(81, 145), (760, 2)]);
        // more left than at the start (a script gave units): nothing used, nothing negative
        let more = vec![PlayerStacks { slot: 0, human: true, alive: false, profile: 0, stacks: vec![st(1, 81, 3100, 0)] }];
        assert!(used_between(&start, &more).is_empty());
    }

    #[test]
    fn a_block_gives_its_detail_row() {
        let mut b = vec![0u8; BLOCK];
        let put16 = |b: &mut Vec<u8>, o: usize, v: u16| b[o..o + 2].copy_from_slice(&v.to_le_bytes());
        // kill (event 0) on profiles 0 and 2, headshot (15) on profile 8, event 73 saturates
        put16(&mut b, 0, 3);
        put16(&mut b, 4, 2);
        put16(&mut b, 15 * 0x12 + 8 * 2, 1);
        for p in 0..9 {
            put16(&mut b, 73 * 0x12 + p * 2, 0x4000);
        }
        // badge slot 5: state 2 once; slot 23: state 1 once (+0 enabled and +4 time are not counts)
        b[0x534 + 12 * 5] = 1;
        b[0x534 + 12 * 5 + 2] = 1;
        b[0x534 + 12 * 23 + 1] = 1;
        b[0x534 + 12 * 23 + 4] = 0xFF;
        b[0x9E8..0x9EC].copy_from_slice(&90_000u32.to_le_bytes());
        b[0x9EC..0x9F0].copy_from_slice(&7u32.to_le_bytes());
        b[0x9F0] = 1;
        put16(&mut b, 0xA08, 2);
        put16(&mut b, 0xA1C, 999); // score: not part of the row
        put16(&mut b, 0xA1E, 6);
        let d = detail_of_block(3, &b);
        assert_eq!(d.slot, 3);
        assert_eq!(d.events[0], 5);
        assert_eq!(d.events[15], 1);
        assert_eq!(d.events[73], 0xFFFF);
        assert_eq!(d.events.iter().filter(|&&v| v != 0).count(), 3);
        assert_eq!(d.badges[5], [0, 1, 0]);
        assert_eq!(d.badges[23], [1, 0, 0]);
        assert_eq!(d.badge_mask(), 1 << 5 | 1 << 23);
        assert_eq!((d.last_active_ms, d.inactive_ext_ms, d.inactive), (90_000, 7, true));
        assert_eq!((d.cur_streak, d.best_streak), (2, 6));
    }
}
