//! Regional relays over an authenticated, firewall-isolated WireGuard link.
//! Only configured tunnel peers may supply PROXY v2 addresses. Public game
//! sockets keep accepting the original protocol. No credentials live here.
//!
//! A peer is its address and the route it came by ([`Peer`]): the servers key
//! their links, seats and voice sessions by it, and a reply leaves by the
//! route of the peer it is for. No table here maps an address to a route.
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, UdpSocket};
use std::time::Duration;

/// The updater's route check (`client/updater/route`) starts with this. It is
/// answered with exactly its own bytes and goes no further.
pub const PROBE_PREFIX: &[u8] = b"SKHID-ROUTE-1:";
/// The most of one login connection's check that is echoed: the updater's
/// `PROBE_BYTES`.
pub const PROBE_BYTES: u64 = 64 * 1024;
/// A check's datagram: one of the game's size at most.
pub fn is_probe(bytes: &[u8]) -> bool {
    bytes.len() <= crate::MAX_DATAGRAM && bytes.starts_with(PROBE_PREFIX)
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
    Ok((SocketAddr::V4(peer), &bytes[HEADER..]))
}

/// One relay of `SRP_RELAYS`: its address inside the tunnel and the public
/// address it is advertised by. Made only from that setting ([`Relays`]),
/// never from a datagram's bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Relay {
    tunnel: Ipv4Addr,
    public: Ipv4Addr,
}

impl Relay {
    /// The address players reach this relay by.
    pub fn public(self) -> Ipv4Addr {
        self.public
    }
}

/// The way a peer's datagrams and connections come in, and its answers go out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Route {
    Direct,
    Relay(Relay),
}

/// Who sent: the client's own address and the route it came by. Two peers
/// with one address and different routes are two peers; nothing looks a peer
/// up by its address alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Peer {
    address: SocketAddr,
    route: Route,
}

impl Peer {
    /// A peer that reached a public socket itself.
    pub fn direct(address: SocketAddr) -> Self {
        Self {
            address,
            route: Route::Direct,
        }
    }

    /// A peer a configured relay names (its PROXY header's address).
    pub fn relayed(address: SocketAddrV4, relay: Relay) -> Self {
        Self {
            address: SocketAddr::V4(address),
            route: Route::Relay(relay),
        }
    }

    /// The client's own address: the datagram's source, or the one its relay
    /// observed. For limits, bans and logs; never a key by itself.
    pub fn address(self) -> SocketAddr {
        self.address
    }

    /// [`Self::address`]'s IP.
    pub fn ip(self) -> IpAddr {
        self.address.ip()
    }

    pub fn route(self) -> Route {
        self.route
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.route {
            Route::Direct => write!(f, "{}", self.address),
            Route::Relay(relay) => write!(f, "{} via relay {}", self.address, relay.public),
        }
    }
}

/// tunnel IPv4 = advertised IPv4, comma-separated. Fail closed on bad config.
/// A tunnel address is a private one (the WireGuard link's), or, for a relay
/// on the same machine (the end-to-end test's), a loopback one advertised at
/// a loopback address: no address an Internet source can have.
#[derive(Clone, Debug, Default)]
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
            let local = tunnel.is_loopback() && public.is_loopback();
            if !(tunnel.is_private() || local)
                || !unicast(public)
                || peers.insert(tunnel, public).is_some()
            {
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

    /// The configured relay whose tunnel address `tunnel` is.
    pub fn relay(&self, tunnel: Ipv4Addr) -> Option<Relay> {
        self.0
            .get(&tunnel)
            .map(|&public| Relay { tunnel, public })
    }

    /// The relay a socket's source address is, if it is one's tunnel address.
    fn source(&self, address: SocketAddr) -> Option<Relay> {
        match address.ip().to_canonical() {
            IpAddr::V4(ip) => self.relay(ip),
            IpAddr::V6(_) => None,
        }
    }

    /// Consume exactly one TCP PROXY header from a trusted tunnel peer.
    pub fn accept(&self, stream: &mut TcpStream) -> io::Result<Peer> {
        let address = stream.peer_addr()?;
        let Some(relay) = self.source(address) else {
            return Ok(Peer::direct(address));
        };
        let timeout = stream.read_timeout()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut header = [0; HEADER];
        let read = stream.read_exact(&mut header);
        stream.set_read_timeout(timeout)?;
        read?;
        let address = decode(&header, 0x11, relay.public, stream.local_addr()?.port())?;
        Ok(Peer::relayed(address, relay))
    }
}

fn unicast(ip: Ipv4Addr) -> bool {
    !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast()
}

fn bad_header() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid relay PROXY v2 header")
}

fn decode(bytes: &[u8], protocol: u8, public: Ipv4Addr, port: u16) -> io::Result<SocketAddrV4> {
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
    Ok(SocketAddrV4::new(ip, port))
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

/// A socket's receive and send by peer. It keeps nothing about a client:
/// only where each configured relay's forwarder sends from now (its port
/// changes when it starts again), which that relay's own datagrams set.
pub struct Datagrams<'a> {
    socket: &'a UdpSocket,
    relays: Relays,
    /// By tunnel address: one entry a configured relay at most.
    forwarders: HashMap<Ipv4Addr, SocketAddr>,
    buffer: Box<[u8; 65536]>,
    port: u16,
}

impl<'a> Datagrams<'a> {
    pub fn new(socket: &'a UdpSocket, relays: Relays) -> io::Result<Self> {
        Ok(Self {
            socket,
            relays,
            forwarders: HashMap::new(),
            buffer: Box::new([0; 65536]),
            port: socket.local_addr()?.port(),
        })
    }

    pub fn recv_from(&mut self, output: &mut [u8]) -> io::Result<(usize, Peer)> {
        let (len, source) = self.socket.recv_from(&mut self.buffer[..])?;
        let (offset, peer) = if let Some(relay) = self.relays.source(source) {
            let address = decode(&self.buffer[..len], 0x12, relay.public, self.port)
                .map_err(|_| dropped())?;
            self.forwarders.insert(relay.tunnel, source);
            (HEADER, Peer::relayed(address, relay))
        } else {
            // A forwarding header means something only from a relay's tunnel
            // address; from anyone else it is no game datagram either.
            if self.buffer[..len].starts_with(SIGNATURE) {
                return Err(dropped());
            }
            (0, Peer::direct(source))
        };
        let payload = &self.buffer[offset..len];
        if is_probe(payload) {
            // An answer that cannot be sent (a source port 0, no route back)
            // is the prober's loss, never this socket's failure.
            let _ = self.send_to(payload, peer);
            return Err(dropped());
        }
        if payload.len() > output.len() {
            return Err(dropped());
        }
        output[..payload.len()].copy_from_slice(payload);
        Ok((payload.len(), peer))
    }

    /// `bytes` to `peer`, by the route it came by.
    pub fn send_to(&self, bytes: &[u8], peer: Peer) -> io::Result<usize> {
        match peer.route {
            Route::Direct => self.socket.send_to(bytes, peer.address),
            Route::Relay(relay) => {
                // A relay nothing came from yet has no forwarder to answer.
                let forwarder = self.forwarders.get(&relay.tunnel).ok_or_else(dropped)?;
                let mut packet = Vec::with_capacity(HEADER + bytes.len());
                packet.extend_from_slice(&encode(peer.address, relay.public, self.port));
                packet.extend_from_slice(bytes);
                self.socket
                    .send_to(&packet, forwarder)
                    .map(|n| n.saturating_sub(HEADER))
            }
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

    const PUBLIC: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 8);

    fn one_relay() -> (Relays, Relay) {
        let relays = Relays(HashMap::from([(Ipv4Addr::LOCALHOST, PUBLIC)]));
        let relay = relays.relay(Ipv4Addr::LOCALHOST).unwrap();
        (relays, relay)
    }

    fn v4(address: &str) -> SocketAddrV4 {
        address.parse().unwrap()
    }

    /// A socket with a second's patience, for what must arrive.
    fn patient() -> UdpSocket {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        socket
    }

    fn forwarded(client: SocketAddrV4, port: u16, payload: &[u8]) -> Vec<u8> {
        let mut packet = encode(SocketAddr::V4(client), PUBLIC, port).to_vec();
        packet.extend(payload);
        packet
    }

    #[test]
    fn tcp_header_is_consumed_once_and_the_encrypted_stream_is_untouched() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = v4("198.51.100.9:45000");
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                let mut bytes = encode(SocketAddr::V4(client), PUBLIC, port);
                bytes[13] = 0x11;
                for byte in bytes {
                    stream.write_all(&[byte]).unwrap();
                }
                stream.write_all(b"cipher").unwrap();
            });
            let (mut stream, _) = listener.accept().unwrap();
            let (relays, relay) = one_relay();
            assert_eq!(
                relays.accept(&mut stream).unwrap(),
                Peer::relayed(client, relay)
            );
            let mut rest = [0; 6];
            stream.read_exact(&mut rest).unwrap();
            assert_eq!(&rest, b"cipher");
        });
    }

    #[test]
    fn reachability_echo_has_no_transport_side_effects() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client = patient();
        let mut socket = Datagrams::new(&server, Relays::default()).unwrap();
        let mut probe = PROBE_PREFIX.to_vec();
        probe.resize(crate::MAX_DATAGRAM, 7);
        client
            .send_to(&probe, server.local_addr().unwrap())
            .unwrap();
        // Answered whole, whatever the size of the caller's buffer.
        let mut buffer = [0; 64];
        assert_eq!(
            socket.recv_from(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let mut answer = [0; 2048];
        let (n, _) = client.recv_from(&mut answer).unwrap();
        assert_eq!(&answer[..n], probe);
        assert!(socket.forwarders.is_empty());
        assert!(is_probe(&probe[..22]));
        assert!(!is_probe(&probe[1..]));
        probe.push(7);
        assert!(!is_probe(&probe));
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
    fn a_tunnel_address_is_private_or_a_local_relays_loopback() {
        assert!(Relays::parse("10.77.0.2=203.0.113.8").is_ok());
        assert!(Relays::parse("127.0.0.1=127.0.0.2").is_ok());
        // A loopback tunnel advertised at a public address, a public tunnel,
        // the same tunnel twice: refused.
        for bad in [
            "127.0.0.1=203.0.113.8",
            "203.0.113.9=203.0.113.8",
            "10.77.0.2=203.0.113.8,10.77.0.2=203.0.113.9",
            "10.77.0.2=0.0.0.0",
        ] {
            assert!(Relays::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_relay_is_only_one_the_setting_names() {
        let relays = Relays::parse("10.77.0.2=203.0.113.8").unwrap();
        let relay = relays.relay("10.77.0.2".parse().unwrap()).unwrap();
        assert_eq!(relay.public(), PUBLIC);
        assert_eq!(relays.relay("10.77.0.3".parse().unwrap()), None);
        // The advertised address is no tunnel address: nothing public names a relay.
        assert_eq!(relays.relay(PUBLIC), None);
        assert_eq!(relays.source("203.0.113.8:1235".parse().unwrap()), None);
        assert_eq!(
            relays.source("[::ffff:10.77.0.2]:40000".parse().unwrap()),
            Some(relay)
        );
    }

    #[test]
    fn every_header_byte_is_checked_and_truncation_is_refused() {
        let client = v4("198.51.100.4:54321");
        let packet = encode(SocketAddr::V4(client), PUBLIC, 1236);
        assert_eq!(decode(&packet, 0x12, PUBLIC, 1236).unwrap(), client);
        for len in 0..HEADER {
            assert!(decode(&packet[..len], 0x12, PUBLIC, 1236).is_err());
        }
        for i in (0..16).chain(20..24).chain(26..28) {
            let mut bad = packet;
            bad[i] ^= 1;
            assert!(decode(&bad, 0x12, PUBLIC, 1236).is_err(), "{i}");
        }
        assert!(decode(&packet, 0x11, PUBLIC, 1236).is_err());
        assert!(decode(&packet, 0x12, PUBLIC, 1237).is_err());
    }

    #[test]
    fn udp_preserves_two_clients_and_returns_only_through_the_relay() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let forwarder = patient();
        let (relays, relay) = one_relay();
        let mut socket = Datagrams::new(&server, relays).unwrap();
        let mut buffer = [0; 2048];
        for client in ["198.51.100.4:54321", "198.51.100.5:54321"] {
            let client = v4(client);
            forwarder
                .send_to(
                    &forwarded(client, socket.port, b"hello"),
                    server.local_addr().unwrap(),
                )
                .unwrap();
            let (n, peer) = socket.recv_from(&mut buffer).unwrap();
            assert_eq!(peer, Peer::relayed(client, relay));
            assert_eq!(&buffer[..n], b"hello");
            socket.send_to(b"answer", peer).unwrap();
            let (n, _) = forwarder.recv_from(&mut buffer).unwrap();
            assert_eq!(
                decode(&buffer[..n], 0x12, PUBLIC, socket.port).unwrap(),
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
        assert_eq!(peer, Peer::direct(client.local_addr().unwrap()));
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

    /// A server socket whose one relay has the tunnel address 127.0.0.2, so
    /// that a socket on 127.0.0.1 is a direct client and one on 127.0.0.2 the
    /// relay's forwarder. None on a host without a second loopback address.
    struct TwoRoutes {
        server: UdpSocket,
        relays: Relays,
        relay: Relay,
        forwarder: UdpSocket,
        near: UdpSocket,
    }

    fn two_routes() -> Option<TwoRoutes> {
        let tunnel = Ipv4Addr::new(127, 0, 0, 2);
        let forwarder = UdpSocket::bind((tunnel, 0)).ok()?;
        forwarder
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let relays = Relays(HashMap::from([(tunnel, PUBLIC)]));
        Some(TwoRoutes {
            server: UdpSocket::bind("127.0.0.1:0").unwrap(),
            relay: relays.relay(tunnel).unwrap(),
            relays,
            forwarder,
            near: patient(),
        })
    }

    #[test]
    fn one_address_by_two_routes_is_two_peers_and_each_is_answered_by_its_own() {
        let Some(TwoRoutes {
            server,
            relays,
            relay,
            forwarder,
            near,
        }) = two_routes()
        else {
            return;
        };
        let at = server.local_addr().unwrap();
        let mut socket = Datagrams::new(&server, relays).unwrap();
        let mut buffer = [0; 2048];
        let address = near.local_addr().unwrap();
        let SocketAddr::V4(named) = address else {
            unreachable!("bound to 127.0.0.1")
        };

        // The direct one first, then a forwarded datagram that names its address.
        near.send_to(b"direct", at).unwrap();
        let (_, direct) = socket.recv_from(&mut buffer).unwrap();
        assert_eq!(direct, Peer::direct(address));
        forwarder
            .send_to(&forwarded(named, socket.port, b"claim"), at)
            .unwrap();
        let (n, relayed) = socket.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..n], b"claim");
        assert_eq!(relayed, Peer::relayed(named, relay));
        assert_ne!(direct, relayed);

        // The direct one is still heard as itself, and answered directly.
        near.send_to(b"still here", at).unwrap();
        let (n, again) = socket.recv_from(&mut buffer).unwrap();
        assert_eq!((&buffer[..n], again), (&b"still here"[..], direct));
        socket.send_to(b"to direct", direct).unwrap();
        let (n, from) = near.recv_from(&mut buffer).unwrap();
        assert_eq!((&buffer[..n], from), (&b"to direct"[..], at));

        // The relayed one's answer leaves through the relay, with its header.
        socket.send_to(b"to relayed", relayed).unwrap();
        let (n, _) = forwarder.recv_from(&mut buffer).unwrap();
        assert_eq!(
            decode(&buffer[..n], 0x12, PUBLIC, socket.port).unwrap(),
            named
        );
        assert_eq!(&buffer[HEADER..n], b"to relayed");
        // Nothing of it reached the address itself.
        near.set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        assert!(near.recv_from(&mut buffer).is_err());
    }

    #[test]
    fn an_address_named_through_the_relay_first_is_still_itself_when_it_speaks() {
        let Some(TwoRoutes {
            server,
            relays,
            relay,
            forwarder,
            near,
        }) = two_routes()
        else {
            return;
        };
        let at = server.local_addr().unwrap();
        let mut socket = Datagrams::new(&server, relays).unwrap();
        let mut buffer = [0; 2048];
        let address = near.local_addr().unwrap();
        let SocketAddr::V4(named) = address else {
            unreachable!("bound to 127.0.0.1")
        };
        for _ in 0..3 {
            forwarder
                .send_to(&forwarded(named, socket.port, b"x"), at)
                .unwrap();
            let (_, peer) = socket.recv_from(&mut buffer).unwrap();
            assert_eq!(peer, Peer::relayed(named, relay));
        }
        near.send_to(b"first words", at).unwrap();
        let (n, peer) = socket.recv_from(&mut buffer).unwrap();
        assert_eq!((&buffer[..n], peer), (&b"first words"[..], Peer::direct(address)));
        socket.send_to(b"answer", peer).unwrap();
        let (n, from) = near.recv_from(&mut buffer).unwrap();
        assert_eq!((&buffer[..n], from), (&b"answer"[..], at));
        forwarder
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        assert!(forwarder.recv_from(&mut buffer).is_err());
    }

    #[test]
    fn the_socket_keeps_one_entry_a_relay_however_many_clients_it_names() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = server.local_addr().unwrap();
        let (relays, relay) = one_relay();
        let mut socket = Datagrams::new(&server, relays).unwrap();
        let forwarder = patient();
        let mut buffer = [0; 64];
        for port in 1..=2000_u16 {
            let client = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), port);
            forwarder
                .send_to(&forwarded(client, socket.port, b"x"), at)
                .unwrap();
            let (_, peer) = socket.recv_from(&mut buffer).unwrap();
            assert_eq!(peer, Peer::relayed(client, relay));
        }
        assert_eq!(socket.forwarders.len(), 1);
    }

    #[test]
    fn a_relay_started_again_is_answered_at_its_new_port() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let at = server.local_addr().unwrap();
        let (relays, relay) = one_relay();
        let mut socket = Datagrams::new(&server, relays).unwrap();
        let client = v4("198.51.100.4:54321");
        let peer = Peer::relayed(client, relay);
        let mut buffer = [0; 2048];
        // Nothing came from the relay yet: no forwarder to answer.
        assert_eq!(
            socket.send_to(b"early", peer).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        for _ in 0..2 {
            let forwarder = patient();
            forwarder
                .send_to(&forwarded(client, socket.port, b"hello"), at)
                .unwrap();
            assert_eq!(socket.recv_from(&mut buffer).unwrap().1, peer);
            socket.send_to(b"answer", peer).unwrap();
            let (n, _) = forwarder.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[HEADER..n], b"answer");
        }
    }

    #[test]
    fn a_peer_says_its_route() {
        let (_, relay) = one_relay();
        let address = v4("198.51.100.4:54321");
        assert_eq!(
            Peer::direct(SocketAddr::V4(address)).to_string(),
            "198.51.100.4:54321"
        );
        assert_eq!(
            Peer::relayed(address, relay).to_string(),
            "198.51.100.4:54321 via relay 203.0.113.8"
        );
        assert_eq!(Peer::relayed(address, relay).ip(), IpAddr::V4(*address.ip()));
        assert_eq!(Peer::relayed(address, relay).route(), Route::Relay(relay));
    }
}
