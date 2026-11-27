//! The kill log (world host protocol 3, `Kills` frame).
//!
//! Every death of the world goes through the statistics handler's vtable slot 3, `0x2001FB490`, called by the victim's
//! death handler `0x2003046B0` with about 25 arguments; the client's own core (0x20033DCF0) keeps only counts of them.
//! A hardware execution breakpoint at the slot's entry (`bp.rs`, perf_event, nothing written to the exe) copies the
//! arguments into a fixed ring from the signal handler; the world thread drains it after each step.
//!
//! Arguments: 1 the statistics object, 2 time, 3 killer (0xFF none), 4 killing item dict (`[[item]+0x4C8]+0x304`).
//! Stack arguments are named by their offset in the caller's outgoing argument area (`+0x20` = argument 5):
//! `+0x20` item uid, `+0x28` category_id of the killing item (dictionary entry +0x307), `+0x38` killer +0x6D8, `+0x40`
//! killer profile, `+0x48` killer health, `+0x50` damage type, `+0x58` victim, `+0x68` victim +0x6D8, `+0x70` victim
//! profile, `+0x78` hit zone "brain" / "face", `+0x80` handler byte, `+0x88` inflictor present, `+0x90` hit zone
//! "health", `+0x98` inflictor +0x69, `+0xA0` / `+0xA8` victim states, `+0xB0` 0x200304F30, `+0xB8` killer +0x748,
//! `+0xC0` victim item +0x44C4, `+0xC8` 0x2006D6A20(victim item), `+0xD0` victim anomaly test, `+0xD8` victim item
//! dict. The victim's damage-by-attacker rows (`B+0x678`, f32[20][9]) are read at the same moment.
//!
//! Three facts the slot does not receive are worked out at the same moment from the death handler's frame (killing
//! item, killer object, victim object), each the way the exe itself computes it ([`kill_flag::HIPFIRE`],
//! [`kill_flag::LAST_ROUND`], [`kill_flag::VICTIM_ANOMALY`]).
//!
//! Rollback (client mode): a late input at time L <= present makes the world simulate again from L, so a death can be
//! caught twice or be undone. [`KillLog`] keys the deaths by (time, victim) and drops every death at or after L before
//! the step's own catches are added (`Step::late_from`). The relay keeps the log only when it agrees with the world's
//! event rows (`match_server::host::checked_kills`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use world_host_proto::{kill_flag, KillRecord, MAX_PLAYERS};

/// The statistics handler's vtable slot 3 (0x2001FB490): one call per death.
pub const KILL_ENTRY: u64 = 0x2001FB490;
const RING: usize = 8192;
/// The loaded exe image (`ImageBase`, `SizeOfImage` of survarium.exe 0.69d): RTTI reads stay inside it.
const IMAGE: std::ops::Range<u64> = 0x2_0000_0000..0x2_15E3_8000;
/// Weapon categories of firearms (`category_id` 8 assault, 13 sniper, 14 shotgun, 15 SMG, 16 machine gun, 17 handgun:
/// the weapon-class achievements 216..221).
const FIREARMS: [u8; 6] = [8, 13, 14, 15, 16, 17];

/// The vtable of the object the victim was using (`victim+0x290`) per ring record (0 = none): the class name is read
/// from its RTTI in [`drain`], outside the signal handler.
static mut USED_VT: [u64; RING] = [0; RING];

static mut RAW: [KillRecord; RING] = [KillRecord {
    t: 0,
    killer: 0,
    victim: 0,
    killer_profile: 0,
    victim_profile: 0,
    weapon_dict: 0,
    weapon_category: 0,
    killer_hp: 0,
    damage_type: 0,
    item_uid: 0,
    victim_weapon_dict: 0,
    arg80: 0,
    flags: 0,
    damagers: 0,
    assists: None,
}; RING];
/// records written by the handler / taken by [`drain`]
static WRITTEN: AtomicUsize = AtomicUsize::new(0);
static TAKEN: AtomicUsize = AtomicUsize::new(0);
static LOST: AtomicUsize = AtomicUsize::new(0);

/// Arm the breakpoint on every thread of the process (the match child), unless `HOST_KILL_LOG=0`.
pub fn arm() -> bool {
    if std::env::var("HOST_KILL_LOG").is_ok_and(|v| v == "0") {
        crate::hlog!("kill log: off (HOST_KILL_LOG=0)");
        return false;
    }
    crate::bp::arm(&[KILL_ENTRY]);
    true
}

/// The breakpoint handler (signal context: no allocation, no locks). `g` = the thread's registers at the entry.
pub unsafe fn on_trap(g: &[libc::greg_t]) {
    let rsp = g[libc::REG_RSP as usize] as u64 + 8;
    let b = |o: u64| *((rsp + o) as *const u8);
    let w = |o: u64| *((rsp + o) as *const u16);
    let d = |o: u64| *((rsp + o) as *const u32);
    let victim = b(0x58);
    let mut flags = 0u16;
    for (o, f) in [
        (0x78, kill_flag::HEAD_ZONE),
        (0x88, kill_flag::INFLICTOR),
        (0x90, kill_flag::HEALTH_ZONE),
        (0x98, kill_flag::INFLICTOR_69),
        (0xA0, kill_flag::ARG_A0),
        (0xA8, kill_flag::ARG_A8),
        (0xB0, kill_flag::ARG_B0),
        (0xB8, kill_flag::KILLER_ALIVE),
        (0xC0, kill_flag::VICTIM_C0),
        (0xC8, kill_flag::VICTIM_C8),
        (0xD0, kill_flag::VICTIM_D0),
        (0x38, kill_flag::KILLER_6D8),
        (0x68, kill_flag::VICTIM_6D8),
    ] {
        if b(o) != 0 {
            flags |= f;
        }
    }
    // the victim's damage received from every attacker j (any profile), before the core clears it at the death
    let mut damagers = 0u32;
    let s = g[libc::REG_RCX as usize] as u64;
    if usize::from(victim) < MAX_PLAYERS && s != 0 {
        let rows = s + 0x30 + u64::from(victim) * 0xA24 + 0x678;
        for j in 0..MAX_PLAYERS as u64 {
            if (0..9u64).any(|p| *((rows + (j * 9 + p) * 4) as *const f32) > 0.0) {
                damagers |= 1 << j;
            }
        }
    }
    let killer = g[libc::REG_R8 as usize] as u8;
    let assists = if s != 0 && usize::from(victim) < MAX_PLAYERS { Some(assists_of(s, killer, victim)) } else { None };
    let damage_type = d(0x50);
    let category = b(0x28);
    // the death handler's frame (its base is unchanged at the slot's entry): killing item at +0x58, killer object at
    // -0x70; the victim object is in a callee-saved register
    let rbp = g[libc::REG_RBP as usize] as u64;
    let item = *((rbp + 0x58) as *const u64);
    let killer_obj = *(rbp.wrapping_sub(0x70) as *const u64);
    let victim_obj = g[libc::REG_R15 as usize] as u64;
    // a firearm kill: the killing item is the weapon_core's item base (weapon_core ctor 0x2006CBD00: base at +0x30)
    if killer != 0xFF && killer != victim && killer_obj != 0 && item != 0 && b(0x80) == 0 && FIREARMS.contains(&category) {
        let w = item - 0x30;
        let f = |o: u64| *((w + o) as *const f32);
        // hipfire: the kill feed's hipfire_icon (0x200304AE3): the aim-in progress (0x2006CC8A0) below 0.5
        let a = f(0x44EC);
        let x = if 0.5 > a { a } else { 1.0 - a };
        let r = f(0x44D4) - x;
        let r = if 0.0 >= r { 0.0 } else { r.min(1.0) };
        if b(0x88) != 0 && 0.5 > r {
            flags |= kill_flag::HIPFIRE;
        }
        // last round: the magazine (+0x44BA) and the chamber (+0x44CD) are empty and the killing shot is the killer's
        // latest one (shot id = killer << 11 | per-player counter, 0x2003BDAE0; counter at [mgr+0x208]+p*0x820+0x81C,
        // mgr = weapon +0x17D0); the shot that fired the last round is the one the exe flags at 0x2002FFF48
        let rounds = u32::from(*((w + 0x44BA) as *const u8)) + u32::from(*((w + 0x44CD) as *const u8) != 0);
        let mgr = *((w + 0x17D0) as *const u64);
        if rounds == 0 && mgr != 0 && usize::from(killer) < MAX_PLAYERS {
            let tbl = *((mgr + 0x208) as *const u64);
            if tbl != 0 {
                let counter = *((tbl + u64::from(killer) * 0x820 + 0x81C) as *const u16);
                let latest = (u32::from(killer) << 11) | (u32::from(counter.wrapping_sub(1)) & 0x7FF);
                if (damage_type >> 8) & 0xFFFF == latest {
                    flags |= kill_flag::LAST_ROUND;
                }
            }
        }
    }
    // the object the victim was using (`+0x290`, its vt[0x30] gives the `+0xD0` argument): its class is resolved in drain()
    let used = if victim_obj != 0 { *((victim_obj + 0x290) as *const u64) } else { 0 };
    let used_vt = if used != 0 { *(used as *const u64) } else { 0 };
    let rec = KillRecord {
        t: g[libc::REG_RDX as usize] as u32,
        killer,
        victim,
        killer_profile: b(0x40),
        victim_profile: b(0x70),
        weapon_dict: g[libc::REG_R9 as usize] as u16,
        weapon_category: category,
        killer_hp: b(0x48),
        damage_type,
        item_uid: d(0x20),
        victim_weapon_dict: w(0xD8),
        // bit 0 the handler's melee byte, bit 1 its ricochet byte (death handler frame +0x78: hit +0xAD && +0xAE, the
        // kill feed's ricochet_icon), bit 2 its penetration byte (frame +0x70: hit +0xAD && !+0xAE, penetration_icon)
        arg80: u8::from(b(0x80) != 0) | u8::from(*((rbp + 0x78) as *const u8) != 0) << 1 | u8::from(*((rbp + 0x70) as *const u8) != 0) << 2,
        flags,
        damagers,
        assists,
    };
    let i = WRITTEN.load(Ordering::SeqCst);
    if i - TAKEN.load(Ordering::SeqCst) >= RING {
        LOST.fetch_add(1, Ordering::SeqCst);
        return;
    }
    (*std::ptr::addr_of_mut!(RAW))[i % RING] = rec;
    (*std::ptr::addr_of_mut!(USED_VT))[i % RING] = used_vt;
    WRITTEN.store(i + 1, Ordering::SeqCst);
}

/// The assists of one death by the core's rule 0x20033E660 (called by the kill handler 0x20033DCF0 with the killer's and
/// the victim's blocks, after slot 3, so the victim's damage rows are still whole here): `total` = the damage the
/// killer's team (the killer included) dealt to the victim, `f32 B(v)+0x678+(j*9+p)*4` over each teammate j and his
/// profiles `p < B(j)+0xA15` (the core addresses it as `S+8+j*0xA24+0xA3D`); |total| <= 1e-6 (0x2009E5BA4) = no assist; else each teammate j != killer gets one when for
/// some profile `s < B(j)+0xA15` his share `row / total` is not below 0.1 (0x2009E5C28). Team of slot j = the byte
/// `[[S+0xEBA8]+8] + j*0x300 + 0x2EC`, players = `S+0xEBE2`; only for an enemy kill (the core's branch). Signal
/// context: plain reads only.
unsafe fn assists_of(s: u64, killer: u8, victim: u8) -> u32 {
    let n = u64::from(*((s + 0xEBE2) as *const u8)).min(MAX_PLAYERS as u64);
    if killer == 0xFF || u64::from(killer) >= n || u64::from(victim) >= n || killer == victim {
        return 0;
    }
    let recs = *((s + 0xEBA8) as *const u64);
    if recs == 0 {
        return 0;
    }
    let tbl = *((recs + 8) as *const u64);
    if tbl == 0 {
        return 0;
    }
    let team = |j: u64| *((tbl + j * 0x300 + 0x2EC) as *const u8);
    let block = |j: u64| s + 0x30 + j * 0xA24;
    let rows = block(u64::from(victim)) + 0x678;
    let row = |j: u64, p: u64| *((rows + (j * 9 + p) * 4) as *const f32);
    let tk = team(u64::from(killer));
    if tk == team(u64::from(victim)) {
        return 0; // a team kill: the core takes its team-kill branch (0x20033E4A4), no assist
    }
    let mut total = 0f32;
    for j in (0..n).filter(|&j| team(j) == tk) {
        for p in 0..u64::from(*((block(j) + 0xA15) as *const u8)).min(9) {
            total += row(j, p);
        }
    }
    if 1e-6 >= total.abs() {
        return 0;
    }
    let mut out = 0u32;
    for j in (0..n).filter(|&j| j != u64::from(killer) && team(j) == tk) {
        for p in 0..u64::from(*((block(j) + 0xA15) as *const u8)).min(9) {
            if !(0.1 > row(j, p) / total) {
                out |= 1 << j;
            }
        }
    }
    out
}

/// The deaths caught since the last call (world thread, between steps). Err = the ring overflowed (deaths were lost:
/// the log is then incomplete and must not be sent).
pub fn drain() -> Result<Vec<KillRecord>, usize> {
    let (from, to) = (TAKEN.load(Ordering::SeqCst), WRITTEN.load(Ordering::SeqCst));
    let v = (from..to)
        .map(|i| unsafe {
            let mut r = (*std::ptr::addr_of!(RAW))[i % RING];
            if class_has((*std::ptr::addr_of!(USED_VT))[i % RING], b"anomaly") {
                r.flags |= kill_flag::VICTIM_ANOMALY;
            }
            r
        })
        .collect();
    TAKEN.store(to, Ordering::SeqCst);
    match LOST.load(Ordering::SeqCst) {
        0 => Ok(v),
        n => Err(n),
    }
}

/// Does the RTTI class name of the object with vtable `vt` contain `part`? (MSVC: the complete object locator at
/// `vt-8`, its type descriptor rva at +12, the decorated name at descriptor +16.) False for anything outside the image.
fn class_has(vt: u64, part: &[u8]) -> bool {
    if vt < IMAGE.start + 8 || !IMAGE.contains(&vt) {
        return false;
    }
    unsafe {
        let col = *((vt - 8) as *const u64);
        if !IMAGE.contains(&col) || !IMAGE.contains(&(col + 16)) {
            return false;
        }
        let name = IMAGE.start + u64::from(*((col + 12) as *const u32)) + 16;
        if !IMAGE.contains(&(name + 128)) {
            return false;
        }
        let bytes = std::slice::from_raw_parts(name as *const u8, 128);
        let n = bytes.iter().position(|&c| c == 0).unwrap_or(128);
        bytes[..n].windows(part.len()).any(|w| w == part)
    }
}

/// The match's deaths, keyed by (time, victim), with the rollback rule (module docs).
#[derive(Debug, Default)]
pub struct KillLog {
    by: BTreeMap<(u32, u8), KillRecord>,
    /// deaths were lost (ring overflow): the log is not sent
    pub broken: bool,
}

impl KillLog {
    /// A late input at `l` (<= the present) was inserted: every death at or after `l` will be simulated again.
    pub fn rollback(&mut self, l: u32) {
        self.by.retain(|&(t, _), _| t < l);
    }

    pub fn add(&mut self, recs: impl IntoIterator<Item = KillRecord>) {
        for r in recs {
            self.by.insert((r.t, r.victim), r);
        }
    }

    pub fn len(&self) -> usize {
        self.by.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by.is_empty()
    }

    /// Every death at or before `t`, in time order.
    pub fn upto(&self, t: u32) -> Vec<KillRecord> {
        self.by.values().filter(|r| r.t <= t).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(t: u32, victim: u8, killer: u8) -> KillRecord {
        KillRecord { t, victim, killer, ..Default::default() }
    }

    #[test]
    fn a_rollback_drops_the_deaths_it_simulates_again() {
        let mut log = KillLog::default();
        log.add([k(1000, 1, 0), k(2000, 2, 0), k(2500, 3, 0)]);
        // a late input at 2000: the deaths from 2000 on are simulated again; the re-simulation kills 2 again (same
        // time and victim: one record) and 3 later (2600), so the 2500 one is gone
        log.rollback(2000);
        log.add([k(2000, 2, 0), k(2600, 3, 4)]);
        assert_eq!(log.upto(u32::MAX).iter().map(|r| (r.t, r.victim, r.killer)).collect::<Vec<_>>(), vec![(1000, 1, 0), (2000, 2, 0), (2600, 3, 4)]);
        // a second catch of the same death replaces it (one record per (time, victim))
        log.add([k(2600, 3, 5)]);
        assert_eq!(log.len(), 3);
        assert_eq!(log.upto(2000).len(), 2);
    }
}
