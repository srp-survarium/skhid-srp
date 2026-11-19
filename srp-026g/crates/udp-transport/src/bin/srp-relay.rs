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
use udp_transport::relay::{HEADER_SIZE, Protocol, forward_header, forwarded_datagram};

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
impl fmt::Display for Flow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.1} s, up {} packets {} bytes, down {} packets {} bytes",
            self.last.duration_since(self.first).as_secs_f64(),
            self.up.packets,
            self.up.bytes,
            self.down.packets,
            self.down.bytes
        )
    }
}

/// Ordered expiration removes only expired peers. Refreshing one peer does not
/// scan the table or allocate a packet; memory is bounded independently of rate.
#[derive(Default)]
struct Peers {
    seen: HashMap<SocketAddr, Flow>,
    order: BTreeSet<(Instant, SocketAddr)>,
}
impl Peers {
    /// A datagram of `bytes` from `peer`, to forward if the table has room.
    fn touch(&mut self, peer: SocketAddr, now: Instant, bytes: usize) -> bool {
        if let Some(flow) = self.seen.get_mut(&peer) {
            self.order.remove(&(flow.last, peer));
            flow.last = now;
            flow.up.add(bytes);
        } else if self.seen.len() >= MAX_PEERS {
            return false;
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
        true
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
    fn take(&mut self, peer: SocketAddr, now: Instant) -> bool {
        if now >= self.collect {
            self.buckets
                .retain(|_, b| now.duration_since(b.seen) < IDLE);
            self.collect = now + Duration::from_secs(10);
        }
        if !self.buckets.contains_key(&peer.ip()) && self.buckets.len() >= MAX_PEERS {
            return false;
        }
        let b = self.buckets.entry(peer.ip()).or_insert(Bucket {
            tokens: self.burst,
            seen: now,
        });
        b.tokens =
            (b.tokens + now.duration_since(b.seen).as_secs_f64() * self.rate).min(self.burst);
        b.seen = now;
        if b.tokens < 1.0 {
            return false;
        }
        b.tokens -= 1.0;
        true
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
                    eprintln!("{service} {client} idle: {flow}");
                }
            }
        })?;
    let (f, b, p) = (front.clone(), back.clone(), peers.clone());
    std::thread::Builder::new()
        .name(format!("udp-{port}-return"))
        .spawn(move || {
            let mut buffer = [0; HEADER_SIZE + MAX_PACKET + 1];
            loop {
                let Ok(n) = b.recv(&mut buffer) else { continue };
                if n > HEADER_SIZE + MAX_PACKET {
                    continue;
                }
                let Ok((client, bytes)) = forwarded_datagram(&buffer[..n], config.public, port)
                else {
                    continue;
                };
                if lock(&p).reply(client, Instant::now(), bytes.len()) {
                    let _ = f.send_to(bytes, client);
                }
            }
        })?;
    let q = peers.clone();
    std::thread::Builder::new()
        .name(format!("udp-{port}-forward"))
        .spawn(move || {
            let mut buffer = [0; HEADER_SIZE + MAX_PACKET + 1];
            let mut budget = Budget::new(UDP_RATE, UDP_BURST);
            loop {
                let Ok((n, client)) = front.recv_from(&mut buffer[HEADER_SIZE..]) else {
                    continue;
                };
                let now = Instant::now();
                if n > MAX_PACKET || !budget.take(client, now) {
                    continue;
                }
                let Ok(header) = forward_header(client, config.public, port, Protocol::Udp) else {
                    continue;
                };
                if !lock(&q).touch(client, now, n) {
                    continue;
                }
                buffer[..HEADER_SIZE].copy_from_slice(&header);
                let _ = back.send(&buffer[..HEADER_SIZE + n]);
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
/// What one direction of a login connection carried, and how it stopped.
struct Half {
    bytes: u64,
    end: io::Result<()>,
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
            (Err(error), _) => write!(f, "up: {}", Ended(error)),
            (Ok(()), Err(error)) => write!(f, "down: {}", Ended(error)),
            (Ok(()), Ok(())) => f.write_str("closed"),
        }
    }
}
struct Ended<'a>(&'a io::Error);
impl fmt::Display for Ended<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.kind() {
            // The read timeout.
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                write!(f, "idle {} s", IDLE.as_secs())
            }
            _ => self.0.fmt(f),
        }
    }
}

fn connection(mut front: TcpStream, client: SocketAddr, config: Config) -> io::Result<Login> {
    let start = Instant::now();
    let mut back = TcpStream::connect_timeout(
        &SocketAddr::from((config.backend, config.login)),
        Duration::from_secs(10),
    )?;
    front.set_nodelay(true)?;
    back.set_nodelay(true)?;
    for stream in [&front, &back] {
        stream.set_read_timeout(Some(IDLE))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
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
            let down = copy(&mut reverse_back, &mut reverse_front);
            // The server is done with this sign-in: end both sides now, so a
            // silent client does not hold its slot until the idle timeout.
            let _ = reverse_front.shutdown(Shutdown::Both);
            let _ = reverse_back.shutdown(Shutdown::Both);
            down
        });
        let up = copy(&mut front, &mut back);
        let _ = back.shutdown(if up.end.is_ok() {
            Shutdown::Write
        } else {
            Shutdown::Both
        });
        if up.end.is_err() {
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
fn copy(source: &mut TcpStream, destination: &mut TcpStream) -> Half {
    let mut buffer = [0; 16384];
    let mut bytes = 0;
    let mut pass = || loop {
        let n = source.read(&mut buffer)?;
        if n == 0 {
            return Ok(());
        }
        destination.write_all(&buffer[..n])?;
        bytes += n as u64;
    };
    let end = pass();
    Half { bytes, end }
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
fn main() -> io::Result<()> {
    let config = Config::parse()?;
    let stop = block_stop_signals()?;
    let login = TcpListener::bind((config.bind, config.login))?;
    let mut ports = Vec::new();
    for service in Service::ALL {
        ports.push((service, udp(config, service)?));
    }
    // A stop reports the flows still open, which no idle expiry will.
    std::thread::Builder::new()
        .name("stop".into())
        .spawn(move || {
            let mut signal = 0;
            // SAFETY: `stop` is the set blocked in every thread.
            unsafe { libc::sigwait(&stop, &mut signal) };
            for (service, peers) in &ports {
                for (client, flow) in &lock(peers).seen {
                    eprintln!("{service} {client} at stop: {flow}");
                }
            }
            std::process::exit(0);
        })?;
    eprintln!(
        "ready: {}:{} -> {} (native UDP, WireGuard backend)",
        config.public, config.login, config.backend
    );
    let active = Arc::new(AtomicUsize::new(0));
    let mut budget = Budget::new(2.0, 10.0);
    for stream in login.incoming() {
        // One sign-in that fails (reset before it is accepted, no thread for
        // it) is dropped: the process also carries every relayed match.
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("login not accepted: {error}");
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let Ok(client) = stream.peer_addr() else {
            continue;
        };
        if active.load(Ordering::Relaxed) >= 128 || !budget.take(client, Instant::now()) {
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let permit = Permit(active.clone());
        let spawned = std::thread::Builder::new()
            .name("login-forward".into())
            .spawn(move || {
                let _permit = permit;
                match connection(stream, client, config) {
                    Ok(login) => eprintln!("login {client}: {login}"),
                    Err(error) => eprintln!("login {client} not forwarded: {error}"),
                }
            });
        if let Err(error) = spawned {
            eprintln!("login dropped: {error}");
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
            assert!(budget.take(peer, now), "shared exit dropped packet {packet}");
        }
    }

    #[test]
    fn peer_refresh_and_expiry_keep_exactly_one_entry() {
        let mut peers = Peers::default();
        let now = Instant::now();
        let client = "198.51.100.9:12000".parse().unwrap();
        for i in 0..10000 {
            assert!(peers.touch(client, now + Duration::from_micros(i), 100));
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
    }
    #[test]
    fn login_permit_releases_on_teardown() {
        let active = Arc::new(AtomicUsize::new(1));
        drop(Permit(active.clone()));
        assert_eq!(active.load(Ordering::Relaxed), 0);
    }
}
