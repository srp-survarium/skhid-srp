//! Custom matches (rooms), lobby side: S->C 0x98 room state, 0xA0 ended, 0xA1 aborted, and the C->S `62 xx`
//! requests (social area; addresses are exe VAs; docs/notes/social.md).
//!
//! 0x98 (block 0x20018EE03, reader 0x20018B7F0, PROVEN field order; names from the UI function 0x2001C86C0 that
//! builds `root.custom_match_fill_state`, PROVEN keys, offsets matched to the stores):
//! ```text
//! u8 n members (<= the client's vector)
//! n x { str128 name, str32 clan abbr, u64 public id, u32 clan id,
//!       u8 team (PROVEN 0x20018BA00 -> 0x2001C8DA3 `role` key, CustomMatchPlayer.as: 0 team A, 1 team B, 2 unassigned; `62 1D` sends the role),
//!       u8 commander (== 1: host), u8 ready, u8 premium, u8 tokens silver, u8 tokens gold }
//! u32 room id (0 = not in a room), u32 clan A id, u32 clan B id, str100 clan A name, str100 clan B name,
//! str256 loading screen,
//! u8 type (1 = clan match, see `custom_match_type`), u8 map, u8 mode, u8 weather, u8 region, u8 voice chat,
//! u8 ai ignore players, u8 tokens allocated, u8 tokens required, u8 ai slots A, u8 ai slots B,
//! u8 player slots A, u8 player slots B, u32 token cost gold, u32 token cost silver,
//! u8 min ai, u8 max ai, u8 min players, u8 max players, u8 x, u8 y   (x, y are read and stored at 0x79F72D/E; no reader found)
//! ```
//! 0xA0: no payload (the client calls `root.custom_match_ended`). 0xA1: `str(2048)` reason text
//! (`root.custom_match_aborted`). Both reset the client's whole room record (0x20018EF4F / 0x20018EEB7: the 0x3208-byte
//! record at +0x79C528 is rebuilt empty) and hide the room window and the lobby mini panel (`setData(null)`,
//! `onHideCustomMatchWindow(null)`): they are how a player leaves a room. A 98 with room id 0 does NOT: the exe builds
//! `custom_match_fill_state` from it as from any room (no room-id test in 0x2001C86C0), so the panel shows an empty
//! room ("0/0" per team) and the window may even open (PROVEN, LobbyMenu.custom_match_fill_state).

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomMember {
    pub name: String,
    pub abbr: String,
    pub id: u64,
    pub clan_id: u32,
    pub team: u8,
    pub commander: bool,
    pub ready: bool,
    pub premium: bool,
    pub tokens_silver: u8,
    pub tokens_gold: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Room98 {
    pub members: Vec<RoomMember>,
    pub room_id: u32,
    pub clan_a: u32,
    pub clan_b: u32,
    pub clan_a_name: String,
    pub clan_b_name: String,
    pub loading_screen: String,
    pub kind: u8,
    pub map: u8,
    pub mode: u8,
    pub weather: u8,
    pub region: u8,
    pub voice: u8,
    pub ai_ignore: u8,
    pub tokens_allocated: u8,
    pub tokens_required: u8,
    pub ai_a: u8,
    pub ai_b: u8,
    pub players_a: u8,
    pub players_b: u8,
    pub cost_gold: u32,
    pub cost_silver: u32,
    pub min_ai: u8,
    pub max_ai: u8,
    pub min_players: u8,
    pub max_players: u8,
    pub unknown: [u8; 2],
}

impl Encode for Room98 {
    fn encode(&self, w: &mut Writer) {
        w.count_u8(self.members.len(), "room members");
        for m in &self.members {
            w.str_max(&m.name, 128).str_max(&m.abbr, 32).u64(m.id).u32(m.clan_id);
            w.u8(m.team).u8(u8::from(m.commander)).u8(u8::from(m.ready)).u8(u8::from(m.premium)).u8(m.tokens_silver).u8(m.tokens_gold);
        }
        w.u32(self.room_id).u32(self.clan_a).u32(self.clan_b);
        w.str_max(&self.clan_a_name, 100).str_max(&self.clan_b_name, 100).str_max(&self.loading_screen, 256);
        w.u8(self.kind).u8(self.map).u8(self.mode).u8(self.weather).u8(self.region).u8(self.voice).u8(self.ai_ignore);
        w.u8(self.tokens_allocated).u8(self.tokens_required).u8(self.ai_a).u8(self.ai_b).u8(self.players_a).u8(self.players_b);
        w.u32(self.cost_gold).u32(self.cost_silver);
        w.u8(self.min_ai).u8(self.max_ai).u8(self.min_players).u8(self.max_players).u8(self.unknown[0]).u8(self.unknown[1]);
    }
}

impl Decode for Room98 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u8()? as usize;
        let mut members = Vec::new();
        for _ in 0..n {
            members.push(RoomMember {
                name: r.str()?,
                abbr: r.str()?,
                id: r.u64()?,
                clan_id: r.u32()?,
                team: r.u8()?,
                commander: r.u8()? == 1,
                ready: r.u8()? != 0,
                premium: r.u8()? != 0,
                tokens_silver: r.u8()?,
                tokens_gold: r.u8()?,
            });
        }
        Ok(Room98 {
            members,
            room_id: r.u32()?,
            clan_a: r.u32()?,
            clan_b: r.u32()?,
            clan_a_name: r.str()?,
            clan_b_name: r.str()?,
            loading_screen: r.str()?,
            kind: r.u8()?,
            map: r.u8()?,
            mode: r.u8()?,
            weather: r.u8()?,
            region: r.u8()?,
            voice: r.u8()?,
            ai_ignore: r.u8()?,
            tokens_allocated: r.u8()?,
            tokens_required: r.u8()?,
            ai_a: r.u8()?,
            ai_b: r.u8()?,
            players_a: r.u8()?,
            players_b: r.u8()?,
            cost_gold: r.u32()?,
            cost_silver: r.u32()?,
            min_ai: r.u8()?,
            max_ai: r.u8()?,
            min_players: r.u8()?,
            max_players: r.u8()?,
            unknown: [r.u8()?, r.u8()?],
        })
    }
}

/// `62 xx` custom match requests (builders 0x200281F40 .. 0x2002828B0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Custom62 {
    /// 18 `custom_match_create`: `u8` (the Training check box of the create dialog: 0 / 1; MatchMakingSettings.as).
    Create { kind: u8 },
    /// 19 `custom_match_join_request`: `u8` (no Flash caller found).
    JoinRequest { arg: u8 },
    /// 1A `custom_match_send_invite`: `u64 target`.
    Invite { target: u64 },
    /// 1B invite answer: `u64 inviter, u8 answer`: 0 = ACCEPT, 1 = decline (PROVEN: `important_message_read` 0x2001E88E0
    /// passes the Flash answer unchanged to 0x20018D2A0; `NotificationInvite` / `CustomMatchInviteMessage` accept = 0).
    Answer { inviter: u64, accept: u8 },
    /// 1C `custom_match_leave`: `u64`: the own id (leave: the lobby mini panel's X, CustomMatchPreviewWindow.onCancelClick,
    /// and the window's back button) or a member's id (the host's remove button of that row, CustomMatchPlayer). PROVEN.
    Leave { player: u64 },
    /// 1D `custom_match_change_member_status`: `u64 player, u8 role`. The row buttons pass 1 (A), 2 (B), 3 (?) to the exe
    /// and its builder 0x2002823B0 sends `value - 1` (0x2002823DC): the wire byte IS the 98 role,
    /// 0 team A, 1 team B, 2 unassigned (PROVEN).
    MemberStatus { player: u64, status: u8 },
    /// 1E `custom_match_change_match_parameter`: `u8 map, weather, game mode, region, global chat, ignore player`
    /// (the order of the Flash call, CustomMatchWindow.as 331).
    Parameters { map: u8, weather: u8, mode: u8, region: u8, voice: u8, ai_ignore: u8 },
    /// 1F: `u8 team (0 = A, 1 = B), u8 max players of that team` (CustomMatchWindow.as 460 / 2254).
    PlayerCount { team: u8, count: u8 },
    /// 20: `u8 team (0 = A, 1 = B), u8 AI slots of that team`.
    AiCount { team: u8, count: u8 },
    /// 21: `i8 gold delta, i8 silver delta`: a CHANGE of the tokens the member pays for, gold first, signed (-1 / +1 from
    /// the buttons; the exe clamps to -100..100). PROVEN 0x200282980 + CustomMatchWindow.onPlayerPaymentChange.
    Tokens { gold: i8, silver: i8 },
    /// 22: the per-player READY toggle: `u8` = the NEW value (the client sends 0xFF for ready, 0 for not ready; any
    /// non-zero is ready). PROVEN 0x20018D5F0 (non-zero = ready), called by the Play button (0x2001E82B0) and by `83 30`
    /// (0x20018F337).
    Ready { ready: u8 },
    /// 23 start.
    Start,
    /// 24 swap teams.
    SwapTeams,
}

impl Custom62 {
    pub fn decode(p: &[u8]) -> Result<Custom62> {
        let mut r = Reader::new(p);
        let v = match r.u8()? {
            0x18 => Custom62::Create { kind: r.u8()? },
            0x19 => Custom62::JoinRequest { arg: r.u8()? },
            0x1A => Custom62::Invite { target: r.u64()? },
            0x1B => Custom62::Answer { inviter: r.u64()?, accept: r.u8()? },
            0x1C => Custom62::Leave { player: r.u64()? },
            0x1D => Custom62::MemberStatus { player: r.u64()?, status: r.u8()? },
            0x1E => Custom62::Parameters { map: r.u8()?, weather: r.u8()?, mode: r.u8()?, region: r.u8()?, voice: r.u8()?, ai_ignore: r.u8()? },
            0x1F => Custom62::PlayerCount { team: r.u8()?, count: r.u8()? },
            0x20 => Custom62::AiCount { team: r.u8()?, count: r.u8()? },
            0x21 => Custom62::Tokens { gold: r.u8()? as i8, silver: r.u8()? as i8 },
            0x22 => Custom62::Ready { ready: r.u8()? },
            0x23 => Custom62::Start,
            0x24 => Custom62::SwapTeams,
            _ => return Err(Error::Unsupported("62 sub")),
        };
        r.finish()?;
        Ok(v)
    }

    /// Payload after `62 x` (tests and scripted clients).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Custom62::Create { kind } => w.u8(0x18).u8(*kind),
            Custom62::JoinRequest { arg } => w.u8(0x19).u8(*arg),
            Custom62::Invite { target } => w.u8(0x1A).u64(*target),
            Custom62::Answer { inviter, accept } => w.u8(0x1B).u64(*inviter).u8(*accept),
            Custom62::Leave { player } => w.u8(0x1C).u64(*player),
            Custom62::MemberStatus { player, status } => w.u8(0x1D).u64(*player).u8(*status),
            Custom62::Parameters { map, weather, mode, region, voice, ai_ignore } => {
                w.u8(0x1E).u8(*map).u8(*weather).u8(*mode).u8(*region).u8(*voice).u8(*ai_ignore)
            }
            Custom62::PlayerCount { team, count } => w.u8(0x1F).u8(*team).u8(*count),
            Custom62::AiCount { team, count } => w.u8(0x20).u8(*team).u8(*count),
            Custom62::Tokens { gold, silver } => w.u8(0x21).u8(*gold as u8).u8(*silver as u8),
            Custom62::Ready { ready } => w.u8(0x22).u8(*ready),
            Custom62::Start => w.u8(0x23),
            Custom62::SwapTeams => w.u8(0x24),
        };
        w.finish().unwrap_or_default()
    }
}
