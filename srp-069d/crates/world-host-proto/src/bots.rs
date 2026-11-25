//! Protocol version 5: the host's per-tick bot entries. With `Create.drive_bots` the host
//! runs the exe's own AI for every options record with `ai_id != -1` and answers **every** stepped DF (empty ones
//! too) with one `Bots` frame. That frame doubles as the relay's "tick done" signal for the hold of DF `t_for`: ai
//! entries exist only at a DF's own time and can never go late.
//!
//! `Bots` (0x88): `u32 t_for, u8 n_in, n_in x {u8 slot, 11-byte player_input}, u8 n_ai, n_ai x {u8 slot, u16 type,
//! u16 param}, u32 offgrid`.
//! * Every entry is at time `t_for` (= the stepped DF's time + tick). The relay files the inputs as DF `t_for`'s
//!   input_mask entries and the ai entries as its ai list, each exactly once.
//! * `player_input` is the DF form (reader 0x2003276E0: u16 heading LE, u16 pitch LE, the low 56 bits of the actions
//!   big-endian).
//! * `offgrid` counts host-made entries at any other time after the step. The relay cannot send those; the host reports
//!   them as `Diverged` itself.

use super::{frame_bytes, Error, Frame, FromHost, R, W};

pub const TY_BOTS: u8 = super::ty::BOTS;
/// One input per player per DF time; the client's buffers hold 20 player inputs and 20 ai entries per DF
/// (0x20032E9EE).
pub const MAX_BOT_ENTRIES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bots {
    pub t_for: u32,
    pub inputs: Vec<(u8, [u8; 11])>,
    pub ai: Vec<(u8, u16, u16)>,
    pub offgrid: u32,
}

impl Bots {
    pub(crate) fn put(&self, w: &mut W) -> Result<(), Error> {
        if self.inputs.len() > MAX_BOT_ENTRIES {
            return Err(Error::Limit("Bots inputs"));
        }
        if self.ai.len() > MAX_BOT_ENTRIES {
            return Err(Error::Limit("Bots ai"));
        }
        w.u32(self.t_for).u8(self.inputs.len() as u8);
        for (s, b) in &self.inputs {
            w.u8(*s).bytes(b);
        }
        w.u8(self.ai.len() as u8);
        for (s, a, p) in &self.ai {
            w.u8(*s).u16(*a).u16(*p);
        }
        w.u32(self.offgrid);
        Ok(())
    }

    pub(crate) fn get(r: &mut R) -> Result<Bots, Error> {
        let t_for = r.u32()?;
        let n = r.count("Bots inputs")?;
        let mut inputs = Vec::with_capacity(n);
        for _ in 0..n {
            let s = r.u8()?;
            inputs.push((s, r.take(11)?.try_into().map_err(|_| Error::Short)?));
        }
        let n = r.count("Bots ai")?;
        let mut ai = Vec::with_capacity(n);
        for _ in 0..n {
            ai.push((r.u8()?, r.u16()?, r.u16()?));
        }
        let offgrid = r.u32()?;
        Ok(Bots { t_for, inputs, ai, offgrid })
    }

    /// The frame bytes (as `FromHost::Bots(..).frame()`).
    pub fn frame(&self) -> Result<Vec<u8>, Error> {
        let mut w = W::default();
        self.put(&mut w)?;
        frame_bytes(TY_BOTS, &w.0)
    }
}

/// Decode a `Bots` frame (as `FromHost::decode`).
pub fn decode(f: &Frame) -> Result<Bots, Error> {
    match FromHost::decode(f)? {
        FromHost::Bots(b) => Ok(b),
        _ => Err(Error::Type(f.ty)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Create, Record, ToHost, KIND_PVE, KIND_PVP};

    #[test]
    fn a_bots_frame_round_trips_and_bounds_its_counts() {
        let b = Bots { t_for: 1150, inputs: vec![(3, [7; 11]), (4, [0; 11])], ai: vec![(5, 1, 2)], offgrid: 0 };
        let bytes = FromHost::Bots(b.clone()).frame().unwrap();
        assert_eq!(bytes, b.frame().unwrap());
        assert_eq!(bytes[4], TY_BOTS);
        let f = Frame { ty: bytes[4], payload: bytes[5..].to_vec() };
        assert_eq!(decode(&f).unwrap(), b);
        let empty = Bots { t_for: 1200, inputs: vec![], ai: vec![], offgrid: 0 };
        assert_eq!(empty.frame().unwrap().len(), 4 + 1 + 4 + 1 + 1 + 4);
        let over = Bots { t_for: 0, inputs: vec![(0, [0; 11]); 21], ai: vec![], offgrid: 0 };
        assert_eq!(over.frame(), Err(Error::Limit("Bots inputs")));
        let mut p = f.payload.clone();
        p.push(0);
        assert_eq!(FromHost::decode(&Frame { ty: TY_BOTS, payload: p }), Err(Error::Trailing(1)));
    }

    #[test]
    fn drive_bots_is_on_the_wire_from_version_5_only() {
        let mk = |version: u16, drive_bots: bool| Create {
            version,
            match_id: 7,
            kind: KIND_PVE,
            project: "pve_01_v1_maze2".into(),
            map: 15,
            mode: 7,
            tick_ms: 50,
            records: vec![Record { bot: false, team: 0 }, Record { bot: true, team: 1 }],
            options: vec![1, 2, 3],
            world: vec![],
            drive_bots,
            ai_ignore_players: false,
        };
        for v in [3u16, 4, 5] {
            let f = ToHost::Create(mk(v, true)).frame().unwrap();
            let fr = Frame { ty: f[4], payload: f[5..].to_vec() };
            let ToHost::Create(c) = ToHost::decode(&fr).unwrap() else { panic!() };
            // below version 5 the flag is not sent: it decodes as false
            assert_eq!(c.drive_bots, v >= 5, "version {v}");
            assert_eq!(f.len(), ToHost::Create(mk(v, false)).frame().unwrap().len());
        }
        let v4 = ToHost::Create(mk(4, false)).frame().unwrap();
        let v5 = ToHost::Create(mk(5, false)).frame().unwrap();
        assert_eq!(v5.len(), v4.len() + 1);
    }

    /// Version 7: `ai_ignore_players` after `drive_bots`; a version-6 Create has no such byte and decodes it as false.
    #[test]
    fn ai_ignore_players_is_on_the_wire_from_version_7_only() {
        let mk = |version: u16, on: bool| Create {
            version,
            match_id: 7,
            kind: KIND_PVP,
            project: "level_07_day_br".into(),
            map: 7,
            mode: 0,
            tick_ms: 50,
            records: vec![Record { bot: false, team: 0 }, Record { bot: true, team: 1 }],
            options: vec![1, 2, 3],
            world: vec![],
            drive_bots: true,
            ai_ignore_players: on,
        };
        let round = |c: Create| {
            let f = ToHost::Create(c).frame().unwrap();
            let ToHost::Create(c) = ToHost::decode(&Frame { ty: f[4], payload: f[5..].to_vec() }).unwrap() else { panic!() };
            (c, f.len())
        };
        let (c7, n7) = round(mk(7, true));
        assert!(c7.ai_ignore_players && c7.drive_bots);
        assert!(!round(mk(7, false)).0.ai_ignore_players);
        let (c6, n6) = round(mk(6, true));
        assert!(!c6.ai_ignore_players && c6.drive_bots, "version 6: no flag on the wire");
        assert_eq!(n7, n6 + 1);
    }
}
