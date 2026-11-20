//! Bans: of an account, or of the addresses players sign in from.
//!
//! A banned account is told so at sign-in (the game's own "user banned"
//! answer), once its password is right, so a guesser learns nothing. A banned
//! address is told so before the password: no account can sign in, or be
//! registered, from it. Banning ends the sessions it covers at once and
//! closes their lobby and chat connections.
//!
//! The admin's chat mute of an account (`docs/notes/admin-chat-mute.md`) is
//! a [`Ban`] too: it signs in and plays as before, but nobody gets its chat
//! lines; it gets [`Ban::chat_notice`] back instead.

use std::fmt;
use std::net::IpAddr;

/// An address, or a range of them (`192.168.0.0/16`, `2001:db8::/48`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Net {
    pub address: IpAddr,
    pub bits: u8,
}

impl Net {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (address, bits) = match text.split_once('/') {
            Some((address, bits)) => (address, Some(bits.parse::<u8>().ok()?)),
            None => (text, None),
        };
        let address: IpAddr = address.parse().ok()?;
        let address = address.to_canonical();
        let longest = if address.is_ipv4() { 32 } else { 128 };
        let bits = bits.unwrap_or(longest);
        (bits <= longest).then_some(Self { address, bits })
    }

    /// One address.
    pub fn single(address: IpAddr) -> Self {
        let address = address.to_canonical();
        Self {
            address,
            bits: if address.is_ipv4() { 32 } else { 128 },
        }
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let (ours, theirs, width) = match (self.address.to_canonical(), ip.to_canonical()) {
            (IpAddr::V4(ours), IpAddr::V4(theirs)) => (
                u128::from(u32::from(ours)),
                u128::from(u32::from(theirs)),
                32,
            ),
            (IpAddr::V6(ours), IpAddr::V6(theirs)) => (u128::from(ours), u128::from(theirs), 128),
            _ => return false,
        };
        let shift = width - u32::from(self.bits.min(width as u8));
        shift == width || ours >> shift == theirs >> shift
    }
}

impl fmt::Display for Net {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let longest = if self.address.is_ipv4() { 32 } else { 128 };
        if self.bits == longest {
            write!(formatter, "{}", self.address)
        } else {
            write!(formatter, "{}/{}", self.address, self.bits)
        }
    }
}

/// How long a ban lasts, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ban {
    /// Unix seconds it ends at; `None`: never.
    pub until: Option<u64>,
    /// For the admin; never shown to the player, but a chat mute's
    /// ([`Self::chat_notice`]).
    pub reason: String,
}

impl Ban {
    /// Whether it still holds at `now` (Unix seconds).
    pub fn holds(&self, now: u64) -> bool {
        self.until.is_none_or(|until| now < until)
    }

    /// As a chat mute, the line its player gets back instead of each line
    /// it sends: `muted for <reason>`, or `muted until <date> <time> UTC:
    /// <reason>`.
    pub fn chat_notice(&self) -> String {
        match self.until {
            None => format!("muted for {}", self.reason),
            Some(at) => format!(
                "muted until {} {} UTC: {}",
                crate::utc::date(at),
                crate::utc::clock(at),
                self.reason
            ),
        }
    }
}

/// A ban of an address or a range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpBan {
    pub net: Net,
    pub ban: Ban,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networks_parse_match_and_print() {
        let net = Net::parse("192.168.0.0/16").unwrap();
        assert!(net.contains("192.168.9.9".parse().unwrap()));
        assert!(!net.contains("192.169.0.1".parse().unwrap()));
        assert!(net.contains("::ffff:192.168.1.1".parse().unwrap()));
        assert!(
            Net::parse("0.0.0.0/0")
                .unwrap()
                .contains("8.8.8.8".parse().unwrap())
        );
        assert!(Net::parse("::1").unwrap().contains("::1".parse().unwrap()));
        assert!(Net::parse("1.2.3.4/33").is_none() && Net::parse("nope").is_none());
        assert_eq!(net.to_string(), "192.168.0.0/16");
        assert_eq!(Net::parse(" 10.0.0.1 ").unwrap().to_string(), "10.0.0.1");
        assert_eq!(
            Net::single("::ffff:10.0.0.1".parse().unwrap()).to_string(),
            "10.0.0.1"
        );
    }

    #[test]
    fn a_chat_mute_tells_its_player_why_and_until_when() {
        // 2026-10-18 12:34 UTC.
        let timed = Ban {
            until: Some(1_792_326_840),
            reason: "spam".into(),
        };
        assert_eq!(
            timed.chat_notice(),
            "muted until 2026-10-18 12:34 UTC: spam"
        );
        let for_good = Ban {
            until: None,
            reason: "insults".into(),
        };
        assert_eq!(for_good.chat_notice(), "muted for insults");
    }

    #[test]
    fn a_ban_holds_until_its_end() {
        let ban = Ban {
            until: Some(100),
            reason: String::new(),
        };
        assert!(ban.holds(99) && !ban.holds(100));
        assert!(
            Ban {
                until: None,
                reason: String::new()
            }
            .holds(u64::MAX)
        );
    }
}
