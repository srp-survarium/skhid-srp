//! Select the direct route first, then a configured regional relay. Probes
//! exercise login TCP and lobby/match UDP without signing in or joining a game:
//! each carries `PROBE_BYTES` each way, which the server echoes.
//! The relay is selected only when the direct route really fails: its probes
//! run in rounds, by `updater_logic::route::Check`'s rule. A probe fails as
//! soon as its flow stops moving ([`STALL`]): a block, silent or cutting a
//! flow after some KB, is known in a second, a slow link that still moves
//! has the whole [`ROUND`].
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use updater_logic::diagnosis::Via;
use updater_logic::route::{Check, Next, Probed, Round};

pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
/// What each protocol's probe carries each way, the servers'
/// `udp_transport::relay::PROBE_BYTES`. Some providers cut a flow to foreign
/// hosting after its first 15-20 KB: a small echo passes, the game does not.
pub const PROBE_BYTES: usize = 64 * 1024;
/// The game's largest datagram (`udp_transport::wire::MAX_DATAGRAM`).
const DATAGRAM: usize = 508;
const DATAGRAMS: usize = PROBE_BYTES.div_ceil(DATAGRAM);
/// How long a round's probes run at most while their flows still move. The
/// three move about 200 KB each way together: some 1.6 s at 1 Mbit/s, with
/// time left to send again what a lossy link dropped.
pub const ROUND: Duration = Duration::from_secs(5);
/// A probe whose flow moved nothing for this long has failed: its
/// connection not made, no byte or datagram of the echo more (a provider's
/// cut, a silent block). Two of the UDP probe's resends ([`RESEND`]). Also
/// the wait for the server's name in a round (a slow name is waited for by
/// the next round).
pub const STALL: Duration = Duration::from_secs(1);
/// The direct route's next round starts no sooner than this after the one
/// before: a round that failed at once (refused, no network) is not asked
/// again at once.
pub const SPREAD: Duration = Duration::from_secs(1);
/// How long the UDP probe waits for answers before it sends the missing again.
const RESEND: Duration = Duration::from_millis(500);

/// A check's times: [`TIMES`]; the tests' own are shorter.
#[derive(Clone, Copy, Debug)]
struct Times {
    stall: Duration,
    round: Duration,
    spread: Duration,
}

const TIMES: Times = Times {
    stall: STALL,
    round: ROUND,
    spread: SPREAD,
};

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
/// them answered whole, the relay's one. `last`: the route found last.
pub fn select(config: &Config, last: Option<Via>) -> Report {
    let mut direct = Lookup::new(&config.direct);
    let mut relay = Lookup::new(&config.relay);
    choose(last, TIMES.spread, |via| match via {
        Via::Direct => round(&mut direct, TIMES),
        Via::Relay => round(&mut relay, TIMES),
    })
}

/// Whether a route answers now: one round, what a wait asks again and again.
pub fn answers(config: &Config, via: Via) -> bool {
    let endpoint = match via {
        Via::Direct => &config.direct,
        Via::Relay => &config.relay,
    };
    round(&mut Lookup::new(endpoint), TIMES).round() == Round::Answered
}

fn choose(last: Option<Via>, spread: Duration, mut round: impl FnMut(Via) -> Sample) -> Report {
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
                    std::thread::sleep(spread.saturating_sub(started.elapsed()));
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

fn round(lookup: &mut Lookup, times: Times) -> Sample {
    match lookup.address(Instant::now() + times.stall) {
        Some(address) => probe(address, times),
        None => Sample::NoName,
    }
}

fn probe(address: SocketAddr, times: Times) -> Sample {
    let clock = Clock {
        stall: times.stall,
        end: Instant::now() + times.round,
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let stream = challenge(nonce, 0, PROBE_BYTES);
    let datagrams: Vec<_> = (1..=DATAGRAMS)
        .map(|part| challenge(nonce, part, DATAGRAM))
        .collect();
    let found = |probe: std::thread::Result<io::Result<()>>| match probe {
        Ok(Ok(())) => Probe::Answered,
        Ok(Err(error)) => Probe::Lost(error.kind()),
        Err(_) => Probe::Lost(io::ErrorKind::Other),
    };
    std::thread::scope(|scope| {
        let tcp = scope.spawn(|| probe_tcp(address, &stream, clock));
        let lobby = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 1),
                &datagrams,
                clock,
            )
        });
        let game = scope.spawn(|| {
            probe_udp(
                SocketAddr::new(address.ip(), address.port() + 2),
                &datagrams,
                clock,
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

/// A probe's time: it fails once its flow has not moved for `stall`, and
/// at `end` (the round's) whatever it does.
#[derive(Clone, Copy, Debug)]
struct Clock {
    stall: Duration,
    end: Instant,
}

impl Clock {
    /// How long to wait for the flow, which last moved at `moved`; `None`
    /// once it stalled or the round ended.
    fn wait(self, moved: Instant) -> Option<Duration> {
        let left = (moved + self.stall)
            .min(self.end)
            .saturating_duration_since(Instant::now());
        (!left.is_zero()).then_some(left)
    }

    /// What is left of the round (a zero timeout is refused).
    fn left(self) -> Duration {
        self.end
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1))
    }
}

fn stalled(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, format!("{what} probe stalled"))
}

fn probe_tcp(address: SocketAddr, challenge: &[u8], clock: Clock) -> io::Result<()> {
    let connect = clock.wait(Instant::now()).ok_or_else(|| stalled("TCP"))?;
    let mut stream = TcpStream::connect_timeout(&address, connect)?;
    // The sender may wait for as long as the echo moves; it ends with the stream.
    stream.set_write_timeout(Some(clock.left()))?;
    let mut writer = stream.try_clone()?;
    std::thread::scope(|scope| {
        // Sent while the echo is read: no buffer on the way need hold it all.
        let sending = scope.spawn(move || writer.write_all(challenge));
        let echo = read_echo(&mut stream, challenge.len(), clock);
        let _ = stream.shutdown(Shutdown::Both);
        let sent = sending
            .join()
            .map_err(|_| io::Error::other("TCP probe panicked"))?;
        // A stalled echo is the failure, not the send it cut short.
        if echo? != challenge {
            return Err(io::Error::other("wrong TCP probe answer"));
        }
        sent
    })
}

fn read_echo(stream: &mut TcpStream, length: usize, clock: Clock) -> io::Result<Vec<u8>> {
    let mut reply = vec![0; length];
    let mut read = 0;
    let mut moved = Instant::now();
    while read < reply.len() {
        let wait = clock.wait(moved).ok_or_else(|| stalled("TCP"))?;
        stream.set_read_timeout(Some(wait))?;
        let count = match stream.read(&mut reply[read..]) {
            Ok(count) => count,
            Err(error) if matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {
                return Err(stalled("TCP"));
            }
            Err(error) => return Err(error),
        };
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "probe ended early",
            ));
        }
        read += count;
        moved = Instant::now();
    }
    Ok(reply)
}

/// Every datagram answered while answers keep coming; the unanswered are
/// sent again each `RESEND`.
fn probe_udp(address: SocketAddr, datagrams: &[Vec<u8>], clock: Clock) -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(address)?;
    let mut answered = vec![false; datagrams.len()];
    let mut left = datagrams.len();
    let mut reply = [0; DATAGRAM + 1];
    let mut moved = Instant::now();
    while left > 0 {
        clock.wait(moved).ok_or_else(|| stalled("UDP"))?;
        for (datagram, _) in datagrams.iter().zip(&answered).filter(|(_, done)| !**done) {
            socket.send(datagram)?;
        }
        let resend = Instant::now() + RESEND;
        while left > 0 {
            let Some(wait) = clock.wait(moved) else { break };
            let wait = wait.min(resend.saturating_duration_since(Instant::now()));
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
                        moved = Instant::now();
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
        // Direct found last: one more round, which may answer.
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

    /// The times of the tests with sockets that count rounds, not seconds.
    const SHORT: Times = Times {
        stall: Duration::from_millis(150),
        round: Duration::from_millis(600),
        spread: Duration::from_millis(150),
    };

    /// Every byte, as a flow nothing cuts.
    const ALL: usize = usize::MAX;

    /// A server on loopback: its login TCP, and its lobby and match UDP
    /// ports. Each echoes `cuts` bytes of a probe at most, as a provider that
    /// cuts a flow after so much would (past the cut, what comes is taken and
    /// nothing answered); the UDP ports answer nothing of the first `deaf`
    /// probes.
    struct Server {
        endpoint: Endpoint,
        stop: Arc<AtomicBool>,
        threads: Vec<std::thread::JoinHandle<()>>,
    }

    impl Server {
        fn new(cuts: [usize; 3], deaf: usize) -> Self {
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
            let [login, lobby_cut, game_cut] = cuts;
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
                    let mut left = login;
                    let mut buffer = [0; 4096];
                    while let Ok(n @ 1..) = stream.read(&mut buffer) {
                        let echoed = n.min(left);
                        if stream.write_all(&buffer[..echoed]).is_err() {
                            break;
                        }
                        left -= echoed;
                    }
                }
            })];
            for (udp, cut) in [(lobby, lobby_cut), (game, game_cut)] {
                let stopped = stop.clone();
                udp.set_read_timeout(Some(pause)).unwrap();
                threads.push(std::thread::spawn(move || {
                    // A probe is told by its nonce (`challenge`).
                    let (mut nonce, mut probes, mut left) = ([0; 8], 0, cut);
                    let mut packet = [0; 2048];
                    while !stopped.load(Ordering::Relaxed) {
                        let Ok((n, peer)) = udp.recv_from(&mut packet) else { continue };
                        if packet[14..22] != nonce {
                            nonce.copy_from_slice(&packet[14..22]);
                            probes += 1;
                            left = cut;
                        }
                        if probes > deaf && n <= left {
                            udp.send_to(&packet[..n], peer).unwrap();
                            left -= n;
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

    fn selected(last: Option<Via>, direct: &Endpoint, relay: &Server, times: Times) -> Report {
        let mut direct = Lookup::new(direct);
        let mut relay = Lookup::new(&relay.endpoint);
        choose(last, times.spread, |via| match via {
            Via::Direct => round(&mut direct, times),
            Via::Relay => round(&mut relay, times),
        })
    }

    /// A check with the players' times: what it selected, its rounds of the
    /// direct route, and how long it took (`bound` at most: the case's
    /// worst wait in the README, on loopback, with a second to spare for a
    /// busy machine).
    fn timed(last: Option<Via>, direct: &Endpoint, relay: &Server, bound: Duration) -> (Selection, usize, Duration) {
        let started = Instant::now();
        let report = selected(last, direct, relay, TIMES);
        let took = started.elapsed();
        eprintln!("{report} -> {:?} in {took:?}", report.selection);
        assert!(took < bound + Duration::from_secs(1), "{took:?}: {report}");
        (report.selection, report.direct.len(), took)
    }

    /// An address nothing listens at: refused at once.
    fn closed() -> Endpoint {
        let server = Server::new([ALL; 3], 0);
        let endpoint = server.endpoint.clone();
        drop(server);
        endpoint
    }

    #[test]
    fn a_direct_route_that_answers_is_selected_in_one_round() {
        let (direct, relay) = (Server::new([ALL; 3], 0), Server::new([ALL; 3], 0));
        let (selection, rounds, _) = timed(None, &direct.endpoint, &relay, Duration::ZERO);
        assert_eq!((selection, rounds), (Selection::Direct(direct.address()), 1));
    }

    #[test]
    fn a_whole_echo_passes_well_inside_the_round() {
        let direct = Server::new([ALL; 3], 0);
        let started = Instant::now();
        let sample = probe(direct.address(), TIMES);
        eprintln!("probe on loopback: {:?}", started.elapsed());
        assert_eq!(sample.round(), Round::Answered, "{sample}");
        assert!(started.elapsed() < STALL / 2);
    }

    #[test]
    fn a_flow_cut_short_is_a_lost_probe_known_once_it_stalls() {
        // 20 KB: a provider cutting a flow after that much; 0: one dropping it.
        for cuts in [[ALL, 0, 0], [20_000, ALL, ALL], [ALL, 20_000, ALL], [ALL, ALL, 20_000]] {
            let direct = Server::new(cuts, 0);
            let started = Instant::now();
            let sample = probe(direct.address(), SHORT);
            let Sample::Probed { login, lobby, game, .. } = sample else {
                panic!("{sample}");
            };
            for (probe, cut) in [login, lobby, game].into_iter().zip(cuts) {
                assert_eq!(probe == Probe::Answered, cut == ALL, "{cuts:?}: {sample}");
            }
            // Known when the flow stopped, not at the round's end.
            assert!(started.elapsed() < SHORT.round / 2, "{:?}", started.elapsed());
        }
    }

    #[test]
    fn a_lost_udp_probe_is_asked_again_and_direct_is_selected() {
        // Each UDP port answers nothing of the first probe: one round's worth.
        let (direct, relay) = (Server::new([ALL; 3], 1), Server::new([ALL; 3], 0));
        let report = selected(None, &direct.endpoint, &relay, SHORT);
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
    fn a_reachable_login_with_cut_udp_selects_the_relay_after_the_retries() {
        let (direct, relay) = (Server::new([ALL, 20_000, 20_000], 0), Server::new([ALL; 3], 0));
        // Five rounds of a second each (UDP stalls past its cut).
        let (selection, rounds, _) = timed(None, &direct.endpoint, &relay, 5 * STALL);
        assert_eq!((selection, rounds), (Selection::Relay(relay.address()), 5));
        // Direct found last: one more round before the relay.
        let (selection, rounds, _) = timed(Some(Via::Direct), &direct.endpoint, &relay, 6 * STALL);
        assert_eq!((selection, rounds), (Selection::Relay(relay.address()), 6));
        let report = selected(None, &direct.endpoint, &relay, SHORT);
        assert!(report.direct.iter().all(|sample| sample.round() == Round::Partly), "{report}");
    }

    #[test]
    fn nothing_answering_selects_the_relay_within_seconds() {
        let relay = Server::new([ALL; 3], 0);
        // Silent (connections taken, nothing echoed): three rounds of a second.
        let silent = Server::new([0; 3], 0);
        let (selection, rounds, took) = timed(None, &silent.endpoint, &relay, 3 * STALL);
        assert_eq!((selection, rounds), (Selection::Relay(relay.address()), 3));
        assert!(took >= 3 * STALL - Duration::from_millis(100), "{took:?}");
        // Cut after 20 KB, every flow: as silent, once each has stalled.
        let cut = Server::new([20_000; 3], 0);
        let (selection, rounds, _) = timed(None, &cut.endpoint, &relay, 3 * STALL);
        assert_eq!((selection, rounds), (Selection::Relay(relay.address()), 3));
        // Refused at once: the rounds' spread, 2 s.
        let (selection, rounds, took) = timed(None, &closed(), &relay, 2 * SPREAD);
        assert_eq!((selection, rounds), (Selection::Relay(relay.address()), 3));
        assert!(took >= 2 * SPREAD, "{took:?}");
    }

    #[test]
    fn nothing_answering_either_way_selects_nothing() {
        let silent = || Server::new([0; 3], 0);
        let report = selected(None, &silent().endpoint, &silent(), SHORT);
        assert_eq!(report.selection, Selection::Unreachable, "{report}");
        assert_eq!(report.direct.len(), 3, "{report}");
        assert!(report.direct.iter().all(|sample| sample.round() == Round::Silent), "{report}");
        assert_eq!(report.relay.map(|sample| sample.round()), Some(Round::Silent), "{report}");
    }

    #[test]
    fn a_slow_name_is_asked_once_and_waited_for_by_the_next_round() {
        static ASKED: AtomicUsize = AtomicUsize::new(0);
        static ADDRESS: std::sync::OnceLock<SocketAddr> = std::sync::OnceLock::new();
        fn slow(_: &str, _: u16) -> io::Result<SocketAddr> {
            ASKED.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(SHORT.stall + SHORT.stall / 2);
            Ok(*ADDRESS.get().unwrap())
        }
        let direct = Server::new([ALL; 3], 0);
        ADDRESS.set(direct.address()).unwrap();
        let named = Endpoint::parse("slow.example:1234").unwrap();
        let mut lookup = Lookup {
            endpoint: &named,
            resolve: slow,
            name: Name::Unasked,
        };
        let report = choose(None, SHORT.spread, |_| round(&mut lookup, SHORT));
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
            let clock = Clock { stall: SHORT.stall, end: Instant::now() + SHORT.round };
            assert!(probe_tcp(tcp_address, b"nonce", clock).is_err());
            assert!(probe_udp(udp_address, &[b"nonce".to_vec()], clock).is_err());
        });
    }
}
