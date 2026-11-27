//! UDP driver: one socket, one thread, the [`Hub`] behind a mutex. The lobby calls `create_match` / `end_match` from its
//! own threads; match ends are delivered to the `on_end` callback on the driver thread.

use crate::hub::{Hub, HubConfig, MatchTicket};
use crate::relay::{CreateError, MatchEnd, MatchSpec, MatchStats};
use crate::world::WorldStore;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct MatchServer {
    hub: Arc<Mutex<Hub>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    local: SocketAddr,
}

impl MatchServer {
    /// Bind `bind` (port 0 = any) and start the driver thread. `on_end` is called for every finished match.
    pub fn start(
        bind: SocketAddr,
        cfg: HubConfig,
        worlds: WorldStore,
        on_end: impl FnMut(MatchEnd) + Send + 'static,
    ) -> io::Result<Self> {
        Self::start_with_hub(bind, Arc::new(Mutex::new(Hub::new(cfg, worlds))), on_end)
    }

    /// Like [`start`](Self::start) over a hub the caller keeps a handle to (the lobby's online hook creates matches
    /// through it). The hub's port is set from the bound socket.
    pub fn start_with_hub(
        bind: SocketAddr,
        hub: Arc<Mutex<Hub>>,
        mut on_end: impl FnMut(MatchEnd) + Send + 'static,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        socket.set_read_timeout(Some(Duration::from_millis(2)))?;
        let local = socket.local_addr()?;
        hub.lock().unwrap().set_port(local.port());
        let stop = Arc::new(AtomicBool::new(false));
        let (h, s) = (hub.clone(), stop.clone());
        let thread = std::thread::Builder::new().name("match-server".into()).spawn(move || {
            let mut buf = [0u8; 2048];
            while !s.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buf) {
                    Ok((n, from)) => {
                        let mut hub = h.lock().unwrap();
                        hub.on_datagram(from, &buf[..n], Instant::now());
                    }
                    Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
                    Err(e) => log::debug!("recv: {e}"), // e.g. ICMP port unreachable on Windows
                }
                let (out, ended) = {
                    let mut hub = h.lock().unwrap();
                    let out = hub.poll(Instant::now());
                    (out, hub.take_ended())
                };
                for (to, d) in out {
                    let _ = socket.send_to(&d, to);
                }
                for e in ended {
                    on_end(e);
                }
            }
        })?;
        Ok(Self { hub, stop, thread: Some(thread), local })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// A fresh match id for a new match's options ([`Hub::new_match_id`]).
    pub fn new_match_id(&self) -> u32 {
        self.hub.lock().unwrap().new_match_id()
    }

    pub fn create_match(&self, spec: &MatchSpec) -> Result<MatchTicket, CreateError> {
        self.hub.lock().unwrap().create_match(spec, Instant::now())
    }

    pub fn set_winner(&self, match_id: u32, team: u8) -> bool {
        self.hub.lock().unwrap().set_winner(match_id, team)
    }

    pub fn end_match(&self, match_id: u32) -> bool {
        self.hub.lock().unwrap().end_match(match_id, Instant::now())
    }

    pub fn stats(&self, match_id: u32) -> Option<MatchStats> {
        self.hub.lock().unwrap().stats(match_id)
    }

    /// Stop the driver thread (joins it).
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MatchServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
