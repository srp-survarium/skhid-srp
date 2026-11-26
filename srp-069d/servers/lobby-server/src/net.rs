//! UDP socket loop around [`Server`]: one thread, 10 ms tick.

use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::Server;

/// Run until `stop` is set. The socket must be bound; a short read timeout is set here.
pub fn serve(server: &mut Server, socket: &UdpSocket, stop: &AtomicBool) -> std::io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(10)))?;
    let mut buf = vec![0u8; 65536];
    while !stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => server.receive(addr, &buf[..n], Instant::now()),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            // Windows: an earlier datagram to a closed client port came back as ICMP unreachable; not fatal
            Err(e) if e.kind() == ErrorKind::ConnectionReset => {}
            Err(e) => return Err(e),
        }
        for (addr, d) in server.poll(Instant::now()) {
            if let Err(e) = socket.send_to(&d, addr) {
                log::debug!("{addr}: send failed: {e}");
            }
        }
    }
    Ok(())
}
