//! Select the direct route first, then a configured regional relay. Probes
//! exercise login TCP and lobby/match UDP without signing in or joining a game.
//! The relay is selected only when the direct route really fails: its probes
//! run in rounds over a few seconds, by `updater_logic::route::Check`'s rule.
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use updater_logic::diagnosis::Via;
use updater_logic::route::{Check, Next, Probed, Round};

pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
/// A round: the name's lookup and the three probes together wait this long
/// at most, and the direct route's next round starts no sooner. A check is
/// seven rounds at most (`Check`: six of the direct route, the relay's one),
/// 10.5 s; three and one, 6 s, when nothing of the direct route answers.
pub const ROUND: Duration = Duration::from_millis(1500);

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
}

type Resolve = fn(&str, u16) -> io::Result<SocketAddr>;

fn resolve(host: &str, port: u16) -> io::Result<SocketAddr> {
    (host, port)
        .to_socket_addrs()?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| io::Error::other("no IPv4 address"))
}

/// An endpoint's address, asked of DNS once for all the rounds of a check:
/// a slow answer is waited for by the next round, not asked anew.
struct Lookup<'a> {
    endpoint: &'a Endpoint,
    resolve: Resolve,
    name: Name,
}

enum Name {
    Unasked,
    Asked(mpsc::Receiver<io::Result<SocketAddr>>),
    Found(SocketAddr),
}

impl<'a> Lookup<'a> {
    fn new(endpoint: &'a Endpoint) -> Self {
        Self {
            endpoint,
            resolve,
            name: Name::Unasked,
        }
    }

    /// The address, waited for until `deadline`.
    fn address(&mut self, deadline: Instant) -> Option<SocketAddr> {
        if let Name::Unasked = self.name {
            let (send, receive) = mpsc::sync_channel(1);
            let (host, port, resolve) = (self.endpoint.host.clone(), self.endpoint.port, self.resolve);
            // The OS DNS resolver has no portable timeout: its thread ends by
            // itself, and never delays trying the numeric relay IP.
            std::thread::spawn(move || {
                let _ = send.send(resolve(&host, port));
            });
            self.name = Name::Asked(receive);
        }
        if let Name::Asked(receive) = &self.name {
            self.name = match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Ok(address)) => Name::Found(address),
                // Still asked: the next round waits on.
                Err(mpsc::RecvTimeoutError::Timeout) => return None,
                // No such name now (offline, say): the next round asks again.
                Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => Name::Unasked,
            };
        }
        match self.name {
            Name::Found(address) => Some(address),
            Name::Unasked | Name::Asked(_) => None,
        }
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

/// One probe of a round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    Answered,
    /// No answer in the round, or a wrong one: how it failed.
    Lost(io::ErrorKind),
}

/// A round of an endpoint's probes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    /// Its name was not found within the round (DNS).
    NoName,
    Probed {
        address: SocketAddr,
        login: Probe,
        lobby: Probe,
        game: Probe,
    },
}

impl Sample {
    pub fn round(&self) -> Round {
        let Self::Probed {
            login, lobby, game, ..
        } = self
        else {
            return Round::Silent;
        };
        let answered = [login, lobby, game]
            .iter()
            .filter(|probe| ***probe == Probe::Answered)
            .count();
        match answered {
            3 => Round::Answered,
            0 => Round::Silent,
            _ => Round::Partly,
        }
    }

    /// The address all three probes answered at.
    fn answered(&self) -> Option<SocketAddr> {
        match self {
            Self::Probed { address, .. } if self.round() == Round::Answered => Some(*address),
            _ => None,
        }
    }
}

/// For the log: "answered", or each probe ("login TCP ok, lobby UDP
/// TimedOut, match UDP ok").
impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Probed {
            login, lobby, game, ..
        } = self
        else {
            return f.write_str("name not found");
        };
        if self.round() == Round::Answered {
            return f.write_str("answered");
        }
        for (at, (name, probe)) in [("login TCP", login), ("lobby UDP", lobby), ("match UDP", game)]
            .iter()
            .enumerate()
        {
            if at > 0 {
                f.write_str(", ")?;
            }
            match probe {
                Probe::Answered => write!(f, "{name} ok")?,
                Probe::Lost(how) => write!(f, "{name} {how:?}")?,
            }
        }
        Ok(())
    }
}

/// A check: what it selected and every round it ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub selection: Selection,
    /// The direct route's rounds, in order.
    pub direct: Vec<Sample>,
    /// The relay's round, run when the direct route failed its own.
    pub relay: Option<Sample>,
}

/// For the log: "direct in 3 rounds: (1) login TCP ok, lobby UDP TimedOut,
/// match UDP ok; (2) ...; (3) ...; relay: answered".
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rounds = self.direct.len();
        write!(f, "direct in {rounds} round{}:", if rounds == 1 { "" } else { "s" })?;
        for (at, sample) in self.direct.iter().enumerate() {
            write!(f, "{} ({}) {sample}", if at > 0 { ";" } else { "" }, at + 1)?;
        }
        if let Some(sample) = &self.relay {
            write!(f, "; relay: {sample}")?;
        }
        Ok(())
    }
}

/// The check of the routes: the direct route's rounds and, when none of
/// them answered whole, the relay's one. `last`: the route the last game
/// played by.
pub fn select(config: &Config, last: Option<Via>) -> Report {
    let mut direct = Lookup::new(&config.direct);
    let mut relay = Lookup::new(&config.relay);
    choose(last, ROUND, |via| match via {
        Via::Direct => round(&mut direct, ROUND),
        Via::Relay => round(&mut relay, ROUND),
    })
}

/// Whether a route answers now: one round, what a wait asks again and again.
pub fn answers(config: &Config, via: Via) -> bool {
    let endpoint = match via {
        Via::Direct => &config.direct,
        Via::Relay => &config.relay,
    };
    round(&mut Lookup::new(endpoint), ROUND).round() == Round::Answered
}

fn choose(last: Option<Via>, period: Duration, mut round: impl FnMut(Via) -> Sample) -> Report {
    let mut report = Report {
        selection: Selection::Unreachable,
        direct: Vec::new(),
        relay: None,
    };
    let mut check = Check::start(last);
    loop {
        let started = Instant::now();
        let sample = round(check.via());
        match check.via() {
            Via::Direct => report.direct.push(sample),
            Via::Relay => report.relay = Some(sample),
        }
        match check.on(sample.round()) {
            Next::Probe(next) => {
                // A route's rounds are spread over the seconds: one that
                // failed at once (refused, no network) is not asked again at once.
                if next.via() == check.via() {
                    std::thread::sleep(period.saturating_sub(started.elapsed()));
                }
                check = next;
            }
            Next::Found(probed) => {
                report.selection = match (probed, sample.answered()) {
                    (Probed::Direct, Some(address)) => Selection::Direct(address),
                    (Probed::Relay, Some(address)) => Selection::Relay(address),
                    _ => Selection::Unreachable,
                };
                return report;
            }
        }
    }
}

fn round(lookup: &mut Lookup, length: Duration) -> Sample {
    let deadline = Instant::now() + length;
    match lookup.address(deadline) {
        Some(address) => probe(address, deadline),
        None => Sample::NoName,
    }
}

fn probe(address: SocketAddr, deadline: Instant) -> Sample {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_le_bytes();
    let mut packet = PROBE_PREFIX.to_vec();
    packet.extend_from_slice(&nonce[..8]);
    let found = |probe: std::thread::Result<io::Result<()>>| match probe {
        Ok(Ok(())) => Probe::Answered,
        Ok(Err(error)) => Probe::Lost(error.kind()),
        Err(_) => Probe::Lost(io::ErrorKind::Other),
    };
    std::thread::scope(|scope| {
        let tcp = scope.spawn(|| probe_tcp(address, &packet, deadline));
        let lobby = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 1),
                &packet,
                deadline,
            )
        });
        let game = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 2),
                &packet,
                deadline,
            )
        });
        Sample::Probed {
            address,
            login: found(tcp.join()),
            lobby: found(lobby.join()),
            game: found(game.join()),
        }
    })
}

fn probe_tcp(address: SocketAddr, packet: &[u8], deadline: Instant) -> io::Result<()> {
    // What is left of the round (a zero timeout is refused).
    let left = || {
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1))
    };
    let mut stream = TcpStream::connect_timeout(&address, left())?;
    stream.set_write_timeout(Some(left()))?;
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

fn probe_udp(address: SocketAddr, packet: &[u8], deadline: Instant) -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

    const DIRECT: &str = "198.51.100.1:1234";
    const RELAY: &str = "203.0.113.8:1234";
    const LOST: Probe = Probe::Lost(io::ErrorKind::TimedOut);

    fn sample(address: &str, login: Probe, udp: Probe) -> Sample {
        Sample::Probed {
            address: address.parse().unwrap(),
            login,
            lobby: udp,
            game: Probe::Answered,
        }
    }

    /// A check whose rounds find `direct` (its last for as long as asked)
    /// and `relay`, without sockets or waiting.
    fn chosen(last: Option<Via>, direct: &[Sample], relay: Sample) -> Report {
        let mut rounds = direct.iter().chain(std::iter::repeat(direct.last().unwrap()));
        choose(last, Duration::ZERO, |via| match via {
            Via::Direct => *rounds.next().unwrap(),
            Via::Relay => relay,
        })
    }

    #[test]
    fn the_relay_only_when_the_direct_route_fails_its_rounds() {
        let answered = sample(DIRECT, Probe::Answered, Probe::Answered);
        let udp_lost = sample(DIRECT, Probe::Answered, LOST);
        let silent = Sample::Probed {
            address: DIRECT.parse().unwrap(),
            login: LOST,
            lobby: LOST,
            game: LOST,
        };
        let relay = sample(RELAY, Probe::Answered, Probe::Answered);
        let relay_down = sample(RELAY, Probe::Answered, LOST);
        let direct = Selection::Direct(DIRECT.parse().unwrap());
        let relayed = Selection::Relay(RELAY.parse().unwrap());

        // Direct at once: one round, the relay not asked.
        let report = chosen(None, &[answered], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (direct.clone(), 1, None));
        // One lost UDP probe, then an answer: direct.
        let report = chosen(None, &[udp_lost, answered], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (direct.clone(), 2, None));
        // The login TCP answers and UDP never: the relay, after five rounds.
        let report = chosen(None, &[udp_lost], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (relayed.clone(), 5, Some(relay)));
        // Nothing answers, or the name is not found: the relay, after three.
        for nothing in [silent, Sample::NoName] {
            let report = chosen(None, &[nothing], relay);
            assert_eq!((report.selection, report.direct.len(), report.relay), (relayed.clone(), 3, Some(relay)));
        }
        // Nothing and no relay: unreachable.
        let report = chosen(None, &[silent], relay_down);
        assert_eq!(
            (report.selection, report.direct.len(), report.relay),
            (Selection::Unreachable, 3, Some(relay_down))
        );
        // The last game played direct: one more round, which may answer.
        let report = chosen(Some(Via::Direct), &[silent, silent, silent, answered], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (direct, 4, None));
        let report = chosen(Some(Via::Direct), &[silent], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (relayed.clone(), 4, Some(relay)));
        let report = chosen(Some(Via::Relay), &[silent], relay);
        assert_eq!((report.selection, report.direct.len(), report.relay), (relayed, 3, Some(relay)));
    }

    #[test]
    fn the_log_names_each_round_and_each_lost_probe() {
        let udp_lost = sample(DIRECT, Probe::Answered, LOST);
        let relay = sample(RELAY, Probe::Answered, Probe::Answered);
        assert_eq!(
            chosen(None, &[udp_lost, Sample::NoName, sample(DIRECT, Probe::Answered, Probe::Answered)], relay).to_string(),
            "direct in 3 rounds: (1) login TCP ok, lobby UDP TimedOut, match UDP ok; (2) name not found; (3) answered"
        );
        assert_eq!(
            chosen(None, &[Sample::NoName], relay).to_string(),
            "direct in 3 rounds: (1) name not found; (2) name not found; (3) name not found; relay: answered"
        );
        assert_eq!(
            chosen(None, &[sample(DIRECT, Probe::Answered, Probe::Answered)], relay).to_string(),
            "direct in 1 round: (1) answered"
        );
    }

    #[test]
    fn rounds_of_one_route_are_spread_and_the_relay_is_asked_at_once() {
        let period = Duration::from_millis(60);
        let started = Instant::now();
        let mut asked = Vec::new();
        // Every round fails at once, as a refused connection does.
        let report = choose(None, period, |via| {
            asked.push((via, started.elapsed()));
            Sample::NoName
        });
        assert_eq!(report.selection, Selection::Unreachable);
        let [(Via::Direct, _), (Via::Direct, second), (Via::Direct, third), (Via::Relay, relay)] = asked[..] else {
            panic!("{asked:?}");
        };
        assert!(second >= period && third >= 2 * period, "{asked:?}");
        assert!(relay < third + period, "{asked:?}");
    }

    /// The length of a round in the tests with sockets.
    const SHORT: Duration = Duration::from_millis(300);

    /// What a test server's login TCP does with a probe.
    #[derive(Clone, Copy)]
    enum Login {
        Answers,
        /// Takes the connection and closes it.
        Closes,
    }

    /// A server on loopback: its login TCP, and its lobby and match UDP
    /// ports, which each answer after the first `deaf` datagrams.
    struct Server {
        endpoint: Endpoint,
        stop: Arc<AtomicBool>,
        threads: Vec<std::thread::JoinHandle<()>>,
    }

    impl Server {
        fn new(login: Login, deaf: usize) -> Self {
            let (tcp, lobby, game) = loop {
                let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let port = tcp.local_addr().unwrap().port();
                if port > 65532 {
                    continue;
                }
                if let (Ok(lobby), Ok(game)) = (
                    UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 1)),
                    UdpSocket::bind((Ipv4Addr::LOCALHOST, port + 2)),
                ) {
                    break (tcp, lobby, game);
                }
            };
            let endpoint = Endpoint::parse(&tcp.local_addr().unwrap().to_string()).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let pause = Duration::from_millis(10);
            tcp.set_nonblocking(true).unwrap();
            let stopped = stop.clone();
            let mut threads = vec![std::thread::spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = tcp.accept() else {
                        std::thread::sleep(pause);
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                    let mut packet = [0; 22];
                    if matches!(login, Login::Answers) && stream.read_exact(&mut packet).is_ok() {
                        let _ = stream.write_all(&packet);
                    }
                }
            })];
            for udp in [lobby, game] {
                let stopped = stop.clone();
                udp.set_read_timeout(Some(pause)).unwrap();
                threads.push(std::thread::spawn(move || {
                    let mut heard = 0;
                    let mut packet = [0; 64];
                    while !stopped.load(Ordering::Relaxed) {
                        let Ok((n, peer)) = udp.recv_from(&mut packet) else { continue };
                        heard += 1;
                        if heard > deaf {
                            udp.send_to(&packet[..n], peer).unwrap();
                        }
                    }
                }));
            }
            Self { endpoint, stop, threads }
        }

        fn address(&self) -> SocketAddr {
            self.endpoint.address().parse().unwrap()
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            for thread in self.threads.drain(..) {
                let _ = thread.join();
            }
        }
    }

    fn selected(last: Option<Via>, direct: &Server, relay: &Server) -> Report {
        let mut direct = Lookup::new(&direct.endpoint);
        let mut relay = Lookup::new(&relay.endpoint);
        choose(last, SHORT, |via| match via {
            Via::Direct => round(&mut direct, SHORT),
            Via::Relay => round(&mut relay, SHORT),
        })
    }

    #[test]
    fn a_direct_route_that_answers_is_selected_in_one_round() {
        let (direct, relay) = (Server::new(Login::Answers, 0), Server::new(Login::Answers, 0));
        let started = Instant::now();
        let report = selected(None, &direct, &relay);
        assert_eq!(report.selection, Selection::Direct(direct.address()));
        assert_eq!((report.direct.len(), report.relay), (1, None));
        assert!(started.elapsed() < SHORT, "{:?}", started.elapsed());
    }

    #[test]
    fn a_lost_udp_probe_is_asked_again_and_direct_is_selected() {
        // Each UDP port drops its first datagram: one round's worth.
        let (direct, relay) = (Server::new(Login::Answers, 1), Server::new(Login::Answers, 0));
        let report = selected(None, &direct, &relay);
        assert_eq!(report.selection, Selection::Direct(direct.address()));
        assert_eq!(report.relay, None);
        let [first, second] = report.direct[..] else {
            panic!("{report}");
        };
        assert_eq!((first.round(), second.round()), (Round::Partly, Round::Answered), "{report}");
        assert!(
            matches!(first, Sample::Probed { login: Probe::Answered, lobby: Probe::Lost(_), game: Probe::Lost(_), .. }),
            "{report}"
        );
    }

    #[test]
    fn a_reachable_login_with_blocked_udp_selects_the_relay_after_the_retries() {
        // Bound but silent UDP ports model a provider dropping the replies.
        let (direct, relay) = (Server::new(Login::Answers, usize::MAX), Server::new(Login::Answers, 0));
        let report = selected(None, &direct, &relay);
        assert_eq!(report.selection, Selection::Relay(relay.address()), "{report}");
        assert_eq!(report.direct.len(), 5, "{report}");
        assert!(report.direct.iter().all(|sample| sample.round() == Round::Partly), "{report}");
        // The last game played direct: one more round before the relay.
        let report = selected(Some(Via::Direct), &direct, &relay);
        assert_eq!(report.selection, Selection::Relay(relay.address()), "{report}");
        assert_eq!(report.direct.len(), 6, "{report}");
    }

    #[test]
    fn nothing_answering_selects_the_relay_and_without_it_nothing() {
        let silent = || Server::new(Login::Closes, usize::MAX);
        let (direct, relay) = (silent(), Server::new(Login::Answers, 0));
        let started = Instant::now();
        let report = selected(None, &direct, &relay);
        assert_eq!(report.selection, Selection::Relay(relay.address()), "{report}");
        assert_eq!(report.direct.len(), 3, "{report}");
        assert!(report.direct.iter().all(|sample| sample.round() == Round::Silent), "{report}");
        assert!(started.elapsed() >= 3 * SHORT, "{:?}", started.elapsed());
        let report = selected(None, &direct, &silent());
        assert_eq!(report.selection, Selection::Unreachable, "{report}");
        assert_eq!(report.direct.len(), 3, "{report}");
        assert_eq!(report.relay.map(|sample| sample.round()), Some(Round::Silent), "{report}");
    }

    #[test]
    fn a_slow_name_is_asked_once_and_waited_for_by_the_next_round() {
        static ASKED: AtomicUsize = AtomicUsize::new(0);
        static ADDRESS: std::sync::OnceLock<SocketAddr> = std::sync::OnceLock::new();
        fn slow(_: &str, _: u16) -> io::Result<SocketAddr> {
            ASKED.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(SHORT + SHORT / 2);
            Ok(*ADDRESS.get().unwrap())
        }
        let direct = Server::new(Login::Answers, 0);
        ADDRESS.set(direct.address()).unwrap();
        let named = Endpoint::parse("slow.example:1234").unwrap();
        let mut lookup = Lookup {
            endpoint: &named,
            resolve: slow,
            name: Name::Unasked,
        };
        let report = choose(None, SHORT, |_| round(&mut lookup, SHORT));
        assert_eq!(report.selection, Selection::Direct(direct.address()), "{report}");
        assert_eq!(report.direct[0], Sample::NoName, "{report}");
        assert_eq!((report.direct.len(), report.relay), (2, None), "{report}");
        assert_eq!(ASKED.load(Ordering::Relaxed), 1);
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
            let deadline = Instant::now() + Duration::from_secs(1);
            probe_tcp(tcp_address, b"nonce", deadline).unwrap();
            probe_udp(udp_address, b"nonce", deadline).unwrap();
        });
    }
}
