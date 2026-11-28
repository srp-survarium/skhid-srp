//! The base process's messages (front <-> base, on the base's control socket, one `fdpass::send_msg` each).
//!
//! The base process (`world-host --base-fd N`, started by the front) runs the level-independent start-up once (steps
//! 1-2 and step 3 up to the cut, `run::step3_base`) and then fork()s one zygote per map on request
//! (`daemon::base_loop`):
//! * base -> front `Ready { pid }` once its start-up is done;
//! * front -> base `Load { project, args }` with the new zygote's control socket attached: `args` is the zygote's
//!   whole command line (what a zygote started on its own would get), which the forked zygote reads instead of the
//!   base's (`cmdline`);
//! * base -> front `Loaded { project, pid }` with a pidfd of the zygote attached (the front is not the zygote's parent:
//!   it watches and kills it through the pidfd), or `Failed { project, reason }` without a descriptor, or
//!   `Busy { project, reason }` (not forked now: an exe thread was not quiet in time; ask again).
//!
//! Encoding: a tag byte, then the fields in order; a string is a u32 little-endian length and its UTF-8 bytes; a u32
//! is little-endian; a list is a u32 count and its strings. Nothing may follow the last field.

/// At most this many arguments in a `Load`.
pub const MAX_ARGS: usize = 128;
/// At most this many bytes in one string.
pub const MAX_STR: usize = 4096;

const TAG_LOAD: u8 = 1;
const TAG_READY: u8 = 0x81;
const TAG_LOADED: u8 = 0x82;
const TAG_FAILED: u8 = 0x83;
const TAG_BUSY: u8 = 0x84;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToBase {
    Load { project: String, args: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FromBase {
    Ready { pid: u32 },
    Loaded { project: String, pid: u32 },
    Failed { project: String, reason: String },
    /// not now: the base did not fork because one of its exe threads was not quiet in time (the fork gate's or the
    /// census's TIMEOUT); the front treats it like a base that is not ready yet and asks again
    Busy { project: String, reason: String },
}

fn put_str(b: &mut Vec<u8>, s: &str) -> Result<(), String> {
    if s.len() > MAX_STR {
        return Err(format!("a string of {} bytes (at most {MAX_STR})", s.len()));
    }
    b.extend_from_slice(&(s.len() as u32).to_le_bytes());
    b.extend_from_slice(s.as_bytes());
    Ok(())
}

struct Reader<'a> {
    b: &'a [u8],
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.b.len() < n {
            return Err("message cut".into());
        }
        let (h, t) = self.b.split_at(n);
        self.b = t;
        Ok(h)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > MAX_STR {
            return Err(format!("a string of {n} bytes (at most {MAX_STR})"));
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "a string that is not UTF-8".to_string())
    }
    fn end(&self) -> Result<(), String> {
        if self.b.is_empty() { Ok(()) } else { Err(format!("{} bytes after the message", self.b.len())) }
    }
}

impl ToBase {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut b = Vec::new();
        match self {
            ToBase::Load { project, args } => {
                if args.len() > MAX_ARGS {
                    return Err(format!("{} arguments (at most {MAX_ARGS})", args.len()));
                }
                b.push(TAG_LOAD);
                put_str(&mut b, project)?;
                b.extend_from_slice(&(args.len() as u32).to_le_bytes());
                for a in args {
                    put_str(&mut b, a)?;
                }
            }
        }
        Ok(b)
    }

    pub fn decode(d: &[u8]) -> Result<ToBase, String> {
        let mut r = Reader { b: d };
        let m = match r.u8()? {
            TAG_LOAD => {
                let project = r.str()?;
                let n = r.u32()? as usize;
                if n > MAX_ARGS {
                    return Err(format!("{n} arguments (at most {MAX_ARGS})"));
                }
                let mut args = Vec::with_capacity(n);
                for _ in 0..n {
                    args.push(r.str()?);
                }
                ToBase::Load { project, args }
            }
            t => return Err(format!("unknown message {t:#x}")),
        };
        r.end()?;
        Ok(m)
    }
}

impl FromBase {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut b = Vec::new();
        match self {
            FromBase::Ready { pid } => {
                b.push(TAG_READY);
                b.extend_from_slice(&pid.to_le_bytes());
            }
            FromBase::Loaded { project, pid } => {
                b.push(TAG_LOADED);
                put_str(&mut b, project)?;
                b.extend_from_slice(&pid.to_le_bytes());
            }
            FromBase::Failed { project, reason } | FromBase::Busy { project, reason } => {
                b.push(if matches!(self, FromBase::Busy { .. }) { TAG_BUSY } else { TAG_FAILED });
                put_str(&mut b, project)?;
                let r: String = reason.chars().take(512).collect();
                put_str(&mut b, &r)?;
            }
        }
        Ok(b)
    }

    pub fn decode(d: &[u8]) -> Result<FromBase, String> {
        let mut r = Reader { b: d };
        let m = match r.u8()? {
            TAG_READY => FromBase::Ready { pid: r.u32()? },
            TAG_LOADED => {
                let project = r.str()?;
                FromBase::Loaded { project, pid: r.u32()? }
            }
            TAG_FAILED => {
                let project = r.str()?;
                FromBase::Failed { project, reason: r.str()? }
            }
            TAG_BUSY => {
                let project = r.str()?;
                FromBase::Busy { project, reason: r.str()? }
            }
            t => return Err(format!("unknown message {t:#x}")),
        };
        r.end()?;
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(n: usize) -> ToBase {
        ToBase::Load { project: "pve_01_v1_maze2".into(), args: (0..n).map(|i| format!("--arg{i} \u{e9} x")).collect() }
    }

    #[test]
    fn every_message_comes_back_as_sent() {
        for m in [load(0), load(1), load(40)] {
            assert_eq!(ToBase::decode(&m.encode().unwrap()).unwrap(), m);
        }
        for m in [
            FromBase::Ready { pid: 4242 },
            FromBase::Loaded { project: "level_01_day_tdm".into(), pid: u32::MAX },
            FromBase::Failed { project: "level_01_day_tdm".into(), reason: "pidfd_open: no such system call".into() },
            FromBase::Failed { project: String::new(), reason: String::new() },
            FromBase::Busy { project: "pve_01_v1_maze2".into(), reason: "fork census timeout".into() },
        ] {
            assert_eq!(FromBase::decode(&m.encode().unwrap()).unwrap(), m);
        }
    }

    #[test]
    fn cut_unknown_and_trailing_messages_are_refused() {
        let b = load(3).encode().unwrap();
        for n in 0..b.len() {
            assert!(ToBase::decode(&b[..n]).is_err(), "cut at {n}");
        }
        let mut t = b.clone();
        t.push(0);
        assert!(ToBase::decode(&t).is_err(), "a byte after the message");
        assert!(ToBase::decode(&[0x7F]).is_err());
        assert!(FromBase::decode(&[TAG_LOAD]).is_err(), "a front message is no base reply");
        let r = FromBase::Ready { pid: 1 }.encode().unwrap();
        assert!(ToBase::decode(&r).is_err());
        assert!(FromBase::decode(&r[..3]).is_err());
    }

    #[test]
    fn limits_hold_both_ways() {
        assert!(load(MAX_ARGS).encode().is_ok());
        assert!(load(MAX_ARGS + 1).encode().is_err());
        let mut b = vec![TAG_LOAD];
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&((MAX_ARGS + 1) as u32).to_le_bytes());
        assert!(ToBase::decode(&b).is_err(), "a count over the limit is refused before reading");
        let long = ToBase::Load { project: "x".repeat(MAX_STR + 1), args: vec![] };
        assert!(long.encode().is_err());
        let mut s = vec![TAG_READY + 2];
        s.extend_from_slice(&((MAX_STR + 1) as u32).to_le_bytes());
        assert!(FromBase::decode(&s).is_err());
        let r = FromBase::Failed { project: "p".into(), reason: "y".repeat(2000) }.encode().unwrap();
        match FromBase::decode(&r).unwrap() {
            FromBase::Failed { reason, .. } => assert_eq!(reason.len(), 512, "a reason is cut to 512 characters"),
            m => panic!("{m:?}"),
        }
    }
}
