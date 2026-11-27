//! Hook for local matches (Training with bots, tutorial), written in `crates/local-match`.
//!
//! The lobby does the queue protocol; the trait supplies the content:
//! * `5B 01` (Play pressed): the lobby sends `85 02`, then `83 5B 01 <packed blob>` with [`LocalMatches::training`].
//! * a later `5B 02` (the first one per connection is the lobby-load handshake): `85 02`, then `83 5B 02 <blob>` with
//!   [`LocalMatches::tutorial`].
//! * `5B 03` (shooting range): `85 02`, then `83 5B 03 <blob>` with [`LocalMatches::shooting_range`]; the next `53` flush
//!   after `match_min_time` ends it with no reward (docs/notes/shooting-range.md).
//! * the first `53` event flush more than `match_min_time` after that: [`LocalMatches::finished`] gives the `97`
//!   results message (optional) and the rewards; the lobby credits them, pushes 97, 8B, 8C, 8E and `85 00`.

use accounts::Profile;
use game_data::GameData;

/// What a local match builder may look at.
pub struct MatchCtx<'a> {
    pub account_id: accounts::AccountId,
    pub account_name: &'a str,
    pub profile: &'a Profile,
    pub data: &'a GameData,
}

/// What the end of a local match brings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchOutcome {
    /// Payload of S->C `97` (after `id x`), None = no results screen.
    pub results97: Option<Vec<u8>>,
    pub exp: u32,
    pub silver: u32,
    /// Reputation points of the active faction.
    pub reputation: u32,
    /// `exp` includes the `played_first_game` event: the credit stores the account's claim (once per account)
    pub first_game: bool,
}

/// The reward of a finished local match: none (`game_data::rewards::LOCAL_MATCH_PAYS`, PROVEN by the client's own
/// "Rewards in this mode are not awarded" hint of the bots mode). The facts below are only built to keep the data formula
/// ready should the constant ever be switched on.
pub fn outcome_from_data(ctx: &MatchCtx, map: u32, mode: u32, won: bool, finished: bool) -> MatchOutcome {
    let mut facts = game_data::MatchFacts::local(map, mode, won, finished, ctx.profile.faction);
    facts.first_game = !ctx.profile.claimed(accounts::CLAIM_FIRST_GAME);
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64) ^ ctx.account_id.unsigned_abs().rotate_left(32);
    let r = if game_data::rewards::LOCAL_MATCH_PAYS {
        ctx.data.match_reward(&facts, game_data::rewards::LOCAL_MATCH_LEVEL, &mut game_data::SplitMix(seed))
    } else {
        Default::default()
    };
    facts.first_game &= game_data::rewards::LOCAL_MATCH_PAYS;
    // premium, the boosters the match carried (charged at its start: still the selected ones) and the worn items'
    // bonuses, per component (same rule as the results screen)
    let carried = crate::boosters::selected(ctx.data, ctx.profile);
    let pct = crate::boosters::reward_percent(ctx.data, ctx.profile, crate::premium::unix_now(), &carried);
    let up = |v: u32, pct: u32| v + (u64::from(v) * u64::from(pct) / 100) as u32;
    MatchOutcome { results97: None, exp: up(r.exp, pct[0]), silver: up(r.silver, pct[1]), reputation: up(r.rep, pct[2]), first_game: facts.first_game && finished }
}

pub trait LocalMatches: Send + Sync {
    /// The unpacked options blob of `83 5B 01` (the lobby packs it). `weather` is the client's field of `5B 01`.
    fn training(&self, ctx: &MatchCtx, map: u32, mode: u32, weather: u32) -> Vec<u8>;
    /// The unpacked options blob of the tutorial `83 5B 02`.
    fn tutorial(&self, ctx: &MatchCtx) -> Vec<u8>;
    /// The unpacked options blob of the shooting range `83 5B 03`. `weapon` is the item record the client sent with
    /// `5B 03` (the weapon to test: an owned item by uid, or a dictionary item by dict id), `profile` the trailing
    /// byte (the client's selected profile index, builder 0x2001970D0). The range pays nothing, so there is no
    /// `finished` counterpart. Default: the minimal blob.
    fn shooting_range(&self, _ctx: &MatchCtx, _weapon: &protocol::items::ItemRecord, _profile: u8) -> Vec<u8> {
        protocol::s2c::PlayBlob::minimal_blob()
    }
    /// A local match ended. `events` are the ids of the client's `53` flush; `won` = id 5 or 0x3D among them, `finished`
    /// = one of 5, 6, 0x3D, 0x3E (none = the player left early).
    fn finished(&self, ctx: &MatchCtx, map: u32, mode: u32, won: bool, finished: bool, events: &[u16]) -> MatchOutcome;
}

/// Default: the minimal path (118 zero bytes as options, no results screen, the rewards of `game_data::levels`).
pub struct MinimalMatches;

impl LocalMatches for MinimalMatches {
    fn training(&self, _: &MatchCtx, _: u32, _: u32, _: u32) -> Vec<u8> {
        protocol::s2c::PlayBlob::minimal_blob()
    }

    fn tutorial(&self, _: &MatchCtx) -> Vec<u8> {
        protocol::s2c::PlayBlob::minimal_blob()
    }

    fn finished(&self, ctx: &MatchCtx, map: u32, mode: u32, won: bool, finished: bool, _: &[u16]) -> MatchOutcome {
        outcome_from_data(ctx, map, mode, won, finished)
    }
}
