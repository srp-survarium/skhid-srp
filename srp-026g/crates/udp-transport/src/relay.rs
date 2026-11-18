//! Regional relays over an authenticated, firewall-isolated WireGuard link.
//! Only configured tunnel peers may supply PROXY v2 addresses. Public game
//! sockets keep accepting the original protocol. No credentials live here.
use std::collections::HashMap;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
pub fn is_probe(bytes: &[u8]) -> bool {
    bytes.len() == PROBE_PREFIX.len() + 8 && bytes.starts_with(PROBE_PREFIX)
}

const SIGNATURE: &[u8; 12] = b"\r\n\r\n\0\r\nQUIT\n";
const HEADER: usize = 28;
pub const HEADER_SIZE: usize = HEADER;

#[derive(Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
}

/// A forwarding header made only from an observed IPv4 peer and fixed target.
pub fn forward_header(
    address: SocketAddr,
    public: Ipv4Addr,
    port: u16,
    protocol: Protocol,
) -> io::Result<[u8; HEADER_SIZE]> {
    let IpAddr::V4(ip) = address.ip() else {
        return Err(bad_header());
    };
    if !unicast(ip) || !unicast(public) || address.port() == 0 || port == 0 {
        return Err(bad_header());
    }
    let mut header = encode(address, public, port);
    header[13] = match protocol {
        Protocol::Tcp => 0x11,
        Protocol::Udp => 0x12,
    };
    Ok(header)
}

/// A backend reply, bounded to the configured relay address and service port.
pub fn forwarded_datagram(
    bytes: &[u8],
    public: Ipv4Addr,
    port: u16,
) -> io::Result<(SocketAddr, &[u8])> {
    let peer = decode(bytes, 0x12, public, port)?;
    Ok((peer, &bytes[HEADER..]))
}
const IDLE: Duration = Duration::from_secs(120);
const MAX_ROUTES: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Direct,
    Relayed(Ipv4Addr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer {
    pub address: SocketAddr,
    pub route: Route,
}

/// tunnel IPv4 = advertised IPv4, comma-separated. Fail closed on bad config.
#[derive(Clone, Default)]
pub struct Relays(HashMap<Ipv4Addr, Ipv4Addr>);

impl Relays {
    pub fn parse(value: &str) -> io::Result<Self> {
        let mut peers = HashMap::new();
        if value.is_empty() {
            return Ok(Self(peers));
        }
        for entry in value.split(',') {
            let (tunnel, public) = entry.split_once('=').ok_or_else(bad_header)?;
            let tunnel: Ipv4Addr = tunnel.parse().map_err(|_| bad_header())?;
            let public: Ipv4Addr = public.parse().map_err(|_| bad_header())?;
            if !tunnel.is_private() || !unicast(public) || peers.insert(tunnel, public).is_some() {
                return Err(bad_header());
            }
        }
        Ok(Self(peers))
    }

    pub fn from_env() -> io::Result<Self> {
        match std::env::var("SRP_RELAYS") {
            Ok(value) => Self::parse(&value).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "SRP_RELAYS must be unique private tunnel IPv4=relay IPv4 pairs",
                )
            }),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(error) => Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
        }
    }

    fn public(&self, address: SocketAddr) -> Option<Ipv4Addr> {
        match address.ip().to_canonical() {
            IpAddr::V4(ip) => self.0.get(&ip).copied(),
            IpAddr::V6(_) => None,
        }
    }

    /// Consume exactly one TCP PROXY header from a trusted tunnel peer.
    pub fn accept(&self, stream: &mut TcpStream) -> io::Result<Peer> {
        let address = stream.peer_addr()?;
        let Some(public) = self.public(address) else {
            return Ok(Peer {
                address,
                route: Route::Direct,
            });
        };
        let timeout = stream.read_timeout()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut header = [0; HEADER];
        let read = stream.read_exact(&mut header);
        stream.set_read_timeout(timeout)?;
        read?;
        let address = decode(&header, 0x11, public, stream.local_addr()?.port())?;
        Ok(Peer {
            address,
            route: Route::Relayed(public),
        })
    }
}

fn unicast(ip: Ipv4Addr) -> bool {
    !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast()
}

fn bad_header() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid relay PROXY v2 header")
}

fn decode(bytes: &[u8], protocol: u8, public: Ipv4Addr, port: u16) -> io::Result<SocketAddr> {
    if bytes.len() < HEADER
        || &bytes[..12] != SIGNATURE
        || bytes[12..16] != [0x21, protocol, 0, 12]
        || bytes[20..24] != public.octets()
        || bytes[26..28] != port.to_be_bytes()
    {
        return Err(bad_header());
    }
    let ip = Ipv4Addr::new(bytes[16], bytes[17], bytes[18], bytes[19]);
    let port = u16::from_be_bytes([bytes[24], bytes[25]]);
    if !unicast(ip) || port == 0 {
        return Err(bad_header());
    }
    Ok(SocketAddr::from((ip, port)))
}

fn encode(address: SocketAddr, public: Ipv4Addr, port: u16) -> [u8; HEADER] {
    let mut header = [0; HEADER];
    header[..12].copy_from_slice(SIGNATURE);
    header[12..16].copy_from_slice(&[0x21, 0x12, 0, 12]);
    let IpAddr::V4(ip) = address.ip() else {
        unreachable!("parsed IPv4 relay peer")
    };
    header[16..20].copy_from_slice(&ip.octets());
    header[20..24].copy_from_slice(&public.octets());
    header[24..26].copy_from_slice(&address.port().to_be_bytes());
    header[26..28].copy_from_slice(&port.to_be_bytes());
    header
}

struct ReturnRoute {
    tunnel: SocketAddr,
    public: Ipv4Addr,
    seen: Instant,
}

/// Owns return routing together with receive/send. Bounded and expired by idle
/// time; an exhausted table drops new peers instead of displacing active ones.
pub struct Datagrams<'a> {
    socket: &'a UdpSocket,
    relays: Relays,
    routes: HashMap<SocketAddr, ReturnRoute>,
    next_collect: Instant,
    buffer: Box<[u8; 65536]>,
    port: u16,
}

impl<'a> Datagrams<'a> {
    pub fn new(socket: &'a UdpSocket, relays: Relays) -> io::Result<Self> {
        Ok(Self {
            socket,
            relays,
            routes: HashMap::new(),
            next_collect: Instant::now(),
            buffer: Box::new([0; 65536]),
            port: socket.local_addr()?.port(),
        })
    }

    pub fn recv_from(&mut self, output: &mut [u8]) -> io::Result<(usize, Peer)> {
        let (len, source) = self.socket.recv_from(&mut self.buffer[..])?;
        let now = Instant::now();
        if now >= self.next_collect {
            self.routes
                .retain(|_, route| now.duration_since(route.seen) < IDLE);
            self.next_collect = now + Duration::from_secs(10);
        }
        let (offset, peer) = if let Some(public) = self.relays.public(source) {
            let address =
                decode(&self.buffer[..len], 0x12, public, self.port).map_err(|_| dropped())?;
            if len - HEADER > output.len()
                || (!self.routes.contains_key(&address) && self.routes.len() >= MAX_ROUTES)
            {
                return Err(dropped());
            }
            self.routes.insert(
                address,
                ReturnRoute {
                    tunnel: source,
                    public,
                    seen: now,
                },
            );
            (
                HEADER,
                Peer {
                    address,
                    route: Route::Relayed(public),
                },
            )
        } else {
            // A public peer cannot inject a forwarding header or redirect an
            // active relayed connection's replies by spoofing its address.
            if self.buffer[..len].starts_with(SIGNATURE) || self.routes.contains_key(&source) {
                return Err(dropped());
            }
            (
                0,
                Peer {
                    address: source,
                    route: Route::Direct,
                },
            )
        };
        let payload = &self.buffer[offset..len];
        if payload.len() > output.len() {
            return Err(dropped());
        }
        output[..payload.len()].copy_from_slice(payload);
        let length = payload.len();
        if is_probe(&output[..length]) {
            self.send_to(&output[..length], peer.address)?;
            return Err(dropped());
        }
        Ok((length, peer))
    }

    pub fn send_to(&self, bytes: &[u8], address: SocketAddr) -> io::Result<usize> {
        match self.routes.get(&address) {
            Some(route) if route.seen.elapsed() < IDLE => {
                let mut packet = Vec::with_capacity(HEADER + bytes.len());
                packet.extend_from_slice(&encode(address, route.public, self.port));
                packet.extend_from_slice(bytes);
                self.socket
                    .send_to(&packet, route.tunnel)
                    .map(|n| n.saturating_sub(HEADER))
            }
            Some(_) => Err(dropped()),
            None => self.socket.send_to(bytes, address),
        }
    }
}

fn dropped() -> io::Error {
    // Each loop still polls its timers after a refused datagram.
    io::Error::new(io::ErrorKind::WouldBlock, "relay datagram refused")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_header_is_consumed_once_and_the_encrypted_stream_is_untouched() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let public = "203.0.113.8".parse().unwrap();
        let client = "198.51.100.9:45000".parse().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                let mut bytes = encode(client, public, port);
                bytes[13] = 0x11;
                for byte in bytes {
                    stream.write_all(&[byte]).unwrap();
                }
                stream.write_all(b"cipher").unwrap();
            });
            let (mut stream, _) = listener.accept().unwrap();
            let routes = Relays(HashMap::from([(Ipv4Addr::LOCALHOST, public)]));
            assert_eq!(
                routes.accept(&mut stream).unwrap(),
                Peer {
                    address: client,
                    route: Route::Relayed(public)
                }
            );
            let mut rest = [0; 6];
            stream.read_exact(&mut rest).unwrap();
            assert_eq!(&rest, b"cipher");
        });
    }

    #[test]
    fn reachability_echo_has_no_transport_side_effects() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut socket = Datagrams::new(&server, Relays::default()).unwrap();
        let mut probe = PROBE_PREFIX.to_vec();
        probe.extend_from_slice(b"12345678");
        client
            .send_to(&probe, server.local_addr().unwrap())
            .unwrap();
        let mut buffer = [0; 64];
        assert_eq!(
            socket.recv_from(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let (n, _) = client.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], probe);
        assert!(socket.routes.is_empty());
        assert!(!is_probe(&probe[..probe.len() - 1]));
    }

    #[test]
    fn strict_config() {
        assert!(Relays::parse("").is_ok());
        assert!(Relays::parse("10.77.0.2=203.0.113.8").is_ok());
        for bad in [
            "203.0.113.2=203.0.113.8",
            "10.77.0.2=0.0.0.0",
            "10.77.0.2=203.0.113.8,10.77.0.2=203.0.113.9",
            "wrong",
        ] {
            assert!(Relays::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn every_header_byte_is_checked_and_truncation_is_refused() {
        let client = "198.51.100.4:54321".parse().unwrap();
        let public = "203.0.113.8".parse().unwrap();
        let packet = encode(client, public, 1236);
        assert_eq!(decode(&packet, 0x12, public, 1236).unwrap(), client);
        for len in 0..HEADER {
            assert!(decode(&packet[..len], 0x12, public, 1236).is_err());
        }
        for i in (0..16).chain(20..24).chain(26..28) {
            let mut bad = packet;
            bad[i] ^= 1;
            assert!(decode(&bad, 0x12, public, 1236).is_err(), "{i}");
        }
        assert!(decode(&packet, 0x11, public, 1236).is_err());
        assert!(decode(&packet, 0x12, public, 1237).is_err());
    }

    #[test]
    fn udp_preserves_two_clients_and_returns_only_through_the_relay() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let relay = UdpSocket::bind("127.0.0.1:0").unwrap();
        relay
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let public = "203.0.113.8".parse().unwrap();
        let routes = Relays(HashMap::from([(Ipv4Addr::LOCALHOST, public)]));
        let mut socket = Datagrams::new(&server, routes).unwrap();
        let mut buffer = [0; 2048];
        for client in ["198.51.100.4:54321", "198.51.100.5:54321"] {
            let client = client.parse().unwrap();
            let mut packet = encode(client, public, socket.port).to_vec();
            packet.extend(b"hello");
            relay
                .send_to(&packet, server.local_addr().unwrap())
                .unwrap();
            let (n, peer) = socket.recv_from(&mut buffer).unwrap();
            assert_eq!(
                peer,
                Peer {
                    address: client,
                    route: Route::Relayed(public)
                }
            );
            assert_eq!(&buffer[..n], b"hello");
            socket.send_to(b"answer", peer.address).unwrap();
            let (n, _) = relay.recv_from(&mut buffer).unwrap();
            assert_eq!(
                decode(&buffer[..n], 0x12, public, socket.port).unwrap(),
                client
            );
            assert_eq!(&buffer[HEADER..n], b"answer");
        }
    }

    #[test]
    fn direct_traffic_works_but_public_forwarding_headers_do_not() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut socket = Datagrams::new(&server, Relays::default()).unwrap();
        let mut buffer = [0; 2048];
        client
            .send_to(b"direct", server.local_addr().unwrap())
            .unwrap();
        let (n, peer) = socket.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], b"direct");
        assert_eq!(peer.route, Route::Direct);
        let header = encode(
            client.local_addr().unwrap(),
            Ipv4Addr::LOCALHOST,
            socket.port,
        );
        client
            .send_to(&header, server.local_addr().unwrap())
            .unwrap();
        assert_eq!(
            socket.recv_from(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}
