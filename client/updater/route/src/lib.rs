//! Select the direct route first, then a configured regional relay. Probes
//! exercise login TCP and lobby/match UDP without signing in or joining a game.
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
const TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    host: String,
    port: u16,
}

impl Endpoint {
    pub fn parse(text: &str) -> Option<Self> {
        let (host, port) = text.split_once(':')?;
        let port: u16 = port.parse().ok()?;
        if host.is_empty()
            || host.len() > 63
            || port == 0
            || port > 65532
            || !host
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c))
        {
            return None;
        }
        Some(Self {
            host: host.into(),
            port,
        })
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    fn resolve(&self, timeout: Duration) -> io::Result<SocketAddr> {
        let endpoint = self.clone();
        let (send, receive) = mpsc::sync_channel(1);
        // The OS DNS resolver has no portable timeout. Its single worker can
        // finish independently; it never delays trying the numeric relay IP.
        std::thread::spawn(move || {
            let answer = (endpoint.host.as_str(), endpoint.port)
                .to_socket_addrs()
                .and_then(|mut addresses| {
                    addresses
                        .find(SocketAddr::is_ipv4)
                        .ok_or_else(|| io::Error::other("no IPv4 address"))
                });
            let _ = send.send(answer);
        });
        receive
            .recv_timeout(timeout)
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS timed out"))?
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub direct: Endpoint,
    pub relay: Endpoint,
}

impl Config {
    /// Exactly two lines, direct first, then the relay. A missing/invalid file
    /// must never silently select an address that wasn't shipped by the owner.
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let direct = Endpoint::parse(lines.next()?)?;
        let relay = Endpoint::parse(lines.next()?)?;
        let relay_ip: Ipv4Addr = relay.host.parse().ok()?;
        if lines.next().is_some()
            || direct == relay
            || relay_ip.is_unspecified()
            || relay_ip.is_multicast()
            || relay_ip.is_broadcast()
        {
            return None;
        }
        Some(Self { direct, relay })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Direct(SocketAddr),
    Relay(SocketAddr),
    Unreachable,
}

pub fn select(config: &Config) -> Selection {
    choose(&config.direct, &config.relay, |endpoint| {
        probe(endpoint, TIMEOUT)
    })
}

fn choose(
    direct: &Endpoint,
    relay: &Endpoint,
    mut check: impl FnMut(&Endpoint) -> io::Result<SocketAddr>,
) -> Selection {
    if let Ok(address) = check(direct) {
        return Selection::Direct(address);
    }
    match check(relay) {
        Ok(address) => Selection::Relay(address),
        Err(_) => Selection::Unreachable,
    }
}

fn probe(endpoint: &Endpoint, timeout: Duration) -> io::Result<SocketAddr> {
    let address = endpoint.resolve(timeout)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_le_bytes();
    let mut packet = PROBE_PREFIX.to_vec();
    packet.extend_from_slice(&nonce[..8]);
    std::thread::scope(|scope| {
        let tcp = scope.spawn(|| probe_tcp(address, &packet, timeout));
        let lobby = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 1),
                &packet,
                timeout,
            )
        });
        let game = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 2),
                &packet,
                timeout,
            )
        });
        tcp.join()
            .map_err(|_| io::Error::other("TCP probe panicked"))??;
        lobby
            .join()
            .map_err(|_| io::Error::other("lobby probe panicked"))??;
        game.join()
            .map_err(|_| io::Error::other("match probe panicked"))??;
        Ok(address)
    })
}

fn probe_tcp(address: SocketAddr, packet: &[u8], timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(packet)?;
    let mut reply = vec![0; packet.len()];
    let mut read = 0;
    while read < reply.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "TCP probe timed out",
            ));
        }
        stream.set_read_timeout(Some(remaining))?;
        let count = stream.read(&mut reply[read..])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "probe ended early",
            ));
        }
        read += count;
    }
    if reply != packet {
        return Err(io::Error::other("wrong TCP probe answer"));
    }
    Ok(())
}

fn probe_udp(address: SocketAddr, packet: &[u8], timeout: Duration) -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
    let deadline = Instant::now() + timeout;
    let mut reply = [0; 64];
    while Instant::now() < deadline {
        socket.send(packet)?;
        socket.set_read_timeout(Some(
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1))
                .min(Duration::from_millis(400)),
        ))?;
        match socket.recv(&mut reply) {
            Ok(n) if &reply[..n] == packet => return Ok(()),
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "UDP probe timed out",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_bounded_and_cannot_inject_arguments() {
        assert!(Config::parse("skhid.link:1234\n203.0.113.8:1234\n").is_some());
        for bad in [
            "a:1234",
            "a:1234\na:1234",
            "a:65534\n203.0.113.8:1234",
            "a -evil:1234\n203.0.113.8:1234",
            "a:1234\n203.0.113.8:1234\nextra",
        ] {
            assert!(Config::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn fallback_only_after_direct_failure_and_no_silent_success() {
        let config = Config::parse("198.51.100.1:1234\n203.0.113.8:1234").unwrap();
        let mut calls = Vec::new();
        let selection = choose(&config.direct, &config.relay, |endpoint| {
            calls.push(endpoint.clone());
            Ok(endpoint.address().parse().unwrap())
        });
        assert_eq!(
            selection,
            Selection::Direct("198.51.100.1:1234".parse().unwrap())
        );
        assert_eq!(calls.as_slice(), std::slice::from_ref(&config.direct));
        assert_eq!(
            choose(&config.direct, &config.relay, |endpoint| {
                if endpoint == &config.direct {
                    Err(io::Error::other("blocked"))
                } else {
                    Ok(endpoint.address().parse().unwrap())
                }
            }),
            Selection::Relay("203.0.113.8:1234".parse().unwrap())
        );
        assert_eq!(
            choose(&config.direct, &config.relay, |_| Err(io::Error::other(
                "offline"
            ))),
            Selection::Unreachable
        );
    }

    #[test]
    fn a_reachable_login_with_blocked_udp_selects_the_relay() {
        fn sockets() -> (std::net::TcpListener, UdpSocket, UdpSocket) {
            loop {
                let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let port = tcp.local_addr().unwrap().port();
                if port > 65532 {
                    continue;
                }
                if let (Ok(lobby), Ok(game)) = (
                    UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 1)),
                    UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 2)),
                ) {
                    return (tcp, lobby, game);
                }
            }
        }
        let (direct, blocked_lobby, blocked_game) = sockets();
        let (relay, lobby, game) = sockets();
        // Bound but silent UDP ports model a provider dropping the replies.
        let _blocked = (blocked_lobby, blocked_game);
        let direct_endpoint = Endpoint::parse(&direct.local_addr().unwrap().to_string()).unwrap();
        let relay_endpoint = Endpoint::parse(&relay.local_addr().unwrap().to_string()).unwrap();
        std::thread::scope(|scope| {
            for tcp in [direct, relay] {
                scope.spawn(move || {
                    let (mut stream, _) = tcp.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut packet = [0; 22];
                    stream.read_exact(&mut packet).unwrap();
                    stream.write_all(&packet).unwrap();
                });
            }
            for udp in [lobby, game] {
                scope.spawn(move || {
                    udp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut packet = [0; 64];
                    let (n, peer) = udp.recv_from(&mut packet).unwrap();
                    udp.send_to(&packet[..n], peer).unwrap();
                });
            }
            let result = choose(&direct_endpoint, &relay_endpoint, |endpoint| {
                probe(endpoint, Duration::from_millis(100))
            });
            assert_eq!(
                result,
                Selection::Relay(relay_endpoint.address().parse().unwrap())
            );
        });
    }

    #[test]
    fn tcp_and_udp_require_the_exact_challenge() {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let tcp_address = tcp.local_addr().unwrap();
        let udp_address = udp.local_addr().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let (mut stream, _) = tcp.accept().unwrap();
                let mut buffer = [0; 5];
                stream.read_exact(&mut buffer).unwrap();
                stream.write_all(&buffer).unwrap();
            });
            scope.spawn(|| {
                let mut buffer = [0; 64];
                let (n, peer) = udp.recv_from(&mut buffer).unwrap();
                udp.send_to(&buffer[..n], peer).unwrap();
            });
            probe_tcp(tcp_address, b"nonce", Duration::from_secs(1)).unwrap();
            probe_udp(udp_address, b"nonce", Duration::from_secs(1)).unwrap();
        });
    }
}
