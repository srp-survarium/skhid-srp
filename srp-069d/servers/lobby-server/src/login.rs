//! Sign-in (2E), the login pushes and the lobby-load handshake answers.

use accounts::{Profile, public_id};
use protocol::s2c::*;

use crate::build;
use crate::dispatch::Ctx;
use crate::peer::{Peer, Phase};
use crate::social;

/// A profile that never changed and owns no module gets the modules linked to its weapons (the module seed).
/// Each seeded module is green with drop grades from the green cells of the general table, as the new-player kit's
/// ([`crate::chests::give_rarity`]). Returns whether anything was added.
pub fn seed_modules(gd: &game_data::GameData, p: &mut Profile, r: &mut dyn game_data::Roll) -> bool {
    if p.version != 0 || p.items.iter().any(|i| gd.items.is_module(i.dict_id)) {
        return false;
    }
    let weapons: Vec<u16> = p.items.iter().filter(|i| gd.items.is_weapon(i.dict_id)).map(|i| i.dict_id).collect();
    let mut added = false;
    for w in weapons {
        for &m in gd.modules.seed_modules(w) {
            let uid = p.add_item(m, gd.items.amount_field(m, 1));
            crate::chests::give_rarity(gd, p, uid, r);
            added = true;
        }
    }
    added
}

/// 2E: `u32 session, u8 ff, u8 ff`. The session comes from the login server. The client sends the same 2E on its own
/// lobby reconnect (sessions.rs), also after a server restart (a persistent session table). A 2E the lobby cannot accept
/// (unknown or expired session, banned account) is refused: [`refuse`].
pub fn sign_in(cx: &mut Ctx, peer: &mut Peer, payload: &[u8]) {
    if peer.account.is_some() {
        return;
    }
    let Some(b) = payload.get(..4) else {
        log::warn!("{}: short 2E: refused", peer.addr);
        refuse(peer);
        return;
    };
    let session = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let Some(id) = cx.lobby.sessions.redeem(session) else {
        log::warn!("{}: 2E with unknown or expired session {session:#x}: refused (FD)", peer.addr);
        refuse(peer);
        return;
    };
    let account = match cx.lobby.store.account(id) {
        Ok(Some(a)) => a,
        other => {
            log::warn!("{}: session {session:#x} names account {id}: {other:?}: refused (FD)", peer.addr);
            refuse(peer);
            return;
        }
    };
    // a session outlives its sign-in (reconnects, restarts): a ban given since then refuses it like the login server
    // refuses a fresh sign-in (0C 02 / 0C 03 there, shown on the login screen the client ends on)
    match cx.lobby.store.ban_of(id) {
        Ok(None) => {}
        Ok(Some(ban)) => {
            log::info!("{}: 2E of account {id}: {:?} (reason {}): refused (FD)", peer.addr, ban.kind, ban.reason);
            refuse(peer);
            return;
        }
        Err(e) => {
            log::error!("{}: 2E of account {id}: ban lookup failed: {e}: refused (FD)", peer.addr);
            refuse(peer);
            return;
        }
    }
    {
        let gd = &cx.lobby.data;
        let res = cx.lobby.roll(|r| cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(seed_modules(gd, p, r))));
        if let Err(e) = res {
            log::error!("module seed for account {id} failed: {e:?}");
        }
    }
    peer.account = Some(id);
    cx.lobby.sessions.mark_online(id);
    peer.name = account.name.clone();
    peer.phase = Phase::SignedPending;
    if let Some(old) = cx.registry.insert(id, peer.addr) {
        cx.kick.push(old);
    }
    cx.registry.set_session(id, session);
    log::info!("{}: account {} (id {id}) signed in", peer.addr, account.name);
    // the client's own id (L+0x7A3960): the key of its own squad / room record
    peer.push(&S2c::SignedIn(SignedIn { account_id: public_id(id), ..SignedIn::default() }));
}

/// The lobby cannot sign this connection in: close it with `FD` (and every later datagram of the peer gets another one,
/// server.rs). The 0.69d sign-in channel knows only `80` (success, 0x200185770), so there is no refusal message; after
/// the sixth dropped reconnect the client opens the login menu (0x200117F90), where a fresh sign-in gets a new session
/// (or the ban text).
pub fn refuse(peer: &mut Peer) {
    peer.phase = Phase::Refused;
    peer.transport.instant_disconnect();
}

/// 9D `F` bit 10: the `5B 02` lobby-load handshake happened (0x20018F3FA sets it on the client too).
pub const HANDSHAKE_DONE: u32 = 0x400;

/// `9D` tutorial gate from the account's stored onboarding flags (docs/notes/onboarding.md). The client picks its
/// lobby step from `F` (0x200173470): nickname screen while bit 7 is clear, then choose-faction while bit 6 is clear,
/// else (or with bit 1 / 5) the full lobby. The faction byte is the active faction once one is chosen, 0 before
/// (`skip_tutorial` then picks the faction-pick step).
pub fn tutorial_gate(p: &Profile, a: u8, extra: u32) -> S2c {
    let chosen = p.onboarding & Profile::FACTION_CHOSEN != 0;
    let mut flags = p.onboarding | extra;
    if !p.nickname_chosen() {
        // bits 1 / 5 send the client to the full lobby before it looks at bit 7: an account that skipped the tutorial
        // (4A 02 is offered on the nickname screen) or finished it (4A 01 01) without a nickname would play under its
        // private login name. Until bit 7 is set the client gets the nickname screen; the stored flags stay.
        flags &= !(Profile::TUTORIAL_DONE | GATE_FULL_LOBBY);
    } else if !chosen && flags & Profile::TUTORIAL_DONE != 0 {
        // tutorial skipped (or played) but no faction yet, e.g. the player quit on the faction pick after 4A 02: bit 5 opens
        // the full lobby, and only bit 1 makes the client force the faction pick there (step 3, armed by
        // `choose_faction_message`, which the client runs itself when the 9D faction byte is 0, 0x2001C1CCB)
        flags |= GATE_FULL_LOBBY;
    }
    S2c::TutorialGate(TutorialGate { a, flags, g: 0, faction: if chosen { p.faction } else { 0 }, h: 0 })
}

/// `AC`: the play gate value 60 (>= 30 needed), the server-enabled mode mask and no `wait_count` rows
/// (docs/notes/server-list.md). The mask gates the PvP mode list (0x200177ED0): mode i is playable only with bit i set.
/// The mask offers exactly the modes the PvP queue can play: a check-box mode (`queue::QUEUE_MODE_BITS`) with a row in
/// the unrated pool that the match server serves (`online::playable_rows`: a world host's config projects); with the
/// test-only override `online_queue` only its mode. Everything else shows "The mode is unavailable" (`st_locked_by_server`).
/// The bits 7..9 (no check box) are never set: an allowed mode 9 (ah) would let the player untick every check box and
/// search with no queue mode (0x20017825A..0x2001782A1). Local bots matches do not read the mask (they play with 0).
pub fn play_gate(cx: &Ctx) -> S2c {
    S2c::PlayGate(PlayGate { value: 60, mode_mask: mode_mask(cx), waits: Vec::new() })
}

fn mode_mask(cx: &Ctx) -> u32 {
    let cfg = &cx.lobby.cfg;
    let online = cx.lobby.online.is_some();
    let rows = if online && cfg.online_queue.is_none() { crate::online::playable_rows(cx) } else { Vec::new() };
    mode_mask_of(cfg, online, &rows)
}

/// [`mode_mask`] over the rows the match server serves (`online`: a match server hook is attached).
fn mode_mask_of(cfg: &crate::LobbyConfig, online: bool, rows: &[crate::queue::Pick]) -> u32 {
    if !online {
        return 0;
    }
    match cfg.online_queue {
        Some((_, mode, _)) => {
            if mode < 32 {
                1u32 << mode
            } else {
                0
            }
        }
        None => rows.iter().filter(|r| r.mode < 32 && r.in_pool(false)).fold(0, |m, r| m | (1u32 << r.mode)) & crate::queue::QUEUE_MODE_BITS,
    }
}

/// 9D `F` bit 1: full lobby like bit 5, and the forced faction pick (step 3, no tutorial mode) while the 9D faction byte
/// is 0 (PROVEN: callback `choose_faction_message` 0x2002CA880 enters step 3 only with this bit). Sent, never stored.
const GATE_FULL_LOBBY: u32 = 0x02;

/// `96`: the faction unlock mask (bit f-1 = faction f unlocked). Sent at sign-in and answers `3A 00`
/// (docs/notes/requests-34-39-3a.md). No file states a rule that locks a faction (the client only shows the
/// `st_faction_*_blocked_text` of a clear bit), so every faction of the four is open: one source for both senders.
pub fn faction_mask_push() -> S2c {
    S2c::FactionMask(FactionMask { mask: 0x0F })
}

/// `94`: premium store data, the daily calendar record and (when one exists) the contract record. Sent at sign-in and
/// answers `39 00` (docs/notes/requests-34-39-3a.md).
pub fn notices_push(cx: &Ctx, p: &Profile) -> S2c {
    let gd = &cx.lobby.data;
    S2c::Notices(crate::login_calendar::with_calendar(cx, p, crate::premium::notices(gd, p, crate::premium::unix_now())))
}

/// The loading messages, in the order the real client accepted: 91 first (the pocket mask must be there before an 87
/// names a pocket item), then 8F, 8B, 9D, 89, 87, B0 06, 85, 9F, 8A, 89, 96, 8E, 8D, 95, 94, 8C, 9C, 9E, AC, B8.
///
/// 8A comes before 96 / 8E (PROVEN): the client handles each message in arrival order (0x200175590), and the 8E faction
/// levels lock only the shop items the 8A has already made; with 8E first the shop shows no level locks.
///
/// The 89 goes out again right after the 8A (PROVEN): the Paint window's camouflage / decal list is made by the 8A
/// (0x2001757A8), and only an 89 (or an added item) marks the owned ones and hands the list to the lobby movie
/// (0x200249FE0 / 0x200249D60); the first 89, before the 8A, finds no list yet (docs/notes/decals.md).
pub fn login_pushes(cx: &Ctx, peer: &Peer, p: &Profile) -> Vec<S2c> {
    let gd = &cx.lobby.data;
    let ignored = social::ignored_rows(cx, peer);
    let mut v = vec![
        S2c::SpecificsOpened(SpecificsOpened { mask: crate::actions::specifics_mask(cx, peer.account.unwrap_or_default()) }),
        S2c::Name(NameMsg { flag: 0, name: peer.name.clone() }),
        S2c::Money(build::money_8b(p)),
        tutorial_gate(p, 0, 0),
        S2c::Inventory(build::inventory_89(gd, p)),
        S2c::Account(build::account_87(gd, p, &peer.name)),
        S2c::Messaging(Messaging::Ignored(ignored)),
        crate::clans::state_msg(cx, peer.account.unwrap_or_default()),
        S2c::QueueState(QueueState { state: 0 }),
        // league: reward groups, level table, season state (league.rs)
        crate::league::login_push(cx, peer.account.unwrap_or_default(), p),
        // the shop items first, then the faction levels that lock them (see above)
        S2c::Prices(build::prices_8a(gd)),
        // again after the 8A: fills the Paint window's camouflage / decal list (see above)
        S2c::Inventory(build::inventory_89(gd, p)),
        faction_mask_push(),
        S2c::Factions(crate::faction_battle::factions_8e(cx, p, crate::premium::unix_now())),
        S2c::ServicePrices(build::service_prices_8d(gd)),
        crate::actions::shop_push(cx),
        notices_push(cx, p),
        S2c::Level(build::level_8c(gd, p)),
        S2c::Flags9C(Flags9C([0; 4])),
        crate::servers::list_9e(&cx.lobby.cfg.game_servers),
        play_gate(cx),
        S2c::EventIds(EventIds { tag: 0, ids: vec![5, 6, 0x3D, 0x3E] }),
        crate::achievements::login_push(gd, p),
    ];
    if cx.lobby.cfg.daily_quests {
        v.push(crate::quests::login_push(cx, p));
    }
    if let Some(m) = crate::hunt::login_push(cx, p) {
        v.push(m);
    }
    // hero of the day: shows the lobby entry (A6)
    if let Some(m) = crate::hero::config_push(cx) {
        v.push(m);
    }
    v
}

/// After the client acked the 80: send the loading messages.
pub fn push_login(cx: &mut Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    let p = match cx.lobby.store.load_profile(id) {
        Ok(p) => p,
        Err(e) => {
            log::error!("{}: loading profile of account {id}: {e}", peer.addr);
            return;
        }
    };
    crate::achievements::sync_on_login(cx, id);
    crate::quests::sync_on_login(cx, id);
    crate::hunt::sync_on_login(cx, id);
    crate::league::sync_on_login(cx, id);
    crate::skins::sync_on_login(cx, id);
    let p = cx.lobby.store.load_profile(id).unwrap_or(p);
    for m in login_pushes(cx, peer, &p) {
        peer.push(&m);
    }
    peer.rejoin_check = cx.registry.online.match_of(id).is_some();
    peer.quest_expires = p.daily.expires;
    peer.phase = Phase::Live;
    social::announce(cx, id, social::ST_LOBBY);
    crate::clans::presence_changed(cx, id);
    crate::clans::pending_invites(cx, peer);
    crate::social::pending_friend_requests(cx, peer);
}

/// First `5B 02` of a connection (the lobby-load handshake): `83 5B 02 <size 0>` (a valid blob would be parsed and
/// stall the lobby), then 9D with A=1 (picks the full-UI step again), 9B craft chances, 91 again (the pocket lists
/// fill only now that the lobby movie is up), the friend list and the welcome line.
pub fn handshake(cx: &mut Ctx, peer: &mut Peer) {
    peer.reply_raw(0x5B, &[2, 0, 0, 0, 0]);
    peer.handshake_5b = true;
    if let Some(p) = peer.account.and_then(|id| cx.lobby.store.load_profile(id).ok()) {
        // bit 10 = the 5B handshake is done; A = 1 makes the client pick its step again (nickname / faction / full lobby)
        peer.push(&tutorial_gate(&p, 1, HANDSHAKE_DONE));
    }
    let (regular, gold) = game_data::craft::CRAFT_CHANCE;
    peer.push(&S2c::CraftChances(CraftChances { regular, gold }));
    peer.push(&S2c::SpecificsOpened(SpecificsOpened { mask: crate::actions::specifics_mask(cx, peer.account.unwrap_or_default()) }));
    crate::presets::push_list(cx, peer);
    // the squad state, always (AE 00000000 = no squad): the client's squad object stays null until the first AE and the
    // PvE window reads it unchecked (crash in PVEMatchSettingsWindow.updateSquadInfo; AE is the only filler,
    // 0x20018F125 -> fill_squad_info 0x2001C4820)
    if let Some(me) = peer.account {
        peer.push(&crate::squads::ae_for(&cx.lobby.store, &cx.lobby.data, &cx.registry, me));
    }
    social::after_handshake(cx, peer);
}

/// Public id the chat and friend messages use for this peer.
pub fn public_id_of(peer: &Peer) -> u64 {
    peer.account.map(public_id).unwrap_or(0)
}

/// `39 00`: the current 94 again.
pub fn refresh_notices(cx: &Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    if let Ok(p) = cx.lobby.store.load_profile(id) {
        peer.push(&notices_push(cx, &p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ac_mode_mask_offers_the_modes_the_queue_can_play() {
        let r = |map, mode, variation, weather| crate::queue::Pick { map, mode, variation, weather };
        // the test world host's PvP projects: level_01..05 day tdm, level_05 day ah / br / ctp / slr_vn
        let host: Vec<crate::queue::Pick> =
            (1..=5).map(|m| r(m, 1, 0, 1)).chain([r(5, 9, 0, 1), r(5, 0, 0, 1), r(5, 2, 0, 1), r(5, 6, 8, 1)]).collect();
        let mut cfg = crate::LobbyConfig::default();
        assert_eq!(mode_mask_of(&cfg, false, &host), 0, "no match server: no PvP mode");
        assert_eq!(mode_mask_of(&cfg, true, &host), 0b100_0111, "br, tdm, ctp, slaughter; no ah (bit 9: no check box)");
        assert_eq!(mode_mask_of(&cfg, true, &[r(1, 0, 0, 1), r(8, 2, 0, 1)]), 0, "Rudnya battery and Cologne research are out of the pool");
        assert_eq!(mode_mask_of(&cfg, true, &[]), 0, "nothing served");
        cfg.online_queue = Some((7, 4, 0));
        assert_eq!(mode_mask_of(&cfg, true, &host), 1 << 4, "the test override offers its mode only");
        cfg.online_queue = Some((7, 40, 0));
        assert_eq!(mode_mask_of(&cfg, true, &host), 0, "no bit for a mode past 31");
        cfg.online_queue = Some((7, 4, 0));
        assert_eq!(mode_mask_of(&cfg, false, &host), 0, "a pinned queue without a match server");
    }

    /// The data's Search and Destroy rows (the four layouts of the client's custom-match list) open the S&D box: a match
    /// server that serves every row of the data sets bit 5 next to br, tdm, ctp and slaughter.
    #[test]
    fn search_and_destroy_rows_set_bit_5_of_the_ac_mask() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let rows: Vec<crate::queue::Pick> = gd.matches.modes().iter().map(crate::queue::Pick::of).collect();
        let sad: Vec<(u32, u32)> = rows.iter().filter(|r| r.mode == crate::queue::SAD_MODE).map(|r| (r.map, r.variation)).collect();
        assert_eq!(sad, vec![(2, 7), (3, 5), (3, 6), (12, 5)]);
        let cfg = crate::LobbyConfig::default();
        assert_eq!(mode_mask_of(&cfg, true, &rows), 0b110_1111, "br, tdm, ctp, gun race, sad, slaughter");
        let only_sad: Vec<crate::queue::Pick> = rows.iter().filter(|r| r.mode == crate::queue::SAD_MODE).copied().collect();
        assert_eq!(mode_mask_of(&cfg, true, &only_sad), 1 << 5);
        let no_sad: Vec<crate::queue::Pick> = rows.iter().filter(|r| r.mode != crate::queue::SAD_MODE).copied().collect();
        assert_eq!(mode_mask_of(&cfg, true, &no_sad) & (1 << 5), 0, "no S&D row served: the box stays locked");
    }

    /// The module seed of an old version-0 profile without modules gives green modules (`(312, 1)`, green cell).
    #[test]
    fn seeded_modules_are_green() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        assert!(seed_modules(&gd, &mut p, &mut game_data::SplitMix(3)));
        let mods: Vec<_> = p.items.iter().filter(|i| gd.items.is_module(i.dict_id)).collect();
        assert!(!mods.is_empty());
        for it in mods {
            let m = &p.mods[&it.uid];
            assert_eq!(m[0], (312, 1), "{} {m:?}", it.dict_id);
            let (a, b) = gd.module_drops.drop_groups(it.dict_id);
            if let (Some(_), Some(_)) = (a, b) {
                assert_eq!(gd.module_drops.cell_value(m[1].1 as usize, m[2].1 as usize), 0, "{} {m:?}", it.dict_id);
            } else if a.is_some() {
                assert_eq!(gd.module_drops.cell_value(m[1].1 as usize, 9), 0, "{} {m:?}", it.dict_id);
            }
        }
    }
}
