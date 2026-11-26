//! 0x97 match results (ordered). Readers: match_stats 0x200180AA0, player 0x200180980, player_result_full 0x200182350
//! (+0x200181C20), spent lists 0x200183010. PROVEN as a byte count: the real client showed the results screen with
//! messages of this layout; the meaning of most fields is LIKELY. The screen is display only: the account
//! changes only through pushes 8B / 8C / 8E.
//!
//! Nothing in the client traffic carries kills / deaths / score: the numbers the server puts
//! in are its own.

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Player row of the statistics: `str name, str clan, u64 public id, u16 score, u16 kills, u16 deaths,
/// u8 1, u8 0, u8 team, 4 x u8 0`. Bots use small ids 1..n (the screen treats ids below
/// `2 * players + 2` as bots, VictoryScreenStatsTeam).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResultPlayer {
    pub name: String,
    pub clan: String,
    pub id: u64,
    /// signed in the client (`movsx`, 0x200141457)
    pub score: i16,
    pub kills: u16,
    pub deaths: u16,
    pub flag: u8,
    pub b1: u8,
    pub team: u8,
    pub tail: [u8; 4],
}

impl ResultPlayer {
    /// A row with the usual fixed bytes (`flag` = the place, see [`set_places`]).
    pub fn new(name: &str, id: u64, score: i16, kills: u16, deaths: u16, team: u8) -> Self {
        ResultPlayer { name: name.into(), id, score, kills, deaths, flag: 1, team, ..Default::default() }
    }
}

/// Rows a `97` may hold: the handler's record array has 20 entries of 0xC0 bytes (0x20018B0F0), the
/// count byte is not checked.
pub const MAX_RESULT_PLAYERS: usize = 20;

/// Fill each row's place byte (`flag`, row +0xAE): the number of rows with a strictly higher score, across the whole
/// table (both teams), 0-based, equal scores share a place (PROVEN: the client's own bot places 0x20013E8E0 and
/// the movie's `DebugData`). The client uses it only to order the rows (`sort_order = 2 * place + (row != me)`,
/// 0x200141494), so the table must hold at most 20 rows and the receiver's own row.
pub fn set_places(players: &mut [ResultPlayer]) {
    let scores: Vec<i16> = players.iter().map(|p| p.score).collect();
    for p in players.iter_mut() {
        p.flag = scores.iter().filter(|s| **s > p.score).count().min(usize::from(u8::MAX)) as u8;
    }
}

impl Encode for ResultPlayer {
    fn encode(&self, w: &mut Writer) {
        w.str(&self.name).str(&self.clan).u64(self.id).u16(self.score as u16).u16(self.kills).u16(self.deaths);
        w.u8(self.flag).u8(self.b1).u8(self.team).bytes(&self.tail);
    }
}
impl Decode for ResultPlayer {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(ResultPlayer {
            name: r.str()?,
            clan: r.str()?,
            id: r.u64()?,
            score: r.u16()? as i16,
            kills: r.u16()?,
            deaths: r.u16()?,
            flag: r.u8()?,
            b1: r.u8()?,
            team: r.u8()?,
            tail: r.array()?,
        })
    }
}

/// Award entry (3 per message): `str64 name, u8 bool, 9 x f32`; a non-zero f32 is followed by `u32, u16, u16`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Award {
    pub name: String,
    pub flag: u8,
    pub values: [f32; 9],
    /// Extra triple for each non-zero value.
    pub extras: [(u32, u16, u16); 9],
}
impl Encode for Award {
    fn encode(&self, w: &mut Writer) {
        w.str_max(&self.name, 63).u8(self.flag);
        for i in 0..9 {
            w.f32(self.values[i]);
            if self.values[i] != 0.0 {
                let e = self.extras[i];
                w.u32(e.0).u16(e.1).u16(e.2);
            }
        }
    }
}
impl Decode for Award {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut a = Award { name: r.str()?, flag: r.u8()?, ..Default::default() };
        for i in 0..9 {
            a.values[i] = r.f32()?;
            if a.values[i] != 0.0 {
                a.extras[i] = (r.u32()?, r.u16()?, r.u16()?);
            }
        }
        Ok(a)
    }
}

/// One score component: `u32 silver (money), u16 reputation, u16 experience` (8 bytes). The human's rewards go
/// into category 10 ("game_mode"), component 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScoreComponent {
    pub money: u32,
    pub rep: u16,
    pub exp: u16,
}

/// Extra list entry after the f32 block: `u32, u16, u8, u8 k, k bytes` (sent empty).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtraEntry {
    pub a: u32,
    pub b: u16,
    pub c: u8,
    pub data: Vec<u8>,
}

/// The category of each score block in wire order (PROVEN, reader 0x200181D0A..0x2001820C6 stores the blocks at
/// R +0x248, +0x290, +0x368, +0x2D8, +0x320, +0x3B0 .. +0x518; fill_match_statistic 0x20014080E..0x200140B1A reads
/// premium_money at +0x2D8, boosters_money at +0x320, league_money at +0x368): league comes third on the wire, before
/// premium and boosters.
pub const SCORE_WIRE_ORDER: [usize; 11] = [0, 1, 4, 2, 3, 5, 6, 7, 8, 9, 10];

/// The human's result block (reader 0x200182350).
#[derive(Debug, Clone, PartialEq)]
pub struct ResultFull {
    /// u64 at the start, 0 for the human.
    pub my_id: u64,
    pub awards: [Award; 3],
    /// 11 categories (daily, premium_items, premium, boosters, league, targeted_gift, friend_in_squad,
    /// premium_players, camo_decal, hero, game_mode) x 9 components, indexed in this category order (the client's
    /// blocks R +0x248 .. +0x518). The wire carries them in [`SCORE_WIRE_ORDER`].
    pub score: [[ScoreComponent; 9]; 11],
    /// Account experience after the match (+8).
    pub exp_total: u32,
    /// Faction reputation after the match (+0xC).
    pub rep_total: u32,
    pub exp_delta: u16,
    pub rep_delta: u16,
    /// +0x14: the match's total silver. The in-match victory splash shows `silverReward = f14 - sum of the three tail
    /// lists' u32s` (0x2001ACCFC, string 0x200966578); 0 shows silver 0.
    pub f14: u32,
    pub f18: u16,
    /// Faction shown (+0x1A).
    pub faction: u8,
    /// 9 x u16 counts (+0x1C).
    pub counts: [u16; 9],
    /// 9 x u8 (+0x2E).
    pub bytes: [u8; 9],
    /// Mask of the 27 score-component kinds, 3 x u8 per set bit.
    pub kinds: Vec<(u8, [u8; 3])>,
    pub quest_money: u32,
    pub quest_exp: u32,
    pub quest_rep: u32,
    pub flag92e: u8,
    /// 27 + 18 + 27 + 3 f32 (9x3, 18, 9x3, 3), sent as zeros.
    pub floats: [f32; 75],
    pub extra: Vec<ExtraEntry>,
    /// League row (reader 0x200182AAC, `fill_match_statistic` 0x200141FF6..0x20014201F, PROVEN):
    /// +0x928 = league - 1 (the row's rank = 9F level count - this, the rank the league window shows).
    pub f928: u8,
    /// `stars_progress` (+0x92A): points inside the league before the match (x 0.25 = stars).
    pub stars_progress: u16,
    /// `stars` (+0x92C): `points_for_levelup` of the league (the bar's maximum, x 0.25).
    pub stars: u8,
    /// `stars_delta` (+0x92D, signed): points won / lost; 0 hides the row.
    pub stars_delta: i8,
}

impl Default for ResultFull {
    fn default() -> Self {
        ResultFull {
            my_id: 0,
            awards: Default::default(),
            score: [[ScoreComponent::default(); 9]; 11],
            exp_total: 0,
            rep_total: 0,
            exp_delta: 0,
            rep_delta: 0,
            f14: 0,
            f18: 0,
            faction: 1,
            counts: [0; 9],
            bytes: [0; 9],
            kinds: vec![],
            quest_money: 0,
            quest_exp: 0,
            quest_rep: 0,
            flag92e: 0,
            floats: [0.0; 75],
            extra: vec![],
            f928: 0,
            stars_progress: 0,
            stars: 0,
            stars_delta: 0,
        }
    }
}

impl ResultFull {
    /// The human's block with the rewards in category 10 / component 0.
    #[allow(clippy::too_many_arguments)]
    pub fn rewards(
        my_id: u64,
        exp_total: u32,
        exp_delta: u16,
        rep_total: u32,
        rep_delta: u16,
        faction: u8,
        money: u32,
        exp: u16,
        rep: u16,
    ) -> Self {
        // `money` is the whole silver of the match here (callers that split it set `f14` themselves)
        let mut r = ResultFull { my_id, exp_total, exp_delta, rep_total, rep_delta, faction, f14: money, ..Default::default() };
        r.score[10][0] = ScoreComponent { money, rep, exp };
        r
    }
}

impl Encode for ResultFull {
    fn encode(&self, w: &mut Writer) {
        w.u64(self.my_id);
        for a in &self.awards {
            a.encode(w);
        }
        for cat in SCORE_WIRE_ORDER.map(|k| &self.score[k]) {
            for c in cat {
                w.u32(c.money).u16(c.rep).u16(c.exp);
            }
        }
        w.u32(self.exp_total).u32(self.rep_total).u16(self.exp_delta).u16(self.rep_delta);
        w.u32(self.f14).u16(self.f18).u8(self.faction);
        for c in self.counts {
            w.u16(c);
        }
        w.bytes(&self.bytes);
        let mut mask = 0u32;
        let mut ks = self.kinds.clone();
        ks.sort_by_key(|k| k.0);
        for (bit, _) in &ks {
            if *bit >= 27 || mask & (1 << bit) != 0 {
                w.fail(Error::Invalid("score kind bit"));
            } else {
                mask |= 1 << bit;
            }
        }
        w.u32(mask);
        for (_, v) in &ks {
            w.bytes(v);
        }
        // money, reputation, experience (reader 0x200182515.., consumers 'quests_money' / 'quests_rep' / 'quests_exp')
        w.u32(self.quest_money).u32(self.quest_rep).u32(self.quest_exp).u8(self.flag92e);
        for f in self.floats {
            w.f32(f);
        }
        w.count_u8(self.extra.len(), "result extra list");
        for e in &self.extra {
            w.u32(e.a).u16(e.b).u8(e.c).count_u8(e.data.len(), "result extra data").bytes(&e.data);
        }
        w.u8(self.f928).u16(self.stars_progress).u8(self.stars).u8(self.stars_delta as u8);
    }
}

impl Decode for ResultFull {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut x = ResultFull { my_id: r.u64()?, ..Default::default() };
        for a in &mut x.awards {
            *a = Award::decode(r)?;
        }
        for k in SCORE_WIRE_ORDER {
            for c in &mut x.score[k] {
                *c = ScoreComponent { money: r.u32()?, rep: r.u16()?, exp: r.u16()? };
            }
        }
        x.exp_total = r.u32()?;
        x.rep_total = r.u32()?;
        x.exp_delta = r.u16()?;
        x.rep_delta = r.u16()?;
        x.f14 = r.u32()?;
        x.f18 = r.u16()?;
        x.faction = r.u8()?;
        for c in &mut x.counts {
            *c = r.u16()?;
        }
        x.bytes = r.array()?;
        let mask = r.u32()? & 0x7FF_FFFF;
        for bit in 0..27u8 {
            if mask & (1 << bit) != 0 {
                x.kinds.push((bit, r.array()?));
            }
        }
        x.quest_money = r.u32()?;
        x.quest_rep = r.u32()?;
        x.quest_exp = r.u32()?;
        x.flag92e = r.u8()?;
        for f in &mut x.floats {
            *f = r.f32()?;
        }
        let k = r.u8()? as usize;
        for _ in 0..k {
            let (a, b, c) = (r.u32()?, r.u16()?, r.u8()?);
            let n = r.u8()? as usize;
            x.extra.push(ExtraEntry { a, b, c, data: r.bytes(n)?.to_vec() });
        }
        x.f928 = r.u8()?;
        x.stars_progress = r.u16()?;
        x.stars = r.u8()?;
        x.stars_delta = r.u8()? as i8;
        Ok(x)
    }
}

/// 0x97 match results. Layout (reader 0x200180AA0, consumer `fill_match_statistic`):
/// `u32 match id, u32 map id, u32 0, u8 winner team (+0x1195: the UI shows VICTORY when the human's team equals it),
/// u8 team 0 score (+0x1196), u8 team 1 score (+0x1197) (drawn as "a:b": 0x20013F177 'team1_score' / 'team2_score'),
/// u32 team 0 param, u32 team 1 param (+0x118C / +0x1190: only DTO slots, 0 in the real server's sample), u8 mode, u8 b3
/// (3 = rated), 8 x str "", u8 player count, u8 match level (+0x1199: "MATCH LEVEL: x", 0x20013F9C8), players,
/// [`ResultFull`], 3 x {u8 n, n x (u32 money, u16 dict)}` (the spent lists, [`MatchResult::spent`]). The scores go
/// in the two u8 bytes; the u32 params are never drawn.
/// A zero result for one player is 442 bytes + the player's name bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    pub match_id: u32,
    pub map_id: u32,
    pub f8: u32,
    pub winner_team: u8,
    /// the score of team 0 as the screen shows it (a match score above 255 is shown as 255)
    pub team0_score: u8,
    pub team1_score: u8,
    /// unused by the screen (0)
    pub team0_param: u32,
    pub team1_param: u32,
    pub mode: u8,
    pub b3: u8,
    /// Eight strings, sent empty.
    pub texts: [String; 8],
    /// the match level shown on the victory plate
    pub match_level: u8,
    pub players: Vec<ResultPlayer>,
    pub full: ResultFull,
    /// The tail `spent_for_autobuy_money` (exe signature string 0x95AE80; reader 0x200183010): three lists
    /// `u8 n, n x {u32 money, u16 dict}` that `fill_match_statistic` hands to Flash as `spent_money_ammunition` (+0),
    /// `spent_money_pockets` (+0xA8), `spent_money_repair` (+0x270), each value shown negative; the lobby screen sums
    /// them into "Ammunition" / "Special items" / "Match costs" and the victory splash shows `full.f14` minus their sum.
    /// Index with [`SPENT_AMMO`] / [`SPENT_POCKETS`] / [`SPENT_REPAIR`].
    pub spent: [Vec<Spent>; 3],
}

/// One entry of a [`MatchResult::spent`] list: 6 bytes, money then dict (PROVEN order, reader 0x200183010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spent {
    pub money: u32,
    pub dict: u16,
}

/// "Ammunition" (`st_autobuy_money_ammo`)
pub const SPENT_AMMO: usize = 0;
/// "Special items" (`st_autobuy_money_consulabels`)
pub const SPENT_POCKETS: usize = 1;
/// "Match costs" (`st_autorepair_money`)
pub const SPENT_REPAIR: usize = 2;

impl Default for MatchResult {
    fn default() -> Self {
        MatchResult {
            match_id: 1,
            map_id: 0,
            f8: 0,
            winner_team: 0,
            team0_score: 0,
            team1_score: 0,
            team0_param: 0,
            team1_param: 0,
            mode: 0,
            b3: 0,
            texts: Default::default(),
            match_level: 0,
            players: vec![],
            full: ResultFull::default(),
            spent: Default::default(),
        }
    }
}

impl Encode for MatchResult {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.match_id).u32(self.map_id).u32(self.f8).u8(self.winner_team).u8(self.team0_score).u8(self.team1_score);
        w.u32(self.team0_param).u32(self.team1_param).u8(self.mode).u8(self.b3);
        for t in &self.texts {
            w.str(t);
        }
        w.count_u8(self.players.len(), "result players").u8(self.match_level);
        for p in &self.players {
            p.encode(w);
        }
        self.full.encode(w);
        for l in &self.spent {
            w.count_u8(l.len(), "spent list");
            for e in l {
                w.u32(e.money).u16(e.dict);
            }
        }
    }
}

impl Decode for MatchResult {
    fn decode(r: &mut Reader) -> Result<Self> {
        let mut m = MatchResult {
            match_id: r.u32()?,
            map_id: r.u32()?,
            f8: r.u32()?,
            winner_team: r.u8()?,
            team0_score: r.u8()?,
            team1_score: r.u8()?,
            team0_param: r.u32()?,
            team1_param: r.u32()?,
            mode: r.u8()?,
            b3: r.u8()?,
            ..Default::default()
        };
        for t in &mut m.texts {
            *t = r.str()?;
        }
        let n = r.u8()? as usize;
        m.match_level = r.u8()?;
        for _ in 0..n {
            m.players.push(ResultPlayer::decode(r)?);
        }
        m.full = ResultFull::decode(r)?;
        for l in &mut m.spent {
            let c = r.u8()? as usize;
            for _ in 0..c {
                l.push(Spent { money: r.u32()?, dict: r.u16()? });
            }
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the first money word of category `k` sits in an encoded `ResultFull`.
    fn money_offset(k: usize) -> usize {
        let zero = ResultFull::default().to_bytes().unwrap();
        let mut x = ResultFull::default();
        x.score[k][0].money = 0x0102_0304;
        let b = x.to_bytes().unwrap();
        zero.iter().zip(&b).position(|(a, c)| a != c).unwrap()
    }

    #[test]
    fn score_blocks_go_out_in_the_readers_order() {
        // the reader stores wire blocks 0..10 at daily, premium_items, league, premium, boosters, then 5..10
        let block = 9 * 8;
        let base = money_offset(0);
        for (wire, &k) in SCORE_WIRE_ORDER.iter().enumerate() {
            assert_eq!(money_offset(k), base + wire * block, "category {k}");
        }
        assert_eq!(money_offset(4), base + 2 * block);
        assert_eq!(money_offset(2), base + 3 * block);
        assert_eq!(money_offset(3), base + 4 * block);
        let mut x = ResultFull::default();
        for k in 0..11 {
            x.score[k][1] = ScoreComponent { money: k as u32 + 1, rep: k as u16 + 20, exp: k as u16 + 40 };
        }
        assert_eq!(ResultFull::from_bytes(&x.to_bytes().unwrap()).unwrap(), x);
    }
}
