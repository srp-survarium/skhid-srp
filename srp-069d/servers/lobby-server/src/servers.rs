//! The game-server list: `9E` (server -> client), `57` (the client's selection), `56` (its ping results).
//! Layouts and meaning: `docs/notes/server-list.md`.

use protocol::s2c::{S2c, ServerInfo9E};

use crate::dispatch::Ctx;
use crate::peer::Peer;

/// The client's server vector holds 8 entries (inline buffer, 0x18388D); a 9th is an assert + `int 3` (PROVEN).
pub const MAX_SERVERS: usize = 8;
/// Host and label are read with a cap of 0x40 bytes each.
pub const MAX_TEXT: usize = 0x40;
/// The label of the default entry ([`GameServer::default_list`]).
pub const DEFAULT_LABEL: &str = "Skhid";

/// One game server of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameServer {
    /// What the client pings (the host name or address; entry +0x00).
    pub host: String,
    /// The label Flash shows in the server window (`label`; entry +0x40).
    pub label: String,
    /// Position of the server's marker on the window's map (`x` / `y`; entries +0x84 / +0x86).
    pub x: u16,
    pub y: u16,
}

impl GameServer {
    /// `host|label|x|y` (x and y optional, 0 by default).
    pub fn parse(v: &str) -> Result<GameServer, String> {
        let p: Vec<&str> = v.split('|').map(str::trim).collect();
        if p.len() < 2 || p.len() > 4 || p[0].is_empty() {
            return Err(format!("{v:?}: use host|label[|x|y]"));
        }
        let num = |i: usize| -> Result<u16, String> {
            p.get(i).map_or(Ok(0), |t| t.parse::<u16>().map_err(|e| format!("{t:?}: {e}")))
        };
        if p[0].len() > MAX_TEXT || p[1].len() > MAX_TEXT {
            return Err(format!("{v:?}: host and label are at most {MAX_TEXT} bytes"));
        }
        Ok(GameServer { host: p[0].to_string(), label: p[1].to_string(), x: num(2)?, y: num(3)? })
    }

    /// The list when the operator names none: this server alone, labelled "Skhid" (a server name, shown as sent: the
    /// label is not a localization key, PROVEN 0x189DA1). The marker sits on Eastern Europe of the server window's map:
    /// the map clip is 310 x 162 px and Flash places a marker at `x * 0.76`, `y * 0.76` (ServerSelect.setServersList,
    /// SrvServerSelector `worldMap`), so (238, 47) is about 30 E, 50 N on a plate carree map (CHOICE: the original
    /// positions are in no file).
    pub fn default_list(host: &str) -> Vec<GameServer> {
        let host: String = host.chars().take(MAX_TEXT).collect();
        vec![GameServer { host, label: DEFAULT_LABEL.into(), x: 238, y: 47 }]
    }

    /// `host|label|x|y;host|label|x|y` (a text form for tests; srp reads `[[game_servers]]` of its config); at most [`MAX_SERVERS`].
    pub fn parse_list(v: &str) -> Result<Vec<GameServer>, String> {
        let l: Vec<GameServer> =
            v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(GameServer::parse).collect::<Result<_, _>>()?;
        if l.len() > MAX_SERVERS {
            return Err(format!("at most {MAX_SERVERS} servers"));
        }
        Ok(l)
    }
}

/// `9E`: three strings the client opens as web pages (empty: no web site), `u8 n`, n x {host, label, `u16 x`,
/// `u16 y`, `u8 id`}. The id is the entry's position (the bit in the `57` mask). An empty list leaves the client's
/// mask at "all" and its window empty.
pub fn list_9e(servers: &[GameServer]) -> S2c {
    S2c::ServerInfo(ServerInfo9E {
        servers: servers
            .iter()
            .take(MAX_SERVERS)
            .enumerate()
            .map(|(i, s)| protocol::s2c::GameServer { name: s.host.clone(), address: s.label.clone(), a: s.x, b: s.y, c: i as u8 })
            .collect(),
        ..ServerInfo9E::default()
    })
}

/// `57 u32 mask, u32 flags`: the player changed which servers to play on. Kept on the peer; the list is sent again
/// (the client's 9E handler keeps the saved mask whenever the list is not empty) so the window shows the server
/// state the server holds (LIKELY that 9E is the answer; the client never waits).
pub fn selected(cx: &Ctx, peer: &mut Peer, mask: u32, flags: u32) {
    let servers = &cx.lobby.cfg.game_servers;
    let valid = servers.len().min(MAX_SERVERS);
    let all = if valid >= 32 { u32::MAX } else { (1u32 << valid) - 1 };
    peer.server_mask = mask & all;
    log::info!("{}: 57 server selection {mask:#x} (flags {flags:#x}) of {valid} servers", peer.addr);
    if valid > 0 {
        peer.push(&list_9e(servers));
    }
}

/// `56`: ping results. Only the ids of the list are kept.
pub fn pinged(peer: &mut Peer, pings: &[(u8, u16)]) {
    peer.server_pings = pings.to_vec();
    log::debug!("{}: 56 pings {pings:?}", peer.addr);
}

/// The servers the peer selected (mask bits) in the order of the list; every server when the mask selects none.
pub fn chosen<'a>(servers: &'a [GameServer], mask: u32) -> Vec<&'a GameServer> {
    let c: Vec<&GameServer> = servers.iter().enumerate().take(MAX_SERVERS).filter(|(i, _)| mask >> i & 1 != 0).map(|(_, s)| s).collect();
    if c.is_empty() {
        servers.iter().take(MAX_SERVERS).collect()
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::wire::Reader;

    #[test]
    fn empty_list_is_the_old_four_zero_bytes() {
        assert_eq!(list_9e(&[]).encode_payload().unwrap(), vec![0; 4]);
    }

    #[test]
    fn list_layout_reads_back_like_the_client_reader() {
        let l = GameServer::parse_list("eu.example|Europe|120|80; us.example|America|40|90").unwrap();
        let m = list_9e(&l);
        assert_eq!(m.id(), 0x9E);
        let payload = m.encode_payload().unwrap();
        let mut r = Reader::new(&payload);
        for _ in 0..3 {
            assert_eq!(r.str().unwrap(), "");
        }
        assert_eq!(r.u8().unwrap(), 2);
        for (i, s) in l.iter().enumerate() {
            assert_eq!(r.str().unwrap(), s.host);
            assert_eq!(r.str().unwrap(), s.label);
            assert_eq!((r.u16().unwrap(), r.u16().unwrap(), r.u8().unwrap()), (s.x, s.y, i as u8));
        }
        r.finish().unwrap();
    }

    #[test]
    fn parse_rejects_bad_settings() {
        assert!(GameServer::parse("onlyhost").is_err());
        assert!(GameServer::parse("h|l|x").is_err());
        let nine = (0..9).map(|i| format!("h{i}|l")).collect::<Vec<_>>().join(";");
        assert!(GameServer::parse_list(&nine).is_err());
        assert_eq!(GameServer::parse_list("").unwrap().len(), 0);
    }

    #[test]
    fn the_default_list_is_one_selectable_skhid_entry() {
        let l = GameServer::default_list("203.0.113.7");
        assert_eq!(l, vec![GameServer { host: "203.0.113.7".into(), label: "Skhid".into(), x: 238, y: 47 }]);
        // the marker lands inside the 310 x 162 map clip after the client's 0.76 scale
        assert!(f64::from(l[0].x) * 0.76 < 310.0 && f64::from(l[0].y) * 0.76 < 162.0);
        let payload = list_9e(&l).encode_payload().unwrap();
        let mut r = Reader::new(&payload);
        for _ in 0..3 {
            r.str().unwrap();
        }
        assert_eq!(r.u8().unwrap(), 1, "n > 0: the client keeps its saved selection and fills both selectors");
        assert_eq!((r.str().unwrap(), r.str().unwrap()), ("203.0.113.7".to_string(), "Skhid".to_string()));
        assert_eq!((r.u16().unwrap(), r.u16().unwrap(), r.u8().unwrap()), (238, 47, 0));
        assert_eq!(GameServer::default_list(&"h".repeat(100))[0].host.len(), MAX_TEXT);
    }

    #[test]
    fn chosen_follows_the_mask() {
        let l = GameServer::parse_list("a|A;b|B;c|C").unwrap();
        assert_eq!(chosen(&l, 0b101).iter().map(|s| s.host.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!(chosen(&l, 0).len(), 3);
    }
}
