//! Daily quests and the daily login reward on the wire (docs/notes/quests.md).
//!
//! PROVEN (exe):
//! * `AB` (block 0x20018E8CB, reader 0x2001881A0) is the DAILY QUEST list, not a faction-battle config: it fills the
//!   client struct at lobby_client+0x79F730 that `set_daily_quest_data` (0x2001792A0) turns into Flash
//!   `root.set_daily_quest_data`. Wire: `u32 time_to_expire_in_sec, u8 quests_count (<= 3), u8 reroll_allowed,
//!   u8 free_reroll_count, count x {u32 quest_id, u32 dict_id, u32 param, u32 progress}, u32 received` where `received`
//!   = `received_reward << 16 | received_reward_amount` (0 = the daily chest was not opened). `dict_id` indexes
//!   `daily_quests.options` `quests` WITHOUT a bounds check; `value_to_reach` comes from that file.
//! * Quest events are `B1` important messages of type 0x0E (0x2001C2140): the `text` is `qg[uid][dict][param]`
//!   (quest granted, only while fewer than 3 are listed), `qu[uid][progress]` (progress) or `qc[uid][progress]`
//!   (completed: popup with the three reward lines). The client answers every one with request `51` (read).
//! * `83 4A 04` (0x200191880): `u16 dict_id, u32 amount` -> `root.daily_quest_chest_reward_received` (the item shown
//!   as the chest reward).
//! * The daily login reward is NOT a message of its own: it is a service record of the `94` offers (type 0x1B,
//!   reader 0x2001B9040 for the body below, emitted by 0x2001C6B62 with `callback = "daily_reward_collect"` and
//!   `callback_value` = the record's first u32, which the client sends back as `5E u32`).

use crate::s2c::SystemMessage;
use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Most quests the client list holds (bound check at 0x1C2302).
pub const MAX_QUESTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbQuest {
    pub uid: u32,
    pub dict: u32,
    /// `parameter` of the quest (the client only formats it into the text of type 2 quests, none in the file)
    pub param: u32,
    pub progress: u32,
}

/// `AB`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DailyQuestsAb {
    pub expires_in: u32,
    pub reroll_allowed: bool,
    pub free_rerolls: u8,
    pub quests: Vec<AbQuest>,
    /// dict id of the opened chest's reward (0 = not opened) and its amount (only 16 bits travel)
    pub received_dict: u16,
    pub received_amount: u16,
}

impl Encode for DailyQuestsAb {
    fn encode(&self, w: &mut Writer) {
        if self.quests.len() > MAX_QUESTS {
            w.fail(Error::Invalid("AB: more than 3 quests"));
        }
        w.u32(self.expires_in).u8(self.quests.len().min(MAX_QUESTS) as u8).u8(u8::from(self.reroll_allowed)).u8(self.free_rerolls);
        for q in &self.quests {
            w.u32(q.uid).u32(q.dict).u32(q.param).u32(q.progress);
        }
        w.u32(u32::from(self.received_dict) << 16 | u32::from(self.received_amount));
    }
}

impl Decode for DailyQuestsAb {
    fn decode(r: &mut Reader) -> Result<Self> {
        let expires_in = r.u32()?;
        let n = usize::from(r.u8()?);
        if n > MAX_QUESTS {
            return Err(Error::Invalid("AB: more than 3 quests"));
        }
        let reroll_allowed = r.u8()? != 0;
        let free_rerolls = r.u8()?;
        let mut quests = Vec::with_capacity(n);
        for _ in 0..n {
            quests.push(AbQuest { uid: r.u32()?, dict: r.u32()?, param: r.u32()?, progress: r.u32()? });
        }
        let v = r.u32()?;
        Ok(DailyQuestsAb { expires_in, reroll_allowed, free_rerolls, quests, received_dict: (v >> 16) as u16, received_amount: v as u16 })
    }
}

/// `83 4A 04` data after the request id: `04 u16 dict, u32 amount`.
pub fn chest_reply(dict: u16, amount: u32) -> Vec<u8> {
    let mut v = vec![4];
    v.extend_from_slice(&dict.to_le_bytes());
    v.extend_from_slice(&amount.to_le_bytes());
    v
}

/// What a quest message says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestEvent {
    Granted { uid: u32, dict: u32, param: u32 },
    Progress { uid: u32, progress: u32 },
    Completed { uid: u32, progress: u32 },
}

impl QuestEvent {
    pub fn text(&self) -> String {
        match *self {
            QuestEvent::Granted { uid, dict, param } => format!("qg[{uid}][{dict}][{param}]"),
            QuestEvent::Progress { uid, progress } => format!("qu[{uid}][{progress}]"),
            QuestEvent::Completed { uid, progress } => format!("qc[{uid}][{progress}]"),
        }
    }

    /// The `B1` message (type 0x0E, no sender) with a unique message id.
    pub fn message(&self, message_id: u32) -> SystemMessage {
        SystemMessage { kind: 0x0E, sender: 0, name: String::new(), clan: String::new(), flag: 0, text: self.text(), message_id }
    }

    /// Parse the text of a type 0x0E message (tests, and the client's own tokenizer 0x2001C4750 in reverse).
    pub fn parse(text: &str) -> Option<QuestEvent> {
        let nums: Vec<u32> = text
            .get(2..)?
            .split(']')
            .filter(|s| !s.is_empty())
            .map(|s| s.strip_prefix('[')?.parse().ok())
            .collect::<Option<_>>()?;
        Some(match (text.get(..2)?, nums.as_slice()) {
            ("qg", &[uid, dict, param]) => QuestEvent::Granted { uid, dict, param },
            ("qu", &[uid, progress]) => QuestEvent::Progress { uid, progress },
            ("qc", &[uid, progress]) => QuestEvent::Completed { uid, progress },
            _ => return None,
        })
    }
}

/// The most days the client's login calendar holds (assert above 64).
pub const MAX_BONUS_DAYS: usize = 64;

/// One day of the daily login reward calendar (body of a `94` service record of type 0x1B, reader 0x2001B9040).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BonusDay {
    pub amount: u32,
    /// item dict id of the reward
    pub reward: u16,
    /// `award_quality` (Flash)
    pub quality: u8,
}

/// Body of the daily login reward record: `u16 n, n x {u32 amount, u16 reward, u8 quality}, u8 has_state`, and with
/// state `u64 received_mask (bit i = day i taken), u8 current_day, u8, u8` (the last two bytes are stored by the
/// client at +0x323 / +0x324 and not used by anything read: UNKNOWN, sent as 0). The client shows
/// `today_reward_received = bit(mask, current_day) || current_day >= n`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BonusBody {
    pub days: Vec<BonusDay>,
    pub state: Option<BonusState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BonusState {
    pub received_mask: u64,
    pub current_day: u8,
}

impl Encode for BonusBody {
    fn encode(&self, w: &mut Writer) {
        // the client's calendar has 64 day slots: more asserts (0x2001B9040 / 0x2001C6B62)
        if self.days.len() > MAX_BONUS_DAYS {
            w.fail(crate::Error::Limit("daily bonus days (64)"));
        }
        w.count_u16(self.days.len(), "daily bonus days");
        for d in &self.days {
            w.u32(d.amount).u16(d.reward).u8(d.quality);
        }
        match self.state {
            None => {
                w.u8(0);
            }
            Some(s) => {
                w.u8(1).u64(s.received_mask).u8(s.current_day).u8(0).u8(0);
            }
        }
    }
}

impl Decode for BonusBody {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n = r.u16()? as usize;
        let n = r.check_count(n, 7)?;
        let mut days = Vec::with_capacity(n);
        for _ in 0..n {
            days.push(BonusDay { amount: r.u32()?, reward: r.u16()?, quality: r.u8()? });
        }
        let state = if r.u8()? != 0 {
            let s = BonusState { received_mask: r.u64()?, current_day: r.u8()? };
            r.skip(2)?;
            Some(s)
        } else {
            None
        };
        Ok(BonusBody { days, state })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ab_layout_and_round_trip() {
        let m = DailyQuestsAb {
            expires_in: 100,
            reroll_allowed: true,
            free_rerolls: 1,
            quests: vec![AbQuest { uid: 7, dict: 2, param: 0, progress: 1 }],
            received_dict: 213,
            received_amount: 1,
        };
        let b = m.to_bytes().unwrap();
        assert_eq!(&b[..7], [100, 0, 0, 0, 1, 1, 1]);
        assert_eq!(&b[7..23], [7, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&b[23..], [1, 0, 213, 0]);
        assert_eq!(DailyQuestsAb::from_bytes(&b).unwrap(), m);
        assert_eq!(DailyQuestsAb::default().to_bytes().unwrap(), vec![0; 11]);
        let four = DailyQuestsAb { quests: vec![m.quests[0]; 4], ..m };
        assert!(four.to_bytes().is_err());
    }

    #[test]
    fn quest_message_text_round_trip() {
        for e in [
            QuestEvent::Granted { uid: 5, dict: 28, param: 0 },
            QuestEvent::Progress { uid: 5, progress: 2 },
            QuestEvent::Completed { uid: 9, progress: 3 },
        ] {
            assert_eq!(QuestEvent::parse(&e.text()), Some(e));
        }
        assert_eq!(QuestEvent::Progress { uid: 5, progress: 2 }.text(), "qu[5][2]");
        let m = QuestEvent::Completed { uid: 1, progress: 1 }.message(77);
        assert_eq!((m.kind, m.message_id, m.sender), (0x0E, 77, 0));
        assert_eq!(QuestEvent::parse("qx[1][2]"), None);
    }

    #[test]
    fn chest_reply_layout() {
        assert_eq!(chest_reply(213, 1), [4, 213, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn bonus_body_layout_and_round_trip() {
        let b = BonusBody {
            days: vec![BonusDay { amount: 1000, reward: 214, quality: 2 }, BonusDay { amount: 1, reward: 213, quality: 3 }],
            state: Some(BonusState { received_mask: 1, current_day: 1 }),
        };
        let bytes = b.to_bytes().unwrap();
        assert_eq!(&bytes[..2], [2, 0]);
        assert_eq!(&bytes[2..9], [0xE8, 3, 0, 0, 214, 0, 2]);
        assert_eq!(bytes.len(), 2 + 14 + 1 + 8 + 3);
        assert_eq!(BonusBody::from_bytes(&bytes).unwrap(), b);
        assert_eq!(BonusBody::default().to_bytes().unwrap(), [0, 0, 0]);
    }
}
