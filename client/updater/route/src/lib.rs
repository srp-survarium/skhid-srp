//! Select the direct route first, then a configured regional relay. Probes
//! exercise login TCP and lobby/match UDP without signing in or joining a game:
//! each carries `PROBE_BYTES` each way, which the server echoes.
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
/// What each protocol's probe carries each way, the servers'
/// `udp_transport::relay::PROBE_BYTES`. Some providers cut a flow to foreign
/// hosting after its first 15-20 KB: a small echo passes, the game does not.
pub const PROBE_BYTES: usize = 64 * 1024;
/// The game's largest datagram (`udp_transport::wire::MAX_DATAGRAM`).
const DATAGRAM: usize = 508;
const DATAGRAMS: usize = PROBE_BYTES.div_ceil(DATAGRAM);
/// The three probes move about 200 KB each way together: some 1.6 s at
/// 1 Mbit/s, with time left to send again what a lossy link dropped.
const TIMEOUT: Duration = Duration::from_secs(5);
/// How long the UDP probe waits for answers before it sends the missing again.
const RESEND: Duration = Duration::from_millis(500);

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
        .as_nanos() as u64;
    let stream = challenge(nonce, 0, PROBE_BYTES);
    let datagrams: Vec<_> = (1..=DATAGRAMS)
        .map(|part| challenge(nonce, part, DATAGRAM))
        .collect();
    std::thread::scope(|scope| {
        let tcp = scope.spawn(|| probe_tcp(address, &stream, timeout));
        let lobby = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 1),
                &datagrams,
                timeout,
            )
        });
        let game = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 2),
                &datagrams,
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

/// The prefix, the nonce and the part's number, then filler that does not
/// compress, so that the wire carries every byte.
fn challenge(nonce: u64, part: usize, length: usize) -> Vec<u8> {
    let mut bytes = PROBE_PREFIX.to_vec();
    bytes.extend(nonce.to_le_bytes());
    bytes.extend((part as u16).to_le_bytes());
    let mut state = (nonce ^ ((part as u64) << 32)) | 1;
    while bytes.len() < length {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bytes.extend(state.to_le_bytes());
    }
    bytes.truncate(length);
    bytes
}

fn probe_tcp(address: SocketAddr, challenge: &[u8], timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_write_timeout(Some(timeout))?;
    let mut writer = stream.try_clone()?;
    std::thread::scope(|scope| {
        // Sent while the echo is read: no buffer on the way need hold it all.
        let sending = scope.spawn(move || writer.write_all(challenge));
        let echo = read_echo(&mut stream, challenge.len(), deadline);
        let _ = stream.shutdown(Shutdown::Both);
        sending
            .join()
            .map_err(|_| io::Error::other("TCP probe panicked"))??;
        if echo? != challenge {
            return Err(io::Error::other("wrong TCP probe answer"));
        }
        Ok(())
    })
}

fn read_echo(stream: &mut TcpStream, length: usize, deadline: Instant) -> io::Result<Vec<u8>> {
    let mut reply = vec![0; length];
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
    Ok(reply)
}

/// Every datagram answered by the deadline; the unanswered are sent again
/// each `RESEND`.
fn probe_udp(address: SocketAddr, datagrams: &[Vec<u8>], timeout: Duration) -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
    let deadline = Instant::now() + timeout;
    let mut answered = vec![false; datagrams.len()];
    let mut left = datagrams.len();
    let mut reply = [0; DATAGRAM + 1];
    while left > 0 {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "UDP probe timed out",
            ));
        }
        for (datagram, _) in datagrams.iter().zip(&answered).filter(|(_, done)| !**done) {
            socket.send(datagram)?;
        }
        let round = (Instant::now() + RESEND).min(deadline);
        while left > 0 {
            let wait = round.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                break;
            }
            socket.set_read_timeout(Some(wait))?;
            match socket.recv(&mut reply) {
                Ok(n) => {
                    if let Some(part) = datagrams.iter().position(|d| d[..] == reply[..n])
                        && !answered[part]
                    {
                        answered[part] = true;
                        left -= 1;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
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

    /// A stand-in server whose login, lobby and match echo `cuts` bytes each
    /// at most, as a provider that cuts a flow after so much would.
    fn stand_in(cuts: [usize; 3]) -> Endpoint {
        let (tcp, udp) = loop {
            let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = tcp.local_addr().unwrap().port();
            if port > 65532 {
                continue;
            }
            if let (Ok(lobby), Ok(game)) = (
                UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 1)),
                UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 2)),
            ) {
                break (tcp, [lobby, game]);
            }
        };
        let endpoint = Endpoint::parse(&tcp.local_addr().unwrap().to_string()).unwrap();
        let [login, lobby, game] = cuts;
        std::thread::spawn(move || {
            let (mut stream, _) = tcp.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut left = login;
            let mut buffer = [0; 4096];
            // Past the cut, what comes is taken and nothing answered.
            while let Ok(n @ 1..) = stream.read(&mut buffer) {
                let echoed = n.min(left);
                stream.write_all(&buffer[..echoed]).unwrap();
                left -= echoed;
            }
        });
        for (udp, mut left) in udp.into_iter().zip([lobby, game]) {
            std::thread::spawn(move || {
                udp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut buffer = [0; 2048];
                while let Ok((n, peer)) = udp.recv_from(&mut buffer) {
                    if n <= left {
                        udp.send_to(&buffer[..n], peer).unwrap();
                        left -= n;
                    }
                }
            });
        }
        endpoint
    }

    #[test]
    fn a_route_that_cuts_any_flow_short_selects_the_relay() {
        const ALL: usize = usize::MAX;
        // Silent UDP: a provider dropping the replies. 20 KB: one cutting
        // a flow after that much.
        for cuts in [
            [ALL, 0, 0],
            [20_000, ALL, ALL],
            [ALL, 20_000, ALL],
            [ALL, ALL, 20_000],
        ] {
            let direct = stand_in(cuts);
            let relay = stand_in([ALL; 3]);
            let result = choose(&direct, &relay, |endpoint| {
                probe(endpoint, Duration::from_millis(500))
            });
            assert_eq!(
                result,
                Selection::Relay(relay.address().parse().unwrap()),
                "{cuts:?}"
            );
        }
    }

    #[test]
    fn a_whole_echo_passes_well_inside_the_timeout() {
        let direct = stand_in([usize::MAX; 3]);
        let started = Instant::now();
        probe(&direct, TIMEOUT).unwrap();
        eprintln!("probe on loopback: {:?}", started.elapsed());
        assert!(started.elapsed() < TIMEOUT / 5);
    }

    #[test]
    fn tcp_and_udp_require_the_exact_challenge() {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let tcp_address = tcp.local_addr().unwrap();
        let udp_address = udp.local_addr().unwrap();
        std::thread::scope(|scope| {
            // Each answers with its last byte changed.
            scope.spawn(|| {
                let (mut stream, _) = tcp.accept().unwrap();
                let mut buffer = [0; 5];
                stream.read_exact(&mut buffer).unwrap();
                buffer[4] ^= 1;
                stream.write_all(&buffer).unwrap();
            });
            scope.spawn(|| {
                udp.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                let mut buffer = [0; 64];
                while let Ok((n, peer)) = udp.recv_from(&mut buffer) {
                    buffer[n - 1] ^= 1;
                    udp.send_to(&buffer[..n], peer).unwrap();
                }
            });
            let timeout = Duration::from_millis(300);
            assert!(probe_tcp(tcp_address, b"nonce", timeout).is_err());
            assert!(probe_udp(udp_address, &[b"nonce".to_vec()], timeout).is_err());
        });
    }
}
