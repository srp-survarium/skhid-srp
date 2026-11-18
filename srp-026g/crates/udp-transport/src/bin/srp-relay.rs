//! Native game forwarding. WireGuard authenticates/encrypts the backend link.
//! UDP uses fixed buffers, immediate sends and one blocking receive thread per
//! direction/port; no timer batching, userspace encryption or packet queues.
use std::collections::{BTreeSet, HashMap};
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

/// Ordered expiration removes only expired peers. Refreshing one peer does not
/// scan the table or allocate a packet; memory is bounded independently of rate.
#[derive(Default)]
struct Peers {
    seen: HashMap<SocketAddr, Instant>,
    order: BTreeSet<(Instant, SocketAddr)>,
}
impl Peers {
    fn touch(&mut self, peer: SocketAddr, now: Instant) -> bool {
        while let Some(&(seen, old)) = self.order.first() {
            if now.duration_since(seen) < IDLE {
                break;
            }
            self.order.remove(&(seen, old));
            self.seen.remove(&old);
        }
        if let Some(previous) = self.seen.get(&peer) {
            self.order.remove(&(*previous, peer));
        } else if self.seen.len() >= MAX_PEERS {
            return false;
        }
        self.seen.insert(peer, now);
        self.order.insert((now, peer));
        true
    }
    fn contains(&self, peer: SocketAddr, now: Instant) -> bool {
        self.seen
            .get(&peer)
            .is_some_and(|seen| now.duration_since(*seen) < IDLE)
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

fn udp(config: Config, port: u16) -> io::Result<()> {
    let front = Arc::new(UdpSocket::bind((config.bind, port))?);
    let back = Arc::new(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?);
    back.connect((config.backend, port))?;
    // Congestion drops a packet instead of building a latency queue here.
    front.set_write_timeout(Some(Duration::from_millis(1)))?;
    back.set_write_timeout(Some(Duration::from_millis(1)))?;
    let peers = Arc::new(Mutex::new(Peers::default()));
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
                let known = p
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains(client, Instant::now());
                if known {
                    let _ = f.send_to(bytes, client);
                }
            }
        })?;
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
                if !peers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .touch(client, now)
                {
                    continue;
                }
                buffer[..HEADER_SIZE].copy_from_slice(&header);
                let _ = back.send(&buffer[..HEADER_SIZE + n]);
            }
        })?;
    Ok(())
}

struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
fn connection(mut front: TcpStream, config: Config) -> io::Result<()> {
    let client = front.peer_addr()?;
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
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let result = copy(&mut reverse_back, &mut reverse_front);
            let _ = reverse_front.shutdown(if result.is_ok() {
                Shutdown::Write
            } else {
                Shutdown::Both
            });
            if result.is_err() {
                let _ = reverse_back.shutdown(Shutdown::Both);
            }
        });
        let result = copy(&mut front, &mut back);
        let _ = back.shutdown(if result.is_ok() {
            Shutdown::Write
        } else {
            Shutdown::Both
        });
        if result.is_err() {
            let _ = front.shutdown(Shutdown::Both);
        }
        result
    })
}
fn copy(source: &mut TcpStream, destination: &mut TcpStream) -> io::Result<()> {
    let mut buffer = [0; 16384];
    loop {
        let n = source.read(&mut buffer)?;
        if n == 0 {
            return Ok(());
        }
        destination.write_all(&buffer[..n])?;
    }
}
fn main() -> io::Result<()> {
    let config = Config::parse()?;
    let login = TcpListener::bind((config.bind, config.login))?;
    for port in config.login + 1..=config.login + 3 {
        udp(config, port)?;
    }
    eprintln!(
        "ready: {}:{} -> {} (native UDP, WireGuard backend)",
        config.public, config.login, config.backend
    );
    let active = Arc::new(AtomicUsize::new(0));
    let mut budget = Budget::new(2.0, 10.0);
    for stream in login.incoming() {
        let stream = stream?;
        if active.load(Ordering::Relaxed) >= 128
            || !budget.take(stream.peer_addr()?, Instant::now())
        {
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let permit = Permit(active.clone());
        std::thread::Builder::new()
            .name("login-forward".into())
            .spawn(move || {
                let _permit = permit;
                if let Err(error) = connection(stream, config) {
                    eprintln!("login ended: {error}");
                }
            })?;
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
            assert!(peers.touch(client, now + Duration::from_micros(i)));
        }
        assert_eq!(peers.seen.len(), 1);
        assert_eq!(peers.order.len(), 1);
        assert!(peers.contains(client, now + IDLE));
        assert!(!peers.contains(client, now + IDLE + Duration::from_secs(1)));
    }
    #[test]
    fn login_permit_releases_on_teardown() {
        let active = Arc::new(AtomicUsize::new(1));
        drop(Permit(active.clone()));
        assert_eq!(active.load(Ordering::Relaxed), 0);
    }
}
