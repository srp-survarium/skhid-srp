//! Native game forwarding. WireGuard authenticates/encrypts the backend link.
//! UDP uses fixed buffers, immediate sends and one blocking receive thread per
//! direction/port; no timer batching, userspace encryption or packet queues.
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use udp_transport::relay::{
    HEADER_SIZE, PROBE_PREFIX, Protocol, forward_header, forwarded_datagram,
};

const MAX_PACKET: usize = 4096;
const MAX_PEERS: usize = 8192;
const IDLE: Duration = Duration::from_secs(120);
// At least the voice server's per-IP allowance: carrier NAT can put forty
// players (under 1,400 voice packets/s together) behind the same address.
const UDP_RATE: f64 = 2000.0;
const UDP_BURST: f64 = 4000.0;

#[derive(Clone, Copy)]
struct Config {
    public: Ipv4Addr,
    backend: Ipv4Addr,
    bind: Ipv4Addr,
    login: u16,
}

impl Config {
    fn parse() -> io::Result<Self> {
        let mut public = None;
        let mut backend = None;
        let mut bind = Ipv4Addr::UNSPECIFIED;
        let mut login = 1234;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args.next().ok_or_else(usage)?;
            match arg.as_str() {
                "--public" => public = Some(value.parse().map_err(|_| usage())?),
                "--backend" => backend = Some(value.parse().map_err(|_| usage())?),
                "--bind" => bind = value.parse().map_err(|_| usage())?,
                "--login-port" => login = value.parse().map_err(|_| usage())?,
                _ => return Err(usage()),
            }
        }
        let config = Self {
            public: public.ok_or_else(usage)?,
            backend: backend.ok_or_else(usage)?,
            bind,
            login,
        };
        if !(1024..=65532).contains(&login)
            || !(config.backend.is_private() || config.backend.is_loopback())
            || config.public.is_unspecified()
            || config.public.is_multicast()
            || config.public.is_broadcast()
        {
            return Err(usage());
        }
        Ok(config)
    }
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "srp-relay --public IPv4 --backend TUNNEL_IP [--bind IPv4] [--login-port 1234]",
    )
}

/// A line on stderr, the journal. A line that cannot be written is let go:
/// the log never stops a thread that forwards (stderr may be a closed pipe).
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr().lock(), $($arg)*);
    }};
}

/// A flow of lobby or match that has sent this long without one reply is
/// reported while it lasts: the server does not answer it.
const UNANSWERED: Duration = Duration::from_secs(10);
/// Login connections forwarded at once, and their per-address allowance.
const LOGINS: usize = 128;
const LOGIN_RATE: f64 = 2.0;
const LOGIN_BURST: f64 = 10.0;

/// The 1st, 10th, 100th, ...: when a count of something that can repeat for
/// every packet or connection is logged.
fn is_power_of_ten(mut n: u64) -> bool {
    while n > 1 && n.is_multiple_of(10) {
        n /= 10;
    }
    n == 1
}

/// The game service on one UDP port: login + 1, + 2, + 3.
#[derive(Clone, Copy)]
enum Service {
    Lobby = 1,
    Match = 2,
    Voice = 3,
}
impl Service {
    const ALL: [Self; 3] = [Self::Lobby, Self::Match, Self::Voice];
    fn port(self, login: u16) -> u16 {
        login + self as u16
    }
    /// Whether the server answers every client that sends: voice sends a
    /// talker nothing while nobody else talks.
    fn answers(self) -> bool {
        !matches!(self, Self::Voice)
    }
}
impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Lobby => "lobby",
            Self::Match => "match",
            Self::Voice => "voice",
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Count {
    packets: u64,
    bytes: u64,
}
impl Count {
    fn add(&mut self, bytes: usize) {
        self.packets += 1;
        self.bytes += bytes as u64;
    }
}

/// One client on one port, from its first datagram to its last.
struct Flow {
    first: Instant,
    last: Instant,
    up: Count,
    down: Count,
}
impl Flow {
    fn lasted(&self) -> Duration {
        self.last.duration_since(self.first)
    }
}
impl fmt::Display for Flow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.1} s, up {} packets {} bytes, down {} packets {} bytes",
            self.lasted().as_secs_f64(),
            self.up.packets,
            self.up.bytes,
            self.down.packets,
            self.down.bytes
        )
    }
}
/// A flow's line once it has ended: idle or at the stop.
struct Ended<'a> {
    service: Service,
    client: SocketAddr,
    flow: &'a Flow,
    how: &'static str,
}
impl fmt::Display for Ended<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            service,
            client,
            flow,
            how,
        } = self;
        write!(f, "{service} {client} {how}: {flow}")?;
        if service.answers() && flow.down.packets == 0 {
            f.write_str(", no reply")?;
        }
        Ok(())
    }
}

/// What a client's datagram did to its flow.
#[derive(Debug, PartialEq)]
enum Touch {
    /// Forward it.
    Forward,
    /// Forward it: the flow has now sent for `UNANSWERED` and had no reply
    /// (said once a flow, at the datagram that crosses it).
    Unanswered(u64),
    /// No room for a new client: drop it.
    Full,
}

/// Ordered expiration removes only expired peers. Refreshing one peer does not
/// scan the table or allocate a packet; memory is bounded independently of rate.
#[derive(Default)]
struct Peers {
    seen: HashMap<SocketAddr, Flow>,
    order: BTreeSet<(Instant, SocketAddr)>,
}
impl Peers {
    /// A datagram of `bytes` from `peer`.
    fn touch(&mut self, peer: SocketAddr, now: Instant, bytes: usize) -> Touch {
        let mut touch = Touch::Forward;
        if let Some(flow) = self.seen.get_mut(&peer) {
            self.order.remove(&(flow.last, peer));
            if flow.down.packets == 0
                && flow.lasted() < UNANSWERED
                && now.duration_since(flow.first) >= UNANSWERED
            {
                touch = Touch::Unanswered(flow.up.packets + 1);
            }
            flow.last = now;
            flow.up.add(bytes);
        } else if self.seen.len() >= MAX_PEERS {
            return Touch::Full;
        } else {
            let mut up = Count::default();
            up.add(bytes);
            let flow = Flow {
                first: now,
                last: now,
                up,
                down: Count::default(),
            };
            self.seen.insert(peer, flow);
        }
        self.order.insert((now, peer));
        touch
    }
    /// A reply of `bytes` to `peer`, to send if it was heard from lately.
    fn reply(&mut self, peer: SocketAddr, now: Instant, bytes: usize) -> bool {
        match self.seen.get_mut(&peer) {
            Some(flow) if now.duration_since(flow.last) < IDLE => {
                flow.down.add(bytes);
                true
            }
            _ => false,
        }
    }
    /// Moves the flows idle since `now - IDLE` into `ended`.
    fn expire(&mut self, now: Instant, ended: &mut Vec<(SocketAddr, Flow)>) {
        while let Some(&(last, peer)) = self.order.first() {
            if now.duration_since(last) < IDLE {
                break;
            }
            self.order.remove(&(last, peer));
            ended.extend(self.seen.remove_entry(&peer));
        }
    }
}

struct Bucket {
    tokens: f64,
    seen: Instant,
}
/// Why a [`Budget`] refused.
#[derive(Debug, PartialEq)]
enum Refusal {
    /// The address has spent its allowance.
    Rate,
    /// `MAX_PEERS` other addresses have buckets.
    Full,
}
struct Budget {
    buckets: HashMap<std::net::IpAddr, Bucket>,
    collect: Instant,
    rate: f64,
    burst: f64,
}
impl Budget {
    fn new(rate: f64, burst: f64) -> Self {
        Self {
            buckets: HashMap::new(),
            collect: Instant::now(),
            rate,
            burst,
        }
    }
    fn take(&mut self, peer: SocketAddr, now: Instant) -> Result<(), Refusal> {
        if now >= self.collect {
            self.buckets
                .retain(|_, b| now.duration_since(b.seen) < IDLE);
            self.collect = now + Duration::from_secs(10);
        }
        if !self.buckets.contains_key(&peer.ip()) && self.buckets.len() >= MAX_PEERS {
            return Err(Refusal::Full);
        }
        let b = self.buckets.entry(peer.ip()).or_insert(Bucket {
            tokens: self.burst,
            seen: now,
        });
        b.tokens =
            (b.tokens + now.duration_since(b.seen).as_secs_f64() * self.rate).min(self.burst);
        b.seen = now;
        if b.tokens < 1.0 {
            return Err(Refusal::Rate);
        }
        b.tokens -= 1.0;
        Ok(())
    }
}

/// A datagram a packet thread did not carry, and why.
enum Lost<'a> {
    // Client to server.
    TooLong(SocketAddr),
    OverRate(SocketAddr),
    Addresses(SocketAddr),
    Clients(SocketAddr),
    Unaddressable(SocketAddr),
    ClientsUnread(&'a io::Error),
    NotSent(&'a io::Error),
    // Server to client.
    ServerUnread(&'a io::Error),
    Unsealed(usize),
    Stranger(SocketAddr),
    Undelivered(SocketAddr, &'a io::Error),
}
impl Lost<'_> {
    const KINDS: usize = 11;
    fn kind(&self) -> usize {
        match self {
            Self::TooLong(_) => 0,
            Self::OverRate(_) => 1,
            Self::Addresses(_) => 2,
            Self::Clients(_) => 3,
            Self::Unaddressable(_) => 4,
            Self::ClientsUnread(_) => 5,
            Self::NotSent(_) => 6,
            Self::ServerUnread(_) => 7,
            Self::Unsealed(_) => 8,
            Self::Stranger(_) => 9,
            Self::Undelivered(..) => 10,
        }
    }
}
impl fmt::Display for Lost<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idle = IDLE.as_secs();
        match self {
            Self::TooLong(client) => {
                write!(
                    f,
                    "a datagram over {MAX_PACKET} bytes from {client} dropped"
                )
            }
            Self::OverRate(client) => write!(
                f,
                "a datagram from {client} dropped: over its address's allowance \
                 ({UDP_RATE}/s, burst {UDP_BURST})"
            ),
            Self::Addresses(client) => write!(
                f,
                "a datagram from {client} dropped: {MAX_PEERS} other addresses are counted"
            ),
            Self::Clients(client) => write!(
                f,
                "a datagram from {client} dropped: {MAX_PEERS} other clients are open"
            ),
            Self::Unaddressable(client) => {
                write!(
                    f,
                    "a datagram from {client} dropped: not an address to forward"
                )
            }
            Self::ClientsUnread(error) => write!(f, "the clients' socket failed: {error}"),
            Self::NotSent(error) => write!(f, "a datagram not sent to the server: {error}"),
            Self::ServerUnread(error) => write!(f, "the server's socket failed: {error}"),
            Self::Unsealed(bytes) => {
                write!(
                    f,
                    "{bytes} bytes from the server dropped: no valid envelope"
                )
            }
            Self::Stranger(client) => write!(
                f,
                "a reply for {client} dropped: not heard from within {idle} s"
            ),
            Self::Undelivered(client, error) => write!(f, "a reply not sent to {client}: {error}"),
        }
    }
}
/// One packet thread's datagrams not carried, by kind: a line at the 1st,
/// 10th, 100th, ... of each kind. Owned by its thread: no lock, no atomic.
struct Drops {
    service: Service,
    counts: [u64; Lost::KINDS],
}
impl Drops {
    fn new(service: Service) -> Self {
        Self {
            service,
            counts: [0; Lost::KINDS],
        }
    }
    fn note(&mut self, lost: Lost) {
        let count = &mut self.counts[lost.kind()];
        *count += 1;
        if is_power_of_ten(*count) {
            say!("{}: {lost}; {count} such so far", self.service);
        }
    }
}

type Shared = Arc<Mutex<Peers>>;
fn lock(peers: &Shared) -> std::sync::MutexGuard<'_, Peers> {
    peers.lock().unwrap_or_else(|e| e.into_inner())
}

fn udp(config: Config, service: Service) -> io::Result<Shared> {
    let port = service.port(config.login);
    let front = Arc::new(UdpSocket::bind((config.bind, port))?);
    let back = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?);
    back.connect((config.backend, port))?;
    // Congestion drops a packet instead of building a latency queue here.
    front.set_write_timeout(Some(Duration::from_millis(1)))?;
    back.set_write_timeout(Some(Duration::from_millis(1)))?;
    let peers = Shared::default();
    let expiring = peers.clone();
    // Expiry is swept here, not by the packet threads: a client that leaves
    // while nobody else sends is still reported.
    std::thread::Builder::new()
        .name(format!("udp-{port}-expire"))
        .spawn(move || {
            let mut ended = Vec::new();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                lock(&expiring).expire(Instant::now(), &mut ended);
                for (client, flow) in ended.drain(..) {
                    let how = "idle";
                    say!(
                        "{}",
                        Ended {
                            service,
                            client,
                            flow: &flow,
                            how
                        }
                    );
                }
            }
        })?;
    let (f, b, p) = (front.clone(), back.clone(), peers.clone());
    std::thread::Builder::new()
        .name(format!("udp-{port}-return"))
        .spawn(move || {
            let mut buffer = [0; HEADER_SIZE + MAX_PACKET + 1];
            let mut drops = Drops::new(service);
            loop {
                let n = match b.recv(&mut buffer) {
                    Ok(n) => n,
                    Err(error) => {
                        drops.note(Lost::ServerUnread(&error));
                        continue;
                    }
                };
                if n > HEADER_SIZE + MAX_PACKET {
                    drops.note(Lost::Unsealed(n));
                    continue;
                }
                let Ok((client, bytes)) = forwarded_datagram(&buffer[..n], config.public, port)
                else {
                    drops.note(Lost::Unsealed(n));
                    continue;
                };
                if !lock(&p).reply(client, Instant::now(), bytes.len()) {
                    drops.note(Lost::Stranger(client));
                } else if let Err(error) = f.send_to(bytes, client) {
                    drops.note(Lost::Undelivered(client, &error));
                }
            }
        })?;
    let q = peers.clone();
    std::thread::Builder::new()
        .name(format!("udp-{port}-forward"))
        .spawn(move || {
            let mut buffer = [0; HEADER_SIZE + MAX_PACKET + 1];
            let mut budget = Budget::new(UDP_RATE, UDP_BURST);
            let mut drops = Drops::new(service);
            loop {
                let (n, client) = match front.recv_from(&mut buffer[HEADER_SIZE..]) {
                    Ok(received) => received,
                    Err(error) => {
                        drops.note(Lost::ClientsUnread(&error));
                        continue;
                    }
                };
                let now = Instant::now();
                if n > MAX_PACKET {
                    drops.note(Lost::TooLong(client));
                    continue;
                }
                match budget.take(client, now) {
                    Ok(()) => {}
                    Err(Refusal::Rate) => {
                        drops.note(Lost::OverRate(client));
                        continue;
                    }
                    Err(Refusal::Full) => {
                        drops.note(Lost::Addresses(client));
                        continue;
                    }
                }
                let Ok(header) = forward_header(client, config.public, port, Protocol::Udp) else {
                    drops.note(Lost::Unaddressable(client));
                    continue;
                };
                let touch = lock(&q).touch(client, now, n);
                match touch {
                    Touch::Forward => {}
                    Touch::Unanswered(packets) if service.answers() => say!(
                        "{service} {client}: {packets} packets up in {} s, none down: \
                         the server does not answer it",
                        UNANSWERED.as_secs()
                    ),
                    Touch::Unanswered(_) => {}
                    Touch::Full => {
                        drops.note(Lost::Clients(client));
                        continue;
                    }
                }
                buffer[..HEADER_SIZE].copy_from_slice(&header);
                if let Err(error) = back.send(&buffer[..HEADER_SIZE + n]) {
                    drops.note(Lost::NotSent(&error));
                }
            }
        })?;
    Ok(peers)
}

struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// An end of a login connection, as the relay sees it.
#[derive(Clone, Copy)]
enum Side {
    Client,
    Server,
}
impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Client => "client",
            Self::Server => "server",
        })
    }
}
/// How one direction of a login connection stopped.
enum End {
    /// Its source closed.
    Closed,
    /// Nothing came for `IDLE` (the read timeout).
    Idle,
    /// Its destination took nothing for the write timeout.
    Stalled(Side),
    /// An error on one side's socket.
    Failed(Side, io::Error),
}
impl End {
    fn read(error: io::Error, side: Side) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Self::Idle,
            _ => Self::Failed(side, error),
        }
    }
    fn write(error: io::Error, side: Side) -> Self {
        match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Self::Stalled(side),
            _ => Self::Failed(side, error),
        }
    }
}
impl fmt::Display for End {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("closed"),
            Self::Idle => write!(f, "idle {} s", IDLE.as_secs()),
            Self::Stalled(side) => write!(f, "{side} took nothing for {} s", WRITE.as_secs()),
            Self::Failed(side, error) => write!(f, "{side}: {error}"),
        }
    }
}
/// What one direction of a login connection carried, and how it stopped.
struct Half {
    bytes: u64,
    /// Its first byte: what the connection is.
    first: Option<u8>,
    end: End,
}
/// A login connection once both directions have stopped.
struct Login {
    lasted: Duration,
    up: Half,
    down: Half,
}
impl fmt::Display for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.1} s, up {} bytes, down {} bytes, ",
            self.lasted.as_secs_f64(),
            self.up.bytes,
            self.down.bytes
        )?;
        match (&self.up.end, &self.down.end) {
            (End::Closed, End::Closed) => f.write_str("closed")?,
            (up, End::Closed) => write!(f, "up: {up}")?,
            (End::Closed, down) => write!(f, "down: {down}")?,
            (up, down) => write!(f, "up: {up}, down: {down}")?,
        }
        if self.down.bytes == 0 {
            f.write_str(", no reply")?;
        }
        Ok(())
    }
}
impl Login {
    /// The updater's route check, or a sign-in (or anything else).
    fn kind(&self) -> &'static str {
        if self.up.first == PROBE_PREFIX.first().copied() {
            "route check"
        } else {
            "login"
        }
    }
}
/// A login connection that was not forwarded.
enum Unforwarded {
    /// The server did not take the connection.
    Unreached(io::Error),
    /// A socket of it failed before anything was forwarded.
    Failed(io::Error),
}
impl From<io::Error> for Unforwarded {
    fn from(error: io::Error) -> Self {
        Self::Failed(error)
    }
}

const WRITE: Duration = Duration::from_secs(10);
fn connection(
    mut front: TcpStream,
    client: SocketAddr,
    config: Config,
) -> Result<Login, Unforwarded> {
    let start = Instant::now();
    let mut back = TcpStream::connect_timeout(
        &SocketAddr::from((config.backend, config.login)),
        Duration::from_secs(10),
    )
    .map_err(Unforwarded::Unreached)?;
    front.set_nodelay(true)?;
    back.set_nodelay(true)?;
    for stream in [&front, &back] {
        stream.set_read_timeout(Some(IDLE))?;
        stream.set_write_timeout(Some(WRITE))?;
    }
    back.write_all(&forward_header(
        client,
        config.public,
        config.login,
        Protocol::Tcp,
    )?)?;
    let mut reverse_front = front.try_clone()?;
    let mut reverse_back = back.try_clone()?;
    let (up, down) = std::thread::scope(|scope| {
        let down = scope.spawn(move || {
            let down = copy(
                &mut reverse_back,
                Side::Server,
                &mut reverse_front,
                Side::Client,
            );
            // The server is done with this sign-in: end both sides now, so a
            // silent client does not hold its slot until the idle timeout.
            let _ = reverse_front.shutdown(Shutdown::Both);
            let _ = reverse_back.shutdown(Shutdown::Both);
            down
        });
        let up = copy(&mut front, Side::Client, &mut back, Side::Server);
        let closed = matches!(up.end, End::Closed);
        let _ = back.shutdown(if closed {
            Shutdown::Write
        } else {
            Shutdown::Both
        });
        if !closed {
            let _ = front.shutdown(Shutdown::Both);
        }
        let down = down
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        (up, down)
    });
    Ok(Login {
        lasted: start.elapsed(),
        up,
        down,
    })
}
fn copy(source: &mut TcpStream, from: Side, destination: &mut TcpStream, to: Side) -> Half {
    let mut buffer = [0; 16384];
    let mut bytes = 0;
    let mut first = None;
    let mut pass = || loop {
        let n = source.read(&mut buffer).map_err(|e| End::read(e, from))?;
        if n == 0 {
            return Ok(());
        }
        first = first.or(Some(buffer[0]));
        destination
            .write_all(&buffer[..n])
            .map_err(|e| End::write(e, to))?;
        bytes += n as u64;
    };
    let end = pass().err().unwrap_or(End::Closed);
    Half { bytes, first, end }
}

/// Blocks SIGTERM and SIGINT in this thread and the threads it starts after,
/// so that only the one waiting for them in `sigwait` takes them.
fn block_stop_signals() -> io::Result<libc::sigset_t> {
    // SAFETY: calls on a signal set of our own.
    unsafe {
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGINT);
        match libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) {
            0 => Ok(set),
            error => Err(io::Error::from_raw_os_error(error)),
        }
    }
}

/// Why the login port did not forward a connection: a line at the 1st,
/// 10th, 100th, ... of each.
#[derive(Default)]
struct Refused {
    busy: u64,
    rate: u64,
    addresses: u64,
    unaccepted: u64,
    threadless: u64,
}
fn refused(count: &mut u64, what: fmt::Arguments) {
    *count += 1;
    if is_power_of_ten(*count) {
        say!("login {what}; {count} such so far");
    }
}

fn main() -> io::Result<()> {
    let config = Config::parse()?;
    // A thread that panics would leave its port unforwarded while the rest
    // goes on: the whole process ends instead, and systemd starts it anew.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        std::process::abort();
    }));
    let stop = block_stop_signals()?;
    let login = TcpListener::bind((config.bind, config.login))?;
    let mut ports = Vec::new();
    for service in Service::ALL {
        ports.push((service, udp(config, service)?));
    }
    let active = Arc::new(AtomicUsize::new(0));
    let open = active.clone();
    // A stop reports the flows still open, which no idle expiry will.
    std::thread::Builder::new()
        .name("stop".into())
        .spawn(move || {
            let mut signal = 0;
            // SAFETY: `stop` is the set blocked in every thread.
            unsafe { libc::sigwait(&stop, &mut signal) };
            let name = if signal == libc::SIGINT {
                "SIGINT"
            } else {
                "SIGTERM"
            };
            let tables: Vec<_> = ports
                .iter()
                .map(|(service, peers)| (*service, lock(peers)))
                .collect();
            let flows = tables
                .iter()
                .map(|(service, peers)| format!("{} {service}", peers.seen.len()))
                .collect::<Vec<_>>()
                .join(", ");
            say!(
                "stopping on {name}: {flows} flows and {} login connections open",
                open.load(Ordering::Relaxed)
            );
            for (service, peers) in &tables {
                for (client, flow) in &peers.seen {
                    let (service, client, how) = (*service, *client, "at stop");
                    say!(
                        "{}",
                        Ended {
                            service,
                            client,
                            flow,
                            how
                        }
                    );
                }
            }
            std::process::exit(0);
        })?;
    say!(
        "ready: {public}, login TCP {login}, lobby UDP {lobby}, match UDP {game}, \
         voice UDP {voice}, to the server {backend} at the same ports (bound to {bind}); \
         idle {idle} s, {MAX_PEERS} clients a port, {UDP_RATE}/s (burst {UDP_BURST}) an \
         address; {LOGINS} logins at once, {LOGIN_RATE}/s (burst {LOGIN_BURST}) an address",
        public = config.public,
        login = config.login,
        lobby = Service::Lobby.port(config.login),
        game = Service::Match.port(config.login),
        voice = Service::Voice.port(config.login),
        backend = config.backend,
        bind = config.bind,
        idle = IDLE.as_secs(),
    );
    let mut budget = Budget::new(LOGIN_RATE, LOGIN_BURST);
    let mut refusals = Refused::default();
    for stream in login.incoming() {
        // One sign-in that fails (reset before it is accepted, no thread for
        // it) is dropped: the process also carries every relayed match.
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                refused(
                    &mut refusals.unaccepted,
                    format_args!("not accepted: {error}"),
                );
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let Ok(client) = stream.peer_addr() else {
            continue;
        };
        if active.load(Ordering::Relaxed) >= LOGINS {
            refused(
                &mut refusals.busy,
                format_args!("{client} refused: {LOGINS} connections open"),
            );
            continue;
        }
        match budget.take(client, Instant::now()) {
            Ok(()) => {}
            Err(Refusal::Rate) => {
                let ip = client.ip();
                refused(
                    &mut refusals.rate,
                    format_args!(
                        "{client} refused: over the allowance of {ip} \
                         ({LOGIN_RATE}/s, burst {LOGIN_BURST})"
                    ),
                );
                continue;
            }
            Err(Refusal::Full) => {
                refused(
                    &mut refusals.addresses,
                    format_args!("{client} refused: {MAX_PEERS} other addresses are counted"),
                );
                continue;
            }
        }
        active.fetch_add(1, Ordering::Relaxed);
        let permit = Permit(active.clone());
        let spawned = std::thread::Builder::new()
            .name("login-forward".into())
            .spawn(move || {
                let _permit = permit;
                match connection(stream, client, config) {
                    Ok(login) => say!("{} {client}: {login}", login.kind()),
                    Err(Unforwarded::Unreached(error)) => say!(
                        "login {client} not forwarded: the server {}:{} not reached: {error}",
                        config.backend,
                        config.login
                    ),
                    Err(Unforwarded::Failed(error)) => {
                        say!("login {client} not forwarded: {error}")
                    }
                }
            });
        if let Err(error) = spawned {
            refused(
                &mut refusals.threadless,
                format_args!("{client} dropped: no thread for it: {error}"),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_shared_exit_can_carry_forty_voice_clients_continuously() {
        let mut budget = Budget::new(UDP_RATE, UDP_BURST);
        let start = Instant::now();
        for packet in 0..14_000 {
            let peer = SocketAddr::from(([198, 51, 100, 9], 20_000 + (packet % 40) as u16));
            let now = start + Duration::from_micros(packet * 1_000_000 / 1400);
            assert_eq!(
                budget.take(peer, now),
                Ok(()),
                "shared exit dropped packet {packet}"
            );
        }
    }

    #[test]
    fn peer_refresh_and_expiry_keep_exactly_one_entry() {
        let mut peers = Peers::default();
        let now = Instant::now();
        let client = "198.51.100.9:12000".parse().unwrap();
        for i in 0..10000 {
            let touch = peers.touch(client, now + Duration::from_micros(i), 100);
            assert_eq!(touch, Touch::Forward);
        }
        assert_eq!(peers.seen.len(), 1);
        assert_eq!(peers.order.len(), 1);
        assert!(peers.reply(client, now + IDLE, 40));
        let later = now + IDLE + Duration::from_secs(1);
        assert!(!peers.reply(client, later, 40));
        let mut ended = Vec::new();
        peers.expire(later, &mut ended);
        assert!(peers.seen.is_empty() && peers.order.is_empty());
        let [(peer, flow)] = &ended[..] else {
            panic!("{} flows ended", ended.len())
        };
        assert_eq!(*peer, client);
        assert_eq!((flow.up.packets, flow.up.bytes), (10000, 1_000_000));
        assert_eq!((flow.down.packets, flow.down.bytes), (1, 40));
        let (service, client, how) = (Service::Match, *peer, "idle");
        assert_eq!(
            Ended {
                service,
                client,
                flow,
                how
            }
            .to_string(),
            "match 198.51.100.9:12000 idle: 0.0 s, up 10000 packets 1000000 bytes, \
             down 1 packets 40 bytes"
        );
    }

    #[test]
    fn a_flow_without_replies_is_said_once_at_ten_seconds() {
        let mut peers = Peers::default();
        let now = Instant::now();
        let client = "198.51.100.9:12000".parse().unwrap();
        let touches: Vec<_> = (0..30)
            .map(|second| peers.touch(client, now + Duration::from_secs(second), 10))
            .collect();
        let said: Vec<_> = touches
            .iter()
            .enumerate()
            .filter(|(_, t)| **t != Touch::Forward)
            .collect();
        assert_eq!(said, [(10, &Touch::Unanswered(11))]);
        // One reply, and a flow that is answered is never said.
        let mut answered = Peers::default();
        answered.touch(client, now, 10);
        assert!(answered.reply(client, now, 10));
        for second in 1..30 {
            let touch = answered.touch(client, now + Duration::from_secs(second), 10);
            assert_eq!(touch, Touch::Forward);
        }
        let flow = &peers.seen[&client];
        let (service, how) = (Service::Lobby, "idle");
        assert!(
            Ended {
                service,
                client,
                flow,
                how
            }
            .to_string()
            .ends_with(", no reply")
        );
        let service = Service::Voice;
        assert!(
            !Ended {
                service,
                client,
                flow,
                how
            }
            .to_string()
            .contains("no reply")
        );
    }

    #[test]
    fn repeated_drops_are_said_at_powers_of_ten() {
        let said: Vec<u64> = (1..=2000).filter(|&n| is_power_of_ten(n)).collect();
        assert_eq!(said, [1, 10, 100, 1000]);
    }

    #[test]
    fn a_login_line_names_each_failed_direction() {
        let failed = |side| End::Failed(side, io::Error::from(io::ErrorKind::ConnectionReset));
        let half = |bytes, end| Half {
            bytes,
            first: Some(b'S'),
            end,
        };
        let login = Login {
            lasted: Duration::from_millis(1200),
            up: half(30_000, failed(Side::Client)),
            down: half(0, End::Stalled(Side::Client)),
        };
        assert_eq!(login.kind(), "route check");
        assert_eq!(
            login.to_string(),
            "1.2 s, up 30000 bytes, down 0 bytes, up: client: connection reset, \
             down: client took nothing for 10 s, no reply"
        );
        assert!(matches!(
            End::read(io::ErrorKind::WouldBlock.into(), Side::Client),
            End::Idle
        ));
    }

    #[test]
    fn login_permit_releases_on_teardown() {
        let active = Arc::new(AtomicUsize::new(1));
        drop(Permit(active.clone()));
        assert_eq!(active.load(Ordering::Relaxed), 0);
    }
}
